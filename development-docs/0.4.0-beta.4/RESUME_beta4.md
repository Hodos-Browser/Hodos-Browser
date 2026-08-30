# RESUME — beta.4, picking up cold

**Written:** 2026-08-30, at a deliberate stopping point. **Status of beta.4: 🔭 scoped, not started.**
**Why it stopped here:** beta.3 is still the active release, and beta.4 does not start until it ships.

> **Read this file first if you are resuming beta.4 after a gap.** It exists so nothing from the
> 2026-08-29 telescope session has to be rediscovered.

---

## 1. Where things stand, in one paragraph

The **telescope pass is done**. The release is scoped into four sprints in a settled order, the
cross-sprint edges are named, the harness question is decided, the folders are moved, and the
reusable scoping process exists. **Nothing has been designed at phase level and no code has been
written.** The next stage is the **microscope pass**, and it should not start until beta.3 ships.

## 2. What to do first when you come back

⛔ **Do not re-scope.** The order and the three kickoff decisions are settled (§4 below).

```
1. Read  0.4.0-beta.4/README.md         ← scope, the four sprints, decisions on record
2. Read  0.4.0-beta.4/TELESCOPE.md      ← the cross-sprint edges + how to split the microscope work
3. Then run the FIRST MICROSCOPE STEP — described in plain terms in §3 below
```

Everything else is read **when you get to that sprint**, not before.

## 3. The first microscope step, in plain English

`TELESCOPE.md` calls it **"M0, the edge context."** That is jargon for something simple:

> **Before splitting the work up between four separate sessions, run one small session that settles
> the two questions those four sessions would otherwise each answer differently.**

The four sprints get one fresh session each, deliberately kept apart so no session has to hold the
whole release in its head. But two questions sit *between* sprints, and if each session answers them
on its own you get four incompatible answers. So one short session goes first and settles them.

**The two questions:**

| | Plain version | Why it can't wait |
|---|---|---|
| **1. What does "this output is a token" get saved as?** | When the wallet decides an output is a token and not spendable money, **where does that fact get stored, and in what form?** Sprint 1 has to invent this. Sprint 2 then implements the actual ordinals spec, which has its own rules for how tokens get filed. If sprint 1 guesses and sprint 2's spec disagrees, sprint 2 rewrites the one thing the whole release's safety rests on. | It is cheap to check the spec now and expensive to migrate later |
| **2. D-1 — what should happen on restore when the wallet can't tell what an output is?** | ⭐ **This one is yours to decide, not ours.** Restoring a wallet from a seed phrase is the moment the wallet knows least. If it finds an output it cannot classify, there are two options — see below | It changes what sprint 1 builds *and* what sprint 4's recovery does |

### ⭐ D-1, the decision waiting for you

When restore finds an output it cannot classify, it can either:

| Option | What the user sees | Risk |
|---|---|---|
| **A. Show it as "held, unidentified"** — not spendable, but visible | Their balance shows the money they can spend, and separately "1 item we could not identify" | The user sees something they may not understand |
| **B. Block the restore until it can be classified** | Restore refuses to finish | ⛔ A user who cannot finish restoring their wallet is worse off than one with a confusing line item |

**Recommendation: A.** The rule the release is built on is *fail closed* — never spend what you
cannot identify. Option A satisfies that and still shows the user everything they own. Option B
fails closed on the wrong thing: it protects the asset by withholding the wallet.

⚠️ The failure this guards against is real and silent: an output that is safe from being spent
**and** invisible to the user has effectively been lost, and the user finds out much later. That is
why `R-RESTORE` in `REGRESSION_ADDITIONS.md` is written as **two checks that are each other's
control** — one proves it can't be spent, the other proves it didn't disappear.

## 4. Decisions on record — ⛔ do not relitigate

| | Decision | When |
|---|---|---|
| **Sprint order** | Guard → 1Sat → OpNS → Backup, then tickets. Each is a prerequisite for the next | 2026-08-29 |
| **beta.3 floor** | The `satoshis > 1` stopgap ships in **beta.3**, not beta.4 | 2026-08-29 |
| **Harness** | beta.4 **references** beta.3's `HARNESS.md` / `REGRESSION_SET.md`; additions live in `HARNESS_DELTA.md` / `REGRESSION_ADDITIONS.md`. Merge into one version-neutral copy **after beta.3 closes** | 2026-08-29 |
| **Guard reach** | General classification seam (`Spendable`/`Token`/`Unknown`, fail closed), **one** classifier implemented | 2026-08-29 |
| **Fungibles** | Deferred with a re-check gate — `WATCH_fungibles.md` | 2026-08-29 |
| **PM skills** | ⛔ Not installed, deliberately — `../SCOPING_PROCESS.md` §8 | 2026-08-30 |
| **Working rules** | Five standing rules adopted into the root `CLAUDE.md` | 2026-08-30 |

## 5. Still owed — the short list

| # | Owed | Who / when |
|---|---|---|
| 1 | ⭐ **D-1** — the restore decision in §3 | **Owner.** Before the microscope pass |
| 2 | **The exposure question** — does an ordinary incoming 1-sat payment become a tracked default-basket row without a recovery scan? ⚠️ **Answer by running something.** It has been read twice already | Sprint 1, early |
| 3 | Six files under `0.4.0-beta.3/` still carry pre-move folder paths — five session prompts (archaeology) and the dust ticket's Links section (**live, genuinely owed**) | Whoever next touches beta.3 |
| 4 | `../SCOPING_PROCESS.md` §7a, §7b, §7b-ii — ~19 proposed adoptions, **one decision each**, none in force | Owner, when there is time. Not urgent |
| 5 | `tickets/TICKET_e2e_specs_wrong_subject_and_never_run.md` — second-hand, unverified. Its first step is verification | Unassigned |
| 6 | Is the OpNS overlay PoC in the release, or a parallel public artifact? | Owner, before sprint 3 |

⛔ **Nothing on this list blocks beta.3.** Items 1 and 2 block the beta.4 microscope pass; the rest
can wait.

## 6. The map — what is where

```
development-docs/
├── SCOPING_PROCESS.md          ← Track B: the reusable 4-stage process. §7 = adoption list, §8 = PM skills
├── 0.4.0-beta.3/               ← ⛔ ACTIVE, another session owns it. READ-ONLY from beta.4
│   ├── HARNESS.md              ← the standard beta.4 inherits (do not fork)
│   ├── REGRESSION_SET.md       ← the standing checks beta.4 inherits
│   └── TICKET_token_outputs_destroyed_by_dust_paths.md   ← the beta.3 floor ships from here
└── 0.4.0-beta.4/
    ├── RESUME_beta4.md         ← you are here
    ├── README.md               ← scope, four sprints, decisions, verified code findings
    ├── SPRINT_PLAN.md          ← sprint + sub-sprint breakdown, no phase detail
    ├── TELESCOPE.md            ← cross-sprint edges, microscope context plan, what we believe & how it fails
    ├── HARNESS_DELTA.md        ← what beta.4 adds to the harness
    ├── REGRESSION_ADDITIONS.md ← R-NOSPEND, R-CLASSIFY, R-RESTORE, R-TOKENPERM
    ├── WATCH_fungibles.md      ← BRC-163 vs 175, with re-check triggers
    ├── tickets/                ← review queue + template
    ├── research/               ← the three research files (a) PRD skills (b) Karpathy (c) testing
    ├── sprint-1-utxo-safety-guard/    ← scope only, new
    ├── sprint-2-1sat-ordinals/        ← carried in (was 1SatOrdinals-BSV21/)
    ├── sprint-3-opns-naming/          ← scope only, new. A naming doc is still owed (sub-sprint 3.1)
    └── sprint-4-onchain-backup-sync/  ← carried in, IMPLEMENTATION_PLAN.md is authoritative
```

Also: `.claude/agents/sprint-scoper.md` — the agent that owns the scoping process.
Also: root `CLAUDE.md` — now carries the five working rules and points at `SCOPING_PROCESS.md`.

## 7. Paste this to restart

```
Read development-docs/0.4.0-beta.4/RESUME_beta4.md in full, then README.md and TELESCOPE.md.

beta.3 has shipped. Start the beta.4 microscope pass with M0 — the small edge session described
in RESUME §3. Settle question 1 (what "classified" persists as, checked against BRC-147 and
BRC-165) and bring me D-1 with a recommendation.

Constraints: do not re-scope, the order and the four decisions in RESUME §4 are settled. Keep M0
small — it owns two questions and nothing else. Stop when its output doc exists and I have
answered D-1.
```

---

## 8. What this scoping pass believes, and how it could be wrong

Repeated from `TELESCOPE.md` §6 because it is the thing most likely to be forgotten across a gap.

⭐ **The belief to test first:** the classification seam is cheap, *because the exclusion logic
already works*. Sprint 1.1's call-site sweep tests this on day one. **If it is wrong — if there are
UTXO-selecting paths that bypass `output_repo` entirely — sprint 1 is bigger than scoped and the
release plan changes.** Better to find that in the first hours than the third week.

The other four beliefs, briefly: ordinals are a prerequisite for names; the backup size problem is
ancestry depth rather than media (**explicitly a hypothesis, settled by measurement**); four sprints
fit in one release (**if not, cut from the back** — sprint 4's plan stands alone and can slip to
beta.5 without waste); and fungibles can be deferred without cost.
