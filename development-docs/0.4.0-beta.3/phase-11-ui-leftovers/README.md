# Phase 11 — UI / layout leftovers (was Phase 10; renumbered 2026-09-15)

**Opened:** 2026-09-15 as a folder; the bundle itself dates from 2026-08-31 (`../SPRINT_PLAN.md` §4.1).
**Status:** ⬜ PLANNED — kickoff not run. **Standard:** `../HARNESS.md`. Runs **after** Phase 10.

## What is in it

| # | Item | Source | Shape |
|---|---|---|---|
| 1 | **Cursor is not in the address bar at launch** — a test user had to click elsewhere first | `../TICKET_omnibox_addressbar_interaction_defects.md` #1 | ⭐ First-run blast radius, external reporter. Establish *which* defect first (focus never lands / first click lost / caret invisible) with an instrumented probe on the header browser at startup; then the fix. 👤 Owner's target: on launch, focus is in the address bar with the caret visible, ready to type. T2 probe + T3 human check |
| 2 | **Omnibox sometimes stays open after selecting a URL** | ticket #2 | reproduce before anything; the hide path (`omnibox_hide`) is the suspect, not Phase 3.5's create/show arm |
| 3 | **URL populates the address bar late after clicking a suggestion** | ticket #3 | measure the gap first (Phase 2's lesson: the control read 2.04 s); then send the URL with the click and set it optimistically. 👤 Owner: the page navigating while the bar still shows the old URL reads as "the click did nothing" |
| 4 | **Tab / Enter autocomplete behaviour** | ticket #4 | prior-art read (Chrome, Firefox, Brave omnibox key handling) before code |
| 5 | **Tear-off window overlay sweep** | 👤 owner 2026-09-15: typing in a torn-off tab's address bar makes that window disappear | Phase 3.5 measured and fixed this for Ctrl+N windows (K9: z-order occlusion; overlays now owned by the requesting window, owner-confirmed `Z5`). It was **never measured on a torn-off window** (`tab_tearoff` → `CreateFullWindow`, same creator). Re-run the Phase 3.5 `winprobe.ps1` z-order read on a torn-off window, **every overlay** (the 3.5 doc predicted "generalises beyond the omnibox" and said "not yet tested on a second overlay"), with `OwnOverlayToRequestingWindow` reverted as the negative control. If it is green, the owner's observation predates the fix and the row closes; if red, tear-off differs and gets fixed here |
| 6 | `modal_buttons_unclickable_small_screen` | old bundle | may already be covered by 7a's viewport work — verify, do not re-do |
| 7 | ❔ `chrome_ui_scales_but_its_window_does_not` | old bundle | may collapse into 3.5 — verify |
| 8 | ❔ `longlived_surfaces_snapshot_state_at_startup` | old bundle | |
| 9 | ❔ `disable_features_autofill_is_a_noop` | old bundle | |
| 10 | Phase 1's overlay dead strip · the DPI matrix overlay section | old bundle | `DPI_RESOLUTION_TEST_MATRIX.md` cells #4/#6/#9 |

## Harness notes

- Items 1–5 are **five separate defects sharing a surface** (the ticket says so); one contract, one row each, no
  single "omnibox fixes" branch.
- Item 1 is the one with a T3 human row: a person launching the installed build and typing without clicking.
  Its instrumented half: a startup probe that reads which HWND has focus and whether the header's `<input>` is
  `document.activeElement` at first paint, RED = the current binary.
- Item 5's instrument exists (`../phase-3.5-layout-window-scoping/` winprobe); reuse it, do not write a new one.
- Hard-reload before every React measurement (Vite HMR fakes negative controls — memory).
- 🍎 macOS: items 1–4 are React + header focus (CEF focus handling differs; relay for their eyes); item 5 has
  its own macOS half already in `HUMAN_TEST_QUEUE.md` B4.
