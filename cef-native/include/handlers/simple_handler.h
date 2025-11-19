// cef_native/src/simple_handler.h
#pragma once

#include "include/cef_client.h"
#include "include/cef_display_handler.h"
#include "include/cef_life_span_handler.h"
#include "include/cef_load_handler.h"
#include "include/cef_request_handler.h"
#include "include/cef_resource_request_handler.h"
#include "include/cef_context_menu_handler.h"
#include "include/cef_keyboard_handler.h"

class SimpleHandler : public CefClient,
                      public CefLifeSpanHandler,
                      public CefDisplayHandler,
                      public CefLoadHandler,
                      public CefRequestHandler,
                      public CefContextMenuHandler,
                      public CefKeyboardHandler {
public:
    explicit SimpleHandler(const std::string& role);

    // CefClient methods
    CefRefPtr<CefLifeSpanHandler> GetLifeSpanHandler() override;
    CefRefPtr<CefDisplayHandler> GetDisplayHandler() override;
    CefRefPtr<CefLoadHandler> GetLoadHandler() override;
    CefRefPtr<CefRequestHandler> GetRequestHandler() override;
    CefRefPtr<CefContextMenuHandler> GetContextMenuHandler() override;
    CefRefPtr<CefKeyboardHandler> GetKeyboardHandler() override;
    static CefRefPtr<CefBrowser> webview_browser_;
    static CefRefPtr<CefBrowser> header_browser_;
    static CefRefPtr<CefBrowser> GetOverlayBrowser();
    static CefRefPtr<CefBrowser> GetHeaderBrowser();
    static CefRefPtr<CefBrowser> GetWebviewBrowser();
    static CefRefPtr<CefBrowser> GetSettingsBrowser();
    static CefRefPtr<CefBrowser> GetWalletBrowser();
    static CefRefPtr<CefBrowser> GetBackupBrowser();
    static CefRefPtr<CefBrowser> GetBRC100AuthBrowser();
    static std::string pending_panel_;
    static bool needs_overlay_reload_;
    static void TriggerDeferredPanel(const std::string& panel);

    // CefDisplayHandler methods
    void OnTitleChange(CefRefPtr<CefBrowser> browser, const CefString& title) override;

    // CefLoadHandler methods
    void OnLoadError(CefRefPtr<CefBrowser> browser,
                     CefRefPtr<CefFrame> frame,
                     ErrorCode errorCode,
                     const CefString& errorText,
                     const CefString& failedUrl) override;

    void OnLoadingStateChange(CefRefPtr<CefBrowser> browser,
                               bool isLoading,
                               bool canGoBack,
                               bool canGoForward) override;

    void OnAfterCreated(CefRefPtr<CefBrowser> browser) override;

    bool OnProcessMessageReceived(CefRefPtr<CefBrowser> browser,
                              CefRefPtr<CefFrame> frame,
                              CefProcessId source_process,
                              CefRefPtr<CefProcessMessage> message) override;

    // HTTP Request Interception
    CefRefPtr<CefResourceRequestHandler> GetResourceRequestHandler(
        CefRefPtr<CefBrowser> browser,
        CefRefPtr<CefFrame> frame,
        CefRefPtr<CefRequest> request,
        bool is_navigation,
        bool is_download,
        const CefString& request_initiator,
        bool& disable_default_handling) override;

    void SetRenderHandler(CefRefPtr<CefRenderHandler> handler);
    CefRefPtr<CefRenderHandler> GetRenderHandler() override;

    // CefContextMenuHandler methods
    void OnBeforeContextMenu(CefRefPtr<CefBrowser> browser,
                            CefRefPtr<CefFrame> frame,
                            CefRefPtr<CefContextMenuParams> params,
                            CefRefPtr<CefMenuModel> model) override;

    bool OnContextMenuCommand(CefRefPtr<CefBrowser> browser,
                             CefRefPtr<CefFrame> frame,
                             CefRefPtr<CefContextMenuParams> params,
                             int command_id,
                             EventFlags event_flags) override;

    // CefKeyboardHandler methods
    bool OnPreKeyEvent(CefRefPtr<CefBrowser> browser,
                       const CefKeyEvent& event,
                       CefEventHandle os_event,
                       bool* is_keyboard_shortcut) override;

private:
    std::string role_;
    CefRefPtr<CefRenderHandler> render_handler_;
    static CefRefPtr<CefBrowser> overlay_browser_;
    static CefRefPtr<CefBrowser> settings_browser_;
    static CefRefPtr<CefBrowser> wallet_browser_;
    static CefRefPtr<CefBrowser> backup_browser_;
    static CefRefPtr<CefBrowser> brc100_auth_browser_;
    IMPLEMENT_REFCOUNTING(SimpleHandler);
};
