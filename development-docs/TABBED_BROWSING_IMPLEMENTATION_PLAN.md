# Tabbed Browsing Implementation Plan

**Date**: October 9, 2025
**Last Updated**: December 2024
**Goal**: Add multi-tab browsing with process-per-tab isolation

## 📊 Implementation Status

### ✅ Completed (Pre-Tab Implementation)
- **Navigation Buttons**: Back/forward/reload buttons are implemented and working
  - ✅ Frontend: `useHodosBrowser.ts` and `MainBrowserView.tsx`
  - ✅ Backend: Message handlers in `simple_handler.cpp`
  - ⚠️ **Note**: Will need minor updates to route to active tab (see "Navigation Buttons with Tabs" section)

### 🚧 In Progress
- None currently

### 📋 Planned (Tab Implementation)
- **Phase 1**: Tab architecture design
- **Phase 2**: TabManager implementation
- **Phase 3**: Multi-HWND layout for tabs
- **Phase 4**: React tab bar UI
- **Phase 5**: Navigation handler updates (for tabs)
- **Phase 6**: Tab state synchronization
- **Phase 7**: Wallet/BRC100 integration testing with tabs

## 🔐 Current Security & Process Architecture

### Current Process Model

Your browser currently runs **8 distinct process types**:

```
┌─────────────────────────────────────────────────────────┐
│                    MAIN PROCESS                         │
│  - Shell Window Management                              │
│  - Window Message Loop                                  │
│  - Global State & Coordination                          │
└─────────────────────────────────────────────────────────┘
                           │
        ┌──────────────────┼──────────────────┐
        ▼                  ▼                  ▼
┌────────────┐    ┌────────────┐    ┌────────────┐
│  HEADER    │    │  WEBVIEW   │    │ OVERLAYS   │
│  Process   │    │  Process   │    │ Processes  │
│            │    │            │    │            │
│ React UI   │    │ Web Content│    │ - Settings │
│ Controls   │    │ (1 site)   │    │ - Wallet   │
│            │    │            │    │ - Backup   │
│            │    │            │    │ - BRC100   │
└────────────┘    └────────────┘    └────────────┘
     │                  │                  │
     ▼                  ▼                  ▼
 Own V8 Context   Own V8 Context   Own V8 Context
 Fresh State      Fresh State      Fresh State
```

### Security Boundaries

**Currently Isolated:**
- ✅ **Header browser** - Runs React UI, isolated from web content
- ✅ **Webview browser** - Runs ONE website at a time, isolated from header
- ✅ **Overlay browsers** - Each overlay (settings, wallet, backup, auth) in separate process
- ✅ **Rust Daemon** - Separate process managing wallet operations

**Security Features:**
- ✅ Process isolation between UI and web content
- ✅ Process isolation between overlays
- ✅ Wallet operations in separate Rust daemon process
- ✅ HTTP request interception for domain whitelisting
- ❌ **NO multi-tab isolation** (only one webview at a time)

## 📊 Tabs Implementation: Process-Per-Tab Architecture

**Design Decision**: Each tab runs in its own separate process for security and isolation.

**Architecture:**
```
┌─────────────────────────────────────────────────────────┐
│              Header Browser (React UI)                  │
│  ┌──────────────────────────────────────────────────┐   │
│  │  Tab Bar: [Tab1] [Tab2] [Tab3] [+]              │   │
│  └──────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────┘
         │
         ├──────────────┬──────────────┬──────────────┐
         ▼              ▼              ▼              ▼
   ┌──────────┐   ┌──────────┐   ┌──────────┐   ┌──────────┐
   │  Tab 1   │   │  Tab 2   │   │  Tab 3   │   │  Tab N   │
   │ Browser  │   │ Browser  │   │ Browser  │   │ Browser  │
   │ Process  │   │ Process  │   │ Process  │   │ Process  │
   │          │   │          │   │          │   │          │
   │ Site A   │   │ Site B   │   │ Site C   │   │ Site D   │
   │ V8 Ctx   │   │ V8 Ctx   │   │ V8 Ctx   │   │ V8 Ctx   │
   └──────────┘   └──────────┘   └──────────┘   └──────────┘
```

**Pros:**
- ✅ **Full process isolation** between tabs
- ✅ Each tab has own V8 context
- ✅ Tab crash doesn't affect others
- ✅ Proper security boundaries
- ✅ Matches Chrome/Brave architecture

**Cons:**
- ⚠️ More memory usage (one process per tab)
- ⚠️ More complex implementation
- ⚠️ Need tab lifecycle management

**Wallet/BRC100 Impact:**
- ✅ **SECURE**: Each tab isolated, can't interfere with others
- ✅ Domain whitelisting works per-tab
- ✅ BRC100 auth requests isolated per tab
- ✅ Wallet API injected independently into each tab

**Architecture**: ✅ **PROCESS-PER-TAB** - Matches your existing process-per-overlay architecture

**Key Principle**: Each tab is a separate CEF browser process with its own V8 context, ensuring complete isolation between tabs.

## 🏗️ Implementation Roadmap

### Navigation Buttons with Tabs

**Current Implementation:**
Navigation buttons (back/forward/reload) are already implemented and work with the current single webview browser. They use `SimpleHandler::GetWebviewBrowser()` to get the browser instance.

**With Process-Per-Tab - Required Changes:**

The navigation buttons will need to route to the **active tab's browser** (which runs in its own process) instead of the single webview. Here's what needs to change:

**Current Code (before tabs - single webview):**
```cpp
// In simple_handler.cpp
if (message_name == "navigate_back") {
    CefRefPtr<CefBrowser> webview = SimpleHandler::GetWebviewBrowser();
    if (webview) {
        webview->GoBack();
    }
    return true;
}
```

**Updated Code (with process-per-tab):**
```cpp
// In simple_handler.cpp - after TabManager is implemented
if (message_name == "navigate_back") {
    Tab* activeTab = TabManager::GetInstance()->GetActiveTab();
    if (activeTab && activeTab->browser) {
        activeTab->browser->GoBack();
    }
    return true;
}

if (message_name == "navigate_forward") {
    Tab* activeTab = TabManager::GetInstance()->GetActiveTab();
    if (activeTab && activeTab->browser) {
        activeTab->browser->GoForward();
    }
    return true;
}

if (message_name == "navigate_reload") {
    Tab* activeTab = TabManager::GetInstance()->GetActiveTab();
    if (activeTab && activeTab->browser) {
        activeTab->browser->Reload();
    }
    return true;
}
```

**Frontend Changes:**
✅ **NO CHANGES NEEDED** - The frontend code in `useHodosBrowser.ts` and `MainBrowserView.tsx` can stay exactly as-is. The message protocol (`navigate_back`, `navigate_forward`, `navigate_reload`) remains the same.

**Summary:**
- ✅ Frontend navigation buttons: **No changes needed**
- 🔧 Backend navigation handlers: **Update to use TabManager::GetActiveTab()**
- ✅ Message protocol: **Stays the same**

### Phase 1: Tab Data Structure

**Create tab management system:**

```cpp
// New file: cef-native/include/core/TabManager.h
struct Tab {
    int id;
    std::string url;
    std::string title;
    HWND hwnd;
    CefRefPtr<CefBrowser> browser;
    bool isActive;
    bool isLoading;
};

class TabManager {
public:
    int CreateTab(const std::string& url);
    void CloseTab(int tabId);
    void SwitchToTab(int tabId);
    Tab* GetActiveTab();
    std::vector<Tab> GetAllTabs();

private:
    std::vector<Tab> tabs_;
    int activeTabId_;
    int nextTabId_;
};
```

**Effort**: 4-6 hours
**Complexity**: Medium

### Phase 2: Multi-Tab Window Management

**Modify window creation:**

```cpp
// In OnContextInitialized or new CreateTab function
void TabManager::CreateTab(const std::string& url) {
    // Create HWND for new tab (same size as webview area)
    RECT tabRect;
    GetClientRect(g_webview_area_hwnd, &tabRect);

    HWND tab_hwnd = CreateWindow(
        L"CEFTabWindow",
        nullptr,
        WS_CHILD,  // Child window (hidden by default)
        0, 0, tabRect.right, tabRect.bottom,
        g_webview_area_hwnd,
        nullptr, g_hInstance, nullptr);

    // Create CEF browser for this tab - each tab runs in its own process
    CefWindowInfo window_info;
    window_info.SetAsChild(tab_hwnd, CefRect(0, 0, width, height));

    // Each tab gets its own SimpleHandler instance with unique role
    // This ensures process isolation - each tab is a separate CEF browser process
    CefRefPtr<SimpleHandler> tab_handler = new SimpleHandler("tab-" + std::to_string(tabId));

    // CreateBrowser() creates a new browser process for this tab
    // Each call to CreateBrowser() spawns a new subprocess with its own V8 context
    CefBrowserHost::CreateBrowser(
        window_info,
        tab_handler,
        url,
        settings,
        nullptr,
        CefRequestContext::GetGlobalContext()
    );

    // Store tab info
    Tab newTab;
    newTab.id = nextTabId_++;
    newTab.url = url;
    newTab.hwnd = tab_hwnd;
    tabs_.push_back(newTab);
}

void TabManager::SwitchToTab(int tabId) {
    // Hide all tabs
    for (auto& tab : tabs_) {
        ShowWindow(tab.hwnd, SW_HIDE);
        tab.isActive = false;
    }

    // Show selected tab
    Tab* tab = GetTabById(tabId);
    if (tab) {
        ShowWindow(tab->hwnd, SW_SHOW);
        tab->isActive = true;
        activeTabId_ = tabId;

        // Notify CEF of activation
        if (tab->browser) {
            tab->browser->GetHost()->SetFocus(true);
            tab->browser->GetHost()->WasResized();
        }
    }
}
```

**Effort**: 8-12 hours
**Complexity**: Medium-High

### Phase 3: React Tab Bar UI

**Create tab management UI:**

```tsx
// New file: frontend/src/components/TabBar.tsx
interface Tab {
    id: number;
    title: string;
    url: string;
    isActive: boolean;
}

const TabBar: React.FC = () => {
    const [tabs, setTabs] = useState<Tab[]>([]);
    const [activeTabId, setActiveTabId] = useState<number>(0);

    const createTab = (url: string = 'https://metanetapps.com/') => {
        window.cefMessage?.send('tab_create', [url]);
    };

    const closeTab = (tabId: number) => {
        window.cefMessage?.send('tab_close', [tabId]);
    };

    const switchTab = (tabId: number) => {
        window.cefMessage?.send('tab_switch', [tabId]);
    };

    return (
        <Box sx={{ display: 'flex', bgcolor: 'grey.200', borderBottom: '1px solid #ccc' }}>
            {tabs.map(tab => (
                <Box key={tab.id} sx={{
                    p: 1,
                    bgcolor: tab.isActive ? 'white' : 'transparent',
                    cursor: 'pointer'
                }} onClick={() => switchTab(tab.id)}>
                    <Typography>{tab.title || 'New Tab'}</Typography>
                    <IconButton size="small" onClick={(e) => {
                        e.stopPropagation();
                        closeTab(tab.id);
                    }}>
                        <CloseIcon />
                    </IconButton>
                </Box>
            ))}
            <IconButton onClick={() => createTab()}>
                <AddIcon />
            </IconButton>
        </Box>
    );
};
```

**Effort**: 4-6 hours
**Complexity**: Medium

### Phase 4: Navigation Integration with Tabs

**Update navigation handlers to work with active tab:**

```cpp
// In simple_handler.cpp - update existing handlers
if (message_name == "navigate_back") {
    Tab* activeTab = TabManager::GetInstance()->GetActiveTab();
    if (activeTab && activeTab->browser) {
        activeTab->browser->GoBack();
        LOG_DEBUG_BROWSER("🔙 GoBack() called on active tab " + std::to_string(activeTab->id));
    }
    return true;
}

if (message_name == "navigate_forward") {
    Tab* activeTab = TabManager::GetInstance()->GetActiveTab();
    if (activeTab && activeTab->browser) {
        activeTab->browser->GoForward();
        LOG_DEBUG_BROWSER("🔜 GoForward() called on active tab " + std::to_string(activeTab->id));
    }
    return true;
}

if (message_name == "navigate_reload") {
    Tab* activeTab = TabManager::GetInstance()->GetActiveTab();
    if (activeTab && activeTab->browser) {
        activeTab->browser->Reload();
        LOG_DEBUG_BROWSER("🔄 Reload() called on active tab " + std::to_string(activeTab->id));
    }
    return true;
}

if (message_name == "navigate") {
    // Navigate should also target active tab
    Tab* activeTab = TabManager::GetInstance()->GetActiveTab();
    if (activeTab && activeTab->browser) {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string path = args->GetString(0);

        // Normalize protocol
        if (!(path.rfind("http://", 0) == 0 || path.rfind("https://", 0) == 0)) {
            path = "http://" + path;
        }

        activeTab->browser->GetMainFrame()->LoadURL(path);
        LOG_DEBUG_BROWSER("🔁 Navigate to " + path + " on active tab " + std::to_string(activeTab->id));
    }
    return true;
}
```

**Effort**: 2-3 hours
**Complexity**: Low (simple refactoring)

## 🔐 Wallet & BRC100 Functionality with Tabs

### How Wallet API Works Per-Tab

**Current Architecture (Single Webview - Before Tabs):**
```
External Website → HTTP Request → CEF Interceptor → Domain Check → Rust Daemon
```

**With Process-Per-Tab Architecture:**
```
Tab 1 (peerpay.com)  ─┐
Tab 2 (metanet.com)  ─┼─→ HTTP Interceptor → Domain Check → Rust Daemon
Tab 3 (thryll.com)   ─┘
```

### Key Insight: Tabs Work Independently!

**Each tab process:**
- ✅ Has own V8 JavaScript context
- ✅ Gets `bitcoinBrowser` API injected independently
- ✅ Makes HTTP requests independently
- ✅ Subject to domain whitelisting independently
- ✅ Can authenticate with BRC100 independently

**Rust Daemon:**
- ✅ Single daemon serves ALL tabs
- ✅ Handles concurrent requests from multiple tabs
- ✅ Domain whitelist applies to all tabs
- ✅ Session management tracks which tab made request

### BRC100 Authentication Per-Tab

**Current Flow:**
```
1. Site requests auth → HTTP Interceptor → Show approval modal
2. User approves → Store in session
3. Site gets auth response
```

**With Tabs:**
```
Tab 1: peerpay.com → Auth request → Modal shows → User approves → Tab 1 authenticated ✅
Tab 2: thryll.com  → Auth request → Modal shows → User approves → Tab 2 authenticated ✅
Tab 3: peerpay.com → Uses Tab 1's session (same domain) ✅
```

**Changes Needed:**
- ✅ **Session management**: Track which tab is authenticated
- ✅ **Domain whitelist**: Shared across all tabs
- ✅ **Modal context**: Know which tab triggered auth request
- ✅ **Concurrent requests**: Handle multiple tabs requesting auth

### Wallet Operations Per-Tab

**Scenario: Two tabs both use wallet**

```
Tab 1 (peerpay.com):
- Requests transaction → Domain check ✅ → User approves → Transaction sent

Tab 2 (thryll.com):
- Requests transaction → Domain check ✅ → User approves → Transaction sent

Both work independently through same Rust daemon!
```

**Concurrency Considerations:**
- ✅ Rust daemon handles concurrent HTTP requests natively
- ✅ Each tab gets independent response
- ⚠️ Need to prevent UTXO double-spend (Rust daemon tracks used UTXOs)
- ⚠️ Transaction confirmation modal should show which tab initiated

## 🛠️ Impact on Existing Code

### Minimal Impact Areas (Won't Need Changes)

1. **Rust Wallet Daemon** ✅
   - Already handles HTTP requests
   - Concurrent request handling built-in
   - No changes needed

2. **HTTP Request Interceptor** ✅
   - Already intercepts requests per-browser
   - Works independently for each browser process
   - Domain whitelisting works as-is

3. **BRC100 Authentication** ✅
   - Each browser gets API injection independently
   - Auth flow works per-browser
   - Minimal changes needed

4. **Overlay Windows** ✅
   - Completely independent from tabs
   - Continue to work as-is
   - No changes needed

### Major Impact Areas (Will Need Changes)

1. **Window Management** 🔧
   - Currently: Single `g_webview_hwnd`
   - With Tabs: Multiple tab HWNDs, manage visibility
   - Change: Tab switching = hide/show different HWNDs

2. **Browser References** 🔧
   - Currently: `SimpleHandler::webview_browser_`
   - With Tabs: Array/map of tab browsers
   - Change: Track multiple browsers, switch active browser

3. **Navigation** 🔧
   - Currently: Navigate changes `webview_browser_` URL
   - With Tabs: Navigate changes active tab's URL
   - Change: Route to active tab browser

4. **Message Routing** 🔧
   - Currently: Messages go to specific browser (header/webview/overlay)
   - With Tabs: Messages need tab context
   - Change: Include tab ID in messages

5. **Tab State Management** 🔧
   - New: Track tab titles, URLs, loading states
   - New: Tab switching logic
   - New: Tab close cleanup

## 📋 Implementation Steps (Recommended Order)

### Step 1: Design Tab Architecture

**Plan:**
- Tab data structure
- Tab manager class
- Window layout changes
- Message protocol for tabs

**Effort**: 2-4 hours (planning/design)

### Step 2: Implement TabManager

**Create:**
- `TabManager` class for tab lifecycle
- Tab creation/deletion/switching
- Browser reference management
- HWND visibility management

**Effort**: 8-12 hours

### Step 3: Multi-HWND Layout

**Modify:**
- Create container HWND for tabs
- Stack tab HWNDs (show/hide on switch)
- Handle WM_SIZE for all tab HWNDs
- Window cleanup on tab close

**Effort**: 6-8 hours

### Step 4: React Tab Bar

**Create:**
- Tab bar component
- Tab switching UI
- New tab button
- Close tab button
- Tab title updates

**Effort**: 4-6 hours

### Step 5: Update Navigation Handlers

**Update existing navigation handlers to use TabManager:**

```cpp
// Update navigate_back, navigate_forward, navigate_reload, navigate
// to use TabManager::GetInstance()->GetActiveTab() instead of GetWebviewBrowser()
```

**Effort**: 2-3 hours
**Complexity**: Low

### Step 6: State Synchronization

**Implement:**
- Tab state updates (title, URL, loading)
- Active tab tracking
- Tab reordering (optional)
- Tab persistence (optional)

**Effort**: 4-6 hours

### Step 7: Wallet/BRC100 Integration

**Test & Verify:**
- Each tab can authenticate independently
- Domain whitelist works per-tab
- Transaction requests from different tabs
- Concurrent wallet operations

**Effort**: 4-6 hours (testing)

## 🎯 Recommendation: Phased Approach

### Phase 1: Tab Architecture Design

**Design:**
1. Tab data structures
2. Process-per-tab model
3. Message protocol
4. State management

**Effort**: 1 day (design/planning)
**Complexity**: MEDIUM
**Value**: MEDIUM (planning)

### Phase 2: Implement Tabs

**Build:**
1. TabManager class
2. Multi-HWND layout
3. React tab bar
4. Tab switching

**Effort**: 1-2 weeks
**Complexity**: HIGH
**Value**: HIGH (major feature)

## 📊 Tabs + Wallet Security Analysis

### Security Model

**Process Isolation:**
```
Tab 1 Process → Can only access own V8 context
Tab 2 Process → Can only access own V8 context
Tab 3 Process → Can only access own V8 context
     ↓                    ↓                    ↓
All communicate via → HTTP Interceptor → Rust Daemon
                         (Security boundary)
```

**Security Benefits:**
- ✅ Tab cannot read another tab's memory
- ✅ Malicious site can't intercept other tab's requests
- ✅ Tab crash doesn't affect other tabs
- ✅ Each tab subject to domain whitelisting independently

**Wallet API Injection:**
```cpp
// In OnContextCreated for EACH tab browser
void InjectWalletAPI(CefRefPtr<CefBrowser> browser) {
    // This is injected into EACH tab's V8 context
    // Each tab gets fresh API injection
    // No sharing between tabs
}
```

### BRC100 Authentication Scenarios

**Scenario 1: Multiple Tabs, Same Domain**
```
Tab 1: peerpay.com → Authenticates → Session stored for "peerpay.com"
Tab 2: peerpay.com → Reuses session from Tab 1 ✅
```

**Scenario 2: Multiple Tabs, Different Domains**
```
Tab 1: peerpay.com → Authenticates → Session for "peerpay.com"
Tab 2: thryll.com  → Authenticates → Session for "thryll.com"
Each independent ✅
```

**Scenario 3: Concurrent Transactions**
```
Tab 1: Sends transaction for 1000 sats
Tab 2: Sends transaction for 500 sats
Rust Daemon: Handles sequentially, prevents UTXO double-spend ✅
```

## ⚠️ Potential Issues to Address

### Issue 1: UTXO Locking

**Problem**: Two tabs trying to use same UTXO simultaneously

**Solution:**
```rust
// In Rust daemon - UTXO locking
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

struct UTXOLock {
    utxos: Arc<Mutex<HashMap<String, bool>>>,  // txid:vout -> locked
}

impl UTXOLock {
    fn reserve_utxo(&self, txid: &str, vout: u32) -> bool {
        let mut utxos = self.utxos.lock().unwrap();
        let key = format!("{}:{}", txid, vout);

        if utxos.contains_key(&key) && *utxos.get(&key).unwrap() {
            return false;  // Already locked
        }
        utxos.insert(key, true);
        true
    }
}
```

### Issue 2: Multiple Auth Modals

**Problem**: Multiple tabs requesting auth simultaneously

**Solution:**
```cpp
// Queue auth requests, show one at a time
class AuthRequestQueue {
    std::queue<AuthRequest> pending_;
    bool modalShowing_;

    void QueueAuthRequest(const std::string& domain, int tabId) {
        pending_.push({domain, tabId});
        if (!modalShowing_) {
            ShowNextAuthModal();
        }
    }
};
```

### Issue 3: Tab Context for Responses

**Problem**: Which tab gets the response?

**Solution:**
```cpp
// Include tab ID in all messages
struct PendingRequest {
    int tabId;
    std::string domain;
    CefRefPtr<CefResourceHandler> handler;
};

// Route response back to correct tab browser
void SendResponseToTab(int tabId, const std::string& response) {
    Tab* tab = TabManager::GetTabById(tabId);
    if (tab && tab->browser) {
        // Send response to specific tab's browser
    }
}
```

## 📊 Effort Estimation

### Navigation Buttons Update (for tabs)
- **Time**: 2-3 hours
- **Complexity**: ⭐ Low
- **Priority**: ⭐⭐⭐ Medium (part of tab implementation)
- **Note**: Frontend stays the same, backend needs TabManager integration

### Tab System (Process-Per-Tab)
- **Time**: 2-3 weeks full implementation
- **Complexity**: ⭐⭐⭐⭐ High
- **Priority**: ⭐⭐⭐ Medium (nice to have)

### Breakdown:
- Tab data structures: 4 hours
- TabManager class: 12 hours
- Multi-HWND management: 8 hours
- React tab bar: 6 hours
- Navigation handler updates: 2-3 hours
- Message routing: 6 hours
- Testing & debugging: 12 hours
- Wallet/BRC100 integration testing: 8 hours
- **Total**: ~58 hours (1.5-2 weeks)

## 🎯 Final Recommendation

### Implementation Order:

**1. Design Tab Architecture (CURRENT PRIORITY)**
- Create detailed design document
- Review security implications
- Plan message protocol
- Design tab UI/UX
- Review process-per-tab implementation details
- Plan TabManager class structure

**2. Implement Tab System (2-3 weeks)**
- Build TabManager class
- Implement multi-HWND layout
- Create React tab bar
- Update navigation handlers to use active tab
- Test tab creation/switching/closing

**3. Integration & Testing**
- Test wallet/BRC100 with tabs
- Test navigation buttons with tabs
- Test concurrent tab operations
- Security validation

## 📚 Key Takeaways

### Tabs + Wallet: YES, They Work Together!

**Answer**: ✅ **YES**, wallet and BRC100 can work independently in each tab's process because:

1. **HTTP Interception**: Works per-process, routes to central Rust daemon
2. **API Injection**: Each tab gets fresh `bitcoinBrowser` API in its V8 context
3. **Domain Whitelisting**: Applies independently to each tab
4. **Process Isolation**: Each tab secure from other tabs

### Navigation Buttons with Tabs

**Answer**: ✅ **Frontend stays the same, backend needs minor updates**

**Current Status:**
- ✅ Navigation buttons are implemented and working
- ✅ Frontend code (`useHodosBrowser.ts`, `MainBrowserView.tsx`) needs **NO changes**
- 🔧 Backend handlers need to route to active tab (process-per-tab) instead of single webview

**What Changes:**
1. **Frontend**: ✅ **No changes** - Message protocol stays the same
2. **Backend**: 🔧 **Update handlers** - Use `TabManager::GetActiveTab()` instead of `GetWebviewBrowser()`
3. **Effort**: 2-3 hours to update 4 message handlers

**Implementation**: See "Navigation Buttons with Tabs" section above for code changes.

### Tabs Are Compatible with Your Architecture

**Good News**: Your process-per-overlay architecture is **perfect** for process-per-tab!

You already have:
- ✅ Experience managing multiple browser processes
- ✅ Message routing between processes
- ✅ HWND management for multiple windows
- ✅ API injection into multiple contexts
- ✅ Security boundary enforcement

**Tabs will follow same pattern as overlays!**

## 🚀 Next Steps

**Current Priority:**
1. Design tab system architecture
2. Create detailed TabManager class design
3. Review security implications for tabs
4. Plan message protocol for tab operations
5. Design tab UI/UX

**Implementation Phase:**
1. Implement TabManager class
2. Build multi-HWND layout for tabs
3. Create React tab bar UI
4. Update navigation handlers to use active tab
5. Test tab creation/switching/closing

**Testing Phase:**
1. Test wallet/BRC100 with tabs
2. Test navigation buttons with tabs
3. Test concurrent tab operations
4. Security validation

---

**Ready to implement tabs!** Focus on TabManager design and process-per-tab architecture.
