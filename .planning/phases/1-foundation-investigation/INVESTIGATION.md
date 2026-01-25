# Foundation Investigation: Address Bar and History Architecture

**Purpose**: Document existing address bar and history database implementation to guide omnibox replacement decisions.

**Investigation Date**: 2026-01-24

---

## Table of Contents

1. [React Component Architecture](#react-component-architecture)
2. [History Database Architecture](#history-database-architecture)
3. [Data Flow Diagram](#data-flow-diagram)
4. [IPC Protocol](#ipc-protocol)
5. [Replace vs Reuse Decision Matrix](#replace-vs-reuse-decision-matrix)

---

## React Component Architecture

### Component Tree

```
MainBrowserView (src/pages/MainBrowserView.tsx)
├── Box (MUI container)
│   ├── TabBar (component)
│   │   └── Manages tabs via useTabManager hook
│   └── Toolbar (MUI - navigation bar)
│       ├── IconButton (Back) → goBack()
│       ├── IconButton (Forward) → goForward()
│       ├── IconButton (Refresh) → reload()
│       ├── Paper (Address Bar Container - lines 183-227)
│       │   └── InputBase (Address Input)
│       │       ├── value={address}
│       │       ├── onChange={setAddress}
│       │       ├── onKeyDown={handleKeyDown}
│       │       ├── onFocus={handleAddressFocus}
│       │       └── onBlur={handleAddressBlur}
│       ├── IconButton (Wallet)
│       ├── IconButton (History)
│       └── IconButton (Settings)
```

### State Management

**Local State (MainBrowserView.tsx)**

```typescript
// Address bar state
const [address, setAddress] = useState('https://metanetapps.com/');
const [isEditingAddress, setIsEditingAddress] = useState(false);
const addressBarRef = useRef<HTMLInputElement>(null);
```

**State Flow Diagram**

```
User Input
    ↓
setAddress(newValue)
    ↓
address state updated
    ↓
InputBase re-renders with new value
```

**Tab Synchronization Logic (lines 45-53)**

```
activeTabId or tabs changes
    ↓
useEffect triggers
    ↓
IF NOT isEditingAddress:
    ↓
    Find active tab
    ↓
    setAddress(activeTab.url)
```

**Editing State Flow**

```
Focus Event → setIsEditingAddress(true) → select all text
    ↓
User types → setAddress(value)
    ↓
EITHER:
    Enter key → handleNavigate() → setIsEditingAddress(false)
    OR
    Escape key → reset to activeTab.url → setIsEditingAddress(false)
    OR
    Blur → reset to activeTab.url → setIsEditingAddress(false)
```

### Event Handlers Inventory

| Handler | Trigger | Purpose | Code Reference |
|---------|---------|---------|----------------|
| `handleNavigate()` | Enter key or manual call | Calls `navigate(address)` from useHodosBrowser, sets `isEditingAddress = false` | Lines 66-69 |
| `handleKeyDown(e)` | Key press in InputBase | Enter → navigate, Escape → cancel editing and reset URL | Lines 71-82 |
| `handleAddressFocus()` | Input gains focus | Sets `isEditingAddress = true`, selects all text after 0ms delay | Lines 84-88 |
| `handleAddressBlur()` | Input loses focus | Sets `isEditingAddress = false`, resets to active tab URL if different | Lines 90-97 |

**Event Handler Logic Details**

```javascript
// handleKeyDown implementation
if (key === 'Enter') {
    handleNavigate(); // Navigate to typed URL
} else if (key === 'Escape') {
    setIsEditingAddress(false);
    // Reset to active tab's current URL (cancel edit)
    setAddress(activeTab.url);
}

// handleAddressBlur implementation
setIsEditingAddress(false);
if (activeTab.url !== address) {
    setAddress(activeTab.url); // Discard uncommitted changes
}
```

### Material-UI Component Mapping

| MUI Component | Props | Purpose | Styling |
|---------------|-------|---------|---------|
| `Toolbar` | `sx={{ minHeight: '54px', ... }}` | Navigation bar container | Fixed height, white background, bottom border |
| `Paper` | `sx={{ flex: 1, borderRadius: 20, ... }}` | Address bar rounded container | Pill-shaped (20px radius), grows to fill space, background changes on hover/focus |
| `InputBase` | `value`, `onChange`, `onKeyDown`, `onFocus`, `onBlur`, `fullWidth` | Text input for URL/search | Unstyled input, 14px font, placeholder styling |
| `IconButton` | `onClick`, `size="small"` | Navigation buttons (back/forward/refresh) | Small size, custom hover colors |

**Styling Characteristics**

```javascript
// Paper (address bar container)
{
    flex: 1,                      // Grows to fill available space
    height: 36,                   // Fixed height
    borderRadius: 20,             // Pill shape
    bgcolor: '#f1f3f4',          // Default gray background
    '&:hover': {
        bgcolor: '#ffffff',       // White on hover
        border: '1px solid rgba(0, 0, 0, 0.1)',
    },
    '&:focus-within': {
        bgcolor: '#ffffff',       // White when focused
        border: '1px solid #1a73e8', // Blue border
        boxShadow: '0 0 0 2px rgba(26, 115, 232, 0.1)', // Blue glow
    },
}
```

### Props and Data Flow

**Hooks Used**

```typescript
// From useHodosBrowser
const { navigate, goBack, goForward, reload } = useHodosBrowser();

// From useTabManager
const {
    tabs,
    activeTabId,
    isLoading,
    createTab,
    closeTab,
    switchToTab,
    // ... other tab methods
} = useTabManager();

// From useKeyboardShortcuts
useKeyboardShortcuts({
    onFocusAddressBar: () => addressBarRef.current?.focus(),
    onReload: reload,
    // ... other shortcuts
});
```

**Data Flow**

1. **User types URL** → `setAddress(value)` → Local state updates → InputBase re-renders
2. **User presses Enter** → `handleNavigate()` → `navigate(address)` → IPC message to C++
3. **Tab switches** → `useEffect` triggers → `setAddress(activeTab.url)` → Display updates
4. **Page loads** → C++ updates tab state → `useTabManager` polls → Tab state updates → `useEffect` syncs address

### Replace vs Keep Analysis (React Layer)

| Component/Module | Current Purpose | Omnibox Needs | Decision | Rationale |
|------------------|-----------------|---------------|----------|-----------|
| **InputBase (lines 183-227)** | Simple text input for URLs | Autocomplete dropdown with suggestions | **REPLACE** | No autocomplete UI, no dropdown, no suggestion rendering. Need new component with dropdown list. |
| **Paper container styling** | Pill-shaped background with hover/focus states | Same visual design | **KEEP** | Visual design can be reused for omnibox container. |
| **address state** | Single string for URL | Input value + suggestion list state | **EXTEND** | Need additional state for suggestions, selected index, dropdown visibility. |
| **isEditingAddress flag** | Binary focus state | Multi-state (idle/typing/selecting) | **EXTEND** | Need more granular state for dropdown interaction. |
| **handleNavigate()** | Navigate to typed URL | Navigate to URL or selected suggestion | **EXTEND** | Same navigation logic, but need to handle suggestion selection. |
| **handleKeyDown()** | Enter/Escape only | Enter/Escape/ArrowUp/ArrowDown/Tab | **EXTEND** | Need arrow key navigation through suggestions. |
| **Tab sync useEffect (lines 45-53)** | Sync address from active tab URL | Same sync behavior | **KEEP** | Works correctly for syncing display URL when not editing. |
| **useHodosBrowser navigation hooks** | navigate(), goBack(), goForward(), reload() | Same navigation methods | **KEEP** | Navigation methods work correctly, no changes needed. |
| **useKeyboardShortcuts integration** | Focus address bar via Ctrl+L | Same keyboard shortcut | **KEEP** | Focus behavior works, just need to focus new omnibox component. |

---

## History Database Architecture

### Database Location

```
Windows: %APPDATA%/HodosBrowser/Default/HodosHistory
macOS: ~/Library/Application Support/HodosBrowser/Default/HodosHistory
```

Database type: SQLite3
Journal mode: WAL (Write-Ahead Logging)
Busy timeout: 5000ms

### Schema Diagram

```sql
┌─────────────────────────────────────────────────────────────────┐
│ urls                                                            │
├─────────────────┬───────────────────┬──────────────────────────┤
│ id              │ INTEGER           │ PRIMARY KEY AUTOINCREMENT │
│ url             │ TEXT              │ NOT NULL UNIQUE           │
│ title           │ TEXT              │                           │
│ visit_count     │ INTEGER           │ DEFAULT 0                 │
│ typed_count     │ INTEGER           │ DEFAULT 0                 │
│ last_visit_time │ INTEGER (int64)   │ NOT NULL (Chromium µs)    │
│ hidden          │ INTEGER (boolean) │ DEFAULT 0                 │
└─────────────────┴───────────────────┴──────────────────────────┘
                            │
                            │ Foreign Key
                            ↓
┌─────────────────────────────────────────────────────────────────┐
│ visits                                                          │
├─────────────────┬───────────────────┬──────────────────────────┤
│ id              │ INTEGER           │ PRIMARY KEY AUTOINCREMENT │
│ url             │ INTEGER           │ NOT NULL (FK → urls.id)  │
│ visit_time      │ INTEGER (int64)   │ NOT NULL (Chromium µs)    │
│ from_visit      │ INTEGER           │ (referrer visit ID)       │
│ transition      │ INTEGER           │ NOT NULL (nav type)       │
│ segment_id      │ INTEGER           │                           │
│ visit_duration  │ INTEGER           │ DEFAULT 0                 │
└─────────────────┴───────────────────┴──────────────────────────┘
```

**Indexes**

```sql
CREATE INDEX idx_urls_url ON urls(url);
CREATE INDEX idx_urls_last_visit_time ON urls(last_visit_time);
CREATE INDEX idx_visits_url ON visits(url);
CREATE INDEX idx_visits_visit_time ON visits(visit_time);
```

**Foreign Key Constraint**

```sql
FOREIGN KEY (url) REFERENCES urls(id) ON DELETE CASCADE
```

When a URL is deleted from `urls`, all corresponding entries in `visits` are automatically deleted.

### C++ API Surface

**Class**: `HistoryManager` (Singleton)
**Header**: `cef-native/include/core/HistoryManager.h`
**Implementation**: `cef-native/src/core/HistoryManager.cpp`

**Initialization**

```cpp
static HistoryManager& GetInstance();
bool Initialize(const std::string& user_data_path);
bool IsInitialized() const;
```

**Core Methods**

```cpp
// Add a page visit (called on successful page load)
bool AddVisit(
    const std::string& url,
    const std::string& title,
    int transition_type = 0
);

// Query history (paginated)
std::vector<HistoryEntry> GetHistory(int limit, int offset);

// Search history with filters
std::vector<HistoryEntry> SearchHistory(const HistorySearchParams& params);

// Get single entry by URL
HistoryEntry GetHistoryEntryByUrl(const std::string& url);

// Delete operations
bool DeleteHistoryEntry(const std::string& url);
bool DeleteAllHistory();
bool DeleteHistoryRange(int64_t start_time, int64_t end_time);
```

**HistoryEntry Structure**

```cpp
struct HistoryEntry {
    int64_t id;                  // URL ID from urls table
    std::string url;             // Full URL
    std::string title;           // Page title
    int visit_count;             // Number of visits to this URL
    int64_t last_visit_time;     // Most recent visit (Chromium µs)
    int64_t visit_time;          // Specific visit time (Chromium µs)
    int transition;              // Transition type (typed, link, etc.)
};
```

**HistorySearchParams Structure**

```cpp
struct HistorySearchParams {
    std::string search_term;  // Search in URL and title (LIKE %term%)
    int64_t start_time;       // Filter visits >= this time
    int64_t end_time;         // Filter visits <= this time
    int limit;                // Max results
    int offset;               // Pagination offset
};
```

**Utility Methods**

```cpp
// Chromium timestamp utilities
static int64_t GetCurrentChromiumTime();
static int64_t ChromiumTimeToUnix(int64_t chromium_time);
static int64_t UnixToChromiumTime(int64_t unix_time);
```

### Chromium Timestamp Format

**Format**: Microseconds since January 1, 1601 UTC (Windows epoch)

**Conversion Formulas**

```cpp
// Current time
int64_t now = std::chrono::system_clock::now();
int64_t chromium_time = unix_microseconds + (11644473600LL * 1000000LL);

// Chromium → Unix seconds
int64_t unix_seconds = (chromium_time / 1000000) - 11644473600LL;

// Unix seconds → Chromium
int64_t chromium_time = (unix_seconds + 11644473600LL) * 1000000LL;
```

**Epoch Difference**: 11,644,473,600 seconds between Unix epoch (1970-01-01) and Windows epoch (1601-01-01)

### SQL Query Examples for Autocomplete

**Query 1: Recent History (Most Recent First)**

```sql
SELECT u.id, u.url, u.title, u.visit_count, u.last_visit_time, v.visit_time, v.transition
FROM urls u
INNER JOIN visits v ON u.id = v.url
WHERE u.hidden = 0
ORDER BY v.visit_time DESC
LIMIT 10 OFFSET 0;
```

**Query 2: Search by Prefix (For Autocomplete)**

```sql
SELECT u.id, u.url, u.title, u.visit_count, u.last_visit_time
FROM urls u
WHERE u.hidden = 0
  AND (u.url LIKE 'https://github%' OR u.title LIKE 'github%')
ORDER BY u.visit_count DESC, u.last_visit_time DESC
LIMIT 5;
```

**Query 3: Frecency-Based Ranking (Frequency + Recency)**

```sql
-- Combine visit count (frequency) and recency for better suggestions
SELECT u.id, u.url, u.title, u.visit_count, u.last_visit_time,
       (u.visit_count * 0.3 + (u.last_visit_time / 1000000000000.0)) AS frecency_score
FROM urls u
WHERE u.hidden = 0
  AND (u.url LIKE ? OR u.title LIKE ?)
ORDER BY frecency_score DESC
LIMIT 10;
```

**Query 4: Typed URLs Only (Higher Priority)**

```sql
-- typed_count tracks URLs user explicitly typed (vs clicked links)
SELECT u.id, u.url, u.title, u.visit_count, u.typed_count, u.last_visit_time
FROM urls u
WHERE u.hidden = 0
  AND u.typed_count > 0
  AND (u.url LIKE ? OR u.title LIKE ?)
ORDER BY u.typed_count DESC, u.visit_count DESC
LIMIT 5;
```

### Access Pattern for Omnibox Autocomplete

**Autocomplete Query Flow**

```
User types "gith" in omnibox
    ↓
Frontend debounces input (300ms)
    ↓
Send IPC message to C++: "autocomplete_query", ["gith"]
    ↓
C++ calls HistoryManager::SearchHistory()
    ↓
SQL Query:
    WHERE (url LIKE '%gith%' OR title LIKE '%gith%')
    ORDER BY frecency (visit_count + recency)
    LIMIT 10
    ↓
Return HistoryEntry[] to frontend
    ↓
Frontend renders dropdown with suggestions
```

**Ranking Strategy for Omnibox**

1. **Typed URLs** (typed_count > 0): Highest priority - user explicitly typed this before
2. **Frecency**: Combine visit_count (frequency) with last_visit_time (recency)
3. **Prefix Match**: URLs starting with input rank higher than substring matches
4. **Visit Count**: More visited URLs rank higher

**Proposed Autocomplete API Method**

```cpp
// New method to add to HistoryManager for omnibox
std::vector<HistoryEntry> GetAutocompletesSuggestions(
    const std::string& input,
    int max_results = 10
) {
    // Algorithm:
    // 1. Split input into terms
    // 2. Search URLs and titles (prefix match + substring match)
    // 3. Rank by: typed_count > prefix_match > frecency > visit_count
    // 4. Return top N results
}
```

### Database Write Pattern (AddVisit)

**Flow**

```
Page loads successfully → OnLoadEnd() triggered
    ↓
SimpleHandler calls HistoryManager::AddVisit(url, title)
    ↓
SQL Transaction:
    ├─ CHECK: Does URL exist in urls table?
    │   ├─ YES: UPDATE urls SET visit_count++, last_visit_time=now, title=new_title
    │   └─ NO:  INSERT INTO urls (url, title, visit_count=1, last_visit_time=now)
    ├─ Get url_id (from existing row or last_insert_rowid)
    └─ INSERT INTO visits (url=url_id, visit_time=now, transition=type)
    ↓
Return success/failure
```

**Key Insight**: Each page load creates ONE row in `visits` table and either creates or updates ONE row in `urls` table.

### Replace vs Keep Analysis (Database Layer)

| Component | Current Purpose | Omnibox Needs | Decision | Rationale |
|-----------|-----------------|---------------|----------|-----------|
| **urls table schema** | Store unique URLs with visit metadata | Same data for autocomplete | **KEEP** | Schema is ideal for autocomplete: url, title, visit_count, last_visit_time all needed. |
| **visits table schema** | Store individual visit records | Not needed for autocomplete | **KEEP** | Useful for history page, doesn't interfere with autocomplete queries. |
| **Indexes** | Optimize queries on url, visit_time | Same indexes useful | **KEEP** | `idx_urls_url` helps autocomplete prefix search, `idx_urls_last_visit_time` helps recency ranking. |
| **HistoryManager::AddVisit()** | Record page visits | Same write pattern | **KEEP** | Existing write logic is correct, no changes needed. |
| **HistoryManager::GetHistory()** | Paginated history for history page | Not used by omnibox | **KEEP** | Used by history page, doesn't conflict with omnibox. |
| **HistoryManager::SearchHistory()** | Generic search with filters | Good foundation for autocomplete | **EXTEND** | Can be used as-is or basis for autocomplete-specific query. |
| **Chromium timestamp format** | Store timestamps as int64 microseconds | Same format | **KEEP** | Standard format, works correctly. |
| **Singleton GetInstance()** | Single HistoryManager instance | Same access pattern | **KEEP** | Singleton is appropriate for database manager. |

**Recommendation**: Add new method `GetAutocompleteSuggestions()` to HistoryManager rather than modifying existing methods.

---

## Data Flow Diagram

### End-to-End Navigation Flow

```
┌─────────────────────────────────────────────────────────────────────────┐
│ REACT LAYER (localhost:5137)                                            │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                           │
│  User types "https://example.com" in InputBase                          │
│         ↓                                                                │
│  onChange → setAddress("https://example.com")                           │
│         ↓                                                                │
│  User presses Enter                                                      │
│         ↓                                                                │
│  handleKeyDown → handleNavigate()                                       │
│         ↓                                                                │
│  navigate(address) [from useHodosBrowser hook]                          │
│         ↓                                                                │
│  window.hodosBrowser.navigation.navigate("https://example.com")         │
│         ↓                                                                │
│  window.cefMessage.send('navigate', ["https://example.com"])            │
│                                                                           │
└─────────────────────────────────────────────────────────────────────────┘
                                   ↓ IPC Message
┌─────────────────────────────────────────────────────────────────────────┐
│ C++ CEF LAYER (Render Process → Browser Process)                        │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                           │
│  CefMessageSendHandler (render process)                                 │
│         ↓                                                                │
│  Create CefProcessMessage("navigate")                                   │
│         ↓                                                                │
│  args.SetString(0, "https://example.com")                               │
│         ↓                                                                │
│  browser->SendProcessMessage(PID_BROWSER, message)                      │
│                                                                           │
└─────────────────────────────────────────────────────────────────────────┘
                                   ↓
┌─────────────────────────────────────────────────────────────────────────┐
│ C++ CEF LAYER (Browser Process)                                         │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                           │
│  SimpleHandler::OnProcessMessageReceived()                              │
│         ↓                                                                │
│  if (message_name == "navigate")                                        │
│         ↓                                                                │
│  Extract URL from args                                                   │
│         ↓                                                                │
│  Normalize protocol (add https:// if missing)                           │
│         ↓                                                                │
│  Tab* active_tab = TabManager::GetInstance().GetActiveTab()             │
│         ↓                                                                │
│  active_tab->browser->GetMainFrame()->LoadURL(url)                      │
│                                                                           │
└─────────────────────────────────────────────────────────────────────────┘
                                   ↓
┌─────────────────────────────────────────────────────────────────────────┐
│ CEF CHROMIUM ENGINE                                                      │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                           │
│  Start navigation to "https://example.com"                              │
│         ↓                                                                │
│  OnLoadStart callback → Update tab loading state                        │
│         ↓                                                                │
│  HTTP request, render page                                               │
│         ↓                                                                │
│  OnLoadEnd callback → Page loaded successfully                          │
│                                                                           │
└─────────────────────────────────────────────────────────────────────────┘
                                   ↓
┌─────────────────────────────────────────────────────────────────────────┐
│ C++ CEF LAYER (OnLoadEnd)                                               │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                           │
│  SimpleHandler::OnLoadEnd(browser, frame, httpStatusCode)               │
│         ↓                                                                │
│  Get URL and title from frame                                            │
│         ↓                                                                │
│  HistoryManager::GetInstance().AddVisit(url, title, transition)         │
│                                                                           │
└─────────────────────────────────────────────────────────────────────────┘
                                   ↓
┌─────────────────────────────────────────────────────────────────────────┐
│ SQLITE DATABASE (%APPDATA%/HodosBrowser/Default/HodosHistory)           │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                           │
│  HistoryManager::AddVisit()                                             │
│         ↓                                                                │
│  BEGIN TRANSACTION                                                       │
│         ↓                                                                │
│  SELECT id FROM urls WHERE url = "https://example.com"                  │
│         ↓                                                                │
│  IF NOT EXISTS:                                                          │
│    INSERT INTO urls (url, title, visit_count=1, last_visit_time=now)   │
│  ELSE:                                                                   │
│    UPDATE urls SET visit_count++, last_visit_time=now, title=title     │
│         ↓                                                                │
│  INSERT INTO visits (url=url_id, visit_time=now, transition=0)         │
│         ↓                                                                │
│  COMMIT                                                                  │
│                                                                           │
└─────────────────────────────────────────────────────────────────────────┘
```

### Tab State Synchronization Flow

```
┌─────────────────────────────────────────────────────────────────────────┐
│ C++ TabManager (Browser Process)                                        │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                           │
│  Tab state changes (URL updates, title updates, loading state)          │
│         ↓                                                                │
│  SimpleHandler::NotifyTabListChanged()                                  │
│         ↓                                                                │
│  Build JSON: { activeTabId, tabs: [{id, url, title, isActive, ...}] }  │
│         ↓                                                                │
│  Create CefProcessMessage("tab_list_response")                          │
│         ↓                                                                │
│  header_browser->SendProcessMessage(PID_RENDERER, message)              │
│                                                                           │
└─────────────────────────────────────────────────────────────────────────┘
                                   ↓
┌─────────────────────────────────────────────────────────────────────────┐
│ REACT LAYER (Header Browser)                                            │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                           │
│  window.addEventListener('message', handleTabListResponse)               │
│         ↓                                                                │
│  if (event.data.type === 'tab_list_response')                           │
│         ↓                                                                │
│  Parse JSON to TabListResponse                                           │
│         ↓                                                                │
│  setState({ tabs, activeTabId, isLoading: false })                      │
│         ↓                                                                │
│  useEffect (lines 45-53) detects activeTabId change                     │
│         ↓                                                                │
│  if (!isEditingAddress) setAddress(activeTab.url)                       │
│         ↓                                                                │
│  InputBase re-renders with new URL                                      │
│                                                                           │
└─────────────────────────────────────────────────────────────────────────┘
```

**Polling**: React calls `refreshTabList()` every 2 seconds to poll for tab updates (lines 110-111 in useTabManager.ts).

### Back/Forward/Reload Flow

```
User clicks Back button
    ↓
goBack() from useHodosBrowser
    ↓
window.cefMessage.send('navigate_back', [])
    ↓
SimpleHandler::OnProcessMessageReceived("navigate_back")
    ↓
Tab* active_tab = TabManager::GetInstance().GetActiveTab()
    ↓
active_tab->browser->GoBack()
    ↓
CEF navigates to previous page in history
    ↓
OnLoadEnd → HistoryManager::AddVisit() (records revisit)
```

Same pattern for `navigate_forward` and `navigate_reload`.

---

## IPC Protocol

### Message Transport Mechanism

**Render Process → Browser Process**

```cpp
// In render process (simple_render_process_handler.cpp)
CefRefPtr<CefProcessMessage> message = CefProcessMessage::Create(message_name);
CefRefPtr<CefListValue> args = message->GetArgumentList();
// ... populate args ...
browser->SendProcessMessage(PID_BROWSER, message);
```

**Browser Process Handler**

```cpp
// In browser process (simple_handler.cpp)
bool SimpleHandler::OnProcessMessageReceived(
    CefRefPtr<CefBrowser> browser,
    CefRefPtr<CefFrame> frame,
    CefProcessId source_process,
    CefRefPtr<CefProcessMessage> message
) {
    std::string message_name = message->GetName();
    CefRefPtr<CefListValue> args = message->GetArgumentList();
    // ... handle message ...
}
```

### Navigation IPC Messages

| Message Name | Arguments | Handler Location | Action |
|--------------|-----------|------------------|--------|
| `navigate` | `[url: string]` | `SimpleHandler::OnProcessMessageReceived` (line 850) | `TabManager::GetActiveTab()->browser->LoadURL(url)` |
| `navigate_back` | `[]` | `SimpleHandler::OnProcessMessageReceived` (line 871) | `TabManager::GetActiveTab()->browser->GoBack()` |
| `navigate_forward` | `[]` | `SimpleHandler::OnProcessMessageReceived` (line 884) | `TabManager::GetActiveTab()->browser->GoForward()` |
| `navigate_reload` | `[]` | `SimpleHandler::OnProcessMessageReceived` (line 897) | `TabManager::GetActiveTab()->browser->Reload()` |

### Tab Management IPC Messages

| Message Name | Arguments | Handler Location | Action |
|--------------|-----------|------------------|--------|
| `get_tab_list` | `[]` | `SimpleHandler::OnProcessMessageReceived` | Calls `NotifyTabListChanged()` to send current tab list |
| `tab_create` | `[url: string]` | `SimpleHandler::OnProcessMessageReceived` | `TabManager::CreateTab(url)` |
| `tab_close` | `[tabId: int]` | `SimpleHandler::OnProcessMessageReceived` | `TabManager::CloseTab(tabId)` |
| `tab_switch` | `[tabId: int]` | `SimpleHandler::OnProcessMessageReceived` | `TabManager::SwitchToTab(tabId)` |

### Response Messages (Browser → Render)

| Message Name | Direction | Payload | Purpose |
|--------------|-----------|---------|---------|
| `tab_list_response` | Browser → Render | `{ activeTabId: int, tabs: [{id, url, title, isActive, isLoading, favicon}] }` | Update React state with current tab list |

**Response Pattern**

```cpp
// Browser process sends response
CefRefPtr<CefProcessMessage> response = CefProcessMessage::Create("tab_list_response");
CefRefPtr<CefListValue> response_args = response->GetArgumentList();
response_args->SetString(0, json_string);
header_browser->GetMainFrame()->SendProcessMessage(PID_RENDERER, response);
```

```typescript
// React receives response
window.addEventListener('message', (event) => {
    if (event.data.type === 'tab_list_response') {
        const data = JSON.parse(event.data.data);
        setState({ tabs: data.tabs, activeTabId: data.activeTabId });
    }
});
```

### JavaScript Bridge API

**Injected in Render Process** (simple_render_process_handler.cpp, OnContextCreated)

```javascript
// window.cefMessage object
window.cefMessage = {
    send: function(message_name, ...args) {
        // Creates CefProcessMessage and sends to browser process
    }
};

// window.hodosBrowser object
window.hodosBrowser = {
    navigation: {
        navigate: function(url) {
            window.cefMessage.send('navigate', [url]);
        }
    },
    overlay: {
        show: function() { ... },
        close: function() { ... },
        toggleInput: function(enable) { ... }
    },
    // ... other APIs ...
};
```

**React Usage**

```typescript
// Defined in initWindowBridge.ts
window.hodosBrowser.navigation.navigate(url);

// Used in useHodosBrowser.ts
const navigate = useCallback((path: string): void => {
    window.hodosBrowser.navigation.navigate(path);
}, []);
```

### Proposed Omnibox IPC Messages

**New messages needed for autocomplete**

| Message Name | Arguments | Purpose |
|--------------|-----------|---------|
| `autocomplete_query` | `[input: string]` | Request autocomplete suggestions for user input |
| `autocomplete_response` | `[suggestions: HistoryEntry[]]` | Return autocomplete suggestions to render process |

**Implementation Plan**

1. **React sends query**:
   ```typescript
   window.cefMessage.send('autocomplete_query', [inputValue]);
   ```

2. **C++ handles in OnProcessMessageReceived**:
   ```cpp
   if (message_name == "autocomplete_query") {
       std::string input = args->GetString(0);
       auto suggestions = HistoryManager::GetInstance()
           .GetAutocompleteSuggestions(input, 10);
       // Build JSON array of suggestions
       // Send autocomplete_response back to render process
   }
   ```

3. **React receives response**:
   ```typescript
   window.addEventListener('message', (event) => {
       if (event.data.type === 'autocomplete_response') {
           setSuggestions(JSON.parse(event.data.data));
       }
   });
   ```

---

## Replace vs Reuse Decision Matrix

### Comprehensive Component Analysis

| Component/Module | Location | Current Purpose | Omnibox Needs | Decision | Rationale | Action Required |
|------------------|----------|-----------------|---------------|----------|-----------|-----------------|
| **MainBrowserView InputBase** | `frontend/src/pages/MainBrowserView.tsx` lines 183-227 | Simple URL text input | Autocomplete dropdown with suggestions | **REPLACE** | No autocomplete UI, no dropdown rendering, no suggestion list management. Need entirely new omnibox component. | Create new `<Omnibox>` component with dropdown, suggestion rendering, keyboard navigation. |
| **Paper container styling** | `MainBrowserView.tsx` lines 183-204 | Pill-shaped address bar background | Same visual container | **KEEP** | Styling is reusable: borderRadius 20px, hover/focus states, flex layout. | Copy Paper `sx` props to new Omnibox component wrapper. |
| **address state** | `MainBrowserView.tsx` line 24 | Single string for current URL | Input value + suggestion list + selected index | **EXTEND** | Need additional state: `suggestions: HistoryEntry[]`, `selectedIndex: number`, `showDropdown: boolean`. | Add new state variables in Omnibox component. |
| **isEditingAddress flag** | `MainBrowserView.tsx` line 25 | Binary editing state | Multi-state dropdown interaction | **EXTEND** | Need states: idle, typing (show suggestions), navigating dropdown (arrow keys), selected. | Replace boolean with enum or multiple flags. |
| **handleNavigate()** | `MainBrowserView.tsx` lines 66-69 | Navigate to typed URL | Navigate to URL or selected suggestion | **EXTEND** | Same `navigate(url)` call, but need to handle: (1) typed URL, (2) selected suggestion URL, (3) Enter on suggestion vs Enter on input. | Add logic to determine final URL before calling `navigate()`. |
| **handleKeyDown()** | `MainBrowserView.tsx` lines 71-82 | Enter/Escape only | Enter/Escape/ArrowUp/ArrowDown/Tab | **EXTEND** | Need arrow key handling: ArrowDown (move down list), ArrowUp (move up list), Enter (select current), Tab (complete to selected). | Add switch cases for arrow keys, manage `selectedIndex`. |
| **handleAddressFocus()** | `MainBrowserView.tsx` lines 84-88 | Select all on focus | Same + maybe show suggestions | **EXTEND** | Same select-all behavior, optionally show suggestions on focus. | Add suggestion query on focus if desired. |
| **handleAddressBlur()** | `MainBrowserView.tsx` lines 90-97 | Reset to tab URL on blur | Close dropdown + reset | **EXTEND** | Need to close dropdown, reset suggestion state, AND reset to tab URL if not navigated. | Add dropdown hide logic before existing reset. |
| **Tab sync useEffect** | `MainBrowserView.tsx` lines 45-53 | Sync address from active tab | Same sync when not editing | **KEEP** | Logic is correct: update address from tab URL when not editing. No changes needed. | No changes, works as-is. |
| **useHodosBrowser navigation** | `frontend/src/hooks/useHodosBrowser.ts` | navigate(), goBack(), goForward(), reload() | Same navigation methods | **KEEP** | Navigation API is correct and sufficient. Omnibox just calls `navigate(url)`. | No changes needed. |
| **useKeyboardShortcuts** | `frontend/src/hooks/useKeyboardShortcuts.ts` | Global shortcuts (Ctrl+L for focus) | Same Ctrl+L behavior | **KEEP** | Shortcut works, just need to focus new omnibox input ref instead of old addressBarRef. | Update `onFocusAddressBar` to reference new omnibox ref. |
| **useTabManager** | `frontend/src/hooks/useTabManager.ts` | Tab state management | Same tab state | **KEEP** | Tab state works correctly, omnibox consumes tab URL like current address bar. | No changes needed. |
| **HistoryManager C++ class** | `cef-native/src/core/HistoryManager.cpp` | Database singleton for history | Autocomplete query source | **KEEP + EXTEND** | Existing class is solid. Add new `GetAutocompleteSuggestions()` method for optimized autocomplete queries. | Add 1 new method, keep all existing methods. |
| **SQLite schema (urls table)** | `HistoryManager.cpp` lines 58-67 | Store URLs with visit metadata | Autocomplete data source | **KEEP** | Schema is perfect for autocomplete: `url`, `title`, `visit_count`, `typed_count`, `last_visit_time` all useful. | No schema changes needed. |
| **SQLite schema (visits table)** | `HistoryManager.cpp` lines 69-78 | Individual visit records | Not directly used by autocomplete | **KEEP** | Table is useful for history page, doesn't interfere with omnibox. Keep for existing functionality. | No changes needed. |
| **Indexes** | `HistoryManager.cpp` lines 80-83 | Optimize history queries | Autocomplete query performance | **KEEP** | `idx_urls_url` helps prefix matching, `idx_urls_last_visit_time` helps recency sorting. Perfect for autocomplete. | No changes needed. |
| **HistoryManager::AddVisit()** | `HistoryManager.cpp` lines 119-221 | Record page visits | Same write pattern | **KEEP** | Existing write logic is correct. Autocomplete reads existing data, doesn't change write pattern. | No changes needed. |
| **HistoryManager::SearchHistory()** | `HistoryManager.cpp` lines 290-376 | Generic search with filters | Good basis for autocomplete | **REUSE** | Can be used as-is or adapted. Already does LIKE matching on url and title, has limit/offset. | Use directly or copy logic into new method. |
| **Chromium timestamp format** | `HistoryManager.cpp` lines 558-578 | int64 microseconds since 1601 | Same timestamp format | **KEEP** | Standard format, works correctly for all queries. | No changes needed. |
| **IPC navigate message** | `simple_handler.cpp` line 850 | Navigate to URL | Same navigation | **KEEP** | Message works for omnibox: send URL, C++ navigates. | No changes needed. |
| **IPC protocol** | `simple_handler.cpp` OnProcessMessageReceived | Message routing | Add autocomplete message | **EXTEND** | Need new message: `autocomplete_query` (input → C++) and `autocomplete_response` (suggestions → React). | Add 2 new message handlers. |
| **window.cefMessage.send()** | `initWindowBridge.ts` lines 8-9 | Send IPC messages | Same IPC transport | **KEEP** | Transport mechanism works. Just send new `autocomplete_query` message. | No changes to bridge itself. |
| **window.hodosBrowser.navigation** | `initWindowBridge.ts` lines 5-14 | Navigation API wrapper | Same navigation API | **KEEP** | API works correctly. Omnibox uses same `navigate(url)` method. | No changes needed. |

### Summary of Decisions

**REPLACE (1 item)**
- MainBrowserView InputBase (lines 183-227) → New `<Omnibox>` component with autocomplete dropdown

**KEEP (12 items)**
- Paper container styling (reuse visual design)
- Tab sync useEffect (correct logic, no changes)
- useHodosBrowser navigation hooks (correct API)
- useKeyboardShortcuts (update focus reference only)
- useTabManager (no changes needed)
- HistoryManager C++ class structure (solid foundation)
- SQLite urls table schema (perfect for autocomplete)
- SQLite visits table schema (keep for history page)
- Indexes (optimize autocomplete queries)
- HistoryManager::AddVisit() (correct write logic)
- Chromium timestamp format (standard format)
- IPC navigate message (works for omnibox)
- window.cefMessage.send() (transport works)
- window.hodosBrowser.navigation (API works)

**EXTEND (7 items)**
- address state → Add suggestions[], selectedIndex, showDropdown
- isEditingAddress → Add dropdown interaction states
- handleNavigate() → Handle selected suggestion URL
- handleKeyDown() → Add arrow key navigation
- handleAddressFocus() → Optionally show suggestions
- handleAddressBlur() → Close dropdown before reset
- IPC protocol → Add autocomplete_query and autocomplete_response messages
- HistoryManager → Add GetAutocompleteSuggestions() method

### Key Insights for Phase 2+

1. **Database is ready**: SQLite schema is already ideal for autocomplete. No schema changes needed.

2. **HistoryManager needs 1 new method**: Add `GetAutocompleteSuggestions(input, max_results)` optimized for autocomplete ranking (typed_count > prefix_match > frecency).

3. **IPC needs 2 new messages**: `autocomplete_query` (React → C++) and `autocomplete_response` (C++ → React).

4. **React needs new component**: Replace simple InputBase with full `<Omnibox>` component that handles:
   - Suggestion dropdown rendering
   - Arrow key navigation
   - Selected index state
   - Click/Enter on suggestion
   - Debounced autocomplete queries

5. **Visual design is reusable**: Keep Paper container styling (pill shape, hover/focus states).

6. **Navigation flow unchanged**: Omnibox still calls `navigate(url)`, just needs to pick correct URL (typed vs selected suggestion).

7. **Tab synchronization works**: Existing `useEffect` syncs omnibox display with active tab URL.

8. **Keyboard shortcuts work**: Ctrl+L just needs to focus new omnibox ref instead of old addressBarRef.

---

## Conclusion

This investigation provides a complete map of the existing address bar and history architecture. The **Replace vs Reuse Decision Matrix** gives clear guidance for implementing the omnibox:

- **Replace**: Only the simple InputBase component (need autocomplete UI)
- **Keep**: Database schema, HistoryManager structure, navigation API, IPC transport
- **Extend**: State management (add suggestion state), event handlers (add arrow keys), IPC protocol (add autocomplete messages), HistoryManager (add autocomplete method)

**Next step**: Phase 2 will design and implement the core Omnibox React component with autocomplete dropdown based on this foundation.
