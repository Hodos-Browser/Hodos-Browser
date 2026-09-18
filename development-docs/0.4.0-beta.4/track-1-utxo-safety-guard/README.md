# Track 1 — UTXO safety guard

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

⛔ **This track is needed even if we never ship ordinals.** It is not a prerequisite of the ordinals
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
> basket. This track closes that gap. It does **not** build a second exclusion system.

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
| Implemented in this track | The **1-sat / inscription** classifier only |
| Added later, without reopening call sites | BSV-20/21 and future protocols, as classifiers |

**Why general rather than a value check at five sites:** the exclusion logic is already generic
(it keys on basket, not value). A 1-sat floor at five sites would be the beta.3 stopgap with more
copies, and every later protocol would reopen all five paths.

**Why one classifier rather than several:** BSV-20/21's model is **contested** — BRC-163 merged, BRC-175
open and competing, same author, one day apart. A fungible classifier written today may encode the
side that loses. See `../WATCH_fungibles.md`.

## Candidate phases

| # | Candidate phase | Produces |
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
| Recovery / sweep / reconcile parity | Ordinal transfer, listing, purchase — track 2 |
| `R-NOSPEND` + `R-CLASSIFY` as standing invariants | Token-spend permission classes — track 2.3 |
| Deciding whether the beta.3 floor is kept as defence in depth | BRC-147/150/165 semantics beyond what the classifier needs |

## Exit condition

- [ ] Every site in 1.1 consults the classification, **or** carries a written reason it does not.
- [ ] `R-NOSPEND` green on all three paths, **each with its own RED observed**.
- [ ] `R-CLASSIFY` green, with the ingest-route list it was run against **named** — including the
      routes it does *not* cover.
- [ ] beta.3's regression set run in full at the boundary.
- [ ] Pre-track `R-NOSPEND` baseline recorded as **RED** (see below).

⭐ **Run `R-NOSPEND` before writing any code and record it as RED.** It is available today, it costs
almost nothing, and a guard whose regression row was never seen failing beforehand has no baseline —
"it passes now" would be unfalsifiable.

## Owed to the microscope pass — do not answer here

> ⭐ **RQ-1 is the top design question of this track, and it is a RESEARCH task.** ⛔ Do not answer it
> from first principles. Per `CLAUDE.md` working rule 5: read the **BRC documentation** (46, 99, 147,
> 150, 165), then the BSV Association's **`wallet-toolbox` in TypeScript *and* Go**, then the other
> **BSV SDKs**. Report where implementations agree — that is the convention — and ⭐ **where they
> disagree, because that is the real design question.** ⚠️ There is no Rust implementation; we port
> **patterns and semantics, never code.** Full instructions: `../RESUME_beta4.md` §3.

| Question | Why it needs the code in front of you |
|---|---|
| ⭐ **RQ-1 — what "classified" persists as: reuse baskets, or sit beside them?** | ⛔ **Research first** (see box above). Baskets carry BRC-99/165 semantics we implement only partially (`domain_basket_permissions` is one-domain-one-basket-binary; the spec scopes by axis with values in tags). Overloading them may be wrong — and the ecosystem has already made this choice, so find out what it chose before we do. |
| Where exactly the seam sits — ingest only, or ingest + a reconcile verification pass | Depends on whether reconcile can introduce rows that bypass ingest. A code answer, not a preference. |
| How an already-populated wallet's existing outputs get classified | Migration. May be "on next reconcile"; may need a one-time pass. |
| Whether the beta.3 `satoshis > 1` floor is removed once the guard lands | Cheap either way. Defence in depth vs one obvious rule. |
| Whether a T0 static gate on unguarded UTXO-selecting paths is worth adding | ⛔ If yes, **baseline it with `preflight.ps1` itself**, never a hand grep — `HARNESS.md` §9 records two gates whose hand counts were wrong. |

## ✅ ANSWERED 2026-09-08 — was "Unverified, and owed before sizing"

> **Does an ordinary incoming 1-sat payment become a tracked default-basket row without a recovery
> scan?**

**YES.** Measured during the beta.3 Phase 8 kickoff, and the ingest path is not the one this
document guessed — it is neither recovery nor sweep, it is **automatic and continuous**:

| # | Link | Evidence |
|---|---|---|
| 1 | `monitor/task_sync_pending.rs :: run` — automatic, **every 30 s** for fresh addresses (3 min / 5 min for older tiers), **plus a full sweep of all pending addresses on every startup** | `monitor/mod.rs:75` `sync_pending: 30`; `task_sync_pending.rs:8-11`, `:53` `FIRST_RUN` |
| 2 | Calls `upsert_received_utxo_with_confirmed` for **every** UTXO returned, with **no value filter of any kind** | `task_sync_pending.rs:~168` (individual tier), `:307` (bulk tier) |
| 3 | That insert writes `spendable = 1`, `basket_id` **unset ⇒ NULL**, `derivation_prefix = '2-receive address'`, `type = 'P2PKH'` hardcoded | `output_repo.rs:491-509` |
| 4 | Such a row passes **all four** WHERE clauses of `get_spendable_confirmed_by_user` | `output_repo.rs:133-150` |

Asserted, not merely read: `output_repo.rs :: token_reserved_exposure_tests` (beta.3 `383bf4f`) drives
the **real ingest function** against a seeded DB and shows the row comes back spendable with
`basket_id IS NULL` — and that the same row filed into a `1sat` basket is correctly excluded.

⇒ **The exclusion logic works. Nothing files a token into a basket. That gap is this track.**

The beta.3 floor (`R-DUST`) now blocks 1-satoshi outputs from every spend path, so the immediate
hazard is contained. ⛔ **It is a value floor, not a classifier** — it cannot tell a token from a
stray 1-satoshi payment and protects nothing at 2 satoshis or above.

## 🚨 Read this before designing the classifier — `locking_script` is a FABRICATION

**Measured against mainnet 2026-09-08.** Full detail and reproducible outpoints:
`../../0.4.0-beta.3/TICKET_synced_outputs_store_a_fabricated_locking_script.md`.

The wallet **never records the locking script it saw on chain.** `utxo_fetcher.rs` generates one from
the address (`generate_p2pkh_script_from_address`) at all three fetch sites, because neither indexer
returns a per-UTXO script. The result is **always exactly 25 bytes in exactly the P2PKH pattern.**

⛔ **A classifier that inspects `outputs.locking_script` will read that fabrication for every output
address sync has ever found, and classify all of them `Spendable`.** The `ord` envelope it exists to
detect is erased at ingest, before it runs.

⚠️ **It would not error.** It would fail closed on nothing, pass everything, and look like it works —
the same shape as the three 0.4.0 farbling harnesses that would each have passed with the feature
absent. `R-CLASSIFY`'s RED ("prove the default is refusal") **cannot be observed** while the input is
synthesised, because every output looks identical and legitimate.

What is actually on chain, for the three real cases:

| Kind | Real script | Stored |
|---|---|---|
| Transferred ordinal | bare P2PKH, **25 B** | correct, by luck |
| Fresh inscription | P2PKH **+ ord envelope — 2,596,810 B measured** | ⛔ 25 B fabrication |
| OrdLock listing | contract, **860 B**, not P2PKH-shaped | ⛔ 25 B fabrication |

🚨 The inscription's **first 25 bytes are a valid P2PKH**, then `OP_FALSE OP_IF "ord" OP_1 "image/png"`.
That is *why* address indexers return it under the owner's address — and why it reaches us at all.

⭐ **The fix needs no new fetch and no schema change**, which is why it belongs at the front of this
track rather than in its own: `cache_parent_transactions` already stores the raw parent tx,
`reconcile.rs :: parse_tx_outputs` already returns per-output `(value, script)`, and
`outputs.script_length` already exists and is never written. ⚠️ But **do not write the naive version** —
copying a 2.6 MB script into the `locking_script` BLOB lands it in the DB *and* in every on-chain
backup. The ticket sets out three bounded options and takes no decision.

⛔ **Sequencing:** do this **before** 1.1's route list is turned into a classifier, not after.
Otherwise track 1 has to build its own negative control proving the classifier can see an envelope
at all — harder than the fix.

## ⚠️ Also carried from beta.3 Phase 8 — two path claims that did not survive

Both were measured false; do not inherit them from the beta.3 ticket:

- **The recovery sweep is the EXTERNAL-wallet import**, not restore-from-seed.
  `scan_external_wallet` / `build_sweep_transactions` have exactly one caller,
  `handlers.rs :: wallet_recover_external`. `recover_wallet_from_mnemonic` builds **no sweep at all**
  — it discovers and reports, so restore-from-seed is an *ingest amplifier* for the sync path above,
  not itself a destroyer.
- **`create_action`'s `send_max` branch bypasses coin selection entirely**
  (`selected_utxos = all_utxos.clone()`). Any guard placed only in the selector misses it. beta.3
  floored it by owner decision; a classifier must cover it as its own route in `R-CLASSIFY`'s list.

⭐ And a guard that is **not** a guard: `task_consolidate_dust :: is_p2pkh_script` cannot return false
for a synced output, because it inspects the fabricated script. Do not count it as a route defence.

## Links

- `../README.md` — release scope and the three kickoff decisions
- `../RELEASE_PLAN.md` — track-level breakdown and cross-track edges
- `../REGRESSION_ADDITIONS.md` — `R-NOSPEND`, `R-CLASSIFY`, `R-RESTORE`
- `../../0.4.0-beta.3/TICKET_token_outputs_destroyed_by_dust_paths.md` — ⛔ read-only
- `../track-2-1sat-ordinals/README.md` — the rule this enforces was written there first
