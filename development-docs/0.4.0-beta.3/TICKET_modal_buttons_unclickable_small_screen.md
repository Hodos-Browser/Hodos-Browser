# Modal buttons are unclickable on a small screen, clickable on a large monitor

**Reported by the owner 2026-08-21**, during the bitgenius.net connect test.
**For the DPI phase (Phase 1 — overlay/DPI).** Owner-observed, not yet instrumented.

## Observation

> "I first opened it on my small screen and I could not click the buttons on that modal. I opened it
> again on the large monitor and I was able to click the buttons in the modal."

Same modal, same build, same session — **only the display changed.** That is a display-geometry
bug, not a React bug.

## Why this is probably a hit-test bug, not clipping

Overlays are **OSR (off-screen rendered)** browsers. They do not get mouse input for free: each
overlay's WndProc forwards it by hand —

```cpp
case WM_LBUTTONDOWN:
    SetFocus(hwnd);
    browser->GetHost()->SetFocus(true);
    browser->GetHost()->SendMouseClickEvent(...);   // ← coordinates translated HERE
```

If the coordinates handed to `SendMouseClickEvent` are not in the same space as the browser's view
size, the click lands somewhere other than where the user aimed. The button **renders correctly and
still does nothing**, which matches the report better than clipping does: clipping would make the
button visibly cut off or off-screen, and the owner described it as present but unresponsive.

Prime suspects, in order:

1. **DPI scale not applied to the click coordinates.** `LayoutHelpers.h` provides `ScalePx(cssPx, hwnd)`
   and `GetHeaderHeightPx(hwnd)` precisely for this. Any overlay sizing itself in raw CSS pixels
   while its OSR view is sized in physical pixels will be offset by the scale factor — invisible at
   100%, wrong by 25–50% at 125%/150%.
2. **Per-monitor DPI.** The offset appearing on one monitor and not the other is the signature of
   a `WM_DPICHANGED` path that resizes the HWND but not the CEF view, or vice versa.
3. **Overlay sized from the wrong monitor's metrics** — e.g. `GetSystemMetrics` (primary monitor)
   instead of the owning window's monitor.

## Reproduce

Run the **notification overlay** (any approval modal — the bitgenius connect bundle is a convenient
trigger) on DPI matrix cells **#4 (125% / 1366×768)**, **#6 (150% / 1366×768)** and **#9 (mixed-DPI)**
per `development-docs/DevOps-CICD/DPI_RESOLUTION_TEST_MATRIX.md`.

⛔ **Instrument the hit test, do not eyeball it.** Log the incoming `WM_LBUTTONDOWN` client
coordinates, the values passed to `SendMouseClickEvent`, the overlay HWND rect and the CEF view
size, all in the same line. If they diverge by the DPI scale factor, that is the bug and the log
line proves it. "I clicked and nothing happened" is not a measurement.

⛔ **Negative control:** the same modal at 100% / large monitor must be seen to WORK in the same
run, or the test cannot distinguish "DPI bug" from "modal is broken everywhere".

## Why it matters more than a cosmetic DPI issue

This modal is a **consent dialog on the money path**. A user who cannot click *Deny* cannot refuse.
Worse, a user who aims at *Deny* and whose click is silently translated elsewhere could land on
*Allow*. Until the hit-test offset is measured, **we cannot rule out that a mis-scaled click
approves something the user tried to refuse** — which would make this a security bug wearing a
layout bug's clothes.

## Related

- CLAUDE.md, "CEF Input Patterns → Focus & Keyboard Handling (C++ side)" — the OSR forwarding contract.
- `cef-native/include/core/LayoutHelpers.h` — `ScalePx`, `GetHeaderHeightPx`.
- The 14 overlay WndProcs in `cef_browser_shell.cpp`; the notification overlay is `NotificationOverlayWndProc`.
- macOS equivalent is unexamined; overlays there are borderless `NSWindow`s with NSEvent monitors.
