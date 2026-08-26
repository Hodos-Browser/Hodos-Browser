# Long-lived surfaces snapshot their state at browser start and are never told it changed

**Found:** 2026-08-25, by the owner, during the Phase 1 (WS1) test session.
**Reported symptom:** *"I changed the avatar on another profile. It did take, but it didn't refresh
in all of the places. I closed it and reopened it and the new avatar was there."*

⛔ **Not a Phase 1 defect** — Phase 1 is overlay input and DPI. This is state invalidation. Filed so
it is not lost, and because it is now the **third** instance of one pattern.

---

## The pattern, which is the real content of this ticket

> **A surface is long-lived (the header browser is created once at startup; overlays are keep-alive),
> but its state was written as if the component mounts fresh whenever the user looks at it.**

Every occurrence looks like an unrelated one-off. They are the same bug:

| # | Where | Symptom | Found |
|---|---|---|---|
| 1 | `/wallet/settings` in the connect modal | Default spending limits were a snapshot **as of browser start**; changing them did nothing until restart | P0.8 round 3 (owner, reading the screen) |
| 2 | Quiet-mode checkboxes in the connect modal | Previous site's choice still on screen for the next site | P0.8 round 3 (owner) |
| 3 | **Profile avatar / name / colour** | Editing a profile updates the panel you edited it in, but not the toolbar icon or other surfaces until reopen | **this ticket** |

⭐ Three for three, all found by a human looking at a screen, none by any gate. P0.8's write-up
predicted this: *"When touching a keep-alive overlay, ask what else is initialised once and assumed
fresh. Expect more of these."*

## Root cause for #3, verified

- `frontend/src/hooks/useProfiles.ts` — `useEffect(() => { fetchProfiles(); }, [])`. **Mount-only.**
  It sends `profiles_get_all` exactly once.
- `frontend/src/pages/MainBrowserView.tsx` — the toolbar profile button renders
  `currentProfile.avatarImage` from that hook. The header is a browser created once at startup and
  never remounted, so its `currentProfile` is a snapshot from browser start.
- **There is no invalidation broadcast at all.** `grep` for `profile_updated` / `profiles_changed` /
  `profile_list_update` across `frontend/src` and `cef-native/src` returns **zero** hits. Nothing
  tells any other surface that a profile changed.
- The editing panel appears correct only because it holds the value it just wrote locally.

## Why it is worth fixing beyond cosmetics

Alone, a stale avatar is cosmetic. But the profile indicator is **how the user knows which profile
they are in**, and profiles are a separation boundary — a stale indicator misidentifies the context
the user believes they are acting in. Combined with `TICKET_wallet_global_profiles_isolated.md`
(one `wallet.db` serves every profile), a user can be looking at profile A's avatar while acting in
profile B's browser state.

## Shape of the fix

⭐ **The precedent already exists** — do not invent a new mechanism. `domain_permissions` changes
already fire a cache-invalidation IPC (`project_domain_permission_cache_invalidation`), and the
tab list already pushes `NotifyWindowTabListChanged` on change rather than polling. Mirror that:
broadcast a `profiles_changed` IPC from the C++ profile mutation handlers, and have `useProfiles`
re-fetch on it.

⚠️ **Do the general sweep, not just the avatar.** The value of this ticket is the audit: enumerate
every `useEffect(..., [])` that fetches state in a **long-lived** surface (the header and all ~14
keep-alive overlays) and decide, per case, whether it needs an invalidation signal. Fixing only the
avatar leaves the pattern in place and guarantees a fourth instance.

⛔ **Refresh before render, never after** — the P0.8 lesson. Re-resolving after paint changes values
under the user's eyes, which on a consent surface is worse than the staleness it fixes.

## Related

- `TICKET_connect_modal_two_views_drift.md` · `TICKET_wallet_global_profiles_isolated.md`
- P0.8 round 3 write-up (defects 4 and 5 — the first two instances)
- `feedback_consent_surface_needs_human_eyes` — all three were found by eye, none by a gate
