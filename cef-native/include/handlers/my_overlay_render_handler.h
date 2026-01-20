#pragma once
#include "include/cef_render_handler.h"
#include "simple_app.h"

#ifdef _WIN32
    #include <windows.h>
#endif

class MyOverlayRenderHandler : public CefRenderHandler {
public:
    // Platform-agnostic constructor (void* for HWND or NSView*)
#ifdef _WIN32
    MyOverlayRenderHandler(HWND hwnd, int width, int height);
#elif defined(__APPLE__)
    MyOverlayRenderHandler(void* nsview, int width, int height);
#endif

    // Destructor for proper resource cleanup
    ~MyOverlayRenderHandler();

    void GetViewRect(CefRefPtr<CefBrowser> browser, CefRect& rect) override;
    void OnPaint(CefRefPtr<CefBrowser> browser,
                 PaintElementType type,
                 const RectList& dirtyRects,
                 const void* buffer,
                 int width, int height) override;

    // Mouse event handling
    bool GetScreenPoint(CefRefPtr<CefBrowser> browser, int viewX, int viewY, int& screenX, int& screenY) override;
    bool GetScreenInfo(CefRefPtr<CefBrowser> browser, CefScreenInfo& screen_info) override;
    void OnPopupShow(CefRefPtr<CefBrowser> browser, bool show) override;
    void OnPopupSize(CefRefPtr<CefBrowser> browser, const CefRect& rect) override;

private:
    int width_;
    int height_;

#ifdef _WIN32
    HWND hwnd_;           // Windows window handle
    HDC hdc_mem_;         // Memory DC for GDI rendering
    HBITMAP hbitmap_;     // Bitmap for DIB section
    void* dib_data_;      // Pointer to raw bitmap memory
#elif defined(__APPLE__)
    void* nsview_;        // macOS NSView pointer (bridged)
#endif

    IMPLEMENT_REFCOUNTING(MyOverlayRenderHandler);
};
