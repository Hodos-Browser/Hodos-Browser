# Archive

This folder contains old tests and debugging scripts that are no longer actively used but preserved for reference.

## Contents

### `old-tests/`
Integration tests from early development (Dec 2025 - Feb 2026). These used hardcoded values from debugging sessions rather than proper test vectors. Superseded by ts-sdk vector-based tests in `../tests/`.

- `interoperability_test.rs` — TypeScript SDK encryption interop testing
- `certificate_decryption_test.rs` — BRC-2 certificate decryption debugging
- `csr_json_serialization_test.rs` — CSR format debugging

### `test-scripts/`
Ad-hoc debugging scripts (~50 files) used during BRC protocol implementation. Mix of JavaScript, PowerShell, and Rust snippets. Useful for understanding debugging approaches but not automated tests.

## Why Archive Instead of Delete?

- Git history preserves everything, but archived files are easier to find
- Some debugging approaches might be useful for future issues
- Keeps the active `tests/` directory clean and trustworthy
