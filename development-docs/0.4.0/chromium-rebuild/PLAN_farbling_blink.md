# PLAN — Blink Farbling Patch Set (Core B1)

**Status:** DETAILED PLAN (Workflow-2 expansion of `CHROMIUM_CEF_BUILD_DESIGN_OUTLINE.md` §3c FEAT-B1 / phase P4). Research + design only — **NO code, NO builds.**
**Created:** 2026-07-10 · **Owner:** Matthew (Marston Enterprises) · **Lead:** Windows Claude
**What this plans:** The exact `third_party/blink` files to patch and the change per file to move farbling from today's detectable JS-injection (`FingerprintScript.h` / `FingerprintProtection.h`) into native Blink C++ in our self-built CEF — the `HodosSessionCache` `Supplement<ExecutionContext>` design, the **persistent per-profile seed** wiring (browser generates + stores → off-cmdline delivery → renderer), worker/worklet coverage, the incremental landing order, the reconciled **farble-vs-omit value table**, the clean-room license plan, rebase cadence, and acceptance gates. **TARGET version is a placeholder** — resolve the exact CEF stable branch per outline §2 Step 0 before landing. **Feeds Q1 (mac), Q3 (OAuth exemption), Q5 (full edit list).**

> **Authoritative inputs:** `0.4.0/B1-farbling-design.md`, `0.4.0/CHROMIUM_CEF_BUILD_DESIGN_OUTLINE.md` §3c (C1–C7) / §5 (OS split) / §7 (acceptance), `DevOps-CICD/research/BRAVE_FORK_FEASIBILITY.md` §B1, `DevOps-CICD/CEF_BUILD_RUNBOOK.md` (patch toolchain — Step 2), sibling `chromium-rebuild/Q2_farbling_adblock.md` (teardown adjacency) + `PLAN_codecs.md` (style/scope). Repo cites verified against working tree 2026-07-10. Brave technique cites are **primary-source reference for a clean-room re-implementation** — see §9 license boundary.

---

## 1. What this plans (one screen) + headline recommendation

- **The migration in one line:** delete the renderer JS that overwrites `getImageData`/`toDataURL`/`readPixels`/`getChannelData`/`plugins` (detectable ≥6 ways; **never fires for workers**), and re-home the same perturbations as native C++ inside Blink, keyed off a `Supplement<ExecutionContext>` that reads a **persistent per-profile, per-first-party seed** delivered off the command line.
- **Headline recommendation:** port Brave's **technique** (Supplement + HMAC domain key + `FarblingPRNG` + per-value farbling), but **improve on Brave in two ways that matter for a wallet browser**: (1) **persist** the seed per-profile instead of per-session (Brave regenerates each launch → breaks fingerprint-based re-auth — the login-breakage we are fixing); (2) **keep the master seed entirely inside the browser process** — deliver only the *derived per-site key* to the renderer (Brave puts its session token on the child command line; our C2 threat model forbids a stable machine-local secret on any cmdline). Land it **incrementally, worker-coverage first** (the single highest-signal fix).
- **The one design decision this doc must settle (feeds Q5):** the `B1-farbling-design.md` list **re-adds** WebGL `UNMASKED_VENDOR/RENDERER` and navigator `hardwareConcurrency`/`deviceMemory` that the *current JS impl deliberately dropped as detectable*. §7 below resolves each value with a recommended default (**valid-set-constrained, desktop-plausible values** — the *constraint technique* is Brave's, but the literal sets are a Hodos decision, not "Brave-verbatim"; **drop WebGL vendor/renderer unless a common-GPU-string map is built incl. Mac ANGLE strings**).

---

## 2. What we port from Brave — and the one thing we cannot copy

Brave implements farbling as a **`chromium_src` compile-time file-shadow**: `brave-core/chromium_src/third_party/blink/renderer/.../<file>.cc` is compiled *in place of* Chromium's file via Brave's `redirect_cc` build machinery. **That shadow mechanism is a Brave-build-system feature we do not have and cannot adopt** — CEF's only source-edit lever is `cef/patch/patch.cfg` `.patch` files applied to the *real* Chromium tree before compile (`CEF_BUILD_RUNBOOK.md` Step 2; `BRAVE_FORK_FEASIBILITY.md` §B1 "Path C"). So we take Brave's **algorithm/technique as a reference blueprint** and express it as **in-place hook insertions** into the genuine Blink files (see §9 clean-room boundary).

**Verified Brave technique (primary sources — reference only):**

| Brave element | What it does | Our re-implementation |
|---|---|---|
| `BraveSessionCache : Supplement<ExecutionContext>` (`execution_context.cc`) | Per-context farbling state; `From(ExecutionContext&)` lazily `MakeGarbageCollected` + `ProvideTo`. | **`HodosSessionCache`** — identical Supplement shape, our name/namespace, our seed source (§3). |
| Session token via `cmd_line->GetSwitchValueASCII(kBraveSessionToken)` → `base::StringToUint64` | Random per-**session** 64-bit key on the child cmdline. | **Diverge (C2):** persistent per-**profile** seed, **never on cmdline** — browser derives the per-site key and delivers it (§4). |
| `domain_key_ = HMAC-SHA256(session_key_, GetDomainAndRegistry(origin, INCLUDE_PRIVATE_REGISTRIES))` | Per-eTLD+1 key so the same site is stable, different sites differ. | Same HMAC construction; **first-party/top-frame** eTLD+1 keying (I4); **computed browser-side** (§4). |
| `MakePseudoRandomGenerator()` → `FarblingPRNG` | Deterministic PRNG seeded from `domain_key_`; drives every perturbation. | `HodosSessionCache::MakePrng()` — same role; replaces today's JS Mulberry32. |
| `GetAudioFarblingCallback(...)` applied per audio sample | OFF / BALANCED (fudge-factor) / MAXIMUM levels. | BALANCED-equivalent per-sample fudge (matches today's `*= 1.0 + (rng()-0.5)*4e-7`), native. |
| `PerturbPixels(data, size)` | Canvas readback pixel noise. | Same, invoked from the canvas readback path (§6 C3). |
| `FarbleDeviceMemory(context)` → selects from `{0.25, 0.5, 1, 2, 4, 8}` (`navigator_device_memory.cc`) | Farbled deviceMemory constrained to a **valid set** (Brave's set is *mobile-inclusive* — spans down to 0.25). | Adopt the **valid-set technique**, not Brave's literal values: pick a **desktop-plausible** set as a Hodos decision (§7). Not "verbatim." |
| `FarbleHardwareConcurrency` → random value in **`[2, real]`** (never exceeds real cores) | Reduces reported cores to a plausible value **≤ the machine's actual count**. | Adopt the **"≤ real, plausible"** rule (§7) — never a fixed set that can *inflate* a low-core machine. |
| `BRAVE_WEBGL_GET_PARAMETER_UNMASKED_RENDERER` macro in `getParameter` | Hooks the WebGL vendor/renderer read. | Only if we choose to farble vendor/renderer (§7 — recommend **drop** unless GPU-string map built). |

**Sources (primary):** brave-core `execution_context.cc` (BraveSessionCache/From/HMAC/session token), `navigator_device_memory.cc` (valid-set), `webgl_rendering_context_base.cc` (getParameter macro), `audio_buffer.cc` (audio callback), brave.com fingerprinting-defenses-2.0 (per-session per-eTLD+1 seed model). See §Sources.

---

## 3. Architecture — `HodosSessionCache` Supplement<ExecutionContext>

Implement one Blink `Supplement<ExecutionContext>` that all patched APIs consult:

```cpp
// (design intent — not code to paste; clean-room per §9)
class HodosSessionCache : public GarbageCollected<HodosSessionCache>,
                          public Supplement<ExecutionContext> {
 public:
  static HodosSessionCache& From(ExecutionContext&);   // lazy ProvideTo, GC-managed
  bool FarblingEnabledForThisContext() const;          // false ⇒ pass-through (auth exempt / disabled)
  HodosPrng MakePrng();                                 // deterministic, seeded from domain_key_
  void PerturbPixels(unsigned char* data, size_t size);
  double FarbleAudioSample(double sample);
  // ... device-memory / concurrency helpers per §7
 private:
  std::array<uint8_t, 32> domain_key_{};   // = HMAC(profile_seed, first-party eTLD+1) — delivered, not derived here
  bool enabled_ = true;
};
```

**Why the Supplement (not `OnContextCreated` JS):**
- `ExecutionContext` is the base of `LocalDOMWindow`, `DedicatedWorkerGlobalScope`, `SharedWorkerGlobalScope`, `ServiceWorkerGlobalScope`, and worklet scopes → the **Supplement class hierarchy attaches to all of them uniformly** (one hook covers every context type). But *coverage ≠ automatic* for the OOP ones: **in-process** contexts (window, same-site iframes, dedicated workers, audio/paint worklets) inherit the key for free once §4 delivery lands, whereas **shared workers and service workers are always out-of-process**, so key *delivery* to them is a separate step (P4e), not automatic. This directly closes today's #1 gap: `OnContextCreated` never fires for workers, so `FingerprintScript.h` leaves worker canvas/WebGL/audio **raw** (window-vs-worker mismatch = the classic JS-injection tell). Confirmed worker-blind in `BRAVE_FORK_FEASIBILITY.md` §B1.
- Farbling runs **at API-call time** (inside `getImageData`/`readPixels`/`getChannelData`), not at context creation → no injection-timing race, no `[native code]` toString tell (Q2 §2.5 net win: restores prototype integrity for anti-bot stacks).
- State is per-context and GC-managed → no manual lifetime, no cross-frame leakage.

**`FarblingEnabledForThisContext()`** returns the pass-through switch used by C7 (auth-domain exemption) and by the user's per-site Privacy-Shield toggle (the shipped `FingerprintProtection::IsSiteEnabled` control — see §4 / Q2 TP-2): when the top-frame origin is exempt or the user disabled the site, the cache is populated with `enabled_ = false` and every patched API calls the real Chromium code unchanged.

---

## 4. Persistent per-profile seed wiring (the login fix + the C2 threat model)

> ### 🔎 Brave research 2026-08-08 — our persistent-seed divergence is VINDICATED, with a citation
>
> Clean-room investigation (published writeups + issue **prose** only; `brave-core` source deliberately
> not read, per M7). Three results that matter here:
>
> 1. **Brave is per-SESSION, we are persistent — and their choice demonstrably causes the exact
>    breakage this section exists to prevent.** Documented: *"Each time you start Brave, a unique,
>    random session token is created … regenerated when you restart Brave,"* then HMAC-256'd with the
>    top-frame eTLD+1 ([privacy-updates/4](https://brave.com/privacy-updates/4-fingerprinting-defenses-2.0/),
>    [wiki](https://github.com/brave/brave-browser/wiki/Fingerprinting-Protections)). And a public
>    complaint reports precisely the predicted symptom: *"I have seen on a lot of sites send me a
>    'login from new device' mail when using brave, where it would NEVER happen when using Firefox"*
>    ([privacyguides discussion #7](https://github.com/orgs/privacyguides/discussions/7)) — with no
>    Brave engineering response found. ⇒ **Keep the persistent per-profile seed. Do not "align with
>    Brave" on this in a future review** — we would be importing a known, reported login regression.
>    Our derivation is otherwise the same shape (session/profile secret ⊕ HMAC ⊕ eTLD+1).
> 2. **Even a full fork shipped OUR bug class.** Brave issue
>    [#49346](https://github.com/brave/brave-browser/issues/49346) (v1.82.x) reports plugin,
>    hardwareConcurrency and speech-voice values **no longer changing between relaunches** — a
>    de-facto constant where a per-session value was expected — triaged **P2 regression**. So the
>    "farbling silently degenerates to a constant" failure is not a Hodos-specific mistake; it is
>    endemic and it survives code review. ⇒ **The durable artifact is the TEST, not the fix.** The
>    cross-session / seed-rotation assertion belongs in CI, because this class of bug is invisible to
>    any same-session check. Brave states no position on constant-vs-native fallback anywhere public,
>    so our fail-closed contract stands as **our own** call, not a borrowed one.
> 3. **Subframe policy confirmed, worker cost confirmed.** Brave documents that third-party frames
>    *"share the seed value of the top level, eTLD+1 domain"* — same policy as our §5 matrix. Workers
>    needed a separate engineering pass ([#42427](https://github.com/brave/brave-browser/issues/42427),
>    titled "follow up") and OOPIFs had historical inheritance gaps
>    ([#12020](https://github.com/brave/brave-browser/issues/12020)). ⇒ P4e's scope is realistic, not
>    conservative.
>
> ⛔ **What this research did NOT answer: the delivery mechanism (our actual blocker).** Which process
> does the HMAC, whether the raw session secret ever enters a renderer, and what ordering guarantee
> puts the value in place before first script — **none of it is published.** Do not spend more time
> looking; the answer is not public. Our delivery design has to be settled on our own evidence.

**Goal:** a fingerprint that is **stable across restarts** (so re-auth reads us as the same device — fixes login breakage) but **different per first-party site** (defeats cross-site tracking) and **different per profile**, with **no stable secret on any child command line** (C2 threat model — a cmdline value is visible to every local process via ProcessExplorer/`ps`).

```
BROWSER PROCESS (C++ shell)                         RENDERER / WORKER PROCESS (Blink)
profile_seed  (32B CSPRNG, generated ONCE per       HodosSessionCache::From(ctx)
  profile, stored in profile data — NOT the           .domain_key_  ← delivered value
  wallet; alongside settings.json/fingerprint_         (master seed NEVER arrives here)
  settings.json in %APPDATA%/HodosBrowser/<profile>)      │
        │  on navigation commit / worker start:          ▼
        ├─ compute domain_key = HMAC-SHA256(          patched Canvas/WebGL/Audio/Navigator
        │    profile_seed, first-party eTLD+1)        read domain_key_ via the Supplement
        ├─ decide enabled = !IsAuthDomain(top) &&
        │    IsSiteEnabled(top)                       (C7 exemption + user per-site toggle
        └─ DELIVER {domain_key, enabled} ────────────►  fold into `enabled` here)
```

**Divergence from Brave, and why it is strictly better:** Brave puts the *master session key* on the child cmdline and lets the renderer compute the HMAC. We instead **compute the HMAC in the browser process and deliver only the per-site `domain_key`** — the master `profile_seed` never leaves the browser. This satisfies C2 *and* supersedes the `B1-farbling-design.md` "renderer computes `domain_seed = HMAC(profile_seed, eTLD+1)`" line (the renderer no longer needs the master seed at all).

**Delivery channel — ✅ DECIDED 2026-08-05 (FB-1): option (A′), owner-approved.**

The original (A) assumed we would have to extend `blink::mojom::CommitNavigationParams` and patch `content/`. **We don't.** Verified on the real 7871 tree: CEF already ships a **per-frame browser↔renderer mojo channel that lives entirely inside our fork**, so the browser half of C2 costs **zero Chromium patches**:

| Piece | Where | Status |
|---|---|---|
| `interface BrowserFrame` / `interface RenderFrame` | `cef/libcef/common/mojom/cef.mojom` | exists — extend it |
| Frame-scoped binder | `cef/libcef/browser/browser_frame.cc :: CefBrowserFrame::RegisterBrowserInterfaceBindersForFrame` (`:30`) | exists — already wired |
| Renderer connects at commit | `cef/libcef/renderer/frame_impl.cc :: ConnectBrowserFrame(ConnectReason::DID_COMMIT)` (`:334`) | exists |
| Renderer→Blink hop | **NEW** — a small Blink public-API setter on `WebLocalFrame` | the only Chromium patch in C2 |

- **(A′) ADOPTED — push over CEF's existing per-frame channel at commit, PLUS a lazy `[Sync]` pull on first farbled API call.** The push alone is async: a page can call `getImageData` from its first inline script before the message lands. The lazy sync pull closes that race deterministically — at most one sync IPC per execution context, and only if the page actually fingerprints. Everything but the final Blink setter is fork-local, which is the cheapest thing to carry across Chromium bumps.
- **(B) REJECTED — ephemeral per-launch nonce on cmdline** (Brave-shaped). It buys nothing here: it needs the same mojo round-trip to redeem the nonce, **plus** a cmdline token and a browser-side nonce→profile map. Q1's first-paint timing argument for (B) is answered by (A′)'s sync pull.
- **⚠️ Scope correction — in-process dedicated workers are NOT literally free.** The Supplement attaches to any `ExecutionContext`, but the key still has to be threaded into `GlobalScopeCreationParams` at worker start (a small Blink patch). Still P4a, but budget it as an edit, not a freebie. The §5 matrix's "Free (P4a)" rows should be read as "no cross-*process* plumbing", not "no code".

**Persistence store:** reuse the existing per-profile fingerprint file (`FingerprintProtection::LoadSiteSettings` already reads `%APPDATA%/HodosBrowser/<profile>/fingerprint_settings.json`) — add a `profileSeed` field (32B, base64), generated on first run with a platform CSPRNG. **Note:** existing code uses `CryptGenRandom` (`FingerprintProtection.h :: Initialize`, `:47`), which Microsoft has **deprecated** in favor of `BCryptGenRandom` — prefer **`BCryptGenRandom`** for this *new* seed generation (Windows) / `SecRandomCopyBytes` (macOS); the deprecation is not introduced by this plan but new code should not extend the deprecated call. Reset only when the user clears that profile's data. **This is browsing-privacy state, never wallet/key state** (Invariant #1/#2 untouched).

**Reset semantics:** clearing a single site's data → re-derive nothing (domain_key is deterministic from the unchanged profile_seed); resetting the profile / "clear on exit" for fingerprint → regenerate `profile_seed` (new fingerprint everywhere). Document so the "Clear data on exit" path (a live prime-suspect for other regressions) has defined farbling behavior.

---

## 5. Worker / worklet / OOP-iframe coverage matrix (the "not free" part — I2 + C-2)

The Supplement covers any `ExecutionContext` **once the `domain_key` reaches that context's process.** In-process contexts inherit it; **out-of-process ones — OOP workers *and* cross-site iframes under default site isolation — need explicit cross-process delivery.**

> ## ⛔⛔ MEASURED 2026-08-10 — **ALL** workers are unfarbled, in-process included. The table below is aspirational, not current.
>
> The "in-process contexts inherit it for free" reasoning is **wrong in practice at `c63654654`**, and
> this was confirmed by measurement on macOS after being reasoned from the code on Windows — so it is
> not a platform artifact.
>
> **Why:** the only key-install path is `CefFrameImpl::MaybeApplyHodosFarblingKey()` →
> `blink_glue::SetHodosFarblingKey(WebLocalFrame*)` → `local_frame->DomWindow()` →
> `HodosSessionCache::From(*window)`. That takes a **`LocalDOMWindow`**. There is no worker-start hook
> anywhere in `libcef/`. A `DedicatedWorkerGlobalScope` is a *different* `ExecutionContext`, gets a
> fresh key-less Supplement, and **fails closed to native** — which the header states outright:
> *"FAIL-CLOSED BY CONSTRUCTION. A freshly created cache has no key, and with no key
> `FarblingEnabled()` is false."*
>
> **Measurement** (`chromium-rebuild/farbling_worker_probe.py`, macOS, one document on `example.com`,
> in-process dedicated worker via a same-origin blob URL):
>
> | example.com | main thread | dedicated worker |
> |---|---|---|
> | farbling **on** | canvas `48922b8f`, cores 5, mem 8 | canvas `2fad2e1a`, cores **8**, mem **16** |
> | farbling **off** (control) | canvas `2fad2e1a`, cores 8, mem 16 | canvas `2fad2e1a`, cores 8, mem 16 |
>
> The worker returns byte-identical **native** values while the main thread of the same document is
> farbled; with farbling off, main and worker agree — which is what makes the difference attributable
> to farbling rather than to the two code paths differing.
>
> ⛔ **Therefore: describe the gap as "window and same-site frames only; ALL workers unfarbled,
> in-process included" — NOT "OOP workers pending".** P4e is larger than this plan says. The owner's
> decision to defer it (2026-08-09) stands; only its described size changes.
>
> ⚠️ Two traps in probing this, both of which yield a confident wrong answer: an **auth-exempt origin
> cannot be the control** (github.com's CSP blocks `blob:` workers, so the worker never starts and the
> control silently yields nothing — use the *same* origin with the per-site opt-out applied), and
> **both sides must use `OffscreenCanvas` and draw no text** (different canvas objects and worker font
> fallback would otherwise make a main-vs-worker difference ambiguous).

| Context | Process | Key delivery | Effort |
|---|---|---|---|
| `LocalDOMWindow` — main frame + **same-site** iframes | same renderer | navigation-commit delivery (§4) | Base (P4a) |
| `LocalDOMWindow` — **cross-site (OOP) iframe** | **separate renderer process** (default site isolation, M136+) | needs the **top-frame's** `{domain_key, enabled}` delivered to the subframe process at its navigation commit — same cross-process class as OOP workers | **Not free — P4e-class** (see below) |
| **Dedicated worker** | same renderer as creator | inherits via §4 delivery to the parent document, Supplement attaches on worker ctx | **Free** once §4 lands (P4a) |
| Audio worklet / paint worklet | in-process | same | Free (P4a) |
| OffscreenCanvas in a dedicated worker | same renderer | same | Free (P4a) — **but explicitly tested** (§11) |
| **Shared worker** | **separate process** | needs per-worker `{domain_key, enabled}` at worker startup, keyed to the worker's **owner first-party** | **Not free — P4e** |
| **Service worker** | **separate process**; origin = registration scope (not top-frame) | needs startup delivery; decide keying (registration scope eTLD+1) | **Not free — P4e** |
| OffscreenCanvas in a shared/service worker | separate process | rides P4e delivery | P4e |

> ⚠️ **Read "Free (P4a)" as "no cross-*process* plumbing", not "no code".** The in-process worker/worklet rows still need the key threaded into `GlobalScopeCreationParams` at worker start — a small Blink patch, in P4a scope (§4, FB-1 note).

**OOP cross-site iframe delivery (same cross-process problem, called out explicitly):** under default site isolation a third-party iframe runs in its own renderer process that only knows *its own* origin — it cannot compute the top-frame eTLD+1 key itself, and the plan's headline design (browser computes `HMAC(profile_seed, first-party eTLD+1)` and delivers only the derived key) *requires* the browser to hand the **top-frame-derived** `{domain_key, enabled}` to that subframe process. The browser knows the full frame tree, so at each cross-site subframe navigation commit it delivers the **top frame's** key (this is what makes the §11 "cross-site iframe → different values across two first parties" gate pass). This is the same delivery machinery as OOP workers → **folded into P4e** (or landed alongside it). It is **not** covered by P4a's same-renderer navigation-commit path.

Brave hit exactly this seam (worker farbling follow-ups: brave-browser #42427 / #28904 — `WorkerContentSettingsClient` plumbing for OOP workers). **P4e enumerates the worker-start hook** (`WorkerThread`/`WorkerGlobalScope` init) *and the OOP-subframe commit hook*, and delivers the top-frame key there. Until P4e lands, ship P4a (window + same-site iframes + in-process workers) — which already closes the CreepJS dedicated-worker column — and **log OOP-worker + OOP-iframe coverage as a known gap**, not a silent one.

---

## 6. The exact Blink files to patch + change per file

Highest fingerprint value first. All paths are `third_party/blink/renderer/...`. Each is an **in-place hook**: call `HodosSessionCache::From(*execution_context)`, early-return the native result if `!FarblingEnabledForThisContext()`, else perturb via the Supplement. Register every `.patch` in `cef/patch/patch.cfg` (P3 must exist first).

### C1 — Supplement (new file + source-list entry) `[foundation]` — ✅ **LANDED 2026-08-05**
- **New:** `hodos_session_cache.{h,cc}` under `core/execution_context/`. Defines `HodosSessionCache` (`From()`, `SetOriginKey()`, `FarblingEnabled()`, `HasOriginKey()`, `MakePrng(Stream)`, `PerturbPixels()`, `AudioFudgeFactor()`) and `HodosPrng`.
- **Source list:** `core/execution_context/build.gni` — a two-line entry. **This is the only existing file C1 touches.**
- **~~Hook: `execution_context.{h,cc}`~~ — NOT NEEDED, do not add.** See the landing note below.
- **Seed intake:** `SetOriginKey(domain_key, farbling_enabled)` is the C2 delivery target — see §4.

> ### ✅ C1 LANDED 2026-08-05 — and it needs **no** `execution_context.{h,cc}` hook
>
> Fork commit `4ed200cf9`, registered as `hodos_farble_session_cache` (`condition: HODOS_FARBLING`). Two findings that make C1–C7's rebase cost lower than this plan assumed — **defend both on every bump**:
>
> 1. **No hook in `execution_context.{h,cc}` is required.** `ExecutionContext` already derives from `Supplementable<ExecutionContext>` (`execution_context.h:130`), so a Supplement attaches entirely from its own translation unit via `ProvideTo`. §6 C1's "hook `execution_context.{h,cc}`" step is **unnecessary** — do not re-add it, and do not let a rebase reintroduce a hunk there.
> 2. **Blink uses per-directory `build.gni` source lists**, not one monolithic `core/BUILD.gn`. The real target is `core/execution_context/build.gni` — a much lower-churn file than "a Blink `BUILD.gn`".
>
> Net: **C1 modifies no existing source file.** Two new files (which can never conflict) plus a two-line list entry. That is the target shape for all of C1–C7 — logic in `hodos_session_cache.cc`, one-liners on Chromium files.
>
> **Fail-closed contract (load-bearing for C7):** a cache with no delivered key reports `FarblingEnabled() == false`, so every call site returns the native value. There is deliberately **no "farble with a zero key" state** — a degenerate constant-seeded perturbation would be a worse fingerprint than none, and Q3 R6 requires the exemption bypass be a **true native pass-through**. `HasOriginKey()` is exposed separately so the C2 delivery path can tell "browser said don't farble" from "browser hasn't answered yet" — that distinction is what the lazy `[Sync]` pull keys off.
>
> Per-vector PRNG **streams** (canvas / webgl-readPixels / audio) so a site reading several vectors cannot correlate them back toward the seed. Perturbation matches the outgoing JS for canvas/WebGL (red-channel LSB on ~3% of pixels) so already-enrolled users keep their fingerprint across the migration. **Audio deliberately does NOT match** — the JS multiplier was below float32 resolution ~15% of the time and farbled nothing; `|delta|` is floored at `2^-23` (fork `c63654654`), which shifts enrolled users' audio fingerprint once. That was accepted as the price of the value being farbled at all. The small-canvas gate stays at the **call site** — only the caller knows the surface dimensions.

### C2 — Seed/enabled delivery `[dep C1]` — ✅ **FIXED AND BEHAVIOURALLY PROVEN 2026-08-07, fork `116b7fd8b`.**

> ## ✅✅ PROVEN BY MEASUREMENT — farbling runs, per-profile keyed, reliable across launches
>
> **6/6 across three fresh browser sessions**, with the browser-side registry reporting
> `4 STORED, 0 misses` every time, and the farbled hash identical (`ee153adb`) across sessions
> while the auth-exempt control stayed `b5534a54`. The cross-page behavioural comparison had
> **never** passed before this (C3's farbling path had never once been observed to run).
>
> ### ⛔⛔ THE TRAP THAT FAKED AN "INTERMITTENT PER-SESSION BUG" — cost hours, read this
>
> Farbling appeared to work in some launches and fail in all navigations of others (6/6 vs 0/5).
> It looked exactly like a race in the `[Sync]` pull. **There was no bug. The harness was driving
> the wrong BROWSER.**
>
> Hodos's header and ~14 overlays are *separate CEF browsers* served from `127.0.0.1:5137`, and
> **CDP reports every one of them as `type: "page"`.** A harness that picks "the first page target"
> — or "the first target that is not 127.0.0.1:5137" — can land on the **tab-list overlay**
> (`role: tablistpanel`). Overlays legitimately receive no farbling key, so the measurement read as
> "farbling is broken", and which target CDP returned first varied per launch, so it read as
> "intermittent". Confirmed from the shell log: `🌐 Resource request: https://example.com/
> (role: tablistpanel)`.
>
> ⛔ **An `href` assertion does NOT catch this** — once the overlay has been navigated, its
> `location.href` really is `https://example.com/`. Three separate harness defects have now produced
> false farbling failures in this project:
> 1. CDP `PUT /json/new` tabs bypass `OnBeforeBrowse`, so they never get a key (fails against a
>    *correct* implementation);
> 2. navigate-sleep-measure can read the *previous document* (invisible, because a fixed synthetic
>    pattern makes both pages' native hashes identical);
> 3. **wrong browser** (this one).
>
> **The fix, and the rule for any future harness:** identify browser chrome **once at startup, by
> CDP target id** (every `127.0.0.1:5137` target except the `/newtab` one) and exclude those ids
> forever; drive only the remaining target. Reference implementation is `canvas_check.py`, whose
> header documents all three defects. **Never identify the tab as "not 127.0.0.1:5137"** — after the
> first navigation an overlay does not match that either.
>
> ### The per-profile assertion (still required, and still not covered by the probe)
>
> `farbling_probe.py --expect-native-canvas` passing is necessary but **not sufficient**: the shipped
> constant-seed bug would have passed every assertion in it. Rotate `profileSeed` in
> `<profile>/fingerprint_settings.json` and restart:
>
> | | seed A | seed B | seed A again |
> |---|---|---|---|
> | exempt `getImageData` | `53225ec8` | `53225ec8` | `53225ec8` |
> | large-canvas CONTROL | `0cdc9b48` | `0cdc9b48` | `0cdc9b48` |
> | **farbled `getImageData`** | `0e4e6251` | **`d9532c84`** | **`0e4e6251`** |
>
> Exempt + control unchanged ⇒ the farbled delta is not render variance. Exact round-trip ⇒ **two §11
> contracts from one experiment**: per-user unlinkability *and* determinism (no login breakage).
>
> ⚠️ **Anyone re-verifying must rotate the seed.** A same-profile run is not evidence; it is exactly
> what was green for two days while nothing worked.

> ## ✅ The fix: FB-1's renderer-side `[Sync]` pull (replaces the push entirely)
>
> Landed as fork commit `116b7fd8b` — **no Chromium patches, no public CEF API, no `CEF_API_HASH`
> churn.** Everything is fork-internal libcef code plus one shell argument.
>
> | Piece | Where |
> |---|---|
> | Shell still sends per navigation, now with **arg 2 = registrable domain** | `simple_handler.cpp :: OnBeforeBrowse` (both branches) |
> | Browser **intercepts** `hodos_farble_key` and files it — never goes on the wire | `libcef/browser/frame_host_impl.cc :: CefFrameHostImpl::SendProcessMessage` |
> | Registry `{registrable domain → key, enabled}`, longest-dot-suffix lookup by host | **NEW** `libcef/browser/hodos_farbling_registry.{h,cc}` |
> | Fork-internal `[Sync] GetHodosFarblingKey(host) => (key_hex, enabled)` | `libcef/common/mojom/cef.mojom :: interface BrowserFrame` |
> | Browser-side handler (answers from browser state only — safe before `FrameAttachedAck`) | `libcef/browser/browser_frame.cc :: CefBrowserFrame::GetHodosFarblingKey` |
> | Renderer **pulls** at context creation | `libcef/renderer/frame_impl.cc :: CefFrameImpl::MaybeApplyHodosFarblingKey`, called from `OnContextCreated` |
>
> **Why the pre-commit send stops being a bug:** it is no longer a delivery, it is a **cache fill**.
> Pre-commit is now *early enough* instead of *too late*, and the browser holds the entry regardless of
> which renderer process the document lands in — which is also the cross-process failure that broke the
> legacy seed path.
>
> **Why `[Sync]` is not a convenience.** Both push directions are excluded, and both were measured:
> a pre-commit push reaches the outgoing document; a post-commit push is queued by
> `SendToBrowserFrame` until the `FrameAttached` ack round-trip completes, which is strictly *after*
> `OnContextCreated`, so it loses to the first inline script. `OnContextCreated` is the only instant
> that is both after the right document exists and before page script runs.
>
> **Why libcef must not re-derive eTLD+1.** `FarblingPolicy::RegistrableDomain` is a deliberately
> hand-rolled reduction (see its header for why it must not be merged with the cookie helper). If the
> browser side reduced independently — e.g. with `net::registry_controlled_domains` — the two could
> disagree and *every* lookup would miss, failing closed and silently. The shell therefore sends the
> domain it used, and the registry only ever does suffix matching on a string it was given.
>
> **Scope + costs:**
> - **Main frame + http/https only**, matching what the shell sends ⇒ one sync round-trip per
>   top-level document, not one per subframe. Subframes/workers are P4e.
> - `pending_farble_key_` and `HandleHodosFarblingKey` are **deleted**; a comment in `frame_impl.h`
>   marks the spot so a per-frame cache is not reintroduced (it cannot work — new `CefFrameImpl` per
>   document).
> - **Single-active-profile assumption**, inherited from `FarblingPolicy::InitializeForProfile` which
>   already caches one seed per browser process. Documented as a tripwire in the registry header.
>
> ~~⚠️ **KNOWN LIMITATION, not a regression:** a **cross-site redirect** (`bit.ly` → `example.com`)
> leaves the registry holding only the pre-redirect site, so the landing page finds no entry and
> **fails closed — unfarbled**. Closing the cross-site case needs a second fill from the shell's
> redirect hook; tracked as a follow-up, not done here.~~
>
> ✅ **REFUTED BY MEASUREMENT 2026-08-09 — there is no cross-site redirect gap, and no follow-up
> is needed.** The limitation above was reasoned from the shape of the code, never measured. It is
> wrong: `SimpleHandler::OnBeforeBrowse` takes an `is_redirect` parameter it deliberately does not
> branch on, and CEF re-fires it on a server redirect, so the `hodos_farble_key` message is sent
> **again** for the landing URL and the registry gets its entry before the landing document's
> `OnContextCreated` pulls.
>
> Experiment — reach one landing document two ways and compare, `youtu.be` → `www.youtube.com`
> being a genuine cross-eTLD+1 redirect:
>
> | | small canvas (farbled) | large canvas (control) |
> |---|---|---|
> | via redirect | `21212854` | `0cdc9b48` |
> | direct navigation | `21212854` | `0cdc9b48` |
>
> ⭐ **With its own negative control**, because identical hashes would *also* be what you'd see if
> **neither** page were farbled. Re-run with farbling disabled for `youtube.com` via the per-site
> opt-out: both arrivals became **`53225ec8`** — which is exactly the native value the auth-exempt
> control page yields — proving `21212854` was a genuinely farbled value and that the experiment
> is sensitive to the feature being off. Same-site host changes remain covered, as documented,
> because entries are keyed by registrable domain.
>
> Harness: `scratchpad/redirect_probe.py` pattern, id-based target selection.
>
> ✅ **CEF BUILD GREEN 2026-08-07** — `BUILD_EXIT=0`, distrib
> `cef_binary_150.0.0-HEAD.3567+g116b7fd+chromium-150.0.7871.187_windows64_minimal`,
> `CEF_COMMIT_HASH 116b7fd8bba50ebf8e6cf2f240744fd4ce9fd282` (= the pin, so the artifact provably
> carries this change). Took 3 attempts; **two real compile errors, both in the plumbing, neither in
> the design** — see the runbook for both, they are M150 porting traps that will recur:
> `GURL::host()` now returns `std::string_view`, and adding one `cef.mojom` `BrowserFrame` method
> obligates **two** implementors (`CefFrameHostImpl` derives from it as well as `CefBrowserFrame`).
>
> ✅ **Behaviourally verified** — see the top of this section: 6/6 across three fresh launches with
> `0` registry misses, plus the per-profile seed-rotation assertion the probe does not cover on its
> own. ⚠️ Verify with a harness that drives the **tab**, not an overlay — that mistake faked an
> "intermittent per-session" bug for hours.

> ## ⭐ ROOT CAUSE PROVEN 2026-08-07 by an instrumented build — read this before touching C2
>
> Timeline for one navigation (from `cef_debug.log` — Chromium `LOG()` goes to `settings.log_file`,
> **not** `debug_output.log`):
>
> ```
> :21.000  CONTEXT-CREATED with NO pending key  url=https://example.com/      frame 11-982F70…
> :26.954  RECV enabled=1 keyPrefix=b0fb635c…   url=https://example.com/      frame 11-982F70…
> :26.954  APPLY-NOW readback_enabled=1                                       frame 11-982F70…
> :26.961  CONTEXT-CREATED with NO pending key  url=https://example.com/?x=1  frame 11-5C4B43…
> ```
>
> 1. **Delivery works and the Supplement takes the key.** `readback_enabled=1` is a genuine read-back
>    of `FarblingEnabled()` from the target `ExecutionContext`, not an echo of the argument. C1, the
>    C2 transport, and C3 are all sound.
> 2. **The key is always ONE DOCUMENT LATE.** The new document's context is created *before* the key
>    arrives; the key lands on the **outgoing** document, which is then discarded.
> 3. ⛔ **Per-frame caching cannot bridge it.** Frame tokens change per document
>    (`11-982F70…` → `11-5C4B43…` → `11-20DEEE…`) and `frame_debug_str_` is built in the constructor,
>    so **each document gets a new `CefFrameImpl`**. A `pending_farble_key_` member therefore lives on
>    an object the next document never sees. The `f429ba1e8` fix was *structurally* incapable of
>    working — this is not a tuning problem.
>
> ⇒ **Any browser-side PUSH sent pre-commit is wrong by construction.** The correct design is the one
> §FB-1 already specified and that was never implemented: **the renderer PULLS the key at
> `OnContextCreated`** — the only moment that is both after the right document exists and before page
> script runs.
>
> Suggested shape that adds **no public CEF API** (so no `CEF_API_HASH` churn): have the browser side
> of libcef intercept the existing `hodos_farble_key` message and cache `{origin → key, enabled}`,
> then add a fork-internal `[Sync]` method on `cef.mojom`'s `BrowserFrame` that the renderer calls
> from `OnContextCreated`, keyed by the document's origin.
>
> ## 🚨 CONFIRMED BY MEASUREMENT — a SHIPPED production privacy bug (2026-08-07)
>
> The legacy `fingerprint_seed` **also** never reaches the renderer. Shipped JS farbling runs on a
> **constant**. Measured with a temporary browser-side log of the computed seed, across two restarts:
>
> | | browser-computed seed | farbled audio output |
> |---|---|---|
> | Run A | `2030444654` | `a10d2ba4` |
> | Run B | `3258985367` | `a10d2ba4` |
>
> The browser computes a correct session-token-derived seed each launch. The farbled output is
> **byte-identical**, so the renderer ignores it and falls back to
> `std::hash<std::string>(url) & 0xFFFFFFFF` — a pure function of the URL, with no session token and
> no profile seed. Controls: the auth-exempt page's audio hash was identical across restarts
> (`84551a93`), proving the measurement is deterministic; the farbled value was also stable across
> repeated away-and-back navigations, so it is not a one-shot-erase artifact.
>
> **Impact on shipped users:** every Hodos user farbles *identically* for a given URL. That is zero
> cross-user unlinkability, and worse, a stable precomputable constant — i.e. a
> **browser-identifying fingerprint**, the exact "worse than no farbling" outcome
> `HodosSessionCache`'s own header warns about.
>
> **Both** the legacy seed and the C2 key fail for the **same** root reason (pre-commit send reaching
> the wrong renderer process/document), so the renderer-side pull fixes both at once. This needs its
> own ticket regardless, because it is live in production today while P4d is several phases away.
>
> 🎫 **Ticket opened: `development-docs/TICKET_farbling_constant_seed_shipped.md`.** Confirmed in every
> released build from `v0.3.0-beta.1` to `v0.3.0-beta.29` (current public Latest) — farbling has never
> worked. ⚠️ **The pull does NOT reach shipped users**: releases are M136, the pull is CEF-150 fork
> code. The release line needs the separate ~5-line **fail-closed** fix (drop the `std::hash(url)`
> fallback, inject nothing when no seed arrives). Owner decision pending in that ticket.
>
> 💡 Related, found the same day: **renderer-process logging is dead.** `Logger::Initialize` is only
> called in the browser process, so every `LOG_*_RENDER` call is a silent no-op and `[RENDER]` has
> never once appeared in `debug_output.log`. Use Chromium's `LOG()` (→ `cef_debug.log`) for renderer
> diagnostics, and consider fixing the logger separately.
>
> ⚠️ **The acceptance harness was itself defective** — see `farbling_probe.py`. It created tabs with
> CDP `PUT /json/new`, and those targets never reach `OnBeforeBrowse`, so they never receive a key at
> all: **it failed against builds where the browser was demonstrably sending the key.** Fixed
> 2026-08-07 to drive an existing CEF tab via `Page.navigate`.

<details><summary>Superseded: the earlier (partial) C2 diagnosis, kept because the mechanism is real</summary>

> **C2 delivered the key to the wrong document, so farbling never ran.** Everything compiled, staged
> and looked green for two days. Caught only by the behavioural half of `farbling_probe.py`.
>
> `CefFrameImpl::ExecuteOnLocalFrame` only **queues** while `context_created_` is false, and that flag
> has exactly one assignment in `frame_impl.cc` — set `true` in `OnContextCreated`, **never reset**.
> From a frame's second document onward it therefore runs **immediately**, and because the browser
> sends `hodos_farble_key` **pre-commit**, that means against the **outgoing** document's
> `LocalDOMWindow`. The incoming document got a key-less `HodosSessionCache`, `FarblingEnabled()` was
> false, and every C3 hook correctly returned the native value.
>
> ⇒ **This plan's claim that `ExecuteOnLocalFrame` gives correct timing "for free" is WRONG** and is
> corrected here. It holds only for a frame's first-ever load. Fix: hold the key in
> `pending_farble_key_`, install it in `OnContextCreated`, overwrite-on-arrival (cancelled navigation)
> + consume-once (`about:blank` must not inherit a key).
>
> **The legacy `fingerprint_seed` path had this right all along** — its renderer handler caches by URL
> and applies at `OnContextCreated`. Reuse-first would have found this: C2 invented a delivery
> mechanism where a working one was sitting beside it.
>
> *(Later correction: the legacy path is **also** delivered too late; it merely hides it behind a
> URL-hash fallback seed. See the proven root cause above — do not copy it.)*

</details>
>
> ⚠️ **FB-1 also specified a lazy `[Sync]` pull on first farbled API call** ("the push alone races the
> first inline script"). That half is **still not implemented**. The push fix above addresses the
> wrong-document bug but **not** the inline-script race — a page whose very first inline script reads
> a canvas before `OnContextCreated`… cannot happen (context creation precedes script), but a
> *subframe* or a worker with no delivery still has no key. Track separately; P4e owns OOP.

### C2 (original plan text)
- **Shell (browser, `cef-native/`):** generate/store `profile_seed`; compute `domain_key`; decide `enabled` (= `!IsAuthDomain(top) && IsSiteEnabled(top)`); deliver at navigation commit + worker start. `#ifdef _WIN32` / `#elif __APPLE__` per Invariant #9; Mac creation paths in `cef_browser_shell_mac.mm`.
- **Blink:** receive `{domain_key, enabled}` into `HodosSessionCache`.

### C3 — Canvas 2D readback `[dep C1]` — ✅ **AUTHORED 2026-08-06, fork `f82b3aae0`, build owed**

> Registered as `hodos_farble_canvas2d` (`condition: HODOS_FARBLING`). Hooks landed exactly as the
> corrected funnel below prescribes; the JS canvas fragment was deleted in the **same** commit (I-4).
> Applies forward to pristine source with no offsets, reverse-checks clean. **Not yet compiled** — the
> shell build does not compile Blink, so nothing beyond "valid patch" is claimed until a CEF build runs.
>
> **One implementation constraint the plan did not state, and a rebase must not "optimise" away:** the
> farbled snapshot on the encode path is a **COPY**. A snapshot can share pixels with the canvas's own
> backing store, and `PerturbPixels` is deterministic — so perturbing that store in place means the next
> read re-flips the same bits and *undoes* the farble, breaking the §11 intra-session-consistency
> criterion. (This is also precisely what the outgoing JS did, via its `getImageData`→`putImageData`
> round-trip.) `getImageData` needs no copy: its `ImageData` is freshly allocated.
>
> ⚠️ **`farbling_probe.py --expect-native-canvas` was NOT sufficient as an acceptance gate** and has
> been extended. It asserted only that the three canvas methods report `[native code]` — true the moment
> the JS fragment is deleted, whether or not any native farbling exists, so it could not have been "C2's
> first behavioural proof" as planned. It now also draws a fixed pattern into a small canvas (inside the
> `<65536px` gate) and a large one (outside it) and asserts the small hashes **differ** between exempt
> and farbled pages while the large hashes **match** — the large canvas being the control that makes the
> comparison sound — plus read-twice stability on both pages.
- **⚠️ Funnel corrected against the real TARGET tree (7871 / Chromium 150, verified 2026-08-05).** `platform/graphics/static_bitmap_image.cc` is **NOT** the shared readback funnel on M150 — it is 118 lines and neither readback path routes through it. Do not patch it. The two real hook points are:
  - **`modules/canvas/canvas2d/base_rendering_context_2d.cc :: BaseRenderingContext2D::getImageDataInternal`** (`:353`) — the single funnel for **both** `getImageData` overloads (`:331`, `:341`) **and** `CanvasRenderingContext2D::getImageDataInternal` (`canvas_rendering_context_2d.cc:774`, which delegates up at `:786`). It is also the path an **OffscreenCanvas in a worker** takes — this one hook is what buys P4a's worker win. `GetTopExecutionContext()` is already in scope inside it.
  - **`core/html/canvas/html_canvas_element.cc :: HTMLCanvasElement::Snapshot`** (`:1270`) — the shared source for `ToDataURLInternal` (`:1312`) and `toBlob` (`:1423`). Prefer hooking the **two encode call sites** over `Snapshot` itself: `Snapshot` has a third caller (`:1896`) that is not a fingerprinting readback.
- `measureText`: gate only (don't perturb text metrics unless §7 chooses to).
- **Preserves today's behavior:** LSB noise on small canvases; `toDataURL`/`toBlob` see already-perturbed pixels. **Does NOT cover WebGL `readPixels`** (that is C4 — confirmed on 7871: framebuffer readback is a separate path and needs its own patch point).
- **⚠️ Why the JS canvas fragment MUST die in this same commit (concrete mechanism, not just hygiene).** Today's JS `toDataURL`/`toBlob` overrides (`FingerprintScript.h:47–71`) implement farbling as a `getImageData` → `putImageData` **round-trip**. Once native canvas farbling is live, that round-trip reads native-farbled pixels and **writes them back into the canvas itself** — mutating the source, not just the readback. Every subsequent read then compounds fresh noise onto already-noised pixels, destroying the intra-session consistency gate (§11) and eventually the image. This is a *corruption* failure, strictly worse than the "double perturbation" the I-4 rule describes elsewhere.

### C4 — WebGL `[dep C1]`
- `modules/webgl/webgl_rendering_context_base.cc`, `webgl2_rendering_context_base.cc`:
  - **`readPixels`** — its **own** patch point (framebuffer readback). Apply pixel noise, matching today's `readPixels` farbling (which we keep).
  - **`getParameter`** — only if §7 chooses to farble `UNMASKED_VENDOR/RENDERER` / `getSupportedExtensions` (recommend **drop** — see §7).

### C5 — WebAudio `[dep C1]`
- `modules/webaudio/audio_buffer.cc` (`getChannelData`), `analyser_handler.cc` / `realtime_analyser.cc` (`getFloatFrequencyData`) — per-sample fudge via `FarbleAudioSample` (BALANCED-equivalent), matching today's `*= 1.0 + (rng()-0.5)*4e-7`.

### C6 — Navigator `[dep C1]`
- `core/frame/navigator_device_memory.cc` — farble `deviceMemory`, **constrained to a desktop-plausible valid set (recommend `{4,8,16,32}`)** — see §7; this is a Hodos decision, *not* Brave's literal `{0.25,0.5,1,2,4,8}`. **NEW vs today (design conflict — §7).**
- `core/execution_context/navigator_base.cc` — `hardwareConcurrency`, **reduced to a plausible value ≤ the real core count** (never inflate). **NEW vs today (§7).**
- `modules/plugins/dom_plugin_array.cc` — keep today's realistic 5-PDF-plugin set (native). *(Note: `navigator.webdriver=false` and the `window.chrome` stub in today's injection are **bot signals, not farbling** — re-home them if we drop the JS block; the `window.chrome` stub at `simple_render_process_handler.cpp :: OnContextCreated` **`:549–573`** (comment `:549`, `isExternalPage` guard recomputed `:551`, object `:559` — **all verified 2026-08-05**; the old `:629-653`/`:634`/`:638` cites were ~80 lines stale) currently stays per Q2 TP-1, but **BOTH** the `webdriver=false` override (`FingerprintScript.h:128–133`) **and** the 5-entry `navigator.plugins` spoof (`FingerprintScript.h:99–126`) live inside the deleted FP script and MUST be preserved elsewhere — see BOT-1.)*

### C7 — Auth-domain exemption at source `[dep C2, Q3]`
- Re-implement **`FingerprintProtection::IsAuthDomain`'s allowlist ONLY** at the browser layer: when top-frame eTLD+1 ∈ allowlist, deliver `enabled=false` → Supplement returns pass-through. **`hodos-unbreak.txt` and adblock scriptlet exemptions are untouched** (adblock concern — Q2 I1). The user per-site toggle (`IsSiteEnabled`) folds into the same `enabled` bit (Q2 TP-2 gap — this plan owns re-homing it). Full design → `Q3-farbling-x-oauth.md`.

### Teardown (M1 — retire, don't orphan) — do as part of P4
Delete `FINGERPRINT_PROTECTION_SCRIPT` injection at `simple_render_process_handler.cpp :: OnContextCreated` **`:501–547`** (**verified 2026-08-05**; both prior cites — the outline's `:586-632` and this plan's own `:581-627` — are stale, the block moved up ~80 lines); retire `FingerprintScript.h`; retire the **JS-injection** parts of `FingerprintProtection.h` (`GetDomainSeed`, `FINGERPRINT_SEED` plumbing, `s_domainSeeds`/`s_seedMutex`, `s_fingerprintDisabledUrls`/`s_fpDisabledMutex`, the `fingerprint_seed`/`fingerprint_site_disabled` IPC at `simple_handler.cpp` `OnBeforeBrowse`); **migrate `IsAuthDomain` into C7**; **preserve** `IsSiteEnabled`/`SetSiteEnabled` + `fingerprint_get/set_site_enabled` IPC (shipped user control — re-home into C2's `enabled` bit, do NOT delete). Keep the adjacent adblock scriptlet block (**`:487–499`**) and `window.chrome` stub (**`:549–573`**) byte-identical (Q2 TP-1/TP-2). **Guard against double-seeding / dead symbols** (Q2 T8 grep sweep).

**Incremental teardown rule (I-4 — how to retire a monolithic JS constant without double-farbling):** `FINGERPRINT_PROTECTION_SCRIPT` is a single embedded JS string that wraps `toDataURL`/`getImageData`/`readPixels`/`getChannelData`/etc. Do **not** try to keep the whole constant alive while native patches land piecemeal — instead **decompose it per-API** so each API's JS override is a separately removable fragment, and **delete a fragment in the exact same step its native patch lands** (canvas JS fragment removed in P4a; WebGL in P4b; audio in P4c). This makes teardown **atomic per value**, which is what actually prevents double-perturbation: if the native canvas farble runs at API-call time *and* the JS `toDataURL`/`getImageData` wrapper is still present, the JS layer would re-perturb the already-native-farbled pixels and, because it re-seeds from its own (soon-dead) `s_domainSeeds` path, would break intra-session consistency. **Because deletion is atomic (native-in / JS-out in one commit), no runtime "double-farbling guard flag" is needed** — there is never a window where both layers wrap the same API. (The earlier "guard flag" phrasing is superseded by this atomic-swap rule; a flag would only be required if a value's JS override could not be removed in the same step, which we forbid.)

---

## 7. Design-conflict reconciliation — per-value farble-vs-omit table (feeds Q5)

The current JS impl (`FingerprintScript.h` header comments) **deliberately dropped** screen resolution, `hardwareConcurrency`, `deviceMemory`, and WebGL vendor/renderer as "detectable / low-entropy / cross-referenced." `B1-farbling-design.md` **re-adds** three of them. Resolve each now (owner default 2026-06-17 Q18 = **Brave-*technique* parity unless concrete breakage** — parity means "adopt Brave's *approach* (valid-set constraint, reduce-only cores, per-eTLD+1 seed)", **not** copy Brave's literal value sets, several of which are mobile-tuned and wrong for a desktop browser; see C-1 corrections below):

| Value | Today (JS) | Recommended default (native) | Reasoning |
|---|---|---|---|
| **Canvas `getImageData`/`toDataURL`/`toBlob`** | farbled (LSB, <65536px) | **Farble** (C3) | Highest-signal vector; native removes the toString tell. Keep the small-canvas gate. |
| **WebGL `readPixels`** | farbled (LSB) | **Farble** (C4, own patch point) | High-signal; already shipped; keep. |
| **WebAudio** | farbled (fudge) | **Farble** (C5) — but with a **delta floor**, see below | High-signal; already shipped; keep. ⚠️ **CORRECTED 2026-08-10 by measurement:** "equivalent to today's `*= 1.0 + (rng()-0.5)*4e-7`" is not implementable as written — audio samples are float32, so any `\|delta\|` below ~`2^-24` relative rounds every sample straight back to itself. ~15% of profile+domain draws were a **complete no-op** and ~30% dead or degraded; the JS had the identical hole, so this shipped broken in every release. `\|delta\|` is now confined to `[2^-23, 2e-7]` (fork `c63654654`). Ceiling unchanged, so never louder than specified. |
| **navigator.plugins** | fake 5-PDF set (**wrong** — said `"Chrome PDF Plugin"`) | ✅ **DONE 2026-08-05 (BOT-1): JS spoof DELETED, native kept. No C6 work.** | Native is the spec'd whatwg/html#6738 list and we build `enable_pdf=true`, so it is present and correct. Our spoof named the pre-2021 `"Chrome PDF Plugin"` where the spec says `"Chromium PDF Viewer"` — a plugin list no real Chrome has. |
| **navigator.webdriver** | `false` | ✅ **DONE 2026-08-05 (BOT-1): JS override DELETED, not re-homed.** | Native is already `false` (two independent margins — see FB-4). Re-homing would define an own-property accessor where Chrome has a prototype accessor: a tamper tell. Guarded by the `remote_debugging_port` TRIPWIRE + `farbling_probe.py`. |
| **deviceMemory** | **omitted** | **Farble, constrained to a desktop-plausible valid set** (recommend `{4,8,16,32}`) | The original JS drop reason (`FingerprintScript.h:12`) was **low entropy (~3–4 bits)**, *not* a perf mismatch — so re-adding buys little privacy while adding surface + a high-churn rebase target. Re-add anyway **only because an out-of-set / absent value is itself a tell**; the win is *parity with real desktops*, so the set must be desktop-plausible. **NOT Brave-verbatim:** Brave's `{0.25,0.5,1,2,4,8}` is mobile-inclusive and *caps at 8*; modern desktop Chrome can report 16/32, so `{4,8,16,32}` is the Hodos-justified desktop set. Never emit a value the machine's real spec makes implausible. Owner note: dropping this entirely is a defensible alternative (accept the ~3–4-bit gap). |
| **hardwareConcurrency** | **omitted** | **Farble to a plausible value ≤ real core count** (Brave's `[2, real]` reduce-only rule), NOT a fixed set | Same low-entropy trade-off as deviceMemory. **Do NOT use a fixed set like `{4,8,12,16}`** — that can *inflate* a 4-core box to 16, which is implausible and cross-referenceable against real CPU perf (`performance.now()` timing, benchmark cores) — the exact detection vector we are avoiding. Constrain to **≤ real, plausible** (reduce, never inflate), matching Brave's actual clamp. |
| **WebGL `UNMASKED_VENDOR`/`RENDERER`** | **omitted** | **DROP (recommended) unless a common-GPU-string map is built** incl. **Apple Silicon + Intel-Mac ANGLE** strings (I8) | Random strings are *more* unique than the truth and create inconsistency with the real extension list (the JS comment was right). Only re-add if mapping to a *small set of real GPU strings*; never noise. This is the load-bearing OPEN item for Mac (Q1). |
| **Screen / `devicePixelRatio`** | omitted | **Omit (accepted gap)** | Only ~3-4 bits; high breakage; JS impl dropped it deliberately. Log as accepted. |
| **getClientRects / font metrics beyond measureText** | omitted | **Omit (accepted gap) for beta.1** | Not scoped; revisit post-beta if CreepJS flags. Log. |
| **UA-CH high-entropy client hints** (`getHighEntropyValues`) | omitted | **Omit (accepted gap), log** | Brave farbles UA via `FarbledUserAgent`; out of beta.1 scope — record explicitly (M2). |
| **enumerateDevices** | omitted | **Omit (accepted gap), log** | Not scoped (M2). |

**Net for Q5:** re-add deviceMemory (desktop-plausible set, or drop — owner call) + hardwareConcurrency (reduce-only, ≤ real cores); **drop WebGL vendor/renderer** (or build the GPU-string map — Mac owns its entries, Q1); keep canvas/WebGL-readPixels/audio/plugins/webdriver; explicitly log screen/DPR, getClientRects, fonts, UA-CH, enumerateDevices as **accepted gaps**. Owner sign-off required on the WebGL vendor/renderer call.

---

## 8. Incremental landing order (maps to outline P4a–P4e)

1. **P4a — C1 Supplement + C2 delivery → WORKER-COVERAGE QUICK WIN.** Ship the Supplement with **Canvas (C3) only**, keyed by the persistent per-profile seed, covering window + in-process workers. This closes the window-vs-worker mismatch **for canvas — the single highest-signal vector** — and proves the seed/delivery channel end-to-end. **WebGL and audio worker parity do NOT ship here** — they remain JS-injected (which never fires in workers) until P4b/P4c, so a worker probe still shows WebGL/audio window-vs-worker mismatch until then. Delete the corresponding JS **canvas** block in this same step (see I-4 teardown rule); keep the WebGL/audio JS blocks until their native replacements land.
2. **P4b — C4 WebGL (incl. `readPixels` own patch point) + resolve §7 vendor/renderer.** Delete JS WebGL block.
3. **P4c — C5 Audio + C6 Navigator (valid-set constrained).** Delete JS audio + finalize navigator; re-home `webdriver`/`window.chrome`.
4. **P4d — C7 auth-domain exemption (IsAuthDomain re-impl) + user per-site toggle re-home (Q3).** Now the JS block + FP IPC can be fully torn down (M1 complete).
5. **P4e — OOP seed plumbing (§5): shared/service-worker startup + cross-site (OOP) iframe top-frame-key delivery at subframe commit.** Then the full worker **and** cross-site-iframe acceptance rows (§11) can go green.

Each step is independently smoke-testable; each **atomically** deletes its own JS counterpart in the same commit its native patch lands (I-4 rule in §6 Teardown), so the two paths never both farble the same value — no runtime guard flag required (Q2 T5 double-wrap is about adblock scriptlets, a separate concern).

---

## 9. License / clean-room plan (M7 — do this right)

- **Re-implement the technique in a genuine clean room.** Brave's farbling files are **MPL-2.0 (file-level copyleft)**: copying their `.cc`/`.h` text obligates *those files* to stay MPL and be offered to users. **Transcribing Brave's logic while reading its MPL source is still derivative-work risk** — maintain a real boundary: read the *behavior/spec* (the fingerprinting-defenses blog, the value tables in this doc, CreepJS expectations), and Brave's *public issue discussions*, then write our patches from that behavioral spec, not from their source buffer open in another window.
- **`fingerprint-chromium` (BSD-3, permissive)** may be read/adapted for structure, **but its WebGL-metadata path is Linux-only** → Win/Mac must re-implement regardless (outline M7). *(Inherited-from-outline claim "Chrome 144 removed the flags it used" is oddly precise and **unverified against a primary source** — treat as "verify at plan time," not as established fact; it does not change the Win/Mac re-implement conclusion either way.)*
- **Bromite = GPL-3 — FORBIDDEN.** Do not open Bromite farbling code.
- Record the clean-room boundary in the commit/PR description for provenance.

---

## 10. Rebase cadence (the recurring cost — the real stable-vs-LTS lever)

Our patch targets are **high-churn Blink files** — `base_rendering_context_2d.cc` (Canvas2D internals get refactored), `webgl_rendering_context_base.cc`, `static_bitmap_image.cc`, `navigator_base.cc` — so they will **conflict on most milestone jumps**. Estimate **~2–8 h per Chromium/CEF bump** to rebase **~5–8 patches** (single figure used doc-wide, matching `B1-farbling-design.md` and the outline); `base_rendering_context_2d.cc` is the riskiest. Mitigations:
- **Minimize each patch's surface** — insert a single call into the existing readback function rather than restructuring it; keep perturbation logic in `hodos_session_cache.cc` (a *new* file, which never conflicts) so patches on Chromium files are one-liners.
- Wire the **Step 5.5 patch drift-audit hook** (re-apply patches, report fuzz/offsets) into the fork toolchain (outline §3b) so a bump surfaces conflicts before a 10–12 h build.
- Feed the measured per-bump rebase hours into `CEF_VERSION_UPDATE_TRACKER.md` (outline §7) — this number is the primary input to the LTS-vs-stable decision (§2 Step 0).

---

## 11. Acceptance criteria (B1 gate — maps to outline §7 "Farbling")

Run on **both** Windows and macOS, with adblock ON (Q2 co-existence):
- [ ] **CreepJS: zero "lies"** on canvas/WebGL/audio (`.toString()` returns `[native code]` — proves native, below JS). This is the single most valuable assertion (Q2 T6).
- [ ] **worker column == window column** for canvas/WebGL/audio — including **service-worker, shared-worker, and OffscreenCanvas-in-worker**, not just CreepJS's dedicated-worker column (I2 / §5). ⛔ **RED, and wider than this row assumed — measured 2026-08-10: NO worker is farbled, in-process dedicated workers included** (§5 box). The claim "P4a satisfies dedicated; P4e satisfies OOP" is **false**; P4a satisfies neither. Deferred by owner decision, logged as a known gap. **CreepJS only exercises the dedicated-worker column, so the OOP cases need a purpose-built harness** — a **P4e deliverable**: a small test page that, inside each worker type (dedicated, shared, service), builds a fingerprint via `OffscreenCanvas` + a WebGL context readback + an OfflineAudioContext render, posts the values back to the page, and asserts they **equal the window-context values** for the same profile+domain. Service workers have no DOM, so the harness must construct the readback from `OffscreenCanvas`/WebGL, not `<canvas>`. Without this harness the row is a checkbox no one can check.
- [ ] **Intra-session consistency:** same read twice in one session+domain → **identical** perturbation (load-bearing for site correctness).
- [x] **Cross-profile difference:** same site in two profiles → different farbled values.
      ✅ **Windows 2026-08-10** (`c63654654`), harness `chromium-rebuild/farbling_cross_profile_check.py`.
      `Default` vs `Profile_1`, independently CSPRNG-seeded, both on `example.com`:
      canvas `0e4e6251` vs `4e5a3154`, WebGL `7da64265` vs `db9131b4`, audio `e8ed8449` vs `7cac00dc`,
      while **all five controls held still** (exempt canvas/WebGL/audio + both outside-the-gate
      readbacks). **Negative control:** copying profile A's seed into profile B collapsed *every*
      farbled value to A's exactly — including navigator `(32,10)` — so the difference is entirely
      seed-derived and the harness does go red. macOS owed.
      ⚠️ Trap this cost a run: **the CDP port is derived from the profile id**
      (`cef_browser_shell.cpp`: `Default`→9222, `Profile_<N>`→9222+N, +100 under `HODOS_DEV`), so the
      second profile is on a *different port* and a single-port harness reports "the browser failed
      to start" three times. Mirrored in the harness as `cdp_port_for()`.
- [ ] **Cross-site iframe:** a third-party origin embedded in two different first parties → **different** values (first-party/top-frame keying works — I4). Because a cross-site iframe is **out-of-process** (default site isolation), this requires the browser to deliver the *top-frame* key to the subframe process — satisfied by **P4e**, not P4a; verify only after P4e lands.
- [x] **Cross-session login test (THE important one):** create an account → restart browser → revisit → appears as the **same device**, logins do **not** break (persistent per-profile seed working).
      ✅ **Windows 2026-08-10** (`c63654654`) — target **`www.youtube.com/feed/history`**, a farbled
      (non-exempt) origin with a real Google session. Logged in before the restart, **still logged in
      after** a real kill-and-relaunch, and the farbled fingerprint came back **byte-identical**
      (canvas `21212854`, WebGL `b32263b5`, audio `228f5d27` — same three values both phases).
      **Negative control:** rotating the profile seed between the phases moved all three
      (`8ce62979` / `4c62b8d5` / `175fa176`), so the harness does go red when the seed stops being
      persistent. **Positive control** on the login detector: soundcloud read as logged out in both
      runs, so "logged in" is a discriminated answer, not the detector's only output.
      ⚠️ **Recorded honestly:** with the seed rotated YouTube *stayed* logged in — it does not bind
      its session to a canvas fingerprint. So this run proves the **persistence guarantee holds**;
      it does not itself demonstrate that a rotating seed breaks logins. The guarantee matters for
      sites that *do* bind, which is why the row is about determinism, not about YouTube.
      Harness: `chromium-rebuild/farbling_cross_session_login_check.py` (Windows, 2026-08-10).
      ⛔ **The vacuous-pass finding, which is the reason this row is not already green:** the only
      logins in the dev profile are **x.com and github.com**, and *both are on
      `FingerprintProtection::IsAuthDomain`'s allowlist* — i.e. not farbled at all. Testing there
      would have produced a confident green about a page nothing farbles. The harness therefore
      parses the allowlist out of `FingerprintProtection.h` **at runtime** (never copied, so it
      cannot drift) and **refuses** an exempt target; verified refusing both x.com and github.com.
      Probed for a session on non-exempt sites — soundcloud, reddit, handcash, alltrails, gitbook —
      all logged out, so there is nothing to carry across a restart yet. The harness also carries a
      positive control on the login detector (a known-logged-out URL must read as logged out) and a
      negative control that rotates the seed between the two phases and asserts the "fingerprint
      survived the restart" half goes RED. **Needs: one hand-made login on any non-exempt site.**
      Note the *mechanism* this row protects — an identical fingerprint across a real restart — is
      already proven by the seed-rotation harness's exact A→B→A round trip.
- [ ] Navigator values within the **standard valid set** (deviceMemory in the desktop set or dropped; hardwareConcurrency ≤ real cores); WebGL vendor/renderer decision applied per §7 — **either "drop" (Mac GPU-string entries then NOT required and must not block this gate) OR "common-string map" (then Mac ANGLE entries required, FB-6)**. Read the checkbox against whichever FB-2 decision was taken.
- [x] **No stable secret on any renderer command line** (C2 threat model): verify via ProcessExplorer/`ps` that no per-profile secret appears on a child cmdline.
      ✅ **Windows 2026-08-10** (`c63654654`), harness `chromium-rebuild/farbling_cmdline_seed_check.py`
      — reads the **live** `Win32_Process.CommandLine` of all 16 processes under the build directory,
      after visiting a farbled and an exempt origin so a domain key actually exists to leak. Searched:
      the profile seed (hex both cases + base64), the derived `HMAC-SHA256(seed, eTLD+1)` for both
      origins, and any switch whose whole value is a 32+ char hex string. **Zero hits.**
      **Positive control** (the point of the exercise — `Win32_Process.CommandLine` returns *empty*
      for processes the caller cannot open, so a blind scan reports a triumphant clean sweep having
      read nothing): 16/16 command lines readable, `--type=renderer` seen, `--profile=` seen; the run
      aborts as **BLIND** rather than passing if any of those fail. **Detector self-test** (`--self-test`)
      plants the real seed and the real domain key into a synthetic process table and asserts both are
      caught while the genuine `--gpu-preferences` blob is not — that blob was a false positive from
      the first, looser regex (its base64 contains a 32-char run of `A`/`B`, which are hex digits),
      and tightening a detector without re-proving it detects is how a check becomes decorative.
- [ ] OAuth/auth-domain exemption (C7) verified: pre-approved sites un-farbled and logging in (Q3); user per-site toggle still works.
- [ ] **Stability soak + renderer-crash-rate** not elevated vs the 136 baseline; **canvas/WebGL readback perf** within budget.
- [x] **BOT-1 bot signals (✅ met 2026-08-05, ahead of P4a; re-assert every release via `farbling_probe.py`):** on **both** an auth-exempt page and a farbled page — `navigator.webdriver === false`, `webdriver` is **NOT** an own property of `navigator`, `webdriver` **IS** a `Navigator.prototype` accessor (native shape), and `navigator.plugins` equals the spec'd 5-entry list exactly with `filename == "internal-pdf-viewer"`. Identical on both page classes is the point: a per-site farbling opt-out must not change the bot signature.
- [ ] Adblock intact incl. YouTube CefResponseFilter + cosmetic/scriptlet (Q2 T1–T8); `window.chrome` stub survived JS-block deletion.

---

## 12. Cross-platform split (feeds Q1)

**One shared cross-platform Blink patch set + one shared `hodos_session_cache.cc`, compiled into each OS's binary; the build is a full first-class parallel effort per OS** (outline §5, I8). Windows (lead) authors the toolchain, patches, and the seed wiring; **Mac inherits the patches** and owns: the framework build (not DLL), the **arm64/x64/universal2 arch decision**, minos/plist wiring, the per-profile-seed platform conditionals in `cef_browser_shell_mac.mm`, macOS farbling acceptance, and — load-bearing — the **Mac GPU-string entries** (Apple Silicon *and* Intel-Mac ANGLE) **if** §7 chooses to farble WebGL vendor/renderer. Coordinate via `CHROMIUM_BUILD_RELAY.md`. → `Q1-mac-farbling.md` expands.

---

## 13. Risks

| Risk | Likelihood | Mitigation |
|---|---|---|
| `base_rendering_context_2d.cc` / canvas internals refactored on a bump → patch conflict | Med-High | One-line hook + logic in the new file (§10); Step 5.5 drift audit before the 10–12 h build. |
| Master seed accidentally reaches a child cmdline (C2 violation) | Med | Deliver only derived `domain_key`; ProcessExplorer/`ps` gate in §11; never mirror Brave's cmdline token. |
| OOP-worker key not delivered → worker leaks raw values (silent regression) | Med | P4e explicit; until then log the gap and gate the worker==window acceptance on it (§5/§11). |
| WebGL vendor/renderer farbling *increases* uniqueness (random strings) | Med | Default **drop**; only re-add via a small real-GPU-string map incl. Mac ANGLE (§7); owner sign-off. |
| Double-farbling during migration (JS block + native both active) | Low-Med | Atomic per-value teardown: delete each JS fragment in the *same commit* its native patch lands (I-4 rule, §6 Teardown) → no overlap window, no guard flag needed; T8 grep sweep for dead symbols. |
| MPL-2.0 derivative-work contamination from reading Brave source | Low | Genuine clean-room boundary (§9); provenance in PR. |
| Perf regression on `readPixels`/`getImageData` hot paths | Low-Med | Small-canvas gate preserved; perf gate in §11; perturb only readback, not every draw. |

---

## 14. Open questions → recommended defaults

| # | Question | Recommended default | Why |
|---|---|---|---|
| FB-1 | Seed delivery channel — mojo/commit-params per-navigation (A) vs ephemeral-nonce-cmdline (B)? | ✅ **CLOSED 2026-08-05 — (A′): push over CEF's existing per-frame mojo channel at commit + lazy `[Sync]` pull on first use.** See §4. | Browser half is **fork-local, zero Chromium patches** (`cef.mojom` + `CefBrowserFrame` already exist); the sync pull removes the async first-script race that was (B)'s only argument. Master seed stays browser-side. |
| FB-2 | WebGL `UNMASKED_VENDOR/RENDERER` — farble or drop? | ✅ **CLOSED 2026-08-05 — DROP.** No GPU-string map. | Random strings are more unique than truth, and contradict the real extension list — today's JS drop was correct. Consequence: C4's `getParameter` hook is unnecessary, and **FB-6 never opens** (Mac ANGLE strings not required; must not block the §11 gate). |
| FB-3 | Service-worker key scope — registration-scope eTLD+1 vs top-frame? | **Registration-scope eTLD+1** | Matches SW origin semantics; top-frame is undefined for a background SW. Confirm in P4e. |
| FB-4 | Re-home `navigator.webdriver=false` + `window.chrome` stub where? | ✅ **CLOSED 2026-08-05 — measure native first, then re-home what actually differs** (BOT-1, own commit, before any teardown). Scope is **`webdriver` AND `plugins`**, not just `webdriver`. | They are bot signals, not farbling; must survive **both** the JS-block teardown **and** a per-site farbling opt-out — a user disabling farbling must not thereby announce they are automated. **New evidence (2026-08-05):** we pass **no `--enable-automation`** (`simple_app.cpp:78–139`), so `navigator.webdriver` is very likely natively `false`; and today the whole FP block is skipped on every `IsAuthDomain` site, so github.com / x.com / accounts.google.com / whatsonchain.com **already run unspoofed and work**. So the shims may be defending a condition that no longer exists. Measure, re-home what genuinely differs, and keep a guarded shim for the rest so a future flag change can't regress us silently. |
| FB-5 | Ship farbling behind an optional `condition` build gate (outline OQ-12)? | **Yes** | Escape hatch to toggle if it destabilizes beta.1 without a full rollback. |
| FB-6 | Mac WebGL string set (if FB-2 = map) — which ANGLE strings? | **Defer to `Q1-mac-farbling.md`** (Apple Silicon + Intel-Mac) | Mac owns its GPU strings; blocking only if FB-2 chooses the map. |
| FB-7 | hardwareConcurrency — fixed set vs reduce-only? | **Reduce-only: random plausible value ≤ real core count** (Brave's `[2, real]`) | A fixed set can *inflate* a low-core machine (4→16) — implausible + cross-referenceable against real CPU perf. Reduce-only never exceeds the truth. Matches Brave's actual clamp. |
| FB-8 | deviceMemory valid set — which values, or drop? | **`{4,8,16,32}` (desktop-plausible)**, or drop entirely | Re-add buys only ~3–4 bits but avoids an out-of-set tell; if kept, use a desktop set (NOT Brave's mobile-inclusive `{0.25..8}`). Dropping is a defensible owner alternative. |

---

*Feeds `Q1-mac-farbling.md` (Mac build/arch/GPU strings), `Q3-farbling-x-oauth.md` (C7 exemption), and `Q5-full-edit-list.md` (the §7 value table becomes the final reconciled rows). This doc stops at a followable plan; the implementing session lands C1–C7 in the order of §8 against the real TARGET build once P3 (patch toolchain) is green.*

---

### Sources (primary)
- Brave — Fingerprinting Defenses 2.0 (per-session, per-eTLD+1 seed; canvas + WebAudio farbling model): https://brave.com/privacy-updates/4-fingerprinting-defenses-2.0/
- brave-core — `BraveSessionCache` Supplement / `From()` / HMAC-SHA256 domain key / session-token cmdline (`execution_context.cc`): https://github.com/brave/brave-core/blob/master/chromium_src/third_party/blink/renderer/core/execution_context/execution_context.cc
- brave-core — `FarbleDeviceMemory` valid-set technique; **Brave's actual set is `{0.25, 0.5, 1, 2, 4, 8}` (mobile-inclusive, caps at 8)** — we adopt the *technique*, not the values (`navigator_device_memory.cc`): https://github.com/brave/brave-core/blob/master/chromium_src/third_party/blink/renderer/core/frame/navigator_device_memory.cc
- brave-core — WebGL `getParameter` UNMASKED_RENDERER farbling macro (`webgl_rendering_context_base.cc`): https://github.com/brave/brave-core/blob/master/chromium_src/third_party/blink/renderer/modules/webgl/webgl_rendering_context_base.cc
- brave-core — WebAudio farbling origin PR (session token perturbs webaudio): https://github.com/brave/brave-core/pull/4597
- brave-browser — OOP worker farbling follow-up (worker seed plumbing is not automatic): https://github.com/brave/brave-browser/issues/42427
- CEF — Branches & Building (patch.cfg / patcher.py source-edit mechanism): https://chromiumembedded.github.io/cef/branches_and_building.html
- MDN — WEBGL_debug_renderer_info (UNMASKED_VENDOR/RENDERER semantics): https://developer.mozilla.org/en-US/docs/Web/API/WEBGL_debug_renderer_info
- In-repo: `0.4.0/B1-farbling-design.md`, `0.4.0/CHROMIUM_CEF_BUILD_DESIGN_OUTLINE.md` §3c/§5/§7, `DevOps-CICD/research/BRAVE_FORK_FEASIBILITY.md` §B1, `chromium-rebuild/Q2_farbling_adblock.md`; working-tree cites `cef-native/include/core/FingerprintScript.h` (plugins `:99–126`, webdriver `:128–133`), `FingerprintProtection.h` — cited **by symbol**, because this commit's own docstring fix shifted every line below `:72` by +6: `:: Initialize` (`CryptGenRandom :47`), `:: IsSiteEnabled` / `:: SetSiteEnabled` (`:129`/`:141`), `:: LoadSiteSettings` / `:: SaveSiteSettings` (`:155`/`:177`), `:: IsAuthDomain` (`:195–276`), `:: ExtractDomain` (`:281`, host-only), `src/handlers/simple_render_process_handler.cpp :: OnContextCreated` **`:471–573`** (FP block `:501–547`), `simple_handler.cpp :: OnBeforeBrowse` **`:7521–7618`** (FP IPC `:7578–7615`) — **all re-verified 2026-08-05 at the P4a kickoff; the 2026-07-10 numbers throughout this doc had drifted 48–95 lines.**
