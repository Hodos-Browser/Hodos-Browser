# MAC BACKUP FAILURE — FORENSIC FINDINGS

**Analyzed 2026-07-13 by Mac Claude. All analysis performed on READ-ONLY copies of DB + logs.**
**Wallet DB: `~/Library/Application Support/HodosBrowser/wallet/wallet.db` (40 MB, copied to scratchpad)**

---

## Executive Summary

The Mac wallet backup has been failing since **April 15, 2026 at 16:52:06** (tx 9 — 3 minutes after the last successful backup tx 8). The error is a **NULLFAIL signature verification failure**, NOT missing inputs, NOT divergence, NOT insufficient funds.

**Critical finding:** The initial hypothesis (backup-token DB-vs-chain DIVERGENCE from the April 11 incident) is **WRONG**. There is **NO divergence**. The DB backup token state matches the on-chain truth exactly. The wallet IS trying to spend the correct, chain-unspent output.

**Surprising result:** Independent cryptographic verification of all 3 input signatures in the failed tx proves they are **mathematically VALID** (ECDSA verification passes for the sighash computed from the raw transaction data). Yet BSV nodes consistently reject the transaction with NULLFAIL. This points to a **BEEF construction or broadcast-layer issue**, not a signing or key derivation bug.

---

## 1. Today's Failed Attempt (from `wallet_rCURRENT.log`)

Three backup attempts on July 13, all producing identical errors:

```
[14:12:10] ⚠️  DB backup PushDrop 7855796df0cf8c23:0 spent-check inconclusive — trusting DB
[14:12:10] ⚠️  No markers found on-chain — trusting DB backup 7855796df0cf8c23
[14:12:16] ❌ Fatal broadcast error: provider whatsonchain returned status 400:
    "unexpected response code 500: 16: mandatory-script-verify-flag-failed
    (Signature must be zero for failed CHECK(MULTI)SIG operation)"
```

Pattern repeats at 14:13:35 and 14:23:35. Every attempt fails identically.

**Attempted-spend outpoint:** `7855796df0cf8c23fc366abee937c78994e79209ee5f44d24d337fea3325b280:0`
**Error:** `mandatory-script-verify-flag-failed (Signature must be zero for failed CHECK(MULTI)SIG operation)` (NULLFAIL — OP_CHECKSIG returned FALSE with non-empty signature)

---

## 2. DB Backup Token History

Four backup token pairs in the DB, all correctly chained:

| Tx ID | ID | Status | Vout 0 (PushDrop) | Vout 1 (Marker) | Vout 2 (Change) | Created |
|-------|------|---------|-------------------|-----------------|-----------------|---------|
| `d1afbcf0...` | 5 | completed | 1000 sats, spendable=0 | 546 sats, spendable=0 | – | 2026-04-15 06:56:28 |
| `ca0223ef...` | 6 | completed | 1000 sats, spendable=0 | 546 sats, spendable=0 | – | 2026-04-15 06:56:31 |
| `11875ee3...` | 7 | completed | 1000 sats, spendable=0 | 546 sats, spendable=0 | – | 2026-04-15 10:49:46 |
| `7855796d...` | 8 | completed | 1000 sats, **spendable=1** | 546 sats, **spendable=1** | 6,441,933 sats, spendable=0 (spent by tx 12) | 2026-04-15 10:49:48 |

**DB backup tip:** `7855796df0cf8c23fc366abee937c78994e79209ee5f44d24d337fea3325b280` — vouts 0 and 1 are the current spendable backup tokens.

**Failed backup (tx 9):** `76c47e92c4a481a680968ef21262ce719d690613617a1d6c3c3f4f7f4c4f358f` — status: failed, created 2026-04-15 10:52:06.

All 4 PushDrop outputs use the same backup pubkey: `02b9d142939687a8e4f27e83b5530c07b10d30338e54b3392d4b8f6a7b7b5ca04b`.

---

## 3. Chain Truth

### On-chain status of each backup PushDrop (vout 0):

| Tx | Vout 0 Spent? | Spent By | Notes |
|----|---------------|----------|-------|
| `d1afbcf0...` | ✅ Spent | `ca0223ef...` | Correctly chained |
| `ca0223ef...` | ✅ Spent | `11875ee3...` | Correctly chained |
| `11875ee3...` | ✅ Spent | `7855796d...` | Correctly chained |
| **`7855796d...`** | **❌ UNSPENT** | — | **Current chain tip** |

### On-chain locking scripts verified:
- PushDrop (`7855796d...:0`): 2648 bytes — **100% byte-for-byte match** with DB `locking_script`
- Marker (`7855796d...:1`): P2PKH to `1APML3WyEUFrvwNig3qQ9861KyKDc74Nvo` — **matches DB exactly**
- Change (`7855796d...:2`): P2PKH to `1KTUmqBMGGuVBrxTpAoJEZESEaPBXz8jLM` — **matches DB** (spent by Paymail tx 12)

### WoC marker address query:
- Queried address: `1APML3WyEUFrvwNig3qQ9861KyKDc74Nvo` (= HASH160 of backup pubkey — verified)
- **Now returns:** `[{"tx_hash":"7855796d...","value":546,"height":944941}]` — the marker IS there
- **Wallet consistently sees:** empty/no markers — likely a WoC rate limit, timeout, or network issue at query time
- The wallet falls through to "trusting DB" which produces the CORRECT result regardless

---

## 4. Divergence Map

### VERDICT: NO DIVERGENCE

| Check | DB State | Chain Truth | Match? |
|-------|----------|-------------|--------|
| Current backup tip txid | `7855796d...` | `7855796d...:0` unspent | ✅ |
| PushDrop locking script | 2648 bytes, starts `2102b9d142...` | Identical | ✅ |
| Marker P2PKH address | `1APML3WyEUFrvwNig3qQ9861KyKDc74Nvo` | Identical | ✅ |
| PushDrop satoshis | 1000 | 1000 | ✅ |
| Marker satoshis | 546 | 546 | ✅ |
| Chain lineage | 5→6→7→8 | Same | ✅ |

The DB's notion of "current backup tip" is **exactly correct**. There is no fork point. The wallet is attempting to spend the right output.

---

## 5. Why Adopt Didn't Heal

**It didn't need to.** The `adopt_onchain_backup` mechanism is designed to heal DB-vs-chain divergence by adopting the on-chain backup token as the DB tip. Since there IS no divergence (the DB already points to the correct chain tip), adopt has nothing to do. This is correct behavior.

The code path that runs:
1. Step 5b: Finds `7855796d...:0` (PushDrop) and `:1` (marker) as spendable — correct
2. Step 5c: WoC marker query returns empty → falls to spent-check → WoC returns 404 (not spent) → trusts DB — correct
3. Proceeds with correct `previous_pushdrop` and `previous_marker` data

---

## 6. c5b Sweep: EXONERATED

The c5b Step 1.5 sweep gate (`backup_outpoints.len() > 2`) was **never triggered**. The DB has exactly 2 spendable backup outpoints (`7855796d...:0` and `:1`), which equals the gate threshold. The sweep code never fires.

---

## 7. Failed Transaction Structure (tx 9)

Decoded from DB `raw_tx` (3144 bytes):

**Inputs (3):**
| # | Outpoint | Script Type | Script Length | Pubkey |
|---|----------|-------------|---------------|--------|
| 0 | `7855796d...:0` | P2PK (sig only) | 72B | `02b9d142...` (from locking script) |
| 1 | `7855796d...:1` | P2PKH (sig+pubkey) | 107B | `02b9d142...` (backup key) |
| 2 | `7855796d...:2` | P2PKH (sig+pubkey) | 106B | `023ada00...` (change address key) |

**Outputs (3):**
| # | Value | Script Type | Size |
|---|-------|-------------|------|
| 0 | 1,000 sats | PushDrop (new backup) | 2,647B |
| 1 | 546 sats | P2PKH marker | 25B |
| 2 | 6,438,753 sats | P2PKH change | 25B |

**Fee:** 3,180 sats (total_in 6,443,479 - total_out 6,440,299)
**Locktime:** 0, **Version:** 1, all sequences `0xFFFFFFFF`
**Txid verification:** computed txid from raw_tx matches DB txid `76c47e92...` ✅

---

## 8. THE KEY FINDING: Signature Verification

### Independent cryptographic verification of all 3 inputs:

Using BIP143 (BSV ForkID) sighash preimage construction with the on-chain locking scripts and values:

| Input | Script Type | Sighash Verified? | Signature Valid? |
|-------|-------------|-------------------|------------------|
| 0 (PushDrop) | Full 2648-byte locking script as scriptCode | ✅ | **✅ VALID** |
| 1 (Marker) | Standard P2PKH scriptCode | ✅ | **✅ VALID** |
| 2 (Change) | Standard P2PKH scriptCode | ✅ | **✅ VALID** |

### Cross-validation with known-good tx 8:

Applied the same sighash computation to tx 8 (the last successful backup):
- Input 0 (PushDrop from tx 7, 2611-byte script): **✅ VALID**
- Input 1 (Marker from tx 7): **✅ VALID**
- Computed txid matches confirmed txid: **✅**

This proves the sighash algorithm implementation is correct — it produces results that match what BSV nodes accept.

### The paradox:

The transaction is **correctly signed** (all ECDSA signatures verify), yet BSV nodes reject it with NULLFAIL. This means the nodes compute a **different sighash** than what the wallet computed.

Since the standalone signatures ARE valid (same algorithm verified against tx 8), the discrepancy must be in what the nodes SEE when they process the broadcast — i.e., the **BEEF envelope** or the **broadcast pathway**.

---

## 9. The "ROLLBACK TEST (synthetic)" (tx 10)

DB transaction 10:
- **Txid:** `0000000000000000000000000000000000000000000000000000000000000001` (fake)
- **Status:** failed
- **Description:** `ROLLBACK TEST (synthetic)`
- **Satoshis:** -100
- **Created:** 2026-04-15 14:08:21 (3 hours after the last successful backup)
- **Raw tx:** NULL/empty
- **Outputs affected:** NONE (no outputs reference this tx via `spent_by` or `transaction_id`)

This is an **orphan test record** — it has no associated outputs, no raw transaction data, and a clearly synthetic txid. It appears to have been inserted manually or by a debugging tool. **It has no impact on the backup failure** and can be safely ignored or cleaned up.

No code path in the current Rust wallet codebase creates a "ROLLBACK TEST" transaction. It was not generated by the backup rollback mechanism (`rollback_backup` only sets status to Failed and restores spent outputs — it doesn't insert new transactions with synthetic descriptions).

---

## 10. Root Cause Analysis

### Eliminated hypotheses:

| Hypothesis | Status | Evidence |
|------------|--------|----------|
| H1: DB-chain divergence | **ELIMINATED** | DB tip matches chain tip exactly |
| H2: c5b sweep corrupted state | **ELIMINATED** | Sweep gate never triggered (len=2, gate>2) |
| H3: Wrong signing key | **ELIMINATED** | Signatures verify with the correct backup pubkey |
| H4: Wrong sighash preimage | **ELIMINATED** | Sighash computation matches known-good tx 8 |
| H5: Wrong locking script | **ELIMINATED** | DB script == on-chain script (100% byte match) |
| H6: Wrong satoshi value | **ELIMINATED** | DB value == on-chain value (1000 sats) |

### Remaining hypothesis: BEEF construction / broadcast-layer issue

The transaction is correctly signed but nodes reject it. The discrepancy must be in what the nodes RECEIVE vs what was SIGNED. The BEEF envelope wraps the raw tx with parent transaction ancestry. Potential failure modes:

1. **BEEF includes incorrect parent tx data**: If the BEEF embeds a parent transaction with different output scripts/values than what's actually on-chain, the node might use the BEEF's version for sighash computation (for SPV-validated unconfirmed parents), producing a different sighash.

2. **Timing issue with unconfirmed parent**: Tx 8 was broadcast just 2 minutes before tx 9. If tx 8 was still unconfirmed, the BEEF must include tx 8 as an unproven parent. If the BEEF construction includes tx 8's raw bytes incorrectly (e.g., an unsigned version cached from before signing), the node would see different outputs.

3. **BEEF ancestry chain depth**: The BEEF builder walks the ancestry chain (tx 8 → tx 7 → tx 6 → ...). If any ancestor is fetched incorrectly, the entire BEEF validation may fail at the node.

### Why it persists (July 13 attempts):

By now, tx 8 IS confirmed (block 944941). The BEEF should be simpler (tx 8 has a merkle proof, no need for deep ancestry). Yet the error persists, which suggests the issue is reproducible regardless of confirmation status. This slightly weakens the "unconfirmed parent" hypothesis and strengthens the "incorrect parent tx data in BEEF" hypothesis — unless the BEEF builder still includes stale cached data.

---

## 11. Recommended Next Diagnostic Steps

### Step A: Capture BEEF bytes at broadcast time
Add temporary logging in `do_onchain_backup` (Step 11) to save the serialized BEEF to a file before broadcast:
```rust
std::fs::write("/tmp/backup_beef_debug.hex", &beef_hex).ok();
```
This allows offline inspection of exactly what the BEEF contains.

### Step B: Test raw tx broadcast (no BEEF)
Attempt broadcasting the raw signed tx directly (without BEEF wrapping) via WoC's `/tx/raw` endpoint. If this succeeds, the issue is definitively in the BEEF construction. If it fails with the same error, the issue is in the raw tx itself (contradicting our signature verification — would indicate a subtle difference between ecdsa library behavior and BSV node behavior).

### Step C: Inspect the BEEF parent tx data
Parse the BEEF bytes (from Step A) and extract the parent transaction for `7855796d...`. Compare its raw bytes with the actual on-chain tx bytes. Any difference would explain the sighash mismatch.

### Step D: Enable INFO-level logging
The sighash function has INFO-level debug logging that captures the preimage hex. Enabling INFO-level logging for `hodos_wallet::transaction::sighash` during a backup attempt would capture the exact preimage the wallet computes, allowing comparison with what a node should compute.

### Step E: Try the BEEF-less broadcast path
If the issue is confirmed to be BEEF-related, consider a fallback: when BEEF broadcast fails with NULLFAIL, retry with raw tx broadcast (no BEEF). This would be a workaround, not a fix.

---

## 12. Wallet Health Summary

| Metric | Status |
|--------|--------|
| Wallet unlocked | ✅ Yes (mnemonic cached) |
| Normal transactions | ✅ Working (Paymail, Register, upvotes all succeed) |
| Master key derivation | ✅ Working (backup pubkey matches on-chain) |
| Backup signing | ✅ Signatures are mathematically valid |
| UTXO availability | ✅ Backup PushDrop + marker unspent on-chain |
| Balance | ~6.4M sats in backup change (spent by Paymail), other UTXOs available |
| DB integrity | ✅ No orphaned outputs, no phantom spends |
| Backup failure since | 2026-04-15 10:52:06 (tx 9, 3 min after last success) |

---

## 13. Key Data for Windows Claude

### Outpoints:
- **Current backup tip:** `7855796df0cf8c23fc366abee937c78994e79209ee5f44d24d337fea3325b280`
  - `:0` PushDrop (1000 sats, spendable=1, chain-unspent)
  - `:1` Marker (546 sats, spendable=1, chain-unspent, address `1APML3WyEUFrvwNig3qQ9861KyKDc74Nvo`)
  - `:2` Change (6,441,933 sats, spendable=0, spent by tx 12 Paymail)

### Backup pubkey:
`02b9d142939687a8e4f27e83b5530c07b10d30338e54b3392d4b8f6a7b7b5ca04b`
Derived via: `BRC-42 self-derivation, invoice="1-wallet-backup-1"`

### Backup address:
`1APML3WyEUFrvwNig3qQ9861KyKDc74Nvo` (= P2PKH of HASH160 of backup pubkey, verified)

### Failed tx 9 raw hex:
Stored in DB `transactions` table, id=9, txid=`76c47e92c4a481a680968ef21262ce719d690613617a1d6c3c3f4f7f4c4f358f`, 3144 bytes.

### Sighash verification details:
- hashPrevouts: `99b336d6043ba44d867e25b9899e1e5c01366a2ed0e4d14ec8e4c357ec79fdf1`
- hashSequence: `82a7d5bb59fc957ff7f737ca0b8be713c705d6173783ad5edb067819bed70be8`
- hashOutputs: `d4f0031ef5c0a148575dfc9971e56c925b71c0b18fe3195fde938acff2dd1b81`
- Input 0 sighash: `334b0baf72df1170b969ce576060c64ebe452b4b14d2d5e7a521f62044c8b0a3` (signature verifies ✅)

### DB snapshot location:
`/private/tmp/claude-501/-Users-matt/23e0a5f6-7966-45d3-83ea-73217e041fe8/scratchpad/backup-forensics/wallet.db` (read-only copy)
