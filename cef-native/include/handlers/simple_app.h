// include/core/simple_app.h
#pragma once

#include "include/cef_app.h"
#include "include/cef_browser.h"
#include "include/cef_render_process_handler.h"
#include "include/cef_browser_process_handler.h"
#include "simple_render_process_handler.h"
#include "simple_handler.h"

// 🧭 Temporary global HWNDs for startup wiring
extern HWND g_hwnd;
extern HWND g_header_hwnd;
extern HWND g_webview_hwnd;

// Global overlay HWNDs for shutdown cleanup
extern HWND g_settings_overlay_hwnd;
extern HWND g_wallet_overlay_hwnd;
extern HWND g_backup_overlay_hwnd;
extern HWND g_brc100_auth_overlay_hwnd;

// globals.h
extern HINSTANCE g_hInstance;

// Global functions
void CreateSettingsOverlayWithSeparateProcess(HINSTANCE hInstance);
void CreateBRC100AuthOverlayWithSeparateProcess(HINSTANCE hInstance);


class SimpleApp : public CefApp,
                  public CefBrowserProcessHandler,
                  public CefRenderProcessHandler {
public:
    SimpleApp();

    CefRefPtr<CefBrowserProcessHandler> GetBrowserProcessHandler() override;
    CefRefPtr<CefRenderProcessHandler> GetRenderProcessHandler() override;

    void OnBeforeCommandLineProcessing(const CefString& process_type,
                                       CefRefPtr<CefCommandLine> command_line) override;

    void OnContextInitialized() override;

    void SetWindowHandles(HWND hwnd, HWND shell, HWND webview);
    HWND hwnd_ = nullptr;
    HWND header_hwnd_ = nullptr;
    HWND webview_hwnd_ = nullptr;
    // HWND overlay_hwnd_ = nullptr;


private:
    CefRefPtr<SimpleRenderProcessHandler> render_process_handler_;

    IMPLEMENT_REFCOUNTING(SimpleApp);
};
