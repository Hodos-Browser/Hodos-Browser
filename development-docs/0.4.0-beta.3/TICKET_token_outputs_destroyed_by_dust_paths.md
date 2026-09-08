# 🚨 1-sat token outputs are destroyed by an automatic daily task (and two other paths)

**Found 2026-08-29**, during beta.4 scoping, while reading the prerequisites for the 1Sat Ordinals
sprint. Not found by a failure — found by reading `monitor/` after noticing that
`0.4.0-beta.4/sprint-2-1sat-ordinals/README.md` already states the rule ("a 1-sat ordinal caught by ordinary coin
selection is a **permanently destroyed asset**... treat this with the same seriousness as the
privacy-perimeter gates") while no code enforces it.

**Status: OPEN. Added to beta.3 at the owner's instruction** (2026-08-29) because path 1 below is
**Sprint:** 📌 **Phase 8 (money-path correctness) — but ⛔ SHOULD NOT WAIT.** Bundled 2026-08-31 (`SPRINT_PLAN.md` §4.1), and flagged there as one of four that should move: **path 1 is an automatic daily task**, so it needs no user action to permanently destroy a 1-sat asset. Recommended before or alongside Phase 4.
automatic and needs no user action. The full classification guard is scoped as beta.4 sprint 1; this
ticket is the **minimal defensive floor** only.

> ⚠️ **Method note.** Everything below is from reading the code, not from executing it. The one thing
> **not** verified is §"How exposed are we actually" — whether an ordinary incoming 1-sat payment
> becomes a tracked default-basket row without a recovery scan. That determines severity, and it
> should be answered before sizing the fix.

## What happens

Nothing in the wallet files a 1-sat output into a protective basket, and three paths treat such an
output as ordinary spendable value.

### 1. The daily dust consolidator — automatic, no user action

`rust-wallet/src/monitor/task_consolidate_dust.rs` runs on the monitor loop every **86,400 seconds**
(`monitor/mod.rs:79`, `consolidate_dust: 86400`; fired at `mod.rs:358-360`, skipped on first tick at
`mod.rs:183`).

```rust
const DUST_THRESHOLD_SATS: i64 = 1000;   // line 25 — candidates for consolidation
const MIN_DUST_COUNT: usize   = 20;      // line 28 — fires once this many accumulate
```

It reads confirmed dust UTXOs **from the default basket** (line 71 comment) and filters on value
alone (lines 84 and 135):

```rust
.filter(|o| o.satoshis <= DUST_THRESHOLD_SATS && o.txid.is_some() ...)
```

then packs them into a single output. **A 1-sat ordinal sitting in the default basket is
consolidated — and its origin destroyed — within 24 hours of the twentieth dust UTXO appearing, with
no user action and no prompt.**

There is no lower bound. `DUST_LIMIT_SATS = 546` (line 31) is applied to the *net output value*
(lines 115 and 157), not to any input, so a 1-sat input batched with other dust passes straight
through.

### 2. The recovery sweep — user-triggered, but it is the restore flow

`rust-wallet/src/recovery.rs` scans derived addresses via `fetch_utxos_for_address` (~line 516) and
sums **every** UTXO into the balance with no value filter. `build_sweep_transactions` then batches
them into one P2PKH output. The only guard is a dust check on the *output* (~line 602), which a 1-sat
input batched with other coins passes.

`grep` finds **zero** `basket` references in `recovery.rs`, `reconcile.rs`, or `utxo_fetcher.rs`.

This is the more dangerous of the two in one respect: it fires exactly when a user restores from
seed, which is when they are least able to notice what was lost.

### 3. Coin selection actively prefers dust

`rust-wallet/src/handlers.rs:7257` `select_utxos_with_preference`:

```rust
dust_threshold_sats: i64,   // line 7257 — "Include UTXOs ≤ this amount"
dust_threshold_sats: 5000,  // line 7263
...
if utxo.satoshis > config.dust_threshold_sats { continue; }   // line 7330
```

That loop *only* adds UTXOs at or under 5000 sats. A 1-sat output with `basket_id` NULL is among the
first things it reaches for.

## What already protects us, and why that shapes the fix

`rust-wallet/src/database/output_repo.rs` lines 98 and 148 already filter payment selection:

```sql
AND (o.basket_id IS NULL OR b.name = 'default')
```

So an output that is **correctly filed into a non-default basket is already excluded** from ordinary
spending. The exclusion logic exists and works.

**The gap is that nothing ever files a token output into a basket.** That means the real fix is
classification on ingest, not new exclusion rules — which is why the full guard belongs in beta.4
sprint 1 rather than here.

Related and already correct: `database/basket_repo.rs:62-65` rejects basket names beginning with
`p ` per BRC-99. That is the right fail-closed default until we implement a permission scheme.

## How exposed are we actually — answer this first

**Unverified.** I confirmed the wallet derives receive addresses users can hand out
(`reconcile.rs:308`, invoice `"2-receive address-{N}"`), and that a tokens UI exists which groups by
basket (`frontend/src/components/wallet/TokensTab.tsx`). I did **not** confirm whether an ordinary
incoming 1-sat payment to a receive address becomes a tracked default-basket row without a recovery
scan.

- If **yes** — path 1 is live for any user who has ever received a 1-sat output, and this is urgent.
- If **no** — exposure is limited to the recovery/sweep flow (path 2) and to anyone who acquires a
  1-sat output through a path that does internalize it. Still real, less urgent.

Either way the population today is probably near zero, because we ship no ordinal support and there
is little reason to send one here yet. That will stop being true the moment the 1Sat sprint or an
OpNS name lands.

## Proposed minimal fix for beta.3

Defensive floor only. Not the classification system.

1. **`task_consolidate_dust.rs`** — exclude `satoshis <= 1` from the candidate filter (lines 84 and
   135). Cheapest, highest-value change here; it disarms the only automatic destroyer.
2. **`recovery.rs`** — exclude 1-sat outputs from sweep batches, and either report them separately in
   the scan result or refuse to sweep them silently. Do not let them be summed into a headline
   balance that implies they are spendable.
3. **`handlers.rs:7330`** — add the same floor to the dust-preference pass.

Deliberately **out of scope here**: inscription detection, basket assignment, BRC-147/150 semantics,
UI. All beta.4.

## Test / negative control

Per `HARNESS.md`, a fix is not done until the gate is shown able to fail:

- Stage a wallet with 20+ dust UTXOs **including one 1-sat output**, run the consolidator, and assert
  the 1-sat output is still unspent and the other 20 consolidated.
- **Negative control:** revert the floor, re-run, and show the 1-sat output *is* consumed. If it is
  not, the test is not exercising the path.
- Same pair for the recovery sweep: scan an address holding a 1-sat output plus ordinary coins,
  assert the sweep excludes it, then show it is swept without the guard.
- Regression candidate for `REGRESSION_SET.md`: **no automatic path may ever spend a 1-satoshi
  output.** That invariant should outlive this ticket and hold through the beta.4 guard work.

## Links

- Full beta.4 context and the classification-guard scope:
  `development-docs/0.4.0-beta.4/SESSION_PROMPT_beta4_kickoff.md` §5.
- The rule this violates was already written down in
  `development-docs/0.4.0-beta.4/sprint-2-1sat-ordinals/README.md` ("Two rules from BRC-147 that are load-bearing for
  us", rule 2).
