# Edit Limits modal (Approved Sites) — long, losable, and it discards work silently

**Status:** 🔵 OPEN — filed 2026-09-04 from owner live review of the Phase 7b connect work.
**Sprint:** 📌 Not yet assigned. Candidate for **Phase 7d** (it is the other half of the consent
surface — the *management* half) or beta.4.
**Surface:** `DomainPermissionForm.tsx` inside `DomainPermissionsTab` → `ApprovedSitesTab`
(wallet overlay) **and** the right-click *"Manage Site Permissions"* path. One component, two entry
points — fix once, lands twice.

---

## Why now

Phase 7b made the **connect** screen tell one truth. This is the screen the user visits *afterwards*,
and it has the opposite problem: it is where a real site's grants are managed, and a real site has a
lot of them. Measured live 2026-09-04, `beta.zanaadu.com` alone: **10 protocol grants + 6 basket
grants + 1 certificate**, each rendered as its own row with a Revoke button. The modal is already
longer than the screen.

⚠️ Phase 7a's `cardStyle` cap means it now **scrolls rather than clipping** — so this is a usability
ticket, not a safety one. It was a safety one until `c7ce5c6`.

## The four items

### 1. 🚨 Click-outside discards unsaved changes with no warning

The modal closes on click-outside. With 17 rows and a set of limit fields, a user can spend a minute
adjusting things and lose all of it by clicking one pixel outside. **No confirmation, no toast, no
recovery.**

**Owner-requested behaviour:** the modal closes **only** on Save or Cancel. A click outside raises a
centred toast — *"Click Save or Cancel to close"* — and does nothing else.

⚠️ Implementation note: this overlay's close paths are C++, not React (root `CLAUDE.md`, *Overlay
Lifecycle*). Suppressing click-outside means the **prevent-close flag** pattern
(`g_wallet_overlay_prevent_close`), and per that doc a React-set flag **cannot** reliably guard
`WM_ACTIVATEAPP` — the guard has to be set synchronously in C++, or set on open rather than on first
edit. ⛔ Do not implement this as a React `onClick` handler and assume it holds.

### 2. Sub-permission list needs its own scroll box

The per-grant list (protocols + baskets + certificates + counterparties) is unbounded. Same shape as
the connect modal's list before Phase 7b — cap it and let it scroll.

⛔ **Check for tooltips first.** Phase 7b found that `InfoIcon` renders its tooltip
`position:absolute` **outside** the icon's box, so an `overflow` on an ancestor clips it — that is
why the connect modal's counterparty block was *collapsed* rather than *capped*. If any sub-permission
row grows a tooltip, capping this list clips it.

### 3. Amounts / limits section should be collapsible

Four limit fields plus the provenance marking. Collapsing them by default shortens the common case
(the user came to revoke one thing, not to re-tune caps).

⛔ **`R-PROV` constraint:** Phase 0.8 contract §6a — *a user must not approve values hidden behind a
collapsed section.* The connect modal solves this with `shouldExpandLimits()`, which force-opens the
section whenever a **site-sourced** value is on screen. This modal shows the user's **own** stored
values, so the rule may not bite — **but it must be checked, not assumed**, and if a site-suggested
value can ever appear here the same force-open applies.

### 4. Store denials, so a declined permission is re-approvable from here

Today unticking an item at connect simply means **no row is written** (verified: Zanaadu's
`xanaverse-upvotes` is absent from `domain_basket_permissions`). So the management UI cannot
distinguish *"never asked for"* from *"user said no"*, and offers no way to turn it back on.

**Owner-requested:** persist the declined item and render it with an **Approve** button.

⚠️ This is a **schema** change (CLAUDE.md invariant #2 — ask first) and it collides with
`TICKET_prompt_denials_should_not_persist.md`, which says *prompt* denials must stay temporary.
⭐ The distinction that reconciles them: a **prompt** denial is a transient answer under time
pressure and must not stick; an **overt untick on the connect screen or in this panel** is a settled
preference and should. That distinction needs stating explicitly wherever it lands, or the two
tickets will be read as contradicting each other.

---

## Deferred, deliberately: always-deny

The owner asked for `always deny` beside `always allow` in the prompt, and then argued against
building it yet — correctly. A permanent deny with no discoverable way back silently breaks an app
for a reason the user cannot see, and the protocol offers no path to fix it from where the problem
appears.

⇒ Recorded as a **protocol** gap, not a UI backlog item:
`Marston Enterprises/Standards/BRCs/drafts/permission-necessity-and-denial-feedback/` — proposals 1
(necessity declaration) and 3 (re-prompt request) are the preconditions that make always-deny safe.

⛔ **Do not implement always-deny before those exist in some form.** The reason is written down there
so this decision is not re-litigated from scratch.

## Related

- `phase-7b-connect-modal/` — the connect half of this surface; `MEASUREMENTS.md` N17 has the live
  Zanaadu grant data that sizes this ticket.
- `TICKET_prompt_denials_should_not_persist.md` — see the §4 reconciliation.
- `TICKET_site_permission_dual_store.md` — different store, same panel; Phase 7d.
- Root `CLAUDE.md` §Overlay Lifecycle — the close-path rules that govern item 1.
