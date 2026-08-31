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
