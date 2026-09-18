# <emoji> <One-line statement of the defect, in the present tense>

**Found:** <date>, <how — reading which file, running what, or reported by whom>
**Status:** ⬜ UNASSIGNED · **Track:** unassigned · **Filed by:** <who>

> ⚠️ **Method note.** State plainly which claims below are **code reading** and which are
> **measurement**, and name the one thing you did **not** verify. Delete this line only when you have
> actually written that note — not to tidy the file.

---

## What happens

<The defect, mechanically. File paths with symbol names — `file.rs :: symbol` survives edits, line
numbers do not. Quote the actual code where the code is the argument.>

## Why it matters

<User-observable consequence. If there isn't one, say so — some tickets are correctness debt with no
current user impact, and that is worth writing rather than inflating.>

## How exposed are we — answer this first

<What determines severity, and whether you know it. If unverified, say so explicitly and say what
would settle it. Severity claimed without this is a guess wearing a number.>

| If | Then |
|---|---|
| | |

## What already protects us, and how that shapes the fix

<Existing guards, if any. This is often the most useful section: it turns "build a system" into
"close a gap". If nothing protects us, say that.>

## Proposed fix

<Smallest change that closes it. Split "the floor" from "the system" where both exist — they usually
belong in different releases.>

**Deliberately out of scope:** <the things you were tempted by. Naming them makes scope creep visible
in the diff.>

## Test and negative control

⛔ Per `../../0.4.0-beta.3/HARNESS.md`: **a fix is not done until the check has been *seen* to fail.**

| | |
|---|---|
| **GREEN** | <what must be true> |
| **RED** | <the exact action that forces failure, and what you observed. Revert the fix, re-run, see it break — *for the right reason*> |
| **SUBJECT** | <what proves you measured the intended thing: which process, which build, which output, which profile> |
| **Tier** | <T0–T4, per HARNESS.md §3> |

**Standing invariant?** <If this defect could recur elsewhere or later, propose the row for
`../REGRESSION_ADDITIONS.md`. Most one-off fixes should not. Say which this is.>

## Links

<Related tickets, track docs, BRCs, prior sessions. Use paths that resolve today — check them.>
