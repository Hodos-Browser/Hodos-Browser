# Codebase Concerns

**Analysis Date:** 2026-01-28

## Tech Debt

### SPV Proof Verification Not Implemented

**Issue:** BEEF (Background Evaluation Extended Format) transaction verification is only partially implemented. Merkle root validation, block header verification, and transaction index validation are stubbed with TODOs.

**Files:** `rust-wallet/src/handlers.rs` (lines 6685-6688)

**Impact:** Transactions processed via BEEF may not be properly verified on-chain. Attackers could potentially submit invalid BEEF formats that bypass proper SPV validation. Current code only warns about structure issues but doesn't reject invalid proofs.

**Fix approach:**
1. Implement merkle root computation from BUMP merkle proof nodes
2. Fetch and cache block headers from WhatsOnChain API
3. Validate merkle root matches block header
4. Verify transaction position against proof position
5. Add unit tests with known valid BEEF formats

**Priority:** HIGH - Security-critical for transaction validation

### Dynamic Fee Rate Not Implemented

**Issue:** Fee calculation uses hardcoded DEFAULT_SATS_PER_KB (1000 sat/kb) instead of querying MAPI (Merchant API) for current network rates.

**Files:** `rust-wallet/src/handlers.rs` (lines 149-170)

**Impact:** Fees may be overestimated or underestimated depending on actual network conditions. Transactions could be slow to confirm if miners' minimum fees change, or overpaid if rates drop.

**Fix approach:**
1. Implement FeeRateCache struct with 1-hour TTL
2. Add async fn fetch_mapi_fee_quote() calling TAAL or GorillaPool MAPI endpoints
3. Store cached rate in AppState
4. Fall back to DEFAULT_SATS_PER_KB on API error
5. Refresh cache on configurable interval

**Priority:** MEDIUM - Quality of life, not security-critical

### Certificate Database Transactions Not Atomic

**Issue:** Certificate insertion with fields is not transactional. Connection is immutable borrow (&self), preventing explicit transaction management.

**Files:** `rust-wallet/src/database/certificate_repo.rs` (line 36)

**Impact:** If certificate insertion succeeds but field insertion fails, certificate is orphaned in database with no fields. Could cause data consistency issues.

**Fix approach:**
1. Change CertificateRepository to take &mut Connection
2. Use conn.transaction() for atomic operations
3. Ensure all certificate tests verify field insertion completes
4. Add migration to clean up orphaned certificates without fields

**Priority:** MEDIUM - Data integrity risk

### User Input Script Types Not Fully Supported

**Issue:** Transaction signing and script parsing only supports P2PKH. P2SH, P2PK, and other script types are not implemented.

**Files:** `rust-wallet/src/handlers.rs` (line 3942)

**Impact:** Wallet cannot handle incoming transactions to non-P2PKH addresses. Users cannot receive payments in different script formats.

**Fix approach:**
1. Implement P2SH unlocking script handling
2. Add P2PK input support
3. Extend script parser to recognize script types
4. Add integration tests for each script type
5. Document which script types are supported

**Priority:** MEDIUM - Feature completeness

### Nonce Replay Attack Prevention Not Implemented

**Issue:** BRC-103/104 authentication uses random nonces without tracking. No replay attack prevention in place. Comment indicates high-volume servers need HMAC-based nonces per BRC-103 Section 6.2.

**Files:** `rust-wallet/src/handlers.rs` (line 312)

**Impact:** Theoretical attack: Attacker could capture valid authentication request and replay it to authenticate as the victim. Low practical risk for low-volume wallet (single user), but violates BRC-103 spec for production systems.

**Fix approach:**
1. Implement nonce storage in AuthSessionManager with timestamp
2. Track used nonces with expiry (configurable, ~5 minutes standard)
3. Reject any request with previously-used nonce
4. Consider HMAC-based nonces (BRC-103 Section 6.2) for future high-volume deployments
5. Add unit tests for nonce tracking

**Priority:** LOW - Single-user wallet has limited replay surface, but should be fixed for completeness

### Certificate Placeholder Txid Workaround

**Issue:** Certificates acquired via issuance protocol (not from transaction) have placeholder TXID generated from SHA256(type + serial_number). This is a workaround for database NOT NULL constraint.

**Files:** `rust-wallet/src/database/certificate_repo.rs` (lines 47-57)

**Impact:** Cannot distinguish between actual on-chain certificates and locally-issued certificates. Placeholder TXIDs are not real and could collide.

**Fix approach:**
1. Make certificate_txid nullable in schema
2. Add migration to allow NULL certificate_txid
3. Update queries to handle NULL txid for local certificates
4. Remove placeholder generation logic
5. Update certificate tracking to distinguish "issued" vs "acquired" certificates

**Priority:** MEDIUM - Schema clarification needed

### Balance Auto-Refresh Disabled

**Issue:** Balance auto-refresh every 30 seconds is commented out in frontend with "DISABLED FOR DEBUGGING" note.

**Files:** `frontend/src/hooks/useBalance.ts` (lines 138-145)

**Impact:** Balance does not update automatically. Users must manually refresh to see latest balance after receiving funds.

**Fix approach:**
1. Re-enable useEffect interval
2. Add configurable refresh rate (environment variable or config)
3. Implement exponential backoff on API errors
4. Add request deduplication to prevent concurrent requests
5. Test with real wallet receiving transactions

**Priority:** MEDIUM - User experience feature

## Known Bugs

### Certificate Listing Placeholder Total Count

**Issue:** Certificate repository returns placeholder total count (len as i64) instead of actual total from database.

**Files:** `rust-wallet/src/handlers/certificate_handlers.rs` (line 253)

**Impact:** Pagination metadata is incorrect. UI showing "10 of 10" when there are actually 100 certificates.

**Fix approach:**
1. Query database for COUNT(*) of non-deleted certificates
2. Return actual total in response
3. Update tests to verify total accuracy
4. Consider caching total for performance

**Priority:** MEDIUM - Correctness issue

### Missing Certificate Field Recovery

**Issue:** When wallet is backed up and recovered from mnemonic, certificates acquired via protocol are not recovered. They're stored in database but not in blockchain/ledger.

**Files:** `rust-wallet/src/recovery.rs` (likely incomplete)

**Impact:** Users who lose wallet data and recover from mnemonic lose all acquired certificates.

**Fix approach:**
1. Implement certificate export to backup
2. Include certificate data in mnemonic recovery package
3. Reconstruct certificate_fields table on recovery
4. Verify certificate signatures still valid after recovery
5. Add integration test for certificate persistence through recovery cycle

**Priority:** MEDIUM - Data loss risk

## Security Considerations

### Private Key Access via Global Variables

**Issue:** Master private key is fetched from database and kept in memory during request handling. No explicit zero-out of sensitive memory.

**Files:** `rust-wallet/src/handlers.rs` (lines 320-329)

**Impact:** Master private key could potentially be exposed in core dumps or memory forensics. Rust's ownership model provides some protection, but active key material should be cleared after use.

**Current mitigation:** Rust's Vec/String drop() clears memory on scope exit; secp256k1 library should handle SecretKey cleanup.

**Recommendations:**
1. Use `zeroize` crate for explicit key material clearing
2. Minimize time master key is in memory (only use when signing)
3. Consider key derivation on-demand rather than storage
4. Document security assumptions in code comments
5. Add security audit checklist for key handling

**Priority:** MEDIUM - Defense in depth, Rust mitigates many issues

### HTTP Request Interception Domain Whitelist File Handling

**Issue:** Domain whitelist is file-based JSON stored at `%APPDATA%/HodosBrowser/wallet/domainWhitelist.json`. File I/O is done without proper error handling in hot path.

**Files:** `cef-native/src/core/HttpRequestInterceptor.cpp` (lines 55-84)

**Impact:** Race conditions if multiple threads access whitelist simultaneously. File corruption could allow/deny unexpected domains. No atomic file updates.

**Current mitigation:** File reads are wrapped in try-catch; write-then-close (not atomic).

**Recommendations:**
1. Implement atomic file writes (write to temp file, rename)
2. Add file locking on Windows (LockFileEx)
3. Cache whitelist in memory with refresh on change
4. Add whitelist integrity validation on load
5. Log all whitelist modifications

**Priority:** MEDIUM - Operational robustness

### Database Backup Contains Sensitive Data

**Issue:** Database backup feature stores entire wallet.db including master private key and certificate field data.

**Files:** `rust-wallet/src/backup.rs`

**Impact:** If backup is not encrypted, attacker with file access could extract private keys and certificates.

**Current mitigation:** Backup stored in secure APPDATA directory with Windows ACLs.

**Recommendations:**
1. Implement AES-GCM encryption for backup files
2. Use password or hardware-key-derived encryption key
3. Add backup integrity check (HMAC)
4. Document backup security assumptions
5. Warn users about backup security in UI

**Priority:** HIGH - Backup is security-critical

### HTTP Interception Bypass Potential

**Issue:** HTTP interceptor routes wallet API calls to Rust backend. If routing logic has bugs, requests could bypass wallet security or reach unexpected endpoints.

**Files:** `cef-native/src/core/HttpRequestInterceptor.cpp` (lines 186-200)

**Impact:** Malicious website could craft requests that bypass wallet security checks or trigger unintended behavior.

**Current mitigation:** Domain whitelist check before forwarding; BRC-100 auth uses separate approval modal.

**Recommendations:**
1. Whitelist allowed wallet endpoints explicitly (not exclude list)
2. Validate request body format before forwarding
3. Add rate limiting per domain
4. Log all intercepted requests with domain/endpoint
5. Add integration tests for interception routing

**Priority:** HIGH - Attack surface

## Performance Bottlenecks

### Balance Calculation Scans All UTXOs

**Issue:** getBalance endpoint calculates balance by iterating all UTXOs for all addresses. No index optimization.

**Files:** `rust-wallet/src/database/utxo_repo.rs` and `rust-wallet/src/handlers.rs`

**Impact:** With thousands of UTXOs, balance calculation could become slow. UI freezes waiting for balance.

**Current capacity:** Tested with ~100 UTXOs, no performance degradation reported yet.

**Improvement path:**
1. Add cached_balance column to addresses table
2. Update cache on UTXO insert/spend
3. Query cached_balance instead of computing
4. Add periodic cache verification job
5. Benchmark with 10k+ UTXO scenario

**Priority:** MEDIUM - Future-proofing, not urgent

### Certificate Listing Scans All Certificates

**Issue:** listCertificates retrieves all certificates from database without pagination in current query.

**Files:** `rust-wallet/src/handlers/certificate_handlers.rs` (around line 225)

**Impact:** With hundreds of certificates, response could be large and slow.

**Fix approach:**
1. Add LIMIT/OFFSET pagination parameters
2. Sort by acquired_at DESC for most recent first
3. Implement cursor-based pagination for consistency
4. Cache certificate count for UI preview
5. Benchmark with 1000+ certificates

**Priority:** MEDIUM - Scales with certificate usage

### Merkle Proof Validation No Caching

**Issue:** SPV proof validation fetches block headers from external API repeatedly for same block.

**Files:** `rust-wallet/src/handlers.rs` (would be in SPV verification implementation)

**Impact:** When processing multiple transactions in same block, header is fetched multiple times.

**Fix approach:**
1. Cache block headers by block_height with expiry
2. Store in block_headers table (schema exists)
3. Use in-memory cache for recent blocks (LRU)
4. Query cache before API call

**Priority:** LOW - Currently N/A (verification not yet implemented)

## Fragile Areas

### CEF Overlay Lifecycle Management

**Files:** `cef-native/cef_browser_shell.cpp` (lines 43-53 globals, 124-148 shutdown)

**Why fragile:** Global HWND pointers for 5 overlays maintained manually. Any new overlay requires:
1. Adding new global HWND declaration
2. Create window in WndProc
3. Add to shutdown cleanup
4. Handle in WM_MOVE and WM_SIZE
5. Register handler in simple_handler.cpp

Missing any step causes memory leaks or crash on shutdown.

**Safe modification:**
1. Encapsulate overlay management in OverlayManager class
2. Use map<string, HWND> instead of individual globals
3. Register overlays with automatic WM_MOVE/WM_SIZE handling
4. Verify shutdown cleanup completeness before committing

**Test coverage:** Minimal - shutdown cleanup not tested; manual verification only

**Priority:** HIGH - Affects stability

### V8 Injection and Message Passing

**Files:** `cef-native/src/handlers/simple_render_process_handler.cpp` (CefMessageSendHandler)

**Why fragile:** V8 injection happens in isolated render process context. Any change to message format breaks all JavaScript->C++ calls.

**Safe modification:**
1. Version message format with explicit version number
2. Add migration layer for old/new formats
3. Update all V8 handler tests before modifying
4. Test in both header and overlay processes
5. Document message contract in header comment

**Test coverage:** Minimal - V8 injection not unit tested; manual browser testing only

**Priority:** HIGH - Core IPC mechanism

### Database Migration System

**Files:** `rust-wallet/src/database/migrations.rs`, `rust-wallet/src/database/connection.rs`

**Why fragile:** Schema version tracking with single version number. Cannot roll back selectively or apply migrations conditionally.

**Safe modification:**
1. Add migration_applied table tracking individual migrations
2. Implement per-migration versioning
3. Add rollback support (inverse migration)
4. Test migration on existing database before deploying
5. Add migration backups before applying

**Test coverage:** Minimal - migrations tested only on fresh database; existing DB upgrade path not well tested

**Priority:** MEDIUM - Schema changes need careful planning

### Certificate Repository Transaction Atomicity

**Files:** `rust-wallet/src/database/certificate_repo.rs` (lines 22-80)

**Why fragile:** Insert certificate, then insert fields. If second insert fails, orphaned certificate remains. No transaction support due to immutable borrow.

**Safe modification:**
1. Change to mutable borrow
2. Wrap in explicit transaction
3. Add test that simulates field insert failure
4. Verify cleanup behavior

**Test coverage:** Partial - happy path tested; failure cases not tested

**Priority:** MEDIUM - Data consistency risk

## Scaling Limits

### Single-Process Wallet Bottleneck

**Current capacity:** Single Actix-web process on port 3301 handling all requests.

**Limit:** Cannot handle >~500 req/sec per thread without queueing. With 4 worker threads, realistic limit ~200 concurrent users.

**Scaling path:**
1. Profile with load testing (Apache Bench or similar)
2. Identify bottleneck (UTXO queries, crypto, I/O)
3. Consider connection pooling if database-bound
4. Consider UTXO indexing or sharding if computation-bound
5. Document performance characteristics

**Priority:** LOW - Single-user wallet; future-proofing only

### SQLite Concurrency Limit

**Current capacity:** SQLite serializes writes; reader-writer lock prevents concurrent access.

**Limit:** With multiple wallets (future feature), SQLite write throughput ~10-50 writes/sec depending on journal mode.

**Scaling path:**
1. Profile real workload (measure actual transactions/sec)
2. Consider moving to PostgreSQL if approaching limit
3. Implement write coalescing (batch transactions)
4. Add connection pooling with rusqlite_bundle

**Priority:** LOW - Not hit in single-user scenario

### Memory Usage

**Current capacity:** No memory limits enforced. Balance cache, UTXO cache, transaction cache all grow unbounded.

**Limit:** With thousands of addresses and UTXOs, could exceed available memory.

**Scaling path:**
1. Add configurable cache size limits
2. Implement LRU eviction
3. Profile memory usage under typical load
4. Add memory monitoring/logging

**Priority:** LOW - Not yet a problem

## Dependencies at Risk

### CEF 136 Browser Binaries

**Risk:** CEF binaries are externally maintained and large (500MB+). Need manual download/update.

**Impact:** If CEF project deprecates version 136 or hosting changes, build pipeline breaks.

**Migration plan:**
1. Lock CEF version in CMakeLists.txt
2. Mirror CEF binaries to internal storage
3. Monitor CEF releases for security updates
4. Plan upgrade path to CEF 137+ (test breaking changes first)

**Priority:** MEDIUM - Infrastructure risk

### secp256k1 Crypto Library

**Risk:** secp256k1 FFI binding (via secp256k1 crate) is security-critical. Any breaking change requires careful review.

**Impact:** Signing failure, key derivation errors, or cryptographic weakness if library updates introduce bugs.

**Current mitigation:** Cargo.lock locks version; Rust prevents memory safety issues.

**Recommendations:**
1. Pin secp256k1 to specific tested version
2. Review upgrade changelog before updating
3. Add integration test for known test vectors (BIP-340 for Schnorr)
4. Test signing/verification round-trip on each update

**Priority:** MEDIUM - Cryptographic security

### rusqlite Database Bindings

**Risk:** rusqlite FFI to SQLite could have memory safety issues (Rust prevents many, but not all).

**Impact:** Database corruption, query injection (mitigated by parameterization), or data loss.

**Current mitigation:** Parameterized queries prevent SQL injection; Rust prevents buffer overflows.

**Recommendations:**
1. Use parameterized queries everywhere (already done)
2. Audit execute/query_row calls for proper error handling
3. Test with malformed/corrupted database files
4. Add database integrity check on startup

**Priority:** LOW - Mitigated by Rust and parameterized queries

## Test Coverage Gaps

### No Tests for Certificate Verification

**What's not tested:** Certificate signature validation, selective disclosure field recovery, certificate chain validation.

**Files:** `rust-wallet/src/certificate/verifier.rs`, `rust-wallet/src/handlers/certificate_handlers.rs`

**Risk:** Certificate security relies on untested code. Could accept forged/invalid certificates.

**Priority:** HIGH - Security-critical

### No Tests for SPV Proof Validation

**What's not tested:** Merkle proof computation, block header verification, SPV validation logic.

**Files:** `rust-wallet/src/handlers.rs` (SPV TODOs)

**Risk:** Validation logic not yet implemented; when added, must be thoroughly tested.

**Priority:** HIGH - Security-critical

### No Tests for BRC-42 Child Key Derivation

**What's not tested:** Child key derivation with various counterparties, cross-app identity keys.

**Files:** `rust-wallet/src/crypto/brc42.rs`

**Risk:** If child key derivation has bugs, different apps could see same identity key (privacy failure) or fail to derive correct keys (authentication failure).

**Priority:** HIGH - Security and privacy-critical

### No Tests for HTTP Interception Routing

**What's not tested:** HTTP interceptor domain whitelist bypass attempts, request routing correctness.

**Files:** `cef-native/src/core/HttpRequestInterceptor.cpp`

**Risk:** Routing bugs could allow malicious requests or bypass security checks.

**Priority:** HIGH - Attack surface

### Limited Integration Tests

**What's not tested:** Full lifecycle: create wallet → derive addresses → receive UTXO → spend → broadcast → verify SPV.

**Files:** Multiple (end-to-end)

**Risk:** Component interactions could fail in production despite passing unit tests.

**Priority:** MEDIUM - Validation before release

### No Fuzzing

**What's not tested:** Malformed inputs to protocol handlers, invalid certificates, corrupted BEEF formats.

**Risk:** Crash or unexpected behavior on invalid input.

**Priority:** MEDIUM - Robustness

---

*Concerns audit: 2026-01-28*
