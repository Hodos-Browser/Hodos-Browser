# Sprint 1 — UTXO safety guard

**Opened:** 2026-08-29 (created at the beta.4 telescope pass — no prior research existed).
**Status:** 🔭 SCOPE ONLY. No phase contracts, no design. The microscope pass writes those.
**Standard:** `../../0.4.0-beta.3/HARNESS.md` + `../HARNESS_DELTA.md`.

---

## Goal

> **No path in the wallet — automatic, user-triggered or recovery — can spend an output the wallet
> has not positively classified as spendable.**

## Why this is first, and why it ships regardless

Users can receive a 1-sat output at a Hodos address **today**, and three code paths treat it as
ordinary spendable value. One of them needs **no user action**.

⛔ **This sprint is needed even if we never ship ordinals.** It is not a prerequisite of the ordinals
feature; it is a live defect that the ordinals feature would multiply.

## The verified findings — read by direct code inspection, 2026-08-29

**Nothing files a 1-sat output into a protective basket.** Full detail in
`../../0.4.0-beta.3/TICKET_token_outputs_destroyed_by_dust_paths.md` (⛔ read-only — another session
owns beta.3).

| # | Path | Trigger | What it does |
|---|---|---|---|
| 1 | `rust-wallet/src/monitor/task_consolidate_dust.rs` | **Automatic, every 86,400 s** (`monitor/mod.rs:79`; fired `mod.rs:358-360`; first tick skipped `mod.rs:183`) | Selects `satoshis <= 1000` (`DUST_THRESHOLD_SATS`, line 25) from the **default basket**, fires at 20 accumulated (`MIN_DUST_COUNT`, line 28), packs them into one output. **There is no lower bound** — `DUST_LIMIT_SATS = 546` (line 31) applies to the *net output* (lines 115, 157), not to any input. **A 1-sat ordinal in the default basket is consolidated and its origin destroyed within 24 h, with no prompt.** |
| 2 | `rust-wallet/src/recovery.rs` | User-triggered — but it is the **restore** flow | `fetch_utxos_for_address` (~line 516) sums **every** UTXO with no value filter; `build_sweep_transactions` batches them into one P2PKH output. Only guard is a dust check on the *output* (~line 602), which a 1-sat input batched with other coins passes. |
| 3 | `rust-wallet/src/handlers.rs:7257` `select_utxos_with_preference` | Ordinary coin selection | `dust_threshold_sats: 5000` (line 7263), and line 7330 **deliberately includes** UTXOs at or under it — the loop *only* adds UTXOs ≤ threshold. A 1-sat output with `basket_id` NULL is among the first things it reaches for. |

⚠️ `grep` finds **zero** `basket` references in `recovery.rs`, `reconcile.rs`, `utxo_fetcher.rs`.

### ⭐ What already works — and why it changes the shape of the fix

`rust-wallet/src/database/output_repo.rs` lines **98** and **148** already filter payment selection:

```sql
AND (o.basket_id IS NULL OR b.name = 'default')
```

An output **correctly filed into a non-default basket is already excluded** from ordinary spending.
**The exclusion logic exists and works.**

> ⛔ **The gap is classification on ingest, not exclusion.** Nothing ever files a token output into a
> basket. This sprint closes that gap. It does **not** build a second exclusion system.

### Related and already correct

`database/basket_repo.rs:62-65` rejects basket names beginning with `p ` per BRC-99 — the right
fail-closed default until we implement a permission scheme. `p ` is **request-time routing only**;
BRC-165 normalization rewrites the storage basket to `1sat`, and the DB never stores a `p ` name.

## Shape — decided at kickoff (owner, 2026-08-29)

**A general classification seam, with one classifier implemented.**

| | |
|---|---|
| The seam returns | `Spendable` / `Token` / `Unknown` |
| ⛔ `Unknown` is | **not spendable** — fail closed |
| Implemented in this sprint | The **1-sat / inscription** classifier only |
| Added later, without reopening call sites | BSV-20/21 and future protocols, as classifiers |

**Why general rather than a value check at five sites:** the exclusion logic is already generic
(it keys on basket, not value). A 1-sat floor at five sites would be the beta.3 stopgap with more
copies, and every later protocol would reopen all five paths.

**Why one classifier rather than several:** BSV-20/21's model is **contested** — BRC-163 merged, BRC-175
open and competing, same author, one day apart. A fungible classifier written today may encode the
side that loses. See `../WATCH_fungibles.md`.

## Sub-sprints

| # | Sub-sprint | Produces |
|---|---|---|
| **1.1** | Enumerate the surface | ⛔ **The definitive list** of places the token/value distinction is needed, proven by call-site sweep. Minimum from the kickoff: **balance, coin selection, dust consolidator, recovery sweep, reconcile** — the deliverable is the *complete* list, not a restatement of those five. |
| **1.2** | Classification on ingest | Every output entering the wallet is classified once, at ingest, and the classification persists. |
| **1.3** | Fail-closed enforcement | Each site in 1.1 either consults the classification, or carries a written reason it does not need to. |
| **1.4** | Recovery and reconcile parity | The three files with zero `basket` references. **Most dangerous path** — it fires when the user is least able to notice a loss. |
| **1.5** | The standing invariant | `R-NOSPEND` and `R-CLASSIFY` live and running at every later boundary (`../REGRESSION_ADDITIONS.md`). |

## In scope / out of scope

| ✅ In | ❌ Out |
|---|---|
| Classification on ingest and its persistence | Inscription rendering, or any UI beyond what proves the classification |
| Fail-closed refusal at every enumerated site | BSV-20/21 classifiers — contested, see `../WATCH_fungibles.md` |
| Recovery / sweep / reconcile parity | Ordinal transfer, listing, purchase — sprint 2 |
| `R-NOSPEND` + `R-CLASSIFY` as standing invariants | Token-spend permission classes — sprint 2.3 |
| Deciding whether the beta.3 floor is kept as defence in depth | BRC-147/150/165 semantics beyond what the classifier needs |

## Exit condition

- [ ] Every site in 1.1 consults the classification, **or** carries a written reason it does not.
- [ ] `R-NOSPEND` green on all three paths, **each with its own RED observed**.
- [ ] `R-CLASSIFY` green, with the ingest-route list it was run against **named** — including the
      routes it does *not* cover.
- [ ] beta.3's regression set run in full at the boundary.
- [ ] Pre-sprint `R-NOSPEND` baseline recorded as **RED** (see below).

⭐ **Run `R-NOSPEND` before writing any code and record it as RED.** It is available today, it costs
almost nothing, and a guard whose regression row was never seen failing beforehand has no baseline —
"it passes now" would be unfalsifiable.

## Owed to the microscope pass — do not answer here

| Question | Why it needs the code in front of you |
|---|---|
| Where exactly the seam sits — ingest only, or ingest + a reconcile verification pass | Depends on whether reconcile can introduce rows that bypass ingest. A code answer, not a preference. |
| What "classified" persists as — reuse baskets, or sit beside them? | Baskets carry BRC-99/165 semantics we implement only partially (`domain_basket_permissions` is one-domain-one-basket-binary; the spec scopes by axis with values in tags). Overloading them may be wrong. |
| How an already-populated wallet's existing outputs get classified | Migration. May be "on next reconcile"; may need a one-time pass. |
| Whether the beta.3 `satoshis > 1` floor is removed once the guard lands | Cheap either way. Defence in depth vs one obvious rule. |
| Whether a T0 static gate on unguarded UTXO-selecting paths is worth adding | ⛔ If yes, **baseline it with `preflight.ps1` itself**, never a hand grep — `HARNESS.md` §9 records two gates whose hand counts were wrong. |

## ⚠️ Unverified, and owed before sizing

> **Does an ordinary incoming 1-sat payment become a tracked default-basket row without a recovery
> scan?**

Confirmed: the wallet derives receive addresses users can hand out (`reconcile.rs:308`, invoice
`"2-receive address-{N}"`), and a tokens UI exists that groups by basket
(`frontend/src/components/wallet/TokensTab.tsx`). **Not** confirmed: the ingest question above.

| If | Then |
|---|---|
| **Yes** | Path 1 is live for any user who has ever received a 1-sat output. Urgent. |
| **No** | Exposure is limited to recovery/sweep and to acquisition paths that do internalize it. Still real, less urgent. |

Either way the affected population today is probably near zero — we ship no ordinal support and there
is little reason to send one here yet. ⚠️ **That stops being true the moment sprint 2 or an OpNS name
lands.** This changes **urgency, not the fix**, which is why the beta.3 floor ships without waiting.

## Links

- `../README.md` — release scope and the three kickoff decisions
- `../SPRINT_PLAN.md` — sprint-level breakdown and cross-sprint edges
- `../REGRESSION_ADDITIONS.md` — `R-NOSPEND`, `R-CLASSIFY`, `R-RESTORE`
- `../../0.4.0-beta.3/TICKET_token_outputs_destroyed_by_dust_paths.md` — ⛔ read-only
- `../sprint-2-1sat-ordinals/README.md` — the rule this enforces was written there first
