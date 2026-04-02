#include "../../include/core/WindowManager.h"
#include "../../include/core/TabManager.h"
#include "../../include/handlers/simple_handler.h"
#include "../../include/handlers/simple_app.h"
#include "../../include/handlers/my_overlay_render_handler.h"
#include "../../include/core/Logger.h"
#include "../../include/core/LayoutHelpers.h"
#include "include/cef_browser.h"
#include "include/cef_request_context.h"
#include <algorithm>
#ifdef _WIN32
#include <dwmapi.h>
#endif

#define LOG_INFO_WM(msg) Logger::Log(msg, 1, 2)
#define LOG_ERROR_WM(msg) Logger::Log(msg, 3, 2)

WindowManager& WindowManager::GetInstance() {
    static WindowManager instance;
    return instance;
}

int WindowManager::CreateWindowRecord() {
    std::lock_guard<std::mutex> lock(mutex_);
    int id = next_window_id_++;
    windows_[id] = std::make_unique<BrowserWindow>(id);
    return id;
}

void WindowManager::RemoveWindow(int window_id) {
    std::lock_guard<std::mutex> lock(mutex_);
    windows_.erase(window_id);
}

BrowserWindow* WindowManager::GetWindow(int window_id) {
    std::lock_guard<std::mutex> lock(mutex_);
    auto it = windows_.find(window_id);
    return (it != windows_.end()) ? it->second.get() : nullptr;
}

BrowserWindow* WindowManager::GetActiveWindow() {
    return GetWindow(active_window_id_);
}

#ifdef _WIN32
BrowserWindow* WindowManager::GetWindowByHwnd(HWND hwnd) {
    std::lock_guard<std::mutex> lock(mutex_);
    for (auto& [id, win] : windows_) {
        if (win->hwnd == hwnd) return win.get();
    }
    return nullptr;
}
#elif defined(__APPLE__)
BrowserWindow* WindowManager::GetWindowByNSWindow(void* nsWindow) {
    std::lock_guard<std::mutex> lock(mutex_);
    for (auto& [id, win] : windows_) {
        if (win->ns_window == nsWindow) return win.get();
    }
    return nullptr;
}
#endif

BrowserWindow* WindowManager::GetWindowForBrowser(int browser_id) {
    std::lock_guard<std::mutex> lock(mutex_);
    for (auto& [id, win] : windows_) {
        // Check all browser refs in this window
        auto check = [browser_id](const CefRefPtr<CefBrowser>& b) {
            return b && b->GetIdentifier() == browser_id;
        };
        if (check(win->header_browser) ||
            check(win->webview_browser) ||
            check(win->wallet_panel_browser) ||
            check(win->overlay_browser) ||
            check(win->settings_browser) ||
            check(win->wallet_browser) ||
            check(win->backup_browser) ||
            check(win->brc100_auth_browser) ||
            check(win->notification_browser) ||
            check(win->settings_menu_browser) ||
            check(win->omnibox_browser) ||
            check(win->cookie_panel_browser) ||
            check(win->download_panel_browser) ||
            check(win->profile_panel_browser) ||
            check(win->menu_browser)) {
            return win.get();
        }
    }
    return nullptr;
}

std::vector<BrowserWindow*> WindowManager::GetAllWindows() {
    std::lock_guard<std::mutex> lock(mutex_);
    std::vector<BrowserWindow*> result;
    result.reserve(windows_.size());
    for (auto& [id, win] : windows_) {
        result.push_back(win.get());
    }
    return result;
}

int WindowManager::GetWindowCount() const {
    std::lock_guard<std::mutex> lock(mutex_);
    return static_cast<int>(windows_.size());
}

void WindowManager::SetActiveWindowId(int id) {
    std::lock_guard<std::mutex> lock(mutex_);
    active_window_id_ = id;
}

int WindowManager::GetActiveWindowId() const {
    std::lock_guard<std::mutex> lock(mutex_);
    return active_window_id_;
}

void WindowManager::SetPrimaryWindowId(int id) {
    std::lock_guard<std::mutex> lock(mutex_);
    primary_window_id_ = id;
}

int WindowManager::GetPrimaryWindowId() const {
    std::lock_guard<std::mutex> lock(mutex_);
    return primary_window_id_;
}

BrowserWindow* WindowManager::GetPrimaryWindow() {
    std::lock_guard<std::mutex> lock(mutex_);
    auto it = windows_.find(primary_window_id_);
    return (it != windows_.end()) ? it->second.get() : nullptr;
}

int WindowManager::GetNextWindowId() const {
    std::lock_guard<std::mutex> lock(mutex_);
    for (auto& [id, win] : windows_) {
        if (id != primary_window_id_) return id;
    }
    return primary_window_id_; // fallback: only one window
}

#ifdef _WIN32

// Forward declarations for WndProcs (defined in cef_browser_shell.cpp)
extern LRESULT CALLBACK ShellWindowProc(HWND, UINT, WPARAM, LPARAM);
extern LRESULT CALLBACK CookiePanelOverlayWndProc(HWND, UINT, WPARAM, LPARAM);
extern LRESULT CALLBACK DownloadPanelOverlayWndProc(HWND, UINT, WPARAM, LPARAM);
extern LRESULT CALLBACK MenuOverlayWndProc(HWND, UINT, WPARAM, LPARAM);
extern HINSTANCE g_hInstance;

BrowserWindow* WindowManager::CreateFullWindow(bool createInitialTab) {
    int wid = CreateWindowRecord();
    BrowserWindow* bw = GetWindow(wid);
    if (!bw) return nullptr;

    LOG_INFO_WM("Creating new browser window (id=" + std::to_string(wid) + ")");

    // --- Screen dimensions ---
    RECT workArea;
    SystemParametersInfo(SPI_GETWORKAREA, 0, &workArea, 0);
    int screenW = workArea.right - workArea.left;
    int screenH = workArea.bottom - workArea.top;

    // Offset each new window slightly so it doesn't stack exactly on top
    int offset = wid * 30;
    int winW = (std::max)(800, screenW - 100);
    int winH = (std::max)(600, screenH - 100);
    int winX = workArea.left + 50 + offset;
    int winY = workArea.top + 50 + offset;

    int shellHeight = GetHeaderHeightPxSystem();
    int tabHeight = winH - shellHeight;

    // --- Create main shell HWND ---
    // Re-use the same window class registered in WinMain ("HodosBrowserWndClass")
    HWND hwnd = CreateWindow(L"HodosBrowserWndClass", L"Hodos Browser",
        WS_POPUP | WS_THICKFRAME | WS_SYSMENU | WS_MINIMIZEBOX | WS_MAXIMIZEBOX | WS_VISIBLE | WS_CLIPCHILDREN,
        winX, winY, winW, winH, nullptr, nullptr, g_hInstance, nullptr);

    if (!hwnd) {
        LOG_ERROR_WM("Failed to create shell HWND for window " + std::to_string(wid));
        RemoveWindow(wid);
        return nullptr;
    }

    // Enable DWM invisible resize borders (same as primary window in cef_browser_shell.cpp)
    MARGINS dwmMargins = {0, 0, 0, 1};
    DwmExtendFrameIntoClientArea(hwnd, &dwmMargins);

    // Store BrowserWindow* in HWND user data so ShellWindowProc can find it
    SetWindowLongPtr(hwnd, GWLP_USERDATA, reinterpret_cast<LONG_PTR>(bw));

    // --- Create header child HWND (inset by resize border for frameless hit-testing) ---
    const int rb = 5; // must match ShellWindowProc WM_NCHITTEST/WM_SIZE
    HWND header_hwnd = CreateWindow(L"CEFHostWindow", nullptr,
        WS_CHILD | WS_VISIBLE, rb, rb, winW - 2 * rb, shellHeight, hwnd, nullptr, g_hInstance, nullptr);

    bw->hwnd = hwnd;
    bw->header_hwnd = header_hwnd;

    ShowWindow(hwnd, SW_SHOW);
    UpdateWindow(hwnd);

    // --- Create header CEF browser ---
    CefWindowInfo headerInfo;
    CefRect headerRect(0, 0, winW, shellHeight);
    headerInfo.SetAsChild(header_hwnd, headerRect);

    CefBrowserSettings browserSettings;
    CefRefPtr<SimpleHandler> headerHandler = new SimpleHandler("header", wid);
    CefBrowserHost::CreateBrowser(headerInfo, headerHandler,
        "http://127.0.0.1:5137", browserSettings,
        nullptr, CefRequestContext::GetGlobalContext());

    // --- Create initial NTP tab (unless restoring session) ---
    if (createInitialTab) {
        int tabId = TabManager::GetInstance().CreateTab(
            "http://127.0.0.1:5137/newtab", hwnd, 0, shellHeight, winW, tabHeight, wid);

        // Only notify the NEW window's header (avoids redundant re-render in other windows)
        SimpleHandler::NotifyWindowTabListChanged(wid);
    }

    SetActiveWindowId(wid);

    // Force a layout refresh on all OTHER windows. Creating a new window can leave
    // stale CEF render buffers in existing windows (the header/tab area on the right
    // side shows artifacts until the user resizes). Sending WM_SIZE with current
    // dimensions triggers the full resize path including CEF WasResized().
    // NOTE: Collect HWNDs first, then SendMessage OUTSIDE the lock (WM_SIZE handler
    // calls back into WindowManager which would deadlock).
    {
        std::vector<HWND> otherHwnds;
        {
            std::lock_guard<std::mutex> lock(mutex_);
            for (auto& [id, win] : windows_) {
                if (id == wid) continue;
                if (win && win->hwnd && IsWindow(win->hwnd)) {
                    otherHwnds.push_back(win->hwnd);
                }
            }
        }
        for (HWND h : otherHwnds) {
            RECT r;
            GetClientRect(h, &r);
            SendMessage(h, WM_SIZE, SIZE_RESTORED,
                        MAKELPARAM(r.right - r.left, r.bottom - r.top));
        }
    }

    LOG_INFO_WM("New browser window created: id=" + std::to_string(wid) +
                ", hwnd=" + std::to_string(reinterpret_cast<uintptr_t>(hwnd)));

    return bw;
}

#endif  // _WIN32
