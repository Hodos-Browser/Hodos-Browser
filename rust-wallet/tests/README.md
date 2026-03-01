# Rust Wallet Tests

## Quick Start — First Run

```bash
# Step 1: Validate test vectors (TypeScript)
cd tests/fixtures
npm install
npm run validate

# Step 2: Run diagnostic (Rust)
cd ../..
cargo test diagnostic -- --nocapture
```

See `FIRST_RUN_DIAGNOSTIC.md` for detailed instructions.

## Directory Structure

```
tests/
├── FIRST_RUN_DIAGNOSTIC.md     # First run guide and gap analysis template
├── diagnostic_test.rs          # Diagnostic runner (catches all errors, reports status)
│
├── fixtures/                    # Test data (shared between tests)
│   ├── ts_sdk_vectors.json     # Vectors copied from BSV TypeScript SDK
│   ├── validate_vectors.ts     # TypeScript validator (proves vectors are correct)
│   └── package.json            # For npm install @bsv/sdk
│
├── brc42_vectors_test.rs       # BRC-42 key derivation (P0 — critical) [TODO]
├── hmac_vectors_test.rs        # HMAC-SHA256 (P0 — critical) [TODO]
├── aesgcm_vectors_test.rs      # AES-256-GCM encryption (P0 — critical) [TODO]
├── bip39_vectors_test.rs       # Mnemonic → seed (P1 — recovery) [TODO]
├── bip32_vectors_test.rs       # HD derivation (P1 — recovery) [TODO]
└── README.md                   # This file
```

## Running Tests

```bash
# All tests
cargo test

# Specific test file
cargo test brc42

# With output (see println!)
cargo test -- --nocapture

# Single test
cargo test test_brc42_private_key_vector_1
```

## Test Vector Validation

Before trusting Rust test results, validate the vectors themselves:

```bash
cd tests/fixtures
npx ts-node validate_vectors.ts
```

If TypeScript validation passes and Rust fails → bug in our Rust code.
If both fail → bug in the vector (typo when copying from ts-sdk).

## Adding New Tests

1. Add vectors to `fixtures/ts_sdk_vectors.json`
2. Add TypeScript validation in `fixtures/validate_vectors.ts`
3. Create Rust test file that reads the JSON
4. Run TypeScript validator first, then Rust tests

## Source of Truth

All test vectors come from the BSV TypeScript SDK:
https://github.com/bitcoin-sv/ts-sdk

Specific source files:
- BRC-42: `src/primitives/__tests/BRC42.private.vectors.ts`
- HMAC: `src/primitives/__tests/HMAC.test.ts`
- AES-GCM: `src/primitives/__tests/AESGCM.test.ts`
- BIP-39: `src/compat/__tests/Mnemonic.vectors.ts`
- BIP-32: `src/compat/__tests/HD.test.ts`
