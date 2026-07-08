# Mac ⇄ Windows relay (0.4.0) — cross-device coordination hub

Both the Windows Claude session and the Mac Claude session coordinate through THIS doc (committed to
`origin/0.4.0`). Pull before reading; push after writing.

---

## PLAN DECISION (2026-07-08): BATCH the Mac work — do NOT start Mac yet
Owner decision: the macOS **dropdown-button consistency** fix is DEFERRED and will be done TOGETHER with
the macOS parity pass of the **Option 2 same-process picker refactor**, in ONE Mac session, AFTER the
Windows session finishes the Option 2 design + adversarial review + Windows implementation. Rationale:
Option 2 is cross-platform (needs a dedicated Mac parity pass anyway), it's adjacent to the profile/overlay
code the buttons touch (batching avoids rework), and the buttons are cosmetic/no-urgency. **So there is
nothing for Mac Claude to start right now on these two items.**

## → FOR THE MAC CLAUDE SESSION (when the Windows Option 2 design lands here)
1. `git pull origin 0.4.0`.
2. Do BOTH together: (a) `MACOS_DROPDOWN_BUTTON_CONSISTENCY_BRIEF.md` (bring `menu`/`profile`/`download`
   to the keep-alive+toggle+0.3s-debounce pattern the other four use; don't touch the already-correct
   bookmark/site-info/tab-list mac branches or any Windows blocks), and (b) the **macOS parity** of the
   Option 2 picker refactor per the Windows design that will be posted here / in the plan doc.
   You IMPLEMENT + compile + smoke on macOS (this is `.mm`/`#elif __APPLE__` code that doesn't build on Windows).
3. Any OTHER independent, still-open mac briefs you CAN do now if you want (verify state first):
   `MACOS_0_4_0_EXECUTION_BRIEF_2026_07_07.md`, `MACOS_PORT_0_4_0.md`, `MACOS_UPDATE_STABILITY_EXECUTION.md`
   — but the buttons + picker parity are the batched item and wait for the Option 2 design.
4. When done: commit + `git push origin 0.4.0`, and **fill in "MAC → WINDOWS REPORT-BACK" below**.

## → FOR THE WINDOWS CLAUDE SESSION (heads-up)
- Mac work (dropdown buttons + Option 2 mac parity) is **batched and deferred** until you finish the
  Option 2 design + review + Windows implementation, then handed to Mac via this doc.
- **Before assuming any macOS state, `git pull origin 0.4.0` and read "MAC → WINDOWS REPORT-BACK".** Don't
  re-implement mac `.mm` code from Windows — Mac owns it (you can't compile it). When your Option 2 design
  is ready, post/point to it here so Mac can do the parity pass.

---

## MAC → WINDOWS REPORT-BACK (Mac Claude fills this in + pushes)
_(empty — Mac session to populate: date, commits, files changed, compile result, smoke result, blockers)_
