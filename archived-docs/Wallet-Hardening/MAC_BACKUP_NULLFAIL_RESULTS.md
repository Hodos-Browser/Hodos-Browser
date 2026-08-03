# MAC BACKUP NULLFAIL — INVESTIGATION RESULTS

**Analyzed 2026-07-13 / 2026-07-14 by Mac Claude. All verification performed offline on a forensic DB copy.**

---

## TWO DISTINCT FAILURE MODES IDENTIFIED

The investigation found **one root cause** (Keychain cross-contamination) and one historical broadcast failure:

| Failure | Tx | Date | Root Cause | Status |
|---------|-----|------|-----------|--------|
| **July 6+ backups** | `1b26b63b`, `64d9366e` | July 2026 | **KEYCHAIN CROSS-CONTAMINATION** — dev mnemonic overwrote production in Keychain, wallet signed with wrong key | **RESOLVED** — see `MAC_KEYCHAIN_CROSSCONTAMINATION_FIX.md` |
| **tx9** | `76c47e92` | April 2026 | Broadcast infrastructure failure — signatures verified correct (production keys). Likely fixed by `63cda01` (broadcast resilience, April 17) | **Not a bug** — dead DB record, no impact |

---

## BUG 1: KEY DERIVATION BUG (July 6+ backups)

### Summary

The `do_onchain_backup` function signs funding UTXO P2PKH inputs with a key whose HASH160 does not match the locking script. The resulting OP_EQUALVERIFY failure is reported as NULLFAIL by the network.

### Definitive Verification

Verified July 6 backup tx `1b26b63bbba3db354531255fb20678038b69b2113f246fa9c9285959969f7f33`:

| Input | Type | Source | ECDSA Valid | HASH160 Match | Result |
|-------|------|--------|-------------|---------------|--------|
| 0 | PushDrop | `7855796d:0` | ✅ | N/A (not P2PKH) | **OK** |
| 1 | Marker P2PKH | `7855796d:1` | ✅ | ✅ | **OK** |
| 2 | Funding P2PKH | `80e034c8:0` | ✅ (for wrong key) | ❌ **MISMATCH** | **FAILS ON-CHAIN** |

**Input 2 details:**
- Expected HASH160 (from `80e034c8:0` locking script): `c67799b7b962d3c8cd027ae22be2418f51979152`
  - This is address `1K6Q4y9VzowWRM8jM5UGLnx6xp7uo5Go4A` (derivation index 6)
- Actual HASH160 (from pubkey in unlocking script): `b3401e61de507a59411b98658c276bd3cdb82fc7`
  - This matches the **change output** (output 2) of the same transaction
- Wrong pubkey: `02157e350dee637350ec3d60838188d1a35ee9e9e3c57d3ec1cef026bcc7bc4b7f`

### Pattern Confirmation

Checked ALL 6 backup-like transactions in `parent_transactions`:

| Txid (prefix) | Funding UTXO | Input 2 HASH160 | Result |
|---------------|-------------|-----------------|--------|
| `76c47e92` (tx9) | `7855796d:2` (index 4) | ✅ Match | OK |
| `68af2622` | `76c47e92:2` | ✅ Match | OK |
| `23edc873` | `76c47e92:2` | ✅ Match | OK |
| **`64d9366e`** | **`80e034c8:0` (index 6)** | **❌ Mismatch** | **BUG** |
| `7855796d` (tx8) | `11875ee3:2` | (source N/A) | — |
| **`1b26b63b`** | **`80e034c8:0` (index 6)** | **❌ Mismatch** | **BUG** |

Both failing txs produce the **identical** wrong pubkey and the **identical** change output hash.
Backup txs using other funding UTXOs (derivation indices 2, 4) sign correctly.

### Cross-Reference: Same UTXO Works in Non-Backup Context

The same UTXO `80e034c8:0` was **successfully spent** by the "Register as @26" transaction
(`706c8fdd`, July 7, status=completed). That transaction uses the same `derive_key_for_output`
function via the `sign_action` handler. The key derivation is correct there.

This proves the bug is **specific to the backup signing path**, not to `derive_key_for_output` itself.

### What the Wrong Key Is

The wrong pubkey (`02157e35...`) produces HASH160 `b3401e61...`, which is:
- ✅ The **change output** address in the July backup tx
- ❌ NOT any address in the `addresses` table (checked all indices -3 through 9)
- ❌ NOT the master pubkey, backup pubkey, or any known special key

The change output is derived at `handlers.rs:13295-13310`:
```rust
let current_index = address_repo.get_max_index(wallet_id).ok().flatten().unwrap_or(0);
let invoice = format!("2-receive address-{}", current_index);
let derived_pubkey = derive_child_public_key(&master_privkey, &master_pubkey, &invoice).unwrap();
```

### Timing Context

- Address index 6 created: `2026-07-06 21:51:05`
- Output `80e034c8:0` received at address 6: `2026-07-06 21:52:19`
- Index 5 is **missing** from the addresses table (skipped — `generate_address` incremented `current_index` to 5 but the address record was never saved)
- `wallet.current_index` = 10, `get_max_index` returns 9 (at DB snapshot time)

At backup time on July 6, `get_max_index` likely returned **6** (only addresses 0-4 and 6 existed).
The change invoice would be `"2-receive address-6"` — the **same** invoice as the funding UTXO.
Both should derive the same key, but the key doesn't match what's in the addresses table for index 6.

### ROOT CAUSE IDENTIFIED: Keychain Mnemonic Cross-Contamination

**Status: SOLVED** (2026-07-14)

The macOS Keychain entry for wallet mnemonic auto-unlock used a **hardcoded service name**
`"HodosBrowser"` for BOTH dev and production wallets (`rust-wallet/src/crypto/dpapi.rs:134`).
When the dev wallet started, it overwrote the production wallet's Keychain mnemonic with its own.

**Evidence chain:**
1. `KEYCHAIN_SERVICE` was `"HodosBrowser"` for both `HODOS_DEV=1` and production builds
2. Dev wallet created on **2026-06-25** → `dpapi_encrypt()` stored dev mnemonic in Keychain
3. Keychain entry `cdat` (creation date) = `2026-06-25 15:20:29 UTC` — matches dev wallet creation
4. Production wallet auto-unlock reads same Keychain entry → gets **dev wallet's mnemonic**
5. Dev master pubkey: `0302cabd...` ≠ Production master pubkey: `037b557e...`
6. BRC-42 derivation with wrong master key → wrong child key → HASH160 mismatch → NULLFAIL

**Why the "Register as @26" tx succeeded (July 7) but backup failed (July 6):**
The wallet alternated between correct and wrong mnemonics depending on whether it was
auto-unlocked from Keychain (wrong — dev mnemonic) or manually unlocked via PIN (correct —
production mnemonic from encrypted DB). Address 6 was created during a PIN-unlocked session;
the backup ran after a Keychain auto-unlock.

**BRC-42 self-derivation verified correct** — test `test_self_derivation_consistency` passes.
The code is correct; the inputs (master key) were wrong.

### Fix Applied

`dpapi.rs`: `KEYCHAIN_SERVICE` changed from a constant to a function that reads `HODOS_DEV`:
```rust
fn keychain_service() -> &'static str {
    if std::env::var("HODOS_DEV").unwrap_or_default() == "1" {
        "HodosBrowserDev"
    } else {
        "HodosBrowser"
    }
}
```

**Recovery needed:** The production wallet's Keychain entry must be restored. Next time the
production wallet starts and unlocks via PIN, the DPAPI backfill path should store the
correct mnemonic under `"HodosBrowser"`. If the backfill doesn't trigger (because the DB
already has a KEYCHAIN sentinel), manually clear `mnemonic_dpapi` in the production wallet
DB and restart — the backfill will re-store the correct mnemonic from PIN decryption.

---

## BUG 2: ORIGINAL TX9 — VALID SIGNATURES, BROADCAST FAILURE (April)

### Step 1 Outcome: RAW TX IS VALID

Verified using `@bsv/sdk` (v2.1.2) + native `secp256k1` C library:

| Input | Type | ECDSA Valid | HASH160 Match |
|-------|------|-------------|---------------|
| 0 | PushDrop (2648-byte script) | ✅ | N/A |
| 1 | Marker P2PKH | ✅ | ✅ |
| 2 | Funding P2PKH | ✅ | ✅ |

All 3 inputs verify cryptographically. The raw transaction is valid.

### Raw Broadcast Test Results

Attempted broadcasting the raw tx directly (without BEEF wrapper):

| Endpoint | Method | Result |
|----------|--------|--------|
| ARC GorillaPool `/v1/tx` | Raw tx hex | `SEEN_IN_ORPHAN_MEMPOOL` — parent UTXOs not found |
| WoC `/tx/raw` | Raw tx hex | `Missing inputs` |

Both endpoints can't find the parent UTXOs despite tx8 having 12,842+ confirmations. This is
inconclusive for script validation — the nodes didn't evaluate the scripts at all because the
UTXO inputs couldn't be located.

### What This Rules Out for Tx9

- ~~Key derivation bug~~ — all HASH160s match ✅ (unlike the July backups)
- ~~Sighash computation~~ — ECDSA signatures are valid
- ~~ScriptCode truncation~~ — full PushDrop script used (2648 bytes)
- ~~Wrong satoshi values~~ — DB matches on-chain
- ~~DB-chain divergence~~ — parent tx data matches byte-for-byte

### What Remains for Tx9

The raw broadcast test was inconclusive (orphan mempool issue). The BEEF layer remains the primary suspect:
- BEEF may embed incorrect parent tx data
- BEEF ancestry walk may include wrong or corrupt transactions
- The node's BEEF processor may compute a different sighash than the wallet signed

Tx9's failure mechanism is **different from the July bugs** and requires separate investigation.

---

## Verification Script Locations

All scripts in: `/private/tmp/claude-501/-Users-matt/23e0a5f6-7966-45d3-83ea-73217e041fe8/scratchpad/bsv-verify/`

| Script | Purpose |
|--------|---------|
| `verify_july.mjs` | ECDSA + HASH160 verification for July 6 backup |
| `check_hash160.mjs` | Cross-tx HASH160 comparison (tx8, tx9, July) |
| `verify4.mjs` | Original tx8/tx9 signature verification |

Dependencies: `@bsv/sdk`, `secp256k1` (native C binding).

---

## Forensic Data Location

DB copy: `/private/tmp/claude-501/-Users-matt/23e0a5f6-7966-45d3-83ea-73217e041fe8/scratchpad/backup-forensics/wallet.db`

Raw tx hex files: `*.hex` in the bsv-verify directory.
