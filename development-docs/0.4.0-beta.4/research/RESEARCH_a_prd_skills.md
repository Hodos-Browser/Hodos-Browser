# Research (a) — PRD / spec-authoring skills: what to adopt

**Researched:** 2026-08-29

**Bottom line.** The PM skill families named in the task brief (`pm-execution`, `pm-product-discovery`,
`pm-product-strategy`, `pm-market-research`, `pm-data-analytics`) are **not installed on this machine** —
`agents/project-manager.md` declares 26 `pm-*` skills that do not exist in any local plugin, so that agent's
tool list is broken. I read the real files where they live: on disk for `bopen-tools` / `bsv-skills` /
`claude-plugins-official`, and from the upstream repos (`phuryn/pm-skills`, `product-on-purpose/pm-skills`)
for the `pm-*` skills themselves. Almost all of the *PRD* apparatus is a poor fit: it is built to answer
"which of these should we build, and for whom", while `HARNESS.md` already answers "what exactly does this
one phase require, and what would falsify it". Four things are genuinely worth taking, and one of them —
the `/derive-tests` **coverage map with a status column** and its four-way test-type taxonomy — is a direct
patch for our weakest point, because it is the only source found that distinguishes *a test exists* from
*a test is proposed* and names the failure mode where a unit test passes while the real enforcement path is
unprotected. Everything else is either ceremony we already do better, or market/discovery machinery that
would add sections a phase contract cannot fill honestly.

---

## 1. Availability — read this first

| Family | Status on this machine |
|---|---|
| `pm-execution`, `pm-product-discovery`, `pm-product-strategy`, `pm-go-to-market` | ❌ **not installed.** No `pm-*` directory under `.claude/`; not in the `b-open-io` marketplace either. Read from `github.com/phuryn/pm-skills`. |
| `pm-market-research`, `pm-data-analytics` | ❌ not installed, same repo. Not read — discovery/analytics, out of scope (§5). |
| `bopen-tools` 1.1.11 | ✅ installed. Contains `linear-planning`, `documentation-writer`, `hunter-skeptic-referee`, `confess`, `benchmark-skills`, tester references. |
| `bsv-skills` 0.2.7 | ✅ installed. Contains one `PRD-TEMPLATE.md`. |
| `claude-plugins-official` | ✅ installed. Contains `project-artifact` and `feature-dev` — the two best on-disk sources. |

⚠️ If a later session wants these skills live: `claude plugin marketplace add phuryn/pm-skills` then install
`pm-execution`. **Do not bother** — §5 argues the installable half is mostly reject.

## 2. Files read

**On disk**

- `C:\Users\archb\.claude\plugins\marketplaces\claude-plugins-official\plugins\project-artifact\skills\project-artifact\SKILL.md` — tabbed project status page; the tab catalog is a section list for "a project too big for one update".
- `…\project-artifact\skills\project-artifact\swe.md` — the same for PR-driven software: X.Y numbering, per-PR write-up, and the "falsifiable check" phrasing.
- `C:\Users\archb\.claude\plugins\marketplaces\claude-plugins-official\plugins\feature-dev\commands\feature-dev.md` — seven-phase feature workflow with two hard user gates.
- `C:\Users\archb\.claude\plugins\cache\b-open-io\bopen-tools\1.1.11\skills\linear-planning\SKILL.md` — turn a spec into agent-ready tickets; five-question brief rule.
- `…\bopen-tools\1.1.11\skills\linear-planning\references\issue-template.md` — four copy-paste ticket templates (epic / feature / bug / refactor).
- `…\bopen-tools\1.1.11\agents\documentation-writer.md` — the "PRD Expertise" claim is four bullet lines (Shape Up, Working Backwards, Five Whys, US-001 stories). No template behind it.
- `…\bopen-tools\1.1.11\agents\project-manager.md` — declares the 26 missing `pm-*` skills; the Linear workflow is real, the PM roster is not.
- `…\bopen-tools\1.1.11\skills\benchmark-skills\SKILL.md` — trap design and contrastive validation for skill evals.
- `…\bopen-tools\1.1.11\skills\confess\SKILL.md` — adversarial self-audit checklist before declaring done.
- `…\bopen-tools\1.1.11\skills\hunter-skeptic-referee\SKILL.md` — three isolated agents with context boundaries and an EV-scored dismissal rule.
- `…\bopen-tools\1.1.11\agents\references\tester\anti-patterns.md`, `integration-testing.md`, `e2e-testing.md` — stack-specific test recipes.
- `C:\Users\archb\.claude\plugins\cache\b-open-io\bsv-skills\0.2.7\templates\PRD-TEMPLATE.md` — not a PRD; an autonomous-loop work order with machine-checkable completion criteria.

**Upstream (not installed)**

- `phuryn/pm-skills` → `pm-execution/skills/{create-prd,user-stories,test-scenarios,sprint-plan,pre-mortem}/SKILL.md`
- `phuryn/pm-skills` → `pm-execution/commands/red-team-prd.md`, `pm-ai-shipping/commands/derive-tests.md`
- `product-on-purpose/pm-skills` → `skills/{deliver-acceptance-criteria,deliver-edge-cases}/SKILL.md` + their `references/TEMPLATE.md`
- `jamesrochabrun/skills` → `skills/prd-generator/SKILL.md` (13-section PRD; read for comparison only)

## 3. The actual artefacts

### 3.1 `create-prd` — the 8-section template (the canonical PRD)

`1. Summary · 2. Contacts · 3. Background · 4. Objective (+ SMART Key Results) · 5. Market Segment(s) ·
6. Value Proposition(s) · 7. Solution (7.1 UX/Prototypes, 7.2 Key Features, 7.3 Technology, 7.4 Assumptions) ·
8. Release`

The `prd-generator` variant is the same idea at 13 sections: `Executive Summary · Problem Statement · Goals &
Objectives · User Personas · User Stories & Requirements · Success Metrics · Scope (in/out) · Technical
Considerations · Design & UX Requirements · Timeline & Milestones · Risks & Mitigation · Dependencies &
Assumptions · Open Questions`.

### 3.2 `/derive-tests` — the coverage map ⭐

One row per use case:

```
| Use case | Rule (doc) | Expected behavior (+ deny case) | Evidence | Type | Status |
```

`Status` ∈ `existing / proposed / none`. `Type` ∈ **unit** (pure, deterministic) · **integration
(deterministic)** (real wiring, local/in-memory dependency, same result every run) · **guarded live**
(real external service, flag-gated, never in default CI) · **manual** (UI/judgment; a reviewer checklist
item, not a test). The report is written in three separated sections: **Existing coverage** / **Proposed
tests** / **Gaps — documented but unverified**.

Rule selection: *"A rule earns a test when getting it wrong harms someone other than the actor."* The
enumerated classes are authorization allow **and deny**, input validation and output encoding at each sink,
idempotency and dedup keys, fail-closed defaults on error/timeout/cache-miss/flag paths, side-effect
conditions (exactly when a write commits or a payment fires), and agent output-contract limits.

The load-bearing warning, quoted:

> The unit test proves the helper's logic; it does **not** prove the framework actually calls it. Wiring and
> policy enforcement (route middleware, DB row-level security, auth guards, provider config) still needs an
> integration or guarded-live check, or the helper becomes a policy shadow that passes while the real path
> is unprotected.

### 3.3 `/red-team-prd` — the kill-assumption block

```
- **Claim:** [load-bearing assertion]
  - **Fails if:** [concrete, falsifiable]
  - **Evidence to get this week:** [specific]
  - **Kill criterion:** [threshold]
  - **Cheapest test:** [smallest experiment]
[3–5 max]

### What's Well-Reasoned
### What I Couldn't Assess
```

Method: keep only claims that are *load-bearing* (false ⇒ the plan dies); steelman each, then attack the
steelman; rank by `(impact if wrong) × (likelihood wrong) × (cheapness to test)`; never fabricate a weakness.

### 3.4 `linear-planning` — ticket as agent brief

Every ticket answers **What · Why · Where · How · Done when**. The feature template's structure:

`## Context · ## What to Build · ## Files (Create: / Update: / Do not touch:) · ## Implementation Notes ·
## Environment / Config · ## Acceptance Criteria`

Its acceptance-criteria rule, stated as a contrast: ❌ "Works correctly" / ✅ "`bun run build` passes with no
type errors"; ❌ "Looks good" / ✅ "PricingCard renders plan name and price from props".

### 3.5 `deliver-edge-cases` — the five failure-surface categories

Each a table of `Scenario | Expected Behavior | Priority | Notes`:

`Input Validation · Boundary Conditions · Error States · Concurrency · Integration Failures`

Plus two sections most specs omit: **Error Messages** (`Error State | User Message | Additional Action`) and
**Recovery Paths** (per error: what the user sees, ordered recovery options, and **data preservation** — what
is saved and what is lost).

### 3.6 `deliver-acceptance-criteria` — four required groups

`## Story Context · ## Happy Path · ## Edge Cases · ## Error States · ## Non-Functional Criteria · ## Notes`,
each criterion `AC-n` in Given/When/Then. Checklist items worth keeping verbatim: *"Each criterion is testable
and has one clear outcome"*, *"No implementation details leak into the acceptance criteria"*, and the
instruction *"If a statement is subjective, rewrite it into a measurable outcome."*

### 3.7 `bsv-skills/PRD-TEMPLATE.md` — completion as a transcript

Not a PRD. Sections: `Objective · Dependencies (USE THESE — NOT ALTERNATIVES) · NOT in scope · RALPH LOOP
PROTOCOL · COMPLETION CRITERIA (ALL MUST PASS) · VERIFICATION SEQUENCE · FUNCTIONAL REQUIREMENTS ·
ERROR HANDLING · FILE STRUCTURE · DO NOT`. Completion is four numbered commands "run IN ORDER, ALL must
succeed", and the sign-off rule is explicit:

> **DO NOT** output the promise based on: "I believe the tests pass" · "The code looks correct" ·
> Previous iteration results (tests can regress!)

It also mandates a **test sandwich**: run the suite *before* the change, change, run *after*; if failures
increase, revert.

### 3.8 `feature-dev` — Phase 3, the clarifying-questions gate

> **CRITICAL**: This is one of the most important phases. DO NOT SKIP.

Underspecification list to sweep: *edge cases, error handling, integration points, scope boundaries, design
preferences, backward compatibility, performance needs*. And the escape-hatch rule: *"If the user says
'whatever you think is best', provide your recommendation and get explicit confirmation."*

### 3.9 `project-artifact/swe.md` — falsifiable checks and X.Y ordering

Success criteria get *"a falsifiable check (static: 'this diff is empty'; dynamic: 'run X with the flag on,
observe Y stays flat')"*, splitting must-have from nice-to-have. Workstreams are numbered `X.Y` — `X`
increments when blocked on the previous stage, `Y` for things that land in parallel — so *"the numbers carry
the dependency order — don't draw a DAG."*

## 4. Adopt — ranked

**1. The coverage-map row and its `Status` column (`/derive-tests`).** Highest value by a distance. Add
`Status ∈ existing / proposed / none` to the evidence table, and split the phase's test inventory into
*existing coverage* / *proposed* / *gaps*. Today a `PHASE_CONTRACT` evidence row cannot express "this row's
test does not exist yet" — it is `⬜` either way, which is the same ambiguity that lets a phase close green.
The rule *"mark a rule existing only when a test in the repo actually asserts it today"* is `HARNESS.md` §8
("a skipped check is SKIPPED, and the run is INCOMPLETE") applied one level up, at contract-authoring time.

**2. The policy-shadow warning, verbatim, in the harness.** *"The unit test proves the helper's logic; it does
not prove the framework actually calls it."* This is a named, quotable instance of §6 Q1 ("can I make this
test pass with the feature removed?") that we have no written example of. It also generalises the four
farbling harnesses: each tested a helper, none tested that the browser called it. Put it beside the
SUBJECT-column explanation.

**3. The four-way test type taxonomy, mapped onto our tiers.** `unit → T1`, `deterministic integration → T2`,
`guarded live → T2/T4`, `manual → T3`. The taxonomy's rule — **only the deterministic local set gates the
merge; guarded-live and manual never block the default run** — is the missing half of our tier table, which
lists tiers but does not say which ones a phase may be blocked on. It also gives the T3 row a justification
in one line rather than the current "a phase that changes UI and declares no T3 row is a smell".

**4. The kill-assumption block as a pre-contract step (`/red-team-prd`).** Before §4's evidence table exists,
force `Claim / Fails if / Kill criterion / Cheapest test` for 3–5 load-bearing claims. This is the artefact
that most directly serves goal (i), preventing drift: it makes the agent write down, in advance, what
observation would end the phase. Take the two closing sections too — **What's well-reasoned** and **What I
couldn't assess**. We have no slot for recorded unknowns; §8's "not reproduced is a valid, complete state"
says the same thing and currently has nowhere to live in the contract.

**5. The `Files: Create / Update / Do not touch` block (`linear-planning`).** A file-level, greppable version
of §5 Blast radius and §6 Out of scope. "Do not touch" beats prose because a reviewer can diff against it.
Adopt the five-question brief rule (What/Why/Where/How/Done-when) as the shape of §1–§2 — it already matches,
which is a useful confirmation rather than a change.

**6. The five edge-case categories as an evidence-row generator (`deliver-edge-cases`).** Not as a separate
document — as a checklist the phase author sweeps to produce RED cells: *Input Validation · Boundary
Conditions · Error States · Concurrency · Integration Failures*. Our contracts skew to one happy row plus one
negative; these five categories are the cheapest way to force the enumeration. **Concurrency** and
**Integration Failures** are the two we systematically miss, and both are live in beta.4 (the dust
consolidator racing a 1-sat ordinal; the 1Sat API as a third party).

**7. The `Non-Functional Criteria` group (`deliver-acceptance-criteria`).** Performance, accessibility,
security and auditability rows currently have no home in the evidence table and end up as prose or nowhere.
Add it as a named group. Take the wording rule with it: *"If a statement is subjective, rewrite it into a
measurable outcome"* — that is `PHASE_CONTRACT_TEMPLATE` §2 in one sentence, and says it better.

**8. "Falsifiable check: static vs dynamic" (`project-artifact/swe.md`).** Adopt the phrasing for GREEN/RED
cells — *static:* "this diff is empty" (our T0 gates); *dynamic:* "run X with the flag on, observe Y stays
flat". Also consider **X.Y phase numbering** for `SPRINT_PLAN.md`: `X` increments on a blocking dependency,
`Y` for parallel work, so the ID carries the ordering and no dependency diagram is needed.

**9. Completion as a transcript, not a claim (`bsv-skills/PRD-TEMPLATE.md`).** The sign-off block should be
"run these commands IN ORDER; all must succeed", with the disallowed-evidence list attached: not
"I believe the tests pass", not "the code looks correct", not a previous iteration's result. Our sign-off
table has the right columns; this supplies the prohibition. The **test sandwich** (run before, change, run
after, revert on regression) is a stricter, cheaper form of §5's fix loop and worth stating explicitly.

**10. Three lines from `confess` in the sign-off.** *"Grep for every symbol you changed — are all callers
updated?"* · *"Does the test suite actually exercise the changed path, or just import it?"* · *"What would
break if someone reverted just your last commit?"* The third is a rollback check that validates §7 by
experiment rather than by assertion. Its framing rule is also right and cheap to copy: **do not ask "what did
I miss?" — assume something was missed and hunt to prove it.**

**11. `feature-dev` Phase 3, as a hard gate.** The underspecification sweep list (edge cases, error handling,
integration points, scope boundaries, backward compatibility, performance) plus the "whatever you think is
best ⇒ recommend and get explicit confirmation" rule. Low novelty, near-zero cost, closes the specific drift
where an agent invents an answer to an unasked question.

## 5. Reject — and the mismatch

- **The 8-section PRD template (`create-prd`), whole.** Sections 2 *Contacts*, 5 *Market Segment(s)*,
  6 *Value Proposition(s)* and 8 *Release* have no referent in a sprint phase. There is no market segment for
  "stop the dust consolidator burning 1-sat ordinals". Adopting the template imports four sections that can
  only be filled with filler, in a process whose central rule is that an empty cell means not done. The two
  sections that do carry weight — *Objective* and *7.4 Assumptions* — are already §1 Goal and (better) the
  kill-assumption block in §4.4 above.

- **`sprint-plan`, entirely.** It runs on story points, "average velocity from the last 3 sprints", a 15–20%
  capacity buffer and a Definition of Ready. There is no velocity series here, and inventing one produces a
  number that looks measured and is not — the exact failure `HARNESS.md` §9 records about hand-rolled greps
  versus baselining with the tool that does the measuring. Our sizing rule is stronger and free: *if you
  cannot state the rollback in two lines, the phase is too big.*

- **`user-stories` (3 C's + INVEST).** "As a [role], I want [action], so that [benefit]" buries the observable
  outcome in a subordinate clause; §1's "one sentence, user-observable" carries the same information with
  less ceremony. INVEST's **Negotiable** is actively wrong for us: a phase contract is deliberately *not*
  negotiable once written, except by amendment in the commit that changes the scope. Keep only **Testable**,
  which we already enforce harder.

- **`pre-mortem`'s Tigers / Paper Tigers / Elephants.** Invented jargon for a three-way sort (real risks /
  overblown concerns / unspoken and unchecked), against the standing no-invented-jargon rule. The sort itself
  is fine in plain words, and `/red-team-prd` reaches the same place with a falsifiable test attached instead
  of a taxonomy. The framing is also launch-scoped — 14 days to launch, revenue targets, reputation damage —
  none of which a phase contract has.

- **`test-scenarios` (pm-execution) as an acceptance format.** Its shape (`Test Objective · Starting
  Conditions · User Role · Test Steps · Expected Outcomes`) is manual-QA prose with **no negative control and
  no subject**. Its own worked example lists "Section displays 4-8 product cards with complete information" —
  which a stubbed component satisfies. That is precisely the farbling-harness failure. **Narrow adoption
  only:** the `Starting Conditions → numbered steps → observable outcome` shape is a good format for a
  **T3** observational row, where a human at a machine needs a script. It must not be used to write GREEN.

- **Discovery and strategy families entirely** — `identify-assumptions`, `prioritize-assumptions`,
  `opportunity-solution-tree`, `prioritize-features`, `prioritization-frameworks` (RICE/ICE/MoSCoW),
  `stakeholder-map`, `product-strategy`, `product-vision`, `swot-analysis`, `ansoff-matrix`, `pestle`,
  `porters-five-forces`, `gtm-strategy`, `beachhead-segment`, `metrics-dashboard`. These answer *which of
  these should we build, for whom, and in what order*. beta.4's scope is already decided — four sprints in a
  fixed order. The scoping process answers a different question: *what exactly does this one require, and
  what observation would falsify it*. Importing a prioritisation framework into a settled scope produces a
  ranking exercise whose output is already known, which is drift wearing the costume of rigour.

- **`linear-planning`'s Linear machinery** — MCP server, teams/projects/cycles, story points, epic
  hierarchies, the `linear-sync` commit guard. We do not use Linear. The value is entirely in the ticket
  *text* templates (§3.4); the control plane is not transferable.

- **The `project-manager` agent as a whole.** Its declared skill list references 26 skills that are not
  installed. Reading it as a design source is fine; invoking it is not.

- **`hunter-skeptic-referee` as a new workflow.** `HARNESS.md` §6 already specifies adversarial review as a
  posture with four written questions, and its cost table already decides where fan-out earns its keep.
  The one idea worth stealing is the **context boundary rule** — the Skeptic sees only the structured finding
  list, never the Hunter's narrative — because *"if the Skeptic sees the Hunter's confidence, it anchors on
  it."* That is a one-line addition to §6, not a replacement workflow.

- **`documentation-writer`'s "PRD Expertise".** Four bullet lines naming Shape Up, Working Backwards, Five
  Whys and US-001 stories, with no template, checklist or worked example behind them. Nothing to adopt.
  Working Backwards in particular — write the press release first — has no analogue for an internal phase
  with no external announcement.

## 6. Where these skills handle "how will this be tested" well

This is our weakest point, and exactly two sources address it seriously.

**`/derive-tests` is the one to mine.** It is the only source found that (a) requires an inventory of what
exists *before* proposing anything — *"skipping this step yields a falsely-green map that claims rules are
pinned when nothing checks them"*; (b) forces the **deny case** into the same cell as the expected behaviour
(`Expected behavior (+ deny case)`), which is our two-sided-row rule arrived at independently; (c) names the
policy-shadow failure quoted in §3.2; and (d) supplies a selection rule for *which* rules deserve a test —
*"a rule earns a test when getting it wrong harms someone other than the actor"* — which is a usable filter
for deciding when a beta.4 invariant needs a T0 gate rather than a T1 assertion. Its *"don't wire external
services into the default CI run; flaky live tests erode the green-before-merge gate until people start
ignoring it"* is the argument for keeping the 1Sat API out of preflight.

**`benchmark-skills` corroborates the RED column from a different domain.** Its **contrastive validation**
requires both directions: the baseline *does* exhibit the bad behaviour, and the skilled output does *not*.
*"If baseline passes an assertion, that assertion is not measuring delta."* Substitute "feature removed" for
"baseline" and that is `HARNESS.md` §2 verbatim. Its trap-design procedure — *verify the baseline actually
falls into the trap; if it passes, redesign the prompt or drop the test case* — is the same instrument as our
`preflight.ps1 -NegativeControl`. Nothing new to adopt mechanically; it is independent confirmation that the
harness's central idea is not idiosyncratic, and its assertion-reliability ordering (`not-contains`/regex
highest, LLM judge lower) is a reasonable basis for choosing between a T0 grep and a T1 assertion.

**`deliver-acceptance-criteria` handles testability at the sentence level** — one outcome per criterion, no
implementation detail, subjective statements rewritten as measurable outcomes. Useful as an editing pass on
GREEN cells, nothing more.

**`bsv-skills/PRD-TEMPLATE.md` handles sign-off honesty well and requirements badly.** The completion block
is a runnable command list with an explicit ban on inferred success; that half is directly adoptable. Its
"functional requirements" half is a stub list.

⛔ **Nothing in any source handles the SUBJECT column** — which process, which browser, which binary, which
build type. Every skill surveyed assumes the test runs against the thing under test and never asks for proof
of it. That is the one part of `HARNESS.md` with no external precedent found, and it is also the part that
killed three farbling harnesses. Do not expect to import it; keep it.

## 7. Net recommendation for the scoping process

Extend `PHASE_CONTRACT_TEMPLATE.md` rather than building a parallel PRD. Four changes, in priority order:

1. Add `Status` (`existing / proposed / none`) to the §4 evidence table, and require the three-way split
   (existing / proposed / gaps) at sign-off.
2. Add a §0 **kill assumptions** block — `Claim / Fails if / Kill criterion / Cheapest test`, 3–5 rows —
   plus `What I couldn't assess`, written before §4 exists.
3. Make the tier table say which tiers may block a merge (the deterministic local set only), and add the
   policy-shadow warning to `HARNESS.md` §6 beside Q1.
4. Replace §5's prose blast radius with `Files: Create / Update / Do not touch`, and add the five edge-case
   categories as a sweep checklist that generates RED cells.

Everything else in §4 is a wording improvement, not a structural one.

---

## Sources

- [phuryn/pm-skills](https://github.com/phuryn/pm-skills) — `pm-execution` (create-prd, user-stories, test-scenarios, sprint-plan, pre-mortem, red-team-prd), `pm-ai-shipping/commands/derive-tests.md`
- [product-on-purpose/pm-skills](https://github.com/product-on-purpose/pm-skills) — `deliver-acceptance-criteria`, `deliver-edge-cases` and their templates
- [jamesrochabrun/skills — prd-generator](https://github.com/jamesrochabrun/skills/blob/main/skills/prd-generator/SKILL.md)
- [pm-execution plugin listing](https://www.claudepluginhub.com/plugins/phuryn-pm-execution-pm-execution)
