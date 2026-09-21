# TICKET — the info-icon tooltip in the permission modal overflows the modal and adds a dead scrollbar

**Opened** 2026-09-21 by the **owner**, at the keyboard, during the W4/W7 sitting. **Status:** ⬜ OPEN,
not investigated. **Severity:** LOW (cosmetic), but it is on the **consent surface**.
⛔ Separate from W4/W7 — noted so it is not lost, and deliberately **not** chased mid-sitting.

## What the owner saw

On the connect/permission modal (example.com, Windows dev build): hovering the **ⓘ info icon** opens
its tooltip, and the tooltip **extends past the right edge of the modal**. That adds a **horizontal
scrollbar** at the bottom of the modal — and the scrollbar **cannot actually be scrolled**.

## Expected

The tooltip stays **inside** the modal's bounds: when the icon is near the right edge, the tooltip opens
**leftward** (or is clamped) so nothing overflows and no scrollbar appears.

## Notes for whoever picks it up

- Consent surface ⇒ [[feedback-consent-surface-needs-human-eyes]]: gates guard the rule, not the
  screen. Verify by eye, not only by a DOM measurement.
- Check at the DPI cells too (`DPI_RESOLUTION_TEST_MATRIX.md` #4/#6/#9) — a tooltip that fits at 100%
  can overflow at 150%.
- 👤 Owner flagged it as a possible rabbit hole. Scope it to "tooltip stays inside the modal"; do not
  redesign the tooltip system.
