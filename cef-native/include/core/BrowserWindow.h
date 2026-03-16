#pragma once

#include <string>
#include <mutex>
#include "include/cef_browser.h"

#ifdef _WIN32
#ifndef WIN32_LEAN_AND_MEAN
#define WIN32_LEAN_AND_MEAN
#endif
#ifndef NOMINMAX
#define NOMINMAX
#endif
#include <windows.h>
#endif

/**
 * @brief Per-window state: HWNDs, overlay handles, mouse hooks, icon offsets,
 *        and CEF browser refs.  One instance per top-level browser window.
 *
 * Replaces the 17 global HWNDs, 6 mouse hooks, 6 icon offsets, and 15 static
 * CefRefPtr<CefBrowser> members that were previously spread across
 * cef_browser_shell.cpp and SimpleHandler.
 */
class BrowserWindow {
public:
    explicit BrowserWindow(int id);
    ~BrowserWindow() = default;

    // Unique identifier for this window
    int window_id;

    // ---- Platform window handles ----
#ifdef _WIN32
    HWND hwnd = nullptr;              // Main shell window (WS_OVERLAPPEDWINDOW)
    HWND header_hwnd = nullptr;       // Header/toolbar child window
    HWND webview_hwnd = nullptr;      // Legacy (unused, kept for compat)

    // Overlay HWNDs (11 total)
    HWND settings_overlay_hwnd = nullptr;
    HWND wallet_overlay_hwnd = nullptr;
    HWND backup_overlay_hwnd = nullptr;
    HWND brc100_auth_overlay_hwnd = nullptr;
    HWND notification_overlay_hwnd = nullptr;
    HWND settings_menu_overlay_hwnd = nullptr;
    HWND omnibox_overlay_hwnd = nullptr;
    HWND cookie_panel_overlay_hwnd = nullptr;
    HWND download_panel_overlay_hwnd = nullptr;
    HWND profile_panel_overlay_hwnd = nullptr;
    HWND menu_overlay_hwnd = nullptr;

    // Mouse hooks for overlay click-outside detection (6 total)
    HHOOK omnibox_mouse_hook = nullptr;
    HHOOK cookie_panel_mouse_hook = nullptr;
    HHOOK download_panel_mouse_hook = nullptr;
    HHOOK profile_panel_mouse_hook = nullptr;
    HHOOK settings_mouse_hook = nullptr;
    HHOOK menu_mouse_hook = nullptr;

    // Icon offsets for right-side panel positioning (physical pixel distance
    // from icon's right edge to header's right edge)
    int settings_icon_right_offset = 0;
    int cookie_icon_right_offset = 0;
    int download_icon_right_offset = 0;
    int profile_icon_right_offset = 0;
    int wallet_icon_right_offset = 0;
    int menu_icon_right_offset = 0;

#elif defined(__APPLE__)
    void* ns_window = nullptr;        // NSWindow*
    void* header_view = nullptr;      // NSView*
    void* webview_view = nullptr;     // NSView* (container for tab NSViews)

    // Overlay NSWindows (11 total — mirrors Windows overlay HWNDs)
    void* settings_overlay_window = nullptr;
    void* wallet_overlay_window = nullptr;
    void* backup_overlay_window = nullptr;
    void* brc100_auth_overlay_window = nullptr;
    void* notification_overlay_window = nullptr;
    void* settings_menu_overlay_window = nullptr;
    void* omnibox_overlay_window = nullptr;
    void* cookie_panel_overlay_window = nullptr;
    void* download_panel_overlay_window = nullptr;
    void* profile_panel_overlay_window = nullptr;
    void* menu_overlay_window = nullptr;

    // NSEvent local monitors for overlay click-outside detection (6 total)
    void* omnibox_event_monitor = nullptr;
    void* cookie_panel_event_monitor = nullptr;
    void* download_panel_event_monitor = nullptr;
    void* profile_panel_event_monitor = nullptr;
    void* settings_menu_event_monitor = nullptr;
    void* menu_event_monitor = nullptr;

    // Icon offsets for right-side panel positioning (physical pixel distance
    // from icon's right edge to header's right edge)
    int settings_icon_right_offset = 0;
    int cookie_icon_right_offset = 0;
    int download_icon_right_offset = 0;
    int profile_icon_right_offset = 0;
    int wallet_icon_right_offset = 0;
    int menu_icon_right_offset = 0;
#endif

    // ---- CEF browser refs (15 total — mirrors old SimpleHandler statics) ----
    CefRefPtr<CefBrowser> header_browser;
    CefRefPtr<CefBrowser> webview_browser;
    CefRefPtr<CefBrowser> wallet_panel_browser;
    CefRefPtr<CefBrowser> overlay_browser;
    CefRefPtr<CefBrowser> settings_browser;
    CefRefPtr<CefBrowser> wallet_browser;
    CefRefPtr<CefBrowser> backup_browser;
    CefRefPtr<CefBrowser> brc100_auth_browser;
    CefRefPtr<CefBrowser> notification_browser;
    CefRefPtr<CefBrowser> settings_menu_browser;
    CefRefPtr<CefBrowser> omnibox_browser;
    CefRefPtr<CefBrowser> cookie_panel_browser;
    CefRefPtr<CefBrowser> download_panel_browser;
    CefRefPtr<CefBrowser> profile_panel_browser;
    CefRefPtr<CefBrowser> menu_browser;

    // ---- Browser ref accessors by role string ----
    void SetBrowserForRole(const std::string& role, CefRefPtr<CefBrowser> browser);
    CefRefPtr<CefBrowser> GetBrowserForRole(const std::string& role) const;
    void ClearBrowserForRole(const std::string& role);
};
