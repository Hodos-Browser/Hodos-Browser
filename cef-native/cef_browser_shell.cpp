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

// Global mouse hook for omnibox click-outside detection
HHOOK g_omnibox_mouse_hook = nullptr;

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

// Graceful shutdown function
void ShutdownApplication() {
    LOG_INFO("🛑 Starting graceful application shutdown...");

    // Step 0: Stop Go daemon first (to prevent orphaned processes)
    LOG_INFO("🔄 Stopping Go daemon...");
    // Note: WalletService destructor will be called automatically when the app exits
    // This ensures the daemon is properly terminated

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

            // Move settings overlay if it exists and is visible
            if (g_settings_overlay_hwnd && IsWindow(g_settings_overlay_hwnd) && IsWindowVisible(g_settings_overlay_hwnd)) {
                SetWindowPos(g_settings_overlay_hwnd, HWND_TOPMOST,
                    mainRect.left, mainRect.top, width, height,
                    SWP_NOACTIVATE | SWP_SHOWWINDOW);
                LOG_DEBUG("🔄 Moved settings overlay to match main window");
            }

            // Move wallet overlay if it exists and is visible
            if (g_wallet_overlay_hwnd && IsWindow(g_wallet_overlay_hwnd) && IsWindowVisible(g_wallet_overlay_hwnd)) {
                SetWindowPos(g_wallet_overlay_hwnd, HWND_TOPMOST,
                    mainRect.left, mainRect.top, width, height,
                    SWP_NOACTIVATE | SWP_SHOWWINDOW);
                LOG_DEBUG("🔄 Moved wallet overlay to match main window");
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

            // Resize settings overlay
            if (g_settings_overlay_hwnd && IsWindow(g_settings_overlay_hwnd) && IsWindowVisible(g_settings_overlay_hwnd)) {
                SetWindowPos(g_settings_overlay_hwnd, HWND_TOPMOST,
                    mainRect.left, mainRect.top, width, height,
                    SWP_NOACTIVATE | SWP_SHOWWINDOW);

                // Notify CEF browser of resize
                CefRefPtr<CefBrowser> settings_browser = SimpleHandler::GetSettingsBrowser();
                if (settings_browser) {
                    settings_browser->GetHost()->WasResized();
                }
                LOG_DEBUG("🔄 Resized settings overlay to match main window");
            }

            // Resize wallet overlay
            if (g_wallet_overlay_hwnd && IsWindow(g_wallet_overlay_hwnd) && IsWindowVisible(g_wallet_overlay_hwnd)) {
                SetWindowPos(g_wallet_overlay_hwnd, HWND_TOPMOST,
                    mainRect.left, mainRect.top, width, height,
                    SWP_NOACTIVATE | SWP_SHOWWINDOW);

                // Notify CEF browser of resize
                CefRefPtr<CefBrowser> wallet_browser = SimpleHandler::GetWalletBrowser();
                if (wallet_browser) {
                    wallet_browser->GetHost()->WasResized();
                }
                LOG_DEBUG("🔄 Resized wallet overlay to match main window");
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
                // App is losing focus - close wallet overlay if it's open
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
            LOG_INFO("👆 Settings Overlay HWND received WM_MOUSEACTIVATE");
            // Allow normal activation without forcing z-order
            return MA_ACTIVATE;

        case WM_LBUTTONDOWN: {
            LOG_DEBUG("🖱️ Settings Overlay received WM_LBUTTONDOWN");
            SetFocus(hwnd);

            POINT pt = { GET_X_LPARAM(lParam), GET_Y_LPARAM(lParam) };
            // Translate to CEF MouseEvent
            CefMouseEvent mouse_event;
            mouse_event.x = pt.x;
            mouse_event.y = pt.y;
            mouse_event.modifiers = 0;

            // Find the settings browser
            CefRefPtr<CefBrowser> settings_browser = SimpleHandler::GetSettingsBrowser();
            if (settings_browser) {
                settings_browser->GetHost()->SendMouseClickEvent(mouse_event, MBT_LEFT, false, 1);  // mouse down
                settings_browser->GetHost()->SendMouseClickEvent(mouse_event, MBT_LEFT, true, 1);   // mouse up
                LOG_DEBUG("🧠 Left-click sent to settings overlay browser");
            } else {
                LOG_WARNING("⚠️ No settings overlay browser to send left-click");
            }

            return 0;
        }

        case WM_RBUTTONDOWN: {
            LOG_DEBUG("🖱️ Settings Overlay received WM_RBUTTONDOWN");
            SetFocus(hwnd);

            POINT pt = { GET_X_LPARAM(lParam), GET_Y_LPARAM(lParam) };
            // Translate to CEF MouseEvent
            CefMouseEvent mouse_event;
            mouse_event.x = pt.x;
            mouse_event.y = pt.y;
            mouse_event.modifiers = 0;

            // Find the settings browser
            CefRefPtr<CefBrowser> settings_browser = SimpleHandler::GetSettingsBrowser();
            if (settings_browser) {
                settings_browser->GetHost()->SendMouseClickEvent(mouse_event, MBT_RIGHT, false, 1);  // mouse down
                settings_browser->GetHost()->SendMouseClickEvent(mouse_event, MBT_RIGHT, true, 1);   // mouse up
                LOG_DEBUG("🧠 Right-click sent to settings overlay browser");
            } else {
                LOG_WARNING("⚠️ No settings overlay browser to send right-click");
            }

            return 0;
        }

        case WM_KEYDOWN: {
            LOG_DEBUG("⌨️ Settings Overlay received WM_KEYDOWN - key: " + std::to_string(wParam));
            SetFocus(hwnd);

            // Find the settings browser
            CefRefPtr<CefBrowser> settings_browser = SimpleHandler::GetSettingsBrowser();
            if (settings_browser) {
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

                settings_browser->GetHost()->SendKeyEvent(key_event);
                LOG_DEBUG("⌨️ Key down sent to settings overlay browser (modifiers: " + std::to_string(modifiers) + ")");
            } else {
                LOG_WARNING("⚠️ No settings overlay browser to send key down");
            }

            return 0;
        }

        case WM_KEYUP: {
            LOG_DEBUG("⌨️ Settings Overlay received WM_KEYUP - key: " + std::to_string(wParam));
            SetFocus(hwnd);

            // Find the settings browser
            CefRefPtr<CefBrowser> settings_browser = SimpleHandler::GetSettingsBrowser();
            if (settings_browser) {
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

                settings_browser->GetHost()->SendKeyEvent(key_event);
                LOG_DEBUG("⌨️ Key up sent to settings overlay browser (modifiers: " + std::to_string(modifiers) + ")");
            } else {
                LOG_WARNING("⚠️ No settings overlay browser to send key up");
            }

            return 0;
        }

        case WM_CHAR: {
            LOG_DEBUG("⌨️ Settings Overlay received WM_CHAR - char: " + std::to_string(wParam));
            SetFocus(hwnd);

            // Find the settings browser
            CefRefPtr<CefBrowser> settings_browser = SimpleHandler::GetSettingsBrowser();
            if (settings_browser) {
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

                settings_browser->GetHost()->SendKeyEvent(key_event);
                LOG_DEBUG("⌨️ Char sent to settings overlay browser (modifiers: " + std::to_string(modifiers) + ")");
            } else {
                LOG_WARNING("⚠️ No settings overlay browser to send char");
            }

            return 0;
        }

        case WM_CLOSE:
            LOG_INFO("❌ Settings Overlay received WM_CLOSE - destroying window");
            DestroyWindow(hwnd);
            return 0;

        case WM_DESTROY:
            LOG_INFO("❌ Settings Overlay received WM_DESTROY - cleaning up");
            // Clean up any resources if needed
            return 0;

        case WM_ACTIVATE:
            LOG_DEBUG("⚡ Settings HWND activated with state: " + std::to_string(LOWORD(wParam)));
            break;

        case WM_WINDOWPOSCHANGING:
            // Allow normal z-order changes for better window management
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

    // Enable CEF's runtime API for JavaScript communication
    CefString(&settings.javascript_flags).FromASCII("--expose-gc --allow-running-insecure-content");

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

    // 💡 Optionally pass handles to app instance
    app->SetWindowHandles(hwnd, header_hwnd, webview_hwnd);

    CefRunMessageLoop();
    CefShutdown();
    return 0;
}
