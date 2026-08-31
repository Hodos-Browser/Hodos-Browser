# Phase 3.5 — layout is window-scoped · PHASE CONTRACT

**Workstream:** WS2 (continued) · **Added:** 2026-08-30, by owner decision during the Phase 3 kickoff
**Status:** 🟡 **PLANNED — awaiting sign-off. No code written.** · **Owner:** Matthew · **Platform:** Windows only
**Standard:** `../HARNESS.md`. Measurements: `../phase-3-window-identity/MEASUREMENTS.md` M5, M6, M9.3, M9.4 + §1 below.
**Parent ticket:** `../../0.4.0-beta.4/tickets/TICKET_window_scoped_work_uses_process_globals.md`

---

## 0. Why this phase exists

Phase 3 fixes the **two symptoms the owner reported**. This phase fixes the **densest cluster of the
same defect** — the layout code — because that is where one coherent change and one test story cover
many sites at once. Everything still scattered after this goes to the beta.4 ticket, held by the
`P3-G11` ratchet.

⭐ **The owner's read was right and is worth recording:** *"it seems like a pattern we could easily
fix"*. For this subsystem it largely is. ⛔ For the rest it is not, which is why the rest is a
separate ticket and not this phase.

## 0.1 ⛔ RE-SCOPE — 2026-08-31, after measurement. This section SUPERSEDES §1.1's verdicts and §5.

Written after the kickoff measured the tree (`MEASUREMENTS.md` K1–K9). **Three of this contract's
original claims were refuted by measurement, and one defect it never named turned out to be the
worst one.** Amended per `HARNESS.md` §1 ("scope changes amend the contract in the same commit").

| Original scope item | Verdict after measurement |
|---|---|
| 🔴 overlay-reposition block, *"27 refs / 7 overlays"* | ⛔ **DROPPED.** Count is **47 / 9** (K1), and the block is **unreachable for secondary windows** — a primary-only guard sits 3 lines above it, so `g_x == bw->x` by construction and `P3.5-A1`'s RED **cannot be observed** (K3). The conversion is a no-op refactor and its test is vacuous |
| 🔴 `WM_SIZE` picker arm | ⛔ **DROPPED.** One picker window per process by construction (K2) — correct as a global |
| 12 × `ScalePx(x, g_hwnd)` | ✅ **KEPT** (K5). Still 📖 a code reading; its mixed-DPI reproduction has **not** been run (K7 — no mixed-DPI rig exists on the machine) |
| `P3.5-G11↓` | ⛔ **NOT SATISFIABLE.** `G11` scans neither this file nor these patterns (K4). Baseline stays **60**, reason recorded in `HARNESS.md` §4 |
| — *(not in the original contract)* | 🚨 **ADDED: the Z-order defect** (K9). Opening a dropdown from window B sends **B behind A**. 👤 Owner-observed, 📏 owner-run measurement, single monitor. **The most user-visible defect this phase has found** |

### 0.1.1 📏 Why "just run the startup overlay creation again for window B" does not work

👤 Owner's question, 2026-08-31: *"can we also just call the whole startup process again to fix it
with what we already have?"* ⭐ **The instinct is right — per-window overlays are the correct end
state, and `BrowserWindow`'s 14 overlay fields exist for exactly that.** But it cannot be reached by
re-calling today's functions:

📏 All 14 `Create*Overlay` functions are **hard-bound to the primary window** — 23 `GetPrimaryWindow()`
calls in `simple_app.cpp`, and every creator ends with the same two lines:

```cpp
g_cookie_panel_overlay_hwnd = cookie_panel_hwnd;                       // process global
BrowserWindow* mainWin = WindowManager::GetInstance().GetPrimaryWindow();
if (mainWin) mainWin->cookie_panel_overlay_hwnd = g_...;               // ...and the PRIMARY's struct
```

⇒ calling them again for window B would (a) **overwrite every global with B's HWNDs**, orphaning A's
overlay HWNDs *and* their CEF browser subprocesses, and (b) write B's handles into **A's**
`BrowserWindow`. Making them per-window means changing all 14 creators to take a `BrowserWindow*` and
stop touching globals — **that is the full migration, i.e. the beta.4 ticket. It is bigger than
re-owning, not smaller.**

### 0.1.2 The re-scoped phase

**IN:**
1. **Z-order fix** — an overlay shown for window B must not send B behind A (K9).
2. **12 `ScalePx(x, g_hwnd)`** sites → the owning window (K5).
3. **Omnibox create-path positioning** (K9.2) — the one live F4 instance, and ~free once (1) is done.

**OUT:** the reposition block, the picker arm, `SaveSession`/`ShutdownApplication`, `G11` lowering,
the 14-creator per-window migration (§0.1.1), and both fenced tickets from §1.2 unless the phase
finishes early.

⚠️ **R-CLOSE is now in play and §4 must be read as amended.** Re-owning a window is **not** a pure
positioning change — an owned window is destroyed with its owner, so an overlay re-owned to B dies
when B closes while `g_*_overlay_hwnd` still points at it (K9.4). This phase can no longer claim it
"touches positioning, never lifetime", and it owes an explicit overlay-lifetime test.

## 1. The scope, established by measurement — 📏 and 📖

`TabManager::GetAllTabs()`'s call sites in `cef_browser_shell.cpp` split into **three groups**, and
they must be treated differently. This is the whole reason the phase is bounded:

| Group | Sites | Verdict |
|---|---|---|
| `HandleFullscreenChange` | L290, L331 | **Phase 3 owns it** — reported symptom 2. Not here |
| ⛔ `SaveSession`, `ShutdownApplication` | L365, L578, L640 | 🚫 **CORRECT AS GLOBAL — DO NOT CONVERT.** Saving the session and shutting down legitimately mean *all tabs in all windows*. Converting these would be a **new defect** |
| ✅ `ShellWindowProc` | L1175, L1244, L1538, L1557, L1719 | **This phase.** Per-window layout: `WM_SIZE`, DPI change, overlay positioning |

⭐ **The fix is already written, three lines from the bugs.** `ShellWindowProc` resolves the owning
window in one arm and then ignores it in its siblings:

```cpp
// cef_browser_shell.cpp:1206 — the CORRECT pattern, already present
HWND thisHeaderHwnd = bw ? bw->header_hwnd : g_header_hwnd;

// …and its siblings in the SAME function, using the global directly:
// :1157  if (g_header_hwnd && IsWindow(g_header_hwnd)) { SetWindowPos(g_header_hwnd, …
// :1277  GetWindowRect(g_header_hwnd, &headerRect);
// :1299  GetWindowRect(g_header_hwnd, &hdrRect);   :1301  GetWindowRect(g_hwnd, &mainWinRect);
// :1323/:1325, :1347/:1349, :1377  — same shape
```

⇒ `ShellWindowProc` receives `HWND hwnd` — **the window the message is for** — as its first
parameter. Like `OnFullscreenModeChange`, the correct context is **already in hand and discarded**.

### 1.1 📏 Measured site inventory — 2026-08-31, so this need not be re-derived

`ShellWindowProc` splits into arms that are **already correct** and arms that are not. The correct
ones are listed too, because the fix is to make the rest look like them.

| Arm | State |
|---|---|
| `WM_ACTIVATE` (L~1492), `WM_ACTIVATEAPP` (L~1503), `WM_CLOSE` (L~1553), `WM_DPICHANGED` (L~1740) | ✅ **Already resolve `bw` from `GetWindowLongPtr(hwnd, GWLP_USERDATA)`.** Leave alone |
| `WM_SIZE` header/tab layout (L~1244) | ✅ Already `bw ? bw->header_hwnd : g_header_hwnd` |
| `WM_SIZE` fullscreen re-expand (L~1209) | ✅ Fixed in Phase 3 (`fsWin`) |
| 🔴 `WM_SIZE` **picker arm** (L~1188) | Uses `g_header_hwnd` + `SimpleHandler::GetHeaderBrowser()` |
| 🔴 **Overlay reposition block** (L~1300–1470) | **27 global references** across **7 overlays** |

The overlay block is the dense cluster and the reason this phase exists. It touches:
`g_settings_overlay_hwnd`, `g_cookie_panel_overlay_hwnd`, `g_download_panel_overlay_hwnd`,
`g_siteinfo_panel_overlay_hwnd`, `g_wallet_overlay_hwnd`, `g_backup_overlay_hwnd`,
`g_notification_overlay_hwnd` — plus `g_header_hwnd`, `g_hwnd`, and the `g_*_icon_*_offset` globals.

⭐ **Every one of these already exists on `BrowserWindow`** (`settings_overlay_hwnd`,
`cookie_panel_overlay_hwnd`, `settings_icon_right_offset`, …). The edit is `g_x` → `bw->x` after one
`bw` resolution at the top of the block, which is exactly the owner's *"it's a pattern"* read.

⚠️ **Do NOT mechanically convert `ScalePx(…, hwnd)` inside this block — it is already CORRECT.**
The block mixes right and wrong: `mainRect` and every `ScalePx` already use the message's own `hwnd`,
while the header/overlay handles use globals. A blanket find-and-replace would damage the correct half.
That mixture is the signature of the half-finished migration and the main hazard of this phase.

**Also in scope:** the **12 `ScalePx(x, g_hwnd)`** sites (`simple_handler.cpp` overlay-show handlers,
`LayoutHelpers.h :: ScalePx`). They take DPI from the **primary** window, so a dropdown opened in a
window on a **different-DPI monitor** is scaled wrong. They are overlay *layout*, so they belong
here. 📖 Code reading — ⛔ **this defect has never been reproduced**; `P3.5-A3` is written to
reproduce it **first**.

### 1.2 ⭐ Two tickets joined this phase — fenced, because they share the RIG, not the CODE

Assigned here 2026-08-31 (`SPRINT_PLAN.md` §4.1). Be precise about **why**, because getting this
wrong is how a phase bloats:

| Ticket | Shares the code? | Shares the rig? |
|---|---|---|
| `TICKET_chrome_ui_scales_but_its_window_does_not` | 🟡 **Probably** — window sizing vs DPI is this subsystem | ✅ yes |
| `TICKET_modal_buttons_unclickable_small_screen` | ❌ **No** — React/CSS inside an overlay, not `ShellWindowProc` | ✅ yes |

⇒ They are here because `P3.5-A3` already stands up the expensive thing: **a human at two monitors at
different scale factors.** Standing that rig up twice is the waste worth avoiding — that is the whole
argument, and it is not an argument that they are the same work.

⛔ **Therefore: the conversion lands FIRST and separately.** These two are a distinct commit (or two),
after the conversion is green. ⛔ Do **not** blend a React/CSS fix into the `g_x → bw->x` diff — a
mixed commit makes the conversion impossible to revert cleanly (§8 promises exactly that).
⚠️ If the conversion consumes the phase, **these two go back to Phase 10 and the phase still closes.**
They are the droppable half; the conversion is not.

## 2. Goal

An action in one window — resize, fullscreen, DPI change, opening a dropdown — changes **nothing** in
any other window.

## 3. Done means

- [ ] Resizing window B does not move or resize window A's header or tabs.
- [ ] A DPI change on window B's monitor does not rescale window A.
- [ ] A dropdown overlay opened in window B is positioned using **B's** DPI, on a mixed-DPI setup.
- [ ] `SaveSession` and `ShutdownApplication` still act on **all** windows — unchanged (§1).
- [ ] `P3-G11`'s baseline is **lowered** by this phase, and the residual is listed by `file:line`
      with a reason (`HARNESS.md` §4).

## 4. Invariants preserved

| ID | Invariant | Why this phase could break it |
|---|---|---|
| **R-CLOSE** | Overlay close guards | ⛔ `g_file_dialog_active` and `g_wallet_overlay_prevent_close` are globals that are **correct**. A mechanical sweep would convert them and break the guards. This phase touches overlay *positioning*, never overlay *lifetime* |
| **R-GOLD** | Gold pill on the correct tab | Touching tab enumeration risks the pill's tab resolution. `Tab::id` ≠ `CefBrowser::GetIdentifier()` |
| **R-COUNT** | Per-session counters reset on tab close | Untouched; listed because tab enumeration is edited |
| **R-UPDATE** | A staged update still applies | `ShutdownApplication` is explicitly **not** converted (§1) — and it is on the shutdown path the updater depends on |
| **R-INTEXT / R-PERIM** | Trust boundary + perimeter gates | Untouched. Listed because `simple_handler.cpp` is edited for the `ScalePx` sites |

## 5.0 ⭐ EVIDENCE TABLE — AMENDED 2026-08-31. This table is authoritative; §5 below is superseded.

⛔ No empty RED or SUBJECT cells. A green result is reported with its red half or not at all.
🎯 **Standing SUBJECT for every T3 row:** two windows of the **same profile made with Ctrl+N** ⇒
**ONE process**, asserted by `winprobe.ps1` (`SUBJECT: PASS`) before the row is read. Two *profiles*
are two processes and would pass every row while proving nothing.
⛔ **`winprobe.ps1 -WatchSeconds N` is mandatory for any row involving a visible overlay.** One-shot
mode cannot see one: overlays hide on focus loss, and clicking the console to run the probe *is*
focus loss (K8.3).

| ID | 🟢 GREEN | 🔴 RED — must be *seen* to fail | 🎯 SUBJECT | Tier | Result |
|---|---|---|---|---|---|
| `P3.5-Z1` | 🚨 **The phase's headline row.** Opening a dropdown in window B leaves B **above** A in Z-order; B stays visible | ✅ **ALREADY OBSERVED PRE-FIX, 2026-08-31 11:20** (K9): B fell from Z10→Z12 at overlay create **and** again at show, on the owner's machine, one process, two Ctrl+N windows | The **`Z` column**, not visibility. ⛔ "B disappeared" is ambiguous — `Z` separates *behind* from *minimized/hidden*, which look identical on screen. B's rect must stay `80,80 1820x932`, never `-32000` | T3 | 🔴 RED seen |
| `P3.5-Z2` | ⭐ **Z1's two-sided partner.** The overlay is still **above the window it belongs to** — opened in B it is visible over B, not buried | Re-own the overlay but drop it from topmost → Z1 passes and Z2 fails. ⛔ Without this, "never raise anything" passes Z1 | The overlay's `Z` vs **B's** `Z`, same sample | T3 | ⬜ |
| `P3.5-Z3` | 🚨 **The R-CLOSE row this re-scope owes** (K9.4). With a dropdown open in B, closing **B** must not destroy or orphan the overlay that A still needs; opening the same dropdown in A afterwards works | Close B with the overlay open, then open that dropdown in A. Pre-fix this is safe (overlay owned by A); **post-fix it is the new risk** — if it goes blank/dead, the fix traded a Z-order bug for a lifetime bug | `g_*_overlay_hwnd` still valid (`IsWindow`) **and** the overlay renders in A | T3 | ⬜ |
| `P3.5-Z4` | The Z-order defect is **all 14 overlays**, not just the omnibox — the menu or shield dropdown opened in B also drops B | Open menu/shield in B pre-fix; B must fall in `Z` exactly as the omnibox did | A second overlay class in the probe output | T3 | ⬜ **UNMEASURED — do not generalise K9 in writing until run** |
| `P3.5-A3` | A dropdown opened in window B uses **B's** DPI | 🔴 **Reproduce pre-fix first** on a mixed-DPI pair: same dropdown in a 100 % and a 150 % window; pre-fix the header-right-edge→panel-right-edge gap is **identical**, post-fix it differs by ~1.5× | ⚠️ **Never reproduced, and 📏 no mixed-DPI rig exists** — all three monitors read 96 dpi (K7). Needs a Windows scale change first. If not run: **SKIPPED ⇒ INCOMPLETE**, and the 12 sites defer to beta.4 | T3 | ⬜ |
| `P3.5-A7` | Omnibox **create** path positions against the requesting window | ✅ **ALREADY OBSERVED PRE-FIX** (K9.2): created at `160,109` = A-relative while typing in B; `Show` then corrected it to `240,189` = B-relative | The rect at `Vis=False` (create), **not** the rect once shown — `Show` masks it | T3 | 🔴 RED seen |
| `P3.5-A4` | 🚫 **Do-not-convert control.** `SaveSession`/`ShutdownApplication` still enumerate **all** tabs in **all** windows | Two windows with distinct tabs → quit → reopen; every tab from both returns. Convert one to per-window → tabs lost, row goes RED | The restored tab set across both windows | T2 + T3 | ⬜ |
| ~~`P3.5-A1`~~ | ⛔ **RETIRED — vacuous** (K3). The reposition block is unreachable for secondary windows, so its RED cannot be observed | — | — | — | ⛔ void |
| ~~`P3.5-A2`~~ | ⛔ **RETIRED** with A1 — it was A1's partner | — | — | — | ⛔ void |
| ~~`P3.5-G11↓`~~ | ⛔ **RETIRED — not satisfiable** (K4). `G11` scans neither `cef_browser_shell.cpp` nor these patterns. Baseline stays 60 with a written reason in `HARNESS.md` §4 | — | — | — | ⛔ void |
| `P3.5-A5` / `A6` | 🎫 The two fenced tickets (§1.2) | unchanged | unchanged | T3 | ⬜ **droppable — the re-scope makes this likely** |

⭐ **Two rows are already RED before any code exists** (`Z1`, `A7`). That is the correct order and it
is the first time this sprint a fix has started from an observed failure rather than a code reading.

## 5. Evidence table  ⛔ SUPERSEDED by §5.0 — kept for provenance

⛔ No empty RED or SUBJECT cells. A green result is reported with its red half or not at all.

| ID | 🟢 GREEN | 🔴 RED — must be *seen* to fail | 🎯 SUBJECT | Tier | Result |
|---|---|---|---|---|---|
| `P3.5-A1` | Resizing window B leaves window A's client rect, header position and tab rects **byte-identical** | Restore the `g_header_hwnd`/`g_hwnd` lookups on the **same binary** → A moves with B. ⛔ Must be *observed*, not asserted from the diff | **Two windows, same profile ⇒ ONE process.** Verify via `Win32_Process` that exactly one non-`--type=` browser process exists. Two *different* profiles would pass while proving nothing | T3 | ⬜ |
| `P3.5-A2` | ⭐ **A1's two-sided partner.** Window B's own resize actually *works* — B's header and tabs track B | Skip B's layout entirely → A1 passes and A2 fails. ⛔ Without this row, "never lay anything out" is a passing fix | Window B's own rects, measured | T3 | ⬜ |
| `P3.5-A3` | A dropdown overlay opened in window B uses **B's** DPI | 🔴 **Reproduce the defect FIRST**, pre-fix, on a mixed-DPI pair (matrix cell #9): open the same dropdown in a 100 % window and a 150 % window; pre-fix the offsets must be **identical** (both scaled by the primary) and post-fix they must **differ** | ⚠️ **Never reproduced.** If the pre-fix arm shows them already differing, the `ScalePx` claim is **wrong** and these 12 sites leave this phase | T3 | ⬜ |
| `P3.5-A4` | 🚫 **The do-not-convert control.** `SaveSession` and `ShutdownApplication` still enumerate **all** tabs across **all** windows | Open two windows with distinct tabs, quit, reopen → every tab from **both** returns. Convert one of them to per-window → tabs from the second window are lost, and this row goes RED | The restored session's tab set, across both windows. ⭐ This row exists because the *correct* fix for four sites is the *wrong* fix for three | T2 + T3 | ⬜ |
| `P3.5-A5` | 🎫 `chrome_ui_scales_but_its_window_does_not` — window and content agree on scale | ❔ **First settle whether Phase 1's DPI work superseded it** — the ticket predates it and was never re-checked. If it did, the row closes as already-fixed with that evidence, which is a legitimate outcome | The window vs its rendered content on a non-primary-DPI monitor | T3 | ⬜ |
| `P3.5-A6` | 🎫 `modal_buttons_unclickable_small_screen` — modal action buttons are reachable at the smallest supported viewport | 🔴 Reproduce first at a small viewport; the buttons must be **observed** unreachable before any fix. ⛔ Phase 1's `a3d8202` touched only `WalletDashboard.css` — a *different* surface — so do not assume it is related | The **modal**, not the wallet dashboard. React/CSS, not `ShellWindowProc` | T3 | ⬜ |
| `P3.5-G11↓` | `P3-G11`'s baseline is lowered by exactly the sites this phase converted; residuals listed with reasons | Re-run `preflight.ps1 -NegativeControl -Only G11` → still fires on an injected violation at the new baseline | ⚠️ Baseline re-measured **by the tool**, never from M9.4's hand counts (`HARNESS.md` §9) | T0 | ⬜ |

**Two-sided pairings.** `A1`/`A2` — *A is untouched* vs *B actually worked*; a fix that lays out
nothing passes A1 alone. `A3`'s own two arms (100 % vs 150 %) are each other's control. `A4` is the
control on the **scope** rather than on a behaviour: it fails if we over-convert.

## 6. Blast radius

| Risk | Why it is real | Mitigation |
|---|---|---|
| ⛔ **Over-conversion** | The single largest risk. Three `GetAllTabs()` sites are **correct** and look identical to the five that are not | §1 names all eight by line; `P3.5-A4` is the control |
| **Correct globals swept up** | `g_file_dialog_active`, `g_wallet_overlay_prevent_close` are process-wide by design | Out of scope by name (§7); R-CLOSE listed |
| **`ShellWindowProc` is the main window procedure** | Every window message routes through it | Change only the arms named in §1; leave message routing alone |
| **Testing is the real cost** | Multi-window defects are invisible to unit tests — every row here is T3, a human with two windows | Accepted and stated. ⚠️ `HARNESS.md`: *"a phase that changes UI and declares no T3 row is a smell"* — here it is almost **all** T3 |
| **Mixed-DPI hardware needed** | `A3` needs two monitors at different scale factors | If unavailable, `A3` is **SKIPPED** and the run is **INCOMPLETE**, never PASS (`HARNESS.md` §8). The 12 sites then defer to the beta.4 ticket |
| **macOS** | Different window model, unassessed | Windows-only; no macOS claims. Nothing added to the Mac relay |

## 7. Out of scope

- ⛔ `SaveSession`, `ShutdownApplication` — **correct as they are** (§1).
- ⛔ `g_file_dialog_active`, `g_wallet_overlay_prevent_close` and other genuinely process-wide flags.
- The **scattered judgment-call sites** (`PostMessage(g_hwnd, WM_CLOSE)` and friends) — beta.4 ticket.
- The 18 backwards-compat static accessors — beta.4 ticket.
- `HandleFullscreenChange` — Phase 3.
- macOS.

## 8. Rollback

One commit reverts. Every change is "resolve the owning window instead of reading a global" inside a
bounded, named set of call sites; no new files, no data migration, no installer or registry change.
`git revert` restores today's behaviour.

---

## Sign-off

- [ ] Every evidence row GREEN **and** its RED observed
- [ ] `scripts/preflight.ps1` run — result + date recorded below
- [ ] `scripts/preflight.ps1 -NegativeControl` run — `G11` seen to fail at its **new** baseline
- [ ] `../REGRESSION_SET.md` run in full at the 3.5 → 4 boundary
- [ ] Adversarial review complete, four questions answered in writing
- [ ] `G11` baseline lowered in `../HARNESS.md` §9, residuals listed with reasons
- [ ] Commit messages cite the row IDs they satisfy

| Item | Result | Date | By |
|---|---|---|---|
| preflight | | | |
| preflight -NegativeControl | | | |
| regression set | | | |
| adversarial review | | | |
