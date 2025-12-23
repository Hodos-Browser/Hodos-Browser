# Hodos Browser Feature Research Documentation (REVISED)

## Critical Architectural Decision

**All browser features (History, Bookmarks, Cookies, Favorites) are implemented in the CEF C++ layer ONLY.**

**The Rust wallet backend remains exclusively for BRC-100 wallet operations.**

## Revised Architecture Summary

### Layer Separation

```
┌─────────────────────────────────────────────────────┐
│  Frontend (React/TypeScript)                        │
│  - UI Components                                    │
│  - React Hooks                                      │
│  - Direct native function calls via V8              │
└────────────────┬────────────────────────────────────┘
                 │ window.hodosBrowser.* (V8 bindings)
                 ▼
┌─────────────────────────────────────────────────────┐
│  CEF C++ Layer                                      │
│  ├─ HistoryManager (accesses CEF's History DB)     │
│  ├─ BookmarkManager (custom SQLite DB)             │
│  ├─ CookieManager (wraps CefCookieManager)         │
│  └─ V8 JavaScript bindings                         │
└────────────────┬────────────────────────────────────┘
                 │ Direct SQLite access
                 ▼
┌─────────────────────────────────────────────────────┐
│  Databases (in CEF user_data_path)                 │
│  ├─ History (CEF built-in, auto-managed)           │
│  ├─ Cookies (CEF built-in, auto-managed)           │
│  ├─ Bookmarks (custom, CEF C++ managed)            │
│  └─ HistoryMetadata (optional, CEF C++ managed)    │
└─────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────┐
│  Rust Wallet Backend (SEPARATE)                    │
│  - BRC-100 wallet operations ONLY                  │
│  - No browser feature involvement                  │
│  - HTTP API on port 3301 for wallet only           │
└─────────────────────────────────────────────────────┘
```

## Documentation Files

### 1. HISTORY_FEATURE_REVISED.md

**CEF's Built-in History Database**

- ✅ CEF automatically creates and populates `History` SQLite database
- ✅ Access directly from C++ using SQLite API
- ✅ No manual navigation tracking needed
- ✅ Standard Chromium schema (well-documented)
- ❌ No Rust backend involvement

**Implementation:**
- `HistoryManager` C++ class accesses CEF's History database
- Direct SQLite queries for retrieving/searching history
- Optional metadata database for custom features (tags, categories)
- V8 bindings expose functions to JavaScript
- React components call native functions synchronously

**Key Files:**
- `cef-native/include/core/HistoryManager.h`
- `cef-native/src/core/HistoryManager.cpp`
- `frontend/src/hooks/useHistory.ts`
- `frontend/src/components/HistoryPanel.tsx`

**Databases:**
- `{user_data_path}/History` - CEF's built-in (read/write)
- `{user_data_path}/HistoryMetadata` - Optional custom (read/write)

### 2. BOOKMARKS_FEATURE_REVISED.md

**Custom Bookmark System**

- ✅ CEF does NOT provide bookmarks - must build from scratch
- ✅ Custom SQLite database managed in CEF C++ layer
- ✅ Full control over schema and features
- ❌ No Rust backend involvement

**Implementation:**
- `BookmarkManager` C++ class handles all bookmark operations
- SQLite database with hierarchical folder support
- Tag system for organization
- V8 bindings expose CRUD operations
- React components with search and organization UI

**Key Files:**
- `cef-native/include/core/BookmarkManager.h`
- `cef-native/src/core/BookmarkManager.cpp`
- `frontend/src/hooks/useBookmarks.ts`
- `frontend/src/components/BookmarksPanel.tsx`

**Database:**
- `{user_data_path}/Bookmarks` - Custom SQLite (read/write)

### 3. FAVORITES_COOKIES_REVISED.md

**Part 1: Favorites (Speed Dial)**

- ✅ Extension of BookmarkManager with `is_favorite` flag
- ✅ Thumbnail storage in bookmark database
- ✅ Smart suggestions based on visit patterns
- ❌ No Rust backend involvement

**Implementation:**
- Extend existing `BookmarkManager` class
- Add favorites-specific methods
- Thumbnail capture/storage
- V8 bindings for favorites operations
- Visual grid component in React

**Part 2: Cookies Management**

- ✅ CEF automatically manages cookies via `CefCookieManager`
- ✅ Wrap CEF's cookie API in C++ layer
- ✅ Optional preferences database for user settings
- ❌ No Rust backend involvement

**Implementation:**
- `HodosCookieManager` wraps CEF's `CefCookieManager`
- `CookieVisitor` for enumerating cookies
- Separate preferences database for per-domain settings
- V8 bindings expose cookie operations
- Cookie manager UI with domain grouping

**Key Files:**
- `cef-native/include/core/BookmarkManager.h` (extended)
- `cef-native/include/core/CookieManager.h`
- `cef-native/src/core/CookieManager.cpp`
- `frontend/src/components/FavoritesGrid.tsx`
- `frontend/src/components/CookieManager.tsx`

**Databases:**
- `{user_data_path}/Cookies` - CEF built-in (via CefCookieManager API)
- `{user_data_path}/CookiePreferences` - Custom preferences (read/write)

## Technology Stack

### CEF C++ Layer (Browser Features)

**Language:** C++17
**Database:** SQLite3 (direct API)
**JSON:** nlohmann/json (optional for data interchange)
**Build:** CMake

**Key Libraries:**
- CEF (Chromium Embedded Framework)
- SQLite3
- Standard C++ Library

### Frontend (UI)

**Language:** TypeScript
**Framework:** React 19.1.0
**Build:** Vite 6.3.5
**UI:** Material-UI 7.1.1

### Rust Backend (Wallet ONLY)

**Language:** Rust
**Framework:** Actix-web 4.11.0
**Database:** SQLite (rusqlite)
**Purpose:** BRC-100 wallet operations exclusively

## Implementation Pattern

### Standard Flow for Browser Features

```
User Action (React Component)
  ↓
window.hodosBrowser.feature.method()
  ↓
V8 JavaScript Binding (CEF Render Process)
  ↓
Feature Manager C++ Class (CEF Browser Process)
  ↓
SQLite Database (Direct Access)
  ↓
Return Value Through V8
  ↓
React Component Updates
```

**No HTTP requests involved** - All browser features use direct native calls.

### CEF C++ Class Pattern

```cpp
class FeatureManager {
public:
    static FeatureManager& GetInstance();  // Singleton

    bool Initialize(const std::string& user_data_path);

    // Feature operations
    std::vector<Data> GetData(...);
    bool AddData(...);
    bool UpdateData(...);
    bool DeleteData(...);

private:
    FeatureManager() = default;
    ~FeatureManager();

    sqlite3* db_;
    std::string db_path_;

    bool OpenDatabase();
    void CloseDatabase();
};
```

### V8 Binding Pattern

```cpp
class FeatureV8Handler : public CefV8Handler {
public:
    bool Execute(const CefString& name,
                 CefRefPtr<CefV8Value> object,
                 const CefV8ValueList& arguments,
                 CefRefPtr<CefV8Value>& retval,
                 CefString& exception) override {

        auto& manager = FeatureManager::GetInstance();

        if (name == "methodName") {
            // Extract arguments
            // Call C++ manager method
            // Convert result to V8Value
            // Return to JavaScript
            return true;
        }

        return false;
    }

    IMPLEMENT_REFCOUNTING(FeatureV8Handler);
};
```

### React Hook Pattern

```typescript
export function useFeature() {
  const [data, setData] = useState<Data[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const loadData = useCallback(() => {
    setLoading(true);
    try {
      // Direct synchronous native call
      const result = window.hodosBrowser.feature.getData();
      setData(result);
    } catch (err) {
      setError('Failed to load data');
    } finally {
      setLoading(false);
    }
  }, []);

  return { data, loading, error, loadData, ... };
}
```

## Database Locations

All databases stored in CEF user data directory:

```
%APPDATA%/HodosBrowser/
├── History                 # CEF built-in (auto-managed)
├── Cookies                 # CEF built-in (auto-managed)
├── Bookmarks              # Custom (CEF C++ managed)
├── HistoryMetadata        # Optional custom (CEF C++ managed)
├── CookiePreferences      # Optional custom (CEF C++ managed)
└── wallet/                # Rust backend (separate)
    └── wallet.db
```

## CEF Built-in vs Custom Databases

### CEF Built-in (Auto-Managed)

**History:**
- Automatically created and populated
- Standard Chromium schema
- Read/write access via SQLite
- Updated on every navigation

**Cookies:**
- Automatically managed
- Access via CefCookieManager API
- Can also access directly via SQLite
- Standard cookie behavior

### Custom (CEF C++ Managed)

**Bookmarks:**
- We create and manage entirely
- Custom schema with folders/tags
- Full control over features

**Metadata Databases:**
- Optional extensions
- Store user preferences
- Link to CEF data via URLs/IDs

## Key Advantages of Revised Architecture

### Performance
- **No HTTP overhead**: Direct native function calls (nanoseconds)
- **No serialization**: Data passed directly through V8
- **Fast SQLite**: Optimized local database access
- **No network latency**: All operations local

### Architecture
- **Clean separation**: Browser features in browser layer, wallet in wallet layer
- **Single responsibility**: Each component has clear purpose
- **CEF native**: Leverages built-in Chromium capabilities
- **No duplication**: Use CEF's existing infrastructure

### Maintainability
- **Standard patterns**: Well-documented CEF approaches
- **Type safety**: C++ and TypeScript strong typing
- **Clear boundaries**: No mixing of concerns
- **Easier testing**: Each layer independently testable

### Security
- **Direct access**: No HTTP API surface for browser features
- **CEF security**: Leverage Chromium's security model
- **Process isolation**: Proper sandboxing
- **Wallet isolation**: Wallet backend completely separate

## Implementation Priority

### Phase 1: History (Highest Priority)
**Why:** CEF already provides it, easiest to implement
1. Create HistoryManager C++ class
2. Access CEF's History database
3. Add V8 bindings
4. Build React UI
**Estimated Effort:** Low (leverages existing database)

### Phase 2: Bookmarks
**Why:** Foundation for Favorites
1. Create BookmarkManager C++ class
2. Design and create Bookmarks database
3. Implement CRUD operations
4. Add V8 bindings
5. Build React UI with folders/tags
**Estimated Effort:** Medium (custom database, more features)

### Phase 3: Favorites
**Why:** Extends Bookmarks
1. Extend BookmarkManager
2. Add favorites methods
3. Extend V8 bindings
4. Build visual grid UI
5. Implement thumbnail capture (optional)
**Estimated Effort:** Low (builds on Bookmarks)

### Phase 4: Cookies
**Why:** CEF provides API, mainly UI work
1. Create HodosCookieManager wrapper
2. Implement CookieVisitor
3. Add preferences database
4. Add V8 bindings (handle async)
5. Build cookie manager UI
**Estimated Effort:** Medium (async handling, UI complexity)

## Development Workflow

### 1. CEF C++ Layer
```bash
# Create manager class
cef-native/include/core/FeatureManager.h
cef-native/src/core/FeatureManager.cpp

# Extend V8 handler
cef-native/src/handlers/simple_render_process_handler.cpp

# Build
cd cef-native
cmake --build build --config Release
```

### 2. Frontend
```bash
# Create types
frontend/src/types/feature.d.ts

# Create hook
frontend/src/hooks/useFeature.ts

# Create component
frontend/src/components/FeaturePanel.tsx

# Build
cd frontend
npm run build
```

### 3. Testing
```bash
# Run browser with dev tools
./build/bin/HodosBrowserShell.exe

# Test native functions in console
window.hodosBrowser.feature.getData()

# Check database
sqlite3 %APPDATA%/HodosBrowser/FeatureDB
```

## Common Patterns

### Singleton Manager

```cpp
class Manager {
public:
    static Manager& GetInstance() {
        static Manager instance;
        return instance;
    }
private:
    Manager() = default;
};
```

### SQLite Query Pattern

```cpp
const char* sql = "SELECT * FROM table WHERE id = ?";
sqlite3_stmt* stmt;

int rc = sqlite3_prepare_v2(db_, sql, -1, &stmt, nullptr);
if (rc != SQLITE_OK) return error;

sqlite3_bind_text(stmt, 1, value.c_str(), -1, SQLITE_STATIC);

while (sqlite3_step(stmt) == SQLITE_ROW) {
    // Process row
}

sqlite3_finalize(stmt);
```

### V8 Value Conversion

```cpp
// String
retval = CefV8Value::CreateString(str);

// Int
retval = CefV8Value::CreateInt(num);

// Bool
retval = CefV8Value::CreateBool(flag);

// Array
retval = CefV8Value::CreateArray(size);
for (int i = 0; i < size; i++) {
    retval->SetValue(i, item);
}

// Object
retval = CefV8Value::CreateObject(nullptr, nullptr);
retval->SetValue("key", value, V8_PROPERTY_ATTRIBUTE_NONE);
```

## Testing Strategy

### Unit Tests (C++)
- Database operations
- SQL query correctness
- Data conversions
- Edge cases

### Integration Tests
- V8 bindings functionality
- End-to-end operations
- Database integrity
- Error handling

### UI Tests (Frontend)
- Component rendering
- User interactions
- Loading states
- Error states

## Performance Optimization

### Database
- Proper indexing on all query columns
- Prepared statements for repeated queries
- Connection reuse
- WAL mode for concurrency

### V8 Bindings
- Efficient data conversion
- Minimal copies
- Smart pointers for memory management
- Avoid unnecessary allocations

### Frontend
- Virtual scrolling for large lists
- Lazy loading
- Debounced search
- React.memo for expensive components

## Security Considerations

### SQL Injection
- Always use prepared statements
- Never concatenate user input into queries
- Validate all inputs

### Data Validation
- Sanitize URLs
- Validate file paths
- Check data types
- Enforce constraints

### Privacy
- Respect incognito mode
- Secure deletion
- Handle sensitive data properly
- Clear data on request

## Troubleshooting

### Database Locked
```cpp
// Enable WAL mode
sqlite3_exec(db_, "PRAGMA journal_mode=WAL;", ...);

// Set busy timeout
sqlite3_busy_timeout(db_, 5000);
```

### V8 Binding Errors
- Check V8 context is valid
- Verify parameter types
- Handle exceptions properly
- Return correct V8 value types

### Frontend Not Updating
- Check V8 bindings are registered
- Verify function names match
- Ensure data format is correct
- Check console for errors

## Conclusion

This revised architecture provides a clean, performant, and maintainable implementation of browser features while keeping the wallet backend focused solely on BRC-100 operations. All browser features are handled natively in CEF where they belong, with direct database access and efficient V8 bindings to the frontend.

The separation of concerns ensures:
- **Wallet backend remains pure**: Only BRC-100 wallet operations
- **Browser features in browser layer**: CEF C++ handles all browser functionality
- **No unnecessary HTTP overhead**: Direct native calls for browser operations
- **Better performance**: Local database access, no network latency
- **Cleaner architecture**: Clear boundaries between components
