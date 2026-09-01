# Phase 3.5 — measurements

Every number in `PHASE_CONTRACT.md` is cited from here. Each entry is labelled
📏 **MEASURED** (a run that produced this output) or 📖 **CLAIM** (a code reading).
⛔ Per `HARNESS.md` §6 Q4, mislabelling one as the other is the error this column exists to prevent.

**Taken:** 2026-08-31, Windows 11 Pro 26200, at `3fae58a` (`HEAD == origin/0.4.0`, tree clean).
⛔ **The owner's installed browser was never touched.** It was running throughout (66 processes under
`%LOCALAPPDATA%\HodosBrowser`); every process query filtered by **exe path**, never by name. The only
process started or stopped was the dev build under `cef-native\build\bin\Release`.

⭐ **The dev build was already current with `HEAD`** — no file under `cef-native/src`, `cef-native/include`
or `cef-native/*.cpp` was newer than `build/bin/Release/HodosBrowser.exe`. So every reproduction below
runs **pre-fix, on unmodified code**, with no rebuild.

---

## K1 — 📏 MEASURED: the §1.1 inventory is current; two of its verdicts are wrong

Line numbers in the contract's §1.1 survived Phase 3's edits to `ShellWindowProc`. The **verdicts**
did not.

| §1.1 claim | Line check | Verdict check |
|---|---|---|
| `WM_ACTIVATE` ~1492 / `WM_ACTIVATEAPP` ~1503 / `WM_CLOSE` ~1553 / `WM_DPICHANGED` ~1740 resolve `bw` | ✅ exact | ✅ correct |
| `WM_SIZE` header/tab ~1244 already `bw ? bw->header_hwnd : g_header_hwnd` | ✅ exact (1244–1245) | ✅ correct |
| `WM_SIZE` fullscreen re-expand ~1209 (`fsWin`, Phase 3) | ✅ exact | ✅ correct |
| 🔴 `WM_SIZE` picker arm ~1188 | ✅ (1187) | ❌ **wrong — see K2** |
| 🔴 overlay reposition block ~1300–1470, *"27 refs / 7 overlays"* | 🟡 block is **1308–1483** | ❌ **wrong — see K3.** And the count is **47 refs / 9 overlays**, not 27/7 |
| 12 × `ScalePx(x, g_hwnd)` in `simple_handler.cpp` | ✅ **exactly 12** | ✅ **correct — see K5** |

📏 The 47/9 count, by tool (`awk` over 1308–1483, `grep -o 'g_[a-z0-9_]*'`):
`g_siteinfo_panel_overlay_hwnd` 5, `g_header_hwnd` 5, `g_wallet_overlay_hwnd` 4,
`g_settings_overlay_hwnd` 4, `g_notification_overlay_hwnd` 4, `g_download_panel_overlay_hwnd` 4,
`g_cookie_panel_overlay_hwnd` 4, `g_brc100_auth_overlay_hwnd` 4, `g_backup_overlay_hwnd` 4,
`g_omnibox_overlay_hwnd` 3, `g_hwnd` 3, and one each of four `g_*_icon_*_offset`.

⚠️ **Fourth hand count wrong this sprint** (after `G2` 6→5, `G5` 11→15, `G11` ~19→60). The rule in
`HARNESS.md` §9 — *baseline with the tool that will do the measuring* — applies to inventories too,
not only to gate baselines.

## K2 — 📖 CLAIM: the picker arm is **not** a defect

`cef_browser_shell.cpp:5233` — a picker-mode process takes
`SingleInstance::TryAcquireInstance(".picker")`, and a second picker launch **exits**:

```cpp
if (g_picker_mode) {
    if (!SingleInstance::TryAcquireInstance(".picker")) {
        LOG_INFO("Picker: another instance is already showing the picker — exiting");
        return 0;
    }
```

It never calls `SendToRunningInstance`, so no `WM_SINGLE_INSTANCE_NEW_WINDOW` can arrive, and picker
mode is excluded from `WindowManager::CreateFullWindow` (Ctrl+N is a `tab_` role shortcut; picker has
no tabs). ⇒ **one picker window per process.** By the contract's own test — *one value per process,
or one per window?* — `g_header_hwnd` in that arm is **correct as a global**.

⇒ Converting it is churn with nonzero risk and no user-visible effect. **Removed from scope.**

## K3 — 🎯 📖 CLAIM: the overlay reposition block is unreachable for secondary windows

Three lines above the block, `cef_browser_shell.cpp:1302`:

```cpp
// Overlays only exist on the primary window (window 0).
// Skip overlay repositioning for secondary windows.
if (bw && bw->window_id != WindowManager::GetInstance().GetPrimaryWindowId()) {
    return 0;
}
```

⇒ whenever the block executes, `bw` **is** the primary window, so `g_header_hwnd == bw->header_hwnd`
and `g_hwnd == hwnd` **by construction**.

🚨 **Therefore `g_x → bw->x` in that block is a no-op refactor, and `P3.5-A1` is vacuous against it.**
A1's RED half — *"restore the `g_header_hwnd`/`g_hwnd` lookups on the same binary → A moves with B"* —
**cannot be observed**, because the restored code and the converted code compute the same values. A
test that cannot go red has not been shown to test anything (`HARNESS.md` preamble).

⚠️ This is the vacuous-test trap **in the contract itself**, not in an implementation. It is the same
family as the four farbling harnesses: it looks rigorous, and it would have produced a confident green.

**The block does conceal a real defect, which this phase is NOT taking:** because it early-returns,
**resizing a secondary window never repositions that window's overlays at all**. Fixing that means
*deleting the guard* and positioning against `hwnd` — a behaviour change, not the mechanical
conversion this phase was scoped as. ⇒ beta.4 ticket.

⚠️ One narrow live path: `SetWindowLongPtr(hwnd, GWLP_USERDATA, …)` runs **after** `CreateWindow`
(`:5707`), so a window's creation-time `WM_SIZE` arrives with `bw == nullptr`, the guard falls
through, and the block mixes `hwnd`-based `mainRect`/`ScalePx` with the **primary's**
`g_header_hwnd`. That is the only path on which the contract's "mixture" warning is live.

## K4 — 🚨 📏 MEASURED: `G11` cannot be lowered by this phase

📏 `pwsh scripts/preflight.ps1 -Only G11` → **`PASS  60 violations, at baseline`**.
📏 Distribution, by the gate's own pattern and paths: `simple_handler.cpp` 36, `simple_app.cpp` 23,
`WindowManager.cpp` 1.

Two independent reasons the phase's work is invisible to it:

1. **Paths.** `G11.Paths = @('cef-native/src/handlers', 'cef-native/src/core')`. The file this phase
   is mostly about, `cef-native/cef_browser_shell.cpp`, is in **neither** — it sits at `cef-native/`.
2. **Pattern.** `G11.Pattern = '(GetPrimaryWindow *\(\)|TabManager::GetInstance\(\)\.GetActiveTab *\(\))'`.
   It matches **no** `g_hwnd`, `g_header_hwnd` or `g_*_overlay_hwnd`. So the 12 `ScalePx` sites are
   invisible **even though their file is scanned**.

⇒ **`P3.5-G11↓` is not satisfiable as written**, and neither is deliverable 3 of the session prompt.
The baseline stays at **60**, with this recorded as the reason (`HARNESS.md` §4: *raising* needs a
written reason; *not lowering* needs one too, or the next reader assumes the phase failed).

⛔ Widening `G11`'s paths/pattern so it *does* cover this code would be **editing the instrument in
the change it measures** — CLAUDE.md working rule #6. If wanted, it is a separate commit, after the
fix, with its own `-NegativeControl` run.

## K5 — ✅ 📖 CLAIM: the 12 `ScalePx` sites are a real defect, and the correct API is already in hand

📏 Exactly **12** `ScalePx(…, g_hwnd)` in `simple_handler.cpp`: L2936, 3099, 3168, 3384, 5906, 7446,
7504, 7611, 7710, 7810, 7813, 7814.

The surrounding machinery is **already window-scoped**, which is what makes these 12 the whole
remaining defect:

- 📏 **Every** `Show*Overlay` takes `BrowserWindow* targetWin` (10 of them: wallet, omnibox, cookie,
  download, siteinfo, tablist, bookmarks, menu, profile) and positions against it —
  `posHwnd = targetWin->hwnd`, `ScalePx(450, posHwnd)`.
- 📏 The `*_show` handlers already pass `GetOwnerWindow()` (e.g. `:2973`).

⇒ the **only** value still taken from the primary window is the icon offset, pre-scaled at these 12
sites *before* being handed to a target-window positioner. Fix is one line each:
`g_hwnd` → the owning window's `hwnd`. Same shape as Phase 3, correct API already used on adjacent lines.

## K6 — ⛔ 📏 MEASURED: F4 as originally proposed is **REFUTED**

The kickoff proposed that *"the first open of any dropdown from a secondary window lands on the
primary"*, on the reading that `Create*Overlay` takes no `targetWin` (true) and that the `*_show`
handlers call it on first open (`if (!alreadyExists) Create… else Show…`, also true).

📏 **Refuted by enumerating the dev process's top-level windows after a launch with no user
interaction at all** — nine overlay HWNDs already existed, hidden:

```
CEFNotificationOverlayWindow  CEFTabListPanelOverlayWindow  CEFBookmarksPanelOverlayWindow
CEFSiteInfoPanelOverlayWindow CEFProfilePanelOverlayWindow  CEFCookiePanelOverlayWindow
CEFDownloadPanelOverlayWindow CEFWalletOverlayWindow        CEFMenuOverlayWindow
```

📖 Cause, found from the measurement: `cef_browser_shell.cpp:5906-5919` pre-warms eight dropdowns
hidden on delayed tasks at 1000–4500 ms ("*avoids the subprocess-creation hitch*", "*Win10
blank-first-open fix*"). ⇒ `alreadyExists` is **true before a user can click**, the `Create` arm is
effectively dead for them, and the already-correct `Show…(offset, GetOwnerWindow())` arm always runs.

⭐ **Caught before the owner was sent to test it.** This is the sprint's recurring shape — a code
reading that reads as a mechanism — and the cost of checking was one process launch.

### What survives, and what does not

| Candidate | State |
|---|---|
| 8 pre-warmed dropdowns (menu, wallet, download, cookie, profile, siteinfo, bookmarks, tablist) | ⛔ **Refuted** — `Create` arm unreachable except in the <4.5 s startup race |
| **Settings overlay** — `CreateSettingsOverlayWithSeparateProcess` takes no `targetWin`, positions wholly against `g_hwnd`/`g_header_hwnd`, and destroy-recreates on every open | ⛔ **Unreachable from the UI.** 📏 Its only trigger is `window.hodosBrowser.overlay.show()` (`initWindowBridge.ts:51`), and 📏 **nothing in `frontend/src` calls it** — settings is the full page (`SettingsPage.tsx`). ⇒ do not fix dead code |
| **Omnibox** — 📏 **not** in the pre-warm list, and 📏 absent from the enumeration above | ✅ **LIVE.** First omnibox use in a session takes `CreateOmniboxOverlay(g_hInstance, true)` (`:2886`), which is window-blind; every later use takes `ShowOmniboxOverlay(GetOwnerWindow())` (`:2888`), which is correct |

📏 The create-or-show asymmetry itself is uniform across **15** call sites in `simple_handler.cpp`
(`Create*Overlay(g_hInstance, true…)`: L2866, 2886, 2971, 3111, 3178, 3260, 3386, 4877, 5936, 7456,
7521, 7622, 7723, 9260, 9297). Only the omnibox pair is reachable in a normal session.

⭐ **The omnibox gives a self-contained pre-fix demonstration needing no code change**: open the
omnibox in window B for the *first* time in a session (wrong window), close it, open it again
(right window). Same action, two results, one session — the defect is its own control.

## K7 — 📏 MEASURED: the rig available on this machine

📏 Three monitors, **all at 96 dpi (100 %)**:

```
\\.\DISPLAY22  1920x1080 at 0,0       dpi=96 (100%)  PRIMARY
\\.\DISPLAY21  1920x1080 at 1920,0    dpi=96 (100%)
\\.\DISPLAY24  1536x960  at -1920,0   dpi=96 (100%)
```

⇒ **no mixed-DPI pair exists today.** `P3.5-A3` needs one monitor set to 125 % or 150 % in Windows
Settings first. That is a settings change, not missing hardware — but it is a real precondition, and
without it A3 is **SKIPPED** and the run **INCOMPLETE**, never PASS (`HARNESS.md` §8).

## K8 — 🔧 Two defects found in the phase's own instrument (`winprobe.ps1`)

Recorded because both produced **clean-looking, wrong** output — the class this project keeps shipping.

1. **`GetWindowTextW`/`GetClassNameW` without `CharSet=CharSet.Unicode`.** The marshaller treated the
   W entry points as ANSI, so `"HodosBrowserWndClass"` read back as **`"H"`**. Plausible output, wrong
   data. Same trap as `FindWindowW`.
2. ⚠️ **Filtering candidate windows by size** (`width < 200 → skip`) **hid the subject itself** when
   the shell window was minimized, printing an empty and entirely clean-looking table. Now filtered by
   **class name**. ⭐ An instrument that silently drops its own subject is exactly the `SUBJECT`-column
   failure the harness exists to catch — found here in the harness, not the product.

⚠️ A rect of `-32000,-32000` is **minimized**, not mispositioned.

3. 🚨 **The one-shot mode could not see its own subject at all.** Every dropdown hides on focus
   loss (`cef_browser_shell.cpp :: HideAllOverlays`, via `WM_ACTIVATEAPP`), and **clicking a
   PowerShell window to run the probe *is* a focus loss** — so a one-shot run reports "no overlay"
   regardless of what was on screen a moment earlier. 👤 The owner ran it twice, correctly, and got
   two empty results; the instrument, not the tester, was wrong. ⭐ Identical lesson to Phase 2's CDP
   timing instrument: **sample continuously; never ask the observer to interact with the thing under
   test.** Fixed by `-WatchSeconds`, which also records **Z-order** — the column that turned out to
   decide K9.

---

## K9 — 🎯 📏 MEASURED, 👤 OWNER-RUN 2026-08-31 11:20: the disappearing window is **Z-ORDER**, and F4 is **confirmed** on the create path

👤 **Owner's report that prompted this** (2026-08-31): *"when I typed a letter into instance B's
address box, the omnibox appeared where window B address bar was (correct spot) but the entire
instance of window B disappeared from the screen. Just the omnibox was floating over window A."*

📏 `winprobe.ps1 -WatchSeconds 30`, one dev process (pid 49284, SUBJECT PASS), two windows made with
**Ctrl+N** so both are guaranteed in one process:

- **A** = `0x190374` @ `0,0 1920x1032`
- **B** = `0x690B84` @ `80,80 1820x932` (entirely inside A)

| # | t | Omnibox HWND | Z-order | Reading |
|---|---|---|---|---|
| 1 | 11:20:01.9 | *absent* | **B**(10) above **A**(11) | baseline — B in front, correct |
| 2 | 11:20:05.2 | **created**, `Vis=False`, `160,109 1606x350` | **A**(11) above **B**(12) | B drops behind A **at creation** |
| 3 | 11:20:07.5 | `Vis=False`, `160,109` | **B**(11) above **A**(12) | B recovers |
| 4 | 11:20:10.2 | **`Vis=True`**, `240,189 1506x350` | **A**(11) above **B**(12) | B drops behind A **again, on show** |

### K9.1 — ⛔ Not minimized. **Behind.** My `-32000` was a red herring I introduced

📏 B's rect reads `80,80 1820x932` in **every** sample and never `-32000`. The earlier `-32000`
reading (K8) was a genuinely minimized leftover window from a previous session, and I offered it to
the owner as possibly-the-bug. It was not. ⭐ The `Z` column — added precisely because "hidden" and
"behind" look identical on screen — is what separated them. **Without it I would have gone looking
for a phantom minimize call.**

⇒ The visual report is fully explained: A is fullscreen `1920x1032` at `0,0`, B is `80,80 1820x932`
and therefore **entirely inside A**. B behind A = invisible. The omnibox is `WS_EX_TOPMOST`, so it
alone keeps floating. *"Window B disappeared, just the omnibox over window A"* is z-order occlusion,
exactly.

### K9.2 — ✅ F4 **CONFIRMED** on the create path, and it is the *harmless* half

📏 At creation (#2) the omnibox sits at `160,109` — that is `A.left+160, A.top+109`, i.e. positioned
against **window A**, while the user was typing in **window B**.
📏 On show (#4) it moves to `240,189` — that is `B.left+160` (80+160), `B.top+109` (80+109),
positioned against **window B**.

⇒ Precisely as K5/K6 read the code: `CreateOmniboxOverlay` uses globals (`simple_app.cpp:1375-1416`),
`ShowOmniboxOverlay(GetOwnerWindow())` uses the target. **This is the live F4 instance**, and the
only one — K6's refutation of the other eight stands.

⚠️ **But it is nearly invisible to a user**: the overlay is `Vis=False` at creation and is
repositioned by `Show` before it is ever displayed. That is why the owner saw it in the *"correct
spot"*. ⇒ **The positioning half of F4 is real but low-impact.**

### K9.3 — 🎯 The damaging half is the Z-order drop, and it fires on **both** create and show

📏 B drops behind A at **#2 (create, still hidden)** *and* at **#4 (show)**. ⇒ this is **not** a
first-open-only bug, and **fixing F4's positioning would not fix it.** Two distinct defects.

🧠 **CANDIDATE CAUSE — not yet established.** 📏 All **14** overlays are created
`CreateWindowEx(..., g_hwnd, nullptr, hInstance, nullptr)` (`simple_app.cpp`: L625, 749, 997, 1071,
1230, 1321, 1416, 1646, 1907, 2169, 2403, 2652, 2902, 3136) — **owned by window A, zero exceptions**,
and ownership is fixed at creation. Win32 keeps an owner and its owned windows together as a z-order
group, which is a plausible route to A being pulled above B.

⛔ **Not proven.** 📏 `ShowOmniboxOverlay` contains no `SetFocus`, `SetForegroundWindow` or minimize
call, so nothing in the show path *explicitly* raises A. The decisive experiment is the fix itself:
re-own the overlay to the requesting window (`SetWindowLongPtr(hwnd, GWLP_HWNDPARENT, targetWin->hwnd)`)
and see whether the drop stops — with reverting it as the negative control.

⚠️ **This generalises beyond the omnibox.** All 14 overlays share the hardcoded owner, so opening
*any* dropdown from a secondary window is predicted to send that window behind the primary. 🔴 **Not
yet tested on a second overlay** — that is the cheapest next measurement and it must be run before
the claim is generalised in writing.

### K9.4 — ⚠️ Blast radius of the candidate fix: this crosses into overlay **lifetime**

Re-owning an overlay is **not** a pure positioning change. An owned window is destroyed when its
owner is destroyed, so an overlay re-owned to window B would be destroyed when **B** closes — while
the process-global `g_*_overlay_hwnd` still points at it. `IsWindow()` guards exist on the show/hide
paths, but this lands squarely on **R-CLOSE**, which the contract (§4) reserves as *"this phase
touches overlay positioning, never overlay lifetime."*

⇒ 🔴 **Owner decision required** before any code: this defect is more user-visible than the 12
`ScalePx` sites, but taking it widens the phase past its own stated fence.

---

# Execute round — 2026-08-31 afternoon, at `a6e9db9` + the P3.5 fix

**Method change worth recording.** Every row below was driven over **CDP** (`p35drive.py`) instead of
by hand. That is not a shortcut — it is what made the rows *observable*. K8.3 established that any
console the observer touches steals focus and fires `HideAllOverlays`, so the action and the probe
cannot both come from the desktop input queue. Driving the action out-of-band lets `winprobe.ps1`
sample continuously through it. ⭐ `p35drive.py send` posts the **same** `cefMessage` IPC from the
**same** browser a React `onClick` does, so `GetOwnerWindow()` resolves identically; most rows in fact
dispatch a real DOM `.click()` on the real button, which is the production path exactly.
⚠️ It does **not** reproduce the window activation a real click also causes — every row therefore
reads the `Z` column from a sample taken **before** the action as its own baseline.

⛔ **SUBJECT for every row:** one dev browser process, two windows made with **Ctrl+N**
(`winprobe.ps1` → `SUBJECT: PASS`). The owner's installed browser (68 processes) ran untouched
throughout; every process query matched by **exe path**.

---

## K10 — 🎯 📏 MEASURED: `P3.5-Z4` — the Z-order defect is **NOT** omnibox-specific

Ran pre-fix on the unmodified build (pid 49284). Menu dropdown opened from **window B's** header:

| # | t | Menu overlay | Z-order | Reading |
|---|---|---|---|---|
| 1 | 12:12:13 | absent (`Vis=False`) | **B**(11) above **A**(12) | baseline, B in front |
| 2 | 12:12:26 | `Vis=True` at `1609,219` | **A**(11) above **B**(12) | B drops behind A |

📏 `1609,219` is **B-relative** (B at `110,110`; `110+109 = 219`), which independently confirms the
header being driven was B's. ⇒ `P3.5-Z4` **RED, observed.**

⭐ **New fact K9 could not see:** the menu overlay is **pre-warmed at startup**, so no `Create` ran —
only `Show`. The drop therefore fires on the **show path alone**, and is not create-related at all.

## K11 — 🚨 📏 MEASURED: the cause is **ESTABLISHED**, not a candidate — and no product code was needed

K9.3 recorded the ownership theory as a candidate and said the decisive experiment was the fix
itself. ⛔ **That was wrong, and expensively so.** 📏 `ShowMenuOverlay` contains no `SetFocus`,
no `SetForegroundWindow` — the entire show path reduces to one Win32 call:

```cpp
SetWindowPos(overlay, HWND_TOPMOST, x,y,w,h, SWP_NOACTIVATE | SWP_SHOWWINDOW);
```

So the hypothesis is testable **from outside the process, against the live overlay HWNDs, before
writing anything** (`zexp.ps1`). Three arms, overlay geometry identical in all three, only the
**owner** differing:

| Arm | `GWLP_HWNDPARENT` | Before show | After show | |
|---|---|---|---|---|
| 1 — as shipped | A | B(11) A(12) | **A(11) B(12)** | the defect |
| 2 — candidate fix | B | B(11) A(12) | **B(11) A(12)** | drop gone |
| 3 — negative control | A | B(11) A(12) | **A(11) B(12)** | drop returns |

⇒ Showing a window **owned by A** raises A's owner-group above B. Arm 3 is the negative control and
it is what makes arms 1–2 mean anything.

⭐ **The lesson, because this sprint keeps paying for it:** "the fix is the only experiment" was a
*belief*, not a measurement. The real test cost one script and no build. Ask what the smallest
external reproduction is **before** accepting that a code change is the only way to learn something.

## K12 — 🚨 📏 MEASURED: the R-CLOSE hazard of the re-own fix is **REAL** (`zexp2.ps1` ARM 5)

Overlay re-owned to B, then B destroyed:

```
IsWindow(overlay) before closing B : True
IsWindow(B)       after  closing B : False
IsWindow(overlay) after  closing B : False   <-- destroyed with its owner
```

⇒ the re-own fix would leave `g_menu_overlay_hwnd` **dangling**. It is recoverable — the `IsWindow()`
guard re-creates the overlay on the next open (new HWND observed) — but re-creation goes through the
window-blind `Create` path, so it lands back on the primary. **Measured before any code was written,
which is why the phase never had to find out the expensive way.**

## K13 — 📏 MEASURED: a **fourth** arm the contract never considered, and the one that shipped

Keep ownership on the primary; assert the requesting window's z-order **after** the show:

```
before show           B(11) A(12)
after  show           A(11) B(12)     <- the drop still happens
after  B->HWND_TOP    B(11) A(12)     <- corrected
```

⇒ closes `Z1` with **zero** lifetime exposure. 👤 Owner chose this arm over re-owning
(2026-08-31), so `PHASE_CONTRACT.md` §4's *"positioning, never lifetime"* fence holds after all.

## K14 — 🚨 📏 MEASURED: **K7 was WRONG — the mixed-DPI rig already exists**

K7 recorded *"all three monitors read 96 dpi ⇒ no mixed-DPI rig"*, which would have made `P3.5-A3`
**SKIPPED ⇒ INCOMPLETE** and deferred the 12 `ScalePx` sites to beta.4. It was an instrument bug.

📏 Re-measured from a process that calls `SetProcessDpiAwarenessContext(PER_MONITOR_AWARE_V2)`
**first** (`dpiprobe.ps1`, which asserts its own awareness before printing anything):

```
\\.\DISPLAY22  1920x1080 @ 0,0        96 dpi  100%   PRIMARY
\\.\DISPLAY21  1920x1080 @ 1920,0     96 dpi  100%
\\.\DISPLAY24  1920x1200 @ -1920,0   120 dpi  125%   laptop panel
```

⛔ **Windows lies to a DPI-unaware process**: it reports 96 for every monitor *and* divides the rects
by the scale factor. That is exactly why K7 read the laptop as `1536x960 @ 96` — `1920 ÷ 1.25 = 1536`.
⭐ **The tell was in K7's own output and nobody read it**: two monitors at a round `1920x1080` and one
at an odd `1536x960` is a scale factor, not a panel. 👤 Caught only because the owner asked what DPI
meant and whether their screens should match.

⚠️ **Fourth instrument bug in this phase's own tooling** (after the two in K8 and the `-WatchSeconds`
gap), and the third whose output looked clean. `dpiprobe.ps1` now refuses to print if
`GetAwarenessFromDpiAwarenessContext` is not `PER_MONITOR`.

## K15 — 📏 MEASURED: `P3.5-A3` RED then GREEN, on the rig K14 uncovered

Window B moved to `\\.\DISPLAY24` (125 %), A left on the primary (100 %). 📏 **Both** headers report
the same `iconRightOffset = 36` CSS px (B's `devicePixelRatio` is `1.25`, so its own scaling is
already correct — the defect is entirely in the C++ conversion). Measured as
`headerRight − overlayRight`, which by construction *is* the scaled icon offset:

| | A @ 100 % | B @ 125 % | |
|---|---|---|---|
| 🔴 **pre-fix** | **36** | **36** | identical ⇒ the defect. `ScalePx(36, g_hwnd)` = ×1.0 |
| 🟢 **post-fix** | **36** | **45** | `36 × 1.25 = 45`. A unchanged ⇒ the control arm holds |

📏 Panel size on B was `350x563` **pre-fix** — `ScalePx(280/450, posHwnd)` — confirming K5's reading
that every `Show*Overlay` was already window-scaled and the pre-scaled icon offset was the sole
residual.

## K16 — 📏 MEASURED: post-fix results for `Z1`, `Z2`, `Z4`, `A7`, `Z3`

New build, pid 58960, A at `0,0 1920x1032`, B at `80,80 1820x932`:

| Row | Observation |
|---|---|
| `Z1`/`Z4` | Menu shown at `1579,189` (B-relative) → **B stays Z10, A Z11.** Shield panel at `1137,189` → **B stays Z10.** Two overlay classes, no drop |
| `Z2` | Overlay at **Z1** in both samples, above B at Z10 — still topmost, not merely "nothing raised" |
| `A7` | First omnibox of the session (true `Create` path — no `CEFOmniboxOverlayWindow` existed) created at **`240,189`** = B-relative (`80+160`, `80+109`). 🔴 Pre-fix K9.2 measured `160,109` = A-relative |
| `Z3` | Menu **`Vis=True` at `1609,219` (B-relative)** at the instant B was destroyed; owner still A; `IsWindow(overlay) = True` after. Reopening in A moved the **same HWND `0xC20AC0`** to A-relative `1599,109`, and its document was live (`New Tab │ Cmd+T │ History │ …`, 4 buttons) |

⭐ `Z3` needed a probe that waits *before* sampling (`z3probe.ps1 -WaitSeconds`), because the console
must already exist when the dropdown is opened — otherwise K8.3's focus loss hides it first.

## K17 — 🎫 📏 MEASURED: `P3.5-A4` is **RED for a pre-existing reason**, and it is a real defect

Two windows, distinct external tabs (`example.com`, `iana.org`), quit both:

```
📋 Session saved: 1 tabs across 1 windows
```

📏 `session.json` contained **only** `example.com` — the second window's tab was lost. 📏 Relaunch
restored that one tab correctly, so the restore machinery itself works.

📖 Cause, read from **unmodified** code (this phase's diff touches none of it):
`ShellWindowProc`'s `WM_CLOSE` arm calls `ShutdownApplication()` — and hence `SaveSession()` — only
when `windowCount <= 1`. The secondary and primary-transfer arms **close that window's tabs first**.
⇒ by the time `SaveSession` runs, every window but the last has already lost its tabs, so the v2
`windows[]` array it builds can never hold more than one.

⇒ **The do-not-convert half of `A4` is GREEN** — `SaveSession`/`ShutdownApplication` still enumerate
all windows and were not touched. **Its stated acceptance criterion is RED**, for a defect that
predates this phase. 🎫 Filed for beta.4; ⛔ not fixed here (it is a shutdown-ordering change, not a
window-scoping one).

⚠️ **The row could not run at all at first**: `browser.restoreSessionOnStart` was **off** in the dev
profile and `SaveSession` returned early with *"Session restore disabled"*. A quieter harness would
have read the missing `session.json` as a failure of the feature. ⭐ Check the setting the feature is
gated on before reading its output.

## K18 — 🎫 📏 MEASURED, incidental: two more pre-existing multi-window defects

Both observed while running the rows above. ⛔ Neither is fixed here — the first is named in
`PHASE_CONTRACT.md` §7 as out of scope, the second is new.

1. **Menu → Exit closes the *primary*, not the window you clicked in.** 📏 `exit` from window B's
   header closed window A and left B running. 📖 `simple_handler.cpp` — both the `exit` IPC and
   `menu_action`'s `"exit"` arm do `PostMessage(g_hwnd, WM_CLOSE, 0, 0)`. This is exactly the
   *"scattered `PostMessage(g_hwnd, WM_CLOSE)` and friends"* the contract fences to beta.4 — now with
   an observation attached rather than a code reading.
2. **Closing the primary destroys every overlay.** 📏 After A closed, **all 14** overlay HWNDs were
   gone from the surviving window's process enumeration, while `TransferPrimaryWindow` had already
   copied those now-dead handles into the new primary's `BrowserWindow`. 📖 Cause: overlays are owned
   by `g_hwnd` at creation and Windows destroys owned windows with their owner; the transfer moves
   *handles*, never *ownership*.
   ✅ **It self-heals**: 📏 the next `menu_show` in the surviving window hit the `IsWindow()` guard,
   re-created the overlay (new HWND `0x2609BA`) and positioned it correctly against that window
   (`1639,249` = its own header right − 41 − 280, its own top + 109). ⇒ user-visible cost is one
   subprocess re-spawn, not a broken overlay. 🎫 beta.4, with the full per-window migration.

## K19 — 📏 MEASURED: re-verified on the **shipping** binary, after the orphan cleanup

The rows in K15/K16 ran on a build made *before* 10 `extern HWND g_hwnd;` declarations — orphaned by
the `ScalePx` conversion — were removed (working rule #3). A declaration removal cannot change
semantics and the build proves each block still compiles, but "cannot change semantics" is exactly the
sort of claim this sprint keeps paying for, so the headline rows were re-run on the final binary
(pid from the post-cleanup build):

| Row | Final-binary observation |
|---|---|
| `Z1`/`Z4` | Baseline had A **above** B (Z10 vs Z11) this time; after `menu_show` in B the menu appeared at `1579,189` (B-relative) and **B moved to Z10, A to Z11**. ⭐ A stronger reading than the earlier run — B did not merely *stay* in front, it was *brought* to the front from behind |
| `A7` | Omnibox first-open in B created at **`240,189`** = B-relative |
| `A3` | Menu in B @125 % → gap **45**; menu in A @100 % → gap **36**. Both arms |

## K20 — 👤 OWNER, 2026-08-31: there is no mixed-DPI case on the Mac in normal use

Recorded so a future macOS parity pass does not stand up a rig it does not need. 👤 *"No DPI exist on
the mac, I can plug it into one of these monitors when needed but normally I just have that mac laptop
by itself."*

⇒ the Mac is normally a **single display**, so the cross-DPI condition `P3.5-A3` reproduces
(two windows, two scale factors, one process) **does not arise there in normal use**. It becomes
reachable only when the laptop is docked to one of the Windows box's monitors.

⛔ **This is not a statement that macOS is unaffected.** The macOS overlay model is different in kind
— borderless `NSWindow`s positioned by `Create*OverlayMacOS`, not `WS_POPUP` windows owned by
`g_hwnd` — so **neither** the Z-order defect nor the `ScalePx` defect transfers by argument. Phase 3.5
makes no macOS claim and changed no macOS code. If a Mac parity pass is ever scoped, it starts by
asking whether an owned-window z-order group even exists in AppKit, not by porting this fix.

## K21 — 🚨 📏 MEASURED: the shipped fix was a **no-op for 4 of the 9 overlays**, including the wallet

Found by checking my own fix before asking the owner to test it — **after** it had been committed,
verified and pushed. `4f87f78` closed `Z1` for five overlays and did nothing for four.

📏 **The evidence, from one watch run** (200 ms sampling), wallet opened in window B:

```
15:16:38.627   wallet Vis=True     B@Z11  A@Z12      <- the correction ran, B in front
15:16:38.852   (225 ms later)      A@Z11  B@Z12      <- B drops behind A anyway
```

📖 Cause: `ShowWalletOverlay` ends with **`SetForegroundWindow(g_wallet_overlay_hwnd)`** — it is one of
the overlays that takes **activation**, because it has text input (PIN entry). Activating a window
**owned by the primary** drags the primary's whole z-order group to the front, and that happens
*after* the correction, so the correction is simply undone.

📏 **Four** `Show*Overlay` functions call `SetForegroundWindow`, and they are exactly the four with
text input: **wallet, tab-list, bookmarks, profile**. The other five (omnibox, shield/cookie,
download, site-info, menu) use `SWP_NOACTIVATE` and never take focus — those were correct.

⭐⭐ **Why it survived the evidence table:** `Z1` and `Z4` were run against the **menu** and the
**shield**, and both are in the working five. The rows were green and the SUBJECT was right; the
*sample* of overlays was not. ⛔ A two-overlay sample was treated as "all 14" because K10 had
generalised from one overlay to two. **`Z4` asked "is it all overlays?" and the answer turned out to
be "the defect is, and so was the gap in the fix."**

**Fix:** move `RaiseTargetWindowAfterOverlayShow(targetWin)` to **after** the `SetForegroundWindow`
in those four, leaving the other five where they are. `SWP_NOACTIVATE` keeps activation on the
overlay, so the wallet's `WM_ACTIVATE(WA_INACTIVE)` close guard is unaffected — 📏 verified, the
wallet stayed open through the raise.

📏 **Post-fix, all four, one process, B at `80,80` inside A at `0,0`:**

| Overlay | Rect (B-relative: `B.top + 109 = 189`) | B vs A |
|---|---|---|
| wallet | `1500,181` | **B Z10 → above** A Z11 |
| tab-list | `205,189` | B Z10, A Z11 |
| bookmarks | `245,189` | B Z10, A Z11 |
| profile | `1425,189` | B Z10, A Z11 |

⭐ Note the wallet arm is the **strongest reading in the phase**: A started *above* B, and opening the
wallet in B **brought B forward**, rather than merely failing to push it back.

⭐⭐ **The lesson, and it is the sprint's own:** `feedback_own_work_is_the_weakest_link` says to panel
your own fixes. This fix was committed, pushed, and reported as complete on the strength of two
overlays out of nine. The check that caught it took four minutes and was only run because the next
step was going to be *"owner, please test this"*.

## K22 — 🚨 👤 OWNER-OBSERVED 2026-08-31, ⛔ NOT YET MEASURED: the **dismiss** path has the same defect

👤 Owner, running the `Z1` acceptance test on `4dbdf61`: *"the wallet opens correctly and window B
stays up, but when I click outside the wallet modal to close the overlay, whole window B minimizes or
goes behind window A (vanishes)."*

⇒ **The phase fixed the SHOW path and left the DISMISS path.** `Z1`/`Z2`/`Z4`/`A3`/`A7` are still
green — the owner confirmed the show half by hand across all four panels — but the symptom the phase
exists to remove is **still reachable**, one interaction later.

### ⛔ Which discriminator is unknown, and the owner's own words say so

👤 *"minimizes **or** goes behind"*. Those are visually identical and have different causes — the `Z`
column exists precisely to separate them (K9.1, where a `-32000` sighting sent this phase after a
phantom). ⛔ **Do not write "behind" into any document until it is measured.** Needed:

| Discriminator | Meaning |
|---|---|
| `Rect` reads `-32000,-32000`, or `IsIconic` true | **MINIMIZED** — something calls `ShowWindow(SW_MINIMIZE)` or the window manager iconifies it |
| B's `Z` > A's `Z`, rect unchanged | **BEHIND** — the mirror of the show-path defect |

### ⛔ My synthetic reproduction FAILED, and the reason is a real SUBJECT difference

📏 `hideprobe.ps1` opened the wallet in B, then called `SetForegroundWindow(B)` — the activation
change a click on B's content produces. Result: **the wallet never closed at all**, and B stayed at
`Z2` above A at `Z3` for 8 s.

📖 Cause of the failed repro, from `cef_browser_shell.cpp :: WalletOverlayWndProc`:

```cpp
case WM_ACTIVATE:  // ... LOWORD(wParam) == WA_INACTIVE
    if (g_wallet_overlay_prevent_close || g_file_dialog_active) {
        LOG_INFO("Wallet overlay lost activation but prevent-close active - keeping open");
        return 0;                      // <-- my run took THIS branch
    }
    HideWalletOverlay();
```

`g_wallet_overlay_prevent_close` is set to `true` **at creation** and cleared only when React reaches
a safe state (live wallet / loading / locked). ⚠️ **The dev profile this session drove has no wallet
in that state, so the flag never cleared and the overlay could not be dismissed.** The owner's profile
does, so they reach a branch this session's rig cannot.

⭐ **That is an instrument limitation worth keeping, not a footnote:** a CDP-driven probe can open
every overlay in this product, but it cannot *dismiss the wallet* unless the wallet is genuinely
usable. The dismiss half of R-CLOSE needs a real profile.

### 🧠 CANDIDATE CAUSE — 📖 a code reading, NOT established

`HideWalletOverlay` does `ShowWindow(g_wallet_overlay_hwnd, SW_HIDE)` and then returns **CEF** focus
to the right window's header (`walletFocusWin->header_browser->GetHost()->SetFocus(true)` — it does
resolve the requesting window correctly). But CEF focus is not Win32 **activation**, and hiding a
window hands activation to the next window in the z-order — which for an overlay **owned by A** is
naturally **A**.

⇒ the mirror of K11: the show path raises A's owner group and we now correct for it; the **hide** path
hands activation back to A and **nothing corrects for it**. That is consistent with "B vanishes when
the overlay closes", and it predicts **BEHIND**, not minimized.

⛔ **It is a reading and it may be wrong** — the owner's report explicitly allows "minimizes", which
this mechanism does *not* explain. If the measurement says minimized, this candidate is refuted and
the cause is somewhere else entirely. Both outcomes are informative; that is why the discriminator is
being measured before anything is written as fact.

### What this costs the phase

🔴 New row **`P3.5-Z5`** (§5.0). ⛔ The phase can no longer be described as "closed but for the
regression set" — its headline symptom is reachable on a path the evidence table never covered,
because every row in it tests **opening** an overlay and none tests **closing** one.
⭐ That is the same shape as K21 (a fix verified on 2 of 9 overlays): the table's coverage, not its
rigour, was the gap.

## K23 — 📏 MEASURED (👤 owner-run RED, assistant-run GREEN): `P3.5-Z5` — it is **BEHIND**, and it takes the keyboard too

### The RED, owner-run on `4dbdf61`, 200 ms sampling

```
16:22:12   B@Z10  A@Z11                          baseline, B in front
16:22:15   wallet Vis=True  Z1 FOCUS   B@Z10  A@Z11    wallet opens, B stays in front
16:22:17   wallet Vis=False            A@Z10 FOCUS  B@Z11
```

⛔ **Not minimized.** B reads `80,80 1820x932` in **every** sample and never `-32000`; `IsIconic`
is false throughout. → **BEHIND**, and the candidate in K22 is confirmed.

⭐ **The `Fg` column decided the fix.** Two candidate fixes were equally consistent with the z-order
data alone — *"always restore the requesting window"* vs *"restore it only if it was the one being
activated"*. The measurement shows activation lands on **A**, so the conditional variant would
**never have fired**. It was refuted by one column, before any code was written.
🎯 That column was added to `winprobe.ps1` specifically because the two fixes were indistinguishable
without it.

### ⛔ I could not reproduce this reliably, and the attempts are recorded rather than hidden

Three runs of `hideprobe.ps1`, three different outcomes:

| Attempt | `SetForegroundWindow(B)` | Result |
|---|---|---|
| 1 | refused (foreground lock) — wallet never even closed | not reproduced |
| 2 | refused; wallet closed anyway | B dropped behind A — symptom matched, **but the user action did not** |
| 3 | succeeded, but foreground landed on **A** | not reproduced |

⛔ **An instrument that gives three answers is not measuring the subject** (the K8 family again). The
owner's run is the authoritative RED for this row; mine is not. ⚠️ Attempt 2 is the dangerous one — it
produced the *right symptom for the wrong reason*, and had it been accepted, a fix could have been
"verified" against a reproduction in which activation never moved at all.

### 📖 Cause — and the code had already decided the right answer

Hiding a window that holds activation makes Windows pick a successor, and for a window **owned by the
primary** the successor is the primary. ⭐ But all four `Hide*Overlay` functions **already resolve the
window the overlay belonged to** (`walletFocusWin`, `tlFocusWin`, `bmFocusWin`, `profFocusWin`) and
return **CEF** focus to that window's header. CEF focus is not Win32 activation, so the two silently
disagreed: window B's header believed it had focus while window A held the keyboard and the top of the
z-order.

⇒ the fix is not a new policy. `RestoreTargetWindowAfterOverlayHide` makes the Win32 half agree with
the intent the surrounding code already expressed. Applied to the same **four** overlays as K21 — the
ones that take activation because they have text input.

### 📏 The GREEN, post-fix

```
16:26:09   A@Z10 FOCUS   B@Z11                        baseline: A in front AND focused
16:26:23   wallet Vis=True Z1 FOCUS   B@Z10  A@Z11    wallet opens in B, B raised
16:26:29   wallet Vis=False           B@Z10 FOCUS  A@Z11
```

⇒ exactly inverted from the RED: B keeps **both** the top of the z-order and the keyboard.
⚠️ **Driven via the toggle path**, not the click-outside path the owner used. Both funnel through
`HideWalletOverlay`, so the fix covers both by construction — but *by construction* is a code
argument, and the owner's re-run of the click-outside path is what settles the row.
📏 Tab-list open+close also exercised: no hang, no loop, no repeated activation.

## K24 — 📏 MEASURED (👤 owner-run): the K23 fix worked for **bookmarks** and **not** the wallet — and the split is the mechanism

👤 One owner watch, 2026-09-01, containing both cases on the **same binary** with the **same edit**
applied to both functions:

```
11:02:09.3   wallet    Vis=True  Z1 FOCUS   B@Z10  A@Z11
11:02:10.9   wallet    Vis=False            A@Z10 FOCUS  B@Z11    ❌ STILL BROKEN
11:02:23.3   bookmarks Vis=True  Z1 FOCUS   B@Z10  A@Z11
11:02:24.4   bookmarks Vis=False            B@Z10 FOCUS  A@Z11    ✅ FIXED
```

⭐⭐ **Neither result alone would have identified anything. The pair does.** Same helper, same call
site shape, opposite outcomes ⇒ the difference is not the fix, it is **how each overlay is
dismissed**:

| Overlay | Dismiss path | Inline `SetForegroundWindow` |
|---|---|---|
| bookmarks, tab-list, profile | `WH_MOUSE_LL` hook callback — ordinary code, no activation change in flight | ✅ works |
| **wallet** | `WalletOverlayWndProc :: WM_ACTIVATE(WA_INACTIVE)` — **inside** an activation change | ❌ silently overwritten |

📖 Win32: activation transfer to the overlay's **owner** completes *after* the `WM_ACTIVATE`
handler returns, so anything the handler does to activation is undone by the system a moment later.

**Fix:** make `RestoreTargetWindowAfterOverlayHide` **defer** the restore —
`CefPostDelayedTask(TID_UI, …, 50)`, the pattern the shell already uses for the overlay pre-warm —
so it lands after the transfer settles. The HWND is captured by value and re-validated with
`IsWindow`/`IsIconic` inside the task, because the window can close during the delay.

⚠️ **Known trade-off, stated rather than discovered later:** the restore is **unconditional**. If a
user dismisses the wallet by deliberately clicking the *primary* window, they will be pulled back to
the secondary after ~50 ms. ⛔ Not fixed, because that case has **not been observed** — fixing it
speculatively means plumbing `WM_ACTIVATE`'s `lParam` (the window *gaining* activation) through
`HideWalletOverlay`, which is real complexity for a hypothetical. ⭐ If it is ever reported, `lParam`
is the discriminator and it is already available at the call site.

### ⛔ My verification of this row is still WEAK, and it is labelled that way

📏 `hideprobe.ps1` post-fix: the wallet closed and B stayed above A. But `SetForegroundWindow`
was **refused by the foreground lock again**, and B was already in front, so *"stayed in front"*
carries almost no information. ⇒ **the owner's re-run is the evidence for `Z5`; mine is not.**
Four attempts now, none reliable — recorded so nobody later mistakes this probe for a working
instrument.

## K25 — 👤 OWNER, 2026-09-01: *"we are fixing the symptom and not the root"* — and the log agrees

👤 *"Browser B does not go invisible now. But it does have a funny looking refresh, it looks like it
goes away and then comes back... This seems like we are fixing the symptom and not the root of the
problem. It is not terrible but it is noticable."*

📏 **Measured, and it is not a repaint artifact — the window really does lose its place:**

```
11:12:00.471   wallet closed    A@Z10 FOCUS   B@Z11     <- B still goes BEHIND
11:12:00.704   (+233 ms)        B@Z10 FOCUS   A@Z11     <- the deferred restore drags it back
```

⇒ the drop is **not prevented**; it happens and is then undone. Sampling is 200 ms and the deferred
task is 50 ms, so the true visible interval is somewhere in **~50–230 ms** — comfortably long enough
to see, which is exactly what the owner reports.

⭐⭐ **The owner's read is correct and worth stating as the finding, not as feedback:** every fix in
this phase so far — `RaiseTargetWindowAfterOverlayShow` and `RestoreTargetWindowAfterOverlayHide`
alike — is a **correction applied after the wrong thing has already happened.** Both exist only
because the overlay is owned by the primary window.

### 🎯 The actual root, and it was measured on day one

📏 K11 ARM 2 already established it: with the overlay **owned by the requesting window**, the drop
does not occur at all — there is nothing to correct, on either the show or the hide path, because the
primary is never promoted in the first place. The z-order asserts exist purely to compensate for an
ownership choice made at `CreateWindowEx` time.

⚠️ It was rejected for a **measured** reason, not a guess: K12 showed an overlay owned by B is
**destroyed when B closes**. But that objection applies to *permanent* re-owning. Re-owning only for
the duration of a show — to the requesting window on show, back to the primary on hide — keeps the
overlay owned by the primary whenever it is not on screen, which is whenever a window close is likely.

⇒ three levels exist, and this phase has so far taken the cheapest:

| Level | What | Cost | Flicker |
|---|---|---|---|
| 1 — **shipped** | correct the z-order after the fact | done | ❌ ~50–230 ms visible |
| 2 | re-own to the requesting window on show, back to the primary on hide | 1 build + `Z3` re-run | ✅ none — cause removed |
| 3 | per-window overlays (14 creators take a `BrowserWindow*`) | the beta.4 migration (§0.1.1) | ✅ none |

⛔ **Do not read level 1 as wasted.** It closed the user-visible vanishing bug, and both of its
helpers become dead code under level 2 — which is the honest test of whether a fix was a patch: it is
deleted, not extended, when the cause is addressed.

## K26 — 📏 MEASURED: the root fix (level 2). Ownership follows the window; both patches deleted

👤 Owner chose level 2 from K25's table. Implemented as **temporary** ownership:
`OwnOverlayToRequestingWindow` on show, `ReturnOverlayOwnershipToPrimary` on hide, plus
`ReleaseOverlaysOwnedBy(closing, newOwner)` from both `WM_CLOSE` arms as the safety net.
⛔ `RaiseTargetWindowAfterOverlayShow` and `RestoreTargetWindowAfterOverlayHide` are **deleted**,
along with all 13 call sites — a patch that survives the real fix was not a patch.

### 📏 No intermediate drop — the flicker's cause is gone, not compensated for

```
11:21:00.4   baseline            A@Z10  B@Z11
11:21:13.9   wallet Vis=True     B@Z10  A@Z11
11:21:21.1   wallet Vis=False    B@Z10  A@Z11
```

⭐ Compare K25, same action on level 1: there the log contained an explicit `A@Z10 FOCUS / B@Z11`
sample **between** open and close, and B was dragged back 233 ms later. That sample is now **absent
entirely** — no state exists in which the primary is in front. ⚠️ Sampling is 200 ms, so a blip
shorter than that could still hide; but the level-1 flicker measured 233 ms and was captured every
time, and this is not.

### 🚨 📏 `P3.5-Z3` re-run, and it is now a REAL test rather than a pass-by-construction

The earlier `Z3` green (K16) was weak: the overlay was owned by the primary anyway, so B's
destruction could not have harmed it. Under dynamic ownership the hazard is live, and it was
exercised directly:

```
overlay = 0x630580  Vis=True  1579,189   (menu, open in B)
overlay OWNER = 0x8E0B9E   <-- window B, the window about to be destroyed
IsWindow(overlay) BEFORE closing B : True
IsWindow(B)       AFTER  closing B : False
IsWindow(overlay) AFTER  closing B : True   <== SURVIVED
```

📏 All 14 overlays still present afterwards. ⇒ `ReleaseOverlaysOwnedBy` demonstrably hands the
overlay back before `DestroyWindow`, and K12's destruction result is now prevented rather than
avoided.

⭐ **Incidental:** the same safety net is called from the primary-transfer arm, so it also addresses
K18.2 (closing the primary destroyed all 14 overlays). Not claimed as fixed — it has not been
re-measured on that path.
