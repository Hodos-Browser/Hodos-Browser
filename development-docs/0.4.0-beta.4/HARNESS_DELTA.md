# beta.4 harness — delta only

**Opened:** 2026-08-29. **Decision (owner, kickoff Q2):** beta.4 **inherits** beta.3's harness by
reference and extends it here. This file is **not** a harness.

⛔ **The standard is `../0.4.0-beta.3/HARNESS.md`.** Read it first. Nothing in it is restated here.
⛔ **Do not edit `0.4.0-beta.3/`** — another session owns that folder.

> **Why reference and not fork.** `HARNESS.md` §9 is a **gate baseline registry**, and baselines are
> *measured state*, not prose. beta.3 is still lowering them. Two copies of a measurement diverge
> without anyone noticing, and a stale baseline is a gate that passes for the wrong reason — the
> exact failure family §9 was written to catch.
>
> **Where this ends up.** Both files fold into a version-neutral `development-docs/HARNESS.md`
> **after beta.3 closes.** Not before — the move would edit beta.3's folder.

---

## 1. What beta.4 adds to the standard

### 1.1 ⛔ Fail closed — the release's load-bearing rule

`HARNESS.md` is built on *provable falsifiability and provable subject*. beta.4 adds one rule of the
same weight, because this release handles assets with **no undo**:

> **An output the wallet cannot classify is not spendable.**
> "We could not tell, so we spent it" destroys a user's asset permanently.

Consequences for evidence tables in this release:

- A row asserting *"tokens are protected"* is void unless its RED half shows the **unclassified**
  case being refused. Protecting the case you can classify is the easy half.
- ⭐ The **SUBJECT column gets a new obligation for this release:** name the *output*, not just the
  process. Which txid, which vout, which satoshi value, which basket. A guard test that never had a
  real 1-sat output in the wallet proves nothing — and is precisely the shape of the three void
  farbling harnesses recorded in `HARNESS.md`.

### 1.2 Destructive tests need a real subject and a scratch profile

The paths under test in track 1 **destroy assets** when they work as currently written. That is the
point of testing them.

- ⛔ Never against the production profile. `REGRESSION_SET.md`'s existing rule ("where a RED is
  destructive, use a scratch profile") is **not optional** in this release — it is the default.
- The negative control for the guard is *deliberately destroying a test asset*. Budget for staging
  real 1-sat outputs, and record their outpoints in the contract.

### 1.3 Measurement rows — a new obligation on track 2

Track 4's central question is answered with **track 2's data**. So track 2's contracts carry rows
whose result is a **number**, not a pass:

| Obligation | Why |
|---|---|
| Record real BRC-150 provenance row sizes (`beefB64`) as they land | Track 4 measures against them. Retrospective measurement is guesswork with better manners. |
| Record ancestry depth, not just byte size | The stated hypothesis is that depth, not media, is the size problem. A hypothesis needs the variable it names. |

⚠️ `HARNESS.md` §8 already says *do not record a cause you have not reproduced*. A measurement row
with no number is **INCOMPLETE**, not green.

## 2. Test tiers — unchanged, with one note

Tiers **T0–T4** are as defined in `HARNESS.md` §3. No new tier.

⚠️ **T3 (human observational) is where this release is weakest**, and beta.3's own boundary record
shows why: `R-GOLD`, `R-CLOSE` and `R-COUNT` sat "not run" across multiple boundaries because each
needs a person and a real payment. beta.4 adds token operations that have the same property.
**Say so in the contract rather than discovering it at sign-off** — `HARNESS.md` §8's rule that a
skipped check is SKIPPED and the run INCOMPLETE applies unchanged.

## 3. Static gates — none added yet

No new T0 gate is proposed at telescope time, deliberately. A gate written before the code it guards
exists is a gate whose baseline is a guess.

**Candidate**, for the microscope pass to accept or reject with the code in front of it:

| Candidate | What it would catch | Decide at |
|---|---|---|
| A gate on **UTXO-selecting code paths that do not consult the classification** | The track-1 failure recurring silently in a path added later | Track 1 microscope |

⛔ If it is added: **baseline it with `preflight.ps1` itself**, never a hand grep — `HARNESS.md` §9
records two gates whose hand counts were wrong (`G2` 6→5, `G5` 11→15), and that is the stated reason
for the rule.

## 4. ⏳ Owed — the harness expansion

The owner intends to expand the harness; **not now**. Three research tasks were dispatched
2026-08-29 to feed it. Their outputs land in `research/`:

| Task | File | Feeds |
|---|---|---|
| (a) PRD/spec-authoring skills — what to adopt | `research/RESEARCH_a_prd_skills.md` | `../SCOPING_PROCESS.md` |
| (b) The Karpathy method — what to lift, and where it belongs | `research/RESEARCH_b_karpathy_method.md` | `../SCOPING_PROCESS.md` + root `CLAUDE.md` |
| (c) Test harness / regression / CI best practice | `research/RESEARCH_c_testing_practice.md` | **this file, and the eventual merged `HARNESS.md`** |

**All three delivered 2026-08-29.** The proposed adoption list — ranked, with costs — is
`../SCOPING_PROCESS.md` §7b and §7b-ii. It is **one decision per item**, and none is in force.

⛔ **Research is advisory until read and decided on.** Nothing in it changes the standard by existing.
Adopting an item is a decision recorded here with a reason, the same as lowering a baseline.

### The two items from (c) that bear on beta.4 directly

| Item | Why it matters to this release |
|---|---|
| ⭐ **`cargo-mutants --in-diff`** on `hodos_permission_engine` | Track 2.3 adds a token-spend permission class to that crate. Mutation testing there is the mechanised form of the hand negative control, on the code where a false green costs the most |
| ⭐ **Split the standing set into R-EVERY (automatable) / R-RELEASE (human, timeboxed, scheduled)** | `R-GOLD`, `R-CLOSE` and `R-COUNT` have been *"owed, not waived"* at every beta.3 boundary because four of six rows need a human and **nobody budgeted the minutes**. beta.4 adds four more rows, three of which need a real wallet and real outputs. ⛔ **Without the split, beta.4 inherits the same debt and doubles it** |

⚠️ The second one is a change to beta.3's file, which this session cannot make. It is recorded here
as the recommendation for the merged `HARNESS.md` after beta.3 closes — and as a warning that
`REGRESSION_ADDITIONS.md`'s boundary table will collect the same blanks if nothing changes.

---

## Change log

| Date | Change |
|---|---|
| 2026-08-29 | Opened. Fail-closed rule, destructive-test rule, measurement obligation on track 2. No new tiers, no new gates. |
