#pragma once

#ifdef _WIN32
#include <windows.h>

// Fixed header height in CSS pixels: tab bar (42px) + toolbar (53px) + 1px buffer.
// Matches macOS fixed headerHeight = 96.
static const int HEADER_CSS_HEIGHT = 96;

// Returns header height in physical pixels, DPI-scaled for the given window's monitor.
inline int GetHeaderHeightPx(HWND hwnd) {
    UINT dpi = GetDpiForWindow(hwnd);
    return MulDiv(HEADER_CSS_HEIGHT, dpi, 96);
}

// Returns header height in physical pixels using system DPI.
// Use during initial window creation when no HWND exists yet.
inline int GetHeaderHeightPxSystem() {
    HDC hdc = GetDC(NULL);
    int dpi = GetDeviceCaps(hdc, LOGPIXELSY);
    ReleaseDC(NULL, hdc);
    return MulDiv(HEADER_CSS_HEIGHT, dpi, 96);
}

#endif // _WIN32
