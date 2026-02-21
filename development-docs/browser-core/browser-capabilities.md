# Browser Capabilities Assessment

**Created**: 2026-02-19
**Status**: Complete (Phase A.4)
**Purpose**: Comprehensive assessment of what the Hodos Browser can and cannot do today, based on code audit of `cef-native/` and `frontend/src/`.

---

## 1. Fully Working

### Navigation & Tab Management
- **Back/Forward/Reload**: Fully implemented in `MainBrowserView.tsx` with UI buttons
- **URL bar / Omnibox**: Manual URL entry, Google search suggestions (`useOmniboxSuggestions`), history-based autocomplete, click-outside dismissal via mouse hook
- **Tab Management**: Multiple tabs (dynamic create/destroy), tab switching (Ctrl+Tab, Ctrl+Shift+Tab), new tab (Ctrl+T), tab close (Ctrl+W), active tab highlighting, tab title and favicon display, "Open link in new tab" from right-click (command 50100), tab bar scrolling

### Cookie Handling
- Cookie storage via CEF's built-in SQLite database
- View all cookies per domain, delete individual/bulk cookies
- Per-domain cookie blocking (`CookieBlockManager` + `DefaultTrackerList.h`)
- Session vs persistent cookie display
- Full attribute display: name, value, domain, path, expires, sameSite, httpOnly, secure
- Cookie search/filtering, real-time count per domain
- Blocked domain list management with UI

### Browser History
- Auto-recorded on navigation via `OnAddressChange`
- Search history by domain/URL/title
- Individual and bulk deletion
- Persistent across restarts (SQLite `HodosHistory` in `%APPDATA%/HodosBrowser/Default/`)
- Favicon URL and visit timestamp tracking
- Domain-based grouping in history panel

### Bookmarks
- Create/edit/delete bookmarks
- Folder organization
- Frontend CRUD UI in `BookmarkManager.tsx`
- Persistent storage via `BookmarkManager` singleton (SQLite `bookmarks.db`)

### Audio/Video Playback
- HTML5 `<audio>` and `<video>` tags work (CEF includes media support)
- No explicit custom handlers needed (uses CEF defaults)

### Fullscreen Mode
- `OnFullscreenModeChange` handler in `simple_handler.cpp`
- Header auto-hides in fullscreen
- Tab windows expand to fill window
- Exit fullscreen restores layout

### Keyboard Shortcuts
| Shortcut | Action |
|----------|--------|
| F12 / Ctrl+Shift+I | Open DevTools |
| Ctrl+T | New tab |
| Ctrl+W | Close current tab |
| Ctrl+Tab | Next tab |
| Ctrl+Shift+Tab | Previous tab |
| Ctrl+L | Focus address bar |
| Ctrl+R / F5 | Reload page |

### Developer Tools
- Remote debugging on port 9222
- F12 / Ctrl+Shift+I shortcuts work
- Right-click > Inspect Element
- Works in all browser windows including overlays

### File Upload
- Native file dialogs via `OnFileDialog` handler (returns `false` for CEF default)
- Guard flag `g_file_dialog_active` prevents wallet overlay closure during dialog

### JavaScript Engine
- JavaScript enabled in all browser settings
- Clipboard access enabled (`javascript_access_clipboard`)
- DOM paste enabled (`javascript_dom_paste`)
- `--expose-gc` flag set

### Overlay/Popup System
- Process-per-overlay architecture with isolated V8 contexts
- 7+ overlay types: settings, wallet, backup, BRC-100 auth, notification, cookie panel, omnibox
- Keep-alive pattern (hide/show without destroying)

---

## 2. Not Working / Not Implemented

### SSL Certificate Handling
- **Status**: No `OnCertificateError` handler
- **Behavior**: CEF default blocks invalid certificates
- **Impact**: Subprocesses may fail on SSL errors; no custom error page
- **User-reported**: Cannot log into x.com (likely SSL-related)
- **Fix needed**: Implement `CefRequestHandler::OnCertificateError` with user-facing UI

### Camera/Microphone Permissions
- **Status**: No `OnRequestMediaAccessPermission` handler
- **Behavior**: CEF default denies all camera/mic access
- **Impact**: Zoom, Teams, Discord video calls won't work
- **Fix needed**: Implement `CefPermissionHandler` with permission prompt UI

### Geolocation Permissions
- **Status**: No `OnRequestGeolocationPermission` handler
- **Behavior**: CEF default denies geolocation
- **Impact**: Maps, location-based services blocked

### Web Notifications
- **Status**: No `Notification` API permission handler
- **Behavior**: CEF default blocks web notifications
- **Note**: Custom notification overlays exist for wallet operations only

### Downloads Manager
- **Status**: COMPLETE (Sprint 3)
- **Behavior**: Save As dialog on download; progress tracked in overlay panel; pause/resume/cancel; open file/show in folder
- **Implementation**: `CefDownloadHandler` in SimpleHandler; `DownloadsOverlayRoot.tsx` overlay with keep-alive HWND; download icon in header toolbar with CircularProgress ring; toast notifications on start/complete

### Print Support
- **Status**: No `CefPrintHandler` implementation
- **Behavior**: Ctrl+P may open Chromium print dialog via CEF default
- **Impact**: No custom print preview

### Find-in-Page
- **Status**: No `CefFindHandler` implementation
- **Behavior**: Ctrl+F behavior unknown (may work via CEF default)
- **Fix needed**: Implement find bar UI with match count

### WebRTC
- **Status**: CEF includes Chromium WebRTC
- **But**: Combined with missing camera/mic permissions, video/audio calls won't work
- **Fix needed**: Camera/mic permissions must be implemented first

### FedCM (Federated Credential Management)
- **Status**: Not implemented anywhere in codebase
- **Impact**: Google/social sign-in flows may not work
- **CEF status**: Support may be limited in CEF 136

---

## 3. Partially Working

### Context Menus
- Right-click opens context menu
- "Inspect Element" option (custom DevTools launch)
- "Open in new tab" for links
- **Missing**: Copy/Paste/Cut, Save Image As, Copy Link Address, View Source, etc.

### Domain Permissions (BRC-100 Only)
- Full permission system for wallet operations (approval, spending limits, rate limiting)
- **Missing**: No permission system for generic web capabilities (geo, camera, notifications)

### Clipboard
- JavaScript clipboard access enabled in settings
- `copyToClipboard()` for wallet addresses
- **Missing**: Paste permission model (allowed but not prompted)

### Local Storage / IndexedDB
- Works via CEF built-in implementation
- No custom handler or access control
- Wallet overlay pre-caches balance/price in localStorage

### Service Workers
- CEF includes Service Worker support
- No custom handler for registration
- Frontend uses polling instead (`useBackgroundBalancePoller`)

---

## 4. CEF Handler Inheritance

**SimpleHandler** implements:
- `CefClient` (base)
- `CefLifeSpanHandler` (popup/tab creation)
- `CefDisplayHandler` (title, address, favicon, fullscreen changes)
- `CefLoadHandler` (loading state, errors)
- `CefRequestHandler` (resource interception for wallet API)
- `CefContextMenuHandler` (right-click menu)
- `CefDialogHandler` (file dialog)
- `CefKeyboardHandler` (keyboard shortcuts, DevTools)

**NOT Implemented** (needed for MVP):
- `CefDownloadHandler` — download progress tracking
- `CefPrintHandler` — print preview
- `CefPermissionHandler` — camera, mic, geolocation, notifications
- `CefFindHandler` — find-in-page
- `CefJsDialogHandler` — custom alert/confirm dialogs

---

## 5. CEF Settings Configuration

### Currently Set
```
remote_debugging_port = 9222
windowless_rendering_enabled = true
javascript_flags = "--expose-gc --allow-running-insecure-content"
javascript = STATE_ENABLED
javascript_access_clipboard = STATE_ENABLED
javascript_dom_paste = STATE_ENABLED
cache_path = %APPDATA%/HodosBrowser/Default
log_severity = LOGSEVERITY_INFO
```

### Not Explicitly Set (CEF Defaults)
- `media_stream_enabled` (unknown default)
- `web_security_enabled` (likely true)
- `file_access_from_file_urls`
- `universal_access_from_file_urls`

---

## 6. Summary Matrix

| Feature | Status | MVP Priority |
|---------|--------|-------------|
| Navigation | Full | N/A (done) |
| Tab Management | Full | N/A (done) |
| Cookies | Full | N/A (done) |
| History | Full | N/A (done) |
| Bookmarks | Full | N/A (done) |
| Audio/Video | Full | N/A (done) |
| Fullscreen | Full | N/A (done) |
| Keyboard Shortcuts | Full | N/A (done) |
| DevTools | Full | N/A (done) |
| File Upload | Full | N/A (done) |
| JavaScript | Full | N/A (done) |
| SSL Certs | Not Handled | **Critical** |
| Camera/Mic | Denied | High |
| Downloads | No UI | High |
| Context Menu | Limited | Medium |
| Find-in-Page | Unknown | Medium |
| Print | CEF Default | Low |
| Geolocation | Denied | Low |
| Web Notifications | Denied | Low |
| WebRTC | Blocked | Low (needs camera/mic first) |
| FedCM | Missing | Low |

---

## 7. FEATURES.md Assessment

The existing `development-docs/FEATURES.md` is **critically stale**:
- Describes Phase 5 (database migration) as "In Progress" — completed months ago
- References `wallet.json`, `actions.json` — deprecated since Phase 9
- Lists TODO items for UTXO caching, address migration — all done
- Does not mention any Phase 1-2 work (DPAPI, domain permissions, notifications, auto-approve)

**Recommendation**: Replace with a current feature matrix or archive and point to this document.

---

**End of Document**
