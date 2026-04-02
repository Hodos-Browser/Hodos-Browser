# Overlay DPI Scaling — IMPLEMENTED (2026-04-02)

## Problem

With Per-Monitor DPI V2 enabled (`SetProcessDpiAwarenessContext` in `WinMain`), the main window header and tab content scale correctly when dragging between monitors with different DPI. However, overlay panels (wallet, cookie, download, menu, profile, omnibox, settings) do not scale — they appear proportionally smaller on high-DPI monitors compared to the rest of the UI.

## Root Cause: Coordinate Space Mismatch

Overlay positioning mixes two incompatible coordinate spaces:

1. **React CSS pixels** — `getBoundingClientRect()` and `window.innerWidth` return values in CSS pixels. These are device-independent. The `iconRightOffset` values sent via IPC from React to C++ are in this space.

2. **Win32 physical pixels** — `GetWindowRect()`, `SetWindowPos()`, and `CreateWindowEx()` operate in physical (device) pixels. With Per-Monitor DPI V2, physical pixels differ from CSS pixels on non-100% DPI monitors.

Example at 150% DPI (144 DPI):
- React reports `iconRightOffset = 50` (CSS pixels)
- Actual physical offset from window edge = `50 * 1.5 = 75` physical pixels
- C++ uses `headerRect.right - 50 - panelWidth` — positions overlay 50 physical pixels from the right instead of 75
- Result: overlay is shifted ~25px to the right

This mismatch affects both overlay **positioning** (X/Y coordinates) and **sizing** (width/height) because both are calculated using a mix of React-reported CSS values and Win32 physical-pixel rects.

## What We Tried (Sprint 3, 2026-04-01)

### Attempt 1: Scale overlay HWND sizes via `ScalePx()`
- Added `ScalePx(cssPx, hwnd)` helper to `LayoutHelpers.h` using `MulDiv(cssPx, GetDpiForWindow(hwnd), 96)`
- Applied to all overlay creation functions in `simple_app.cpp` and reposition code in `cef_browser_shell.cpp`
- **Result**: Overlays opened at correct size briefly, then snapped to wrong size/position. The `iconRightOffset` values from React were still in CSS pixels, creating positioning errors.

### Attempt 2: Scale `device_scale_factor` in render handler
- Changed `my_overlay_render_handler.cpp` `GetScreenInfo()` from hardcoded `1.0f` to `GetDpiForWindow(hwnd_) / 96.0f`
- **Result**: CEF rendered overlay content at higher resolution, but the HWND size wasn't scaled to match, so content appeared cropped or the viewport was smaller than expected.

### What Was Reverted
All overlay DPI changes were reverted. Overlays use hardcoded CSS pixel sizes and `device_scale_factor = 1.0f`. They appear slightly smaller on high-DPI monitors but are correctly positioned and functional.

### What Was Kept
- `SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)` in `WinMain` — enables `WM_DPICHANGED` which fixes the header scrollbar issue (B-4)
- `WM_DPICHANGED` handler with `NotifyScreenInfoChanged()` on all browsers — header and tab content scale correctly
- `ScalePx()` helper remains in `LayoutHelpers.h` for future use

## Fix Applied (2026-04-02)

All three layers were changed simultaneously to keep the coordinate pipeline synchronized:

A complete fix requires changes at three layers:

### 1. React: Send DPI-scaled offsets
The `iconRightOffset` calculation in `MainBrowserView.tsx` needs to account for `window.devicePixelRatio`:

```tsx
// Current (CSS pixels — wrong for high DPI)
const iconRightOffset = Math.round(window.innerWidth - rect.right + rect.width / 2);

// Fixed (physical pixels)
const dpr = window.devicePixelRatio || 1;
const iconRightOffset = Math.round((window.innerWidth - rect.right + rect.width / 2) * dpr);
```

This applies to every IPC call that sends pixel offsets: `cookie_panel_show`, `download_panel_show`, `toggle_wallet_panel`, `profile_panel_show`, `menu_show`.

### 2. C++: Scale overlay HWND sizes
All hardcoded overlay sizes in `simple_app.cpp` and reposition code in `cef_browser_shell.cpp` need `ScalePx()`:

```cpp
// simple_app.cpp — overlay creation
int panelWidth = ScalePx(380, g_hwnd);
int panelHeight = ScalePx(400, g_hwnd);
int overlayY = headerRect.top + ScalePx(104, g_hwnd);

// cef_browser_shell.cpp — WM_MOVE and WM_SIZE reposition
int dpWidth = ScalePx(380, hwnd);
```

The `104` offset (header height in CSS pixels) must also be scaled, or better, replaced with `GetHeaderHeightPx(g_hwnd)` which already handles DPI.

### 3. Render handler: Report correct device_scale_factor
In `my_overlay_render_handler.cpp`, `GetScreenInfo()` should report the actual DPI:

```cpp
UINT dpi = GetDpiForWindow(hwnd_);
screen_info.device_scale_factor = static_cast<float>(dpi) / 96.0f;
```

This tells CEF to render at the correct resolution for the monitor. With both the HWND size scaled (step 2) and the scale factor correct (step 3), the overlay content will be sharp and correctly sized.

### 4. Handle monitor transitions
When the main window moves to a different monitor (`WM_DPICHANGED`), visible overlays need to be repositioned and resized with the new DPI. The existing reposition code in `WM_MOVE`/`WM_SIZE` handlers will handle this if steps 1-3 are in place.

## Files Involved

| File | What to Change |
|------|---------------|
| `frontend/src/pages/MainBrowserView.tsx` | Multiply icon offsets by `devicePixelRatio` before sending IPC |
| `cef-native/src/handlers/simple_app.cpp` | `ScalePx()` on all overlay panel sizes (~18 instances) |
| `cef-native/cef_browser_shell.cpp` | `ScalePx()` on overlay sizes in WM_MOVE and WM_SIZE handlers |
| `cef-native/src/handlers/my_overlay_render_handler.cpp` | `device_scale_factor` from `GetDpiForWindow()` |
| `cef-native/include/core/LayoutHelpers.h` | `ScalePx()` helper already exists |

## Testing

Test on a multi-monitor setup with different DPI settings (e.g., external monitor at 100%, laptop at 125% or 150%):

1. Open each overlay on the primary monitor — correct size and position
2. Drag window to secondary monitor — overlays should reposition correctly
3. Open each overlay on the secondary monitor — correct size and position
4. Verify overlay content is sharp (not blurry from bitmap stretching)
5. Verify overlay close-on-click-outside still works (hit areas must match visual bounds)
