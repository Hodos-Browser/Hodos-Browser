# 📋 ROUND 2026-08-30 (Windows) — Phase 3 (WS2): window / instance / focus identity

Its own file so it cannot conflict with `MAC_RELAY_BETA3.md`, `MAC_RELAY_P1_ROUND.md` or
`MAC_RELAY_P2_ROUND.md` if you are editing those.

Detail: `phase-3-window-identity/{PHASE_CONTRACT.md,MEASUREMENTS.md}`.

---

## 👉 One-line ask

**Phase 3 is Windows-only by design** (`SPRINT_PLAN.md` §3: macOS Spaces is a different mechanism and
the taskbar item has no macOS analogue), so this round adds **no work to your queue**. But it found a
defect class that **almost certainly exists on macOS too**, and I changed a shared function's
signature, so you need to know both. ⚠️ Everything below is **written and compiled for Windows and
has never been executed on macOS.**

---

## ⚠️ M1 — I changed a signature you share. It compiles on Mac, but read why.

`HandleFullscreenChange(bool)` → **`HandleFullscreenChange(BrowserWindow*, bool)`**, because the
shared caller `SimpleHandler::OnFullscreenModeChange` (in `simple_handler.cpp`, cross-platform) now
resolves the owning window and passes it. Without the macOS signature change the Mac build would
fail to link.

I made the **minimum** change on your side: the parameter is accepted and used to record
`win->is_content_fullscreen`, and **the macOS body is otherwise untouched**.

⛔ **I did NOT convert the macOS body, deliberately.** It still drives `g_main_window`,
`g_header_view`, `g_webview_view` and `TabManager::GetActiveTab()` — all process-globals. I cannot
test macOS from here, the window model is structurally different (NSWindow + presentation options
vs HWND + child windows), and a blind cross-platform "fix" is precisely the failure this project
keeps paying for. It is commented as such at the function.

## 🚨 M2 — The defect, and why I think you have it

A second launch of the **same profile** does not start a second process. `SingleInstance` forwards
over a named pipe and the running process opens a **second window**. So two windows of one profile
share one process — and any code asking a process-global *"which window am I?"* can act on the wrong
one.

👤 **Owner-observed on Windows, 2026-08-26**, two symptoms:

| # | Symptom |
|---|---|
| 1 | **Ctrl+F** in the second window opened the find bar in the **first** and raised it, switching virtual desktops. *"Nothing happens in the second one"* — the keypress was consumed and acted on elsewhere |
| 2 | **HTML5 video fullscreen** in one window fullscreened the **other**, hid its header, and resized its tabs to the *primary* window's rect — visibly wrong across two monitors |

**Fixed on Windows** by resolving `GetOwnerWindow()` instead of `GetPrimaryWindow()`, and by moving
the fullscreen flag onto `BrowserWindow`.

### ⭐ Your code was already right about one thing, and it shaped our fix

Windows had a **single** `g_is_fullscreen`. macOS has **two** — `g_content_fullscreen` and
`g_native_fullscreen`. That distinction turned out to be load-bearing: Windows' three-dot-menu
fullscreen button read the flag but never wrote it, so it was a **one-way trap** (enter, never
exit). The obvious one-line fix — just set the shared flag — would have been **wrong**, because
`WM_SIZE` expands tabs over the whole client area whenever the content flag is set, so the menu
button would have started covering the header.

⇒ Windows now has `BrowserWindow::is_content_fullscreen` **and** `is_window_fullscreen`, matching
the model you already had. Noted here because it is the second time this sprint the macOS side
carried the better structure.

### 🙋 M2.1 — the ask (low priority, no deadline)

**Does symptom 1 or 2 reproduce on macOS?** Open two windows of the **same** profile and:

1. `Cmd+F` in the second window — does the find bar open in the second, or the first?
2. Fullscreen an HTML5 video in one — does the other window's header disappear or its content resize?

⚠️ **Two different profiles is NOT the test** — those are separate processes and would pass while
proving nothing. It must be two windows of the **same** profile, i.e. one process.
⚠️ And confirm the second launch actually forwards on macOS rather than starting a second process —
if macOS starts a second process, you do not have this defect at all and that is a useful answer.

## 📏 M3 — Scale, in case you go looking

Measured on the **Windows** tree by `scripts/preflight.ps1` (gate `G11`, baseline **60**): the
multi-window migration is roughly **half done** — `TabManager::GetActiveTabForWindow(id)` and
`GetOwnerWindow()` already exist and are already used in ~18 places, against ~19 global uses on the
tab axis alone, plus 18 static `Get*Browser()` accessors that all route through
`GetPrimaryWindow()`.

⛔ **The gate is Windows-only on purpose.** Adding `.mm` would produce a large baseline that hides
the lines that matter — the same reasoning as `G8`. If you decide macOS has the defect, it wants its
own gate with its own baseline, not a widened `G11`.

⚠️ **Do not trust my hand counts.** I published "137" for this and it was a raw `grep -c` including
`extern` declarations; corrected to ~110, and then the tool measured 60 for the two patterns it
matches. Baseline with the tool.

Ticket for the remainder: `../0.4.0-beta.4/tickets/TICKET_window_scoped_work_uses_process_globals.md`.

---

## ⛔ What I am NOT claiming

- **Nothing here has run on macOS.** Not the signature change, not the two-flag model. The Mac build
  is expected to compile; that is a code reading, not a measurement.
- **I do not know whether macOS forwards a second same-profile launch into one process.** The whole
  defect depends on that, and it is the first thing to check (M2.1).
- **The Windows fix is not fully verified either.** `P3-A9` (two windows, two monitors) and
  `P3-A4/A5` (a real single-profile install) are still owed at the time of writing.

## Still open from earlier rounds — not superseded by this one

**D1** (overlay sizing contract), **D2** (Retina bottom-of-panel click), **D3.1**, **D3.2**, Sparkle
2.9.6 green + its negative control, your call on §A4 (Big Sur), `T1g` on macOS, and from P2:
**D5.1**/**D5.2** — including the orphan-sweep marker depth you found, which belongs to whoever picks
up the profile work and is **not** Phase 3.
