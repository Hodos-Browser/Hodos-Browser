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

### Measurement results

> **TODO** — paste DIAG-BACKUP log output here once captured.

```
📊 [DIAG-BACKUP] === payload composition (raw JSON, pre-gzip) ===
📊 [DIAG-BACKUP] transactions                ???? bytes
📊 [DIAG-BACKUP] outputs                     ???? bytes
📊 [DIAG-BACKUP] addresses                   ???? bytes
📊 [DIAG-BACKUP] proven_txs                  ???? bytes
📊 [DIAG-BACKUP] proven_tx_reqs              ???? bytes
📊 [DIAG-BACKUP] certificates                ???? bytes
📊 [DIAG-BACKUP] certificate_fields          ???? bytes
📊 [DIAG-BACKUP] output_baskets              ???? bytes
📊 [DIAG-BACKUP] output_tags                 ???? bytes
📊 [DIAG-BACKUP] output_tag_map              ???? bytes
📊 [DIAG-BACKUP] tx_labels                   ???? bytes
📊 [DIAG-BACKUP] tx_labels_map               ???? bytes
📊 [DIAG-BACKUP] commissions                 ???? bytes
📊 [DIAG-BACKUP] settings                    ???? bytes
📊 [DIAG-BACKUP] sync_states                 ???? bytes
📊 [DIAG-BACKUP] parent_transactions         ???? bytes  ← should be near zero (cleared)
📊 [DIAG-BACKUP] block_headers               ???? bytes
📊 [DIAG-BACKUP] domain_permissions          ???? bytes
📊 [DIAG-BACKUP] cert_field_perms            ???? bytes
📊 [DIAG-BACKUP] users                       ???? bytes
```

The order of optimizations below should be re-prioritized once these numbers are in.

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

### 5. UTXO consolidation (USER-INITIATED)

**Goal:** Allow users to merge many small UTXOs into one larger output, dramatically reducing the number of `outputs` records in the backup.

**The privacy tradeoff (the user's concern):**

| Approach | Backup impact | Privacy impact |
|---|---|---|
| Aggressive auto-consolidation | Huge | Bad — links all addresses |
| Same-counterparty consolidation only | Modest | Acceptable — counterparty already knows the addresses |
| Time-spaced consolidation | Modest | Better — observers see one consolidation tx, not full graph |
| **User-initiated, with explicit warning** | **User choice** | **User aware of tradeoff** |

**Recommendation:** Implement as a user-initiated feature only, accessible from wallet settings or wallet panel. UI must explain the privacy implication clearly:

> "Consolidating UTXOs combines many small outputs into one larger one. This reduces wallet storage and backup size, but creates a single transaction that publicly links all the addresses being combined. Anyone watching the blockchain can see they belong to the same wallet. Recommended for power users with many small UTXOs from PeerPay/marketplace activity."

**Implementation pieces:**
- New endpoint `POST /wallet/consolidate` that takes a basket filter and a target output count (default 1)
- New UI control in wallet panel: "Consolidate UTXOs" button with privacy warning modal
- Selection logic: choose all UTXOs in the same basket (or all spendable UTXOs if user opts to merge across baskets)
- Build a single transaction with N inputs, 1 output (or M outputs if user wants partial consolidation)
- Standard service fee applies

**Estimated impact:** Massive for power users (1000 outputs → 1 output is a 99% reduction in `outputs` field size). Zero impact for users who don't use it.

**Risk:** Low. Standard tx-building path, no new crypto, no schema changes.

**Effort:** 1–2 days for the backend endpoint + UI.

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
3. The compressed backup size on a representative moderate wallet is ≤ 50% of its pre-sprint size (measured against the same DB state)
4. Recovery has been dry-run-tested for every optimization
5. The DIAG-BACKUP patch has been reverted
6. `ONCHAIN_FULL_BACKUP_RESEARCH.md` has been updated with the new strip list
7. The post-beta3-cleanup doc has a session log entry summarizing the results

---

*Implementation gates on the parent doc's planning workflow. Do not start any item before measurement is complete.*
