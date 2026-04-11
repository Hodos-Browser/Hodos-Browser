# Wallet Backup Efficiency Plan

**Created:** 2026-04-11
**Status:** Planning — measurement pending
**Parent:** [`wallet-efficiency-and-bsv-alignment.md`](./wallet-efficiency-and-bsv-alignment.md)

---

## Problem statement

The on-chain wallet backup is currently costing 5–6K sats per backup after ~2 weeks of moderate usage. The original research projected this scale should remain in the "moderate" cost band ($0.68/year at daily backups, $40 BSV) but the **growth trajectory** is the concern: backup size grows monotonically with usage, and exponential growth would push the cost into the $5–20/year range within a few months. That's not catastrophic for individuals but is bad for product economics on the free tier.

The big-bang reduction (stripping `raw_tx` from confirmed transactions, decided 2026-03-25) is already applied. **Remaining wins are smaller, more nuanced, and carry more risk of breaking recovery edge cases.**

## Backup architecture (what we MUST NOT break)

See parent doc § "Backup architecture invariants — DO NOT BREAK" for the full list. Critical points:

1. PushDrop is nonstandard script → not address-indexable → must have a P2PKH marker as discovery anchor
2. Recovery flow: derive backup address → query marker → fetch tx → decrypt PushDrop at vout 0
3. Hash comparison gates each backup (skip if unchanged)
4. Encryption AFTER compression
5. Recovery is the most fragile path. Default to "keep recent operationally-relevant data, drop only old history"

### Hard rule: non-standard token preservation

**Any backup field that touches `outputs`, `transactions`, `addresses`, or proof tracking MUST explicitly verify it doesn't break recovery for:**

1. **PushDrop tokens (current and future protocols)** — non-discoverable by address scan, MUST be in the backup. Current uses are limited (certificate publish, wallet backup itself) but more BRC-X protocols using non-standard scripts will exist in the future.
2. **BRC-42 with external counterparty** — addresses derived from a third-party app's identity key, **cannot be re-derived from the master key alone**, MUST be preserved in the backup. These records live in the OUTPUTS table (`derivation_prefix` + `derivation_suffix` + `sender_identity_key`), not the addresses table.
3. **Future BRC-X protocols** — any protocol that creates non-standard scripts or non-HD/non-self derivations.

**Default position:** Assume any output where `derivation_prefix != "2-receive address"` AND `derivation_prefix != "bip32"`, OR where `sender_identity_key IS NOT NULL`, is **non-recoverable from master key alone** and MUST be preserved in the backup.

**No optimization may drop a record matching these criteria, regardless of age.** This is a load-bearing invariant for user fund safety. Token loss is permanent and unrecoverable.

### Hard rule: recovery code stays the same unless explicitly approved

The recovery code path is fragile and "took a long time to get working." Optimizations that would require changes to `wallet_recover_onchain`, `wallet_restore`, or `import_to_db` are HIGH RISK and require explicit pre-approval. **Verified 2026-04-11 by reading `handlers.rs:12443`: recovery does NOT do sequential address derivation — it deletes auto-created addresses and imports from the backup payload directly.** Dropping addresses entirely would silently break receive-UTXO discovery on recovery. We use only optimizations that work with the existing recovery code unchanged.

## Current strip list (already implemented, verified by code reading)

| Field | Where | Reason |
|---|---|---|
| `mnemonic` | `backup.rs:988` | Re-entered on recovery |
| `raw_tx` for confirmed transactions (with `proven_tx_id`) | `backup.rs:447-448` | On-chain, free to re-fetch |
| `locking_script` for spent outputs (`spendable=false`) | `backup.rs:498-499` | Never needed once spent |
| `raw_tx` in `proven_txs` | `backup.rs:544-551` | On-chain, free to re-fetch |
| `merkle_path` in `proven_txs` (entire field) | `backup.rs:996-998` | **Big win** — proofs are large, free to re-derive via WoC TSC endpoint |
| `raw_tx` in `proven_tx_reqs` | `backup.rs:992-995` | Re-fetchable |
| `input_beef` in `proven_tx_reqs` | `backup.rs:992-995` | Re-fetchable |
| `parent_transactions` table (entire) | `backup.rs:991` | Re-buildable from chain |
| Orphan FK references | `backup.rs:1000-1031` | Cleanup pass |

## Measurement plan (DO BEFORE ANY OPTIMIZATION)

Without per-field byte counts from a real wallet, every optimization estimate is guesswork. The diagnostic patch is already in place.

### How to run the measurement

1. **Already done:** `[DIAG-BACKUP]` instrumentation patch added to `compress_payload` in `backup.rs:1037+`. Build verified clean.
2. Start the wallet (`cargo run --release` from `rust-wallet/`).
3. Trigger a backup. Two ways:
   - **Wait for the monitor.** TaskBackup runs every 30s. Make any small change first (e.g., navigate to a domain to trigger a domain_permission update, or send a small tx) so the hash check doesn't skip.
   - **Hit the endpoint manually:** `curl -X POST http://127.0.0.1:31301/wallet/backup/onchain -H "Content-Type: application/json" -d '{}'`
4. Capture the wallet log lines tagged `📊 [DIAG-BACKUP]`. They look like:
   ```
   📊 [DIAG-BACKUP] === payload composition (raw JSON, pre-gzip) ===
   📊 [DIAG-BACKUP] transactions               12345 bytes
   📊 [DIAG-BACKUP] outputs                    23456 bytes
   ...
   📊 [DIAG-BACKUP] (counts: tx=42, out=87, addr=15, ptx=42)
   ```
5. Paste the captured output into this doc under § "Measurement results" below.
6. **Revert the diagnostic.** It adds N+1 redundant `serde_json::to_value` passes per backup — fine for one-shot measurement, too expensive for production.

### Measurement results — captured 2026-04-11

Real wallet measurement on a moderate-active wallet (105 transactions, 293 outputs, 170 addresses, 121 proven_txs, 10 certificates, ~25 spendable UTXOs out of 293 total — meaning **268 of the outputs are spent**).

| Rank | Field | Bytes | % of total | Per-record | Notes |
|---|---|---|---|---|---|
| 1 | **outputs** | **189,645** | **48.8%** | ~647 B/record (293) | **Dominant cost.** ~92% of records are spent. |
| 2 | transactions | 46,899 | 12.1% | ~447 B/record (105) | After raw_tx strip. |
| 3 | proven_tx_reqs | 45,297 | 11.6% | ~374 B/record (121) | `history` field is unbounded log. |
| 4 | addresses | 40,768 | 10.5% | ~240 B/record (170) | All HD/BRC-42-self derivable. |
| 5 | proven_txs | 35,797 | 9.2% | ~296 B/record (121) | After raw_tx + merkle_path strip. |
| 6 | tx_labels_map | 7,977 | 2.1% | — | |
| 7 | certificates | 6,384 | 1.6% | ~638 B/record (10) | |
| 8 | certificate_fields | 5,470 | 1.4% | — | |
| 9 | output_tag_map | 3,383 | 0.9% | — | |
| 10 | output_baskets | 1,877 | 0.5% | — | |
| 11 | domain_permissions | 1,702 | 0.4% | — | |
| 12 | block_headers | 1,442 | 0.4% | — | |
| 13 | tx_labels | 679 | 0.2% | — | |
| 14 | cert_field_perms | 398 | 0.1% | — | |
| 15 | output_tags | 220 | 0.1% | — | |
| 16 | users | 171 | 0.04% | — | |
| 17 | settings | 155 | 0.04% | — | |
| ✓ | **parent_transactions** | **2** | **~0%** | — | **Verified: clear() strip working** |
| ✓ | commissions | 2 | ~0% | — | (empty for this wallet) |
| ✓ | sync_states | 2 | ~0% | — | (empty for this wallet) |

**Total: 388,814 bytes JSON → 69,882 bytes compressed (82.0% reduction).**
**Backup tx fee at 100 sat/KB: ~7,045 sats** (broadcast txid `4520fd5487a9dcfdd112415f312e9eb0c51c1db7b769f48a5db58c4a76f88a7a`).

### Key insights from the measurement

1. **Outputs is HALF THE BACKUP (48.8%).** Spent-output time-tiered strip is by far the highest-leverage optimization. With ~92% of outputs being spent, even a conservative 30-day cutoff drops most of this field.

2. **Top 5 fields = 92.2% of the backup.** Everything else combined is rounding error. Optimizing fields outside the top 5 is not worth the engineering time.

3. **`addresses` is 10.5%.** All address records in the table are HD/BRC-42-self derivable (per the `addresses` table schema: `index 0+` = derived, `index -1` = master pubkey, `index -2` = external placeholder for custom scripts). **HOWEVER**, code reading at `handlers.rs:12443` confirmed that recovery does NOT do sequential derivation — it executes `DELETE FROM addresses WHERE wallet_id = ?1` and then imports addresses directly from the backup payload. **Dropping addresses entirely from the backup would leave the recovered wallet with zero address records**, which would break receive-UTXO discovery and the receive-address UI. We will NOT drop addresses entirely. Instead, see the time-tiered address strip in candidate optimizations below.

4. **`proven_tx_reqs` is 11.6%, with 374 bytes per record.** The `history` field is confirmed to be an unbounded JSON audit log appended on every status transition (`add_history_note(id, event, details)` per `database/CLAUDE.md`). Capping history to last N entries is safe and reduces this field.

5. **`proven_txs` looks smaller than expected to drop.** After raw_tx + merkle_path strips, what remains (296 bytes/record) is the FK linkage between transactions and proofs. Dropping it would orphan the `proven_tx_id` FK on transactions, requiring either nulling those FKs or adding rebuild-on-recovery logic. **Verdict: keep as-is.** 9.2% of backup is acceptable for the safety guarantee.

6. **Validation passed:** `parent_transactions` is 2 bytes (essentially empty). The existing `payload.parent_transactions.clear()` strip at `backup.rs:991` is working correctly.

---

## Candidate optimizations

### 1. Spent-output time-tiered strip (HIGHEST CONFIDENCE)

**Goal:** Drop spent outputs older than 30 days entirely from the backup. Keep recent ones with all fields.

**Why time-tiered, not "drop all":** A code audit found 22 hits across 11 files where spent outputs are read. Three categories matter:

| Use case | Example code | Field needs | Implication |
|---|---|---|---|
| Cert reclaim via `spending_description LIKE 'pending-%'` | `handlers.rs:5453, 11771`, `output_repo.rs:835/870/896` | `spending_description`, `txid`, `vout` | Need recent records intact |
| PushDrop token lookups by `txid+vout` | `certificate_handlers.rs:5236/5335`, `task_replay_overlay.rs:98` | `txid`, `vout`, `output_id` | Need recent records intact for in-flight unpublish/replay |
| Status reconciliation | `task_review_status.rs:79/119` | `spendable`, `spent_by`, `transaction_id` | Operates on local DB only; safe after restore |
| Old purge | `output_repo.rs:982` (`DELETE FROM outputs WHERE spendable=0 AND updated_at<?`) | (none — just deletes) | **Already locally drops old spent outputs**; backup should match |

**Critical safety insight:** The local DB already purges spent outputs older than some threshold via `output_repo.rs:982`. The backup is currently MORE conservative than local storage, keeping things the local DB has thrown away. **Aligning the backup strip rule with the existing local-purge rule is by-construction safe** — we're not introducing new logic, we're just propagating existing logic into the backup serialization.

**Proposed implementation:**

```rust
// In compress_for_onchain after line 988 (mnemonic clear):

// Drop spent outputs older than the local-purge threshold.
// Local DB already drops these via output_repo.rs:982 — backup should match.
const SPENT_OUTPUT_BACKUP_RETENTION_SECS: i64 = 30 * 24 * 60 * 60; // 30 days
let now = current_unix_timestamp();
payload.outputs.retain(|o| {
    o.spendable || (now - o.updated_at) < SPENT_OUTPUT_BACKUP_RETENTION_SECS
});
```

**Open question:** What threshold does `output_repo.rs:982` actually use? Need to read the caller (probably a monitor task). If it's not 30 days, match it. If it has no fixed threshold (only purges on explicit request), pick 30 days as a defensible default.

**Estimated impact:** Unknown until measurement. Hypothesis: 30–60% reduction in `outputs` field size on a moderately-used wallet.

**Risk:** Low IF the threshold matches the local purge. Medium if we're more aggressive than local purge (could affect in-flight cert reclaim that started >30 days ago — rare but possible).

**Verification:** After implementing, run a backup, then `wallet_backup_onchain_verify` (`handlers.rs:12211`) — confirm count diffs only show old spent outputs as missing, nothing else.

---

### 1b. Address time-tiered strip (NEW — added post-measurement)

**Goal:** Drop "operationally dead" addresses from the backup. Same pattern as the spent-output time-tiered strip, applied to addresses. **Does not require recovery code changes** — recovery still inserts the kept addresses normally.

**An address is "operationally dead" if ALL of the following are true:**
- `used = true` (it's been used at some point)
- It has zero spendable outputs in the wallet (all its UTXOs are spent)
- `pending_utxo_check = false` (not flagged for active sync)
- More than 30 days since last activity (`updated_at`)
- `index >= 0` (NOT the master pubkey at index -1, NOT the external placeholder at index -2)

**Why this is safe (verified by code reading at `handlers.rs:12443`):**
- Recovery deletes auto-created addresses then imports from backup. We keep all the operationally-relevant addresses (active, unused, pending-check, special indices) — recovery imports those normally.
- Dropped addresses have no funds, no in-flight operations, no expected payments.
- New address generation uses `wallet.current_index` (in the wallets table, not the addresses table) — unaffected.
- Receive UTXO upsert flow creates new address records on demand if a payment somehow arrives at a dropped address — graceful degradation, not failure.

**What we lose:**
- Receive-address UI shows fewer historical addresses (acceptable history loss).
- Address-level history view ("what happened at address X 6 months ago") doesn't work for dropped addresses.
- A late payment to a dropped address wouldn't be auto-discovered until manual rescan.

**What we DO NOT lose:**
- ✅ Any current spendable balance (outputs are independent)
- ✅ Any in-flight transactions
- ✅ Any tokens (PushDrop or otherwise — those live in OUTPUTS)
- ✅ Any BRC-42-with-external-counterparty derivations (also in OUTPUTS)
- ✅ Recovery functionality — no schema changes needed

**Implementation:**

```rust
// In compress_for_onchain after the spent-output strip:
const ADDRESS_BACKUP_RETENTION_SECS: i64 = 30 * 24 * 60 * 60; // 30 days
let now = current_unix_timestamp();

// Build a set of address indices that have spendable outputs
let active_indices: std::collections::HashSet<i32> = payload.outputs.iter()
    .filter(|o| o.spendable)
    .filter_map(|o| {
        // Extract the HD index from derivation_suffix if it's a wallet receive address
        if o.derivation_prefix.as_deref() == Some("2-receive address") {
            o.derivation_suffix.as_deref().and_then(|s| s.parse::<i32>().ok())
        } else { None }
    })
    .collect();

payload.addresses.retain(|a| {
    // ALWAYS keep special indices (master, external placeholder)
    if a.index < 0 { return true; }
    // ALWAYS keep unused addresses (might receive)
    if !a.used { return true; }
    // ALWAYS keep addresses pending UTXO check
    if a.pending_utxo_check { return true; }
    // ALWAYS keep addresses with active spendable UTXOs
    if active_indices.contains(&a.index) { return true; }
    // ALWAYS keep recent addresses
    if (now - a.created_at) < ADDRESS_BACKUP_RETENTION_SECS { return true; }
    // Otherwise, drop (operationally dead)
    false
});
```

**Estimated impact** (from measurement analysis): If 80% of the 170 addresses fit the "operationally dead" criteria, drop ~136 records → save ~33 KB raw → ~5 KB compressed → ~500 sats per backup. Modest but real.

**Risk:** Low. The filter is conservative (5 inclusion clauses, drop only if NONE match). No recovery code changes.

---

### 2. Strip `block_headers` entirely

**Goal:** Stop including `block_headers` in the on-chain backup.

**Why safe:** Block headers are public, deterministic, free to fetch from any block explorer. They're cached locally for SPV verification of older transactions, but a fresh recovery doesn't need historical headers — they're re-fetched on demand by the proof verification path.

**Implementation:**

```rust
// In compress_for_onchain after the parent_transactions clear:
payload.block_headers.clear();
```

Plus a corresponding background task or post-restore step that re-populates the block_headers cache as transactions are re-validated. **That post-restore step probably already exists** — header lookup is handled by `cache_helpers.rs` which fetches on miss. So nothing else is needed; just clear the field.

**Estimated impact:** Unknown. Hypothesis: 5–25 KB raw → 1–5 KB compressed savings. Depends on how many headers are cached.

**Risk:** Very low. Header fetching is already lazy. Worst case: first few seconds after recovery have to fetch a handful of headers from WoC.

---

### 3. Old confirmed-transaction collapse

**Goal:** Reduce confirmed transactions older than 90 days to a minimal accounting stub.

**Current state:** `BackupTransaction` keeps all metadata fields. For old confirmed transactions, most of these fields (description, reference_number, lock_time, version) are pure history.

**Proposed minimal stub for old confirmed transactions:**
- Keep: `id`, `user_id`, `proven_tx_id`, `txid`, `status`, `is_outgoing`, `satoshis`, `created_at`, `updated_at`
- Drop: `description`, `reference_number`, `version`, `lock_time`, `block_height`, `confirmations`, `failed_at` (these can be re-derived from the on-chain transaction or are not needed)

**Implementation sketch:**

```rust
const OLD_TX_THRESHOLD_SECS: i64 = 90 * 24 * 60 * 60; // 90 days
let now = current_unix_timestamp();
for tx in &mut payload.transactions {
    let is_old_confirmed = tx.proven_tx_id.is_some()
        && (now - tx.updated_at) > OLD_TX_THRESHOLD_SECS;
    if is_old_confirmed {
        tx.description = None;
        tx.reference_number = String::new();
        tx.version = 1;
        tx.lock_time = 0;
        tx.block_height = None;
        tx.confirmations = 0;
        tx.failed_at = None;
    }
}
```

**Estimated impact:** Unknown until measurement. Likely modest unless the wallet has many old transactions with long descriptions.

**Risk:** Low. Dropped fields are display-only. Recovery shows old transactions with less detail until re-fetched.

**UX cost:** After recovery, the activity view for old transactions shows minimal detail (just amount and date). User clicks for detail → triggers background re-fetch from WoC → details populate within seconds. Acceptable IF the user is told via a one-time post-recovery message.

---

### 4. Binary serialization (CBOR or MessagePack)

**Goal:** Replace JSON with a binary serialization format before gzip. Expected 10–30% additional reduction.

**Why:** JSON has overhead even after gzip — repeated field name strings, quoting, escaping, base64-encoded binary blobs. CBOR or MessagePack encode the same data in a binary format that compresses tighter and avoids the base64 overhead entirely (binary blobs stored as native bytes).

**Tradeoff:** Backup format version bump. Old backups must still be readable by `wallet_recover` for users with backups in the old JSON format. Need migration logic.

**Implementation effort:** Medium. The serde structs don't change — only the serializer at `backup.rs:1042`:
```rust
// Before:
let json_bytes = serde_json::to_vec(payload)?;

// After:
let cbor_bytes = serde_cbor::to_vec(payload)?;
```
Plus the inverse on the recovery side, plus a format-version field to discriminate which decoder to use, plus a one-time migration test that confirms an old JSON backup can still be decoded.

**Estimated impact:** 10–30% additional reduction on top of current size. Variable.

**Risk:** Medium. Format bump is irreversible — once a wallet writes a CBOR backup, all future versions must be able to read CBOR (forward-compat). Recovery code must support BOTH formats (backward-compat).

**Recommendation:** Defer to P2. If P0+P1 wins are sufficient, skip this entirely — the migration cost may not be worth the marginal reduction.

---

### 5. UTXO consolidation — five strategies, ranked by privacy

**Goal:** Reduce the number of spendable UTXOs over time. **Spendable UTXO count is the dominant unbounded growth source** in the backup once historical data is windowed (see § "Growth strategy" below). Every UTXO removed from the spendable set saves ~200 bytes from every future backup.

The privacy concern is real — naive consolidation links many addresses together publicly. But there are multiple strategies with very different privacy profiles. We can pick a combination that gives most of the size win with minimal privacy cost.

#### Strategy 5.1: Lazy consolidation in real sends ⭐⭐⭐⭐⭐ (cleverest)

**The idea:** Don't make consolidation transactions at all. When the user sends a real payment, the coin selection algorithm normally picks the smallest sufficient set of UTXOs. **Change it to greedily include extra small UTXOs that bring the input set up to the recipient's amount + a fee buffer.**

**Why it's brilliant for privacy:**
- The transaction is a NORMAL payment to a real recipient
- An observer sees "wallet sent 50000 sats to address X" — exactly what wallets do all the time
- The fact that 12 inputs were combined instead of 2 is invisible — there's no "consolidation transaction" to flag
- **Zero new linkage created** — those inputs were going to be spent eventually anyway
- Reduces backup size with every real transaction the user already makes

**Cost:** Slightly higher tx fees (more inputs = bigger tx body). At BSV fee rates (~250 sat/KB), each extra input adds ~40 bytes ≈ ~10 sats. Trivial.

**Implementation:** Modify UTXO selection in `create_action` (`handlers.rs`) to prefer "select small UTXOs first up to N inputs" over "select fewest UTXOs." Tunable threshold like "include any UTXO ≤ 5000 sats if total inputs ≤ 50."

**Privacy leak:** Effectively zero. Looks like any other send.

**Estimated impact:** Bounds the spendable UTXO count to roughly the last few weeks of receives that haven't been spent yet. Critical for keeping growth flat.

#### Strategy 5.2: Same-counterparty consolidation ⭐⭐⭐⭐ (no new linkage)

**The idea:** For BRC-42 derived UTXOs (PeerPay receives), all UTXOs from the same sender are derived from the same `sender_identity_key`. **The sender already knows all these addresses belong to you.** Consolidating only same-sender UTXOs creates **no new linkage** — the sender already had the information.

**Privacy leak:** Reveals to the SENDER (not the public) "I had this many of your payments unconsolidated." But the sender already knew they paid you N times.

**Implementation:** New endpoint `POST /wallet/consolidate/by-sender` that takes a `sender_identity_key` and merges all UTXOs from that sender into one output. UI prompt: *"You have 23 small payments from `arch@handcash`. Combine them into one larger UTXO?"*

**Bonus:** This is automatable with high confidence. The query is straightforward — `SELECT outputs WHERE sender_identity_key = ? AND spendable = 1`. Could even run as a background task with no UI prompt.

**Estimated impact:** High for users who receive many small PeerPay payments from the same source.

#### Strategy 5.3: Dust-threshold consolidation ⭐⭐⭐ (rational behavior)

**The idea:** Below some satoshi threshold (e.g., 1000 sats), UTXOs are "economically irrational" to keep separate — the fee to spend them is comparable to their value. **Auto-consolidate dust UTXOs whenever they exceed N count.**

**Privacy leak:** Observers see "this wallet consolidated dust." It's a known wallet behavior pattern (Bitcoin Core has dust thresholds). The leak is "this user accumulated some dust" — not particularly identifying. The links between the consolidated dust UTXOs are NEW linkage, but they're between economically-low-value UTXOs that an observer wasn't paying close attention to anyway.

**Implementation:** New monitor task `task_consolidate_dust.rs` that runs every 24 hours. If the wallet has > 20 UTXOs below 1000 sats, build a single tx that consolidates them. Opt-out via setting.

**Risk:** Creates linkage between counterparties whose payments became dust. Less informational than full consolidation but not zero.

#### Strategy 5.4: Same-basket consolidation ⭐⭐ (acceptable for power users)

**The idea:** Each output has a `basket_id` indicating purpose ("default", "peerpay", "marketplace"). Consolidating within a basket reveals "this user has this many UTXOs in this purpose category" but preserves inter-basket privacy.

**Privacy leak:** Reveals basket size for each consolidated category. Users with multiple baskets retain inter-basket privacy.

**Implementation:** User-initiated only, with UI showing "Combine all 47 UTXOs in basket 'marketplace'?"

#### Strategy 5.5: Aggressive cross-counterparty consolidation ⭐ (worst — but the strongest size win)

**The idea:** Wait until N UTXOs accumulate, then consolidate everything in one transaction.

**Privacy leak:** Makes the entire wallet's address graph publicly linkable in one transaction. **This is what the user explicitly wanted to avoid as automatic behavior.**

**Verdict:** Available only as a user-initiated power-user feature with a full privacy warning modal:

> "This will combine ALL of your spendable UTXOs into a single output. Anyone watching the blockchain will be able to see that all the addresses being combined belong to the same wallet. This permanently links your entire transaction history. Recommended only if you understand the privacy tradeoff and prioritize storage cost over privacy. Continue?"

#### Recommended combination

**Auto-enabled by default:**
- ✅ **5.1 Lazy consolidation in real sends** — invisible, free, beneficial for everyone
- ✅ **5.3 Dust-threshold consolidation** — opt-out, runs daily, ≤1000 sat UTXOs only

**User-initiated, single-click UI (no warning needed):**
- ✅ **5.2 Same-counterparty consolidation** — "Combine 23 payments from arch@handcash"

**User-initiated, with privacy warning modal:**
- ⚠️ **5.4 Same-basket consolidation**
- ⚠️ **5.5 Aggressive cross-counterparty consolidation**

**The combination of (5.1) + (5.3) alone keeps spendable UTXO count bounded for nearly all real users without ever showing them a privacy warning.** Strategy 5.1 catches users who actively transact, strategy 5.3 catches users who only receive. Together they make the spendable UTXO count growth approach zero.

**Effort:**
- 5.1: 1 day (modify existing coin selection logic in `create_action`)
- 5.2: 1 day (new endpoint + UI prompt)
- 5.3: 1 day (new monitor task + opt-out setting)
- 5.4 + 5.5: 1 day combined (one endpoint with parameters, one UI screen with warnings)

Total for the recommended auto-enabled pair (5.1 + 5.3): 2 days.

---

## Growth strategy: making the curve flat

**The user's question is the right one:** the static number doesn't matter, the slope matters. Even a small per-backup cost becomes a big problem if the curve goes up forever. The acceptance criterion should be **"growth approaches zero,"** not "size below X bytes."

### Where growth comes from (after current strips)

| Source | Per-item cost | Scales with |
|---|---|---|
| New `transactions` records | ~150–300 bytes (raw_tx already stripped) | Every send + receive |
| New `outputs` records | ~100–250 bytes (locking_script stripped if spent) | Every received UTXO |
| New `proven_txs` records | ~150 bytes (raw_tx + merkle_path stripped) | Every confirmed tx |
| New `addresses` records | ~100 bytes | Sub-linearly (HD index reuse) |
| New `certificates` | ~500 bytes | Sub-linearly (you don't acquire many) |
| New `commissions` | ~200 bytes | Every send (Hodos service fee record) |

**Math after current strips:** ~600–900 bytes raw → ~50–100 bytes compressed per transaction. At 5 transactions per day, that's ~250–500 bytes/day compressed. Over a year that's ~100–200 KB compressed → ~25–50K sats per backup. **The trajectory is linear, not exponential, but linear is still bad in a context where users expect "free."**

### The path to flat growth

The only way to make growth FLAT is to **drop OLD entries at the same rate NEW entries arrive.** Three approaches considered:

#### Approach A: Time-window backup (sliding window)

**Idea:** The on-chain backup contains only data from the last N days. Older entries are dropped. The user can re-derive history from the chain (background job) if they want a longer historical view.

- ✅ Bounds: transactions, proven_txs, spent outputs, commissions, tx_labels
- ❌ Doesn't bound: spendable outputs (UTXO set), certificates, addresses

Result: backup size = `f(spendable UTXO count) + f(certificates) + g(last N days of activity)`. The first term is the dominant unbounded growth — but **UTXO consolidation directly attacks this**.

#### Approach B: Pure pointers (state snapshot only)

**Idea:** Store only the current STATE in the backup (UTXO set, certs, identity). NO history. History lives on-chain, fetchable by address scan during recovery.

- ✅ Truly flat backup growth
- ❌ More complex recovery; slower full restoration
- ⚠️ Concern: BRC-42 derived outputs (PeerPay) aren't address-scannable. Each derivation is unique per sender. A fresh recovery could miss PeerPay history unless the sender list is preserved. Mitigation: back up just `(sender_identity_key, last_received_index)` per counterparty — extremely small.

#### Approach C: Hybrid (RECOMMENDED)

**Combine A and B for the best of both worlds:**

**Always backed up (small, bounded by user's current state):**
- Wallet metadata, identity, mnemonic-derived state
- All currently spendable UTXOs (kept small via consolidation)
- All certificates
- All baskets, tags, labels (user-defined and small)
- Settings, sync_states, domain_permissions
- Per-counterparty PeerPay state `(sender_identity_key, last_received_index)`

**Backed up for the last N days only (sliding window):**
- Transactions (history)
- proven_txs older than the window dropped (proofs are re-fetchable)
- Spent outputs older than the window dropped (already proposed in P0)

**Never backed up (re-derivable):**
- Block headers (already proposed in P0)
- parent_transactions (already excluded)
- raw_tx for confirmed (already excluded)
- merkle_path for proven_txs (already excluded)

### The size formula under Approach C

```
backup_size ≈ (spendable_utxo_count × 200B)
            + (cert_count × 500B)
            + (basket_count × 100B)
            + (recent_N_days_tx_count × 200B)
            + ~5KB constant overhead
```

### Projected sizes (revised after 2026-04-11 measurement)

These projections are calibrated against the actual measurement data: 70 KB compressed, 7K sats fee, with 293 outputs (268 spent), 105 transactions, 121 proven_txs, 121 proven_tx_reqs, 170 addresses, 10 certs.

**After P0 optimizations applied to the measured wallet:**

| Field | Now (raw) | After (raw) | How |
|---|---|---|---|
| outputs | 189 KB | ~30-50 KB | Drop spent > 30 days. Assume 60% of 268 spent records are >30 days old → drop ~160 records → save ~104 KB. Conservative. |
| addresses | 41 KB | ~10-15 KB | Time-tiered strip. Assume 70% are operationally dead → drop ~120 records → save ~28 KB. |
| proven_tx_reqs | 45 KB | ~25-30 KB | Cap history to last 5 entries → estimated ~40% reduction. |
| transactions | 47 KB | ~20-25 KB | 60-day sliding window. Assume ~50% are older. |
| proven_txs | 36 KB | 36 KB | Unchanged (retracted dropping entirely). |
| block_headers | 1.4 KB | 0 KB | Drop entirely. |
| Everything else | ~30 KB | ~30 KB | Already small. |
| **Raw total** | **~389 KB** | **~150-185 KB** | **~52-61% reduction** |
| **Compressed (assume same 82% ratio)** | **~70 KB** | **~27-33 KB** | |
| **Sats/backup at 100 sat/KB** | **~7,000** | **~2,700-3,300** | |

**Realistic target after P0: ~3,000 sats per backup. Goal of ≤5K achieved with comfortable margin.**

### Growth behavior (the more important question)

**Each P0 strip is bounded by either current state or a sliding window:**
- Spent outputs: bounded by 30-day window + spendable count
- Addresses: bounded by active addresses (those with spendable UTXOs) + 30-day window
- proven_tx_reqs.history: hard cap at 5 entries per record
- Transactions: bounded by 60-day window
- proven_txs: scales with transactions (not bounded), but each record is small after stripping

**The unbounded contributors** (after P0) are: `proven_txs` (linear with all-time tx count, ~296 B/record), `certificates` (sub-linear), and `outputs` for currently-spendable UTXOs (controlled by Strategy 5.1 lazy consolidation in P1).

**Without P1 (consolidation), growth is bounded but slow:**
- Per new transaction: ~296 B (proven_txs) + 200 B (transactions, until it ages out of window) → ~50 B compressed per tx
- 5 tx/day × 365 days = ~90 KB/year raw → ~15 KB/year compressed → ~1500 sats/year added per backup
- After 5 years: ~3K + 7.5K = ~10.5K sats per backup. Slow growth.

**With P1 (lazy consolidation + dust consolidation), the spendable UTXO term stays flat AND the proven_txs term is the only remaining growth source.** With sliding window for transactions, proven_txs becomes the asymptote — for a 5-tx/day user that's about 1825 records/year × 296 B = 540 KB/year raw → ~90 KB compressed → ~9K sats/year added.

**Hmm — proven_txs is the long-term issue.** For truly flat growth we'd need to also sliding-window proven_txs, which means handling the FK orphan problem (drop proven_txs entries > 60 days AND null the corresponding `proven_tx_id` on transactions in the same window). This is doable but adds complexity. **Reframed as P2 candidate** rather than skipped entirely. See "Future considerations" below.

### Implementation order for flat growth

1. **P0 (already in plan):** Spent-output time strip + block_headers strip
2. **P0 (NEW):** Add sliding window for `transactions` and `proven_txs` — drop entries older than 30 or 60 days. This converts the linear growth term into a bounded rolling window. **Implementation is small** — same pattern as the spent-output strip, just with different table and threshold.
3. **P1 (NEW):** Lazy consolidation in real sends (Strategy 5.1). Modify coin selection to greedily eat small UTXOs during normal sends. **Reduces UTXO count over time without any new transactions.**
4. **P1 (NEW):** Auto dust consolidation (Strategy 5.3). Background task with opt-out.
5. **P2:** User-initiated same-counterparty consolidation (Strategy 5.2)
6. **P2:** Old confirmed-transaction collapse (already in plan, lower priority now that the sliding window does most of this work)

Items 1+2 alone get us flat growth on the historical data side. Items 3+4 get us bounded growth on the spendable UTXO side. Together: **flat backup growth.**

---

## Out of scope for this plan

These were considered and intentionally NOT included:

- **Tiered hot/cold backup** (off-chain encrypted file for cold history) — Real value but huge complexity, two storage paths, two recovery flows. Defer until P0–P2 ship and we measure whether more is needed.
- **Delta backups** (full snapshot + diffs) — Complicates recovery (must walk a chain). Defer.
- **Stripping `addresses` table** — Used for HD index gap-limit during recovery. Cannot strip without breaking recovery.
- **Stripping `certificates`** — Identity-critical, must persist.
- **Increasing PushDrop output amount above 1000 sats** — Doesn't help size, only changes how much is locked.

## Verification protocol for every backup-affecting change

After implementing any item from the candidate list, before merging:

1. **Build clean** — `cargo build --release` from `rust-wallet/`. Zero new warnings related to the change.
2. **Run a backup against a real wallet** — `POST /wallet/backup/onchain`. Capture the new compressed size from existing logs at `backup.rs:1052`.
3. **Run the verify endpoint** — `POST /wallet/backup/onchain/verify`. This compares the on-chain backup against the live DB and reports count diffs. The diff must be **exactly the records you intentionally dropped, nothing else**.
4. **Recovery dry run on a fresh DB** — Wipe a test wallet, recover from the backup that was just made. Confirm the wallet is functional (can list balance, list outputs, send a small test transaction).
5. **Document the size delta** — In the commit message: "Backup size before: X bytes, after: Y bytes (Z% reduction)."

Skipping any of these steps risks shipping a recovery-breaking change.

## Rollback plan

Each optimization is committed as its own atomic commit. If a problem is found post-merge, `git revert <hash>` undoes that single optimization without affecting others. The format-version-bump optimizations (binary serialization) are NOT revertible once a user has written a backup in the new format — those must be additionally gated on a wallet setting that defaults to off until widely tested.

---

## Acceptance criteria

This plan is "done" when:

1. Measurement is captured and pasted into this doc
2. Each P0 item has been implemented OR explicitly skipped with documented reasoning
3. **Static target — backup compressed size on a moderate wallet:** ≤ **5K sats** is the goal, ≤ 10K sats is acceptable
4. **Growth target — the slope, not just the value:** simulated 6-month wallet activity (or projection from real measurement) must show backup size **growing by less than 2× from month 1 to month 6**. A flat curve is the win condition. A 2× cap is the soft acceptance bar; 1× (truly flat) is the goal.
5. Recovery has been dry-run-tested for every optimization
6. The DIAG-BACKUP patch has been reverted
7. `ONCHAIN_FULL_BACKUP_RESEARCH.md` has been updated with the new strip list
8. The post-beta3-cleanup doc has a session log entry summarizing the results

**The growth criterion (#4) is the load-bearing one.** A wallet that ships at 5K sats per backup but grows 5×/year is worse than a wallet that ships at 8K sats per backup but stays at 8K forever. The goal is the slope.

---

*Implementation gates on the parent doc's planning workflow. Do not start any item before measurement is complete.*
