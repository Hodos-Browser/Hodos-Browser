#pragma once

#ifdef _WIN32
#include <windows.h>

// Fixed header height in CSS pixels: tab bar (42px) + toolbar (53px) + 1px buffer.
// ⛔ This is NOT the macOS number — see `kMacHeaderHeightPt` below, which is 104.
// The two platforms' tab strips genuinely differ: `TabBar.tsx` renders
// `height: isMac ? 46 : 42` plus `paddingTop: isMac ? '4px' : 0` (content-box), so
// the mac strip is 50 and Windows' is 42. ⚠️ The comment here used to read
// *"Matches macOS fixed headerHeight = 96"* and stayed that way after the April
// change that grew the mac strip — which is precisely how macOS spent five months
// rendering 104 px of header into a 96 pt view. If you change one platform's strip,
// look at both constants.
static const int HEADER_CSS_HEIGHT = 96;

// \U0001f6a8 beta.3 Phase 11 item 7 route 2 (`P11-I7b`) — the Windows text-scale factor.
//
// ⛔ THE HEADER WINDOW AND ITS CONTENT WERE SIZED BY DIFFERENT NUMBERS. Chromium sizes a
// window's CONTENT by `monitor DPI × the accessibility text scale` — `ScreenWin::
// GetScaleFactorForHWND` is documented *"including accessibility adjustments"*, and
// `screen_win.cc :: GetScaleFactorForDPI` literally returns
// `scale * UwpTextScaleFactor::Instance()->GetTextScaleFactor()`. `GetDpiForWindow` carries
// **only** the monitor DPI. So every notch of Settings → Accessibility → Text size grew the
// header's content while its window stayed put, and the overflow hid behind the webview.
//
// 📏 MEASURED 2026-09-19 with the owner at the machine, text size 125% on a 125% monitor:
//     devicePixelRatio 1.5625   (= 1.25 × 1.25, both factors compounding)
//     content needs 95 CSS px · window gave 77 · **18 px hidden behind the webview**
// 👤 *"The bottom of the header section goes into the web view section underneath"* — and
// horizontal was fine, which is the signature: width follows the content, height did not.
//
// ⚠️ Why it is read from the registry rather than from Chromium: this header is ours and links
// against libcef, not Chromium's `ui/display`. `HKCU\Software\Microsoft\Accessibility\
// TextScaleFactor` is the value the Settings slider writes and the value the UWP
// `UISettings.TextScaleFactor` API reports — same source, reachable without the engine.
//
// ⚠️ NOT cached. The user can move the slider while we are running, and a cached factor would
// reintroduce exactly this bug in the window between the change and the next restart. The read
// is a single registry lookup on a layout path that already calls `GetDpiForWindow`.
inline UINT GetTextScalePercent() {
    DWORD value = 100, size = sizeof(value), type = 0;
    // ⛔ Absent key/value is the NORMAL case — the slider only writes it once it has been
    // moved. Failing closed to 100 means "no scaling", i.e. exactly the old behaviour.
    if (RegGetValueW(HKEY_CURRENT_USER, L"Software\\Microsoft\\Accessibility",
                     L"TextScaleFactor", RRF_RT_REG_DWORD, &type, &value, &size) != ERROR_SUCCESS) {
        return 100;
    }
    // Windows offers 100-225. Clamp rather than trust: a junk value must not be able to
    // produce a zero-height or absurd header.
    if (value < 100) value = 100;
    if (value > 300) value = 300;
    return static_cast<UINT>(value);
}

// Returns header height in physical pixels, scaled for the given window's monitor DPI **and**
// the accessibility text scale — the same two factors Chromium applies to the content.
inline int GetHeaderHeightPx(HWND hwnd) {
    UINT dpi = GetDpiForWindow(hwnd);
    return MulDiv(MulDiv(HEADER_CSS_HEIGHT, dpi, 96), GetTextScalePercent(), 100);
}

// Returns header height in physical pixels using system DPI.
// Use during initial window creation when no HWND exists yet.
inline int GetHeaderHeightPxSystem() {
    HDC hdc = GetDC(NULL);
    int dpi = GetDeviceCaps(hdc, LOGPIXELSY);
    ReleaseDC(NULL, hdc);
    return MulDiv(MulDiv(HEADER_CSS_HEIGHT, dpi, 96), GetTextScalePercent(), 100);
}

// Scale a CSS pixel value to physical pixels for the given window's DPI.
// Use for overlay panel sizes (e.g., ScalePx(380, hwnd) on a 150% monitor → 570).
inline int ScalePx(int cssPx, HWND hwnd) {
    UINT dpi = GetDpiForWindow(hwnd);
    return MulDiv(cssPx, dpi, 96);
}

#elif defined(__APPLE__)

// 🍎 macOS header height in POINTS — `D-h1`, owner decision 2026-09-19.
//
// ⭐ ONE constant for BOTH the primary window and secondary (⌘N / torn-off) windows.
// Before this they were 96 and 99 respectively, and the React header has rendered 104
// since April, so the primary clipped 8 pt and a secondary window clipped 5.
// 📏 Measured 2026-09-19 pre-fix, both windows of one process:
//     primary   window.innerHeight 96 · #root scrollHeight 104  -> 8 pt behind the webview
//     secondary window.innerHeight 99 · #root scrollHeight 104  -> 5 pt behind the webview
//
// The number is pinned against React, not chosen: tab strip 50 (`TabBar.tsx` —
// `height: isMac ? 46 : 42` + `paddingTop: isMac ? '4px' : 0`, content-box) + toolbar 54.
// ⛔ Changing the mac tab strip without changing this re-creates the original defect,
// and it fails INVISIBLY: every control stays clickable, the bottom of the toolbar just
// slides under the webview. Cause of the original: `5c0bcd7` (2026-04-15) grew the strip
// 42 -> 50 and left the native header at 96.
//
// ⚠️ NO DPI TERM, unlike the Windows half above. macOS folds nothing but
// `backingScaleFactor` into the device scale (`ui/display/mac/screen_mac.mm`), CSS px ARE
// points, and there is no accessibility text-scale factor to compound — so a point here
// is a CSS pixel there, always.
inline constexpr int kMacHeaderHeightPt = 104;

#endif // _WIN32 / __APPLE__
