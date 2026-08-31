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

The user clicks Exit in window B and window A vanishes instead — including whatever they had open in
it. If A held the tabs they cared about, the loss is immediate and there is no undo. It is also the
mechanism by which the app has **no application-wide quit at all**: after Exit, the app is still
running, so a user who wanted to quit clicks Exit again and closes a second window.

⚠️ **It compounds a second defect.** Because there is no quit-all, the only path that reaches
`ShutdownApplication()` is closing the last window — which is precisely why multi-window session
restore loses everything but that window (sibling ticket).

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

**The floor** (one line each, two sites): resolve the owning window instead of `g_hwnd`.

```cpp
BrowserWindow* win = GetOwnerWindow();
HWND target = (win && win->hwnd) ? win->hwnd : g_hwnd;
PostMessage(target, WM_CLOSE, 0, 0);
```

⚠️ **But decide the product question first, because the floor may be the wrong answer.** "Exit"
conventionally means *quit the application*, not *close this window* — Chrome and Firefox both quit
everything. If that is the intent, the fix is not "post to the right window" but "add a real quit-all
path that closes every window", and the menu item stays application-wide. The floor above turns Exit
into a duplicate of the window's X button, which is arguably a worse UI than the bug.

⇒ 👤 **Owner decision needed** before coding: is Exit *quit the app* or *close this window*?
The sibling session-restore ticket wants the former.

## Related

- `development-docs/0.4.0-beta.3/phase-3.5-layout-window-scoping/MEASUREMENTS.md` **K18.1** — the run.
- `TICKET_window_scoped_work_uses_process_globals.md` — the parent pattern.
- `TICKET_multiwindow_session_restore_loses_all_but_last_window.md` — depends on the quit-all path.
