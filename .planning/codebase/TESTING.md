# Testing Patterns

**Analysis Date:** 2026-01-28

## Test Framework

**Frontend:**
- No test framework detected
- No Jest, Vitest, or Playwright configuration present
- Build process: `npm run build` runs TypeScript type-check (`tsc -b`) then Vite bundling
- No test scripts in `package.json`

**Rust:**
- Built-in test framework: `cargo test`
- Async test support via Tokio runtime
- Config: `Cargo.toml` contains test dependencies in dev section
- Run tests: `cargo test` or `cargo test --release` for optimized builds
- Run specific test: `cargo test test_name`

## Test File Organization

**Frontend:**
- No test files present
- Structure would follow component co-location if implemented
- Pattern: `ComponentName.test.tsx` or `ComponentName.spec.tsx` alongside source

**Rust:**
- Location: Tests embedded within source files using `#[cfg(test)]` modules
- Separate test files for integration tests: `src/crypto/aesgcm_custom_test.rs`, `src/script/pushdrop_tests.rs`
- Naming: Module name suffixed with `_test` or `_tests` (e.g., `aesgcm_custom_test.rs`)
- Test modules declared as: `#[cfg(test)] mod tests { ... }`

## Test Structure

**Rust Test Pattern (from `src/crypto/aesgcm_custom_test.rs`):**
```rust
#[cfg(test)]
mod tests {
    use crate::crypto::aesgcm_custom;
    use hex;

    #[test]
    fn test_aesgcm_roundtrip_4_bytes() {
        // Arrange: Set up test data
        let plaintext = b"true";
        let key = [0u8; 32];
        let iv = [0u8; 32];

        // Act: Execute encryption
        let (ciphertext, auth_tag) = aesgcm_custom::aesgcm_custom(
            plaintext,
            &[],
            &iv,
            &key,
        ).unwrap();

        // Assert: Verify roundtrip decryption
        let decrypted = aesgcm_custom::aesgcm_decrypt_custom(
            &ciphertext,
            &[],
            &iv,
            &auth_tag,
            &key,
        ).unwrap();

        assert_eq!(plaintext, decrypted.as_slice());
    }
}
```

**Patterns:**
- Arrange-Act-Assert pattern for test structure
- Module imports at top of test module
- Test functions marked with `#[test]` attribute
- Error handling: `.unwrap()` acceptable in tests (panic on error)
- Assertions: Standard `assert_eq!()`, `assert!()` macros

## Mocking

**Frontend:**
- No mocking framework detected
- Manual mocking via mock callbacks (e.g., `window.onAddressGenerated`)
- Mock pattern shown in bridge communication: Response callbacks are manually invoked from C++

**Rust:**
- No mocking framework (Mockito, etc.) detected in dependencies
- Mocking via trait implementations or conditional compilation
- Example: Test fixtures in `src/certificate/test_utils.rs` for creating test data
- Pattern: Direct calls to functions with known good inputs

**Test Data Pattern (from `src/crypto/aesgcm_custom_test.rs`):**
```rust
// Hex-encoded known values for regression testing
let plaintext = hex::decode("74727565").unwrap(); // "true"
let key = hex::decode("42b79dacfdca814a26a29522c53a50923574bf98c13cbaa5709053b71492e52b").unwrap();
let iv = hex::decode("41113d6599ece0d23e9ec3e1e80b168019087a1d2e4e27061de54b4b79f5cb6c").unwrap();
```

## Fixtures and Factories

**Test Data:**
- Rust test modules often use hex-encoded constants for known values
- Test utilities: `src/certificate/test_utils.rs` provides factory functions
- Fixture pattern: Zero-initialized arrays for deterministic testing (e.g., `[0u8; 32]`)
- Known-good values used for regression testing (hex-encoded key material)

**Location:**
- Fixtures colocated with tests via `#[cfg(test)]` module
- Separate utilities module: `src/certificate/test_utils.rs` (imported by certificate tests)
- No central fixtures directory

## Coverage

**Requirements:**
- Frontend: Not enforced, no coverage tools detected
- Rust: Not enforced, no coverage configuration visible

**View Coverage:**
- Rust: `cargo tarpaulin` would be required (not currently in dependencies)
- Not currently measured

**Critical Gaps:**
- Frontend: Zero test coverage
  - No unit tests for components
  - No integration tests for bridge communication
  - No E2E tests for wallet flows

- Rust: Partial coverage
  - Crypto modules have inline tests (good coverage on `brc42`, `aesgcm_custom`)
  - Database operations untested
  - HTTP handlers (`src/handlers.rs`) untested
  - Certificate verification untested

## Test Types

**Unit Tests:**
- Scope: Individual functions, typically crypto primitives
- Approach: Direct function calls with known inputs
- Example: `test_aesgcm_roundtrip_4_bytes()` in `src/crypto/aesgcm_custom_test.rs`
- Coverage: BRC-42 key derivation, encryption/decryption roundtrips

**Integration Tests:**
- Scope: None detected
- Should cover: Bridge communication, wallet operations, transaction flows

**E2E Tests:**
- Framework: Not used
- Should cover: User workflows (create wallet, send funds, view balance)
- Manual testing via running `HodosBrowserShell.exe`

## Common Patterns

**Async Testing:**
- Rust: Tokio runtime via `#[actix_web::main]` for web tests
- Frontend: Would use async test support in Jest/Vitest (not implemented)
- No async tests currently present in codebase

**Error Testing:**
- Rust pattern: Use `.unwrap()` to assert success or panic
- Example: `aesgcm_custom::aesgcm_custom(...).unwrap()` assumes encryption succeeds
- Could improve: Add explicit error case tests (e.g., invalid key length)
- Error handling tests: None detected

**Test Isolation:**
- Rust: Tests run in parallel by default (`cargo test -- --test-threads=1` to serialize)
- Database tests: Would need transaction rollback or test isolation strategy
- State tests: No global state shared between tests currently

## Running Tests

**Commands:**

```bash
# Rust - run all tests
cd rust-wallet
cargo test

# Rust - run tests with output
cargo test -- --nocapture

# Rust - run single test
cargo test test_aesgcm_roundtrip

# Rust - run with release optimizations
cargo test --release

# Frontend - no tests
npm run test  # Not available
```

## Gaps and Recommendations

**High Priority (Frontend):**
- Add Jest or Vitest for unit testing React components
- Test critical paths: address generation, balance display, transaction sending
- Bridge communication integration tests

**High Priority (Rust):**
- HTTP handler tests (mock AppState, test response codes)
- Database operation tests (use in-memory SQLite with `:memory:` connection string)
- Error case tests for crypto operations

**Medium Priority:**
- End-to-end tests via Playwright or similar
- Performance benchmarks for crypto operations
- Test coverage reporting (use `cargo tarpaulin` or `cargo llvm-cov`)

**Medium Priority (Rust):**
- Certificate verification tests
- Transaction serialization tests
- Fee calculation accuracy tests

---

*Testing analysis: 2026-01-28*
