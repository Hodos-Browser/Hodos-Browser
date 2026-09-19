# A transaction row can sit at status `created` while its coin is on chain -- and nothing reconciles it

**Found:** 2026-09-19, by code reading, while answering the owner's question *"do we need to make sure
the payment stopped?"* after the macOS side measured that the wallet keeps running a `createAction`
after the client hangs up. **Status:** OPEN -- reasoned, **not measured**.
**Severity:** LOW. No money is lost, no money is missing, and the row is VISIBLE to the user. It is a
mislabelled row, not a leak.

> This ticket exists because the same *shape* just cost us a scare:
> `TICKET_reservation_can_be_held_indefinitely.md` was a money-path row in a state no code expects,
> which 27 hours later was mistaken for a live defect. This is another one. Writing it down is the
> cheap half.

---

## What prompted it, and what the answer turned out to be

The owner asked whether a payment needs to be *stoppable* when the page goes away. Short answer: **no,
and adding that would be worse.** There is no safe window -- an abort landing after broadcast but
before the DB write would give money moved with no record, which is working-rule 7's trip-wire 1. The
property that actually matters is *no spend without a visible record*, and that property **holds**:

| step in `create_action_internal` | when |
|---|---|
| `transactions` row written, `status = created` | **before** signing |
| reservations resolved placeholder -> real txid (`update_spending_description_batch`) | **before** broadcast |
| `broadcast_transaction` | -- |
| `update_broadcast_status(txid, "broadcast")` | after |

All of it runs in Rust and none of it depends on a client listening. And `/wallet/activity`
(`handlers.rs :: wallet_activity`) is `SELECT txid, satoshis, is_outgoing, status, created_at, ...
FROM transactions ORDER BY created_at DESC` -- **no status filter at all**, so every row shows.

## The gap

If the process dies between a successful broadcast and `update_broadcast_status`, the row is left at
`status = created` while the coin is on chain. **Nothing reconciles that:**

| task | filter | matches `created`? |
|---|---|---|
| `TaskFailAbandoned` | `status IN ('unprocessed','unsigned')` | no |
| `TaskSendWaiting` | `status = 'sending'` | no |

`create_action_internal` never sets `'sending'` -- the only `conn_update_status(..., "sending")` is on
the separate broadcast/retry endpoint (`handlers.rs :~20098`). So the row falls between both nets.

### The non-coverage is PROTECTIVE, and that is the important part

It would be easy to "fix" this by widening `TaskFailAbandoned` to include `created`. **Do not.** That
task's cleanup is *mark failed -> delete ghost outputs -> restore inputs*, and the inputs of a
broadcast transaction are genuinely spent. Restoring them hands spent coins back to the selector --
the `P0.7` double-spend path, reintroduced. The status partition is exactly what prevents it.

Any fix here must therefore prove *on chain* that the transaction exists before touching the row, the
same conservative rule `TaskSweepReservations` follows. `reconcile::check_outpoint_spent` and the ARC
lookup `TaskSendWaiting` already uses are the existing primitives.

## How reachable is it, honestly

Narrower than it first looks, and the macOS measurement is why:

- A **client disconnect does NOT** trigger it. macOS measured (round h) that after `curl -m 0.3` the
  handler still reached coin selection ~16.5 s later and actix logged the request complete. Actix-web
  is not cancelling these handlers, so the page going away leaves the sequence intact.
- The realistic trigger is **process death** -- crash, power loss, or our own `stop-dev.ps1` -- landing
  in the window between broadcast returning success and the status write. That window is small (one
  DB lock + one UPDATE) but it is real, and a wallet is exactly the program where "small window,
  survives forever in the DB" is the combination that matters.

## What the user sees today

The spend **is** in the activity feed, because there is no status filter -- so this is not a hidden
payment. It is labelled with a status that says it was never broadcast, while it was.
Worth weighing against P10, which found this same feed announcing **rejected payments as received**:
status labelling here has form.

## Suggested shape (not implemented)

1. A reconciler for `status = 'created'` rows older than some age that are **positively observed on
   chain** -> promote to the broadcast/unproven status the ARC ladder expects.
2. Or, cheaper and arguably better: set `'sending'` immediately **before** `broadcast_transaction` in
   `create_action_internal`, which hands the whole case to `TaskSendWaiting`, a reconciler that
   already checks ARC before re-broadcasting and already has the failure cleanup. Prefer this if the
   status transitions allow it -- it reuses a net rather than adding one.
3. **Negative control:** whatever is built must be shown to leave a genuinely-unbroadcast `created`
   row alone, and must never restore inputs for a transaction observed on chain. A fix that acts on
   age alone, without on-chain proof, reintroduces `P0.7` and must fail this test.

## Not verified

- No dropped-future run was performed. The window is inferred from the write order, not observed.
- Whether any status between `created` and broadcast is set by a path not read here.
- The `ActionStatus::Created` -> `TransactionStatus` mapping in `action_storage.rs :: from_legacy`
  was not traced; the feed shows the raw `status` column, but what the UI *renders* for `created` was
  not checked.

## Cross-references

- `TICKET_reservation_can_be_held_indefinitely.md` -- same shape, now fixed; its sweeper is the model
  for "positive on-chain evidence or do nothing".
- `phase-0.7-utxo-reservation-leak/` -- why restoring inputs without proof is the thing to fear.
- `rust-wallet/src/handlers.rs :: create_action_internal`, `:: wallet_activity`;
  `rust-wallet/src/monitor/task_send_waiting.rs`, `task_fail_abandoned.rs`.
