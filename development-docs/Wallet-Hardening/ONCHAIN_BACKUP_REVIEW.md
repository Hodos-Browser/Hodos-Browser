# On-Chain Backup Token — Deep Review, Field Bug & Remediation

> Living technical review of the on-chain wallet-backup ("backup token") subsystem:
> architecture, adversarial findings, a **diagnosed live field bug**, its **root
> cause**, the `/wallet/sync` interaction, a **remediation plan for the next
> version update**, and the **fix-vs-redesign (v2)** decision inputs.
>
> Companion to [`README.md`](./README.md) (the wallet-hardening register). Sibling
> of the design docs: `development-docs/ONCHAIN_BACKUP_SYSTEM.md`,
> `Final-MVP-Sprint/wallet-backup-efficiency-plan.md`,
> `Final-MVP-Sprint/backup-double-spend-incident-2026-04-11.md`.

**Last updated:** 2026-07-07
**Owner:** Matt
**Status:** review complete; field bug diagnosed; remediation NOT yet implemented.
**Method:** 6 parallel adversarial agents (5 on backup, 1 on `/wallet/sync`) +
owner-supplied production logs + direct code verification of the key claims.

> **⚠️ CORRECTION (2026-07-07): §2 below is superseded.** §2 claimed the graceful
> shutdown path doesn't exist and OD-2 never runs (based on a mis-scoped grep that
> found only dead code). **Verified reality: the graceful path EXISTS** —
> `StopWalletServer()` (`cef_browser_shell.cpp:3581`) does `POST /shutdown` → 5s
> bounded wait → `TerminateProcess` *fallback*; the `WalletService::…TerminateProcess`
> code §2 cites is dead. The ghost still forms because the **5s fallback fires
> mid-backup** (backup ~8.6s, handler doesn't observe the shutdown token). The fix is
> therefore "make the kill data-safe in Rust," not "add graceful shutdown." Full
> corrected root cause + fix: **`FIX_B_CRASH_SAFETY_SHUTDOWN_PLAN.md` §0.** The two
> implementation plans (`FIX_A_RECONCILE_PLAN.md`, `FIX_B_CRASH_SAFETY_SHUTDOWN_PLAN.md`)
> are the authoritative remediation; §6 P0/P1 here are an earlier sketch.

---

## 0. TL;DR

- The **design and cryptography are sound.** The problems are in **operational
  implementation**: concurrency, crash-safety, recovery robustness, and payload
  completeness.
- A **live field wallet is stuck** in a permanent backup-retry loop
  (`"Missing inputs"`). Diagnosed: the DB's UTXO set is **stale vs. the chain** —
  it's trying to fund each backup with an output already spent by a newer backup
  the DB never recorded.
- **Root cause of the stale DB = the wallet is hard-killed (`TerminateProcess`)
  on app shutdown**, so a backup that broadcast successfully but hadn't yet
  written its DB record is stranded → on-chain/DB divergence ("ghost backup").
  **This confirms the owner's shutdown hypothesis.**
- **Do NOT run `/wallet/sync ?full=true` to fix it.** Current sync is
  *insert-only* (no reconcile), so it won't clear the phantom — and full sync has
  its own **critical bug** that sweeps the backup address into spendable balance
  with the **wrong signing key**, breaking future sends. (This is almost
  certainly the owner's "every full sync breaks something.")
- Fixable? **Yes, all of it.** But the **completeness⟷cost tension** is inherent
  to "whole DB on-chain, repeatedly" and is the strongest argument for a
  deliberately-designed **v2**. Recommendation: **stabilize now (ship field fixes
  in the next update), design v2 in parallel.**

---

## 1. The diagnosed field bug — stuck backup loop

### Symptom (from production logs)
Three identical `POST /wallet/backup/onchain` attempts, all failing:
```
WARN  ⚠️ Services broadcast chain exhausted: whatsonchain status 400:
      "unexpected response code 500: Missing inputs"
ERROR ❌ Backup broadcast failed ... — rolling back placeholder
INFO  ♻️  Restored 1 output(s) with placeholder pending-backup-17834
```
Retries forever (~every tick), never succeeds.

### Trace (what the logs prove)
1. DB believes the backup tip is `7c4423f4` (`7c4423f4:0` PushDrop + `7c4423f4:1`
   marker).
2. Chain disagrees: `⚠️ DB backup 7c4423f4 not found on-chain — adopting most
   recent: ef67fd9e (block 956718)`. A **newer** confirmed backup `ef67fd9e`
   exists that the DB **never recorded**.
3. The adopt logic correctly swaps the token/marker to `ef67fd9e:0` / `ef67fd9e:1`.
4. **But funding is still selected from the stale DB.** Decoding the input
   preimages, the built tx spends `ef67fd9e:0`, `ef67fd9e:1`, and **`7c4423f4:2`**
   (value 2,796,283 sats = the *change output of the DB's old backup*).
5. The BEEF dump confirms the divergence: `ef67fd9e`'s raw tx (BEEF "TX 1") begins
   `01000000 03 cbafe85f…423447c 00…` → its **first input is `7c4423f4:0`**. So
   `ef67fd9e` was built by spending `7c4423f4` and (per the self-chaining funding
   pattern) almost certainly consumed `7c4423f4:2` as its funding too.

### Conclusion
**`7c4423f4:2` is a phantom** — the DB lists it spendable, but the chain spent it
when `ef67fd9e` was created. Every backup rebuilds a tx with that dead input →
`"Missing inputs"` → rollback → retry. The wallet **can never back up again**
until its UTXO set is reconciled with the chain. Real funds are safe (they live at
`ef67fd9e:2` on-chain) but the DB tracks a ghost and likely shows a wrong balance.

**Confidence:** High. Confirmed: `ef67fd9e` spends `7c4423f4:0` (BEEF dump); the
failing tx funds from `7c4423f4:2` (preimage decode); DB tip is stale (adopt WARN).
Remaining confirmation (optional): a WhatsOnChain lookup that `7c4423f4:2` is spent.

### The retry never self-quiets
The "skip if unchanged" hash guard doesn't fire because the payload hash **changes
every run** (logs show three different `new:` hashes) — `updated_at` churn from
background tasks (finding **BS-M1**). So a permanently-failing backup re-runs the
full build+broadcast indefinitely.

---

## 2. Root cause — ghost divergence via hard-kill on shutdown

**How does a backup get confirmed on-chain but never recorded in the DB?**

The backup write path is deliberately ordered: **broadcast FIRST, then write the
DB record** (`handlers.rs` Step 11 → Step 12) so a crash leaves a reclaimable
placeholder rather than a ghost output. That ordering is correct — but it opens a
window: **broadcast succeeds (tx now on-chain) → process dies → Step-12 DB write
never happens.** On restart the DB still thinks the *previous* backup is tip, and
its funding change is a phantom (exactly §1).

**What kills the process in that window? App shutdown.** Verified in C++:

- The Rust wallet daemon is spawned by `WalletService::createDaemonProcess()` with
  **`CREATE_NO_WINDOW`** (`WalletService.cpp:738`) — it does **not** share the
  shell's console, so a console Ctrl+C is **not** delivered to it by the OS.
- The **only** daemon-stop path is `cleanupDaemonProcess()` →
  **`TerminateProcess(daemonProcess_, 0)`** (`WalletService.cpp:773`) — a **hard
  kill** (SIGKILL-equivalent). The adjacent comment *"Try to terminate gracefully
  first"* is **wrong**; there is nothing graceful about `TerminateProcess`.
- No graceful signal is ever sent from C++ to the wallet: grep found **no**
  `POST /shutdown`, **no** `GenerateConsoleCtrlEvent`, **no** SIGINT. The
  `ConsoleCtrlHandler` (`WalletService.cpp:786`) also just calls
  `stopDaemon()` → `cleanupDaemonProcess()` → `TerminateProcess`.

**Therefore the Rust wallet's elaborate OD-2 graceful-exit sequence in `main.rs`
(drain in-flight HTTP, quiesce Monitor, WAL checkpoint, finish the backup
DB-write) NEVER RUNS in production** — it requires a Ctrl+C or `POST /shutdown`
that the shell never delivers. Every app close/restart is a hard kill. If a backup
(observed here taking **8.6 s**) is mid-flight, the kill strands it → ghost
divergence.

**This matches the owner's hypothesis exactly** ("did we break waiting for the
wallet to finish on our immediate-restart change?"). The auto-update / restart flow
assumes a graceful wallet exit — `UpdateFs.cpp:221` literally comments *"after a
graceful wallet exit the snapshot is just wallet.db, -wal stays"* — but the actual
stop mechanism is a hard kill, so that assumption is false.

> **Note on `synchronous=FULL` (money-DB durability, commit `3677583`):** it
> prevents *torn/corrupt* writes, but does **not** help here — if the process is
> killed *before* Step 12 even executes, there is no write to make durable. The
> ghost divergence is a *missing* write, not a *corrupt* one.

### Verification still open (one step)
- Confirm there is no *other* graceful stop in the quit / auto-update path:
  `ShowQuitConfirmationAndShutdown()` (`simple_handler.cpp:2836`) and the updater
  relaunch — do either send `POST /shutdown` or wait on the wallet before
  `TerminateProcess`? (Grep suggests not, but confirm.) Also check whether a
  `StopWalletServer` (referenced in `main.rs` comments + `WALLET_GRACEFUL_EXIT_SPEC.md`)
  was ever wired on the C++ side — grep currently finds **none**.

---

## 3. System architecture (condensed)

Full detail from the architecture agent; summary:

- **Purpose:** back up the *whole wallet DB* (history, BRC-42 counterparty
  derivations, PushDrop tokens, certificates, permissions, baskets) — state a seed
  phrase **cannot** re-derive — into a single on-chain **PushDrop token** at a
  deterministic BRC-42 address (index **-3**, invoice `1-wallet-backup-1`).
- **Pipeline:** `collect DB → serde_json → gzip-9 → AES-256-GCM → PushDrop field`.
  Key = `SHA256(master_privkey ‖ "hodos-wallet-backup-v1")`; random 12-byte nonce
  per backup; mnemonic *is* the decryption credential and is never in the payload.
- **Self-consuming:** each backup spends the prior token+marker (recycling ~1546
  sats) so only the latest exists on-chain. Idle wallets skip via a payload-hash
  dirty check.
- **Triggers:** event-driven (>$3 USD → 3-min debounce, 10-min cap) + periodic
  every **3 h**. Each real backup pays a miner fee (no Hodos service fee on
  backups). Observed payload here: **422 KB JSON → 93 KB compressed → 284 KB BEEF**.
- **Recover:** mnemonic → re-derive backup address → find token on chain → decrypt
  → import into a fresh DB → refetch stripped data → reconcile.

---

## 4. Findings register (backup subsystem)

Severity key: **Critical / High / Medium / Low**. IDs: `BS-*` (backup subsystem).
Verified items note the code check performed.

### Create / broadcast / reconcile
| ID | Sev | Finding | Where | Notes |
|----|-----|---------|-------|-------|
| **BS-C1** | **Critical** | `do_onchain_backup` holds **neither** `create_action_lock` nor `utxo_selection_lock`, and **discards** its `mark_multiple_spent` result (`let _ =`) → double-spend when a backup overlaps a payment / dust-consolidation / another backup. Open write-side of the Apr-2026 incident. | `handlers.rs:12756`, `:13180` | **Verified**: grep of fn body finds no lock acquisition. |
| **BS-H3** | High | Crash between broadcast-success and DB-write → startup `restore_pending_placeholders()` flips on-chain-spent inputs back to spendable → phantom funding (this is §1/§2). | `handlers.rs:13440`, `main.rs:462` | The actual field mechanism. |
| **BS-H6** | High | Suspected-double-spend relabel defeats `rollback_backup` on a **false positive** → funds stranded + next backup orphans the live on-chain backup (locks 1546 sats). | `handlers.rs:13449`, `:13594` | Plausible. |
| **BS-L1** | Low | Sub-dust change (≤546) silently burned to fee, no accounting entry. | `handlers.rs:13208` | Confirmed. |

### Payload / crypto / completeness
| ID | Sev | Finding | Where | Notes |
|----|-----|---------|-------|-------|
| **BS-H2** | High | **Restore silently drops permission/settings state**: V18 scoped-grant tables (`domain_protocol/basket/counterparty_permissions`) not in payload at all; `domain_permissions` missing `max_tx_per_session`/`identity_key_disclosure_allowed`/`bundled_scope_grant`; `settings` only 7 of ~15 cols + no-PK INSERT → duplicate rows. Most likely *reported* non-fund bug. | `backup.rs:804`, `:1565`, `:730` | Confirmed. |
| **BS-M3** | Med | `proven_txs` re-inserted with **empty** `merkle_path`+`raw_tx`. | `backup.rs:559`, `:1044` | **Verified benign**: empty BLOB fails `serde_json::from_slice` → `get_merkle_proof_as_tsc` returns `None` → `build_beef_for_txid` falls back to `fetch_tsc_proof_from_api` (`beef_helpers.rs:259/269`). **Self-heals**; residual = needs network at spend time. |
| **BS-M4** | Med | `transaction_inputs` / `transaction_outputs` tables not backed up → tx-detail views incomplete after restore. | `backup.rs` (absent) | Confirmed omission. |
| **BS-M5** | Med | `peerpay_outbox` not backed up → in-flight BRC-29 delivery lost → **recipient-side** fund loss (narrow window). | `migrations.rs:924` | Plausible. |
| **BS-L2** | Low | `payload.version` never checked on decode; no `deny_unknown_fields` → newer→older loses fields silently; older→newer opaque error. | `backup.rs:839`, `:1297` | Confirmed. |
| **BS-L3** | Low | `outputs.confirmed` not backed up → defaults to 1 (cosmetic). | `backup.rs:132` | Confirmed. |
| — | — | **Verified SOUND:** AES-GCM nonce/key/tag, i64 satoshi precision end-to-end. | `backup.rs:960`, `:1270` | No crypto defect. |

### Recovery / verify / import
| ID | Sev | Finding | Where | Notes |
|----|-----|---------|-------|-------|
| **BS-C2** | **Critical** | Transient WoC failure/429 during recovery is swallowed → user told **"No backup found"** and nudged to *Create New* over a valid backup. "Chain scanning" fallback doesn't exist in this path. | `handlers.rs:14202` | Confirmed. |
| **BS-H1** | High | Recovery can restore a **stale** backup during WoC index-propagation window; no sequence number to detect it (recovery-side of the incident, unfixed TODO). | `handlers.rs:13910`, `:13944` | Confirmed (TODO + incident doc). |
| **BS-H4** | High | Pre-import cleanup DELETEs run **outside** the import transaction → mid-import failure leaves a "wallet exists but no user" DB that **traps** the user (409 on retry). | `handlers.rs:14241` vs `backup.rs:1352` | Confirmed. |
| **BS-H5** | High | "Newest marker" tiebreak (`max_by_key`, unconfirmed→`i64::MAX`) unreliable → can restore an **older** token. | `handlers.rs:13946` | Confirmed logic. |
| **BS-M7** | Med | Recovery only queries **unspent** markers → a spent-marker backup is invisible (no script-hash history fallback). | `handlers.rs:13914` | Plausible. |
| **BS-M8** | Med | Unbounded/unchecked raw-tx parsing (`try_into().unwrap()`, unguarded slices) → **panics** recovery on corrupt/truncated data instead of clean error. | `handlers.rs:13600+` | Confirmed no bounds checks. |
| **BS-M9** | Med | Post-commit refetch/reconcile is best-effort → restore "succeeds" but coins temporarily **unspendable-for-BEEF** if WoC down. | `handlers.rs:14276` | Plausible. |

### Scheduling / lifecycle
| ID | Sev | Finding | Where | Notes |
|----|-----|---------|-------|-------|
| **BS-M1** | Med | 3-h periodic "nothing changed" hash guard defeated by `updated_at` churn → ~8 real-fee backups/day; and a failing backup retries the full build forever (no backoff). | `handlers.rs:12803`, `backup.rs:442`; `mod.rs:315` | This is why §1's loop never quiets. |
| **BS-M2** | Med | Post-broadcast crash before `set_backup_hash` → redundant backup spend on restart. | `handlers.rs:13566` | Confirmed path. |
| **BS-L4** | Low | Significant-send backup silently dropped when price cache is cold (both `get_cached` and `get_stale` None). | `main.rs:267` | Confirmed. |
| — | — | **Verified SOUND:** timer cap/extend math; path-traversal hardening (`validate_backup_path`); `broadcast_transaction` txid-mismatch guard (`handlers.rs:8632`, incident "Bug B" closed). | — | — |

---

## 5. `/wallet/sync` — what it does, and why full sync "breaks things"

> The owner reports "every time we run a full sync, something seems to break," and
> asked that we understand it **before** running it. Adversarial review done. **Do
> not run `?full=true` on a real wallet until BS-SYNC-1 is fixed.**

### What it actually does (verified)
`POST /wallet/sync` (`handlers.rs:3149`):
1. **Address selection.**
   - `?full=true` → `AddressRepository::get_all_by_wallet` (`address_repo.rs:119`)
     — **every** address row, **no index filter**: derived (0+), master (-1),
     external (-2), **and the backup address (-3)**.
   - default → `get_pending_utxo_check` — `pending_utxo_check=1` OR index `-1`, and
     **explicitly `AND "index" != -3`** (backup excluded).
2. **Fetch UTXOs** from WhatsOnChain in chunks (no DB lock held; **fails closed** if
   the whole address set errors). UTXOs map back to an address purely by index.
3. **Insert only** via `upsert_received_utxo` = `INSERT OR IGNORE` on
   `UNIQUE(txid,vout)`. Mark address used.
4. **No reconcile.** `reconcile_for_derivation` was **removed 2026-04-20**
   (`output_repo.rs:1044`) because it wrongly killed $15+ of valid outputs.
   **Sync no longer marks anything `external-spend`.** The word "reconcile" in the
   CLAUDE.md docs is **stale** (BS-SYNC-2).
5. Invalidate + recompute balance.

Derivation is set from index alone: `≥0 → "2-receive address"/{index}`;
`-1 → "master"/-1`; **any other negative (incl. -3) → NULL/NULL but hardcoded
`spendable=1`**. Externally-received (PeerPay/counterparty) UTXOs are **not**
scanned here (they live on per-payment derived addresses stored via
`store_derived_utxo` with `sender_identity_key`).

### Findings
| ID | Sev | Finding | Where |
|----|-----|---------|-------|
| **BS-SYNC-1** | **High** | **Full sync sweeps the backup address (-3) into spendable balance with the WRONG signing key.** `get_all_by_wallet` has no index filter → full sync scans -3 → the backup marker is inserted `NULL/NULL, spendable=1`. `derive_key_for_output(NULL,NULL,NULL)` returns the **master** key, but -3 is a BRC-42 `1-wallet-backup-1` key. If UTXO selection ever picks it, it's signed with the wrong key → `mandatory-script-verify-flag-failed` → **the whole bundling tx fails to broadcast**, and the backup chain is corrupted. Only `?full=true` hits this (periodic sync excludes -3). **This is almost certainly the owner's "full sync breaks something."** | `output_repo.rs:463-480`, `address_repo.rs:119` |
| **BS-SYNC-2** | Med | Three CLAUDE.md files claim sync "reconciles → `external-spend`," but the live code doesn't. A future contributor "restoring" it would re-introduce the removed silent-fund-loss bug. | `output_repo.rs:1044`; docs |
| **BS-SYNC-3** | Low | No gap-limit/look-ahead: full sync only scans addresses already in the table → higher-index/counterparty UTXOs not discovered (looks like "lost coins"). | `handlers.rs:3267` |
| **BS-SYNC-4** | Low | `reconciled_count` is dead/always-zero but logged/returned → reinforces the false "reconcile runs" impression. | `handlers.rs:3315` |

### ⚠️ Consequence for the §1 field bug
**A full sync will NOT fix the stuck wallet** — sync is insert-only and does **not**
mark the phantom `7c4423f4:2` spent. Worse, it would **plant the -3 poison**
(BS-SYNC-1). The owner's instinct to not run it is correct. The right fix is a
**proper, targeted reconcile** (below), which is exactly the owner's proposed
design: *ask the chain where the output went; if the successor output is ours, find
its address, record it, and spend that instead.*

---

## 6. Remediation plan (for the next version update)

Ordered by "protect existing field wallets first." Field wallets are **already**
affected (§1 is live), so these ship regardless of the v2 decision.

### P0 — stop creating ghost divergences (the root cause)
1. **Graceful wallet shutdown from C++.** Before `TerminateProcess`, send the
   wallet a real graceful stop — `POST /shutdown` (the Rust side already implements
   the OD-2 drain+checkpoint) — and **wait** for it to exit (bounded, e.g. 10 s;
   the observed backup took 8.6 s, so 5 s is too short). Only `TerminateProcess` on
   timeout. Wire this into **both** normal quit (`ShowQuitConfirmationAndShutdown`)
   **and** the auto-update relaunch. (Root cause §2.)
2. **Serialize `do_onchain_backup`** with a dedicated async mutex mirroring
   `create_action`; stop ignoring `mark_multiple_spent`'s result. (BS-C1.)

### P1 — self-heal already-broken field wallets (they won't fix themselves)
3. **Startup reconciler for stale-tip DBs.** On startup (and when the backup path
   hits "DB backup not on-chain"), detect the divergence and **properly reconcile
   the funding**, per the owner's design:
   - Ask the chain what spent the DB's believed-spendable output (`7c4423f4:2`).
   - If the spending tx's outputs pay a **wallet-owned** address (e.g. `ef67fd9e:2`),
     insert that real UTXO and mark the phantom spent.
   - Then the backup can fund from a real output and succeed.
   This is a **new, correct reconcile** — NOT the removed `reconcile_for_derivation`
   and NOT the insert-only `/wallet/sync`.
4. **Backoff on repeated backup failure** + fix the `updated_at`-churn hash guard so
   a broken backup doesn't spin every tick. (BS-M1.)

### P2 — make recovery trustworthy (before anyone relies on it)
5. WoC error ≠ "no backup" (BS-C2); atomic import incl. pre-cleanup (BS-H4);
   reliable newest-marker selection + sequence number (BS-H1/H5); spent-marker
   fallback (BS-M7); bounds-checked parsing (BS-M8).

### P3 — completeness + guardrails
6. Add missing tables/columns to the payload (BS-H2/M4/M5) **and** a test that
   **fails when a new schema table isn't represented in `BackupPayload`** (kills the
   drift class) — weighed against payload cost (see §7).
7. Wire-format version tag + `#[serde(...)]` discipline (BS-L2).

### P-sync — make `/wallet/sync` safe to run
8. Exclude `-3` (and all non-`{≥0,-1}` indices) from the full-scan set, or insert
   them `spendable=0`. Update the stale "reconcile" docs. (BS-SYNC-1/2/4.)

---

## 7. Fix vs. redesign (v2) — decision inputs

**Everything above is fixable** — implementation defects, not architectural dead
ends. But one pressure the fixes can't remove:

> **Completeness ⟷ cost.** The payload is inscribed on-chain (**93 KB here, 284 KB
> BEEF**) and paid for on **every** backup, forever. Completeness (adding the
> missing tables) makes the payload bigger → higher recurring fee; the strips that
> control cost are exactly what cause the silent restore gaps (BS-H2/M4/M5). This
> tension is **inherent to "whole DB on-chain, repeatedly"** and worsens as a wallet
> ages and the payload grows.

**Recommendation:** **stabilize the current system now** (P0–P1 are non-optional —
live wallets are broken) **and design a v2 in parallel.** Don't let a months-long v2
delay the field fixes.

### v2 design questions (to explore separately)
- Off-chain encrypted blob (cloud/user-held) + a tiny on-chain pointer/hash?
- Incremental/delta backups instead of full-DB each time?
- Back up only the **non-re-derivable minimum** and re-derive the rest on restore?
- Keep the on-chain token but cap/shard it; move bulk history off-chain?
- Relationship to the threshold-key / Dolphin-Milk direction, if any?

---

## 8. Open questions / verification TODO
- [ ] Confirm no graceful stop exists in `ShowQuitConfirmationAndShutdown` /
      auto-update relaunch before `TerminateProcess` (grep says no; confirm).
- [ ] WhatsOnChain lookup: confirm `7c4423f4:2` is spent (finalize §1).
- [ ] Confirm `ef67fd9e:2` pays a wallet-owned address (validates the P1 reconcile).
- [ ] Was a C++ `StopWalletServer` ever wired? (`main.rs` comments +
      `WALLET_GRACEFUL_EXIT_SPEC.md` reference one; grep finds none.)
- [ ] How many field wallets are likely diverged? (Any telemetry / the treasury
      wallet from the Apr incident?)

## 9. Related
- `README.md` — wallet-hardening register (add BS-C1/H3 → root-cause link here)
- `development-docs/ONCHAIN_BACKUP_SYSTEM.md` — original design
- `Final-MVP-Sprint/backup-double-spend-incident-2026-04-11.md` — prior incident
  (write-side half-fixed; recovery side + this shutdown root cause still open)
- `Final-MVP-Sprint/wallet-backup-efficiency-plan.md` — the cost-vs-size work
- `WALLET_GRACEFUL_EXIT_SPEC.md` (DevOps-CICD) — the graceful-exit design that the
  C++ hard-kill currently defeats
