# Q3 — Farbling × OAuth / pre-approved auth sites: re-implementing the auth-domain exemption at source

**Created:** 2026-07-10 · **Owner:** Matthew (Marston Enterprises) · **Lead:** Windows Claude
**Status:** DETAILED PLAN (expands `CHROMIUM_CEF_BUILD_DESIGN_OUTLINE.md` §3c C7 + §6 Q3). Research + design only — **NO code, NO builds.**
**Answers:** Today JS-farbling **skips auth/OAuth domains** so logins and bot-detection don't break. When farbling moves into Blink (native, below JS), that exemption must be re-implemented **at source**. Where does the exempt list live, how is the per-navigation decision made, how are subframes / eTLD+1 handled, what happens to `hodos-unbreak.txt`, and should the exemption also gate scriptlets? Plus the concrete plan + acceptance so auth sites still log in.

**Cross-refs:** `CHROMIUM_CEF_BUILD_DESIGN_OUTLINE.md` §3c (FEAT-B1 / C1–C7 patch set; C2 seed channel; M1 teardown; I1/I2/I4), §4 (P4d/P6 acceptance), §6 Q2/Q3; `Q2_farbling_adblock.md` (TP-2 flags `IsSiteEnabled` re-homing as an **unresolved gap this plan must close**); `B1-farbling-design.md`; `DevOps-CICD/research/BRAVE_FORK_FEASIBILITY.md` §B1. `PLAN_farbling_blink.md` (the standalone C1–C7 patch plan) is **planned, not-yet-written** — until it lands, outline §3c is source-of-truth for C1–C6 and the seed model; **this doc owns C7**. Code anchors verified current 2026-07-10.

---

## TL;DR — VERDICT: move the *membership test* into the browser process; deliver ONE per-navigation farble on/off bit to the renderer; key it on the TOP-FRAME eTLD+1

Today the exemption is a **three-input decision** computed in the **browser process** on every main-frame navigation (`simple_handler.cpp::OnBeforeBrowse:7484-7521`):

```
farble THIS navigation  ⇔  GlobalEnabled  AND  !IsAuthDomain(url)  AND  IsSiteEnabled(host)
```

…and then **re-checked (and, for subframes, solely enforced)** in the renderer at injection time (`simple_render_process_handler.cpp:585`). The browser-side decision above fires **only for `frame->IsMain()`** (`simple_handler.cpp:7485`); the renderer's `IsAuthDomain(url)` at `:585` runs **per-frame** and is therefore the *only* auth check that covers subframes — not purely redundant. Only **`IsAuthDomain`** is the "auth/OAuth exemption." `IsSiteEnabled` is the **user's per-site Privacy-Shield toggle** (a shipped feature, backed by `fingerprint_settings.json`). `hodos-unbreak.txt` is **not** farbling at all — it is the adblock **scriptlet** exemption and is **untouched by this sprint (I1)**.

**Recommendation for the Blink migration (C7):**
1. **Keep the allowlist in the SHELL browser process (C++), not in a Blink patch.** The list changes ~monthly; a shell rebuild is cheap, a Chromium rebuild is not. The renderer never sees the list.
2. **Collapse the three inputs into ONE browser-process function** `ShouldFarble(top_frame_registrable_domain) → bool`, evaluated per **top-level navigation**. This is where the `IsSiteEnabled` user toggle that `Q2` TP-2 flagged as "unresolved / farbling-plan owns re-homing" **lands** — C7 owns re-homing it (scope note in §3).
3. **Key the decision on the TOP-FRAME eTLD+1**, and deliver the resulting **single boolean** (`farble_enabled`) to the renderer(s) **alongside the C2 seed material**, for *every* frame in the page (incl. cross-site OOP iframes). When `false`, `HodosSessionCache` returns **native pass-through** for all patched APIs.
4. **This structurally removes the known Turnstile parent/iframe *inconsistency* failure mode** the current code hand-patches (whatsonchain triple-listing + CAPTCHA-widget hosts). Because parent and all subframes share one decision **and one seed**, the fingerprint is internally consistent — which is *necessary* to pass, though not proven *sufficient* (§2.5). The pure-widget host entries (`challenges.cloudflare.com`, `www.gstatic.com`, `hcaptcha.com`, …) become **candidates for trimming — gated on a live CAPTCHA pass and reverted on failure** (§5 OQ2).
5. **Do NOT couple the exemption to the scriptlet layer.** Farbling-exempt gates farbling only. Scriptlet exemption stays with `hodos-unbreak.txt` (Q2-2 default: keep split).

Net: **one list, one decision function, one delivered bit, top-frame keyed** — simpler and *more* correct than today's dual-checked / one-shot / exact-host scheme.

---

## 1. What the exemption actually is today (3 layers, only 1 is "auth farbling")

| Layer | Mechanism | Owns | Code anchor (verified 2026-07-10) | This sprint |
|---|---|---|---|---|
| **A. Auth/OAuth farbling exemption** | `FingerprintProtection::IsAuthDomain(url)` — hardcoded C++ allowlist, ~37 entries, **exact host match** on `ExtractDomain(url)` | Farbling OFF for auth/CAPTCHA/bank/e-comm hosts | `FingerprintProtection.h:189-270` | **C7 re-implements THIS** |
| **B. User per-site toggle** | `IsSiteEnabled(domain)` / `SetSiteEnabled` — Privacy-Shield on/off per site, persisted to `fingerprint_settings.json` (`siteOverrides_`) | User-chosen farbling OFF for a site | `FingerprintProtection.h:123-187`; IPC `fingerprint_get/set_site_enabled` `simple_handler.cpp:6191/6210` | **C7 must RE-HOME (Q2 TP-2 gap)** |
| **C. Adblock scriptlet exemption** | `hodos-unbreak.txt` blanket `#@#+js()` per domain | Scriptlet injection OFF for auth sites | adblock-engine filter file | **UNTOUCHED (I1)** |

**Where the decision is made today (browser process):** `simple_handler.cpp::OnBeforeBrowse:7484-7521` — on `frame->IsMain()` navigation, if global FP enabled and not localhost:
- if `IsAuthDomain(url)` **or** `!IsSiteEnabled(domain)` **or** `!GlobalEnabled` → send **`fingerprint_site_disabled`** IPC (url).
- else → compute `GetDomainSeed(url)` and send **`fingerprint_seed`** IPC (seed, url).

**Where it is re-checked (renderer):** `simple_render_process_handler.cpp:581-627` — `OnContextCreated` **independently** calls `IsAuthDomain(url)` (line 585), consults the one-shot `s_fingerprintDisabledUrls` (from the disable IPC), else looks up `s_domainSeeds` and injects `FINGERPRINT_PROTECTION_SCRIPT`. Note the `window.chrome` stub (`:629-653`) is injected for **all** external pages and is **NOT** auth-gated — it is a bot-signal helper, unrelated to this plan, and stays.

### Two correctness smells in today's scheme that C7 should fix (not port)
1. **Exact-host, per-frame matching.** `IsAuthDomain` matches the *exact host* of *each frame's* URL. A Turnstile/reCAPTCHA iframe (`challenges.cloudflare.com`) is a different host than its parent, so the two frames get **independent** farbling decisions → inconsistent parent/iframe fingerprint → Turnstile rejects it. The code works around this by **listing both** the parent (`whatsonchain.com`, `www.whatsonchain.com`, `test.whatsonchain.com`) **and** the widget hosts — a fragile, ever-growing hack (see the explicit comment at `FingerprintProtection.h:215-224` citing `brave/brave-browser#45608`).
2. **One-shot seed/disable caches.** `s_domainSeeds` / `s_fingerprintDisabledUrls` are **erased after the main frame reads them** (`:595`, `:609`). Subframes then fall back to a **URL-hash seed** (`:613`) — a *different* seed than the parent → again inconsistent within one page.

Both smells vanish under **top-frame keying with a persistent per-page decision** (§2).

---

## 2. The C7 design — browser decides, renderer obeys, top-frame keyed

### 2.1 The single decision function (browser process, shell C++)
Replace the three scattered checks with one function in the shell (keep it in `FingerprintProtection` or a small `FarblingPolicy` helper — **browser process, not a Blink patch**):

```
ShouldFarble(top_frame_host, top_frame_registrable_domain)  ⇔
      GlobalEnabled
  AND !IsAuthDomain(top_frame_host)                   // A — auth/OAuth allowlist: match FULL host (OQ3)
  AND  IsSiteEnabled(top_frame_registrable_domain)    // B — user per-site toggle (re-homed)
```

- **Two inputs, deliberately.** The **membership test (A) matches the full committed top-frame HOST** — NOT the registrable domain — so we do not exempt all of `google.com`/`youtube.com` when only `accounts.google.com` / `www.google.com` (reCAPTCHA) is on the list (the list contains both of those Google hosts today; collapsing to `google.com` would be actively wrong). The **registrable domain (eTLD+1) is reserved for the seed HMAC key** (and for the user-toggle (B), whose granularity is per-site). This makes §2.1 agree with OQ3 on its face.
- **Single source of truth** for the allowlist (A) and the user toggle (B). The global flag, the allowlist, and the persisted `fingerprint_settings.json` all already live in the browser process — no new home needed.
- **Keying vs matching are independent.** The decision is *keyed* to the TOP-FRAME (one decision per page, §2.5), but the allowlist *match* is host-precise. Do not conflate them.

### 2.2 Per-navigation origin & eTLD+1 resolution
- The decision is made **once per top-level (main-frame) navigation**, keyed to the **committed top-frame origin's registrable domain**.
- **eTLD+1 computation:** the seed is already specified to key on `first_party_eTLD+1` (outline C2 / I4). Compute the registrable domain via the Public Suffix List. In Blink the canonical call is `SecurityOrigin::RegistrableDomain()` / `net::registry_controlled_domains::GetDomainAndRegistry`. **Decision (OQ1 default): do the eTLD+1 computation and the allowlist membership test in the BROWSER PROCESS**, so the renderer receives only a boolean + the top-frame domain string (for seed HMAC). If the shell process cannot cleanly reach `net::registry_controlled_domains` as a CEF embedder, bundle a minimal PSL matcher in the shell (small, static) rather than shipping the allowlist into Blink. *(This is the one plumbing unknown — see §4 R2 / OQ5.)*
- Today's hand-rolled `ExtractDomain` (host only, **not** eTLD+1 despite the header docstring falsely claiming eTLD+1) stays **host-precise for the allowlist match**; the eTLD+1 computation is added **only for the seed key** (and the per-site toggle). **Do NOT collapse allowlist entries to eTLD+1** (e.g. `accounts.google.com` must NOT become "all of `google.com`" — the list also carries `www.google.com` for reCAPTCHA, so registrable-domain collapse would over-exempt every Google subdomain; see §5 OQ3). Default: match the **full committed top-frame host** against the list, and use registrable-domain only for the seed HMAC.

### 2.3 Delivery to the renderer — HARD FORK on C2's channel choice
> ⚠️ **The "no new IPC, rides on C2's payload" claim is conditional, not settled.** It holds ONLY if C2 picks a **per-navigation** delivery channel. Outline C2 (line 153) leaves the channel open between **(a)** a startup ephemeral-nonce **cmdline switch** (the `B1-farbling-design.md` §45–53 default: profile seed delivered *once at renderer startup*, `domain_seed = HMAC(profile_seed, eTLD+1)` computed *inside the renderer* per page — **there is no per-navigation payload in this model**) and **(b)** a **per-navigation** mojo / pref-on-navigation channel. Today's code is per-navigation (`fingerprint_seed` IPC in `OnBeforeBrowse:7515`); this doc's simplicity claim implicitly assumes the migration keeps that model — but per-navigation is exactly what C2's threat-model fix may replace.

**The fork C7 must respect:**
- **If C2 = (b) per-navigation channel:** **C7 adds ONE boolean `farble_enabled` to that same payload.** No new IPC message, no new channel. This is the assumed-happy path throughout this doc.
- **If C2 = (a) startup-cmdline / B1 default:** there is no per-navigation payload to ride, so C7 **must EITHER** add its own per-navigation `{top_frame_domain, farble_enabled}` message (directly contradicting "no new IPC," §7) **OR** ship the allowlist into the renderer so it can self-decide (directly contradicting OQ1 "renderer never sees the list"). Neither is free — **downgrade the "rides on C2, adds nothing" claim accordingly.** This is a real precondition, resolved by C2, not a footnote.

**Deliver to every frame/renderer of the page.** Under Chromium site isolation, cross-site iframes run in **separate renderer processes**. The browser must push the **same top-frame-derived** `{seed_material, farble_enabled}` to **each** such renderer (this is the identical cross-process plumbing C2/I2 already owes for cross-site-iframe seed consistency — under channel (b), C7 rides on it and adds nothing new).

### 2.4 Renderer read-side (Blink, `HodosSessionCache`)
- `HodosSessionCache` (the `Supplement<ExecutionContext>` from C1) stores `farble_enabled` alongside the domain seed, **for the lifetime of the execution context** — **NOT one-shot**. Every patched API (`getImageData`, `toDataURL`, `getParameter`, `getChannelData`, and `hardwareConcurrency`/`deviceMemory` *if* farbled at all — subject to the §3c value-table decision in Q5; C6 may drop it) begins with:

```
auto& cache = HodosSessionCache::From(*execution_context);
if (!cache.farbling_enabled()) return <native value>;   // hard bypass, C7
... else apply farbling with cache.domain_seed() ...
```

- **Hard bypass, not "seed = 0".** When exempt, patched APIs return the true native value — never a degenerate/zeroed farble that would itself be a tell.
- Because the value is stored per-context (window + in-process workers via the Supplement) and identical across all page frames, **parent + every subframe + workers share the same decision** → internally consistent fingerprint. This is what fixes Turnstile structurally.

### 2.5 Why top-frame keying is correct (and matches Brave)
Brave keys farbling to the **top-level (first-party) origin**: all frames in a page share one farbling seed and one on/off decision, so the fingerprint a page presents is internally consistent whether the reader is the top document or an embedded CAPTCHA iframe. We adopt the same rule. Consequence:
- **Exempt top-frame (e.g. `accounts.google.com`):** the whole page tree — including any embedded reCAPTCHA/Turnstile iframe — is native. Consistent → passes.
- **Non-exempt top-frame that embeds Turnstile (e.g. a random site):** parent **and** the Turnstile iframe are farbled with the **same top-frame seed** → internally consistent. **Consistency is *necessary* but its *sufficiency* is unverified.** The only primary evidence we have (`FingerprintProtection.h:215-224` comment + `brave/brave-browser#45608`) establishes that *inconsistency is rejected* — NOT that a consistent-but-implausible farbled fingerprint always passes (e.g. the random WebGL vendor/renderer strings outline §3c flags as "more unique than the truth" could still be scored as a bot). So: consistent farbling *removes the known parent/child failure mode*, but the widget-host trim must be proven live, not assumed.
- **Therefore the widget-host allowlist entries *may* no longer be load-bearing** — but trimming is **gated on a live pass/fail (OQ2), and reverts (re-adds the widget host) on any failure.**

### 2.6 OAuth-specific contexts (the doc's namesake flows)
Because the decision is **top-frame keyed** (§2.5), the three common OAuth surfaces fall out cleanly — stated explicitly so nobody mis-plumbs them:

- **OAuth popup windows** (`window.open('https://accounts.google.com/...')`): a popup is a **separate top-level browsing context** whose *own* top-frame IS the auth origin. `ShouldFarble` runs against `accounts.google.com` as the top frame → **correctly exempt**, independent of whatever opened it. No special handling; the top-frame rule already covers it.
- **FedCM "Sign in with Google"** (browser-native account chooser, outline §3g): FedCM renders in **browser-native UI, not a web page**, so there is **no farblable JS surface** — it is unaffected by farbling either way. **Do NOT add FedCM IdP origins to the allowlist** expecting an effect; there is nothing to exempt.
- **Same-document / SPA navigations** (history.pushState, hash routes) that do **not** re-fire `OnBeforeBrowse`: no new top-frame commit occurs, so **the decision persists from the last committed top-frame** — which is the correct behaviour (the origin hasn't actually changed). No re-decision needed; just don't assume every visible URL change re-runs `ShouldFarble`.

---

## 3. `hodos-unbreak.txt` and the scriptlet layer — role UNCHANGED; do NOT couple

- **`hodos-unbreak.txt` is not touched by C7.** It is an adblock-engine file (`#@#+js()` blanket scriptlet exemption). It governs the **scriptlet** layer, not farbling. The "Keep in sync with hodos-unbreak.txt" comment in `IsAuthDomain` (`FingerprintProtection.h:202`) is *manual guidance*, not a code dependency — the two lists serve two layers and legitimately overlap on sensitive sites (defense per-layer). **Keep them as two separate lists** (Q2-2 default; do not merge — merging couples adblock + farbling maintenance).
- **Should the C7 exemption ALSO suppress scriptlet injection? → NO (default).** Rationale:
  1. Scriptlets are ad-behavioural overrides (`fetch`/`XHR`/`JSON.parse`), usually harmless on auth sites and sometimes needed (auth sites still serve ads).
  2. Coupling means one list edit silently changes two behaviours → maintenance/regression risk.
  3. The scriptlet layer already has its own tuned mechanism (`hodos-unbreak.txt`), maintained with the filter lists.
  If a specific site needs **both** farbling-off and scriptlet-off, add it to **both** lists (exactly as today). C7 gates **farbling only**.
- **Scope note — `IsSiteEnabled` re-homing (closes the Q2 TP-2 gap).** `Q2` explicitly deferred the user per-site toggle to "the farbling plan." **This plan is that owner.** C7's `ShouldFarble()` (§2.1) absorbs `IsSiteEnabled` as input B, so the toggle survives the migration. ⚠️ The outline literally scopes C7 as "re-implement `IsAuthDomain` **ONLY**"; folding in `IsSiteEnabled` is a **deliberate scope extension** — flag for owner sign-off. If the owner wants C7 kept minimal, split B into a sibling **C7b (user-toggle re-home)** landing in the same P4d step so the toggle is never dropped. Either way, **the `fingerprint_get/set_site_enabled` IPC + `fingerprint_settings.json` persistence must NOT be deleted by the M1 teardown until `ShouldFarble` consumes them** (Q2 T8 gate).

---

## 4. Risks

| # | Risk | Mitigation |
|---|---|---|
| **R1** | **Semantic change vs today (exact-host → top-frame eTLD+1).** A host farbled independently today may now inherit the top-frame's state. | Audit the allowlist for host-vs-registrable intent (OQ3); live-test the standard basket + every exempt parent site before trimming widget hosts (OQ2). Default keeps host-level match for the allowlist test, so behaviour changes only where it *fixes* consistency. |
| **R2** | **C2's delivery model is doubly unresolved and both axes are load-bearing.** (i) **Per-startup vs per-navigation channel:** if C2 lands on the B1-default **startup-cmdline** model there is *no per-navigation payload to ride* → C7's "no new IPC" claim breaks (must add its own message OR ship the list to the renderer; §2.3 fork). (ii) **Per-renderer-origin vs per-top-frame keying:** even on a per-navigation channel, if C2 keys the payload to the *renderer's own origin* rather than the *top-frame*, cross-site subframes desync → Turnstile breaks again. | **Hard dependency on C2 choosing (a) a per-navigation channel AND (b) top-frame keying (I2/I4).** C7 adds no new plumbing *only under (a)+(b)*; it **must not land before C2's channel + cross-site delivery are both settled and proven.** Add a cross-site-iframe consistency assertion to P6 (§6 T5). If C2 = startup-cmdline, re-scope C7 per §2.3 before implementation. |
| **R3** | **eTLD+1 resolution in the shell.** The CEF embedder shell may not cleanly reach `net::registry_controlled_domains`. | Compute eTLD+1 in the browser process via a bundled minimal PSL matcher, OR let Blink compute the registrable domain (it has `SecurityOrigin::RegistrableDomain()`) and pass it up — but keep the **allowlist membership test in the browser** (renderer never holds the list). Settle in the C7 detailed plan (OQ5). |
| **R4** | **List staleness.** Hardcoded allowlist can't update without a rebuild. | Keep it in the **shell** (cheap rebuild), never in a Blink patch (expensive Chromium rebuild). Optionally back it with a small on-disk override file later (out of scope for beta.1). |
| **R5** | **Privacy trade-off.** An exempt site sees our **real** fingerprint. | Intentional: these are first-party auth sites we are logging into anyway; cross-site unlinkability (the actual goal) is unaffected because exemption is top-frame scoped. Document in the privacy note. |
| **R6** | **Degenerate bypass.** A "seed=0" or partial farble on exempt sites would be its own tell. | §2.4 mandates a **hard native pass-through**, not a zeroed seed. Assert in T2. |
| **R7** | **Silent teardown drops the user toggle.** M1 deletes `fingerprint_*` symbols. | Gate: do not delete `IsSiteEnabled`/`SetSiteEnabled`/`fingerprint_get/set_site_enabled`/`fingerprint_settings.json` until `ShouldFarble` consumes them (Q2 T8; §3 scope note). |

---

## 5. Open questions → recommended defaults

| # | Question | Recommended default | Why |
|---|---|---|---|
| **OQ1** | Where is the allowlist **membership test** done — browser or renderer? | **Browser process.** Renderer receives only a boolean + the top-frame domain (for seed HMAC). | One source of truth; no list duplicated into every renderer; renderer needn't resolve top-frame origin for OOP iframes. ⚠️ **This SUPERSEDES outline C7 (line 158)**, which currently reads the opposite ("eTLD+1 auth allowlist *passed to renderer*; `HodosSessionCache` returns pass-through when top-frame origin ∈ allowlist" — i.e. renderer holds the list). **Action item: update the outline's C7 row** to "membership test stays in the browser; renderer receives only `{top_frame_domain, farble_enabled}`," so the roadmap doesn't carry two contradictory plumbing descriptions into implementation. Note: this browser-side model is also what makes the §2.3 startup-cmdline fork painful (a startup channel + browser-held list can't self-decide per navigation). |
| **OQ2** | Trim the CAPTCHA/widget host entries (`challenges.cloudflare.com`, `www.gstatic.com`, `hcaptcha.com`, `newassets.hcaptcha.com`, …) now that top-frame keying gives parent/child consistency? | **Audit-and-trim in C7, gated by a live Turnstile + reCAPTCHA + hCaptcha test on both an exempt-parent and a non-exempt-parent site.** Keep the top-level parent/auth sites. | Widget hosts were only needed because of exact-host per-frame matching; top-frame keying makes them redundant — but verify before deleting. |
| **OQ3** | Allowlist granularity: match the full committed **top-frame host** (e.g. only `accounts.google.com`) or the **registrable domain** (all of `google.com`)? | **Match top-frame host against the list first** (preserve today's precision — we do NOT want to exempt all of `google.com`/`youtube.com` from farbling); use registrable domain only for the **seed key**. | Exempting an entire eTLD+1 over-broadly disables farbling on non-auth subdomains (privacy regression). Host precision + top-frame *keying* are independent choices. |
| **OQ4** | Should C7 also gate the **scriptlet** layer? | **No — keep split** (Q2-2). Farbling-exempt gates farbling only; scriptlet exemption stays in `hodos-unbreak.txt`. | Two layers, two mechanisms; coupling raises maintenance/regression risk. |
| **OQ5** | Fold the **user per-site toggle** (`IsSiteEnabled`) into `ShouldFarble` (extends C7 beyond "`IsAuthDomain` only"), or land it as sibling **C7b**? | **Fold into `ShouldFarble` (single decision function), flagged for owner sign-off; C7b is the fallback if the owner wants C7 minimal.** Either way land it in P4d. | Q2 TP-2 explicitly punted the toggle to "the farbling plan." Dropping it silently deletes a shipped feature (Q2 T8). |
| **OQ6** | Boolean on/off, or a Brave-style level (`OFF`/`BALANCED`/`MAXIMUM`)? | **Boolean (`FARBLE`/`OFF`) for beta.1** (matches today's binary behaviour); design the payload field so it can widen to an enum later. | Don't over-build; today's exemption is binary. |
| **OQ7** | Filename convention: outline §6 registers `Q3-farbling-x-oauth.md`; siblings use underscores (`Q2_farbling_adblock.md`) and this file is `Q3_farbling_oauth.md`. | **Do a single rename pass across all Q-docs at the end** (Q2-6 already flagged this); not renamed here to keep cross-refs stable mid-review. | Track the drift, don't silently accept it. |

---

## 6. Acceptance criteria (fold into P4d + P6; run Windows AND macOS)

The headline gate: **auth/OAuth sites still log in, and CAPTCHAs still pass, with native farbling active.**

| # | Test | Method | Pass criterion |
|---|---|---|---|
| **T1** | **Exempt auth sites log in** | Sign in on a representative subset of the allowlist: `accounts.google.com` (Google SSO), `login.microsoftonline.com`, `appleid.apple.com`, `github.com`, `x.com`, `paypal.com`, plus one bank | Login completes; no "unrecognized device"/bot loop. **NOTE:** `toDataURL.toString() == [native code]` is *not* an exemption proof — under Blink farbling **every** page (exempt or farbled) returns `[native code]`, so it only demonstrates *below-JS farbling in general* (that check belongs to the B1/§7 farbling gate). A dead exemption that silently farbles the auth site would still show `[native code]`. **Proof-of-exemption is T2, not T1.** |
| **T2** | **Hard bypass (native, not zeroed) — SOLE proof of a live exemption** | On an exempt site, read canvas/WebGL/audio values and compare against the same site with **farbling globally OFF** (the true-native baseline) | Values equal the **true native** baseline, **not** a stable-but-perturbed farble; no seed-0 artifact. This value-equality is the only test that distinguishes an *active* exemption from a *broken* one that farbles the auth site while still reporting `[native code]`. |
| **T3** | **CAPTCHA on a NON-exempt parent** | Load a non-allowlisted site embedding Cloudflare Turnstile (e.g. a WoC test page if de-listed per OQ2) and one embedding reCAPTCHA | **Pass:** challenge solves; parent and iframe present the **same** farbled fingerprint (consistency), page not stuck in "Verify you are human". **Fail branch:** if consistent farbling still trips the challenge (consistency ≠ sufficiency, §2.5), **re-list the CAPTCHA widget host** in the allowlist and re-run — do NOT ship the trim. |
| **T4** | **CAPTCHA on an EXEMPT parent** | `whatsonchain.com` (if kept exempt) or another allowlisted parent embedding Turnstile | **Pass:** whole page native; challenge passes. **Fail branch:** if it fails, the exempt parent's own listing (not just widget hosts) is implicated — keep the parent listed and file against C2 top-frame delivery before de-listing anything. |
| **T5** | **Cross-site iframe consistency (R2 gate)** | A non-exempt page with a cross-site iframe (separate renderer under site isolation); read farbled canvas in both | **Identical** farbled hash in parent and iframe (proves C2 delivered the same top-frame seed+bool cross-process). ⚠️ **This is the *consistency* test (same 3p within one page → same value); it is ADDED TO, not a substitute for, outline §7's *anti-tracking difference* test (same 3p under two different first-parties → DIFFERENT values). Both follow from top-frame keying and both must exist — an implementer must not drop either.** |
| **T6** | **Non-exempt sites STILL farbled** | CreepJS / browserleaks on a normal site with farbling on | Canvas/WebGL/audio farbled; **worker column == window column**; stable within profile+session |
| **T7** | **User per-site toggle survives** | Toggle Privacy-Shield fingerprint OFF for a specific non-auth site via the UI; reload | That site returns native values; `fingerprint_settings.json` persists the override; other sites still farbled (proves `IsSiteEnabled` re-home, OQ5) |
| **T8** | **Global toggle survives** | Turn fingerprint protection OFF globally | All sites native; back ON restores farbling |
| **T9** | **No orphaned FP-exempt symbols after teardown** | grep build for retired symbols per M1 — but **do NOT zero the user-toggle group until `ShouldFarble` consumes it** (Q2 T8) | `IsAuthDomain`'s old exact-host/per-frame path gone; `s_fingerprintDisabledUrls`, `fingerprint_site_disabled` IPC removed; `ShouldFarble` is the sole decider |
| **T10** | **Login persistence across restart (belt-and-suspenders)** | Log into an exempt AND a non-exempt (farbled, persistent-seed) site; restart browser; revisit | Both remain logged in — exempt via native, non-exempt via persistent per-profile seed (C2). Confirms the exemption is *belt-and-suspenders*, not the only thing holding logins together |

**T2's native-value equality + T5's cross-frame hash equality are the two most valuable Q3 gates** — the first is the *sole* proof the exemption is actually live (not silently farbling behind a `[native code]` façade); the second proves top-frame keying reached OOP iframes. (T1's `[native code]` check is retained only as the general below-JS-farbling gate, not as exemption proof.)

---

## 7. What this does and does NOT require

**Requires (all in the farbling migration; nothing in the adblock engine):**
- One browser-process `ShouldFarble(top_frame_registrable_domain)` decision function (absorbs `IsAuthDomain` + `IsSiteEnabled` + global flag).
- One boolean `farble_enabled` added to the **C2 seed payload** (no new IPC), delivered per top-frame to **all** page renderers.
- `HodosSessionCache` stores it **non-one-shot**; patched APIs hard-bypass to native when false.
- Allowlist re-expressed for top-frame matching; widget-host entries audit-trimmed (OQ2).
- T1–T10 folded into P4d/P6.

**Does NOT require:**
- Any change to `hodos-unbreak.txt` or the adblock scriptlet/cosmetic pipeline (I1; §3).
- Shipping the allowlist into Blink / any renderer (OQ1 — browser decides).
- A new IPC channel (rides C2's payload — **precondition: C2's cross-site top-frame delivery is settled**, R2).
- Deleting the user per-site toggle or its persistence (R7 / Q2 T8).

**Bottom line:** Q3 is **GREEN conditional on TWO C2 choices — (a) a per-navigation delivery channel AND (b) top-frame keying (R2/T5, §2.3 fork).** If C2 picks the B1-default startup-cmdline channel, C7 loses its "no new IPC" property and must be re-scoped before implementation. Given (a)+(b), the auth/OAuth exemption re-homes cleanly as a single browser-process decision keyed on the top-frame eTLD+1, delivered as one bit alongside the seed. It is *simpler and more correct* than today's exact-host, dual-checked, one-shot scheme — and it structurally removes the Turnstile parent/iframe inconsistency the current code hand-patches. Scriptlet exemption stays split; the persistent per-profile seed (C2) means the exemption is belt-and-suspenders for the most sensitive OAuth flows, not the sole login safeguard.

---

## Sources
- Brave per-origin (first-party/top-frame) farbling & Shields level model: [brave/brave-core `brave_session_cache` / farbling](https://github.com/brave/brave-core), [Brave "What is fingerprinting and how does Brave protect me"](https://brave.com/privacy-updates/), parent/iframe Turnstile inconsistency: [brave/brave-browser#45608](https://github.com/brave/brave-browser/issues/45608)
- eTLD+1 / registrable domain: Chromium `net::registry_controlled_domains::GetDomainAndRegistry`, Blink `SecurityOrigin::RegistrableDomain()`; [Public Suffix List](https://publicsuffix.org/)
- Anti-bot parent/child fingerprint-agreement behaviour: `Q2_farbling_adblock.md` §2.5; [uBO fingerprinting wiki](https://github.com/gorhill/uBlock/wiki/Does-uBO-protect-against-fingerprinting%3F)
- Repo (verified 2026-07-10): `cef-native/include/core/FingerprintProtection.h` (`IsAuthDomain` :189-270; `IsSiteEnabled`/`SetSiteEnabled` :123-145; `Load/SaveSiteSettings` :147-187), `cef-native/src/handlers/simple_handler.cpp` (`OnBeforeBrowse` seed/disable decision :7484-7521; toggle IPC :6191/6210), `cef-native/src/handlers/simple_render_process_handler.cpp` (injection + auth recheck :581-627; `window.chrome` stub :629-653), `development-docs/0.4.0/CHROMIUM_CEF_BUILD_DESIGN_OUTLINE.md` §3c (C1–C7, I1/I2/I4), §6 Q3; `development-docs/0.4.0/chromium-rebuild/Q2_farbling_adblock.md` (TP-2, T8); `development-docs/0.4.0/B1-farbling-design.md`.
