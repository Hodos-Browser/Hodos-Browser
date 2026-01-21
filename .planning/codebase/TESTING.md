# Testing Patterns

**Analysis Date:** 2026-01-20

## Test Framework

**Rust:**
- Runner: Built-in `#[test]` macro (standard library)
- Assertion Library: `assert!`, `assert_eq!`, `assert_ne!`
- No additional test dependencies

**TypeScript:**
- Runner: **Not configured** - No Vitest, Jest, or other framework
- Status: Manual testing via browser interaction only

**C++:**
- Runner: **Not configured** - Manual integration testing only

**Run Commands:**
```bash
# Rust tests
cargo test                              # Run all tests
cargo test -- --nocapture               # With output
cargo test brc42                        # Filter by name

# TypeScript (linting only, no tests)
npm run lint                            # ESLint check
npm run build                           # tsc + vite build

# C++ (no automated tests)
# Manual testing via running the browser
```

## Test File Organization

**Rust:**
- Location: Inline within source files using `#[cfg(test)]` modules
- No separate `tests/` directory for unit tests
- Pattern: Tests co-located with implementation

**Structure:**
```
rust-wallet/src/
  crypto/
    brc42.rs           # Contains #[cfg(test)] mod tests
    keys.rs            # Contains #[cfg(test)] mod tests
    aesgcm_custom_test.rs  # Dedicated test file
  transaction/
    sighash.rs         # Contains #[cfg(test)] mod tests
    mod.rs             # Contains #[cfg(test)] mod tests
  certificate/
    selective_disclosure.rs  # Contains #[cfg(test)] mod tests
    test_utils.rs      # Shared test utilities
  script/
    pushdrop_tests.rs  # Dedicated test file
```

**TypeScript:**
- No `__tests__/` directory
- No `*.test.ts` or `*.spec.tsx` files
- No test utilities or fixtures

## Test Structure

**Rust Suite Organization:**
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_private_key_derivation_vector_1() {
        // Arrange: Set up test data from BRC-42 spec
        let sender_pubkey = hex::decode("...").unwrap();
        let recipient_privkey = hex::decode("...").unwrap();
        let invoice_number = "2-3241645161d8-satoshis coins";
        let expected = hex::decode("...").unwrap();

        // Act: Call function under test
        let derived = derive_child_private_key(
            &recipient_privkey,
            &sender_pubkey,
            invoice_number
        ).unwrap();

        // Assert: Verify result
        assert_eq!(derived, expected);
    }

    #[test]
    fn test_shared_secret_symmetry() {
        // Test that ECDH is symmetric
        // ...
    }
}
```

**Patterns:**
- Use test vectors from BRC specifications
- Name tests descriptively: `test_{function}_{scenario}`
- Group related tests in same `mod tests` block
- Use `unwrap()` in tests (panics are expected on failure)

## Mocking

**Rust:**
- No mocking framework (like `mockall`)
- Tests use real implementations
- External API calls not mocked (UTXO fetcher, etc.)

**What's Tested Without Mocks:**
- Cryptographic algorithms (pure functions)
- Transaction signing (deterministic)
- Certificate parsing (isolated logic)

**What Would Need Mocks (Not Currently Tested):**
- HTTP handlers (database, external APIs)
- UTXO fetching (WhatsOnChain API)
- File system operations

## Fixtures and Factories

**Test Data:**
```rust
// BRC-42 test vectors from specification
#[cfg(test)]
mod tests {
    // Test vector 1 from BRC-42 spec
    const SENDER_PUBKEY_1: &str = "...";
    const RECIPIENT_PRIVKEY_1: &str = "...";
    const EXPECTED_DERIVED_1: &str = "...";
}
```

**Location:**
- Test vectors: Inline in test modules
- Shared utilities: `rust-wallet/src/certificate/test_utils.rs`
- No fixtures directory

## Coverage

**Requirements:**
- No enforced coverage target
- No coverage reporting configured
- Focus on security-critical code (crypto, signing)

**Configuration:**
- Rust: Could use `cargo tarpaulin` but not configured
- TypeScript: No coverage tool

**Current Coverage (Estimated):**
- `rust-wallet/src/crypto/` - Partially tested (BRC-42, keys, signing)
- `rust-wallet/src/handlers.rs` - **Not tested** (7500+ lines)
- `rust-wallet/src/handlers/certificate_handlers.rs` - **Not tested** (3300 lines)
- `frontend/src/` - **Not tested**
- `cef-native/` - **Not tested**

## Test Types

**Unit Tests (Rust):**
- Scope: Single function/module in isolation
- Mocking: None (pure functions)
- Speed: Fast (< 1s per test)
- Examples: `crypto/brc42.rs`, `crypto/keys.rs`, `transaction/sighash.rs`

**Integration Tests:**
- **Not implemented**
- Would test: Full BRC-100 auth flow, HTTP handlers with database
- Location: Would go in `rust-wallet/tests/`

**E2E Tests:**
- **Not implemented**
- Manual testing by running CEF browser
- Note from CLAUDE.md: "User runs the browser to test"

## Common Patterns

**Async Testing:**
```rust
// Rust async tests would use #[tokio::test]
// Currently not used in codebase
#[tokio::test]
async fn test_async_function() {
    let result = async_function().await;
    assert!(result.is_ok());
}
```

**Error Testing:**
```rust
#[test]
fn test_invalid_input_returns_error() {
    let result = parse_invalid_data(&[]);
    assert!(result.is_err());
}

// Or for specific error types
#[test]
fn test_specific_error() {
    let result = derive_key(&invalid_key, &pubkey, "");
    match result {
        Err(Brc42Error::InvalidKeyLength) => (),
        _ => panic!("Expected InvalidKeyLength error"),
    }
}
```

**Test Vector Pattern:**
```rust
#[test]
fn test_private_key_derivation_vector_1() {
    // Data from official BRC-42 specification
    let sender_pubkey = hex::decode(
        "0293029218..."
    ).unwrap();

    let recipient_privkey = hex::decode(
        "e8e57e7e4f..."
    ).unwrap();

    let invoice_number = "2-3241645161d8-satoshis coins";

    let expected_privkey = hex::decode(
        "3ef9e1aacc..."
    ).unwrap();

    let derived = derive_child_private_key(
        &recipient_privkey,
        &sender_pubkey,
        invoice_number
    ).unwrap();

    assert_eq!(derived, expected_privkey);
}
```

## Files with Tests

**Confirmed test coverage:**
- `rust-wallet/src/crypto/brc42.rs` - 5 tests (BRC-42 key derivation)
- `rust-wallet/src/crypto/keys.rs` - Multiple tests (key utilities)
- `rust-wallet/src/transaction/sighash.rs` - Transaction signing tests
- `rust-wallet/src/transaction/mod.rs` - Transaction type tests
- `rust-wallet/src/certificate/selective_disclosure.rs` - Certificate tests
- `rust-wallet/src/crypto/aesgcm_custom_test.rs` - AES-GCM tests
- `rust-wallet/src/script/pushdrop_tests.rs` - Script parsing tests

**No test coverage:**
- `rust-wallet/src/handlers.rs` (7507 lines)
- `rust-wallet/src/handlers/certificate_handlers.rs` (3298 lines)
- `rust-wallet/src/beef.rs` (limited)
- All frontend code
- All C++ code

## Missing/Gaps

| Gap | Impact | Priority |
|-----|--------|----------|
| No frontend test framework | Cannot automate UI/hook testing | High |
| No integration tests | Cross-layer bugs undetected | High |
| Handler functions untested | Regressions in API endpoints | High |
| No E2E framework | Full flow testing is manual | Medium |
| No coverage reporting | Unknown test coverage | Low |

---

*Testing analysis: 2026-01-20*
*Update when test patterns change*
