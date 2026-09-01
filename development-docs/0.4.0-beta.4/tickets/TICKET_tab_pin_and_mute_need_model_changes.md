# 🎫 Tab pin and tab/site mute have no data model — deferred out of Phase 4

**Found:** 2026-09-01, Phase 4 kickoff inventory
**Status:** ⬜ UNASSIGNED · **Sprint:** unassigned (beta.4 candidate) · **Filed by:** Phase 4 kickoff

> ⚠️ **Method note.** All claims here are **code reading** of the current tree — `Tab.h`,
> `TabManager.h`, `simple_handler.cpp`. Nothing was run. ⛔ **Not verified:** whether CEF's
> `SetAudioMuted` survives a tab's browser being recreated, which decides whether mute needs
> persistence or only in-memory state.

---

## What happens

Four of Chrome's tab context-menu items cannot be built from what exists:

| Item | Missing |
|---|---|
| **Pin tab** | 📖 `struct Tab` (`cef-native/include/core/Tab.h`) has **no `pinned` field**. Pinned tabs also sort ahead of unpinned ones, so `TabManager::ReorderTabs` gains a constraint it does not have |
| **Mute tab** | 📖 no `muted` field. The CEF call exists — `SetAudioMuted` is already used on the shutdown path — but nothing records or restores the state |
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

**The floor** — mute tab only. In-memory `muted` on `Tab`, `SetAudioMuted` on toggle, no persistence,
no ordering. Self-contained and does not touch session restore at all.

**The system** — pin + mute site. Adds `pinned` to `Tab`, a pinned-first invariant in `ReorderTabs`,
per-domain mute in `SitePermissionStore`, and session persistence for pins. ⛔ Sequence it **after**
the session-restore ticket.

## Related

- `development-docs/0.4.0-beta.3/phase-4-tab-peripheral-parity/PHASE_CONTRACT.md` §1 — the deferral.
- `TICKET_multiwindow_session_restore_loses_all_but_last_window.md` — must land first if pins persist.
- `development-docs/0.4.0-beta.3/TICKET_site_permission_dual_store.md` — why not to add a second store.
