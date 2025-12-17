# Phase 1 Implementation - File Modifications Guide

This document provides exact code modifications needed for Phase 1 tab implementation.

---

## File 1: simple_handler.h

**Location**: `cef-native/include/handlers/simple_handler.h`

### Modification 1.1: Add TabManager Include

**After line 10** (after other includes), add:
```cpp
#include "include/core/TabManager.h"
```

### Modification 1.2: Add Helper Method Declaration

**In the `private:` section** (around line 100), add:
```cpp
    /**
     * @brief Extract tab ID from role string
     * Role format: "tab_1", "tab_2", etc.
     * @return Tab ID, or -1 if not a tab role
     */
    static int ExtractTabIdFromRole(const std::string& role);
```

---

## File 2: simple_handler.cpp

**Location**: `cef-native/src/handlers/simple_handler.cpp`

### Modification 2.1: Add TabManager Include

**At the top** (after existing includes), add:
```cpp
#include "include/core/TabManager.h"
```

### Modification 2.2: Implement ExtractTabIdFromRole Helper

**Add this function** somewhere after the constructor (around line 200):
```cpp
// Static helper to extract tab ID from role string
int SimpleHandler::ExtractTabIdFromRole(const std::string& role) {
    if (role.rfind("tab_", 0) == 0) {
        // Role is "tab_X" - extract X
        std::string id_str = role.substr(4);  // Skip "tab_"
        try {
            return std::stoi(id_str);
        } catch (...) {
            return -1;
        }
    }
    return -1;
}
```

### Modification 2.3: Update OnAfterCreated for Tab Registration

**In `OnAfterCreated()` method** (around line 240-285), **ADD** this code **BEFORE** the existing role checks:
```cpp
    // Check if this is a tab browser
    int tab_id = ExtractTabIdFromRole(role_);
    if (tab_id != -1) {
        // This is a tab browser - register with TabManager
        TabManager::GetInstance().RegisterTabBrowser(tab_id, browser);
        LOG_DEBUG_BROWSER("Tab browser registered: ID " + std::to_string(tab_id)
                         + ", Browser ID: " + std::to_string(browser->GetIdentifier()));
        browser->GetHost()->WasResized();
        return;
    }
```

### Modification 2.4: Add OnAddressChange Handler

**Find the `OnTitleChange()` method** (should be around line 2000+) and **ADD THIS METHOD AFTER IT**:
```cpp
void SimpleHandler::OnAddressChange(CefRefPtr<CefBrowser> browser,
                                   CefRefPtr<CefFrame> frame,
                                   const CefString& url) {
    CEF_REQUIRE_UI_THREAD();

    if (!frame->IsMain()) {
        return;
    }

    // Check if this is a tab browser
    int tab_id = ExtractTabIdFromRole(role_);
    if (tab_id != -1) {
        TabManager::GetInstance().UpdateTabURL(tab_id, url.ToString());
    }

    // Also update header browser's address bar
    // TODO: Send message to header to update address bar display
}
```

### Modification 2.5: Update OnTitleChange

**In `OnTitleChange()` method**, **ADD** this code **AT THE BEGINNING**:
```cpp
void SimpleHandler::OnTitleChange(CefRefPtr<CefBrowser> browser, const CefString& title) {
    CEF_REQUIRE_UI_THREAD();

    // Check if this is a tab browser
    int tab_id = ExtractTabIdFromRole(role_);
    if (tab_id != -1) {
        TabManager::GetInstance().UpdateTabTitle(tab_id, title.ToString());
    }

    // ... existing code continues ...
```

### Modification 2.6: Update OnLoadingStateChange

**In `OnLoadingStateChange()` method**, **ADD** this code **AT THE BEGINNING**:
```cpp
void SimpleHandler::OnLoadingStateChange(CefRefPtr<CefBrowser> browser,
                                        bool isLoading,
                                        bool canGoBack,
                                        bool canGoForward) {
    CEF_REQUIRE_UI_THREAD();

    // Check if this is a tab browser
    int tab_id = ExtractTabIdFromRole(role_);
    if (tab_id != -1) {
        TabManager::GetInstance().UpdateTabLoadingState(tab_id, isLoading, canGoBack, canGoForward);
    }

    // ... existing code continues ...
```

### Modification 2.7: Add Tab Message Handlers

**In `OnProcessMessageReceived()` method**, **ADD THESE HANDLERS** at the top of the message handling section (around line 350):

```cpp
    // ========== TAB MANAGEMENT MESSAGES ==========

    if (message_name == "tab_create") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string url = args->GetSize() > 0 ? args->GetString(0).ToString() : "";

        // Get main window dimensions for tab size
        RECT rect;
        GetClientRect(g_hwnd, &rect);
        int width = rect.right - rect.left;
        int height = rect.bottom - rect.top;

        // Account for header height (8%)
        int shellHeight = std::max(60, static_cast<int>(height * 0.08));
        int tabHeight = height - shellHeight;

        int tab_id = TabManager::GetInstance().CreateTab(url, g_hwnd, width, tabHeight);

        LOG_DEBUG_BROWSER("Tab created: ID " + std::to_string(tab_id));

        // TODO: Send tab list update to frontend
        return true;
    }

    if (message_name == "tab_close") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        if (args->GetSize() > 0) {
            int tab_id = args->GetInt(0);
            bool success = TabManager::GetInstance().CloseTab(tab_id);

            LOG_DEBUG_BROWSER("Tab close: ID " + std::to_string(tab_id)
                             + (success ? " succeeded" : " failed"));

            // TODO: Send tab list update to frontend
        }
        return true;
    }

    if (message_name == "tab_switch") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        if (args->GetSize() > 0) {
            int tab_id = args->GetInt(0);
            bool success = TabManager::GetInstance().SwitchToTab(tab_id);

            LOG_DEBUG_BROWSER("Tab switch: ID " + std::to_string(tab_id)
                             + (success ? " succeeded" : " failed"));
        }
        return true;
    }

    if (message_name == "get_tab_list") {
        // Get all tabs and send to frontend
        std::vector<Tab*> tabs = TabManager::GetInstance().GetAllTabs();
        int active_tab_id = TabManager::GetInstance().GetActiveTabId();

        // Build JSON response
        std::stringstream json;
        json << "{\"tabs\":[";
        for (size_t i = 0; i < tabs.size(); i++) {
            Tab* tab = tabs[i];
            if (i > 0) json << ",";
            json << "{";
            json << "\"id\":" << tab->id << ",";
            json << "\"title\":\"" << tab->title << "\",";
            json << "\"url\":\"" << tab->url << "\",";
            json << "\"isActive\":" << (tab->id == active_tab_id ? "true" : "false") << ",";
            json << "\"isLoading\":" << (tab->is_loading ? "true" : "false");
            json << "}";
        }
        json << "],\"activeTabId\":" << active_tab_id << "}";

        // Send response to header browser
        CefRefPtr<CefBrowser> header = SimpleHandler::GetHeaderBrowser();
        if (header) {
            CefRefPtr<CefProcessMessage> response = CefProcessMessage::Create("tab_list_response");
            CefRefPtr<CefListValue> response_args = response->GetArgumentList();
            response_args->SetString(0, json.str());
            header->GetMainFrame()->SendProcessMessage(PID_RENDERER, response);
        }

        return true;
    }
```

### Modification 2.8: Update Navigation Handlers to Use Active Tab

**FIND** each of these message handlers and **REPLACE** `SimpleHandler::GetWebviewBrowser()` with `TabManager::GetInstance().GetActiveTab()`:

**Message: "navigate"**
```cpp
    if (message_name == "navigate") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string path = args->GetString(0);

        // CHANGE FROM:
        // CefRefPtr<CefBrowser> webview = SimpleHandler::GetWebviewBrowser();

        // CHANGE TO:
        Tab* active_tab = TabManager::GetInstance().GetActiveTab();
        if (active_tab && active_tab->browser) {
            // Normalize protocol
            if (!(path.rfind("http://", 0) == 0 || path.rfind("https://", 0) == 0)) {
                path = "http://" + path;
            }

            active_tab->browser->GetMainFrame()->LoadURL(path);
            LOG_DEBUG_BROWSER("Navigate to " + path + " on active tab " + std::to_string(active_tab->id));
        }
        return true;
    }
```

**Message: "navigate_back"**
```cpp
    if (message_name == "navigate_back") {
        // CHANGE FROM:
        // CefRefPtr<CefBrowser> webview = SimpleHandler::GetWebviewBrowser();

        // CHANGE TO:
        Tab* active_tab = TabManager::GetInstance().GetActiveTab();
        if (active_tab && active_tab->browser) {
            active_tab->browser->GoBack();
            LOG_DEBUG_BROWSER("GoBack() on active tab " + std::to_string(active_tab->id));
        }
        return true;
    }
```

**Message: "navigate_forward"**
```cpp
    if (message_name == "navigate_forward") {
        // CHANGE FROM:
        // CefRefPtr<CefBrowser> webview = SimpleHandler::GetWebviewBrowser();

        // CHANGE TO:
        Tab* active_tab = TabManager::GetInstance().GetActiveTab();
        if (active_tab && active_tab->browser) {
            active_tab->browser->GoForward();
            LOG_DEBUG_BROWSER("GoForward() on active tab " + std::to_string(active_tab->id));
        }
        return true;
    }
```

**Message: "navigate_reload"**
```cpp
    if (message_name == "navigate_reload") {
        // CHANGE FROM:
        // CefRefPtr<CefBrowser> webview = SimpleHandler::GetWebviewBrowser();

        // CHANGE TO:
        Tab* active_tab = TabManager::GetInstance().GetActiveTab();
        if (active_tab && active_tab->browser) {
            active_tab->browser->Reload();
            LOG_DEBUG_BROWSER("Reload() on active tab " + std::to_string(active_tab->id));
        }
        return true;
    }
```

---

## File 3: cef_browser_shell.cpp

**Location**: `cef-native/cef_browser_shell.cpp`

### Modification 3.1: Add TabManager Include

**After existing includes** (around line 20), add:
```cpp
#include "include/core/TabManager.h"
```

### Modification 3.2: Comment Out or Remove Single Webview HWND Creation

**In WinMain**, FIND the webview window creation code (around line 990-1000) and **COMMENT IT OUT**:
```cpp
    // OLD CODE - COMMENT OUT:
    /*
    HWND webview_hwnd = CreateWindow(L"CEFHostWindow", nullptr,
        WS_CHILD | WS_VISIBLE,
        0, shellHeight, width, webviewHeight,
        hwnd, nullptr, hInstance, nullptr);

    if (!webview_hwnd) {
        MessageBox(nullptr, L"Failed to create webview window", L"Error", MB_OK | MB_ICONERROR);
        return 1;
    }

    g_webview_hwnd = webview_hwnd;
    */

    // NEW: Tabs will create their own HWNDs dynamically
    // Set g_webview_hwnd to nullptr for now (or to a container HWND if you create one)
    g_webview_hwnd = nullptr;
```

### Modification 3.3: Create Initial Tab After CEF Initialization

**In `simple_app.cpp`**, FIND the `OnContextInitialized()` method (around line 100-150) and **ADD** at the **END** of the method:

```cpp
void SimpleApp::OnContextInitialized() {
    CEF_REQUIRE_UI_THREAD();

    // ... existing header browser creation code ...

    // OLD: Create single webview browser - COMMENT THIS OUT
    /*
    CefWindowInfo webview_window_info;
    webview_window_info.SetAsChild(g_webview_hwnd, CefRect(0, 0, webviewWidth, webviewHeight));
    CefRefPtr<SimpleHandler> webview_handler = new SimpleHandler("webview");
    CefBrowserSettings webview_settings;
    bool webview_result = CefBrowserHost::CreateBrowser(...);
    */

    // NEW: Create initial tab using TabManager
    RECT mainRect;
    GetClientRect(g_hwnd, &mainRect);
    int width = mainRect.right - mainRect.left;
    int height = mainRect.bottom - mainRect.top;
    int shellHeight = std::max(60, static_cast<int>(height * 0.08));
    int tabHeight = height - shellHeight;

    // Create first tab
    int initial_tab_id = TabManager::GetInstance().CreateTab(
        "https://metanetapps.com/",
        g_hwnd,
        width,
        tabHeight
    );

    LOG(INFO) << "Initial tab created: ID " << initial_tab_id;

    // ... existing overlay creation code continues ...
}
```

### Modification 3.4: Update WM_SIZE Handler

**In WM_SIZE handler** (around line 333-440), **ADD** tab resizing:

```cpp
case WM_SIZE: {
    RECT rect;
    GetClientRect(hwnd, &rect);
    int width = rect.right - rect.left;
    int height = rect.bottom - rect.top;
    int shellHeight = std::max(60, static_cast<int>(height * 0.08));
    int webviewHeight = height - shellHeight;

    // Resize header window and browser (existing code - keep it)
    // ...

    // NEW: Resize all tab windows and browsers
    std::vector<Tab*> tabs = TabManager::GetInstance().GetAllTabs();
    for (Tab* tab : tabs) {
        if (tab && tab->hwnd && IsWindow(tab->hwnd)) {
            // Resize tab HWND
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
}
break;
```

---

## File 4: CMakeLists.txt

**Location**: `cef-native/CMakeLists.txt`

### Modification 4.1: Add New Source Files

**FIND** the `set(SOURCES ...)` section and **ADD**:
```cmake
set(SOURCES
    cef_browser_shell.cpp
    src/handlers/simple_handler.cpp
    src/handlers/simple_render_process_handler.cpp
    src/handlers/simple_app.cpp
    src/handlers/my_overlay_render_handler.cpp
    src/core/WalletService.cpp
    src/core/IdentityHandler.cpp
    src/core/NavigationHandler.cpp
    src/core/AddressHandler.cpp
    src/core/BRC100Bridge.cpp
    src/core/BRC100Handler.cpp
    src/core/HttpRequestInterceptor.cpp
    src/core/WebSocketServerHandler.cpp
    # NEW FILES FOR TABS:
    include/core/Tab.h
    include/core/TabManager.h
    src/core/TabManager.cpp
    # Add other source files here
)
```

---

## Testing Checklist

After making these modifications:

1. **Build the project**:
   ```bash
   cd cef-native
   cmake --build build --config Release
   ```

2. **Run the browser**:
   ```bash
   ./build/bin/Release/HodosBrowserShell.exe
   ```

3. **Test basic functionality**:
   - [ ] Browser starts without crashes
   - [ ] Initial tab created automatically
   - [ ] Can navigate in the tab
   - [ ] Navigation buttons work (back/forward/reload)
   - [ ] Address bar works

4. **Test tab management** (from browser console):
   ```javascript
   // Create new tab
   window.cefMessage.send("tab_create", "https://google.com");

   // Get tab list
   window.cefMessage.send("get_tab_list");

   // Switch to tab (replace 1 with actual tab ID)
   window.cefMessage.send("tab_switch", 1);

   // Close tab (replace 1 with actual tab ID)
   window.cefMessage.send("tab_close", 1);
   ```

5. **Check CEF logs** for any errors:
   - Look for "TabManager initialized"
   - Look for "Creating tab X with URL: ..."
   - Look for "Tab browser registered: ID X"
   - Check for any errors or warnings

---

## Next Steps After Phase 1

Once Phase 1 is working:
1. Test thoroughly with multiple tabs
2. Check for memory leaks (create/close 20+ tabs)
3. Verify process isolation (check Task Manager for multiple processes)
4. Proceed to Phase 2: React Tab Bar UI
5. Update TABBED_IMPLEMENTATION_STATUS.md

---

**End of Modification Guide**
