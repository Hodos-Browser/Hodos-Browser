# Wallet Efficiency & BSV Ecosystem Alignment — Parent Plan

**Created:** 2026-04-11
**Status:** Planning — no implementation yet
**Children:**
- [`wallet-backup-efficiency-plan.md`](./wallet-backup-efficiency-plan.md) — backup token size reduction
- [`bsv-ecosystem-alignment-plan.md`](./bsv-ecosystem-alignment-plan.md) — adopting patterns from `BSV_RUST_ECOSYSTEM_COMPARISON.md`

---

## Why this exists

Two separate but overlapping efforts surfaced in the same planning session:

1. **The on-chain backup token is growing fast.** Currently 5–6K sats per backup at ~2 weeks of moderate usage. Trajectory is exponential because the backup grows monotonically with usage. Need to find ways to keep it small without breaking recovery.
2. **The BSV Rust ecosystem comparison doc** identified 12 candidate improvements from sibling repos (`bsv-wallet-toolbox-rs`, `bsv-rs`, `bsv-rust-sdk`). Some are quality wins independent of backup size; we want to evaluate them in one pass rather than ad-hoc.

These are **two separate sprints** with different acceptance criteria, but they share files (`backup.rs`, `handlers.rs`, `monitor/`) so we want to plan them together to avoid stepping on each other.

## Backup architecture invariants — DO NOT BREAK

These are the load-bearing properties of the current on-chain backup system. Any optimization must preserve them:

1. **PushDrop is nonstandard script → not address-indexable.** The encrypted payload sits in vout 0 of the backup transaction. Block explorers (WhatsOnChain, etc.) only index standard script types. The PushDrop is invisible to address-based UTXO queries.

2. **A separate P2PKH marker output at a BRC-42 self-counterparty address acts as the discovery anchor.** Recovery flow:
   - Derive backup address from mnemonic via `BRC-42(master, self, "1-wallet-backup-1")`
   - Query that address for unspent UTXOs (the marker)
   - Marker txid → fetch transaction → read PushDrop at vout 0 of the same tx
   - Decrypt → decompress → restore

3. **Hash comparison gates each backup.** `do_onchain_backup` (`handlers.rs:11183-11205`) computes SHA256 of the compressed-but-not-yet-encrypted payload, compares to `settings.backup_hash`, and returns `Err("skipped: no changes")` if unchanged. Prevents wasteful re-broadcasts when nothing changed.

4. **Both PushDrop and marker are consumed as inputs to the next backup**, recovering their sats. Only the latest backup is unspent on-chain. Cleanup logic at `handlers.rs:11371+` sweeps orphaned markers from old backup cycles.

5. **`encrypt_compressed` MUST be called AFTER compression.** Encrypted data does not compress. Order is `JSON → gzip → AES-256-GCM → on-chain`.

6. **Recovery is the system's most fragile path.** It "took a long time to get working." Any optimization that touches what's in the backup must include a documented argument for why recovery still works after the change. Default: keep recent operationally-relevant data, drop old history.

7. **Recovery code stays the same unless explicitly approved.** Verified 2026-04-11 by reading `handlers.rs:12443`: recovery does NOT do sequential address derivation — it deletes auto-created addresses then imports from the backup payload directly. Optimizations that would require changes to `wallet_recover_onchain`, `wallet_restore`, or `import_to_db` are HIGH RISK and require explicit pre-approval. Prefer optimizations that work with the existing recovery code unchanged.

8. **Non-standard token preservation (HARD RULE).** Any backup field that touches `outputs`, `transactions`, `addresses`, or proof tracking MUST verify it doesn't break recovery for: (a) PushDrop tokens current and future, (b) BRC-42 with external counterparty, (c) future BRC-X protocols with non-standard scripts. **Default position:** assume any output where `derivation_prefix != "2-receive address"` AND `derivation_prefix != "bip32"`, OR where `sender_identity_key IS NOT NULL`, is non-recoverable from master key alone and MUST be preserved in the backup, regardless of age. **Token loss is permanent and unrecoverable.**

## Cross-cutting context

### What's already stripped from the on-chain backup
Verified by reading `backup.rs` (`compress_for_onchain` line 979 + `collect_payload` strips):

| Field | Where stripped | Reason |
|---|---|---|
| `mnemonic` | `compress_for_onchain:988` | Re-entered on recovery |
| `raw_tx` for confirmed transactions | `collect_payload:447-448` | On-chain, free to re-fetch |
| `locking_script` for spent outputs | `collect_payload:498-499` | Never needed once spent |
| `raw_tx` in `proven_txs` | `collect_payload:544-551` | On-chain, free to re-fetch |
| `merkle_path` in `proven_txs` | `compress_for_onchain:996-998` | Free to re-derive (BIG win — proofs are large) |
| `raw_tx` + `input_beef` in `proven_tx_reqs` | `compress_for_onchain:992-995` | Re-fetchable |
| `parent_transactions` (entire table) | `compress_for_onchain:991` | Re-buildable from chain |
| Orphan FK references | `compress_for_onchain:1000-1031` | Cleanup pass |

**Key implication:** The "easy" wins are already taken. Remaining backup-size reductions are smaller, more nuanced, and carry more risk of breaking recovery edge cases. Measure first, then attack the largest contributor.

### Existing instrumentation
- Total JSON byte count: `backup.rs:1052-1054` (`"On-chain backup: {N} bytes JSON → {M} bytes compressed"`)
- Total encrypted payload size: `handlers.rs:11212` (`"Encrypted payload: {N} bytes"`)
- Compressed-size warning at 200 KB: `backup.rs:1056-1058`
- Per-entity COUNT comparison (for backup verification, not size): `handlers.rs:12286-12298`
- **Per-entity BYTE COUNT instrumentation: NEW, added 2026-04-11 as `[DIAG-BACKUP]` patch in `backup.rs:1037+`. Must be reverted before any release.**

### What is NOT yet stripped that probably could be
See child doc for full analysis. Highest-leverage candidates: `block_headers` (free to re-fetch), spent outputs older than 30 days (operationally dead), confirmed transactions older than N days (history stub instead of full record).

### Affected files (deconfliction map)

Both child plans touch these files. **When implementing in parallel, check this map before starting:**

| File | Backup efficiency plan touches | BSV ecosystem plan touches | Conflict risk |
|---|---|---|---|
| `rust-wallet/src/backup.rs` | YES — `compress_for_onchain`, `compress_payload`, possible new strip rules | NO | None |
| `rust-wallet/src/handlers.rs` | Maybe — UTXO consolidation endpoint if added | YES — broadcast classification (1a), WoC BUMP endpoint (1d) | **Medium** — both edit large file. Coordinate via small focused commits. |
| `rust-wallet/src/monitor/` | Maybe — new task for spent-output purge in backup | YES — new `task_compact_beef.rs` (1c) | **Low** — different files. |
| `rust-wallet/src/cache_helpers.rs` | NO | YES — adaptive timeouts (1b), WoC BUMP (1d) | None |
| `rust-wallet/src/utxo_fetcher.rs` | NO | YES — adaptive timeouts (1b) | None |
| `rust-wallet/src/price_cache.rs` | NO | YES — adaptive timeouts (1b) | None |
| `rust-wallet/src/crypto/signing.rs` | NO | YES — constant-time comparisons (3a) | None |
| `rust-wallet/src/fee_rate_cache.rs` | NO | Maybe — could use adaptive timeouts pattern | None |

**Conflict mitigation rule:** If both efforts need to touch `handlers.rs` in the same week, the backup efficiency change goes first (smaller scope), the BSV ecosystem changes follow on a fresh branch.

---

## Master prioritized checklist

Cross-cutting priority list for both child plans. Each item points to its child doc for details. **All items are gated on a successful planning session — no implementation until each child doc has explicit approval.**

### P0 — Measurement complete 2026-04-11. Priorities below are data-driven.

**Measurement summary:** Backup is 70 KB compressed (~7K sats fee). Top 5 fields = 92.2% of payload: outputs (48.8%), transactions (12.1%), proven_tx_reqs (11.6%), addresses (10.5%), proven_txs (9.2%). 92% of `outputs` records are spent (268 of 293). See `wallet-backup-efficiency-plan.md` § "Measurement results" for full table.

- [x] **Measurement run** — DONE 2026-04-11. DIAG-BACKUP captured per-field byte counts. Patch will be reverted as part of this commit cycle.
- [ ] **Spent-output time-tiered strip** (was always #1; **measurement confirmed it's even higher leverage than projected**, 48.8% of backup with 92% spent records) — Drop spent outputs older than 30 days entirely; keep recent ones with all fields for cert-reclaim safety. See child: `wallet-backup-efficiency-plan.md` § "Candidate optimizations."
- [ ] **Address time-tiered strip (NEW from measurement)** — Drop "operationally dead" addresses (used + zero spendable outputs + not pending check + > 30 days old + index ≥ 0). **NOT a "drop addresses entirely" change** — recovery code stays the same; we just thin the records the backup includes. Modest win (~5 KB compressed) but safe and easy. See child: `wallet-backup-efficiency-plan.md` § "1b Address time-tiered strip."
- [ ] **Cap `proven_tx_reqs.history` field** — Confirmed unbounded JSON audit log appended on every status transition. Cap to last 5 entries. ~3-4 KB compressed savings. See child: `wallet-backup-efficiency-plan.md`.
- [ ] **Sliding window for `transactions`** — Drop entries older than 60 days from the backup. **NOTE: do NOT also drop proven_txs** (see retraction below). Recommend null-ing the `proven_tx_id` FK on transactions that get sliding-window-dropped to avoid orphan refs. See child: `wallet-backup-efficiency-plan.md` § "Growth strategy."
- [ ] **`block_headers` strip** — Drop entirely from on-chain backup (re-fetchable from network). Low risk, tiny win (~150 bytes compressed). See child: `wallet-backup-efficiency-plan.md`.
- [ ] **`3a` Constant-time comparisons** — Add `subtle` crate, fix `verify_hmac_sha256` and any other comparison sites. ~30 min, no risk. See child: `bsv-ecosystem-alignment-plan.md`.

**RETRACTED from earlier proposal (post-measurement reasoning):**
- ❌ ~~Drop addresses entirely~~ — Verified `handlers.rs:12443` deletes auto-created addresses and imports from backup. No sequential derivation step in recovery. Dropping addresses entirely would break the recovered wallet. The time-tiered version above is the safe alternative.
- ❌ ~~Drop proven_txs entirely~~ — Would orphan the `proven_tx_id` FK on transactions. Could be solved by also nulling the FKs but that adds complexity for only 9.2% savings. **Keep proven_txs as-is** for now; revisit only if other P0/P1 wins are insufficient.

### P1 — Do next (after P0 lands and is verified)

- [ ] **Lazy consolidation in real sends (Strategy 5.1)** — Modify UTXO selection in `create_action` to greedily eat small UTXOs during normal payments. **Reduces spendable UTXO count over time without making any new transactions.** Best privacy profile of all consolidation strategies — completely invisible to outside observers. ~1 day. See child: `wallet-backup-efficiency-plan.md` § "UTXO consolidation."
- [ ] **Auto dust-threshold consolidation (Strategy 5.3)** — New monitor task that consolidates UTXOs below 1000 sats when 20+ accumulate. Opt-out via setting. Daily. ~1 day. See child: `wallet-backup-efficiency-plan.md`.
- [ ] **`1a` Broadcast failure classification** — Systematize permanent vs transient broadcast errors across `task_send_waiting`, `task_check_for_proofs`, broadcast handler. 1–2 days. Quality win independent of backup. See child: `bsv-ecosystem-alignment-plan.md`.
- [ ] **`1c` BEEF compaction task** — New monitor task that trims proven ancestors from `parent_transactions` table. **NOT a backup-shrinker** (parent_transactions is already stripped from the backup) — this is a runtime/in-memory state win. 1 day. See child: `bsv-ecosystem-alignment-plan.md`.

### P2 — Evaluate after P0+P1 ship

- [ ] **User-initiated same-counterparty consolidation (Strategy 5.2)** — One-click "Combine 23 payments from arch@handcash" UI. No privacy warning needed (no new linkage created). ~1 day. See child: `wallet-backup-efficiency-plan.md`.
- [ ] **Old confirmed-transaction collapse** — Lower priority now that the sliding window does most of this work. Only revisit if measurement shows old txs are still a significant size contributor after the sliding window lands. See child: `wallet-backup-efficiency-plan.md`.

### P2 — Evaluate after P0+P1 ship (continued)

- [ ] **`1b` Adaptive service timeouts** — EMA-based timeout tracking per API provider. 2–3 days. Real but invisible quality win. See child: `bsv-ecosystem-alignment-plan.md`.
- [ ] **`1d` WhatsOnChain BUMP endpoint** — Skip TSC↔BUMP conversion when fetching from WoC. Cleanup, ~1–2 days. See child: `bsv-ecosystem-alignment-plan.md`.
- [ ] **Binary serialization (CBOR/MessagePack)** — Replace JSON with binary format before gzip. ~10–30% additional reduction expected. **Bumps backup format version** — requires migration logic. See child: `wallet-backup-efficiency-plan.md`. **Skip if P0+P1 already meet the flat-curve acceptance criterion** — the migration cost may not be worth a marginal reduction.

### P3 — Power-user / opt-in features

- [ ] **Same-basket consolidation (Strategy 5.4)** — User-initiated, with privacy warning modal. See child: `wallet-backup-efficiency-plan.md`.
- [ ] **Aggressive cross-counterparty consolidation (Strategy 5.5)** — User-initiated, full privacy warning modal. The "I know what I'm doing" power-user button. See child: `wallet-backup-efficiency-plan.md`.

### Skip / Defer

- ❌ **`2a` bsv-rs migration** — Hard skip. Unpublished crate, irreversible refactor in security-critical code. See child for full reasoning.
- ❌ **`2b` Storage trait hierarchy** — Skip. Refactor for capabilities (multi-backend storage) we don't need.
- ❌ **`3c` FIFO spend lock** — Skip. Marginal vs current `utxo_selection_lock`.
- ⏸ **`2c` BRC-29 RemittanceManager** — Defer. Only revisit if extending PeerPay.
- ⏸ **`3b` Fuzz testing** — Defer to post-launch hardening.
- ⏸ **Tiered hot/cold backup** (off-chain cold storage) — Defer. Real value but biggest complexity. Revisit if backup size still problematic after P0–P2 ship.
- ⏸ **Delta backups** (snapshot + diffs) — Defer. Recovery walks a chain. Revisit only if monolithic full backups remain too large.

---

## Planning workflow

### Phase 1 — Measurement (DO BEFORE EVERYTHING ELSE)

1. The `[DIAG-BACKUP]` patch is already committed (in working tree as of 2026-04-11). Build is verified clean.
2. Start the wallet, trigger a backup either by waiting for the monitor (~30s tick) or hitting `POST /wallet/backup/onchain` manually.
3. Capture the log lines tagged `📊 [DIAG-BACKUP]` from the wallet log.
4. Paste the per-field byte counts into `wallet-backup-efficiency-plan.md` § "Measurement results."
5. Revert the diagnostic block from `backup.rs:1037+` once measurements are recorded. The diagnostic adds N+1 redundant serde passes per backup — too expensive for production.

### Phase 2 — Decision documentation

Once measurements are in hand, both child docs get filled in with:
- Specific size targets for each optimization (now grounded in real numbers)
- Concrete acceptance criteria
- Implementation order rationale

### Phase 3 — Implementation

Each item from the master checklist becomes its own commit. **Every backup-affecting change must be paired with a verification step:** after making the change, run a backup, decrypt it locally, and confirm `wallet_backup_onchain_verify` (`handlers.rs:12211`) reports zero count diffs against the live DB. This catches any unintended drops.

### Phase 4 — Cleanup

- Revert all `[DIAG-BACKUP]` instrumentation
- Update `ONCHAIN_FULL_BACKUP_RESEARCH.md` with the new strip list
- Update `CLAUDE.md` Key Files table if any new modules were added
- Commit a "post-efficiency-sprint" summary entry into the post-beta3-cleanup doc

---

## Open questions — RESOLVED 2026-04-11

| # | Question | Decision |
|---|---|---|
| 1 | History loss tolerance after recovery | **Acceptable.** Recovery shows wallet working with current state + recent history (30–60 days). Older history can be re-fetched from chain in background if user wants it. Sliding window approved for `transactions` and `proven_txs`. |
| 2 | Multi-device sync importance | **Not a current concern. Future problem.** Don't constrain backup design around it. If multi-device matters later, we'll add a separate sync mechanism then. |
| 3 | Format version bump policy (CBOR) | **Defer.** Skip CBOR entirely if P0+P1 wins meet the flat-curve acceptance criterion. Migration cost not worth a marginal reduction. |
| 4 | Acceptance criterion | **Static target: 5K sats (goal), 10K sats (acceptable). GROWTH target is the load-bearing one: backup must grow by less than 2× from month 1 to month 6. A flat curve is the win. The slope matters more than the absolute number.** |

---

*This is a parent index document. All implementation details, code references, and decision rationale live in the two child documents.*
