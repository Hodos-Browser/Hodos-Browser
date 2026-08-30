# Research (b) — "the Karpathy method"

**Researched:** 2026-08-29 · **For:** 0.4.0-beta.4 scoping · **Status:** research file, no project change made.

---

## 1. What the term refers to, and how sure I am

There is no canonical "Karpathy method." Karpathy has never published a named methodology. The phrase is
ecosystem shorthand that different people attach to at least five different things. Ranked by how likely
each is to be what the owner means:

| # | Candidate reading | Source | Likelihood |
|---|---|---|---|
| **A** | **The four behavioural rules distilled from his 26 Jan 2026 X post** — think before coding, simplicity first, surgical changes, goal-driven execution — as packaged in the `CLAUDE.md` that hit ~209k GitHub stars | Post is Karpathy's; **the file is not** | **High** |
| B | The Apr 2025 "certain rhythm in AI-assisted coding" tweet — context → approaches → draft → manual review → test → commit, and "keep AI on a tight leash" | Karpathy, verbatim | Medium |
| C | The verifiability thesis (Nov 2025 blog) and the generation/verification loop from the Jun 2025 YC talk | Karpathy | Medium-low |
| D | The `autoresearch` agent-loop design (Mar 2026): `program.md`, fixed budget, keep/discard/revert | Karpathy, and it is real code | Low as the intended referent, **highest value if adopted** |
| E | "Vibe coding" (Feb 2025) | Karpathy, but he has since disowned the framing | Low |

**My reading: A, with B underneath it.** The task frames the question as "what to lift into this project's
development harness and guidelines… scoping doc vs top-level `CLAUDE.md`." That framing — behavioural rules,
a file called `CLAUDE.md` — matches the January-2026 package almost exactly, and that package is what people
mean by "the Karpathy method" in Aug 2026. Confidence that A is the intended referent: **high**. Confidence
that A is a *methodology* rather than a list of complaints about model behaviour: **low** — see §3.

**Where the sources disagree with each other.** They mostly disagree with Karpathy.

- **The most-cited artefact is not his.** `multica-ai/andrej-karpathy-skills` (208,613 stars, created
  2026-01-27) says so itself: "derived from Andrej Karpathy's observations." Author is Jiayuan
  (`x.com/jiayuan_jy`, formerly `forrestchang`). Headlines calling it "Andrej Karpathy's CLAUDE.md" are wrong.
- A second repo, `white-sand-grand/karpathy-claude-md` (31 stars), claims to be "Internal Andrej Karpathy
  CLAUDE.md rules… **Sourced from X leak**." No corroboration for that provenance anywhere. **Treat as fabricated.**
- **Karpathy's own repos carry no `CLAUDE.md` and no `AGENTS.md`.** Verified by listing the git trees of
  `karpathy/nanochat`, `karpathy/nanoGPT`, `karpathy/llm.c`, `karpathy/autoresearch`. The only agent context
  file he ships is `autoresearch/program.md`, and that is a task script for one autonomous experiment loop,
  not a standing conventions file. **I could not find any statement from Karpathy that he uses a repo-level
  agent context file at all.** So "Karpathy's advice on repo-level context files" is, as far as I can
  establish, a thing the ecosystem built *about* him rather than *from* him.
- **He rejected the vocabulary the ecosystem hung on him twice.** Feb 2026 retrospective: the vibe-coding
  tweet was "a shower of thoughts throwaway tweet that I just fired off," and he proposed **"agentic
  engineering"** as the term for the professional practice. "Loop engineering," "the Karpathy loop," and
  "the gen-verify loop" are third-party branding, not his phrases.
- Blog posts claiming measured error-rate improvements from the file ("41% to 11%") give no methodology and
  no data. Ignore them.

### Source quality

**Primary, read directly:** `karpathy.bearblog.dev/verifiability/` (2025-11-17);
`karpathy.bearblog.dev/sequoia-ascent-2026/` (2026-04-30); `github.com/karpathy/autoresearch` README and
`program.md`; git trees of four Karpathy repos.

**Primary in origin, obtained through a mirror:** the Apr 2025 rhythm tweet (`x.com/karpathy/status/
1915581920022585597`), read via threadreaderapp because x.com returns HTTP 402 to fetchers and xcancel is
shut down. The mirrored text numbers its steps 1, 2, 3, 4, 6, 7 — **I could not confirm whether a step 5
exists in the original.** Treat the sequence as accurate and the numbering as unverified.

**Primary in origin, only partially recovered:** the 26 Jan 2026 post (`.../status/2015883857489522876`).
Four passages are quoted verbatim and consistently across independent secondhand sources, including the
repo that links directly to the post; I use only those four. I never obtained the full text. Everything
else attributed to that post below is flagged as secondhand.

**Secondhand throughout:** the Jun 2025 YC "Software Is Changing (Again)" talk. I read summaries only, no
transcript. Nothing in this file rests on it alone.

---

## 2. The practices, stated concretely

### From the Apr 2025 rhythm tweet (verbatim, via mirror)

> "Noticing myself adopting a certain rhythm in AI-assisted coding (i.e. code I actually and professionally
> care about, contrast to vibe code). 1. Stuff everything relevant into context… 2. Describe the next single,
> concrete incremental change we're trying to implement. Don't ask for code, ask for a few high-level
> approaches, pros/cons. There's almost always a few ways to do thing and the LLM's judgement is not always
> great… 3. Pick one approach, ask for first draft code. 4. Review / learning phase: (Manually…) pull up all
> the API docs in a side browser of functions I haven't called before or I am less familiar with, ask for
> explanations, clarifications, changes, wind back and try a different approach. 6. Test. 7. Git commit. Ask
> for suggestions on what we could implement next. Repeat."

Same thread, the framing line: keep "a very tight leash" on "an over-eager junior intern savant with
encyclopedic knowledge of software, but who also bullshits you all the time, has an over-abundance of
courage and shows little to no taste for good code" — be "slow, defensive, careful, paranoid," and always
take the inline learning opportunity rather than delegating it.

### From the 26 Jan 2026 post (the four verbatim passages I could confirm)

> "The models make wrong assumptions on your behalf and just run along with them without checking. They
> don't manage their confusion, don't seek clarifications, don't surface inconsistencies, don't present
> tradeoffs, don't push back when they should."

> "They really like to overcomplicate code and APIs, bloat abstractions, don't clean up dead code…
> implement a bloated construction over 1000 lines when 100 would do."

> "They still sometimes change/remove comments and code they don't sufficiently understand as side effects,
> even if orthogonal to the task."

> "LLMs are exceptionally good at looping until they meet specific goals and this is where most of the
> 'feel the AGI' magic is to be found. Don't tell it what to do, give it success criteria and watch it go."

Note the shape of this: **three of the four are complaints about model behaviour, not instructions.** The
methodology in the viral `CLAUDE.md` is the packager's inversion of the complaints. That inversion is
reasonable, but it is the packager's work, and it should be judged on its merits rather than on the byline.

### From `karpathy/autoresearch` (primary, and the only place he shipped a harness)

`program.md` is the agent's instruction file; `train.py` is the only file the agent may edit; `prepare.py`
— which contains the evaluation function — is **explicitly read-only to the agent**; every experiment gets a
fixed 5-minute budget; results go to a `results.tsv` with a `keep` / `discard` / `crash` status column;
improvement advances the branch, no improvement is `git reset` away. Plus an explicit tiebreak: "All else
being equal, simpler is better… A 0.001 val_bpb improvement that adds 20 lines of hacky code? Probably not
worth it. A 0.001 val_bpb improvement from deleting code? Definitely keep."

### From "Verifiability" (blog, 2025-11-17)

Automation follows verifiability. A task is amenable to an agent loop when it is "resettable (you can start
a new attempt), efficient (a lot attempts can be made) and rewardable (there is some automated process to
reward any specific attempt)." Progress is "jagged" — verifiable domains race ahead, unverifiable ones crawl.

### From the Sequoia Ascent summary (blog, 2026-04-30)

The unit of delegation is now a macro action — implement a feature, refactor a subsystem, write tests and
fix the failures, compare approaches and propose a plan. Agents "are like these intern entities"; the human
"still ha[s] to be in charge of aesthetics, judgment, taste, and oversight." Start by working with the agent
to design a detailed spec, "maybe basically the docs." Worked example: on MenuGen his agent proposed joining
Stripe purchases to Google accounts on email address — plausible, and wrong, because the two emails can
differ; the human had to catch it and insist on a persistent user ID.

---

## 3. Honest assessment before recommending anything

Three things to hold on to while reading §4.

**Most of this is opinion, and the strongest-sounding line is the weakest.** "Don't tell it what to do, give
it success criteria and watch it go" is a tweet. It is also, applied naively, **the exact mechanism that
produced this project's four false-green harnesses.** An agent handed a success criterion and told to loop
until it is met will meet it — including by writing a check the bug passes. Karpathy's formulation has no
falsifiability requirement at all. `HARNESS.md` §2 does. **On this point the project is ahead of the source,
and lifting the source unmodified would be a regression.**

**The best material is the least-cited.** `autoresearch/program.md` is Karpathy's only shipped harness, it
is code rather than commentary, and it independently arrives at two rules this project already believes
(one reversible commit per unit of work; the evaluation harness is out of scope for whoever is being
evaluated). It is worth more than the four rules.

**Two items risk feeding the owner's named failure mode.** "Ask for a few approaches with pros/cons" and
"design a detailed spec first" are both invitations to loop on architecture. They are adoptable only with a
cap and a forced decision attached. See §4 items 2 and 10.

---

## 4. Practice-by-practice, against the existing harness

Legend — **Adopt** / **Adopt trimmed** / **Redundant** (already covered, do nothing) / **Reject**.

| # | Practice | Verdict | Placement | Reasoning |
|---|---|---|---|---|
| 1 | Load all relevant context before asking | Redundant | — | `CLAUDE.md` kickoff steps 1–2 already require re-reading the phase docs and re-verifying every cited `file :: symbol`, which is strictly stronger than "stuff everything in." |
| 2 | Ask for 2–3 approaches with pros/cons before any code | **Adopt trimmed** | **Scoping doc** | Not currently anywhere. But unbounded it is over-planning fuel. Bounded form: *at sprint scoping only, for a phase whose approach is genuinely open, list at most three options in one pass, one line of cost and one line of risk each, then the owner picks. No second pass. If a phase's approach is not open, skip this entirely.* Belongs in the scoping doc because it is a sprint-start procedure, and because as standing behaviour it would add a ceremony round to every one-line edit. |
| 3 | One single concrete incremental change per step; test; commit; repeat | Redundant | — | `CLAUDE.md` invariants 4/5 and `HARNESS.md` §1 §7 (rollback in one commit, or the phase is too big) already say this at higher resolution. |
| 4 | Manually read the docs for any API you haven't called before; don't delegate the learning | **Adopt** | **`CLAUDE.md`** | Genuine gap and a real hazard here. This codebase calls CEF, Win32, AppKit and our own Chromium fork patches, where a plausible-looking call can be subtly wrong (`FarblingPolicy.h`'s hand-rolled `RegistrableDomainFromUrl` is a live example of an API that must not be independently re-derived). Standing behaviour, every session: **a call to a CEF / Win32 / AppKit / fork-patched API that this repo does not already use elsewhere gets its documentation read and cited in the phase contract before the diff is accepted.** |
| 5 | Give success criteria, not instructions; let the model loop | Redundant, **and weaker than what exists** | — | The evidence table is this idea with the hole closed. Do **not** import the tweet. Worth one sentence in the scoping doc recording *why* we don't: a success criterion with no RED half is a criterion the agent can satisfy by writing a check that cannot fail. |
| 6 | State assumptions; present interpretations rather than picking one silently; stop and name the confusion | **Adopt** | **`CLAUDE.md`** | Real gap. `CLAUDE.md` today has *targeted* stop-and-ask rules — invariant 2 (schema), 3 (crypto), 13 (production code behind a failing test) — and kickoff step 6 hands back open questions once per phase. There is no general rule against silently resolving an ambiguity mid-phase. Cheap, and it is the single failure mode Karpathy names first. Standing behaviour, so `CLAUDE.md`. |
| 7 | Don't overcomplicate; no unrequested abstraction, flexibility, or speculative feature | **Adopt trimmed** | **`CLAUDE.md`** | Partially covered — kickoff step 3 (reuse-first) stops *duplication*, invariant 5 prefers minimal reversible changes — but nothing stops gold-plating inside an approved change. One line is enough. ⛔ **Do not** copy the viral file's phrasing: "no error handling for impossible scenarios" is wrong for a browser that handles real money, and "match existing style, even if you'd do it differently" collides with invariant 9's platform-conditional requirement and with the CEF input patterns section. |
| 8 | Surgical changes — don't touch orthogonal code, don't delete comments or code you don't understand | **Adopt** | **`CLAUDE.md`** | This one generalises three warnings the file already carries ad hoc: the gold-pill payment badge "must survive every refactor," `FingerprintProtection.h`'s "shipped user-facing control — never delete while tidying," and the `ManifestFetcher` "do not relax it back." Those are three instances of one rule that is never stated. State it once: **every changed line traces to the phase contract; adjacent code you did not come for is out of scope, and unrelated dead code is reported in the contract's blast-radius section rather than deleted.** Note it also gives `HARNESS.md` §1.5 teeth at the diff level rather than only at the phase level. |
| 9 | Human owns aesthetics, judgment, taste, oversight; keep the diff under review | Redundant | — | `HARNESS.md` §6 (adversarial review, four written questions, refutation posture) is a mechanism where this is a disposition. Nothing to add. |
| 10 | Spec first — "work with your agent to design a detailed spec, maybe basically the docs" | Redundant, **and hazardous** | — | The phase contract *is* this, already bounded to seven sections and explicitly capped at "~15–30 minutes of work, not a re-plan." Karpathy's version has no cap. Adopting it would directly feed the over-planning risk. Do nothing. |
| 11 | Verifiability triage — resettable, efficient, rewardable | **Adopt** | **Scoping doc** | The best conceptual import, and it formalises a judgement the harness already makes informally. `HARNESS.md`'s tier table and "where a workflow earns its cost" both turn on how cheaply a claim can be re-tested; T0/T1 work is resettable and efficient, T3 (a human at two monitors) is neither. Use it as an explicit scoping question per phase: **can this phase's acceptance be re-run automatically, and how many times per hour?** Phases that answer "no" get scoped smaller, get their T3 row named up front, and do not get handed to a long autonomous loop. Sprint-start procedure ⇒ scoping doc. |
| 12 | The evaluation harness is read-only to whoever is being evaluated (`prepare.py` in `autoresearch`) | **Adopt** | **`CLAUDE.md`** | The strongest transferable item in the whole body of work, and it lands on a live soft spot. `HARNESS.md` §4 says raising a gate baseline "requires a written reason in this file" — a norm with no enforcement, in a project whose documented history includes a preflight bug that reported PASS while running zero checks. Standing rule: **`scripts/preflight.ps1` gate patterns, gate baselines and `REGRESSION_SET.md` are not edited in the same change that implements the code they measure. Loosening a pattern or raising a baseline is its own commit, with the reason in `HARNESS.md` §4, and re-runs `-NegativeControl`.** |
| 13 | Fixed budget; log every attempt as keep / discard / crash; revert on no improvement | **Adopt trimmed** | **Scoping doc** | Only for *exploratory* work — the beta.3 items explicitly logged as hypotheses with named experiments (the beta.1 stall, `:5137` reachability). Give such work a wall-clock budget and a three-state outcome up front, and let `discard` be a complete, reportable result. This is the same discipline as `HARNESS.md` §8's "'Not reproduced' is a valid, complete state," and it is directly anti-looping: it puts a clock on the investigation. Not `CLAUDE.md` — it applies to a scoped investigation, not to every session. |
| 14 | Simplicity as tiebreak — an improvement that comes from deleting code beats an equal improvement that adds it | **Adopt trimmed** | **Scoping doc** | Fold into the phase-contract review as one line. Low cost, and it is the correct default for a codebase whose stated dependency risk is patch-scale churn per Chromium bump. |

---

## 5. Placement summary

**Top-level `CLAUDE.md`** — standing behaviour, every session, four additions, all short:

1. Unfamiliar CEF / Win32 / AppKit / fork-patched API ⇒ read and cite the doc before the diff is accepted. *(item 4)*
2. State assumptions; on genuine ambiguity present the options rather than resolving silently; when confused, stop and name the confusion. *(item 6)*
3. No unrequested abstraction, configurability or speculative feature inside an approved change. *(item 7)*
4. Surgical diffs: every changed line traces to the phase contract; adjacent code is out of scope; unrelated dead code is reported in blast radius, not deleted. *(item 8)*
5. The measuring instrument is not edited by the change it measures — preflight patterns, gate baselines and the regression set move in their own commit, with a reason, and re-run the negative control. *(item 12)*

**Scoping-process doc** — invoked deliberately at sprint start:

1. Bounded approach comparison for architecturally open phases only: ≤3 options, one line of cost and one of risk each, one pass, owner decides. No second pass. *(item 2)*
2. Verifiability triage per phase: is acceptance re-runnable automatically, and how often? Phases that are not get scoped smaller and name their T3 row up front. *(item 11)*
3. Exploratory items get a wall-clock budget and a keep / discard / crash outcome declared before work starts; `discard` is a complete result. *(item 13)*
4. Simplicity tiebreak at contract review: equal outcome, fewer lines wins; an improvement that deletes code beats one that adds it. *(item 14)*
5. One recorded sentence on why we do not adopt "give it success criteria and watch it loop" in its source form. *(item 5)*

---

## 6. Do not adopt

| Item | Why |
|---|---|
| **The viral `CLAUDE.md` as a file** | Not written by Karpathy. Three of its four principles are already covered here in stronger form, and two of its concrete lines are wrong for this codebase ("no error handling for impossible scenarios"; "match existing style, even if you'd do it differently"). Take items 6–8 as sentences; do not `curl` the file. |
| **"Don't tell it what to do, give it success criteria and watch it go," unmodified** | It is the false-green mechanism with the safety off. This project has already paid days for exactly that. The evidence table is the corrected version. |
| **`program.md`'s "NEVER STOP" autonomous loop** | Designed for a resettable, rewardable, throwaway-branch metric loop, and it explicitly instructs the agent never to pause for the human. Directly contradicts `CLAUDE.md` invariants 2, 3, 8 and 13. This is production software moving real money. |
| **"Vibe coding," in name or practice** | Karpathy scoped it to throwaway weekend projects, later called the tweet a throwaway, and by Feb 2026 had replaced it with "agentic engineering." |
| **"Loop engineering" / "the Karpathy loop" / "gen-verify loop"** | Third-party branding, not his vocabulary, and it names nothing this project doesn't already have a plainer word for. House rule on invented jargon applies. If a term is wanted for the professional practice, **"agentic engineering"** is Karpathy's own and should be defined on first use. |
| **`white-sand-grand/karpathy-claude-md` and its "internal rules, sourced from X leak" claim** | Uncorroborated provenance on a 31-star repo. Treat as fabricated. |
| **Claimed metrics for the rules file ("41% → 11% errors")** | Marketing blog, no methodology, no data. Not evidence. |
| **A detailed spec phase beyond the existing contract** | The phase contract already fills this role and is deliberately capped. Adding Karpathy's uncapped version would feed the over-planning risk the owner named. |

---

## 7. Open and unverified

- Full text of the 26 Jan 2026 post. x.com returns HTTP 402 to fetchers; xcancel is shut down. Four verbatim
  passages recovered and used; the rest of that post is unread. If the owner has X access, pulling the full
  text would firm up items 6–8 — though none of the three depends on the unrecovered portion.
- Whether the Apr 2025 tweet's step numbering skips 5, or the mirror dropped it.
- No transcript of the Jun 2025 YC talk was read. Nothing above rests on it.
- **No evidence found that Karpathy uses a repo-level agent context file.** His four repos carry none. If a
  later session wants to cite "Karpathy on `CLAUDE.md`," that citation does not currently exist.

### Sources

Primary: [Verifiability](https://karpathy.bearblog.dev/verifiability/) · [Sequoia Ascent 2026 summary](https://karpathy.bearblog.dev/sequoia-ascent-2026/) · [karpathy/autoresearch](https://github.com/karpathy/autoresearch) (README + `program.md`) · [karpathy/nanochat](https://github.com/karpathy/nanochat) · [karpathy.ai/tweets.html](https://karpathy.ai/tweets.html)

Primary via mirror: [rhythm tweet, Apr 2025](https://x.com/karpathy/status/1915581920022585597) via [threadreaderapp](https://threadreaderapp.com/thread/1915581920022585597.html) · [claude-coding notes, Jan 2026](https://x.com/karpathy/status/2015883857489522876) · [vibe-coding tweet, Feb 2025](https://x.com/karpathy/status/1886192184808149383) · [one-year retrospective, Feb 2026](https://x.com/karpathy/status/2019137879310836075)

Secondhand: [multica-ai/andrej-karpathy-skills](https://github.com/multica-ai/andrej-karpathy-skills) · [white-sand-grand/karpathy-claude-md](https://github.com/white-sand-grand/karpathy-claude-md) · [Visual Studio Magazine, Apr 2025](https://visualstudiomagazine.com/articles/2025/04/25/vibe-coding-pioneer-advises-tight-leash-to-rein-in-ai-bs.aspx) · [pixelsham, Jan 2026](https://www.pixelsham.com/2026/01/27/andrej-karpathy-a-few-random-notes-from-claude-coding-quite-a-bit-last-few-weeks/) · [dev.to field notes](https://dev.to/jasonguo/karpathys-claude-code-field-notes-real-experience-and-deep-reflections-on-the-ai-programming-era-4e2f) · [The New Stack on "agentic engineering"](https://thenewstack.io/vibe-coding-is-passe/) · [Enersys, Sequoia quotes](https://enersys.co.th/en/insights/karpathy-vibe-coding-awkward-gross-2026)
