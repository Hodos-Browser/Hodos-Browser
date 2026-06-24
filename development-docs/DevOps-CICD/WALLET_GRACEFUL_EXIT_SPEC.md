# Wallet Graceful-Exit Spec — process::exit(0)-after-drain (OD-2)

**Created:** 2026-06-24 / **Status:** Spec — DOCS-ONLY, NO code / **Owner:** DevOps + wallet owner sign-off
**Canonical home:** `development-docs/DevOps-CICD/` / Pairs with `WINDOWS_AUTOUPDATE_PLAN.md` §E.5/OD-2 (the silent updater needs deterministic wallet-image-lock release).

> **What this is.** A hardened spec for giving the Rust wallet (`hodos-wallet.exe`, :31301) a deterministic clean self-exit after its graceful drain, instead of relying on the C++ `TerminateProcess` hard-kill. Owner-approved in principle ("we should have had this anyway"); this spec defines the exact sequence, the owner-sign-off gates, and the test matrix BEFORE any code.
>
> **Method.** Authored by adversarial workflow `wf_895ad407-6ff` (2026-06-24, 5 agents): 4 parallel skeptics (DB durability / Monitor quiesce / shutdown-path duality+necessity / Drop-skip secret hygiene) -> referee synthesis. Every claim verified against source.
>
> **Headline verdicts (all four skeptics): Safe-with-fixes, NONE worse than the status-quo `TerminateProcess`. Referee: GO, with two gates.**

## 0. TL;DR

- **exit(0)-after-drain is STRICTLY SAFER than today's `TerminateProcess`** on every axis (in-flight HTTP, Monitor ops, updater lock-release) and **NEUTRAL on secrets** — never worse. Confirmed because the wallet already survives hard-kill via **SQLite WAL + extensive startup recovery**.
- **The sequence is what matters** (NOT adblock's blind 100ms timer): drain HTTP -> **JOIN the Monitor with a 3s cap** (capture the `JoinHandle` that's discarded today) -> WAL `checkpoint(TRUNCATE)` (proceed-on-failure) -> flush logger -> `exit(0)`. Total budget **< ~4.0s**, inside the C++ 5s `WaitForSingleObject` so the clean branch fires, not `TerminateProcess`. The `/shutdown` handler stays thin so the 200 flushes before exit (C++ `SendShutdownRequest` needs to READ the 200).
- **exit(0) is NOT strictly necessary** (TerminateProcess also frees the lock) but it IS the better fix; **recommendation: do BOTH** exit(0) AND an updater-side wait-for-PID-exit/lock-release as belt-and-suspenders.
- **TWO items need owner sign-off** (money-DB adjacency, CLAUDE.md #2/#3): (1) make `PRAGMA synchronous=NORMAL` explicit; (2) wrap four multi-statement money-DB ops (`task_send_waiting` / `task_fail_abandoned` / `task_unfail` cleanups + `transaction_repo::add_transaction`) in real SQLite transactions. These close a **pre-existing** torn-write window and are independently correct, but edit money-DB write sequencing. The exit-sequencing core needs no sign-off beyond normal review.
- **Do NOT mis-sell as a secrets fix** — the plaintext mnemonic is not zeroed on exit, identical to today; secret hygiene is a separate, out-of-scope hardening pass.

---

## 1. Goal & scope

### What changes

Add a deterministic, clean self-exit to the Rust wallet process (`hodos-wallet.exe`, actix-web on `127.0.0.1:31301`) after its graceful drain completes. Today the `/shutdown` handler (`rust-wallet/src/handlers.rs:193-197`) only calls `data.shutdown.cancel()` and returns 200 — the process keeps running until the C++ side hard-kills it (`StopWalletServer` → `TerminateProcess`, `cef_browser_shell.cpp:3514-3527`) or the Job Object kill fires. We replace "drain and wait to be killed" with "drain → quiesce the Monitor → checkpoint WAL → flush logger → `std::process::exit(0)`", generalizing the adblock pattern (`adblock-engine/src/handlers.rs:30-38`) but accounting for the wallet's money-DB and its 13-task background Monitor, which adblock does not have.

Two concrete goals:
- **(a) Deterministic, fast image-file-lock release** so the Windows silent auto-updater can replace `hodos-wallet.exe` without racing a lingering lock.
- **(b) Stop relying on the C++ `TerminateProcess` hard-kill** that can land mid-DB-write, by converting the C++ `WaitForSingleObject(5000)` path from "always times out then force-kill" into a fast clean-exit path.

### What does NOT change

- **No crypto / signing / derivation logic** (`src/crypto/`). Untouched.
- **No DB schema / migrations** (`src/database/migrations.rs`). No new tables, no column changes. The one new PRAGMA (`synchronous=NORMAL`) makes the *existing implicit WAL default explicit*; it is not a schema change.
- **No on-chain backup on shutdown** — the existing decision (`handlers.rs:188-192`) that on-chain backup is too slow (~60s) for the shutdown window stands. We do NOT wait for `TaskBackup`.
- **No new secret-zeroization** — out of scope (see §4).
- **C++ side is left functionally unchanged** except a small, optional logging/verification touch-up in `StopWalletServer` (see §5). The 5s timeout does **not** need raising.

### Money-process sign-off note

This change lives in the shutdown/process path, not in crypto or schema, so it is not a §2/§3 invariant change. **However, two sub-items touch money-DB write logic and REQUIRE explicit owner sign-off before implementation:**

1. **Wrapping the Monitor's multi-statement cleanup ops** (`task_send_waiting.rs::cleanup_failed_sending_impl`, `task_fail_abandoned.rs` ghost cleanup, `task_unfail.rs::recover_transaction`, and `transaction_repo.rs::add_transaction`) in real SQLite transactions. This is independently-correct defense-in-depth but it edits money-DB write sequencing — flag for sign-off (CLAUDE.md #2/#3 adjacency).
2. **Adding `PRAGMA synchronous=NORMAL`** explicitly in `connection.rs`. It only makes the current behavior auditable, but it is a durability-pragma change adjacent to the money DB — flag for sign-off.

The exit-sequencing, Monitor `JoinHandle` capture, between-task cancellation check, WAL checkpoint-at-exit, and logger flush do **not** require sign-off beyond normal review — they do not alter what is written, only *when the process stops*.

---

## 2. The exact shutdown sequence

The sequence runs in a **separate `tokio::spawn`ed task**, not inline in the `/shutdown` handler. The handler stays a thin `cancel + 200` so the C++ caller's `WinHttpReceiveResponse` reads the 200 cleanly (`SendShutdownRequest` returns `success = WinHttpReceiveResponse(...) == TRUE`, confirmed at `cef_browser_shell.cpp:3358,3364` — it requires reading the response, so the 200 MUST flush before exit).

Ordered steps, from `/shutdown` signal to `process::exit(0)`:

| # | Step | Where | Detail |
|---|------|-------|--------|
| 0 | Handler returns `200 {"status":"shutting_down"}` | `handlers.rs::shutdown` | Stays thin. Only `data.shutdown.cancel()` + `HttpResponse::Ok()`. The shutdown is driven by the cancellation token, observed by the coordinator below — **not** by spawning the exit from inside the handler. |
| 1 | Cancel the token | (already happens in handler) | `CancellationToken` is the single shutdown signal. Both Ctrl+C and `/shutdown` cancel it. |
| 2 | **Drain in-flight HTTP** | `main.rs:1096-1100` shutdown watcher | `shutdown_token.cancelled().await` → `server_handle.stop(true).await`. This finishes in-flight HTTP requests (including any in-flight `createAction` holding `create_action_lock`). This is the **existing** mechanism; we keep it and exit strictly *after* it resolves. |
| 3 | **Quiesce the Monitor (JOIN, bounded)** | new coordinator in `main.rs` | Capture the `JoinHandle` that `Monitor::start` currently discards (`monitor/mod.rs:115-117`) and `tokio::time::timeout(Duration::from_secs(3), monitor_handle).await`. The Monitor loop's `tokio::select!` already breaks on `shutdown.cancelled()` at the top of the loop (`mod.rs:185-189`); we add a between-task `is_cancelled()` check so the loop breaks after the current in-flight task instead of running the rest of the tick. **Decision: JOIN, not drop** (reconciliation below). |
| 4 | **WAL checkpoint** | new, before exit | Acquire the DB mutex one final time, run `PRAGMA wal_checkpoint(TRUNCATE)` with a short timeout and **proceed-on-failure** (never block exit on it). This (i) releases/truncates the `-wal`/`-shm` so the next open is a clean no-replay open, (ii) forces the NORMAL-mode fsync, closing the last-transaction-loss window, and (iii) is what makes the image-file area releasable cleanly for the updater. |
| 5 | **Flush logger** | new, last-but-one | Emit a final `"clean exit"` log line, then call the `flexi_logger` `LoggerHandle::flush()` synchronously on the same thread that calls `exit()`. `WriteMode::Direct` (`main.rs:155`) already flushes per-record, but the explicit flush + final line removes the cross-thread torn-write race and lets the boot log distinguish clean-exit from kill. Keep `_logger` alive (do not let it `Drop`) until after the flush. |
| 6 | `std::process::exit(0)` | new, last | Strictly after steps 2–5. |

### Join-vs-drop decision (step 3)

**JOIN the Monitor with a 3s bounded timeout. Do not drop it.** Rationale: `server.stop(true)` drains only HTTP — it does **not** touch the Monitor, which is a separate `tokio::spawn` whose handle is discarded today. If we exit immediately after the HTTP drain (adblock-style 100ms timer), `exit(0)` can land *between two statements* of a Monitor multi-statement op (e.g. `task_fail_abandoned`: mark-failed → delete-outputs → restore-inputs), tearing it at a non-deterministic point. That tear is recoverable (startup recovery heals it), but it undermines the entire value proposition of a *deterministic clean exit*. Joining the Monitor guarantees no task is mid-sequence at the moment of exit, making `exit(0)` **strictly ≥** status quo rather than merely equal to it. Dropping it would leave us exactly where `TerminateProcess` already is.

### Timeout budget (reconciled against the C++ 5s hard-kill)

The C++ `StopWalletServer` does `WaitForSingleObject(hProcess, 5000)` (`cef_browser_shell.cpp:3514`) before falling through to `TerminateProcess`. The wallet MUST self-exit before that 5000ms elapses, **measured from when C++ sends the POST**. Budget:

| Phase | Budget | Notes |
|-------|--------|-------|
| HTTP drain (`server.stop(true)`) | ~fast (in-flight requests are short; a multi-second `createAction` is the worst case but it holds a request slot, not the whole budget) | Already exists. |
| Monitor join | **3s hard cap** (`timeout(3s, handle)`) | This is the dominant term. 3s < `TaskBackup`'s 60s, so a backup-in-flight is *intentionally abandoned* (see §4). |
| WAL checkpoint | sub-100ms typical; capped, proceed-on-failure | Never blocks past its short cap. |
| Logger flush | negligible | |
| **Total target** | **< ~4.0s** | Leaves ≥1s headroom under the C++ 5s wait so the clean `WAIT_OBJECT_0` branch fires deterministically, not the `TerminateProcess` fallback. |

Do **NOT** use a fixed 100ms-then-exit timer like adblock — adblock has neither a Monitor nor a money DB.

---

## 3. Code-change map

| File / function | Change | Approach |
|---|---|---|
| **`rust-wallet/src/handlers.rs`** `shutdown()` (lines 193-197) | Keep thin. | No `process::exit` here. It stays `data.shutdown.cancel()` + `HttpResponse::Ok()`. The exit is owned by the coordinator in `main.rs` driven off the same token, guaranteeing the 200 flushes before exit (the C++ `SendShutdownRequest` requires reading the response). |
| **`rust-wallet/src/main.rs`** shutdown watcher (lines 1094-1102) | Extend into the **shutdown coordinator**. | After `server_handle.stop(true).await`, add: (a) `tokio::time::timeout(Duration::from_secs(3), monitor_handle).await` (log whether it joined or timed out); (b) final WAL checkpoint via the DB mutex (proceed-on-failure); (c) emit final log line + `logger_handle.flush()`; (d) `std::process::exit(0)`. The `monitor_handle` and `logger_handle` must be captured into scope (see below). Keep the existing `server.await` at line 1102 as the Ctrl+C/normal-return path — the coordinator's `exit(0)` will normally fire first under `/shutdown`; the `server.await` return remains the fallback for a Ctrl+C path that reaches it. |
| **`rust-wallet/src/main.rs`** `_logger` (line 276) | Hold the `LoggerHandle` reachable by the coordinator. | Move/clone the `LoggerHandle` so the coordinator task can call `.flush()` before exit. Do not let it `Drop` early. |
| **`rust-wallet/src/monitor/mod.rs`** `Monitor::start` (lines 109-118) | **Return the `JoinHandle`.** | Change signature to `pub fn start(state) -> Option<JoinHandle<()>>` (or `JoinHandle<()>`), returning the handle from `tokio::spawn(...)` instead of discarding it. Preserve the `MONITOR_STARTED` `AtomicBool` no-op-on-second-call behavior (return `None` on the duplicate call). `main.rs` stores the returned handle for the coordinator to join. |
| **`rust-wallet/src/monitor/mod.rs`** run loop (lines 183-365) | Add a **between-task cancellation check**. | After each `task_xxx::run(...).await`, add `if self.state.shutdown.is_cancelled() { info!("Monitor: cancellation observed between tasks — breaking"); break; }`. This bounds worst-case quiesce to **one in-flight task** rather than a full tick of all 13 tasks. The tasks themselves do NOT become cancellation-aware mid-run (too invasive, §2/§3 adjacency); the check sits only at safe points between whole logical operations. |
| **`rust-wallet/src/database/connection.rs`** PRAGMA block (lines 59-73) | Add `PRAGMA synchronous=NORMAL` explicitly. | After the WAL/foreign_keys/busy_timeout PRAGMAs, set `synchronous=NORMAL` and `query_row("PRAGMA synchronous")` to log/verify the live value is `1`. This makes the implicit WAL default auditable. **Requires owner sign-off** (durability pragma adjacent to money DB). |
| **`rust-wallet/src/database/connection.rs`** new helper | Add a `checkpoint_truncate()` method on `WalletDatabase`. | Thin wrapper: `self.conn.query_row("PRAGMA wal_checkpoint(TRUNCATE)", ...)` returning a `Result` the coordinator can log-and-ignore. Called once from the coordinator at step 4. No schema impact. |
| **`rust-wallet/src/monitor/task_send_waiting.rs`** `cleanup_failed_sending_impl` (lines ~254-321) | Wrap the 4-statement cleanup in one SQLite transaction. | Use `conn.unchecked_transaction()` / `commit()` (the pattern already used in `address_repo.rs:241`, `backup.rs:1360`) around UPDATE-failed → disable-outputs → restore-inputs (balance-cache invalidate stays outside the txn). **Owner sign-off required** (money-DB write logic). |
| **`rust-wallet/src/monitor/task_fail_abandoned.rs`** ghost cleanup (lines ~60-107) | Same transactional wrap. | mark-failed → delete-outputs → restore-inputs in one txn. **Owner sign-off required.** |
| **`rust-wallet/src/monitor/task_unfail.rs`** `recover_transaction` | Same transactional wrap for its mark/restore sequence. | **Owner sign-off required.** |
| **`rust-wallet/src/database/transaction_repo.rs`** `add_transaction` (lines ~22-120) | Wrap the 4+ INSERTs (transactions → tx_labels → tx_labels_map → inputs → outputs) in one transaction. | Closes the pre-existing torn-write window on the `createAction` DB-write path so an unclean kill mid-sequence rolls back atomically. **Owner sign-off required** — this is the most money-adjacent of the set. |

> The transactional-wrap items (last four rows) are **independently correct** and close a *pre-existing* torn-write window that both `exit(0)` and `TerminateProcess` can hit today. They are strongly recommended to land alongside the exit change so "deterministic clean exit" rests on atomic writes rather than on recovery-task cleanup — but they are gated on owner sign-off and can be sequenced as a separate, reviewed commit if the owner prefers to land the exit-sequencing first.

---

## 4. What we deliberately do NOT do — and why

1. **Do NOT wait for `TaskBackup` to finish.** `task_backup::run` POSTs `/wallet/backup/onchain` with a 60s reqwest timeout and the handler broadcasts an on-chain tx (~60s). The 3s monitor-join timeout is *deliberately shorter* than this, so a backup-in-flight is **intentionally abandoned**. Waiting for it would blow past the C++ 5s `WaitForSingleObject` and force the `TerminateProcess` path anyway — strictly worse. The abandoned state (on-chain tx broadcast but DB backup-state row unwritten) is **tolerated and recoverable**: backup state is hash-compared and idempotent on the next Monitor run. This matches the existing `handlers.rs:188-192` decision that removed on-chain backup from `/shutdown`.

2. **Do NOT set `PRAGMA synchronous=FULL`.** For a *clean* `exit(0)` after a WAL checkpoint, NORMAL is crash-safe for committed transactions and the pre-exit checkpoint forces the fsync. FULL only matters for OS-crash/power-loss, which this change does not affect. Adding FULL would slow every commit during normal operation for zero benefit on the clean-exit path.

3. **Do NOT add `zeroize` / secret-zeroization as part of this change.** `process::exit(0)` skips `Drop`, so the plaintext `cached_mnemonic` (`connection.rs:78`) is not zeroed — but this is **byte-for-byte identical to the status-quo `TerminateProcess`**, which also skips `Drop`. There is no zero-on-`Drop` impl today (`Drop` only logs). A single-site zeroize would give false confidence while the mnemonic is re-expanded into seed/master-privkey buffers on 152 call sites across 15 files, none zeroized. Secret hygiene is a real but **pre-existing, out-of-scope** hardening pass; file it separately, gated on owner sign-off (crypto adjacency). Do not mis-sell OD-2 as a secrets improvement — it is secrets-neutral.

4. **Do NOT raise the C++ 5s timeout.** The budget (§2) keeps total self-exit under ~4.0s, comfortably inside the existing 5s `WaitForSingleObject`. Raising it would only matter if we *waited* for slow background work — which we explicitly don't.

5. **Do NOT exit on a fixed wall-clock timer (the adblock 100ms pattern).** The wallet's Monitor multi-statement ops are not (yet) all transaction-wrapped and are not cancellation-interruptible mid-task, so a blind timer can tear a logical op. Exit must be *sequenced* (drain → join → checkpoint → flush → exit), never raced.

6. **Do NOT make the Monitor tasks cancellation-aware mid-run.** Only a between-task `is_cancelled()` check is added. Pushing the token *into* tasks would touch money-DB control flow far more invasively than a shutdown-path change warrants.

---

## 5. Interaction with the C++ side & the updater

### Is `exit(0)` strictly necessary? (verdict from the ordering skeptic)

**`exit(0)` is NOT strictly necessary for correctness** — `TerminateProcess` and the Job-Object kill *also* release the `hodos-wallet.exe` image lock the instant the process dies, and the updater could instead POST `/shutdown`, then poll/wait for the PID to exit (or the file lock to free) before swapping the exe. That is the simpler fix and avoids all Monitor/flush/checkpoint concerns.

**However, `exit(0)` is the BETTER fix, grounded in the current code:**
- **(a)** It converts the `StopWalletServer` `WaitForSingleObject(5000)` from an *always-times-out-then-`TerminateProcess`* path (`cef_browser_shell.cpp:3514-3527`) into a fast clean-exit path — eliminating both the up-to-5s shutdown stall *and* the mid-DB-write hard-kill OD-2 wants to stop relying on.
- **(b)** It gives the updater a deterministic, fast "process is gone" signal (wait on the PID handle) instead of a fragile "poll the file lock until it frees" loop that must guess a timeout.

**Recommendation: do BOTH.** Implement `exit(0)` (drained, Monitor-quiesced, WAL-checkpointed, logger-flushed, response-flushed) as the **primary** mechanism, AND have the updater **wait-for-PID-exit / wait-for-lock-release** as a belt-and-suspenders guard so the updater never swaps the exe while the lock is still held even if `exit(0)` is delayed. The WAL checkpoint at step 4 is what makes the lock release *clean* (no lingering `-wal`/`-shm` across the swap).

### Does the C++ 5s timeout need raising?

**No.** The self-exit budget is < ~4.0s. The existing `WaitForSingleObject(hProcess, 5000)` is left as-is and will now hit its `WAIT_OBJECT_0` (graceful) branch instead of timing out.

### C++ side change (minimal, optional)

The only C++ touch is **verification + logging**, not behavior:
- In `StopWalletServer` (`cef_browser_shell.cpp:3512-3521`), the `shutdownSent && WaitForSingleObject == WAIT_OBJECT_0` branch already logs "exited gracefully" and skips `TerminateProcess` — this is already correct. No code change required; optionally add a clearer log line distinguishing "wallet self-exited (clean)" from the timeout fallback for release diagnostics.
- **Confirmed:** `SendShutdownRequest` returns `success = (WinHttpReceiveResponse(...) == TRUE)` (`cef_browser_shell.cpp:3357-3364`). Its success predicate is "the 200 response was received," NOT merely "POST sent." Because our `exit(0)` fires *after* the 200 is flushed (steps 0 → 6), the C++ read of the 200 completes first, so `shutdownSent == true` and the clean `WAIT_OBJECT_0` path is taken deterministically. The thin-handler design (§3) is what guarantees this.

### Job-Object-kill interaction (benign)

Both wallet and adblock children are bound to a Job Object with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`. `StopWalletServer` closes the job handle (`cef_browser_shell.cpp:3534-3537`) *after* the process is already gone via self-exit. The three kill mechanisms (self-`exit(0)`, `TerminateProcess`, job-close kill) are mutually exclusive once the process is dead — by the time any later signal could land, the PID is gone and its file handles / image lock are already released. **No double-kill corruption.** The double-signal is harmless.

### The second C++ kill path (confirm or document as dead)

There is a *separate* hard-kill-only path: `WalletService::stopDaemon()` → `cleanupDaemonProcess()` (`WalletService.cpp:769-782`) calls `TerminateProcess(daemonProcess_, 0)` directly with **no `/shutdown` POST first**. `exit(0)` does nothing for this path. **Spec action:** confirm in production-shutdown ordering whether `stopDaemon`/`cleanupDaemonProcess` ever runs against the wallet PID alongside `StopWalletServer` (the parallel-thread `StopWalletServer` at `cef_browser_shell.cpp:543` appears to be the live one). If `stopDaemon` is live, route it through `/shutdown` too so it also benefits from the clean exit; if it is dead/legacy, document it as unused. `exit(0)` only helps paths that POST `/shutdown` first.

---

## 6. Verification / test matrix

For every case: after the exit, **re-open the DB** and assert (i) it opens clean with no WAL replay error, (ii) startup recovery (`main.rs:391-553`) runs to completion, (iii) balance reconciles. Run on **both Windows and macOS** per Testing Standards.

| # | Case | Action | Pass criteria |
|---|------|--------|---------------|
| 1 | **Clean close, no in-flight work** | Idle wallet → POST `/shutdown`. | Process exits 0 in < 5s (target < 4s); `-wal`/`-shm` truncated or absent after exit; C++ logs the `WAIT_OBJECT_0` graceful branch (NOT `TerminateProcess`); final "clean exit" log line present; next open clean, recovery runs, balance unchanged. |
| 2 | **Close mid user-`createAction`** | Start a `createAction` (holds `create_action_lock`, select→sign→BEEF→broadcast), POST `/shutdown` mid-flight. | `server.stop(true)` drains the in-flight request to completion before exit; the createAction either completed and committed or never wrote; next open: NO orphan `transactions` row without inputs/outputs, balance reconciles. (Proves exit fires strictly after HTTP drain — the protection createAction has today is preserved.) |
| 3 | **Close mid Monitor backup-broadcast** | Seed a dirty backup so `TaskBackup` is broadcasting, POST `/shutdown`. | Process exits 0 in < 5s (the 3s join timeout fires; backup is abandoned — NOT waited on); next open: backup state is recomputed idempotently (hash-compare) on next Monitor run; no DB corruption; documented as tolerated/recoverable. |
| 4 | **Close mid Monitor DB-write** | Seed a `sending` tx so `TaskSendWaiting` runs its cleanup; POST `/shutdown` while the multi-statement cleanup is executing. | With transactional wrap: the cleanup either fully committed or fully rolled back — never tx=failed-with-inputs-not-restored. Without the wrap (if owner defers it): startup recovery + `TaskUnFail`/`TaskReviewStatus` heal the half-applied state on next boot. Either way: next open consistent, no orphaned reserved outputs, balance reconciles. The between-task `is_cancelled()` check breaks the loop after the current task; join completes within 3s. |
| 5 | **`kill -9` equivalent (no regression vs today)** | `TerminateProcess`-equivalent hard kill (or simulate the C++ 5s-timeout fallback) at a random point. | Identical outcome to today's status quo: WAL file-level safe, startup recovery reconciles, no NEW corruption path introduced by any of the §3 changes. (Proves we did not regress the unclean-kill path.) |
| 6 | **Repeated / double shutdown signals** | POST `/shutdown` twice rapidly; also let the C++ Job-Object close fire after self-exit. | First signal drives the exit; the coordinator is idempotent (token already cancelled, `server.stop` already in progress); no panic, no double-exit hazard; process exits 0 exactly once. Job-close after exit is a no-op. |
| 7 | **Dev-run wallet (no C++ parent)** | Launch via `.\dev-wallet.ps1` (no Job Object, no C++ killer), POST `/shutdown`. | Process exits 0 cleanly (improvement over today where a stray `/shutdown` drains-but-lingers). Confirm NO integration test relies on the process surviving a `/shutdown`. Hits the dev data dir (`HodosBrowserDev/`), not production. |
| 8 | **WAL-lock-release for updater** | After case 1's exit, assert the `-wal` file is truncated/absent and the exe image area is releasable. | `-wal` truncated/absent; a `CreateFile`-with-no-sharing (or PID-exit wait) on the exe succeeds, proving the updater can swap `hodos-wallet.exe`. |
| 9 | **`synchronous` live value** | After startup, query `PRAGMA synchronous`. | Returns `1` (NORMAL), asserting the explicit pragma took effect. |
| 10 | **Logger final-line guarantee** | Any clean exit. | The final "clean exit" line is present in `wallet_rCURRENT.log` (proves `flush()` before `exit(0)`); a killed run (case 5) lacks it, so the boot log can distinguish clean-exit from kill. |

**Integration test (key):** kill the wallet via the new exit path mid-`createAction` (case 2) and mid-Monitor-cleanup (case 4) and assert next-open recovery yields a consistent DB. Plus a smoke assertion that after `/shutdown` the `-wal` is truncated/absent (case 8) — proving checkpoint + lock-release works for the updater.

---

## 7. Risk verdict

### Is this strictly safer than status-quo `TerminateProcess`?

**Yes — strictly safer, conditional on the §2 sequencing.** The argument:
- **File-level durability:** EQUIVALENT to status quo. `exit(0)` is a clean userspace exit; already-written WAL frames survive, committed transactions are recoverable. Both `exit(0)` and `TerminateProcess` skip `Drop` and leave the same WAL state.
- **In-flight HTTP (createAction):** STRICTLY BETTER. `exit(0)` lands *after* `server.stop(true)` drains, whereas `TerminateProcess` can land *during* an in-flight rusqlite `write()` syscall.
- **Monitor multi-statement ops:** EQUIVALENT-or-BETTER. With the JoinHandle quiesce (+ between-task cancellation check), no task is mid-sequence at exit → strictly better than the status quo tear. With the transactional wraps, even a forced fallback kill rolls back atomically → better still.
- **Updater image-lock release:** STRICTLY BETTER. Deterministic, fast, WAL-truncated clean release vs. an unpredictable hard-kill timing.
- **Secrets:** NEUTRAL. Identical to status quo (no regression, no improvement).

### Residual risks

1. **Monitor join times out (3s) with a genuinely slow non-backup task.** Bounded by design — after the timeout we exit anyway, falling back to the status-quo tear (recoverable by startup recovery). Worst case = today's behavior, never worse.
2. **Transactional wraps not landed (owner defers them).** The `createAction` / cleanup torn-write window remains *exactly as it is today* — recovery-task-healed, not corruption. Acceptable interim; the exit change is still ≥ status quo.
3. **WAL checkpoint blocked by a held connection.** Mitigated by short-timeout + proceed-on-failure; we never block exit on it. Worst case: `-wal` persists to the next open, which replays it cleanly (status-quo behavior).
4. **`stopDaemon` second kill path** (if live) bypasses `/shutdown` and won't get the clean exit. Mitigated by the §5 confirm-or-route action.
5. **Secret hygiene unchanged** — pre-existing, out of scope, separate task.

### Go / no-go recommendation

**GO**, with two gates:
1. **Land the exit-sequencing core** (handler stays thin; `main.rs` coordinator; `Monitor::start` returns `JoinHandle`; between-task cancellation check; WAL-checkpoint-at-exit proceed-on-failure; explicit logger flush; updater PID-exit/lock-wait belt-and-suspenders) — this is ≥ status quo on every axis and clears the OD-2 goals.
2. **Get owner sign-off** for the durability-pragma (`synchronous=NORMAL` made explicit) and the four money-DB transactional wraps. These are independently-correct and make the deterministic-exit guarantee rest on atomic writes; they may be sequenced as a separate reviewed commit if the owner prefers, but are strongly recommended to land together.

Confirm the live C++ kill path (`StopWalletServer` vs `stopDaemon`) before merge, and verify the full matrix on both Windows and macOS.
