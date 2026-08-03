# C4 / C5 Wiring Plan — spent-input reconcile into the money path

> **Status:** PLAN for owner review (2026-07-12). No code written for c4/c5 yet.
> The reconcile primitive (`reconcile_spent_inputs`) is built, adversarially reviewed
> (3 rounds), compiles, 31 unit tests green — but **unwired** (`#[allow(dead_code)]`).
> c4/c5 connect it to live money paths → **owner sign-off + live smoke required**.
> Parent design: [`RECONCILE_PHASE2_DESIGN.md`](./RECONCILE_PHASE2_DESIGN.md) §2a/§2b/§2c.

All file:line references verified current on branch `0.4.0` (2026-07-12).

---

## P0 — Prerequisite refactor (zero-risk, do first)

`reconcile_spent_inputs`, `promote_proven_local_tx`, and `recover_transaction` currently
take `&web::Data<AppState>`. Two of the three wiring callers take `&AppState`:

| Caller | Its state param |
|---|---|
| `create_action_internal` (c4) | `web::Data<AppState>` (owned) |
| certificate handler `_core` (c5a) | `&AppState` (`certificate_handlers.rs:2663`) |
| `do_onchain_backup` (c5b) | `&AppState` (`handlers.rs:12757`) |
| `TaskUnFail` (existing recover_transaction callers) | `&web::Data<AppState>` |

**Change the three fns to take `&AppState`.** Deref coercion then satisfies **all**
callers with no ripple: `create_action` passes `&state` (`&web::Data` → `&AppState`),
cert/backup pass `state` directly, TaskUnFail passes `state` (`&web::Data` → `&AppState`).
Bodies are unchanged (`state.database`/`state.balance_cache`/`state.services` all resolve
either way). Rebuild + rerun the 31 reconcile tests to confirm.

---

## c4 — `create_action` "Missing inputs" (THE critical fix)

**Site:** broadcast-failure branch, `handlers.rs:6340–6420`, inside `create_action_internal`
(`handlers.rs:4511`), which holds `create_action_lock` for the entire handler (`:4523`).
`selected_utxos` (the reserved wallet inputs) is in scope; each has `.txid`/`.vout`.

**Current flow (`:6341–6419`):** delete ghost change (`disable_by_txid`, `:6346–6364`) →
double-spend check + suspected-mark (`:6370–6380`) → restore inputs
(`restore_spent_by_txid` / `restore_by_spending_description`, `:6385–6398`) → commission
cleanup (`:6400–6408`) → `balance_cache.invalidate()` (`:6411`) → return
`ERR_BROADCAST_FAILED` (`:6416`).

**Change (design §2a):**

1. Delete ghost change — **unchanged**.
2. Restore selected inputs to `spendable=1` — **unchanged** (`:6385–6398`). Required so
   reconcile's `mark_spent ... WHERE spendable=1` is satisfiable and the *good*
   (non-phantom) inputs become re-selectable on retry.
3. **Drop the `db` lock**, then `reconcile_spent_inputs(&state, &selected_outpoints).await`
   where `selected_outpoints = selected_utxos.iter().map(|u| (u.txid.clone(), u.vout))`.
   Reconcile does its own locking + network reads — the `db` guard **must not** be held
   across this `.await` (restructure the current single `db` scope into: [reserve/restore
   burst] → drop → [await reconcile] → [re-acquire for commission cleanup]).
4. Commission cleanup — **unchanged** (`:6400–6408`), runs on every branch.
5. **If `report.changed()` → return new `ERR_RECONCILED_RETRY`.** Else → today's
   `ERR_BROADCAST_FAILED`.

**Gate:** only reconcile when the broadcast error is the phantom signature —
`e.to_string().to_lowercase().contains("missing inputs")` **AND**
`!crate::arc_status::is_double_spend_error(&e.to_string())`. Every other fatal error
(BEEF-460, script-verify, real double-spend) keeps today's behavior untouched
(guardrail #11 — preserve non-phantom paths).

**Lock note (design §3):** `create_action_lock` is held across reconcile's multi-second
network reads. Accepted on a single-user wallet; reconcile's per-provider timeouts bound
the stall. The retry is a **fresh top-level `create_action` call** (frontend), so the lock
is released on return and re-acquired on retry → exactly **one reconcile per invocation**
(guardrail #9 — no in-handler rebuild, D-I2).

---

## Frontend — one-retry hook (the only non-Rust change)

On `ERR_RECONCILED_RETRY`, the send flow **auto-retries `create_action` once from the top**
(lock-free; the phantom is now `spendable=0`/excluded, recovered change is `spendable=1`).
Show an inline "Updating your balance…" status + a short success toast, non-blocking.
**Guard: retry at most once** (a second `ERR_RECONCILED_RETRY` surfaces the error — no loop).
Mirrors the cert path's existing "please retry" contract (design §2a / Decision C).

---

## c5a — certificate acquire (replace the fail-open retry)

**Site:** `certificate_handlers.rs:3013–3069` (state `&AppState`). `selected_utxos` in scope.

**Current (broken):** on "Missing inputs", probes WoC `/tx/{txid}/outspend/{vout}` — the
**wrong** endpoint — treats **404 → spent (fail-OPEN)** (`:3029–3032`), `mark_spent(txid,
vout, "unknown")` (`:3056`), returns a "please retry" error.

**Change (design §2b):** replace the whole `:3016–3068` block with
`reconcile_spent_inputs(state, &selected_outpoints).await`, then return the **same
retryable error** (the already-built cert tx can't be re-broadcast — its input is spent; it
needs a fresh top-level rebuild, which the `createAction`-based caller already tolerates via
the `broadcast_status=='failed'` check). Net: removes the fail-open, the wrong `/outspend/`
endpoint, and the `spent_by="unknown"` poison; gains cross-validated marking + change recovery.

---

## c5b — `do_onchain_backup` (subsumes FIX_A)

**Site:** `handlers.rs:13114–13180` (state `&AppState`). `funding_utxos` selected at
`:13114–13156`; reserved (placeholder + previous backup inputs) at `:13163–13179`.

**Change (design §2c):** between selection (`:13156`) and reservation (`:13163`):

1. **Acquire `utxo_selection_lock`** around the reconcile + re-select (backup holds
   **neither** lock today = BS-C1; without this, reconcile-in-backup races a concurrent
   send — D-R3).
2. `reconcile_spent_inputs(state, &funding_outpoints).await` with **only `funding_utxos`** —
   **never** `previous_pushdrop` / `previous_marker` (those are separately reserved backup
   inputs — D-I6).
3. **Re-run the funding selection once** (recovered change + newly-marked phantoms now
   reflected), then continue to reservation.

Inputs are still `spendable=1` here (reservation is later), so no D-I1 issue.

---

## Test plan

- **Unit:** existing 31 + gate-matching (missing-inputs-and-not-double-spend) + the
  `report.changed() → ERR_RECONCILED_RETRY` branch.
- **Integration (mocked WoC):** create_action diverged fixture → restore → reconcile →
  `ERR_RECONCILED_RETRY` → frontend retry succeeds; healthy wallet → no-op;
  provider-disagreement → no-op; mark **sticks** across a `TaskReviewStatus` /
  `restore_pending_placeholders` pass (loop-free terminal); idempotent across repeated sends.
- **Manual smoke (dev):** the diverged dev wallet `7c4423f4 → ef67fd9e @956718` (design §2d)
  — reconcile logs, retry funds from recovered change, relaunch no-op. **P7 live-probe**:
  confirm WoC `/address/{addr}/unspent/all` returns `ef67fd9e`'s change as a live UTXO.
- **Parity:** `cargo test` + smoke on Windows and seeded macOS (portable Rust).

## Sequencing (each behind owner sign-off)

1. **P0 refactor** (&AppState) → rebuild + retest.
2. **c4 + frontend hook** — the critical path. Smoke on the diverged dev wallet before c5.
3. **c5a (cert)** and **c5b (backup)** after c4 proves out.

---

## c5b — backup path (DESIGN v2, expanded after live smoke found the backup-dust bug)

**Live-smoke finding (2026-07-13):** on the diverged wallet, the send-path reconcile healed the
funding phantom `7c4423f4:2`, but the **old backup token + marker** `7c4423f4:0/:1` (1,546 sats,
`1-wallet-backup` basket) stayed `spendable=1` in the DB while spent on-chain. Root cause verified:
`adopt_onchain_backup` (`handlers.rs:12711`) only swaps the **in-memory** `previous_pushdrop`/
`previous_marker` pointers — it never marks the superseded DB backup outputs spent. And the send-path
reconcile **deliberately skips** the `1-wallet-backup` prefix (D4 anchor protection), so nothing
cleans them. The affected user has the same class. → c5b must clean stale backup phantoms too.

### Part 1 — Backup-token phantom sweep (fixes the reported bug; testable now)
Insert a **Step 1.5** in `do_onchain_backup`, **before** the hash-change early-return (`:12841`) so it
fires on every backup cycle a stale predecessor exists (not gated on a wallet state change):
1. Gather all spendable `1-wallet-backup` outpoints (`get_spendable_by_derivation("1-wallet-backup","1")`
   for tokens + `"marker"` for markers).
2. **Gate (cheap, local): only act if `> 2` outpoints** — more than one token+marker pair, i.e. a
   superseded predecessor is still tracked alongside the current backup. A **healthy wallet has one
   pair (len 2) → skip entirely, no network** (this is the key property: it does NOT run in steady
   state). When it does run, under `utxo_selection_lock`, `reconcile_spent_inputs(state, &backup_outpoints)`.
   Reconcile no-ops on the unspent current token and marks only the genuinely-spent predecessor.
   **Self-limiting:** once the stale pair is marked spent, len drops to 2 and the gate closes.
3. **Single-stale-pair (`len==2`) note:** if a wallet's *only* backup pair is itself stale, this gate
   skips it — but it's cleaned on the wallet's next backup (which adds a new pair → len 4 → this fires).
   Adversarial review confirmed the residual is otherwise cosmetic: the stale marker/token are never
   send-selectable (`get_spendable_by_user` restricts to the `default` basket) nor counted in the
   displayed balance (`calculate_balance` excludes the `1-wallet-backup` prefix). Chosen over an
   always-on sweep so a healthy wallet never does background probes.

**Why this is safe re: "identify the newest token first" (owner ask):** `reconcile_spent_inputs`
only mutates on an **explicit on-chain `Spent`**. The current (newest) backup token is **unspent**
on-chain → `check_outpoint_spent → Unspent → no-op` (single probe, no mark, no follow). Only the
stale (spent) predecessors are marked. So the newest token is protected *by the chain, per-token* —
a stronger guarantee than a heuristic "pick the newest." The PushDrop token (non-P2PKH) and the
`-3` marker are never wrongly "recovered" as change (`recover_change_index` skips both — D4).

### Part 2 — Funding reconcile on backup broadcast failure (the §2c FIX_A intent)
At the backup broadcast-failure arm (`:13475`), after `rollback_backup` restores the reserved inputs,
`reconcile_missing_inputs(state, &e, funding_outpoints)` (same gate/helper as the 4 send sites).
No retry code needed — TaskBackup re-runs on its schedule and the next cycle selects real coins
(exactly how backup #2 succeeded after the send healed the phantom in the live smoke).

### Trigger on THIS wallet (owner ask)
Part 1 runs pre-hash-check whenever `> 2` backup outpoints are tracked → on the dev copy (4 backup
outpoints: `7c4423f4:0/:1` stale + `8012239517:0/:1` current) it fires on the **next TaskBackup tick
or any `/wallet/backup/onchain` call** and marks `7c4423f4:0/:1` spent — the built-in pass/fail
fixture. (D-I6 relaxation: token/marker ARE included in reconcile candidates now — safe because
reconcile no-ops on the unspent current one and never recovers non-P2PKH/`-3` outputs.)

## Open questions for owner

1. **Frontend retry UX** — auto-retry once + "Updating your balance…" status/toast
   (Decision C), or a manual "retry" button? (Plan assumes auto-once.)
2. **c5a scope** — owner previously flagged concern about touching cert code. Confirm go
   for c5a, and whether to *also* fix the dormant fail-open `outspend()` provider
   (`whatsonchain.rs:149`, wrong `/outspend/` 404→spent) or just replace the retry block
   here. (Plan assumes: replace the retry block only; note the dormant one for a follow-up.)
3. **Backup lock** — OK to add `utxo_selection_lock` acquisition to the backup path (new
   for backup)? Required for D-R3 safety.
4. **create_action_lock held during reconcile network** — accept the multi-second stall on
   a single-user wallet (Plan's assumption), or add an explicit reconcile time budget?
