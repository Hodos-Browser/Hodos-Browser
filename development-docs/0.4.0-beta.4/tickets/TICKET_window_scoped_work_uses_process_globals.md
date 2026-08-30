# 🪟 Window-scoped work is performed against process-globals, so one window acts on another

**Found:** 2026-08-26 → 2026-08-30, during the beta.3 Phase 3 (WS2) kickoff — two symptoms reported by the owner at the machine, mechanism established by code reading, scale established by counting.
**Status:** ⬜ UNASSIGNED · **Sprint:** unassigned · **Filed by:** Phase 3 kickoff session

> ⚠️ **Method note.** The **two symptoms** are 👤 owner-observed measurements. The **mechanism** is a
> 📖 code reading — no debugger was attached. The **counts** are 📏 measured by `grep`, but they are
> *hand counts* and ⛔ **must be re-measured by `preflight.ps1` before any gate baseline is set from
> them** (`HARNESS.md` §9: two hand counts were wrong when the tool re-measured them).
> **Not verified:** whether macOS has the same defect. Its window model differs and was not examined.

---

## What happens

A second launch of the **same profile** does not start a second process. `SingleInstance` forwards
over a named pipe and the running process opens a **second window**
(`SingleInstance.cpp :: listener thread` → `cef_browser_shell.cpp` new-window path). So two windows
of one profile share one process.

Much of the browser-process code, asked "which window am I acting on?", resolves a **process-global**
instead of the window that owns the event. Work done in window B lands on window A.

⚠️ The pipe is keyed on `profileId`, so two **different** profiles are genuinely separate processes
and are unaffected. That asymmetry is why the original report described results as *"mixed and
inconsistent"*.

The clearest instance — `SimpleHandler::OnFullscreenModeChange` is handed the browser and **discards
it**:

```cpp
void SimpleHandler::OnFullscreenModeChange(CefRefPtr<CefBrowser> browser, bool fullscreen) {
    HandleFullscreenChange(fullscreen);   // <-- `browser` dropped; no window context survives
}
```

`HandleFullscreenChange(bool)` (`cef_browser_shell.cpp`) then operates on five globals:
`g_is_fullscreen`, `g_hwnd`, `g_header_hwnd`, `TabManager::GetAllTabs()`, `GetHeaderBrowser()`.

## Why it matters

Two user-visible defects, both 👤 observed:

| # | Symptom | Observed |
|---|---|---|
| 1 | **Ctrl+F** in the second window opens the find bar in the **first**, switching virtual desktops to do it. *"Nothing happens in the second one"* — the keypress is consumed and acted on elsewhere | 2026-08-26 |
| 2 | **HTML5 video fullscreen** in one window fullscreens the **other**, hides its header, and resizes its tabs to the *primary* window's rect — visibly wrong across two monitors | 2026-08-26 |

📖 A third, **latent and unobserved**: 12 sites compute overlay offsets as `ScalePx(x, g_hwnd)` —
DPI taken from the **primary** window. Two windows on **different-DPI monitors** ⇒ dropdown overlays
in the non-primary window are positioned with the wrong scale factor. ⛔ Not reproduced; it needs
the mixed-DPI matrix cell (`DPI_RESOLUTION_TEST_MATRIX.md` #9) **with two windows**, which no test
has ever run.

`g_is_fullscreen` is the sharpest illustration: one `bool` per **process**, so "window A fullscreen,
window B normal" is a state Hodos **cannot represent**. That is a missing variable, not a missing
feature.

## How exposed are we — answer this first

| If | Then |
|---|---|
| Users routinely run **two windows of the same profile** | Exposed continuously. The owner does, and hit two symptoms in one sitting without looking for them |
| Users run one window, or several **different** profiles | Not exposed — separate processes, each correctly resolving its own primary |
| Multi-monitor with **mixed DPI** | Additional latent exposure via the 12 `ScalePx` sites |

⇒ Severity tracks *"how many users open a second window of the same profile"*. **Unmeasured.** What
would settle it: the second-window path is already logged (`SingleInstance: Creating new window`),
so a count from real logs would answer it without new instrumentation.

## What already protects us, and how that shapes the fix

⭐ **This is the most useful section here: the architecture is already correct and half-adopted.**

| Exists today | Purpose |
|---|---|
| `BrowserWindow` | The per-window record — the right home for this state |
| `WindowManager` | The registry of windows |
| `SimpleHandler::GetOwnerWindow()` | "which window owns this handler" — **already used correctly in ~18 places** |
| `TabManager::GetActiveTabForWindow(int)` | The window-scoped counterpart of the global `GetActiveTab()` |

⇒ This is **an unfinished migration, not a redesign**. `simple_handler.cpp` says so itself:

> *"Static getters — redirect to WindowManager window 0 for backwards compatibility. Cross-browser
> IPC within a handler should use `GetOwnerWindow()` instead."*

⛔ **And some globals are correct.** `g_file_dialog_active` and `g_wallet_overlay_prevent_close` are
genuinely process-wide. The test is **not** "is it global" but **"does this have one value per
process, or one per window?"** A blanket conversion would be a defect.

## Scale — 📏 measured 2026-08-30 (hand counts, see method note)

| Global | Raw lines | **Real uses** | Note |
|---|---|---|---|
| `g_hwnd` | 77 | **52** | 25 raw lines are `extern HWND g_hwnd;` declarations, not logic |
| `g_header_hwnd` | 25 | **25** | all in `cef_browser_shell.cpp` |
| `TabManager::GetAllTabs()` | 16 | **15** | one is the definition |
| `TabManager::GetActiveTab()` | 19 | **18** | one is the definition; **18 correct `…ForWindow` calls already exist** |
| | | **~110** | |

⚠️ **An earlier statement of "137" was a raw line count and was wrong.** Corrected here.

## Proposed fix

**The floor** — already taken by beta.3, listed so this ticket's scope is unambiguous:

- beta.3 **Phase 3** — symptoms 1 and 2 only.
- beta.3 **Phase 3.5** — the **layout subsystem** cluster (`g_header_hwnd` + the `GetAllTabs()`
  resize/DPI/fullscreen sites in `cef_browser_shell.cpp`), where the density is and where one test
  story covers many sites.

**This ticket owns the remainder:**

1. The **scattered judgment-call sites** — each read individually, because the answer differs.
   `PostMessage(g_hwnd, WM_CLOSE)` is the type case: sometimes "close my window", sometimes "quit".
2. The **12 `ScalePx(x, g_hwnd)`** DPI sites (unless Phase 3.5 absorbs them — they are overlay
   layout, so it may).
3. Drive the `P3-G11` ratchet baseline toward **0**.
4. Decide the fate of the **18 backwards-compat static accessors**: retire, or keep with a lint.

**Deliberately out of scope:** retiring `WindowManager`/`BrowserWindow` in favour of a different
window model; anything on macOS; the `Tab`/`CefBrowser` identity mapping (`R-GOLD` territory).

## Test and negative control

⛔ Per `../../0.4.0-beta.3/HARNESS.md`: **a fix is not done until the check has been *seen* to fail.**

| | |
|---|---|
| **GREEN** | With two windows of one profile open: an action in window B (find, fullscreen, resize, DPI change, overlay open) changes **nothing** in window A — asserted on window A's client rect, header visibility and fullscreen state |
| **RED** | 👤 **Already observed for both symptoms** (see *Why it matters*). For each converted site, restore the global lookup on the **same binary** → the cross-window effect returns |
| **SUBJECT** | Two windows of the **same** profile, i.e. **one process** — confirm via `Win32_Process` that only one non-`--type=` browser process exists. ⛔ Two *different* profiles are two processes and would pass while proving nothing — that is this ticket's vacuous-test trap |
| **Tier** | T3 (human, two windows) + T0 (the ratchet gate) |

⭐ **Two-sided pairing.** "Window A is untouched" and "window B's own action actually worked" are
each other's control. A fix that simply ignores B's fullscreen entirely satisfies the first alone.

## The gate that makes deferral safe

`P3-G11` (lands in beta.3 Phase 3): a **ratcheted** T0 gate counting process-globals used for
window-scoped work. Baseline = the count when it lands; any **new** violation fails the build even
at a non-zero baseline (`HARNESS.md` §4).

⇒ Without it this ticket is a wish. With it, the count cannot silently grow while the ticket waits,
and this ticket's job becomes *"drive the baseline down"* rather than *"find them all again"*.

## Risks for whoever picks this up

- ⚠️ **R-CLOSE** — overlay close guards are globals that are **correct**. Do not convert mechanically.
- ⚠️ **R-GOLD** — `Tab::id` ≠ `CefBrowser::GetIdentifier()`. Window→tab changes risk the payment pill
  landing on the wrong tab.
- ⚠️ **CLAUDE.md invariant #8** — CEF lifecycle/threading is fragile; window creation timing is not to
  be changed casually.
- ⚠️ **macOS unassessed** — do not assume counts or fixes transfer.
