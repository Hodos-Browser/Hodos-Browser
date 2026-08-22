# UTXO reservations should be owned by a transaction row, not by a placeholder string

**Filed 2026-08-22**, out of Phase 0.7 (beta.3) after reading how `wallet-toolbox` handles the same
conflict.

**⛔ Do NOT do this during beta.3.** It reorders the money path. It is filed to be picked up
deliberately, after the sprint, with its own phase and test plan.

**Type:** structural refactor. **Value:** removes a class of bug rather than an instance of one.

## 1. The problem in one line

Our UTXO reservation is a **string with no owner**, so nothing in the database can answer "which
transaction is holding this coin, and how far did it get?"

## 2. Current state

`create_action_internal` reserves its selected UTXOs by writing a marker into the
`outputs.spending_description` column:

```rust
let placeholder_txid = new_reservation_placeholder();   // "pending-{ts}-{seq}"
output_repo.mark_multiple_spent(&utxos_to_reserve, &placeholder_txid)
```

`spending_description` is otherwise a human-readable description of an input. We overloaded it as a
reservation key. The `outputs` table **already has** `spent_by`, a real FK to `transactions(id)` —
but it is only populated later, when the placeholder resolves to a txid.

Consequences we are living with:

1. **The sweeper must ask the network.** `monitor/task_sweep_reservations` cannot tell a stranded
   reservation from a broadcast one locally, so it fetches the wallet's on-chain unspent set on
   every pass. If WhatsOnChain is unreachable, it releases nothing and stranded coins stay stranded.
2. **External inputs can never be released.** A user-supplied basket input on an address we do not
   own can't be verified against our own unspent set, so it is withheld forever.
3. **Two mechanisms that can disagree.** Transaction `status` and the placeholder string are
   parallel lifecycles. The Phase 0.7 double-spend window existed in the gap between them.
4. **A schema trap.** Anyone reading `spending_description` reasonably assumes it means what it says.

## 3. How wallet-toolbox does it

Read from `bsv-blockchain/wallet-toolbox` @ `master`, 2026-08-22:

- **Reservation is the FK, from the first moment.** `src/storage/methods/createAction.ts` creates the
  transaction row first — `status: 'unsigned'`, `txid: undefined` — then marks each input
  `{ spendable: false, spentBy: ctx.transactionId }`, inside `storage.transaction(...)` so the whole
  allocation is atomic. `spendingDescription` keeps its literal meaning
  (`spendingDescription: i.inputDescription`).
- **Release is a consequence of status, never a sweep over reservations.**
  `StorageProvider.updateTransactionStatus('failed', id)` walks that transaction's inputs and sets
  `{ spendable: true, spentBy: undefined }` — *only* for outputs referencing that transaction. It
  also refuses to un-complete or un-fail a transaction.
- **Staleness is a transaction concern.** `monitor/tasks/TaskFailAbandoned.ts` runs every 5 minutes,
  finds transactions still in `unprocessed`/`unsigned` older than `abandonedMsecs`
  (default `1000 * 60 * 5` — **5 minutes**), and fails them, which releases their inputs.
- **The conflict check is local.** When an input is already reserved, `createAction` inspects
  `spendingTx?.txid`. Set → a real competing transaction → returns `status: 'doubleSpend'` with the
  competing txid and BEEF (`WERR_REVIEW_ACTIONS`). Unset → it never got past reservation, so it is
  safe. **This is the question our sweeper has to ask the blockchain, answered from the database.**
- **They never bulk-grant spendability.** `monitor/tasks/TaskReviewUtxos.ts` — the only task that
  re-evaluates utxos wholesale — has `trigger()` returning `run: false`: disabled, manual-trigger
  only, per identity key. Its automatic direction is marking utxos **unspendable**.

## 4. What to change

1. Create the `transactions` row **before** reserving inputs, and reserve with
   `spent_by = <transaction_id>` (plus `spendable = 0`) instead of a placeholder string.
2. Restore `spending_description` to its literal meaning.
3. Drive release off transaction status: extend the existing `set_transaction_status` /
   `TransactionStatus::Failed` path to restore that transaction's inputs, mirroring
   `updateTransactionStatus`.
4. Fold the stale-reservation sweep into the existing `task_fail_abandoned` (which already fails
   stuck `unprocessed`/`unsigned` transactions), rather than keeping a second sweeper.
5. Adopt the local conflict check — `spent_by` set **and** the owning transaction has a txid ⇒
   report a double-spend to the caller with the competing txid, instead of a generic failure. This
   is also better BRC-100 conformance: dApps expect that shape.

## 5. ⛔ What NOT to copy

**Keep the on-chain unspent check as a second gate on release.** This is a deliberate divergence.

`TaskFailAbandoned` releases inputs purely because a local status says `unsigned`/`unprocessed` and a
timer expired. If that status is ever wrong — a crash at the wrong instant, a partial write, a bug in
a transition — nothing catches it and a real spend is un-spent. Phase 0.7 found our status/marker
*was* wrong in a reachable way, so this is not hypothetical for us.

Converging gives us their local answerability; keeping the check gives us an independent oracle.
Both together is strictly better than either alone, and the cost is one bulk UTXO fetch per sweep on
a path we already use.

## 6. Ordering constraint — the actual work

The hard part is not the schema; `spent_by` already exists and **no migration is needed**. It is that
`create_action_internal` currently reserves during UTXO *selection*, well before the transaction row
is created. Converging means creating that row earlier and threading its id through selection.

That touches the most sensitive function in the wallet, on the money path, in a function that already
has 19 in-window early returns. It needs its own phase, its own contract, and its own negative
controls — which is exactly why it is not a beta.3 item.

Do `0.4.0-beta.3/TICKET_placeholder_resolution_failure_broadcasts_anyway.md` first regardless: it is
small, local, and valuable whether or not this refactor ever happens.

## 7. Invariants any implementation must preserve

| ID | Invariant |
|---|---|
| `R-NODOUBLE` | The wallet never offers an already-spent output as spendable. The reordering must not create a window where a reservation is released on local state alone. |
| `R-NORACE` | Two concurrent createActions never select the same UTXO. Today this rests on `mark_multiple_spent`'s `AND spendable = 1` under `utxo_selection_lock`; any rewrite must keep an equivalent atomic claim and be tested by asserting the **second claim takes 0 rows**, not by observing that two sends happened to pick different coins. |
| — | A guard/rollback may only ever free rows its own call reserved. |

## 8. Test plan sketch

Reuse the Phase 0.7 harness — it already covers this surface and its negative controls are known
to go red: `output_repo.rs :: stale_reservation_tests` (7) and `reservation_race_tests` (2), plus the
live A1–A6 evidence in `0.4.0-beta.3/phase-0.7-utxo-reservation-leak/PHASE_CONTRACT.md` §3a. The
refactor is done when that whole set still passes **and** its negative controls still fail.

Add: a test that a reservation whose owning transaction has a txid is never released locally, and one
that an abandoned `unsigned` transaction releases its inputs and no others.

## 9. Related

- `0.4.0-beta.3/phase-0.7-utxo-reservation-leak/PHASE_CONTRACT.md` — the guard + sweeper this would supersede in part.
- `0.4.0-beta.3/TICKET_createaction_strands_utxos_on_error.md` — the original leak.
- `0.4.0-beta.3/TICKET_placeholder_resolution_failure_broadcasts_anyway.md` — the cheap local fix; do first.
- Upstream: `bsv-blockchain/wallet-toolbox` — `src/storage/methods/createAction.ts`,
  `src/storage/StorageProvider.ts :: updateTransactionStatus`, `src/monitor/Monitor.ts`,
  `src/monitor/tasks/TaskFailAbandoned.ts`, `src/monitor/tasks/TaskReviewUtxos.ts`.
- Our deliberate divergence from wallet-toolbox generally (local SQLite storage) is not an argument
  against this specific change: it is about correctness, not architectural conformity.
