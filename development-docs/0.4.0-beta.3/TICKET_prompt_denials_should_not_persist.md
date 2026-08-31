# "Allow this time" is a lie, and prompt denials stick, for location / notifications / clipboard

**Status:** ✅ **CLOSED** — Phase 0.9. A denial blocks **in-memory for the session only** (`HttpRequestInterceptor.cpp :: `"blocked in-memory for this session"`) and writes no `domain_permissions` row; confirmed live in the 2 → 3 regression run. Labelled 2026-08-31 (had no status line).

**Opened 2026-08-24** from Phase 0.9. **Status:** 🔵 OPEN.
Loopback / local-network were fixed in 0.9; these three were deliberately left alone because the 0.9
contract forbids changing the five existing prompts. Camera/mic are **not** affected.

## Two defects, one cause

CEF exposes only `ACCEPT / DENY / DISMISS / IGNORE`. **There is no "grant once".**

1. **"Allow this time" persists.** ACCEPT makes Chromium write a permanent content setting.
   MEASURED on loopback: clicking "Allow this time" for `example.com` produced
   `loopback_network → ALLOW`, surviving restart. Location / notifications / clipboard take the same
   code path, so they carry the same lie.
2. **"Don't allow" persists.** DENY writes a permanent BLOCK. Per the owner standard below, a
   decision made in a prompt should be temporary.

Camera/mic are exempt: they ride `OnRequestMediaAccessPermission`, whose `Continue()` persists
nothing, so their "Allow this time" is truthful.

## The standard to apply

> "Decisions based on prompts are temporary and get re-prompted; overt user actions stay until the
> user overtly changes them back." — owner, 2026-08-24

Rationale: a user answering a prompt was interrupted and may not have understood the ask; they should
not have to find a settings panel to undo a snap judgement. Someone who meant "no" simply does not
return to the site. A denial in the **Site controls panel** is deliberate and stays put.

## The fix

Mirror Phase 0.9's treatment:

- Drop "Allow this time" for these three in `BRC100AuthOverlayRoot.tsx` — the `noOnce` flag on the
  `PERM` map already exists, add these three types to it.
- Resolve a prompt denial with `CEF_PERMISSION_RESULT_DISMISS` instead of `DENY`, and do not write a
  `Block` row. `simple_handler.cpp :: promptDenialIsTemporary` already implements this; widen the
  type check.

## Negative control

Block a site's notifications **at the prompt**, then revisit. Before: no prompt, permanently blocked.
After: prompted again. MEASURED for loopback — `13:27:12 permission_response 'block'` →
`13:27:16 OnShowPermissionPrompt`, re-asked 4 s later.

## Related

`TICKET_site_permission_dual_store.md` — same three types, and the panel that cannot revoke them.
