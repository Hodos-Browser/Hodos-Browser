// cef_native/src/simple_handler.h
#pragma once

#include "include/cef_client.h"
#include "include/cef_display_handler.h"
#include "include/cef_life_span_handler.h"
#include "include/cef_load_handler.h"
#include "include/cef_request_handler.h"
#include "include/cef_resource_request_handler.h"
#include "include/cef_context_menu_handler.h"
#include "include/cef_dialog_handler.h"
#include "include/cef_keyboard_handler.h"
#include <set>

// Forward declarations to avoid circular dependency
struct Tab;
class TabManager;

class SimpleHandler : public CefClient,
                      public CefLifeSpanHandler,
                      public CefDisplayHandler,
                      public CefLoadHandler,
                      public CefRequestHandler,
                      public CefContextMenuHandler,
                      public CefDialogHandler,
                      public CefKeyboardHandler {
public:
    explicit SimpleHandler(const std::string& role);

    // CefClient methods
    CefRefPtr<CefLifeSpanHandler> GetLifeSpanHandler() override;
    CefRefPtr<CefDisplayHandler> GetDisplayHandler() override;
    CefRefPtr<CefLoadHandler> GetLoadHandler() override;
    CefRefPtr<CefRequestHandler> GetRequestHandler() override;
    CefRefPtr<CefContextMenuHandler> GetContextMenuHandler() override;
    CefRefPtr<CefDialogHandler> GetDialogHandler() override;
    CefRefPtr<CefKeyboardHandler> GetKeyboardHandler() override;
    static CefRefPtr<CefBrowser> webview_browser_;
    static CefRefPtr<CefBrowser> header_browser_;
    static CefRefPtr<CefBrowser> wallet_panel_browser_;
    static CefRefPtr<CefBrowser> GetOverlayBrowser();
    static CefRefPtr<CefBrowser> GetHeaderBrowser();
    static CefRefPtr<CefBrowser> GetWebviewBrowser();
    static CefRefPtr<CefBrowser> GetWalletPanelBrowser();
    static CefRefPtr<CefBrowser> GetSettingsBrowser();
    static CefRefPtr<CefBrowser> GetWalletBrowser();
    static CefRefPtr<CefBrowser> GetBackupBrowser();
    static CefRefPtr<CefBrowser> GetBRC100AuthBrowser();
    static CefRefPtr<CefBrowser> GetNotificationBrowser();
    static CefRefPtr<CefBrowser> GetSettingsMenuBrowser();
    static CefRefPtr<CefBrowser> GetOmniboxBrowser();
    static CefRefPtr<CefBrowser> GetCookiePanelBrowser();
    static std::string pending_panel_;
    static bool needs_overlay_reload_;
    static void TriggerDeferredPanel(const std::string& panel);
    static void NotifyTabListChanged();  // Notify frontend of tab list changes

    // CefDisplayHandler methods
    void OnTitleChange(CefRefPtr<CefBrowser> browser, const CefString& title) override;

    void OnAddressChange(CefRefPtr<CefBrowser> browser,
                        CefRefPtr<CefFrame> frame,
                        const CefString& url) override;

    void OnFaviconURLChange(CefRefPtr<CefBrowser> browser,
                          const std::vector<CefString>& icon_urls) override;

    void OnFullscreenModeChange(CefRefPtr<CefBrowser> browser,
                                bool fullscreen) override;

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

    void OnBeforeClose(CefRefPtr<CefBrowser> browser) override;

    bool OnBeforePopup(CefRefPtr<CefBrowser> browser,
                      CefRefPtr<CefFrame> frame,
                      int popup_id,
                      const CefString& target_url,
                      const CefString& target_frame_name,
                      CefLifeSpanHandler::WindowOpenDisposition target_disposition,
                      bool user_gesture,
                      const CefPopupFeatures& popupFeatures,
                      CefWindowInfo& windowInfo,
                      CefRefPtr<CefClient>& client,
                      CefBrowserSettings& settings,
                      CefRefPtr<CefDictionaryValue>& extra_info,
                      bool* no_javascript_access) override;

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

    // CefRequestHandler - SSL certificate error handling
    bool OnCertificateError(CefRefPtr<CefBrowser> browser,
                            cef_errorcode_t cert_error,
                            const CefString& request_url,
                            CefRefPtr<CefSSLInfo> ssl_info,
                            CefRefPtr<CefCallback> callback) override;

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

    // CefDialogHandler methods
    bool OnFileDialog(CefRefPtr<CefBrowser> browser,
                      FileDialogMode mode,
                      const CefString& title,
                      const CefString& default_file_path,
                      const std::vector<CefString>& accept_filters,
                      const std::vector<CefString>& accept_extensions,
                      const std::vector<CefString>& accept_descriptions,
                      CefRefPtr<CefFileDialogCallback> callback) override;

    // CefKeyboardHandler methods
    bool OnPreKeyEvent(CefRefPtr<CefBrowser> browser,
                       const CefKeyEvent& event,
                       CefEventHandle os_event,
                       bool* is_keyboard_shortcut) override;

private:
    std::string role_;

    /**
     * @brief Show DevTools for browser or focus if already open
     * @param browser The browser instance to open DevTools for
     */
    void ShowOrFocusDevTools(CefRefPtr<CefBrowser> browser);
    CefRefPtr<CefRenderHandler> render_handler_;
    static CefRefPtr<CefBrowser> overlay_browser_;
    static CefRefPtr<CefBrowser> settings_browser_;
    static CefRefPtr<CefBrowser> wallet_browser_;
    static CefRefPtr<CefBrowser> backup_browser_;
    static CefRefPtr<CefBrowser> brc100_auth_browser_;
    static CefRefPtr<CefBrowser> notification_browser_;
    static CefRefPtr<CefBrowser> settings_menu_browser_;
    static CefRefPtr<CefBrowser> omnibox_browser_;
    static CefRefPtr<CefBrowser> cookie_panel_browser_;

    /**
     * @brief Extract tab ID from role string (format: "tab_1", "tab_2", etc.)
     * @return Tab ID, or -1 if not a tab role
     */
    static int ExtractTabIdFromRole(const std::string& role);

    // Domains user has chosen to proceed past cert errors (session-only)
    static std::set<std::string> allowed_cert_exceptions_;

    IMPLEMENT_REFCOUNTING(SimpleHandler);
};
