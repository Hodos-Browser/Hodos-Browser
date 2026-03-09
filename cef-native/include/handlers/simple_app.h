// include/core/simple_app.h
#pragma once

#include "include/cef_app.h"
#include "include/cef_browser.h"
#include "include/cef_render_process_handler.h"
#include "include/cef_browser_process_handler.h"
#include "simple_render_process_handler.h"
#include "simple_handler.h"

#ifdef _WIN32
    #include <windows.h>
#elif defined(__APPLE__)
    #ifdef __OBJC__
        #import <Cocoa/Cocoa.h>
    #else
        // Forward declarations for C++ files
        struct NSWindow;
        struct NSView;
    #endif
#endif

// Platform-specific global window/view references
#ifdef _WIN32
    // Windows: HWNDs
    extern HWND g_hwnd;
    extern HWND g_header_hwnd;
    extern HWND g_webview_hwnd;
    extern HWND g_settings_overlay_hwnd;
    extern HWND g_wallet_overlay_hwnd;
    extern bool g_wallet_overlay_prevent_close;
    extern HWND g_backup_overlay_hwnd;
    extern HWND g_brc100_auth_overlay_hwnd;
    extern HWND g_settings_menu_overlay_hwnd;
    extern HWND g_omnibox_overlay_hwnd;
    extern HWND g_notification_overlay_hwnd;
    extern HINSTANCE g_hInstance;

    // Windows overlay creation functions
    void CreateSettingsOverlayWithSeparateProcess(HINSTANCE hInstance, int iconRightOffset = 0);
    void CreateBRC100AuthOverlayWithSeparateProcess(HINSTANCE hInstance);
    void CreateNotificationOverlay(HINSTANCE hInstance, const std::string& type, const std::string& domain, const std::string& extraParams = "");
    void CreateSettingsMenuOverlay(HINSTANCE hInstance);
    void CreateOmniboxOverlay(HINSTANCE hInstance, bool showImmediately = true);
    void ShowOmniboxOverlay(BrowserWindow* targetWin = nullptr);
    void HideOmniboxOverlay();

#elif defined(__APPLE__)
    // macOS: NSWindow* and NSView* (forward declared as void*)
    extern NSWindow* g_main_window;
    extern NSView* g_header_view;
    extern NSView* g_webview_view;
    extern NSWindow* g_settings_overlay_window;
    extern NSWindow* g_wallet_overlay_window;
    extern NSWindow* g_backup_overlay_window;
    extern NSWindow* g_brc100_auth_overlay_window;
    extern NSWindow* g_notification_overlay_window;
    extern NSWindow* g_settings_menu_overlay_window;

    // macOS overlay creation functions
    void CreateSettingsOverlayWithSeparateProcess(int iconRightOffset = 0);
    void CreateWalletOverlayWithSeparateProcess(int iconRightOffset = 0);
    void CreateBackupOverlayWithSeparateProcess();
    void CreateBRC100AuthOverlayWithSeparateProcess();
    void CreateNotificationOverlay(const std::string& type, const std::string& domain, const std::string& extraParams = "");
    void CreateSettingsMenuOverlay();

    // Helper function to get NSView dimensions (implemented in cef_browser_shell_mac.mm)
    struct ViewDimensions {
        int width;
        int height;
    };
    ViewDimensions GetViewDimensions(void* nsview);
#endif


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

    // Platform-specific window handle methods
#ifdef _WIN32
    void SetWindowHandles(HWND hwnd, HWND shell, HWND webview);
    HWND hwnd_ = nullptr;
    HWND header_hwnd_ = nullptr;
    HWND webview_hwnd_ = nullptr;
#elif defined(__APPLE__)
    void SetMacOSWindow(void* main_window, void* header_view, void* webview_view);
    void* main_window_ = nullptr;
    void* header_view_ = nullptr;
    void* webview_view_ = nullptr;
#endif


private:
    CefRefPtr<SimpleRenderProcessHandler> render_process_handler_;

    IMPLEMENT_REFCOUNTING(SimpleApp);
};
