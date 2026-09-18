# 🎫 Session restore keeps only the last window's tabs — every other window's tabs are lost on quit

**Found:** 2026-08-31, running beta.3 Phase 3.5's `P3.5-A4` row (the do-not-convert control)
**Status:** ⬜ UNASSIGNED · **Track:** unassigned (beta.4 candidate) · **Filed by:** Phase 3.5

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

⭐ **The structural difference is *when* the session is written, not what writes it.** Hodos snapshots
the whole session **once**, at shutdown. By then only one window still has tabs. A browser that saves
the session **continuously** — updating it as windows and tabs open and close — never has this
problem, because the file is already correct before the first window closes.

**The floor.** Update the persisted session on each window close as well as at shutdown, so a window
that closes early contributes its tabs before they are discarded. `SaveSession()` needs no change; it
is only ever called too late.

⚠️ **This raises a product question that must be answered first, because it decides the shape:**
if the user *deliberately* closes window B and then quits, should B come back on next launch?

| Answer | Implication |
|---|---|
| No — restore only what was open at quit | Then a per-close save must also *remove* that window. Closing B is a deliberate discard, and "Recently closed" is where it should live instead |
| Yes — restore everything from the session | Simpler to implement, but a window the user closed on purpose reappears, which most people read as a bug |

⛔ **Do not settle this from memory** — per working rule #5, check what Chrome and Firefox actually do
with a window closed mid-session, and record it in `PRIOR_ART.md`. An earlier draft of this ticket
guessed and was wrong.

⛔ **Not blocked on the `Exit` ticket.** An earlier draft claimed this fix depended on adding an
application-wide quit path. It does not: saving per window close fixes it regardless of what `Exit`
ends up meaning.

## Related

- `development-docs/0.4.0-beta.3/phase-3.5-layout-window-scoping/MEASUREMENTS.md` **K17** — the run.
- `TICKET_window_scoped_work_uses_process_globals.md` — the parent window-scoping ticket.
- `TICKET_menu_exit_closes_primary_not_the_clicked_window.md` — the other multi-window quit defect. Related, **not** a dependency.
