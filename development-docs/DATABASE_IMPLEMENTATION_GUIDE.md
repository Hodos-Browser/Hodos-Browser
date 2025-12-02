# Database Implementation Guide

> **Status**: Planning Phase → Research Complete → Ready for Implementation
> **Last Updated**: 2025-11-19

## Overview

This guide outlines the plan to migrate HodosBrowser from JSON file storage to a proper database solution. The migration will enable efficient UTXO management, BEEF/SPV transaction building, improved performance for wallet operations, and proper browser data storage.

## Research Summary

### How Browsers Store Data

**Chrome/Brave/Firefox (Chromium-based)**:
- **Internal Browser Data**: Uses **SQLite** databases stored in user profile directory
  - `History` - Browsing history
  - `Cookies` - Cookie storage
  - `Login Data` - Saved passwords
  - `Web Data` - Autofill, search engines
  - `Top Sites` - Most visited sites
  - Each browser profile has its own database files
- **Web App Storage**: Exposes **IndexedDB** API to web applications
  - Browser-managed, domain-origin based
  - Stored in browser profile directory
  - Large storage capacity (GB range)
  - Supports transactions, indexing, structured queries

**Key Insight**: Browsers use SQLite for internal management, but expose IndexedDB to web apps. We need both approaches - SQLite for native Rust backend, IndexedDB consideration for frontend if needed.

### CEF (Chromium Embedded Framework) Database Support

**CEF Database Capabilities**:
- ✅ **IndexedDB**: Fully supported via Chromium's Blink engine
  - Exposed through V8 JavaScript bindings
  - Same API as standard Chrome browser
  - Stored in CEF's user data directory
- ✅ **localStorage/sessionStorage**: Supported (Web Storage API)
- ✅ **Cookies**: Supported (managed by Chromium)
- ❌ **No native CEF database classes**: CEF doesn't expose direct SQLite APIs
  - Must use Chromium's IndexedDB for web app storage
  - For native Rust code, we use our own SQLite implementation

**Recommendation**:
- **Rust Wallet Backend**: Use SQLite directly (rusqlite) - not CEF's IndexedDB
- **Frontend Web Apps**: Can use IndexedDB if needed, but wallet data flows through Rust backend
- **Browser Functions**: Consider using IndexedDB for browser-specific data (history, bookmarks, cache) if exposed to frontend

### BSV TypeScript Wallet Database Analysis

**Reference**: `reference/ts-brc100/src/storage/`

**Database Strategy**:
1. **Dual Database Support**: `StorageIdb` (IndexedDB) + `StorageKnex` (SQLite/MySQL)
2. **Unified Interface**: `StorageProvider` abstraction works with both
3. **Query Builder**: Uses `knex` for SQL query building
4. **Libraries Used**:
   - `idb` - Promise wrapper for IndexedDB (browser)
   - `sqlite3` - SQLite bindings (Node.js/server)
   - `knex` - SQL query builder supporting multiple DBs

**Database Schema** (from `ts-brc100`):
- `users` - User accounts
- `transactions` - Transaction records (with rawTx, inputBEEF)
- `proven_txs` - Proven transactions with Merkle proofs (height, index, merklePath, rawTx, blockHash)
- `proven_tx_reqs` - Transaction requests waiting for proofs
- `outputs` - UTXO outputs (vout, satoshis, lockingScript, txid)
- `output_baskets` - Token baskets (BRC-46)
- `output_tags` - Output categorization
- `certificates` - BRC-42/BRC-84 certificates
- `certificate_fields` - Certificate data fields
- `commissions` - Transaction fees
- `tx_labels` - Transaction labels/categories
- `sync_states` - Sync status for multi-store scenarios
- `monitor_events` - Wallet monitoring events
- `settings` - Wallet settings

**Why They Chose This**:
1. **Flexibility**: Single codebase works in browser (IndexedDB) and Node.js (SQLite)
2. **Compatibility**: Browser-first approach with IndexedDB
3. **Query Power**: SQL-like queries via knex for complex operations
4. **Scalability**: Support for MySQL for server/multi-user scenarios
5. **Separation**: BEEF data stored separately (proven_txs vs transactions)

**Key Learnings**:
- ✅ Separate tables for proven transactions (with Merkle proofs) vs unproven transactions
- ✅ Store raw transaction bytes (`rawTx` as `number[]`)
- ✅ Store Merkle proof data (height, index, merklePath, blockHash)
- ✅ Outputs linked to transactions via foreign keys
- ✅ Support for output baskets (tokenization/BRC-46)

## Current State

**Storage Location**: `%APPDATA%/HodosBrowser/wallet/`

**Current Database Storage** (Migrated from JSON):
- ✅ `wallet.db` - SQLite database with all wallet data
  - `wallets` - Wallet identity and mnemonic
  - `addresses` - HD wallet addresses with `pending_utxo_check` flag
  - `transactions` - Transaction history (migrated from `actions.json`)
  - `utxos` - UTXO cache with spending tracking
  - `baskets`, `certificates`, `messages` - Schema ready for future features
  - `parent_transactions`, `merkle_proofs`, `block_headers` - Schema ready for Phase 5

**Legacy JSON Files** (No longer used):
- `wallet.json` - ✅ Migrated to `wallets` and `addresses` tables
- `actions.json` - ✅ Migrated to `transactions` table
- `domainWhitelist.json` - Still used (future: migrate to database)

**Previous Limitations (Now Resolved)**:
- ✅ UTXO caching - UTXOs stored in database, background sync every 5 minutes
- ✅ Fast balance checks - Calculated from database cache (no API calls unless cache empty)
- ✅ ACID transactions - SQLite provides atomic operations
- ✅ Indexing - Fast lookups with proper database indexes
- ✅ Background sync - Automatic UTXO updates with gap limit scanning
- ✅ New address detection - Pending cache checks new addresses immediately
- ⏳ Parent transaction storage - Schema ready, Phase 5 implementation
- ⏳ Merkle proof storage - Schema ready, Phase 5 implementation

## Goals

1. **UTXO Management**: Store UTXOs locally with automatic syncing
2. **BEEF/SPV Support**: Pre-fetch and store parent transactions and Merkle proofs
3. **Performance**: Eliminate on-demand API calls during transaction building
4. **Reliability**: ACID transactions, proper error handling
5. **Scalability**: Support thousands of addresses and UTXOs efficiently

## Database Architecture Decision

### Recommendation: **Dual Database Architecture**

**1. Browser Database (SQLite)** - For browser-specific data
- **Location**: `%APPDATA%/HodosBrowser/browser/`
- **Purpose**: History, bookmarks, cache, cookies, browsing data
- **Implementation**: SQLite via Rust (future consideration)
- **Rationale**: Separate from wallet data for security and organization

**2. Wallet Database (SQLite)** - For wallet data
- **Location**: `%APPDATA%/HodosBrowser/wallet/`
- **Purpose**: Transactions, UTXOs, addresses, BEEF data, Merkle proofs
- **Implementation**: SQLite via Rust (`rusqlite` crate)
- **Rationale**:
  - Native Rust backend - direct SQLite access (not through CEF/IndexedDB)
  - Single file - easy backup
  - ACID transactions
  - Full SQL query capabilities
  - Excellent performance for single-user wallet
  - Portable and easy to migrate

### Why Separate Databases?

1. **Security**: Wallet data requires encryption and strict access control
2. **Backup Strategy**: Users may want to back up wallet separately from browser data
3. **Performance**: Browser data can be large (history, cache) - separate DBs prevent bloat
4. **Organization**: Clear separation of concerns
5. **Compliance**: Different data retention/cleanup policies

### Why SQLite (Not IndexedDB)?

**For Rust Wallet Backend**:
- ✅ Direct native access (no CEF/JavaScript layer)
- ✅ Better performance for complex queries
- ✅ Full SQL capabilities
- ✅ Easier migration from JSON files
- ✅ Better Rust ecosystem support (`rusqlite`, `diesel`)
- ✅ Single file backup (copy `.db` file)

**IndexedDB Consideration**:
- Only needed if frontend web apps need direct database access
- Our architecture: Frontend → Rust Backend → SQLite (no frontend DB needed)
- CEF's IndexedDB could be used for browser-specific frontend features (future)

## Implementation Plan

### Phase 1: Database Foundation ✅ **COMPLETE** (2025-12-02)
- [x] Evaluate database options
- [x] Research browser database patterns
- [x] Analyze BSV TypeScript wallet implementation
- [x] Design schema for all wallet data
- [x] Create database module structure (`rust-wallet/src/database/`)
- [x] Set up `rusqlite` dependency and connection management
- [x] Create migration system (schema versioning)

### Phase 2: Schema Implementation ✅ **COMPLETE** (2025-12-02)
- [x] Create database schema SQL files
- [x] Implement table creation/migration functions
- [x] Create Rust structs matching database tables
- [x] Implement database connection management
- [x] Add database initialization on wallet startup
- [x] Test schema creation and migrations

### Phase 3: Data Migration ✅ **COMPLETE** (2025-12-02)
- [x] Implement JSON → SQLite migration scripts
- [x] Migrate `wallet.json` → `wallets` and `addresses` tables
- [x] Migrate `actions.json` → `transactions` table
- [x] Test migration with real wallet data
- [x] Remove JSON file dependencies

### Phase 4: Core Functionality ✅ **COMPLETE** (2025-12-02)
- [x] Implement address CRUD operations
- [x] Implement transaction CRUD operations
- [x] Implement action storage/retrieval (replace JSON storage)
- [x] Update wallet handlers to use database instead of JSON
- [x] Remove JSON file dependencies

### Phase 5: UTXO Management ✅ **COMPLETE** (2025-12-02)
- [x] Implement UTXO table operations
- [x] Create UTXO sync service (fetch from WhatsOnChain)
- [x] Implement UTXO selection algorithm (uses database cache)
- [x] Mark UTXOs as spent when used in transactions
- [x] Background sync process (every 5 minutes with gap limit)
- [x] Detect new incoming UTXOs (pending address cache)
- [x] Retry logic with exponential backoff for API failures
- [x] Rate limiting protection

### Phase 6: BEEF/SPV Caching ⏳ **PENDING** (Next Phase)
- [ ] Implement `parent_transactions` table operations
- [ ] Implement `merkle_proofs` table operations
- [ ] Implement `block_headers` table operations
- [ ] Create background service to pre-fetch parent transactions
- [ ] Pre-fetch Merkle proofs for confirmed parent transactions
- [ ] Update `signAction()` to use cached data (fallback to API if missing)
- [ ] Implement proof refresh on reorg detection

### Phase 7: Performance & Optimization
- [ ] Add database indexes (based on query patterns)
- [ ] Implement query optimization
- [ ] Add connection pooling if needed
- [ ] Implement BLOB compression for large `raw_tx` data (optional)
- [ ] Cache frequently accessed data in memory
- [ ] Performance testing with large datasets

### Phase 8: Browser Database (Future)
- [ ] Design browser database schema (history, bookmarks, cache)
- [ ] Implement browser database initialization
- [ ] Migrate browser data if needed
- [ ] Integration with CEF frontend (if using IndexedDB)

### Phase 9: Cleanup & Documentation
- [ ] Remove all JSON file dependencies
- [ ] Update documentation
- [ ] Add database backup/restore utilities
- [ ] Performance benchmarks
- [ ] Security audit

## Database Security & Location

### Security Concerns

**Wallet Database**:
- ✅ **Location**: `%APPDATA%/HodosBrowser/wallet/wallet.db`
  - Windows: `C:\Users\{username}\AppData\Roaming\HodosBrowser\wallet\`
  - Protected by Windows user permissions
  - Not in temp/cache directories (persistent)
- ✅ **Encryption**: Sensitive data (mnemonic, private keys) already encrypted at application level
- ✅ **Access Control**: Only Rust wallet daemon can access (local socket/HTTP)
- ✅ **Backup**: Single `.db` file - easy to copy/backup securely

**Browser Database** (Future):
- ✅ **Location**: `%APPDATA%/HodosBrowser/browser/browser.db`
- ✅ **Isolation**: Separate from wallet data
- ⚠️ **Privacy**: Consider encryption for sensitive browsing data (history, cookies)

### File System Security

**Windows Best Practices**:
- Store in `%APPDATA%` (roaming profile) - persistent, user-specific
- NOT in `%TEMP%` or `%LOCALAPPDATA%\Temp` - cleared on reboot
- Use file permissions to restrict access
- Consider Windows Encrypted File System (EFS) for additional security

### Database File Locations

```
%APPDATA%/HodosBrowser/
├── wallet/
│   ├── wallet.db          ← Wallet SQLite database
│   ├── wallet.json        ← Legacy (migrate to DB)
│   └── actions.json       ← Legacy (migrate to DB)
├── browser/               ← Future browser database
│   └── browser.db
└── cache/                 ← Temporary cache files (separate from DB)
```

## Schema Design (Based on BSV Wallet Reference)

### Proposed Schema

**Core Tables**:

1. **`addresses`** - HD wallet addresses
   - `address_id`, `index`, `address`, `public_key`, `used`, `created_at`

2. **`utxos`** - Unspent Transaction Outputs
   - `utxo_id`, `address_id`, `txid`, `vout`, `satoshis`, `script` (hex), `spent_at`, `created_at`
   - **Indexes**: `(txid, vout)`, `address_id`, `spent_at IS NULL`

3. **`transactions`** - Transaction records
   - `transaction_id`, `txid`, `reference_number`, `raw_tx` (BLOB), `input_beef` (BLOB), `status`, `is_outgoing`, `satoshis`, `description`, `block_height`, `confirmations`, `created_at`, `updated_at`
   - **Indexes**: `txid`, `status`, `block_height`

4. **`proven_transactions`** - Transactions with Merkle proofs (BEEF/SPV)
   - `proven_tx_id`, `txid`, `height`, `index`, `merkle_path` (BLOB), `raw_tx` (BLOB), `block_hash`, `merkle_root`, `created_at`
   - **Indexes**: `txid`, `height`, `block_hash`

5. **`parent_transactions`** - Cached parent transaction data
   - `parent_tx_id`, `txid`, `raw_tx` (BLOB), `fetched_at`, `created_at`
   - **Indexes**: `txid`

6. **`merkle_proofs`** - Cached Merkle proof data (TSC/BUMP format)
   - `proof_id`, `txid`, `height`, `tx_index`, `merkle_path` (BLOB), `block_hash`, `fetched_at`, `created_at`
   - **Indexes**: `txid`, `(txid, tx_index)`, `block_hash`

7. **`block_headers`** - Cached block header data (for height resolution)
   - `block_id`, `block_hash`, `height`, `header_data` (BLOB), `created_at`
   - **Indexes**: `block_hash`, `height`

8. **`actions`** - Transaction history (migrated from actions.json)
   - `action_id`, `txid`, `reference_number`, `raw_tx` (BLOB), `status`, `is_outgoing`, `satoshis`, `description`, `labels` (JSON), `block_height`, `confirmations`, `created_at`, `updated_at`
   - **Indexes**: `txid`, `status`, `created_at`

**Relationships**:
- `utxos.address_id` → `addresses.address_id`
- `utxos.txid` → `transactions.txid` (parent transaction)
- `proven_transactions.txid` → `transactions.txid`
- `parent_transactions.txid` → `transactions.txid` (reference)

### Key Design Decisions

1. **Separate `proven_transactions`**: Store transactions with Merkle proofs separately (like BSV wallet)
2. **BLOB Storage**: Store `raw_tx`, `merkle_path` as BLOB (binary data)
3. **Cache Tables**: `parent_transactions`, `merkle_proofs`, `block_headers` for offline capability
4. **UTXO Tracking**: `spent_at` field for soft-delete (don't delete, mark as spent)

## Answered Questions

### 1. Database Choice: ✅ SQLite
- **Expected Scale**: Thousands of addresses, tens of thousands of UTXOs, millions of transactions (over time)
- **Multi-user**: Single-user wallet (one wallet per installation)
- **Backup**: Single file backup (`wallet.db`)

### 2. Schema Design: ✅ Proposed Above
- UTXOs: Separate table with indexes on `(txid, vout)` and `address_id`
- Parent Transactions: Cache table for efficient BEEF building
- Merkle Proofs: Separate table (can be linked to proven_transactions)
- Relationships: Foreign keys for data integrity

### 3. Sync Strategy: (To be implemented)
- How often: Every 5 minutes (configurable)
- Detect new: Compare fetched UTXOs with stored ones
- Invalidation: Mark `spent_at` when UTXO is spent in transaction
- Refresh proofs: Periodic background task, or on-demand when needed

### 4. Performance: (To be optimized)
- Indexes: `(txid, vout)` unique index for UTXOs, `txid` indexes everywhere
- Large data: BLOB storage for `raw_tx`, `merkle_path` (compression optional)
- Query patterns: Optimize for UTXO selection, transaction lookup, BEEF building

### 5. Migration: (To be planned)
- Migrate JSON → SQLite in Rust
- Maintain JSON fallback during transition
- Rollback: Keep JSON files as backup until migration verified

### 6. Data Integrity: (To be implemented)
- UTXO consistency: Use transactions (BEGIN/COMMIT) for atomic updates
- Reorgs: Invalidate proofs when block height changes (compare block_hash)
- Proof verification: Verify Merkle proof validity on read (optional check)

## Remaining Questions

1. **Sync Frequency**: What's optimal sync interval? (Start with 5 min)
2. **Proof Refresh**: When to refresh Merkle proofs? (On BEEF build, or proactive?)
3. **Cache Expiration**: How long to keep parent transactions/proofs? (Forever, or TTL?)
4. **Compression**: Should we compress `raw_tx` BLOBs? (Probably yes for large txs)
5. **Browser Database**: When to implement browser data storage? (Phase 2 or later?)

## Technology Stack

### Database Library: `rusqlite`

**Why `rusqlite`**:
- ✅ Official SQLite bindings for Rust
- ✅ Active maintenance and community
- ✅ Full SQLite feature support
- ✅ Transaction support (BEGIN/COMMIT/ROLLBACK)
- ✅ Prepared statements (SQL injection prevention)
- ✅ BLOB support for binary data

**Alternatives Considered**:
- `diesel`: ORM - more complex, may be overkill for our needs
- `sqlx`: Async SQL - not needed for our sync Rust backend
- Direct SQLite C API: More work, no clear benefit

### Migration Strategy

**Approach**: Gradual migration with backward compatibility

1. **Dual-Write Period**: Write to both JSON and database
2. **Dual-Read Period**: Read from database first, fallback to JSON
3. **Migration Script**: One-time migration of existing JSON data
4. **JSON Backup**: Keep JSON files as backup until migration verified
5. **Cleanup**: Remove JSON dependencies after verification

### Database Backup Strategy

**Manual Backup**:
- Users can copy `wallet.db` file directly
- Single file backup is simple and portable

**Automatic Backup** (Future):
- Option to enable automatic backups
- Store backups in `%APPDATA%/HodosBrowser/wallet/backups/`
- Keep last N backups (e.g., last 7 days)
- Compress old backups to save space

**Recovery**:
- Restore from backup: Replace `wallet.db` with backup file
- Rollback to JSON: Export database back to JSON (emergency fallback)

---

## Implementation Status

### ✅ **Completed Phases** (2025-12-02)

**Phase 1-4: COMPLETE**
- ✅ Database foundation and schema
- ✅ Data migration from JSON to SQLite
- ✅ Core functionality (addresses, transactions)
- ✅ UTXO management with background sync

**Key Achievements:**
- Database-backed wallet storage
- Fast balance checks (database cache)
- Background UTXO sync (every 5 minutes)
- New address detection (pending cache)
- Error handling with retry logic
- Change address privacy (new address per change)

### ⏳ **Next Steps** (Phase 5)

1. **Parent Transaction Caching**: Cache parent transactions for BEEF building
2. **TSC Proof Caching**: Cache Merkle proofs for SPV verification
3. **Block Header Caching**: Cache block headers for height resolution
4. **Performance Optimization**: Query optimization and indexing

---

## References

- **BSV TypeScript Wallet**: `reference/ts-brc100/src/storage/`
  - `StorageIdb.ts` - IndexedDB implementation
  - `StorageKnex.ts` - SQLite/MySQL implementation
  - `schema/` - Database schema definitions
- **Chrome Browser Storage**: `%LOCALAPPDATA%\Google\Chrome\User Data\Default\`
- **SQLite Documentation**: https://www.sqlite.org/docs.html
- **rusqlite Documentation**: https://docs.rs/rusqlite/
