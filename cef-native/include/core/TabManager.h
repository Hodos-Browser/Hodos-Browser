#ifndef TAB_MANAGER_H
#define TAB_MANAGER_H

#include <map>
#include <vector>
#include <string>
#include <memory>
#include <windows.h>
#include "include/cef_browser.h"
#include "Tab.h"

/**
 * @brief Singleton class that manages all browser tabs
 *
 * TabManager handles:
 * - Tab creation and destruction
 * - Tab switching (show/hide HWNDs)
 * - Tab state management (title, URL, loading state)
 * - Browser registration from SimpleHandler callbacks
 * - Routing navigation commands to active tab
 *
 * Architecture:
 * - Each tab runs in a separate CEF browser process (process-per-tab)
 * - Each tab has its own HWND (show/hide for tab switching)
 * - Only one tab is visible at a time (active tab)
 * - TabManager maintains map of all tabs
 *
 * Thread Safety:
 * - All methods must be called on the CEF UI thread
 * - CEF_REQUIRE_UI_THREAD() checks are used where appropriate
 */
class TabManager {
public:
    /**
     * @brief Get singleton instance
     * @return Reference to the singleton TabManager
     */
    static TabManager& GetInstance();

    /**
     * @brief Destructor - cleanup all tabs
     */
    ~TabManager();

    // Delete copy constructor and assignment operator (singleton pattern)
    TabManager(const TabManager&) = delete;
    TabManager& operator=(const TabManager&) = delete;

    // ========== Tab Lifecycle Methods ==========

    /**
     * @brief Create a new tab
     *
     * This method:
     * 1. Creates a new HWND for the tab's browser
     * 2. Creates a CEF browser instance in a new process
     * 3. Registers the tab in the tabs_ map
     * 4. Switches to the new tab (makes it visible)
     *
     * @param url Initial URL to load
     * @param parent_hwnd Parent window for the tab's HWND
     * @param x X position within parent window
     * @param y Y position within parent window (typically below header)
     * @param width Width of the tab's browser
     * @param height Height of the tab's browser
     * @return ID of the created tab
     *
     * @note The browser is created asynchronously. The CefRefPtr<CefBrowser>
     *       will be set later in RegisterTabBrowser() when OnAfterCreated is called
     */
    int CreateTab(const std::string& url, HWND parent_hwnd, int x, int y, int width, int height);

    /**
     * @brief Close a tab
     *
     * This method:
     * 1. Closes the CEF browser (triggers browser process cleanup)
     * 2. Destroys the tab's HWND
     * 3. Removes tab from tabs_ map
     * 4. If closing active tab, switches to another tab
     *
     * @param tab_id ID of tab to close
     * @return true if tab was found and closed, false otherwise
     */
    bool CloseTab(int tab_id);

    /**
     * @brief Switch to a different tab
     *
     * This method:
     * 1. Hides all tabs (ShowWindow(SW_HIDE))
     * 2. Shows the selected tab (ShowWindow(SW_SHOW))
     * 3. Sets focus to the selected tab's browser
     * 4. Updates active_tab_id_
     *
     * @param tab_id ID of tab to switch to
     * @return true if switch was successful, false if tab not found
     */
    bool SwitchToTab(int tab_id);

    // ========== Tab Query Methods ==========

    /**
     * @brief Get tab by ID
     * @param tab_id ID of tab to retrieve
     * @return Pointer to Tab struct, or nullptr if not found
     */
    Tab* GetTab(int tab_id);

    /**
     * @brief Get currently active tab
     * @return Pointer to active Tab struct, or nullptr if no tabs exist
     */
    Tab* GetActiveTab();

    /**
     * @brief Get all tabs
     * @return Vector of pointers to all tabs (in creation order)
     */
    std::vector<Tab*> GetAllTabs();

    /**
     * @brief Get ID of active tab
     * @return Active tab ID, or -1 if no tabs exist
     */
    int GetActiveTabId() const { return active_tab_id_; }

    /**
     * @brief Get number of tabs
     * @return Count of all tabs
     */
    size_t GetTabCount() const { return tabs_.size(); }

    // ========== Tab State Update Methods ==========
    // These are called from SimpleHandler callbacks to keep tab state in sync

    /**
     * @brief Update tab title (called from SimpleHandler::OnTitleChange)
     * @param tab_id ID of tab to update
     * @param title New page title
     */
    void UpdateTabTitle(int tab_id, const std::string& title);

    /**
     * @brief Update tab URL (called from SimpleHandler::OnAddressChange)
     * @param tab_id ID of tab to update
     * @param url New URL
     */
    void UpdateTabURL(int tab_id, const std::string& url);

    /**
     * @brief Update tab loading state (called from SimpleHandler::OnLoadingStateChange)
     * @param tab_id ID of tab to update
     * @param is_loading Whether page is currently loading
     * @param can_go_back Whether back navigation is possible
     * @param can_go_forward Whether forward navigation is possible
     */
    void UpdateTabLoadingState(int tab_id, bool is_loading, bool can_go_back, bool can_go_forward);

    // ========== Browser Registration ==========

    /**
     * @brief Register a browser with a tab (called from SimpleHandler::OnAfterCreated)
     *
     * When CEF creates a browser asynchronously, OnAfterCreated is called.
     * The SimpleHandler extracts the tab ID from its role string (e.g., "tab_1")
     * and calls this method to associate the browser with the correct tab.
     *
     * @param tab_id ID of tab to register browser with
     * @param browser CEF browser instance to register
     * @return true if registration was successful, false if tab not found
     */
    bool RegisterTabBrowser(int tab_id, CefRefPtr<CefBrowser> browser);

private:
    /**
     * @brief Private constructor (singleton pattern)
     */
    TabManager();

    // ========== Private Helper Methods ==========

    /**
     * @brief Get next available tab ID
     * @return Unique tab ID
     */
    int GetNextTabId() { return next_tab_id_++; }

    /**
     * @brief Find a tab to switch to after closing a tab
     *
     * Strategy:
     * - If closing active tab, switch to most recently accessed tab
     * - If no other tabs, return -1
     *
     * @param closing_tab_id ID of tab being closed
     * @return ID of tab to switch to, or -1 if no other tabs
     */
    int FindTabToSwitchTo(int closing_tab_id);

    // ========== Member Variables ==========

    // Map of tab ID to Tab struct
    std::map<int, Tab> tabs_;

    // ID of currently active (visible) tab
    int active_tab_id_;

    // Next tab ID to assign (monotonically increasing)
    int next_tab_id_;

    // Singleton instance
    static std::unique_ptr<TabManager> instance_;
};

#endif // TAB_MANAGER_H
