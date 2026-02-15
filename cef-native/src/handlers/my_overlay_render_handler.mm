#include "../../include/handlers/my_overlay_render_handler.h"
#include <iostream>
#include <fstream>
#include "include/wrapper/cef_helpers.h"

#ifdef _WIN32
    #define _WIN32_WINNT 0x0601
    #include <windows.h>
    #include <dwmapi.h>
#elif defined(__APPLE__)
    #import <Cocoa/Cocoa.h>
    #import <QuartzCore/QuartzCore.h>
    #import <CoreGraphics/CoreGraphics.h>
#endif

// ============================================================================
// Constructor - Windows Implementation
// ============================================================================

#ifdef _WIN32

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

#elif defined(__APPLE__)

// ============================================================================
// Constructor - macOS Implementation
// ============================================================================

MyOverlayRenderHandler::MyOverlayRenderHandler(void* nsview, int width, int height)
    : nsview_(nsview), width_(width), height_(height) {

    std::ofstream debugLog("debug_output.log", std::ios::app);
    debugLog << "🎨 MyOverlayRenderHandler constructor called for NSView (macOS) - size: " << width_ << "x" << height_ << std::endl;
    debugLog.close();

    // macOS: CALayer setup handled in NSView subclass
    // The NSView already has a CALayer with setWantsLayer:YES
    std::cout << "✅ macOS render handler initialized for windowless rendering" << std::endl;
}

#endif

// ============================================================================
// Destructor - Platform-Specific Implementations
// ============================================================================

#ifdef _WIN32

MyOverlayRenderHandler::~MyOverlayRenderHandler() {
    // Clean up Windows GDI resources
    if (hbitmap_) {
        DeleteObject(hbitmap_);
        hbitmap_ = nullptr;
    }
    if (hdc_mem_) {
        DeleteDC(hdc_mem_);
        hdc_mem_ = nullptr;
    }
    std::cout << "🧹 MyOverlayRenderHandler destructor: Windows resources cleaned up" << std::endl;
}

#elif defined(__APPLE__)

MyOverlayRenderHandler::~MyOverlayRenderHandler() {
    // macOS: NSView and CALayer are managed by ARC, no manual cleanup needed
    std::cout << "🧹 MyOverlayRenderHandler destructor: macOS (no manual cleanup needed)" << std::endl;
}

#endif

void MyOverlayRenderHandler::GetViewRect(CefRefPtr<CefBrowser>, CefRect& rect) {
    rect = CefRect(0, 0, width_, height_);
    std::cout << "🔍 GetViewRect called: " << width_ << "x" << height_ << std::endl;
    std::ofstream debugLog("debug_output.log", std::ios::app);
    debugLog << "🔍 GetViewRect called: " << width_ << "x" << height_ << std::endl;
    debugLog.close();
}

// ============================================================================
// OnPaint - Platform-Specific Implementations
// ============================================================================

void MyOverlayRenderHandler::OnPaint(CefRefPtr<CefBrowser> browser,
                                     PaintElementType type,
                                     const RectList& dirtyRects,
                                     const void* buffer,
                                     int width, int height) {
    CEF_REQUIRE_UI_THREAD();

#ifdef _WIN32
    // ====== Windows Implementation ======

    std::cout << "🧪 OnPaint called (Windows) - type: " << type << " size: " << width << "x" << height << std::endl;
    std::ofstream debugLog("debug_output.log", std::ios::app);
    debugLog << "🧪 OnPaint called (Windows) - type: " << type << " size: " << width << "x" << height << std::endl;
    debugLog.close();

    bool isMostlyTransparent = true;
    const uint8_t* alpha = reinterpret_cast<const uint8_t*>(buffer);

    for (int i = 3; i < width * height * 4; i += 4) {
        if (alpha[i] > 20) {
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

    POINT* ptWinPos = nullptr;  // Use HWND's current position
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
    std::cout << "🖱️ Window handle: " << hwnd_ << std::endl;
    std::cout << "🖱️ Window enabled: " << IsWindowEnabled(hwnd_) << std::endl;
    std::cout << "🖱️ Window visible: " << IsWindowVisible(hwnd_) << std::endl;

#elif defined(__APPLE__)
    // ====== macOS Implementation ======

    std::cout << "🧪 OnPaint called (macOS) - type: " << type << " size: " << width << "x" << height << std::endl;
    std::ofstream debugLog("debug_output.log", std::ios::app);
    debugLog << "🧪 OnPaint called (macOS) - type: " << type << " size: " << width << "x" << height << std::endl;
    debugLog.close();

    if (!buffer || !nsview_) {
        std::cout << "❌ Invalid buffer or NSView pointer" << std::endl;
        return;
    }

    NSView* view = (__bridge NSView*)nsview_;
    CALayer* layer = [view layer];

    if (!layer) {
        std::cout << "❌ NSView does not have a CALayer" << std::endl;
        return;
    }

    // CRITICAL: Copy buffer immediately - CEF may reuse it causing ghosting
    size_t bufferSize = width * height * 4;
    void* bufferCopy = malloc(bufferSize);
    if (!bufferCopy) {
        std::cout << "❌ Failed to allocate buffer copy" << std::endl;
        return;
    }
    memcpy(bufferCopy, buffer, bufferSize);

    // Create CGImage from copied buffer with deallocation callback
    CGDataProviderRef provider = CGDataProviderCreateWithData(
        nullptr, bufferCopy, bufferSize,
        [](void* info, const void* data, size_t size) {
            free(const_cast<void*>(data));  // Free buffer when image is released
        });

    if (!provider) {
        std::cout << "❌ Failed to create CGDataProvider" << std::endl;
        free(bufferCopy);
        return;
    }

    CGColorSpaceRef colorSpace = CGColorSpaceCreateDeviceRGB();
    CGImageRef image = CGImageCreate(
        width, height,                              // Width, height
        8, 32,                                      // Bits per component, bits per pixel
        width * 4,                                  // Bytes per row
        colorSpace,                                 // Color space
        kCGImageAlphaPremultipliedFirst | kCGBitmapByteOrder32Little,  // BGRA format
        provider,                                   // Data provider
        nullptr,                                    // Decode array
        false,                                      // Should interpolate
        kCGRenderingIntentDefault);                 // Rendering intent

    CGColorSpaceRelease(colorSpace);
    CGDataProviderRelease(provider);

    if (!image) {
        std::cout << "❌ Failed to create CGImage" << std::endl;
        return;
    }

    // Update layer on main thread (CALayer is not thread-safe)
    dispatch_async(dispatch_get_main_queue(), ^{
        // CRITICAL: Disable implicit animations - prevents fade-in ghosting effect
        [CATransaction begin];
        [CATransaction setDisableActions:YES];

        layer.contents = (__bridge id)image;

        [CATransaction commit];
        CGImageRelease(image);
    });

#endif
}

bool MyOverlayRenderHandler::GetScreenPoint(CefRefPtr<CefBrowser> browser, int viewX, int viewY, int& screenX, int& screenY) {
#ifdef _WIN32
    RECT windowRect;
    GetWindowRect(hwnd_, &windowRect);

    screenX = windowRect.left + viewX;
    screenY = windowRect.top + viewY;
    return true;

#elif defined(__APPLE__)
    NSView* view = (__bridge NSView*)nsview_;
    NSWindow* window = [view window];

    if (!window) return false;

    NSRect windowFrame = [window frame];
    NSPoint screenPoint = windowFrame.origin;

    screenX = screenPoint.x + viewX;
    screenY = screenPoint.y + viewY;
    return true;

#endif
}

bool MyOverlayRenderHandler::GetScreenInfo(CefRefPtr<CefBrowser> browser, CefScreenInfo& screen_info) {
#ifdef _WIN32
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

#elif defined(__APPLE__)
    NSView* view = (__bridge NSView*)nsview_;
    NSWindow* window = [view window];

    if (!window) return false;

    NSRect windowFrame = [window frame];
    NSScreen* screen = [window screen];

    // Get backing scale factor (Retina display support)
    CGFloat scaleFactor = [screen backingScaleFactor];

    screen_info.device_scale_factor = scaleFactor;
    screen_info.depth = 32;
    screen_info.depth_per_component = 8;
    screen_info.is_monochrome = false;
    screen_info.rect = CefRect((int)windowFrame.origin.x,
                              (int)windowFrame.origin.y,
                              (int)windowFrame.size.width,
                              (int)windowFrame.size.height);
    screen_info.available_rect = screen_info.rect;

    return true;

#endif
}

void MyOverlayRenderHandler::OnPopupShow(CefRefPtr<CefBrowser> browser, bool show) {
    // Handle popup show/hide
}

void MyOverlayRenderHandler::OnPopupSize(CefRefPtr<CefBrowser> browser, const CefRect& rect) {
    // Handle popup size changes
}
