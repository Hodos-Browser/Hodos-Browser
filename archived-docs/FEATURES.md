> **ARCHIVED**: This feature roadmap predates the browser-core audit (Feb 2026) and is significantly stale. For current feature status, see `browser-core/browser-capabilities.md`. For the MVP implementation plan, see `browser-core/implementation-plan.md`.

# Hodos Browser - Feature Roadmap

## Feature Categories

- **🌐 Browser Features**: Standard browser functionality (tabs, history, bookmarks, cookies, etc.)
- **💼 Wallet Features**: BSV wallet and BRC-100 protocol functionality

---

## ✅ Completed Features - PRODUCTION READY

### Core Integration (Phase 1-3) ✅ COMPLETE
- [x] C++ HTTP client integration with Rust wallet daemon
- [x] Wallet service class for API communication
- [x] Identity management (create, get, mark backed up)
- [x] V8 context setup for JavaScript bridge
- [x] React frontend integration
- [x] Overlay system with backup modal
- [x] Complete pipeline: React → C++ → Rust Wallet → Response

### Process Architecture (Phase 4) ✅ COMPLETE
- [x] **Process-Per-Overlay System**
  - [x] Each overlay runs in dedicated CEF subprocess
  - [x] Fresh V8 context for each overlay
  - [x] Process isolation and security
  - [x] Message handling between processes
  - [x] Window management and cleanup

### Frontend Integration (Phase 4) ✅ COMPLETE
- [x] **React UI Components**
  - [x] Transaction forms with validation
  - [x] Balance display with USD conversion
  - [x] Address generation interface
  - [x] Transaction confirmation modals
  - [x] Success/error message handling
  - [x] Real-time UI updates

### Production Deployment ✅ COMPLETE
- [x] **Standalone Executable**
  - [x] Production-ready `HodosBrowserShell.exe`
  - [x] Easy startup configuration
  - [x] Complete documentation and README
  - [x] Clean debug logging removal

---

## 💼 WALLET FEATURES

### ✅ Completed Wallet Features (Need Database Migration)

#### HD Wallet System (Phase 4) ✅ COMPLETE ⚠️ **NEEDS DATABASE MIGRATION**
- [x] **BIP44 Hierarchical Deterministic Wallet**
  - [x] Mnemonic generation and storage (currently `wallet.json`)
  - [x] HD key derivation (BIP44 standard)
  - [x] Address generation with proper indexing
  - [x] Wallet file storage (`wallet.json`) → **MIGRATE TO `addresses` TABLE**
  - [x] Private key management and security
  - [ ] **TODO**: Migrate to SQLite `addresses` table

#### Transaction Management (Phase 4) ✅ COMPLETE ⚠️ **NEEDS DATABASE MIGRATION**
- [x] **Complete Transaction Flow**
  - [x] Transaction creation with UTXO selection (fetches from API on-demand)
  - [x] Transaction signing using BSV SDK
  - [x] Transaction broadcasting to multiple miners
  - [x] Real transaction ID extraction and display
  - [x] On-chain verification via WhatsOnChain
  - [x] Unified `/transaction/send` endpoint
  - [x] Transaction storage (`actions.json`) → **MIGRATE TO `transactions` TABLE**
  - [ ] **TODO**: Migrate to SQLite `transactions` table
  - [ ] **TODO**: Cache UTXOs in database instead of fetching on-demand

#### Balance & UTXO Management (Phase 4) ✅ COMPLETE ⚠️ **NEEDS DATABASE MIGRATION**
- [x] **Real-time Balance Display**
  - [x] Total balance calculation across all addresses
  - [x] Live UTXO fetching from WhatsOnChain API (on-demand) → **MIGRATE TO CACHED UTXOs**
  - [x] USD price conversion using CryptoCompare API
  - [x] Balance updates after transactions
  - [x] Multi-address balance aggregation
  - [ ] **TODO**: Store UTXOs in `utxos` table with background sync
  - [ ] **TODO**: Eliminate on-demand API calls for balance checks

#### Address Management (Phase 4) ✅ COMPLETE ⚠️ **NEEDS DATABASE MIGRATION**
- [x] **HD Address Generation**
  - [x] Generate new addresses on demand
  - [x] Address indexing and storage (`wallet.json`) → **MIGRATE TO `addresses` TABLE**
  - [x] Clipboard integration for address copying
  - [x] Address display in wallet UI
  - [x] Current address retrieval
  - [ ] **TODO**: Migrate address storage to SQLite `addresses` table

#### BRC-100 Authentication System ✅ COMPLETE
- [x] **Complete BRC-100 Protocol Implementation**
  - [x] Identity certificate generation and validation
  - [x] BRC-42 key derivation for P2P communication
  - [x] Authentication challenge/response flow
  - [x] Session management with cleanup
  - [x] Selective disclosure for privacy
  - [x] 16 HTTP API endpoints for BRC-100 operations
  - [x] Well-known auth endpoint (`/.well-known/auth`)

#### BEEF/SPV Integration ✅ COMPLETE ⚠️ **NEEDS DATABASE MIGRATION**
- [x] **Real Blockchain Integration**
  - [x] BEEF transaction creation and broadcasting
  - [x] SPV verification with real Merkle proofs
  - [x] Multi-API support (WhatsOnChain, GorillaPool, TAAL)
  - [x] Real blockchain transaction testing
  - [x] WebSocket support for real-time communication
  - [x] Parent transaction fetching (on-demand) → **MIGRATE TO CACHED PARENT TXs**
  - [x] Merkle proof fetching (on-demand) → **MIGRATE TO CACHED PROOFS**
  - [ ] **TODO**: Store parent transactions in `parent_transactions` table
  - [ ] **TODO**: Store Merkle proofs in `merkle_proofs` table
  - [ ] **TODO**: Store proven transactions in `proven_transactions` table
  - [ ] **TODO**: Implement background sync for BEEF/SPV data

---

## 🔄 IN PROGRESS - DATABASE MIGRATION

### Phase 5: Database Migration (Priority #1) 🔄
- [ ] **Database Foundation**
  - [ ] Create database module structure (`rust-wallet/src/database/`)
  - [ ] Set up `rusqlite` dependency and connection management
  - [ ] Create migration system (schema versioning)
  - [ ] Implement database initialization on wallet startup

- [ ] **Schema Implementation**
  - [ ] Create `addresses` table (migrate from `wallet.json`)
  - [ ] Create `transactions` table (migrate from `actions.json`)
  - [ ] Create `utxos` table (new - for UTXO caching)
  - [ ] Create `parent_transactions` table (new - for BEEF caching)
  - [ ] Create `merkle_proofs` table (new - for SPV caching)
  - [ ] Create `proven_transactions` table (new - for proven tx storage)
  - [ ] Create `block_headers` table (new - for block height resolution)

- [ ] **Data Migration**
  - [ ] Migrate `wallet.json` → `addresses` table
  - [ ] Migrate `actions.json` → `transactions` table
  - [ ] Maintain JSON fallback during transition
  - [ ] Test migration with real wallet data

- [ ] **UTXO Management**
  - [ ] Implement UTXO storage in database
  - [ ] Create background sync service (fetch every 5 minutes)
  - [ ] Update balance calculation to use cached UTXOs
  - [ ] Mark UTXOs as spent when used in transactions

- [ ] **BEEF/SPV Caching**
  - [ ] Pre-fetch and cache parent transactions
  - [ ] Pre-fetch and cache Merkle proofs
  - [ ] Update `signAction()` to use cached data
  - [ ] Implement proof refresh on reorg detection

---

## 🚀 PLANNED - WALLET FEATURES

### Phase 6: Database Optimization (Priority #2)
- [ ] **Performance Improvements**
  - [ ] Add database indexes (based on query patterns)
  - [ ] Implement query optimization
  - [ ] Add connection pooling if needed
  - [ ] Implement BLOB compression for large `raw_tx` data
  - [ ] Performance testing with large datasets

- [ ] **Cleanup**
  - [ ] Remove JSON file dependencies
  - [ ] Remove API fallback code (or keep as backup)
  - [ ] Database backup/restore utilities

### Phase 7: Advanced Wallet Features
- [ ] **Transaction History**
  - [x] Local transaction storage (JSON) → **MIGRATE TO DATABASE**
  - [ ] Transaction categorization and filtering
  - [ ] Search and export functionality
  - [ ] Transaction details view with BEEF/SPV data

- [ ] **Advanced Address Management**
  - [ ] Gap limit implementation (20-address standard)
  - [ ] Address pruning and cleanup
  - [ ] High-volume address generation
  - [ ] Privacy-preserving UTXO consolidation
  - [ ] Address usage tracking

- [ ] **SPV Verification**
  - [ ] Simplified Payment Verification implementation
  - [ ] Merkle proof verification
  - [ ] Transaction validation without full node
  - [ ] Blockchain reorg handling

- [ ] **Wallet Security**
  - [ ] PIN/password protection
  - [ ] Biometric authentication (if available)
  - [ ] Session timeout
  - [ ] Secure key storage
  - [ ] Wallet export/import
  - [ ] Backup file encryption

### Phase 8: BRC-100 Protocol Integration
- [ ] **BRC-100 Core Features**
  - [ ] BRC-100 protocol support
  - [ ] Token creation and management
  - [ ] State machine implementation
  - [ ] Protocol inheritance system

- [ ] **Identity & Authentication**
  - [x] Digital certificate management → **ADD DATABASE STORAGE**
  - [x] BRC-100 identity verification
  - [x] Certificate-based authentication
  - [ ] Multi-identity support
  - [ ] Identity database storage

- [ ] **Basket Management**
  - [ ] UTXO basket creation (BRC-46)
  - [ ] Basket-based token tracking
  - [ ] Application-specific UTXO grouping
  - [ ] Basket state synchronization

- [ ] **BRC-100 Applications**
  - [ ] Deploy BRC-100 applications
  - [ ] Interact with existing protocols
  - [ ] Child application creation
  - [ ] Protocol extension support

---

## 🌐 BROWSER FEATURES

### ✅ Completed Browser Features
- [x] **Chromium Embedded Framework (CEF)**
  - [x] Browser shell with CEF
    - **CEF API**: `CefInitialize()`, `CefShutdown()` - Initialize/shutdown CEF
    - **CEF API**: `CefBrowserHost::CreateBrowser()` - Create browser instances
    - **CEF API**: `CefSettings` - Configure CEF settings
  - [x] Window management
    - **CEF API**: `CefWindowInfo::SetAsChild()` - Embed browser in window
    - **CEF API**: `CefBrowserHost::GetWindowHandle()` - Get native window handle
    - **Implementation**: Windows API (`CreateWindow`, `SetWindowPos`) for window management
  - [x] Process isolation
    - **CEF API**: `CefSettings::multi_threaded_message_loop` - Multi-process architecture
    - **CEF API**: `CefExecuteProcess()` - Execute subprocesses
    - **CEF API**: `CefBrowserHost::GetIdentifier()` - Track browser instances
  - [x] V8 JavaScript engine integration
    - **CEF API**: `CefV8Context`, `CefV8Value` - V8 JavaScript bindings
    - **CEF API**: `CefV8Handler::Execute()` - Expose native functions to JavaScript
    - **CEF API**: `CefFrame::ExecuteJavaScript()` - Execute JavaScript from C++

### Phase 5: Core Browser Features (Priority #1)
- [ ] **Tab Management**
  - [ ] Multiple tabs support
    - **CEF API**: `CefBrowserHost::CreateBrowser()` - Create multiple `CefBrowser` instances
    - **CEF API**: `CefLifeSpanHandler::OnBeforePopup()` - Handle new tab/window requests
    - **Implementation**: Manage multiple `CefBrowser` instances in UI, each representing a tab
  - [ ] Tab creation/closing
    - **CEF API**: `CefBrowserHost::CreateBrowser()` - Create new browser instance
    - **CEF API**: `CefBrowserHost::CloseBrowser()` - Close browser instance
    - **CEF API**: `CefLifeSpanHandler::OnBeforeClose()` - Handle cleanup before tab closes
  - [ ] Tab switching (keyboard shortcuts)
    - **CEF API**: `CefKeyboardHandler::OnKeyEvent()` - Handle keyboard events
    - **Implementation**: Track active tab, switch focus between `CefBrowser` instances
  - [ ] Tab drag-and-drop reordering
    - **CEF API**: Custom UI implementation (CEF doesn't provide tab UI)
    - **Implementation**: Handle drag events in UI layer, reorder `CefBrowser` instances
  - [ ] New tab page
    - **CEF API**: `CefBrowserHost::CreateBrowser()` with custom URL
    - **Implementation**: Load custom HTML page for new tabs
  - [ ] Tab history navigation
    - **CEF API**: `CefBrowser::CanGoBack()`, `CefBrowser::CanGoForward()` - Check history state
    - **CEF API**: `CefBrowser::GoBack()`, `CefBrowser::GoForward()` - Navigate history

- [ ] **Navigation** ✅ PARTIAL
  - [x] Address bar (URL bar) ✅ COMPLETE
    - **CEF API**: `CefBrowser::GetMainFrame()->GetURL()` - Get current URL
    - **CEF API**: `CefBrowser::GetMainFrame()->LoadURL(url)` - Navigate to URL
    - **CEF API**: `CefDisplayHandler::OnAddressChange()` - Monitor URL changes
  - [x] Back/Forward navigation ✅ COMPLETE
    - **CEF API**: `CefBrowser::GoBack()` - Navigate back
    - **CEF API**: `CefBrowser::GoForward()` - Navigate forward
    - **CEF API**: `CefBrowser::CanGoBack()`, `CefBrowser::CanGoForward()` - Check navigation state
  - [x] Refresh/reload ✅ COMPLETE
    - **CEF API**: `CefBrowser::Reload()` - Reload current page
    - **CEF API**: `CefBrowser::ReloadIgnoreCache()` - Reload without cache
  - [ ] Stop loading
    - **CEF API**: `CefBrowser::StopLoad()` - Stop current page load
    - **CEF API**: `CefLoadHandler::OnLoadingStateChange()` - Monitor loading state
  - [ ] Home button
    - **CEF API**: `CefBrowser::GetMainFrame()->LoadURL(homeUrl)` - Load home URL
    - **Implementation**: Store home URL in settings, load on button click
  - [ ] Browser history access
    - **CEF API**: `CefNavigationEntryVisitor` - Visit navigation entries
    - **CEF API**: `CefBrowserHost::GetNavigationEntries()` - Get navigation history
    - **CEF API**: `CefNavigationEntry::GetURL()`, `GetTitle()`, `GetDisplayURL()` - Get history details

- [ ] **History Management**
  - [ ] Browsing history storage (database)
    - **CEF API**: `CefNavigationEntryVisitor` - Iterate through history entries
    - **CEF API**: `CefNavigationEntry::GetURL()`, `GetTitle()`, `GetTimestamp()` - Get history data
    - **Implementation**: Store in SQLite database (separate from wallet DB)
  - [ ] History search
    - **Implementation**: Query database for matching URLs/titles
  - [ ] History clearing
    - **CEF API**: `CefRequestContext::ClearCertificateExceptions()` - Clear SSL exceptions
    - **CEF API**: `CefRequestContext::ClearHttpAuthCredentials()` - Clear auth data
    - **Implementation**: Delete history entries from database, clear CEF cache
  - [ ] Private browsing mode
    - **CEF API**: `CefRequestContext::CreateContext()` - Create isolated context
    - **CEF API**: `CefRequestContextSettings::cache_path` - Set to empty for no persistence
    - **Implementation**: Use separate `CefRequestContext` with no persistent storage
  - [ ] History export
    - **Implementation**: Export database records to file (JSON/CSV)

- [ ] **Bookmarks/Favorites**
  - [ ] Bookmark storage (database)
    - **CEF API**: None - CEF doesn't provide bookmark management
    - **Implementation**: Custom SQLite database storage
  - [ ] Bookmark bar
    - **Implementation**: Custom UI component displaying bookmarks
  - [ ] Bookmark folders
    - **Implementation**: Database schema with folder hierarchy
  - [ ] Bookmark import/export
    - **Implementation**: Import/export to HTML bookmark format (Netscape format)
  - [ ] Quick bookmark access
    - **Implementation**: UI shortcuts, keyboard shortcuts via `CefKeyboardHandler`

- [ ] **Cookies Management**
  - [ ] Cookie storage (database)
    - **CEF API**: `CefCookieManager::GetGlobalManager()` - Get cookie manager
    - **CEF API**: `CefCookieManager::VisitAllCookies()` - Visit all cookies
    - **CEF API**: `CefCookieVisitor::Visit()` - Process each cookie
    - **Implementation**: Store cookies in SQLite database for management UI
  - [ ] Cookie viewing/editing
    - **CEF API**: `CefCookieManager::SetCookie()` - Set/modify cookie
    - **CEF API**: `CefCookie::GetName()`, `GetValue()`, `GetDomain()`, etc. - Get cookie properties
  - [ ] Cookie deletion
    - **CEF API**: `CefCookieManager::DeleteCookies()` - Delete cookies
    - **CEF API**: `CefCookieManager::FlushStore()` - Persist changes
  - [ ] Cookie blocking per site
    - **CEF API**: `CefRequestHandler::OnBeforeResourceLoad()` - Intercept requests
    - **CEF API**: `CefCookieManager::SetCookie()` with validation - Block specific cookies
    - **Implementation**: Maintain blocklist, filter cookies in request handler
  - [ ] Third-party cookie blocking
    - **CEF API**: `CefRequestHandler::OnBeforeResourceLoad()` - Check request origin
    - **CEF API**: `CefCookieManager::SetSupportedSchemes()` - Control cookie schemes
    - **Implementation**: Check if cookie domain matches page domain, block if different

- [ ] **Ad Blocker** 🚨 **HIGH PRIORITY**
  - [ ] Ad blocking engine
    - **CEF API**: `CefRequestHandler::OnBeforeResourceLoad()` - Intercept all resource requests
    - **CEF API**: `CefRequest::GetURL()`, `GetResourceType()` - Get request details
    - **CEF API**: Return `RV_CANCEL` to block request
    - **Implementation**: Parse EasyList/EasyPrivacy rules, match against requests
  - [ ] Blocklist management (EasyList, EasyPrivacy)
    - **Implementation**: Download and parse filter lists, convert to efficient data structure
  - [ ] Custom filter rules
    - **Implementation**: User-defined filter rules, merge with blocklists
  - [ ] Whitelist support
    - **Implementation**: Check whitelist before blocking, allow whitelisted domains
  - [ ] Trackers blocking
    - **CEF API**: `CefRequestHandler::OnBeforeResourceLoad()` - Block tracking requests
    - **Implementation**: Identify tracking domains/patterns, block requests
  - [ ] Malware/phishing protection
    - **CEF API**: `CefRequestHandler::OnBeforeResourceLoad()` - Check URL against threat lists
    - **Implementation**: Maintain threat database, block malicious URLs
  - [ ] Privacy protection (fingerprinting, etc.)
    - **CEF API**: `CefRequestHandler::OnBeforeResourceLoad()` - Modify request headers
    - **CEF API**: `CefRequest::SetHeaderMap()` - Remove/modify identifying headers
    - **Implementation**: Block canvas fingerprinting, WebRTC leaks, etc.
  - [ ] Ad blocker statistics
    - **Implementation**: Track blocked requests, display counts per domain

### Phase 6: Browser Advanced Features
- [ ] **Downloads**
  - [ ] Download manager
    - **CEF API**: `CefDownloadHandler::OnBeforeDownload()` - Handle download start
    - **CEF API**: `CefDownloadHandler::OnDownloadUpdated()` - Monitor download progress
    - **CEF API**: `CefDownloadItem::GetURL()`, `GetFullPath()`, `GetReceivedBytes()` - Get download info
    - **Implementation**: Track downloads, show progress UI
  - [ ] Download history
    - **CEF API**: `CefDownloadItem::GetId()`, `GetStartTime()` - Get download metadata
    - **Implementation**: Store download records in database
  - [ ] Download location selection
    - **CEF API**: `CefDownloadHandler::OnBeforeDownload()` - Set download path
    - **CEF API**: `CefBrowserHost::SetDownloadPath()` - Set default download directory
    - **Implementation**: Show file dialog, set `suggested_name` in callback
  - [ ] Download pause/resume
    - **CEF API**: `CefDownloadItem::IsPaused()`, `IsInProgress()` - Check download state
    - **CEF API**: `CefDownloadItem::Pause()`, `Resume()`, `Cancel()` - Control download
  - [ ] File type associations
    - **Implementation**: Windows file associations, open downloaded files with default app

- [ ] **Security Features**
  - [ ] SSL/TLS certificate validation
    - **CEF API**: `CefRequestHandler::OnCertificateError()` - Handle certificate errors
    - **CEF API**: `CefSSLInfo::GetSubject()`, `GetIssuer()` - Get certificate details
    - **CEF API**: `CefRequestHandler::OnQuotaRequest()` - Handle quota requests
  - [ ] Secure connection indicators
    - **CEF API**: `CefRequestHandler::OnCertificateError()` - Check certificate validity
    - **CEF API**: `CefSSLStatus::IsSecureConnection()` - Check if connection is secure
    - **CEF API**: `CefDisplayHandler::OnStatusMessage()` - Display security status
  - [ ] Phishing protection
    - **CEF API**: `CefRequestHandler::OnBeforeResourceLoad()` - Check URL against threat lists
    - **Implementation**: Maintain phishing database, block malicious URLs
  - [ ] Malware scanning
    - **CEF API**: `CefRequestHandler::OnBeforeResourceLoad()` - Scan URLs/files
    - **Implementation**: Integrate with threat intelligence APIs
  - [ ] Content Security Policy (CSP)
    - **CEF API**: `CefRequestHandler::OnBeforeResourceLoad()` - Modify CSP headers
    - **CEF API**: `CefRequest::SetHeaderMap()` - Set/modify headers
    - **Implementation**: Parse and enforce CSP rules
  - [ ] Mixed content blocking
    - **CEF API**: `CefRequestHandler::OnBeforeResourceLoad()` - Check request scheme
    - **CEF API**: `CefRequest::GetURL()` - Check if HTTP on HTTPS page
    - **Implementation**: Block HTTP resources on HTTPS pages

- [ ] **Privacy Features**
  - [ ] Do Not Track (DNT) support
    - **CEF API**: `CefRequestHandler::OnBeforeResourceLoad()` - Add DNT header
    - **CEF API**: `CefRequest::SetHeaderMap()` - Set `DNT: 1` header
  - [ ] Referrer policy controls
    - **CEF API**: `CefRequestHandler::OnBeforeResourceLoad()` - Modify referrer
    - **CEF API**: `CefRequest::SetReferrer()` - Set referrer policy
  - [ ] Canvas fingerprinting protection
    - **CEF API**: `CefV8Handler` - Intercept JavaScript canvas API calls
    - **Implementation**: Inject noise into canvas operations, block canvas data extraction
  - [ ] WebRTC leak prevention
    - **CEF API**: `CefRequestHandler::OnBeforeResourceLoad()` - Block WebRTC requests
    - **CEF API**: `CefV8Handler` - Intercept `RTCPeerConnection` API
    - **Implementation**: Block or modify WebRTC connections
  - [ ] Browser fingerprint randomization
    - **CEF API**: `CefRequestHandler::OnBeforeResourceLoad()` - Randomize headers
    - **CEF API**: `CefRequest::SetHeaderMap()` - Modify User-Agent, Accept-Language, etc.
    - **Implementation**: Randomize browser characteristics per session
  - [ ] Extension isolation
    - **CEF API**: `CefRequestContext::CreateContext()` - Create isolated context per extension
    - **Implementation**: Run extensions in separate contexts

- [ ] **Developer Tools**
  - [ ] Developer console
    - **CEF API**: `CefBrowserHost::ShowDevTools()` - Open DevTools window
    - **CEF API**: `CefBrowserHost::CloseDevTools()` - Close DevTools
    - **CEF API**: `CefV8Context::GetCurrentContext()->GetFrame()->ExecuteJavaScript()` - Execute JS
  - [ ] Inspect element
    - **CEF API**: `CefBrowserHost::ShowDevTools()` - Open DevTools with element inspector
    - **CEF API**: `CefContextMenuHandler::OnBeforeContextMenu()` - Add "Inspect Element" option
  - [ ] Network inspector
    - **CEF API**: `CefRequestHandler::OnBeforeResourceLoad()` - Log all network requests
    - **CEF API**: `CefRequestHandler::GetResourceRequestHandler()` - Track request lifecycle
    - **Implementation**: Display network requests in DevTools panel
  - [ ] JavaScript debugger
    - **CEF API**: `CefBrowserHost::ShowDevTools()` - Includes JavaScript debugger
    - **CEF API**: Remote debugging via `CefSettings::remote_debugging_port`
  - [ ] Performance profiler
    - **CEF API**: `CefBrowserHost::ShowDevTools()` - Includes performance profiler
    - **CEF API**: `CefV8Context::Enter()` / `Exit()` - Measure execution time
  - [ ] Application storage inspector
    - **CEF API**: `CefBrowserHost::ShowDevTools()` - Includes storage inspector
    - **CEF API**: `CefCookieManager::VisitAllCookies()` - Inspect cookies
    - **CEF API**: IndexedDB/LocalStorage accessible via DevTools

### Phase 7: Browser Polish
- [ ] **User Interface**
  - [ ] Customizable toolbar
    - **Implementation**: Custom UI layer, not CEF-specific
  - [ ] Themes/skins
    - **Implementation**: CSS/UI styling, not CEF-specific
  - [ ] Dark mode
    - **CEF API**: `CefBrowserHost::SetZoomLevel()` - Can affect rendering
    - **CEF API**: `CefRequestHandler::OnBeforeResourceLoad()` - Inject dark mode CSS
    - **Implementation**: Inject CSS or use `prefers-color-scheme` media query
  - [ ] Font size controls
    - **CEF API**: `CefBrowserHost::SetZoomLevel()` - Zoom affects font size
    - **CEF API**: `CefRequestHandler::OnBeforeResourceLoad()` - Inject CSS font-size rules
  - [ ] Zoom controls
    - **CEF API**: `CefBrowserHost::SetZoomLevel(level)` - Set zoom level
    - **CEF API**: `CefBrowserHost::GetZoomLevel()` - Get current zoom level
    - **CEF API**: `CefDisplayHandler::OnLoadingStateChange()` - Monitor page load for zoom
  - [ ] Fullscreen mode
    - **CEF API**: `CefDisplayHandler::OnFullscreenModeChange()` - Handle fullscreen requests
    - **CEF API**: `CefBrowserHost::SetFullscreen()` - Enter/exit fullscreen
    - **Implementation**: Handle fullscreen state in window management

- [ ] **Settings & Preferences**
  - [ ] Settings UI
    - **Implementation**: Custom UI, store settings in database/registry
  - [ ] Privacy settings
    - **CEF API**: `CefRequestContextSettings` - Configure privacy settings
    - **CEF API**: `CefCookieManager::SetSupportedSchemes()` - Control cookie schemes
    - **Implementation**: Store privacy preferences, apply via CEF settings
  - [ ] Security settings
    - **CEF API**: `CefRequestHandler::OnCertificateError()` - Configure certificate handling
    - **CEF API**: `CefRequestContextSettings::accept_language_list` - Set language preferences
    - **Implementation**: Store security preferences, apply via CEF settings
  - [ ] Content settings
    - **CEF API**: `CefRequestContextSettings` - Configure content settings
    - **CEF API**: `CefRequestHandler::OnBeforeResourceLoad()` - Apply content filters
  - [ ] Search engine management
    - **Implementation**: Custom search engine database, inject into address bar
  - [ ] Startup page configuration
    - **CEF API**: `CefBrowserHost::CreateBrowser()` - Set initial URL
    - **Implementation**: Store startup page preference, load on browser start

- [ ] **Extensions/Plugins**
  - [ ] Extension API
    - **CEF API**: `CefExtensionHandler::OnExtensionLoaded()` - Handle extension loading
    - **CEF API**: `CefExtension::GetIdentifier()`, `GetPath()` - Get extension info
    - **CEF API**: `CefV8Handler` - Expose extension APIs to JavaScript
    - **Implementation**: Custom extension system built on CEF V8 bindings
  - [ ] Extension store
    - **Implementation**: Custom extension marketplace, not CEF-specific
  - [ ] Extension management UI
    - **CEF API**: `CefExtensionHandler::OnExtensionLoaded()` - Track loaded extensions
    - **CEF API**: `CefExtension::Unload()` - Unload extensions
    - **Implementation**: UI to list, enable, disable, remove extensions
  - [ ] Plugin support (if needed)
    - **CEF API**: `CefRequestHandler::OnBeforeResourceLoad()` - Handle plugin requests
    - **Note**: CEF doesn't support NPAPI plugins (deprecated), use extensions instead

- [ ] **Performance**
  - [ ] Memory optimization
    - **CEF API**: `CefBrowserHost::WasHidden()` - Hide browser to reduce memory
    - **CEF API**: `CefBrowserHost::NotifyScreenInfoChanged()` - Optimize rendering
    - **Implementation**: Monitor memory usage, optimize browser instances
  - [ ] Tab discarding (unused tabs)
    - **CEF API**: `CefBrowserHost::WasHidden()` - Hide inactive tabs
    - **CEF API**: `CefBrowserHost::CloseBrowser()` - Close inactive tabs
    - **Implementation**: Track tab activity, discard unused tabs after timeout
  - [ ] Startup time improvement
    - **CEF API**: `CefSettings::multi_threaded_message_loop` - Use multi-threaded mode
    - **CEF API**: `CefSettings::log_severity` - Reduce logging overhead
    - **Implementation**: Lazy load components, optimize initialization
  - [ ] Page load optimization
    - **CEF API**: `CefRequestHandler::OnBeforeResourceLoad()` - Block unnecessary resources
    - **CEF API**: `CefRequestContext::ClearCache()` - Clear cache for performance
    - **Implementation**: Resource blocking, cache optimization
  - [ ] Cache management
    - **CEF API**: `CefRequestContext::ClearCache()` - Clear browser cache
    - **CEF API**: `CefRequestContextSettings::cache_path` - Set cache location
    - **CEF API**: `CefRequestContextSettings::persist_session_cookies` - Control cookie persistence
    - **Implementation**: Cache size limits, cache clearing UI

---

## 🔄 INTEGRATION FEATURES

### Phase 5: Browser-Wallet Integration
- [ ] **BRC-100 Site Authentication**
  - [ ] Automatic BRC-100 site detection
    - **CEF API**: `CefRequestHandler::OnBeforeResourceLoad()` - Intercept requests to `/.well-known/auth`
    - **CEF API**: `CefRequest::GetURL()` - Check for BRC-100 endpoints
    - **Implementation**: Detect BRC-100 authentication requests, show auth UI
  - [ ] Authentication overlay/popup
    - **CEF API**: `CefBrowserHost::CreateBrowser()` - Create overlay browser
    - **CEF API**: `CefLifeSpanHandler::OnBeforePopup()` - Control popup creation
    - **Implementation**: Custom overlay window for authentication
  - [ ] Identity selection for sites
    - **Implementation**: Store site-identity mappings, select identity per domain
  - [ ] Persistent authentication per site
    - **CEF API**: `CefCookieManager` - Store authentication tokens as cookies
    - **Implementation**: Store auth sessions in database, restore on site visit
  - [ ] Logout functionality
    - **CEF API**: `CefCookieManager::DeleteCookies()` - Clear auth cookies
    - **Implementation**: Clear site authentication data

- [ ] **Transaction Integration**
  - [ ] In-page transaction requests
    - **CEF API**: `CefRequestHandler::OnBeforeResourceLoad()` - Intercept transaction requests
    - **CEF API**: `CefV8Handler` - Expose transaction API to JavaScript
    - **Implementation**: Detect transaction requests from web pages, show confirmation
  - [ ] Transaction confirmation overlay
    - **CEF API**: `CefBrowserHost::CreateBrowser()` - Create confirmation overlay
    - **Implementation**: Custom overlay showing transaction details
  - [ ] Transaction status in address bar
    - **CEF API**: `CefDisplayHandler::OnAddressChange()` - Monitor URL changes
    - **CEF API**: `CefDisplayHandler::OnStatusMessage()` - Display status messages
    - **Implementation**: Show transaction status icon/indicator in address bar
  - [ ] Payment request API support
    - **CEF API**: `CefV8Handler` - Implement Payment Request API
    - **CEF API**: `CefRequestHandler::OnBeforeResourceLoad()` - Handle payment requests
    - **Implementation**: Implement W3C Payment Request API for BSV payments

- [ ] **Wallet UI in Browser**
  - [ ] Wallet button in toolbar
    - **Implementation**: Custom UI button, not CEF-specific
  - [ ] Wallet panel/overlay
    - **CEF API**: `CefBrowserHost::CreateBrowser()` - Create wallet overlay browser
    - **Implementation**: Custom overlay window for wallet UI
  - [ ] Quick balance display
    - **CEF API**: `CefV8Handler` - Expose balance API to JavaScript
    - **Implementation**: Fetch balance from Rust wallet, display in UI
  - [ ] Quick address copy
    - **CEF API**: `CefV8Handler` - Expose address API to JavaScript
    - **Implementation**: Copy address to clipboard via Windows API
  - [ ] Transaction history access
    - **CEF API**: `CefV8Handler` - Expose transaction history API
    - **Implementation**: Query database, display transaction list

---

## 📋 Feature Status Legend

- [ ] **Not Started** - Feature not yet implemented
- [🔄] **In Progress** - Currently being worked on
- [✅] **Completed** - Feature fully implemented and tested
- [⚠️] **Blocked** - Waiting on dependencies or external factors
- [❌] **Cancelled** - Feature removed from roadmap

---

## 🏗️ Technical Debt & Improvements

- [ ] **Code Organization**
  - [ ] Refactor C++ handlers for better maintainability
  - [ ] Improve error handling across all layers
  - [ ] Add comprehensive logging system
  - [ ] Create unit tests for critical components

- [ ] **Database Migration**
  - [ ] Complete JSON → SQLite migration
  - [ ] Remove JSON file dependencies
  - [ ] Implement database backup utilities
  - [ ] Performance optimization

- [ ] **Documentation**
  - [ ] API documentation for Rust wallet daemon
  - [ ] C++ class documentation
  - [ ] React component documentation
  - [ ] User manual
  - [ ] Database schema documentation

- [ ] **Build & Deployment**
  - [ ] Automated build pipeline
  - [ ] Cross-platform builds
  - [ ] Installer creation
  - [ ] Update mechanism

---

*Last Updated: November 19, 2025*
*Next Review: After Database Migration completion*
