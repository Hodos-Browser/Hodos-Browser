# beta.4 tickets

**Opened:** 2026-08-29.

This folder replaces beta.3's flat `TICKET_*.md` at folder root. The difference is not cosmetic:

> ⛔ **A ticket is not work until the owner has assigned it to a sprint.**
> This folder is a **review queue**, not a backlog anyone picks from.

---

## How it works

1. Anyone (session or owner) files a ticket here from `TICKET_TEMPLATE.md`.
2. It sits at **Sprint: unassigned**.
3. **The owner reviews and assigns it** to a sprint, to the misc bucket, or closes it.
4. Only then does it become work, and only inside the sprint it was assigned to.

⚠️ The owner has items on paper that are not here yet. ⛔ **Do not chase them.** They arrive when he
files them.

## Naming

`TICKET_<short_snake_case_description>.md` — the beta.3 convention, kept, because it greps well and
existing links use it. Describe the **symptom or the defect**, not the fix:
`TICKET_recovery_sweep_ignores_classification.md`, not `TICKET_add_classification_to_sweep.md`.

## Status values

| Status | Meaning |
|---|---|
| ⬜ **UNASSIGNED** | Filed, not yet reviewed by the owner. The default. |
| 📌 **ASSIGNED** | Owner has put it in a sprint. Names which one. |
| 🚧 **IN PROGRESS** | Being worked, inside its sprint's phase structure. |
| ✅ **CLOSED** | Fixed, with evidence, or closed with a written reason. |
| ❄️ **DEFERRED** | Deliberately not now, **with a re-check condition** — the `WATCH_fungibles.md` pattern. A deferral with no condition is a ticket rotting. |

## Rules

- ⛔ **State method.** Say whether a claim is a **code reading** or a **measurement**
  (`HARNESS.md` §8). Both are legitimate; mislabelling one as the other is not. The dust-consolidator
  ticket does this well — it opens with a method note naming the one thing it did *not* verify.
- ⛔ **Say what you did not check.** A ticket that reads as complete when it is partial is worse than
  a short one.
- **Every ticket that proposes a fix proposes its negative control.** Per `HARNESS.md`: not done
  until the check has been *seen* to fail.
- **Do not size a fix you have not scoped.** "Small" is a claim.

## Related tickets living elsewhere

| Ticket | Where | Why it is not here |
|---|---|---|
| `TICKET_token_outputs_destroyed_by_dust_paths.md` | `../../0.4.0-beta.3/` | Its **minimal defensive floor** ships in beta.3 (owner, 2026-08-29). The **full classification guard** is beta.4 sprint 1. ⛔ Read-only from here — another session owns that folder. ⏳ Its "Links" section still points at the pre-move ordinals path; that fix is owed to whoever next touches beta.3. |

---

## Register

| Ticket | Status | Sprint | Filed |
|---|---|---|---|
| `TICKET_e2e_specs_wrong_subject_and_never_run.md` | ⬜ UNASSIGNED | — | 2026-08-29 |

⚠️ The one ticket here is **second-hand** — reported by research (c), **not independently verified**.
Its first proposed step is verification. That is deliberate: a ticket may record an unverified report
as long as it says so.
