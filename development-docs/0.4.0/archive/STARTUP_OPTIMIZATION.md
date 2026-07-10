# Startup Optimization (pivot from B2 header-opt) — findings + plan

**Status:** 🔬 Investigation/design — NO production code changes yet. Read-only study + adversarial review BEFORE touching the (fragile) startup sequence.
**Date:** 2026-06-22
**Supersedes for this work item:** B2 (`B2-header-to-cpp.md` / FEAT-B2-SLIM/WARM). The header is NOT the bottleneck — see below.

---

## ⭐⭐⭐ W0 GATE RESULT (2026-06-22) — manifest thesis CONFIRMED, proceed to W1

W0 was run with **zero code** by attaching to the already-open `remote_debugging_port=9222` on a current `0.4.0` dev build (prod transport) and reading `chrome://version` + `SystemInfo.getInfo` over CDP. Both halves of the skeptic's go/no-go came back pointing the same way:

1. **OS MISDETECTED.** `chrome://version` reports **`OS: Windows 8 Version 25H2 (Build 9200.8655)`**. The real machine is Windows 11 (build 26100+). Build 9200 = Windows 8.0 — the classic manifest-compatibility-shim version `RtlGetVersion` returns when **no `app.manifest` declares the Win10/11 `supportedOS` GUID**. (UA still says "Windows NT 10.0" — that's hard-coded for privacy and is NOT the OS-detection path Chromium's GPU stack uses.)

2. **GPU-STACK COLLAPSE A/B (the smoking gun).** `SystemInfo.getInfo` feature status:

| Feature | `--disable-gpu-compositing` ON (default band-aid) | band-aid OFF (`HODOS_GPU_COMPOSITING=1`), manifest absent |
|---|---|---|
| `gpu_compositing` | disabled_software *(the flag)* | **disabled_software (STAYS)** |
| `2d_canvas` | enabled | **unavailable_software** |
| `opengl` | enabled_on | **disabled_off** |
| `rasterization` | enabled | **disabled_software** |
| `webgl/webgl2/webgpu` | enabled_readback | **unavailable_software** |
| `video_decode/encode` | enabled | **disabled_software** |
| driver bug workarounds | 15 | **0** |
| GPU devices | Intel UHD + NVIDIA RTX A2000, ANGLE **D3D11** | **none** |

**Interpretation:** When Chromium thinks it's on Windows 8 and you *ask* for real GPU compositing, it decides the GPU is unsupported and **disables the entire GPU stack → full software (WARP/SwiftShader)** → black screen + the measured ~2236 ms first paint. The `--disable-gpu-compositing` band-aid only "works" because it keeps the *renderer* on D3D11 while forcing just the *display compositor* to software (webgl `enabled_readback` is the tell) — that compositor-software path is what produces the ~2 s first paint. This is a direct, measured mechanism; the skeptic's competing "global-OSR / BeginFrame" alternative is **not needed** to explain it.

**Verdict: GO.** The Windows `app.manifest` (Win10/11 `supportedOS` GUID) is the confirmed root fix. Proceed to **W1** (add the manifest), then the **Phase-1 confirmation A/B**: with the manifest IN, re-run `HODOS_GPU_COMPOSITING=1` and confirm the GPU stack comes up hardware (devices listed, `gpu_compositing` not software) and `headerPainted` collapses from ~2236 ms toward ~173 ms. Reusable CDP probe lives at `cef-native/build/bin/Release/w0_sysinfo.mjs` (gitignored build dir; `node w0_sysinfo.mjs` against a running 9222).

> Re-launch gotcha for re-tests: launch the GUI exe **detached** via PowerShell `Start-Process` (a bash `&`/background wrapper kills it). `cd build/bin/Release; $env:HODOS_DEV=1; Start-Process .\HodosBrowser.exe '--profile=Default'`.

---

## 🎯🎯🎯 ROOT CAUSE OF THE ~2s FIRST PAINT — FOUND + FIX VALIDATED (2026-06-22)

**It was never GPU/compositing/manifest. It is one line: `App.tsx:135 await brc100.isAvailable()`.**

### The chain
`App.tsx` `initializeBRC100()` runs in the mount `useEffect` of **every** renderer (App wraps all routes). It calls `await brc100.isAvailable()` — whose **only effect is a `console.log`** — which invokes the **synchronous** C++ V8 binding `window.hodosBrowser.brc100.isAvailable` → `BRC100Handler::Execute` → `BRC100Bridge::isAvailable()` → **blocking WinHTTP** to `localhost:31301`. Two compounding costs: (a) `WinHttpOpen(WINHTTP_ACCESS_TYPE_DEFAULT_PROXY)` triggers **WPAD proxy auto-detect**, and (b) the **wallet is cold** (~2s to healthy) at startup. The call **blocks the renderer main thread ~1.9s**. The `await` is cosmetic — the native binding is synchronous, so it freezes the thread (every timer + rAF stalls, page stays black) until it returns.

### How it was found (BeginFrame investigation, 2026-06-22)
1. Double-rAF + `setTimeout` probe (`MainBrowserView`): BOTH stall to ~2.1s then burst → renderer **main-thread block**, not compositor/visibility. `document.visibilityState==visible`.
2. localStorage probe (`main.tsx`): `firstLocalStorageMs=1` → ruled out localStorage.
3. Chromium startup trace (`--trace-startup-format=json`): every renderer's `CrRendererMain` parked ~2.1s in `SimpleWatcher::OnHandleReady → MessagePort::Accept → v8.callFunction` (React scheduler), browser/GPU threads idle.
4. CPU-profiler samples in-window: **96.5% in `createSPVProof`** — a native binding with no JS URL. All brc100 methods share one `BRC100Handler`; the profiler mis-symbolized to the **last-registered** name (`createSPVProof`, `BRC100Handler.cpp:97`). Actual call = `isAvailable` (`App.tsx:135`).
5. Fix experiment: defer the probe → **headerPainted 2276→276–468 ms (~1.9s faster)**, tracking `headerReady` (~190–230ms). CONFIRMED.

### Fix options (owner to choose; nothing committed)
- **F-A (minimal, recommended now):** remove/defer `initializeBRC100()` in `App.tsx` — it only `console.log`s, zero functional effect; the C++ bridge is constructed at `OnContextCreated` regardless. Eliminates the user-visible 2s. Pure frontend, zero risk.
- **F-B (the real architectural fix):** make the BRC100 V8 bindings **asynchronous** — they ALL do synchronous WinHTTP on the render main thread today (`createSPVProof`, `authenticate`, `createBEEF`, …), so ANY dApp calling brc100 from page context freezes that page. Return a Promise / route over async IPC. Bigger; touches render-process V8 + IPC (invariant #8) — design + adversarial review first.
- **F-C (mitigation):** `BRC100Bridge` `WinHttpOpen` → `WINHTTP_ACCESS_TYPE_NO_PROXY` (localhost never needs a proxy/WPAD). Cuts the WPAD slice but leaves sync-on-main-thread.
- Recommend **F-A now** + track **F-B** as the durable fix + **F-C** as a cheap hardening. NOTE: relationship to W1/manifest — the manifest is still worth keeping (correctness + GPU-hardening + enables retiring `--disable-gpu-compositing`), but it is independent of this paint fix.

### Throwaway diagnostics still in tree (revert before any feature commit)
`main.tsx` firstLocalStorageMs probe; `MainBrowserView.tsx` beginframeDiag block; `App.tsx` the `setTimeout(…,4000)` experiment (replace with the chosen real fix). Reusable: `build/bin/Release/w0_sysinfo.mjs`, the trace via `--trace-startup-format=json`.

### ✅ IMPLEMENTED (2026-06-22) — owner approved REMOVE after adversarial review (`wf_7d79e2b2-839`). NOT committed.
- **Step 1 (frontend perf fix):** removed `initializeBRC100()` + the dead `await brc100.isAvailable()` probe + unused import from `App.tsx`; reverted all throwaway frontend diagnostics (main.tsx/MainBrowserView back to HEAD). Measured clean: **headerPainted 215–389 ms** (was ~2276–2383). `tsc` + vite + smoke clean.
- **Steps 3-4 (delete legacy Line-2 bindings):** deleted `frontend/src/bridge/brc100.ts` (+ its conflicting `declare global`), cleaned `e2e/helpers/bridge-mock.ts`, removed `RegisterBRC100API` call + `#include` from `simple_render_process_handler.cpp`, removed both source pairs from `CMakeLists.txt` (3 lists), deleted `BRC100Handler.{cpp,h}` + `BRC100Bridge.{cpp,h}`. Frontend `tsc` clean (TS now resolves to canonical `types/hodosBrowser.d.ts` — fixes the useBookmarks declaration conflict). **Windows build clean.** Smoke: header + wallet overlay + `/brc100-auth` Phase 2.5 auth overlay load; no errors/"Unknown method"; wallet API `{"exists":true,"locked":false}`; **`window.CWI/yours/panda` shim still injects on external pages** (verified via CDP on example.com).
- **REMAINING before commit:** (a) interactive wallet function test — Send / Receive / Publish Cert / balance / unlock (real-money UI, owner to drive); (b) **macOS build** (the deletion removed the libcurl branch of BRC100Bridge; unbuildable here); (c) commit grouping — recommend Step 1 and Steps 3-4 as two separate commits. W1 manifest stays staged (independent, keep). C++ perf instrumentation (PerfMarks/env toggles) still uncommitted — revert before feature commit.

---

## ⭐⭐⭐ W1 APPLIED + VALIDATED (2026-06-22) — manifest WORKS but is NOT the paint fix. PIVOT.

**Changes made (uncommitted, in tree):** new `cef-native/hodos.manifest` (4 supportedOS GUIDs) + `cef-native/CMakeLists.txt:449` link flags `+ /MANIFEST:EMBED /MANIFESTINPUT:".../hodos.manifest"`. Rebuilt; `mt.exe -inputresource:HodosBrowser.exe;#1` confirms the merge — resource #1 now has BOTH `<trustInfo>`(asInvoker) AND `<compatibility>` with all 4 GUIDs (single manifest, no collision). Route B works exactly as planned.

**What the manifest FIXED (real wins):**
- OS detection: `chrome://version` `Windows 8 Build 9200` → **`Windows 11 Version 25H2 (Build 26200)`**.
- GPU stack no longer collapses when the band-aid is removed. Full A/B (`SystemInfo.getInfo`):

| Config | `gpu_compositing` | `webgl` | devices |
|---|---|---|---|
| no manifest, band-aid OFF | disabled_software | unavailable_software | NONE (collapse) |
| **manifest IN, band-aid OFF** | **enabled** | **enabled** | Intel UHD + RTX A2000 (D3D11) |

→ The manifest is a genuine correctness + GPU-hardening win and **safely enables W2** (dropping `--disable-gpu-compositing`) and almost certainly fixes the black-screen-on-flag-removal.

**What the manifest did NOT fix — the first paint (`headerPainted`, double-rAF, ms rel navStart):**

| Config | headerPainted |
|---|---|
| baseline (no manifest) | ~2383–3054 |
| W1 manifest, band-aid ON | **2276** |
| W1+W2 manifest, band-aid OFF (GPU comp confirmed HARDWARE) | **2335** |
| + `HODOS_NO_OCCLUSION` | **2258** |

**The ~2.2 s first-paint lag is STABLE (~2.26–2.34 s) across manifest in/out, GPU-comp on/off, occlusion on/off, and (per prior A/B) overlays on/off.** `headerReady` (React mounted) is ~190–230 ms; the compositor produces NO frame for the windowed header for ~2.1 s after that. So **the paint lag is a BeginFrame/frame-scheduling problem, NOT GPU compositing mode, NOT the manifest** — the skeptic's competing hypothesis is upheld.

**UI-thread timeline during the gap (W1-only run):** header loaded 13.32s → [~750 ms quiet] → overlays pre-created sequentially (menu 14.10, wallet 14.60, download 15.11, notification 15.33) → backends healthy 15.19 → header fetches wallet/status+balance 15.43 → **headerPainted 15.44**. The first header frame appears only as the whole startup UI-thread sequence drains at ~2.2 s. No literal 2 s timer found (grepped). NOTE: `HODOS_NO_OVERLAYS` A/B previously showed overlays aren't causal (OFF 2450 ≈ ON 2223), so the correlation with overlay-creation-finishing is not proof overlays cause it — more likely the header's compositor is throttled/not scheduled until the UI thread is free.

### DECISION NEEDED / NEXT (pivot the first-paint hunt)
- **Keep W1?** Recommend YES but re-scope it: it is a **correctness + GPU-hardening fix + W2 enabler + black-screen fix**, NOT the paint fix. It is low-risk, reversible, and a prerequisite for safely retiring `--disable-gpu-compositing`. (Owner to approve commit; nothing committed yet.)
- **First-paint root cause is now the live target.** Leads, in order: (1) windowed-header **BeginFrame scheduling** — does the header browser get BeginFrames before the UI thread finishes startup? try `external_begin_frame_enabled` / forcing an early `Invalidate`/`WasResized` on the header right after LoadEnd; (2) is the header HWND actually shown/sized early, or only late? add a log at header `ShowWindow`/first `WasResized`; (3) the `windowless_rendering_enabled=true` GLOBAL flag (`cef_browser_shell.cpp:3798`) — does global OSR change frame scheduling for the windowed header? (can't disable globally — overlays need it — but worth understanding); (4) is the double-rAF being **rAF-throttled** because Chromium considers the header not-visible/background during startup? Cross-check with a non-rAF paint signal.
- Reusable probes in `build/bin/Release/`: `w0_sysinfo.mjs` (GPU status), `w0_cdp.mjs` (chrome://version). Measure headerPainted via `HODOS_PERF_TRACE=1` → `debug_output.log` `⏱️ PERF_REPORT ... headerPainted`.

---

## TL;DR — the pivot

B2 was scoped as "the React header loads slowly → slim MUI / pre-warm the subprocess." **Measurement disproved that premise.** The React header is already fast (~230 ms). The real, user-perceived cost is **cold *startup* in the installed (production) app: ~2.4 s to a visible header** — the "spinning logo for 1–3 s" the owner sees on every launch. That time is in the **C++ startup sequence**, not React.

**Decisions:** drop FEAT-B2-SLIM, FEAT-B2-WARM, and the cache-header idea — the data supports none of them (warm≈cold ⇒ ~0 V8-code-cache delta; header is the smallest phase). New target = **cold startup time**.

---

## Measurement (B2-MEASURE, 2026-06-22)

Env-gated instrumentation (`HODOS_PERF_TRACE=1`) + the pre-existing `elapsed()` STARTUP logs. Dev measured against the **prod transport** (staged `frontend/` next to the exe → `LocalFileResourceHandler`, not Vite).

### Dev build (wallet pre-running, warm) — clean single-process run, profile=Default
- Header `CreateBrowser → OnAfterCreated` (SPAWN + V8 init): **60–71 ms**
- `CreateBrowser → LoadEnd`: **~205 ms**
- JS marks (rel navigationStart): responseEnd 76–85, DOMContentLoaded 131–141, **headerReady (splash removed) 160–170**, React mount only **~35 ms**
- Full process→header-interactive (STARTUP logs): **~520 ms**
- **Run #1 (cold) ≈ Run #2 (warm)** → V8 on-disk code cache delta ≈ **0** ⇒ cache-headers won't help.

### Installed/production app (`AppData\Local\HodosBrowser\debug_output.log`, run 2026-06-17, a 0.3.x beta)
| Phase | Time | Note |
|---|---|---|
| Process start → **window shown** | **~1114 ms** | incl. an **885 ms** gap between "Launching backend processes" (T+175) and "Settings loaded" (T+1060) |
| CefInitialize start → done | T+1114 → **T+1597 (≈483 ms)** | 5× slower than dev's ~95 ms (GPU cold-spawn? bigger prod profile?) |
| Message-loop → **header loaded** (hodosBrowser API injected) | T+1630 → **~T+2447 (≈817 ms)** | renderer spawn + bundle load + React mount |
| **Total → header visible** | **≈ 2.4 s** | matches the owner's "1–3 s spinner" |

> ⚠️ The 2.4 s is from a **2026-06-17 beta build** — likely older than current `0.4.0`. **MUST re-confirm on current code** (cheapest: dev perf build with the wallet KILLED so the browser cold-spawns backends, isolating the 885 ms pre-window gap).

### ⭐ 2026-06-22 UPDATE — current `0.4.0` code is ~757 ms (the 2.4 s was the OLD beta)
Cold-wallet run of the CURRENT dev build (prod transport, profile=Default, browser cold-spawns the wallet):
| Phase | Installed beta (06-17) | **Current 0.4.0** |
|---|---|---|
| Pre-window (incl. backend spawn) | ~1114 ms (885 ms gap) | **~302 ms (gap GONE)** |
| CefInitialize | 483 ms | **163 ms** |
| → header ready | ~2447 ms | **~757 ms** |

- The **885 ms pre-window gap is absent** in current code; backend spawn is confirmed fire-and-forget (wallet PID @T+130 ms, adblock @T+248 ms, non-blocking).
- **Implication:** the owner's "1–3 s spinner every launch" is the **installed 06-17 beta**, not current `0.4.0`. Startup already improved ~3× since.
- **Caveat:** dev uses the clean `HodosBrowserDev` data dir; the production `HodosBrowser` profile is larger (cache/cookies/history Chromium loads at init), which likely explains part of the 163 ms→483 ms CefInitialize difference. **A current production build on the real profile is unmeasured — could land anywhere between ~757 ms and ~2.4 s.** Confirm by building/installing a current 0.4.0 release and timing it before investing in further startup work.

### Where the time goes (production, 3 phases — none are the React header)
1. **~885 ms pre-window**: process init + spawning wallet/adblock backends + settings/profile/DB load (between the two STARTUP log lines).
2. **~483 ms CefInitialize** (blocks UI thread; the "2–5 s" code comment at `cef_browser_shell.cpp:4228` is stale — measured ~95 ms dev / ~483 ms prod).
3. **~817 ms** renderer spawn + bundle parse + React mount (splash → header).

The spinner is the React `#splash` in `frontend/index.html` (gold logo, `@keyframes spin`), removed by `window.removeSplash()` in `MainBrowserView.tsx:73-75` (first mount effect).

---

## Known-fragile startup safeguards (do NOT break)
The team has already spent significant effort here; `cef_browser_shell.cpp` startup is load-bearing and fragile. Verified anchors (re-verify line numbers — they shift):
- **Window is shown BEFORE CefInitialize** (`~:4228-4248`) specifically so a window is visible before CefInitialize blocks the UI thread. "skeleton toolbar visible before CefInitialize blocks" (`:4239`).
- **Backends launched EARLY** (`~:3920`) so the Rust wallet + adblock "warm up during CefInitialize." Fire-and-forget by design.
- `disable-gpu-compositing` (`simple_app.cpp:86`) exists to fix first-render black screen.
- 150 ms delayed `WasResized()/Invalidate()` in `OnAfterCreated` (`simple_handler.cpp:~1347`) fixes first-render black screen for tabs; mac header uses immediate + 100 ms forced paint.
- Header is **windowed** (`SetAsChild`) on Windows, **OSR** on macOS — different paint/visibility models.
- Header created first in `OnContextInitialized` (`simple_app.cpp:~210`), before tabs/session restore. `g_picker_mode` branches the header URL to the profile picker.

## Code anchors (verified 2026-06-22)
- Header create: `cef-native/src/handlers/simple_app.cpp:~210` (`OnContextInitialized`)
- STARTUP `elapsed()` logs + window-show + CefInitialize: `cef-native/cef_browser_shell.cpp:~4170-4403`
- Backend launch: `cef_browser_shell.cpp:~3920`
- Header load-state: `simple_handler.cpp:OnLoadingStateChange:~1013`, `OnAfterCreated:~1331`
- Splash: `frontend/index.html` (#splash), removed `MainBrowserView.tsx:73-75`
- Prod transport: `cef-native/include/core/LocalFileResourceHandler.h` (no cache headers), dispatch `simple_handler.cpp:~7473`

## Instrumentation added (env-gated, UNCOMMITTED — keep or remove later)
- `cef-native/include/core/PerfMarks.h` (new)
- `simple_app.cpp` / `simple_handler.cpp`: CreateBrowser stamp, OnAfterCreated/LoadEnd logs, `perf_report` IPC, `CefBeginTracing`/`CefEndTracing` (all gated on `HODOS_PERF_TRACE=1`)
- `main.tsx` + `MainBrowserView.tsx`: `performance.mark`s + `perf_report` send
- `cef-native/stage_frontend_for_perf.ps1`, `cef-native/win_run_perf.ps1`
- Run: stage frontend → `$env:HODOS_DEV=1; $env:HODOS_PERF_TRACE=1; .\HodosBrowser.exe --profile=Default`. Trace → `hodos_perf_trace.json` (async write; may not flush if closed too fast).

---

## Plan (study-first; no code until an adversarial review clears a specific change)
1. **Confirm current numbers** — dev perf build, wallet cold, to validate the 2.4 s on `0.4.0` code + localize the 885 ms gap.
2. **MAP (read-only)** the current startup sequence (C++ main→header-visible), root-cause each of the 3 phases, and catalog the fragile safeguards so we know what NOT to break. Include macOS (`cef_browser_shell_mac.mm`) differences.
3. **RESEARCH** how Vivaldi (Chromium + React chrome) and peers make startup feel instant / show chrome immediately (native skeleton pre-paint, splash strategy, deferring CefInitialize-blocked work, persisted-UI snapshot). Key question: the React header can't paint before CefInitialize — so what's the fastest *perceived*-ready trick?
4. **SYNTHESIZE** ranked candidate levers, each with expected saving + RISK to the fragile startup + macOS parity + safe-vs-needs-review.
5. **Adversarial review** (reviewer + skeptic) of any specific proposed change BEFORE implementing. Then per-chunk harness (implement → build → smoke → adversarial code review → commit only when asked).

---

## Deep-study synthesis (2026-06-22, workflow `wf_04cf7e5a-389`, 6 agents, read-only)

> ⭐⭐ **SUPERSEDING BREAKTHROUGH (2026-06-22, later): it's a first-PAINT lag, not startup/React/overlays.** Measured with a double-`requestAnimationFrame` "header painted" mark (vs the old JS-ready mark): `headerReady` (React mounted) = **173 ms**, `headerPainted` (browser actually draws) = **~2236 ms** → **~2 s of pure first-paint lag.** Ruled OUT by measurement: React, MUI/bundle (chunks fetch 18-25 ms warm), hooks, cold-disk startup, AND overlays (clean A/B: OFF=2450 ms ≈ ON=2223 ms via `HODOS_NO_OVERLAYS`). The spinner is honest — owner confirmed black-for-2s without it. **PRIME SUSPECT: `--disable-gpu-compositing`** (`simple_app.cpp:88`, set globally, "fix first-render black screen") → software compositing of the WINDOWED header → delayed first present (rAF not firing until 2.2 s = compositor not scheduling frames). Confirmation test wired: `HODOS_GPU_COMPOSITING=1` skips the flag. GPU-settings deep-dive running (`wf_c984f31f-1a0`): right config for fast paint without the black screen, Win+mac, security+perf. The cold-disk analysis below is real but SECONDARY to this paint lag. New instrumentation (UNCOMMITTED, throwaway): `headerPainted` rAF + per-chunk resource timing (`MainBrowserView.tsx`), splash stripped for test (`index.html`), `HODOS_NO_OVERLAYS` (`cef_browser_shell.cpp:4358`), `HODOS_GPU_COMPOSITING` (`simple_app.cpp:88`).

**Thesis:** the 2.4 s (old beta) is overwhelmingly **cold-disk + cold-process** cost, not React. Cold-disk cost is *environmental*, so it would still hit a **cold production launch of current code** — the ~757 ms dev number was a WARM run. **We still lack a cold-prod measurement of current `0.4.0`.**

**Refined 3-phase root causes:** (1) Pre-window ~885 ms = cold `CreateProcessA` of the two Rust exes off disk + two synchronous `IsPortListening` 100 ms Winsock probes (`cef_browser_shell.cpp:3923-3928`) that always time out in prod (dev short-circuits). (2) CefInitialize 483 ms prod vs 95 ms dev = cold pak/snapshot/DLL reads + populated disk-cache-backend init + GPU bring-up; **two prod-only suspects added by research: WPAD/proxy auto-detect (1-2 s) and field-trial/background-net init.** (3) Renderer→header 817 ms = cold subprocess spawn + V8 init + parse of ~693 KB JS (vendor-mui 370 + index 277 + vendor-react 46), served header-less/uncompressed by `LocalFileResourceHandler`; `removeSplash` already fires as early as possible (nothing awaits it).

**macOS (inferred from code; NO startup instrumentation exists):** structure is **inverted and worse** — CefInitialize first, then `StartWalletServer`/`StartAdblockServer` **block the main thread up to 10 s each** (20×500 ms poll, `mac.mm:4770`), THEN the NSWindow — nothing visible until all done. Doc correction: mac header is **windowed `SetAsChild`** (`mac.mm:4819`), not OSR.

### Ranked levers
| # | Lever | Phase | Saving | Risk | Tag |
|---|---|---|---|---|---|
| **L1** | Proxy auto-detect kill switch (`--winhttp-proxy-resolver`, not `--no-proxy-server`) in `OnBeforeCommandLineProcessing` | 2 | REAL 1-2 s *if* WPAD is the cause (UNVERIFIED) | low to sequence; breaks corp proxy if wrong variant | NEEDS-REVIEW |
| **L2** | Skeleton toolbar before React — **HTML/CSS in index.html (cross-platform, SAFE)** or native GDI in shell window (covers more, more C++) | 3 | PERCEIVED — kills the spinner | low (HTML variant: zero C++) | SAFE (win) / RISKY (mac native) |
| **L3** | `log_severity=WARNING` in prod builds | 1+2 | REAL ~20-80 ms | low | SAFE |
| **L4** | Extend `--disable-features` (+OptimizationHints,MediaRouter,…) + `--disable-background-networking` etc. | 2-3 | REAL 50-200 ms | MEDIUM (bg-net can break XHR onload → smoke) | NEEDS-REVIEW |
| **L5** | Windows `/prefetch:1` launcher hint | 1 | REAL (HDD only, 2nd+ launch) | none | SAFE |
| **L6** | disk-cache cap + SQLite VACUUM | 2 | REAL 50-200 ms on big profiles | MEDIUM (VACUUM locking) | NEEDS-REVIEW |
| **L7** | V8 code-cache: stable URL + cache headers on `LocalFileResourceHandler` (+`//# allFunctionsCalledOnLoad`) | 3 | REAL 20-40% parse on warm launches *if* it warms (UNVERIFIED) | low to sequence | NEEDS-REVIEW |
| **L8** | Move `IsPortListening`/spawn off UI thread (win) | 1 | REAL ~200 ms | HIGH — touches fragile backends-early safeguard (`c398350`) | RISKY |
| **L9** | macOS: make backend launch non-blocking (mirror Windows) | 1 (mac) | REAL up to 10 s worst case | HIGH — inverts mac ordering; gate on instrumentation | RISKY, mac-only |
| L10-13 | multi-threaded msg loop / warm-start service / slim subprocess exe / V8 heap snapshot | — | deep | RISKY/future | OUT |

### #1 recommendation (from study)
Pursue **L1 gated behind a trace**, bundled with **L2 (HTML/CSS skeleton)** as the guaranteed perceived-win backstop. **Do NOT ship L1 on hypothesis** — first trace a real cold prod install across `CefInitialize start→done` (PerfMarks/`CefBeginTracing` already wired) + WPR/ETW to confirm WPAD vs cold-disk vs cache-init. That single trace decides whether the real-time lever is L1 (proxy) or L6/L7 (cache).

### Must-measure-before-optimizing
Cold-prod trace of current `0.4.0` (the decisive unknown); split Phase 1 with 2 more `elapsed()` logs; confirm the `IsPortListening` 100 ms timeouts fire; confirm whether V8 code-cache warms for the header-less disk bundle (decides L7); **add `elapsed()` instrumentation to macOS** (none today) before any mac change.

### Do-NOT-touch safeguards (expanded by study)
ShowWindow+DwmFlush before CefInitialize; `disable-gpu-compositing`; 150 ms delayed WasResized/Invalidate; backends-early fire-and-forget + detached health threads + atomic flags (`c398350` "8 s → 400 ms"); `persist_session_cookies` MUST stay disabled (`3761e075`); header windowed not OSR; header-first in OnContextInitialized; `g_picker_mode` branch; macOS 2 s `dispatch_after` subview fix-up.

---

## ⭐⭐ GPU/compositing research synthesis (2026-06-22, `wf_c984f31f-1a0`, 5 agents) — the REAL lead

**The 2 s is a first-PAINT lag (header renderer gets no BeginFrames for ~2 s), and the root is almost certainly a MISSING Windows `app.manifest`.**

### Live A/B results (this drove everything)
- `headerReady` (React mounted) = **173 ms** ; `headerPainted` (double-rAF, actual frame) = **~2236 ms**.
- `HODOS_GPU_COMPOSITING=1` (GPU compositing ON, naive flag removal): **2698–3054 ms + black screen/flicker** → WORSE. Per the research, that black screen is the *tell* of the missing manifest (see below), not proof GPU compositing is bad.
- Overlays A/B (`HODOS_NO_OVERLAYS`): OFF 2450 ≈ ON 2223 → not overlays.

### Root-cause lead — verified gap
**No Windows `app.manifest` exists** (no `.manifest` file in `cef-native/`, no manifest tooling in `CMakeLists.txt` — only the unrelated wallet `ManifestFetcher`). Without an `app.manifest` declaring the **Win10/11 `supportedOS` GUID**, Chromium misdetects the OS as Vista/7 and **disables modern GPU paths** — the #1 cause of GPU-compositing black screen on Win10/11 (CefSharp #4782/#4707). Explains the black screen, the slow fallback, AND why `--disable-gpu-compositing` was added as a band-aid. **The whole "band-aid family"** (`disable-gpu-compositing` + WasResized/Invalidate hacks `simple_handler.cpp:1372-1572` + mac `dispatch_after(2s)` `mac.mm:4884-4921` + dark-fill + show-before-CefInitialize) all fight this one gremlin.

### Recommended fix (Windows) — all NEEDS-ADVERSARIAL-REVIEW, staged & reversible
1. **Prereq:** add an `app.manifest` with the Win10/11 `supportedOS` GUID (may fix the black screen alone).
2. Drop `--disable-gpu-compositing` (Win) + pin `--use-angle=d3d11` + keep `--ignore-gpu-blocklist`. **GPU sandbox KEPT** (this flag ≠ `--disable-gpu-sandbox`).
3. Structural: **show `g_header_hwnd` only when first compositor frame is ready** (parent fills the slot dark) → lets the dark-fill + invalidate hacks retire.
4. `--enable-gpu-rasterization` as a separate later step. Smoke matrix: Intel iGPU / hybrid / blocklisted / discrete GPU; cold-start; multi-window; resize; all OSR overlays; Win11 24H2 MPO. Keep all WasResized/Invalidate nets until fleet-clean, retire last.

### Recommended fix (macOS) — separate track, structural not flags
Add `elapsed()` startup instrumentation FIRST (none exists) → `[setWantsLayer:YES]` on header NSView ancestors BEFORE `CreateBrowser()` (CEF #17240) + show-on-ready → then retire the `dispatch_after(2s)` + forced-paint hacks. `--use-angle=metal` likely already default (verify via chrome://gpu).

### Security (drop these regardless of the paint fix)
- **DROP `--in-process-gpu` AND `--disable-gpu-sandbox`** even from the macOS `HODOS_MAC_DEV_FLAGS` gate (sandbox-escape class + broken/Won't-Fix in M136; if GPU crashes in dev, fix code-signing entitlements instead — CEF #18900).
- Question **`--remote-allow-origins=*`** (`simple_app.cpp:85`) — opens remote-debug to any origin.
- **KEEP** the GPU sandbox on both platforms.

### Diagnostics to keep in mind
Expose `chrome://gpu` (dev-only CEF URL handler) to read "Disabled Features"/"Driver Bug Workarounds" for silent software-compositing fallbacks. `--gpu-startup-dialog` to time GPU-process init.

---

## 🧭 NEXT CONTEXT — the workflow the owner wants (we cleared context to start fresh here)
Author + run a WORKFLOW (medium-weight; owner on prepaid credits):
- **(A) Full command-line-flags audit** — enumerate EVERY flag in `simple_app.cpp:74-128` (OnBeforeCommandLineProcessing) + anywhere else; what each does; WHY we set it (code/comments/git `log -S`); are they correct vs **other browser projects** (Brave/Electron/cefclient/CEF docs). **Adversarial SECURITY review + adversarial OPTIMIZATION review.** Very careful on change recommendations (CLAUDE invariant #5).
- **(B) Header-load agent** with the new focus: first-PAINT lag → app.manifest + GPU compositing + show-on-ready (use the A/B data + this synthesis).
- **(C) macOS agent** — wantsLayer + show-on-ready + instrument-first + drop the dangerous dev flags.
- Synthesize → **adversarial design review** → propose code → owner approves EACH change → commit ONLY when asked.

**Uncommitted instrumentation in the tree** (env-gated/throwaway): `PerfMarks.h`, `HODOS_GPU_COMPOSITING`/`HODOS_NO_OCCLUSION`/`HODOS_NO_OVERLAYS` toggles, `headerPainted`+chunk timing, `stage_frontend_for_perf.ps1`/`win_run_perf.ps1`/`PERF_TESTING.md`. **`index.html` splash spinner RESTORED (2026-06-22).** Decide keep-gated vs revert before any feature commit (see C1 below).

---

## 📋 WORKFLOW OUTPUT — 16-proposal sequenced plan (workflow `wf_bed7a9d7-386`, 7 agents, 2026-06-22)

Flags audit + adversarial security/optimization + header + macOS → synthesis → adversarial design review (skeptic). **All proposals; no production code written. Owner approves each; commit only when asked.** Skeptic **greenlit-now (zero/low risk, paint-neutral):** `W0`(done) `M1` `C2`(done) `C1` `S5` `S3`.

**Phase 0 — measure first, NO code (gate):** `W0` ✅ DONE → GO (see top). Baseline `headerPainted` vs `headerReady`. `dumpbin /headers HodosBrowser.exe` to check for an auto-embedded RT_MANIFEST before W1 (avoid double-manifest link error). `M1` add macOS `elapsed()` startup logs (none today).

**Phase 0.5 — safe-now, parallel, paint-neutral:** `C2` skeleton/spinner (✅ spinner restored) · `C1` resolve dirty perf toggles (KEEP `HODOS_GPU_COMPOSITING` iff W2 proceeds; fold `HODOS_NO_OCCLUSION` into the GPU A/B; restore spinner ✅) · `S5` `log_severity=WARNING` in release · `S3` drop macOS `--in-process-gpu` + `--disable-gpu-sandbox` from the dev gate (keep GPU sandbox; fix dev GPU via entitlements).

**Phase 1 — Windows root fix (W0 confirmed it):** `W1` add `cef-native/hodos.manifest` (Win10/11 GUID `{8e0f7a12-bfb3-4fe8-b9a5-48fd50a15a9a}` + 8.1/8/7 + longPathAware) via linker `/MANIFESTINPUT` appended to `CMakeLists.txt:~449` `LINK_FLAGS` (keep `hodos.rc` icon-only) — **land ALONE, measure paint collapse** → `W2` flip `--disable-gpu-compositing` off by default (`simple_app.cpp:90-94`, env becomes rollback) + pin `--use-angle=d3d11`; **`--ignore-gpu-blocklist` split out as a SEPARATE owner-approved decision, default NO** (skeptic caught it smuggled in) → `W3` show-on-ready: gate `ShowWindow(g_header_hwnd)` on **existing `OnLoadingStateChange`** + **hard timeout fallback** (NOT a new async IPC), only if the flash survives W1/W2. `W1b` DPI/PerMonitorV2 = wholly separate later effort (will desync the 1.0f-hardcoded OSR overlays — audit overlay anchor math first).

**Phase 2 — Windows cleanup, LAST:** `W4` retire band-aids **one per commit, full GPU matrix each**; per-overlay `WasResized/Invalidate` nets default **KEEP** (OSR gremlin the manifest doesn't touch); dark-fill retired last.

**Phase 3 — macOS (after M1):** `M2` `setWantsLayer:YES` on header NSView ancestors before `CreateBrowser()` (Stage A alone may fix it) → retire `dispatch_after(2s)` fix-up only after smoke. `M3` non-blocking backend launch (mirror Windows atomic-flag + detached health thread) — **highest risk**, M1-gated, inverts mac ordering.

**Security workstream (paint-neutral, does NOT block startup):** `S1` `command_line_args_disabled=true` in release (`cef_browser_shell.cpp:3795`) — keystone that makes S2/S3 enforceable. `S2` gate `remote_debugging_port` OFF in prod (`HODOS_DEVTOOLS?9222:0` at `:3961`+mac `:4739`) + drop/scope `--remote-allow-origins=*` (`simple_app.cpp:85`). **OPEN Q for wallet team:** is spend auth gated on `domain_permissions`/approval gates and NOT the origin header alone? Determines whether S2 is "critical wallet drain" or "important hardening." `S4` gate macOS `--use-mock-keychain` behind unsigned/dev before mac release.

**Gth corrections from skeptic (verify before editing):** `remote_debugging_port` is **conditional** (0 picker / 9222 Default / 9222+N others), not unconditional. All `mac.mm` 47xx/48xx line numbers are **unverified — re-grep**. W3/M3 intrude on the fragile pre-CefInitialize sequence (invariant #8) → adversarial code review required.

**Must-measure-before-ANY-commit:** post-W1 manifest A/B (the GO proof); double-rAF `headerPainted` baseline; `dumpbin /headers`; the global-OSR (`windowless_rendering_enabled=true` `cef_browser_shell.cpp:3798`) co-variable; M1 macOS cold-launch logs + the suspected 20×500ms backend block; cold-PROD ETW/WPR trace before ANY speculative perf flag (WPAD/bg-net — pure hypothesis until traced).
