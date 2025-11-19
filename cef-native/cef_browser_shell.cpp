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
#include <shellapi.h>
#include <windows.h>
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

// Log levels
enum class LogLevel {
    DEBUG = 0,
    INFO = 1,
    WARNING = 2,
    ERROR_LEVEL = 3
};

// Process types for identification
enum class ProcessType {
    MAIN = 0,
    RENDER = 1,
    BROWSER = 2
};

// Centralized Logger class
class Logger {
private:
    static std::ofstream logFile;
    static bool initialized;
    static ProcessType currentProcess;
    static std::string logFilePath;

    static std::string GetTimestamp() {
        auto now = std::chrono::system_clock::now();
        auto time_t = std::chrono::system_clock::to_time_t(now);
        auto ms = std::chrono::duration_cast<std::chrono::milliseconds>(
            now.time_since_epoch()) % 1000;

        std::stringstream ss;
        ss << std::put_time(std::localtime(&time_t), "%Y-%m-%d %H:%M:%S");
        ss << "." << std::setfill('0') << std::setw(3) << ms.count();
        return ss.str();
    }

    static std::string GetProcessName(ProcessType process) {
        switch (process) {
            case ProcessType::MAIN: return "MAIN";
            case ProcessType::RENDER: return "RENDER";
            case ProcessType::BROWSER: return "BROWSER";
            default: return "UNKNOWN";
        }
    }

    static std::string GetLogLevelName(LogLevel level) {
        switch (level) {
            case LogLevel::DEBUG: return "DEBUG";
            case LogLevel::INFO: return "INFO";
            case LogLevel::WARNING: return "WARN";
            case LogLevel::ERROR_LEVEL: return "ERROR";
            default: return "UNKNOWN";
        }
    }

public:
    static void Initialize(ProcessType process, const std::string& filePath = "debug_output.log") {
        if (initialized) return;

        currentProcess = process;
        logFilePath = filePath;

        // Open log file
        logFile.open(logFilePath, std::ios::app);
        if (logFile.is_open()) {
            initialized = true;
            Log("Logger initialized for " + GetProcessName(process), 1);
        } else {
            // Fallback to stdout if file can't be opened
            std::cout << "WARNING: Could not open log file: " << filePath << std::endl;
        }
    }

    static void Log(const std::string& message, int level = 1, int process = 0) {
        LogLevel logLevel = static_cast<LogLevel>(level);
        ProcessType processType = static_cast<ProcessType>(process);

        if (!initialized) {
            // Fallback logging if not initialized
            std::cout << "[" << GetTimestamp() << "] [" << GetProcessName(processType) << "] [" << GetLogLevelName(logLevel) << "] " << message << std::endl;
            return;
        }

        std::string logEntry = "[" + GetTimestamp() + "] [" + GetProcessName(processType) + "] [" + GetLogLevelName(logLevel) + "] " + message;

        // Write to file
        if (logFile.is_open()) {
            logFile << logEntry << std::endl;
            logFile.flush();
        }

        // Also write to stdout (for debugging)
        std::cout << logEntry << std::endl;
    }

    static void Shutdown() {
        if (initialized && logFile.is_open()) {
            Log("Logger shutting down", 1);
            logFile.close();
            initialized = false;
        }
    }

    static bool IsInitialized() {
        return initialized;
    }
};

// Static member definitions
std::ofstream Logger::logFile;
bool Logger::initialized = false;
ProcessType Logger::currentProcess = ProcessType::MAIN;
std::string Logger::logFilePath = "";

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

            // IMPORTANT: Call DefWindowProc to ensure Windows updates internal state
            break;  // Let DefWindowProc handle WM_MOVE
        }

        case WM_SIZE: {
            // Handle window resizing - resize child windows and CEF browsers
            RECT rect;
            GetClientRect(hwnd, &rect);
            int width = rect.right - rect.left;
            int height = rect.bottom - rect.top;
            int shellHeight = 80; // Header height
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

            // Resize webview window
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

            return 0;
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
    int shellHeight = 80;
    int webviewHeight = height - shellHeight;

    WNDCLASS wc = {}; wc.lpfnWndProc = ShellWindowProc; wc.hInstance = hInstance;
    wc.lpszClassName = L"BitcoinBrowserWndClass"; RegisterClass(&wc);

    WNDCLASS browserClass = {}; browserClass.lpfnWndProc = DefWindowProc; browserClass.hInstance = hInstance;
    browserClass.lpszClassName = L"CEFHostWindow"; RegisterClass(&browserClass);


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

    HWND hwnd = CreateWindow(L"BitcoinBrowserWndClass", L"Bitcoin Browser / Babbage Browser",
        WS_OVERLAPPEDWINDOW | WS_VISIBLE | WS_CLIPCHILDREN,
        rect.left, rect.top, width, height, nullptr, nullptr, hInstance, nullptr);

    HWND header_hwnd = CreateWindow(L"CEFHostWindow", nullptr,
        WS_CHILD | WS_VISIBLE, 0, 0, width, shellHeight, hwnd, nullptr, hInstance, nullptr);

    HWND webview_hwnd = CreateWindow(L"CEFHostWindow", nullptr,
        WS_CHILD | WS_VISIBLE, 0, shellHeight, width, webviewHeight, hwnd, nullptr, hInstance, nullptr);

    // 🌍 Assign to globals
    g_hwnd = hwnd;
    g_header_hwnd = header_hwnd;
    g_webview_hwnd = webview_hwnd;

    ShowWindow(hwnd, SW_SHOW);        UpdateWindow(hwnd);
    ShowWindow(header_hwnd, SW_SHOW); UpdateWindow(header_hwnd);
    ShowWindow(webview_hwnd, SW_SHOW); UpdateWindow(webview_hwnd);

    LOG_DEBUG("Initializing CEF...");
    bool success = CefInitialize(main_args, settings, app, nullptr);
    LOG_DEBUG("CefInitialize success: " + std::string(success ? "true" : "false"));

    if (!success) return 1;

    // 💡 Optionally pass handles to app instance
    app->SetWindowHandles(hwnd, header_hwnd, webview_hwnd);

    CefRunMessageLoop();
    CefShutdown();
    return 0;
}
