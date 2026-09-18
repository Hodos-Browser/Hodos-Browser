# RESUME — beta.4, picking up cold

**Written:** 2026-08-30, at a deliberate stopping point. **Status of beta.4: 🔭 scoped, not started.**
**Why it stopped here:** beta.3 is still the active release, and beta.4 does not start until it ships.

> **Read this file first if you are resuming beta.4 after a gap.** It exists so nothing from the
> 2026-08-29 telescope session has to be rediscovered.

---

## 1. Where things stand, in one paragraph

The **telescope pass is done**. The release is scoped into four tracks in a settled order, the
cross-track edges are named, the harness question is decided, the folders are moved, and the
reusable scoping process exists. **Nothing has been designed at phase level and no code has been
written.** The next stage is the **microscope pass**, and it should not start until beta.3 ships.

## 2. What to do first when you come back

⛔ **Do not re-scope.** The order and the three kickoff decisions are settled (§4 below).

```
1. Read  0.4.0-beta.4/README.md         ← scope, the four tracks, decisions on record
2. Read  0.4.0-beta.4/TELESCOPE.md      ← the cross-track edges + how to split the microscope work
3. Then run the FIRST MICROSCOPE STEP — described in plain terms in §3 below
```

Everything else is read **when you get to that track**, not before.

## 3. The first microscope step, in plain English

`TELESCOPE.md` calls it **"M0, the edge context."** That is jargon for something simple:

> **Before splitting the work up between four separate sessions, run one session on the two questions
> those four sessions would otherwise each answer differently.**

The four tracks get one fresh session each, deliberately kept apart so no session has to hold the
whole release in its head. But two questions sit *between* tracks, and if each session answers them
on its own you get four incompatible answers.

⛔ **Owner correction, 2026-08-30: both of these are RESEARCH TASKS, not decisions to be made on the
spot.** M0's job is to **research them and present them for decision** — not to settle them. Neither
was answerable in the telescope session and neither should be answered from first principles now.

### ⭐ RQ-1 — What does "this output is a token" get saved as? *(the top design question)*

**The question.** When the wallet decides an output is a token rather than spendable money, **where
does that fact get stored, and in what form?** Track 1 has to build this. Track 2 then implements
the ordinals spec, which has its own rules for how tokens are filed. If track 1 guesses and the spec
disagrees, track 2 rewrites the one thing the release's safety rests on.

⛔ **Do not design this from first principles. Go and find out how it is already done, and why.**

**Research instructions — this is the deliverable:**

| # | Read | Looking for |
|---|---|---|
| 1 | **BRC documentation** — 46, 99, 147, 150, 165 in particular | What the spec *requires* to be persisted, versus what it leaves to the implementer |
| 2 | **BSV Association `wallet-toolbox`** — the **TypeScript** and **Go** implementations | How a conforming wallet actually stores basket/token classification. This is the closest thing to a reference answer that exists |
| 3 | **The other BSV SDKs**, across languages | Where they agree, that is the convention. ⭐ **Where they disagree, that is the real design question**, and it should be reported as such |
| 4 | Our own code — `output_repo.rs`, `basket_repo.rs`, `domain_permission_repo.rs` | What we already have, and how far it is from the above |

⚠️ **There is no Rust implementation of wallet-toolbox.** We are porting **patterns and semantics**,
never code. That is also why this needs research rather than a library call.

**The output:** a short document saying what the ecosystem does, where implementations disagree, what
we should do, **and why** — with the trade-offs visible. Then the owner decides.

### ⭐ RQ-2 (was "D-1") — What happens on restore when the wallet cannot identify an output?

**The question.** Restoring from a seed phrase is the moment the wallet knows least. If it finds an
output it cannot classify, what should happen?

⛔ **Owner correction, 2026-08-30: this needs a full conversation and a good / bad / ugly outcome
matrix, not a two-option recommendation.** My earlier framing (two options, pick A) was too thin for
the decision it is carrying. **It belongs in the track planning session, with the research done
first.**

**What M0 must produce for it:**

| Required | Meaning |
|---|---|
| ⭐ **A good / bad / ugly outcome matrix** | For **each** candidate behaviour: what the good case looks like, what the bad case looks like, and **what the ugly case looks like** — the one where the user loses something and does not find out for months |
| The candidate behaviours | At least: show as held-but-unidentified · block the restore · classify-later-on-reconcile · something the ecosystem does that we have not thought of |
| ⭐ **How other wallets handle it** | Per rule 5. `wallet-toolbox`'s recovery path, the BSV SDKs, and any BRC that speaks to recovery. **Somebody has hit this already** |
| The user-facing consequence of each | Stated in plain language, not in terms of database state |

**Why it matters this much:** the two failure modes are opposites and a naive fix for one causes the
other. An output that is **silently spendable** breaks the release's core rule at the worst possible
moment. An output that is **silently dropped** is safe from spending and effectively lost — the user
finds out much later, if ever. That is why `R-RESTORE` in `REGRESSION_ADDITIONS.md` is written as
**two checks that are each other's control**.

## 4. Decisions on record — ⛔ do not relitigate

| | Decision | When |
|---|---|---|
| **Track order** | Guard → 1Sat → OpNS → Backup, then tickets. Each is a prerequisite for the next | 2026-08-29 |
| **beta.3 floor** | The `satoshis > 1` stopgap ships in **beta.3**, not beta.4 | 2026-08-29 |
| **Harness** | beta.4 **references** beta.3's `HARNESS.md` / `REGRESSION_SET.md`; additions live in `HARNESS_DELTA.md` / `REGRESSION_ADDITIONS.md`. Merge into one version-neutral copy **after beta.3 closes** | 2026-08-29 |
| **Guard reach** | General classification seam (`Spendable`/`Token`/`Unknown`, fail closed), **one** classifier implemented | 2026-08-29 |
| **Fungibles** | Deferred with a re-check gate — `WATCH_fungibles.md` | 2026-08-29 |
| **PM skills** | ⛔ Not installed, deliberately — `../SCOPING_PROCESS.md` §8 | 2026-08-30 |
| **Working rules** | Five standing rules adopted into the root `CLAUDE.md` | 2026-08-30 |

## 5. Still owed — the short list

| # | Owed | Who / when |
|---|---|---|
| 1 | ⭐ **RQ-1** — research what "classified" persists as, against BRC docs + `wallet-toolbox` (TS and Go) + the other BSV SDKs. ⛔ **Research task, not a decision** | M0, before any track work |
| 1b | ⭐ **RQ-2** — research restore behaviour and produce the **good / bad / ugly outcome matrix**. ⛔ **Owner decides in the track planning session, after the research** | M0 researches; owner decides |
| 2 | **The exposure question** — does an ordinary incoming 1-sat payment become a tracked default-basket row without a recovery scan? ⚠️ **Answer by running something.** It has been read twice already | Track 1, early |
| 3 | Six files under `0.4.0-beta.3/` still carry pre-move folder paths — five session prompts (archaeology) and the dust ticket's Links section (**live, genuinely owed**) | Whoever next touches beta.3 |
| 4 | `../SCOPING_PROCESS.md` §7a, §7b, §7b-ii — ~19 proposed adoptions, **one decision each**, none in force | Owner, when there is time. Not urgent |
| 5 | `tickets/TICKET_e2e_specs_wrong_subject_and_never_run.md` — second-hand, unverified. Its first step is verification | Unassigned |
| 6 | Is the OpNS overlay PoC in the release, or a parallel public artifact? | Owner, before track 3 |

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
    ├── README.md               ← scope, four tracks, decisions, verified code findings
    ├── RELEASE_PLAN.md          ← track + candidate phase breakdown, no phase detail
    ├── TELESCOPE.md            ← cross-track edges, microscope context plan, what we believe & how it fails
    ├── HARNESS_DELTA.md        ← what beta.4 adds to the harness
    ├── REGRESSION_ADDITIONS.md ← R-NOSPEND, R-CLASSIFY, R-RESTORE, R-TOKENPERM
    ├── WATCH_fungibles.md      ← BRC-163 vs 175, with re-check triggers
    ├── tickets/                ← review queue + template
    ├── research/               ← the three research files (a) PRD skills (b) Karpathy (c) testing
    ├── track-1-utxo-safety-guard/    ← scope only, new
    ├── track-2-1sat-ordinals/        ← carried in (was 1SatOrdinals-BSV21/)
    ├── track-3-opns-naming/          ← scope only, new. A naming doc is still owed (candidate phase 3.1)
    └── track-4-onchain-backup-sync/  ← carried in, IMPLEMENTATION_PLAN.md is authoritative
```

Also: `.claude/agents/track-scoper.md` — the agent that owns the scoping process.
Also: root `CLAUDE.md` — now carries the five working rules and points at `SCOPING_PROCESS.md`.

## 7. Paste this to restart

```
Read development-docs/0.4.0-beta.4/RESUME_beta4.md in full, then README.md and TELESCOPE.md.

beta.3 has shipped. Start the beta.4 microscope pass with M0 — the edge session described in
RESUME §3. M0 owns two RESEARCH questions and nothing else:

  RQ-1  What "classified" persists as. Research it — BRC docs (46/99/147/150/165), then the BSV
        Association's wallet-toolbox in TypeScript AND Go, then the other BSV SDKs. Report where
        implementations agree (that's the convention) and where they disagree (that's the real
        design question). There is no Rust implementation — we port patterns, never code.

  RQ-2  What restore does with an output it cannot identify. Research how other wallets handle it,
        then produce a good / bad / ugly outcome matrix per candidate behaviour, in plain language.
        Do NOT recommend a two-option answer. I decide this in the track planning session.

Constraints: do not re-scope — the order and the decisions in RESUME §4 are settled. Do not design
either question from first principles; go and read how it is already done (CLAUDE.md working rule 5).
Stop when both research outputs exist and I have seen the RQ-2 matrix.
```

---

## 8. What this scoping pass believes, and how it could be wrong

Repeated from `TELESCOPE.md` §6 because it is the thing most likely to be forgotten across a gap.

⭐ **The belief to test first:** the classification seam is cheap, *because the exclusion logic
already works*. Track 1.1's call-site sweep tests this on day one. **If it is wrong — if there are
UTXO-selecting paths that bypass `output_repo` entirely — track 1 is bigger than scoped and the
release plan changes.** Better to find that in the first hours than the third week.

The other four beliefs, briefly: ordinals are a prerequisite for names; the backup size problem is
ancestry depth rather than media (**explicitly a hypothesis, settled by measurement**); four tracks
fit in one release (**if not, cut from the back** — track 4's plan stands alone and can slip to
beta.5 without waste); and fungibles can be deferred without cost.
