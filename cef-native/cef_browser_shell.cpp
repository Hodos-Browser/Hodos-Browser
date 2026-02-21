// #define CEF_ENABLE_SANDBOX 0

#pragma once

#ifndef WIN32_LEAN_AND_MEAN
#define WIN32_LEAN_AND_MEAN
#endif

#ifndef NOMINMAX
#define NOMINMAX
#endif

#undef ERROR  // 💥 Avoid conflict with wingdi.h macro

#include "cef_app.h"
#include "cef_client.h"
#include "cef_browser.h"
#include "cef_command_line.h"
#include "cef_life_span_handler.h"
#include "wrapper/cef_helpers.h"
#include "include/cef_render_process_handler.h"
#include "include/cef_v8.h"
#include "include/cef_browser.h"
#include "include/internal/cef_types.h"
#include "include/handlers/simple_handler.h"
#include "include/handlers/simple_render_process_handler.h"
#include "include/handlers/simple_app.h"
#include "include/core/WalletService.h"
#include "include/core/TabManager.h"
#include "include/core/HistoryManager.h"
#include "include/core/CookieBlockManager.h"
#include "include/core/BookmarkManager.h"
#include "include/core/Logger.h"
#include <shellapi.h>
#include <windows.h>
#include <algorithm>  // For std::max
#include <windowsx.h>
#include <filesystem>
#include <iostream>
#include <fstream>
#include <chrono>
#include <iomanip>
#include <sstream>

HWND g_hwnd = nullptr;
HWND g_header_hwnd = nullptr;
HWND g_webview_hwnd = nullptr;
HINSTANCE g_hInstance = nullptr;

// Global overlay HWNDs for shutdown cleanup
HWND g_settings_overlay_hwnd = nullptr;
HWND g_wallet_overlay_hwnd = nullptr;
HWND g_backup_overlay_hwnd = nullptr;
HWND g_brc100_auth_overlay_hwnd = nullptr;
HWND g_settings_menu_overlay_hwnd = nullptr;
HWND g_omnibox_overlay_hwnd = nullptr;
HWND g_cookie_panel_overlay_hwnd = nullptr;
HWND g_notification_overlay_hwnd = nullptr;

// File dialog guard — prevents overlay close when a native file dialog is open
bool g_file_dialog_active = false;

// Global mouse hooks for overlay click-outside detection
HHOOK g_omnibox_mouse_hook = nullptr;
HHOOK g_cookie_panel_mouse_hook = nullptr;
HHOOK g_settings_mouse_hook = nullptr;

// Stored icon right offsets for repositioning overlays on WM_SIZE/WM_MOVE
// (physical pixel distance from icon's right edge to header's right edge)
int g_settings_icon_right_offset = 0;
int g_cookie_icon_right_offset = 0;
int g_wallet_icon_right_offset = 0;

// Fullscreen state tracking
bool g_is_fullscreen = false;

// Wallet server process management
PROCESS_INFORMATION g_walletServerProcess = {};
bool g_walletServerRunning = false;
HANDLE g_walletJobObject = nullptr;  // Job object: auto-kills child when parent exits

// Forward declarations for wallet server management
void StartWalletServer();
void StopWalletServer();

// Convenience macros for easier logging
#define LOG_DEBUG(msg) Logger::Log(msg, 0, 0)
#define LOG_INFO(msg) Logger::Log(msg, 1, 0)
#define LOG_WARNING(msg) Logger::Log(msg, 2, 0)
#define LOG_ERROR(msg) Logger::Log(msg, 3, 0)

#define LOG_DEBUG_RENDER(msg) Logger::Log(msg, 0, 1)
#define LOG_INFO_RENDER(msg) Logger::Log(msg, 1, 1)
#define LOG_WARNING_RENDER(msg) Logger::Log(msg, 2, 1)
#define LOG_ERROR_RENDER(msg) Logger::Log(msg, 3, 1)

#define LOG_DEBUG_BROWSER(msg) Logger::Log(msg, 0, 2)
#define LOG_INFO_BROWSER(msg) Logger::Log(msg, 1, 2)
#define LOG_WARNING_BROWSER(msg) Logger::Log(msg, 2, 2)
#define LOG_ERROR_BROWSER(msg) Logger::Log(msg, 3, 2)

// Legacy DebugLog function for backward compatibility
void DebugLog(const std::string& message) {
    LOG_INFO(message);
}

// Handle fullscreen mode transitions (called from SimpleHandler::OnFullscreenModeChange)
void HandleFullscreenChange(bool fullscreen) {
    g_is_fullscreen = fullscreen;

    if (!g_hwnd || !IsWindow(g_hwnd)) return;

    RECT rect;
    GetClientRect(g_hwnd, &rect);
    int width = rect.right - rect.left;
    int height = rect.bottom - rect.top;

    if (fullscreen) {
        LOG_DEBUG("🖥️ Entering fullscreen — hiding header, expanding tabs");
        // Hide header
        if (g_header_hwnd && IsWindow(g_header_hwnd)) {
            ShowWindow(g_header_hwnd, SW_HIDE);
        }
        // Expand all tab windows to fill entire client area
        std::vector<Tab*> tabs = TabManager::GetInstance().GetAllTabs();
        for (Tab* tab : tabs) {
            if (tab && tab->hwnd && IsWindow(tab->hwnd)) {
                SetWindowPos(tab->hwnd, nullptr, 0, 0, width, height,
                            SWP_NOZORDER | SWP_NOACTIVATE);
                if (tab->browser) {
                    HWND cef_hwnd = tab->browser->GetHost()->GetWindowHandle();
                    if (cef_hwnd && IsWindow(cef_hwnd)) {
                        SetWindowPos(cef_hwnd, nullptr, 0, 0, width, height,
                                    SWP_NOZORDER | SWP_NOACTIVATE);
                        tab->browser->GetHost()->WasResized();
                    }
                }
            }
        }
    } else {
        LOG_DEBUG("🖥️ Exiting fullscreen — restoring header and tab layout");
        // Show header
        if (g_header_hwnd && IsWindow(g_header_hwnd)) {
            ShowWindow(g_header_hwnd, SW_SHOW);
        }
        // Restore normal layout (same as WM_SIZE)
        int shellHeight = (std::max)(100, static_cast<int>(height * 0.12));
        int webviewHeight = height - shellHeight;

        if (g_header_hwnd && IsWindow(g_header_hwnd)) {
            SetWindowPos(g_header_hwnd, nullptr, 0, 0, width, shellHeight,
                SWP_NOZORDER | SWP_NOACTIVATE);
            CefRefPtr<CefBrowser> header_browser = SimpleHandler::GetHeaderBrowser();
            if (header_browser) {
                HWND header_cef_hwnd = header_browser->GetHost()->GetWindowHandle();
                if (header_cef_hwnd && IsWindow(header_cef_hwnd)) {
                    SetWindowPos(header_cef_hwnd, nullptr, 0, 0, width, shellHeight,
                        SWP_NOZORDER | SWP_NOACTIVATE);
                    header_browser->GetHost()->WasResized();
                }
            }
        }
        // Restore all tab windows below header
        std::vector<Tab*> tabs = TabManager::GetInstance().GetAllTabs();
        for (Tab* tab : tabs) {
            if (tab && tab->hwnd && IsWindow(tab->hwnd)) {
                SetWindowPos(tab->hwnd, nullptr, 0, shellHeight, width, webviewHeight,
                            SWP_NOZORDER | SWP_NOACTIVATE);
                if (tab->browser) {
                    HWND cef_hwnd = tab->browser->GetHost()->GetWindowHandle();
                    if (cef_hwnd && IsWindow(cef_hwnd)) {
                        SetWindowPos(cef_hwnd, nullptr, 0, 0, width, webviewHeight,
                                    SWP_NOZORDER | SWP_NOACTIVATE);
                        tab->browser->GetHost()->WasResized();
                    }
                }
            }
        }
    }
}

// Graceful shutdown function
void ShutdownApplication() {
    LOG_INFO("🛑 Starting graceful application shutdown...");

    // Step 0: Stop wallet server first (to prevent orphaned processes)
    LOG_INFO("🔄 Stopping wallet server...");
    StopWalletServer();

    // Step 1: Close all CEF browsers first
    LOG_INFO("🔄 Closing CEF browsers...");
    CefRefPtr<CefBrowser> header_browser = SimpleHandler::GetHeaderBrowser();
    CefRefPtr<CefBrowser> webview_browser = SimpleHandler::GetWebviewBrowser();
    CefRefPtr<CefBrowser> settings_browser = SimpleHandler::GetSettingsBrowser();
    CefRefPtr<CefBrowser> wallet_browser = SimpleHandler::GetWalletBrowser();
    CefRefPtr<CefBrowser> backup_browser = SimpleHandler::GetBackupBrowser();
    CefRefPtr<CefBrowser> brc100_auth_browser = SimpleHandler::GetBRC100AuthBrowser();

    if (header_browser) {
        LOG_INFO("🔄 Closing header browser...");
        header_browser->GetHost()->CloseBrowser(false);
    }

    if (webview_browser) {
        LOG_INFO("🔄 Closing webview browser...");
        webview_browser->GetHost()->CloseBrowser(false);
    }

    if (settings_browser) {
        LOG_INFO("🔄 Closing settings browser...");
        settings_browser->GetHost()->CloseBrowser(false);
    }

    if (wallet_browser) {
        LOG_INFO("🔄 Closing wallet browser...");
        wallet_browser->GetHost()->CloseBrowser(false);
    }

    if (backup_browser) {
        LOG_INFO("🔄 Closing backup browser...");
        backup_browser->GetHost()->CloseBrowser(false);
    }

    if (brc100_auth_browser) {
        LOG_INFO("🔄 Closing BRC-100 auth browser...");
        brc100_auth_browser->GetHost()->CloseBrowser(false);
    }

    // Step 2: Destroy overlay windows
    LOG_INFO("🔄 Destroying overlay windows...");
    if (g_settings_mouse_hook) {
        UnhookWindowsHookEx(g_settings_mouse_hook);
        g_settings_mouse_hook = nullptr;
        LOG_INFO("🔄 Settings mouse hook removed during shutdown");
    }
    if (g_settings_overlay_hwnd && IsWindow(g_settings_overlay_hwnd)) {
        LOG_INFO("🔄 Destroying settings overlay window...");
        DestroyWindow(g_settings_overlay_hwnd);
        g_settings_overlay_hwnd = nullptr;
    }

    if (g_wallet_overlay_hwnd && IsWindow(g_wallet_overlay_hwnd)) {
        LOG_INFO("🔄 Destroying wallet overlay window...");
        DestroyWindow(g_wallet_overlay_hwnd);
        g_wallet_overlay_hwnd = nullptr;
    }

    if (g_backup_overlay_hwnd && IsWindow(g_backup_overlay_hwnd)) {
        LOG_INFO("🔄 Destroying backup overlay window...");
        DestroyWindow(g_backup_overlay_hwnd);
        g_backup_overlay_hwnd = nullptr;
    }

    if (g_brc100_auth_overlay_hwnd && IsWindow(g_brc100_auth_overlay_hwnd)) {
        LOG_INFO("🔄 Destroying BRC-100 auth overlay window...");
        DestroyWindow(g_brc100_auth_overlay_hwnd);
        g_brc100_auth_overlay_hwnd = nullptr;
    }

    if (g_notification_overlay_hwnd && IsWindow(g_notification_overlay_hwnd)) {
        LOG_INFO("🔄 Destroying notification overlay window...");
        DestroyWindow(g_notification_overlay_hwnd);
        g_notification_overlay_hwnd = nullptr;
    }

    if (g_omnibox_overlay_hwnd && IsWindow(g_omnibox_overlay_hwnd)) {
        LOG_INFO("🔄 Destroying omnibox overlay window...");
        // Remove mouse hook if still installed
        if (g_omnibox_mouse_hook) {
            UnhookWindowsHookEx(g_omnibox_mouse_hook);
            g_omnibox_mouse_hook = nullptr;
            LOG_INFO("🔄 Omnibox mouse hook removed during shutdown");
        }
        DestroyWindow(g_omnibox_overlay_hwnd);
        g_omnibox_overlay_hwnd = nullptr;
    }

    if (g_cookie_panel_overlay_hwnd && IsWindow(g_cookie_panel_overlay_hwnd)) {
        LOG_INFO("🔄 Destroying cookie panel overlay window...");
        // Remove mouse hook if still installed
        if (g_cookie_panel_mouse_hook) {
            UnhookWindowsHookEx(g_cookie_panel_mouse_hook);
            g_cookie_panel_mouse_hook = nullptr;
            LOG_INFO("🔄 Cookie panel mouse hook removed during shutdown");
        }
        DestroyWindow(g_cookie_panel_overlay_hwnd);
        g_cookie_panel_overlay_hwnd = nullptr;
    }


    // Step 3: Destroy main windows (child windows first)
    LOG_INFO("🔄 Destroying main windows...");
    if (g_header_hwnd && IsWindow(g_header_hwnd)) {
        LOG_INFO("🔄 Destroying header window...");
        DestroyWindow(g_header_hwnd);
        g_header_hwnd = nullptr;
    }

    if (g_webview_hwnd && IsWindow(g_webview_hwnd)) {
        LOG_INFO("🔄 Destroying webview window...");
        DestroyWindow(g_webview_hwnd);
        g_webview_hwnd = nullptr;
    }

    // Step 4: Destroy main shell window last
    if (g_hwnd && IsWindow(g_hwnd)) {
        LOG_INFO("🔄 Destroying main shell window...");
        DestroyWindow(g_hwnd);
        g_hwnd = nullptr;
    }

    LOG_INFO("✅ Application shutdown complete");

    // Shutdown logger
    Logger::Shutdown();
}

LRESULT CALLBACK ShellWindowProc(HWND hwnd, UINT msg, WPARAM wParam, LPARAM lParam) {
    switch (msg) {
        case WM_MOVE: {
            // Handle window movement - move overlay windows with main window
            RECT mainRect;
            GetWindowRect(hwnd, &mainRect);
            int width = mainRect.right - mainRect.left;
            int height = mainRect.bottom - mainRect.top;

            LOG_DEBUG("🔄 Main window moved to: " + std::to_string(mainRect.left) + ", " + std::to_string(mainRect.top));

            // Move settings overlay if it exists and is visible (right-side popup)
            if (g_settings_overlay_hwnd && IsWindow(g_settings_overlay_hwnd) && IsWindowVisible(g_settings_overlay_hwnd)) {
                RECT headerRect;
                GetWindowRect(g_header_hwnd, &headerRect);
                int panelWidth = 450;
                int panelHeight = 450;
                int overlayX = headerRect.right - g_settings_icon_right_offset - panelWidth;
                int overlayY = headerRect.top + 104;
                if (overlayY + panelHeight > mainRect.bottom) {
                    panelHeight = mainRect.bottom - overlayY;
                    if (panelHeight < 200) panelHeight = 200;
                }
                SetWindowPos(g_settings_overlay_hwnd, HWND_TOPMOST,
                    overlayX, overlayY, panelWidth, panelHeight,
                    SWP_NOACTIVATE | SWP_SHOWWINDOW);
            }

            // Move cookie panel overlay if it exists and is visible (right-side popup)
            if (g_cookie_panel_overlay_hwnd && IsWindow(g_cookie_panel_overlay_hwnd) && IsWindowVisible(g_cookie_panel_overlay_hwnd)) {
                RECT hdrRect;
                GetWindowRect(g_header_hwnd, &hdrRect);
                int cpWidth = 450;
                int cpHeight = 450;
                int cpX = hdrRect.right - g_cookie_icon_right_offset - cpWidth;
                int cpY = hdrRect.top + 104;
                SetWindowPos(g_cookie_panel_overlay_hwnd, HWND_TOPMOST,
                    cpX, cpY, cpWidth, cpHeight,
                    SWP_NOACTIVATE | SWP_SHOWWINDOW);
            }

            // Move wallet overlay if it exists and is visible
            if (g_wallet_overlay_hwnd && IsWindow(g_wallet_overlay_hwnd) && IsWindowVisible(g_wallet_overlay_hwnd)) {
                SetWindowPos(g_wallet_overlay_hwnd, HWND_TOPMOST,
                    mainRect.left, mainRect.top, width, height,
                    SWP_NOACTIVATE | SWP_SHOWWINDOW);
            }

            // Move backup overlay if it exists and is visible
            if (g_backup_overlay_hwnd && IsWindow(g_backup_overlay_hwnd) && IsWindowVisible(g_backup_overlay_hwnd)) {
                SetWindowPos(g_backup_overlay_hwnd, HWND_TOPMOST,
                    mainRect.left, mainRect.top, width, height,
                    SWP_NOACTIVATE | SWP_SHOWWINDOW);
                LOG_DEBUG("🔄 Moved backup overlay to match main window");
            }

            // Move BRC-100 auth overlay if it exists and is visible
            if (g_brc100_auth_overlay_hwnd && IsWindow(g_brc100_auth_overlay_hwnd) && IsWindowVisible(g_brc100_auth_overlay_hwnd)) {
                SetWindowPos(g_brc100_auth_overlay_hwnd, HWND_TOPMOST,
                    mainRect.left, mainRect.top, width, height,
                    SWP_NOACTIVATE | SWP_SHOWWINDOW);
                LOG_DEBUG("🔄 Moved BRC-100 auth overlay to match main window");
            }

            // Move notification overlay if it exists and is visible
            if (g_notification_overlay_hwnd && IsWindow(g_notification_overlay_hwnd) && IsWindowVisible(g_notification_overlay_hwnd)) {
                SetWindowPos(g_notification_overlay_hwnd, HWND_TOPMOST,
                    mainRect.left, mainRect.top, width, height,
                    SWP_NOACTIVATE | SWP_SHOWWINDOW);
            }

            // Dismiss omnibox overlay on window move (as per CONTEXT.md decision)
            if (g_omnibox_overlay_hwnd && IsWindow(g_omnibox_overlay_hwnd) && IsWindowVisible(g_omnibox_overlay_hwnd)) {
                extern void HideOmniboxOverlay();
                HideOmniboxOverlay();
                LOG_DEBUG("🔍 Dismissed omnibox overlay on window move");
            }

            // IMPORTANT: Call DefWindowProc to ensure Windows updates internal state
            break;  // Let DefWindowProc handle WM_MOVE
        }

        case WM_SIZE: {
            // Handle window resizing - resize child windows and CEF browsers
            RECT rect;
            GetClientRect(hwnd, &rect);
            int width = rect.right - rect.left;
            int height = rect.bottom - rect.top;

            // If in fullscreen mode, keep tabs filling entire window
            if (g_is_fullscreen) {
                std::vector<Tab*> fsTabs = TabManager::GetInstance().GetAllTabs();
                for (Tab* tab : fsTabs) {
                    if (tab && tab->hwnd && IsWindow(tab->hwnd)) {
                        SetWindowPos(tab->hwnd, nullptr, 0, 0, width, height,
                                    SWP_NOZORDER | SWP_NOACTIVATE);
                        if (tab->browser) {
                            HWND cef_hwnd = tab->browser->GetHost()->GetWindowHandle();
                            if (cef_hwnd && IsWindow(cef_hwnd)) {
                                SetWindowPos(cef_hwnd, nullptr, 0, 0, width, height,
                                            SWP_NOZORDER | SWP_NOACTIVATE);
                                tab->browser->GetHost()->WasResized();
                            }
                        }
                    }
                }
                return 0;
            }

            // Header: 12% of parent height, minimum 100px (for tab bar 40px + toolbar 52px)
            int shellHeight = (std::max)(100, static_cast<int>(height * 0.12));
            int webviewHeight = height - shellHeight;

            LOG_DEBUG("🔄 Main window resized: " + std::to_string(width) + "x" + std::to_string(height));

            // Resize header window
            if (g_header_hwnd && IsWindow(g_header_hwnd)) {
                SetWindowPos(g_header_hwnd, nullptr, 0, 0, width, shellHeight,
                    SWP_NOZORDER | SWP_NOACTIVATE);

                // Resize the CEF browser in the header window
                CefRefPtr<CefBrowser> header_browser = SimpleHandler::GetHeaderBrowser();
                if (header_browser) {
                    HWND header_cef_hwnd = header_browser->GetHost()->GetWindowHandle();
                    if (header_cef_hwnd && IsWindow(header_cef_hwnd)) {
                        SetWindowPos(header_cef_hwnd, nullptr, 0, 0, width, shellHeight,
                            SWP_NOZORDER | SWP_NOACTIVATE);
                        header_browser->GetHost()->WasResized();
                    }
                }
            }

            // Resize webview window (legacy - will be removed when fully migrated to tabs)
            if (g_webview_hwnd && IsWindow(g_webview_hwnd)) {
                SetWindowPos(g_webview_hwnd, nullptr, 0, shellHeight, width, webviewHeight,
                    SWP_NOZORDER | SWP_NOACTIVATE);

                // Resize the CEF browser in the webview window
                CefRefPtr<CefBrowser> webview_browser = SimpleHandler::GetWebviewBrowser();
                if (webview_browser) {
                    HWND webview_cef_hwnd = webview_browser->GetHost()->GetWindowHandle();
                    if (webview_cef_hwnd && IsWindow(webview_cef_hwnd)) {
                        SetWindowPos(webview_cef_hwnd, nullptr, 0, 0, width, webviewHeight,
                            SWP_NOZORDER | SWP_NOACTIVATE);
                        webview_browser->GetHost()->WasResized();
                    }
                }
            }

            // Resize all tab windows and browsers (NEW for tab management)
            std::vector<Tab*> tabs = TabManager::GetInstance().GetAllTabs();
            for (Tab* tab : tabs) {
                if (tab && tab->hwnd && IsWindow(tab->hwnd)) {
                    // Position tab window below header
                    SetWindowPos(tab->hwnd, nullptr, 0, shellHeight, width, webviewHeight,
                                SWP_NOZORDER | SWP_NOACTIVATE);

                    // Resize tab's CEF browser
                    if (tab->browser) {
                        HWND cef_hwnd = tab->browser->GetHost()->GetWindowHandle();
                        if (cef_hwnd && IsWindow(cef_hwnd)) {
                            SetWindowPos(cef_hwnd, nullptr, 0, 0, width, webviewHeight,
                                        SWP_NOZORDER | SWP_NOACTIVATE);
                            tab->browser->GetHost()->WasResized();
                        }
                    }
                }
            }

            // Resize overlay windows if they exist and are visible
            // Get the new main window screen position for overlays
            RECT mainRect;
            GetWindowRect(hwnd, &mainRect);

            // Reposition settings panel (right-side popup, right edge under icon)
            if (g_settings_overlay_hwnd && IsWindow(g_settings_overlay_hwnd) && IsWindowVisible(g_settings_overlay_hwnd)) {
                RECT headerRect;
                GetWindowRect(g_header_hwnd, &headerRect);
                int panelWidth = 450;
                int panelHeight = 450;
                int overlayX = headerRect.right - g_settings_icon_right_offset - panelWidth;
                int overlayY = headerRect.top + 104;
                if (overlayY + panelHeight > mainRect.bottom) {
                    panelHeight = mainRect.bottom - overlayY;
                    if (panelHeight < 200) panelHeight = 200;
                }
                SetWindowPos(g_settings_overlay_hwnd, HWND_TOPMOST,
                    overlayX, overlayY, panelWidth, panelHeight,
                    SWP_NOACTIVATE | SWP_SHOWWINDOW);

                CefRefPtr<CefBrowser> settings_browser = SimpleHandler::GetSettingsBrowser();
                if (settings_browser) {
                    settings_browser->GetHost()->WasResized();
                }
            }

            // Reposition cookie panel (right-side popup, right edge under icon)
            if (g_cookie_panel_overlay_hwnd && IsWindow(g_cookie_panel_overlay_hwnd) && IsWindowVisible(g_cookie_panel_overlay_hwnd)) {
                RECT hdrRect;
                GetWindowRect(g_header_hwnd, &hdrRect);
                int cpWidth = 450;
                int cpHeight = 450;
                int cpX = hdrRect.right - g_cookie_icon_right_offset - cpWidth;
                int cpY = hdrRect.top + 104;
                SetWindowPos(g_cookie_panel_overlay_hwnd, HWND_TOPMOST,
                    cpX, cpY, cpWidth, cpHeight,
                    SWP_NOACTIVATE | SWP_SHOWWINDOW);

                CefRefPtr<CefBrowser> cookie_browser = SimpleHandler::GetCookiePanelBrowser();
                if (cookie_browser) {
                    cookie_browser->GetHost()->WasResized();
                }
            }

            // Resize wallet overlay
            if (g_wallet_overlay_hwnd && IsWindow(g_wallet_overlay_hwnd) && IsWindowVisible(g_wallet_overlay_hwnd)) {
                SetWindowPos(g_wallet_overlay_hwnd, HWND_TOPMOST,
                    mainRect.left, mainRect.top, width, height,
                    SWP_NOACTIVATE | SWP_SHOWWINDOW);

                CefRefPtr<CefBrowser> wallet_browser = SimpleHandler::GetWalletBrowser();
                if (wallet_browser) {
                    wallet_browser->GetHost()->WasResized();
                }
            }

            // Resize backup overlay
            if (g_backup_overlay_hwnd && IsWindow(g_backup_overlay_hwnd) && IsWindowVisible(g_backup_overlay_hwnd)) {
                SetWindowPos(g_backup_overlay_hwnd, HWND_TOPMOST,
                    mainRect.left, mainRect.top, width, height,
                    SWP_NOACTIVATE | SWP_SHOWWINDOW);

                // Notify CEF browser of resize
                CefRefPtr<CefBrowser> backup_browser = SimpleHandler::GetBackupBrowser();
                if (backup_browser) {
                    backup_browser->GetHost()->WasResized();
                }
                LOG_DEBUG("🔄 Resized backup overlay to match main window");
            }

            // Resize BRC-100 auth overlay
            if (g_brc100_auth_overlay_hwnd && IsWindow(g_brc100_auth_overlay_hwnd) && IsWindowVisible(g_brc100_auth_overlay_hwnd)) {
                SetWindowPos(g_brc100_auth_overlay_hwnd, HWND_TOPMOST,
                    mainRect.left, mainRect.top, width, height,
                    SWP_NOACTIVATE | SWP_SHOWWINDOW);

                // Notify CEF browser of resize
                CefRefPtr<CefBrowser> auth_browser = SimpleHandler::GetBRC100AuthBrowser();
                if (auth_browser) {
                    auth_browser->GetHost()->WasResized();
                }
                LOG_DEBUG("🔄 Resized BRC-100 auth overlay to match main window");
            }

            // Resize notification overlay
            if (g_notification_overlay_hwnd && IsWindow(g_notification_overlay_hwnd) && IsWindowVisible(g_notification_overlay_hwnd)) {
                SetWindowPos(g_notification_overlay_hwnd, HWND_TOPMOST,
                    mainRect.left, mainRect.top, width, height,
                    SWP_NOACTIVATE | SWP_SHOWWINDOW);
                CefRefPtr<CefBrowser> notif_browser = SimpleHandler::GetNotificationBrowser();
                if (notif_browser) {
                    notif_browser->GetHost()->WasResized();
                }
            }

            // Dismiss omnibox overlay on window resize (as per CONTEXT.md decision)
            if (g_omnibox_overlay_hwnd && IsWindow(g_omnibox_overlay_hwnd) && IsWindowVisible(g_omnibox_overlay_hwnd)) {
                extern void HideOmniboxOverlay();
                HideOmniboxOverlay();
                LOG_DEBUG("🔍 Dismissed omnibox overlay on window resize");
            }

            return 0;
        }

        case WM_ACTIVATEAPP: {
            // wParam is TRUE if app is being activated, FALSE if deactivated
            if (!wParam) {
                // App is losing focus

                // If a native file dialog is open (e.g. from <input type="file">),
                // skip overlay destruction — the dialog steals activation temporarily.
                if (g_file_dialog_active) {
                    LOG_DEBUG("📱 App losing focus but file dialog is active - keeping overlays open");
                    break;
                }

                // Close wallet overlay if it's open
                LOG_DEBUG("📱 App losing focus - closing wallet overlay if open");
                if (g_wallet_overlay_hwnd && IsWindow(g_wallet_overlay_hwnd) && IsWindowVisible(g_wallet_overlay_hwnd)) {
                    LOG_INFO("💰 Closing wallet overlay due to app focus loss");
                    // Hide immediately for instant visual feedback
                    ShowWindow(g_wallet_overlay_hwnd, SW_HIDE);
                    // Then close the browser and destroy window
                    CefRefPtr<CefBrowser> wallet_browser = SimpleHandler::GetWalletBrowser();
                    if (wallet_browser) {
                        wallet_browser->GetHost()->CloseBrowser(false);
                    }
                    DestroyWindow(g_wallet_overlay_hwnd);
                    g_wallet_overlay_hwnd = nullptr;
                }

                // Dismiss omnibox overlay on focus loss (as per CONTEXT.md decision)
                if (g_omnibox_overlay_hwnd && IsWindow(g_omnibox_overlay_hwnd) && IsWindowVisible(g_omnibox_overlay_hwnd)) {
                    extern void HideOmniboxOverlay();
                    HideOmniboxOverlay();
                    LOG_DEBUG("🔍 Dismissed omnibox overlay on focus loss");
                }
            } else {
                // App regaining focus — clear file dialog guard
                if (g_file_dialog_active) {
                    LOG_DEBUG("📱 App regaining focus - clearing file dialog guard");
                    g_file_dialog_active = false;
                }
            }
            break;
        }

        case WM_CLOSE:
            LOG_INFO("🛑 Main shell window received WM_CLOSE - starting graceful shutdown...");
            ShutdownApplication();
            PostQuitMessage(0);
            return 0;

        case WM_DESTROY:
            LOG_INFO("🛑 Main shell window received WM_DESTROY");
            PostQuitMessage(0);
            break;
    }

    return DefWindowProc(hwnd, msg, wParam, lParam);
}


LRESULT CALLBACK SettingsOverlayWndProc(HWND hwnd, UINT msg, WPARAM wParam, LPARAM lParam) {
    switch (msg) {
        case WM_MOUSEACTIVATE:
            // Prevent focus theft - matches cookie panel pattern
            return MA_NOACTIVATE;

        case WM_SETCURSOR: {
            SetCursor(LoadCursor(nullptr, IDC_ARROW));
            return TRUE;
        }

        case WM_MOUSEMOVE: {
            // Forward mouse moves to CEF for hover states
            POINT pt = { GET_X_LPARAM(lParam), GET_Y_LPARAM(lParam) };
            CefMouseEvent mouse_event;
            mouse_event.x = pt.x;
            mouse_event.y = pt.y;
            mouse_event.modifiers = 0;

            CefRefPtr<CefBrowser> settings_browser = SimpleHandler::GetSettingsBrowser();
            if (settings_browser) {
                settings_browser->GetHost()->SendMouseMoveEvent(mouse_event, false);
            }
            return 0;
        }

        case WM_LBUTTONDOWN: {
            SetCapture(hwnd);  // Capture mouse so we get WM_LBUTTONUP
            POINT pt = { GET_X_LPARAM(lParam), GET_Y_LPARAM(lParam) };
            CefMouseEvent mouse_event;
            mouse_event.x = pt.x;
            mouse_event.y = pt.y;
            mouse_event.modifiers = 0;

            CefRefPtr<CefBrowser> settings_browser = SimpleHandler::GetSettingsBrowser();
            if (settings_browser) {
                settings_browser->GetHost()->SendMouseClickEvent(mouse_event, MBT_LEFT, false, 1);
            }
            return 0;
        }

        case WM_LBUTTONUP: {
            ReleaseCapture();
            POINT pt = { GET_X_LPARAM(lParam), GET_Y_LPARAM(lParam) };
            CefMouseEvent mouse_event;
            mouse_event.x = pt.x;
            mouse_event.y = pt.y;
            mouse_event.modifiers = 0;

            CefRefPtr<CefBrowser> settings_browser = SimpleHandler::GetSettingsBrowser();
            if (settings_browser) {
                settings_browser->GetHost()->SendMouseClickEvent(mouse_event, MBT_LEFT, true, 1);
            }
            return 0;
        }

        case WM_MOUSEWHEEL: {
            POINT pt = { GET_X_LPARAM(lParam), GET_Y_LPARAM(lParam) };
            int delta = GET_WHEEL_DELTA_WPARAM(wParam);

            CefMouseEvent mouse_event;
            mouse_event.x = pt.x;
            mouse_event.y = pt.y;
            mouse_event.modifiers = 0;

            CefRefPtr<CefBrowser> settings_browser = SimpleHandler::GetSettingsBrowser();
            if (settings_browser) {
                settings_browser->GetHost()->SendMouseWheelEvent(mouse_event, 0, delta);
            }
            return 0;
        }

        case WM_CLOSE:
            LOG_INFO("❌ Settings Overlay received WM_CLOSE - destroying window");
            DestroyWindow(hwnd);
            return 0;

        case WM_DESTROY:
            return 0;

        case WM_WINDOWPOSCHANGING:
            break;
    }
    return DefWindowProc(hwnd, msg, wParam, lParam);
}

LRESULT CALLBACK WalletOverlayWndProc(HWND hwnd, UINT msg, WPARAM wParam, LPARAM lParam) {
    switch (msg) {
        case WM_MOUSEACTIVATE:
            LOG_DEBUG("👆 Wallet Overlay HWND received WM_MOUSEACTIVATE");
            // Allow normal activation without forcing z-order
            return MA_ACTIVATE;

        case WM_LBUTTONDOWN: {
            LOG_DEBUG("🖱️ Wallet Overlay received WM_LBUTTONDOWN");
            SetFocus(hwnd);

            POINT pt = { GET_X_LPARAM(lParam), GET_Y_LPARAM(lParam) };
            // Translate to CEF MouseEvent
            CefMouseEvent mouse_event;
            mouse_event.x = pt.x;
            mouse_event.y = pt.y;
            mouse_event.modifiers = 0;

            // Find the wallet browser
            CefRefPtr<CefBrowser> wallet_browser = SimpleHandler::GetWalletBrowser();
            if (wallet_browser) {
                wallet_browser->GetHost()->SendMouseClickEvent(mouse_event, MBT_LEFT, false, 1);  // mouse down
                wallet_browser->GetHost()->SendMouseClickEvent(mouse_event, MBT_LEFT, true, 1);   // mouse up
                LOG_DEBUG("🧠 Left-click sent to wallet overlay browser");
            } else {
                LOG_DEBUG("⚠️ No wallet overlay browser to send left-click");
            }

            return 0;
        }

        case WM_RBUTTONDOWN: {
            LOG_DEBUG("🖱️ Wallet Overlay received WM_RBUTTONDOWN");
            SetFocus(hwnd);

            POINT pt = { GET_X_LPARAM(lParam), GET_Y_LPARAM(lParam) };
            // Translate to CEF MouseEvent
            CefMouseEvent mouse_event;
            mouse_event.x = pt.x;
            mouse_event.y = pt.y;
            mouse_event.modifiers = 0;

            // Find the wallet browser
            CefRefPtr<CefBrowser> wallet_browser = SimpleHandler::GetWalletBrowser();
            if (wallet_browser) {
                wallet_browser->GetHost()->SendMouseClickEvent(mouse_event, MBT_RIGHT, false, 1);  // mouse down
                wallet_browser->GetHost()->SendMouseClickEvent(mouse_event, MBT_RIGHT, true, 1);   // mouse up
                LOG_DEBUG("🧠 Right-click sent to wallet overlay browser");
            } else {
                LOG_DEBUG("⚠️ No wallet overlay browser to send right-click");
            }

            return 0;
        }

        case WM_KEYDOWN: {
            LOG_DEBUG("⌨️ Wallet Overlay received WM_KEYDOWN - key: " + std::to_string(wParam));
            SetFocus(hwnd);

            // Find the wallet browser
            CefRefPtr<CefBrowser> wallet_browser = SimpleHandler::GetWalletBrowser();
            if (wallet_browser) {
                // Create CEF key event
                CefKeyEvent key_event;
                key_event.type = KEYEVENT_KEYDOWN;
                key_event.windows_key_code = wParam;
                key_event.native_key_code = lParam;
                key_event.is_system_key = false;

                // Check for modifier keys
                int modifiers = 0;
                if (GetKeyState(VK_CONTROL) & 0x8000) modifiers |= EVENTFLAG_CONTROL_DOWN;
                if (GetKeyState(VK_SHIFT) & 0x8000) modifiers |= EVENTFLAG_SHIFT_DOWN;
                if (GetKeyState(VK_MENU) & 0x8000) modifiers |= EVENTFLAG_ALT_DOWN;
                if (GetKeyState(VK_LWIN) & 0x8000 || GetKeyState(VK_RWIN) & 0x8000) modifiers |= EVENTFLAG_COMMAND_DOWN;
                key_event.modifiers = modifiers;

                wallet_browser->GetHost()->SendKeyEvent(key_event);
                LOG_DEBUG("⌨️ Key down sent to wallet overlay browser (modifiers: " + std::to_string(modifiers) + ")");
            } else {
                LOG_DEBUG("⚠️ No wallet overlay browser to send key down");
            }

            return 0;
        }

        case WM_KEYUP: {
            LOG_DEBUG("⌨️ Wallet Overlay received WM_KEYUP - key: " + std::to_string(wParam));
            SetFocus(hwnd);

            // Find the wallet browser
            CefRefPtr<CefBrowser> wallet_browser = SimpleHandler::GetWalletBrowser();
            if (wallet_browser) {
                // Create CEF key event
                CefKeyEvent key_event;
                key_event.type = KEYEVENT_KEYUP;
                key_event.windows_key_code = wParam;
                key_event.native_key_code = lParam;
                key_event.is_system_key = false;

                // Check for modifier keys
                int modifiers = 0;
                if (GetKeyState(VK_CONTROL) & 0x8000) modifiers |= EVENTFLAG_CONTROL_DOWN;
                if (GetKeyState(VK_SHIFT) & 0x8000) modifiers |= EVENTFLAG_SHIFT_DOWN;
                if (GetKeyState(VK_MENU) & 0x8000) modifiers |= EVENTFLAG_ALT_DOWN;
                if (GetKeyState(VK_LWIN) & 0x8000 || GetKeyState(VK_RWIN) & 0x8000) modifiers |= EVENTFLAG_COMMAND_DOWN;
                key_event.modifiers = modifiers;

                wallet_browser->GetHost()->SendKeyEvent(key_event);
                LOG_DEBUG("⌨️ Key up sent to wallet overlay browser (modifiers: " + std::to_string(modifiers) + ")");
            } else {
                LOG_DEBUG("⚠️ No wallet overlay browser to send key up");
            }

            return 0;
        }

        case WM_CHAR: {
            LOG_DEBUG("⌨️ Wallet Overlay received WM_CHAR - char: " + std::to_string(wParam));
            SetFocus(hwnd);

            // Find the wallet browser
            CefRefPtr<CefBrowser> wallet_browser = SimpleHandler::GetWalletBrowser();
            if (wallet_browser) {
                // Create CEF key event
                CefKeyEvent key_event;
                key_event.type = KEYEVENT_CHAR;
                key_event.windows_key_code = wParam;
                key_event.native_key_code = lParam;
                key_event.is_system_key = false;

                // Check for modifier keys
                int modifiers = 0;
                if (GetKeyState(VK_CONTROL) & 0x8000) modifiers |= EVENTFLAG_CONTROL_DOWN;
                if (GetKeyState(VK_SHIFT) & 0x8000) modifiers |= EVENTFLAG_SHIFT_DOWN;
                if (GetKeyState(VK_MENU) & 0x8000) modifiers |= EVENTFLAG_ALT_DOWN;
                if (GetKeyState(VK_LWIN) & 0x8000 || GetKeyState(VK_RWIN) & 0x8000) modifiers |= EVENTFLAG_COMMAND_DOWN;
                key_event.modifiers = modifiers;

                wallet_browser->GetHost()->SendKeyEvent(key_event);
                LOG_DEBUG("⌨️ Char sent to wallet overlay browser (modifiers: " + std::to_string(modifiers) + ")");
            } else {
                LOG_DEBUG("⚠️ No wallet overlay browser to send char");
            }

            return 0;
        }

        case WM_CLOSE:
            LOG_DEBUG("❌ Wallet Overlay received WM_CLOSE - destroying window");
            DestroyWindow(hwnd);
            return 0;

        case WM_DESTROY:
            LOG_DEBUG("❌ Wallet Overlay received WM_DESTROY - cleaning up");
            // Clean up any resources if needed
            return 0;

        case WM_ACTIVATE:
            LOG_DEBUG("⚡ Wallet HWND activated with state: " + std::to_string(LOWORD(wParam)));
            break;

        case WM_WINDOWPOSCHANGING:
            // Allow normal z-order changes for better window management
            break;
    }
    return DefWindowProc(hwnd, msg, wParam, lParam);
}

LRESULT CALLBACK BackupOverlayWndProc(HWND hwnd, UINT msg, WPARAM wParam, LPARAM lParam) {
    switch (msg) {
        case WM_MOUSEACTIVATE:
            LOG_DEBUG("👆 Backup Overlay HWND received WM_MOUSEACTIVATE");
            return MA_ACTIVATE;

        case WM_LBUTTONDOWN: {
            LOG_DEBUG("🖱️ Backup Overlay received WM_LBUTTONDOWN");
            SetFocus(hwnd);
            POINT pt = { GET_X_LPARAM(lParam), GET_Y_LPARAM(lParam) };
            CefMouseEvent mouse_event;
            mouse_event.x = pt.x;
            mouse_event.y = pt.y;
            mouse_event.modifiers = 0;
            CefRefPtr<CefBrowser> backup_browser = SimpleHandler::GetBackupBrowser();
            if (backup_browser) {
                backup_browser->GetHost()->SendMouseClickEvent(mouse_event, MBT_LEFT, false, 1);
                backup_browser->GetHost()->SendMouseClickEvent(mouse_event, MBT_LEFT, true, 1);
                LOG_DEBUG("🧠 Left-click sent to backup overlay browser");
            } else {
                LOG_DEBUG("⚠️ No backup overlay browser to send left-click");
            }
            return 0;
        }

        case WM_RBUTTONDOWN: {
            LOG_DEBUG("🖱️ Backup Overlay received WM_RBUTTONDOWN");
            SetFocus(hwnd);
            POINT pt = { GET_X_LPARAM(lParam), GET_Y_LPARAM(lParam) };
            CefMouseEvent mouse_event;
            mouse_event.x = pt.x;
            mouse_event.y = pt.y;
            mouse_event.modifiers = 0;
            CefRefPtr<CefBrowser> backup_browser = SimpleHandler::GetBackupBrowser();
            if (backup_browser) {
                backup_browser->GetHost()->SendMouseClickEvent(mouse_event, MBT_RIGHT, false, 1);
                backup_browser->GetHost()->SendMouseClickEvent(mouse_event, MBT_RIGHT, true, 1);
                LOG_DEBUG("🧠 Right-click sent to backup overlay browser");
            } else {
                LOG_DEBUG("⚠️ No backup overlay browser to send right-click");
            }
            return 0;
        }

        case WM_CLOSE:
            LOG_DEBUG("❌ Backup Overlay received WM_CLOSE - destroying window");
            DestroyWindow(hwnd);
            return 0;

        case WM_DESTROY:
            LOG_DEBUG("❌ Backup Overlay received WM_DESTROY - cleaning up");
            return 0;

        case WM_ACTIVATE:
            LOG_DEBUG("⚡ Backup HWND activated with state: " + std::to_string(LOWORD(wParam)));
            break;

        case WM_WINDOWPOSCHANGING:
            break;
    }
    return DefWindowProc(hwnd, msg, wParam, lParam);
}

LRESULT CALLBACK BRC100AuthOverlayWndProc(HWND hwnd, UINT msg, WPARAM wParam, LPARAM lParam) {
    switch (msg) {
        case WM_MOUSEACTIVATE:
            LOG_DEBUG("👆 BRC-100 Auth Overlay HWND received WM_MOUSEACTIVATE");
            return MA_ACTIVATE;

        case WM_LBUTTONDOWN: {
            LOG_DEBUG("🖱️ BRC-100 Auth Overlay received WM_LBUTTONDOWN");
            SetFocus(hwnd);
            POINT pt = { GET_X_LPARAM(lParam), GET_Y_LPARAM(lParam) };
            CefMouseEvent mouse_event;
            mouse_event.x = pt.x;
            mouse_event.y = pt.y;
            mouse_event.modifiers = 0;
            CefRefPtr<CefBrowser> auth_browser = SimpleHandler::GetBRC100AuthBrowser();
            if (auth_browser) {
                auth_browser->GetHost()->SendMouseClickEvent(mouse_event, MBT_LEFT, false, 1);
                auth_browser->GetHost()->SendMouseClickEvent(mouse_event, MBT_LEFT, true, 1);
                LOG_DEBUG("🧠 Left-click sent to BRC-100 auth overlay browser");
            } else {
                LOG_DEBUG("⚠️ No BRC-100 auth overlay browser to send left-click");
            }
            return 0;
        }

        case WM_RBUTTONDOWN: {
            LOG_DEBUG("🖱️ BRC-100 Auth Overlay received WM_RBUTTONDOWN");
            SetFocus(hwnd);
            POINT pt = { GET_X_LPARAM(lParam), GET_Y_LPARAM(lParam) };
            CefMouseEvent mouse_event;
            mouse_event.x = pt.x;
            mouse_event.y = pt.y;
            mouse_event.modifiers = 0;
            CefRefPtr<CefBrowser> auth_browser = SimpleHandler::GetBRC100AuthBrowser();
            if (auth_browser) {
                auth_browser->GetHost()->SendMouseClickEvent(mouse_event, MBT_RIGHT, false, 1);
                auth_browser->GetHost()->SendMouseClickEvent(mouse_event, MBT_RIGHT, true, 1);
                LOG_DEBUG("🧠 Right-click sent to BRC-100 auth overlay browser");
            } else {
                LOG_DEBUG("⚠️ No BRC-100 auth overlay browser to send right-click");
            }
            return 0;
        }

        case WM_CLOSE:
            LOG_DEBUG("❌ BRC-100 Auth Overlay received WM_CLOSE - destroying window");
            DestroyWindow(hwnd);
            return 0;

        case WM_DESTROY:
            LOG_DEBUG("❌ BRC-100 Auth Overlay received WM_DESTROY - cleaning up");
            return 0;

        case WM_ACTIVATE:
            LOG_DEBUG("⚡ BRC-100 Auth HWND activated with state: " + std::to_string(LOWORD(wParam)));
            break;

        case WM_WINDOWPOSCHANGING:
            break;
    }
    return DefWindowProc(hwnd, msg, wParam, lParam);
}

// Notification Overlay Window Procedure (same pattern as BRC-100 auth overlay)
LRESULT CALLBACK NotificationOverlayWndProc(HWND hwnd, UINT msg, WPARAM wParam, LPARAM lParam) {
    switch (msg) {
        case WM_MOUSEACTIVATE:
            return MA_ACTIVATE;

        case WM_LBUTTONDOWN: {
            SetFocus(hwnd);
            POINT pt = { GET_X_LPARAM(lParam), GET_Y_LPARAM(lParam) };
            CefMouseEvent mouse_event;
            mouse_event.x = pt.x;
            mouse_event.y = pt.y;
            mouse_event.modifiers = 0;
            CefRefPtr<CefBrowser> notif_browser = SimpleHandler::GetNotificationBrowser();
            if (notif_browser) {
                notif_browser->GetHost()->SendMouseClickEvent(mouse_event, MBT_LEFT, false, 1);
                notif_browser->GetHost()->SendMouseClickEvent(mouse_event, MBT_LEFT, true, 1);
            }
            return 0;
        }

        case WM_LBUTTONDBLCLK: {
            SetFocus(hwnd);
            POINT pt = { GET_X_LPARAM(lParam), GET_Y_LPARAM(lParam) };
            CefMouseEvent mouse_event;
            mouse_event.x = pt.x;
            mouse_event.y = pt.y;
            mouse_event.modifiers = 0;
            CefRefPtr<CefBrowser> notif_browser = SimpleHandler::GetNotificationBrowser();
            if (notif_browser) {
                notif_browser->GetHost()->SendMouseClickEvent(mouse_event, MBT_LEFT, false, 2);
                notif_browser->GetHost()->SendMouseClickEvent(mouse_event, MBT_LEFT, true, 2);
            }
            return 0;
        }

        case WM_RBUTTONDOWN: {
            SetFocus(hwnd);
            POINT pt = { GET_X_LPARAM(lParam), GET_Y_LPARAM(lParam) };
            CefMouseEvent mouse_event;
            mouse_event.x = pt.x;
            mouse_event.y = pt.y;
            mouse_event.modifiers = 0;
            CefRefPtr<CefBrowser> notif_browser = SimpleHandler::GetNotificationBrowser();
            if (notif_browser) {
                notif_browser->GetHost()->SendMouseClickEvent(mouse_event, MBT_RIGHT, false, 1);
                notif_browser->GetHost()->SendMouseClickEvent(mouse_event, MBT_RIGHT, true, 1);
            }
            return 0;
        }

        case WM_MOUSEMOVE: {
            POINT pt = { GET_X_LPARAM(lParam), GET_Y_LPARAM(lParam) };
            CefMouseEvent mouse_event;
            mouse_event.x = pt.x;
            mouse_event.y = pt.y;
            mouse_event.modifiers = 0;
            CefRefPtr<CefBrowser> notif_browser = SimpleHandler::GetNotificationBrowser();
            if (notif_browser) {
                notif_browser->GetHost()->SendMouseMoveEvent(mouse_event, false);
            }
            return 0;
        }

        case WM_KEYDOWN: {
            SetFocus(hwnd);
            CefRefPtr<CefBrowser> notif_browser = SimpleHandler::GetNotificationBrowser();
            if (notif_browser) {
                CefKeyEvent key_event;
                key_event.type = KEYEVENT_KEYDOWN;
                key_event.windows_key_code = wParam;
                key_event.native_key_code = lParam;
                key_event.is_system_key = false;
                int modifiers = 0;
                if (GetKeyState(VK_CONTROL) & 0x8000) modifiers |= EVENTFLAG_CONTROL_DOWN;
                if (GetKeyState(VK_SHIFT) & 0x8000) modifiers |= EVENTFLAG_SHIFT_DOWN;
                if (GetKeyState(VK_MENU) & 0x8000) modifiers |= EVENTFLAG_ALT_DOWN;
                if (GetKeyState(VK_LWIN) & 0x8000 || GetKeyState(VK_RWIN) & 0x8000) modifiers |= EVENTFLAG_COMMAND_DOWN;
                key_event.modifiers = modifiers;
                notif_browser->GetHost()->SendKeyEvent(key_event);
            }
            return 0;
        }

        case WM_KEYUP: {
            SetFocus(hwnd);
            CefRefPtr<CefBrowser> notif_browser = SimpleHandler::GetNotificationBrowser();
            if (notif_browser) {
                CefKeyEvent key_event;
                key_event.type = KEYEVENT_KEYUP;
                key_event.windows_key_code = wParam;
                key_event.native_key_code = lParam;
                key_event.is_system_key = false;
                int modifiers = 0;
                if (GetKeyState(VK_CONTROL) & 0x8000) modifiers |= EVENTFLAG_CONTROL_DOWN;
                if (GetKeyState(VK_SHIFT) & 0x8000) modifiers |= EVENTFLAG_SHIFT_DOWN;
                if (GetKeyState(VK_MENU) & 0x8000) modifiers |= EVENTFLAG_ALT_DOWN;
                if (GetKeyState(VK_LWIN) & 0x8000 || GetKeyState(VK_RWIN) & 0x8000) modifiers |= EVENTFLAG_COMMAND_DOWN;
                key_event.modifiers = modifiers;
                notif_browser->GetHost()->SendKeyEvent(key_event);
            }
            return 0;
        }

        case WM_CHAR: {
            SetFocus(hwnd);
            CefRefPtr<CefBrowser> notif_browser = SimpleHandler::GetNotificationBrowser();
            if (notif_browser) {
                CefKeyEvent key_event;
                key_event.type = KEYEVENT_CHAR;
                key_event.windows_key_code = wParam;
                key_event.native_key_code = lParam;
                key_event.is_system_key = false;
                int modifiers = 0;
                if (GetKeyState(VK_CONTROL) & 0x8000) modifiers |= EVENTFLAG_CONTROL_DOWN;
                if (GetKeyState(VK_SHIFT) & 0x8000) modifiers |= EVENTFLAG_SHIFT_DOWN;
                if (GetKeyState(VK_MENU) & 0x8000) modifiers |= EVENTFLAG_ALT_DOWN;
                if (GetKeyState(VK_LWIN) & 0x8000 || GetKeyState(VK_RWIN) & 0x8000) modifiers |= EVENTFLAG_COMMAND_DOWN;
                key_event.modifiers = modifiers;
                notif_browser->GetHost()->SendKeyEvent(key_event);
            }
            return 0;
        }

        case WM_CLOSE:
            DestroyWindow(hwnd);
            return 0;

        case WM_DESTROY:
            return 0;

        case WM_WINDOWPOSCHANGING:
            break;
    }
    return DefWindowProc(hwnd, msg, wParam, lParam);
}

// Settings Menu Dropdown Overlay Window Procedure
LRESULT CALLBACK SettingsMenuOverlayWndProc(HWND hwnd, UINT msg, WPARAM wParam, LPARAM lParam) {
    switch (msg) {
        case WM_LBUTTONDOWN:
        case WM_RBUTTONDOWN:
        case WM_MOUSEMOVE: {
            POINT pt = { GET_X_LPARAM(lParam), GET_Y_LPARAM(lParam) };
            CefMouseEvent mouse_event;
            mouse_event.x = pt.x;
            mouse_event.y = pt.y;

            CefRefPtr<CefBrowser> menu_browser = SimpleHandler::GetSettingsMenuBrowser();
            if (menu_browser && menu_browser->GetHost()) {
                if (msg == WM_LBUTTONDOWN) {
                    menu_browser->GetHost()->SendMouseClickEvent(mouse_event, MBT_LEFT, false, 1);
                    menu_browser->GetHost()->SendMouseClickEvent(mouse_event, MBT_LEFT, true, 1);
                } else if (msg == WM_MOUSEMOVE) {
                    menu_browser->GetHost()->SendMouseMoveEvent(mouse_event, false);
                }
            }
            return 0;
        }

        case WM_CLOSE:
            DestroyWindow(hwnd);
            return 0;

        case WM_DESTROY:
            g_settings_menu_overlay_hwnd = nullptr;
            return 0;
    }

    return DefWindowProc(hwnd, msg, wParam, lParam);
}

// Low-level mouse hook for omnibox click-outside detection
LRESULT CALLBACK OmniboxMouseHookProc(int nCode, WPARAM wParam, LPARAM lParam) {
    if (nCode == HC_ACTION) {
        // Check for mouse down events (left or right button)
        if (wParam == WM_LBUTTONDOWN || wParam == WM_RBUTTONDOWN) {
            // Only process if omnibox overlay is visible
            if (g_omnibox_overlay_hwnd && IsWindow(g_omnibox_overlay_hwnd) && IsWindowVisible(g_omnibox_overlay_hwnd)) {
                MSLLHOOKSTRUCT* mouseInfo = (MSLLHOOKSTRUCT*)lParam;
                POINT clickPoint = mouseInfo->pt;

                // Get overlay window rect
                RECT overlayRect;
                GetWindowRect(g_omnibox_overlay_hwnd, &overlayRect);

                // Check if click is outside overlay bounds
                if (!PtInRect(&overlayRect, clickPoint)) {
                    LOG_DEBUG("🖱️ Click detected outside omnibox overlay bounds - dismissing");
                    extern void HideOmniboxOverlay();
                    HideOmniboxOverlay();
                }
            }
        }
    }
    return CallNextHookEx(g_omnibox_mouse_hook, nCode, wParam, lParam);
}

// Omnibox Overlay Window Procedure
LRESULT CALLBACK OmniboxOverlayWndProc(HWND hwnd, UINT msg, WPARAM wParam, LPARAM lParam) {
    switch (msg) {
        case WM_MOUSEACTIVATE:
            LOG_DEBUG("👆 Omnibox Overlay HWND received WM_MOUSEACTIVATE");
            // CRITICAL: Return MA_NOACTIVATE to prevent focus theft from address bar
            return MA_NOACTIVATE;

        case WM_SETCURSOR: {
            // Force hand cursor for omnibox overlay (all content is clickable)
            SetCursor(LoadCursor(nullptr, IDC_HAND));
            return TRUE;
        }

        case WM_MOUSEMOVE: {
            // Forward mouse moves to CEF for hover states
            POINT pt = { GET_X_LPARAM(lParam), GET_Y_LPARAM(lParam) };
            CefMouseEvent mouse_event;
            mouse_event.x = pt.x;
            mouse_event.y = pt.y;
            mouse_event.modifiers = 0;

            CefRefPtr<CefBrowser> omnibox_browser = SimpleHandler::GetOmniboxBrowser();
            if (omnibox_browser) {
                omnibox_browser->GetHost()->SendMouseMoveEvent(mouse_event, false);
            }
            return 0;
        }

        case WM_LBUTTONDOWN: {
            LOG_DEBUG("🖱️ Omnibox Overlay received WM_LBUTTONDOWN");
            POINT pt = { GET_X_LPARAM(lParam), GET_Y_LPARAM(lParam) };
            CefMouseEvent mouse_event;
            mouse_event.x = pt.x;
            mouse_event.y = pt.y;
            mouse_event.modifiers = 0;

            // Forward clicks to omnibox browser
            CefRefPtr<CefBrowser> omnibox_browser = SimpleHandler::GetOmniboxBrowser();
            if (omnibox_browser) {
                omnibox_browser->GetHost()->SendMouseClickEvent(mouse_event, MBT_LEFT, false, 1);
                omnibox_browser->GetHost()->SendMouseClickEvent(mouse_event, MBT_LEFT, true, 1);
                LOG_DEBUG("🧠 Left-click sent to omnibox overlay browser");
            }
            return 0;
        }

        case WM_CLOSE:
            LOG_DEBUG("❌ Omnibox Overlay received WM_CLOSE - hiding window");
            // Keep-alive: hide instead of destroy
            ShowWindow(hwnd, SW_HIDE);
            return 0;

        case WM_DESTROY:
            LOG_DEBUG("❌ Omnibox Overlay received WM_DESTROY - cleaning up");
            // No cleanup - window persists
            return 0;

        case WM_WINDOWPOSCHANGING:
            // Allow normal z-order changes for better window management
            break;
    }
    return DefWindowProc(hwnd, msg, wParam, lParam);
}

// Settings Panel Mouse Hook for click-outside detection
LRESULT CALLBACK SettingsPanelMouseHookProc(int nCode, WPARAM wParam, LPARAM lParam) {
    if (nCode == HC_ACTION) {
        if (wParam == WM_LBUTTONDOWN || wParam == WM_RBUTTONDOWN) {
            if (g_settings_overlay_hwnd && IsWindow(g_settings_overlay_hwnd) && IsWindowVisible(g_settings_overlay_hwnd)) {
                MSLLHOOKSTRUCT* mouseInfo = (MSLLHOOKSTRUCT*)lParam;
                POINT clickPoint = mouseInfo->pt;

                RECT overlayRect;
                GetWindowRect(g_settings_overlay_hwnd, &overlayRect);

                if (!PtInRect(&overlayRect, clickPoint)) {
                    LOG_DEBUG("🖱️ Click detected outside settings panel - closing");
                    // Close the browser and destroy window
                    CefRefPtr<CefBrowser> settings_browser = SimpleHandler::GetSettingsBrowser();
                    if (settings_browser) {
                        settings_browser->GetHost()->CloseBrowser(false);
                    }
                    // Remove hook before destroying
                    if (g_settings_mouse_hook) {
                        UnhookWindowsHookEx(g_settings_mouse_hook);
                        g_settings_mouse_hook = nullptr;
                    }
                    DestroyWindow(g_settings_overlay_hwnd);
                    g_settings_overlay_hwnd = nullptr;
                }
            }
        }
    }
    return CallNextHookEx(g_settings_mouse_hook, nCode, wParam, lParam);
}

// Cookie Panel Mouse Hook for click-outside detection
LRESULT CALLBACK CookiePanelMouseHookProc(int nCode, WPARAM wParam, LPARAM lParam) {
    if (nCode == HC_ACTION) {
        // Check for mouse down events (left or right button)
        if (wParam == WM_LBUTTONDOWN || wParam == WM_RBUTTONDOWN) {
            // Only process if cookie panel overlay is visible
            if (g_cookie_panel_overlay_hwnd && IsWindow(g_cookie_panel_overlay_hwnd) && IsWindowVisible(g_cookie_panel_overlay_hwnd)) {
                MSLLHOOKSTRUCT* mouseInfo = (MSLLHOOKSTRUCT*)lParam;
                POINT clickPoint = mouseInfo->pt;

                // Get overlay window rect
                RECT overlayRect;
                GetWindowRect(g_cookie_panel_overlay_hwnd, &overlayRect);

                // Check if click is outside overlay bounds
                if (!PtInRect(&overlayRect, clickPoint)) {
                    LOG_DEBUG("🖱️ Click detected outside cookie panel overlay bounds - dismissing");
                    extern void HideCookiePanelOverlay();
                    HideCookiePanelOverlay();
                }
            }
        }
    }
    return CallNextHookEx(g_cookie_panel_mouse_hook, nCode, wParam, lParam);
}

// Cookie Panel Overlay Window Procedure
LRESULT CALLBACK CookiePanelOverlayWndProc(HWND hwnd, UINT msg, WPARAM wParam, LPARAM lParam) {
    switch (msg) {
        case WM_MOUSEACTIVATE:
            LOG_DEBUG("👆 Cookie Panel Overlay HWND received WM_MOUSEACTIVATE");
            // CRITICAL: Return MA_NOACTIVATE to prevent focus theft
            return MA_NOACTIVATE;

        case WM_SETCURSOR: {
            // Default arrow cursor for cookie panel (has both clickable and scrollable areas)
            SetCursor(LoadCursor(nullptr, IDC_ARROW));
            return TRUE;
        }

        case WM_MOUSEMOVE: {
            // Forward mouse moves to CEF for hover states
            POINT pt = { GET_X_LPARAM(lParam), GET_Y_LPARAM(lParam) };
            CefMouseEvent mouse_event;
            mouse_event.x = pt.x;
            mouse_event.y = pt.y;
            mouse_event.modifiers = 0;

            CefRefPtr<CefBrowser> cookie_browser = SimpleHandler::GetCookiePanelBrowser();
            if (cookie_browser) {
                cookie_browser->GetHost()->SendMouseMoveEvent(mouse_event, false);
            }
            return 0;
        }

        case WM_LBUTTONDOWN: {
            LOG_DEBUG("🖱️ Cookie Panel Overlay received WM_LBUTTONDOWN");
            POINT pt = { GET_X_LPARAM(lParam), GET_Y_LPARAM(lParam) };
            CefMouseEvent mouse_event;
            mouse_event.x = pt.x;
            mouse_event.y = pt.y;
            mouse_event.modifiers = 0;

            // Forward clicks to cookie panel browser
            CefRefPtr<CefBrowser> cookie_browser = SimpleHandler::GetCookiePanelBrowser();
            if (cookie_browser) {
                cookie_browser->GetHost()->SendMouseClickEvent(mouse_event, MBT_LEFT, false, 1);
                cookie_browser->GetHost()->SendMouseClickEvent(mouse_event, MBT_LEFT, true, 1);
                LOG_DEBUG("🧠 Left-click sent to cookie panel overlay browser");
            }
            return 0;
        }

        case WM_MOUSEWHEEL: {
            // Forward mouse wheel events for scrolling
            POINT pt = { GET_X_LPARAM(lParam), GET_Y_LPARAM(lParam) };
            int delta = GET_WHEEL_DELTA_WPARAM(wParam);

            CefMouseEvent mouse_event;
            mouse_event.x = pt.x;
            mouse_event.y = pt.y;
            mouse_event.modifiers = 0;

            CefRefPtr<CefBrowser> cookie_browser = SimpleHandler::GetCookiePanelBrowser();
            if (cookie_browser) {
                cookie_browser->GetHost()->SendMouseWheelEvent(mouse_event, 0, delta);
            }
            return 0;
        }

        case WM_CLOSE:
            LOG_DEBUG("❌ Cookie Panel Overlay received WM_CLOSE - hiding window");
            // Keep-alive: hide instead of destroy
            ShowWindow(hwnd, SW_HIDE);
            return 0;

        case WM_DESTROY:
            LOG_DEBUG("❌ Cookie Panel Overlay received WM_DESTROY - cleaning up");
            // No cleanup - window persists
            return 0;

        case WM_WINDOWPOSCHANGING:
            // Allow normal z-order changes for better window management
            break;
    }
    return DefWindowProc(hwnd, msg, wParam, lParam);
}

// Lightweight health check — returns true if GET /health responds with "ok".
// Uses a short 2-second timeout so it fails fast when nothing is listening.
static bool QuickHealthCheck() {
    HINTERNET hSession = WinHttpOpen(L"HodosBrowser/HealthCheck",
        WINHTTP_ACCESS_TYPE_DEFAULT_PROXY,
        WINHTTP_NO_PROXY_NAME, WINHTTP_NO_PROXY_BYPASS, 0);
    if (!hSession) return false;

    // Set connect + receive timeouts to 2 seconds
    DWORD timeout = 2000;
    WinHttpSetOption(hSession, WINHTTP_OPTION_CONNECT_TIMEOUT, &timeout, sizeof(timeout));
    WinHttpSetOption(hSession, WINHTTP_OPTION_RECEIVE_TIMEOUT, &timeout, sizeof(timeout));
    WinHttpSetOption(hSession, WINHTTP_OPTION_SEND_TIMEOUT, &timeout, sizeof(timeout));

    HINTERNET hConnect = WinHttpConnect(hSession, L"localhost", 3301, 0);
    if (!hConnect) { WinHttpCloseHandle(hSession); return false; }

    HINTERNET hRequest = WinHttpOpenRequest(hConnect, L"GET", L"/health",
        nullptr, WINHTTP_NO_REFERER, WINHTTP_DEFAULT_ACCEPT_TYPES, 0);
    if (!hRequest) { WinHttpCloseHandle(hConnect); WinHttpCloseHandle(hSession); return false; }

    BOOL ok = WinHttpSendRequest(hRequest, WINHTTP_NO_ADDITIONAL_HEADERS, 0,
        WINHTTP_NO_REQUEST_DATA, 0, 0, 0);
    if (!ok) { WinHttpCloseHandle(hRequest); WinHttpCloseHandle(hConnect); WinHttpCloseHandle(hSession); return false; }

    ok = WinHttpReceiveResponse(hRequest, nullptr);
    bool healthy = false;
    if (ok) {
        // Read a small response to check for "ok"
        DWORD dwSize = 0;
        WinHttpQueryDataAvailable(hRequest, &dwSize);
        if (dwSize > 0 && dwSize < 4096) {
            std::vector<char> buf(dwSize + 1, 0);
            DWORD dwRead = 0;
            WinHttpReadData(hRequest, buf.data(), dwSize, &dwRead);
            std::string body(buf.data(), dwRead);
            healthy = (body.find("\"ok\"") != std::string::npos);
        }
    }

    WinHttpCloseHandle(hRequest);
    WinHttpCloseHandle(hConnect);
    WinHttpCloseHandle(hSession);
    return healthy;
}

// Start the Rust wallet server as a subprocess (or detect if already running)
void StartWalletServer() {
    // First check if wallet server is already running (dev workflow: cargo run separately)
    if (QuickHealthCheck()) {
        LOG_INFO("Wallet server already running (dev mode) - skipping launch");
        g_walletServerRunning = true;
        return;
    }

    // Resolve exe path relative to browser executable
    char exePath[MAX_PATH];
    GetModuleFileNameA(nullptr, exePath, MAX_PATH);
    std::string exeDir(exePath);
    size_t lastSlash = exeDir.find_last_of("\\/");
    if (lastSlash != std::string::npos) {
        exeDir = exeDir.substr(0, lastSlash);
    }
    std::string walletExe = exeDir + "\\..\\..\\..\\..\\rust-wallet\\target\\release\\hodos-wallet.exe";

    // Check if the exe exists
    if (GetFileAttributesA(walletExe.c_str()) == INVALID_FILE_ATTRIBUTES) {
        LOG_WARNING("Wallet server executable not found: " + walletExe);
        LOG_WARNING("Browser will run without auto-launched wallet server");
        return;
    }

    LOG_INFO("Launching wallet server: " + walletExe);

    STARTUPINFOA si;
    ZeroMemory(&si, sizeof(si));
    si.cb = sizeof(si);
    si.dwFlags = STARTF_USESHOWWINDOW;
    si.wShowWindow = SW_HIDE;

    ZeroMemory(&g_walletServerProcess, sizeof(PROCESS_INFORMATION));

    if (!CreateProcessA(
        walletExe.c_str(),
        nullptr,
        nullptr,
        nullptr,
        FALSE,
        CREATE_NO_WINDOW,
        nullptr,
        nullptr,
        &si,
        &g_walletServerProcess)) {
        LOG_ERROR("Failed to launch wallet server. Error: " + std::to_string(GetLastError()));
        return;
    }

    LOG_INFO("Wallet server process created (PID: " + std::to_string(g_walletServerProcess.dwProcessId) + ")");

    // Assign to a Job Object so the child is auto-killed when the browser exits
    // (covers crashes, Task Manager kills, and any exit path we miss)
    g_walletJobObject = CreateJobObject(nullptr, nullptr);
    if (g_walletJobObject) {
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION jobInfo = {};
        jobInfo.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        SetInformationJobObject(g_walletJobObject, JobObjectExtendedLimitInformation,
            &jobInfo, sizeof(jobInfo));
        AssignProcessToJobObject(g_walletJobObject, g_walletServerProcess.hProcess);
        LOG_INFO("Wallet server assigned to job object (auto-kill on browser exit)");
    }

    // Poll for health — max 10 attempts, 500ms apart (5 seconds total)
    for (int i = 0; i < 10; i++) {
        Sleep(500);
        if (QuickHealthCheck()) {
            LOG_INFO("Wallet server is healthy after " + std::to_string((i + 1) * 500) + "ms");
            g_walletServerRunning = true;
            return;
        }
    }

    LOG_WARNING("Wallet server did not become healthy within 5 seconds - continuing anyway");
    g_walletServerRunning = true;  // Process was launched, just slow to start
}

// Stop the Rust wallet server subprocess
void StopWalletServer() {
    if (!g_walletServerRunning) return;

    if (g_walletServerProcess.hProcess) {
        LOG_INFO("Terminating wallet server (PID: " + std::to_string(g_walletServerProcess.dwProcessId) + ")");
        TerminateProcess(g_walletServerProcess.hProcess, 0);
        WaitForSingleObject(g_walletServerProcess.hProcess, 3000);
        CloseHandle(g_walletServerProcess.hProcess);
        CloseHandle(g_walletServerProcess.hThread);
        ZeroMemory(&g_walletServerProcess, sizeof(PROCESS_INFORMATION));
    }

    if (g_walletJobObject) {
        CloseHandle(g_walletJobObject);
        g_walletJobObject = nullptr;
    }

    g_walletServerRunning = false;
    LOG_INFO("Wallet server stopped");
}

int WINAPI WinMain(HINSTANCE hInstance, HINSTANCE, LPSTR, int nCmdShow) {
    g_hInstance = hInstance;
    CefMainArgs main_args(hInstance);
    CefRefPtr<SimpleApp> app(new SimpleApp());

    int exit_code = CefExecuteProcess(main_args, app, nullptr);
    if (exit_code >= 0) return exit_code;

    // Initialize centralized logger FIRST
    Logger::Initialize(ProcessType::MAIN, "debug_output.log");

    LOG_INFO("=== NEW SESSION STARTED ===");
    LOG_INFO("Shell starting...");

    // Redirect stdout and stderr to debug_output.log as backup
    FILE* dummy;
    errno_t result1 = freopen_s(&dummy, "debug_output.log", "a", stdout);
    errno_t result2 = freopen_s(&dummy, "debug_output.log", "a", stderr);

    if (result1 != 0 || result2 != 0) {
        LOG_WARNING("freopen failed - stdout: " + std::to_string(result1) + ", stderr: " + std::to_string(result2));
    } else {
        LOG_INFO("stdout/stderr successfully redirected to debug_output.log");
    }

    CefSettings settings;
    settings.command_line_args_disabled = false;
    CefString(&settings.log_file).FromASCII("debug.log");
    settings.log_severity = LOGSEVERITY_INFO;
    settings.remote_debugging_port = 9222;
    settings.windowless_rendering_enabled = true;

    // Set root cache path for browser data (history, cookies, etc.)
    std::string appdata_path = std::getenv("APPDATA") ? std::getenv("APPDATA") : "";
    std::string user_data_path = appdata_path + "\\HodosBrowser";
    CefString(&settings.root_cache_path).FromString(user_data_path);
    CefString(&settings.cache_path).FromString(user_data_path + "\\Default");

    // Persist session cookies across browser restarts
    // TEMPORARILY DISABLED - testing if this causes extra windows
    // settings.persist_session_cookies = true;

    // Enable CEF's runtime API for JavaScript communication
    CefString(&settings.javascript_flags).FromASCII("--expose-gc");

    // Get the executable path for subprocess
    wchar_t exe_path[MAX_PATH];
    GetModuleFileNameW(nullptr, exe_path, MAX_PATH);

    // Set CEF paths - use relative paths from the executable
    CefString(&settings.resources_dir_path).FromWString(L"cef-binaries\\Resources");
    CefString(&settings.locales_dir_path).FromWString(L"cef-binaries\\Resources\\locales");
    CefString(&settings.browser_subprocess_path).FromWString(exe_path);

    RECT rect;
    SystemParametersInfo(SPI_GETWORKAREA, 0, &rect, 0);
    int width  = rect.right - rect.left;
    int height = rect.bottom - rect.top;
    // Header: 12% of parent height, minimum 100px (for tab bar 40px + toolbar 52px)
    int shellHeight = (std::max)(100, static_cast<int>(height * 0.12));
    int webviewHeight = height - shellHeight;

    WNDCLASS wc = {}; wc.lpfnWndProc = ShellWindowProc; wc.hInstance = hInstance;
    wc.lpszClassName = L"HodosBrowserWndClass"; RegisterClass(&wc);

    WNDCLASS browserClass = {};
    browserClass.style = CS_HREDRAW | CS_VREDRAW | CS_DBLCLKS;  // Redraw on resize, no border styles
    browserClass.lpfnWndProc = DefWindowProc;
    browserClass.hInstance = hInstance;
    browserClass.lpszClassName = L"CEFHostWindow";
    browserClass.hbrBackground = (HBRUSH)GetStockObject(BLACK_BRUSH);  // Black background
    RegisterClass(&browserClass);


    WNDCLASS settingsOverlayClass = {};
    settingsOverlayClass.lpfnWndProc = SettingsOverlayWndProc;  // ✅ Settings-specific message handler
    settingsOverlayClass.hInstance = hInstance;
    settingsOverlayClass.lpszClassName = L"CEFSettingsOverlayWindow";

    if (!RegisterClass(&settingsOverlayClass)) {
        LOG_DEBUG("❌ Failed to register settings overlay window class. Error: " + std::to_string(GetLastError()));
    }

    WNDCLASS walletOverlayClass = {};
    walletOverlayClass.lpfnWndProc = WalletOverlayWndProc;  // ✅ Wallet-specific message handler
    walletOverlayClass.hInstance = hInstance;
    walletOverlayClass.lpszClassName = L"CEFWalletOverlayWindow";

    if (!RegisterClass(&walletOverlayClass)) {
        LOG_DEBUG("❌ Failed to register wallet overlay window class. Error: " + std::to_string(GetLastError()));
    }

    // Register backup overlay window class
    WNDCLASS backupOverlayClass = {};
    backupOverlayClass.lpfnWndProc = BackupOverlayWndProc;  // ✅ Backup-specific message handler
    backupOverlayClass.hInstance = hInstance;
    backupOverlayClass.lpszClassName = L"CEFBackupOverlayWindow";

    if (!RegisterClass(&backupOverlayClass)) {
        LOG_DEBUG("❌ Failed to register backup overlay window class. Error: " + std::to_string(GetLastError()));
    }

    // Register BRC-100 auth overlay window class
    WNDCLASS brc100AuthOverlayClass = {};
    brc100AuthOverlayClass.lpfnWndProc = BRC100AuthOverlayWndProc;  // ✅ BRC-100 auth-specific message handler
    brc100AuthOverlayClass.hInstance = hInstance;
    brc100AuthOverlayClass.lpszClassName = L"CEFBRC100AuthOverlayWindow";

    if (!RegisterClass(&brc100AuthOverlayClass)) {
        LOG_DEBUG("❌ Failed to register BRC-100 auth overlay window class. Error: " + std::to_string(GetLastError()));
    }

    // Register Notification overlay window class (full-screen, for all notification types)
    WNDCLASS notificationOverlayClass = {};
    notificationOverlayClass.style = CS_DBLCLKS;
    notificationOverlayClass.lpfnWndProc = NotificationOverlayWndProc;
    notificationOverlayClass.hInstance = hInstance;
    notificationOverlayClass.lpszClassName = L"CEFNotificationOverlayWindow";

    if (!RegisterClass(&notificationOverlayClass)) {
        LOG_DEBUG("Failed to register notification overlay window class. Error: " + std::to_string(GetLastError()));
    }

    // Register Settings Menu overlay window class (small dropdown)
    WNDCLASS settingsMenuOverlayClass = {};
    settingsMenuOverlayClass.lpfnWndProc = SettingsMenuOverlayWndProc;
    settingsMenuOverlayClass.hInstance = hInstance;
    settingsMenuOverlayClass.lpszClassName = L"CEFSettingsMenuOverlayWindow";

    if (!RegisterClass(&settingsMenuOverlayClass)) {
        LOG_DEBUG("❌ Failed to register settings menu overlay window class. Error: " + std::to_string(GetLastError()));
    }

    // Register Omnibox overlay window class
    WNDCLASS omniboxOverlayClass = {};
    omniboxOverlayClass.lpfnWndProc = OmniboxOverlayWndProc;
    omniboxOverlayClass.hInstance = hInstance;
    omniboxOverlayClass.lpszClassName = L"CEFOmniboxOverlayWindow";

    if (!RegisterClass(&omniboxOverlayClass)) {
        LOG_DEBUG("❌ Failed to register omnibox overlay window class. Error: " + std::to_string(GetLastError()));
    }

    // Register Cookie Panel overlay window class
    WNDCLASS cookiePanelOverlayClass = {};
    cookiePanelOverlayClass.lpfnWndProc = CookiePanelOverlayWndProc;
    cookiePanelOverlayClass.hInstance = hInstance;
    cookiePanelOverlayClass.lpszClassName = L"CEFCookiePanelOverlayWindow";

    if (!RegisterClass(&cookiePanelOverlayClass)) {
        LOG_DEBUG("❌ Failed to register cookie panel overlay window class. Error: " + std::to_string(GetLastError()));
    }

    HWND hwnd = CreateWindow(L"HodosBrowserWndClass", L"Hodos Browser",
        WS_OVERLAPPEDWINDOW | WS_VISIBLE | WS_CLIPCHILDREN,
        rect.left, rect.top, width, height, nullptr, nullptr, hInstance, nullptr);

    HWND header_hwnd = CreateWindow(L"CEFHostWindow", nullptr,
        WS_CHILD | WS_VISIBLE, 0, 0, width, shellHeight, hwnd, nullptr, hInstance, nullptr);

    // OLD: Single webview window - NO LONGER USED WITH TAB SYSTEM
    // Kept for compatibility but made invisible (tabs now handle content display)
    HWND webview_hwnd = CreateWindow(L"CEFHostWindow", nullptr,
        WS_CHILD, 0, shellHeight, width, webviewHeight, hwnd, nullptr, hInstance, nullptr);
    // Note: Removed WS_VISIBLE - this window was blocking input to tabs!

    // 🌍 Assign to globals
    g_hwnd = hwnd;
    g_header_hwnd = header_hwnd;
    g_webview_hwnd = webview_hwnd;

    ShowWindow(hwnd, SW_SHOW);        UpdateWindow(hwnd);
    ShowWindow(header_hwnd, SW_SHOW); UpdateWindow(header_hwnd);
    // Don't show webview_hwnd - it's no longer used (tabs handle content now)
    // ShowWindow(webview_hwnd, SW_SHOW); UpdateWindow(webview_hwnd);

    LOG_DEBUG("Initializing CEF...");
    bool success = CefInitialize(main_args, settings, app, nullptr);
    LOG_DEBUG("CefInitialize success: " + std::string(success ? "true" : "false"));

    if (!success) return 1;

    // Initialize HistoryManager with cache path (where CEF creates History database)
    LOG_INFO("Initializing HistoryManager...");
    std::string cache_dir = user_data_path + "\\Default";
    if (HistoryManager::GetInstance().Initialize(cache_dir)) {
        LOG_INFO("✅ HistoryManager initialized successfully");
    } else {
        LOG_ERROR("❌ Failed to initialize HistoryManager");
    }

    // Initialize CookieBlockManager with same cache path
    LOG_INFO("Initializing CookieBlockManager...");
    if (CookieBlockManager::GetInstance().Initialize(cache_dir)) {
        LOG_INFO("✅ CookieBlockManager initialized successfully");
    } else {
        LOG_ERROR("❌ Failed to initialize CookieBlockManager");
    }

    // Initialize BookmarkManager with same cache path
    LOG_INFO("Initializing BookmarkManager...");
    if (BookmarkManager::GetInstance().Initialize(cache_dir)) {
        LOG_INFO("BookmarkManager initialized successfully");
    } else {
        LOG_ERROR("Failed to initialize BookmarkManager");
    }

    // Start wallet server (auto-launch or detect already running)
    LOG_INFO("Starting wallet server...");
    StartWalletServer();

    // 💡 Optionally pass handles to app instance
    app->SetWindowHandles(hwnd, header_hwnd, webview_hwnd);

    CefRunMessageLoop();

    // Stop wallet server before CEF shutdown
    LOG_INFO("Stopping wallet server...");
    StopWalletServer();

    CefShutdown();
    return 0;
}
