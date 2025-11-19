#define _WIN32_WINNT 0x0601

#include "../../include/handlers/my_overlay_render_handler.h"
#include <windows.h>
#include <dwmapi.h>
#include <iostream>
#include <fstream>
#include "include/wrapper/cef_helpers.h"  // ✅ required for CEF_REQUIRE_UI_THREAD()

MyOverlayRenderHandler::MyOverlayRenderHandler(HWND hwnd, int width, int height)
    : hwnd_(hwnd), width_(width), height_(height),
      hdc_mem_(nullptr), hbitmap_(nullptr), dib_data_(nullptr) {

    std::ofstream debugLog("debug_output.log", std::ios::app);
    debugLog << "🎨 MyOverlayRenderHandler constructor called for HWND: " << hwnd_ << " size: " << width_ << "x" << height_ << std::endl;
    debugLog.close();

    // Confirm DWM composition
    BOOL dwmEnabled = FALSE;
    if (SUCCEEDED(DwmIsCompositionEnabled(&dwmEnabled))) {
        std::cout << "→ DWM composition enabled: " << (dwmEnabled ? "true" : "false") << std::endl;
    }

    // Create memory DC
    HDC screenDC = GetDC(NULL);
    hdc_mem_ = CreateCompatibleDC(screenDC);
    ReleaseDC(NULL, screenDC);

    // Create simple RGB bitmap (no alpha masks)
    BITMAPINFO bmi = {};
    bmi.bmiHeader.biSize = sizeof(BITMAPINFOHEADER);
    bmi.bmiHeader.biWidth = width_;
    bmi.bmiHeader.biHeight = -height_; // top-down
    bmi.bmiHeader.biPlanes = 1;
    bmi.bmiHeader.biBitCount = 32;
    bmi.bmiHeader.biCompression = BI_RGB;

    hbitmap_ = CreateDIBSection(hdc_mem_, &bmi, DIB_RGB_COLORS, &dib_data_, nullptr, 0);
    if (!hbitmap_ || !dib_data_) {
        std::cout << "❌ CreateDIBSection failed." << std::endl;
        return;
    }

    // Select bitmap into DC
    if (!SelectObject(hdc_mem_, hbitmap_)) {
        std::cout << "❌ SelectObject failed." << std::endl;
    }

    // Prime layered HWND with dummy pixel for early hit-test
    // CRITICAL: Use nullptr for position so it respects SetWindowPos
    UpdateLayeredWindow(hwnd_, hdc_mem_, nullptr, nullptr, hdc_mem_, nullptr, 0, nullptr, ULW_ALPHA);
    DWORD err = GetLastError();
    std::cout << "🧪 Dummy layered update error: " << err << " (0 = success)" << std::endl;

    // Log bitmap info
    BITMAP bmp = {};
    GetObject(hbitmap_, sizeof(BITMAP), &bmp);
    std::cout << "→ Bitmap stride: " << bmp.bmWidthBytes << " bytes\n";
    std::cout << "→ Bitmap size: " << bmp.bmWidth << " x " << bmp.bmHeight << std::endl;
    std::cout << "→ bmBitsPixel: " << bmp.bmBitsPixel << ", bmPlanes: " << bmp.bmPlanes
              << ", bmType: " << bmp.bmType << std::endl;
}

void MyOverlayRenderHandler::GetViewRect(CefRefPtr<CefBrowser>, CefRect& rect) {
    rect = CefRect(0, 0, width_, height_);
}

void MyOverlayRenderHandler::OnPaint(CefRefPtr<CefBrowser> browser,
                                     PaintElementType type,
                                     const RectList& dirtyRects,
                                     const void* buffer,
                                     int width, int height) {
    CEF_REQUIRE_UI_THREAD();  // ✅ Confirm we're on the UI thread

    std::cout << "🧪 OnPaint called for backup overlay - type: " << type << " size: " << width << "x" << height << std::endl;
    std::ofstream debugLog("debug_output.log", std::ios::app);
    debugLog << "🧪 OnPaint called for backup overlay - type: " << type << " size: " << width << "x" << height << std::endl;
    debugLog.close();

    bool isMostlyTransparent = true;
    const uint8_t* alpha = reinterpret_cast<const uint8_t*>(buffer);

    for (int i = 3; i < width * height * 4; i += 4) {
        if (alpha[i] > 20) {  // found a visible pixel
            isMostlyTransparent = false;
            break;
        }
    }

    if (buffer && dib_data_) {
        std::memcpy(dib_data_, buffer, width * height * 4);
    }

    RECT hwndRect;
    GetWindowRect(hwnd_, &hwndRect);
    std::cout << "→ HWND real size: " << (hwndRect.right - hwndRect.left)
              << " x " << (hwndRect.bottom - hwndRect.top) << std::endl;

    // CRITICAL FIX: Use nullptr for ptWinPos to respect HWND position set by SetWindowPos
    // If we pass {0,0}, it will ALWAYS render at screen position (0,0)!
    POINT* ptWinPos = nullptr;  // nullptr = use HWND's current position
    SIZE sizeWin = {width_, height_};
    POINT ptSrc = {0, 0};

    BLENDFUNCTION blend = {};
    blend.BlendOp = AC_SRC_OVER;
    blend.SourceConstantAlpha = 255;
    blend.AlphaFormat = AC_SRC_ALPHA;

    LONG exStyle = GetWindowLong(hwnd_, GWL_EXSTYLE);
    std::cout << "→ HWND EXSTYLE: 0x" << std::hex << exStyle << std::endl;
    std::cout << "→ Has WS_EX_LAYERED: " << ((exStyle & WS_EX_LAYERED) != 0) << std::endl;

    HDC screenDC = GetDC(NULL);

    BOOL result = UpdateLayeredWindow(hwnd_, screenDC, ptWinPos, &sizeWin,
        hdc_mem_, &ptSrc, 0, &blend, ULW_ALPHA);

    if (result) {
        // Ensure window can receive input (but don't steal focus on every paint)
        LONG exStyle = GetWindowLong(hwnd_, GWL_EXSTYLE);
        if (exStyle & WS_EX_TRANSPARENT) {
            SetWindowLong(hwnd_, GWL_EXSTYLE, exStyle & ~WS_EX_TRANSPARENT);
            std::cout << "🖱️ Removed WS_EX_TRANSPARENT for input handling" << std::endl;
        }
    }

    ReleaseDC(NULL, screenDC);

    DWORD err = GetLastError();
    std::cout << "→ UpdateLayeredWindow result: " << (result ? "success" : "fail")
            << ", error: " << err << std::endl;
    std::cout << "→ IsWindowVisible(hwnd_): " << IsWindowVisible(hwnd_) << std::endl;

    // Add the diagnostic logs here:
    std::cout << "🖱️ Window handle: " << hwnd_ << std::endl;
    std::cout << "🖱️ Window enabled: " << IsWindowEnabled(hwnd_) << std::endl;
    std::cout << "🖱️ Window visible: " << IsWindowVisible(hwnd_) << std::endl;
}

bool MyOverlayRenderHandler::GetScreenPoint(CefRefPtr<CefBrowser> browser, int viewX, int viewY, int& screenX, int& screenY) {
    RECT windowRect;
    GetWindowRect(hwnd_, &windowRect);

    screenX = windowRect.left + viewX;
    screenY = windowRect.top + viewY;
    return true;
}

bool MyOverlayRenderHandler::GetScreenInfo(CefRefPtr<CefBrowser> browser, CefScreenInfo& screen_info) {
    RECT windowRect;
    GetWindowRect(hwnd_, &windowRect);

    screen_info.device_scale_factor = 1.0f;
    screen_info.depth = 32;
    screen_info.depth_per_component = 8;
    screen_info.is_monochrome = false;
    screen_info.rect = CefRect(windowRect.left, windowRect.top,
                              windowRect.right - windowRect.left,
                              windowRect.bottom - windowRect.top);
    screen_info.available_rect = screen_info.rect;

    return true;
}

void MyOverlayRenderHandler::OnPopupShow(CefRefPtr<CefBrowser> browser, bool show) {
    // Handle popup show/hide
}

void MyOverlayRenderHandler::OnPopupSize(CefRefPtr<CefBrowser> browser, const CefRect& rect) {
    // Handle popup size changes
}
