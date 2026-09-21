# PLANNED — review & reorganise this folder, and start a real lessons-learned practice

**Status:** ⬜ NOT SCHEDULED. **Opened:** 2026-09-21, 👤 owner's call during beta.3 Phase 12.
**This is a scoping note, not a plan.** It exists so the thinking isn't lost. The owner will decide
when it runs and how big it is.

> 👤 *"We should keep a running lessons-learned doc inside the Chromium build and probably the whole
> project… a whole review and revision of the DevOps CI/CD folder, how it's organized, how it can be
> better organized, how we want to do the next Chromium builds after lessons learned from the past one,
> make sure we document everything as we go."*

---

## 1. The four asks

1. **Reorganise this folder** so a human can find things. It is 20 files / ~7,300 lines and the
   organising principle is currently "what happened", not "what do I need".
2. **A running lessons-learned doc for Chromium builds** — one place, added to as we go, not written up
   afterwards.
3. **Probably a project-wide lessons-learned doc too** — the same idea beyond the engine build.
4. **Decide how the next Chromium builds should run**, informed by what the last ones taught us.

---

## 2. What already exists — ⛔ read before building anything new

| Thing | Where | Verdict going in |
|---|---|---|
| Front page for engine builds: what goes in, standing list, pending queue | `NEXT_CHROMIUM_BUILD.md` | ⭐ **New 2026-09-21.** Probably the model for the rest — start here, don't replace it |
| **~870 lines of Chromium-build lessons, already written** | `CEF_BUILD_RUNBOOK.md` §"Lessons learned", line 451→end: **14 subsections**, organised by *build event* ("From the 2026-08-05 macOS CEF 150 build", "From the 2026-03-12 build") | 🚨 **The lessons doc mostly already exists — it is just buried inside a 1,335-line runbook and indexed by date instead of by topic.** ⇒ The job is probably **extract and re-index**, not write from scratch |
| Per-bump institutional memory | `CEF_VERSION_UPDATE_TRACKER.md` (515 lines) | Overlaps the above — decide which owns what |
| Folder index | `README.md` (83 lines) | Good shape; ⚠️ its status line still reads **2026-07-09**, 2½ months stale — symptom of the problem |

⚠️ **Do not start by writing a new lessons doc.** Two already-large documents hold most of the content.
Duplicating them makes three sources that disagree.

---

## 3. Observations from this session worth carrying in

- ⭐ **The organising axis is wrong.** Lessons are filed by *when we learned them*. You read them by
  *what you are about to do*. A build-event index is a diary; the reader needs a topic index.
- ⭐ **"The build" means two different things** and it confused the owner directly: the 35-minute app
  build vs the multi-hour engine build. `NEXT_CHROMIUM_BUILD.md` and root `CLAUDE.md` now say so, but
  the naming across this folder still blurs it.
- ⭐ **Work found between builds was being lost.** The runbook's only home for it was one line at Step 2
  — *"Any other custom patches — list and version them"* — buried 175 lines into a 1,335-line doc.
  That is now `NEXT_CHROMIUM_BUILD.md` §PART 2. **The same "where does this go between events?" gap
  probably exists for release, signing and auto-update.** Check each.
- ⚠️ **Living P&P is mixed with finished one-offs.** `VMP_SIGNING_SPIKE.md`, `AV_SEEDING_GATE_PLAN.md`,
  `WSL_HYBRID_WORKSPACE.md` (planned, never executed) sit beside daily-use docs. Consider an
  `archive/` or a clear status convention — the index has a Status column, so partly it just needs use.
- ⚠️ **The two biggest files are the two most-needed ones** (`BUILD_AND_RELEASE.md` 1,712 lines,
  `CEF_BUILD_RUNBOOK.md` 1,335). Both would likely split into *front page → steps → lessons*.
- ⭐ **Lessons already live in three other places** outside this folder: `CLAUDE.md`'s working rules,
  `development-docs/PRIOR_ART.md`, and per-phase READMEs. Before creating a project-wide lessons doc,
  decide **what it is for that those three are not** — otherwise it becomes a fourth pile.

---

## 4. Questions the owner should answer before this runs

1. **Scope:** reorganise this folder only, or the project-wide lessons practice too? (They are
   separable; the folder one is concrete and bounded, the project-wide one is a habit change.)
2. **Lessons doc shape:** extract from the runbook into a topic-indexed doc, or leave them in place and
   add an index? ⭐ The second is much cheaper and may be enough.
3. **What is a project-wide lessons doc *for*,** given `CLAUDE.md` working rules + `PRIOR_ART.md`
   already exist? If the answer is "the ones that aren't rules and aren't prior art", say so explicitly.
4. **Does "document everything as we go" need a mechanism** (a checklist item at phase close, a
   preflight nudge), or is it a discipline? ⛔ A rule with no mechanism is the thing this project keeps
   learning about — see `feedback_never_kill_by_image_name` ⇒ *embody a rule in a tool*.

---

## 5. 📌 Carried in from beta.3 Phase 12 — the pending engine patch

**This is the concrete item that prompted the whole conversation.** It is already filed in the right
place; recorded here only so this review knows why the queue was created.

- **The patch:** cosmetic-filter / ad-blocker payload delivery — file the scriptlets browser-side and
  have the renderer **pull** them at context creation, instead of pushing them pre-commit.
- **Why it needs the engine:** the ad-blocker's scriptlets are pushed to a render process *before*
  Chromium has decided which process will show the page. On any cross-site click the payload lands in
  the wrong process (or, on a new-tab arrival, in **no** process) and is discarded. The correct moment
  to hand it over is inside libcef, unreachable from app code.
- **Filed at:** `NEXT_CHROMIUM_BUILD.md` §PART 2 · full spec
  `../0.4.0-beta.3/phase-12-adblock-redirect-arrivals/PHASE_CONTRACT_registry_pull.md`
- **Status:** a partial fix shipped (`d79a869`) — blocking now starts ~30 ms in instead of ~2.6 s — so
  it is **not urgent**, but only an engine patch closes the gap.
- **Owner decision still owed:** b1 (engine exposes a pull call our code uses — recommended, keeps the
  logic testable without a multi-hour build) vs b2 (engine injects it itself, like the farbling patch).

⭐ **Two lessons from Phase 12 that belong in whatever lessons doc comes out of this review**, because
both are about *the engine build*, not about ad-blocking:

1. **The fix for this bug already existed in our own engine** — `hodos::FarblingRegistry` solves the
   identical push-lands-in-the-outgoing-document problem, and its comment says so in plain words. It was
   forty lines away from the broken code and nobody carried it across. ⇒ **When an engine patch solves a
   class of problem, record the *class*, not just the instance.** A lessons doc organised by topic would
   have surfaced it; one organised by build date would not.
2. **Engine-patch work is discovered by app-side debugging, months before any build.** The gap between
   "we learned this" and "we can act on it" is the whole reason a pending queue is needed — and the
   reason the lessons have to be written *at discovery time*, while the reasoning is still intact.

---

> Per `CLAUDE.md` invariant #12 this folder is the canonical P&P home. ⛔ This note is scoping only —
> nothing here has been agreed, sized or scheduled.
