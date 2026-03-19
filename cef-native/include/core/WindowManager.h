#pragma once

#include <map>
#include <unordered_map>
#include <memory>
#include <mutex>
#include <vector>
#include "BrowserWindow.h"

/**
 * @brief Singleton managing all BrowserWindow instances.
 *
 * Thread-safe.  Window 0 is created on first access and corresponds to the
 * original single-window globals.  Phase 2 (multi-window) will create
 * additional windows via CreateWindowRecord().
 */
class WindowManager {
public:
    static WindowManager& GetInstance();

    // Allocate a new window_id + BrowserWindow.  Returns the new id.
    int CreateWindowRecord();

    // Remove a window (called when a top-level window is closed).
    void RemoveWindow(int window_id);

    // Lookups
    BrowserWindow* GetWindow(int window_id);
    BrowserWindow* GetActiveWindow();

#ifdef _WIN32
    BrowserWindow* GetWindowByHwnd(HWND hwnd);
#elif defined(__APPLE__)
    BrowserWindow* GetWindowByNSWindow(void* nsWindow);
#endif

    // Find which window owns a particular CEF browser (by browser identifier).
    BrowserWindow* GetWindowForBrowser(int browser_id);

    std::vector<BrowserWindow*> GetAllWindows();
    int GetWindowCount() const;

    void SetActiveWindowId(int id);
    int GetActiveWindowId() const;

#ifdef _WIN32
    // Create a full new top-level browser window (Phase 2: multi-window).
    // Creates HWND, header browser, pre-created overlays, and an initial NTP tab.
    // Pass createInitialTab=false during session restore (tabs will be added by caller).
    // Returns the BrowserWindow* (owned by WindowManager).
    BrowserWindow* CreateFullWindow(bool createInitialTab = true);
#elif defined(__APPLE__)
    BrowserWindow* CreateFullWindow(bool createInitialTab = true);
#endif

private:
    WindowManager() = default;
    ~WindowManager() = default;
    WindowManager(const WindowManager&) = delete;
    WindowManager& operator=(const WindowManager&) = delete;

    std::unordered_map<int, std::unique_ptr<BrowserWindow>> windows_;
    int active_window_id_ = 0;
    int next_window_id_ = 0;
    mutable std::mutex mutex_;
};
