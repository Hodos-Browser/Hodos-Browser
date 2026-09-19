# A reservation whose outpoint is SPENT is never reconciled — it sits in `pending-` limbo forever

**Found:** 2026-09-17 while measuring `P11-11-A4`. **Status:** ✅ **FIXED 2026-09-19** — see § *The fix, as built* at the end.
**Severity:** 🟢 **LOW.** No money is lost, no money is missing from the balance, and the
sweeper is behaving correctly. It is a data-integrity wart with no live path to it in production.

> ⛔⛔ **THIS TICKET WAS REWRITTEN 2026-09-17 AFTER CHECKING THE CHAIN. Its first version was wrong
> in the way that matters**, and the wrong version was committed in `5afe140`. It claimed *"the
> balance has been understated by that amount the whole time"* and implied a live wallet-breaking
> leak. 👤 The owner escalated on exactly that basis — correctly, given what I had written.
>
> ⭐ **The coin is SPENT.** I asserted a loss without querying the chain. That is the mistake to learn
> from here: **before calling a wallet state a leak, ask the chain what it thinks.** One `curl` settled
> it.

---

## What it actually is

```
outpoint  b00be69ce8f2ab6ab2e73212331550f36e86d308631d1987d28f342f58ed1ff7 : 2
          1,419,268 sats · confirmed = 1 · spendable = 0 · spent_by = NULL
          spending_description = pending-1789591461036-0   (2026-09-16 14:44:21)
```

**Ground truth from the chain:**

| Question | Answer |
|---|---|
| Is the parent on chain? | ✅ block 966923, 306 confirmations, vout 2 = 1,419,268 sats |
| Is the outpoint unspent? | ❌ **No.** The address `1P47wgdRcW37H4qHgjXZ2ggr3HeSnhnmu2` has **zero** unspent outputs |
| What spent it? | `80d5821fefffff43a4cb2519347335f3ca1c238f31c755c0004e283d2df724cc`, block 967057, 172 confirmations. Its `vin[0]` **is** `b00be69c…:2`. Outputs: 5,000 (the send) / 1,000 (service fee) / 1,413,068 (change) |

⭐ **And that transaction is the `M4` RED** from the payment sitting — the deliberate fault-injection
run (`PAYMENT_TEST_BATCH.md` M4) that removed all three guards to prove a resolution failure would
broadcast anyway. The seam disabled `update_spending_description_batch`, which is exactly the code
whose job is to replace the `pending-` placeholder with the real txid. It never ran, **by design**.

⇒ **This row is the expected residue of our own negative control.** Not a production defect.

## Why nothing released it — and why that is correct

`TaskSweepReservations` releases **only** outpoints positively observed in the on-chain unspent set.
This one is **spent**, so it declines — every time, for 27 hours. ⭐ **That is the right answer.**
Releasing it would mark a spent coin `spendable = 1` again, which is precisely the `P0.7`
double-spend path the conservative rule was written to close. ⛔ **Do not relax the sweeper.**

## The real (narrow) gap

The row sits in a state no code expects — **`spendable = 0` with `spent_by = NULL`** — and **nothing
ever reconciles it**, even though the information needed is available and we already have the
primitives:

- `reconcile.rs :: check_outpoint_spent` (two-provider) can establish *spent* as a positive fact;
- `reconcile.rs :: recover_change_index_pure` exists and is dormant;
- the spending txid is discoverable, and in this case is already a `completed` row in our own DB.

⇒ The sweeper can express *"observed unspent ⇒ release"*. It **cannot** express *"observed **spent**
⇒ stop asking, link the row to its spender and close it out."* That second verdict has no owner.

⚠️ **Reaching this state in production needs all three guards on the `create_action` path to fail at
once** (M4 measured that they are three deep). That is why this is LOW and not a live hazard.

## Suggested shape (not implemented)

1. Give the sweeper the **second verdict**: on a positive *spent* observation, resolve the row —
   set `spent_by` to the spending transaction and clear the placeholder — instead of declining forever.
2. Record **why** a decline happened and how many times. A reservation declined 320 times for
   "observed spent" is a different animal from one declined once for "API down", and today they are
   indistinguishable from the outside.
3. ⛔ **Negative control:** a reservation whose outpoint is genuinely spent must **never** become
   `spendable = 1`. The test must show the new arm distinguishing *spent* from *unobservable* — a fix
   that merely acts after a timer reintroduces `P0.7`.

## ⭐ Process finding, arguably the more useful half

**Our own fault-injection test left durable state in the dev wallet, and nothing said so.** 27 hours
later that residue was mistaken — by me — for a live wallet defect, and cost the owner a scare and this
investigation.

⇒ `PAYMENT_TEST_BATCH.md` M4 should **disclose its residue** ("leaves one `pending-` reservation whose
outpoint is spent; expected; do not treat as a leak"), and ideally clean up after itself. A negative
control that deliberately breaks bookkeeping should say what it left behind.

## Cross-references

- `PAYMENT_TEST_BATCH.md` M4 — the run that created this row.
- `TICKET_brc121_paid_retry_aborts_and_mints_a_payment_each_time.md` — ⚠️ its "1,419,268 sats still
  reserved" line attributes this reservation to the 402 aborts. **It is M4's.** Same afternoon, two
  different causes; corrected there.
- `phase-0.7-utxo-reservation-leak/` — why the sweeper is conservative. ⛔ Read before touching it.
- `rust-wallet/src/monitor/task_sweep_reservations.rs`, `rust-wallet/src/reconcile.rs`.

---

## The fix, as built (2026-09-19)

👤 Owner's call on the ticket's item 2: **log-only, no schema change.** The *resolution* is a
durable DB write; only the diagnostics live in the log.

### What the sweeper can now say

`TaskSweepReservations` had one verdict -- *observed unspent => release*. It now has two. The
withhold branch no longer just declines: it asks `reconcile::check_outpoint_spent` for
**positive** spent evidence, and on `Spent { spending_txid }` resolves the row through the new
`OutputRepository::resolve_reserved_outpoint_as_spent`.

⛔ **Absence from the unspent set is still never treated as a spend.** That was the whole point of
the conservative design and it is unchanged -- absence covers three different causes (really
broadcast / address not ours / could not read). Only a two-provider agreed, validated successor
txid reaches the resolve arm; `decide_spent` already fails closed to `Unknown` on a flat
contradiction or on providers naming different successors.

### The three things the ticket's suggested shape did not survive

1. ⛔ **`mark_spent` could not be reused.** Its predicate ends `AND spendable = 1`. Every row we are
   fixing is `spendable = 0`, so it matches nothing and returns **`Ok(0)` -- a success value for
   work it did not do**. Pinned by `mark_spent_is_a_silent_noop_on_a_reserved_row`.
2. ⚠️ **`spent_by` is an FK to `transactions.id`, not a txid.** The ticket says "set `spent_by` to
   the spending transaction"; that is only possible when the spender is already a row in our own
   `transactions` table (it is, for M4's coin). When it is not, `spent_by` stays `NULL` and the
   real txid lands in `spending_description`. That is still a **resolved** row -- placeholder gone,
   real txid present -- not an unreconciled one, and the docstring says so, because the next
   reader would otherwise call a `NULL` `spent_by` a bug.
3. ⭐ **The resolve statement never writes `spendable` at all.** Not "writes 0" -- omits the column.
   That absence is the load-bearing line of the change.

### Item 2, honestly: partly done

- ✅ **Verdicts are logged** -- each withheld row now says `verdict: not-ours` or
  `verdict: unobservable` instead of one undifferentiated warning, and the run reports
  released / resolved / withheld separately.
- ⬜ **Cross-run decline counts are NOT recorded.** "Declined 320 times" would have needed the
  column the owner declined. A row that is genuinely spent is now resolved on its first
  conclusive sweep, so the 320-declines case is the one this fix removes rather than counts.

### Negative control -- observed RED, for the right reason

⛔ The rule is that a green nobody has seen fail proves nothing, so `spendable = 1` was injected
into the resolve statement and the suite re-run:

```
assertion `left == right` failed: a SPENT outpoint must never be handed back to the selector (P0.7)
  left: 1
 right: 0
```

Two of the five tests caught it; reverted, all five green again. Full suite: **479 passed, 0
failed, 0 filtered out** (the filter count was checked -- a filtered run that matches nothing also
prints `ok`).

⭐ The other half of the control -- *"distinguishes spent from unobservable"* -- was **already
covered** and is cited rather than duplicated: `reconcile::tests::flat_contradiction_is_unknown`,
`both_spent_disagreeing_is_unknown`, `no_signal_at_all_is_unknown`, against
`one_explicit_spent_other_silent_is_spent` and `both_spent_agreeing_is_spent`.

### Not done

⬜ The ticket's **process finding** -- that `PAYMENT_TEST_BATCH.md` M4 should declare the residue it
leaves -- is untouched. It is a doc change in a file this change does not own, and it is arguably
the more useful half. Still open.
