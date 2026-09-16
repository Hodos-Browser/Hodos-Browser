# Phase 8b — do not broadcast a spend you failed to record · PHASE CONTRACT

**Workstream:** money-path correctness · **Ticket:** `TICKET_placeholder_resolution_failure_broadcasts_anyway.md`
**Status:** ✅ **IMPLEMENTED.** T1 green; ⬜ the live `A1` row is **owed** — see §4.
**Opened:** 2026-09-08 · **Owner:** Matthew Archbold · **Platforms:** Windows (Rust — platform-neutral)
**Standard:** `../HARNESS.md`. **Base:** `c68ed57`.

---

## 0. Plan-vs-tree delta

### 0.1 ✅ `D-1` — the six call sites are exactly as described

3 × `warn!` + 3 × `let _ =`, in the three files the ticket names. Nothing moved.
⭐ Independently corroborated: `output_repo.rs :: list_stale_pending_reservations`'s doc comment,
written during Phase 0.7 by someone reasoning about the same failure, describes it identically.

### 0.2 🚨 `D-2` — **the ticket's central premise is FALSE for one of the six sites**

> §2: *"At every one of these sites the transaction is signed but **not yet sent**. Aborting costs
> nothing — no money has moved."*

**Measured — in `do_onchain_backup` the broadcast comes first:**

| | Line | |
|---|---|---|
| `broadcast_transaction(...)` | **14149** | ⬅ money moves here |
| `update_spending_description_batch(...)` | **14263** | ⬅ ~110 lines later |

Both are inside `do_onchain_backup` (`:13421`) in one linear flow — not alternate branches.

⇒ **That site cannot abort.** By the time the failure is visible the transaction is already on the
network. Aborting is not a weaker option there; it is an impossible one.

**Ordering verified at all six** rather than assumed:

| Site | Resolution | Broadcast | Abortable? |
|---|---|---|---|
| `create_action_internal` (txid-changed) | 6373 | 6399 | ✅ |
| `create_action_internal` (txid-unchanged) | 6384 | 6399 | ✅ |
| `sign_action` | 8533 | 8594 | ✅ |
| `certificate_handlers` (publish/unpublish) | 4788 | 4804 | ✅ |
| `task_consolidate_dust` | 361 | 389 | ✅ |
| **`do_onchain_backup`** | **14263** | **14149** | ⛔ **NO** |

### 0.3 ⭐ `D-3` — the suggested retry is already there, so it is not built

§5.4: *"Consider whether a retry… is warranted, since `SQLITE_BUSY` is transient by nature."*

`connection.rs:72` already sets **`busy_timeout(5s)`**, so SQLite retries a busy database internally
for five seconds before returning an error. An explicit application-level retry would re-solve a
solved problem. ⛔ Not added (working rule 2).

### 0.4 ⚠️ `D-4` — a failure mode the ticket does not mention: **`Ok(0)`**

`update_spending_description_batch` returns `Ok(rows_affected)`. A resolution that matches **no
rows** is `Ok(0)` — **not an error** — so the new abort branch does not fire and the wallet
broadcasts having recorded nothing. The bookkeeping harm is identical to the `Err` case.

⭐ **Chased, and it is not a live double-spend path.** The obvious fear was: `TaskSweepReservations`
releases a `pending-%` reservation after 15 min, a slow `createAction` → `signAction` then resolves
0 rows and broadcasts, leaving inputs marked spendable but actually spent. **Already guarded** —
`task_sweep_reservations.rs:56-62` drops any candidate whose placeholder is held by a live
`PENDING_TRANSACTIONS` entry, *"however old the reservation looks."*

⇒ `Ok(0)` remains legitimate when a transaction spends only external inputs never in our table.
Distinguishing "resolved 0 because nothing was reserved" from "resolved 0 because something went
wrong" needs the **reserved count** threaded from `mark_multiple_spent` to the resolution site,
which is not retained today. ⛔ **Out of scope** — recorded in §8, documented in a test.

## 1. Goal

If the wallet cannot record which coins a transaction spends, it does not broadcast that
transaction.

## 2. Done means

- [x] All five **abortable** sites refuse to broadcast on a resolution failure and return an error.
- [x] The reservation is **left standing** on abort — not hand-released. Phase 0.7's sweeper owns
      release, and it verifies the outpoint on-chain first.
- [x] The **sixth** site (`do_onchain_backup`), which cannot abort, logs at `error` with the
      placeholder and txid so the row is findable, and says in-place why it differs.
- [x] One shared helper (`resolution_failed_response`) carries the rule and its reasoning.
- [x] `Ok(0)` is documented as a known, unguarded gap rather than silently ignored.
- [ ] ⬜ **`A1` (live) owed** — see §4.

## 3. Invariants preserved

| ID | Invariant | Why this phase could break it |
|---|---|---|
| `R-DUST` | No incidental path spends a 1-sat output | Touches `create_action` + the dust consolidator, both `R-DUST` subjects. ✅ 19 floor tests still green |
| `R-INTEXT` | Internal never prompts, external always gates | Adds an early return **inside** `create_action`/`sign_action`. It is downstream of the gate — the gate has already run — so it changes the *response*, never the *decision* |
| `R-GOLD` | Gold pill on auto-approved payment | ⚠️ **Real interaction.** The pill fires from C++ `OnWalletCallSuccess` on a **successful** wallet call. An abort returns 500, so no pill — which is correct: no payment happened |
| `R-NODOUBLE` | (Phase 0.7) reservations never wrongly released | ⭐ The abort deliberately does **not** release. Hand-releasing here would re-open exactly what 0.7 closed |

⛔ No schema change, no crypto change. CLAUDE.md #2/#3 not engaged.

## 4. Evidence table

| ID | 🟢 GREEN | 🔴 RED — must be *seen* to fail | 🎯 SUBJECT | Tier | Result |
|---|---|---|---|---|---|
| `P8b-A1` | With the resolution write forced to fail, the send returns an error and **nothing reaches the network** | Pre-fix binary, same forced failure ⇒ the transaction **is** broadcast | ⛔ **WhatsOnChain for the txid** — *did it reach the network* — not the HTTP response | T2 | ⬜ **OWED — scheduled as `M4` in `../PAYMENT_TEST_BATCH.md`.** ⭐ Its GREEN half costs nothing (the assertion IS "nothing was broadcast"); only the RED half spends. Not run; not claimed |
| `P8b-A2` | A write failure surfaces as `Err`, so the abort branch is **reachable** | If this ever returns `Ok`, every abort added here is dead code — the test says so | The repo call itself | T1 | 🟢 `a_write_failure_surfaces_as_err_so_the_abort_branch_can_fire` |
| `P8b-A3` | The happy path resolves every reserved input and sets its spending txid | — (regression half of `A2`) | `outputs.spending_description` after the call | T1 | 🟢 `resolution_succeeds_and_reports_the_row_count` |
| `P8b-A4` | A resolution claims **only** its own placeholder | A concurrent transaction's reservation must be untouched | The other placeholder's row | T1 | 🟢 `resolution_claims_only_its_own_placeholder` |
| `P8b-A5` | `Ok(0)` is documented as unguarded, with the reason | — | The test asserts `Ok(0)`, not `Err` | T1 | 🟢 `a_resolution_matching_no_rows_is_ok_zero_not_an_error` |
| `P8b-A6` | Full suite green | 🚨 `cargo build --release` skips `cfg(test)` — it is not evidence | `cargo test` | T0 | 🟢 **462 lib + 530 bin + 16 integration, 0 failed** |

⛔ **`A1` is the row with teeth and it is not run.** T1 proves the branch is reachable and correct in
isolation; it does **not** prove "nothing was broadcast". Saying otherwise would be the code-reading-
as-measurement mistake this sprint keeps catching. The ticket's own §6 says the negative control must
show the **pre-fix binary broadcasting anyway** — that needs a fault-injection seam plus a live run,
and is owed.

## 5. Blast radius

`handlers.rs` (`create_action_internal` ×2, `sign_action`, `do_onchain_backup`) ·
`handlers/certificate_handlers.rs` (publish/unpublish) · `monitor/task_consolidate_dust.rs` ·
`database/output_repo.rs` (tests only).

⚠️ **Behaviour change, deliberate:** a class of send that previously *succeeded* (broadcast, badly
recorded) now *fails* (nothing broadcast, error returned). That is the point of the ticket. It is
also why `A1` matters: the new failure path has never been exercised live.

## 6. Out of scope

`Ok(0)` count-checking (§8) · the reservation-ownership refactor
(`../TICKET_reservation_ownership_converge_on_spent_by.md`) · the BRC-121 paid-retry audit the ticket
§5.3 asks for — ⚠️ **not done**, see §8.

## 7. Rollback

One `git revert`. No schema, no migration, no persisted state.

## 8. Open, and honestly owed

| # | Item |
|---|---|
| 1 | ⬜ **`A1` live run + fault-injection seam** — now scheduled as `M4` in `../PAYMENT_TEST_BATCH.md`, with the seam design (gate on `HODOS_DEV`, which `enforce_dev_safeguard` already guarantees a production binary cannot see). |
| 2 | ⚠️ **BRC-121 paid-retry audit not done.** §5.3 asks whether an abort can strand a paid retry or double-mint a payment. The five abort sites are all pre-broadcast, so no payment has been *made* at the abort point — but `pay_402` / `broadcast_nosend` were **not** traced. Code reading, not a measurement, and it is the one place an abort could plausibly cost money. |
| 3 | ⚠️ **`Ok(0)` gap** (`D-4`) — needs the reserved count threaded through. |
| 4 | ⚠️ **`do_onchain_backup` still has the original defect**, by necessity (`D-2`). Fixing it means moving the resolution before the broadcast, which is a restructure of that function and its rollback path — a separate ticket if it is worth doing. |
