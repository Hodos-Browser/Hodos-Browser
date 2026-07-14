# macOS Keychain Cross-Contamination: Root Cause, Fix, and Cross-Platform Report

**Date:** 2026-07-14
**Severity:** Critical (data integrity — wrong key used for signing)
**Platform:** macOS only (Windows NOT affected)
**Status:** Fully resolved — fix applied, funds recovered, backup operational

---

## Executive Summary

The macOS Keychain service name for wallet mnemonic auto-unlock was hardcoded to `"HodosBrowser"` for both dev and production wallets. When the dev wallet was created on June 25, 2026, it overwrote the production wallet's mnemonic in the system-wide Keychain. After that date, any production wallet restart that auto-unlocked from Keychain would silently use the **dev wallet's mnemonic**, causing all BRC-42 key derivations to produce wrong keys. This manifested as NULLFAIL errors on backup transactions starting July 6.

**No funds were lost.** The network rejected every transaction signed with the wrong key. Coins remain at their correct addresses.

---

## Root Cause

### The Bug

`rust-wallet/src/crypto/dpapi.rs` contained:

```rust
#[cfg(target_os = "macos")]
const KEYCHAIN_SERVICE: &str = "HodosBrowser";  // SAME for dev and production
```

Both `dpapi_encrypt` and `dpapi_decrypt` used this constant. When the dev wallet called `dpapi_encrypt()` during wallet creation, it called `set_generic_password("HodosBrowser", "wallet-mnemonic", dev_mnemonic)`, **overwriting** the production mnemonic that was stored under the same service name.

### Evidence Chain

| Evidence | Value |
|----------|-------|
| Keychain entry `cdat` (creation date) | `2026-06-25 15:20:29 UTC` |
| Dev wallet `created_at` | `1782400829` (~June 25, 2026) |
| Dev master pubkey | `0302cabd012b3dd3f277851aca311e055ccc4a6657e0461208e52a691eb6c9aa49` |
| Production master pubkey | `037b557ea34639105dc5a1172361a30406dc1f7ef3802f4a5ac6ada69bc8d5e075` |
| These are different keys | Confirms separate wallets/mnemonics |

### Why It Was Intermittent

The wallet alternated between correct and wrong states depending on how it unlocked at startup:

| Startup path | Mnemonic used | Result |
|-------------|---------------|--------|
| Keychain auto-unlock succeeded | **Wrong** (dev mnemonic) | All signing fails |
| Keychain access denied (new binary, locked Keychain) → PIN unlock | **Correct** (from encrypted DB) | Everything works |

This explains why "Register as @26" (July 7) succeeded but backup (July 6) failed — different startup sessions.

### Cryptographic Proof

July 6 backup tx `1b26b63b...`:

| Input | Type | ECDSA Valid | HASH160 Match | Result |
|-------|------|-------------|---------------|--------|
| 0 | PushDrop | Yes | N/A | OK |
| 1 | Marker P2PKH | Yes | Yes | OK |
| 2 | Funding P2PKH | Yes (for wrong key) | **MISMATCH** | **FAILS** |

The signing key's HASH160 (`b3401e61...`) didn't match the locking script's expected HASH160 (`c67799b7...`). The wallet derived the correct key structure but from the wrong master key.

BRC-42 self-derivation code was verified correct via `test_self_derivation_consistency` (6 key pairs x 3 invoices, all consistent). The code is correct; only the input was wrong.

---

## The Fix

### Code Change (`rust-wallet/src/crypto/dpapi.rs`)

Replaced the hardcoded constant with a function that reads the `HODOS_DEV` environment variable:

```rust
#[cfg(target_os = "macos")]
fn keychain_service() -> &'static str {
    if std::env::var("HODOS_DEV").unwrap_or_default() == "1" {
        "HodosBrowserDev"
    } else {
        "HodosBrowser"
    }
}
```

All call sites updated to use `keychain_service()` instead of the old `KEYCHAIN_SERVICE` constant.

### Recovery Steps Performed (2026-07-14)

1. **Deleted contaminated Keychain entry:** `security delete-generic-password -s "HodosBrowser" -a "wallet-mnemonic"`
2. **Cleared `mnemonic_dpapi` in production DB:** `UPDATE wallets SET mnemonic_dpapi = NULL WHERE id = 1` — so the backfill code will re-store the correct mnemonic on next PIN unlock
3. **Cleared `mnemonic_dpapi` in dev DB:** Same — so dev wallet's backfill will store under the new `"HodosBrowserDev"` service name
4. **Removed all diagnostic logging** added during investigation
5. **Removed temporary debug endpoint** (`/wallet/debug/verify-derivation`)
6. **Rebuilt release binary** with all changes

### What Happens on Next Startup

**Production wallet:**
- Auto-unlock fails (no Keychain entry) → wallet locked
- User enters PIN → correct mnemonic from encrypted DB
- Backfill detects `mnemonic_dpapi = NULL` → stores correct mnemonic in Keychain under `"HodosBrowser"`
- Future startups auto-unlock correctly

**Dev wallet (with `HODOS_DEV=1`):**
- Auto-unlock tries `"HodosBrowserDev"` → not found → wallet locked
- User enters PIN → correct dev mnemonic
- Backfill stores dev mnemonic under `"HodosBrowserDev"`
- Future dev startups auto-unlock from isolated entry

---

## Impact Assessment

### Funds

**No funds were lost.** Transactions signed with the wrong key were rejected by the network (NULLFAIL). However, internally-consistent transactions (where change outputs were derived from the wrong mnemonic) did move funds to dev-keyed addresses. Specifically, 12,496,323 sats at address index 9 were locked to the dev wallet's derived key.

These funds were recovered on 2026-07-14 via a cross-key sweep (see "Fund Recovery" section below).

### On-Chain Backup

The existing on-chain backup token (tx8, `7855796d`, April 15) was **valid** — created before the June 25 contamination, but stale. A fresh backup was created successfully on 2026-07-14 after recovery.

The failed tx9 (`76c47e92`, April 15) was verified to have **correct production-key signatures** — not a wrong-key issue. It was a broadcast failure, likely due to pre-resilience broadcast infrastructure (fixed in commit `63cda01` on April 17). Dead record in DB, no impact.

### Affected Date Range

June 25, 2026 (dev wallet creation) through July 14, 2026 (fix applied). Only macOS sessions where Keychain auto-unlock succeeded were affected.

---

## Cross-Platform Analysis: Windows Is NOT Vulnerable

This bug is **macOS-specific**. The fundamental difference:

| Platform | How `dpapi_encrypt` stores the mnemonic | Isolation mechanism |
|----------|----------------------------------------|---------------------|
| **macOS** | In the **system-wide Keychain** (DB stores sentinel `b"KEYCHAIN"`) | Service name string — was shared, now separated |
| **Windows** | Encrypted blob stored **directly in the DB column** `mnemonic_dpapi` | Each wallet has its own DB in its own directory |

On Windows:
- `CryptProtectData` encrypts the mnemonic into a blob that goes into the `mnemonic_dpapi` column
- Production DB is at `%APPDATA%/HodosBrowser/wallet/wallet.db`
- Dev DB is at `%APPDATA%/HodosBrowserDev/wallet/wallet.db`
- The encrypted blobs are in separate files — no shared credential store is involved
- Even if both wallets run on the same machine, they cannot interfere with each other

**No Windows code changes are needed.** The fix in `dpapi.rs` only affects the `#[cfg(target_os = "macos")]` code paths.

### Why This Can't Happen on Windows

The DPAPI architecture stores secrets **per-database** (the blob is in the DB column), while macOS Keychain stores secrets **per-service-name** (the DB column just holds a pointer). The per-database model provides implicit isolation that the Keychain model had to achieve through naming.

---

## Remaining Work

| Item | Status | Notes |
|------|--------|-------|
| Fix code (`dpapi.rs`) | Done | `keychain_service()` function replaces constant |
| Delete contaminated Keychain entry | Done | `security delete-generic-password` |
| Clear production DB sentinel | Done | `mnemonic_dpapi = NULL` |
| Clear dev DB sentinel | Done | `mnemonic_dpapi = NULL` |
| Remove diagnostic logging | Done | 14 `DIAG` log lines removed |
| Remove debug endpoint | Done | `/wallet/debug/verify-derivation` removed |
| Rebuild release binary | Done | `cargo build --release` passed |
| PIN unlock production wallet | Done | Keychain auto-unlock works with fixed service name |
| Sweep dev-keyed funds | Done | 12,496,323 sats swept to prod index 0 (tx `9b3bef7a`) |
| Remove sweep endpoint | Done | One-time handler + route removed after use |
| Create fresh backup | Done | Backup successful after sweep — wallet fully operational |
| tx9 (`76c47e92`) | Not a Keychain issue | April 15 failed backup — signatures verified correct (production keys). Broadcast failure, likely pre-resilience infrastructure issue (fixed in `63cda01` on April 17). Dead record in DB, no impact |
| Keep `test_self_derivation_consistency` | Recommended | Useful regression test in `brc42.rs` |

---

## Files Changed

| File | Change |
|------|--------|
| `rust-wallet/src/crypto/dpapi.rs` | `KEYCHAIN_SERVICE` constant → `keychain_service()` function with `HODOS_DEV` check |
| `rust-wallet/src/crypto/brc42.rs` | Added `test_self_derivation_consistency` test (kept — regression test) |
| `rust-wallet/src/database/helpers.rs` | Removed 4 diagnostic log lines |
| `rust-wallet/src/handlers.rs` | Removed 10 diagnostic log lines + `debug_verify_derivation` endpoint (~130 lines) |
| `rust-wallet/src/main.rs` | Removed `/wallet/debug/verify-derivation` route |

---

## Fund Recovery (2026-07-14)

After restoring the correct Keychain entry, backup still failed because the 12.5M sat funding UTXO (`fb8620b6:4`, address index 9) was locked to the dev wallet's derived key. During wrong-mnemonic sessions, internally-consistent transactions moved funds through dev-keyed change addresses.

**Verification**: Python crypto scripts confirmed that the HASH160 in the locking script matched the dev key's derivation at index 9, not the production key. Indices 6-9 were all dev-derived; indices 0-4 were production-derived.

**Sweep**: Built a one-time `debug_sweep_crosskey` endpoint that:
1. Took the dev mnemonic as input, derived the dev master key
2. Classified all spendable UTXOs by checking HASH160 against both dev and prod keys
3. Signed the dev-keyed UTXO with the dev-derived private key
4. Sent output to production address index 0 (prod-keyed)

**Result**: Transaction `9b3bef7a3cf98d6858503e955e474fd38d61f1b40a1a1eded3e0744f8831c188` — swept 12,496,323 sats with 200 sat fee → 12,496,123 sats at production address index 0. Accepted by network (`SEEN_ON_NETWORK`).

The sweep endpoint was removed immediately after use.

---

## Lessons Learned

1. **System-wide credential stores need per-application isolation.** The Keychain is shared across all processes for a user. Any service name collision is a silent overwrite.

2. **Dev/production isolation must extend to OS credential stores**, not just file paths. The `HODOS_DEV` environment variable correctly separated DB directories but the Keychain service name was overlooked.

3. **Silent auto-unlock with wrong credentials is dangerous.** The wallet appeared to unlock successfully but was operating with wrong keys. Consider adding a startup check that verifies the master pubkey from the cached mnemonic matches the `users.identity_key` in the DB.

4. **The network catches invalid signatures, but internally-consistent wrong-key transactions succeed.** When a transaction's inputs are already locked to the wrong key, signing with that same wrong key produces valid signatures. Change outputs derived from the wrong key perpetuate the problem — funds migrate into the wrong key's address space through a chain of valid transactions. Recovery requires a cross-key sweep.

5. **A startup mnemonic-check would catch this earlier.** Comparing the master pubkey derived from the cached mnemonic against `users.identity_key` at startup would immediately detect mnemonic contamination, rather than discovering it only when an external-facing operation (like backup) fails.
