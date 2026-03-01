# First Run Diagnostic

**Purpose**: Discover the delta between our Rust wallet and the BSV TypeScript SDK — not to pass/fail, but to understand what's working, what's broken, and what's missing.

**Created**: 2026-02-27

---

## Philosophy

The first test run is **diagnostic, not a gate**. We expect failures. The goal is to produce a gap analysis that tells us:

1. **What's implemented and correct** → matches ts-sdk output
2. **What's implemented but wrong** → function exists, output differs
3. **What's missing** → function/module doesn't exist
4. **What's broken** → panics, unwrap failures, etc.

---

## How to Run

### Step 1: Validate Test Vectors (TypeScript)

Before running Rust tests, prove the vectors themselves are correct:

```bash
cd rust-wallet/tests/fixtures
npm install
npm run validate
```

**Expected output**: All vectors pass against @bsv/sdk. If this fails, the vector data is wrong (typo when copying from ts-sdk).

### Step 2: Run Diagnostic (Rust)

```bash
cd rust-wallet
cargo test diagnostic -- --nocapture 2>&1 | tee diagnostic_report.txt
```

Or use the full test runner:

```powershell
cd Hodos-Browser
./scripts/test-all.ps1 -Verbose -Filter "diagnostic" | tee diagnostic_report.txt
```

### Step 3: Review Report

The diagnostic outputs a markdown-formatted gap analysis. Look for:

- `✓ PASS` — implementation matches ts-sdk
- `✗ WRONG OUTPUT` — function works but produces different result
- `✗ PANIC` — function crashed (unwrap on None, index out of bounds, etc.)
- `✗ NOT FOUND` — module or function doesn't exist
- `⊘ SKIPPED` — test couldn't run (dependency not met)

---

## Test Categories

### Priority 0 (Critical — handles real money)

| Test | Module | Function | Status |
|------|--------|----------|--------|
| BRC-42 Private Key Derivation | `crypto::brc42` | `derive_child_private_key()` | ⏳ |
| BRC-42 Public Key Derivation | `crypto::brc42` | `derive_child_public_key()` | ⏳ |
| HMAC-SHA256 | `crypto::hmac` | `hmac_sha256()` | ⏳ |
| AES-256-GCM Encrypt | `crypto::aesgcm_custom` | `aesgcm_custom()` | ⏳ |
| AES-256-GCM Decrypt | `crypto::aesgcm_custom` | `aesgcm_decrypt_custom()` | ⏳ |

### Priority 1 (Recovery)

| Test | Module | Function | Status |
|------|--------|----------|--------|
| BIP-39 Mnemonic → Seed | `recovery` | `mnemonic_to_seed()` | ⏳ |
| BIP-32 HD Derivation | `recovery` | `derive_private_key_bip32()` | ⏳ |

### Priority 2 (Compliance)

| Test | Module | Function | Status |
|------|--------|----------|--------|
| BRC-3 Signature | `crypto::signing` | Full pipeline | ⏳ |
| BRC-2 HMAC | `brc2` | Full pipeline | ⏳ |

---

## Expected Failure Patterns

### Pattern 1: Module Not Found

```
error[E0433]: failed to resolve: could not find `brc42` in `crypto`
```

**Meaning**: The module doesn't exist or has a different path.
**Action**: Find where this functionality lives (grep for function name) or note as "not implemented."

### Pattern 2: Function Signature Mismatch

```
error[E0061]: this function takes 3 arguments but 4 were supplied
```

**Meaning**: Our API differs from what the test expects.
**Action**: Update test to match actual API, or note the difference.

### Pattern 3: Wrong Output

```
assertion failed: `(left == right)`
  left:  `"761656715bbfa172..."`
  right: `"a3b4c5d6e7f8..."`
```

**Meaning**: Function exists and runs, but produces different output than ts-sdk.
**Action**: This is a real bug — investigate the implementation.

### Pattern 4: Panic

```
thread 'test_brc42_vector_1' panicked at 'called `Option::unwrap()` on a `None` value'
```

**Meaning**: Function doesn't handle this input correctly.
**Action**: Note the panic location, investigate edge case handling.

---

## Gap Analysis Template

After running diagnostics, fill in this template:

```markdown
# Gap Analysis — [DATE]

## Summary

| Category | Passing | Failing | Missing | Total |
|----------|---------|---------|---------|-------|
| BRC-42   | ?       | ?       | ?       | 10    |
| HMAC     | ?       | ?       | ?       | 4     |
| AES-GCM  | ?       | ?       | ?       | 2     |
| BIP-39   | ?       | ?       | ?       | ?     |
| BIP-32   | ?       | ?       | ?       | ?     |

## Passing (matches ts-sdk)

- [ ] List functions that work correctly

## Wrong Output (implemented but incorrect)

- [ ] Function: `xxx`
  - Expected: `abc123`
  - Got: `def456`
  - Notes: ...

## Missing (not implemented)

- [ ] Module `crypto::brc42` doesn't exist
- [ ] Function `derive_child_private_key` not found

## Panics (crashes)

- [ ] `test_hmac_vector_3` panics at `src/crypto/hmac.rs:45`
  - Cause: unwrap on None
  - Input: 100-byte key (> blocklen)

## Next Steps

1. [ ] Implement missing functions: ...
2. [ ] Fix wrong outputs: ...
3. [ ] Handle edge cases: ...
```

---

## After First Run

Once you have the gap analysis:

1. **Fix critical issues first** (P0 — BRC-42, HMAC, AES-GCM)
2. **Update test status** in the table above
3. **Convert passing tests** from diagnostic to regular `#[test]`
4. **Add to CI** once core tests pass consistently

---

## Files

| File | Purpose |
|------|---------|
| `FIRST_RUN_DIAGNOSTIC.md` | This document |
| `diagnostic_test.rs` | Diagnostic test runner |
| `fixtures/ts_sdk_vectors.json` | Test vector data |
| `fixtures/validate_vectors.ts` | TypeScript validator |
| `diagnostic_report.txt` | Generated output (gitignored) |
