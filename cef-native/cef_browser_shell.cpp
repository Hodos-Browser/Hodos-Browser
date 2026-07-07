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
#include "include/core/AppPaths.h"
#include "include/core/WalletService.h"
#include "include/core/PortConfig.h"
#include "include/core/TabManager.h"
#include "include/core/HistoryManager.h"
#include "include/core/CookieBlockManager.h"
#include "include/core/BookmarkManager.h"
#include "include/core/SitePermissionStore.h"
#include "include/core/PaidContentCache.h"
#include "include/core/SettingsManager.h"
#include "include/core/FingerprintProtection.h"
#include "include/core/ProfileManager.h"
#include "include/core/ProfileLock.h"
#include "include/core/TaskbarProfile.h"
#include "include/core/SingleInstance.h"
#include "include/core/AdblockCache.h"
#include "include/core/WindowManager.h"
#include "include/core/AutoUpdater.h"
#include "include/core/UpdateStager.h"
#include "include/core/UpdateApply.h"
#include "include/core/UpdateFs.h"
#include "include/core/UpdateLock.h"
#include "include/core/SilentStateWriter.h"
#include "include/core/LayoutHelpers.h"
#ifdef HODOS_SILENT_AUTOUPDATE
#include <tlhelp32.h>   // 6c.2: D.0 all-instances-gone sibling enumeration
#include "update-helper/splash.h"   // A1: shell-side "Hodos is updating…" splash before backup
#endif
#include "include/core/Logger.h"
#include <shellapi.h>
#include <objbase.h>   // CoInitializeEx for taskbar profile integration
#include <shobjidl.h>  // SetCurrentProcessExplicitAppUserModelID
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
#include <chrono>
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
HWND g_bookmarks_panel_overlay_hwnd = nullptr;
HWND g_siteinfo_panel_overlay_hwnd = nullptr;
HWND g_tablist_panel_overlay_hwnd = nullptr;

// File dialog guard — prevents overlay close when a native file dialog is open
bool g_file_dialog_active = false;

// Wallet close prevention — prevents overlay close during mnemonic display / PIN creation
bool g_wallet_overlay_prevent_close = false;

// Pre-window profile-picker mode (CHUNK 2). When true, THIS process owns no
// profile: it shows the chooser window (React /profile-picker), then spawns the
// chosen profile via --profile= and exits. Set once in main() before window
// creation; read by ShellWindowProc (WM_SIZE), SimpleApp::OnContextInitialized
// (URL + skip tabs), and the profiles_switch IPC (spawn-then-close).
bool g_picker_mode = false;

// Timestamps of last hide — used to suppress toggle race condition
// (WM_ACTIVATE hides overlay before toggle IPC arrives, causing re-open)
ULONGLONG g_wallet_last_hide_tick = 0;
ULONGLONG g_profile_last_hide_tick = 0;
ULONGLONG g_profile_last_show_tick = 0;  // Suppress immediate WM_ACTIVATE hide after show
ULONGLONG g_bookmarks_last_show_tick = 0;  // Suppress immediate WM_ACTIVATE hide after show
ULONGLONG g_tablist_last_show_tick = 0;    // Same guard for the tab-list overlay (MA_ACTIVATE)
// Site-info hub: the mouse hook hides the panel on the SAME click that hits the
// TuneIcon (the icon is outside the overlay rect), microseconds before the async
// siteinfo_panel_show IPC arrives. Without this, the toggle-off re-opens instead of
// closing. The IPC's "not visible" branch suppresses re-show within the guard window.
ULONGLONG g_siteinfo_last_hide_tick = 0;
// Same same-click-toggle-off guard for bookmarks + tab-list now that they also install a
// click-outside mouse hook (B2). The mouse hook hides on the button click microseconds
// before the *_panel_show IPC lands; the toggle's hide-tick branch suppresses the re-show.
ULONGLONG g_bookmarks_last_hide_tick = 0;
ULONGLONG g_tablist_last_hide_tick = 0;

// Global mouse hooks for overlay click-outside detection
HHOOK g_omnibox_mouse_hook = nullptr;
HHOOK g_cookie_panel_mouse_hook = nullptr;
HHOOK g_download_panel_mouse_hook = nullptr;
HHOOK g_profile_panel_mouse_hook = nullptr;
HHOOK g_settings_mouse_hook = nullptr;
HHOOK g_menu_mouse_hook = nullptr;
HHOOK g_bookmarks_panel_mouse_hook = nullptr;
HHOOK g_siteinfo_panel_mouse_hook = nullptr;
HHOOK g_tablist_panel_mouse_hook = nullptr;

// Stored icon right offsets for repositioning overlays on WM_SIZE/WM_MOVE
// (physical pixel distance from icon's right edge to header's right edge)
int g_settings_icon_right_offset = 0;
int g_cookie_icon_right_offset = 0;
int g_download_icon_right_offset = 0;
int g_profile_icon_right_offset = 0;
int g_wallet_icon_right_offset = 0;
int g_menu_icon_right_offset = 0;
// Bookmarks dropdown is LEFT-anchored (button sits left of the address bar), so it
// stores a LEFT offset (physical px from header's left edge to the button's left).
int g_bookmarks_icon_left_offset = 0;
// Site-info dropdown is LEFT-anchored (TuneIcon at the address-bar left).
int g_siteinfo_icon_left_offset = 0;
// Tab-list caret is LEFT-anchored (caret at the left of the tab strip).
int g_tablist_icon_left_offset = 0;
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

// Set true at the top of ShutdownApplication() so the (detached) silent-update
// staging thread stops downloading / logging before Logger teardown. Polled by
// that thread and threaded into StagePendingUpdate's abort check. (Commit 4d.)
static std::atomic<bool> g_update_abort{false};

// 6d: set in the 6c.1 honor-probe block when THIS launch is the silent-update
// supervisor's --post-update-health-probe relaunch (arg + armed apply.json). A
// detached thread then waits for the children to be healthy and writes
// apply.json=healthy so the supervisor confirms the apply instead of rolling back.
static std::atomic<bool> g_post_update_probe{false};
static long g_post_update_to_build = 0;  // expected build from the armed apply.json

// UTF-8 (AppPaths) -> wide. Always compiled (used by the 6c.1 honor-probe + 6d
// health marker, which are NOT behind HODOS_SILENT_AUTOUPDATE).
static std::wstring HodosUtf8ToWide(const std::string& s) {
    if (s.empty()) return L"";
    int n = MultiByteToWideChar(CP_UTF8, 0, s.c_str(), -1, nullptr, 0);
    std::wstring w(n > 0 ? n - 1 : 0, L'\0');
    if (n > 0) MultiByteToWideChar(CP_UTF8, 0, s.c_str(), -1, &w[0], n);
    return w;
}

// 6a (WINDOWS_AUTOUPDATE_PLAN §D.0 / OD-D): a session-namespace mutex marking THIS
// HodosBrowser.exe as a live instance for the auto-update all-instances-gone gate.
// Created at startup (profile AND picker modes), held for the process lifetime,
// closed in main()'s final cleanup. INERT in 6a — checked by nothing yet; commit 6c
// detects live siblings via OpenMutexW on this name before applying a staged update.
static HANDLE g_instance_mutex = nullptr;

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
    // Signal the detached silent-update staging thread to stop BEFORE Logger
    // teardown, so a long download can't race Logger::Shutdown (commit 4d).
    g_update_abort.store(true);
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

    // R2/R3: the profile lock is intentionally NOT released here anymore. It used
    // to be freed early (for fast relaunch), but the SQLite browser DBs stay open
    // until much later (singleton destructors at process exit) — so a quick
    // relaunch could win the freed lock and open a live-WAL DB (SQLITE_BUSY). The
    // lock is now released in main()'s final cleanup, AFTER an explicit DB-close
    // cascade. See the "final cleanup" block in main().

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

    if (g_bookmarks_panel_overlay_hwnd && IsWindow(g_bookmarks_panel_overlay_hwnd)) {
        LOG_INFO("Destroying bookmarks panel overlay window...");
        if (g_bookmarks_panel_mouse_hook) {
            UnhookWindowsHookEx(g_bookmarks_panel_mouse_hook);
            g_bookmarks_panel_mouse_hook = nullptr;
            LOG_INFO("Bookmarks panel mouse hook removed during shutdown");
        }
        DestroyWindow(g_bookmarks_panel_overlay_hwnd);
        g_bookmarks_panel_overlay_hwnd = nullptr;
    }

    if (g_siteinfo_panel_overlay_hwnd && IsWindow(g_siteinfo_panel_overlay_hwnd)) {
        LOG_INFO("Destroying site-info panel overlay window...");
        if (g_siteinfo_panel_mouse_hook) {
            UnhookWindowsHookEx(g_siteinfo_panel_mouse_hook);
            g_siteinfo_panel_mouse_hook = nullptr;
            LOG_INFO("Site-info panel mouse hook removed during shutdown");
        }
        DestroyWindow(g_siteinfo_panel_overlay_hwnd);
        g_siteinfo_panel_overlay_hwnd = nullptr;
    }

    if (g_tablist_panel_overlay_hwnd && IsWindow(g_tablist_panel_overlay_hwnd)) {
        LOG_INFO("Destroying tab-list panel overlay window...");
        if (g_tablist_panel_mouse_hook) {
            UnhookWindowsHookEx(g_tablist_panel_mouse_hook);
            g_tablist_panel_mouse_hook = nullptr;
            LOG_INFO("Tab-list panel mouse hook removed during shutdown");
        }
        DestroyWindow(g_tablist_panel_overlay_hwnd);
        g_tablist_panel_overlay_hwnd = nullptr;
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
    extern void HideSiteInfoPanelOverlay();

    if (g_omnibox_overlay_hwnd && IsWindow(g_omnibox_overlay_hwnd) && IsWindowVisible(g_omnibox_overlay_hwnd))
        HideOmniboxOverlay();
    if (g_cookie_panel_overlay_hwnd && IsWindow(g_cookie_panel_overlay_hwnd) && IsWindowVisible(g_cookie_panel_overlay_hwnd))
        HideCookiePanelOverlay();
    if (g_download_panel_overlay_hwnd && IsWindow(g_download_panel_overlay_hwnd) && IsWindowVisible(g_download_panel_overlay_hwnd))
        HideDownloadPanelOverlay();
    if (g_siteinfo_panel_overlay_hwnd && IsWindow(g_siteinfo_panel_overlay_hwnd) && IsWindowVisible(g_siteinfo_panel_overlay_hwnd))
        HideSiteInfoPanelOverlay();
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
        case WM_GETMINMAXINFO: {
            // WS_POPUP windows ignore the taskbar on maximize. Clamp the maximize
            // size/position to the current monitor's work area so the browser
            // doesn't cover the taskbar when maximized.
            HMONITOR hMonitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
            MONITORINFO mi = { sizeof(mi) };
            if (GetMonitorInfo(hMonitor, &mi)) {
                MINMAXINFO* mmi = reinterpret_cast<MINMAXINFO*>(lParam);
                mmi->ptMaxPosition.x = mi.rcWork.left - mi.rcMonitor.left;
                mmi->ptMaxPosition.y = mi.rcWork.top - mi.rcMonitor.top;
                mmi->ptMaxSize.x = mi.rcWork.right - mi.rcWork.left;
                mmi->ptMaxSize.y = mi.rcWork.bottom - mi.rcWork.top;
                mmi->ptMaxTrackSize.x = mmi->ptMaxSize.x;
                mmi->ptMaxTrackSize.y = mmi->ptMaxSize.y;
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

            // Move site-info hub overlay if it exists and is visible (LEFT-anchored)
            if (g_siteinfo_panel_overlay_hwnd && IsWindow(g_siteinfo_panel_overlay_hwnd) && IsWindowVisible(g_siteinfo_panel_overlay_hwnd)) {
                RECT hdrRect;
                GetWindowRect(g_header_hwnd, &hdrRect);
                int siWidth = ScalePx(360, hwnd);
                // Preserve the auto-sized height (set by siteinfo_panel_resize); don't
                // force a fixed height or a window move would undo the content fit.
                RECT siCur; GetWindowRect(g_siteinfo_panel_overlay_hwnd, &siCur);
                int siHeight = siCur.bottom - siCur.top;
                int siX = hdrRect.left + g_siteinfo_icon_left_offset;
                int siY = hdrRect.top + ScalePx(104, hwnd);
                if (siX + siWidth > mainRect.right - ScalePx(8, hwnd))
                    siX = mainRect.right - siWidth - ScalePx(8, hwnd);
                if (siX < mainRect.left + ScalePx(8, hwnd))
                    siX = mainRect.left + ScalePx(8, hwnd);
                if (siY + siHeight > mainRect.bottom - ScalePx(20, hwnd)) {
                    siHeight = mainRect.bottom - siY - ScalePx(20, hwnd);
                    if (siHeight < ScalePx(120, hwnd)) siHeight = ScalePx(120, hwnd);
                }
                SetWindowPos(g_siteinfo_panel_overlay_hwnd, HWND_TOPMOST,
                    siX, siY, siWidth, siHeight,
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

            // Picker mode: one full-window chooser browser, no tabs. Fill the
            // client with the header browser and skip the normal header/tab layout.
            if (g_picker_mode) {
                if (g_header_hwnd && IsWindow(g_header_hwnd)) {
                    SetWindowPos(g_header_hwnd, nullptr, 0, 0, width, height,
                                 SWP_NOZORDER | SWP_NOACTIVATE);
                    CefRefPtr<CefBrowser> hb = SimpleHandler::GetHeaderBrowser();
                    if (hb) {
                        HWND ch = hb->GetHost()->GetWindowHandle();
                        if (ch && IsWindow(ch)) {
                            SetWindowPos(ch, nullptr, 0, 0, width, height,
                                         SWP_NOZORDER | SWP_NOACTIVATE);
                            hb->GetHost()->WasResized();
                        }
                    }
                }
                return 0;
            }

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

            // Reposition site-info hub overlay (LEFT-anchored, left edge under TuneIcon)
            if (g_siteinfo_panel_overlay_hwnd && IsWindow(g_siteinfo_panel_overlay_hwnd) && IsWindowVisible(g_siteinfo_panel_overlay_hwnd)) {
                RECT hdrRect;
                GetWindowRect(g_header_hwnd, &hdrRect);
                RECT siMainRect;
                GetWindowRect(g_hwnd, &siMainRect);
                int siWidth = ScalePx(360, hwnd);
                // Preserve the auto-sized height (set by siteinfo_panel_resize).
                RECT siCur; GetWindowRect(g_siteinfo_panel_overlay_hwnd, &siCur);
                int siHeight = siCur.bottom - siCur.top;
                int siX = hdrRect.left + g_siteinfo_icon_left_offset;
                int siY = hdrRect.top + ScalePx(104, hwnd);
                if (siX + siWidth > siMainRect.right - ScalePx(8, hwnd))
                    siX = siMainRect.right - siWidth - ScalePx(8, hwnd);
                if (siX < siMainRect.left + ScalePx(8, hwnd))
                    siX = siMainRect.left + ScalePx(8, hwnd);
                if (siY + siHeight > siMainRect.bottom - ScalePx(20, hwnd)) {
                    siHeight = siMainRect.bottom - siY - ScalePx(20, hwnd);
                    if (siHeight < ScalePx(120, hwnd)) siHeight = ScalePx(120, hwnd);
                }
                SetWindowPos(g_siteinfo_panel_overlay_hwnd, HWND_TOPMOST,
                    siX, siY, siWidth, siHeight,
                    SWP_NOACTIVATE | SWP_SHOWWINDOW);

                CefRefPtr<CefBrowser> si_browser = SimpleHandler::GetSiteInfoPanelBrowser();
                if (si_browser) {
                    si_browser->GetHost()->WasResized();
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

                // Dismiss site-info hub on focus loss (MA_NOACTIVATE, so it can't
                // close itself via WM_ACTIVATE — avoid a ghost panel on Alt+Tab).
                if (g_siteinfo_panel_overlay_hwnd && IsWindow(g_siteinfo_panel_overlay_hwnd) && IsWindowVisible(g_siteinfo_panel_overlay_hwnd)) {
                    extern void HideSiteInfoPanelOverlay();
                    HideSiteInfoPanelOverlay();
                    LOG_DEBUG("🛈 Dismissed site-info panel overlay on focus loss");
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

// ========== BOOKMARKS PANEL OVERLAY ==========

LRESULT CALLBACK BookmarksPanelMouseHookProc(int nCode, WPARAM wParam, LPARAM lParam) {
    if (nCode == HC_ACTION) {
        if (wParam == WM_LBUTTONDOWN || wParam == WM_RBUTTONDOWN) {
            if (g_bookmarks_panel_overlay_hwnd && IsWindow(g_bookmarks_panel_overlay_hwnd) && IsWindowVisible(g_bookmarks_panel_overlay_hwnd)) {
                MSLLHOOKSTRUCT* mouseInfo = (MSLLHOOKSTRUCT*)lParam;
                POINT clickPoint = mouseInfo->pt;
                RECT overlayRect;
                GetWindowRect(g_bookmarks_panel_overlay_hwnd, &overlayRect);
                if (!PtInRect(&overlayRect, clickPoint)) {
                    LOG_DEBUG("🖱️ Click detected outside bookmarks panel overlay bounds - dismissing");
                    extern void HideBookmarksPanelOverlay();
                    HideBookmarksPanelOverlay();
                }
            }
        }
    }
    return CallNextHookEx(g_bookmarks_panel_mouse_hook, nCode, wParam, lParam);
}

// Tab-list overlay click-outside dismiss — mirrors the bookmarks hook (B2). The tab-list
// panel is a clone of the bookmarks panel (MA_ACTIVATE dropdown with a search box) and,
// like it, previously had NO installed mouse hook, so it only closed on the WM_ACTIVATE
// path — which is exactly the fragile close the owner hit. Give it the same reliable
// click-outside close.
LRESULT CALLBACK TabListPanelMouseHookProc(int nCode, WPARAM wParam, LPARAM lParam) {
    if (nCode == HC_ACTION) {
        if (wParam == WM_LBUTTONDOWN || wParam == WM_RBUTTONDOWN) {
            if (g_tablist_panel_overlay_hwnd && IsWindow(g_tablist_panel_overlay_hwnd) && IsWindowVisible(g_tablist_panel_overlay_hwnd)) {
                MSLLHOOKSTRUCT* mouseInfo = (MSLLHOOKSTRUCT*)lParam;
                POINT clickPoint = mouseInfo->pt;
                RECT overlayRect;
                GetWindowRect(g_tablist_panel_overlay_hwnd, &overlayRect);
                if (!PtInRect(&overlayRect, clickPoint)) {
                    LOG_DEBUG("🖱️ Click detected outside tab-list panel overlay bounds - dismissing");
                    extern void HideTabListPanelOverlay();
                    HideTabListPanelOverlay();
                }
            }
        }
    }
    return CallNextHookEx(g_tablist_panel_mouse_hook, nCode, wParam, lParam);
}

// Mirrors ProfilePanelOverlayWndProc: a dropdown WITH a text input (search box),
// so it takes activation (MA_ACTIVATE) + forwards keyboard, and closes on
// WM_ACTIVATE(WA_INACTIVE) (click-outside) with a show-tick race guard.
LRESULT CALLBACK BookmarksPanelOverlayWndProc(HWND hwnd, UINT msg, WPARAM wParam, LPARAM lParam) {
    switch (msg) {
        case WM_MOUSEACTIVATE:
            return MA_ACTIVATE;

        case WM_SETFOCUS: {
            ImmAssociateContext(hwnd, nullptr);
            CefRefPtr<CefBrowser> bm_browser = SimpleHandler::GetBookmarksPanelBrowser();
            if (bm_browser) {
                bm_browser->GetHost()->SetFocus(true);
            }
            return 0;
        }

        case WM_MOUSEMOVE: {
            POINT pt = { GET_X_LPARAM(lParam), GET_Y_LPARAM(lParam) };
            CefMouseEvent mouse_event;
            mouse_event.x = pt.x;
            mouse_event.y = pt.y;
            mouse_event.modifiers = 0;
            CefRefPtr<CefBrowser> bm_browser = SimpleHandler::GetBookmarksPanelBrowser();
            if (bm_browser) {
                bm_browser->GetHost()->SendMouseMoveEvent(mouse_event, false);
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
            CefRefPtr<CefBrowser> bm_browser = SimpleHandler::GetBookmarksPanelBrowser();
            if (bm_browser) {
                bm_browser->GetHost()->SendMouseClickEvent(mouse_event, MBT_LEFT, false, 1);
            }
            return 0;
        }

        case WM_LBUTTONUP: {
            POINT pt = { GET_X_LPARAM(lParam), GET_Y_LPARAM(lParam) };
            CefMouseEvent mouse_event;
            mouse_event.x = pt.x;
            mouse_event.y = pt.y;
            mouse_event.modifiers = 0;
            CefRefPtr<CefBrowser> bm_browser = SimpleHandler::GetBookmarksPanelBrowser();
            if (bm_browser) {
                bm_browser->GetHost()->SendMouseClickEvent(mouse_event, MBT_LEFT, true, 1);
            }
            return 0;
        }

        case WM_KEYDOWN: {
            CefRefPtr<CefBrowser> bm_browser = SimpleHandler::GetBookmarksPanelBrowser();
            if (bm_browser) {
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
                bm_browser->GetHost()->SendKeyEvent(key_event);
            }
            return 0;
        }

        case WM_KEYUP: {
            CefRefPtr<CefBrowser> bm_browser = SimpleHandler::GetBookmarksPanelBrowser();
            if (bm_browser) {
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
                bm_browser->GetHost()->SendKeyEvent(key_event);
            }
            return 0;
        }

        case WM_CHAR: {
            CefRefPtr<CefBrowser> bm_browser = SimpleHandler::GetBookmarksPanelBrowser();
            if (bm_browser) {
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
                bm_browser->GetHost()->SendKeyEvent(key_event);
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
            CefRefPtr<CefBrowser> bm_browser = SimpleHandler::GetBookmarksPanelBrowser();
            if (bm_browser) {
                bm_browser->GetHost()->SendMouseWheelEvent(mouse_event, 0, delta);
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
                // Suppress immediate hide if just shown (<200ms) — SetForegroundWindow
                // in ShowBookmarksPanelOverlay bounces focus and would otherwise self-close.
                // Consume the ONE focus-bounce WA_INACTIVE that SetForegroundWindow fires
                // right after show — machine-INDEPENDENTLY. The old fixed <200ms window
                // mis-fired on SLOW PCs (the bounce lands later than 200ms → the panel
                // self-closed). The first WA_INACTIVE after a show IS the bounce; the next
                // one is a real click-outside.
                if (g_bookmarks_last_show_tick != 0) {
                    g_bookmarks_last_show_tick = 0;  // consume the bounce; next one closes
                    return 0;
                }
                LOG_INFO("Hiding bookmarks panel — lost activation (click-outside)");
                extern void HideBookmarksPanelOverlay();
                HideBookmarksPanelOverlay();
                return 0;
            }
            break;

        case WM_WINDOWPOSCHANGING:
            break;

        // IME suppression (same as wallet / profile)
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

// ========== TAB-LIST PANEL OVERLAY ==========
// Mirrors the BOOKMARKS panel (MA_ACTIVATE dropdown WITH a search box → takes
// activation, forwards keyboard, closes on WM_ACTIVATE(WA_INACTIVE) with a
// show-tick race guard). LEFT-anchored at the caret on the tab strip.
LRESULT CALLBACK TabListPanelOverlayWndProc(HWND hwnd, UINT msg, WPARAM wParam, LPARAM lParam) {
    switch (msg) {
        case WM_MOUSEACTIVATE:
            return MA_ACTIVATE;

        case WM_SETFOCUS: {
            ImmAssociateContext(hwnd, nullptr);
            CefRefPtr<CefBrowser> tl_browser = SimpleHandler::GetTabListPanelBrowser();
            if (tl_browser) {
                tl_browser->GetHost()->SetFocus(true);
            }
            return 0;
        }

        case WM_MOUSEMOVE: {
            POINT pt = { GET_X_LPARAM(lParam), GET_Y_LPARAM(lParam) };
            CefMouseEvent mouse_event;
            mouse_event.x = pt.x;
            mouse_event.y = pt.y;
            mouse_event.modifiers = 0;
            CefRefPtr<CefBrowser> tl_browser = SimpleHandler::GetTabListPanelBrowser();
            if (tl_browser) {
                tl_browser->GetHost()->SendMouseMoveEvent(mouse_event, false);
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
            CefRefPtr<CefBrowser> tl_browser = SimpleHandler::GetTabListPanelBrowser();
            if (tl_browser) {
                tl_browser->GetHost()->SendMouseClickEvent(mouse_event, MBT_LEFT, false, 1);
            }
            return 0;
        }

        case WM_LBUTTONUP: {
            POINT pt = { GET_X_LPARAM(lParam), GET_Y_LPARAM(lParam) };
            CefMouseEvent mouse_event;
            mouse_event.x = pt.x;
            mouse_event.y = pt.y;
            mouse_event.modifiers = 0;
            CefRefPtr<CefBrowser> tl_browser = SimpleHandler::GetTabListPanelBrowser();
            if (tl_browser) {
                tl_browser->GetHost()->SendMouseClickEvent(mouse_event, MBT_LEFT, true, 1);
            }
            return 0;
        }

        case WM_KEYDOWN: {
            CefRefPtr<CefBrowser> tl_browser = SimpleHandler::GetTabListPanelBrowser();
            if (tl_browser) {
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
                tl_browser->GetHost()->SendKeyEvent(key_event);
            }
            return 0;
        }

        case WM_KEYUP: {
            CefRefPtr<CefBrowser> tl_browser = SimpleHandler::GetTabListPanelBrowser();
            if (tl_browser) {
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
                tl_browser->GetHost()->SendKeyEvent(key_event);
            }
            return 0;
        }

        case WM_CHAR: {
            CefRefPtr<CefBrowser> tl_browser = SimpleHandler::GetTabListPanelBrowser();
            if (tl_browser) {
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
                tl_browser->GetHost()->SendKeyEvent(key_event);
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
            CefRefPtr<CefBrowser> tl_browser = SimpleHandler::GetTabListPanelBrowser();
            if (tl_browser) {
                tl_browser->GetHost()->SendMouseWheelEvent(mouse_event, 0, delta);
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
                // Consume the ONE focus-bounce WA_INACTIVE after show (machine-independent;
                // the old fixed 200ms window mis-fired on slow PCs). See bookmarks above.
                if (g_tablist_last_show_tick != 0) {
                    g_tablist_last_show_tick = 0;  // consume the bounce; next one closes
                    return 0;
                }
                LOG_INFO("Hiding tab-list panel — lost activation (click-outside)");
                extern void HideTabListPanelOverlay();
                HideTabListPanelOverlay();
                return 0;
            }
            break;

        case WM_WINDOWPOSCHANGING:
            break;

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

// ========== SITE-INFO HUB OVERLAY ==========
// Left-anchored dropdown (TuneIcon at the address-bar left). No text input, so it
// uses the light download-panel pattern: MA_NOACTIVATE + an installed low-level
// mouse hook for click-outside (NOT the bookmarks MA_ACTIVATE/keyboard pattern).

LRESULT CALLBACK SiteInfoPanelMouseHookProc(int nCode, WPARAM wParam, LPARAM lParam) {
    if (nCode == HC_ACTION) {
        if (wParam == WM_LBUTTONDOWN || wParam == WM_RBUTTONDOWN) {
            if (g_siteinfo_panel_overlay_hwnd && IsWindow(g_siteinfo_panel_overlay_hwnd) && IsWindowVisible(g_siteinfo_panel_overlay_hwnd)) {
                MSLLHOOKSTRUCT* mouseInfo = (MSLLHOOKSTRUCT*)lParam;
                POINT clickPoint = mouseInfo->pt;
                RECT overlayRect;
                GetWindowRect(g_siteinfo_panel_overlay_hwnd, &overlayRect);
                if (!PtInRect(&overlayRect, clickPoint)) {
                    LOG_DEBUG("🖱️ Click detected outside site-info panel overlay bounds - dismissing");
                    extern void HideSiteInfoPanelOverlay();
                    HideSiteInfoPanelOverlay();
                }
            }
        }
    }
    return CallNextHookEx(g_siteinfo_panel_mouse_hook, nCode, wParam, lParam);
}

LRESULT CALLBACK SiteInfoPanelOverlayWndProc(HWND hwnd, UINT msg, WPARAM wParam, LPARAM lParam) {
    switch (msg) {
        case WM_MOUSEACTIVATE:
            return MA_NOACTIVATE;

        case WM_MOUSEMOVE: {
            POINT pt = { GET_X_LPARAM(lParam), GET_Y_LPARAM(lParam) };
            CefMouseEvent mouse_event;
            mouse_event.x = pt.x;
            mouse_event.y = pt.y;
            mouse_event.modifiers = 0;
            CefRefPtr<CefBrowser> si_browser = SimpleHandler::GetSiteInfoPanelBrowser();
            if (si_browser) {
                si_browser->GetHost()->SendMouseMoveEvent(mouse_event, false);
            }
            return 0;
        }

        case WM_LBUTTONDOWN: {
            POINT pt = { GET_X_LPARAM(lParam), GET_Y_LPARAM(lParam) };
            CefMouseEvent mouse_event;
            mouse_event.x = pt.x;
            mouse_event.y = pt.y;
            mouse_event.modifiers = 0;
            CefRefPtr<CefBrowser> si_browser = SimpleHandler::GetSiteInfoPanelBrowser();
            if (si_browser) {
                si_browser->GetHost()->SendMouseClickEvent(mouse_event, MBT_LEFT, false, 1);
                si_browser->GetHost()->SendMouseClickEvent(mouse_event, MBT_LEFT, true, 1);
            }
            return 0;
        }

        case WM_MOUSEWHEEL: {
            // WM_MOUSEWHEEL provides SCREEN coords — convert to client (like the
            // wallet panel) or the wheel target is wrong and scrolling won't work.
            POINT screenPt = { GET_X_LPARAM(lParam), GET_Y_LPARAM(lParam) };
            POINT clientPt = screenPt;
            ScreenToClient(hwnd, &clientPt);
            int delta = GET_WHEEL_DELTA_WPARAM(wParam);
            CefMouseEvent mouse_event;
            mouse_event.x = clientPt.x;
            mouse_event.y = clientPt.y;
            mouse_event.modifiers = 0;
            CefRefPtr<CefBrowser> si_browser = SimpleHandler::GetSiteInfoPanelBrowser();
            if (si_browser) {
                si_browser->GetHost()->SendMouseWheelEvent(mouse_event, 0, delta);
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
                // Consume the ONE focus-bounce WA_INACTIVE after show (machine-independent;
                // the old fixed 200ms window mis-fired on slow PCs). See bookmarks above.
                extern ULONGLONG g_profile_last_show_tick;
                if (g_profile_last_show_tick != 0) {
                    g_profile_last_show_tick = 0;  // consume the bounce; next one closes
                    LOG_INFO("Profile panel WM_ACTIVATE suppressed — focus bounce consumed");
                    return 0;
                }
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

    HINTERNET hConnect = WinHttpConnect(hSession, L"localhost", hodos::WalletPort(), 0);
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
    if (IsPortListening(hodos::WalletPort())) {
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
        bool shutdownSent = SendShutdownRequest(hodos::WalletPort());

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

    HINTERNET hConnect = WinHttpConnect(hSession, L"localhost", hodos::AdblockPort(), 0);
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
    if (IsPortListening(hodos::AdblockPort())) {
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
        bool shutdownSent = SendShutdownRequest(hodos::AdblockPort());

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

#ifdef HODOS_SILENT_AUTOUPDATE
// ============================================================================
// 6c.2 — MaybeApplyStagedUpdate: the Phase-A apply bootstrap.
// AUTOUPDATE_6B_SUPERVISOR_DESIGN.md §9 v3 Phase A. Runs at the :3922 startup seam
// (sole instance, !picker, before profile lock / backends / CefInitialize). On a
// verified staged update it: locks-first, proves the fleet idle + the wallet dead,
// re-verifies the installer (Authenticode + anti-rollback + signer-continuity),
// backs up {app} + snapshots the money DB, arms the RunOnce recovery hook, and
// spawns the external supervisor (hodos-update-helper.exe) with an INHERITED owner-
// lock handle + bootstrap process handle, then _exit(0)s. Compiled OUT by default
// (HODOS_SILENT_AUTOUPDATE) + inert under HODOS_DEV (unless HODOS_UPDATE_TEST).
// ============================================================================
static std::wstring SU_Widen(const std::string& s) {
    if (s.empty()) return L"";
    int n = MultiByteToWideChar(CP_UTF8, 0, s.c_str(), -1, nullptr, 0);
    std::wstring w(n > 0 ? n - 1 : 0, L'\0');
    if (n > 0) MultiByteToWideChar(CP_UTF8, 0, s.c_str(), -1, &w[0], n);
    return w;
}

// Exclusively openable for write (== not image-locked). Absent file => free.
static bool SU_ExclusiveOpenable(const std::wstring& p) {
    if (GetFileAttributesW(p.c_str()) == INVALID_FILE_ATTRIBUTES) return true;
    HANDLE h = CreateFileW(p.c_str(), GENERIC_WRITE, 0, nullptr, OPEN_EXISTING, FILE_ATTRIBUTE_NORMAL, nullptr);
    if (h == INVALID_HANDLE_VALUE) return false;
    CloseHandle(h);
    return true;
}

// Count live HodosBrowser.exe processes whose FULL module path == appDir\HodosBrowser.exe
// (M6 — by path, so a dev build elsewhere isn't counted). -1 on snapshot failure.
static int SU_CountSelfBrowsers(const std::wstring& appDir) {
    HANDLE snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
    if (snap == INVALID_HANDLE_VALUE) return -1;
    std::wstring needle = appDir + L"\\HodosBrowser.exe";
    for (auto& c : needle) c = (wchar_t)towlower(c);
    int count = 0;
    PROCESSENTRY32W pe{}; pe.dwSize = sizeof(pe);
    if (Process32FirstW(snap, &pe)) {
        do {
            if (_wcsicmp(pe.szExeFile, L"HodosBrowser.exe") != 0) continue;
            HANDLE h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, FALSE, pe.th32ProcessID);
            if (!h) continue;
            wchar_t path[MAX_PATH]; DWORD sz = MAX_PATH;
            if (QueryFullProcessImageNameW(h, 0, path, &sz)) {
                std::wstring lp(path); for (auto& c : lp) c = (wchar_t)towlower(c);
                if (lp == needle) ++count;
            }
            CloseHandle(h);
        } while (Process32NextW(snap, &pe));
    }
    CloseHandle(snap);
    return count;
}

static void SU_ArmRunOnce(const std::wstring& helperPath) {
    HKEY k;
    if (RegCreateKeyExW(HKEY_CURRENT_USER, L"Software\\Microsoft\\Windows\\CurrentVersion\\RunOnce",
                        0, nullptr, 0, KEY_SET_VALUE, nullptr, &k, nullptr) == ERROR_SUCCESS) {
        std::wstring cmd = L"\"" + helperPath + L"\" --resume";
        RegSetValueExW(k, L"HodosUpdateResume", 0, REG_SZ,
                       reinterpret_cast<const BYTE*>(cmd.c_str()),
                       static_cast<DWORD>((cmd.size() + 1) * sizeof(wchar_t)));
        RegCloseKey(k);
    }
}
static void SU_ClearRunOnce() {
    HKEY k;
    if (RegOpenKeyExW(HKEY_CURRENT_USER, L"Software\\Microsoft\\Windows\\CurrentVersion\\RunOnce",
                      0, KEY_SET_VALUE, &k) == ERROR_SUCCESS) {
        RegDeleteValueW(k, L"HodosUpdateResume");
        RegCloseKey(k);
    }
}

// Spawn the helper inheriting EXACTLY {lockH, bootH, instH} (PROC_THREAD_ATTRIBUTE_HANDLE_LIST),
// detached + no-window + breakaway (retry without breakaway on ACCESS_DENIED, M2).
// instH (the pinned, deny-write installer handle) is inherited but unreferenced — it
// just stays open in the helper, keeping the verified installer bytes un-swappable
// across the install (review #1). Returns the process HANDLE (caller closes) or nullptr.
static HANDLE SU_SpawnHelper(const std::wstring& cmdline, HANDLE lockH, HANDLE bootH, HANDLE instH) {
    SIZE_T size = 0;
    InitializeProcThreadAttributeList(nullptr, 1, 0, &size);
    std::vector<unsigned char> buf(size);
    auto* al = reinterpret_cast<LPPROC_THREAD_ATTRIBUTE_LIST>(buf.data());
    if (!InitializeProcThreadAttributeList(al, 1, 0, &size)) return nullptr;
    HANDLE handles[3]; DWORD nh = 0;
    handles[nh++] = lockH;
    if (bootH) handles[nh++] = bootH;
    if (instH && instH != INVALID_HANDLE_VALUE) handles[nh++] = instH;
    if (!UpdateProcThreadAttribute(al, 0, PROC_THREAD_ATTRIBUTE_HANDLE_LIST,
                                   handles, nh * sizeof(HANDLE), nullptr, nullptr)) {
        DeleteProcThreadAttributeList(al); return nullptr;
    }
    STARTUPINFOEXW si{}; si.StartupInfo.cb = sizeof(si); si.lpAttributeList = al;
    PROCESS_INFORMATION pi{};
    auto mk = [&](DWORD f) -> BOOL {
        std::vector<wchar_t> cl(cmdline.begin(), cmdline.end()); cl.push_back(L'\0');
        return CreateProcessW(nullptr, cl.data(), nullptr, nullptr, TRUE, f,
                              nullptr, nullptr, &si.StartupInfo, &pi);
    };
    DWORD flags = CREATE_NO_WINDOW | DETACHED_PROCESS | EXTENDED_STARTUPINFO_PRESENT | CREATE_BREAKAWAY_FROM_JOB;
    BOOL ok = mk(flags);
    if (!ok && GetLastError() == ERROR_ACCESS_DENIED) ok = mk(flags & ~CREATE_BREAKAWAY_FROM_JOB);
    DeleteProcThreadAttributeList(al);
    if (!ok) return nullptr;
    CloseHandle(pi.hThread);
    return pi.hProcess;
}

// Simple detached spawn (no inherited handles) — for the helper --resume path.
static HANDLE SU_SpawnDetached(const std::wstring& cmdline) {
    STARTUPINFOW si{}; si.cb = sizeof(si);
    PROCESS_INFORMATION pi{};
    auto mk = [&](DWORD f) -> BOOL {
        std::vector<wchar_t> cl(cmdline.begin(), cmdline.end()); cl.push_back(L'\0');
        return CreateProcessW(nullptr, cl.data(), nullptr, nullptr, FALSE, f, nullptr, nullptr, &si, &pi);
    };
    DWORD flags = CREATE_NO_WINDOW | DETACHED_PROCESS | CREATE_BREAKAWAY_FROM_JOB;
    BOOL ok = mk(flags);
    if (!ok && GetLastError() == ERROR_ACCESS_DENIED) ok = mk(flags & ~CREATE_BREAKAWAY_FROM_JOB);
    if (!ok) return nullptr;
    CloseHandle(pi.hThread);
    return pi.hProcess;
}

// 6e.1 — in-browser watchdog tripwire (SECONDARY to the per-user RunOnce --resume).
// At cold boot, if apply.json shows an unconfirmed apply (installing/awaiting-health)
// AND no live supervisor holds the lock AND we're not the health-probe, the supervisor
// died mid-apply: re-spawn the helper --resume (it rolls back DB-first + relaunches old)
// and _exit(0) so it can overwrite our image. Covers the window where the user relaunches
// before the next logon fires RunOnce. Returns false => nothing to do => continue startup.
static bool MaybeResumeUnconfirmedApply() {
    using namespace hodos;
    if (IsDevEnv() && !std::getenv("HODOS_UPDATE_TEST")) return false;
    if (g_post_update_probe.load()) return false;  // we ARE the probe — let 6d confirm health

    const std::string pendingDir = AppPaths::GetPendingUpdateDir();
    const std::string lockPath = AppPaths::GetUpdateLockPath();
    if (pendingDir.empty() || lockPath.empty()) return false;

    std::string content;
    ApplyRecord rec;
    if (!updatefs::ReadFileAll(SU_Widen(pendingDir + "\\apply.json"), content) ||
        !ParseApplyRecord(content, rec)) return false;
    if (rec.phase != ApplyPhase::Installing && rec.phase != ApplyPhase::AwaitingHealth) return false;

    // A live supervisor (lock held) is already handling it — don't interfere.
    if (UpdateLockIsHeld(SU_Widen(lockPath))) return false;

    const std::string helperExe = AppPaths::GetHelperStageDir() + "\\hodos-update-helper.exe";
    if (GetFileAttributesW(SU_Widen(helperExe).c_str()) == INVALID_FILE_ATTRIBUTES) {
        LOG_WARNING("Resume: unconfirmed apply but helper copy missing — leaving for RunOnce --resume");
        return false;
    }
    LOG_WARNING("Resume: unconfirmed apply (phase=" + std::string(ApplyPhaseToString(rec.phase)) +
                ") with no live supervisor — re-spawning helper --resume");
    HANDLE h = SU_SpawnDetached(L"\"" + SU_Widen(helperExe) + L"\" --resume");
    if (h) CloseHandle(h);
    fflush(nullptr);
    _exit(0);   // let the helper restore rollback + relaunch the old build over our image
    return true;  // unreachable
}

// Returns false => not eligible / aborted => caller continues normal startup. On a
// successful apply it _exit(0)s (never returns).
static bool MaybeApplyStagedUpdate(const std::string& profileId) {
    using namespace hodos;
    if (IsDevEnv() && !std::getenv("HODOS_UPDATE_TEST")) return false;  // inert in dev

    const std::string updateDir = AppPaths::GetUpdateDir();
    const std::string pendingDir = AppPaths::GetPendingUpdateDir();
    const std::string appDir = AppPaths::GetAppInstallDir();
    const std::string walletDir = AppPaths::GetWalletDir();
    const std::string statePath = AppPaths::GetUpdateStatePath();
    const std::string lockPath = AppPaths::GetUpdateLockPath();
    if (updateDir.empty() || pendingDir.empty() || appDir.empty() || lockPath.empty()) return false;

    // Eligibility (global state — settings aren't loaded at this seam). Missing/
    // corrupt update-state.json => NOT eligible (fail-safe-off, V3-7).
    UpdateState state;
    {
        std::string c;
        if (!updatefs::ReadFileAll(SU_Widen(statePath), c) || !ParseUpdateState(c, state)) return false;
    }
    // NOTE: `paused` is intentionally NOT checked here (#2). A prior rollback sets paused,
    // but a strictly NEWER build (a fix) must break through it — see PausedBlocksStagedBuild
    // after the marker parse below. Gating on paused here would permanently wedge the fleet
    // to notify-only after a single rollback and block silently pushing the fix.
    if (!state.silent) return false;

    StagedUpdateMarker marker;
    {
        std::string c;
        if (!updatefs::ReadFileAll(SU_Widen(pendingDir + "\\update-info.json"), c) ||
            !UpdateStager::ParseMarker(c, marker)) return false;
    }
    const std::string installerPath = pendingDir + "\\" + marker.installerFileName;
    const std::string manifestPath = pendingDir + "\\expected-new-manifest.json";
    if (GetFileAttributesW(SU_Widen(installerPath).c_str()) == INVALID_FILE_ATTRIBUTES) return false;
    if (GetFileAttributesW(SU_Widen(manifestPath).c_str()) == INVALID_FILE_ATTRIBUTES) return false;
    if (GetFileAttributesW(SU_Widen(manifestPath + ".ed").c_str()) == INVALID_FILE_ATTRIBUTES) return false;

    // A build already rejected for a PERSISTENT reason (signer/rollback/tamper — won't
    // change for the same staged bytes) is skipped HERE, before the lock + wallet
    // shutdown, so a bad stage doesn't churn the wallet on every cold boot (review #4).
    if (marker.buildNumber != 0 && state.lastFailureBuild == marker.buildNumber) {
        LOG_INFO("Silent apply: build " + std::to_string(marker.buildNumber) +
                 " previously rejected (" + state.lastFailureReason + ") — skip");
        return false;
    }

    // #2: a prior ROLLBACK paused silent updates. Stay paused for the same-or-older build
    // (never retry the bad one), but let a strictly NEWER build (a fix) break through — on
    // health-confirmed success the helper clears paused, healing the fleet. Safe: every build
    // is still EdDSA+Authenticode+manifest-verified, health-gated, and rollback-protected, so
    // a newer (possibly-also-bad) build can't brick — it just rolls back + re-pauses.
    if (PausedBlocksStagedBuild(state.paused, marker.buildNumber, state.lastFailureBuild)) {
        LOG_INFO("Silent apply: paused after a prior rollback; staged build " +
                 std::to_string(marker.buildNumber) + " not newer than failed build " +
                 std::to_string(state.lastFailureBuild) + " — skip (a newer build will apply)");
        return false;
    }

    LOG_INFO("Silent apply: eligible staged build " + std::to_string(marker.buildNumber) + " — Phase A");

    updatefs::EnsureDirExists(SU_Widen(updateDir));  // RISK-A: dir before the first Acquire

    UpdateLockOwner lock;  // LOCK-FIRST (V3-6): own the apply window or bail.
    if (!lock.AcquireWithRetry(SU_Widen(lockPath))) {
        LOG_INFO("Silent apply: another apply owner is live — normal startup");
        return false;
    }

    // Record a PERSISTENT rejection so this exact staged build is skipped on future
    // boots (review #4: avoid per-boot wallet churn). Use ONLY for reasons that won't
    // change for the same bytes (tamper/signer/rollback) — NOT for transient defers.
    auto rejectPersistent = [&](const std::string& reason) -> bool {
        LOG_WARNING("Silent apply: reject build " + std::to_string(marker.buildNumber) + " — " + reason);
        state.lastFailureBuild = marker.buildNumber;
        state.lastFailureReason = reason;
        updatefs::WriteFileAtomic(SU_Widen(statePath), SerializeUpdateState(state));
        return false;
    };

    const std::wstring appDirW = SU_Widen(appDir);
    // D.0 sole-instance gate — with a bounded WAIT for the transient picker (bug #3).
    // Other {app}\HodosBrowser.exe processes inflate the count: the profile PICKER + its
    // CEF subprocess tree, or a real browser. The picker is TRANSIENT — it
    // PostMessage(WM_CLOSE)s itself the instant it spawns us (simple_handler.cpp ~3247)
    // and its whole tree dies within a few seconds — whereas a real browser (booting OR
    // up) stays. So poll: if the count settles to 1 (everything else exited — the picker
    // died) we are genuinely sole → apply; if it stays >1 (a real browser is live) → defer.
    // This waits the picker out WITHOUT the process-classification/lock-probe an adversarial
    // review REJECTED as unsafe (a lock/cmdline probe misses a concurrently-BOOTING real
    // browser that hasn't taken its profile.lock yet, and V3-2 below would then kill its
    // shared 31301/31401 wallet). Waiting is safe: only the transient picker's death drops
    // the count; a real browser never dies mid-wait, so we can never over-approve. Exits
    // early the moment count==1 (picker case ~2-4s); ~8s cap otherwise. See
    // DevOps-CICD/AUTOUPDATE_PICKER_GATE_DESIGN.md.
    int selfCount = SU_CountSelfBrowsers(appDirW);
    for (int i = 0; selfCount > 1 && i < 16 && !g_update_abort.load(); ++i) {  // ~8s cap
        Sleep(500);
        selfCount = SU_CountSelfBrowsers(appDirW);
    }
    if (selfCount != 1) {
        LOG_INFO("Silent apply: not sole instance (count=" + std::to_string(selfCount) +
                 ") after wait — defer");
        return false;  // Fix D: a persistent >1 (always-open browser / slow picker) is a
                       // TRANSIENT defer — future chronic-deferral valve degrades to notify.
    }

    // Prove the WALLET DEAD before any snapshot (V3-2): port unbound AND exe openable.
    if (IsPortListening(WalletPort())) {
        SendShutdownRequest(WalletPort());
        for (int i = 0; i < 12 && IsPortListening(WalletPort()); ++i) Sleep(250);  // ~3s grace
    }
    if (IsPortListening(WalletPort()) || !SU_ExclusiveOpenable(appDirW + L"\\hodos-wallet.exe")) {
        LOG_WARNING("Silent apply: wallet still alive — defer (never snapshot a live-writer DB)");
        return false;
    }

    // PIN the installer against tamper (review #1 / OD-B TOCTOU): open it deny-write,
    // deny-delete, INHERITABLE before verifying, and keep the handle open through to
    // the spawn (the helper inherits it). The bytes we verify here cannot be swapped
    // in the bootstrap-exit -> helper-spawn window — no apply-time re-verify needed.
    SECURITY_ATTRIBUTES instSa{}; instSa.nLength = sizeof(instSa); instSa.bInheritHandle = TRUE;
    HANDLE instH = CreateFileW(SU_Widen(installerPath).c_str(), GENERIC_READ,
                               FILE_SHARE_READ, &instSa, OPEN_EXISTING, FILE_ATTRIBUTE_NORMAL, nullptr);
    if (instH == INVALID_HANDLE_VALUE) { LOG_WARNING("Silent apply: cannot pin installer — abort"); return false; }
    struct InstHGuard { HANDLE h; ~InstHGuard() { if (h != INVALID_HANDLE_VALUE) CloseHandle(h); } } instGuard{instH};

    // Apply-time verify (fail-closed; reuse UpdateStager) of the PINNED bytes.
    // Authenticode is the trustworthy gate (self-contained; a local attacker can't
    // re-sign as Marston). sha256==marker is a cheap extra (the marker is plaintext).
    if (UpdateStager::Sha256File(installerPath) != marker.sha256) return rejectPersistent("installer sha256 != marker");
    auto instAuth = UpdateStager::VerifyAuthenticode(installerPath, UpdateStager::ExpectedSigner());
    bool authOk = instAuth.trusted;
#ifdef HODOS_UPDATE_TEST_SEAM
    // TEST-BUILD ONLY (compiled OUT of production): the rig installer is self-signed
    // and won't chain to Marston — stage on the EdDSA/manifest gates alone, like
    // UpdateStager::StagePendingUpdate's test seam. Production Authenticode is mandatory.
    if (const char* t = std::getenv("HODOS_UPDATE_TEST"); t && std::string(t) == "1") authOk = true;
#endif
    if (!authOk) return rejectPersistent("installer Authenticode failed");

    // Verify the SIGNED expected-new manifest + read its bound buildNumber, so
    // anti-rollback trusts a SIGNED number, not the plaintext (attacker-writable)
    // marker (review #2). The manifest's Ed25519 sidecar is verified with the embedded
    // key; an attacker can't forge buildNumber without the private key.
    long signedBuild = 0;
    {
        std::string mbytes, msig;
        if (!updatefs::ReadFileAll(SU_Widen(manifestPath), mbytes) ||
            !updatefs::ReadFileAll(SU_Widen(manifestPath + ".ed"), msig) ||
            !updatefs::VerifyManifestSignature(mbytes, msig)) {
            return rejectPersistent("expected-new-manifest signature invalid/missing");
        }
        FileManifest sm;
        if (!ParseManifest(mbytes, sm) || sm.buildNumber <= 0) {
            return rejectPersistent("expected-new-manifest unparseable / no buildNumber");
        }
        signedBuild = sm.buildNumber;
        if (marker.buildNumber != signedBuild) {
            return rejectPersistent("marker buildNumber " + std::to_string(marker.buildNumber) +
                                    " != signed manifest " + std::to_string(signedBuild));
        }
    }

    // Anti-rollback on the SIGNED build number. The floor max(APP_BUILD_NUMBER,
    // highWater) also stops a JSON edit of update-state.json from going below the
    // running build.
    const long currentBuild = APP_BUILD_NUMBER;
    const long floor = (state.highWaterBuild > currentBuild) ? state.highWaterBuild : currentBuild;
    if (!(signedBuild > floor)) {
        return rejectPersistent("anti-rollback (build " + std::to_string(signedBuild) +
                                " <= floor " + std::to_string(floor) + ")");
    }

    // Signer-continuity (OD-E/H5): derive BOTH thumbprints from the actual binaries;
    // a signer change => degrade to notify (never silent-apply across reputation reset).
    std::string selfPathA(MAX_PATH, '\0');
    for (;;) {
        DWORD n = GetModuleFileNameA(nullptr, &selfPathA[0], (DWORD)selfPathA.size());
        if (n == 0) { selfPathA.clear(); break; }
        if (n < selfPathA.size()) { selfPathA.resize(n); break; }
        selfPathA.resize(selfPathA.size() * 2);  // truncated -> grow + retry (review #8L)
    }
    auto liveAuth = UpdateStager::VerifyAuthenticode(selfPathA, UpdateStager::ExpectedSigner());
    if (liveAuth.trusted && instAuth.trusted && liveAuth.thumbprint != instAuth.thumbprint) {
        return rejectPersistent("signer changed (" + liveAuth.thumbprint + " -> " + instAuth.thumbprint +
                                ") — degrade to notify");
    }

    // Kill-list (6e.2 / H4 / §H.7): a build WE retracted server-side must not apply
    // even if already staged. Fail-open (best-effort safety; the build is already
    // Marston+EdDSA verified — this only stops our own bad build). NOT recorded as a
    // persistent rejection (a build could be UN-retracted), so use a plain return.
    bool checkKillList = true;
#ifdef HODOS_UPDATE_TEST_SEAM
    // Skip the prod-URL kill-list fetch in the rig (it would 404 -> fail-open anyway,
    // but this avoids the ~10s network wait on the local apply path).
    if (const char* t = std::getenv("HODOS_UPDATE_TEST"); t && std::string(t) == "1") checkKillList = false;
#endif
    if (checkKillList &&
        UpdateStager::IsBuildRetracted(signedBuild, "https://hodosbrowser.com/kill-list.json")) {
        LOG_WARNING("Silent apply: build " + std::to_string(signedBuild) + " retracted by kill-list — defer");
        return false;
    }

    // A1: we are now COMMITTED to applying — every eligibility/verify/defer gate above
    // (sole-instance wait, wallet-dead, Authenticode, anti-rollback, signer-continuity,
    // kill-list) has passed. Put the "Hodos is updating…" splash up BEFORE the heavy {app}
    // backup + wallet snapshot below — that local copy is the multi-second gap the owner
    // saw on the apply-boot. RAII: any early `return false` below closes it and normal
    // startup resumes; on the success path we _exit(0) after spawning the helper (the
    // dtor won't run, but the process dies and the helper's own splash — up from the top
    // of RunApplyTransaction — has already taken over with no visible seam).
    UpdateSplash applySplash;

    // Backup the full {app} tree (exclude the update\ working area) + its manifest.
    // Clear any partial rollback\ from a prior aborted run first (review #8) so the
    // backup describes exactly this run's tree.
    const std::wstring rollbackW = SU_Widen(AppPaths::GetRollbackDir());
    updatefs::RemoveTree(rollbackW);
    FileManifest oldManifest;
    if (!updatefs::BuildManifestForTree(appDirW, oldManifest, {L"update"})) {
        LOG_WARNING("Silent apply: cannot manifest {app} — abort"); return false;
    }
    if (!updatefs::CopyTreeRecursive(appDirW, rollbackW, {L"update"})) {
        LOG_WARNING("Silent apply: {app} backup failed — abort"); return false;
    }
    const std::string rbManifestPath = AppPaths::GetRollbackDir() + "\\manifest.json";
    updatefs::WriteFileAtomic(SU_Widen(rbManifestPath), SerializeManifest(oldManifest));

    // Snapshot the money DB (raw db+wal, no checkpoint/shm) — wallet proven dead above.
    const std::wstring walletW = SU_Widen(walletDir);
    if (!walletDir.empty() &&
        GetFileAttributesW((walletW + L"\\wallet.db").c_str()) != INVALID_FILE_ATTRIBUTES) {
        if (!updatefs::SnapshotWalletDbSet(walletW, rollbackW + L"\\wallet")) {
            LOG_WARNING("Silent apply: money-DB snapshot failed — abort (no rollback safety)"); return false;
        }
    } else {
        LOG_INFO("Silent apply: no wallet.db to snapshot (fresh install) — continuing");
    }

    // Verify the backup is COMPLETE before arming (M3).
    {
        auto vr = updatefs::VerifyTreeAgainstManifest(rollbackW, oldManifest);
        if (!vr.ok) { LOG_WARNING("Silent apply: backup incomplete (" + vr.reason + ") — abort"); return false; }
    }

    // Copy the helper OUT of {app} so the installer can replace it.
    const std::wstring helperStageW = SU_Widen(AppPaths::GetHelperStageDir());
    updatefs::EnsureDirExists(helperStageW);
    const std::wstring helperDst = helperStageW + L"\\hodos-update-helper.exe";
    if (!CopyFileW((appDirW + L"\\hodos-update-helper.exe").c_str(), helperDst.c_str(), FALSE)) {
        LOG_WARNING("Silent apply: failed to copy helper out — abort"); return false;
    }

    // apply.json = preparing (BEFORE arming RunOnce, V3-14) -> arm -> armed.
    ApplyRecord rec;
    rec.phase = ApplyPhase::Preparing;
    rec.fromBuild = currentBuild;
    rec.toBuild = signedBuild;   // the SIGNED build number (review #2), not the plaintext marker
    rec.installerPath = installerPath;
    rec.rollbackDir = AppPaths::GetRollbackDir();
    rec.rollbackManifestPath = rbManifestPath;
    rec.expectedNewManifestPath = manifestPath;
    rec.profileId = profileId;
    rec.toVersion = marker.version;
    rec.signerThumbprint = instAuth.thumbprint;
    rec.stagedAt = marker.stagedAt;
    const std::wstring applyW = SU_Widen(pendingDir + "\\apply.json");
    updatefs::WriteFileAtomic(applyW, SerializeApplyRecord(rec));
    SU_ArmRunOnce(helperDst);
    rec.phase = ApplyPhase::Armed;
    updatefs::WriteFileAtomic(applyW, SerializeApplyRecord(rec));

    // NOTE: we do NOT close g_instance_mutex here. On the SUCCESS path _exit(0) (OS
    // teardown) closes it AFTER the helper inherited everything — and the helper waits
    // for THIS process's death before it runs Inno, so Inno's AppMutex sees the mutex
    // gone anyway. On a spawn-FAILURE abort below we MUST keep it (this browser keeps
    // running and must stay registered for the all-instances-gone / AppMutex gate).
    // Closing it pre-spawn would de-register a still-live session on abort (review #6).

    // Inheritable SYNCHRONIZE handle to SELF — the helper's PID-reuse-immune wait target.
    HANDLE bootH = nullptr;
    DuplicateHandle(GetCurrentProcess(), GetCurrentProcess(), GetCurrentProcess(),
                    &bootH, SYNCHRONIZE, TRUE, 0);

    std::wstring cmd = L"\"" + helperDst + L"\""
        + L" --app-dir \"" + appDirW + L"\""
        + L" --update-dir \"" + SU_Widen(updateDir) + L"\""
        + L" --installer \"" + SU_Widen(installerPath) + L"\""
        + L" --from-build " + std::to_wstring(currentBuild)
        + L" --to-build " + std::to_wstring(marker.buildNumber)
        + L" --lock-handle " + std::to_wstring(reinterpret_cast<uintptr_t>(lock.raw()));
    if (bootH) cmd += L" --bootstrap-handle " + std::to_wstring(reinterpret_cast<uintptr_t>(bootH));

    HANDLE helper = SU_SpawnHelper(cmd, lock.raw(), bootH, instH);
    if (!helper) {
        LOG_ERROR("Silent apply: helper spawn FAILED — aborting, cleaning up");
        SU_ClearRunOnce();
        rec.phase = ApplyPhase::Aborted; rec.failureReason = "helper-spawn-failed";
        updatefs::WriteFileAtomic(applyW, SerializeApplyRecord(rec));
        if (bootH) CloseHandle(bootH);
        return false;  // lock dtor releases (DELETE_ON_CLOSE removes update.lock); old build runs
    }
    CloseHandle(helper);
    if (bootH) CloseHandle(bootH);  // the helper inherited its own copy

    LOG_INFO("Silent apply: helper spawned for build " + std::to_string(marker.buildNumber) + " — bootstrap exiting");
    fflush(nullptr);
    _exit(0);   // hold nothing; the helper inherited the lock handle so the lock object survives
    return true;  // unreachable
}
#endif  // HODOS_SILENT_AUTOUPDATE

int WINAPI WinMain(HINSTANCE hInstance, HINSTANCE, LPSTR, int nCmdShow) {
    // ── Startup performance timer ──
    auto t0 = std::chrono::steady_clock::now();
    auto elapsed = [&t0]() -> std::string {
        auto ms = std::chrono::duration_cast<std::chrono::milliseconds>(
            std::chrono::steady_clock::now() - t0).count();
        return "[T+" + std::to_string(ms) + "ms] ";
    };

    g_hInstance = hInstance;

    // Initialize COM for taskbar integration (AUMID, ITaskbarList3).
    // CoInitializeEx is reference-counted; CEF's later COM init is compatible.
    CoInitializeEx(nullptr, COINIT_APARTMENTTHREADED);

    // Dev safeguard: refuse to run from build directory without HODOS_DEV=1
    {
        char exe_path[MAX_PATH];
        GetModuleFileNameA(NULL, exe_path, MAX_PATH);
        if (!AppPaths::EnforceDevSafeguard(std::string(exe_path))) {
            MessageBoxA(NULL,
                "DEV SAFEGUARD: HODOS_DEV=1 is not set!\n\n"
                "Running a dev build without it would use the production database.\n\n"
                "Use win_build_run.sh to launch, or set HODOS_DEV=1 in your terminal.",
                "HodosBrowser - Dev Safeguard", MB_OK | MB_ICONERROR);
            return 1;
        }
    }

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

    // Initialize centralized logger FIRST. Log to an ABSOLUTE path OUTSIDE {app}: the
    // installed shortcut's working dir is {app}, so a relative "debug_output.log" lands
    // inside the install root — and the browser holds it open-for-write while the
    // silent-update backup tries to hash the whole {app} tree, failing the backup so the
    // update never applies (Stage-2 real-build test finding). GetLogDir() is %APPDATA%\
    // <ns>\logs (roaming, next to the wallet's logs). Falls back to the relative name
    // only if APPDATA is unavailable.
    std::string logPath = "debug_output.log";
    {
        const std::string logDir = AppPaths::GetLogDir();
        if (!logDir.empty()) {
            std::error_code lec;
            std::filesystem::create_directories(std::filesystem::u8path(logDir), lec);
            if (!lec) logPath = logDir + "\\debug_output.log";
        }
    }
    Logger::Initialize(ProcessType::MAIN, logPath);
    LOG_INFO(elapsed() + "STARTUP: Logger initialized");

    LOG_INFO("=== NEW SESSION STARTED ===");
    LOG_INFO("Shell starting...");

    // Redirect stdout and stderr to the SAME out-of-{app} log (see logPath above).
    FILE* dummy;
    errno_t result1 = freopen_s(&dummy, logPath.c_str(), "a", stdout);
    errno_t result2 = freopen_s(&dummy, logPath.c_str(), "a", stderr);

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
    std::string user_data_path = appdata_path + "\\" + AppPaths::GetAppDirName();

    // Initialize ProfileManager BEFORE CefInitialize so cache_path is correct
    LOG_INFO("Initializing ProfileManager...");
    if (!ProfileManager::GetInstance().Initialize(user_data_path)) {
        LOG_ERROR("Failed to initialize ProfileManager");
    }

    // Resolve which profile this process opens (CHUNK 1 + R7 + picker gate).
    // ParseProfileArgument returns "" when no --profile flag was passed (the
    // taskbar/desktop/Start no-arg launch). ResolveStartup then decides:
    //   explicit valid --profile -> that profile
    //   no-arg + 1 profile        -> that profile
    //   no-arg + >1 + picker on   -> picker mode (branch below)
    //   no-arg + >1 + picker off  -> the default (starred) profile
    std::string argProfile = ProfileManager::ParseProfileArgument(GetCommandLineW());
    auto& pm = ProfileManager::GetInstance();
    std::vector<std::string> existingIds;
    for (const auto& p : pm.GetAllProfiles()) existingIds.push_back(p.id);
    ProfileManager::StartupResolution res = ProfileManager::ResolveStartup(
        argProfile, existingIds, pm.GetDefaultProfileId(), pm.ShouldShowPickerOnStartup());
    std::string profileId = res.profileId;
    // In-memory only — startup never rewrites the registry (R5: no boot churn).
    pm.SetCurrentProfileId(profileId, /*persist=*/false);
    // C3 diagnostic: the code says the picker keeps showing whenever there are >1 profiles
    // and the setting is on, yet the owner's Win10 box shows it only on the first launch.
    // Log the exact decision inputs so debug_output.log pinpoints WHICH input flipped
    // (profile count dropped to 1? picker setting off? a stray --profile arg?).
    LOG_INFO(elapsed() + "STARTUP: Profile resolved: " + profileId +
             (argProfile.empty() ? " (no-arg)" : " (--profile='" + argProfile + "')") +
             (res.showPicker ? " [picker-pending]" : "") +
             " | pickerDecision: profileCount=" + std::to_string(existingIds.size()) +
             " pickerSettingOn=" + (pm.ShouldShowPickerOnStartup() ? "1" : "0") +
             " defaultId=" + pm.GetDefaultProfileId() +
             " -> showPicker=" + (res.showPicker ? "1" : "0"));
    g_picker_mode = res.showPicker;

    // 6a/6c.1 (WINDOWS_AUTOUPDATE_PLAN §D.0 / OD-C): honor a fleet-wide update.lock
    // at launch. The silent-apply supervisor (commit 6b) holds this lock for the
    // entire install -> relaunch -> health window; while a LIVE owner holds it, EVERY
    // normal launch (profile OR picker) defers so it can never run a half-written
    // {app}. **6c.1 makes the probe REAL:** liveness is an exclusive-open probe
    // (UpdateLockIsHeld), not file presence — so a power-loss remnant (an ownerless
    // file) opens fine and does NOT brick launches (V3-5/V3-6). Still INERT until 6c.2
    // creates a lock. The supervisor's own health-probe relaunch runs WHILE the lock
    // is held and MUST bypass this; the bypass is DOUBLE-GATED (V3-5/N1): the
    // --post-update-health-probe arg AND a real armed apply.json (phase awaiting-
    // health). A stray/forged arg with no matching apply is ignored (defers if locked).
    {
        // Is this the supervisor's health-probe relaunch? DOUBLE-GATED (V3-5/N1):
        // the --post-update-health-probe arg AND a real armed apply.json. A stray or
        // forged arg with no matching armed apply is ignored (so it still defers if
        // the lock is held). On a match, record it so 6d writes apply.json=healthy.
        bool healthProbe = false;
        {
            int pargc = 0;
            LPWSTR* pargv = CommandLineToArgvW(GetCommandLineW(), &pargc);
            if (pargv) {
                for (int i = 1; i < pargc; ++i) {
                    if (wcscmp(pargv[i], L"--post-update-health-probe") == 0) { healthProbe = true; break; }
                }
                LocalFree(pargv);
            }
            if (healthProbe) {
                const std::string applyPath = AppPaths::GetPendingUpdateDir().empty()
                    ? "" : AppPaths::GetPendingUpdateDir() + "\\apply.json";
                std::string content;
                hodos::ApplyRecord ar;
                const bool armed = !applyPath.empty() &&
                    hodos::updatefs::ReadFileAll(HodosUtf8ToWide(applyPath), content) &&
                    hodos::ParseApplyRecord(content, ar) &&
                    ar.phase == hodos::ApplyPhase::AwaitingHealth;
                if (armed) {
                    g_post_update_probe.store(true);          // 6d: arm the healthy-marker writer
                    g_post_update_to_build = ar.toBuild;
                } else {
                    healthProbe = false;
                }
            }
        }

        const std::wstring lockPathW = HodosUtf8ToWide(AppPaths::GetUpdateLockPath());
        if (!lockPathW.empty() && hodos::UpdateLockIsHeld(lockPathW) && !healthProbe) {
            LOG_INFO("Update in progress (update.lock held by a live owner) — deferring this launch");
            return 0;
        }
    }

    // 6a (§D.0 / OD-D): register this live instance for the all-instances-gone gate.
    // Done AFTER the update.lock defer (a deferring instance must NOT register as a
    // sibling) and BEFORE the SingleInstance/picker early-exits (a forwarded/duplicate
    // launch briefly holds it; the OS closes the handle on return, and the instance it
    // forwarded to keeps the object alive — so coverage never gaps). Picker holds it
    // too (OD-D: a user mid-pick is about to launch one). Best-effort: a creation
    // failure is logged, never fatal — 6a depends on nothing here.
    g_instance_mutex = CreateMutexW(nullptr, FALSE, AppPaths::GetInstanceMutexNameW().c_str());
    if (!g_instance_mutex) {
        LOG_WARNING("Could not create instance-presence mutex (err=" +
                    std::to_string(GetLastError()) + ") — auto-update gate may be degraded");
    }

    // Set process AUMID early (before window creation) so Windows gives dev vs
    // prod (and multi-profile) DISTINCT taskbar buttons. A dev build ALWAYS gets a
    // ".Dev" identity — even single-profile — so it never merges with the installed
    // build's taskbar button; prod keeps its existing (set-only-when-multi-profile)
    // behavior. The picker owns no profile -> keep the base AUMID.
    if (!g_picker_mode && (hodos::IsDevEnv() || ProfileManager::GetInstance().GetAllProfiles().size() > 1)) {
        std::wstring aumid = hodos::IsDevEnv() ? L"HodosBrowser.Dev" : L"HodosBrowser";
        if (profileId != "Default") {
            std::wstring pw(profileId.begin(), profileId.end());
            aumid += L"." + pw;
        }
        SetCurrentProcessExplicitAppUserModelID(aumid.c_str());
        LOG_INFO("AUMID set: " + std::string(hodos::IsDevEnv() ? "dev " : "") + profileId);
    }

    // Data directory. Picker mode uses a NEUTRAL cache derived from the resolved
    // dev/prod root (never hardcode HodosBrowser) so it touches no real profile
    // and CEF's per-root SingletonLock can't collide with a running profile.
    // remote_debugging_port is forced to 0 below for the same reason.
    std::string profile_cache = g_picker_mode
        ? (user_data_path + "\\.picker-cache")
        : ProfileManager::GetInstance().GetCurrentProfileDataPath();
    LOG_INFO(std::string(g_picker_mode ? "Picker cache path: " : "Profile data path: ") + profile_cache);

    // Picker mode owns no profile: gate on a dedicated ".picker" pipe so a 2nd
    // no-arg launch can't start a second picker (which would deadlock on CEF's
    // own root-cache SingletonLock — see PROFILE_STARTUP_PICKER_DESIGN.md C-1).
    // No listener thread, no profile lock, no backend processes. ('.' is not a
    // valid profile id char, so the pipe name can never collide with a profile.)
    if (g_picker_mode) {
        if (!SingleInstance::TryAcquireInstance(".picker")) {
            LOG_INFO("Picker: another instance is already showing the picker — exiting");
            return 0;
        }
        LOG_INFO(elapsed() + "STARTUP: Picker instance gate acquired (no profile lock / backends)");
    }

    // The single-instance forward / profile-lock / backend-launch path is profile-only.
    if (!g_picker_mode) {
#ifdef HODOS_SILENT_AUTOUPDATE
    // 6e.1: first recover an unconfirmed apply whose supervisor died (re-spawn
    // helper --resume + _exit), THEN 6c.2: apply a verified staged update at this
    // cold-boot seam — BEFORE SingleInstance/profile-lock/backends/CefInitialize, so
    // the bootstrap holds nothing in {app} but its own image. Both _exit(0) on action;
    // otherwise normal startup continues on the current build. Compiled out by default.
    MaybeResumeUnconfirmedApply();
    MaybeApplyStagedUpdate(profileId);
#endif
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
    }  // end if (!g_picker_mode) — profile single-instance / lock / backends

    // Initialize SettingsManager with profile-specific path
    SettingsManager::GetInstance().Initialize(profile_cache);
    LOG_INFO(elapsed() + "STARTUP: Settings loaded for profile: " + profileId);

#if defined(_WIN32) && defined(HODOS_SILENT_AUTOUPDATE)
    // Mirror the (global) autoUpdateMode into the silent apply-eligibility gate for the
    // NEXT cold boot, and — on the first run under the global-mode scheme — collapse the
    // mode to ONE global value across profiles, taking the MOST CONSERVATIVE (so an
    // explicit notify/off in any profile is never promoted to silent). Normal launches
    // only: a launch with update.lock held already deferred at the honor-probe, and the
    // health-probe / picker are excluded here — so this never races the helper's
    // update-state writes (no lock needed). See AUTOUPDATE_SILENT_STATE_WRITER_DESIGN.md.
    if (!g_picker_mode && !g_post_update_probe.load()) {
        auto& sm = SettingsManager::GetInstance();
        sm.SetUpdateModeChangeCallback([](const std::string& m) {
            hodos::MirrorSilentEligibility(m);
        });

        std::string updMode = sm.GetBrowserSettings().autoUpdateMode;
        if (sm.GlobalUpdateModeWasAbsentAtLoad()) {
            for (const auto& p : ProfileManager::GetInstance().GetAllProfiles()) {
                std::string pm = SettingsManager::ReadModeFromProfileSettings(p.path);
                if (!pm.empty()) updMode = hodos::MoreConservativeMode(updMode, pm);
            }
            sm.SetGlobalUpdateModeAuthoritative(updMode);
            LOG_INFO("Silent mirror: one-time global update-mode collapse -> " + updMode);
        }
        hodos::MirrorSilentEligibility(updMode);
    }
#endif

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
    // Default=9222, others get 9223+ based on profile number, or 0 to disable.
    // Picker mode owns no profile -> disable (0) so it can't collide with a
    // running profile's DevTools port.
    if (g_picker_mode) {
        settings.remote_debugging_port = 0;
    } else if (profileId == "Default") {
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
    // Dev build offsets the DevTools port (+100) so it never collides with the
    // installed build's port — both otherwise use 9222 for the Default profile,
    // and the 2nd instance to start would fail to bind it.
    if (hodos::IsDevEnv() && settings.remote_debugging_port != 0) {
        settings.remote_debugging_port += 100;
    }
    LOG_INFO("Remote debugging port: " + std::to_string(settings.remote_debugging_port));

    // persist_session_cookies MUST stay disabled — enabling it causes CEF to spawn
    // extra windows on startup (overlay URLs get restored as top-level windows).
    // Tested and confirmed broken 2026-04-30. Session persistence for login sites
    // is handled by the sites themselves via persistent cookies (explicit expiry).
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

    // Picker-UI redesign: the launch profile picker is a SMALL, CENTERED launcher window
    // (not the full-work-area browser window) so it reads as a deliberate "pick a profile"
    // launcher rather than a broken/empty browser window. Picker mode only — the normal
    // window path below is untouched. Fraction-of-workarea + cap keeps it DPI-agnostic and
    // sane on small screens; the picker page (a single full-window chooser) fills it.
    if (g_picker_mode) {
        int pw = width * 60 / 100;  if (pw > 980) pw = 980;  if (pw > width)  pw = width;
        int ph = height * 78 / 100; if (ph > 660) ph = 660;  if (ph > height) ph = height;
        rect.left += (width - pw) / 2;
        rect.top  += (height - ph) / 2;
        width = pw;
        height = ph;
        LOG_INFO("Picker launcher window: " + std::to_string(width) + "x" + std::to_string(height) + " centered");
    }

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

    // Register Bookmarks Panel overlay window class
    WNDCLASS bookmarksPanelOverlayClass = {};
    bookmarksPanelOverlayClass.lpfnWndProc = BookmarksPanelOverlayWndProc;
    bookmarksPanelOverlayClass.hInstance = hInstance;
    bookmarksPanelOverlayClass.lpszClassName = L"CEFBookmarksPanelOverlayWindow";

    if (!RegisterClass(&bookmarksPanelOverlayClass)) {
        LOG_DEBUG("Failed to register bookmarks panel overlay window class. Error: " + std::to_string(GetLastError()));
    }

    // Register Site-Info Panel overlay window class
    WNDCLASS siteInfoPanelOverlayClass = {};
    siteInfoPanelOverlayClass.lpfnWndProc = SiteInfoPanelOverlayWndProc;
    siteInfoPanelOverlayClass.hInstance = hInstance;
    siteInfoPanelOverlayClass.lpszClassName = L"CEFSiteInfoPanelOverlayWindow";

    if (!RegisterClass(&siteInfoPanelOverlayClass)) {
        LOG_DEBUG("Failed to register site-info panel overlay window class. Error: " + std::to_string(GetLastError()));
    }

    // Register Tab-List Panel overlay window class
    WNDCLASS tabListPanelOverlayClass = {};
    tabListPanelOverlayClass.lpfnWndProc = TabListPanelOverlayWndProc;
    tabListPanelOverlayClass.hInstance = hInstance;
    tabListPanelOverlayClass.lpszClassName = L"CEFTabListPanelOverlayWindow";

    if (!RegisterClass(&tabListPanelOverlayClass)) {
        LOG_DEBUG("Failed to register tab-list panel overlay window class. Error: " + std::to_string(GetLastError()));
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
    // Picker mode shows a single full-window chooser browser (no tab strip), so
    // the header child fills the whole client area. WM_SIZE mirrors this.
    HWND header_hwnd = CreateWindow(L"CEFHostWindow", nullptr,
        WS_CHILD | WS_VISIBLE,
        g_picker_mode ? 0 : rb,
        g_picker_mode ? 0 : rb,
        g_picker_mode ? width : (width - 2 * rb),
        g_picker_mode ? height : shellHeight,
        hwnd, nullptr, hInstance, nullptr);

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

    // Set per-profile taskbar grouping and icon badge (skips if single profile).
    // Picker owns no profile -> no badge.
    if (!g_picker_mode) {
        SetupTaskbarProfile(hwnd, hInstance);
    }

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
    // Picker mode opened no profile -> no per-profile DBs and no backend health
    // to wait on. (Shutdown's DB cascade is null-guarded, so skipping init here
    // is safe at teardown — PROFILE_STARTUP_PICKER_DESIGN.md L-1.)
    if (!g_picker_mode) {
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
        // Site permission store (camera/mic/location/notifications/clipboard).
        // Same SQLite/per-profile pattern; init alongside bookmarks.
        if (SitePermissionStore::GetInstance().Initialize(profile_cache)) {
            LOG_INFO("SitePermissionStore initialized successfully");
        } else {
            LOG_ERROR("Failed to initialize SitePermissionStore");
        }
    });

    std::thread paidCacheThread([&profile_cache]() {
        if (PaidContentCache::GetInstance().Initialize(profile_cache)) {
            // Sync enabled flag from persisted settings (default true).
            PaidContentCache::GetInstance().SetEnabled(
                SettingsManager::GetInstance().GetPrivacySettings().paidContentCacheEnabled);
            LOG_INFO("PaidContentCache initialized successfully");
        } else {
            LOG_ERROR("Failed to initialize PaidContentCache");
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
    paidCacheThread.join();

    // Detach server health threads — they set atomic flags when done
    walletThread.detach();
    adblockThread.detach();

    // 6d: if this launch is the silent-update supervisor's --post-update-health-probe
    // relaunch (armed in the 6c.1 honor-probe block), confirm the new build is healthy
    // and write apply.json=healthy so the supervisor commits the apply instead of
    // rolling back. We re-run a REAL /health (not the "launched-but-slow" flag): wallet
    // /health responds (port bound + DB openable) AND adblock port bound, AND our build
    // == the armed toBuild. Bounded < the supervisor's ~120s health wait. Detached +
    // touches no CEF — only QuickHealthCheck/IsPortListening/filesystem.
    if (g_post_update_probe.load()) {
        const long expectedBuild = g_post_update_to_build;
        std::thread([expectedBuild]() {
            const DWORD start = GetTickCount();
            bool walletOk = false, adblockOk = false;
            while (GetTickCount() - start < 110000 && !g_update_abort.load()) {
                if (!walletOk)  walletOk  = QuickHealthCheck();
                if (!adblockOk) adblockOk = IsPortListening(hodos::AdblockPort());
                if (walletOk && adblockOk) break;
                std::this_thread::sleep_for(std::chrono::milliseconds(500));
            }
            if (!(walletOk && adblockOk)) {
                LOG_WARNING("Post-update health: children not healthy in time — supervisor will roll back");
                return;
            }
            if (static_cast<long>(APP_BUILD_NUMBER) != expectedBuild) {
                LOG_WARNING("Post-update health: running build " + std::to_string(APP_BUILD_NUMBER) +
                            " != expected " + std::to_string(expectedBuild) + " — not marking healthy");
                return;
            }
            // Atomically flip apply.json -> healthy (re-read to preserve all fields).
            const std::string applyPath = AppPaths::GetPendingUpdateDir().empty()
                ? "" : AppPaths::GetPendingUpdateDir() + "\\apply.json";
            std::string content;
            hodos::ApplyRecord ar;
            if (applyPath.empty() ||
                !hodos::updatefs::ReadFileAll(HodosUtf8ToWide(applyPath), content) ||
                !hodos::ParseApplyRecord(content, ar)) {
                LOG_WARNING("Post-update health: apply.json gone/unreadable — cannot mark healthy");
                return;
            }
            ar.phase = hodos::ApplyPhase::Healthy;
            if (hodos::updatefs::WriteFileAtomic(HodosUtf8ToWide(applyPath),
                                                 hodos::SerializeApplyRecord(ar))) {
                LOG_INFO("Post-update health: wrote apply.json=healthy (build " +
                         std::to_string(expectedBuild) + ") — apply confirmed");
            }
        }).detach();
    }

    auto initMs = std::chrono::duration_cast<std::chrono::milliseconds>(
        std::chrono::steady_clock::now() - initStart).count();
    LOG_INFO(elapsed() + "STARTUP: DB init done in " + std::to_string(initMs) + "ms (server health in background)");
    }  // end if (!g_picker_mode) — DB init + server health

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
    if (!g_picker_mode) {
        extern void CreateCookiePanelOverlay(HINSTANCE hInstance, bool showImmediately, int iconRightOffset);
        extern void CreateDownloadPanelOverlay(HINSTANCE hInstance, bool showImmediately, int iconRightOffset);
        extern void CreateProfilePanelOverlay(HINSTANCE hInstance, bool showImmediately, int iconRightOffset);
        extern void CreateMenuOverlay(HINSTANCE hInstance, bool showImmediately, int iconRightOffset);
        extern void CreateWalletOverlay(HINSTANCE hInstance, bool showImmediately, int iconRightOffset);
        extern void CreateSiteInfoPanelOverlay(HINSTANCE hInstance, bool showImmediately, int iconLeftOffset);

        HINSTANCE hInst = g_hInstance;
        CefPostDelayedTask(TID_UI, base::BindOnce([](HINSTANCE h) { CreateMenuOverlay(h, false, 30); }, hInst), 1000);
        CefPostDelayedTask(TID_UI, base::BindOnce([](HINSTANCE h) { CreateWalletOverlay(h, false, 50); }, hInst), 1500);
        CefPostDelayedTask(TID_UI, base::BindOnce([](HINSTANCE h) { CreateDownloadPanelOverlay(h, false, 100); }, hInst), 2000);
        CefPostDelayedTask(TID_UI, base::BindOnce([](HINSTANCE h) { CreateCookiePanelOverlay(h, false, 100); }, hInst), 2500);
        CefPostDelayedTask(TID_UI, base::BindOnce([](HINSTANCE h) { CreateProfilePanelOverlay(h, false, 50); }, hInst), 3000);
        // Site-info hub: pre-warm hidden so the first TuneIcon click just shows it
        // (avoids the subprocess-creation hitch that repainted the header on first open).
        CefPostDelayedTask(TID_UI, base::BindOnce([](HINSTANCE h) { CreateSiteInfoPanelOverlay(h, false, 100); }, hInst), 3500);
        LOG_INFO(elapsed() + "STARTUP: Overlay creation deferred");
    }

    // Initialize auto-updater after windows are created.
    // WinSparkle will check for updates in the background if auto-check is enabled.
    // (Skip in picker mode — the chooser is transient and spawns the real instance.)
    if (!g_picker_mode) {
        auto& settings = SettingsManager::GetInstance();
        auto browserSettings = settings.GetBrowserSettings();
        bool autoCheck = (browserSettings.autoUpdateMode != "off");
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
        updater.Initialize(appVersion, appcastUrl, autoCheck);
        LOG_INFO("Auto-updater initialized (version=" + appVersion +
                 ", mode=" + browserSettings.autoUpdateMode + ")");

#ifdef HODOS_SILENT_AUTOUPDATE
        // Commit 4d — silent download-WHILE-running (NOT apply; apply is commit 6).
        // COMPILE-TIME GATED: this is built only when -DHODOS_SILENT_AUTOUPDATE=ON
        // (CMake option, default OFF) so the shipped browser never background-
        // downloads until commit 7 flips it on after the apply path + soak. Even
        // when compiled in, only runs for autoUpdateMode=="silent".
        //
        // Off-thread + detached (matches the startup init-thread idiom). Touches
        // NO CEF objects — only SyncHttpClient/filesystem/OpenSSL/Logger — so it
        // is safe to abandon at exit. Sleeps past first-paint, bails on shutdown
        // via g_update_abort, and single-flights across profiles via a named mutex
        // (multiple HodosBrowser.exe share the one %LOCALAPPDATA% pending dir).
        if (browserSettings.autoUpdateMode == "silent") {
            std::string silentAppcastUrl = appcastUrl;
            int stageDelaySec = 60;
#ifdef HODOS_UPDATE_TEST_SEAM
            // RIG-ONLY (compiled OUT of production): point the real shell's staging at
            // a localhost feed + skip the 60s stagger so a local real-build test can
            // exercise the full bootstrap -> supervisor -> apply path without waiting.
            if (const char* u = std::getenv("HODOS_UPDATE_RIG_URL"); u && *u) { silentAppcastUrl = u; stageDelaySec = 1; }
#endif
            std::thread([silentAppcastUrl, stageDelaySec]() {
                // Stagger ~60s past startup; wake early if shutdown begins.
                for (int i = 0; i < stageDelaySec && !g_update_abort.load(); ++i) {
                    std::this_thread::sleep_for(std::chrono::seconds(1));
                }
                if (g_update_abort.load()) return;

                std::string pendingDir = AppPaths::GetPendingUpdateDir();
                if (pendingDir.empty()) {
                    LOG_WARNING("Silent update: LOCALAPPDATA unavailable — skipping staging");
                    return;
                }
                // Single-flight: only one instance stages at a time (shared dir).
                HANDLE mtx = CreateMutexW(nullptr, FALSE, L"Local\\HodosUpdateStaging");
                if (!mtx || WaitForSingleObject(mtx, 0) != WAIT_OBJECT_0) {
                    if (mtx) CloseHandle(mtx);
                    LOG_INFO("Silent update: another instance is staging — skipping");
                    return;
                }
                try {
                    if (!g_update_abort.load()) {
                        hodos::UpdateStager::StagePendingUpdate(
                            silentAppcastUrl, pendingDir, APP_BUILD_NUMBER, &g_update_abort);
                    }
                } catch (...) {
                    // A detached thread must never let an exception escape
                    // (std::terminate). Staging is best-effort; swallow + retry next launch.
                    LOG_WARNING("Silent update: staging threw — ignored (best-effort)");
                }
                ReleaseMutex(mtx);
                CloseHandle(mtx);
            }).detach();
            LOG_INFO("Silent update: background staging thread scheduled");
        }
#endif  // HODOS_SILENT_AUTOUPDATE
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

    // R2/R3: deterministically checkpoint + close the SQLite browser DBs while the
    // profile lock is STILL held, THEN release the lock. All browsers are closed
    // (message loop exited above), so the DBs are quiescent. This closes the
    // quick-restart SQLITE_BUSY race where a relaunch won the freed lock while the
    // old process still held live-WAL DB handles. CloseDatabase() is idempotent, so
    // the singleton destructors at process exit become no-ops. Done before
    // Logger::Shutdown() so the close logs are captured, and before CefShutdown()
    // (the DBs aren't CEF-managed) per the conventional teardown shape.
    LOG_INFO("Closing browser databases (checkpoint + close)...");
    HistoryManager::GetInstance().Shutdown();
    BookmarkManager::GetInstance().Shutdown();
    SitePermissionStore::GetInstance().Shutdown();
    CookieBlockManager::GetInstance().Shutdown();
    PaidContentCache::GetInstance().Shutdown();

    LOG_INFO("Releasing profile lock...");
    ReleaseProfileLock();

    // 6a: drop the instance-presence mutex so the all-instances-gone gate (commit 6c)
    // sees this instance disappear promptly. The OS would auto-close it at process
    // exit anyway; explicit close is tidy + deterministic for the gate's timing.
    if (g_instance_mutex) {
        CloseHandle(g_instance_mutex);
        g_instance_mutex = nullptr;
    }

    Logger::Shutdown();
    CefShutdown();
    return 0;
}
