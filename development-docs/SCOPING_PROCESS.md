# The scoping process

**Opened:** 2026-08-29. **Status:** 🚧 **FIRST CUT.** The four stages and their exit conditions are
settled; the adoption list in §7 is proposed and awaits owner decision.
**Feedback loop:** `0.4.0-beta.4/` is the first release run through this. Amend from what actually
happens there, not from what should have worked.

> **What this is.** A reusable procedure for scoping a sprint or sub-sprint before implementation.
> Four stages, each with defined outputs and an exit condition.
>
> **What this is not.** A design method, a template to fill in, or a stage anyone is required to run.
> Small work does not need it. ⛔ **Running this on a two-file change is the failure mode it is
> supposed to prevent.**

---

## 0. The problem it solves, stated plainly

Two failures, pulling in opposite directions:

| Failure | What it looks like | The stage that catches it |
|---|---|---|
| **Drift** | An agent builds something adjacent to what was asked, and nobody notices until review. Requirements were never stated precisely enough to be violated | Scope + Microscope's phase contracts |
| **Over-planning** | Tokens spent looping through architecture that just needed a decision. The owner's own named risk | ⛔ The loop limit and the human decision points |

> **The owner's words, kept verbatim because paraphrasing them loses the point:**
> he over-plans, and wants to avoid "spend a lot of tokens looping through architecture and design
> that really just need a decision instead of going back and forth."
>
> **Development sometimes just requires building, testing and adapting.**

⛔ **Human-in-the-loop is a control, not a courtesy.** When two options are close: **both in one
message, with a recommendation, and ask.** Do not iterate silently, and do not fan out agents to have
the conversation for you — that is the most expensive possible way to make a coin-flip decision.

## 1. The four stages

```
  ┌─────────┐    ┌───────────┐    ┌──────────────┐    ┌───────────┐
  │  SCOPE  │───►│ TELESCOPE │───►│  MICROSCOPE  │───►│ TELESCOPE │───► implement
  │         │    │           │    │   (parallel, │    │  (close)  │
  │ outline │    │ how they  │    │  one context │    │  did it   │
  │ the work│    │ fit; how  │    │  per sprint) │    │  break?   │
  │         │    │ to divide │    │              │    │           │
  └─────────┘    └───────────┘    └──────┬───────┘    └─────┬─────┘
                                         │                  │
                                         └──── loop back ◄──┘
                                    ⛔ only if a cross-sprint
                                       edge was invalidated
```

Each stage runs in a **fresh context** and reads the previous stage's **output file**, not the
previous stage's reasoning. That is the point of writing the file.

---

### Stage 1 — SCOPE

> **Goal.** Name the work and cut it into sprints or sub-sprints. Nothing else.

| | |
|---|---|
| **Input** | The owner's intent, the code, whatever tickets and research exist |
| **Output** | A sprint list with an **order** and the **reason for that order** |
| **Exit** | The order is settled and written. ⛔ Settled means **the owner has agreed**, not that the reasoning is good |
| **Decides** | Owner |

**Rules.** State why each item is in the release. Order by **dependency**, not by preference or
excitement, and write the dependency down — an order without stated reasons gets relitigated in every
later session.

---

### Stage 2 — TELESCOPE

> **Goal.** The long-distance look: how the pieces fit together, and **how to divide the microscope
> work across contexts.**

| | |
|---|---|
| **Input** | Stage 1's output; the code, read enough to be specific; the ecosystem state |
| **Output** | A telescope file — see below |
| **Exit** | ⛔ **The deliverables exist and the owner has answered the open questions. Then stop.** |
| **Decides** | Agent proposes; owner answers the open questions |

⛔ **This is not detailed design.** No APIs, no schemas, no phase-level implementation. If you are
tempted to specify an interface, the *question* goes in the microscope's owed list instead.

**The telescope file must contain:**

| § | Content |
|---|---|
| 1 | **The cross-sprint edges** — the things no single sprint owns. If nobody names them here, nobody names them |
| 2 | **The context recommendation** — which microscope contexts exist, what each reads, and ⛔ **what each must not read** |
| 3 | **Where a workflow earns its cost**, per sprint — and where one does not |
| 4 | **Human decision points** — each stated as one question with a recommendation |
| 5 | **The loop limit and exit conditions** for the microscope pass |
| 6 | ⭐ **What this pass believes and how it could be wrong** — so the closing pass has something to check rather than a mood to match |

**Ask the open questions early**, in one message, with a recommendation on each. Not at the end.

---

### Stage 3 — MICROSCOPE

> **Goal.** Dig into one sprint and its phases. Produce signable phase contracts.

| | |
|---|---|
| **Input** | The telescope file, **one** sprint folder, and the code |
| **Output** | Phase contracts from `PHASE_CONTRACT_TEMPLATE.md`, plus a **findings note** |
| **Exit** | Contracts exist; every evidence row has a non-empty RED and SUBJECT; the findings note is written |
| **Decides** | Agent, within the telescope's boundaries. Escalates anything that crosses one |

⛔ **One sprint per context.** A microscope that reads every sprint folder has recreated the problem
the telescope pass exists to solve.

**Where two microscopes genuinely cross over:**

| Need | Handle as |
|---|---|
| One needs another's **result** | A data handoff — numbers, a decision, an interface. Not narrative |
| One needs another's **reasoning** | ⛔ That reasoning belongs in a document neither owns. Write it there |

⭐ **Merging contexts to share understanding is how a multi-sprint release becomes one un-reviewable
plan.** Resist it specifically.

**The findings note is the deliverable that makes stage 4 possible.** It lists anything found that
**contradicts the telescope output** — not a summary of the work. If nothing contradicted it, say
that; it is a real and useful result.

---

### Stage 4 — TELESCOPE (close)

> **Goal.** Zoom out. Assess the microscope findings against the broad plan, change the plan where it
> broke, and loop back **only if warranted**.

| | |
|---|---|
| **Input** | All findings notes, the original telescope file |
| **Output** | An amended telescope file and sprint plan — **and an explicit statement of what did not change** |
| **Exit** | Amendments made; loop-back decision taken and justified in one paragraph |
| **Decides** | Owner, on any loop-back |

⛔ **Silence is not confirmation.** Saying "the plan still holds" is a claim, and it needs the
findings that support it named.

**Loop back into a microscope only when a finding invalidates a cross-sprint edge.** Not for detail,
not for polish, not for a better idea. Both of those are implementation, and implementation checks
happen again anyway.

---

## 2. ⛔ The process scopes itself

Without these, this document becomes the over-planning it exists to prevent.

| Control | Value |
|---|---|
| **Loop limit** | ⛔ **Two microscope passes per sprint.** If a second pass has not produced signable contracts, that is not a planning problem — **build the smallest testable piece and learn from it.** |
| **Stage exit conditions** | Above, per stage. An exit condition is met or it is not; there is no "nearly" |
| **Human decision points** | Named in the telescope file. Each **stops the pass** until answered |
| **Skip rule** | ⛔ **Skip stages for small work.** A single sprint with no cross-sprint edges needs stage 1 and stage 3. The full four-stage run is for **multi-sprint releases** |
| **Cost rule** | A stage that has not changed a decision has not earned its tokens. Say so in the findings note |

⭐ **The strongest control is the last one.** Checks happen again before implementation regardless —
the phase contract, the adversarial review, the regression set. Scoping does not have to be complete.
**It has to be right about the things that are expensive to get wrong later**, which is a much smaller
set.

## 3. Anti-drift — what every stage owes

Drift is prevented by **stating requirements precisely enough to be violated**, not by more review.

Every sprint or phase document states, before any code:

1. **What the feature requires** — concretely, in results not activities.
2. **How it will be tested** — including ⛔ **the negative control**: what must be seen to fail.
3. **What is out of scope** — explicitly, including the things you were tempted by, so scope creep
   shows up in the diff.

⛔ **Scope changes amend the document in the same commit that changes the scope** (`HARNESS.md` §1).
Silent drift is how a phase "passes" without having done the thing.

## 4. Relationship to the harness

| | Owns |
|---|---|
| **This document** | How work is **scoped** — before a contract exists |
| **`HARNESS.md`** | How work is **proven** — the phase contract, the evidence table, the tiers, the ratchets, the adversarial posture |
| **`REGRESSION_SET.md`** | What must **keep** being true |

They meet at the phase contract: the microscope stage's output is the harness's input. ⛔ **Neither
overrides the other, and this document never lowers a harness standard.**

## 5. Research inputs

Three research tasks were dispatched 2026-08-29 to feed this process. Outputs live in
`0.4.0-beta.4/research/`.

| Task | Output | Status |
|---|---|---|
| **(a)** PRD / spec-authoring skills — what to adopt | `RESEARCH_a_prd_skills.md` | ✅ Delivered |
| **(b)** The Karpathy method — what to lift, and where it belongs | `RESEARCH_b_karpathy_method.md` | ✅ Delivered |
| **(c)** Test harness, regression sets and CI testing practice | `RESEARCH_c_testing_practice.md` | ✅ Delivered (502 lines) |

⛔ **Research is advisory until read and decided on.** Nothing in those files changes this document or
the harness by existing. Adopting an item is a decision, recorded with a reason — the same standard as
lowering a gate baseline.

## 6. ⚠️ Two findings from the research that change how you read it

Recorded here because both are the kind of thing that gets repeated as fact if it is not corrected:

1. **There is no canonical "Karpathy method."** He never published one. The widely-circulated
   `CLAUDE.md` attributed to him is **not his** — the repository hosting it says so itself. A second
   repo claiming to carry his "internal rules from a leak" has no corroboration; treat it as
   fabricated. **None** of his own repositories carries a `CLAUDE.md` or `AGENTS.md`. What survives is
   a small set of behavioural rules from his own posts, and one shipped harness (`autoresearch`).
   ⭐ **Read directly 2026-08-30, so it is on record rather than second-hand.** The repo is
   `multica-ai/andrej-karpathy-skills` (~209k stars; author Jiayuan, formerly `forrestchang`). Its
   `CLAUDE.md` is **65 lines, four principles**, and it says of itself that it is *"derived from
   Andrej Karpathy's observations"* — it is **not his file**. Verdict: **principles 1, 2 and 3 are
   genuinely good and are now adopted** (§7c). Principle 4 — *"give it success criteria and watch it
   go"* — is **declined**, see below. Two of its bullet points are actively wrong here and were
   dropped: *"no error handling for impossible scenarios"* (wrong for a wallet) and *"match existing
   style, even if you'd do it differently"* (collides with invariant 9). ⚠️ Its own framing line is
   honest and worth keeping in mind: *"These guidelines bias toward caution over speed. For trivial
   tasks, use judgment."*

   ⚠️ **The underlying Karpathy post is not recoverable by any fetcher.** x.com returns HTTP 402, and
   xcancel was shut down by a cease-and-desist on 2026-08-24. Four passages are quoted verbatim and
   consistently across independent sources and are all we rely on. ⭐ **Three of those four are
   complaints about model behaviour, not instructions** — the "methodology" is the packager's
   inversion of the complaints. That inversion is reasonable; judge it on its merits, not the byline.

2. **The PM skill families are not installed on this machine**, and the agent definition that lists
   26 of them is referencing skills that do not exist locally. Research (a) read the real upstream
   sources instead. ⚠️ Do not assume a skill exists because an agent's tool list names it.

   ⚠️ **Naming trap: `pm-*` here means PRODUCT manager, not PROJECT manager.** Everything about the
   name suggests otherwise. See §7c-ii — we need both intents, and the skill families cover only one.
3. **Two defects in our own test tooling**, found by research (c) and **not independently verified by
   this session** — filed as tickets rather than acted on:
   - the six `frontend/e2e/*.spec.ts` specs run **stock Playwright Chromium against a mocked bridge**,
     which fails `HARNESS.md`'s own SUBJECT rule, and they are invoked by **neither** `preflight.ps1`
     **nor** `test.yml`;
   - `scripts/test-all.ps1` uses `cargo tarpaulin` while `test.yml:209` says *"NOT tarpaulin"*.

## 7. Proposed adoptions — ⏳ awaiting owner decision

⛔ **Not yet in force.** Each is one decision, and they are independent — accept, reject or defer
individually. Grouped by where the item would live.

### 7a. Into this document (sprint-scoping procedure)

| # | Item | Why | Source |
|---|---|---|---|
| A1 | **Kill-assumption block** before the evidence table: `Claim / Fails if / Kill criterion / Cheapest test`, plus **"what I could not assess"** | Forces the cheapest disproof to be named before building. Directly serves anti-drift | (a) |
| A2 | **Bounded approach comparison — at most 3 options, decision forced in the same message** | ⚠️ This is the item that could feed the over-planning risk. It is worth having **only** with the cap and the forced decision. Without both, reject it | (b) |
| A3 | **Wall-clock budget on exploratory items, with keep / discard / crash stated up front** | Turns "we'll see how it goes" into a decision with a deadline | (b) |
| A4 | **Simplicity as the stated tiebreak** when options score equally | Removes a class of loop entirely | (b) |
| A5 | **`Files: create / update / do-not-touch`** in each sprint doc | Greppable blast radius. Cheap | (a) |

### 7b. Into the harness (evidence and contracts)

| # | Item | Why | Source |
|---|---|---|---|
| B1 | ⭐ **A `Status` column on evidence rows: `existing` / `proposed` / `none`** | **The strongest single finding.** Today a contract row is `⬜` whether the test exists or has never been written — the ambiguity that lets a phase close green | (a) |
| B2 | **The policy-shadow warning, quotable verbatim:** *"the unit test proves the helper's logic; it does not prove the framework actually calls it"* | That sentence **is** the four void farbling harnesses. `HARNESS.md` §6 Q1 has no worked example; this is one | (a) |
| B3 | **Say which tiers may gate a merge.** Our tier table lists T0–T4 but never states which can block | A tier that cannot block and a tier that must are different instruments | (a) |
| B4 | **Five edge-case categories as a RED-cell generator** | ⚠️ **Concurrency and integration-failure are our two blind spots, and both are live in beta.4** | (a) |
| B5 | **The instrument is not edited by the change it measures** | `HARNESS.md` §4 makes "raising a baseline needs a written reason" a norm with **no enforcement**, in a project whose preflight once reported PASS while running zero checks | (b) |
| B6 | **Sign-off is a command transcript, never "I believe the tests pass"** | Already the spirit of §8; making it literal costs nothing | (a) |

### 7b-ii. Into the harness — from research (c), the testing pass

⚠️ These carry a cost in tooling and CI time, unlike 7b. They are ranked; adopt from the top.

| # | Item | What it catches that we cannot today | Cost |
|---|---|---|---|
| T1 | ⭐ **`cargo-mutants --in-diff --in-place`**, starting on `hodos_permission_engine` | The **mechanised** form of the hand negative control — a test that passes with the logic broken. We do this one file at a time, by hand, when we remember | Low. ⚠️ It does not understand `#[cfg]`, so Windows DPAPI paths produce noise |
| T2 | **`necessist`** (Trail of Bits) as a per-sprint offline audit | It deletes statements **from tests** and reports the ones that still pass — a different defect class from T1, and aimed exactly at the void-harness failure | Low, offline |
| T3 | **CDP-driven Playwright** (`connectOverCDP` against the shell launched with `--remote-debugging-port`) | ⭐ **The only route by which `R-GOLD`, `R-COUNT` and `R-INTEXT`'s UI half stop needing a human.** Those three have been owed at every boundary | A phase of work, plus a security review of the debug port — which is already an open beta.3 ticket |
| T4 | **`cargo-nextest`** — process isolation, explicit FLAKY marking; retries **in CI only, never in preflight** | Flake policy. `retries: 1` is already set with no policy behind it | Low |
| T5 | **ASan on `hodos_tests` only** (not the shell) | Memory defects in the C++ test target | Low |
| T6 | **Make `ci.yml` a required check** the day the Actions quota returns (~2026-09-01) | Advisory checks that cannot fail a build are documentation | Free |
| T7 | **Ratchet clippy's 198 warnings** the way the T0 gates ratchet, rather than flipping to `-D warnings` | Same instrument the project already trusts, applied to the same problem | Low |
| T8 | **Replace `cargo audit \|\| true`** — which exits 0 while reporting findings — with `cargo deny check advisories` plus a written allowlist. Drop or replace `npm audit` with OSV-Scanner | ⛔ A gate that reports and exits 0 is the false-green shape, in the supply-chain lane | Low |

⛔ **Not worth it, per (c), stated plainly:** Mull for C++ (needs a clang toolchain against an MSVC ABI
pin — keep the manual per-file negative control, which this project already does well); Stryker (no
frontend unit tests to mutate yet); coverage thresholds; FlaUI / WinAppDriver (**the overlays are
windowless CEF — there is no UIA tree**); a commercial flake platform.

#### ⭐ The structural finding — a bucketing problem, not a discipline problem

`R-GOLD`, `R-CLOSE` and `R-COUNT` have been **"owed, not waived"** at every boundary. Four of the six
standing checks need a human, and **nobody budgeted the minutes**. Proposed fix:

| Bucket | Contents | When |
|---|---|---|
| **R-EVERY** | Automatable rows | Always, every boundary |
| **R-RELEASE** | Human rows | **One scheduled, timeboxed session with a session sheet** |

Splitting the set is what makes both halves honest. A standing set nobody can run is not a standard.

#### Scoping-time obligations from (c) — these belong in §3 of this document

1. Every acceptance row names its **Tier *and* an existing mechanism** at scoping time — not "we'll
   figure out how to test it".
2. **Sum the T3 minutes** into the sprint plan. Human testing that is not budgeted does not happen.
3. Add the question: ⭐ **"which existing check would have caught this, and why didn't it?"**
4. `preflight.ps1 -Contracts` — make an unbacked evidence row a **command**, not a convention.
   `HARNESS.md` §7's ID scheme already supports this and nothing exploits it.
5. ⛔ **A blank boundary cell is INCOMPLETE**, not a pass.

### 7c. Into the root `CLAUDE.md` — ✅ **ADOPTED 2026-08-30**

Live in `CLAUDE.md` § "Working rules". No longer proposals.

| # | Item | Source | Landed as |
|---|---|---|---|
| C1 | Read the docs for an unfamiliar API before using it — do not infer the signature | (b) | Rule 4 |
| C2 | **State assumptions; present readings; stop when confused** rather than guessing | (b) + owner | Rule 1 |
| C3 | ⭐ **Minimum code that solves the problem. Nothing speculative.** | (b) + owner request | Rule 2 |
| C4 | **Surgical diffs** — every changed line traces to the request | (b) | Rule 3 |
| C5 | The instrument rule — the harness is not edited by the change it measures | (b) | Rule 5 |

⭐ C4 generalises three warnings the root `CLAUDE.md` already carried ad hoc — the gold pill,
`FingerprintProtection.h`, `ManifestFetcher`. One rule replaced three special cases.

⚠️ **Two guardrails were written in with C3**, because the source's phrasing is wrong for this
codebase: "minimum code" is **not** "take shortcuts", and it does **not** license thinner error
handling in a wallet. And the source's "match existing style even if you'd do it differently"
was **dropped** — it collides with invariant 9 and the CEF input patterns.

### 7c-ii. Product intent vs project intent — ✅ **ADOPTED 2026-08-30** *(owner, 2026-08-30)*

Folded into `CLAUDE.md` rule 1. Stated here because it is a **scoping** distinction first:

| Intent | The question it answers | Who usually holds it |
|---|---|---|
| **Product** | *What should be true for the user, and why?* | Product manager |
| **Project** | *In what order, with what dependencies, in which release, by when?* | Project manager |

This process does **both**, and never says so. Stage 1 (Scope) and stage 2 (Telescope) are largely
**project** work — order, dependencies, cross-sprint edges, decision points. The "what the feature
requires" line in §3 is **product** work. A sprint doc that nails the order and never says what the
user gets is as incomplete as one that does the reverse.

⛔ **The rule: when either intent is ambiguous, ask. Do not infer one from the other.**
"Add ordinals support" is product intent with the project intent missing (which release? before or
after the guard?). "Do sprint 2 next" is project intent with the product intent missing (what does
the user get, and how do we know it worked?). ⭐ **Guessing looks like progress and is the most
expensive kind of wrong**, because the work arrives finished and aimed at the wrong target.

### 7d. ⛔ Explicitly rejected — with the mismatch stated

| Rejected | Why |
|---|---|
| The viral "Karpathy" `CLAUDE.md`, wholesale | Misattributed (§6.1), and **two of its lines are actively wrong here** | 
| *"Don't tell it what to do, give it success criteria and watch it go"* | ⛔ **This is our false-green mechanism with the safety off.** `HARNESS.md` §2's RED and SUBJECT columns are the corrected form. Recorded as declined, with the reason, so it is not re-proposed |
| `autoresearch/program.md`'s **"NEVER STOP" loop** | Contradicts the project invariants that require stopping and asking |
| **"Vibe coding"**, "loop engineering", "the Karpathy loop" | Invented or borrowed jargon for things we already have words for |
| Unmethodologised metric claims from secondary sources | We do not cite numbers whose method we cannot see |
| The 8-section PRD format | Half its sections are market apparatus with no referent in an engineering phase |
| `sprint-plan` velocity estimation | Requires a velocity series we do not have. ⛔ Inventing one is the exact false-measurement failure `HARNESS.md` §9 records |
| `user-stories` / INVEST | "Negotiable" is wrong for a contract that changes only by amendment |
| `pre-mortem`'s Tigers / Paper Tigers / Elephants | Invented jargon, and launch-scoped rather than phase-scoped |
| `pm-execution:test-scenarios` **as an acceptance format** | ⚠️ Its own worked example would pass with the component stubbed. Narrow use only, as a T3 observational script |
| Any spec stage beyond the phase contract | The contract is already the cap. A second one is the over-planning risk wearing a process hat |

⭐ **One gap with no external precedent:** nothing in any source handles the **SUBJECT** column. Every
skill surveyed assumes the test runs against the thing under test. Ours does not assume that, because
three times it was not true. **Keep it; do not expect to import it.**

---

## 8. Do we install the PM skills? — ⛔ **No.** *(decided 2026-08-30)*

**The state, verified:** two marketplaces are installed (`b-open-io`, `claude-plugins-official`).
**Neither contains any `pm-*` skill.** Nothing named `pm-execution`, `pm-product-discovery`,
`pm-product-strategy`, `pm-market-research`, `pm-data-analytics` or `pm-go-to-market` exists anywhere
under `~/.claude`. Installing them would mean **adding a third-party marketplace** (research (a)
found the real sources upstream: `phuryn/pm-skills`, `product-on-purpose/pm-skills`).

**Decision: don't.** Reasons, in order:

1. ⭐ **We already extracted the value.** Research (a) read the upstream files and found roughly six
   adoptable items out of the whole family. They are in §7a and §7b. **Installing the packages now
   would add ~26 skills to get things we already have written down.**
2. **Most of it does not apply.** Over half is market, discovery and stakeholder apparatus that has
   no referent in an engineering phase — see §7d for the item-by-item mismatch.
3. ⛔ **One of them is actively unsafe here.** `pm-execution:test-scenarios`' own worked example
   would pass with the component stubbed. In a project whose named failure mode is tests that cannot
   fail, that is not a neutral addition.
4. **Third-party marketplaces are a supply-chain decision**, not a convenience one, and this repo's
   own dependency policy is freeze-at-the-moment-we-took-control.

**Revisit if:** we want product-discovery work specifically (customer interviews, feature
prioritisation, opportunity trees) — that is the half we correctly did not import, and it is real
work, just not *scoping* work.

### The broken reference, and how to fix it

⚠️ `bopen-tools/agents/project-manager.md` declares 26 `pm-*` skills that do not exist locally, so
that agent's tool list is broken **today** — the skills silently are not there.

| Option | Effect |
|---|---|
| ⭐ **Leave it** (recommended) | Costs us nothing. We do not use that agent for this work, and the scoping process does not depend on it |
| Report upstream | It is a `bopen-tools` defect, not ours. Worth mentioning if we are in there anyway |
| Install the marketplaces to satisfy it | ⛔ Fixes a symptom we do not feel, at the cost above |

⛔ **The transferable lesson is worth more than the fix:** an agent's declared tool list is a
*claim*, not proof the tool exists. Same family as every other false green in this project — verify
before relying on it.

## 9. Change log

| Date | Change |
|---|---|
| 2026-08-29 | Opened. Four stages, self-scoping controls, anti-drift rules. Research (a) and (b) landed; adoption list in §7 proposed, not in force. Research (c) in flight. |
| 2026-08-30 | Research (c) folded in (§7b-ii). **§7c adopted** into the root `CLAUDE.md` as five working rules, including "minimum code that solves the problem" *(owner request)*. **§7c-ii added** — product vs project intent, and the rule to ask rather than infer *(owner)*. Karpathy repo and post read directly and recorded in §6. **§8 added** — PM skills: do not install, with reasons. |
