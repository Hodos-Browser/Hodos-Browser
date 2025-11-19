# Tabbed Browsing Implementation Plan

**Date**: October 9, 2025
**Goal**: Add multi-tab browsing with process-per-tab isolation

## 🔐 Current Security & Process Architecture

### Current Process Model

Your browser currently runs **7 distinct process types**:

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
- ✅ **Go Daemon** - Separate process managing wallet operations

**Security Features:**
- ✅ Process isolation between UI and web content
- ✅ Process isolation between overlays
- ✅ Wallet operations in separate Go daemon process
- ✅ HTTP request interception for domain whitelisting
- ❌ **NO multi-tab isolation** (only one webview at a time)

## 📊 Tabs Implementation: Two Approaches

### Approach 1: Single Webview with Tab Management (Simpler)

**Architecture:**
```
┌─────────────────────────────────────┐
│         Header Browser              │
│  ┌─────────────────────────────┐    │
│  │  React Tab Bar UI           │    │
│  │  [Tab1] [Tab2] [Tab3] [+]   │    │
│  └─────────────────────────────┘    │
└─────────────────────────────────────┘
         │
         ▼
┌─────────────────────────────────────┐
│    Single Webview Browser           │
│  Loads different URLs on tab switch │
│  LoadURL() when changing tabs       │
└─────────────────────────────────────┘
```

**Pros:**
- ✅ Simple to implement
- ✅ Minimal code changes
- ✅ Lower memory usage
- ✅ Fast tab switching

**Cons:**
- ❌ **NO process isolation between tabs**
- ❌ All tabs share same V8 context
- ❌ Security risk if one site compromises another
- ❌ All tabs stop if one crashes

**Wallet/BRC100 Impact:**
- ⚠️ **SECURITY RISK**: All tabs can access wallet API simultaneously
- ⚠️ Domain whitelisting complex (multiple sites in same process)
- ⚠️ One malicious tab can intercept another tab's wallet operations

**Verdict**: ❌ **NOT RECOMMENDED** for Bitcoin wallet browser

### Approach 2: Process-Per-Tab (Recommended)

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

**Verdict**: ✅ **RECOMMENDED** - Matches your existing process-per-overlay architecture

## 🏗️ Implementation Roadmap

### Phase 1: Basic Navigation (Do This First!) 🎯

**RECOMMENDATION: Implement back/forward/refresh BEFORE tabs**

**Why First:**
- ✅ Essential browser functionality
- ✅ Simple to implement
- ✅ No architecture changes needed
- ✅ Tests window management system
- ✅ Users need this immediately

**Implementation:**

```typescript
// In MainBrowserView.tsx
const { navigate, goBack, goForward, reload } = useBitcoinBrowser();

<IconButton onClick={goBack}>
    <ArrowBackIcon />
</IconButton>
<IconButton onClick={goForward}>
    <ArrowForwardIcon />
</IconButton>
<IconButton onClick={reload}>
    <RefreshIcon />
</IconButton>
```

```cpp
// In NavigationHandler.cpp (already exists!)
void NavigationHandler::goBack() {
    CefRefPtr<CefBrowser> browser = SimpleHandler::GetWebviewBrowser();
    if (browser) {
        browser->GoBack();
    }
}

void NavigationHandler::goForward() {
    CefRefPtr<CefBrowser> browser = SimpleHandler::GetWebviewBrowser();
    if (browser) {
        browser->GoForward();
    }
}

void NavigationHandler::reload() {
    CefRefPtr<CefBrowser> browser = SimpleHandler::GetWebviewBrowser();
    if (browser) {
        browser->Reload();
    }
}
```

**Effort**: 1-2 hours
**Complexity**: Low
**Value**: High (essential browser features)

### Phase 2: Tab Data Structure

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

### Phase 3: Multi-Tab Window Management

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

    // Create CEF browser for this tab
    CefWindowInfo window_info;
    window_info.SetAsChild(tab_hwnd, CefRect(0, 0, width, height));

    CefRefPtr<SimpleHandler> tab_handler = new SimpleHandler("tab-" + std::to_string(tabId));

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

### Phase 4: React Tab Bar UI

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

## 🔐 Wallet & BRC100 Functionality with Tabs

### How Wallet API Works Per-Tab

**Current Architecture (Single Webview):**
```
External Website → HTTP Request → CEF Interceptor → Domain Check → Go Daemon
```

**With Tabs (Process-Per-Tab):**
```
Tab 1 (peerpay.com)  ─┐
Tab 2 (metanet.com)  ─┼─→ HTTP Interceptor → Domain Check → Go Daemon
Tab 3 (thryll.com)   ─┘
```

### Key Insight: Tabs Work Independently!

**Each tab process:**
- ✅ Has own V8 JavaScript context
- ✅ Gets `bitcoinBrowser` API injected independently
- ✅ Makes HTTP requests independently
- ✅ Subject to domain whitelisting independently
- ✅ Can authenticate with BRC100 independently

**Go Daemon:**
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

Both work independently through same Go daemon!
```

**Concurrency Considerations:**
- ✅ Go daemon handles concurrent HTTP requests natively
- ✅ Each tab gets independent response
- ⚠️ Need to prevent UTXO double-spend (Go daemon tracks used UTXOs)
- ⚠️ Transaction confirmation modal should show which tab initiated

## 🛠️ Impact on Existing Code

### Minimal Impact Areas (Won't Need Changes)

1. **Go Wallet Daemon** ✅
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

### Step 0: Basic Navigation First! 🎯 **DO THIS FIRST**

**Implement:**
- Back button functionality
- Forward button functionality
- Refresh button functionality

**Reason:**
- Essential features users need immediately
- Tests existing single-webview system
- No architecture changes
- Can implement in 1-2 hours

**Code Changes:**
```typescript
// In useBitcoinBrowser.ts - these might already exist!
const goBack = () => {
    window.cefMessage?.send('navigate_back', []);
};

const goForward = () => {
    window.cefMessage?.send('navigate_forward', []);
};

const reload = () => {
    window.cefMessage?.send('navigate_reload', []);
};
```

```cpp
// In simple_handler.cpp - add handlers
if (message_name == "navigate_back") {
    if (webview_browser_) {
        webview_browser_->GoBack();
    }
    return true;
}

if (message_name == "navigate_forward") {
    if (webview_browser_) {
        webview_browser_->GoForward();
    }
    return true;
}

if (message_name == "navigate_reload") {
    if (webview_browser_) {
        webview_browser_->Reload();
    }
    return true;
}
```

**Effort**: 1-2 hours
**Value**: HIGH (essential functionality)

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

### Step 5: State Synchronization

**Implement:**
- Tab state updates (title, URL, loading)
- Active tab tracking
- Tab reordering (optional)
- Tab persistence (optional)

**Effort**: 4-6 hours

### Step 6: Wallet/BRC100 Integration

**Test & Verify:**
- Each tab can authenticate independently
- Domain whitelist works per-tab
- Transaction requests from different tabs
- Concurrent wallet operations

**Effort**: 4-6 hours (testing)

## 🎯 Recommendation: Phased Approach

### Phase 1 (THIS WEEK): Essential Navigation ⭐ **PRIORITY**

**Implement:**
1. ✅ Back button
2. ✅ Forward button
3. ✅ Refresh button
4. ✅ URL bar updates on navigation

**Effort**: 2-3 hours
**Complexity**: LOW
**Value**: HIGH

**Why First:**
- Users need this immediately
- Tests existing architecture
- No process changes needed
- Foundation for tabs

### Phase 2 (NEXT WEEK): Tab Architecture Design

**Design:**
1. Tab data structures
2. Process-per-tab model
3. Message protocol
4. State management

**Effort**: 1 day (design/planning)
**Complexity**: MEDIUM
**Value**: MEDIUM (planning)

### Phase 3 (FUTURE): Implement Tabs

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
All communicate via → HTTP Interceptor → Go Daemon
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
Go Daemon: Handles sequentially, prevents UTXO double-spend ✅
```

## ⚠️ Potential Issues to Address

### Issue 1: UTXO Locking

**Problem**: Two tabs trying to use same UTXO simultaneously

**Solution:**
```go
// In Go daemon - UTXO locking
type UTXOLock struct {
    utxos map[string]bool  // txid:vout -> locked
    mu    sync.Mutex
}

func (u *UTXOLock) ReserveUTXO(txid string, vout int) bool {
    u.mu.Lock()
    defer u.mu.Unlock()

    key := fmt.Sprintf("%s:%d", txid, vout)
    if u.utxos[key] {
        return false  // Already locked
    }
    u.utxos[key] = true
    return true
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

### Back/Forward/Refresh Buttons
- **Time**: 2-3 hours
- **Complexity**: ⭐ Low
- **Priority**: ⭐⭐⭐⭐⭐ CRITICAL

### Tab System (Process-Per-Tab)
- **Time**: 2-3 weeks full implementation
- **Complexity**: ⭐⭐⭐⭐ High
- **Priority**: ⭐⭐⭐ Medium (nice to have)

### Breakdown:
- Tab data structures: 4 hours
- TabManager class: 12 hours
- Multi-HWND management: 8 hours
- React tab bar: 6 hours
- Message routing: 6 hours
- Testing & debugging: 12 hours
- Wallet/BRC100 integration testing: 8 hours
- **Total**: ~56 hours (1.5-2 weeks)

## 🎯 Final Recommendation

### Do This Order:

**1. Basic Navigation (THIS WEEK)** ⭐⭐⭐⭐⭐
- Implement back/forward/refresh buttons
- Essential functionality
- 2-3 hours effort
- High value, low risk

**2. Test & Polish Current Features (THIS WEEK)**
- Test wallet operations thoroughly
- Test overlay windows
- Test window management
- Polish UX

**3. Design Tab Architecture (NEXT WEEK)**
- Create detailed design document
- Review security implications
- Plan message protocol
- Design tab UI/UX

**4. Implement Tabs (FUTURE - 2-3 weeks)**
- Build TabManager
- Implement multi-HWND layout
- Create React tab bar
- Test wallet/BRC100 with tabs

## 📚 Key Takeaways

### Tabs + Wallet: YES, They Work Together!

**Answer**: ✅ **YES**, wallet and BRC100 can work independently in each tab's process because:

1. **HTTP Interception**: Works per-process, routes to central Go daemon
2. **API Injection**: Each tab gets fresh `bitcoinBrowser` API in its V8 context
3. **Domain Whitelisting**: Applies independently to each tab
4. **Process Isolation**: Each tab secure from other tabs

### Should You Implement Navigation First?

**Answer**: ✅ **ABSOLUTELY YES!**

**Reasons:**
1. **Users need it now** - Back/forward/refresh are essential
2. **Low risk** - Simple implementation, no architecture changes
3. **Fast implementation** - 2-3 hours vs 2-3 weeks for tabs
4. **Tests system** - Validates current architecture works
5. **Foundation** - Understanding navigation helps with tabs

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

**Immediate (Tonight/Tomorrow):**
1. Implement back/forward/refresh buttons (2-3 hours)
2. Test navigation thoroughly
3. Polish UI for buttons

**This Week:**
1. Complete navigation features
2. Test wallet operations
3. Document current architecture

**Next Week:**
1. Design tab system architecture
2. Create tab implementation plan
3. Review security implications

**Future:**
1. Implement TabManager
2. Build tab UI
3. Test with wallet/BRC100

---

**Ready to implement back/forward/refresh buttons first?** It's the smart move - essential features, quick win, foundation for tabs!
