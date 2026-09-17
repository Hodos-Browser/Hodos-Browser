# A UTXO reservation can be held forever, and the money is invisible while it is

**Found:** 2026-09-17 while measuring `P11-11-A4`. **Status:** 🔴 OPEN — measured, not fixed.
**Severity:** medium (no loss; funds unusable and unexplained for an unbounded time).

---

## What was measured

```
txid b00be69ce8f2ab6ab2e73212  vout 2
  satoshis   1,419,268
  confirmed  1
  spendable  0
  reservation  pending-1789591461036-0
  age          26.9 hours
```

⭐ **1,419,268 sats is the same figure `TICKET_brc121_paid_retry_aborts_and_mints_a_payment_each_time.md`
recorded on 2026-09-16.** So this coin has been held since that incident — more than a day — and the
balance has been understated by that amount the whole time, with nothing on screen to explain it.

## Why the sweeper did not release it

`monitor/task_sweep_reservations.rs` runs every **300 s** against a **15-minute** age threshold, so it
has had ~320 opportunities. It declined every one, and it was **right to** by its own rule:

> ⛔ Releases only outpoints **positively observed** in the on-chain unspent set; every uncertainty
> (API down, address not ours, outpoint absent) leaves the reservation standing.

That rule exists for a good reason and replaced something worse — an unconditional startup restore
that could un-spend a broadcast transaction (`P0.7`). ⛔ **Do not "fix" this by relaxing it.**

**The gap is not the rule, it is that there is no other exit.** A reservation whose outpoint cannot be
positively observed as unspent is held **indefinitely**, and nothing ever revisits the decision or
tells anybody.

## Why it matters

- The coin is `confirmed = 1` and real. It is simply not selectable, so it is missing from the balance.
- ⚠️ **Silent.** No error, no notice, no entry in Activity. The user sees a smaller number than they
  own and has no way to find out why — the same failure shape as the bulk-UTXO 20-cap ticket.
- It compounds with the ancestry work: a wallet short of selectable coins is likelier to hit
  `ERR_BRC121_BEEF_TOO_LARGE` and the PeerPay size limit, because the clean coins may be the reserved
  ones.

## What is NOT wrong

- ⭐ **No money is lost.** The coin is intact on chain and the reservation is a local flag.
- `P11-11-A4` prevents **new** reservations of this shape on the server-refusal path, and `A3` stops
  the re-click case creating them. Neither helps a coin already stuck.
- The conservative sweeper rule is correct and should stay.

## Suggested shape of a fix (not implemented)

1. **Ask why, and record it.** When the sweeper declines, store the reason and the attempt count on
   the row. A reservation declined 300 times for "outpoint not observed" is a different animal from
   one declined once because an API blipped, and today they are indistinguishable.
2. **Escalate rather than relax.** Past some age/attempt count, resolve it *positively* — query the
   outpoint directly (`reconcile.rs :: check_outpoint_spent` already does two-provider work), and act
   on a definite answer in either direction: unspent ⇒ release; **spent ⇒ the reservation is moot and
   the row should be reconciled**, which is the case the current rule cannot express.
3. **Tell the user.** A coin held out of the balance for hours deserves a line in Activity, not
   silence. 👤 Owner's standing rule: the activity log must show every spend — this is the same
   principle applied to money that has stopped being spendable.
4. ⛔ **Negative control:** a reservation whose outpoint is genuinely spent must still **never** be
   naively released back to spendable — that is the `P0.7` double-spend path. The test must show the
   new escalation distinguishing *spent* from *unobservable*, not merely acting after a timer.

## Cross-references

- `phase-11-ui-leftovers/PHASE_CONTRACT_item11_brc121_feedback.md` — `A4`, where this was found.
- `TICKET_brc121_paid_retry_aborts_and_mints_a_payment_each_time.md` — the 2026-09-16 incident that
  created this reservation, and where the 1,419,268 figure first appears.
- `phase-0.7-utxo-reservation-leak/` — why the sweeper is conservative. ⛔ Read before changing it.
- `rust-wallet/src/monitor/task_sweep_reservations.rs`, `rust-wallet/src/reconcile.rs`.
