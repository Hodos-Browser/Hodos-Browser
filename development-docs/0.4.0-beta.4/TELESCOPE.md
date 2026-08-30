# beta.4 telescope output — how the sprints interact, and how to run the microscope pass

**Written:** 2026-08-29, telescope pass. **Read by:** the microscope sessions, before they start.
**Companions:** `README.md` (scope, settled decisions), `SPRINT_PLAN.md` (sprint breakdown).

> **What a telescope output is for.** Not a plan — the plan is `SPRINT_PLAN.md`. This file carries
> the two things only a long-distance look can produce: **the places where sprints touch each other**,
> and **how to divide the microscope work across contexts so those places are not lost.**

⛔ **Hard rule for every microscope session:** the sprints are separated so their contexts can be.
A session that reads all four sprint folders has recreated the problem this pass exists to solve.

---

## 1. The four cross-sprint edges

Everything inside one sprint is that sprint's problem. These four are nobody's, which is why they get
named here or not at all.

### ⭐ E1 — S1 → S2: the classifier must match BRC-165 normalization

Sprint 1 writes a classifier before sprint 2 reads the spec that says what a classified ordinal
*is*. If sprint 1 invents its own basket naming, sprint 2 rewrites it — and the rewrite lands after
the guard is already the thing everything trusts.

| | |
|---|---|
| **Cheapest mitigation** | Sprint 1's microscope reads **BRC-147 + BRC-165 normalization rules** before choosing what "classified" persists as. Nothing else from sprint 2. |
| **Owner** | Sprint 1 microscope |
| **Failure if missed** | A migration in sprint 2, on the one data structure the release's safety depends on |

### ⭐ E2 — S2 → S4: sprint 4's central question is answered with sprint 2's data

Sprint 4 asks *what actually needs backing up vs what can be re-fetched*. Its stated hypothesis — the
size problem is **`beefB64` ancestry depth**, not inscription media — is a hypothesis, and the data
that settles it is created during sprint 2.

| | |
|---|---|
| **Cheapest mitigation** | Sprint 2 carries **measurement rows** (`HARNESS_DELTA.md` §1.3): real provenance-row sizes **and ancestry depth**, recorded as they land |
| **Owner** | Sprint 2 microscope writes the rows; sprint 4 consumes them |
| **Failure if missed** | Sprint 4 measures retrospectively or assumes. ⛔ `HARNESS.md` §8: *do not record a cause you have not reproduced* |

### ⭐ E3 — S1 → S4: fail-closed must survive recovery

The sharpest edge in the release. Sprint 1 says an unclassified output is not spendable. Sprint 4
owns restore-from-seed, which is where classification is least likely to be available and where a
user is least able to notice a loss.

Two failures, and **a fix for one is the other**:

| Failure | Shape |
|---|---|
| Silently spendable | Restore defaults unknown outputs to spendable → sprint 1's rule evaporates at the one moment it matters most |
| Silently lost | Restore drops what it cannot classify → the output is safe from spending and gone from the user |

| | |
|---|---|
| **Cheapest mitigation** | `R-RESTORE` in `REGRESSION_ADDITIONS.md` — **two GREENs that are each other's control.** Written now, run in sprint 4 |
| **Owner** | Neither sprint alone ⇒ **an owner decision point**, §4 below |

### E4 — S2 → S3: does a name reuse the token-spend permission class?

An OpNS name is a 1-sat output, so the default answer is "reuse sprint 2.3's class". Default answers
about permissions are how permission surfaces quietly widen.

| | |
|---|---|
| **Cheapest mitigation** | Sprint 3's microscope **verifies** rather than assumes, and says which |
| **Owner** | Sprint 3 microscope |

---

## 2. Context strategy — recommended structure for the microscope pass

**Five contexts. Four are independent. One is shared and comes first.**

```
                    ┌──────────────────────────────────┐
                    │  M0 — the edge context (FIRST)   │
                    │  Resolves E1 · E3. Small.        │
                    └───────────────┬──────────────────┘
                                    │ its output is an input to M1 and M4
        ┌───────────────┬───────────┴───────┬───────────────┐
        ▼               ▼                   ▼               ▼
    ┌───────┐       ┌───────┐           ┌───────┐       ┌───────┐
    │  M1   │       │  M2   │           │  M3   │       │  M4   │
    │ Guard │       │ 1Sat  │           │ OpNS  │       │Backup │
    └───────┘       └───────┘           └───────┘       └───────┘
     fresh           fresh               fresh           fresh
```

### M0 — the edge context. Run this first, and keep it small.

> **In plain terms:** before splitting the work across four separate sessions, run **one short
> session** that settles the two questions those four would otherwise each answer differently — what
> "this output is a token" gets saved as (**E1**), and what restore does with an output it cannot
> identify (**E3 / decision D-1**). Four sessions answering these independently produce four
> incompatible answers. `RESUME_beta4.md` §3 states both in non-technical language.

| | |
|---|---|
| **Job** | Settle **E1** (what "classified" persists as, against BRC-147/165) and produce the **E3** decision brief for the owner |
| **Reads** | `README.md`, this file, `REGRESSION_ADDITIONS.md`, BRC-147 + BRC-165, `output_repo.rs`, `basket_repo.rs`, `domain_permission_repo.rs` |
| **Does NOT read** | Any sprint folder in full. `IMPLEMENTATION_PLAN.md`. `RESEARCH_FINDINGS.md` |
| **Output** | One short doc: the persistence decision with its reason, and the E3 question stated for the owner |
| **Exit** | That doc exists and the owner has answered E3. **Nothing else.** |

⭐ **Why M0 exists at all.** E1 and E3 are the only decisions that are *cheaper to make once* than to
make twice. Everything else genuinely belongs inside a sprint. Resist the urge to grow M0 — a context
that owns "the cross-cutting concerns" becomes the context that owns everything.

### M1–M4 — one per sprint, fresh, and deliberately blinkered

| Context | Reads | ⛔ Does **not** read |
|---|---|---|
| **M1 Guard** | `sprint-1-utxo-safety-guard/README.md`, M0's output, `HARNESS_DELTA.md`, `REGRESSION_ADDITIONS.md`, the `rust-wallet` paths named in the sprint README | Sprints 2/3/4 folders. `WATCH_fungibles.md` beyond its one-line verdict |
| **M2 1Sat** | `sprint-2-1sat-ordinals/` (both files), M0's output, BRC-147/150/159/160/165, `WATCH_fungibles.md` | Sprints 3/4 folders. The old naming research |
| **M3 OpNS** | `sprint-3-opns-naming/README.md`, `tokens/0174.md`, `Future-Features/Decentralized-Naming/` **once**, sprint 2's *outcomes* (not its folder) | Sprints 1/4 folders. The naming research a second time |
| **M4 Backup** | `sprint-4-onchain-backup-sync/` (the plan is authoritative), M0's E3 answer, sprint 2's **measurement rows** | Sprints 1/2/3 folders |

**Each M-context produces:** phase contracts from `PHASE_CONTRACT_TEMPLATE.md`, and a short
**findings note** listing anything that contradicts this telescope output. The findings notes are the
input to the closing telescope pass.

⛔ **M4's plan is not open for redesign.** `IMPLEMENTATION_PLAN.md` carries 8 phases, D1–D15, two
adversarial reviews and owner sign-offs. M4 **reconciles** it against what sprints 1–3 built. If a
sprint outcome breaks a D-decision, that is a **finding to surface**, not a licence to re-plan.

### Where the microscopes genuinely cross over — use a sub-loop, not a shared context

Only two places, and both are **narrow queries**, not merged contexts:

| Crossover | Handle it as |
|---|---|
| M2 needs to know what M1's classifier produces | A **question to M1's output doc**, not a read of M1's context. If the doc cannot answer it, the doc is incomplete — fix the doc. |
| M4 needs sprint 2's measured sizes | A **data handoff** — the measurement rows. Numbers, not narrative. |

⭐ **The rule:** if one microscope needs another's *reasoning*, that reasoning belongs in a document
neither of them owns. If it needs another's *result*, hand over the result. Merging contexts to share
understanding is how a four-sprint release becomes one un-reviewable plan.

---

## 3. Where a workflow earns its cost

Following `HARNESS.md` §6 — fan-out is for **breadth**, not ceremony. The default is one agent.

| Where | Workflow? | Why |
|---|---|---|
| M0 — the edge context | ❌ | Two decisions and a spec read. One agent, one document. |
| **S1.1 — enumerate the surface** | ✅ **call-site sweep** | The deliverable *is* completeness across a codebase. Exactly the shape `HARNESS.md` §6 already blesses for the routing predicate. **Fan out by path family** (monitor, recovery, reconcile, handlers, repos) — one agent per family, blind to the others, then reconcile the lists. Disagreement between agents is signal. |
| S1.2–1.4 — the implementation | ❌ | Bounded, and the evidence is a test with a real output in a scratch profile. |
| **S2.3 — token-spend permission class** | ✅ **adversarial panel** | A permission boundary. `HARNESS.md` §6 already routes security boundaries to distinct lenses, and BRC-147's MUST NOT makes this the same class as the privacy-perimeter gates. |
| S2.1/2.2/2.4 | ❌ | Spec implementation against merged, coherent BRCs. |
| **S2.6 — BSV-20/21 review** | ✅ **research fan-out** | It is a research phase by definition, and its gate question is answerable from the ecosystem, not our code. Cheap, parallel, and the output is findings. |
| S3.1–3.3 | ❌ | Bounded. First-implementation friction, not breadth. |
| **S3.4 — the overlay PoC** | ⚠️ **only if it targets §10.1 economics** | If it produces numbers, an independent verification lens is worth it. If it is a demo, one agent. |
| S4 | ❌ | The plan already absorbed **two** adversarial reviews. A third is ceremony. |
| **Every phase boundary** | ✅ **regression sweep** | Independent, parallel, cheap — unchanged from beta.3. |

⛔ **Note what is not on this list.** No workflow for architecture exploration, option comparison, or
"considering approaches". Those are the owner's named risk, and a fan-out is the most expensive
possible way to have that conversation. **When two options are close, put both in one message with a
recommendation and ask.**

---

## 4. Human decision points — required, not courtesy

Each of these **stops the pass** until answered. Each is stated as one question with a recommendation.

| # | Decision | Blocks | When |
|---|---|---|---|
| **D-1** | **E3** — on restore, does an unclassifiable output present as held-but-unclassified, or is restore blocked until it can be classified? | S1's rule shape, S4's recovery UX | **M0**, before M1 starts |
| **D-2** | The **exposure question** — does an ordinary incoming 1-sat payment become a tracked default-basket row without a recovery scan? | Sizing, not design. Answer by **experiment** | M1, early |
| **D-3** | **Is the OpNS overlay PoC in the release**, or a parallel public artifact? | S3's size | Before M3 |
| **D-4** | **S2.6's gate question** — do wallets need BSV20/21 code at all? | Whether 2.6 stays a review | M2, at 2.6 |

⚠️ **D-2 is answered by running something, not by reading more code.** It has already been read twice.

---

## 5. Loop limit and exit conditions

The scoping process must scope itself. For beta.4 specifically:

| | |
|---|---|
| **Microscope loop limit** | ⛔ **Two passes per sprint.** If a second microscope pass on the same sprint has not produced signable phase contracts, that is not a planning problem — **stop and build the smallest testable piece.** |
| **M-context exit** | Phase contracts exist for the sprint's sub-sprints, every evidence row has a non-empty RED and SUBJECT, and the findings note is written. |
| **Closing telescope exit** | All four findings notes read; `SPRINT_PLAN.md` and this file amended where they broke; and an explicit statement of **what did not change** — silence is not confirmation. |
| **Loop back into a microscope** | ⛔ Only when a finding **invalidates a cross-sprint edge**. Not for detail, not for polish, not for a better idea. |

---

## 6. What this telescope pass believes, and how it could be wrong

Written down so the closing pass has something to check rather than a mood to match.

| # | Belief | How it fails |
|---|---|---|
| 1 | The classification seam is cheap because exclusion already works | 1.1's sweep finds selection paths that bypass `output_repo` entirely. **Then sprint 1 is bigger than scoped** — the most likely way this plan breaks |
| 2 | Ordinals are a prerequisite for names | BRC-174 turns out not to need ordinal handling at all, and sprint 3 could have run earlier |
| 3 | The backup size problem is ancestry depth, not media | **Explicitly a hypothesis.** E2's measurement settles it. If media dominates, sprint 4's phase 3 changes shape |
| 4 | Four sprints fit in one release | The first honest phase-contract set says otherwise. **Cut from the back** — sprint 4's plan already stands alone and can slip to beta.5 without waste |
| 5 | Fungibles can be deferred without cost | A partner or user need forces BSV-21 mid-release. Trigger 5 in `WATCH_fungibles.md` |

⭐ **Belief 1 is the one to test first**, and 1.1 tests it on day one. If it is wrong, the release
plan changes while it is still cheap to change.
