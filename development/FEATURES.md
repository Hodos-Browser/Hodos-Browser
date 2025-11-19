# Hodos Browser - Feature Roadmap

## 📊 Feature Categories

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
  - [x] Window management
  - [x] Process isolation
  - [x] V8 JavaScript engine integration

### Phase 5: Core Browser Features (Priority #1)
- [ ] **Tab Management**
  - [ ] Multiple tabs support
  - [ ] Tab creation/closing
  - [ ] Tab switching (keyboard shortcuts)
  - [ ] Tab drag-and-drop reordering
  - [ ] New tab page
  - [ ] Tab history navigation

- [ ] **Navigation** ✅ PARTIAL
  - [x] Address bar (URL bar) ✅ COMPLETE
  - [x] Back/Forward navigation ✅ COMPLETE
  - [x] Refresh/reload ✅ COMPLETE
  - [ ] Stop loading
  - [ ] Home button
  - [ ] Browser history access

- [ ] **History Management**
  - [ ] Browsing history storage (database)
  - [ ] History search
  - [ ] History clearing
  - [ ] Private browsing mode
  - [ ] History export

- [ ] **Bookmarks/Favorites**
  - [ ] Bookmark storage (database)
  - [ ] Bookmark bar
  - [ ] Bookmark folders
  - [ ] Bookmark import/export
  - [ ] Quick bookmark access

- [ ] **Cookies Management**
  - [ ] Cookie storage (database)
  - [ ] Cookie viewing/editing
  - [ ] Cookie deletion
  - [ ] Cookie blocking per site
  - [ ] Third-party cookie blocking

- [ ] **Ad Blocker** 🚨 **HIGH PRIORITY**
  - [ ] Ad blocking engine
  - [ ] Blocklist management (EasyList, EasyPrivacy)
  - [ ] Custom filter rules
  - [ ] Whitelist support
  - [ ] Trackers blocking
  - [ ] Malware/phishing protection
  - [ ] Privacy protection (fingerprinting, etc.)
  - [ ] Ad blocker statistics

### Phase 6: Browser Advanced Features
- [ ] **Downloads**
  - [ ] Download manager
  - [ ] Download history
  - [ ] Download location selection
  - [ ] Download pause/resume
  - [ ] File type associations

- [ ] **Security Features**
  - [ ] SSL/TLS certificate validation
  - [ ] Secure connection indicators
  - [ ] Phishing protection
  - [ ] Malware scanning
  - [ ] Content Security Policy (CSP)
  - [ ] Mixed content blocking

- [ ] **Privacy Features**
  - [ ] Do Not Track (DNT) support
  - [ ] Referrer policy controls
  - [ ] Canvas fingerprinting protection
  - [ ] WebRTC leak prevention
  - [ ] Browser fingerprint randomization
  - [ ] Extension isolation

- [ ] **Developer Tools**
  - [ ] Developer console
  - [ ] Inspect element
  - [ ] Network inspector
  - [ ] JavaScript debugger
  - [ ] Performance profiler
  - [ ] Application storage inspector

### Phase 7: Browser Polish
- [ ] **User Interface**
  - [ ] Customizable toolbar
  - [ ] Themes/skins
  - [ ] Dark mode
  - [ ] Font size controls
  - [ ] Zoom controls
  - [ ] Fullscreen mode

- [ ] **Settings & Preferences**
  - [ ] Settings UI
  - [ ] Privacy settings
  - [ ] Security settings
  - [ ] Content settings
  - [ ] Search engine management
  - [ ] Startup page configuration

- [ ] **Extensions/Plugins**
  - [ ] Extension API
  - [ ] Extension store
  - [ ] Extension management UI
  - [ ] Plugin support (if needed)

- [ ] **Performance**
  - [ ] Memory optimization
  - [ ] Tab discarding (unused tabs)
  - [ ] Startup time improvement
  - [ ] Page load optimization
  - [ ] Cache management

---

## 🔄 INTEGRATION FEATURES

### Phase 5: Browser-Wallet Integration
- [ ] **BRC-100 Site Authentication**
  - [ ] Automatic BRC-100 site detection
  - [ ] Authentication overlay/popup
  - [ ] Identity selection for sites
  - [ ] Persistent authentication per site
  - [ ] Logout functionality

- [ ] **Transaction Integration**
  - [ ] In-page transaction requests
  - [ ] Transaction confirmation overlay
  - [ ] Transaction status in address bar
  - [ ] Payment request API support

- [ ] **Wallet UI in Browser**
  - [ ] Wallet button in toolbar
  - [ ] Wallet panel/overlay
  - [ ] Quick balance display
  - [ ] Quick address copy
  - [ ] Transaction history access

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
