# Session prompt — comprehensive DPI / display-scaling assessment

Written 2026-08-23 after P0.8 round 3 (`6fc35d2`, pushed to `origin/0.4.0`).

⛔ **This session opens with RESEARCH and ADVERSARIAL REVIEW. No layout code until the
assessment is written and the owner has signed off on it.** This class has been fixed
three times (`7277980`, the header-UX regression, `ff3e2ee`) and each fix was sound in
isolation while the class kept recurring. Patch number four is not the goal.

⭐ Note: the DPI matrix is a **standing pre-release gate** in `DevOps-CICD/`, **not**
phase 0.9 — 0.9 is Chromium prompt branding, a separate piece of work.

---

Paste the block below to open the session.

---

Do a **comprehensive assessment of display-scaling / DPI correctness** across Hodos,
then propose a plan. ⛔ **Research and adversarial review FIRST — do not write layout
code until I approve the assessment.**

We have fixed this class three times and it keeps coming back. I want it understood
properly once rather than patched a fourth time.

## 0. Read first

1. **Auto-loaded MEMORY.md** — especially `project_p08_consent_surface_round3_2026_08_23`
   (what just changed, and what was explicitly NOT verified) and
   `feedback_consent_surface_needs_human_eyes`.
2. `development-docs/DevOps-CICD/DPI_RESOLUTION_TEST_MATRIX.md` — the standing gate: the
   9 cells, simulation methods M1/M2/M3, the code-review checklist, and the History
   section, which is the record of this class recurring.
3. `development-docs/0.4.0-beta.3/MAC_RELAY_BETA3.md` §B4 — the macOS half and the open
   question already sent to that side.
4. Root `CLAUDE.md` — Testing Standards (the cross-DPI gate) and the overlay model.

## 1. ⭐ A gap I already found — start here, but do not stop here

The matrix's **pass criteria and its single programmatic assertion cover the
header/toolbar only.** They say nothing about **overlays or modals** — and overlays are
where every layout change of the last session landed:

- three checkboxes now share one wrapping row in *Default Limits for New Sites*;
- two consent labels in the connect modal now wrap;
- a `<label>` is a flex container where **each child is its own flex item** — this
  already shattered a line into per-word fragments once (see the round-3 memory), and
  the fix (wrap the text in one `<span>`) looks like redundant markup to anyone tidying.

⚠️ **Cells #4/#6/#9 have never been run against any of it.**

## 2. What the assessment must cover

Be comprehensive — that is the point of the session.

- **Best practice, researched properly** — Windows per-monitor DPI v2, `WM_DPICHANGED`,
  CSS logical vs device pixels, flex/grid shrink behaviour, minimum click-target sizes,
  and OS **text scaling**. ⚠️ Windows "Make text bigger" is a *separate* setting from
  display scaling — establish whether we honour it at all, because I do not know.
- **Every surface, not just the header** — header/toolbar, all ~14 overlays, the connect
  and payment consent modals, wallet panel, settings, dialogs.
- **macOS parity** — backing-scale factor, Retina/non-Retina, Display "Larger Text", and
  the fact that macOS overlays are borderless `NSWindow`s, not `WS_POPUP`.
- **The C++ side** — the matrix checklist invariants: `...ForDpi` metrics, `MulDiv` via
  `LayoutHelpers.h`, and the single `SetProcessDpiAwarenessContext` call.
  ⚠️ A recorded open follow-up says that call **discards its `BOOL` return**, so a silent
  fallback to UNAWARE is invisible. Confirm whether that is still true.
- **What can be automated** — the matrix has exactly one programmatic assertion. Decide
  honestly what can become a real gate and what needs human eyes, and say which is which.

## 3. Adversarial review — required before any plan is accepted

After drafting the assessment, attack it:

- Which findings are **claims** and which are **measurements**? Label every one.
- For each proposed fix: **what would make it wrong?** Would it regress a cell that
  currently passes?
- ⛔ **Every proposed check needs its negative control** — shown to go RED when the thing
  it guards is broken. This project has shipped **four** harnesses that would have passed
  with the feature absent (three farbling, plus a contrast probe whose injection silently
  no-op'd after a restyle). ⭐ **A control that cannot prove it injected anything is worth
  nothing** — assert the injection actually changed the source.
- ⚠️ **The guard-in-front-of-the-property trap**: a check can go red for the wrong reason
  and still look like a working control. T1f's first control did exactly this — a
  magnitude cap rejected the fixture before the unit rule was ever reached.
- ⚠️ **Assert the right SUBJECT.** Hodos's header and ~14 overlays are separate CEF
  browsers that CDP all reports as `type:"page"`. Driving the wrong one has faked a bug
  here before.

## 4. Deliverable

A written assessment I sign off on **before** any code:

1. current state per surface, claims and measurements separated;
2. gaps, ranked by likelihood x blast radius;
3. proposed changes — to code, to the matrix doc, and to what is gated in CI;
4. for each check, its negative control;
5. what stays manual, stated plainly rather than implied.

Then run the actual cells (#4/#6/#9 minimum, full sweep if the assessment argues for it)
and report **measured** results.

## 5. Machine state

- Dev stack was left running: wallet **31401 (schema V25)**, Vite **5137**, dev browser
  from `cef-native/build/bin/Release`. Re-check; it may be closed.
- ⛔ The owner's **installed** browser (`AppData\Local\HodosBrowser`, ~61 procs) and its
  wallet on **31301** must never be touched. Match by **exe path**, never by process
  name. ⛔ `cargo build` fails with "Access is denied" while the dev wallet runs — stop
  that PID by path first.
- ⚠️ The notification overlay is **keep-alive**: after a frontend change, restart the dev
  browser or it keeps serving the old JS.
- ⛔ Never commit `development-docs/X402_INTEGRATION.md` or
  `development-docs/Onchain-Backup-and-Sync/*` — the owner's parallel work. Stash and pop
  them around a rebase.
- **M2 is the fast lever**: `--force-device-scale-factor=1.5 --window-size=1366,728`, and
  do not maximize. ⛔ Never ship that flag. ⚠️ M1 (Windows Settings + sign-out) is higher
  fidelity and the only method that exercises the real startup `WM_DPICHANGED` — if a
  finding depends on that path, M2 is not sufficient evidence for it.

## 6. Working style

⭐ Walk me through anything I need to click **step by step**. I am doing the clicking.
Tell me what to do, what I should see, and what a wrong result would mean.
