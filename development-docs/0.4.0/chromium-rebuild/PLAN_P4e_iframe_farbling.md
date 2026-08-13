# PLAN — P4e (iframe half): deliver the top-frame farbling key to subframes

> **Status:** drafted 2026-08-13, during the `v0.4.0-beta.1` CI build. Scope is the **iframe half only**.
> The worker half of P4e (all workers, in-process included) stays deferred and is planned separately.
>
> **Owner decision that produced this doc:** the 2026-08-09 deferral was taken on the understanding
> that the gap was "OOP workers + cross-site iframes". Two measurements since have widened it, and the
> code reading in §2 suggests it is wider still. The owner re-opened the decision on 2026-08-13 and
> asked for the iframe half to start immediately. **Nothing is promoted until this lands and is tested.**

---

## 1. Why this is not just a third-party-tracker gap

Brave — the implementation our design is modeled on — applies its fingerprinting defenses **in both
first- and third-party contexts**, keyed on the **top / eTLD+1 origin but applied to all third parties
on the page**, precisely because third-party frames colluding with the first party is the common case.
That is the model `PLAN_farbling_blink.md` specifies for us (I4, top-frame keying). We do not yet meet it.

But §2 is the reason this moved from "known gap" to "start immediately".

---

## 2. ⛔ FINDING — the gate is on *every* subframe, not just cross-site ones

**Claim:** every iframe is unfarbled — same-site and same-origin included — and therefore **any
first-party page can read native, unfarbled values through a same-origin child frame.** If so, farbling
is defeatable by roughly one line of JavaScript on the page it is meant to protect against.

**Status: reasoned from code, NOT yet measured. Step 0 below measures it before any patch work.**

### The evidence chain

| Step | Symbol | What it does |
|---|---|---|
| 1 | `simple_handler.cpp :: OnBeforeBrowse` | Computes `domain_key = HMAC(profile_seed, registrable_domain)` and sends `hodos_farble_key`. Gated `if (frame->IsMain() && …)` — **main frames only**. |
| 2 | `CefFrameHostImpl::SendProcessMessage` → `hodos::FarblingRegistry::Set` | Files `{key, enabled}` in the **browser** under the registrable domain. |
| 3 | `CefFrameImpl::MaybeApplyHodosFarblingKey` | Renderer PULLs at `OnContextCreated`. **`if (frame_->Parent() != nullptr) { return; }`** — bails for *every* subframe, with the comment "Subframes and workers are P4e's scope". |
| 4 | `blink_glue::SetHodosFarblingKey` | Installs the key on `local_frame->DomWindow()` — **that frame's own `LocalDOMWindow`**. |
| 5 | C3/C4/C5/C6 patches | Read `HodosSessionCache::From(*execution_context)` of the **canvas's / navigator's own** context. `HodosSessionCache` is fail-closed by construction: no key ⇒ `FarblingEnabled() == false` ⇒ native value. |

Each frame has its own `LocalDOMWindow`, so it needs its own key. Step 3 denies it to all of them, so
steps 4–5 fail closed in every subframe regardless of site relationship.

### Why that is a bypass and not merely a gap

A **same-origin** iframe is fully scriptable from its parent. So a page can do the equivalent of:

```js
const f = document.createElement('iframe');   // about:blank — same origin, inherits
document.body.appendChild(f);
// f.contentWindow's canvas / WebGL / AudioContext / navigator are UNFARBLED
```

and read the machine's real values while the top document is farbled. Cross-site iframes are the
*tracker* problem; same-origin iframes are a **correctness** problem — they make the protection
opt-out-able by the adversary.

⚠️ **The non-HTTP scheme gate compounds it.** `MaybeApplyHodosFarblingKey` returns early unless
`url.SchemeIsHTTPOrHTTPS()`. `about:blank`, `srcdoc`, `blob:` and `data:` frames **inherit their
parent's origin** but have no HTTP URL of their own, so a naive fix that only removes the `Parent()`
check would still leave exactly the bypass frame above unfarbled. §4 D3 handles this explicitly.

### Consequence for the documented scope line

If Step 0 confirms it, the release-note scope line **"main frame + same-site frames only"** is wrong and
must become **"main frame only"** until this lands. `FARBLING_COMPLETION_PLAN.md` and
`MAC_WINDOWS_RELAY.md` both carry the old wording and need correcting.

---

## 3. Step 0 — measure it first (mandatory, before any patch)

Per the CLAUDE.md negative-control rule: *a test that has never been seen to fail has not been shown to
test anything*, and *assert the test is measuring the intended **subject***.

Extend `chromium-rebuild/farbling_iframe_check.py`, which already implements the correct **three-way
diagnostic** (`different` = keying live · `== native` = coverage gap · `== farbled top` = wrong model).
Add two rows:

| Row | Subject | Expected today | Expected after the fix |
|---|---|---|---|
| **S1** | same-**origin** `about:blank` child frame, read via `contentWindow` from the parent | `== native` (the bypass) | `==` parent's farbled value |
| **S2** | same-**site**, cross-origin child (`a.example.com` under `example.com`) | `== native` | `==` parent's farbled value |
| **S3** *(exists)* | cross-site child under two different first parties | `== native` | **different** between the two parents |

**Negative control for each:** turn the feature off (per-site Privacy Shield off, or an auth-exempt top
frame) and confirm the row goes red — i.e. parent and child converge on native and the "different"
assertion for S3 fails. A row that cannot be made to fail is not measuring anything.

**Subject controls (the trap that has bitten us three times):** the iframe harness already takes its
measurement from the frame's own CDP target rather than the parent's isolated world — keep that. Do not
drive Hodos's header or an overlay by mistake; they are separate CEF browsers that CDP reports as
`type:"page"`. And keep the existing oversize-canvas invariants (≥65536 px, ≥262144 B `readPixels`),
which sit outside the farbling size gates and must be identical everywhere — if one of those moves,
nothing else in the run is comparable.

Step 0 runs against the **current** engine — no rebuild needed. Budget: ~half a day.

---

## 4. Design

### D1 — Resolve the top frame **in the browser**, not the renderer ✅ recommended

`CefBrowserFrame` is constructed with a `content::RenderFrameHost*`, so the browser can answer
authoritatively:

```
render_frame_host->GetMainFrame()->GetLastCommittedURL().host()
```

and feed *that* host to `FarblingRegistry::Lookup`. The `host` argument the renderer passes becomes
advisory (keep it for logging a mismatch; do not trust it).

**Why this and not the renderer-side alternative.** The obvious alternative is to have the subframe read
its own top-level site from `ExecutionContext::GetStorageKey().TopLevelSite()`, which *is* plumbed to
subframe renderers for storage partitioning. Rejected for two reasons:

1. **It re-derives the registrable domain.** `hodos_farbling_registry.h` carries an explicit ⚠️: the
   authoritative reduction is `FarblingPolicy::RegistrableDomainFromUrl` in the shell, deliberately
   hand-rolled, and *"if this side reduced independently the two could disagree and the lookup would
   miss"* — failing closed, silently. `StorageKey`'s site comes from `net::registry_controlled_domains`,
   i.e. exactly the independent reduction that header forbids.
2. **It trusts the renderer** about which site frames it. Browser-side resolution cannot be lied to.

D1 also **unifies the two paths**: a main frame's `GetMainFrame()` is itself, so the same code serves
both and the main-frame behavior is provably unchanged.

### D2 — Drop the `Parent() != nullptr` early return

That is the whole of the renderer-side change, given D1.

### D3 — Origin-inheriting schemes must not fall through the scheme gate

The `SchemeIsHTTPOrHTTPS()` check exists to keep a blocking sync IPC off `about:blank`, the initial
empty document, `devtools://` and our own `127.0.0.1:5137` UI. With subframes in scope it must be
rewritten as: **the top frame's** URL decides eligibility, not the subframe's. A subframe whose own URL
is `about:blank` / `srcdoc` / `blob:` / `data:` under an HTTP(S) top frame **must** get the top frame's
key — that is the §2 bypass frame. The internal-UI skip stays, keyed on the **top** frame's host.

⚠️ Keep the initial empty document out of scope where possible: it exists transiently for every frame
before its real load commits, and pulling for it wastes a round trip. Prefer gating on the top frame
being a committed HTTP(S) document.

### D4 — Perf: this converts 1 sync IPC per page into 1 per frame

`MaybeApplyHodosFarblingKey`'s comment is explicit that skipping subframes *"keeps this to ONE sync
browser round-trip per top-level document rather than one per frame on an iframe-heavy page."* An
ad-heavy page can carry 20–50 frames, each firing a **blocking** sync call on the first-paint path.

**Mitigation:** memoize in the renderer process, keyed by the **top frame's frame token**, so N frames
sharing a top frame cost one round trip. Invalidate on the top frame's next commit (a Privacy Shield
toggle must take effect on the next navigation — the registry's "newest verdict wins" contract).

**Gate:** re-run `farbling_perf_check.py`, which P6 reshaped to **absolute µs**. Budget the delta
explicitly; do not accept "looks fine". If the memo cannot hold the regression, fall back to pulling
only for frames whose top frame has a registry entry.

### D5 — Exemption inherits from the top frame, for free

The `enabled` bit is filed by the **top frame's** navigation, so an auth-exempt top frame yields
`enabled=false` for every subframe automatically. This is what Q3 §2.1 requires (Cloudflare Turnstile
in an iframe on an exempt login page must see native values) and what R2 warned would break if the
payload were keyed per-renderer-origin. D1 satisfies it by construction.

---

## 5. Out of scope

**All workers** — dedicated (in-process, measured unfarbled 2026-08-10), shared, and service. The only
installer is `SetHodosFarblingKey(blink::WebLocalFrame*, …)` and a worker is not a frame, so workers need
a separate install point at worker startup plus, for shared/service workers, cross-process delivery keyed
by registration scope. Tracked as **P4e-workers**, unchanged.

Landing the iframe half **does not** let us claim worker coverage, and the window-vs-worker mismatch
(itself a fingerprinting signal — CreepJS compares those columns directly) remains until it lands.

---

## 6. Test plan

| # | Test | Negative control |
|---|---|---|
| T1 | S1/S2/S3 from §3 all green | feature off ⇒ all three converge to native and S3's difference assertion fails |
| T2 | Main-frame behavior **unchanged** — full acceptance battery re-run (7/7 incl. BOT-1, T8 persistence) | `--negative-control` inverts |
| T3 | **Seed rotation A→B→A** across the whole set — the standing release gate, `farbling_seed_rotation_check.py` | its built-in `--negative-control` mode; must go RED on the discriminating rows |
| T4 | Exempt top frame ⇒ subframes native (D5); Turnstile-style third-party widget on an exempt login page still works | remove the exemption ⇒ subframe farbles |
| T5 | Perf gate, absolute µs, on a deliberately iframe-heavy page | compare against the pre-change number, recorded first |
| T6 | Regression basket 10/10 + soak (140 loads / 0 crashes bar) | crash detectors positive-controlled, as in the CEF-150 baseline |

Windows **and** macOS, per the parity standard. Mac runs T1–T6 against the same fork pin.

---

## 7. Sequencing and cost

1. **Step 0 measurement** — current engine, no rebuild. ~½ day. **Gates everything else**: if S1 comes
   back farbled, §2's bypass claim is wrong and the whole plan shrinks to the cross-site case.
2. **Patch** D1+D2+D3+D4 — small, three files (`browser_frame.cc`, `frame_impl.cc`, plus the memo).
   Note this is **libcef only, not a Chromium patch** — it touches no `patch/patches/*.patch`, so it
   costs no rebase surface at the next Chromium bump. ~1 day.
3. **Build** — ~5 h wall clock (measured 4h49m). **Batch every change into one build**; this dominates
   the calendar, not the typing.
4. **T1–T6 on Windows**, then hand the pin to Mac for parity. ~1 day.
5. **beta.2** carries it.

**Realistic total: 3–4 days elapsed**, dominated by one build cycle and two test passes. The estimate is
now grounded in the actual code rather than the earlier inference, and the iframe half is confirmed
*not* to need the worker-class cross-process machinery.

---

## 8. Risks

| Risk | Handling |
|---|---|
| **Perf regression on iframe-heavy pages** (D4) — the one change that could hurt every user | absolute-µs gate, memo by top-frame token, fallback to entry-gated pulling |
| **Wrong-model outcome** — keying on the iframe's own origin instead of the top frame's; *looks* like success | the §3 three-way diagnostic names this outcome explicitly; D1 makes it structurally impossible |
| Registrable-domain reductions disagreeing | D1 avoids re-deriving entirely |
| Breaking main-frame behavior while touching shared code | T2 re-runs the full battery; D1 keeps main frames on the identical path |
| Sync IPC on transient/empty documents | D3 gates on the **top** frame being a committed HTTP(S) document |
| Build-cycle churn | batch all four design items into one build |
