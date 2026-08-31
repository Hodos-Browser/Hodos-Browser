# Modal buttons are unclickable on a small screen, clickable on a large monitor

**Status:** 🔴 **OPEN — verified 2026-08-31.** Phase 1's `a3d8202` touched **only** `WalletDashboard.css`, a different surface; the modal is untouched. Labelled 2026-08-31 (had no status line).
**Sprint:** 📌 **Phase 3.5 (layout), not Phase 10** — moved 2026-08-31. Same subsystem and the SAME T3 rig (two windows, two monitors, mixed-DPI matrix cell #9) that `P3.5-A3` already requires.

**Reported by the owner 2026-08-21**, during the bitgenius.net connect test.
**For the DPI phase (Phase 1 — overlay/DPI).** Owner-observed, not yet instrumented.

## Observation

> "I first opened it on my small screen and I could not click the buttons on that modal. I opened it
> again on the large monitor and I was able to click the buttons in the modal."

Same modal, same build, same session — **only the display changed.** That is a display-geometry
bug, not a React bug.

## ⛔ CORRECTION 2026-08-23 — the premise below is wrong, the mechanism is real

**MEASURED** (grep, `cef-native/`): every Windows overlay is created with **`SetAsPopup`**
(`simple_app.cpp` — settings, wallet, backup, brc100auth, notification, menu, omnibox, cookie,
download, …). That is a **windowed** CEF browser, which receives mouse input from Windows directly.
There is **no `SetAsWindowless`** in that file at all. So *"overlays are OSR"* is **false on
Windows** — it is true on macOS, and `MAC_RELAY_BETA3.md` §D1 says so explicitly.

⭐ **But the hand-forwarding is real anyway**: `cef_browser_shell.cpp` has **48 `GET_X_LPARAM`
sites** feeding `SendMouseClickEvent` / `SendMouseMoveEvent` with **zero DPI conversion**, while the
process is `PER_MONITOR_AWARE_V2`.

So the real question for Phase 1 is sharper than this ticket originally framed it: **windowed
browsers that already receive native input ALSO have a parallel hand-rolled injection path.** Ask
why it exists before assuming its coordinates are the bug — a double-delivery or a
correct-native-plus-wrong-synthetic pair produces different symptoms than a single wrong path, and
the fix differs accordingly.

⚠️ Still an **assumption, not a finding**: nobody has reproduced the offset under instrumentation.

## Why this is probably a hit-test bug, not clipping

~~Overlays are **OSR (off-screen rendered)** browsers.~~ (See the correction above — on Windows they
are windowed `SetAsPopup` browsers.) Each overlay's WndProc nonetheless forwards mouse input by
hand —

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
