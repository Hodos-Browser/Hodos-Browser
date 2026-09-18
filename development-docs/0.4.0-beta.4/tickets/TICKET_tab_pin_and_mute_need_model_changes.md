# 🎫 Tab pin and SITE mute have no data model — deferred out of Phase 4

> 🚨 **SCOPE CORRECTED 2026-09-01, after Phase 4 landed. ⛔ Mute *tab* is DONE and is no longer part
> of this ticket.**
>
> This ticket was filed covering pin + mute-tab + mute-site as one group. 📏 That grouping was
> **wrong**: mute-tab needed no model change at all, and this ticket's own "Proposed fix" section
> said so (*"The floor — mute tab only… self-contained and does not touch session restore"*).
> 👤 The owner caught it, asking why the item their user had asked for was missing.
>
> ⭐ **But the ticket was right that a `muted` field is needed — for a reason it did not state.** Its
> stated reason was persistence across restart. 📏 The real one, measured during implementation:
> **CEF's mute is per-DOCUMENT and clears on any navigation** (same-origin or cross-origin, same
> `CefBrowser`, no `OnBeforeClose`). So `Tab::muted` holds the user's intent and
> `OnLoadingStateChange` re-applies it. That is the "unverified" question in the method note below,
> now answered — see `0.4.0-beta.3/phase-4-tab-peripheral-parity/MEASUREMENTS.md` M14.
>
> **Still open here:** ⛔ **pin** and ⛔ **mute site** (per-domain) only.

**Found:** 2026-09-01, Phase 4 kickoff inventory
**Status:** ⬜ UNASSIGNED · **Track:** unassigned (beta.4 candidate) · **Filed by:** Phase 4 kickoff

> ⚠️ **Method note.** All claims here were **code reading** of the tree at filing time — `Tab.h`,
> `TabManager.h`, `simple_handler.cpp`. Nothing was run. ⛔ The item flagged "not verified" —
> whether `SetAudioMuted` survives — was **measured on 2026-09-01 and the answer was NO**, which is
> why the correction above exists. ⭐ The unverified line was the load-bearing one; a ticket that
> names what it did not check is what made that findable.

---

## What happens

Four of Chrome's tab context-menu items cannot be built from what exists:

| Item | Missing |
|---|---|
| **Pin tab** | 📖 `struct Tab` (`cef-native/include/core/Tab.h`) has **no `pinned` field**. Pinned tabs also sort ahead of unpinned ones, so `TabManager::ReorderTabs` gains a constraint it does not have |
| ~~**Mute tab**~~ | ✅ **DONE, beta.3 Phase 4** (`P4-A7`/`A8`/`A9`). 📏 The measured blocker was not persistence but **navigation**: CEF's mute is per-document, so `Tab::muted` holds intent and `OnLoadingStateChange` re-applies it. Session-lived; touches no storage |
| **Mute site** | 📖 needs per-domain storage. ⭐ `SitePermissionStore` is the existing per-host store; extend it. ⛔ Do **not** add a parallel top-level store — that is the mistake `TICKET_site_permission_dual_store.md` already documents |
| **Pin persistence** | 📖 `SaveSession` writes only `url` and `title` per tab, so a pin would not survive a restart |

## Why it matters

They are the two most-used items in Chrome's tab menu after Close. Their absence is the visible half
of the parity gap. But no user data is at risk and nothing degrades while they are missing — this is
a feature gap, not a defect.

## How exposed are we — answer this first

| If | Then |
|---|---|
| A user right-clicks a tab | They get the six items Phase 4 ships and notice pin/mute are absent |
| Anything else | **No impact.** Nothing silently misbehaves |

⇒ severity is **low and purely cosmetic**, which is exactly why 👤 the owner deferred it rather than
doubling Phase 4.

## What already protects us, and how that shapes the fix

The hard parts are already built: `TabManager::ReorderTabs` exists, `SetAudioMuted` exists,
`SitePermissionStore` exists with host normalisation, and the session v2 format already carries a
per-window tab array that could hold a flag.

⚠️ **The trap is `session.json`.** Persisting a pin means editing the same save/restore path that
carries an open defect — `TICKET_multiwindow_session_restore_loses_all_but_last_window.md`. Doing
both at once means a change to session restore whose two halves cannot be tested independently.
⇒ **fix the session-restore defect first, or persist pins somewhere else.**

## Proposed fix

~~**The floor** — mute tab only.~~ ✅ **Shipped in Phase 4**, in the shape described here plus the
re-apply-on-navigation the measurement forced.

**The system** — pin + mute site. Adds `pinned` to `Tab`, a pinned-first invariant in `ReorderTabs`,
per-domain mute in `SitePermissionStore`, and session persistence for pins. ⛔ Sequence it **after**
the session-restore ticket.

## Related

- `development-docs/0.4.0-beta.3/phase-4-tab-peripheral-parity/PHASE_CONTRACT.md` §1 — the deferral.
- `TICKET_multiwindow_session_restore_loses_all_but_last_window.md` — must land first if pins persist.
- `development-docs/0.4.0-beta.3/TICKET_site_permission_dual_store.md` — why not to add a second store.
