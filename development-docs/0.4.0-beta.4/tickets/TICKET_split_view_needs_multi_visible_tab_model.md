# 🎫 Split view — our tab model allows exactly one visible tab per window

**Found:** 2026-09-01, 👤 owner raised it while reviewing the Phase 4 tab menu
**Status:** ⬜ UNASSIGNED · **Track:** unassigned (beta.4 candidate) · **Filed by:** Phase 4

> ⚠️ **Method note.** This is **code reading** plus one live observation of Chrome's feature. Nothing
> was prototyped. The cost estimate below is therefore a **shape**, not a number — treat "which
> subsystems change" as the reliable part and any effort figure as unmeasured.

---

## What was asked

👤 *"I am also looking at the 'add tab to new split view' function in Chrome, that is actually pretty
cool but we might not want to build that much right now."*

Chrome shows **two tabs side by side in one window**, with a draggable divider, each pane keeping its
own active tab, and a tab-menu entry to move a tab into the split.

## Why it is not a menu item

📖 It reads like one because Chrome surfaces it from the tab context menu. It is not.

📏 **Our whole tab model is "exactly one visible tab per window"**:

| Site | What it assumes |
|---|---|
| `TabManager::SwitchToTab` | `ShowWindow(SW_SHOW)` the target, `SW_HIDE` every other tab in the window — one visible, by construction |
| `Tab::is_visible` | a single bool per tab, with the invariant "only one true per window" stated in `TabManager.h` |
| `TabManager::CreateTab(url, parent, x, y, w, h, window_id)` | every tab is created at the **full** content rect below the header |
| `ShellWindowProc :: WM_SIZE` | resizes *the* active tab to the full client area minus the header |
| `TabManager::GetActiveTabIdForWindow(window_id)` | **one** active tab per window — a split needs one per *pane* |
| `SendTabListToWindow` | emits `isActive` as a single flag; the strip renders one selected tab |

⇒ split view is a change to the **window layout model**, not an addition to the tab menu. Every one of
those six sites has to learn about panes.

## What it would actually take

| Piece | Work |
|---|---|
| Pane model | A window gains N panes (2 is enough); each pane owns a rect and an active tab. `GetActiveTabIdForWindow` becomes per-pane, and `Tab` needs a pane assignment |
| Layout | `WM_SIZE` splits the content rect and sizes **two** tab HWNDs; a draggable divider HWND with hit-testing and a live-resize path |
| Tab strip | The strip must show which tabs are in which pane, and the "one selected tab" rendering becomes two |
| Focus / input | Two visible CEF browsers in one window — which has keyboard focus, and what `Ctrl+W` / `Ctrl+F` / find-bar target |
| Tear-off + reorder | `MoveTabToWindow` and `ReorderTabs` both currently reason about a flat per-window list |
| Session restore | ⛔ Panes would want persisting, which drags in `session.json` — **which carries an open defect** (`TICKET_multiwindow_session_restore_loses_all_but_last_window.md`) |
| 🍎 macOS | The same again in `cef_browser_shell_mac.mm`, whose window model is structurally different |

⭐ **The good news:** it needs **no new CEF capability**. Tabs are already separate windowed browsers
with their own HWNDs — showing two at once is a sizing and bookkeeping problem, not an engine one.
That is genuinely different from, say, the audio-state signal this CEF build does not expose.

## How exposed are we

| If | Then |
|---|---|
| A user wants two pages side by side today | They open a second window (`Ctrl+N`) and tile it themselves — 📏 which works, and Phase 3.5 made multi-window behave correctly |
| Anything else | **No impact.** Nothing misbehaves; this is a missing convenience |

⇒ severity **cosmetic / convenience**. 👤 The owner's own read — *"we might not want to build that
much right now"* — matches the code.

## Recommendation

⛔ **Do not build it inside a phase whose scope is a context menu.** If it is wanted, it is its own
scoping run (`SCOPING_PROCESS.md`) because it crosses `TabManager`, `WindowManager`, the WndProc
layout path, the tab strip, and both platforms.

⭐ **Sequence it after** the session-restore defect, for the same reason pin is sequenced after it:
otherwise two changes to session restore land together and their halves cannot be tested apart.

## Related

- `0.4.0-beta.3/phase-4-tab-peripheral-parity/PHASE_CONTRACT.md` §7 — out of scope for Phase 4.
- `TICKET_tab_pin_and_mute_need_model_changes.md` — the other tab-model ticket; same
  session-restore ordering constraint.
- `TICKET_multiwindow_session_restore_loses_all_but_last_window.md` — must land first if panes persist.
