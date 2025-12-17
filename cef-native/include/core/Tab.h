#ifndef TAB_H
#define TAB_H

#include <string>
#include <chrono>
#include <windows.h>
#include "include/cef_browser.h"

// Forward declaration to avoid circular dependency
class SimpleHandler;

/**
 * @brief Represents a single browser tab with its associated browser instance and state
 *
 * Each Tab maintains:
 * - A unique identifier
 * - Window handle (HWND) for the tab's browser container
 * - CEF browser instance (runs in separate process for isolation)
 * - Handler for CEF callbacks
 * - Current URL and title
 * - State flags (visible, loading, navigation capabilities)
 * - Creation timestamp
 */
struct Tab {
    // Unique identifier for this tab
    int id;

    // Current page title (updated from OnTitleChange)
    std::string title;

    // Current URL (updated from OnAddressChange)
    std::string url;

    // Window handle for this tab's browser container
    // Each tab gets its own HWND which is shown/hidden on tab switch
    HWND hwnd;

    // CEF browser instance for this tab
    // Each tab runs in a separate render process for security isolation
    CefRefPtr<CefBrowser> browser;

    // Handler for this tab's browser (manages CEF callbacks)
    CefRefPtr<SimpleHandler> handler;

    // Whether this tab is currently visible (active tab)
    // Only one tab should be visible at a time
    bool is_visible;

    // Whether this tab is currently loading a page
    bool is_loading;

    // Whether this tab is being closed (pending destruction)
    bool is_closing;

    // Navigation state (from OnLoadingStateChange)
    bool can_go_back;
    bool can_go_forward;

    // When this tab was created
    std::chrono::system_clock::time_point created_at;

    // Last time this tab was accessed (for tab ordering/LRU)
    std::chrono::system_clock::time_point last_accessed;

    /**
     * @brief Constructor - initializes a new tab
     */
    Tab()
        : id(0),
          title("New Tab"),
          url(""),
          hwnd(nullptr),
          browser(nullptr),
          handler(nullptr),
          is_visible(false),
          is_loading(false),
          is_closing(false),
          can_go_back(false),
          can_go_forward(false),
          created_at(std::chrono::system_clock::now()),
          last_accessed(std::chrono::system_clock::now()) {
    }

    /**
     * @brief Constructor with ID and URL
     */
    Tab(int tab_id, const std::string& initial_url)
        : id(tab_id),
          title("New Tab"),
          url(initial_url),
          hwnd(nullptr),
          browser(nullptr),
          handler(nullptr),
          is_visible(false),
          is_loading(true),
          is_closing(false),
          can_go_back(false),
          can_go_forward(false),
          created_at(std::chrono::system_clock::now()),
          last_accessed(std::chrono::system_clock::now()) {
    }
};

#endif // TAB_H
