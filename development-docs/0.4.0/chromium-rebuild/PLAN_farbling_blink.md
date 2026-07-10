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

**Delivery channel — two options; pick in this plan's implementation step (open C2 item):**
- **(A) RECOMMENDED — per-navigation delivery over a mojo/commit-params channel** (a small `blink::mojom` interface method, or `{domain_key, enabled}` carried as commit data alongside `blink::mojom::CommitNavigationParams` at each navigation commit — **not** a "pref," which is not a per-document channel): browser sends `{domain_key, enabled}` at navigation commit and at worker/worklet/OOP-subframe startup. No secret on cmdline; master seed stays browser-side. Cost: one small mojo interface (or commit-params field) + the worker/OOP-subframe delivery hook (P4e).
- **(B) FALLBACK — ephemeral per-launch nonce on cmdline** (Brave-shaped): a random per-launch handle on the child cmdline that the renderer presents back to the browser to fetch its `domain_key`. Simpler to wire (mirrors `kBraveSessionToken` plumbing) but adds a round-trip and still needs a browser-side nonce→profile map. Use only if (A)'s mojo plumbing proves heavy on the TARGET branch.

**Persistence store:** reuse the existing per-profile fingerprint file (`FingerprintProtection::LoadSiteSettings` already reads `%APPDATA%/HodosBrowser/<profile>/fingerprint_settings.json`) — add a `profileSeed` field (32B, base64), generated on first run with a platform CSPRNG. **Note:** existing code uses `CryptGenRandom` (`FingerprintProtection.h:47`), which Microsoft has **deprecated** in favor of `BCryptGenRandom` — prefer **`BCryptGenRandom`** for this *new* seed generation (Windows) / `SecRandomCopyBytes` (macOS); the deprecation is not introduced by this plan but new code should not extend the deprecated call. Reset only when the user clears that profile's data. **This is browsing-privacy state, never wallet/key state** (Invariant #1/#2 untouched).

**Reset semantics:** clearing a single site's data → re-derive nothing (domain_key is deterministic from the unchanged profile_seed); resetting the profile / "clear on exit" for fingerprint → regenerate `profile_seed` (new fingerprint everywhere). Document so the "Clear data on exit" path (a live prime-suspect for other regressions) has defined farbling behavior.

---

## 5. Worker / worklet / OOP-iframe coverage matrix (the "not free" part — I2 + C-2)

The Supplement covers any `ExecutionContext` **once the `domain_key` reaches that context's process.** In-process contexts inherit it; **out-of-process ones — OOP workers *and* cross-site iframes under default site isolation — need explicit cross-process delivery.**

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

**OOP cross-site iframe delivery (same cross-process problem, called out explicitly):** under default site isolation a third-party iframe runs in its own renderer process that only knows *its own* origin — it cannot compute the top-frame eTLD+1 key itself, and the plan's headline design (browser computes `HMAC(profile_seed, first-party eTLD+1)` and delivers only the derived key) *requires* the browser to hand the **top-frame-derived** `{domain_key, enabled}` to that subframe process. The browser knows the full frame tree, so at each cross-site subframe navigation commit it delivers the **top frame's** key (this is what makes the §11 "cross-site iframe → different values across two first parties" gate pass). This is the same delivery machinery as OOP workers → **folded into P4e** (or landed alongside it). It is **not** covered by P4a's same-renderer navigation-commit path.

Brave hit exactly this seam (worker farbling follow-ups: brave-browser #42427 / #28904 — `WorkerContentSettingsClient` plumbing for OOP workers). **P4e enumerates the worker-start hook** (`WorkerThread`/`WorkerGlobalScope` init) *and the OOP-subframe commit hook*, and delivers the top-frame key there. Until P4e lands, ship P4a (window + same-site iframes + in-process workers) — which already closes the CreepJS dedicated-worker column — and **log OOP-worker + OOP-iframe coverage as a known gap**, not a silent one.

---

## 6. The exact Blink files to patch + change per file

Highest fingerprint value first. All paths are `third_party/blink/renderer/...`. Each is an **in-place hook**: call `HodosSessionCache::From(*execution_context)`, early-return the native result if `!FarblingEnabledForThisContext()`, else perturb via the Supplement. Register every `.patch` in `cef/patch/patch.cfg` (P3 must exist first).

### C1 — Supplement (new file + hooks) `[foundation]`
- **New:** `hodos_session_cache.{h,cc}` under `core/execution_context/` (or a Hodos subdir added to the Blink BUILD.gn via patch). Defines `HodosSessionCache`, `From()`, `MakePrng()`, `PerturbPixels()`, `FarbleAudioSample()`, device-value helpers, `FarblingEnabledForThisContext()`.
- **Hook:** `core/execution_context/execution_context.{h,cc}` — nothing behavioral; just make the Supplement attachable (Brave's shadow lives here — we insert the include/Provide hook instead).
- **Seed intake:** the C2 delivery target (mojo method impl or cmdline-nonce reader) — see §4.

### C2 — Seed/enabled delivery (wiring, mostly shell-side) `[dep C1]`
- **Shell (browser, `cef-native/`):** generate/store `profile_seed`; compute `domain_key`; decide `enabled` (= `!IsAuthDomain(top) && IsSiteEnabled(top)`); deliver at navigation commit + worker start. `#ifdef _WIN32` / `#elif __APPLE__` per Invariant #9; Mac creation paths in `cef_browser_shell_mac.mm`.
- **Blink:** receive `{domain_key, enabled}` into `HodosSessionCache`.

### C3 — Canvas 2D readback `[dep C1]`
- `modules/canvas/canvas2d/base_rendering_context_2d.cc`, `canvas_rendering_context_2d.cc` — hook the readback of `getImageData` (and the pixel source feeding `toDataURL`/`toBlob`); apply `PerturbPixels`. Prefer the **shared bitmap-readback path** `platform/graphics/static_bitmap_image.cc` so both `getImageData` and the `toDataURL`/`toBlob` encode path funnel through one perturbation site (fewer patch points, matches today's LSB-flip behavior).
- `measureText`: gate only (don't perturb text metrics unless §7 chooses to).
- **Preserves today's behavior:** LSB noise on small canvases; `toDataURL`/`toBlob` see already-perturbed pixels. **Does NOT cover WebGL `readPixels`** (that is C4 — framebuffer readback does not route through `StaticBitmapImage`; **verify the code path before sizing C3/C4** per outline).

### C4 — WebGL `[dep C1]`
- `modules/webgl/webgl_rendering_context_base.cc`, `webgl2_rendering_context_base.cc`:
  - **`readPixels`** — its **own** patch point (framebuffer readback). Apply pixel noise, matching today's `readPixels` farbling (which we keep).
  - **`getParameter`** — only if §7 chooses to farble `UNMASKED_VENDOR/RENDERER` / `getSupportedExtensions` (recommend **drop** — see §7).

### C5 — WebAudio `[dep C1]`
- `modules/webaudio/audio_buffer.cc` (`getChannelData`), `analyser_handler.cc` / `realtime_analyser.cc` (`getFloatFrequencyData`) — per-sample fudge via `FarbleAudioSample` (BALANCED-equivalent), matching today's `*= 1.0 + (rng()-0.5)*4e-7`.

### C6 — Navigator `[dep C1]`
- `core/frame/navigator_device_memory.cc` — farble `deviceMemory`, **constrained to a desktop-plausible valid set (recommend `{4,8,16,32}`)** — see §7; this is a Hodos decision, *not* Brave's literal `{0.25,0.5,1,2,4,8}`. **NEW vs today (design conflict — §7).**
- `core/execution_context/navigator_base.cc` — `hardwareConcurrency`, **reduced to a plausible value ≤ the real core count** (never inflate). **NEW vs today (§7).**
- `modules/plugins/dom_plugin_array.cc` — keep today's realistic 5-PDF-plugin set (native). *(Note: `navigator.webdriver=false` and the `window.chrome` stub in today's injection are **bot signals, not farbling** — re-home them if we drop the JS block; the `window.chrome` stub at `simple_render_process_handler.cpp:629-653` (comment `:629`, `isExternalPage` guard `:634`, object `:638` — reconciled with Q2's `:629-653`) currently stays per Q2 TP-1, but the `webdriver=false` override lives inside the deleted FP script and MUST be preserved elsewhere.)*

### C7 — Auth-domain exemption at source `[dep C2, Q3]`
- Re-implement **`FingerprintProtection::IsAuthDomain`'s allowlist ONLY** at the browser layer: when top-frame eTLD+1 ∈ allowlist, deliver `enabled=false` → Supplement returns pass-through. **`hodos-unbreak.txt` and adblock scriptlet exemptions are untouched** (adblock concern — Q2 I1). The user per-site toggle (`IsSiteEnabled`) folds into the same `enabled` bit (Q2 TP-2 gap — this plan owns re-homing it). Full design → `Q3-farbling-x-oauth.md`.

### Teardown (M1 — retire, don't orphan) — do as part of P4
Delete `FINGERPRINT_PROTECTION_SCRIPT` injection at `simple_render_process_handler.cpp:581-627` (**note:** outline cites `:586-632`; working tree 2026-07-10 is **`:581-627`** — reconcile at edit time); retire `FingerprintScript.h`; retire the **JS-injection** parts of `FingerprintProtection.h` (`GetDomainSeed`, `FINGERPRINT_SEED` plumbing, `s_domainSeeds`/`s_seedMutex`, `s_fingerprintDisabledUrls`/`s_fpDisabledMutex`, the `fingerprint_seed`/`fingerprint_site_disabled` IPC at `simple_handler.cpp` `OnBeforeBrowse`); **migrate `IsAuthDomain` into C7**; **preserve** `IsSiteEnabled`/`SetSiteEnabled` + `fingerprint_get/set_site_enabled` IPC (shipped user control — re-home into C2's `enabled` bit, do NOT delete). Keep the adjacent adblock scriptlet block (`:567-579`) and `window.chrome` stub byte-identical (Q2 TP-1/TP-2). **Guard against double-seeding / dead symbols** (Q2 T8 grep sweep).

**Incremental teardown rule (I-4 — how to retire a monolithic JS constant without double-farbling):** `FINGERPRINT_PROTECTION_SCRIPT` is a single embedded JS string that wraps `toDataURL`/`getImageData`/`readPixels`/`getChannelData`/etc. Do **not** try to keep the whole constant alive while native patches land piecemeal — instead **decompose it per-API** so each API's JS override is a separately removable fragment, and **delete a fragment in the exact same step its native patch lands** (canvas JS fragment removed in P4a; WebGL in P4b; audio in P4c). This makes teardown **atomic per value**, which is what actually prevents double-perturbation: if the native canvas farble runs at API-call time *and* the JS `toDataURL`/`getImageData` wrapper is still present, the JS layer would re-perturb the already-native-farbled pixels and, because it re-seeds from its own (soon-dead) `s_domainSeeds` path, would break intra-session consistency. **Because deletion is atomic (native-in / JS-out in one commit), no runtime "double-farbling guard flag" is needed** — there is never a window where both layers wrap the same API. (The earlier "guard flag" phrasing is superseded by this atomic-swap rule; a flag would only be required if a value's JS override could not be removed in the same step, which we forbid.)

---

## 7. Design-conflict reconciliation — per-value farble-vs-omit table (feeds Q5)

The current JS impl (`FingerprintScript.h` header comments) **deliberately dropped** screen resolution, `hardwareConcurrency`, `deviceMemory`, and WebGL vendor/renderer as "detectable / low-entropy / cross-referenced." `B1-farbling-design.md` **re-adds** three of them. Resolve each now (owner default 2026-06-17 Q18 = **Brave-*technique* parity unless concrete breakage** — parity means "adopt Brave's *approach* (valid-set constraint, reduce-only cores, per-eTLD+1 seed)", **not** copy Brave's literal value sets, several of which are mobile-tuned and wrong for a desktop browser; see C-1 corrections below):

| Value | Today (JS) | Recommended default (native) | Reasoning |
|---|---|---|---|
| **Canvas `getImageData`/`toDataURL`/`toBlob`** | farbled (LSB, <65536px) | **Farble** (C3) | Highest-signal vector; native removes the toString tell. Keep the small-canvas gate. |
| **WebGL `readPixels`** | farbled (LSB) | **Farble** (C4, own patch point) | High-signal; already shipped; keep. |
| **WebAudio** | farbled (fudge) | **Farble** (C5) | High-signal; already shipped; keep. |
| **navigator.plugins** | fake 5-PDF set | **Keep native** (C6) | Empty array is a bot tell; realistic set is safe. |
| **navigator.webdriver** | `false` | **Keep** (re-home, bot signal not farble) | Absence/`true` = bot tell. Must survive JS-block deletion. |
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
- [ ] **worker column == window column** for canvas/WebGL/audio — including **service-worker, shared-worker, and OffscreenCanvas-in-worker**, not just CreepJS's dedicated-worker column (I2 / §5). P4a satisfies dedicated; P4e satisfies OOP. **CreepJS only exercises the dedicated-worker column, so the OOP cases need a purpose-built harness** — a **P4e deliverable**: a small test page that, inside each worker type (dedicated, shared, service), builds a fingerprint via `OffscreenCanvas` + a WebGL context readback + an OfflineAudioContext render, posts the values back to the page, and asserts they **equal the window-context values** for the same profile+domain. Service workers have no DOM, so the harness must construct the readback from `OffscreenCanvas`/WebGL, not `<canvas>`. Without this harness the row is a checkbox no one can check.
- [ ] **Intra-session consistency:** same read twice in one session+domain → **identical** perturbation (load-bearing for site correctness).
- [ ] **Cross-profile difference:** same site in two profiles → different farbled values.
- [ ] **Cross-site iframe:** a third-party origin embedded in two different first parties → **different** values (first-party/top-frame keying works — I4). Because a cross-site iframe is **out-of-process** (default site isolation), this requires the browser to deliver the *top-frame* key to the subframe process — satisfied by **P4e**, not P4a; verify only after P4e lands.
- [ ] **Cross-session login test (THE important one):** create an account → restart browser → revisit → appears as the **same device**, logins do **not** break (persistent per-profile seed working).
- [ ] Navigator values within the **standard valid set** (deviceMemory in the desktop set or dropped; hardwareConcurrency ≤ real cores); WebGL vendor/renderer decision applied per §7 — **either "drop" (Mac GPU-string entries then NOT required and must not block this gate) OR "common-string map" (then Mac ANGLE entries required, FB-6)**. Read the checkbox against whichever FB-2 decision was taken.
- [ ] **No stable secret on any renderer command line** (C2 threat model): verify via ProcessExplorer/`ps` that no per-profile secret appears on a child cmdline.
- [ ] OAuth/auth-domain exemption (C7) verified: pre-approved sites un-farbled and logging in (Q3); user per-site toggle still works.
- [ ] **Stability soak + renderer-crash-rate** not elevated vs the 136 baseline; **canvas/WebGL readback perf** within budget.
- [ ] Adblock intact incl. YouTube CefResponseFilter + cosmetic/scriptlet (Q2 T1–T8); `webdriver=false` + `window.chrome` stub survived JS-block deletion.

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
| FB-1 | Seed delivery channel — mojo/commit-params per-navigation (A) vs ephemeral-nonce-cmdline (B)? | **(A) mojo interface or commit-params per-navigation, browser-side HMAC** (not a "pref") | Keeps master seed browser-only; supersedes B1-design's renderer-HMAC; cleanest C2 satisfaction. Fall back to (B) only if mojo plumbing is heavy on TARGET. |
| FB-2 | WebGL `UNMASKED_VENDOR/RENDERER` — farble or drop? | **Drop** unless a real-GPU-string map (incl. Mac ANGLE) is built | Random strings are more unique than truth; JS impl's drop was defensible. Owner sign-off (Q18 Brave-parity leans re-add-with-map). |
| FB-3 | Service-worker key scope — registration-scope eTLD+1 vs top-frame? | **Registration-scope eTLD+1** | Matches SW origin semantics; top-frame is undefined for a background SW. Confirm in P4e. |
| FB-4 | Re-home `navigator.webdriver=false` + `window.chrome` stub where? | **Keep as tiny native/JS bot-signal shims independent of farbling** | They are bot signals, not farbling; must survive JS-block teardown regardless of per-site enable. |
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
- In-repo: `0.4.0/B1-farbling-design.md`, `0.4.0/CHROMIUM_CEF_BUILD_DESIGN_OUTLINE.md` §3c/§5/§7, `DevOps-CICD/research/BRAVE_FORK_FEASIBILITY.md` §B1, `chromium-rebuild/Q2_farbling_adblock.md`; working-tree cites `cef-native/include/core/FingerprintScript.h`, `FingerprintProtection.h`, `src/handlers/simple_render_process_handler.cpp:551-653` (FP block :581-627), `simple_handler.cpp` `OnBeforeBrowse` FP IPC (verified 2026-07-10).
