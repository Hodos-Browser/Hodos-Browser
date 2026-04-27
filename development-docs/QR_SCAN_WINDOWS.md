# QR Code Scanning — Windows Implementation — COMPLETE

> Platform-specific details for Windows. See [QR_SCAN_OVERVIEW.md](./QR_SCAN_OVERVIEW.md) for architecture and Phase 1 (DOM scanning, cross-platform).

## Status: COMPLETE (2026-04-27)

Both Phase 1 (DOM scan) and Phase 2 (screen capture) are implemented and tested on Windows.

## Phase 1: DOM Scanning (No Windows-Specific Work)

Phase 1 is pure JavaScript + C++ IPC. All work is cross-platform. See overview doc.

The fingerprint farbling concern was a non-issue — jsQR's error correction handles LSB perturbation from our canvas farbling (QR codes are high-contrast black/white).

## Phase 2: Screen Region Capture — COMPLETE

### Overview

When DOM scan finds zero results, screen capture auto-triggers (no extra button click):
1. Close wallet overlay
2. Show full-screen transparent overlay with crosshair cursor
3. User drags to select region
4. Capture pixels from screen within that region
5. Decode QR from captured pixels
6. Reopen wallet overlay with results

### Existing Patterns to Follow

The codebase already has a very similar pattern: **ghost tab window** for tab tear-off (`HodosGhostTab` in `cef_browser_shell.cpp`). This is a `WS_POPUP | WS_EX_LAYERED | WS_EX_TOPMOST` window with GDI painting and mouse tracking. The QR selection overlay follows the same approach.

### Implementation Plan

#### 1. Selection Overlay Window

Create a new function in `cef_browser_shell.cpp`:

```cpp
// Globals
static HWND g_qr_selection_hwnd = nullptr;
static POINT g_qr_drag_start = {0, 0};
static POINT g_qr_drag_current = {0, 0};
static bool g_qr_is_dragging = false;

void CreateQRSelectionOverlay();
void DestroyQRSelectionOverlay();
LRESULT CALLBACK QRSelectionWndProc(HWND hwnd, UINT msg, WPARAM wParam, LPARAM lParam);
```

Window properties:
- `WS_POPUP | WS_VISIBLE` with `WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TOOLWINDOW`
- Covers entire virtual screen (`GetSystemMetrics(SM_XVIRTUALSCREEN)`, etc.)
- Semi-transparent dark overlay (alpha ~128) with clear rectangle where user is dragging
- Custom crosshair cursor (`SetCursor(LoadCursor(NULL, IDC_CROSS))`)

#### 2. Mouse Handling in QRSelectionWndProc

```
WM_LBUTTONDOWN  -> Record drag start point, set g_qr_is_dragging = true
WM_MOUSEMOVE    -> Update g_qr_drag_current, repaint selection rectangle
WM_LBUTTONUP    -> Capture selection, destroy overlay, decode
WM_RBUTTONDOWN  -> Cancel (destroy overlay, reopen wallet with no results)
WM_KEYDOWN(ESC) -> Cancel
```

#### 3. Screen Capture (BitBlt)

On `WM_LBUTTONUP`, capture the selected region:

```cpp
void CaptureRegion(RECT selection) {
    // Get screen DC
    HDC hdcScreen = GetDC(NULL);
    HDC hdcMem = CreateCompatibleDC(hdcScreen);

    int width = selection.right - selection.left;
    int height = selection.bottom - selection.top;

    // Create bitmap
    HBITMAP hBitmap = CreateCompatibleBitmap(hdcScreen, width, height);
    SelectObject(hdcMem, hBitmap);

    // Copy screen pixels
    BitBlt(hdcMem, 0, 0, width, height,
           hdcScreen, selection.left, selection.top, SRCCOPY);

    // Extract pixel data (BGRA)
    BITMAPINFO bmi = {};
    bmi.bmiHeader.biSize = sizeof(BITMAPINFOHEADER);
    bmi.bmiHeader.biWidth = width;
    bmi.bmiHeader.biHeight = -height; // Top-down
    bmi.bmiHeader.biPlanes = 1;
    bmi.bmiHeader.biBitCount = 32;
    bmi.bmiHeader.biCompression = BI_RGB;

    std::vector<uint8_t> pixels(width * height * 4);
    GetDIBits(hdcMem, hBitmap, 0, height, pixels.data(), &bmi, DIB_RGB_COLORS);

    // Cleanup
    DeleteObject(hBitmap);
    DeleteDC(hdcMem);
    ReleaseDC(NULL, hdcScreen);

    // Decode QR from pixels (BGRA -> pass to decoder)
    DecodeQRFromPixels(pixels.data(), width, height);
}
```

#### 4. QR Decoding from Pixel Buffer

Two options:

**Option A: JavaScript decoder via hidden CEF browser**
- Create a tiny offscreen CEF browser
- Pass pixel data as base64 via IPC
- Decode using jsQR in JavaScript
- Return result via IPC
- Pro: reuse same jsQR library as DOM scan
- Con: overhead of CEF browser creation

**Option B: C++ QR library**
- Use `quirc` (tiny C library, ~30KB, public domain) or `zxing-cpp`
- Link directly, call from C++
- Pro: fast, no CEF overhead
- Con: additional dependency

**Recommendation**: Option B with `quirc`. It's 2 files (`quirc.c` + `quirc.h`), public domain, and handles the pixel buffer directly. No need to spin up a CEF browser for a 10ms decode.

#### 5. Result Flow

After decoding:
1. Destroy selection overlay
2. Reopen wallet overlay with URL params: `?qr_recipient={address}&qr_amount={amount}`
3. Wallet React code reads params and populates TransactionForm

### Painting the Selection Rectangle

Use `UpdateLayeredWindow` with a GDI-painted surface:

```cpp
case WM_PAINT: {
    // Fill entire overlay with semi-transparent black
    // Draw clear (or highlighted) rectangle for selection area
    // Draw crosshair at cursor position
    // Optional: draw instruction text "Drag to select QR code"
}
```

Follow the `HodosGhostTab` pattern which already does GDI painting with `UpdateLayeredWindow`.

### Key Files to Modify

| File | Change |
|------|--------|
| `cef_browser_shell.cpp` | Add `CreateQRSelectionOverlay()`, `QRSelectionWndProc`, globals |
| `simple_handler.cpp` | Handle `qr_screen_capture_start` IPC: close wallet, create selection overlay |
| `simple_handler.cpp` | Handle capture result: reopen wallet with QR data params |
| New: `quirc.c` + `quirc.h` | QR decoder library (or link zxing-cpp) |
| `CMakeLists.txt` | Add quirc source files |

### DPI Awareness

The selection coordinates must account for DPI scaling:
- Use `GetDpiForWindow()` or `GetDeviceCaps(hdcScreen, LOGPIXELSX)`
- `BitBlt` works in physical pixels, but mouse coordinates from `WM_MOUSEMOVE` may be in logical pixels
- The existing codebase handles DPI in several places — follow the same pattern

### Multi-Monitor

`GetSystemMetrics(SM_XVIRTUALSCREEN/SM_YVIRTUALSCREEN/SM_CXVIRTUALSCREEN/SM_CYVIRTUALSCREEN)` covers the full virtual desktop across all monitors. The overlay should span the entire virtual screen.
