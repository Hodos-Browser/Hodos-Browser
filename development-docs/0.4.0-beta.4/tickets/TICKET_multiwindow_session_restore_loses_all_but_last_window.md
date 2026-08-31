# 🎫 Session restore keeps only the last window's tabs — every other window's tabs are lost on quit

**Found:** 2026-08-31, running beta.3 Phase 3.5's `P3.5-A4` row (the do-not-convert control)
**Status:** ⬜ UNASSIGNED · **Sprint:** unassigned (beta.4 candidate) · **Filed by:** Phase 3.5

> ⚠️ **Method note.** The **failure is MEASURED** — two windows with distinct external tabs, quit,
> and `session.json` came back holding one. The **cause is a CODE READING** of `ShellWindowProc`'s
> `WM_CLOSE` arm, not a stepped-through observation. ⛔ **Not verified:** whether any other quit path
> (Windows session end / `WM_QUERYENDSESSION`, task-manager close, crash-restore) reaches
> `SaveSession` with more than one window still populated. That is the one thing that could make this
> less severe than it reads, and it has not been checked.

---

## What happens

📏 **Measured**, dev build, `browser.restoreSessionOnStart = true`, two windows made with Ctrl+N:

| Window | Tab |
|---|---|
| primary | `https://example.com` |
| secondary | `https://www.iana.org` |

Both closed, then the log said:

```
📋 Session saved: 1 tabs across 1 windows
```

and `session.json` contained only `example.com`. Relaunching restored that one tab correctly, so the
restore half of the machinery is fine.

📖 **Cause, from the code.** `cef_browser_shell.cpp :: ShellWindowProc`, `WM_CLOSE`:

```cpp
if (windowCount <= 1) {
    // Last window — full graceful shutdown
    ShutdownApplication();          // <-- the ONLY caller path that reaches SaveSession()
} else if (wid == primaryId) {
    TransferPrimaryWindow(nextWid);
    for (Tab* tab : allTabs) if (tab->window_id == wid) TabManager::CloseTab(tab->id);   // tabs gone
    ...
} else {
    for (Tab* tab : allTabs) if (tab->window_id == wid) TabManager::CloseTab(tab->id);   // tabs gone
    ...
}
```

⇒ `SaveSession()` runs exactly once, when the **last** window closes. Every other window has already
had its tabs closed by one of the two branches above. `SaveSession` itself is **correct** — it
iterates `WindowManager::GetAllWindows()` and builds a v2 `windows[]` array explicitly designed to
hold several — but it can never be handed more than one populated window.

⭐ Both halves of the feature are written for multi-window and neither is wrong on its own.
The defect lives entirely in the **ordering** between them.

## Why it matters

A user with two or three windows open loses all but one of them on every quit. Session restore is a
feature they have deliberately turned on, so the failure is silent *and* contradicts an explicit
setting. There is no error, no warning, and the restored window looks entirely normal — which is why
this survived: the single-window case, which is what anyone tests, works perfectly.

## How exposed are we — answer this first

| If | Then |
|---|---|
| The user runs one window (the common case) | **No impact.** Restore works exactly as advertised |
| The user runs 2+ windows and has restore ON | Data loss on every quit — all but one window's tabs |
| The user runs 2+ windows and has restore OFF (⚠️ the current dev default, and it is worth checking what the shipped default is) | No impact; nothing is saved either way |

⛔ The shipped default for `browser.restoreSessionOnStart` was **not** checked. `useSettings.ts`
initialises it `false`, but that is the frontend's fallback, not necessarily what `SettingsManager`
writes on a fresh profile. Settle that before assigning severity.

## What already protects us, and how that shapes the fix

Nothing protects against it, but two things make the fix small:

- `SaveSession()` already handles N windows correctly — no change needed there.
- The v2 `windows[]` restore path in `simple_app.cpp :: OnContextInitialized` already reconstructs
  several windows with per-window `x`/`y`/`width`/`height` and `activeTabIndex`.

⇒ this is **not** "build multi-window session restore". It is "call the existing save before the
existing teardown starts".

## Proposed fix

**The floor.** Call `SaveSession()` once at the *start* of an application-wide quit, before any
window's tabs are closed — i.e. on the first `WM_CLOSE` that is going to end the process, not the
last. That needs a notion of "the app is quitting" distinct from "this window is closing", which
`g_app_shutting_down` almost is.

⚠️ **The hard part is that there is no application-wide quit today** — see the sibling ticket on
`PostMessage(g_hwnd, WM_CLOSE)`: menu → Exit closes only the primary window. Closing N windows is N
independent user actions, and after each one the surviving windows are legitimately still running. So
"save at the start of quit" has no unambiguous trigger until a real quit-all path exists.

**The system.** Give the app a genuine quit-all command (menu → Exit closes *all* windows), have it
snapshot the session first, then tear down. That also fixes the sibling ticket. Until then, a cheaper
partial: have each window's `WM_CLOSE` merge its own tabs into a persisted session file rather than
writing the whole file once at the end — worse in that a closed-on-purpose window would linger in the
restore set, which is a product question, not a technical one.

## Related

- `development-docs/0.4.0-beta.3/phase-3.5-layout-window-scoping/MEASUREMENTS.md` **K17** — the run.
- `TICKET_window_scoped_work_uses_process_globals.md` — the parent window-scoping ticket.
- `TICKET_menu_exit_closes_primary_not_the_clicked_window.md` — the missing quit-all path, which this
  fix depends on.
