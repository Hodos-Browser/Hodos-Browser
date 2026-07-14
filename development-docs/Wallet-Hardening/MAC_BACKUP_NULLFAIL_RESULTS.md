# MAC BACKUP NULLFAIL — INVESTIGATION RESULTS

**Analyzed 2026-07-13 / 2026-07-14 by Mac Claude. All verification performed offline on a forensic DB copy.**

---

## TWO DISTINCT FAILURE MODES IDENTIFIED

The investigation found **two separate bugs**, not one:

| Failure | Tx | Date | Root Cause | Status |
|---------|-----|------|-----------|--------|
| **July 6+ backups** | `1b26b63b`, `64d9366e` | July 2026 | **KEY DERIVATION BUG** — wallet signs funding UTXO with the wrong key | **ROOT CAUSE FOUND** |
| **Original tx9** | `76c47e92` | April 2026 | BEEF/broadcast layer issue — raw tx is cryptographically valid | Separate issue, unresolved |

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

### What Remains Unknown

Despite exhaustive code analysis, the exact mechanism is not yet identified:
1. The `derive_key_for_output` code paths are identical between backup and `sign_action`
2. The derivation fields in the output record are correct (`"2-receive address"`, `"6"`, NULL)
3. Both code paths call the same `get_master_private_key_from_db` → `derive_child_private_key`
4. No variable shadowing, no stale cache, no hidden characters in the DB

Possible remaining theories:
- The **compiled binary** running on the Mac has different code than what's in git (build mismatch)
- A **race condition** between the async backup function and another task modifying wallet state
- A subtle **secp256k1 state corruption** from the change address derivation affecting the subsequent signing derivation
- The `derive_child_public_key` (used for change address) and `derive_child_private_key` (used by `derive_key_for_output`) produce **inconsistent** key pairs under some edge condition

### Recommended Diagnostic: Live Key Derivation Test

Add temporary logging to `derive_key_for_output` in `database/helpers.rs` to capture the ACTUAL derived key for `80e034c8:0`:

```rust
// TEMPORARY DIAGNOSTIC — remove after investigation
if suffix == "6" && prefix == "2-receive address" {
    let pubkey = crate::crypto::keys::derive_public_key(&result)?;
    let h160 = {
        use sha2::{Sha256, Digest};
        let sha = Sha256::digest(&pubkey);
        ripemd::Ripemd160::digest(&sha)
    };
    log::warn!("🔬 DIAG derive_key_for_output invoice='2-receive address-6': pubkey={}, h160={}",
        hex::encode(&pubkey), hex::encode(&h160));
}
```

And in the backup change address derivation at `handlers.rs:13300`:

```rust
// TEMPORARY DIAGNOSTIC — remove after investigation
log::warn!("🔬 DIAG backup change: index={}, invoice={}, pubkey={}, h160={}",
    current_index, invoice, hex::encode(&derived_pubkey),
    hex::encode(&pubkey_hash));
```

This will capture the actual keys at runtime and reveal whether:
1. `derive_key_for_output` produces the wrong key (derivation bug)
2. The change address derivation produces the wrong key (different invoice number)
3. Both produce the same wrong key (master key mismatch)

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
