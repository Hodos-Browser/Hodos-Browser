# Phase 8 (dust guard) — adversarial review

**Run 2026-09-08**, against `b45cb90`. Standard: `../HARNESS.md` §6 — a pass whose job is to
**refute**, four questions answered in writing.

> ⛔ **Declared limitation, up front.** §6 asks for a pass "by someone or something that did not
> write the code." **This was written by the session that wrote the code.** That is a real weakening
> and it is not hand-waved away by trying hard. What it can still do — and did — is run experiments
> whose outcome the author does not control. The finding in `F1` came out of one of those, not out
> of re-reading. ⭐ An independent pass is still worth commissioning; this does not substitute.

---

## Q1 — Can I make these tests pass with the feature removed?

**No, for every test that claims the floor holds. Demonstrated, not argued.**

The feature has a single point of control, so it can be disabled with a one-line edit that no test
knows about: `TOKEN_RESERVED_SATS: i64 = 1` → `0` makes `is_token_reserved_value(1)` return `false`
and turns the floor off everywhere at once.

| Result with the constant set to `0` | Count |
|---|---|
| 🔴 **Tests asserting the floor — FAILED** | **11** |
| ✅ `without_the_floor_*` controls — still green (they assert the *unguarded* behaviour, so this is correct) | 5 |
| ✅ `token_reserved_exposure_tests` (×3, `A7`) — still green | 3 |
| ⚠️ `candidate_predicate_is_stable_across_repeated_evaluation` — still green | 1 |

Restored to `1`: **458 lib + 526 bin, 0 failed.**

⭐ This is stronger than the per-site control run at implementation time (`A1`, `left: 21, right: 20`),
because it disables the *whole feature* from one place rather than one site at a time, and it cannot
be satisfied by a test that merely re-derives the predicate.

**`A7`'s three staying green is correct and worth stating:** they measure the *ingest* behaviour —
that a 1-sat payment becomes a spendable `basket_id IS NULL` row — which is true with or without the
floor. That is the defect the floor works *around*, not the floor itself. Filed under the right
question, they are not evidence for `R-DUST` and are not counted as such.

### 🔴 `F2` — one test cannot fail with the feature removed

`candidate_predicate_is_stable_across_repeated_evaluation` asserts
`is_consolidation_candidate(&o) == is_consolidation_candidate(&o)`. It is a determinism check. It
stayed green under the global disable **because it does not test the guard**, and by §6 Q1 that means
it is not evidence for anything. It is not harmful, but it pads the count.

⇒ **Action:** it stays (it is free), but the phase contract's "19 tests" figure must not be read as
19 pieces of evidence. **Eleven** tests fail when the feature is removed. That is the honest number.

---

## Q2 — What is the subject?

| Claim | Subject | Proven how |
|---|---|---|
| The floor holds | The **real** functions in the **real** crates — `is_consolidation_candidate`, `select_utxos_greedy`, `select_all_spendable`, `split_token_reserved`, `build_sweep_transactions`. No mocks, no reimplementations | The global disable in Q1 moves them; a reimplementation would not |
| `A3` excludes the carrier | The **serialised transaction** — input-count byte `raw[4]` and a byte search for the carrier's little-endian outpoint | The test also asserts an *ordinary* outpoint **is** found by the same search, so "absent" cannot come from a broken search |
| `A7` exposure | A seeded SQLite DB driven through the **real** `upsert_received_utxo_with_confirmed` | Not a hand-written INSERT — the real ingest function |

⛔ **What the subject is NOT, and this bounds every claim above:** no live wallet, no real dApp, no
on-chain transaction, no consolidation actually executed. The floor is proven at the
predicate/selection layer only. A defect between "the selector excluded it" and "the broadcast tx
lacks it" would not be caught here.

---

## Q3 — What would I expect to see if this were broken, and did anyone look?

Both directions were checked, which matters — a floor can fail by being too weak *or* too strong.

| Failure direction | Expected symptom | Looked? |
|---|---|---|
| Too weak | A 1-sat output in a candidate set / selection / tx inputs | ✅ every `A1`–`A6` green half |
| **Too strong** | A 2-sat output withheld; a wallet unable to spend ordinary dust | ✅ four dedicated 2-sat tests, one per site |
| Silent no-op | Consolidation adds nothing at all, so "carrier absent" is trivially true | ✅ `consolidation_pass_does_not_sweep_up_the_carrier` asserts an ordinary small output **is** still consolidated |

### 🚨 `F1` — and here is what nobody looked for: **a fifth spend path**

I enumerated every `tx.add_input` / `TxInput::new` site in the crate rather than trusting the
ticket's list of three. That surfaced a path the ticket, the contract, and `R-DUST` all miss:

**`create_action`'s `user_inputs` — a dApp names an outpoint directly, and it never touches the
selector.**

- Accepted from the BRC-100 `createAction` request (`handlers.rs:4808-4886`); the caller supplies an
  outpoint plus either the source tx in `inputBEEF` or `inputSatoshis`.
- Added to the transaction at **`handlers.rs:5366-5381`**, *before* wallet inputs, with **no value
  check of any kind**.
- If no `unlockingScript` is supplied the wallet logs *"will sign later"* and **signs it itself** —
  `sign_action` (`:7494`) looks the outpoint up in our DB, derives the key, and signs
  (`:7600-7740`).

⇒ **A dApp can name one of our 1-satoshi outputs and the wallet will sign it away.** The Phase 8
floor does not touch this path.

**Is leaving it uncovered wrong?** ⛔ **No — blocking it would be wrong.** A deliberate ordinal
transfer *is* a dApp naming a 1-sat outpoint; that is precisely what beta.4 sprint 2 must be able to
do. A blanket refusal here would make the feature unimplementable.

**What IS wrong is the claim.** `R-DUST` is titled *"no path may spend a 1-satoshi output"* and its
GREEN row enumerates four paths. The title asserts something broader than the row proves, and the
gap is not named anywhere. That is the overclaim pattern this sprint keeps catching in other
people's documents.

⚠️ **And the missing gate is exactly the one BRC-147 names.** Rule 2, already quoted in
`0.4.0-beta.4/sprint-2-1sat-ordinals/README.md`: *"a general 'pay' or auto-pay grant **MUST NOT**
authorize spending them"* — and *"this must be enforced in the Rust permission engine, not just the
UI."* Today `hodos_permission_engine` has no notion of a token-carrying input, so a site holding a
payment grant is not stopped from naming one. **That enforcement is beta.4 sprint 1 work**, not a
beta.3 floor.

⇒ **Actions:** (a) correct `R-DUST`'s wording so the claim matches the row and the uncovered path is
named; (b) add `user_inputs` to `R-CLASSIFY`'s route list in beta.4; (c) record it in the phase
contract's blast radius as considered-and-excluded, with the reason.

### 🟡 `F3` — balance and spendability now diverge (noted, not a defect)

`calculate_balance` (`output_repo.rs:281`) sums every spendable output with **no value filter**, so
1-satoshi outputs still count toward the displayed balance while no longer being spendable.
Consequence: **"Send max" now leaves a non-zero balance behind** — 3 ordinals ⇒ 3 satoshis remain.

Not filed as a defect: the same file already documents a deliberate divergence of exactly this shape
(nosend outputs are included in the balance but excluded from selection), and for a *token* the
divergence is arguably correct — you do still hold the asset. ⚠️ But it is a UX edge the owner
should know about, and it is the natural place a "send absolutely everything" affordance would
surface later.

### 🟡 `F4` — the contract's blast radius was incomplete

§5 listed the four call sites of `select_utxos_greedy` but did not mention `user_inputs` at all —
not even to exclude it. A reviewer reading §5 could not tell whether I had looked. The section is
supposed to be "specific enough that a reviewer can check you looked"; on this point it was not.

---

## Q4 — Is each claim a measurement or a code reading?

| Claim | Label | Correct? |
|---|---|---|
| The floor holds at four sites | **Measurement** (11 tests flip under the global disable) | ✅ |
| A 1-sat payment becomes a spendable default-pool row | **Measurement** (`A7`, seeded DB, real ingest fn) | ✅ |
| Real inscription script = 2,596,810 B; OrdLock = 860 B; WoC and GorillaPool return disjoint sets | **Measurement** (mainnet, 2026-09-08, reproducible outpoints) | ✅ |
| `task_sync_pending` ingests within 30 s with no value filter | **Code reading**, and labelled as such in the contract's `D-2` table | ✅ — the *consequence* is measured by `A7`; the timing is not |
| Non-1-satoshi outputs could carry a wrong script and break transactions | **Code reading**, labelled in the D-5 ticket | ✅ |
| `user_inputs` is signed by the wallet (`F1`) | **Code reading** — traced through `:5366` → `:7494` → `:7600`. ⛔ **Not executed.** No dApp was driven | ⚠️ Labelled here |

⭐ No claim is mislabelled. `F1` is the one new code reading and it is marked as such rather than
promoted — the sprint's recurring failure is exactly that promotion.

---

## Verdict

**The implementation holds. The claim about it did not.**

The code does what the phase said, at the four sites it named, and survives a whole-feature disable
that no test could anticipate. `F1` is a defect in the **invariant's wording and the contract's blast
radius**, not in the shipped behaviour — and leaving `user_inputs` unguarded is the *correct*
engineering choice, because guarding it belongs in the permission engine and would otherwise make
beta.4 sprint 2 impossible.

| ID | Finding | Severity | Action |
|---|---|---|---|
| `F1` | `R-DUST` says "no path"; a fifth path (`user_inputs`) exists and is deliberately uncovered | 🔴 **overclaim** | Reword `R-DUST`; add the route to `R-CLASSIFY`; record in §5 |
| `F2` | One test cannot fail with the feature removed | 🟡 evidence hygiene | Keep it; report **11**, not 19, as the evidence count |
| `F3` | Balance includes 1-sat outputs that can no longer be spent | 🟡 note | None. Owner aware |
| `F4` | Contract §5 omitted `user_inputs` even as an exclusion | 🟡 doc | Add it |

⛔ **No production-code change is proposed by this review.** Per CLAUDE.md invariant #13, the
evidence points at the *documentation* being wrong, not the code — so the doc is what changes.
