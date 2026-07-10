# Fix B — Crash-Safe Backup + Shutdown/Restart: Implementation Plan

> Prevents NEW "ghost divergences" (a backup confirmed on-chain but absent from the
> DB, which strands funding forever). Makes a hard kill data-safe at ANY instant by
> persisting backup INTENT before broadcast and reconciling `sending` backups against
> the chain on startup — so shutdown never has to wait for the ~8.6s backup.
>
> Companion: [`ONCHAIN_BACKUP_REVIEW.md`](./ONCHAIN_BACKUP_REVIEW.md) and
> [`FIX_A_RECONCILE_PLAN.md`](./FIX_A_RECONCILE_PLAN.md) (Fix A heals wallets ALREADY
> diverged; Fix B stops new ones).

**Status:** design complete, hardened against adversarial review. NOT implemented.
**Basis:** Rust research agent + C++ research agent + adversarial red-team + direct
code verification.

---

## 0. ⚠️ Corrected root cause (supersedes REVIEW §2)

The REVIEW doc's §2 said the graceful path doesn't exist and OD-2 never runs. **That
was wrong** (based on a mis-scoped grep that found only dead code). Verified reality:

- **The graceful stop path EXISTS and is wired.** `StopWalletServer()`
  (`cef_browser_shell.cpp:3581`, called from the quit thread `:594`) does
  `SendShutdownRequest` (`POST /shutdown`, `:3428`) → `WaitForSingleObject(…, 5000)`
  (`:3592`) → `TerminateProcess` **fallback** (`:3597`). macOS mirrors it
  (`cef_browser_shell_mac.mm`, SIGTERM fallback). The auto-update path
  (`update-helper` `transaction.cpp`) also POSTs `/shutdown` + polls image-unlock +
  has a sibling-guard ("never install over a live wallet").
- **`WalletService::cleanupDaemonProcess → TerminateProcess` is DEAD code** —
  `startDaemon()` has zero call sites; `WalletService` is only a stateless HTTP client.
- SQLite already: `journal_mode=WAL` + `busy_timeout=5s` + `synchronous=FULL` +
  `foreign_keys=ON` (`connection.rs:61/66/72/85`). OD-2 checkpoints on clean exit
  (`main.rs:1142-1173`). The `/shutdown` route cancels the token (`handlers.rs:193`).

**So how does the ghost form?** The **fallback kill fires mid-backup**: a backup takes
~8.6s; the backup handler does **not** observe the shutdown token; `server.stop(true)`
keeps draining the in-flight backup; C++ waits only **5s**, then `TerminateProcess`
fires — landing in the broadcast→DB-write window → tx confirmed on-chain, DB record
never written → ghost divergence (`ef67fd9e`). `synchronous=FULL` doesn't help: the
write never happens, so there's nothing to make durable.

**Consequence for the fix:** the fix is **NOT "add graceful shutdown"** (it exists) —
it's **make the kill data-safe in Rust** so the 5s-fallback (or any kill) can't strand
a backup. The C++ side shrinks to: verify, delete dead code, and close two real
relaunch hazards. **The DB lock is a red herring — WAL is already multi-process-safe;
the real relaunch chokepoint is the TCP port bind (see §3, RED-TEAM H1).**

---

## 1. Rust half — intent-before-broadcast + startup `sending`-reconcile

### 1a. Current ordering & the kill window
`do_onchain_backup` (`handlers.rs:12756`): reserve inputs under `pending-backup-{ts}`
placeholder (`:13180`) → build → sign → `txid` known (`:13356`) → cache raw tx → BEEF
→ **broadcast (`:13440`)** → **only on success** write tx row `status='unproven'` +
create outputs + relabel reservation → txid (`:13468-13545`) → hash/last_backup_at
(`:13549-13578`). **Kill window = broadcast Ok (`:13441`) → Step-12 commit (`:13545`).**
A kill there = confirmed on-chain, no tx row, no outputs, inputs still under
`pending-backup-{ts}`; next boot `restore_pending_placeholders()` (`main.rs:462`) frees
the on-chain-spent inputs → phantom funding. **Ghost-output avoidance confirmed real:**
output rows are created only post-broadcast, so a kill leaves NO ghost outputs.

### 1b. New ordering (preserves ghost-output avoidance)
Insert **Step 11a (persist intent)** after `txid` is known, before broadcast — one
SQLite txn: INSERT tx row `status='sending'` (real txid, `description='On-chain wallet
backup'`, raw_tx) + relabel reservation `pending-… → txid` + `ptx_repo.create(…,"sending")`.
**Do NOT create outputs yet** (that preserves ghost-avoidance). Then broadcast
(shutdown-aware). On Ok → **`finalize_backup_records()`** (shared fn: create the 3
outputs, `UPDATE transactions SET status='unproven'`). On fatal reject → rollback keyed
on real txid.

Relabeling the reservation to the **real txid** removes it from
`restore_pending_placeholders`' `LIKE 'pending-%'` net (`output_repo.rs:947`), so the
old restorer can't fight the new reconciler.

### 1c. Recoverability at every kill point
| Kill at | tx row | outputs | reservation | on-chain | recovered by |
|---|---|---|---|---|---|
| before 11a | none | none | `pending-%` | no | `restore_pending_placeholders` (unchanged) |
| mid-11a | none (txn rolls back) | none | `pending-%` | no | same |
| after 11a, before broadcast returns | `sending` | none | real txid | maybe | **`sending`-reconcile** (ask chain) |
| after broadcast Ok, before finalize | `sending` | none | real txid | **yes** | **`sending`-reconcile** → adopt/finalize |
| after finalize | `unproven` | present | consumed | yes | benign |

No window produces a ghost output; no window both broadcasts and leaves inputs eligible
for `restore_pending_placeholders`. **That is the whole point.**

### 1d. Startup `sending`-reconcile (replaces `main.rs:484-513`)
For each `status='sending' AND description='On-chain wallet backup'`: query
`services.tx_status(txid)` (multi-provider, reuse `task_send_waiting.rs:97`). **Landed
(Mined/InMempool) →** rebuild output descriptors from the on-chain raw tx (recover the
change derivation by script-match, like Fix A) → `finalize_backup_records` → advance
tip. **Authoritatively absent/rejected (after a min-age gate) →** rollback (restore
inputs by real txid, delete tx row + req). **Unknown / network error / 429 →** leave
`sending` untouched, retry next boot. Run it **after AppState is built (so `services`
exists), before the Monitor starts.**

### 1e. Shutdown-token abort
`tokio::select!` the broadcast await against `state.shutdown.cancelled()`; on cancel,
**return a DISTINCT abort outcome that leaves the `sending` row + reservation intact and
does NOT call `rollback_backup`** (RED-TEAM C4). Plus a top-of-fn `is_cancelled` guard.
Safe because intent is already persisted; the next-boot reconcile decides landed-or-not.
This lets OD-2 drain fast instead of blocking ~8.6s.

## 2. Guardrails (from red-team — BLOCKING = 1–5, required-before-merge = 6–10)
1. **Fail-closed reconcile:** roll back a `sending` backup ONLY on an authoritative
   "not found" from multi-provider `services.tx_status`; on any error/timeout/429/Unknown
   keep `sending` + reservation, retry. A false rollback re-creates the exact divergence.
2. **Reuse `services.tx_status` + `TxState`** (ARC→WoC→JungleBus→Bitails), not a single WoC call.
3. **Finalize via UPDATE/upsert, NOT `INSERT OR IGNORE`** (the current finalize at
   `:13478` is `INSERT OR IGNORE`; with a pre-existing `sending` row it would be ignored
   → stuck `sending` forever). Explicit state machine `sending → unproven → completed`,
   reconcile the only other writer.
4. **Replace, don't stack, the incumbent startup reconcilers** (`main.rs:462` and
   `:484-513`); gate `restore_pending_placeholders` to skip inputs owned by a pending
   `sending` backup; run `sending`-reconcile BEFORE generic placeholder restore.
5. **Distinguish shutdown-abort from broadcast-reject** (C4 above).
6. **Unify the reservation linkage** (tx-row txid / `spent_by` FK) so rollback, the
   suspected-double-spend verifier (`:13449`), and the reconcile all key off one thing —
   not a brittle `spending_description` string a third path rewrites.
7. **Startup barrier:** Monitor backup task blocked until all `sending` backups are
   reconciled; backup selection treats `sending`-backup inputs as reserved.
8. **Actually observe the token** around the broadcast `select!`, and bound the
   server-drain so an in-flight backup can't stall shutdown past the C++ wait.
9. **`TaskSendWaiting` must exclude backups** (`AND description != 'On-chain wallet
   backup'`) — else it `promote_to_unproven` a landed backup WITHOUT creating outputs
   (`task_send_waiting.rs:216`), moving the divergence from tx-row-missing to outputs-missing.
10. **Idempotent finalize/adopt** so reconcile + `TaskUnFail` + `/wallet/sync` can't duplicate outputs.

## 3. C++ / shutdown-lock half — mostly verify + subtract
Because the graceful path already exists and the Rust side makes kills data-safe:
- **MINIMAL change.** Keep `StopWalletServer`'s graceful→bounded-wait→fallback design;
  do NOT lengthen the wait (with crash-safety, letting the fallback fire is fine).
- **Delete the dead `WalletService` daemon-management code** (`WalletService.cpp:673-800`
  + `_mac.cpp:174-181` + `WalletService.h` decls) — it's the only non-graceful
  `TerminateProcess` in the tree and is what made this look un-done. Pure subtraction.
- **G1 — attach-to-a-dying-wallet (correctness):** on fast relaunch, `LaunchWalletProcess`
  (`:3483`) reuses a listening port — which may be a wallet mid-drain about to `exit(0)`.
  Fix: `/health` reports not-ready once the shutdown token is cancelled, AND/OR
  `LaunchWalletProcess` requires a *ready* health check before reusing the port.
- **H1 — relaunch fails on the PORT bind, not the DB lock (RED-TEAM):** if the new
  wallet binds `31301` before the old released it, `main.rs:1111 .bind(…)?` returns
  AddrInUse and the new wallet exits → permanent no-boot. Fix: **bounded port-bind (and
  ProfileLock) retry-with-backoff** at Rust startup, and/or C++ waits for the old
  process handle to signal before spawning the new wallet. `busy_timeout` does NOT
  help here — WAL is already multi-process-safe; the port is the chokepoint.
- **M3 — updater/WAL invariant:** the updater must complete the file swap BEFORE the new
  instance opens the DB (else `-wal`/`-shm` reappear, violating `UpdateFs.cpp:220-226`).
- **De-hardcode ports** in `update-helper` (`transaction.cpp` uses literal 31301/31302
  instead of `hodos::WalletPort()`) — latent, low urgency.
- **Cross-platform parity:** delete the dead daemon-mgmt symbols on Win + mac together;
  `/health` not-ready + port-bind-retry are in shared Rust (portable).

## 4. Composition with Fix A (explicit ordering)
Fix B's `sending`-reconcile heals the FRESH broadcast→finalize window (a `sending` row
exists). Fix A's funding-reconcile heals LEGACY divergences (no `sending` row; the
historical bug never wrote one). They key on disjoint state and must run **Fix B first,
then Fix A** at startup: Fix B finalizes/rolls-back the in-flight backup so the DB tip is
truthful, then Fix A's "is my funding a phantom?" check sees a consistent picture.
Both fail-closed on network error. For legacy wallets Fix B is a no-op and Fix A heals;
for new-code wallets Fix B prevents divergence and Fix A rarely fires.

## 5. Phased commits
**Rust:** (1) extract `finalize_backup_records` (idempotent, UPDATE-not-IGNORE; pure
refactor). (2) reorder to intent-before-broadcast + real-txid rollback. (3) shutdown-token
`select!` + top-of-fn guard. (4) startup `sending`-reconcile replacing `main.rs:484-513`
(chain-aware, fail-closed, after AppState, before Monitor; startup barrier). (5) exclude
backups from `TaskSendWaiting`. (6) compose ordering with Fix A + docs.
**C++:** (7) verify-only test pass on current binaries (graceful path fires). (8) close
G1 (`/health` not-ready on cancel + ready-check before port reuse). (9) port-bind retry
(Rust) / wait-for-handle-before-spawn (C++) for H1. (10) delete dead `WalletService`
daemon code. (11) optional: de-hardcode updater ports.
Each commit builds green and is independently revertible; Rust 1–2 are load-bearing, 4 is
the self-heal.

## 6. Test plan
**Unit (in-memory sqlite):** `finalize_backup_records` idempotency + UPDATE advances
`sending→unproven`; intent-row shape (one `sending` row, inputs keyed on txid not
`pending-%`, zero outputs); `restore_pending_placeholders` non-interference; reconcile
decision table (Mined→finalize, rejected/absent→rollback, network-error→untouched);
change-derivation recovery (match→correct prefix, no-match→`spendable=0`); shutdown guard
(pre-cancel → returns abort Err without broadcasting); `TaskSendWaiting` filter excludes backups.
**Integration (real sqlite, mocked broadcaster/services):** happy path; broadcast-failure
rollback; **Fix A + Fix B ordering** (a wallet with both a `sending` landed backup and a
phantom funding output → B finalizes first, then A finds nothing to do).
**Crash-injection (the proof-of-fix):** fund a dev wallet → trigger backup → hard-kill
(`taskkill /F` Win / `kill -9` mac) **between broadcast-accept and finalize** → assert
on-chain tx exists, DB has `sending` + no outputs + inputs reserved on real txid → restart
→ assert reconcile finalized it (outputs present incl. correctly-derived change, status
`unproven`, tip advanced, `restore_pending_placeholders` did NOT free spent inputs) →
second backup succeeds (no "Missing inputs" loop). Negative variant: kill BEFORE broadcast
→ rollback, inputs spendable, next backup ok.
**Relaunch/lock:** quit → relaunch within <500ms (same profile), 50× loop → new wallet
opens DB with no `SQLITE_BUSY`/AddrInUse and no ghost; simulated auto-update relaunch on a
funded wallet (N−1→N) → images unlock, `{db,-wal,-shm}` round-trips, funds intact, no
divergence; attach-to-dying-wallet (G1) → new instance doesn't reuse the dying port;
sibling-guard → update aborts rather than kill a shared wallet.
**Parity:** crash-injection + relaunch tests on Windows AND macOS (per the standing
"verify real N−1→N update+relaunch on funded wallets before promote" principle). This
Rust fix must self-heal even if C++ keeps hard-killing.

## 7. Open decisions
- C++ bounded wait value (keep 5s vs tune) once the Rust fast-abort lands.
- Min-age / confirmation gate before the reconcile may conclude "never landed."
- Whether to add a running-process backup-aware `TaskSendWaiting` variant, or rely
  solely on startup coverage (every kill is a restart, so startup suffices).
