# Codebase Concerns

**Analysis Date:** 2026-01-24

## Tech Debt

**Monolithic Handler File:**
- Issue: Single 8107-line file contains all HTTP endpoints
- Files: `rust-wallet/src/handlers.rs`
- Why: Rapid development, single-file convenience
- Impact: Difficult to navigate, merge conflicts likely, slow compile times
- Fix approach: Extract into module-based structure (`handlers/transaction.rs`, `handlers/identity.rs`, `handlers/certificate.rs`, etc.)

**Database Lock Panics (Critical):**
- Issue: 61 instances of `.lock().unwrap()` on `state.database`
- Files: `rust-wallet/src/handlers.rs` lines 225, 261, 319, 425, 706, 1000, 1309, 1499, 1572, 1604, 1645, 1691, 1731, 1786, 1825, 2034, 2254, 2349, 2936, 2975, 3039, 3188, 3325, 3563, 3819, 3842, 4509, 4532, 4573, 4612, 4653, 4724, 4734, 4747, 4863, 5072, 5092, 5110, 5412, 5508, 5559, 5619, 5640, 5912, 6086, 6177, 6273, 6322, 6785, 6851, 6917, 6976 (and more)
- Why: Quick error handling during development
- Impact: If mutex is poisoned (thread panic), all subsequent requests panic immediately - cascading failure
- Fix approach: Replace with `.map_err()` or recover from poisoned locks with `PoisonError` handling

**Message Handler Race Condition:**
- Issue: Callback overwrites in simultaneous requests
- Files: `frontend/src/bridge/initWindowBridge.ts` lines 124-142
- Why: Single global callback pattern (e.g., `window.onAddressGenerated`)
- Impact: Second concurrent address generation overwrites first callback, first caller never receives result
- Fix approach: Use message IDs with Map-based callback registry

**Large Complex Components:**
- Issue: Several components exceed 400+ lines with mixed concerns
- Files:
  - `frontend/src/pages/WalletOverlayRoot.tsx` (656 lines) - State management + rendering + business logic
  - `frontend/src/bridge/brc100.ts` (475 lines) - BRC-100 protocol implementation
  - `frontend/src/bridge/initWindowBridge.ts` (413 lines) - V8 bridge with 30+ duplicate pattern handlers
  - `frontend/src/components/TransactionForm.tsx` (379 lines) - Complex form with scattered validation
- Why: Rapid development, component evolution
- Impact: Hard to maintain, test, and reason about
- Fix approach: Extract hooks, utility functions, and smaller components

## Known Bugs

**Balance Auto-Refresh Disabled:**
- Symptoms: Balance doesn't update without manual refresh
- Files: `frontend/src/hooks/useBalance.ts` lines 138-145
- Trigger: Auto-refresh interval commented out
- Workaround: Manual refresh button works
- Root cause: Intentionally disabled (TODO comment suggests future implementation)
- Fix: Uncomment interval and add cleanup in useEffect

**Missing Request Timeout Protection:**
- Symptoms: Requests hang indefinitely if C++ process crashes
- Files: `frontend/src/hooks/useHodosBrowser.ts` line 61-64 (only address generation has 10s timeout)
- Trigger: C++ CEF process crashes or hangs
- Workaround: Refresh page
- Root cause: No timeout for most wallet methods (lines 149-231)
- Fix: Add Promise.race with timeout for all bridge calls

## Security Considerations

**Nonce Replay Protection Missing (Critical):**
- Risk: Unsigned requests can be replayed multiple times
- Files: `rust-wallet/src/handlers.rs` line 312 (TODO comment)
- Current mitigation: None
- Recommendations: Implement nonce tracking for BRC-100 `createAction`/`signAction` endpoints

**Incomplete BEEF Validation (Critical):**
- Risk: Merkle proof validation incomplete, block headers not verified
- Files: `rust-wallet/src/handlers.rs` lines 6685-6688 (multiple TODOs)
- Current mitigation: None (validation skipped)
- Recommendations: Complete merkle proof parsing, merkle root computation, block header verification against blockchain

**Unvalidated Server Signatures:**
- Risk: Authrite server responses not signature-verified
- Files: `rust-wallet/src/handlers/certificate_handlers.rs` line 2263 (TODO comment)
- Current mitigation: None (X-Authrite-Signature header ignored)
- Recommendations: Verify server response signature before processing certificate

**Excessive Debug Logging:**
- Risk: Sensitive data exposed in console logs
- Files:
  - `frontend/src/bridge/initWindowBridge.ts` line 18: Logs entire `window.hodosBrowser` object
  - `frontend/src/bridge/initWindowBridge.ts` lines 40-45: Logs BRC-100 auth request details (domain, method, endpoint)
  - `cef-native/src/handlers/simple_render_process_handler.cpp`: 59+ `console.log()` calls
- Current mitigation: None (debug logs accessible to website JavaScript)
- Recommendations: Remove or gate debug logs behind environment variable, never log sensitive data

**Hardcoded Network Configuration:**
- Risk: Cannot switch to testnet or other networks
- Files: `rust-wallet/src/handlers.rs` line 8102 (hardcoded `"mainnet"`)
- Current mitigation: None
- Recommendations: Add network configuration (mainnet/testnet) to settings

## Performance Bottlenecks

**Inefficient UTXO Fetching:**
- Problem: N+1 query pattern for UTXO retrieval
- Files: `frontend/src/pages/WalletOverlayRoot.tsx` line 236 (TODO comment)
- Measurement: Unknown (not profiled)
- Cause: Individual API calls per address instead of batch endpoint
- Improvement path: Add batch UTXO endpoint in Rust, single request from frontend

**No Virtualization for Long Lists:**
- Problem: Renders full certificate/action tables without virtualization
- Files: `frontend/src/pages/WalletOverlayRoot.tsx`
- Measurement: Performance degrades with 100+ entries
- Cause: No virtual scrolling implementation
- Improvement path: Add react-window or similar for large lists

**Disabled UTXO Background Sync:**
- Problem: UTXO cache not auto-updated
- Files: `rust-wallet/src/handlers.rs` line 1745 (calls `get_pending_utxo_check()` on every balance check)
- Measurement: Unknown (not profiled)
- Cause: Background sync interval not configured or disabled
- Improvement path: Configure background sync interval in `utxo_sync.rs`

## Fragile Areas

**CEF Lifecycle & Threading:**
- Files: `cef-native/cef_browser_shell.cpp`, `cef-native/src/handlers/simple_handler.cpp`
- Why fragile: CEF has strict threading rules, message loop timing critical
- Common failures: Deadlocks, crashes if CEF calls happen on wrong thread
- Safe modification: Do not change message loop, browser creation timing, or render-process handlers without deep CEF understanding
- Test coverage: None (manual testing only)

**V8 Context Injection:**
- Files: `cef-native/src/handlers/simple_render_process_handler.cpp`
- Why fragile: V8 context creation timing is critical, injection must happen in OnContextCreated()
- Common failures: window.hodosBrowser undefined, JavaScript errors
- Safe modification: Do not change injection timing or V8 object creation sequence
- Test coverage: None (manual testing only)

**Mutex-Heavy Rust State:**
- Files: `rust-wallet/src/main.rs` (AppState with Arc<Mutex<T>> for database, auth sessions, balance cache)
- Why fragile: 61+ `.lock().unwrap()` calls can poison mutex on panic
- Common failures: Mutex poisoning cascades to all handlers
- Safe modification: Replace .unwrap() with proper error handling
- Test coverage: None for concurrent access patterns

## Missing Critical Features

**P2SH and P2PK Script Support:**
- Problem: Only P2PKH addresses supported
- Files: `rust-wallet/src/handlers.rs` line 3942 (TODO comment)
- Current workaround: Users limited to P2PKH addresses only
- Blocks: Multi-sig wallets, advanced scripts
- Implementation complexity: Medium (add script parsing for P2SH and P2PK in `rust-wallet/src/script/`)

**User Input Signing:**
- Problem: Cannot sign user-provided inputs (only wallet-controlled inputs)
- Files: `rust-wallet/src/handlers.rs` line 4309 (TODO comment)
- Current workaround: None (feature unavailable)
- Blocks: Advanced transaction flows, CoinJoin, custom transactions
- Implementation complexity: Low (extend signing logic in handlers.rs)

**Dynamic Fee Rate Fetching:**
- Problem: Hardcoded 1 sat/byte fee rate
- Files: `rust-wallet/src/handlers.rs` line 149 (TODO: MAPI integration)
- Current workaround: Users pay fixed fee regardless of network conditions
- Blocks: Fee optimization, fast confirmation when needed
- Implementation complexity: Low (HTTP call to TAAL/GorillaPool MAPI fee quote endpoint)

**Certificate Type/Certifier Support:**
- Problem: Only single certifier supported
- Files: `rust-wallet/src/handlers/certificate_handlers.rs` line 225 (TODO comment)
- Current workaround: Users limited to one certificate type
- Blocks: Multiple identity providers, diverse certificate types
- Implementation complexity: Medium (database schema change required)

## Test Coverage Gaps

**No HTTP Endpoint Tests:**
- What's not tested: All 40+ HTTP endpoints in `rust-wallet/src/handlers.rs`
- Risk: Breaking changes to wallet API undetected until runtime
- Priority: High
- Difficulty to test: Low (actix-web has good test utilities)

**No Frontend Tests:**
- What's not tested: Entire React UI layer, hooks, components, pages
- Risk: UI regressions, broken user flows
- Priority: High
- Difficulty to test: Low (add vitest + React Testing Library)

**No C++ Tests:**
- What's not tested: V8 injection, HTTP interception, message routing, overlay management
- Risk: Browser functionality breaks silently
- Priority: Medium
- Difficulty to test: High (requires CEF test harness, mocking framework)

**No Integration Tests for Full Flows:**
- What's not tested: BRC-100 auth flow, transaction signing, certificate acquisition
- Risk: Protocol flows break across layer boundaries
- Priority: High
- Difficulty to test: Medium (requires multi-layer test setup)

## Dependency Versioning

**No Outdated Dependencies Detected:**
- Frontend dependencies appear current (React 19, TypeScript 5.8, Vite 6.3)
- Rust dependencies appear current (actix-web 4.9, secp256k1 0.28, rusqlite 0.30)
- No known security vulnerabilities from version inspection

## Missing Input Validation

**HTTP Handler Input Validation:**
- Problem: JSON deserialized without field constraint validation
- Files: `rust-wallet/src/handlers.rs` (throughout)
- Examples:
  - publicKey not validated as 33 bytes
  - Amounts not validated as non-negative
  - Addresses not validated for format
  - Transaction sizes not bounded
- Risk: Invalid data accepted, potential panics or incorrect behavior
- Fix: Add validation with `validator` crate or custom validators

**TypeScript Type Safety Violations:**
- Problem: 87 instances of `any` type, losing type safety
- Files:
  - `frontend/src/pages/WalletOverlayRoot.tsx` lines 46-47: `inputs?: any[], outputs?: any[]`
  - `frontend/src/bridge/initWindowBridge.ts` (throughout): `(event: any)`, callback data typed as `any`
  - `frontend/src/hooks/useHodosBrowser.ts` line 39: `(event: any)`
- Risk: Runtime type errors, no compile-time checking
- Fix: Define proper TypeScript interfaces for all data structures

---

*Concerns audit: 2026-01-24*
*Update as issues are fixed or new ones discovered*
