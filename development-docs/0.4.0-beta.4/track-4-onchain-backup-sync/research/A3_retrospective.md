# A3 — Retrospective: why the backup system has hurt us more than any other component

**Task:** A3 (git archaeology, read-only). **Written:** 2026-08-22. **Second verification pass:** 2026-08-22 (later session) — independently re-derived the git logs, churn counts, and episode commits; re-read the incident doc in full, the review, the FIX/FOLLOWUP docs, and the live code sites; corrected three items (Mac forensic doc paths — archived; tx9 resolution — closed by `a85985f`; broad-grep count) and added one new doc-vs-code disagreement (§7.4).
**Method:** `git log --follow` on the backup files, repo-wide grep for backup/recover/restore/revert/fix commits, direct reads of the significant diffs and commit messages, and the two incident/forensic doc trails. Every claim below is labelled VERIFIED (read the commit/diff/code/doc directly) or INFERRED (my synthesis). Checked / NOT-checked lists at the end.

---

## 1. Churn statistics (VERIFIED)

Commits per file (`git log --follow --oneline | wc -l`), current line counts:

| File | Commits | Lines | Notes |
|---|---|---|---|
| `rust-wallet/src/backup.rs` | 26 | 2,284 | 20 commits in the on-chain era (2026-03-25 → 2026-06-18); of those, **14 are fixes or reverts** |
| `rust-wallet/src/recovery.rs` | 11 | 801 | |
| `rust-wallet/src/monitor/task_backup.rs` | 5 | 110 | |
| `rust-wallet/src/database/migrations.rs` | 41 | 1,335 | churn is feature-driven (V10→V23+), not bug-driven |
| `rust-wallet/src/beef.rs` (comparable complexity) | 11 | 2,055 | similar size, **less than half the commits** |
| `rust-wallet/src/utxo_fetcher.rs` | 13 | 569 | |
| `rust-wallet/src/paymail.rs` | 6 | 747 | |
| `rust-wallet/src/messagebox.rs` | 3 | 381 | |
| `rust-wallet/src/monitor/task_sync_pending.rs` | 13 | — | heavily backup-entangled (see episodes) |
| `rust-wallet/src/handlers/certificate_handlers.rs` | 53 | — | higher count but feature-sprint-driven |

Repo-wide (re-measured, pass 2, `git log --all -i --grep`): **118 commits** mention "backup" in the message; **62** of those touch `rust-wallet/`; **747** match the broader backup|recover|restore|revert|fix|regression|corrupt grep — the broad number is dominated by non-wallet noise (farbling, autoupdate rollback, beta.3 security fixes) and is quoted only to show how common "fix" traffic is repo-wide; the backup-specific 118/62 is the meaningful figure.

What no other component has (VERIFIED by doc inventory):

- Dedicated **incident forensics documents**: `Final-MVP-Sprint/backup-double-spend-incident-2026-04-11.md`, plus five Mac forensic docs (`MAC_BACKUP_FAILURE_{HANDOFF,FINDINGS,CODE_HUNT}.md`, `MAC_BACKUP_NULLFAIL_{NEXT_STEPS,RESULTS}.md`) now retired to `archived-docs/Wallet-Hardening/` (moved by `f37adfe`, 2026-08-03).
- A 420-line **adversarial review** with a findings register of 25+ items (`Wallet-Hardening/ONCHAIN_BACKUP_REVIEW.md`, 2026-07-07).
- Two remediation plan docs (`FIX_A_RECONCILE_PLAN.md` — implemented; `FIX_B_CRASH_SAFETY_SHUTDOWN_PLAN.md` — designed, **not implemented**, see §5).
- A **same-day revert** of a fix (b9afc0b → d324ab8, 2026-05-07) and a **two-day design flip-flop** (ec78600 2026-04-01 adds a pending-tx guard; e4b5d1d 2026-04-03 removes it).
- **Manual SQL surgery on the treasury (real-money) database** to un-corrupt state (incident doc, 2026-04-11).
- Six fix commits within ~24 hours of the feature landing (0dca522 2026-03-25 → six fixes dated 2026-03-26).

---

## 2. Episode catalog

Chronological. Each entry: date, symptom, fix, **root-cause mechanism**, and the **enabling design choice**. All commit hashes and messages VERIFIED by reading `git show`; where I read the actual diff I say so.

### E1 — Launch week: six fixes in 24 hours (2026-03-25/26)

`0dca522` implemented on-chain backup (Sprints 1–3). Within a day:

- **`b734956`** — recovery FK failures. `create_wallet_from_existing_mnemonic()` auto-creates user/address/basket rows that collide with the imported backup; stripped payload entities left dangling FK references (`spent_by`, `transaction_id`, `basket_id`, `proven_tx_id`) → FOREIGN KEY constraint failures. Fix: delete auto-created rows pre-import; null orphan FKs. *Mechanism:* the payload is a raw relational dump with live autoincrement IDs and FKs; recovery was bolted onto a wallet-creation path that mutates the same tables. *Design choice:* raw-dump payload + non-dedicated recovery path.
- **`9d3760b`** — including backup transactions in the payload caused **exponential payload growth** (each backup contains the record of prior backups; proven_txs merkle paths and proven_tx_reqs metadata compound). Reverted to excluding backup txs from all 5 tables. *Mechanism:* self-reference — the backup writes records into the very tables it snapshots. *Design choice:* backup bookkeeping lives inside the backed-up DB.
- **`58cf9a3`** — excluding backup txs then created the mirror bug: outputs spent *by* excluded backup txs entered the payload with `spent_by=NULL` (from the FK cleanup), so after recovery `TaskReviewStatus` restored them to `spendable=1`, **inflating the recovered balance**. *Mechanism:* strip-rule/recovery coupling — a strip decision changed the meaning of remaining rows, and a monitor task "healed" them in the wrong direction. *Design choice:* strip rules and recovery semantics maintained separately, no invariant tying them.
- **`addf1a1`** — failed txs in the payload → dead txids that 404 on WoC → monitor misbehavior post-recovery; also stale-backup adoption left orphan tokens on-chain.
- **`524e260` + `1094a8e`** — trigger system: hash-of-payload change detection, shutdown/periodic/manual triggers. Immediately needed: post-backup re-hash (the backup itself changes the DB, so the pre-backup hash is stale the moment a backup runs), change-address reuse (a new address per backup would perpetually invalidate the hash), and disabling the BRC-42 recovery scan (created phantom/duplicate UTXOs at backup change addresses). *Mechanism:* the dirty-check is a fixed-point problem — the act of backing up changes the thing being hashed. *Design choice:* change detection = SHA256 of the serialized whole-DB payload.

### E2 — Trigger-semantics flip-flop and the orphan-marker reveal (2026-04-01 → 04-03)

- **`ec78600`** (Apr 1): *skip* backup while nosend/sending/unproven txs pending ("prevents capturing ghost outputs from txs that never make it on-chain").
- **`e4b5d1d`** (Apr 3, "Backup reliability sprint"): *remove* that same guard ("ghost outputs are self-healing, missing tokens are not"). Also fixed: the backup "soon" flag being consumed without a backup running; debounce timer ignoring events inside its window; `TaskSyncPending` blindly deleting unconfirmed outputs after 30 min even when still in mempool; and **swept 18 orphaned backup markers** left by earlier interrupted cycles (9,828 sats of dust).

*Mechanism:* it was genuinely undecidable, under the design, what DB state was safe to snapshot — the DB is mutated concurrently by ~15 monitor tasks and there is no transactional snapshot boundary. Two opposite policies each fixed one failure mode and caused the other. The 18 orphan markers are physical evidence of **silent failures that never got commits** (see §6). *Design choice:* snapshot a live, concurrently-mutated DB; no quiescence mechanism.

### E3 — Payload-size crisis (2026-04-11 → 04-13)

Symptom (VERIFIED, `wallet-backup-efficiency-plan.md`): backups costing **5–6K sats each after ~2 weeks of moderate usage**, growth monotonic. Response: byte-count instrumentation (`e3abeca`, reverted `72ecce1` same day, as planned), then strip rules P0 #1–#5 (`a525ff3`, `e10ef76`, `5649c99`, `a7f684e`): time-tiered dropping of old spent outputs and addresses, proven_tx_reqs history cap, 60-day transaction window, block_headers dropped entirely. The plan doc's own words: remaining wins "carry more risk of breaking recovery edge cases," and it needed a **hard rule** that no strip may drop non-re-derivable outputs (PushDrop tokens, BRC-42 counterparty outputs) — i.e., the cost-control mechanism directly endangers the product promise.

*Mechanism:* whole-DB snapshot on every backup → payload grows with wallet age → recurring on-chain fees grow → forced lossy stripping → each strip is a potential recovery bug (E1's `58cf9a3` was already this class). This is the **completeness⟷cost tension** the July review calls "inherent to 'whole DB on-chain, repeatedly'" (VERIFIED, `ONCHAIN_BACKUP_REVIEW.md` §7). *Design choice:* full snapshot per backup; no delta structure.

### E4 — The double-spend cascade incident (2026-04-11, treasury wallet)

The worst single day. Full forensics in `backup-double-spend-incident-2026-04-11.md` (VERIFIED, read). Testing the P0 strip with back-to-back backups + a wallet restart triggered three pre-existing bugs:

- **Bug A:** the orphan-marker sweep queries WoC's `/address/{addr}/unspent/all`, whose index **lags the chain by ~30s–5min**. In that window it returned an already-spent marker as unspent → the new backup consumed it → guaranteed double-spend.
- **Bug B:** ARC returns `status:200, txStatus:SEEN_ON_NETWORK, txid:<existing>` when *rejecting* a conflicting tx; the handler read `status:200` as success and recorded the attempted txid as broadcast. The wallet believed a tx was on-chain that never was.
- **Bug C:** `TaskCheckForProofs` got `DOUBLE_SPEND_ATTEMPTED` for **both** txs in the first-seen conflict (including the eventual winner) and treated it as terminal: deleted the winning tx's outputs and **restored its already-spent inputs to spendable**. Cascade left the DB tracking phantoms while the chain was fine.

DB repaired by hand (SQL surgery). Fixes: `3a6fd2e` (Apr 12) — DB cross-reference + cooldown for the sweep; txid-collision detection on broadcast (still live today, `handlers.rs:8905-8934`, VERIFIED); WoC cross-verification before mark-failed. Then `b65c39b`/`dce3236` (Apr 15) added a **three-oracle quorum** for failed-tx rollback.

*Mechanisms:* (1) treating an eventually-consistent third-party index as an authoritative input-selection oracle inside its propagation window; (2) treating a provider response as a boolean instead of verifying the returned txid; (3) monitor tasks holding **destructive authority** (delete outputs, restore inputs) acting on a single ambiguous signal. *Design choices:* chain-truth queries with no lag model; no cross-validation; destructive monitor semantics.

### E5 — Serialization drift: certificate publish status silently lost (2026-04-17)

`36fe5db` (VERIFIED): backup/restore dropped `publish_status`, `publish_txid`, `publish_vout` — after restore, every certificate looked unpublished even with a live on-chain PushDrop. Fixed by hand-adding three fields to `BackupCertificate` + SELECT + INSERT + serde defaults.

*Mechanism:* `BackupPayload` is a **hand-maintained parallel schema**. Every migration must be manually mirrored in struct + export query + import statement; missing any one silently loses data on the next restore. The July review found the same class again at larger scale (**BS-H2**: V18 permission child tables absent entirely, `domain_permissions` missing V12/V17/V22 columns, `settings` only 7 of ~15 columns). VERIFIED against today's code: `backup.rs:32-59` still has no `domain_protocol/basket/counterparty_permissions`, no `peerpay_outbox`, no `transaction_inputs/outputs`. *Design choice:* duplicate schema with no drift gate (no test that fails when a migration adds an unrepresented table).

### E6 — Chain-truth hardening (2026-04-21)

`1fa686f` (VERIFIED message): backup-token adoption hit **PushDrop NULLFAIL** because WoC's JSON endpoint *truncates large `scriptPubKey.hex`* — fixed by switching to the raw-hex endpoint. Also removed `reconcile_for_derivation` (falsely marked ~$15 of valid outputs external-spend from incomplete WoC responses) and removed `TaskValidateUtxos` (competed with other tasks over output state → circular balance bugs); added `TaskVerifyDoubleSpend` and a startup phantom-UTXO sweep.

*Mechanisms:* indexer response truncation treated as ground truth; multiple concurrent tasks with overlapping write-authority over `outputs`. *Design choices:* same as E4 plus: several tasks own the same state with no single reconciliation authority.

### E7 — Nondeterministic hash, silent no-op persistence (2026-04-25/26)

- **`c082a34`**: the no-backup path created wallet rows then DELETEd them, advancing SQLite autoincrement → later `user_id`/`wallet_id` mismatches; fix added full **ID remapping** on import. Same raw-ID design choice as E1.
- **`30c6c89`** (VERIFIED message): ~20 payload queries had **no ORDER BY** — SQLite row order drifts as pages fragment from monitor-task UPDATEs → hash changes with zero logical change → **paid, pointless on-chain backups**. And `set_backup_hash()` was a bare UPDATE on a settings row nobody ever INSERTed — silently affected 0 rows, so the hash was never persisted at all.

*Mechanism:* hash-of-bytes change detection requires total determinism of serialization, which SQL does not give by default; plus a write path that couldn't tell "wrote nothing" from "wrote". *Design choice:* D2 again (hash of serialized payload) + no result-checking discipline on money-adjacent writes.

### E8 — Time-dependent strips burn money (2026-05-05)

`2f07982` (VERIFIED): the P0 strips used `SystemTime::now()` for their 7d/30d/60d thresholds. Items **aging past a threshold between runs** changed the payload hash → unnecessary backups, "~1200 sats each time with zero actual wallet changes." Fix: parameterize the timestamp (pre-check uses `last_backup_at`, post-backup uses now).

*Mechanism:* the strip made the payload a function of (wallet state, wall clock), so the dirty-check fired on the passage of time. Direct interaction of D1's cost-control (E3) with D2's change detection. *Design choice:* time-based strip rules evaluated at run time inside the hashed content.

### E9 — The ordering problem nobody could win: write-before-broadcast, same-day revert, shutdown backup removed (2026-05-07)

- **`b9afc0b`**: moved DB-record writes *before* broadcast, to close the crash window where a tx is on-chain but invisible to the wallet. **Reverted the same day** (`d324ab8`, no stated reason in the commit).
- **`f768eb3`**: removed the shutdown-trigger backup entirely — "the C++ side force-kills the process after 5 seconds, but backup requires ~60s (BEEF ancestry + broadcast). A crash mid-backup leaves the DB inconsistent: tx broadcast but no DB records."

Two months later, `FOLLOWUP_RECORD_BEFORE_BROADCAST_TOKENS.md` (2026-07-13, VERIFIED) rationalized the broadcast-first ordering as *deliberately correct for this path* — but only because every backup output is **self-derivable** (adopt re-discovers the token, sync re-discovers change, WS1 reconcile heals phantoms), and explicitly flags that record-before-broadcast becomes mandatory the day we hold non-self-derivable outputs (ordinals, BSV21).

*Mechanism:* broadcast+record is not atomic and cannot be made atomic; either ordering leaves a ghost on one side of a crash. The design compounded it by (a) triggering backups at shutdown, (b) inside a host kill-timeout (5s) far shorter than the operation (~8.6–60s). *Design choices:* shutdown-time backup; kill window < operation duration; no persisted intent record.

### E10 — Security fixes on the file surface (2026-06-18, 06-24)

`5b9cc7c`: backup/restore file endpoints had a **path-traversal** hole + needed an internal-only gate. `a95e01e`: export/import origin guard. Minor class relative to the above, but note: the backup system's *file* surface needed its own hardening round. (The path-validation logic is now the best-tested part of the module — see §5.)

### E11 — The July field bug: permanent backup-retry loop (diagnosed 2026-07-07)

`ONCHAIN_BACKUP_REVIEW.md` §1–2 (VERIFIED, read in full): a **live field wallet** stuck in an infinite loop — every backup attempt fails `"Missing inputs"`, rolls back, retries every tick, forever. Diagnosis: a previous backup **broadcast successfully but the process was killed before the DB write** (the E9 window, realized in the field). The DB's believed-spendable funding output was actually consumed on-chain by the ghost backup; the adopt logic found the newer token but **funding selection still used the stale DB**. And the "skip if unchanged" hash guard never quieted the loop because `updated_at` churn from background tasks changed the hash every run (**BS-M1**) — no backoff existed.

Root-cause correction chain worth preserving: the review first claimed no graceful shutdown existed; `FIX_B_CRASH_SAFETY_SHUTDOWN_PLAN.md` §0 corrected it — the graceful path **exists** (`POST /shutdown` → 5s bounded wait → `TerminateProcess` fallback) but **the 5s fallback fires mid-backup (~8.6s)**. Fix A (chain reconcile: `check_outpoint_spent`, `recover_change_index`, `reconcile_spent_inputs`, promote-mined-successor) was implemented 2026-07-10/13 (`983655a`, `7ad0916`, `8903a2b`, `4e7c94f`, `e4f5bb7` — VERIFIED). **Fix B (persist backup intent before broadcast) was designed but never implemented** — no `BackupIntent` code exists; broadcast-first with placeholder rollback is still live (`handlers.rs:13957-13960`, crash-window comment still at `:13471`; VERIFIED by grep).

*Mechanisms:* E9's window realized; D2's hash guard defeated by volatile metadata; unbounded retry with full rebuild+broadcast per tick.

### E12 — The Mac failures: three months of silent divergence + wrong-key signing (2026-07-13/14)

Forensic docs (`archived-docs/Wallet-Hardening/MAC_BACKUP_*`, `Wallet-Hardening/MAC_KEYCHAIN_CROSSCONTAMINATION_FIX.md`; VERIFIED commit messages `947d251`→`deff765`, `a85985f`):

- The Mac wallet's backups had **diverged since the Apr-11 incident — three months — without anyone noticing** (`073c7b7` retitled the investigation from "July regression" to "divergence since Apr-11").
- **Two distinct failure modes** untangled: (1) April's tx9 — all signatures cryptographically valid yet nodes reject ("signature paradox"); resolved 2026-07-14 by `a85985f`: **broadcast infrastructure failure** from April 15, not a BEEF or signing bug — signatures re-verified against production keys, "Not a bug — dead DB record, no impact," likely resolved by `63cda01` (broadcast resilience, Apr 17); (2) July 6+ — the wallet **signed the funding UTXO with the wrong key** (HASH160 mismatch → OP_EQUALVERIFY failure).
- Root cause of (2), `deff765`: dev and prod wallets **shared the macOS Keychain service name** `"HodosBrowser"` — creating a dev wallet overwrote the production mnemonic; every production session that auto-unlocked from Keychain after June 25 signed with dev keys. **Funds migrated to dev-keyed addresses** through internally-consistent change outputs; **12.5M sats swept back**. Windows unaffected (DPAPI blobs are per-DB).

*Mechanisms:* credential store keyed by app name, not wallet identity, shared across environments; and no monitoring — the backup system can fail totally, for months, and nothing tells anyone (see §6). *Design choices:* single shared Keychain entry; no backup-health signal.

### E13 — Full sync poisons the backup address (found 2026-07-07, review §5)

**BS-SYNC-1** (VERIFIED in review; code refs `output_repo.rs:463-480`, `address_repo.rs:119`): `?full=true` sync scans **every** address row including the backup address (index -3), inserts its marker as `spendable=1` with NULL derivation → key derivation falls back to the **master key**, but -3 is a BRC-42 key → any tx that selects it fails script verification and **the whole bundling tx fails to broadcast**. Almost certainly the owner's "every time we run a full sync, something breaks." *Mechanism/design choice:* a special-purpose address kept in the common table, with signing-key identity **inferred from index conventions** rather than stored explicitly.

---

## 3. Root-cause mechanism classes (synthesis)

Every episode above reduces to one of seven mechanisms (INFERRED grouping; each mechanism cites VERIFIED episodes):

| # | Mechanism | Episodes |
|---|---|---|
| M1 | **Completeness⟷cost tension of a full-DB on-chain snapshot**: growth forces strips; strips are recovery bugs | E3, E1(58cf9a3), E5, BS-H2/M4/M5 |
| M2 | **Hash-of-serialized-payload change detection** demands total determinism the system doesn't have (row order, wall clock, `updated_at` churn, self-mutation) | E1(524e260), E7, E8, E11 |
| M3 | **Self-reference**: the backup mutates and records itself inside the state it backs up | E1(9d3760b, 58cf9a3), E7(post-backup hash) |
| M4 | **Raw relational dump**: autoincrement IDs, FKs, and a hand-mirrored parallel schema with no drift gate | E1(b734956), E7(c082a34), E5, BS-L2 |
| M5 | **External oracles trusted beyond their contract**: WoC index lag, WoC JSON truncation, ARC 200-on-reject, ambiguous DOUBLE_SPEND_ATTEMPTED | E4(A/B/C), E6, BS-C2/H1/H5 |
| M6 | **Non-atomic broadcast/record inside a hard-kill window**, retried without backoff | E9, E11, BS-M2 |
| M7 | **Distributed, destructive authority over output state** (monitor tasks + sync + backup all writing `spendable`/`spent_by` with no lock, no single reconciler) | E4(C), E6, E13, BS-C1, E1(58cf9a3) |

## 4. Why has this component hurt us more than any other?

(INFERRED synthesis; every sentence traceable to episodes above.)

1. **It has maximal coupling surface by construction.** The payload hashes the entire DB, so *every other subsystem's writes are backup inputs*. A permission migration (E5/BS-H2), a monitor task's `updated_at` touch (E11), SQLite page fragmentation (E7), even the passage of time (E8) all become backup behavior. No other component is a function of everything else.

2. **Its failure modes cost real money, immediately and irreversibly.** False-positive triggers burned ~1200 sats each (E8), ~8/day at worst (BS-M1); double-spends and cascades corrupted the treasury DB (E4); the Keychain bug moved 12.5M sats onto wrong keys (E12). Bugs elsewhere waste time; bugs here spend the user's sats or strand their funds.

3. **It sits on the largest set of implicitly-trusted external contracts** — WoC index freshness and JSON fidelity, ARC response semantics, SQLite ordering/autoincrement behavior, the OS credential store, the C++ host's kill timing — and an episode exists for each contract being weaker than assumed (E4, E6, E7, E12, E9/E11).

4. **The half that matters is the half that never runs.** Recovery is the product promise, but it executes only after disaster, on whatever a months-old strip policy preserved. The strip rules (cost) and recovery semantics (correctness) were maintained separately with no invariant test binding them — the only automated tests in `backup.rs` today are crypto round-trip and path validation (VERIFIED, `backup.rs:2044-2286`); nothing round-trips the payload through strip→import.

5. **It is self-referential.** The backup transaction spends, creates, and records state inside the very DB being snapshotted, producing fixed-point bugs (exponential growth E1, hash baselines E1/E7, marker adoption E4/E11) that no amount of local correctness fixes.

6. **It fails silently.** Three months of Mac divergence (E12), 18 orphaned markers (E2), an unknown number of diverged field wallets (review §8) — there is no health signal, so damage compounds until a human notices a symptom.

The review's own verdict (VERIFIED, §0): "the design and cryptography are sound. The problems are in operational implementation: concurrency, crash-safety, recovery robustness, and payload completeness" — plus the one pressure fixes can't remove, completeness⟷cost, "the strongest argument for a deliberately-designed v2."

## 5. Constraints: the new design must not repeat…

Each constraint is traceable to at least one concrete episode (cited).

1. **…a monolithic full-DB payload whose growth forces lossy strips.** Separate non-re-derivable data (tokens, BRC-42 counterparty outputs, certificates) from re-fetchable data structurally, not by per-field strip rules. (E3; E1-58cf9a3; BS-H2.)
2. **…change detection over non-canonical bytes.** Any dirty-check must hash canonical content: explicit ordering, no volatile metadata (`updated_at`), no wall-clock-dependent membership, computed identically pre- and post-write. (E7-30c6c89, E8-2f07982, E11/BS-M1.)
3. **…backup bookkeeping stored inside the backed-up state.** The backup's own txs/outputs/hash/tip-pointer must be excluded by construction (separate table or derived from chain), not by SQL filter rules that need mirror rules at recovery. (E1-9d3760b, E1-58cf9a3.)
4. **…raw autoincrement IDs and FKs in the wire format.** Payload entities need stable natural keys (txid:vout, derivation path, identity key); import must not depend on target-DB ID coincidence. (E1-b734956, E7-c082a34.)
5. **…a hand-mirrored schema with no drift gate.** A CI check must fail when a migration adds a table/column unrepresented in the payload (or explicitly tier-3-excluded, per the work-plan tiers); the wire format carries a version that decode actually checks, with defined newer→older and older→newer behavior. (E5-36fe5db, BS-H2, BS-L2. Item 0's BRC-38 mapping table is the right vehicle.)
6. **…"newest backup" selection by indexer timestamps.** Every token carries an explicit sequence number and prev-txid link; tip selection follows the chain, never `max_by_key` over WoC confirmation times. (E4, BS-H1/H5. The delta-chain design already does this — keep it load-bearing.)
7. **…trusting indexer/broadcaster responses beyond their contract.** Model WoC address-index lag as a first-class state (never select inputs from an index younger than the propagation window); verify returned txid equals submitted txid; require multi-oracle agreement before any destructive state change. (E4 Bugs A/B/C, E6 truncation, dce3236 quorum.)
8. **…non-atomic broadcast/record inside a kill window.** Persist intent (or the full record) durably before broadcast, or prove every output self-derivable *and* run a startup chain-reconcile — and never schedule backup work where the host's kill timeout is shorter than the operation. Fix B's design exists and was never implemented; the new design must resolve this explicitly, not inherit the ambiguity. (E9, E11, FIX_B plan, FOLLOWUP doc.)
9. **…unbounded retry of a failing backup.** Failure must converge: backoff, retry ceiling, and a quiescent error state that surfaces to the user instead of re-running build+broadcast every tick. (E11/BS-M1.)
10. **…conflating "indexer unavailable" with "no backup exists."** Recovery must fail closed on transient errors and never steer the user toward creating a fresh wallet over a live backup. (BS-C2.)
11. **…non-atomic import.** Pre-cleanup and import run in one DB transaction; a mid-import failure leaves the wallet in its prior state, never a half-state that traps retry. (BS-H4, E1-b734956.)
12. **…backup building outside the wallet's concurrency discipline.** The builder takes the same locks as spends; no `let _ =` on state-mutating results. (BS-C1, E4, E7 settings-row silent no-op.)
13. **…shipping recovery without an automated strip→backup→wipe→restore→verify round-trip test**, covering token outputs, BRC-42 counterparty outputs, and certificate status. (E1 week, E5; current test coverage VERIFIED as crypto+path only.)
14. **…shared credential storage across environments or wallets.** Key material storage must be namespaced per environment and per wallet identity on every platform. (E12-deff765.)
15. **…deriving signing-key identity from address-table index conventions.** Special addresses carry explicit derivation metadata; no code path may fall back to the master key for an address it doesn't understand. (E13/BS-SYNC-1, E1-1094a8e phantom scan.)
16. **…zero observability of backup health.** The chain tip, last-success time, and consecutive-failure count must be user-visible (and ideally device-visible for multi-device); three silent months (E12) must be impossible. (E12, E2's 18 orphan markers, review §8's open "how many field wallets are diverged?")

## 6. What the history does NOT show

- **Silent failures never got commits.** The 18 orphaned markers swept in `e4b5d1d` are physical evidence of ≥18 interrupted/failed backup cycles with no corresponding incident record. The Mac divergence ran ~3 months before a commit exists. Assume the episode catalog above is a **lower bound**.
- **Field prevalence is unknown.** The review's open question — how many field wallets are diverged — was never answered in any commit or doc I found.
- **No evidence of routine successful recoveries.** History records recovery *fixes*, not recovery *use*; there is no record of a real user (non-developer) recovery succeeding in the field. The most safety-critical path has effectively zero production exercise.
- **Bug C's trigger was never confirmed.** Why ARC returned DOUBLE_SPEND_ATTEMPTED for the winning tx has three hypotheses in the incident doc and no recorded resolution.
- **Fix B's status is a live disagreement.** `FIX_B_CRASH_SAFETY_SHUTDOWN_PLAN.md` says the crash-window fix is designed and needed; `FOLLOWUP_RECORD_BEFORE_BROADCAST_TOKENS.md` (same date) argues the current broadcast-first ordering is "correct as-is" given WS1 reconcile. No implementing commit exists (VERIFIED by grep for BackupIntent/intent). The new design cannot treat this as settled.
- **Total money burned is unquantified.** ~1200 sats/false backup and ~8/day figures exist for one wallet at one time; no aggregate accounting was ever done.
- **The April "signature paradox" (tx9) was closed on paper, not by a fix commit.** `a85985f` (2026-07-14, VERIFIED) re-verified all three signatures against production keys and reclassified tx9 as an April-15 broadcast-infrastructure failure "likely resolved by `63cda01`" — *likely*, not proven; no commit demonstrably fixed the specific failure, and the DB record was simply left dead.

## 7. Disagreements with docs (quoted, per method rule 3)

1. `ONCHAIN_BACKUP_SYSTEM.md` (current-system doc): "Recoverable from the 12-word mnemonic alone — zero coins required for recovery." — True only when WoC is up and honest: BS-C2 (VERIFIED in review, `handlers.rs:14202`) shows a transient WoC failure is reported as "No backup found" and the UI nudges toward creating a new wallet. The claim should read "mnemonic + a working indexer."
2. `FOLLOWUP_RECORD_BEFORE_BROADCAST_TOKENS.md`: "This path is correct as-is and does NOT need changing." — Defensible only because of the self-derivability argument *and* WS1 reconcile; but E11 shows the same ordering produced a months-long field failure before WS1 existed, and BS-M2 (redundant spend after crash-before-`set_backup_hash`) remains. "Correct" here means "eventually self-healing at some sat cost," which the new design should state honestly rather than inherit as "correct."
3. `ONCHAIN_BACKUP_REVIEW.md` §2 ("graceful path doesn't exist, OD-2 never runs") is **superseded by its own correction** and FIX_B §0 — the graceful path exists; the 5s fallback is simply shorter than the ~8.6s backup. Any future doc citing §2 uncorrected is citing a known-wrong claim.
4. `backup.rs:7` module header: "Encrypted wallet backup (AES-256-GCM, all entities, **excludes mnemonic**)" — but `collect_payload()` (`backup.rs:841`) writes `mnemonic: mnemonic.to_string()` into the payload, and the file-export caller (`handlers.rs:17266`) passes the **real mnemonic**. Only the on-chain path blanks it (`backup.rs:1013`, `payload.mnemonic = String::new()`). So `ONCHAIN_BACKUP_SYSTEM.md`'s "Mnemonic is NEVER in the backup" is true **for the on-chain token only**; the password-encrypted file export contains the mnemonic, and the module comment is wrong as written. (VERIFIED against code, pass 2. Relevant to A2's scope, flagged here because the shared `BackupPayload` struct is exactly the hand-mirrored-schema mechanism M4.)
5. `ONCHAIN_BACKUP_SYSTEM.md` "Storage Format": "**Service fee**: 1000 sats to Hodos treasury (standard for all wallet transactions)" — the code disagrees: `handlers.rs:13602` "No Hodos service fee for wallet backups — this is infrastructure protecting the user", matching the original implementation commit (`0dca522`: "No Hodos service fee for backups"). The current-system doc is wrong on this point. (VERIFIED, pass 2.)

---

## Checked (files/sections actually read)

**Git history (full `--follow` logs read):** `rust-wallet/src/backup.rs` (26 commits), `recovery.rs` (11), `monitor/task_backup.rs` (5), `database/migrations.rs` (41, list), `handlers.rs` (142, list), plus repo-wide `--grep` sweeps (backup: 118; broad: 297).

**Commit messages + stats read (`git show --stat`):** cf3be47, 0dca522, b734956, 9d3760b, 524e260, 1094a8e, 58cf9a3, addf1a1, e3abeca, 72ecce1, a525ff3, e10ef76 (listed), 5649c99 (listed), a7f684e, 36fe5db, 1fa686f, c082a34, 30c6c89, 2f07982, b9afc0b, d324ab8, f768eb3, 5b9cc7c, ec78600, e4b5d1d, 3a6fd2e, e4f5bb7, 947d251, 073c7b7, 4fb2f50, de4b201, deff765, a95e01e (listed). Pass 2 additions: a85985f (tx9 reclassification), b65c39b, dce3236 (three-oracle quorum, message only), f37adfe (doc archival, name-status), 05d7cdf; diff hunks read for 30c6c89 (ORDER BY lines) and stat-level symmetry check of the b9afc0b→d324ab8 revert pair (94/81 lines inverted).

**Code read:** `backup.rs:32-77` (BackupPayload struct, current), `backup.rs:2044-2286` (test inventory), `monitor/task_backup.rs` (full, 110 lines), `handlers.rs:8905-8940` (txid-collision guard), `handlers.rs:13471,13957-13960` (broadcast-first still live, via grep), `migrations.rs` version markers (grep). Pass 2 re-reads: `backup.rs:25-80` (struct), full `fn` inventory + mnemonic call sites (`backup.rs:372,841,1013`; `handlers.rs:14866,17266`), `handlers.rs:8895-8940` (collision guard text), `handlers.rs:13472,13953-13997` (broadcast-first Step 11/12 comments), `handlers.rs:13602` (no-service-fee comment), test-fn list (`backup.rs:2028-2278` — path validation + on-chain crypto only, confirming §4.4).

**Docs read:** `Wallet-Hardening/ONCHAIN_BACKUP_REVIEW.md` (pass 1 full ~420 lines; pass 2 re-read lines 1-268 incl. §5/BS-SYNC-1 detail), `Final-MVP-Sprint/backup-double-spend-incident-2026-04-11.md` (**full, 268 lines** — exec summary, timeline, DB-state table, Bugs A/B/C detail + fix options + recovery plan + resume checklist), `Final-MVP-Sprint/wallet-backup-efficiency-plan.md` (lines 1-30: problem statement, hard rule), `Wallet-Hardening/FOLLOWUP_RECORD_BEFORE_BROADCAST_TOKENS.md` (full, 60 lines), `Wallet-Hardening/FIX_B_CRASH_SAFETY_SHUTDOWN_PLAN.md` (§0 correction via review header; lines 1-30 pass 1), `Wallet-Hardening/README.md` (lines 1-60), `ONCHAIN_BACKUP_SYSTEM.md` (**full, 182 lines**), `0.4.0-beta.4/sprint-4-onchain-backup-sync/README.md` (lines 1-60), `archived-docs/Wallet-Hardening/MAC_BACKUP_NULLFAIL_RESULTS.md` (conclusion table rows via grep, incl. the tx9 verdict line).

## NOT checked

- Full diffs of most commits (read `--stat` + message; read actual diff hunks only via the quoted code above). In particular I did **not** read the full diffs of 524e260, 1094a8e, c082a34, 1fa686f, 3a6fd2e (the latter two are whitespace-noisy: line-ending churn inflates their stats to ~30k lines).
- `MAC_BACKUP_FAILURE_FINDINGS.md` / `MAC_BACKUP_NULLFAIL_RESULTS.md` full contents (read commit messages + the RESULTS conclusion-table rows only).
- `FIX_A_RECONCILE_PLAN.md` (archived), `WALLET_GRACEFUL_EXIT_SPEC.md`, `wallet-efficiency-and-bsv-alignment.md` (parent doc).
- The C++ side (`WalletService.cpp`, `cef_browser_shell.cpp`) — shutdown-path claims are taken from FIX_B §0's stated verification, not my own read.
- Frontend recovery UI code; macOS-specific Rust code paths.
- Contents of the `rust-wallet/tests/tier*.rs` integration suites (not opened). However, VERIFIED by grep: **no file in `rust-wallet/tests/` mentions "backup", `compress_for_onchain`, `import_to_db`, or `BackupPayload` at all** — so the "no strip→restore round-trip test exists anywhere" claim stands at repo level, not just within `backup.rs`.
- Any branch-only history not reachable from `--all` refs present locally.
