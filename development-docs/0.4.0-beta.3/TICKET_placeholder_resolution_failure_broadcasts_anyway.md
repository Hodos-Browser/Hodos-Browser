# A failed placeholder→txid resolution is swallowed, and the transaction broadcasts anyway

**Filed 2026-08-22**, out of Phase 0.7. **Reasoned from code, not measured** — see §4 before
treating this as a shipping defect.

**Scope: beta.3, small.** Suggested as a short **Phase 0.7b**. ⛔ **Not a blocker** — Phase 0.7's
on-chain check already contains the dangerous half of this. See §3 for why that matters to ordering.

## 1. What happens

When a transaction is signed, the wallet replaces each input's reservation marker
(`spending_description = 'pending-…'`) with the real txid. That write is what records *which coins
this transaction spent*. It happens **before** broadcast on every path.

If that write fails, every call site logs a warning (or discards the error entirely) and **execution
continues to the broadcast**. Six sites, none of which can stop the send:

| Site | Handling |
|---|---|
| `handlers.rs :: create_action_internal` (auto-sign, txid-changed branch) | `if let Err(e) => warn!` |
| `handlers.rs :: create_action_internal` (txid-unchanged branch) | `if let Err(e) => warn!` |
| `handlers.rs :: sign_action` | `match … Err(e) => warn!` |
| `handlers.rs` on-chain backup path | `let _ = …` |
| `handlers/certificate_handlers.rs` (publish/unpublish) | `let _ = …` |
| `monitor/task_consolidate_dust.rs` | `let _ = …` |

The three `let _ =` sites discard even the ability to notice.

The result is a transaction that is **on-chain but unattributable**: the coins it spent still wear a
`pending-` marker, `spent_by` is never set, and nothing in the wallet links those inputs to the
transaction that consumed them.

## 2. Why the fix is "don't broadcast"

At every one of these sites the transaction is signed but **not yet sent**. Aborting costs nothing —
no money has moved, and the reservation is then correctly released by Phase 0.7's sweeper (the
outpoint is genuinely unspent, so the on-chain check passes and frees it). The alternative we ship
today is to broadcast a spend we have just failed to record.

The rule to encode: **if the wallet cannot record which coins a transaction spends, it must not
broadcast that transaction.**

This is also how `wallet-toolbox` is safe here without any equivalent check — it advances the
transaction's `status` before posting to the network, so a row that never got that far is never
eligible for release. Our equivalent step exists but is advisory.

## 3. Severity — read this before scheduling

Before Phase 0.7 this was the mechanism that made the shipped blanket startup restore a
double-spend path: a `pending-` marker could belong to a broadcast transaction, and the old
`restore_pending_placeholders()` freed markers unconditionally.

**Phase 0.7 already closed that.** The sweeper now verifies the outpoint is unspent on-chain before
releasing, so a marker belonging to a broadcast transaction is *withheld*, not freed.

So the residual harm today is **not** a double-spend. It is:

- a **permanent leak** — the reservation can never be released, because the coin is genuinely spent,
  so the sweeper correctly refuses it forever; and
- **wrong bookkeeping** — `spent_by` unset, the spend unattributed, and activity/history incomplete
  for a transaction that did move money.

That is worth fixing, and it is what makes Phase 0.7's network call belt-and-braces rather than
load-bearing. But it does not justify jumping ahead of Phase 0.8/0.9.

## 4. Status of the evidence

⚠️ **This has not been observed firing.** It is read off the six call sites above, and the code path
is unambiguous. What is *not* established is how often the underlying write actually fails —
reaching it needs a SQLite write failure at that exact moment (disk full, `SQLITE_BUSY` under
contention, corruption). Treat the frequency as unknown, not as zero and not as common.

Do not repeat the sprint's earlier pattern of promoting a code-read to a measured finding. If this
phase runs, **measure it first** (§6 A1).

## 5. Suggested fix

1. Make the three `warn!` sites return an error response instead of proceeding, and convert the
   three `let _ =` sites to checked handling with the same outcome.
2. Leave the reservation in place on abort — do **not** hand-release it. Phase 0.7's guard/sweeper
   already own that, and a hand-rolled release here would be the enumerate-don't-gate pattern again.
3. Audit the BRC-121 paid-retry interaction (`pay_402` / `broadcast_nosend`): an abort must not
   strand a paid retry mid-flight or double-mint a payment.
4. Consider whether a retry (once, immediately) is warranted before aborting, since `SQLITE_BUSY` is
   transient by nature. Retry-then-abort is probably better UX than abort-on-first-failure.

## 6. Test plan

| ID | 🟢 GREEN | 🔴 RED — must be *seen* to fail | 🎯 SUBJECT |
|---|---|---|---|
| `A1` | With the resolution write forced to fail, `/transaction/send` returns an error and **nothing is broadcast** | ⛔ Pre-fix: the same forced failure logs a warning and the transaction **is** broadcast | WhatsOnChain for the txid — *did it reach the network* — not just the HTTP response |
| `A2` | The reservation from an aborted send is released by the Phase 0.7 sweeper on its next pass | — | `spending_description LIKE 'pending-%'` before/after |
| `A3` | A normal send is unaffected — resolution succeeds, `spent_by` set, broadcast proceeds | Regression half | `outputs.spent_by` + activity log |

**Forcing the failure** is the only awkward part: the write has to fail at exactly that moment. Two
workable seams — a `#[cfg(test)]`/env-gated fault injection flag consulted by
`update_spending_description_batch`, or making the DB read-only for the duration of the call.
Whichever is chosen, ⛔ the negative control must show the **pre-fix binary broadcasting anyway**, or
this test proves nothing.

## 7. Out of scope

- Changing what the reservation *is* — that is the separate convergence ticket,
  `../TICKET_reservation_ownership_converge_on_spent_by.md`. This ticket is a strictly local fix
  that is worth doing whether or not that refactor ever happens.

## 8. Related

- `phase-0.7-utxo-reservation-leak/PHASE_CONTRACT.md` §3a — where this was found.
- `TICKET_createaction_strands_utxos_on_error.md` — the leak this sits behind.
- `../TICKET_reservation_ownership_converge_on_spent_by.md` — the structural fix.
