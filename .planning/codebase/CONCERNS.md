# Codebase Concerns

**Analysis Date:** 2026-01-20

## Tech Debt

**Excessive `.unwrap()` in Security-Critical Code:**
- Issue: 73+ instances of `.unwrap()` on database locks and JSON parsing
- Files: `rust-wallet/src/handlers.rs` (56), `rust-wallet/src/handlers/certificate_handlers.rs` (18)
- Why: Rapid development prioritized functionality over graceful error handling
- Impact: If mutex is poisoned or JSON malformed, wallet crashes instead of returning error
- Fix approach: Replace with `.map_err()` chains returning `HttpResponse::InternalServerError`

**Monolithic Handler Files:**
- Issue: Single handler files exceeding maintainable size
- Files: `rust-wallet/src/handlers.rs` (7,507 lines), `rust-wallet/src/handlers/certificate_handlers.rs` (3,298 lines)
- Why: Incremental feature additions without refactoring
- Impact: Difficult to test, maintain, and reason about; hidden coupling
- Fix approach: Split into domain-specific modules (`auth_handlers.rs`, `wallet_handlers.rs`, `certificate_handlers.rs`)

**Excessive `any` Types in Frontend:**
- Issue: 12+ instances of `any` type bypassing TypeScript safety
- Files: `frontend/src/App.tsx`, `frontend/src/pages/SettingsOverlayRoot.tsx`, `frontend/src/bridge/brc100.ts`
- Pattern: `(window as any)`, `data: any`, `Promise<any>`
- Impact: Runtime errors in production that TypeScript should catch
- Fix approach: Define proper interfaces in `frontend/src/types/`, use `declare global` for window extensions

## Known Bugs

**Race Condition in Subscription Updates (per CLAUDE.md):**
- Symptoms: User shows incorrect state briefly after operations
- Trigger: Fast navigation after Stripe-style checkout redirect
- Workaround: Eventually consistent (self-heals)
- Root cause: Webhook processing slower than user navigation
- Blocked by: Architectural decision on optimistic updates

**Settings Overlay Auth Response Incomplete:**
- Symptoms: BRC-100 authentication flow may not complete
- File: `frontend/src/pages/SettingsOverlayRoot.tsx` lines 56-64
- Trigger: User approves BRC-100 auth request
- Workaround: None documented
- Root cause: TODO comments indicate response handling not implemented

## Security Considerations

**Incomplete Merkle Proof Validation:**
- Risk: BEEF-formatted transactions not properly verified; attackers could inject invalid proofs
- Files: `rust-wallet/src/handlers.rs` lines 6247-6250
- Current mitigation: Logging that validation is stubbed
- Recommendations: Implement merkle root computation, block header fetching, transaction index verification before production

**Certificate Signature Validation Missing:**
- Risk: Certificates could be forged or tampered with in transit
- File: `rust-wallet/src/handlers/certificate_handlers.rs` line 2262
- TODO: "Verify server's response signature (X-Authrite-Signature header)"
- Recommendations: Implement ECDSA signature verification using secp256k1

**Nonce Validation Missing in Certificate Acquisition:**
- Risk: Replay attacks and nonce collision vulnerabilities
- File: `rust-wallet/src/handlers/certificate_handlers.rs` line 1441
- TODO: "Validate nonces (verify hash(clientNonce + serverNonce) == validationKey/serialNumber)"
- Recommendations: Implement nonce validation per BRC-53 spec

**CORS Allow-Any-Origin in Development:**
- Risk: In production, would allow any website to call wallet API
- File: `rust-wallet/src/main.rs`
- Current mitigation: Development only (not deployed)
- Recommendations: Restrict CORS to specific origins before production deployment

## Performance Bottlenecks

**UTXO Tag Filtering N+1 Pattern:**
- Problem: Likely filtering UTXOs in application code after fetching all
- File: `rust-wallet/src/handlers.rs` line 6986
- TODO: "Implement tag filtering in UtxoRepository"
- Measurement: Not profiled, but degrades with large UTXO sets
- Cause: Repository returns all UTXOs, filtering happens in handler
- Improvement path: Add SQL WHERE clause to `UtxoRepository` methods

**Heavy Console Logging in Frontend:**
- Problem: 304+ `console.log` statements throughout production code
- Files: `frontend/src/**/*.ts*`
- Measurement: Not profiled
- Cause: Debug logging left in production
- Improvement path: Remove or gate behind `DEBUG` environment variable

## Fragile Areas

**macOS BRC-100 Implementation:**
- Files: `cef-native/src/handlers/simple_handler.cpp`, `cef-native/src/handlers/simple_render_process_handler.cpp`
- Why fragile: 9+ TODOs indicate entire BRC-100 flow is stubbed on macOS
- Common failures: Authentication completely non-functional
- Safe modification: Must implement all TODOs together; partial implementation will break flow
- Test coverage: None - manual testing only

**Certificate Acquisition Nonce Logic:**
- File: `rust-wallet/src/handlers/certificate_handlers.rs` lines 1459-1465
- Why fragile: Complex nonce handling with subtle timing requirements
- Common failures: Nonce mismatch errors, replay vulnerabilities
- Safe modification: Must understand TypeScript SDK's `Peer.toPeer()` behavior
- Test coverage: None for certificate handlers

**Global State in CEF Layer:**
- Files: `cef-native/cef_browser_shell.cpp`, `cef-native/src/core/HttpRequestInterceptor.cpp`
- Why fragile: Multiple global window handles without synchronization
- Pattern: `g_pendingAuthRequest`, `g_hwnd`, overlay HWNDs
- Common failures: Race conditions if overlays created/destroyed concurrently
- Test coverage: None

## Scaling Limits

**SQLite Database:**
- Current capacity: Suitable for single-user wallet with thousands of transactions
- Limit: Concurrent writes may cause lock contention
- Symptoms at limit: Slow operations, potential SQLITE_BUSY errors
- Scaling path: Acceptable for desktop app; no action needed

**In-Memory Pending Transactions:**
- File: `rust-wallet/src/handlers.rs` lines 2452-2453
- Current capacity: `Lazy<StdMutex<HashMap<...>>>`
- Limit: Memory unbounded if transactions never cleaned
- Symptoms at limit: Memory growth over long sessions
- Scaling path: Add TTL-based cleanup for pending transactions

## Dependencies at Risk

**No Critical Dependency Risks Detected:**
- Rust dependencies: actix-web 4.9, tokio 1, secp256k1 0.28, sha2 0.10 are current
- Frontend: React 19.1.0, TypeScript 5.8.3, Vite 6.3.5 are current
- Recommendation: Run `cargo audit` and `npm audit` periodically

## Missing Critical Features

**Dynamic Fee Estimation:**
- Problem: Fee rate hardcoded to 1 sat/byte (DEFAULT_SATS_PER_KB = 1000)
- File: `rust-wallet/src/handlers.rs` line 148
- Current workaround: Users may overpay or underpay for transactions
- Blocks: Optimal fee selection, network responsiveness
- Implementation complexity: Medium (integrate MAPI from GorillaPool)

**macOS HTTP Request Interception:**
- Problem: Wallet API calls don't route to Rust backend on macOS
- File: `cef-native/src/handlers/simple_handler.cpp` line 2240
- Current workaround: macOS users cannot use wallet features
- Blocks: macOS production release
- Implementation complexity: High (platform-specific HTTP handling)

**Script Type Support:**
- Problem: Only P2PKH supported; complex scripts fail
- Files: `rust-wallet/src/handlers.rs` lines 3870, 4011
- TODO: "Add P2SH, P2PK, and other script types"
- Blocks: Multi-sig, smart contract interactions
- Implementation complexity: Medium

## Test Coverage Gaps

**Handler Functions Untested:**
- What's not tested: `rust-wallet/src/handlers.rs` (7507 lines), `rust-wallet/src/handlers/certificate_handlers.rs` (3298 lines)
- Risk: Regressions in API endpoints undetected
- Priority: High
- Difficulty to test: Need to mock database and HTTP clients

**No Integration Tests:**
- What's not tested: Full BRC-100 auth flow, BEEF verification, HTTP interception + wallet routing
- Risk: Cross-layer bugs only caught in manual testing
- Priority: High
- Difficulty to test: Requires test harness spanning Rust, CEF, and React

**Frontend Untested:**
- What's not tested: All React components, hooks, bridge code
- Risk: UI regressions, broken navigation
- Priority: Medium
- Difficulty to test: Need to configure Vitest or Jest with CEF mocks

**BRC-2 Encryption Missing Test Vectors:**
- What's not tested: AES-GCM encryption implementation against spec
- File: `rust-wallet/src/crypto/brc2.rs` line 379
- TODO: "Add test vectors from BRC-2 spec when available"
- Risk: Encryption could be incompatible with other implementations
- Priority: Medium

---

*Concerns audit: 2026-01-20*
*Update as issues are fixed or new ones discovered*
