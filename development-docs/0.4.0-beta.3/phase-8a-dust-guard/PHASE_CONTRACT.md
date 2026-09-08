# Phase 8 (first ticket) — the 1-satoshi destruction guard · PHASE CONTRACT

**Workstream:** money-path correctness · **Ticket:** `TICKET_token_outputs_destroyed_by_dust_paths.md`
**Status:** ✅ **IMPLEMENTED — all 9 evidence rows GREEN with their RED halves observed.**
`Q-1` answered by the owner 2026-09-08: *"1 sats are special tokens. Do not send them with max, only
normal UTXOs."* ⇒ row `A6` in scope and landed.
**Opened:** 2026-09-08 · **Owner:** Matthew Archbold · **Platforms:** Windows (Rust — platform-neutral; no macOS relay row)
**Standard:** `../HARNESS.md`.

**Base commit:** `b651aa8`. Local `HEAD == origin/0.4.0` tracking ref, 0 ahead / 0 behind.
⚠️ **Not re-verified against the network.** `git ls-remote` cannot authenticate in this shell — the
`gh.exe` credential helper resolves to a non-existent WSL path and git then falls through to a
`/dev/tty` prompt. So *"pushed and verified"* is **inherited from the session prompt, not re-measured
here.** Labelled per HARNESS §6 Q4 rather than quietly assumed.

---

## 0. Plan-vs-tree delta — measured before anything was written

> ⛔ Everything below is a **code reading** unless it says *measured live*. No live run has happened.
> The ticket carries the same caveat about itself; this section does not inherit its uncertainty —
> where it says CONFIRMED or REFUTED, I traced the code.

Phase 7c found four false claims in one document; Phase 7d found eight. **This kickoff found four
corrections, one unticketed path, one dead doc link — and, unusually, every single line number in the
ticket verified accurate.** That last point is worth stating plainly: it is the first document this
sprint whose citations all held.

### 0.1 ✅ `D-1` — Every cited line number is CORRECT

| Citation | Tree, 2026-09-08 | |
|---|---|---|
| `task_consolidate_dust.rs:25` `DUST_THRESHOLD_SATS = 1000` | exact | ✅ |
| `task_consolidate_dust.rs:28` `MIN_DUST_COUNT = 20` | exact | ✅ |
| `task_consolidate_dust.rs:84` / `:135` candidate filters | exact — and they are **byte-identical duplicates** | ✅ |
| `task_consolidate_dust.rs:31` `DUST_LIMIT_SATS = 546`, applied at `:115` / `:157` to **net output** | exact | ✅ |
| `monitor/mod.rs:79` `consolidate_dust: 86400`; fired `:358-360`; skipped first tick `:183` | exact | ✅ |
| `handlers.rs:7257` / `:7263` / `:7330` dust-preference pass | exact | ✅ |
| `recovery.rs:~516` `fetch_utxos_for_address`; `~602` output dust check | 516 exact; the check is at **600** (cited "~") | ✅ |
| `output_repo.rs:98` / `:148` `AND (o.basket_id IS NULL OR b.name = 'default')` | exact, both | ✅ |
| `reconcile.rs:308` `derive_receive_address`, invoice `"2-receive address-{N}"` | exact | ✅ |
| `basket_repo.rs:62-65` rejects `p ` prefix per BRC-99 | exact | ✅ |

⇒ **Nothing in the ticket needed a line-number repair.** The corrections below are all about *meaning*.

### 0.2 🚨 `D-2` — SEVERITY ANSWERED: **YES.** And the ingest path is not the one the ticket guessed

The ticket's one unverified question:

> *Whether an ordinary incoming 1-sat payment becomes a tracked default-basket row without a recovery scan.*

**It does. Automatically, within 30 seconds, with no user action and no recovery scan.** The full
chain, every link read:

| # | Link | Evidence |
|---|---|---|
| 1 | `monitor/task_sync_pending.rs :: run` — **an automatic monitor task, every 30 s** — scans every address with `pending_utxo_check = 1`, tiered: fresh 0–3 h **every 30 s**, recent 3–18 h every 3 min, old 18 h+ every 5 min, **plus a full sweep of all pending addresses on every startup** | `mod.rs:75` `sync_pending: 30`; `task_sync_pending.rs:8-11`, `:32-34`, `:53` `FIRST_RUN` |
| 2 | Both tiers call `output_repo.upsert_received_utxo_with_confirmed(...)` for **every** UTXO returned, with **no value filter of any kind** | `task_sync_pending.rs:~168` (individual), `:307` (bulk) |
| 3 | That insert writes `spendable = 1`, `basket_id` **unset ⇒ NULL**, `derivation_prefix = '2-receive address'`, `type = 'P2PKH'` **hardcoded**, `transaction_id` NULL, `confirmed` from the API | `output_repo.rs:491-509` |
| 4 | `get_spendable_confirmed_by_user` — the consolidator's source — passes such a row on **all four** WHERE clauses: `spendable=1` ✅, `(transaction_id IS NULL AND confirmed=1)` ✅, `derivation_prefix IS NOT NULL` ✅, `(basket_id IS NULL OR b.name='default')` ✅ | `output_repo.rs:133-150` |
| 5 | `task_consolidate_dust` filters on `satoshis <= 1000` — **no lower bound** — and consolidates | `task_consolidate_dust.rs:84`, `:135` |

⇒ **Path 1 is live today for anyone who has ever received a 1-sat output at a Hodos receive address.**
The ticket hedged between "recovery scan only" and "live"; it is the live branch. The same endpoint is
reachable manually via `wallet_sync` (`handlers.rs:3151`), which runs the identical insert at `:3282`.

⚠️ **One real precondition remains, and it is not a small one.** The consolidator still needs
**20 dust UTXOs** *and* `net_value >= 546` after the mining fee and the **1000-sat Hodos service fee**
(`:114-120`). At ~148 bytes/input, 20 inputs is ~3 000 sats of mining fee, so total dust must exceed
roughly **4 550 sats** before the task fires at all. So: the mechanism is armed and automatic, but it
does not fire on a wallet holding one ordinal and nothing else. **Urgent, not on fire.**

### 0.3 🚨 `D-3` — Path 2 is misattributed. It is **external-wallet import**, not restore-from-seed

The ticket: *"it fires exactly when a user restores from seed, which is when they are least able to
notice what was lost."* **Refuted.**

| | Ticket's claim | Tree |
|---|---|---|
| Function | `recovery.rs` scan + `build_sweep_transactions` = the restore flow | `scan_external_wallet` (`:487`) + `build_sweep_transactions` (`:568`) are reached from **one** caller: `handlers.rs :: wallet_recover_external` (`:16182`, calls at `:16276` / `:16328`) — the **import-an-external-wallet** flow (`ExternalWalletConfig::centbee()` et al.) |
| Restore-from-seed | assumed to be the same code | is `recover_wallet_from_mnemonic` (`recovery.rs:136`, called at `handlers.rs:15785` / `:16019`). It **discovers and reports**; it builds **no sweep transaction at all** |

⇒ The destruction still happens — sweeping a Centbee/HandCash wallet into Hodos will batch a 1-sat
ordinal into a single P2PKH output (`:585` sums all, `:600` only checks the *output*) — but the
**trigger, the affected population and the emotional framing are all different**. Restore-from-seed is
not itself a destroyer; it is an **ingest amplifier for path 1** (recovered addresses become
`pending_utxo_check` rows and flow into D-2's chain).

### 0.4 🚨 `D-4` — A **fourth** path the ticket does not name: `send_max` bypasses the selector entirely

`handlers.rs :: create_action`, `:5178-5180`:

```rust
if send_max {
    // Send max: select ALL available UTXOs to drain the wallet
    selected_utxos = all_utxos.clone();
```

This does **not** go through `select_utxos_with_preference`. ⛔ **The ticket's proposed fix #3 — a
floor inside the dust-preference pass — would not touch it.** A guard that looks complete while
leaving the "drain everything" path open is the worst possible shape for this ticket.

- It is **user-visible and one click**: `TransactionForm.tsx:522` sets `sendMax: true` from a Max button.
- It is **one site, not two**: `send_transaction` (`:10051`, the `/send` endpoint) delegates inward to
  `create_action` (`:10233`), so both routes converge on `:5178`.
- ⚠️ **It is user-initiated, so fixing it is a behaviour change** — "send max" would leave 1 satoshi
  behind. That is an owner decision, flagged in §8 as `Q-1`, not something I will assume.

### 0.5 ⛔ `D-5` — `is_p2pkh_script` is **not** a guard for the dominant ingest path

`task_consolidate_dust.rs:445-454` carries this comment:

> *"This guards against accidentally trying to spend PushDrop tokens or other non-standard scripts."*

**For anything discovered by address sync, it has no teeth.** The stored `locking_script` is
**synthesised from the address**, never observed on-chain — `generate_p2pkh_script_from_address()`
(`utxo_fetcher.rs:290`) is applied to every returned UTXO in all three fetch paths: WoC (`:163`),
GorillaPool (`:213`), and bulk (`:418-420`). It always produces exactly 25 bytes in exactly the P2PKH
pattern, so `is_p2pkh_script()` **cannot return false** for a sync-discovered row regardless of what is
actually on chain.

Two consequences, and they point the same way:

1. **Value is the only reliable discriminator available at this layer.** This *strengthens* the
   ticket's proposed fix rather than complicating it.
2. ⚠️ **Adjacent, out of scope, worth a ticket:** the GorillaPool fallback is
   `ordinals.gorillapool.io/api/txos/address/{addr}/unspent` — an *ordinals* indexer — and any
   non-P2PKH output it returns is stored with a fabricated script. Signing against a wrong script
   yields an invalid signature and a rejected broadcast: a **failure**, not a loss. Noted, not fixed
   here.

### 0.6 ⚠️ `D-6` — Path 3's mechanism is the consolidation pass, not the primary pass

The ticket: *"A 1-sat output with `basket_id` NULL is among the first things it reaches for."*
`select_utxos_greedy` (`:7296`) sorts **largest-first** (`:7305`) and breaks as soon as the target is
met — so the primary pass reaches a 1-sat output **last**, only on a near-total drain. What actively
pulls it in is the **lazy-consolidation pass** (`:7325-7341`), capped at `max_extra_inputs: 10`.
⇒ Minor, but it decides *where* the floor goes: **both passes need it**, for different reasons.

### 0.7 ⚠️ `D-7` — Dead doc link, cited twice

`development-docs/1SatOrdinals-BSV21/README.md` **does not exist**. The rule lives at
`development-docs/0.4.0-beta.4/sprint-2-1sat-ordinals/README.md:38` ("Two rules from BRC-147 that are
load-bearing for us"). Rule 2 read and confirmed — it says exactly what the ticket says it says,
including *"a general 'pay' or auto-pay grant **MUST NOT** authorize spending them"* and
*"permanently destroyed asset"*. The path is stale in **both** `TICKET_...dust_paths.md:142` and
`SESSION_PROMPT_beta3_phase8_dust_guard.md:24`. Fix both as part of this phase.

### 0.8 ⭐ `D-8` — The expensive part of the test plan is already solved

The session prompt flags *"staging 20 dust UTXOs is the real cost of this ticket"* and asks for an
early fixture-vs-live decision. **Fixture, decisively** — the pattern already exists and is proven:

| Asset | Where |
|---|---|
| In-memory SQLite fixture: `Connection::open_in_memory()` + `migrations::create_schema_v1` + a seeded user | `output_repo.rs :: stale_reservation_tests::seed_db` (`:1264-1275`), landed by P0.7 |
| A one-line `reserved(...)` helper that inserts an output row — trivially adapted to seed 20 dust rows | `output_repo.rs:1278-1287` |
| `recovery::build_sweep_transactions` is **pure and synchronous** | `recovery.rs:568` — testable with no DB at all |
| `handlers::select_utxos_greedy` is **pure and synchronous** | `handlers.rs:7296` — same |

⇒ Only the consolidator resists: its filter is inline inside `run_inner`, an `async fn` needing
`web::Data<AppState>`, network and broadcast. **That is the one extraction this phase needs** (§7).

---

## 1. Goal

No automatic path in the wallet can spend a 1-satoshi output, and no manual bulk path can spend one
without the user having asked for that specific thing.

## 2. Done means

- [x] The daily dust consolidator excludes `satoshis <= 1` from its candidate set, in **both** the
      pre-lock and under-lock filters, via **one** shared predicate rather than two copies.
- [x] The external-wallet sweep excludes 1-sat outputs from sweep batches and **reports them
      separately** (`ExternalScanResult::token_reserved`) instead of summing them into a headline
      balance that implies they are spendable. Floored in **both** `scan_external_wallet` and, as a
      second line of defence, in the `pub` `build_sweep_transactions` itself.
- [x] Coin selection excludes `satoshis <= 1` in **both** the primary greedy pass and the lazy
      consolidation pass — one filter ahead of the sort covers both.
- [x] `send_max` excludes `satoshis <= 1` (owner-approved, `Q-1`).
- [x] The floor is **one named constant with one predicate** (`TOKEN_RESERVED_SATS` /
      `is_token_reserved_value` in `utxo_fetcher.rs` — see `D-9`), not four `<= 1` literals.
- [x] Each site has a unit test **and** an in-file negative control proving the test would fail
      without the floor. 19 new tests — ⚠️ but **11** is the evidence count: that is how many fail
      when the feature is disabled at `TOKEN_RESERVED_SATS`. See ADVERSARIAL_REVIEW.md `F2`.
- [ ] `REGRESSION_SET.md` gains `R-DUST` (§9) — ⛔ **separate commit**, working rule 6.
- [x] The dead `1SatOrdinals-BSV21` path is corrected in the ticket and the session prompt.

## 3. Invariants preserved

| ID | Invariant | Why this phase could break it |
|---|---|---|
| `R-GOLD` | Gold pill fires on every auto-approved payment | Not touched — this phase adds no spend path and changes no IPC. Listed to record that it was considered and why it is inapplicable. |
| `R-COUNT` | Per-session counters | Not touched. Same. |
| `R-PERIM` | Four privacy-perimeter gates | Not touched — no permission-engine change. ⚠️ BRC-147 rule 2 says the *real* home for this rule is `hodos_permission_engine`. **That is beta.4 sprint 1, not here** — this phase is a floor beneath the selector, not a gate. |
| **new `R-DUST`** | **No automatic path may ever spend a 1-satoshi output** | This phase creates it. |

⛔ **CLAUDE.md #2 / #3 — neither is engaged.** The fix needs **no DB schema change** and **no
crypto/derivation change**. If implementation makes me want either, I stop and ask.

## 4. Evidence table

⛔ No empty RED or SUBJECT cells.

| ID | 🟢 GREEN — must be true | 🔴 RED — must be *seen* to fail, and how | 🎯 SUBJECT — proves the right thing was measured | Tier | Result |
|---|---|---|---|---|---|
| `P8-A1` | Fixture: 20 dust UTXOs (200–999 sats) + one 1-sat output → consolidator candidate set has **20 rows, and the 1-sat outpoint is not among them** | Delete the floor from the shared predicate, re-run: candidate set is **21 rows and includes the 1-sat outpoint** | Assert candidate count **rises to 21** without the floor. ⛔ If it stays 20, nothing reached the filter and `A1` is VACUOUS — the failure mode the ticket names by name | T1 | 🟢 **GREEN.** `one_sat_output_is_not_a_consolidation_candidate`. 🔴 **RED OBSERVED — production floor deleted from `is_consolidation_candidate`, re-run: `assertion left == right failed: left: 21, right: 20`.** Exactly the predicted count rise. Floor restored, re-run green |
| `P8-A2` | An in-file negative-control test calls the **same** candidate set with the floor bypassed and asserts the 1-sat output **is** included | `A2` itself fails if the bypass stops reaching the filter | `A2` is `A1`'s permanent control — it makes `A1` un-vacuous **for every future run**, not just today's | T1 | 🟢 **GREEN.** `without_the_floor_the_one_sat_output_would_be_consolidated` — asserts 21, ships with the code |
| `P8-A3` | `build_sweep_transactions` over [1 sat + 3 ordinary UTXOs] produces a tx whose inputs **exclude** the 1-sat outpoint | Remove the floor: the 1-sat outpoint **appears in the tx inputs** | Assert on the built transaction's **input outpoints**, not on a count or a log line | T1 | 🟢 **GREEN.** `sweep_transaction_inputs_exclude_the_token_carrier` — asserts input-count byte `raw[4] == 3` **and** that the carrier's little-endian outpoint is absent from the serialised tx. 🔴 `sweep_of_only_carriers_refuses_rather_than_spending_them` (errors, does not sweep). ⭐ The test carries its **own** control: it asserts an ordinary outpoint **is** found by the same byte search, so an absent-carrier result cannot come from a broken search |
| `P8-A4` | `scan_external_wallet`'s reported `total_balance` **excludes** 1-sat outputs, and they surface in a distinct field | Without the change, `total_balance` includes them | Assert both fields — a balance that merely got smaller could be any bug | T1 | 🟢 **GREEN.** `scan_split_excludes_token_carrier_from_balance_and_reports_it` — 3 sweepable, `total == 100_000`, carrier present in `token_reserved`. 🔴 `without_the_floor_the_carrier_would_be_swept_and_counted` — same input unguarded is 4 outputs / **100_001** sats |
| `P8-A5` | `select_utxos_greedy` with a 1-sat present: neither the primary pass nor the consolidation pass selects it, at any `amount_needed` including "needs everything" | Remove the floor: the consolidation pass selects it; and at drain-level `amount_needed`, so does the primary pass | Two sub-cases, run separately — `D-6` says the two passes reach it for different reasons | T1 | 🟢 **GREEN, both passes separately.** `consolidation_pass_does_not_sweep_up_the_carrier` (and asserts an ordinary small output **is** still consolidated, so it cannot pass by consolidation doing nothing); `primary_pass_fails_rather_than_spending_the_carrier` (returns empty at `ordinary_total + 1`; succeeds at `ordinary_total`). 🔴 `without_the_floor_the_carrier_is_selected` — at **2 sats** the identical calls **do** select it |
| `P8-A6` | `send_max` selection over a wallet containing a 1-sat output excludes it; the send still drains everything else | Remove the floor: the 1-sat is in `selected_utxos` | Assert on `selected_utxos`, **not** on the `/send` response — the response is identical either way | T1 | 🟢 **GREEN.** `send_max_drains_everything_except_the_carrier` — 3 of 4 outputs, 54,900 sats, asserted on the exact value the branch assigns via the extracted `select_all_spendable`. 🔴 `without_the_floor_send_max_would_take_the_carrier` — 4 outputs / **54,901** sats |
| `P8-A7` | **`D-2` is a measurement, not a code reading:** a row inserted by the real `upsert_received_utxo_with_confirmed`, at 1 sat, is returned by `get_spendable_confirmed_by_user` | Same row filed into a non-default basket is **not** returned | The RED half proves the query's basket filter works — so the GREEN half is about the **missing basket assignment**, which is the actual defect and beta.4's actual job | T1 | 🟢 **GREEN — severity question now MEASURED.** `incoming_one_sat_payment_becomes_a_spendable_default_pool_row`: spendable, `basket_id IS NULL`. 🔴 `the_same_row_in_a_protective_basket_is_excluded`: filed into a `1sat` basket ⇒ excluded. ⭐ Uses the **real ingest function**, not a hand-written INSERT |
| `P8-A8` | `cargo test` passes in full | — | 🚨 **`cargo build --release` does not compile `cfg(test)` code** — it passed over 8 broken call sites in Phase 7c. `cargo test` is the only instrument that counts here | T0 | 🟢 **GREEN. Full suite, not filtered:** 458 lib + 526 bin + 16 integration binaries. **0 failed, 2 ignored** (pre-existing network tests) |
| `P8-A9` | `scripts/preflight.ps1 -Full` green; `-NegativeControl` shows every T0 gate fail | as stated | ⛔ **Bare `preflight.ps1` skips `T1d` and says so — a skip is never a pass.** `-Full` or it did not run | T0 | 🟢 **GREEN.** `-Full`: all 8 static gates at baseline (G11 **60**, unmoved), T1a–T1g all PASS incl. **T1d not skipped**. 🔴 `-NegativeControl`: **all 11 gates seen to fail** on injection |

**Two-sided pairing:** `A1`+`A2` are each other's control and must be read together — `A2` exists
precisely so `A1` cannot silently become vacuous later. `A7`'s two halves are the same shape.

⛔ **No live-wallet row.** Every row is a fixture or a pure-function test, deliberately: per `D-8`
they are all reachable that way, and the session prompt is explicit that *"a live run that cannot be
repeated is worth less than a fixture that can."* The consolidator's economics gate (`D-2`, ~4 550
sats of real dust) also makes a live trigger expensive and slow to arrange for no added signal.

## 5. Blast radius

| Touched | Not about it, but touched |
|---|---|
| `monitor/task_consolidate_dust.rs` | The extraction in §7 moves the duplicated filter. ⚠️ The two copies at `:84` / `:135` are currently byte-identical; the extraction must keep them identical, which is half its value. |
| `recovery.rs` | `build_sweep_transactions` + `scan_external_wallet`. ⚠️ `ExternalScanResult` gains a field ⇒ its consumer `handlers.rs :: wallet_recover_external` (`:16182`) must be updated. Struct change, **not** a DB change. |
| `handlers.rs` | `select_utxos_greedy` (`:7296`) — **shared by 4 call sites**: `create_action` `:5187`, backup `:13808`, certificate publish `certificate_handlers.rs:2798`, certificate unpublish `:4509`. The three non-`create_action` sites pass `None` for consolidation, so a primary-pass floor changes their behaviour too — **intended**, and the reason `A5` tests the primary pass separately. |
| `handlers.rs :: create_action` `:5178` | Only if `Q-1` is approved. |
| `REGRESSION_SET.md` | ⛔ **Separate commit.** Working rule 6: the instrument is not edited by the change it measures. |

**⛔ A fifth spend path, considered and deliberately NOT floored** — added 2026-09-08 by the
adversarial review (`F1`); §5 originally omitted it entirely, so a reviewer could not tell it had
been looked at:

| Path | Where | Why it is left alone |
|---|---|---|
| `create_action` **`user_inputs`** — a dApp names an outpoint explicitly | accepted `handlers.rs:4808-4886`, added to the tx at **`:5366-5381`** with no value check; signed by `sign_action` (`:7494`, `:7600-7740`) when no `unlockingScript` is supplied | A deliberate ordinal transfer **is** a dApp naming a 1-sat outpoint. A blanket refusal here would make beta.4 sprint 2 unimplementable. ⚠️ The right guard is BRC-147 rule 2's — enforced in `hodos_permission_engine`, which has no notion of a token-carrying input today ⇒ **beta.4 sprint 1** |

**Deliberately NOT touched, though adjacent and tempting:**
`is_p2pkh_script` and its false comment (`D-5`) · the fabricated-locking-script issue (`D-5`.2) ·
`task_consolidate_dust`'s unrequested 1000-sat service fee · the missing basket assignment on ingest
(`output_repo.rs:491`) — **that last one is beta.4 sprint 1 and is the actual fix.**

## 6. Out of scope

Inscription detection · basket assignment on ingest · BRC-147/150 semantics · any UI · the permission
engine · the four `D-5` follow-ups. **All beta.4.**

⭐ **Say this out loud in the commit and the close-out, per the session prompt:** `output_repo.rs`
already excludes correctly-basketed outputs from spending, and that logic **works**. The defect is
that **nothing ever files a token into a basket**. This phase is a **floor**, not a solution.
beta.4 sprint 1's classification-on-ingest is the answer.

## 7. Extractions — planned one, delivered three

⚠️ **Delta from the plan, recorded rather than quietly absorbed.** The contract predicted **one**
extraction. Implementation needed **three**, for the same reason each time: the code that had to be
proven was inline in an `async fn` that needs `AppState`, a network and a broadcast, so `A4` and `A6`
would have been code readings rather than tests. Each is a few lines, single-use, and replaces an
inline branch — none introduces an abstraction layer.

| Fn | File | Why it had to exist | Row |
|---|---|---|---|
| `is_consolidation_candidate(&Output) -> bool` | `task_consolidate_dust.rs` | **Planned.** Also de-duplicates the byte-identical filters at `:84` / `:135` so they cannot drift | `A1`, `A2` |
| `split_token_reserved(Vec<ExternalUTXO>) -> (Vec, Vec, i64)` | `recovery.rs` | **Unplanned.** `scan_external_wallet` is `async` and hits WhatsOnChain; without this, `A4` could not run offline | `A4` |
| `select_all_spendable(&[UTXO]) -> Vec<UTXO>` | `handlers.rs` | **Unplanned.** ⛔ Testing `is_token_reserved_value` alone would **not** prove the `send_max` branch calls it — that is precisely the vacuous-test trap. This is the exact value the branch assigns to `selected_utxos` | `A6` |

### 🚨 `D-9` — the floor could not live where the contract put it (found by the compiler)

The contract proposed the constant in `handlers.rs` beside `HODOS_SERVICE_FEE_SATS`. **That does not
compile.** `rust-wallet` has **two crate roots**: `main.rs` declares 37 modules including `handlers`,
while `lib.rs` declares only 19 — and **`handlers` is not among them.** `recovery.rs` is declared by
**both**, so it compiles twice, and `crate::handlers::…` is unresolvable in the library build:

```
error[E0433]: cannot find `handlers` in `crate`
   --> src\recovery.rs:546:35
```

⇒ The floor lives in **`utxo_fetcher.rs`**, which owns the `UTXO` type and is declared by both roots.
`handlers.rs` re-exports it (`pub use crate::utxo_fetcher::{is_token_reserved_value, TOKEN_RESERVED_SATS}`)
so the monitor's existing `crate::handlers::` import style is unchanged.

⭐ This also puts the constant beside `generate_p2pkh_script_from_address` — the function whose
synthesised scripts are the reason (`D-5`) value is the only discriminator available. The doc comment's
cross-reference is now local and checkable rather than a claim about a distant file.

## 8. Open question for the owner — blocking `A6` only

| ID | Question | My recommendation |
|---|---|---|
| `Q-1` | Should the floor apply to **`send_max`** (`D-4`)? It is user-initiated and one click, and applying the floor means "Send max" leaves 1 satoshi behind. | **Yes, apply it.** "Send everything I have" is a statement about *money*; a user clicking Max is not asking to destroy an asset, and the residue is 1 satoshi. Leaving it out means shipping a guard with a one-click hole and an invariant that reads as broader than it is. ⚠️ But it is a visible behaviour change on a shipped button, so it is your call, not mine. |

Everything else proceeds without an answer — `Q-1` gates row `A6` and that row alone.

## 9. Proposed `REGRESSION_SET.md` addition — `R-DUST`

⛔ Lands in **its own commit**, after the code, per working rule 6.

> **`R-DUST` — no automatic path may ever spend a 1-satoshi output.**
> **GREEN:** with a 1-sat output present in the default pool, neither the dust consolidator, coin
> selection, nor any sweep includes it in a transaction's inputs.
> **RED:** removing the shared floor predicate puts it back in the candidate set — asserted by count,
> not merely by absence.
> **SUBJECT:** the candidate/selection set itself, not a broadcast result or a log line.
> ⚠️ **Survives beta.4.** When sprint 1 lands classification-on-ingest, this invariant must still hold
> — and it must then hold *for a different reason*. If the guard work makes `R-DUST` pass because
> nothing reaches the filter any more, `R-DUST` has gone vacuous and needs re-basing.

⭐ **Carried from the Phase 7d boundary, unrelated to this ticket but owed here:** `R-CLOSE`'s SUBJECT
names three C++ paths and no React one, which is why a React backdrop discarding unsaved edits was
missed on the first pass of Phase 7d item 1. That set should grow a React layer.

## 10. Rollback

One `git revert` of the code commit restores all four call sites; the `R-DUST` commit reverts
independently. No schema, no migration, no persisted state.

---

## Sign-off

- [x] Every evidence row GREEN **and** its RED observed
- [x] `scripts/preflight.ps1 -Full` run — result + date recorded below
- [x] `scripts/preflight.ps1 -NegativeControl` run — every T0 gate seen to fail
- [x] `cargo test` full pass (**not** `cargo build --release`)
- [ ] `../REGRESSION_SET.md` run in full at this boundary — ⬜ **OWED at the phase boundary**
- [x] Adversarial review — ✅ **DONE 2026-09-08**, `ADVERSARIAL_REVIEW.md`. Found `F1` (an overclaim in
      R-DUST, now corrected) plus three minor items. ⛔ Its own limitation is declared up front: written
      by the session that wrote the code, which HARNESS §6 does not consider sufficient
- [x] Commit messages cite the row IDs they satisfy

| Item | Result | Date | By |
|---|---|---|---|
| preflight -Full | **PASS** — 8/8 static gates at baseline (G11 60, unmoved), T1a–T1g PASS, **T1d ran** | 2026-09-08 | Claude |
| preflight -NegativeControl | **PASS** — all 11 gates seen to fail on injection | 2026-09-08 | Claude |
| cargo test | **PASS** — 458 lib + 526 bin + 16 integration binaries, 0 failed | 2026-09-08 | Claude |
| A1 production-code negative control | **RED OBSERVED** — floor deleted ⇒ `left: 21, right: 20`; restored ⇒ green | 2026-09-08 | Claude |
| regression set | ⬜ OWED | | |
| adversarial review | 🟡 **DONE** — `ADVERSARIAL_REVIEW.md`; found `F1` (overclaim), corrected. ⛔ Same-session, not independent | 2026-09-08 | Claude |

### Residuals — carried, not rounded up

1. ✅ **`R-DUST` landed in `REGRESSION_SET.md`** (`b885875`, separate commit per working rule 6), and
   its title was corrected 2026-09-08 after the adversarial review — see item 2.
2. 🟡 **Adversarial review DONE** (`ADVERSARIAL_REVIEW.md`) — but by the same session that wrote the
   code, which §6 does not consider sufficient. ⭐ An independent pass is still worth commissioning.
   It found `F1`: `R-DUST` overclaimed ("no path") when a fifth path — `create_action`'s `user_inputs`
   — exists and is deliberately uncovered. Invariant and §5 corrected; no production change.
3. ⬜ **Regression-set boundary run owed.**
4. ⚠️ **No live-wallet run, by design** (§4). The floor is proven at the predicate and selection
   layer, not against a real 20-UTXO consolidation on chain.
5. ⛔ **This is a floor, not the fix.** Nothing files a token into a basket; `A7` measures exactly
   that. beta.4 sprint 1 is the answer.
6. ⚠️ **`D-5` follow-ups untouched and unticketed**: `is_p2pkh_script`'s comment overstates what it
   guards (now corrected in-place, but the underlying issue stands), and address-sync stores a
   **fabricated** locking script for every discovered output — including anything the GorillaPool
   *ordinals* endpoint returns. Spending such an output would produce an invalid signature and a
   rejected broadcast: a failure, not a loss. **Worth a ticket; not opened here.**
7. ⚠️ **`send_max` leaves 1 satoshi behind** where a carrier is held — the accepted cost of `Q-1`.
   Owner noted a possible future "send absolutely everything" affordance; deliberately not built.
