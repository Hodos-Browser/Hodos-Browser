# beta.4 additions to the standing regression set

**Opened:** 2026-08-29. **Extends:** `../0.4.0-beta.3/REGRESSION_SET.md` — which is the standard and
is **not restated or forked** here (kickoff Q2).

⛔ **Run beta.3's set in full at every beta.4 phase boundary, plus these.** The additions do not
replace anything.
⛔ **Do not edit `0.4.0-beta.3/`.**

> Same rule as beta.3: **a check that has never been seen to fail is not a check.** Every row below
> carries its own RED. Where a RED is destructive — and in this release most of them are — use a
> **scratch profile**, never the production one.

---

## R-NOSPEND — no automatic path spends an unclassified output ⭐

**The load-bearing one for this release.** It is the invariant the whole guard sprint exists to
create, and it must outlive that sprint.

This row was proposed by `0.4.0-beta.3/TICKET_token_outputs_destroyed_by_dust_paths.md` as an
invariant that *"should outlive this ticket and hold through the beta.4 guard work"*. This is where
it lives.

**The mechanism, so the check tests the right thing:** the discriminator is **not** the satoshi
value. A 1-sat floor is the beta.3 stopgap, not the invariant. The invariant is
**classification**: an output is spendable only if the wallet has positively classified it as
spendable. `Unknown` is refused. Exclusion already works
(`database/output_repo.rs:98,148` — `basket_id IS NULL OR b.name = 'default'`); the thing under
test is whether classification reaches every path that selects UTXOs.

| | |
|---|---|
| **GREEN a** | The dust consolidator runs against a wallet holding 20+ dust UTXOs **including one unclassified/token output**, consolidates the ordinary dust, and leaves the token output **unspent** |
| **GREEN b** | The recovery sweep scans an address holding a token output plus ordinary coins, and the sweep **excludes** the token output — and **reports** it rather than silently omitting it from a headline balance that implies it is spendable |
| **GREEN c** | Ordinary coin selection (`select_utxos_with_preference`) never reaches for it, at any `dust_threshold_sats` |
| **RED** | ⛔ **Revert the guard, re-run each, and observe the output *consumed*.** If it survives without the guard, the test is not exercising the path and the green is void. Observe **per path** — three reds, not one. |
| **SUBJECT** | The **output itself**: txid, vout, satoshi value, classification, basket. Named in the contract before the run. A guard test that never had a real token output in the wallet proves nothing. Also name **which** run: the consolidator's timer fire, not a hand-called function. |
| **Tier** | T1 (classification logic) + T2 (real paths, real wallet, scratch profile) |

⚠️ **Path 1 is on a 24-hour timer** (`monitor/mod.rs:79`, `consolidate_dust: 86400`) and skips its
first tick (`mod.rs:183`). A test that only calls the task function directly does **not** prove the
scheduled path is guarded. State in the contract which one you ran, and do not round one up to the
other.

## R-CLASSIFY — classification reaches every ingest path

`R-NOSPEND` proves outputs are not *spent*. This proves they are *classified in the first place* —
the gap identified 2026-08-29 as the real defect.

| | |
|---|---|
| **GREEN** | An output arriving by **each** ingest route is classified, and the classification is persisted |
| **RED** | Introduce an output by a route that bypasses the classification point → it must land as `Unknown` and therefore **unspendable**, not as spendable-by-default. **The failure mode under test is a silent default, so the red is "prove the default is refusal."** |
| **SUBJECT** | Name every ingest route the run covers, and every route it does **not**. ⚠️ `grep` finds **zero** `basket` references in `recovery.rs`, `reconcile.rs`, `utxo_fetcher.rs` — those three are the routes most likely to bypass, and a run that skips them has proven nothing about the dangerous half |
| **Tier** | T1 + T2 |

⛔ **The complete route list is sprint 1.1's deliverable.** Until it exists, this row cannot be
honestly signed off — it can only be run against a route list someone believes is complete. Say which.

## R-RESTORE — fail-closed survives recovery

The sharpest cross-sprint edge in the release: sprint 1's fail-closed rule meets sprint 4's recovery
path. **Neither sprint owns it alone.**

| | |
|---|---|
| **GREEN a** | After restore-from-seed, an output that cannot be classified is **not spendable** |
| **GREEN b** | It is also **not lost** — it appears to the user as held-but-unclassified, not omitted |
| **RED** | Each half is the other's control. Force everything to classify as `Spendable` → (a) must fail. Force everything to `Unknown` → (b) must still show the outputs. **A restore that silently drops what it cannot classify passes (a) and fails the user.** |
| **SUBJECT** | The **restored** wallet's DB state, not the pre-backup state. Recovery is the path where a user is least able to notice a loss |
| **Tier** | T2 |

⏳ **Not runnable until sprint 4.** Recorded now so it is not invented late, and so sprint 1 knows the
shape its rule must survive.

## R-TOKENPERM — pay grants do not authorize token spends

⛔ **BRC-147: pay and auto-pay grants MUST NOT authorize ordinal spends.** This is a spec
requirement, and it sits on the same surface as the four privacy-perimeter gates.

| | |
|---|---|
| **GREEN a** | A domain with a pay / auto-pay grant attempts a token spend → **prompts**, regardless of amount or limits |
| **GREEN b** | The approval modal states **which baskets and sub-categories** the grant covers |
| **RED** | Grant the domain the broadest pay permission the UI allows, then attempt the token spend. It must **still** prompt. If a sufficiently broad pay grant ever silences it, that is a **defect, not a setting** — same standing as sensitive certificate fields in `R-PERIM` |
| **SUBJECT** | The **Rust decision** (`PermissionDecision` kind + reason), not the modal's appearance. A modal that renders is not proof the engine decided to prompt — `R-PERIM`'s rule, applied here |
| **Tier** | T1 (engine) + T2 (end-to-end) |

⏳ **Not runnable until sprint 2.3.** Its shape is fixed now because sprint 2 must build to it rather
than discover it.

---

## Boundary run record

Fill at every beta.4 phase boundary. ⛔ A blank cell is **not** a pass — `HARNESS.md` §8: a skipped
check is SKIPPED and the run is INCOMPLETE.

| Phase boundary | Date | R-NOSPEND | R-CLASSIFY | R-RESTORE | R-TOKENPERM | beta.3 set in full |
|---|---|---|---|---|---|---|
| *(pre-sprint-1 baseline)* | | ⬜ expected **RED** — the guard does not exist yet | ⬜ n/a | ⏳ not until S4 | ⏳ not until S2.3 | ⬜ |
| S1 → S2 | | | | ⏳ | ⏳ | |
| S2 → S3 | | | | ⏳ | | |
| S3 → S4 | | | | ⏳ | | |
| S4 → RC | | | | | | |

⭐ **Run `R-NOSPEND` once before sprint 1 starts and record it as RED.** A guard whose regression row
was never seen failing beforehand has no baseline, and "it passes now" would be unfalsifiable. This
is the cheapest negative control in the release and it is available today.

---

## Not added, and why

| Considered | Verdict |
|---|---|
| A row asserting "ordinals display correctly" | Not an invariant — a feature. Belongs in sprint 2's evidence table, not the standing set. The standing set is for what must not break *later*. |
| A row on fungible/BSV-21 handling | ⛔ Nothing to guard. Fungibles are deferred — see `WATCH_fungibles.md`. Adding a row for absent behaviour is how a set rots. |
| A 1-satoshi value floor as a standing invariant | Rejected. That is beta.3's stopgap. Writing the stopgap into the standing set would freeze the wrong rule: **value is not the discriminator, classification is.** |
