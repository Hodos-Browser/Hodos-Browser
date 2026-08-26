#pragma once

// Physical-pixel -> CEF view (logical/DIP) conversion for the windowless overlay browsers.
//
// WHY THIS EXISTS
// ---------------
// The ~15 overlays are windowless (OSR) CEF browsers. There is no CEF window to receive
// native input, so each overlay's WndProc forwards mouse events by hand. Those events carry
// Windows CLIENT coordinates, which are PHYSICAL pixels because the process is
// PER_MONITOR_AWARE_V2. CEF documents CefMouseEvent's x/y as "relative to the upper-left
// corner of the VIEW" — and our view is LOGICAL: MyOverlayRenderHandler::GetViewRect divides
// the client rect by the DPI scale before reporting it, and GetScreenInfo reports the scale
// so CEF renders at the monitor's real resolution.
//
// Three of the four legs of that contract already converted. This is the fourth.
//
// MEASURED 2026-08-25 at 125% (see
// development-docs/0.4.0-beta.3/phase-1-overlay-input-dpi/MEASUREMENTS.md, M1):
// an overlay with a 500x1000 physical client reports a 400x800 view; a click at physical
// (200,400) reached the DOM as view (200,400) verbatim instead of (160,320). Consequences,
// both observed rather than predicted:
//   * the user aims at one control and a DIFFERENT control receives the event
//     (aimed BUTTON:Advanced, got the balance panel) — on a consent modal that is
//     "aims at Deny, activates Allow";
//   * everything below 1/scale of the overlay's height lands outside the view entirely
//     (elementFromPoint == NONE) — the bottom 20% is dead at 125%, 33% at 150%.
//
// ⛔ Every SendMouseClickEvent / SendMouseMoveEvent / SendMouseWheelEvent call site must go
//    through here. A raw GET_X_LPARAM reaching CefMouseEvent is the bug. Physical and
//    logical pixels are both `int`, so nothing but discipline distinguishes them — which is
//    why preflight carries a gate that fails when a raw coordinate appears at one of those
//    call sites (P1-A4). The gate is the durable fix; the arithmetic below is today's bug.
//
// ⚠️ Input must already be CLIENT-relative. WM_MOUSEWHEEL delivers SCREEN coordinates —
//    call ScreenToClient first. Two overlays shipped without that step.
//
// macOS needs no equivalent: its overlays are windowless against an NSView whose coordinates
// are already logical, and GetScreenPoint there adds no scale factor.

#ifdef _WIN32
#include <windows.h>

namespace hodos {

// Pure core: physical pixels -> view (logical) pixels at a given DPI.
//
// MulDiv rounds to nearest and rounds negatives away from zero, so a click is not
// systematically biased toward the overlay's top-left, and a negative client coordinate
// (possible from ScreenToClient when the pointer is outside the window during a drag)
// converts symmetrically. Deliberately free of HWND and CEF so it is unit-testable
// without a window or a browser (P1-A3).
inline void PhysicalToView(int physX, int physY, unsigned int dpi, int& viewX, int& viewY) {
    if (dpi == 0) {
        dpi = 96;  // fail safe: an unknown DPI is treated as 100%, i.e. no conversion
    }
    viewX = MulDiv(physX, 96, static_cast<int>(dpi));
    viewY = MulDiv(physY, 96, static_cast<int>(dpi));
}

// Negative-control lever. HODOS_OVERLAY_RAW_MOUSE=1 restores the pre-fix behaviour — the
// raw physical coordinate, unconverted — so the acceptance test can be *seen* to fail on
// the same binary that passes it. Without this the only red available is a different
// build, which leaves "did the binary change, or did the behaviour?" unanswered.
// ⛔ Test lever only. Never set in a shipped configuration.
inline bool RawMouseOverrideEnabled() {
    static int cached = -1;
    if (cached < 0) {
        char buf[8] = {0};
        DWORD n = GetEnvironmentVariableA("HODOS_OVERLAY_RAW_MOUSE", buf, sizeof(buf));
        cached = (n > 0 && buf[0] == '1') ? 1 : 0;
    }
    return cached == 1;
}

// WndProc convenience: resolve the window's current DPI and convert.
//
// The DPI is re-read per event rather than cached because an overlay can be moved to a
// monitor with a different scale between one event and the next, and a cached scale would
// reintroduce exactly this bug in the window between the move and the next WM_DPICHANGED.
inline void ClientToViewPoint(HWND hwnd, int clientX, int clientY, int& viewX, int& viewY) {
    if (RawMouseOverrideEnabled()) {
        viewX = clientX;  // pre-fix behaviour, for the negative control
        viewY = clientY;
        return;
    }
    PhysicalToView(clientX, clientY, hwnd ? GetDpiForWindow(hwnd) : 96, viewX, viewY);
}

}  // namespace hodos

#endif  // _WIN32
