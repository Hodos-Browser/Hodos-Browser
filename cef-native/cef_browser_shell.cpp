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
#include "include/cef_task.h"
#include "include/base/cef_callback.h"
#include "include/wrapper/cef_closure_task.h"
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
#include "include/core/SettingsManager.h"
#include "include/core/FingerprintProtection.h"
#include "include/core/ProfileManager.h"
#include "include/core/ProfileLock.h"
#include "include/core/SingleInstance.h"
#include "include/core/AdblockCache.h"
#include "include/core/WindowManager.h"
#include "include/core/AutoUpdater.h"
#include "include/core/LayoutHelpers.h"
#include "include/core/Logger.h"
#include <shellapi.h>
#include <winsock2.h>
#include <ws2tcpip.h>
#pragma comment(lib, "ws2_32.lib")
#include <windows.h>
#include <imm.h>      // For IME message handling (ISC_SHOWUICOMPOSITIONWINDOW)
#pragma comment(lib, "imm32.lib")
#include <algorithm>  // For std::max
#include <windowsx.h>
#include <dwmapi.h>
#include <filesystem>
#include <iostream>
#include <fstream>
#include <chrono>
#include <iomanip>
#include <thread>
#include <atomic>
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
HWND g_download_panel_overlay_hwnd = nullptr;
HWND g_profile_panel_overlay_hwnd = nullptr;
HWND g_notification_overlay_hwnd = nullptr;
HWND g_menu_overlay_hwnd = nullptr;

// File dialog guard — prevents overlay close when a native file dialog is open
bool g_file_dialog_active = false;

// Wallet close prevention — prevents overlay close during mnemonic display / PIN creation
bool g_wallet_overlay_prevent_close = false;

// Timestamps of last hide — used to suppress toggle race condition
// (WM_ACTIVATE hides overlay before toggle IPC arrives, causing re-open)
ULONGLONG g_wallet_last_hide_tick = 0;
ULONGLONG g_profile_last_hide_tick = 0;

// Global mouse hooks for overlay click-outside detection
HHOOK g_omnibox_mouse_hook = nullptr;
HHOOK g_cookie_panel_mouse_hook = nullptr;
HHOOK g_download_panel_mouse_hook = nullptr;
HHOOK g_profile_panel_mouse_hook = nullptr;
HHOOK g_settings_mouse_hook = nullptr;
HHOOK g_menu_mouse_hook = nullptr;

// Stored icon right offsets for repositioning overlays on WM_SIZE/WM_MOVE
// (physical pixel distance from icon's right edge to header's right edge)
int g_settings_icon_right_offset = 0;
int g_cookie_icon_right_offset = 0;
int g_download_icon_right_offset = 0;
int g_profile_icon_right_offset = 0;
int g_wallet_icon_right_offset = 0;
int g_menu_icon_right_offset = 0;
int g_peerpay_count = 0;
int g_peerpay_amount = 0;

// Fullscreen state tracking
bool g_is_fullscreen = false;

// Shutdown state: set when app is shutting down, checked by OnBeforeClose
// to call PostQuitMessage only after all browsers have fully closed.
bool g_app_shutting_down = false;

// Startup: window is hidden until header browser loads (smooth startup)
bool g_window_shown = false;

// Wallet server process management
PROCESS_INFORMATION g_walletServerProcess = {};
std::atomic<bool> g_walletServerRunning{false};
HANDLE g_walletJobObject = nullptr;  // Job object: auto-kills child when parent exits
static bool g_walletProcessLaunched = false;  // Set by LaunchWalletProcess

// Forward declarations for wallet server management (two-phase startup)
void LaunchWalletProcess();   // Phase 1: CreateProcess only (non-blocking)
void WaitForWalletHealth();   // Phase 2: Poll /health with exponential backoff
void StopWalletServer();

// Forward declaration for wallet overlay keep-alive (defined in simple_app.cpp)
void HideWalletOverlay();

// Adblock server process management
PROCESS_INFORMATION g_adblockServerProcess = {};
std::atomic<bool> g_adblockServerRunning{false};
HANDLE g_adblockJobObject = nullptr;
static bool g_adblockProcessLaunched = false;  // Set by LaunchAdblockProcess

// Forward declarations for adblock server management (two-phase startup)
void LaunchAdblockProcess();   // Phase 1: CreateProcess only (non-blocking)
void WaitForAdblockHealth();   // Phase 2: Poll /health with exponential backoff
void StopAdblockServer();

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
        // Restore normal layout with resize border inset (same as WM_SIZE)
        const int rb = 5; // resize border — must match WM_NCHITTEST/WM_SIZE
        int shellHeight = GetHeaderHeightPx(g_hwnd);
        int contentWidth = width - 2 * rb;
        int webviewHeight = height - shellHeight - 2 * rb;

        if (g_header_hwnd && IsWindow(g_header_hwnd)) {
            SetWindowPos(g_header_hwnd, nullptr, rb, rb, contentWidth, shellHeight,
                SWP_NOZORDER | SWP_NOACTIVATE);
            CefRefPtr<CefBrowser> header_browser = SimpleHandler::GetHeaderBrowser();
            if (header_browser) {
                HWND header_cef_hwnd = header_browser->GetHost()->GetWindowHandle();
                if (header_cef_hwnd && IsWindow(header_cef_hwnd)) {
                    SetWindowPos(header_cef_hwnd, nullptr, 0, 0, contentWidth, shellHeight,
                        SWP_NOZORDER | SWP_NOACTIVATE);
                    header_browser->GetHost()->WasResized();
                }
            }
        }
        // Restore all tab windows below header
        std::vector<Tab*> tabs = TabManager::GetInstance().GetAllTabs();
        for (Tab* tab : tabs) {
            if (tab && tab->hwnd && IsWindow(tab->hwnd)) {
                SetWindowPos(tab->hwnd, nullptr, rb, rb + shellHeight, contentWidth, webviewHeight,
                            SWP_NOZORDER | SWP_NOACTIVATE);
                if (tab->browser) {
                    HWND cef_hwnd = tab->browser->GetHost()->GetWindowHandle();
                    if (cef_hwnd && IsWindow(cef_hwnd)) {
                        SetWindowPos(cef_hwnd, nullptr, 0, 0, contentWidth, webviewHeight,
                                    SWP_NOZORDER | SWP_NOACTIVATE);
                        tab->browser->GetHost()->WasResized();
                    }
                }
            }
        }
    }
}

// Save current session tabs to session.json (called before browsers are closed)
// Version 2 format supports multi-window layout.
void SaveSession() {
    // Only save if session restore is enabled
    auto browserSettings = SettingsManager::GetInstance().GetBrowserSettings();
    if (!browserSettings.restoreSessionOnStart) {
        LOG_INFO("📋 Session restore disabled — skipping session save");
        return;
    }

    std::string profilePath = ProfileManager::GetInstance().GetCurrentProfileDataPath();
    if (profilePath.empty()) {
        LOG_WARNING("📋 No profile path — cannot save session");
        return;
    }

    std::vector<Tab*> allTabs = TabManager::GetInstance().GetAllTabs();
    int activeTabId = TabManager::GetInstance().GetActiveTabId();
    std::vector<BrowserWindow*> windows = WindowManager::GetInstance().GetAllWindows();

    nlohmann::json sessionJson;
    sessionJson["version"] = 2;
    sessionJson["windows"] = nlohmann::json::array();

    int totalSavedTabs = 0;

    for (BrowserWindow* bw : windows) {
        if (!bw) continue;

        nlohmann::json winJson;
        winJson["tabs"] = nlohmann::json::array();
        winJson["activeTabIndex"] = 0;

        // Save window position/size
#ifdef _WIN32
        if (bw->hwnd && IsWindow(bw->hwnd)) {
            RECT wr;
            GetWindowRect(bw->hwnd, &wr);
            winJson["x"] = static_cast<int>(wr.left);
            winJson["y"] = static_cast<int>(wr.top);
            winJson["width"] = static_cast<int>(wr.right - wr.left);
            winJson["height"] = static_cast<int>(wr.bottom - wr.top);
        }
#endif

        int tabIndex = 0;
        int activeIndex = 0;
        for (Tab* tab : allTabs) {
            if (!tab || tab->window_id != bw->window_id) continue;

            std::string url = tab->url;
            // Filter out internal URLs — don't save NTP, about:blank, or empty
            if (url.empty() || url == "about:blank") continue;
            if (url.find("127.0.0.1:5137") != std::string::npos) continue;

            nlohmann::json tabEntry;
            tabEntry["url"] = url;
            tabEntry["title"] = tab->title;
            winJson["tabs"].push_back(tabEntry);

            if (tab->id == activeTabId) {
                activeIndex = tabIndex;
            }
            tabIndex++;
        }

        winJson["activeTabIndex"] = activeIndex;

        // Only include windows that have restorable tabs
        if (!winJson["tabs"].empty()) {
            sessionJson["windows"].push_back(winJson);
            totalSavedTabs += static_cast<int>(winJson["tabs"].size());
        }
    }

    // Only save if we have actual tabs to restore
    if (totalSavedTabs == 0) {
        LOG_INFO("📋 No restorable tabs — skipping session save");
        return;
    }

#ifdef _WIN32
    std::string sessionPath = profilePath + "\\session.json";
#else
    std::string sessionPath = profilePath + "/session.json";
#endif

    try {
        std::ofstream out(sessionPath);
        if (out.is_open()) {
            out << sessionJson.dump(2);
            out.close();
            LOG_INFO("📋 Session saved: " + std::to_string(totalSavedTabs) + " tabs across " +
                     std::to_string(sessionJson["windows"].size()) + " windows to " + sessionPath);
        } else {
            LOG_ERROR("📋 Failed to open session.json for writing: " + sessionPath);
        }
    } catch (const std::exception& e) {
        LOG_ERROR("📋 Failed to save session: " + std::string(e.what()));
    }
}

// Clear browsing data on exit (PS3) — called after SaveSession, before browsers close
void ClearBrowsingDataOnExit() {
    auto privacySettings = SettingsManager::GetInstance().GetPrivacySettings();
    if (!privacySettings.clearDataOnExit) {
        return;
    }

    LOG_INFO("🧹 Clear-on-exit enabled — clearing browsing data...");

    // 1. Clear history (synchronous SQLite)
    try {
        LOG_INFO("🧹 Step 1: Clearing history...");
        if (HistoryManager::GetInstance().DeleteAllHistory()) {
            LOG_INFO("🧹 History cleared");
        } else {
            LOG_WARNING("🧹 Failed to clear history (returned false)");
        }
    } catch (const std::exception& e) {
        LOG_ERROR("🧹 History clear exception: " + std::string(e.what()));
    } catch (...) {
        LOG_ERROR("🧹 History clear unknown exception");
    }

    // 2. Clear cookies (async, fire-and-forget — CEF processes pending tasks during shutdown)
    try {
        LOG_INFO("🧹 Step 2: Clearing cookies...");
        CefRefPtr<CefCookieManager> cookieMgr = CefCookieManager::GetGlobalManager(nullptr);
        if (cookieMgr) {
            cookieMgr->DeleteCookies("", "", nullptr);
            LOG_INFO("🧹 Cookie deletion requested");
        } else {
            LOG_WARNING("🧹 No cookie manager available");
        }
    } catch (const std::exception& e) {
        LOG_ERROR("🧹 Cookie clear exception: " + std::string(e.what()));
    } catch (...) {
        LOG_ERROR("🧹 Cookie clear unknown exception");
    }

    // 3. Clear cache via CDP (requires a live browser)
    try {
        LOG_INFO("🧹 Step 3: Clearing cache...");
        CefRefPtr<CefBrowser> header_browser = SimpleHandler::GetHeaderBrowser();
        if (header_browser) {
            header_browser->GetHost()->ExecuteDevToolsMethod(0, "Network.clearBrowserCache", nullptr);
            LOG_INFO("🧹 Cache clear requested via CDP");
        } else {
            LOG_WARNING("🧹 No header browser for cache clear");
        }
    } catch (const std::exception& e) {
        LOG_ERROR("🧹 Cache clear exception: " + std::string(e.what()));
    } catch (...) {
        LOG_ERROR("🧹 Cache clear unknown exception");
    }

    // 4. Clear cookie block log (synchronous SQLite)
    try {
        LOG_INFO("🧹 Step 4: Clearing cookie block log...");
        if (CookieBlockManager::GetInstance().ClearBlockLog()) {
            LOG_INFO("🧹 Cookie block log cleared");
        }
    } catch (const std::exception& e) {
        LOG_ERROR("🧹 Block log clear exception: " + std::string(e.what()));
    } catch (...) {
        LOG_ERROR("🧹 Block log clear unknown exception");
    }

    // 5. Delete session.json so stale sessions aren't restored (G2 conflict resolution)
    try {
        LOG_INFO("🧹 Step 5: Deleting session.json...");
        std::string profilePath = ProfileManager::GetInstance().GetCurrentProfileDataPath();
        if (!profilePath.empty()) {
#ifdef _WIN32
            std::string sessionPath = profilePath + "\\session.json";
#else
            std::string sessionPath = profilePath + "/session.json";
#endif
            if (std::filesystem::exists(sessionPath)) {
                std::filesystem::remove(sessionPath);
                LOG_INFO("🧹 session.json deleted (clear-on-exit overrides session restore)");
            } else {
                LOG_INFO("🧹 No session.json to delete");
            }
        }
    } catch (const std::exception& e) {
        LOG_ERROR("🧹 Session delete exception: " + std::string(e.what()));
    } catch (...) {
        LOG_ERROR("🧹 Session delete unknown exception");
    }

    LOG_INFO("🧹 Clear-on-exit complete");
}

// Graceful shutdown function
void ShutdownApplication() {
    // Guard against double-entry. OnTabBrowserClosed posts WM_CLOSE when no tabs
    // remain, which can re-trigger ShutdownApplication while the first call is
    // still closing browsers. The second call would find nothing to close but
    // would prevent CefRunMessageLoop from exiting cleanly.
    static bool s_shutdown_started = false;
    if (s_shutdown_started) {
        LOG_WARNING("🛑 ShutdownApplication already in progress — skipping duplicate call");
        return;
    }
    s_shutdown_started = true;

    LOG_INFO("🛑 Starting graceful application shutdown...");

    // B-6: Signal the single-instance pipe listener to respond "shutting_down"
    // to any new clients that connect during the shutdown window.
    SingleInstance::SetShuttingDown();

    // Step 0: Clean up auto-updater (cancels any pending operations)
    AutoUpdater::GetInstance().Cleanup();

    // Step 0a: Save session tabs before anything is closed
    SaveSession();

    // Step 0b: Clear browsing data if "clear on exit" is enabled (PS3)
    ClearBrowsingDataOnExit();

    // Step 0c: Mute ALL browsers immediately to stop audio
    LOG_INFO("🔇 Muting all browsers...");
    {
        std::vector<Tab*> allTabs = TabManager::GetInstance().GetAllTabs();
        for (Tab* tab : allTabs) {
            if (tab && tab->browser) {
                tab->browser->GetHost()->SetAudioMuted(true);
            }
        }
        std::vector<BrowserWindow*> allWindows = WindowManager::GetInstance().GetAllWindows();
        const std::string muteRoles[] = {
            "header", "webview", "wallet_panel", "overlay", "settings",
            "wallet", "backup", "brc100auth", "notification", "settings_menu",
            "omnibox", "cookiepanel", "downloadpanel", "profilepanel", "menu"
        };
        for (BrowserWindow* bw : allWindows) {
            if (!bw) continue;
            for (const auto& role : muteRoles) {
                CefRefPtr<CefBrowser> b = bw->GetBrowserForRole(role);
                if (b) b->GetHost()->SetAudioMuted(true);
            }
        }
    }

    // Step 0d: Close wallet-facing overlay browsers first (they talk to wallet server)
    LOG_INFO("🔄 Closing wallet-facing overlays...");
    {
        std::vector<BrowserWindow*> allWindows = WindowManager::GetInstance().GetAllWindows();
        for (BrowserWindow* bw : allWindows) {
            if (!bw) continue;
            for (const auto& role : {"wallet", "backup", "brc100auth", "wallet_panel"}) {
                CefRefPtr<CefBrowser> b = bw->GetBrowserForRole(std::string(role));
                if (b) {
                    LOG_INFO("🔄 Closing wallet-facing browser: " + std::string(role) + " (window " + std::to_string(bw->window_id) + ")");
                    b->GetHost()->CloseBrowser(true);
                }
            }
        }
    }

    // Step 0e: Stop child servers in parallel (saves ~5s vs sequential)
    LOG_INFO("🔄 Stopping wallet + adblock servers in parallel...");
    {
        std::thread walletThread(StopWalletServer);
        std::thread adblockThread(StopAdblockServer);
        walletThread.join();
        adblockThread.join();
    }
    LOG_INFO("🔄 Both servers stopped.");

    // Step 1: Force-close ALL CEF browsers (tabs, overlays, header)
    // Using CloseBrowser(true) = force close, skips beforeunload handlers.
    // All browsers must be closed before CefShutdown() or it hangs waiting
    // for renderer processes that were never told to exit.
    LOG_INFO("🔄 Closing all CEF browsers...");

    // 1a: Close all tab browsers (each tab has its own CefBrowser + renderer process)
    {
        std::vector<Tab*> allTabs = TabManager::GetInstance().GetAllTabs();
        LOG_INFO("🔄 Closing " + std::to_string(allTabs.size()) + " tab browser(s)...");
        for (Tab* tab : allTabs) {
            if (tab && tab->browser) {
                tab->browser->GetHost()->CloseBrowser(true);
            }
        }
    }

    // 1b: Close all overlay and window browsers via BrowserWindow refs
    // (wallet-facing overlays already closed in step 0d, but CloseBrowser(true) is safe to call twice)
    {
        std::vector<BrowserWindow*> allWindows = WindowManager::GetInstance().GetAllWindows();
        const std::string roles[] = {
            "header", "webview", "wallet_panel", "overlay", "settings",
            "wallet", "backup", "brc100auth", "notification", "settings_menu",
            "omnibox", "cookiepanel", "downloadpanel", "profilepanel", "menu"
        };
        for (BrowserWindow* bw : allWindows) {
            if (!bw) continue;
            for (const auto& role : roles) {
                CefRefPtr<CefBrowser> b = bw->GetBrowserForRole(role);
                if (b) {
                    LOG_INFO("🔄 Closing browser for role: " + role + " (window " + std::to_string(bw->window_id) + ")");
                    b->GetHost()->CloseBrowser(true);
                }
            }
        }
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
        LOG_INFO("Destroying wallet overlay window...");
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

    if (g_settings_menu_overlay_hwnd && IsWindow(g_settings_menu_overlay_hwnd)) {
        LOG_INFO("🔄 Destroying settings menu overlay window...");
        DestroyWindow(g_settings_menu_overlay_hwnd);
        g_settings_menu_overlay_hwnd = nullptr;
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

    if (g_download_panel_overlay_hwnd && IsWindow(g_download_panel_overlay_hwnd)) {
        LOG_INFO("Destroying download panel overlay window...");
        if (g_download_panel_mouse_hook) {
            UnhookWindowsHookEx(g_download_panel_mouse_hook);
            g_download_panel_mouse_hook = nullptr;
            LOG_INFO("Download panel mouse hook removed during shutdown");
        }
        DestroyWindow(g_download_panel_overlay_hwnd);
        g_download_panel_overlay_hwnd = nullptr;
    }

    if (g_profile_panel_overlay_hwnd && IsWindow(g_profile_panel_overlay_hwnd)) {
        LOG_INFO("Destroying profile panel overlay window...");
        if (g_profile_panel_mouse_hook) {
            UnhookWindowsHookEx(g_profile_panel_mouse_hook);
            g_profile_panel_mouse_hook = nullptr;
            LOG_INFO("Profile panel mouse hook removed during shutdown");
        }
        DestroyWindow(g_profile_panel_overlay_hwnd);
        g_profile_panel_overlay_hwnd = nullptr;
    }

    if (g_menu_overlay_hwnd && IsWindow(g_menu_overlay_hwnd)) {
        LOG_INFO("Destroying menu overlay window...");
        if (g_menu_mouse_hook) {
            UnhookWindowsHookEx(g_menu_mouse_hook);
            g_menu_mouse_hook = nullptr;
            LOG_INFO("Menu mouse hook removed during shutdown");
        }
        DestroyWindow(g_menu_overlay_hwnd);
        g_menu_overlay_hwnd = nullptr;
    }

    // B-6: Stop the single-instance pipe listener thread now, inside ShutdownApplication.
    SingleInstance::StopListenerThread();

    // Safety net: force-close any browsers that ShutdownApplication didn't reach.
    // This catches leaked notification overlays from torn-off/B-6 windows whose
    // browser refs aren't tracked in the primary BrowserWindow's overlay slots.
    // Without this, browser_handler_map_ never empties and CefQuitMessageLoop never fires.
    SimpleHandler::ForceCloseRemainingBrowsers();

    // Step 3: DO NOT destroy main shell window here.
    // The message loop must keep running so CEF can process CloseBrowser callbacks.
    // OnBeforeClose will call PostQuitMessage when all browsers have fully closed,
    // and post-message-loop cleanup in main() will handle final window destruction.
    LOG_INFO("✅ ShutdownApplication complete — waiting for browsers to close...");
}

// Hide all visible overlay HWNDs (keep-alive pattern — don't destroy).
// Used during primary window transfer to prevent visual glitches.
void HideAllOverlays() {
    // Use existing Hide functions for keep-alive overlays (they unhook mouse hooks too)
    extern void HideOmniboxOverlay();
    extern void HideCookiePanelOverlay();
    extern void HideDownloadPanelOverlay();
    extern void HideMenuOverlay();
    extern void HideProfilePanelOverlay();

    if (g_omnibox_overlay_hwnd && IsWindow(g_omnibox_overlay_hwnd) && IsWindowVisible(g_omnibox_overlay_hwnd))
        HideOmniboxOverlay();
    if (g_cookie_panel_overlay_hwnd && IsWindow(g_cookie_panel_overlay_hwnd) && IsWindowVisible(g_cookie_panel_overlay_hwnd))
        HideCookiePanelOverlay();
    if (g_download_panel_overlay_hwnd && IsWindow(g_download_panel_overlay_hwnd) && IsWindowVisible(g_download_panel_overlay_hwnd))
        HideDownloadPanelOverlay();
    if (g_menu_overlay_hwnd && IsWindow(g_menu_overlay_hwnd) && IsWindowVisible(g_menu_overlay_hwnd))
        HideMenuOverlay();
    if (g_profile_panel_overlay_hwnd && IsWindow(g_profile_panel_overlay_hwnd) && IsWindowVisible(g_profile_panel_overlay_hwnd))
        HideProfilePanelOverlay();

    // Non-keep-alive overlays: just hide (they'll be repositioned on next show)
    if (g_settings_overlay_hwnd && IsWindow(g_settings_overlay_hwnd) && IsWindowVisible(g_settings_overlay_hwnd)) {
        if (g_settings_mouse_hook) { UnhookWindowsHookEx(g_settings_mouse_hook); g_settings_mouse_hook = nullptr; }
        ShowWindow(g_settings_overlay_hwnd, SW_HIDE);
    }
    if (g_wallet_overlay_hwnd && IsWindow(g_wallet_overlay_hwnd) && IsWindowVisible(g_wallet_overlay_hwnd))
        ShowWindow(g_wallet_overlay_hwnd, SW_HIDE);
    if (g_backup_overlay_hwnd && IsWindow(g_backup_overlay_hwnd) && IsWindowVisible(g_backup_overlay_hwnd))
        ShowWindow(g_backup_overlay_hwnd, SW_HIDE);
    if (g_brc100_auth_overlay_hwnd && IsWindow(g_brc100_auth_overlay_hwnd) && IsWindowVisible(g_brc100_auth_overlay_hwnd))
        ShowWindow(g_brc100_auth_overlay_hwnd, SW_HIDE);
    if (g_notification_overlay_hwnd && IsWindow(g_notification_overlay_hwnd) && IsWindowVisible(g_notification_overlay_hwnd))
        ShowWindow(g_notification_overlay_hwnd, SW_HIDE);
    if (g_settings_menu_overlay_hwnd && IsWindow(g_settings_menu_overlay_hwnd) && IsWindowVisible(g_settings_menu_overlay_hwnd))
        ShowWindow(g_settings_menu_overlay_hwnd, SW_HIDE);
}

// Transfer the "primary window" role from the current primary to a surviving window.
// Called when the primary window's WM_CLOSE fires but other windows still exist.
// After transfer, overlays will reposition relative to the new g_hwnd on next show.
void TransferPrimaryWindow(int newPrimaryId) {
    int oldPrimaryId = WindowManager::GetInstance().GetPrimaryWindowId();
    BrowserWindow* oldWin = WindowManager::GetInstance().GetWindow(oldPrimaryId);
    BrowserWindow* newWin = WindowManager::GetInstance().GetWindow(newPrimaryId);
    if (!newWin) {
        LOG_ERROR("TransferPrimaryWindow: window " + std::to_string(newPrimaryId) + " not found");
        return;
    }

    LOG_INFO("Transferring primary window role from " + std::to_string(oldPrimaryId) +
             " to " + std::to_string(newPrimaryId));

    // 1. Hide all overlays — they'll reposition on next show via the new g_hwnd
    HideAllOverlays();

    // 2. Transfer overlay browser refs and HWNDs from old to new BrowserWindow.
    //    This is critical: ShutdownApplication iterates BrowserWindow refs to close
    //    browsers. Without transfer, overlay browsers become orphaned and the process
    //    never exits (browser_handler_map_ never empties).
    if (oldWin) {
        // Transfer overlay HWNDs
        newWin->settings_overlay_hwnd = oldWin->settings_overlay_hwnd;
        newWin->wallet_overlay_hwnd = oldWin->wallet_overlay_hwnd;
        newWin->backup_overlay_hwnd = oldWin->backup_overlay_hwnd;
        newWin->brc100_auth_overlay_hwnd = oldWin->brc100_auth_overlay_hwnd;
        newWin->notification_overlay_hwnd = oldWin->notification_overlay_hwnd;
        newWin->settings_menu_overlay_hwnd = oldWin->settings_menu_overlay_hwnd;
        newWin->omnibox_overlay_hwnd = oldWin->omnibox_overlay_hwnd;
        newWin->cookie_panel_overlay_hwnd = oldWin->cookie_panel_overlay_hwnd;
        newWin->download_panel_overlay_hwnd = oldWin->download_panel_overlay_hwnd;
        newWin->profile_panel_overlay_hwnd = oldWin->profile_panel_overlay_hwnd;
        newWin->menu_overlay_hwnd = oldWin->menu_overlay_hwnd;

        // Null out old window's overlay HWNDs so they aren't double-freed
        oldWin->settings_overlay_hwnd = nullptr;
        oldWin->wallet_overlay_hwnd = nullptr;
        oldWin->backup_overlay_hwnd = nullptr;
        oldWin->brc100_auth_overlay_hwnd = nullptr;
        oldWin->notification_overlay_hwnd = nullptr;
        oldWin->settings_menu_overlay_hwnd = nullptr;
        oldWin->omnibox_overlay_hwnd = nullptr;
        oldWin->cookie_panel_overlay_hwnd = nullptr;
        oldWin->download_panel_overlay_hwnd = nullptr;
        oldWin->profile_panel_overlay_hwnd = nullptr;
        oldWin->menu_overlay_hwnd = nullptr;

        // Transfer overlay browser refs
        const std::string overlayRoles[] = {
            "wallet_panel", "overlay", "settings", "wallet", "backup",
            "brc100auth", "notification", "settings_menu", "omnibox",
            "cookiepanel", "downloadpanel", "profilepanel", "menu"
        };
        for (const auto& role : overlayRoles) {
            CefRefPtr<CefBrowser> b = oldWin->GetBrowserForRole(role);
            if (b) {
                newWin->SetBrowserForRole(role, b);
                oldWin->ClearBrowserForRole(role);

                // Update overlay handler's window_id so IPC routes correctly
                SimpleHandler* handler = SimpleHandler::GetHandlerForBrowser(b->GetIdentifier());
                if (handler) {
                    handler->SetWindowId(newPrimaryId);
                }
            }
        }

        // Transfer icon offsets
        newWin->settings_icon_right_offset = oldWin->settings_icon_right_offset;
        newWin->cookie_icon_right_offset = oldWin->cookie_icon_right_offset;
        newWin->download_icon_right_offset = oldWin->download_icon_right_offset;
        newWin->profile_icon_right_offset = oldWin->profile_icon_right_offset;
        newWin->wallet_icon_right_offset = oldWin->wallet_icon_right_offset;
        newWin->menu_icon_right_offset = oldWin->menu_icon_right_offset;
    }

    // 3. Reassign global HWNDs to the new primary window
    g_hwnd = newWin->hwnd;
    g_header_hwnd = newWin->header_hwnd;
    g_webview_hwnd = nullptr;  // legacy, unused

    // 4. Update WindowManager's primary window ID
    WindowManager::GetInstance().SetPrimaryWindowId(newPrimaryId);

    LOG_INFO("Primary window transferred to window " + std::to_string(newPrimaryId) +
             " (hwnd=" + std::to_string(reinterpret_cast<uintptr_t>(g_hwnd)) + ")");
}

// Flag: once the header browser has loaded React, stop painting the startup logo
bool g_header_browser_loaded = false;

LRESULT CALLBACK ShellWindowProc(HWND hwnd, UINT msg, WPARAM wParam, LPARAM lParam) {
    switch (msg) {
        case WM_PAINT: {
            // During startup, just paint the dark background. The window class brush
            // handles this via DefWindowProc, but we handle it explicitly here to
            // ensure a clean dark frame appears before CEF loads React content.
            // The header child window (WS_CHILD) covers the top once React renders.
            if (!g_header_browser_loaded) {
                PAINTSTRUCT ps;
                HDC hdc = BeginPaint(hwnd, &ps);
                HBRUSH bgBrush = CreateSolidBrush(RGB(26, 26, 26));
                FillRect(hdc, &ps.rcPaint, bgBrush);
                DeleteObject(bgBrush);
                EndPaint(hwnd, &ps);
                return 0;
            }
            break;
        }
        case WM_MOVE: {
            // Handle window movement - move overlay windows with main window
            RECT mainRect;
            GetWindowRect(hwnd, &mainRect);
            int width = mainRect.right - mainRect.left;
            int height = mainRect.bottom - mainRect.top;

            LOG_DEBUG("🔄 Main window moved to: " + std::to_string(mainRect.left) + ", " + std::to_string(mainRect.top));

            // Overlays only exist on the primary window (window 0).
            // Skip overlay repositioning for secondary windows.
            BrowserWindow* moveBw = reinterpret_cast<BrowserWindow*>(GetWindowLongPtr(hwnd, GWLP_USERDATA));
            if (moveBw && moveBw->window_id != WindowManager::GetInstance().GetPrimaryWindowId()) {
                break;  // Secondary window — no overlays to reposition
            }

            // Move settings overlay if it exists and is visible (right-side popup)
            if (g_settings_overlay_hwnd && IsWindow(g_settings_overlay_hwnd) && IsWindowVisible(g_settings_overlay_hwnd)) {
                RECT headerRect;
                GetWindowRect(g_header_hwnd, &headerRect);
                int panelWidth = ScalePx(450, hwnd);
                int panelHeight = ScalePx(450, hwnd);
                int overlayX = headerRect.right - g_settings_icon_right_offset - panelWidth;
                int overlayY = headerRect.top + ScalePx(104, hwnd);
                if (overlayY + panelHeight > mainRect.bottom) {
                    panelHeight = mainRect.bottom - overlayY;
                    if (panelHeight < ScalePx(200, hwnd)) panelHeight = ScalePx(200, hwnd);
                }
                SetWindowPos(g_settings_overlay_hwnd, HWND_TOPMOST,
                    overlayX, overlayY, panelWidth, panelHeight,
                    SWP_NOACTIVATE | SWP_SHOWWINDOW);
            }

            // Move cookie/privacy shield panel overlay if it exists and is visible (right-side popup)
            if (g_cookie_panel_overlay_hwnd && IsWindow(g_cookie_panel_overlay_hwnd) && IsWindowVisible(g_cookie_panel_overlay_hwnd)) {
                RECT hdrRect;
                GetWindowRect(g_header_hwnd, &hdrRect);
                int cpWidth = ScalePx(450, hwnd);
                int cpHeight = ScalePx(370, hwnd);
                int cpX = hdrRect.right - g_cookie_icon_right_offset - cpWidth;
                int cpY = hdrRect.top + ScalePx(104, hwnd);
                if (cpY + cpHeight > mainRect.bottom) {
                    cpHeight = mainRect.bottom - cpY;
                    if (cpHeight < ScalePx(200, hwnd)) cpHeight = ScalePx(200, hwnd);
                }
                SetWindowPos(g_cookie_panel_overlay_hwnd, HWND_TOPMOST,
                    cpX, cpY, cpWidth, cpHeight,
                    SWP_NOACTIVATE | SWP_SHOWWINDOW);
            }

            // Move download panel overlay if it exists and is visible (right-side popup)
            if (g_download_panel_overlay_hwnd && IsWindow(g_download_panel_overlay_hwnd) && IsWindowVisible(g_download_panel_overlay_hwnd)) {
                RECT hdrRect;
                GetWindowRect(g_header_hwnd, &hdrRect);
                int dpWidth = ScalePx(380, hwnd);
                int dpHeight = ScalePx(400, hwnd);
                int dpX = hdrRect.right - g_download_icon_right_offset - dpWidth;
                int dpY = hdrRect.top + ScalePx(104, hwnd);
                if (dpY + dpHeight > mainRect.bottom) {
                    dpHeight = mainRect.bottom - dpY;
                    if (dpHeight < ScalePx(200, hwnd)) dpHeight = ScalePx(200, hwnd);
                }
                SetWindowPos(g_download_panel_overlay_hwnd, HWND_TOPMOST,
                    dpX, dpY, dpWidth, dpHeight,
                    SWP_NOACTIVATE | SWP_SHOWWINDOW);
            }

            // Move wallet overlay if it exists and is visible (right-side panel below header)
            if (g_wallet_overlay_hwnd && IsWindow(g_wallet_overlay_hwnd) && IsWindowVisible(g_wallet_overlay_hwnd)) {
                RECT hdrRect;
                GetWindowRect(g_header_hwnd, &hdrRect);
                RECT wpClientRect;
                GetClientRect(hwnd, &wpClientRect);
                POINT wpClientBR = { wpClientRect.right, wpClientRect.bottom };
                ClientToScreen(hwnd, &wpClientBR);
                int wpWidth = ScalePx(400, hwnd);
                int wpHeight = wpClientBR.y - hdrRect.bottom;
                int wpX = wpClientBR.x - wpWidth;
                int wpY = hdrRect.bottom;
                SetWindowPos(g_wallet_overlay_hwnd, HWND_TOPMOST,
                    wpX, wpY, wpWidth, wpHeight,
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

            // Resize border for frameless window — child windows are inset to expose
            // the parent window at edges for WM_NCHITTEST resize detection.
            const int rb = 5; // must match WM_NCHITTEST border constant

            // Fixed header height, DPI-scaled (tab bar 42px + toolbar 53px + 1px buffer = 96 CSS px)
            int shellHeight = GetHeaderHeightPx(hwnd);
            int contentWidth = width - 2 * rb;
            int webviewHeight = height - shellHeight - 2 * rb; // rb at top and bottom

            LOG_DEBUG("🔄 Main window resized: " + std::to_string(width) + "x" + std::to_string(height));

            // Get BrowserWindow for this HWND (works for primary + secondary windows)
            BrowserWindow* bw = reinterpret_cast<BrowserWindow*>(GetWindowLongPtr(hwnd, GWLP_USERDATA));
            HWND thisHeaderHwnd = bw ? bw->header_hwnd : g_header_hwnd;

            // Resize header window (inset on all sides for resize border)
            if (thisHeaderHwnd && IsWindow(thisHeaderHwnd)) {
                SetWindowPos(thisHeaderHwnd, nullptr, rb, rb, contentWidth, shellHeight,
                    SWP_NOZORDER | SWP_NOACTIVATE);

                // Resize the CEF browser in the header window
                CefRefPtr<CefBrowser> header_browser = bw ? bw->header_browser : SimpleHandler::GetHeaderBrowser();
                if (header_browser) {
                    HWND header_cef_hwnd = header_browser->GetHost()->GetWindowHandle();
                    if (header_cef_hwnd && IsWindow(header_cef_hwnd)) {
                        SetWindowPos(header_cef_hwnd, nullptr, 0, 0, contentWidth, shellHeight,
                            SWP_NOZORDER | SWP_NOACTIVATE);
                        header_browser->GetHost()->WasResized();
                    }
                }
            }

            // Resize webview window (legacy - will be removed when fully migrated to tabs)
            if (g_webview_hwnd && IsWindow(g_webview_hwnd)) {
                SetWindowPos(g_webview_hwnd, nullptr, rb, rb + shellHeight, contentWidth, webviewHeight,
                    SWP_NOZORDER | SWP_NOACTIVATE);

                // Resize the CEF browser in the webview window
                CefRefPtr<CefBrowser> webview_browser = SimpleHandler::GetWebviewBrowser();
                if (webview_browser) {
                    HWND webview_cef_hwnd = webview_browser->GetHost()->GetWindowHandle();
                    if (webview_cef_hwnd && IsWindow(webview_cef_hwnd)) {
                        SetWindowPos(webview_cef_hwnd, nullptr, 0, 0, contentWidth, webviewHeight,
                            SWP_NOZORDER | SWP_NOACTIVATE);
                        webview_browser->GetHost()->WasResized();
                    }
                }
            }

            // Resize tab windows belonging to this window
            int thisWindowId = bw ? bw->window_id : 0;
            std::vector<Tab*> tabs = TabManager::GetInstance().GetAllTabs();
            for (Tab* tab : tabs) {
                if (tab && tab->window_id == thisWindowId && tab->hwnd && IsWindow(tab->hwnd)) {
                    // Position tab window below header, inset all sides
                    SetWindowPos(tab->hwnd, nullptr, rb, rb + shellHeight, contentWidth, webviewHeight,
                                SWP_NOZORDER | SWP_NOACTIVATE);

                    // Resize tab's CEF browser (fills its parent tab HWND)
                    if (tab->browser) {
                        HWND cef_hwnd = tab->browser->GetHost()->GetWindowHandle();
                        if (cef_hwnd && IsWindow(cef_hwnd)) {
                            SetWindowPos(cef_hwnd, nullptr, 0, 0, contentWidth, webviewHeight,
                                        SWP_NOZORDER | SWP_NOACTIVATE);
                            tab->browser->GetHost()->WasResized();
                        }
                    }
                }
            }

            // Overlays only exist on the primary window (window 0).
            // Skip overlay repositioning for secondary windows.
            if (bw && bw->window_id != WindowManager::GetInstance().GetPrimaryWindowId()) {
                return 0;
            }

            // Resize overlay windows if they exist and are visible
            // Get the new main window screen position for overlays
            RECT mainRect;
            GetWindowRect(hwnd, &mainRect);

            // Reposition settings panel (right-side popup, right edge under icon)
            if (g_settings_overlay_hwnd && IsWindow(g_settings_overlay_hwnd) && IsWindowVisible(g_settings_overlay_hwnd)) {
                RECT headerRect;
                GetWindowRect(g_header_hwnd, &headerRect);
                int panelWidth = ScalePx(450, hwnd);
                int panelHeight = ScalePx(450, hwnd);
                int overlayX = headerRect.right - g_settings_icon_right_offset - panelWidth;
                int overlayY = headerRect.top + ScalePx(104, hwnd);
                if (overlayY + panelHeight > mainRect.bottom) {
                    panelHeight = mainRect.bottom - overlayY;
                    if (panelHeight < ScalePx(200, hwnd)) panelHeight = ScalePx(200, hwnd);
                }
                SetWindowPos(g_settings_overlay_hwnd, HWND_TOPMOST,
                    overlayX, overlayY, panelWidth, panelHeight,
                    SWP_NOACTIVATE | SWP_SHOWWINDOW);

                CefRefPtr<CefBrowser> settings_browser = SimpleHandler::GetSettingsBrowser();
                if (settings_browser) {
                    settings_browser->GetHost()->WasResized();
                }
            }

            // Reposition cookie/privacy shield panel (right-side popup, right edge under icon)
            if (g_cookie_panel_overlay_hwnd && IsWindow(g_cookie_panel_overlay_hwnd) && IsWindowVisible(g_cookie_panel_overlay_hwnd)) {
                RECT hdrRect;
                GetWindowRect(g_header_hwnd, &hdrRect);
                RECT mainWinRect;
                GetWindowRect(g_hwnd, &mainWinRect);
                int cpWidth = ScalePx(450, hwnd);
                int cpHeight = ScalePx(370, hwnd);
                int cpX = hdrRect.right - g_cookie_icon_right_offset - cpWidth;
                int cpY = hdrRect.top + ScalePx(104, hwnd);
                if (cpY + cpHeight > mainWinRect.bottom) {
                    cpHeight = mainWinRect.bottom - cpY;
                    if (cpHeight < ScalePx(200, hwnd)) cpHeight = ScalePx(200, hwnd);
                }
                SetWindowPos(g_cookie_panel_overlay_hwnd, HWND_TOPMOST,
                    cpX, cpY, cpWidth, cpHeight,
                    SWP_NOACTIVATE | SWP_SHOWWINDOW);

                CefRefPtr<CefBrowser> cookie_browser = SimpleHandler::GetCookiePanelBrowser();
                if (cookie_browser) {
                    cookie_browser->GetHost()->WasResized();
                }
            }

            // Reposition download panel (right-side popup, right edge under icon)
            if (g_download_panel_overlay_hwnd && IsWindow(g_download_panel_overlay_hwnd) && IsWindowVisible(g_download_panel_overlay_hwnd)) {
                RECT hdrRect;
                GetWindowRect(g_header_hwnd, &hdrRect);
                RECT dlMainRect;
                GetWindowRect(g_hwnd, &dlMainRect);
                int dpWidth = ScalePx(380, hwnd);
                int dpHeight = ScalePx(400, hwnd);
                int dpX = hdrRect.right - g_download_icon_right_offset - dpWidth;
                int dpY = hdrRect.top + ScalePx(104, hwnd);
                if (dpY + dpHeight > dlMainRect.bottom) {
                    dpHeight = dlMainRect.bottom - dpY;
                    if (dpHeight < ScalePx(200, hwnd)) dpHeight = ScalePx(200, hwnd);
                }
                SetWindowPos(g_download_panel_overlay_hwnd, HWND_TOPMOST,
                    dpX, dpY, dpWidth, dpHeight,
                    SWP_NOACTIVATE | SWP_SHOWWINDOW);

                CefRefPtr<CefBrowser> dl_browser = SimpleHandler::GetDownloadPanelBrowser();
                if (dl_browser) {
                    dl_browser->GetHost()->WasResized();
                }
            }

            // Resize wallet overlay (right-side panel below header)
            if (g_wallet_overlay_hwnd && IsWindow(g_wallet_overlay_hwnd) && IsWindowVisible(g_wallet_overlay_hwnd)) {
                RECT hdrRect;
                GetWindowRect(g_header_hwnd, &hdrRect);
                RECT wpClientRect;
                GetClientRect(hwnd, &wpClientRect);
                POINT wpClientBR = { wpClientRect.right, wpClientRect.bottom };
                ClientToScreen(hwnd, &wpClientBR);
                int wpWidth = ScalePx(400, hwnd);
                int wpHeight = wpClientBR.y - hdrRect.bottom;
                int wpX = wpClientBR.x - wpWidth;
                int wpY = hdrRect.bottom;
                SetWindowPos(g_wallet_overlay_hwnd, HWND_TOPMOST,
                    wpX, wpY, wpWidth, wpHeight,
                    SWP_NOACTIVATE | SWP_SHOWWINDOW);

                CefRefPtr<CefBrowser> wallet_browser = SimpleHandler::GetWalletBrowser();
                if (wallet_browser) {
                    wallet_browser->GetHost()->WasResized();
                    // Push updated dimensions to React
                    std::string js = "window.postMessage({type:'wallet_resize',panelHeight:" +
                        std::to_string(wpHeight) + ",panelWidth:" +
                        std::to_string(wpWidth) + "},'*');";
                    wallet_browser->GetMainFrame()->ExecuteJavaScript(js, "", 0);
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

        case WM_ACTIVATE: {
            // Track which window is active for per-window operations (Ctrl+T, etc.)
            if (LOWORD(wParam) != WA_INACTIVE) {
                BrowserWindow* activatedBw = reinterpret_cast<BrowserWindow*>(GetWindowLongPtr(hwnd, GWLP_USERDATA));
                if (activatedBw) {
                    WindowManager::GetInstance().SetActiveWindowId(activatedBw->window_id);
                    LOG_DEBUG("🪟 Active window set to " + std::to_string(activatedBw->window_id));
                }
            }
            break;
        }

        case WM_ACTIVATEAPP: {
            // Only the primary window manages overlay dismissal on focus loss
            BrowserWindow* activeBw = reinterpret_cast<BrowserWindow*>(GetWindowLongPtr(hwnd, GWLP_USERDATA));
            if (activeBw && activeBw->window_id != WindowManager::GetInstance().GetPrimaryWindowId()) break;

            // wParam is TRUE if app is being activated, FALSE if deactivated
            if (!wParam) {
                // App is losing focus

                // If a native file dialog is open (e.g. from <input type="file">),
                // skip overlay destruction — the dialog steals activation temporarily.
                if (g_file_dialog_active) {
                    LOG_DEBUG("📱 App losing focus but file dialog is active - keeping overlays open");
                    break;
                }

                // Hide wallet overlay on focus loss UNLESS prevent-close flag is active.
                // Keep-alive: hide instead of destroy.
                LOG_DEBUG("App losing focus - checking wallet overlay");
                if (g_wallet_overlay_prevent_close) {
                    LOG_INFO("Wallet overlay prevent-close active - keeping overlay open on focus loss");
                } else if (g_wallet_overlay_hwnd && IsWindow(g_wallet_overlay_hwnd) && IsWindowVisible(g_wallet_overlay_hwnd)) {
                    LOG_INFO("Hiding wallet overlay due to app focus loss");
                    HideWalletOverlay();
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

        case WM_CLOSE: {
            // Determine which BrowserWindow this HWND belongs to
            BrowserWindow* bw = reinterpret_cast<BrowserWindow*>(GetWindowLongPtr(hwnd, GWLP_USERDATA));
            int wid = bw ? bw->window_id : 0;
            int primaryId = WindowManager::GetInstance().GetPrimaryWindowId();
            int windowCount = WindowManager::GetInstance().GetWindowCount();

            if (windowCount <= 1) {
                // Last window — full graceful shutdown
                LOG_INFO("🛑 Last window received WM_CLOSE - starting graceful shutdown...");
                g_app_shutting_down = true;
                ShowWindow(hwnd, SW_HIDE);
                ShutdownApplication();
            } else if (wid == primaryId) {
                // Primary window closing but other windows survive — transfer primary role
                int nextWid = WindowManager::GetInstance().GetNextWindowId();
                LOG_INFO("🛑 Primary window " + std::to_string(wid) + " closing - transferring to window " + std::to_string(nextWid));
                TransferPrimaryWindow(nextWid);

                // Now close this window like a secondary
                std::vector<Tab*> allTabs = TabManager::GetInstance().GetAllTabs();
                for (Tab* tab : allTabs) {
                    if (tab && tab->window_id == wid) {
                        TabManager::GetInstance().CloseTab(tab->id);
                    }
                }
                if (bw->header_browser) {
                    bw->header_browser->GetHost()->CloseBrowser(false);
                }
                if (bw->header_hwnd && IsWindow(bw->header_hwnd)) {
                    DestroyWindow(bw->header_hwnd);
                }
                WindowManager::GetInstance().RemoveWindow(wid);
                DestroyWindow(hwnd);
                SimpleHandler::NotifyTabListChanged();
            } else {
                // Secondary window — close only this window's tabs and clean up
                LOG_INFO("🛑 Window " + std::to_string(wid) + " received WM_CLOSE - closing window...");

                std::vector<Tab*> allTabs = TabManager::GetInstance().GetAllTabs();
                for (Tab* tab : allTabs) {
                    if (tab && tab->window_id == wid) {
                        TabManager::GetInstance().CloseTab(tab->id);
                    }
                }
                if (bw->header_browser) {
                    bw->header_browser->GetHost()->CloseBrowser(false);
                }
                if (bw->header_hwnd && IsWindow(bw->header_hwnd)) {
                    DestroyWindow(bw->header_hwnd);
                }
                WindowManager::GetInstance().RemoveWindow(wid);
                DestroyWindow(hwnd);
                SimpleHandler::NotifyTabListChanged();
            }
            return 0;
        }

        case WM_SINGLE_INSTANCE_NEW_WINDOW: {
            // B-6: Second instance requested a new window via named pipe.
            // lParam = heap-allocated std::string* (URL or empty). Must delete.
            std::string* urlPtr = reinterpret_cast<std::string*>(lParam);
            std::string url = urlPtr ? *urlPtr : "";
            delete urlPtr;

            LOG_INFO("SingleInstance: Creating new window" +
                     (url.empty() ? "" : " with URL: " + url));

            BrowserWindow* primary = WindowManager::GetInstance().GetPrimaryWindow();
            BrowserWindow* newWin = WindowManager::GetInstance().CreateFullWindow(true);

            if (newWin && !url.empty()) {
                // Navigate the initial tab to the requested URL.
                Tab* activeTab = TabManager::GetInstance().GetActiveTabForWindow(newWin->window_id);
                if (activeTab && activeTab->browser) {
                    activeTab->browser->GetMainFrame()->LoadURL(url);
                }
            }

            // Bring the NEW window to the foreground.
            if (newWin && newWin->hwnd && IsWindow(newWin->hwnd)) {
                SetForegroundWindow(newWin->hwnd);
                BringWindowToTop(newWin->hwnd);
            }
            return 0;
        }

        case WM_DESTROY:
            // Only quit if no windows remain
            if (WindowManager::GetInstance().GetWindowCount() == 0) {
                LOG_INFO("🛑 Last window destroyed - quitting CEF message loop");
                CefQuitMessageLoop();
            }
            break;

#ifdef _WIN32
        case WM_NCCALCSIZE: {
            if (wParam == TRUE) {
                // Return 0 to make entire window = client area (frameless, no title bar)
                return 0;
            }
            break;
        }

        case WM_NCACTIVATE: {
            // Prevent Windows from drawing the non-client border on focus change.
            // DefWindowProc redraws the border to show active/inactive state — skip it.
            return TRUE;
        }

        case WM_NCHITTEST: {
            // Resize borders for frameless window. Child windows are inset by RESIZE_BORDER
            // pixels, exposing the parent window at the edges for hit-testing.
            const int border = 5;
            POINT pt = { GET_X_LPARAM(lParam), GET_Y_LPARAM(lParam) };
            ScreenToClient(hwnd, &pt);
            RECT rc;
            GetClientRect(hwnd, &rc);
            bool top = pt.y < border;
            bool bottom = pt.y >= rc.bottom - border;
            bool left = pt.x < border;
            bool right = pt.x >= rc.right - border;
            if (top && left) return HTTOPLEFT;
            if (top && right) return HTTOPRIGHT;
            if (bottom && left) return HTBOTTOMLEFT;
            if (bottom && right) return HTBOTTOMRIGHT;
            if (top) return HTTOP;
            if (bottom) return HTBOTTOM;
            if (left) return HTLEFT;
            if (right) return HTRIGHT;
            return HTCLIENT;
        }

        case WM_DPICHANGED: {
            // Accept Windows' suggested rect when dragging between monitors with different DPI.
            // SetWindowPos triggers WM_SIZE which recalculates header/tab layout via GetHeaderHeightPx().
            RECT* suggested = reinterpret_cast<RECT*>(lParam);
            SetWindowPos(hwnd, nullptr,
                suggested->left, suggested->top,
                suggested->right - suggested->left,
                suggested->bottom - suggested->top,
                SWP_NOZORDER | SWP_NOACTIVATE);

            // Notify all CEF browsers in this window that screen info changed,
            // so they re-render at the new DPI scale factor.
            BrowserWindow* dpiBw = reinterpret_cast<BrowserWindow*>(GetWindowLongPtr(hwnd, GWLP_USERDATA));
            CefRefPtr<CefBrowser> hdrBrowser = dpiBw ? dpiBw->header_browser : SimpleHandler::GetHeaderBrowser();
            if (hdrBrowser) {
                hdrBrowser->GetHost()->NotifyScreenInfoChanged();
            }
            int dpiWinId = dpiBw ? dpiBw->window_id : 0;
            std::vector<Tab*> dpiTabs = TabManager::GetInstance().GetAllTabs();
            for (Tab* tab : dpiTabs) {
                if (tab && tab->window_id == dpiWinId && tab->browser) {
                    tab->browser->GetHost()->NotifyScreenInfoChanged();
                }
            }

            // Notify overlay browsers (primary window only).
            // NotifyScreenInfoChanged triggers GetScreenInfo re-query (new device_scale_factor).
            // WasResized triggers GetViewRect re-query (new logical viewport at new DPI).
            // The WM_SIZE handler above already repositions/resizes visible overlay HWNDs.
            if (dpiWinId == WindowManager::GetInstance().GetPrimaryWindowId()) {
                auto notifyOverlay = [](CefRefPtr<CefBrowser> b) {
                    if (b) {
                        b->GetHost()->NotifyScreenInfoChanged();
                        b->GetHost()->WasResized();
                    }
                };
                notifyOverlay(SimpleHandler::GetSettingsBrowser());
                notifyOverlay(SimpleHandler::GetWalletPanelBrowser());
                notifyOverlay(SimpleHandler::GetOmniboxBrowser());
                notifyOverlay(SimpleHandler::GetCookiePanelBrowser());
                notifyOverlay(SimpleHandler::GetDownloadPanelBrowser());
                notifyOverlay(SimpleHandler::GetProfilePanelBrowser());
                notifyOverlay(SimpleHandler::GetMenuBrowser());
            }
            return 0;
        }
#endif
    }

    return DefWindowProc(hwnd, msg, wParam, lParam);
}


LRESULT CALLBACK SettingsOverlayWndProc(HWND hwnd, UINT msg, WPARAM wParam, LPARAM lParam) {
    switch (msg) {
        case WM_MOUSEACTIVATE:
            // Prevent focus theft - matches cookie panel pattern
            return MA_NOACTIVATE;

        // WM_SETCURSOR: intentionally not handled — OnCursorChange in
        // MyOverlayRenderHandler sets the cursor based on CSS (pointer, text, etc.)

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

// No mouse hook for wallet overlay — WM_ACTIVATE(WA_INACTIVE) handles click-outside
// because wallet uses MA_ACTIVATE (takes focus). Mouse hooks add latency to all
// mouse events system-wide and are only needed for MA_NOACTIVATE overlays.

LRESULT CALLBACK WalletOverlayWndProc(HWND hwnd, UINT msg, WPARAM wParam, LPARAM lParam) {
    // Get the wallet browser from the BrowserWindow* stored in GWLP_USERDATA.
    // This correctly routes to the window that opened the wallet (not always window 0).
    auto getWalletBrowser = [hwnd]() -> CefRefPtr<CefBrowser> {
        BrowserWindow* bw = reinterpret_cast<BrowserWindow*>(GetWindowLongPtr(hwnd, GWLP_USERDATA));
        if (bw && bw->wallet_browser) return bw->wallet_browser;
        return SimpleHandler::GetWalletBrowser();  // fallback to window 0
    };

    switch (msg) {
        case WM_MOUSEACTIVATE:
            return MA_ACTIVATE;

        case WM_SETFOCUS: {
            // Disable IME to prevent composition window overlay
            ImmAssociateContext(hwnd, nullptr);
            
            // Set CEF browser focus — required for text input to work
            CefRefPtr<CefBrowser> wallet_browser = getWalletBrowser();
            if (wallet_browser) {
                wallet_browser->GetHost()->SetFocus(true);
            }
            return 0;
        }

        case WM_LBUTTONDOWN: {
            SetFocus(hwnd);

            POINT pt = { GET_X_LPARAM(lParam), GET_Y_LPARAM(lParam) };
            CefMouseEvent mouse_event;
            mouse_event.x = pt.x;
            mouse_event.y = pt.y;
            mouse_event.modifiers = EVENTFLAG_LEFT_MOUSE_BUTTON;

            CefRefPtr<CefBrowser> wallet_browser = getWalletBrowser();
            if (wallet_browser) {
                wallet_browser->GetHost()->SendMouseClickEvent(mouse_event, MBT_LEFT, false, 1);
            }
            return 0;
        }

        case WM_LBUTTONUP: {
            POINT pt = { GET_X_LPARAM(lParam), GET_Y_LPARAM(lParam) };
            CefMouseEvent mouse_event;
            mouse_event.x = pt.x;
            mouse_event.y = pt.y;
            mouse_event.modifiers = 0;

            CefRefPtr<CefBrowser> wallet_browser = getWalletBrowser();
            if (wallet_browser) {
                wallet_browser->GetHost()->SendMouseClickEvent(mouse_event, MBT_LEFT, true, 1);
            }
            return 0;
        }

        case WM_RBUTTONDOWN: {
            SetFocus(hwnd);

            POINT pt = { GET_X_LPARAM(lParam), GET_Y_LPARAM(lParam) };
            CefMouseEvent mouse_event;
            mouse_event.x = pt.x;
            mouse_event.y = pt.y;
            mouse_event.modifiers = EVENTFLAG_RIGHT_MOUSE_BUTTON;

            CefRefPtr<CefBrowser> wallet_browser = getWalletBrowser();
            if (wallet_browser) {
                wallet_browser->GetHost()->SendMouseClickEvent(mouse_event, MBT_RIGHT, false, 1);
            }
            return 0;
        }

        case WM_RBUTTONUP: {
            POINT pt = { GET_X_LPARAM(lParam), GET_Y_LPARAM(lParam) };
            CefMouseEvent mouse_event;
            mouse_event.x = pt.x;
            mouse_event.y = pt.y;
            mouse_event.modifiers = 0;

            CefRefPtr<CefBrowser> wallet_browser = getWalletBrowser();
            if (wallet_browser) {
                wallet_browser->GetHost()->SendMouseClickEvent(mouse_event, MBT_RIGHT, true, 1);
            }
            return 0;
        }

        case WM_KEYDOWN: {
            CefRefPtr<CefBrowser> wallet_browser = getWalletBrowser();
            if (wallet_browser) {
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

                wallet_browser->GetHost()->SendKeyEvent(key_event);
            }
            return 0;
        }

        case WM_KEYUP: {
            CefRefPtr<CefBrowser> wallet_browser = getWalletBrowser();
            if (wallet_browser) {
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

                wallet_browser->GetHost()->SendKeyEvent(key_event);
            }
            return 0;
        }

        case WM_CHAR: {
            CefRefPtr<CefBrowser> wallet_browser = getWalletBrowser();
            if (wallet_browser) {
                CefKeyEvent key_event;
                key_event.type = KEYEVENT_CHAR;
                key_event.windows_key_code = static_cast<int>(wParam);
                key_event.native_key_code = static_cast<int>(lParam);
                key_event.character = static_cast<char16_t>(wParam);
                key_event.unmodified_character = static_cast<char16_t>(wParam);
                key_event.is_system_key = false;

                int modifiers = 0;
                if (GetKeyState(VK_CONTROL) & 0x8000) modifiers |= EVENTFLAG_CONTROL_DOWN;
                if (GetKeyState(VK_SHIFT) & 0x8000) modifiers |= EVENTFLAG_SHIFT_DOWN;
                if (GetKeyState(VK_MENU) & 0x8000) modifiers |= EVENTFLAG_ALT_DOWN;
                if (GetKeyState(VK_LWIN) & 0x8000 || GetKeyState(VK_RWIN) & 0x8000) modifiers |= EVENTFLAG_COMMAND_DOWN;
                key_event.modifiers = modifiers;

                wallet_browser->GetHost()->SendKeyEvent(key_event);
            }

            return 0;
        }

        case WM_MOUSEMOVE: {
            POINT pt = { GET_X_LPARAM(lParam), GET_Y_LPARAM(lParam) };
            CefMouseEvent mouse_event;
            mouse_event.x = pt.x;
            mouse_event.y = pt.y;
            mouse_event.modifiers = 0;

            CefRefPtr<CefBrowser> wallet_browser = getWalletBrowser();
            if (wallet_browser) {
                wallet_browser->GetHost()->SendMouseMoveEvent(mouse_event, false);
            }
            return 0;
        }

        case WM_MOUSEWHEEL: {
            // WM_MOUSEWHEEL provides SCREEN coords — must convert to client coords
            POINT screenPt = { GET_X_LPARAM(lParam), GET_Y_LPARAM(lParam) };
            POINT clientPt = screenPt;
            ScreenToClient(hwnd, &clientPt);
            int delta = GET_WHEEL_DELTA_WPARAM(wParam);
            CefMouseEvent mouse_event;
            mouse_event.x = clientPt.x;
            mouse_event.y = clientPt.y;
            mouse_event.modifiers = 0;

            CefRefPtr<CefBrowser> wallet_browser = getWalletBrowser();
            if (wallet_browser) {
                wallet_browser->GetHost()->SendMouseWheelEvent(mouse_event, 0, delta);
            }
            return 0;
        }

        case WM_CLOSE:
            // Keep-alive: hide instead of destroy
            LOG_DEBUG("Wallet Overlay received WM_CLOSE - hiding (keep-alive)");
            ShowWindow(hwnd, SW_HIDE);
            return 0;

        case WM_DESTROY:
            LOG_DEBUG("Wallet Overlay received WM_DESTROY - cleaning up");
            return 0;

        case WM_ACTIVATE:
            LOG_DEBUG("Wallet HWND activated with state: " + std::to_string(LOWORD(wParam)));
            if (LOWORD(wParam) != WA_INACTIVE) {
                // Wallet becoming active — disable IME to prevent composition overlay
                ImmAssociateContext(hwnd, nullptr);
            } else {
                // Check prevent-close flag (set at creation, cleared by React when safe)
                if (g_wallet_overlay_prevent_close || g_file_dialog_active) {
                    LOG_INFO("Wallet overlay lost activation but prevent-close active - keeping open");
                    return 0;
                }
                // Wallet lost focus — hide it (keep-alive)
                LOG_INFO("Hiding wallet overlay — lost activation (click-outside)");
                HideWalletOverlay();
                return 0;
            }
            break;

        case WM_WINDOWPOSCHANGING:
            break;

        // ========== IME SUPPRESSION ==========
        // Suppress Windows IME composition window that overlays text input
        // This white overlay appears when typing in windowless CEF browsers
        case WM_IME_SETCONTEXT:
            // Fully suppress — do NOT pass to DefWindowProc which can still
            // render IME candidate/guide windows as white rectangles on layered HWNDs
            return 0;

        case WM_IME_STARTCOMPOSITION:
            LOG_DEBUG("⌨️ Wallet Overlay WM_IME_STARTCOMPOSITION - suppressing");
            return 0;  // Suppress default IME composition window

        case WM_IME_COMPOSITION:
            LOG_DEBUG("⌨️ Wallet Overlay WM_IME_COMPOSITION - suppressing");
            return 0;  // Suppress default IME composition rendering

        case WM_IME_ENDCOMPOSITION:
            LOG_DEBUG("⌨️ Wallet Overlay WM_IME_ENDCOMPOSITION - suppressing");
            return 0;
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

        // WM_SETCURSOR: not handled — OnCursorChange sets cursor from CSS

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

        // WM_SETCURSOR: not handled — OnCursorChange sets cursor from CSS

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
            // WM_MOUSEWHEEL lParam contains SCREEN coords — must convert to client
            POINT pt = { GET_X_LPARAM(lParam), GET_Y_LPARAM(lParam) };
            ScreenToClient(hwnd, &pt);
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

// ========== DOWNLOAD PANEL OVERLAY ==========

LRESULT CALLBACK DownloadPanelMouseHookProc(int nCode, WPARAM wParam, LPARAM lParam) {
    if (nCode == HC_ACTION) {
        if (wParam == WM_LBUTTONDOWN || wParam == WM_RBUTTONDOWN) {
            if (g_download_panel_overlay_hwnd && IsWindow(g_download_panel_overlay_hwnd) && IsWindowVisible(g_download_panel_overlay_hwnd)) {
                MSLLHOOKSTRUCT* mouseInfo = (MSLLHOOKSTRUCT*)lParam;
                POINT clickPoint = mouseInfo->pt;
                RECT overlayRect;
                GetWindowRect(g_download_panel_overlay_hwnd, &overlayRect);
                if (!PtInRect(&overlayRect, clickPoint)) {
                    LOG_DEBUG("🖱️ Click detected outside download panel overlay bounds - dismissing");
                    extern void HideDownloadPanelOverlay();
                    HideDownloadPanelOverlay();
                }
            }
        }
    }
    return CallNextHookEx(g_download_panel_mouse_hook, nCode, wParam, lParam);
}

// ========== PROFILE PANEL OVERLAY ==========

LRESULT CALLBACK ProfilePanelMouseHookProc(int nCode, WPARAM wParam, LPARAM lParam) {
    if (nCode == HC_ACTION) {
        if (wParam == WM_LBUTTONDOWN || wParam == WM_RBUTTONDOWN) {
            if (g_profile_panel_overlay_hwnd && IsWindow(g_profile_panel_overlay_hwnd) && IsWindowVisible(g_profile_panel_overlay_hwnd)) {
                MSLLHOOKSTRUCT* mouseInfo = (MSLLHOOKSTRUCT*)lParam;
                POINT clickPoint = mouseInfo->pt;
                RECT overlayRect;
                GetWindowRect(g_profile_panel_overlay_hwnd, &overlayRect);
                if (!PtInRect(&overlayRect, clickPoint)) {
                    LOG_DEBUG("🖱️ Click detected outside profile panel overlay bounds - dismissing");
                    extern void HideProfilePanelOverlay();
                    HideProfilePanelOverlay();
                }
            }
        }
    }
    return CallNextHookEx(g_profile_panel_mouse_hook, nCode, wParam, lParam);
}

LRESULT CALLBACK DownloadPanelOverlayWndProc(HWND hwnd, UINT msg, WPARAM wParam, LPARAM lParam) {
    switch (msg) {
        case WM_MOUSEACTIVATE:
            return MA_NOACTIVATE;

        // WM_SETCURSOR: not handled — OnCursorChange sets cursor from CSS

        case WM_MOUSEMOVE: {
            POINT pt = { GET_X_LPARAM(lParam), GET_Y_LPARAM(lParam) };
            CefMouseEvent mouse_event;
            mouse_event.x = pt.x;
            mouse_event.y = pt.y;
            mouse_event.modifiers = 0;
            CefRefPtr<CefBrowser> dl_browser = SimpleHandler::GetDownloadPanelBrowser();
            if (dl_browser) {
                dl_browser->GetHost()->SendMouseMoveEvent(mouse_event, false);
            }
            return 0;
        }

        case WM_LBUTTONDOWN: {
            POINT pt = { GET_X_LPARAM(lParam), GET_Y_LPARAM(lParam) };
            CefMouseEvent mouse_event;
            mouse_event.x = pt.x;
            mouse_event.y = pt.y;
            mouse_event.modifiers = 0;
            CefRefPtr<CefBrowser> dl_browser = SimpleHandler::GetDownloadPanelBrowser();
            if (dl_browser) {
                dl_browser->GetHost()->SendMouseClickEvent(mouse_event, MBT_LEFT, false, 1);
                dl_browser->GetHost()->SendMouseClickEvent(mouse_event, MBT_LEFT, true, 1);
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
            CefRefPtr<CefBrowser> dl_browser = SimpleHandler::GetDownloadPanelBrowser();
            if (dl_browser) {
                dl_browser->GetHost()->SendMouseWheelEvent(mouse_event, 0, delta);
            }
            return 0;
        }

        case WM_CLOSE:
            ShowWindow(hwnd, SW_HIDE);
            return 0;

        case WM_DESTROY:
            return 0;

        case WM_WINDOWPOSCHANGING:
            break;
    }
    return DefWindowProc(hwnd, msg, wParam, lParam);
}

// Profile Panel Overlay Window Procedure
LRESULT CALLBACK ProfilePanelOverlayWndProc(HWND hwnd, UINT msg, WPARAM wParam, LPARAM lParam) {
    switch (msg) {
        case WM_MOUSEACTIVATE:
            return MA_ACTIVATE;

        case WM_SETFOCUS: {
            ImmAssociateContext(hwnd, nullptr);
            CefRefPtr<CefBrowser> profile_browser = SimpleHandler::GetProfilePanelBrowser();
            if (profile_browser) {
                profile_browser->GetHost()->SetFocus(true);
            }
            return 0;
        }

        case WM_MOUSEMOVE: {
            POINT pt = { GET_X_LPARAM(lParam), GET_Y_LPARAM(lParam) };
            CefMouseEvent mouse_event;
            mouse_event.x = pt.x;
            mouse_event.y = pt.y;
            mouse_event.modifiers = 0;
            CefRefPtr<CefBrowser> profile_browser = SimpleHandler::GetProfilePanelBrowser();
            if (profile_browser) {
                profile_browser->GetHost()->SendMouseMoveEvent(mouse_event, false);
            }
            return 0;
        }

        case WM_LBUTTONDOWN: {
            SetFocus(hwnd);
            POINT pt = { GET_X_LPARAM(lParam), GET_Y_LPARAM(lParam) };
            CefMouseEvent mouse_event;
            mouse_event.x = pt.x;
            mouse_event.y = pt.y;
            mouse_event.modifiers = EVENTFLAG_LEFT_MOUSE_BUTTON;
            CefRefPtr<CefBrowser> profile_browser = SimpleHandler::GetProfilePanelBrowser();
            if (profile_browser) {
                profile_browser->GetHost()->SendMouseClickEvent(mouse_event, MBT_LEFT, false, 1);
            }
            return 0;
        }

        case WM_LBUTTONUP: {
            POINT pt = { GET_X_LPARAM(lParam), GET_Y_LPARAM(lParam) };
            CefMouseEvent mouse_event;
            mouse_event.x = pt.x;
            mouse_event.y = pt.y;
            mouse_event.modifiers = 0;
            CefRefPtr<CefBrowser> profile_browser = SimpleHandler::GetProfilePanelBrowser();
            if (profile_browser) {
                profile_browser->GetHost()->SendMouseClickEvent(mouse_event, MBT_LEFT, true, 1);
            }
            return 0;
        }

        case WM_RBUTTONDOWN: {
            SetFocus(hwnd);
            POINT pt = { GET_X_LPARAM(lParam), GET_Y_LPARAM(lParam) };
            CefMouseEvent mouse_event;
            mouse_event.x = pt.x;
            mouse_event.y = pt.y;
            mouse_event.modifiers = EVENTFLAG_RIGHT_MOUSE_BUTTON;
            CefRefPtr<CefBrowser> profile_browser = SimpleHandler::GetProfilePanelBrowser();
            if (profile_browser) {
                profile_browser->GetHost()->SendMouseClickEvent(mouse_event, MBT_RIGHT, false, 1);
            }
            return 0;
        }

        case WM_RBUTTONUP: {
            POINT pt = { GET_X_LPARAM(lParam), GET_Y_LPARAM(lParam) };
            CefMouseEvent mouse_event;
            mouse_event.x = pt.x;
            mouse_event.y = pt.y;
            mouse_event.modifiers = 0;
            CefRefPtr<CefBrowser> profile_browser = SimpleHandler::GetProfilePanelBrowser();
            if (profile_browser) {
                profile_browser->GetHost()->SendMouseClickEvent(mouse_event, MBT_RIGHT, true, 1);
            }
            return 0;
        }

        case WM_KEYDOWN: {
            CefRefPtr<CefBrowser> profile_browser = SimpleHandler::GetProfilePanelBrowser();
            if (profile_browser) {
                CefKeyEvent key_event;
                key_event.type = KEYEVENT_KEYDOWN;
                key_event.windows_key_code = wParam;
                key_event.native_key_code = lParam;
                key_event.is_system_key = false;
                int modifiers = 0;
                if (GetKeyState(VK_CONTROL) & 0x8000) modifiers |= EVENTFLAG_CONTROL_DOWN;
                if (GetKeyState(VK_SHIFT) & 0x8000) modifiers |= EVENTFLAG_SHIFT_DOWN;
                if (GetKeyState(VK_MENU) & 0x8000) modifiers |= EVENTFLAG_ALT_DOWN;
                key_event.modifiers = modifiers;
                profile_browser->GetHost()->SendKeyEvent(key_event);
            }
            return 0;
        }

        case WM_KEYUP: {
            CefRefPtr<CefBrowser> profile_browser = SimpleHandler::GetProfilePanelBrowser();
            if (profile_browser) {
                CefKeyEvent key_event;
                key_event.type = KEYEVENT_KEYUP;
                key_event.windows_key_code = wParam;
                key_event.native_key_code = lParam;
                key_event.is_system_key = false;
                int modifiers = 0;
                if (GetKeyState(VK_CONTROL) & 0x8000) modifiers |= EVENTFLAG_CONTROL_DOWN;
                if (GetKeyState(VK_SHIFT) & 0x8000) modifiers |= EVENTFLAG_SHIFT_DOWN;
                if (GetKeyState(VK_MENU) & 0x8000) modifiers |= EVENTFLAG_ALT_DOWN;
                key_event.modifiers = modifiers;
                profile_browser->GetHost()->SendKeyEvent(key_event);
            }
            return 0;
        }

        case WM_CHAR: {
            CefRefPtr<CefBrowser> profile_browser = SimpleHandler::GetProfilePanelBrowser();
            if (profile_browser) {
                CefKeyEvent key_event;
                key_event.type = KEYEVENT_CHAR;
                key_event.windows_key_code = static_cast<int>(wParam);
                key_event.native_key_code = static_cast<int>(lParam);
                key_event.character = static_cast<char16_t>(wParam);
                key_event.unmodified_character = static_cast<char16_t>(wParam);
                key_event.is_system_key = false;
                int modifiers = 0;
                if (GetKeyState(VK_CONTROL) & 0x8000) modifiers |= EVENTFLAG_CONTROL_DOWN;
                if (GetKeyState(VK_SHIFT) & 0x8000) modifiers |= EVENTFLAG_SHIFT_DOWN;
                if (GetKeyState(VK_MENU) & 0x8000) modifiers |= EVENTFLAG_ALT_DOWN;
                key_event.modifiers = modifiers;
                profile_browser->GetHost()->SendKeyEvent(key_event);
            }
            return 0;
        }

        case WM_MOUSEWHEEL: {
            POINT screenPt = { GET_X_LPARAM(lParam), GET_Y_LPARAM(lParam) };
            POINT clientPt = screenPt;
            ScreenToClient(hwnd, &clientPt);
            int delta = GET_WHEEL_DELTA_WPARAM(wParam);
            CefMouseEvent mouse_event;
            mouse_event.x = clientPt.x;
            mouse_event.y = clientPt.y;
            mouse_event.modifiers = 0;
            CefRefPtr<CefBrowser> profile_browser = SimpleHandler::GetProfilePanelBrowser();
            if (profile_browser) {
                profile_browser->GetHost()->SendMouseWheelEvent(mouse_event, 0, delta);
            }
            return 0;
        }

        case WM_CLOSE:
            ShowWindow(hwnd, SW_HIDE);
            return 0;

        case WM_DESTROY:
            return 0;

        case WM_ACTIVATE:
            if (LOWORD(wParam) != WA_INACTIVE) {
                ImmAssociateContext(hwnd, nullptr);
            } else {
                if (g_file_dialog_active) return 0;
                extern ULONGLONG g_profile_last_hide_tick;
                LOG_INFO("Hiding profile panel — lost activation (click-outside)");
                extern void HideProfilePanelOverlay();
                HideProfilePanelOverlay();
                return 0;
            }
            break;

        case WM_WINDOWPOSCHANGING:
            break;

        // IME suppression (same as wallet)
        case WM_IME_SETCONTEXT:
            return 0;
        case WM_IME_STARTCOMPOSITION:
            return 0;
        case WM_IME_COMPOSITION:
            return 0;
        case WM_IME_ENDCOMPOSITION:
            return 0;
    }
    return DefWindowProc(hwnd, msg, wParam, lParam);
}

// ========== MENU OVERLAY ==========

LRESULT CALLBACK MenuMouseHookProc(int nCode, WPARAM wParam, LPARAM lParam) {
    if (nCode == HC_ACTION) {
        if (wParam == WM_LBUTTONDOWN || wParam == WM_RBUTTONDOWN) {
            if (g_menu_overlay_hwnd && IsWindow(g_menu_overlay_hwnd) && IsWindowVisible(g_menu_overlay_hwnd)) {
                MSLLHOOKSTRUCT* mouseInfo = (MSLLHOOKSTRUCT*)lParam;
                POINT clickPoint = mouseInfo->pt;
                RECT overlayRect;
                GetWindowRect(g_menu_overlay_hwnd, &overlayRect);
                if (!PtInRect(&overlayRect, clickPoint)) {
                    LOG_DEBUG("Click detected outside menu overlay bounds - dismissing");
                    extern void HideMenuOverlay();
                    HideMenuOverlay();
                }
            }
        }
    }
    return CallNextHookEx(g_menu_mouse_hook, nCode, wParam, lParam);
}

LRESULT CALLBACK MenuOverlayWndProc(HWND hwnd, UINT msg, WPARAM wParam, LPARAM lParam) {
    switch (msg) {
        case WM_MOUSEACTIVATE:
            return MA_NOACTIVATE;

        // WM_SETCURSOR: not handled — OnCursorChange sets cursor from CSS

        case WM_MOUSEMOVE: {
            POINT pt = { GET_X_LPARAM(lParam), GET_Y_LPARAM(lParam) };
            CefMouseEvent mouse_event;
            mouse_event.x = pt.x;
            mouse_event.y = pt.y;
            mouse_event.modifiers = 0;
            CefRefPtr<CefBrowser> menu_browser = SimpleHandler::GetMenuBrowser();
            if (menu_browser) {
                menu_browser->GetHost()->SendMouseMoveEvent(mouse_event, false);
            }
            return 0;
        }

        case WM_LBUTTONDOWN: {
            POINT pt = { GET_X_LPARAM(lParam), GET_Y_LPARAM(lParam) };
            CefMouseEvent mouse_event;
            mouse_event.x = pt.x;
            mouse_event.y = pt.y;
            mouse_event.modifiers = 0;
            CefRefPtr<CefBrowser> menu_browser = SimpleHandler::GetMenuBrowser();
            if (menu_browser) {
                menu_browser->GetHost()->SendMouseClickEvent(mouse_event, MBT_LEFT, false, 1);
                menu_browser->GetHost()->SendMouseClickEvent(mouse_event, MBT_LEFT, true, 1);
            }
            return 0;
        }

        case WM_MOUSEWHEEL: {
            POINT pt = { GET_X_LPARAM(lParam), GET_Y_LPARAM(lParam) };
            ScreenToClient(hwnd, &pt);
            int delta = GET_WHEEL_DELTA_WPARAM(wParam);
            CefMouseEvent mouse_event;
            mouse_event.x = pt.x;
            mouse_event.y = pt.y;
            mouse_event.modifiers = 0;
            CefRefPtr<CefBrowser> menu_browser = SimpleHandler::GetMenuBrowser();
            if (menu_browser) {
                menu_browser->GetHost()->SendMouseWheelEvent(mouse_event, 0, delta);
            }
            return 0;
        }

        case WM_CLOSE:
            ShowWindow(hwnd, SW_HIDE);
            return 0;

        case WM_DESTROY:
            return 0;

        case WM_WINDOWPOSCHANGING:
            break;
    }
    return DefWindowProc(hwnd, msg, wParam, lParam);
}

// Lightweight health check — returns true if GET /health responds with "ok".
// Uses a short timeout so it fails fast when nothing is listening.
// timeoutMs defaults to 500ms for polling; use 200ms for pre-launch "already running?" checks.
static bool QuickHealthCheck(DWORD timeoutMs = 500) {
    HINTERNET hSession = WinHttpOpen(L"HodosBrowser/HealthCheck",
        WINHTTP_ACCESS_TYPE_DEFAULT_PROXY,
        WINHTTP_NO_PROXY_NAME, WINHTTP_NO_PROXY_BYPASS, 0);
    if (!hSession) return false;

    DWORD timeout = timeoutMs;
    WinHttpSetOption(hSession, WINHTTP_OPTION_CONNECT_TIMEOUT, &timeout, sizeof(timeout));
    WinHttpSetOption(hSession, WINHTTP_OPTION_RECEIVE_TIMEOUT, &timeout, sizeof(timeout));
    WinHttpSetOption(hSession, WINHTTP_OPTION_SEND_TIMEOUT, &timeout, sizeof(timeout));

    HINTERNET hConnect = WinHttpConnect(hSession, L"localhost", 31301, 0);
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
// Send POST /shutdown to a localhost service. Returns true if the request succeeded.
// Used for graceful shutdown of wallet (31301) and adblock (31302) servers.
static bool SendShutdownRequest(int port) {
    HINTERNET hSession = WinHttpOpen(L"HodosBrowser/Shutdown",
        WINHTTP_ACCESS_TYPE_DEFAULT_PROXY,
        WINHTTP_NO_PROXY_NAME, WINHTTP_NO_PROXY_BYPASS, 0);
    if (!hSession) return false;

    DWORD timeout = 2000;
    WinHttpSetOption(hSession, WINHTTP_OPTION_CONNECT_TIMEOUT, &timeout, sizeof(timeout));
    WinHttpSetOption(hSession, WINHTTP_OPTION_RECEIVE_TIMEOUT, &timeout, sizeof(timeout));
    WinHttpSetOption(hSession, WINHTTP_OPTION_SEND_TIMEOUT, &timeout, sizeof(timeout));

    HINTERNET hConnect = WinHttpConnect(hSession, L"localhost", static_cast<INTERNET_PORT>(port), 0);
    if (!hConnect) { WinHttpCloseHandle(hSession); return false; }

    HINTERNET hRequest = WinHttpOpenRequest(hConnect, L"POST", L"/shutdown",
        nullptr, WINHTTP_NO_REFERER, WINHTTP_DEFAULT_ACCEPT_TYPES, 0);
    if (!hRequest) { WinHttpCloseHandle(hConnect); WinHttpCloseHandle(hSession); return false; }

    BOOL ok = WinHttpSendRequest(hRequest, WINHTTP_NO_ADDITIONAL_HEADERS, 0,
        WINHTTP_NO_REQUEST_DATA, 0, 0, 0);
    bool success = false;
    if (ok) {
        success = WinHttpReceiveResponse(hRequest, nullptr) == TRUE;
    }

    WinHttpCloseHandle(hRequest);
    WinHttpCloseHandle(hConnect);
    WinHttpCloseHandle(hSession);
    return success;
}

// Phase 1: Launch wallet process (non-blocking). Called early in startup so the
// Rust server warms up during CefInitialize (which blocks for 2-5 seconds).
// Fast TCP port probe — returns true if something is listening on localhost:port.
// Uses raw Winsock with a tight timeout instead of WinHTTP (which ignores short timeouts).
static bool IsPortListening(int port) {
    WSADATA wsaData;
    if (WSAStartup(MAKEWORD(2, 2), &wsaData) != 0) return false;

    SOCKET sock = socket(AF_INET, SOCK_STREAM, IPPROTO_TCP);
    if (sock == INVALID_SOCKET) { WSACleanup(); return false; }

    // Set non-blocking
    u_long mode = 1;
    ioctlsocket(sock, FIONBIO, &mode);

    sockaddr_in addr = {};
    addr.sin_family = AF_INET;
    addr.sin_port = htons(static_cast<u_short>(port));
    addr.sin_addr.s_addr = htonl(INADDR_LOOPBACK);

    connect(sock, (sockaddr*)&addr, sizeof(addr));

    // Wait up to 100ms for connection
    fd_set writeSet;
    FD_ZERO(&writeSet);
    FD_SET(sock, &writeSet);
    timeval tv = {0, 100000};  // 100ms
    int result = select(0, nullptr, &writeSet, nullptr, &tv);

    closesocket(sock);
    WSACleanup();
    return (result > 0);
}

void LaunchWalletProcess() {
    // Fast check if wallet server is already running (dev workflow: cargo run separately)
    // Uses raw TCP probe (~1ms if listening, ~100ms max if not) instead of WinHTTP
    // which ignores short timeouts on localhost.
    if (IsPortListening(31301)) {
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
    // Production: same directory as browser exe
    std::string walletExe = exeDir + "\\hodos-wallet.exe";
    if (GetFileAttributesA(walletExe.c_str()) == INVALID_FILE_ATTRIBUTES) {
        // Dev fallback: source tree relative path
        walletExe = exeDir + "\\..\\..\\..\\..\\rust-wallet\\target\\release\\hodos-wallet.exe";
    }

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

    g_walletProcessLaunched = true;
}

// Phase 2: Poll for wallet health with exponential backoff.
// Called in a background thread after CefInitialize. By this point the Rust
// process has had a 2-5 second head start, so it's often healthy on first check.
void WaitForWalletHealth() {
    // If already running (dev mode detected in LaunchWalletProcess), skip polling
    if (g_walletServerRunning) return;

    // If process wasn't launched, nothing to wait for
    if (!g_walletProcessLaunched) return;

    // Exponential backoff: 50, 100, 200, 400, 800, 1600ms (total 3.15s vs old 5s)
    int delays[] = {50, 100, 200, 400, 800, 1600};
    int elapsed = 0;
    for (int i = 0; i < 6; i++) {
        Sleep(delays[i]);
        elapsed += delays[i];
        if (QuickHealthCheck()) {
            LOG_INFO("Wallet server is healthy after " + std::to_string(elapsed) + "ms");
            g_walletServerRunning = true;
            return;
        }
    }

    LOG_WARNING("Wallet server did not become healthy within ~3s - continuing anyway");
    g_walletServerRunning = true;  // Process was launched, just slow to start
}

// Stop the Rust wallet server subprocess — graceful first, forceful fallback
void StopWalletServer() {
    if (!g_walletServerRunning) return;

    if (g_walletServerProcess.hProcess) {
        // Step 1: Try graceful shutdown via HTTP (lets wallet flush SQLite WAL)
        LOG_INFO("Requesting graceful wallet server shutdown (PID: " +
                 std::to_string(g_walletServerProcess.dwProcessId) + ")...");
        bool shutdownSent = SendShutdownRequest(31301);

        if (shutdownSent) {
            // Wait up to 5 seconds for graceful exit
            DWORD waitResult = WaitForSingleObject(g_walletServerProcess.hProcess, 5000);
            if (waitResult == WAIT_OBJECT_0) {
                LOG_INFO("Wallet server exited gracefully");
            } else {
                LOG_WARNING("Wallet server did not exit within 5s — force terminating");
                TerminateProcess(g_walletServerProcess.hProcess, 0);
                WaitForSingleObject(g_walletServerProcess.hProcess, 2000);
            }
        } else {
            // Step 2: Graceful request failed — force terminate
            LOG_WARNING("Graceful shutdown request failed — force terminating wallet server");
            TerminateProcess(g_walletServerProcess.hProcess, 0);
            WaitForSingleObject(g_walletServerProcess.hProcess, 2000);
        }

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

// ============================================================================
// Adblock Server Management
// ============================================================================

// Health check for adblock engine — checks GET /health on port 31302
// Returns true if response contains "ready" (engine loaded and ready to check)
// timeoutMs defaults to 500ms for polling; use 200ms for pre-launch checks.
static bool QuickAdblockHealthCheck(DWORD timeoutMs = 500) {
    HINTERNET hSession = WinHttpOpen(L"HodosBrowser/AdblockCheck",
        WINHTTP_ACCESS_TYPE_DEFAULT_PROXY,
        WINHTTP_NO_PROXY_NAME, WINHTTP_NO_PROXY_BYPASS, 0);
    if (!hSession) return false;

    DWORD timeout = timeoutMs;
    WinHttpSetOption(hSession, WINHTTP_OPTION_CONNECT_TIMEOUT, &timeout, sizeof(timeout));
    WinHttpSetOption(hSession, WINHTTP_OPTION_RECEIVE_TIMEOUT, &timeout, sizeof(timeout));
    WinHttpSetOption(hSession, WINHTTP_OPTION_SEND_TIMEOUT, &timeout, sizeof(timeout));

    HINTERNET hConnect = WinHttpConnect(hSession, L"localhost", 31302, 0);
    if (!hConnect) { WinHttpCloseHandle(hSession); return false; }

    HINTERNET hRequest = WinHttpOpenRequest(hConnect, L"GET", L"/health",
        nullptr, WINHTTP_NO_REFERER, WINHTTP_DEFAULT_ACCEPT_TYPES, 0);
    if (!hRequest) { WinHttpCloseHandle(hConnect); WinHttpCloseHandle(hSession); return false; }

    BOOL ok = WinHttpSendRequest(hRequest, WINHTTP_NO_ADDITIONAL_HEADERS, 0,
        WINHTTP_NO_REQUEST_DATA, 0, 0, 0);
    if (!ok) { WinHttpCloseHandle(hRequest); WinHttpCloseHandle(hConnect); WinHttpCloseHandle(hSession); return false; }

    ok = WinHttpReceiveResponse(hRequest, nullptr);
    bool ready = false;
    if (ok) {
        DWORD dwSize = 0;
        WinHttpQueryDataAvailable(hRequest, &dwSize);
        if (dwSize > 0 && dwSize < 4096) {
            std::vector<char> buf(dwSize + 1, 0);
            DWORD dwRead = 0;
            WinHttpReadData(hRequest, buf.data(), dwSize, &dwRead);
            std::string body(buf.data(), dwRead);
            ready = (body.find("\"ready\"") != std::string::npos);
        }
    }

    WinHttpCloseHandle(hRequest);
    WinHttpCloseHandle(hConnect);
    WinHttpCloseHandle(hSession);
    return ready;
}

// Phase 1: Launch adblock engine process (non-blocking). Called early in startup
// so the Rust engine warms up during CefInitialize.
void LaunchAdblockProcess() {
    // Fast check if adblock engine is already running (dev mode)
    if (IsPortListening(31302)) {
        LOG_INFO("Adblock engine already running (dev mode) - skipping launch");
        g_adblockServerRunning = true;
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
    // Production: same directory as browser exe
    std::string adblockExe = exeDir + "\\hodos-adblock.exe";
    if (GetFileAttributesA(adblockExe.c_str()) == INVALID_FILE_ATTRIBUTES) {
        // Dev fallback: source tree relative path
        adblockExe = exeDir + "\\..\\..\\..\\..\\adblock-engine\\target\\release\\hodos-adblock.exe";
    }

    // Check if the exe exists
    if (GetFileAttributesA(adblockExe.c_str()) == INVALID_FILE_ATTRIBUTES) {
        LOG_WARNING("Adblock engine executable not found: " + adblockExe);
        LOG_WARNING("Browser will run without ad blocking");
        return;
    }

    LOG_INFO("Launching adblock engine: " + adblockExe);

    STARTUPINFOA si;
    ZeroMemory(&si, sizeof(si));
    si.cb = sizeof(si);
    si.dwFlags = STARTF_USESHOWWINDOW;
    si.wShowWindow = SW_HIDE;

    ZeroMemory(&g_adblockServerProcess, sizeof(PROCESS_INFORMATION));

    if (!CreateProcessA(
        adblockExe.c_str(),
        nullptr,
        nullptr,
        nullptr,
        FALSE,
        CREATE_NO_WINDOW,
        nullptr,
        nullptr,
        &si,
        &g_adblockServerProcess)) {
        LOG_WARNING("Failed to launch adblock engine. Error: " + std::to_string(GetLastError()));
        LOG_WARNING("Browser will run without ad blocking");
        return;
    }

    LOG_INFO("Adblock engine process created (PID: " + std::to_string(g_adblockServerProcess.dwProcessId) + ")");

    // Assign to Job Object — auto-kill when browser exits
    g_adblockJobObject = CreateJobObject(nullptr, nullptr);
    if (g_adblockJobObject) {
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION jobInfo = {};
        jobInfo.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        SetInformationJobObject(g_adblockJobObject, JobObjectExtendedLimitInformation,
            &jobInfo, sizeof(jobInfo));
        AssignProcessToJobObject(g_adblockJobObject, g_adblockServerProcess.hProcess);
        LOG_INFO("Adblock engine assigned to job object (auto-kill on browser exit)");
    }

    g_adblockProcessLaunched = true;
}

// Phase 2: Poll for adblock health with exponential backoff.
void WaitForAdblockHealth() {
    // If already running (dev mode detected in LaunchAdblockProcess), skip polling
    if (g_adblockServerRunning) return;

    // If process wasn't launched, nothing to wait for
    if (!g_adblockProcessLaunched) return;

    // Exponential backoff: 50, 100, 200, 400, 800ms (total 1.55s vs old 3s)
    // Shorter than wallet since adblock is non-critical
    int delays[] = {50, 100, 200, 400, 800};
    int elapsed = 0;
    for (int i = 0; i < 5; i++) {
        Sleep(delays[i]);
        elapsed += delays[i];
        if (QuickAdblockHealthCheck()) {
            LOG_INFO("Adblock engine ready after " + std::to_string(elapsed) + "ms");
            g_adblockServerRunning = true;
            return;
        }
    }

    // Engine launched but still loading filter lists — mark as running
    // (it will respond to /check once ready; AdblockCache will get false until then)
    LOG_INFO("Adblock engine launched but still loading — ad blocking available once ready");
    g_adblockServerRunning = true;
}

// Stop the adblock engine subprocess — graceful first, forceful fallback
void StopAdblockServer() {
    if (!g_adblockServerRunning) return;

    if (g_adblockServerProcess.hProcess) {
        LOG_INFO("Requesting graceful adblock engine shutdown (PID: " +
                 std::to_string(g_adblockServerProcess.dwProcessId) + ")...");
        bool shutdownSent = SendShutdownRequest(31302);

        if (shutdownSent) {
            DWORD waitResult = WaitForSingleObject(g_adblockServerProcess.hProcess, 3000);
            if (waitResult == WAIT_OBJECT_0) {
                LOG_INFO("Adblock engine exited gracefully");
            } else {
                LOG_WARNING("Adblock engine did not exit within 3s — force terminating");
                TerminateProcess(g_adblockServerProcess.hProcess, 0);
                WaitForSingleObject(g_adblockServerProcess.hProcess, 2000);
            }
        } else {
            LOG_WARNING("Graceful shutdown request failed — force terminating adblock engine");
            TerminateProcess(g_adblockServerProcess.hProcess, 0);
            WaitForSingleObject(g_adblockServerProcess.hProcess, 2000);
        }

        CloseHandle(g_adblockServerProcess.hProcess);
        CloseHandle(g_adblockServerProcess.hThread);
        ZeroMemory(&g_adblockServerProcess, sizeof(PROCESS_INFORMATION));
    }

    if (g_adblockJobObject) {
        CloseHandle(g_adblockJobObject);
        g_adblockJobObject = nullptr;
    }

    g_adblockServerRunning = false;
    LOG_INFO("Adblock engine stopped");
}

int WINAPI WinMain(HINSTANCE hInstance, HINSTANCE, LPSTR, int nCmdShow) {
    // ── Startup performance timer ──
    auto t0 = std::chrono::steady_clock::now();
    auto elapsed = [&t0]() -> std::string {
        auto ms = std::chrono::duration_cast<std::chrono::milliseconds>(
            std::chrono::steady_clock::now() - t0).count();
        return "[T+" + std::to_string(ms) + "ms] ";
    };

    g_hInstance = hInstance;

    // Enable Per-Monitor DPI V2 awareness. Must be called before any window creation.
    // This ensures WM_DPICHANGED fires when dragging between monitors with different DPI,
    // and CEF browsers render at the correct scale factor for each monitor.
    SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);

    // Create the primary BrowserWindow (window 0) in WindowManager.
    // This must happen before any code that accesses MAIN_WINDOW().
    WindowManager::GetInstance().CreateWindowRecord();  // returns 0

    CefMainArgs main_args(hInstance);
    CefRefPtr<SimpleApp> app(new SimpleApp());

    int exit_code = CefExecuteProcess(main_args, app, nullptr);
    if (exit_code >= 0) return exit_code;

    // Initialize centralized logger FIRST
    Logger::Initialize(ProcessType::MAIN, "debug_output.log");
    LOG_INFO(elapsed() + "STARTUP: Logger initialized");

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
    settings.windowless_rendering_enabled = true;

    // Set base app data path
    std::string appdata_path = std::getenv("APPDATA") ? std::getenv("APPDATA") : "";
    std::string user_data_path = appdata_path + "\\HodosBrowser";

    // Initialize ProfileManager BEFORE CefInitialize so cache_path is correct
    LOG_INFO("Initializing ProfileManager...");
    if (!ProfileManager::GetInstance().Initialize(user_data_path)) {
        LOG_ERROR("Failed to initialize ProfileManager");
    }

    // Parse --profile argument from command line
    std::string profileId = ProfileManager::ParseProfileArgument(GetCommandLineW());
    ProfileManager::GetInstance().SetCurrentProfileId(profileId);
    LOG_INFO(elapsed() + "STARTUP: Profile parsed: " + profileId);

    // Get profile-specific data directory
    std::string profile_cache = ProfileManager::GetInstance().GetCurrentProfileDataPath();
    LOG_INFO("Profile data path: " + profile_cache);

    // B-6: Single-instance check — forward to running instance instead of error dialog.
    // Must be AFTER CefExecuteProcess (line 2968) so CEF subprocesses aren't affected,
    // and AFTER profile ID parsing so we use the correct pipe name.
    if (!SingleInstance::TryAcquireInstance(profileId)) {
        // Another instance owns this profile's pipe. Forward command to it.
        // Parse URL from command line if present (e.g., launched from file association).
        std::string launchUrl;
        int argc = 0;
        LPWSTR* argv = CommandLineToArgvW(GetCommandLineW(), &argc);
        if (argv) {
            for (int i = 1; i < argc; i++) {
                std::wstring arg(argv[i]);
                // Skip flags (--profile=, etc.)
                if (arg.size() > 0 && arg[0] != L'-') {
                    // Convert wide string URL to narrow
                    int len = WideCharToMultiByte(CP_UTF8, 0, arg.c_str(), -1, nullptr, 0, nullptr, nullptr);
                    if (len > 0) {
                        std::string narrow(len - 1, '\0');
                        WideCharToMultiByte(CP_UTF8, 0, arg.c_str(), -1, &narrow[0], len, nullptr, nullptr);
                        launchUrl = narrow;
                    }
                    break;
                }
            }
            LocalFree(argv);
        }

        if (SingleInstance::SendToRunningInstance(profileId, launchUrl)) {
            // Running instance acknowledged — exit cleanly (no error dialog).
            LOG_INFO("SingleInstance: Forwarded to running instance, exiting");
            return 0;
        }
        // SendToRunningInstance returned false — old instance exited during shutdown,
        // and we became the new first instance. Continue normal startup.
        LOG_INFO("SingleInstance: Became new first instance after shutdown handoff");
    }

    // B-6: Start the pipe listener thread EARLY so second instances can connect
    // immediately. The listener will respond "not_ready" until g_hwnd is set.
    SingleInstance::StartListenerThread(profileId);
    LOG_INFO(elapsed() + "STARTUP: SingleInstance check done");

    // Acquire exclusive lock on profile directory (prevents SQLite corruption)
    if (!AcquireProfileLock(profile_cache)) {
        MessageBoxA(nullptr,
            ("Profile \"" + profileId + "\" is already in use by another instance.\n\n"
             "Close the other instance first, or launch with a different profile.").c_str(),
            "Hodos Browser - Profile Locked",
            MB_OK | MB_ICONERROR);
        return 1;
    }
    LOG_INFO(elapsed() + "STARTUP: Profile lock acquired");

    // Launch backend processes EARLY so they warm up during CefInitialize (2-5s).
    // Phase 1 only: CreateProcess + job object, no health polling yet.
    LOG_INFO(elapsed() + "STARTUP: Launching backend processes");
    LaunchWalletProcess();
    LaunchAdblockProcess();

    // Initialize SettingsManager with profile-specific path
    SettingsManager::GetInstance().Initialize(profile_cache);
    LOG_INFO(elapsed() + "STARTUP: Settings loaded for profile: " + profileId);

    // Initialize AdblockCache with profile path (loads per-site settings from JSON)
    AdblockCache::GetInstance().Initialize(profile_cache);
    // Sync global ad-block toggle from persisted settings
    AdblockCache::GetInstance().SetGlobalEnabled(
        SettingsManager::GetInstance().GetPrivacySettings().adBlockEnabled);
    LOG_INFO(elapsed() + "STARTUP: AdblockCache settings loaded");

    // Sprint 12c: Initialize fingerprint protection session token
    FingerprintProtection::GetInstance().Initialize();
    // Load per-site fingerprint overrides from fingerprint_settings.json
    FingerprintProtection::GetInstance().LoadSiteSettings(profile_cache);
    // Sync global toggle from persisted settings
    FingerprintProtection::GetInstance().SetEnabled(
        SettingsManager::GetInstance().GetPrivacySettings().fingerprintProtection);
    LOG_INFO("Fingerprint protection initialized (enabled=" +
        std::string(SettingsManager::GetInstance().GetPrivacySettings().fingerprintProtection ? "true" : "false") + ")");

    // Each profile instance needs its own root to avoid CEF SingletonLock conflicts
    // root_cache_path = profile dir, cache_path = profile dir + /cache (must be child of root)
    CefString(&settings.root_cache_path).FromString(profile_cache);
    std::string cache_subdir = profile_cache + "\\cache";
    CefString(&settings.cache_path).FromString(cache_subdir);

    // Remote debugging port: each profile gets a unique port so multiple instances can coexist
    // Default=9222, others get 9223+ based on profile number, or 0 to disable
    if (profileId == "Default") {
        settings.remote_debugging_port = 9222;
    } else {
        // Extract number from profile ID (e.g., "Profile_2" -> 2) for port offset
        int portOffset = 0;
        size_t underscorePos = profileId.find('_');
        if (underscorePos != std::string::npos) {
            try { portOffset = std::stoi(profileId.substr(underscorePos + 1)); } catch (...) {}
        }
        settings.remote_debugging_port = 9222 + portOffset;
    }
    LOG_INFO("Remote debugging port: " + std::to_string(settings.remote_debugging_port));

    // Persist session cookies across browser restarts
    // TEMPORARILY DISABLED - testing if this causes extra windows
    // settings.persist_session_cookies = true;

    // Enable CEF's runtime API for JavaScript communication
    CefString(&settings.javascript_flags).FromASCII("--expose-gc");

    // Get the executable path for subprocess
    wchar_t exe_path[MAX_PATH];
    GetModuleFileNameW(nullptr, exe_path, MAX_PATH);

    // Set CEF resource paths — production vs dev
    {
        char exe_path_a[MAX_PATH];
        GetModuleFileNameA(nullptr, exe_path_a, MAX_PATH);
        std::string exe_dir_str(exe_path_a);
        size_t ls = exe_dir_str.find_last_of("\\/");
        if (ls != std::string::npos) exe_dir_str = exe_dir_str.substr(0, ls);

        std::string res_pak = exe_dir_str + "\\resources.pak";
        if (GetFileAttributesA(res_pak.c_str()) != INVALID_FILE_ATTRIBUTES) {
            // Production: resources are next to the exe
            CefString(&settings.resources_dir_path).FromString(exe_dir_str);
            CefString(&settings.locales_dir_path).FromString(exe_dir_str + "\\locales");
        } else {
            // Dev: use relative path to cef-binaries
            CefString(&settings.resources_dir_path).FromWString(L"cef-binaries\\Resources");
            CefString(&settings.locales_dir_path).FromWString(L"cef-binaries\\Resources\\locales");
        }
    }
    CefString(&settings.browser_subprocess_path).FromWString(exe_path);

    RECT rect;
    SystemParametersInfo(SPI_GETWORKAREA, 0, &rect, 0);
    int width  = rect.right - rect.left;
    int height = rect.bottom - rect.top;
    // Fixed header height, DPI-scaled (no HWND yet, use system DPI)
    int shellHeight = GetHeaderHeightPxSystem();
    int webviewHeight = height - shellHeight;

    LOG_INFO(elapsed() + "STARTUP: Registering window classes...");
    WNDCLASS wc = {}; wc.lpfnWndProc = ShellWindowProc; wc.hInstance = hInstance;
    wc.lpszClassName = L"HodosBrowserWndClass";
    wc.hbrBackground = CreateSolidBrush(RGB(26, 26, 26));
    RegisterClass(&wc);

    WNDCLASS browserClass = {};
    browserClass.style = CS_HREDRAW | CS_VREDRAW | CS_DBLCLKS;  // Redraw on resize, no border styles
    browserClass.lpfnWndProc = DefWindowProc;
    browserClass.hInstance = hInstance;
    browserClass.lpszClassName = L"CEFHostWindow";
    browserClass.hbrBackground = CreateSolidBrush(RGB(26, 26, 26));  // Dark theme background
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

    // Register Download Panel overlay window class
    WNDCLASS downloadPanelOverlayClass = {};
    downloadPanelOverlayClass.lpfnWndProc = DownloadPanelOverlayWndProc;
    downloadPanelOverlayClass.hInstance = hInstance;
    downloadPanelOverlayClass.lpszClassName = L"CEFDownloadPanelOverlayWindow";

    if (!RegisterClass(&downloadPanelOverlayClass)) {
        LOG_DEBUG("Failed to register download panel overlay window class. Error: " + std::to_string(GetLastError()));
    }

    // Register Profile Panel overlay window class
    WNDCLASS profilePanelOverlayClass = {};
    profilePanelOverlayClass.lpfnWndProc = ProfilePanelOverlayWndProc;
    profilePanelOverlayClass.hInstance = hInstance;
    profilePanelOverlayClass.lpszClassName = L"CEFProfilePanelOverlayWindow";

    if (!RegisterClass(&profilePanelOverlayClass)) {
        LOG_DEBUG("Failed to register profile panel overlay window class. Error: " + std::to_string(GetLastError()));
    }

    // Register Menu overlay window class
    WNDCLASS menuOverlayClass = {};
    menuOverlayClass.lpfnWndProc = MenuOverlayWndProc;
    menuOverlayClass.hInstance = hInstance;
    menuOverlayClass.lpszClassName = L"CEFMenuOverlayWindow";

    if (!RegisterClass(&menuOverlayClass)) {
        LOG_DEBUG("Failed to register menu overlay window class. Error: " + std::to_string(GetLastError()));
    }

    LOG_INFO(elapsed() + "STARTUP: Creating main window...");
    HWND hwnd = CreateWindow(L"HodosBrowserWndClass", L"Hodos Browser",
        WS_POPUP | WS_THICKFRAME | WS_SYSMENU | WS_MINIMIZEBOX | WS_MAXIMIZEBOX | WS_CLIPCHILDREN,
        rect.left, rect.top, width, height, nullptr, nullptr, hInstance, nullptr);

    // Set window icon explicitly (preserves 32-bit RGBA alpha for rounded corners)
    HICON hIconLarge = (HICON)LoadImage(hInstance, MAKEINTRESOURCE(1), IMAGE_ICON, 256, 256, LR_DEFAULTCOLOR);
    HICON hIconSmall = (HICON)LoadImage(hInstance, MAKEINTRESOURCE(1), IMAGE_ICON, 32, 32, LR_DEFAULTCOLOR);
    if (hIconLarge) SendMessage(hwnd, WM_SETICON, ICON_BIG, (LPARAM)hIconLarge);
    if (hIconSmall) SendMessage(hwnd, WM_SETICON, ICON_SMALL, (LPARAM)hIconSmall);

    // Enable DWM invisible resize borders outside the window bounds.
    // The {0,0,0,1} margin tells DWM this window has a frame, enabling
    // compositor-level resize hit-testing that works even with child HWNDs covering the client area.
    MARGINS dwmMargins = {0, 0, 0, 1};
    DwmExtendFrameIntoClientArea(hwnd, &dwmMargins);

    const int rb = 5; // resize border inset — must match ShellWindowProc WM_NCHITTEST/WM_SIZE
    HWND header_hwnd = CreateWindow(L"CEFHostWindow", nullptr,
        WS_CHILD | WS_VISIBLE, rb, rb, width - 2 * rb, shellHeight, hwnd, nullptr, hInstance, nullptr);

    // OLD: Single webview window - NO LONGER USED WITH TAB SYSTEM
    // Kept for compatibility but made invisible (tabs now handle content display)
    HWND webview_hwnd = CreateWindow(L"CEFHostWindow", nullptr,
        WS_CHILD, rb, rb + shellHeight, width - 2 * rb, webviewHeight - 2 * rb, hwnd, nullptr, hInstance, nullptr);
    // Note: Removed WS_VISIBLE - this window was blocking input to tabs!

    // 🌍 Assign to globals + sync to BrowserWindow 0
    g_hwnd = hwnd;
    g_header_hwnd = header_hwnd;
    g_webview_hwnd = webview_hwnd;

    // Mirror into BrowserWindow 0 for WindowManager-based lookups
    BrowserWindow* mainWin = WindowManager::GetInstance().GetPrimaryWindow();
    if (mainWin) {
        mainWin->hwnd = hwnd;
        mainWin->header_hwnd = header_hwnd;
        mainWin->webview_hwnd = webview_hwnd;
    }

    // Store BrowserWindow* in HWND user data so ShellWindowProc can find it
    SetWindowLongPtr(hwnd, GWLP_USERDATA, reinterpret_cast<LONG_PTR>(mainWin));

    // Show window immediately so user sees a dark-themed shell within ~100ms.
    // The header_hwnd child and shell class brush are both RGB(26,26,26),
    // so the window appears as a cohesive dark frame. CEF content replaces it
    // once the header browser loads React.
    ShowWindow(hwnd, SW_SHOW);
    UpdateWindow(hwnd);
    g_window_shown = true;

    // Pump pending window messages so the DWM compositor actually presents the
    // window to screen BEFORE CefInitialize blocks the UI thread for 2-5 seconds.
    // Without this, ShowWindow/UpdateWindow queue internally but the compositor
    // never gets a chance to render until the blocking call returns.
    {
        MSG msg;
        while (PeekMessage(&msg, nullptr, 0, 0, PM_REMOVE)) {
            TranslateMessage(&msg);
            DispatchMessage(&msg);
        }
    }
    // Block until DWM has composited our frame to screen (~6-16ms).
    // This guarantees the skeleton toolbar is visible before CefInitialize blocks.
    DwmFlush();
    LOG_INFO(elapsed() + "STARTUP: Window shown + DwmFlush complete");

    LOG_INFO(elapsed() + "STARTUP: CefInitialize starting...");
    bool success = CefInitialize(main_args, settings, app, nullptr);
    LOG_INFO(elapsed() + "STARTUP: CefInitialize done (success=" + std::string(success ? "true" : "false") + ")");

    if (!success) {
        ReleaseProfileLock();
        return 1;
    }

    // Parallelize DB initialization. Backend processes were already launched
    // before CefInitialize (LaunchWalletProcess/LaunchAdblockProcess), so they've
    // had 2-5 seconds of head start. Health polling runs in detached threads.
    LOG_INFO("Starting parallel initialization (3 DBs + 2 health checks)...");
    auto initStart = std::chrono::steady_clock::now();

    std::thread historyThread([&profile_cache]() {
        if (HistoryManager::GetInstance().Initialize(profile_cache)) {
            LOG_INFO("HistoryManager initialized successfully");
        } else {
            LOG_ERROR("Failed to initialize HistoryManager");
        }
    });

    std::thread cookieThread([&profile_cache]() {
        if (CookieBlockManager::GetInstance().Initialize(profile_cache)) {
            LOG_INFO("CookieBlockManager initialized successfully");
        } else {
            LOG_ERROR("Failed to initialize CookieBlockManager");
        }
    });

    std::thread bookmarkThread([&profile_cache]() {
        if (BookmarkManager::GetInstance().Initialize(profile_cache)) {
            LOG_INFO("BookmarkManager initialized successfully");
        } else {
            LOG_ERROR("Failed to initialize BookmarkManager");
        }
    });

    // Phase 2: health polling in detached threads — don't block startup.
    // The atomic g_walletServerRunning / g_adblockServerRunning flags are set
    // when healthy. Callers handle the "not yet ready" case gracefully.
    std::thread walletThread([]() {
        LOG_INFO("Polling wallet server health...");
        WaitForWalletHealth();
    });

    std::thread adblockThread([]() {
        LOG_INFO("Polling adblock engine health...");
        WaitForAdblockHealth();
    });

    // Join ONLY the fast DB threads (~100ms). Server health threads run independently.
    historyThread.join();
    cookieThread.join();
    bookmarkThread.join();

    // Detach server health threads — they set atomic flags when done
    walletThread.detach();
    adblockThread.detach();

    auto initMs = std::chrono::duration_cast<std::chrono::milliseconds>(
        std::chrono::steady_clock::now() - initStart).count();
    LOG_INFO(elapsed() + "STARTUP: DB init done in " + std::to_string(initMs) + "ms (server health in background)");

    // Pass handles to app instance
    app->SetWindowHandles(hwnd, header_hwnd, webview_hwnd);

    // Safety timeout: ensure window is visible (should already be shown at creation).
    // Kept as a defensive fallback in case early ShowWindow was somehow skipped.
    CefPostDelayedTask(TID_UI, base::BindOnce([]() {
        extern HWND g_hwnd;
        extern bool g_window_shown;
        if (!g_window_shown && g_hwnd && IsWindow(g_hwnd)) {
            LOG_WARNING("Startup safety timeout - force-showing window");
            ShowWindow(g_hwnd, SW_SHOW);
            UpdateWindow(g_hwnd);
            g_window_shown = true;
        }
    }), 2000);

    // Defer overlay pre-creation to after the message loop starts.
    // Staggered at 500ms intervals so they don't compete with the header browser
    // and initial tab creation for CPU/memory. Each overlay spawns a CEF subprocess
    // and loads React, which takes ~300-500ms per overlay.
    {
        extern void CreateCookiePanelOverlay(HINSTANCE hInstance, bool showImmediately, int iconRightOffset);
        extern void CreateDownloadPanelOverlay(HINSTANCE hInstance, bool showImmediately, int iconRightOffset);
        extern void CreateProfilePanelOverlay(HINSTANCE hInstance, bool showImmediately, int iconRightOffset);
        extern void CreateMenuOverlay(HINSTANCE hInstance, bool showImmediately, int iconRightOffset);
        extern void CreateWalletOverlay(HINSTANCE hInstance, bool showImmediately, int iconRightOffset);

        HINSTANCE hInst = g_hInstance;
        CefPostDelayedTask(TID_UI, base::BindOnce([](HINSTANCE h) { CreateMenuOverlay(h, false, 30); }, hInst), 1000);
        CefPostDelayedTask(TID_UI, base::BindOnce([](HINSTANCE h) { CreateWalletOverlay(h, false, 50); }, hInst), 1500);
        CefPostDelayedTask(TID_UI, base::BindOnce([](HINSTANCE h) { CreateDownloadPanelOverlay(h, false, 100); }, hInst), 2000);
        CefPostDelayedTask(TID_UI, base::BindOnce([](HINSTANCE h) { CreateCookiePanelOverlay(h, false, 100); }, hInst), 2500);
        CefPostDelayedTask(TID_UI, base::BindOnce([](HINSTANCE h) { CreateProfilePanelOverlay(h, false, 50); }, hInst), 3000);
        LOG_INFO(elapsed() + "STARTUP: Overlay creation deferred");
    }

    // Initialize auto-updater after windows are created.
    // WinSparkle will check for updates in the background if auto-check is enabled.
    {
        auto& settings = SettingsManager::GetInstance();
        auto browserSettings = settings.GetBrowserSettings();
        bool autoCheck = browserSettings.autoUpdateEnabled;
        bool notifications = browserSettings.autoUpdateNotifications;
        std::string appVersion = APP_VERSION; // Injected by CMake via -DAPP_VERSION=
        std::string appcastUrl = "https://hodosbrowser.com/appcast.xml";

        auto& updater = AutoUpdater::GetInstance();
        updater.SetShutdownCallback([]() {
            // WinSparkle needs us to shut down so the installer can run.
            // Post WM_CLOSE to the main window to trigger graceful shutdown.
            extern HWND g_hwnd;
            if (g_hwnd && IsWindow(g_hwnd)) {
                PostMessage(g_hwnd, WM_CLOSE, 0, 0);
            }
        });
        // Auto-check only runs with periodic dialog notifications if both flags are on.
        // When notifications OFF (default), WinSparkle is initialized but won't show
        // periodic dialogs — user can still manually check via Settings > About.
        updater.Initialize(appVersion, appcastUrl, autoCheck && notifications);
        LOG_INFO("Auto-updater initialized (version=" + appVersion +
                 ", autoCheck=" + std::string(autoCheck ? "true" : "false") +
                 ", notifications=" + std::string(notifications ? "true" : "false") + ")");
    }

    LOG_INFO(elapsed() + "STARTUP: Entering CefRunMessageLoop");
    CefRunMessageLoop();

    // All browsers have closed (OnBeforeClose posted quit message).
    // Now safe to clean up remaining resources and call CefShutdown().
    LOG_INFO("Message loop exited — final cleanup...");

    // Destroy any remaining HWNDs
    if (g_header_hwnd && IsWindow(g_header_hwnd)) {
        DestroyWindow(g_header_hwnd);
        g_header_hwnd = nullptr;
    }
    if (g_webview_hwnd && IsWindow(g_webview_hwnd)) {
        DestroyWindow(g_webview_hwnd);
        g_webview_hwnd = nullptr;
    }
    if (g_hwnd && IsWindow(g_hwnd)) {
        DestroyWindow(g_hwnd);
        g_hwnd = nullptr;
    }

    // Stop child servers before CEF shutdown (defensive — may already be stopped)
    LOG_INFO("Stopping wallet server...");
    StopWalletServer();
    LOG_INFO("Stopping adblock engine...");
    StopAdblockServer();

    ReleaseProfileLock();

    Logger::Shutdown();
    CefShutdown();
    return 0;
}
