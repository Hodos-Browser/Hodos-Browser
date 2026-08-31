# 🎫 Menu → Exit closes the primary window, not the window it was clicked in

**Found:** 2026-08-31, running beta.3 Phase 3.5's evidence rows (incidental)
**Status:** ⬜ UNASSIGNED · **Sprint:** unassigned (beta.4 candidate) · **Filed by:** Phase 3.5

> ⚠️ **Method note.** **MEASURED**: the `exit` IPC sent from window B's header closed window **A** and
> left B running. The two-line **cause is a code reading**, though a direct one — the handler contains
> a literal `g_hwnd`. ⛔ **Not verified:** macOS, which takes a different arm
> (`ShowQuitConfirmationAndShutdown()`) and may not have this defect at all.

---

## What happens

📏 With two windows open, sending `exit` from **window B's** header closed **window A**. B stayed
running. 📖 Both call sites hardcode the primary window's HWND, `simple_handler.cpp`:

```cpp
// the "exit" IPC
extern HWND g_hwnd;
PostMessage(g_hwnd, WM_CLOSE, 0, 0);

// menu_action, action == "exit" — same two lines
extern HWND g_hwnd;
PostMessage(g_hwnd, WM_CLOSE, 0, 0);
```

`g_hwnd` is the **primary** window, never the window whose menu was clicked. The correct handle is
already in hand three lines away: every neighbouring arm in this file resolves `GetOwnerWindow()`.

⭐ This is a named instance of the pattern in `TICKET_window_scoped_work_uses_process_globals.md` —
*"the scattered `PostMessage(g_hwnd, WM_CLOSE)` and friends"*, which beta.3 Phase 3.5 explicitly
fenced out of scope (`PHASE_CONTRACT.md` §7). It is filed separately now because it stopped being a
code reading and became an observation.

## Why it matters

⛔ **This is NOT the window X button.** Clicking X closes only that window, correctly, in Hodos —
📏 verified (`ShellWindowProc`'s `WM_CLOSE` secondary arm closes just that window's tabs). This ticket
is only about the **`Exit` item in the three-dot menu** (`MenuOverlay.tsx`, label `"Exit"`).

The user clicks Exit in window B and window **A** vanishes instead — including whatever they had open
in it. If A held the tabs they cared about, the loss is immediate and there is no undo. B, the window
they were actually looking at, is still there, so the action reads as "nothing happened, and something
else broke".

It is wrong under **either** reading of what Exit should mean, which is why the fix is not blocked on
settling that:

| If `Exit` means | Correct behaviour | What happens today |
|---|---|---|
| quit the application | close **all** windows | closes one window, the wrong one |
| close this window | close **B** | closes **A** |

## How exposed are we — answer this first

| If | Then |
|---|---|
| One window open | **No impact** — `g_hwnd` *is* the window you clicked in |
| 2+ windows, Exit used from a secondary | Wrong window closes; its tabs go through the normal close path (they are not saved — see sibling ticket) |
| macOS | ⛔ **Unknown.** Different arm, not tested |

## What already protects us, and how that shapes the fix

`GetOwnerWindow()` already exists on `SimpleHandler` and is used by ~20 neighbouring arms in the same
`OnProcessMessageReceived` switch, including every overlay show path. The `WM_CLOSE` arm in
`ShellWindowProc` already distinguishes last-window / primary / secondary correctly, so posting to the
right HWND is all that is needed — the receiving side needs no change.

## Proposed fix

**The floor** (one line each, two sites — the `exit` IPC and `menu_action`'s `"exit"` arm): resolve
the owning window instead of `g_hwnd`.

```cpp
BrowserWindow* win = GetOwnerWindow();
HWND target = (win && win->hwnd) ? win->hwnd : g_hwnd;
PostMessage(target, WM_CLOSE, 0, 0);
```

That makes Exit close the window it was clicked in, which is 👤 **the owner's stated expectation
(2026-08-31)** and is unambiguously better than today under either reading above.

⚠️ **One product question is left open, and it does not block the floor:** a menu item labelled
*Exit* next to an X button that does the same thing is redundant — `Exit` in a browser's ⋮ menu
conventionally means *quit the whole application*. ⛔ **Do not settle this from memory.** Per working
rule #5, check what Chrome/Firefox/Vivaldi actually do with their menu Exit/Quit item before choosing,
and record it in `PRIOR_ART.md`. An earlier draft of this ticket asserted Chrome's behaviour without
checking and was wrong about it.

## Related

- `development-docs/0.4.0-beta.3/phase-3.5-layout-window-scoping/MEASUREMENTS.md` **K18.1** — the run.
- `TICKET_window_scoped_work_uses_process_globals.md` — the parent pattern.
- `TICKET_multiwindow_session_restore_loses_all_but_last_window.md` — related, but ⛔ **not** blocked on this one; see its own Proposed fix.
