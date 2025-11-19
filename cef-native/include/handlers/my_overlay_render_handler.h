#pragma once
#include "include/cef_render_handler.h"
#include "simple_app.h"

class MyOverlayRenderHandler : public CefRenderHandler {
public:
    MyOverlayRenderHandler(HWND hwnd, int width, int height);

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
    HWND hwnd_;      // ✅ store HWND
    int width_;
    int height_;

    HDC hdc_mem_;        // Our reusable memory DC
    HBITMAP hbitmap_;    // The bitmap CEF will draw into
    void* dib_data_;     // Pointer to the raw bitmap memory

    IMPLEMENT_REFCOUNTING(MyOverlayRenderHandler);
};
