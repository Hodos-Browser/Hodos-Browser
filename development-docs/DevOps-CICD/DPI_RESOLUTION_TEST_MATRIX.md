# DPI & Resolution Test Matrix

**Created:** 2026-07-20
**Purpose:** A standing pre-release gate to catch "works on my machine" layout bugs caused by Windows display-scaling / resolution variance — the class of bug where browser chrome (toolbar, header, overlays) fits on the dev machines but breaks on a user's differently-scaled screen.

> Canonical home per invariant #12. Referenced from the root `CLAUDE.md` Testing Standards table. Windows-focused (per-monitor DPI is a Windows problem); macOS uses backing-scale and is covered by the same visual pass at Retina/non-Retina.

---

## Why this exists (the recurring failure mode)

Windows machines run at different **display scaling** (100 / 125 / 150 / 175 %) and **resolutions**. The number that actually governs a React layout is the **logical (CSS) width = physical width ÷ scale**. A layout tuned on a 1920×1080 @100% box (1920 logical px) silently breaks on a 1366×768 @150% laptop (~910 logical px). Because the dev machines happen to sit at the tuned scaling, "it works here" tells you nothing.

**Concrete instance (2026-07):** the top toolbar's right-side buttons (wallet / profile / menu) were clipped on an older 1366×768 / ~150 % laptop but not on the dev boxes. Root cause was **not** C++ or DPI-awareness (both exonerated — the native header width is a physical `clientWidth − 10px` passthrough that cannot overflow, and the manifest declares no `dpiAware` so `PER_MONITOR_AWARE_V2` genuinely takes effect). It was a **React CSS regression**: the address bar's `minWidth` had drifted from `0` back to `200`, and a later header-UX pass added un-shrinkable children (bookmarks, site-info hub, logo SVG, privacy-shield) — pushing the toolbar's min-content width above what fits at ~910 logical px, so the `nowrap` + `overflow:hidden` toolbar clipped its last flex child. See History below.

Without a DPI test gate, this class recurs on **every** layout change. This matrix is the gate.

---

## The matrix

Logical width ≈ physical width ÷ scale. Cells are ranked by how likely they are to expose a shrink/overflow bug (narrower logical width = higher risk).

| Cell | Scaling | Resolution | ≈ Logical width | Priority | Notes |
|------|---------|-----------|-----------------|----------|-------|
| 1 | 100 % | 1920×1080 | 1920 | baseline | tuned dev case; must stay pixel-clean |
| 2 | 100 % | 1366×768 | 1366 | med | common cheap-laptop native res |
| 3 | 125 % | 1920×1080 | 1536 | med | |
| 4 | 125 % | 1366×768 | ~1093 | **HIGH** | plausible "Dad's machine" |
| 5 | 150 % | 1920×1080 | 1280 | med | |
| 6 | 150 % | 1366×768 | ~910 | **HIGHEST** | narrowest realistic; the reported machine |
| 7 | 175 % | 1920×1080 | ~1097 | med | |
| 8 | 175 % | 1366×768 | ~781 | **HIGH** | extreme; stress the shrink floor |
| 9 | mixed-DPI | primary 150 % + secondary 100 % | — | **HIGH** | drag window across the monitor boundary → exercises `WM_DPICHANGED` re-layout |

**Minimum gate before a public promote:** cells **#4, #6, #9**. Full sweep (#1–#9) for a release/demo build or any change to the header, toolbar, or overlays.

---

## How to simulate each cell on ONE dev box (no target hardware)

- **M1 — Windows Settings scaling + sign-out (highest fidelity).** Settings → Display → Scale = `<s>`, then **sign out** (scaling only fully applies to a fresh session). Optionally drop desktop resolution to 1366×768. Launch via the normal dev flow. This exercises the entire native pipeline including the startup `WM_DPICHANGED`.
- **M2 — CEF flags (fast smoke / CI).** Launch the built shell with `--force-device-scale-factor=<s> --window-size=<w>,<h>` — do **not** maximize (that would widen the logical viewport). Example for the highest-risk cell #6:
  ```
  HodosBrowser.exe --force-device-scale-factor=1.5 --window-size=1366,728
  ```
  (cell #4 = `--force-device-scale-factor=1.25 --window-size=1366,728`; cell #8 = `--force-device-scale-factor=1.75 --window-size=1366,728`). **Never ship this flag** — it is a test lever only.
- **M3 — low-res VM.** A VM at exactly 1366×768 gives the full native pipeline, and a dual-monitor VM (primary 150 % / secondary 100 %) covers cell #9.

---

## Pass criteria + smoke assertion

**PASS =** every toolbar control (back / forward / refresh / bookmarks / wallet / profile / menu / download-when-present) is **fully visible and clickable**; the header fills the window client width edge-to-edge (no gaps or dark strips); no horizontal clip; the address bar shrinks (and its own inline controls clip) **before** any main-toolbar button is pushed off.

**Programmatic assertion** (run in the header browser's devtools console):
```js
document.querySelector('[aria-label="Menu"]').getBoundingClientRect().right <= window.innerWidth
```
Should be `true` at every cell. This is the one-line check to wire into a future automated smoke run at cell #6.

---

## Code-review checklist (DPI invariants — apply on any header / overlay / layout change)

- **Address-bar box must keep `minWidth: 0` (NOT `200`).** This exact value regressing `0 → 200` is what caused the toolbar clip. Load-bearing — do not re-introduce a fixed shrink floor. (`frontend/src/pages/MainBrowserView.tsx`.)
- Any flexible input/box that must shrink needs **`minWidth: 0`** on the item (and often on its `<input>`), or CSS min-content will refuse to shrink it.
- The toolbar's right-side cluster (`flexShrink:0`) is the *last* flex child — ensure something to its left (the address bar) can collapse first, so the right cluster never spills.
- **C++:** no `GetSystemMetrics` without the `...ForDpi` variant; every fixed px that positions a native child goes through `MulDiv(px, GetDpiForWindow(hwnd), 96)` (see `cef-native/include/core/LayoutHelpers.h`).
- **DPI awareness is declared in EXACTLY one place** — the runtime `SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)` at `cef-native/cef_browser_shell.cpp:4412`. **Do NOT add `<dpiAware>`/`<dpiAwareness>` to `cef-native/hodos.manifest`** — a manifest declaration silently *overrides* the runtime call, dropping the app to the wrong awareness with no error.
- **Open follow-up:** that `SetProcessDpiAwarenessContext` call discards its `BOOL` return. Add a one-line log so a silent fallback to UNAWARE (e.g. if a window is ever created before line 4412) stops being invisible.

---

## History

- **`7277980` (2025-12-16) "Fixed scaling for header bar"** — the original *horizontal* fix: set the address container to `flex: 1, minWidth: 0 // allow shrinking below content size`, added `overflow: 'hidden'` to the Toolbar, `flexShrink: 0` on the fixed buttons.
- **Header-UX pass (`aeef6e9` bookmarks, `bdd4beb` site-info hub, `396042e` hub auto-size, `6989ef7` downloads)** — reshaped the address box to `flex:'0 1 1200px', minWidth:200, maxWidth:1200` (a centered, capped design) and added un-shrinkable children inside it. This reintroduced the shrink floor `7277980` had removed → the toolbar clip regressed.
- **`ff3e2ee` (2026-07-20)** — restored `minWidth:0` + `overflow:'hidden'` on the address box and `minWidth:0` on the input; C++/DPI/manifest left untouched (investigated and exonerated). Pure shared-React CSS, macOS parity unaffected.

**Lesson:** the DPI machinery was correct; a plain CSS shrink-floor was the killer, and it re-drifted because there was no cross-DPI test. This matrix exists so the next header change is checked at cell #6 before it ships.
