# Phase <N> — <title> · PHASE CONTRACT

**Workstream:** <WSx> · **Ticket:** `<TICKET_*.md>` · **Status:** ⬜ NOT STARTED / 🚧 IN PROGRESS / ✅ SIGNED OFF
**Opened:** <date> · **Owner:** <who> · **Platforms:** Windows / macOS / both
**Standard:** `../HARNESS.md`. Read it before filling this in.

---

## 1. Goal

<One sentence, user-observable. What is true for a user afterwards that is not true now.>

## 2. Done means

<Results, not activities. Each line measurable. "Deleted the writes" is an activity;
"`{app}` contains no volatile files after a 2-hour session" is a result.>

- [ ]
- [ ]

## 3. Invariants preserved

<Named, from `../REGRESSION_SET.md`. Say which apply and why. Generic reassurance is banned.>

| ID | Invariant | Why this phase could break it |
|---|---|---|

## 4. Evidence table

⛔ No empty RED or SUBJECT cells. A green result is reported with its red half or not at all.

| ID | 🟢 GREEN — must be true | 🔴 RED — must be *seen* to fail, and how | 🎯 SUBJECT — proves the right thing was measured | Tier | Result |
|---|---|---|---|---|---|
| `P<N>-A1` |  |  |  | T0–T4 | ⬜ |

**Two-sided rows:** where the requirement is "A must happen AND B must not", write both and make each
the other's control. Note the pairing here.

## 5. Blast radius

<What this touches that it is not about. Files, subsystems, call sites. Be specific enough that a
reviewer can check you looked.>

## 6. Out of scope

<Explicit, so scope creep is visible in the diff. Include the things you were tempted by.>

## 7. Rollback

<How to undo in one commit. If you cannot say it in two lines, the phase is too big — split it.>

---

## Sign-off

- [ ] Every evidence row GREEN **and** its RED observed
- [ ] `scripts/preflight.ps1` run — result + date recorded below
- [ ] `scripts/preflight.ps1 -NegativeControl` run — every T0 gate seen to fail
- [ ] `../REGRESSION_SET.md` run in full at this boundary — result recorded
- [ ] Adversarial review complete, four questions answered in writing
- [ ] Any baseline lowered in `../HARNESS.md` §4, residuals listed with reasons
- [ ] Commit messages cite the row IDs they satisfy

| Item | Result | Date | By |
|---|---|---|---|
| preflight | | | |
| preflight -NegativeControl | | | |
| regression set | | | |
| adversarial review | | | |
