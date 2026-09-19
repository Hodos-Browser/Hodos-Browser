#pragma once

#ifdef _WIN32
#include <windows.h>

// Fixed header height in CSS pixels: tab bar (42px) + toolbar (53px) + 1px buffer.
// Matches macOS fixed headerHeight = 96.
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

#endif // _WIN32
