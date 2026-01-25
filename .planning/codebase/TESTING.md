# Testing Patterns

**Analysis Date:** 2026-01-24

## Test Framework

**Rust:**
- Framework: Native Rust `#[test]` attribute (no external framework)
- Location: `rust-wallet/tests/` directory (integration tests)
- Configuration: No explicit test config (uses cargo defaults)

**TypeScript/React:**
- Framework: None configured
- Status: No test files found, no testing dependencies in package.json
- No vitest, jest, @testing-library, or similar frameworks

**C++:**
- Framework: None configured
- Testing: Manual application testing only
- Note: README mentions `cef-native/tests/` but no files found

**Run Commands:**
```bash
# Rust
cd rust-wallet
cargo test                    # Run all tests
cargo test --release         # Run tests in release mode
cargo test [test_name]       # Run specific test

# TypeScript
# No test command configured

# C++
# No automated tests
```

## Test File Organization

**Rust Location:**
- Integration tests: `rust-wallet/tests/*.rs`
- Unit tests: Some inline in `rust-wallet/src/crypto/*_test.rs`
- Pattern: `[feature]_test.rs` naming convention

**Rust Structure:**
```
rust-wallet/
├── tests/
│   ├── interoperability_test.rs         # Protocol interop (209 lines)
│   ├── certificate_decryption_test.rs   # BRC-52 cert encryption (79 lines)
│   └── csr_json_serialization_test.rs   # Cert request JSON handling
└── src/
    └── crypto/
        └── aesgcm_custom_test.rs         # AES-GCM unit tests (101 lines)
```

**TypeScript:**
- No test files found in `frontend/src/`
- No `__tests__/` directories
- No `.test.ts` or `.spec.ts` files

**C++:**
- No test files found

## Test Structure

**Rust Suite Organization:**
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_certificate_roundtrip() {
        // Arrange
        let plaintext = vec![1, 2, 3, 4];
        let key = generate_test_key();

        // Act
        let encrypted = encrypt(&plaintext, &key).unwrap();
        let decrypted = decrypt(&encrypted, &key).unwrap();

        // Assert
        assert_eq!(plaintext, decrypted);
    }
}
```

**Patterns:**
- Use `#[test]` attribute for test functions
- Module-level `#[cfg(test)]` for test-only code
- Arrange/Act/Assert pattern implicit in code structure
- No explicit setup/teardown (tests are independent)

## Mocking

**Rust:**
- No mocking framework configured
- Tests use real implementations where possible
- External API calls not mocked (tests may hit blockchain APIs)

**TypeScript:**
- No mocking framework (no tests)

**C++:**
- No mocking framework (no tests)

## Fixtures and Factories

**Rust Test Data:**
```rust
// Factory pattern in test files
fn create_test_user() -> User {
    User {
        id: "test-id".to_string(),
        name: "Test User".to_string(),
        email: "test@example.com".to_string(),
    }
}

// Inline test data
let test_certificate = CertificateRequest {
    serialNumber: "test-serial".to_string(),
    // ...
};
```

**Location:**
- Factory functions defined in test files near usage
- No shared fixtures directory
- No central test utilities

## Coverage

**Requirements:**
- No enforced coverage target
- Coverage tracked for awareness only
- Focus on critical paths (cryptography, protocol compliance)

**Configuration:**
- Rust: cargo-tarpaulin or llvm-cov (not configured in project)
- TypeScript: No coverage tool configured

**View Coverage:**
```bash
# Rust (if tarpaulin installed)
cargo tarpaulin --out Html
open tarpaulin-report.html

# TypeScript
# No coverage configured
```

## Test Types

**Rust Unit Tests:**
- Scope: Test single function/module in isolation
- Location: Inline with source in some crypto modules (`src/crypto/aesgcm_custom_test.rs`)
- Mocking: None (use real implementations)
- Speed: Fast (<1s per test)

**Rust Integration Tests:**
- Scope: Test multiple modules together, verify protocol compliance
- Location: `rust-wallet/tests/`
- Examples:
  - `interoperability_test.rs` - Cross-protocol compatibility (BRC-42, BRC-43, BRC-52)
  - `certificate_decryption_test.rs` - End-to-end certificate encryption/decryption
  - `csr_json_serialization_test.rs` - JSON serialization of certificate requests
- Mocking: None (may hit external APIs)
- Speed: Moderate (depends on blockchain API calls)

**TypeScript/React:**
- No unit tests
- No integration tests
- No E2E tests
- Testing: Manual browser testing only

**C++:**
- No automated tests
- Testing: Manual application testing

## Common Patterns

**Rust Async Testing:**
```rust
#[tokio::test]
async fn test_async_operation() {
    let result = async_function().await;
    assert_eq!(result, expected_value);
}
```

**Rust Error Testing:**
```rust
#[test]
fn test_invalid_input() {
    let result = function_that_should_fail(invalid_input);
    assert!(result.is_err());

    // Or with specific error
    match result {
        Err(Brc42Error::InvalidPrivateKey(_)) => {},
        _ => panic!("Expected InvalidPrivateKey error"),
    }
}
```

**Rust Roundtrip Testing:**
```rust
#[test]
fn test_encryption_roundtrip() {
    let original = "test data".as_bytes();
    let encrypted = encrypt(original, &key).unwrap();
    let decrypted = decrypt(&encrypted, &key).unwrap();
    assert_eq!(original, decrypted.as_slice());
}
```

## Test Coverage Summary

**Rust:**
- **Cryptography**: Well-tested (AES-GCM, BRC-42, BRC-43, BRC-52)
  - `tests/interoperability_test.rs` - 209 lines
  - `tests/certificate_decryption_test.rs` - 79 lines
  - `src/crypto/aesgcm_custom_test.rs` - 101 lines
- **HTTP Handlers**: Not tested (8107 lines in handlers.rs, zero tests)
- **Database Repositories**: Not tested
- **Transaction Building**: Not tested
- **Background Services**: Not tested (utxo_sync.rs, cache_sync.rs)

**TypeScript/React:**
- **All components**: Not tested
- **All hooks**: Not tested
- **Bridge integration**: Not tested
- **UI flows**: Manual testing only

**C++:**
- **V8 injection**: Not tested
- **HTTP interception**: Not tested
- **Message routing**: Not tested
- **Overlay management**: Not tested
- **All features**: Manual testing only

## Testing Gaps & Recommendations

**Critical Gaps:**
1. **No HTTP endpoint tests** - 40+ endpoints in handlers.rs untested
2. **No database tests** - Repository operations untested
3. **No frontend tests** - Entire UI layer untested
4. **No integration tests for full flows** - BRC-100 auth flow, transaction signing, etc.
5. **No C++ tests** - Browser functionality untested

**Recommendations:**
1. **Rust**: Add integration tests for HTTP endpoints using actix-web test utilities
2. **TypeScript**: Add vitest or Jest with React Testing Library
3. **C++**: Add Google Test or Catch2 framework
4. **E2E**: Add Playwright or similar for full user flows
5. **Rust**: Expand coverage for repositories, transaction building, background services

## Build & Type-Check Commands

**Rust:**
```bash
cargo build                   # Fast compile check
cargo build --release         # Optimized build
cargo clippy                  # Lint
```

**TypeScript:**
```bash
npm run build                 # TypeScript compilation + Vite build (tsc -b && vite build)
npm run lint                  # ESLint check
npm run dev                   # Development server
npx tsc                       # Type-check only
```

**C++:**
```bash
cmake --build . --config Release
```

## Production Quality Standards

From `CLAUDE.md`:
- "Run `npx tsc` after changing code to prevent TypeScript compilation errors"
- "Build with a production-focused mindset. Do not take shortcuts"
- "This is production software handling real money; security and correctness take priority over development speed"

**Current State:**
- Type safety enforced by TypeScript and Rust compilers
- ESLint enforces React best practices
- Limited test coverage despite production-critical nature
- Heavy reliance on manual testing

---

*Testing analysis: 2026-01-24*
*Update when test patterns change*
