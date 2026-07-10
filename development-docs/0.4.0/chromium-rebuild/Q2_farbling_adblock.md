# Q2 — Farbling × Adblock: does moving farbling into the CEF/Blink binary change how our adblock engine works?

**Created:** 2026-07-10 · **Owner:** Matthew (Marston Enterprises) · **Lead:** Windows Claude
**Status:** DETAILED PLAN (expands `CHROMIUM_CEF_BUILD_DESIGN_OUTLINE.md` §6 Q2). Research + design only — **NO code, NO builds.**
**Answers:** When farbling moves from JS-injection (renderer, above JS) into Blink C++ patches (renderer, below JS), does it collide with the adblock engine (separate Rust process on 31302 + C++ `AdblockCache` + `AdblockResponseFilter` + cosmetic CSS + scriptlet injection)? What are the concrete touch points, ordering hazards, and regression tests?

**Cross-refs:** `CHROMIUM_CEF_BUILD_DESIGN_OUTLINE.md` §3c (FEAT-B1 / M1 — the C1–C7 Blink patch set + teardown checklist; this doc assumes its teardown/seed model) and §4 (P4/P6 acceptance), `B1-farbling-design.md`, `DevOps-CICD/research/BRAVE_FORK_FEASIBILITY.md` §B1. Authoritative code cites below are verified current as of 2026-07-10.
> **Doc-ordering note:** `PLAN_farbling_blink.md` (the standalone C1–C7 patch plan) and `Q3-farbling-x-oauth.md` (auth-domain exemption plan) are **planned, not-yet-written** as of 2026-07-10. Until they land, the outline §3c/§4 is the source-of-truth for C1–C7, the M1 teardown checklist, the C2 seed model, P4e, and P6. Where this doc says a decision "migrates into C7," treat that as a *proposed* dependency to be settled when the farbling/OAuth plans are authored — Q2 does not itself decide it.

---

## TL;DR — VERDICT: independent layers, one net win, two teardown-hygiene touch points

**Farbling and adblock live on different layers and do not interact at runtime.** Adblock's *blocking* work (network cancellation + response-body rewriting) runs in the **browser process on the IO thread**; farbling runs in the **renderer, inside Blink, below JavaScript**. Adblock's *cosmetic* work (CSS hide + scriptlet injection) runs in the **renderer as injected JavaScript, above Blink** — so it sits *on top of* native farbling and reads already-farbled values, which is correct.

- **Net win:** moving farbling below JS makes `getImageData.toString()` etc. return `[native code]` again, which *improves* adblock/anti-adblock resilience (fewer tamper tells) and removes the `OnContextCreated` JS-injection timing coupling.
- **Only real touch points are teardown hygiene, not runtime behaviour:**
  1. The JS-farbling injection block being **deleted** (`simple_render_process_handler.cpp:581–627`) sits *directly adjacent* to the adblock **scriptlet injection** block (`:567–579`) — delete cleanly without disturbing it.
  2. The `fingerprint_seed` / `fingerprint_site_disabled` IPC being **removed** (`simple_handler.cpp:7484–7521`) sits *directly adjacent* to the adblock `preload_cosmetic_script` IPC (`:7445–7461`) in the same `OnBeforeBrowse` — same hazard, same dispatcher.
- **Recommendation:** proceed; treat Q2 as a **regression-test + deletion-hygiene** concern, not a design blocker. No adblock-engine code changes are required. `hodos-unbreak.txt` is an adblock-engine file and is **untouched** by this sprint (per outline I1).

---

## 1. Layer map — where each piece actually runs

| Concern | Process | Thread | Layer vs JS | Code anchor |
|---|---|---|---|---|
| **Adblock network block** (`AdblockCache::check` → `AdblockBlockHandler` cancels) | **Browser** | **IO** | n/a (pre-network) | `simple_handler.cpp::GetResourceRequestHandler`; `AdblockCache.h` |
| **Adblock response rewrite** (`AdblockResponseFilter` — YouTube ad-key strip) | **Browser** | **IO** | n/a (pre-renderer bytes) | `simple_handler.cpp:7541+` (`CefResponseFilter`) |
| **Adblock cosmetic CSS** (`<style id="hodos-cosmetic-css">`) | **Renderer** | render | **above** JS (injected JS builds a `<style>`) | `simple_render_process_handler.cpp:1224–1263` (`inject_cosmetic_css` IPC) |
| **Adblock scriptlets** (fetch/XHR/JSON.parse overrides) | **Renderer** | render | **above** JS (executes as page-context JS) | pre-cache `s_scriptCache` → inject in `OnContextCreated:567–579`; IPC `preload_cosmetic_script` `simple_handler.cpp:7454` |
| **Farbling TODAY** (`FINGERPRINT_PROTECTION_SCRIPT`) | **Renderer** | render | **above** JS (Mulberry32 JS overwrites Canvas/WebGL/etc.) | `OnContextCreated:581–627`; IPC `fingerprint_seed` `simple_handler.cpp:7515` |
| **Farbling AFTER (C1–C7)** (Blink patches reading `HodosSessionCache` Supplement) | **Renderer** | render | **below** JS (native C++ in `third_party/blink`) | new `.patch` files (see outline §3c FEAT-B1) |

**The key structural fact:** the migration moves farbling from **"renderer / above JS"** to **"renderer / below JS."** Adblock never leaves its two homes: **"browser / IO"** (block + response filter) and **"renderer / above JS"** (cosmetic CSS + scriptlets). So after the migration:

```
NETWORK  ──►  [Browser/IO] adblock block + response filter   (unchanged; farbling absent here)
              │
              ▼
RENDERER  ──► [Blink C++]  farbling (native, below JS)        ← farbling NEW HOME
              │
              ▼
              [Page JS + injected scriptlets/CSS] adblock cosmetic   (unchanged; reads farbled output)
```

Farbling and the two adblock homes never share a thread, process boundary, or execution phase. This is the basis for the "independent" verdict.

---

## 2. Point-by-point interaction analysis

### 2.1 Adblock network block — ZERO interaction ✅
`AdblockCache::check()` runs on the IO thread in the browser process and returns `AdblockBlockHandler` (`RV_CANCEL`) before a request ever reaches the renderer. Farbling has **no network component** (no port, no HTTP; the seed travels off-cmdline per C2). Different process, different thread, different phase. **No touch point.**

### 2.2 Adblock response filter (YouTube ad-key strip) — ZERO interaction ✅
`AdblockResponseFilter` (`CefResponseFilter`) rewrites the response **body bytes** on the IO thread *before the renderer sees them* (`simple_handler.cpp:7536–7540` comment confirms). Farbling perturbs **canvas/WebGL/audio readback**, never network bytes. **No touch point.**

### 2.3 Adblock scriptlets vs farbling — ordering, and the "double-wrap" question ⚠️ (low risk)
Both scriptlets (post-migration) and farbling execute in the renderer. But they execute at **different layers**:

- **Farbling (Blink):** applies at **API-call time** — every `getImageData` / `getParameter` / `getChannelData` call funnels through the patched native function and reads `HodosSessionCache::From(ctx)`. It does **not** run at context-creation time and holds **no `OnContextCreated` timing assumption**.
- **Scriptlets (JS):** inject at **context-creation time** (`OnContextCreated:567–579`, one-shot from `s_scriptCache`), *before* page JS runs, and typically override `fetch` / `XMLHttpRequest` / `JSON.parse` to strip ad data.

Because farbling is native and below JS, when a scriptlet or page script calls a canvas/WebGL/audio API, **Blink farbles first and the JS sees already-farbled bytes.** That is the correct order and matches Brave.

**The one edge case to flag:** a small number of uBO-style scriptlets *also* wrap or noop canvas/audio methods (a scriptlet that overrides `HTMLCanvasElement.prototype.toDataURL` / `getImageData` / `AudioContext` readback to defuse canvas-fingerprinting). Note `set-constant.js` is **not** such a scriptlet — it pins a property to a constant value, it does not intercept `toDataURL`; the real concern is any scriptlet that replaces one of these methods with a JS wrapper. Whether any such scriptlet ships in our four lists is an empirical question answered by grepping the compiled scriptlet set (T5), not assumed here. If one is active on a domain, it will JS-wrap the already-native-farbled function. Result: the JS wrapper sees farbled bytes and then applies its own transform — **double perturbation, but not a crash and not a correctness break** (farbling tolerates any downstream transform; it only guarantees *its own* output is stable per session+domain). Risk is **low** and bounded to whatever domains ship a canvas-touching scriptlet in our four filter lists. **Test, don't redesign** (see §4 T5).

### 2.4 Adblock cosmetic CSS — ZERO interaction ✅
`inject_cosmetic_css` builds a `<style>` element to hide DOM nodes. It touches layout/visibility, never canvas/WebGL/audio/navigator. **No touch point.**

### 2.5 Anti-adblock detection × farbled canvas/audio — NET WIN, minor residual ✅➕
Two sub-questions:

- **"Does farbling break anti-adblock detection (making sites think we block)?"** No. Anti-adblock scripts detect blocking by planting bait elements / bait requests and checking whether they survive (confirmed by the uBO wiki: *"a script can't directly tell if a browser has an ad blocker… instead it tries adding something to the page to see if it gets blocked"*). That is orthogonal to canvas/audio perturbation. **No new breakage from farbling.**
- **"Does native farbling help against anti-adblock/anti-bot fingerprinting?"** **Yes — this is the headline win.** Today's JS injection replaces `HTMLCanvasElement.prototype.toDataURL` (etc.) with a JS function, so `toDataURL.toString()` no longer returns `"function toDataURL() { [native code] }"`. Anti-bot stacks (Cloudflare Turnstile, DataDome, PerimeterX) flag exactly this kind of prototype tampering. Moving to Blink patches restores `[native code]` integrity → **fewer bot-detection false positives**, which also reduces the Turnstile "Verify you are human" loops the codebase already fights (see the `OnBeforeBrowse` comment at `simple_handler.cpp:7440–7442`). Adblock's own resilience against *anti-adblock* fingerprinting improves for free.

> Residual: farbling still perturbs the fingerprint, so a site that *keys* anti-bot on a stable canvas hash will see our (stable-per-profile-per-site) farbled hash. Our persistent-per-profile seed (outline §3c C2) keeps it stable across sessions → no new friction vs today. No adblock impact.

### 2.6 Workers — different concerns, no collision ✅
Adblock already covers worker *requests* at the network layer (`CefResourceTypeToAdblock` maps `RT_WORKER` / `RT_SHARED_WORKER` / `RT_SERVICE_WORKER` → `"other"`, `AdblockCache.h:56–62`). Farbling in workers is **new** (the `Supplement<ExecutionContext>` covers in-process workers; OOP workers need explicit plumbing per outline §4 P4e). These are unrelated: adblock decides *whether a worker's fetch is blocked*; farbling decides *what a worker's canvas readback returns*. **No touch point.**

---

## 3. The two REAL touch points — deletion/teardown hygiene (not runtime)

Both are consequences of the M1 teardown checklist (retire the JS farbling path) landing in files the adblock engine *also* uses. Neither is a behavioural interaction; both are "don't nick the neighbour" surgical-hygiene items.

### TP-1 — `OnContextCreated`: FP-injection block is adjacent to the scriptlet-injection block
`simple_render_process_handler.cpp`:
- **Keep:** scriptlet injection `:567–579` (`s_scriptCache` / `preload_cosmetic_script`) — **adblock, stays.**
- **Delete (farbling):** `:581–627` (auth-domain skip, `s_fingerprintDisabledUrls`, `s_domainSeeds` lookup, `FINGERPRINT_PROTECTION_SCRIPT` patch+inject).
- **Keep:** the `window.chrome` stub `:629–653` — **not farbling, not adblock; stays** (bot-signal). Note it currently sits *after* the FP block; verify its guard (`isExternalPage`) is independent of the deleted block (it is — recomputed at `:631`).
- **Also delete:** the static caches/mutexes used only by FP — `s_domainSeeds` + `s_seedMutex` (`:37–38`), `s_fingerprintDisabledUrls` + `s_fpDisabledMutex` (`:42–43`) — but **keep** `s_scriptCache` + `s_scriptCacheMutex` (`:33–34`, adblock).

**Acceptance:** after deletion, the scriptlet block still compiles and injects (T2), and `git diff` shows the `:567–579` block byte-identical.

### TP-2 — `OnBeforeBrowse`: `fingerprint_seed` IPC is adjacent to `preload_cosmetic_script` IPC
`simple_handler.cpp::OnBeforeBrowse`:
- **Keep:** `:7445–7461` `preload_cosmetic_script` send — **adblock, stays.**
- **Delete (farbling):** `:7484–7521` — the `fingerprint_seed` + `fingerprint_site_disabled` sends and the `IsAuthDomain` (hardcoded auth allowlist) gate. The auth-domain logic is **proposed to migrate into C7** (per the not-yet-written OAuth plan; do not leave a second source of truth). **Delete only once C7 actually owns it.**
- **⚠️ DO NOT blindly delete `IsSiteEnabled` / `SetSiteEnabled` — that is a shipped user control, not the auth allowlist.** `IsSiteEnabled` is the user-facing Privacy-Shield **per-site fingerprint on/off toggle**, backed by real IPC (`fingerprint_get_site_enabled` / `fingerprint_set_site_enabled` at `simple_handler.cpp:6191/6210`; `FingerprintProtection.h:123/135`). Outline C7 **only re-implements `IsAuthDomain`, not this toggle.** Deleting it drops a shipped feature with no replacement. **This is OUT of Q2 scope → the farbling plan owns re-homing it** (browser decides farbling on/off per eTLD+1 from the user toggle and passes it to the renderer alongside the seed, per C2's "browser process decides farbling on/off … per eTLD+1"). Until the farbling plan provides that destination, treat `IsSiteEnabled` + its IPC chain as an **unresolved gap — do not lump into C7.**
- **Keep:** `:7463–7482` domain-permission pre-warm (wallet, unrelated).
- **Renderer side:** delete the matching IPC handlers `fingerprint_seed` (`simple_render_process_handler.cpp:1198`) and `fingerprint_site_disabled` (`:1213`) — but **keep** `preload_cosmetic_script` (`:1183`), `inject_cosmetic_css` (`:1224`), `inject_cosmetic_script` (`:1264`), and `cosmetic_class_id_query` handling.

**Acceptance:** after deletion, `preload_cosmetic_script` still fires on nav (T2), and the `cosmetic_class_id_query` post-load path (`simple_handler.cpp:6265`) is untouched.

> Both TPs are pure deletion adjacency. There is **no shared state** between the FP caches/IPC and the adblock caches/IPC — they are distinct message names, distinct maps, distinct mutexes. The risk is a careless multi-line delete clipping a neighbouring block, which the T2 regression catches.

---

## 4. Regression test matrix (add to P6 farbling acceptance + adblock basket)

Run on **both** Windows and macOS. All must pass **in the same session as farbling is active** (the point of Q2 is co-existence).

| # | Test | Method | Pass criterion |
|---|---|---|---|
| **T1** | Adblock network block still cancels | Load a page with a known blocked tracker (EasyPrivacy entry); check the per-browser blocked count (confirm the exact accessor/message name in `AdblockCache` at execution time — the seed only guarantees "per-browser blocked counts", not a specific `adblock_get_blocked_count` identifier) | Blocked count increments; tracker request absent in devtools |
| **T2** | Scriptlet + cosmetic injection still fires after FP teardown | YouTube (ad-key strip + scriptlet), plus a cosmetic-heavy news site | No pre-roll ad; cosmetic elements hidden; `preload_cosmetic_script` logged in `debug_output.log` |
| **T3** | YouTube `AdblockResponseFilter` intact | Play 3 YouTube videos | No mid-roll/pre-roll; `adPlacements`→`adPlacements_` rename observed |
| **T4** | Farbling active AND adblock active, same session | CreepJS with adblock ON | **Checkable:** CreepJS worker column == window column (farbling holds with adblock on). Note: CreepJS is a *fingerprint* tester, not an adblock detector — do **not** gate on a CreepJS "adblock detected" signal (may not be a real, readable column). Note also that CreepJS exercises only the *dedicated-worker* column, so this is **necessary-but-not-sufficient**: service/shared-worker + OffscreenCanvas farbling (P4e) is deferred (§2.6) and the full worker matrix lives in the farbling acceptance (P6), not here |
| **T5** | Canvas-touching scriptlet double-wrap | Enumerate our 4 filter lists for `+js(` scriptlets that touch canvas/`toDataURL`/audio (grep the compiled scriptlet set); load one such domain | Page renders; no console error; farbled value still stable on repeat read (intra-session consistency holds) |
| **T6** | Anti-bot false-positive did NOT regress | Cloudflare Turnstile site + a DataDome site (e.g. a retailer) with adblock ON | **GATE (objective):** `toString()` of `toDataURL`/`getParameter` returns `[native code]` in the devtools console — proves the migration landed below JS. **Non-blocking observation only:** whether the "Verify you are human" loop disappears (Turnstile/DataDome verdicts also depend on IP reputation, TLS/JA3, behavioral signals, and a farbled canvas hash can itself be a tell — see §2.5 Residual — so this is not a deterministic pass/fail) |
| **T7** | Auth-domain exemption moved cleanly (no double source) | An `IsAuthDomain` site (Google/Microsoft sign-in) | Un-farbled per C7; adblock still active; `hodos-unbreak.txt` behaviour unchanged (its `#@#+js()` scriptlet exemptions still apply) |
| **T8** | No orphaned FP IPC / dead caches | grep build for **all** retired FP symbols: `s_domainSeeds`, `s_fingerprintDisabledUrls`, `fingerprint_seed`, `FINGERPRINT_PROTECTION_SCRIPT`, `FingerprintProtection`, `FingerprintScript`, `IsAuthDomain`, and — **only once the farbling plan re-homes the user toggle (I1/TP-2)** — `IsSiteEnabled`, `SetSiteEnabled`, `fingerprint_get_site_enabled`, `fingerprint_set_site_enabled`, `fingerprint_get_site_enabled_response` | Zero references remain for the fully-retired symbols (M1 teardown complete). **The `IsSiteEnabled` / `fingerprint_*_site_enabled` group must NOT go to zero until its destination lands — if the farbling plan hasn't re-homed the user toggle yet, scope T8 to "adblock-adjacent + fully-retired FP symbols only" and defer the user-toggle sweep to the farbling plan's acceptance, so this gate can't go green with the per-site toggle IPC dangling.** |

**T6's `[native code]` check is the single most valuable Q2 assertion** — it proves the net win and that the migration actually landed below JS.

---

## 5. Open questions → recommended defaults

| # | Question | Recommended default | Why |
|---|---|---|---|
| Q2-1 | Do any of our 4 filter lists (EasyList, EasyPrivacy, uBO Filters, uBO Privacy) + 6 bundled scriptlets ship a scriptlet that patches canvas/WebGL/audio, creating a double-wrap with native farbling? | **Audit at plan-execution time (T5); accept double-wrap if found** — it is non-breaking. Only escalate if a specific site breaks. | adblock-rust scriptlets are ad-behavioural (fetch/XHR/JSON), canvas-touching ones are rare; native-first order is safe. |
| Q2-2 | Should the auth-domain exemption stay split (farbling `IsAuthDomain` in C7 vs adblock `hodos-unbreak.txt`)? | **Keep split** (outline I1). C7 owns farbling exemption; `hodos-unbreak.txt` owns scriptlet exemption. **Do not merge.** | Two different mechanisms for two different layers; merging couples adblock and farbling maintenance. |
| Q2-3 | Does removing the JS-farbling injection free up `OnContextCreated` timing in a way we should exploit (e.g. inject scriptlets even earlier)? | **No behaviour change** — just delete FP block; leave scriptlet timing as-is. | Scriptlet timing is already correct (pre-page-JS via `s_scriptCache`); farbling removal simply removes an adjacent `ExecuteJavaScript` call, marginally reducing renderer-startup work. Log the win, don't refactor. |
| Q2-4 | Could native farbling change the **hash** an anti-adblock/anti-bot service keys on, versus today's JS farbling? | **Accept** — persistent per-profile seed keeps it stable across sessions (C2), so no new cross-session friction. Verify in T6. | The value differs from raw, but so does today's; stability (not rawness) is what avoids re-auth friction. |
| Q2-5 | Do OOP service/shared workers need any adblock-side change when farbling reaches them (P4e)? | **No adblock change** — adblock already handles worker requests at network layer; farbling worker plumbing is renderer-only. | Confirmed distinct concerns (§2.6). |
| Q2-6 | Filename convention drift: outline §6 registers this as `Q2-farbling-x-adblock.md` but the file is `Q2_farbling_adblock.md` (underscores, dropped "x"). | **Pick one convention** (hyphen-`x` per outline §6) and either rename this file + siblings or update outline §6 to match. Deferred here to avoid breaking in-flight cross-refs mid-review; do it as a single rename pass across all Q-docs. | Not renamed inside this edit pass to keep the reference stable for reviewers; flagged so the drift is tracked, not silently accepted. |

---

## 6. What this does and does NOT require

**Requires (all in the farbling migration, none in the adblock engine):**
- Clean deletion at TP-1 and TP-2 without touching adjacent adblock blocks.
- The T1–T8 regression matrix folded into P6 (§7 of the outline's readiness checklist already lists "Adblock still works incl. YouTube CefResponseFilter ad-strip + cosmetic/scriptlet (Q2)"; T4/T6 extend it).

**Does NOT require:**
- Any change to the adblock Rust process (31302), `AdblockCache`, `AdblockResponseFilter`, cosmetic CSS/scriptlet pipeline, `PortConfig`, or `hodos-unbreak.txt`.
- Any ordering change between adblock and farbling (native-first is automatic and correct).
- Any new IPC — **on the precondition that C2 chooses the mojo/pref or ephemeral-nonce seed channel (still an OPEN outline OQ).** If C2 instead keeps a renderer IPC to deliver the seed, that message would share the `OnBeforeBrowse` / `OnProcessMessageReceived` dispatcher with `preload_cosmetic_script` — still a distinct message name, distinct map, no collision — but the "no new IPC" claim above should be re-confirmed when C2's delivery channel is settled.

**Bottom line:** Q2 is **GREEN**. Farbling → Blink and adblock are independent layers; the migration's only adblock-adjacent work is deletion hygiene in two shared files (`simple_render_process_handler.cpp` `OnContextCreated`, `simple_handler.cpp` `OnBeforeBrowse`), fully covered by an 8-test regression matrix. There is a concrete net benefit (`[native code]` toString integrity → better anti-bot/anti-adblock resilience). No adblock-engine redesign.

---

## Sources
- uBO fingerprinting/anti-adblock behaviour: [Does uBO protect against fingerprinting? — gorhill/uBlock Wiki](https://github.com/gorhill/uBlock/wiki/Does-uBO-protect-against-fingerprinting%3F)
- Canvas fingerprinting mechanics & API-interception defenses: [How ad blockers can be used for browser fingerprinting — Fingerprint](https://fingerprint.com/blog/ad-blocker-fingerprinting/), [The WASM Cloak (arXiv 2508.21219)](https://arxiv.org/html/2508.21219v1)
- Repo (verified 2026-07-10): `cef-native/src/handlers/simple_render_process_handler.cpp` (`OnContextCreated` :551–653; FP block :581–627; scriptlet block :567–579; IPC handlers :1183–1271), `cef-native/src/handlers/simple_handler.cpp` (`OnBeforeBrowse` :7434–7524; `AdblockResponseFilter` :7541+), `cef-native/include/core/AdblockCache.h`, `development-docs/0.4.0/B1-farbling-design.md`, `development-docs/0.4.0/CHROMIUM_CEF_BUILD_DESIGN_OUTLINE.md` §6 Q2.
