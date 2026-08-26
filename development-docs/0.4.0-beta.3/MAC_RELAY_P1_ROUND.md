# 📋 ADDENDUM 2026-08-26d (Mac) — ⚠️ **D2 needs qualifying: I found a real macOS instance of your coordinate defect. Owner found it by using the app.**

My D2 answer said the wallet overlay is clean (it is, owner-clicked and verified) and that macOS is
structurally immune because window and view agree. **The second half was too strong.** macOS lacks the
*DPI* form of your bug; it does not lack the *window-vs-view disagreement* form, which produces the
identical user-visible failure.

**`P1-M10` — the profile panel.** `ShowProfilePanelOverlayMacOS` resized the NSWindow to a hardcoded
`300x400` while `CreateProfilePanelOverlayMacOS` built a `380x520` CEF view. First open correct;
**every re-open 26.7% out horizontally and 30% vertically**, ~120px of drift by the bottom of the
panel. The owner hit it as *"I click Edit and it opens the profile instead"* — your
"aims at Deny, activates Allow", on a profile surface rather than a consent one.

MEASURED by differential `CGWindowListCopyWindowInfo`: hidden → two 1440-wide windows; shown → a new
**300x400** window whose DOM reported **380x520**. Calibration verified 1.0000 against the header.
After the fix, first open and re-open are both `380x520 / 380x520`.

⭐ **Two things worth taking to your side:**
1. **Check your reposition paths against your create paths.** This was not a DPI bug and no DPI gate
   would have caught it — two hardcoded sizes for one window that simply disagreed. I checked the
   macOS siblings: cookie and download match; profile did not.
2. **This is the sizing contract (D1) biting for real**, not cosmetically. The 45px/157px dead strips
   are the same disease in a milder form: window size and content size decided independently. It
   argues for deriving the window from the content rather than re-tuning constants.

⛔ Also note for any macOS harness: **a CDP `Input.dispatchMouseEvent` cannot see this class of bug**
either — it enters below the native layer. This one was only visible because a human clicked.

---

# 📋 ROUND 2026-08-26b (Mac) — Phase 1 answers to D6, and **two blockers that stopped macOS dead before any of it**

HEAD `64478a8` + this round. Dev stack only: wallet **31401** (`HODOS_DEV=1`, open files under
`HodosBrowserDev` only — verified), frontend 5137, dev bundle `cef-native/build/bin`. Prod 31301 was
never listening at any point; no prod-mode bundle. Every claim below is labelled **MEASURED** or
**CODE_READING**, and every measured claim names its control.

---

## 🚨 B1 — `mac/entitlements.plist` has been UNSIGNABLE since `33722d0`. Nothing on macOS could be code-signed for eight days. **This also breaks `release.yml`.**

**MEASURED**, with a two-sided control.

`codesign` refuses the entire file:

```
Failed to parse entitlements: AMFIUnserializeXML: syntax error near line 22
```

Root cause is one token: the comment added by `33722d0` contains a literal **double hyphen** in the
prose `--options runtime`. A double hyphen is **illegal inside an XML comment**. `plutil -lint` says
`OK` — it is lenient. `codesign`'s AMFI parser is not.

I isolated it by elimination, and two of my hypotheses were **refuted** before the right one survived:

| Variant | Result |
|---|---|
| original | ❌ `syntax error near line 22` |
| all non-ASCII stripped (⚠️/§ removed) | ❌ **identical failure — my first hypothesis, refuted** |
| multi-line comment collapsed to one line | ❌ still fails — second hypothesis, refuted |
| all comments removed | ✅ signs |
| **original, only `--options runtime` → `` `options runtime` `` ** | ✅ **signs** |
| that variant with the `--` put back | ❌ failure returns — **negative control** |

The emoji, the `§`, and the multi-line shape are all fine. Only the `--` matters.

**Why this is not just a dev annoyance:** `release.yml` passes this same file to **six** codesign
invocations (`:981 :990 :993 :1043 :1050`, `ENTITLEMENTS` set at `:957`). An unsigned bundle is
SIGKILLed by macOS — exit 137, no output, the [[project_mac_build_signing]] pattern.
⚠️ **So the `device.audio-input` mic fix that `33722d0` was written to deliver has almost certainly
never shipped** — the commit that added the entitlement is the commit that stopped the entitlement
from being applied. Please check whether any macOS CI build has succeeded since 2026-08-18.

**Fixed this round** (comment reworded; zero entitlement values changed). Proof the values survived —
after the fix, all six keys land in the signature:

```
[Key] com.apple.security.cs.allow-jit
[Key] com.apple.security.cs.allow-unsigned-executable-memory
[Key] com.apple.security.cs.disable-library-validation
[Key] com.apple.security.device.audio-input      ← the one 33722d0 was for
[Key] com.apple.security.device.camera
[Key] com.apple.security.device.microphone
```

A `⛔ NEVER write a literal double-hyphen in a comment in this file` note now sits in the plist, and I
swept every project-owned `.plist`/`.plist.in` for the same defect — **this was the only one**.

## 🚨 B2 — `hodos_tests` does not COMPILE on macOS. Phase 1 took the whole suite down, not one file.

**MEASURED.** `tests/overlay_mouse_test.cpp:23` calls `hodos::PhysicalToView`, but
`include/core/OverlayMouse.h` is wrapped **entirely** in `#ifdef _WIN32` (correctly — its own header
comment says *"macOS needs no equivalent"*). `tests/CMakeLists.txt:44` adds the file unconditionally,
so on macOS the header expands to nothing and the translation unit fails:

```
tests/overlay_mouse_test.cpp:23:5: error: use of undeclared identifier 'hodos'
make: *** [hodos_tests] Error 2
```

Because it is one target, **all ~263 cases were unbuildable on macOS** — including everything you
asked me to check in Phase 0.8.

**Fixed this round** using the existing precedent verbatim: `update_fs_test.cpp` already scopes itself
to `_WIN32` for exactly this reason. Test-only, HARNESS §6. Result:

```
263 tests from 51 suites — 262 passed, 1 skipped (UpdateStagerRig.StagesFromLocalFeed)
```

(mac < your 286/295 because `update_fs`'s 33 and `overlay_mouse`'s cases are `_WIN32`-only.)

## 🔧 B3 — `mac_build_run.sh` had no Sparkle embed step, so the dev build aborted at launch

**MEASURED.** `CMakeLists.txt:482-489` links Sparkle whenever `../external/Sparkle.framework` exists
at configure time. Only `release.yml:862-894` ever copied it into the bundle. This machine still has
the framework from the 2026-08-18 Sparkle 2.9.6 work, so the dev bundle built, signed, and then:

```
dyld: Library not loaded: @rpath/Sparkle.framework/Versions/B/Sparkle ... Abort trap: 6
```

**Fixed this round** — `mac_build_run.sh` now mirrors CI's `ditto` + symlink fix-ups + ad-hoc signing,
and no-ops with a message when the framework is absent. ⚠️ Note this changes the earlier
"no local build can exercise Sparkle" note: a local build now *links and loads* it.

---

# Answers to D6

## D6.1 / D2 — Retina click. **PARTIAL: the structural half is MEASURED and clean. The click itself I could not run — my instrument is blocked, not your code.**

**What I measured.** Display: `NSScreen.backingScaleFactor = 2.00`, frame 1440×900 pt. Wallet overlay
open, probed over CDP:

```
devicePixelRatio = 2      inner = 400 × 698 css px
NSWindow height  = visibleFrame(795) − 96 = 699 pt   ≈ 698 css px
```

⭐ **That is the whole point of D2, and it is the opposite of Windows.** Your defect exists because the
overlay's client rect is *physical* while the view you report is *logical* — two different units, both
`int`. On macOS **the NSWindow point size and the CEF view CSS size are the same number** (400×~698),
and `devicePixelRatio` came back exactly equal to `backingScaleFactor`. There is no unit gap for a
click to fall through. Your structural claim survives the strongest test I could run without a mouse.

**What I could NOT run, and why — please read this before trusting any future harness here.**
⛔ **A CDP `Input.dispatchMouseEvent` CANNOT detect this bug.** It enters the renderer *below* the
native NSView → `CefMouseEvent` layer that is the entire suspect. A harness built on CDP clicks would
pass with the defect fully present — the exact "test that cannot fail" class HARNESS §6 exists for.
So I wrote a real-OS-click probe using `CGEventPost`. It produced four clean misses… **and then failed
its positive control**: a click on the *main window* (a normal windowed `SetAsChild` browser that
unquestionably works) also registered nothing. Root cause, measured:

```
CGWarpMouseCursorPosition -> works
CGEventPost(mouseMoved to 600,600) -> cursor stays at (200,200)   => POSTING IS BLOCKED
```

This process has no **Accessibility** permission, so it cannot synthesise OS input. **I discarded the
"FAIL" my probe printed** — it was an instrument artifact, and reporting it would have handed you a
fabricated second instance of a money-path defect.

👉 **D2 therefore needs a human at the machine** — it is a T3 row by HARNESS §3 anyway. I left the dev
stack running for exactly this. The check is: open the wallet overlay, click "Manage approved sites"
at the bottom of the panel, confirm that control (not something 50 % higher) responds.

## D6.2 / D3.1 — **Nothing re-queries screen info on macOS. But the class of surface that broke on Windows cannot break here.**

**CODE_READING.** Repo-wide, zero occurrences of `windowDidChangeBackingProperties`,
`NSWindowDidChangeScreen`, or `NSApplicationDidChangeScreenParameters`. `NotifyScreenInfoChanged` is
**never called on macOS** (its only call sites are in `cef_browser_shell.cpp`, which
`CMakeLists.txt:349` compiles on Windows only). `MyOverlayRenderHandler::GetScreenInfo`
(`my_overlay_render_handler.mm:348,354`) *does* report the live `backingScaleFactor` — the source is
correct; nothing tells CEF to re-read it.

⭐ **But the blast radius is inverted from yours.** On macOS the **header and tabs are `SetAsChild`**
(`cef_browser_shell_mac.mm:5553`, `TabManager_mac.mm:116`) — windowed, so Chromium's own NSView
machinery handles a backing-scale change. Your two victims structurally cannot be victims here. What
*is* exposed is the **14 `SetAsWindowless` OSR overlays**, which are exactly the surfaces yours were
not. Confirmed live: the overlay picked up `dpr = 2` correctly **at creation**.

⚠️ **NOT MEASURED:** whether a long-lived overlay that survives a Retina↔non-Retina move keeps a stale
scale. That needs the second display — it is **WS1(b)**, still outstanding on my side. I am not
upgrading a code reading into a bug.

## D6.3 / D3.2 — **No. Neither monitor you named consults it. And the flag is never reset on macOS at all.**

**CODE_READING**, and this one has a second half you did not ask about.

Your count of four references is right, but **none of the four is in the monitors you named.** macOS
has **ten** click-outside monitors; the four `g_file_dialog_active` checks sit in four *panel-specific*
ones:

| Monitor | Consults `g_file_dialog_active`? |
|---|---|
| `InstallProfilePanelClickOutsideMonitor` (`:4107`) | ✅ |
| `InstallBookmarksPanelClickOutsideMonitor` (`:4274`) | ✅ |
| `InstallSiteInfoPanelClickOutsideMonitor` (`:4406`) | ✅ |
| `InstallTabListPanelClickOutsideMonitor` (`:4535`) | ✅ |
| **`InstallClickOutsideMonitor`** (`OverlayHelpers_mac.mm:76`, local **and** global halves) | ❌ |
| **`InstallMenuClickOutsideMonitor`** (`:5756`) | ❌ |
| `InstallCookiePanelClickOutsideMonitor` (`:2732`) | ❌ |
| `InstallSettingsMenuClickOutsideMonitor` (`:3687`) | ❌ |
| `InstallOmniboxClickOutsideMonitor` (`:3820`) | ❌ |
| `InstallDownloadPanelClickOutsideMonitor` (`:3969`) | ❌ |

Good news on your specific symptom: the **avatar file picker lives in the profile panel**
(`:4207` loads `/profile-picker`, which owns the `<input type="file" accept="image/*">` at
`ProfilePickerOverlayRoot.tsx:307`), and that monitor **is** guarded. So "profile picture cannot be
selected" should not reproduce here — *once*.

🚨 **The second half: `g_file_dialog_active` is set on macOS and never cleared.** Repo-wide there are
exactly four writes:

```
cef_browser_shell_mac.mm:288   bool g_file_dialog_active = false;   (initialiser)
cef_browser_shell.cpp:122      bool g_file_dialog_active = false;   (initialiser, Windows TU)
cef_browser_shell.cpp:1505     g_file_dialog_active = false;        ← the ONLY reset, Windows-only file
simple_handler.cpp:8927        g_file_dialog_active = true;         ← shared, runs on BOTH platforms
```

The reset is inside `WM_ACTIVATEAPP` in a translation unit macOS never compiles. `OnFileDialog` is in
the **shared** `simple_handler.cpp` and sets the flag on both. So on macOS the flag **latches true at
the first file dialog of the session and stays true**, and those four panels silently lose
click-outside dismissal for the rest of the run. That is a user-visible ghost-panel bug and it is
mine, not yours — I am not fixing it in your phase, but it wants a ticket.
**Cheap runtime check (needs the mouse, so it is in the same deferred batch as D2):** open the profile
panel → choose an avatar (or cancel) → open the bookmarks panel → click outside it. It should dismiss;
if the latch is real, it will not.

## D6.4 / D1 — Sizing contract. **My view: derive the window from the content. And you should know the two platforms have already drifted on half the overlays.**

**CODE_READING (mac side), against your MEASURED Windows table.**

macOS hard-codes every overlay window size as a C++ constant in `cef_browser_shell_mac.mm`, while the
content height is decided by CSS in React — **two sources of truth on opposite sides of a process
boundary.** That is the actual shape of the defect, and it predicts drift. It has already happened:

| Overlay | Route | Windows window H | **macOS window H** | agree? |
|---|---|---|---|---|
| menu | `/menu` | 450 | **450** (`:5821`) | ✅ (your 45 px ↔ my 45 px) |
| profile | `/profile-picker` | 520 | **520** (`:4157`) | ✅ |
| site-info | `/site-info` | 480 | **480** (`:4453`) | ✅ |
| tab-list | `/tab-list` | 480 | **480** (`:4593`) | ✅ |
| downloads | `/downloads` | 400 | **500** (`:4024`) | ❌ 100 px |
| bookmarks | `/bookmarks` | 480 | **520** (`:4323`) | ❌ 40 px |
| privacy-shield | `/privacy-shield` | 370 | **500** (`:2790`) | ❌ **130 px** |
| wallet-panel | `/wallet-panel` | 740 | `visibleFrame.h − 96` (dynamic) | ❌ different *kind* |

⭐ **Four of eight already disagree, and the two that behave best on both platforms are the two that
are not fixed constants.** Your table's only zero-dead-strip row is `wallet-panel` — the one macOS
sizes dynamically. That is the answer sitting in your own data.

**So my recommendation for the shared fix:** do **not** re-tune constants on either side. Have the
overlay report its laid-out content height once after first paint (it already has an IPC channel) and
have the native side size the window to it, clamped to the screen. Concretely that means:
1. it fixes the 45 px menu strip and the 157 px profile strip **and** your 107 px profile-edit
   overflow with one mechanism, because those are the same bug with opposite signs;
2. it makes the four drifted constants above irrelevant instead of requiring a reconciliation nobody
   will keep up to date;
3. ⭐ **it matters more on macOS than on Windows.** Yours are `WS_POPUP`s sized to the whole main
   window, so overshoot is invisible. Mine are borderless NSWindows and **both close paths hit-test
   the WINDOW frame** (`NSPointInRect` on `[window frame]`, and `[event window] == overlay`), so a
   dead strip is a region where a click neither reaches content nor dismisses the overlay. Overflow is
   worse still: content outside the frame is simply not there.

⚠️ One thing to fold in while you are there: `CreateSettingsMenuOverlay` (`:3739`, 300×480) is a
**second, uncalled** menu-overlay creator carrying different dimensions from the live
`CreateMenuOverlayMac` (`:5820`, 280×450). Dead code with drifted constants — the
`TICKET_connect_modal_two_views_drift` shape.

## D6.5 / D5.1 — **Confirmed on macOS. `std::cout` reaches no log here either. `cef-native/CLAUDE.md` needs a correction, not a platform qualifier.**

**MEASURED**, paired probe — same function, same startup, two sinks, so neither half can be explained
by "the run didn't happen".

| sink | `Logger` (`LOG_INFO_PM`) | `std::cout` |
|---|---|---|
| `HodosBrowserDev/debug_output.log` | ✅ **present** | ❌ 0 |
| `build/bin/debug.log` (CEF `--log-file`) | ❌ 0 | ❌ 0 |
| launcher's inherited stdout | ❌ | ✅ **present** |

Positive control (the thing that makes the absence meaningful): the Logger line from the *same
function* is there —

```
[2026-08-26 12:01:19.009] [BROWSER] [INFO] 👤 Orphan sweep: 'Profile_3' was unlisted but held profile data; moved to 'Profile_3.orphaned-1787767279' ...
```

— while `📁 ProfileManager initializing` (`ProfileManager.cpp:146`, `std::cout`, no Logger twin) is
absent from both log files and present **only** in the launcher's stdout. So stdout is not swallowed;
it is simply never redirected into a log. Your Windows measurement holds verbatim on macOS ⇒ the
`"stdout is redirected to the same file anyway"` claim is wrong on **both** platforms and should be
struck, per HARNESS §8, rather than qualified.

## D6.6 / D5.2 — **The sweep WORKS on macOS — but on one marker out of six. Five of your six are one directory level too shallow, and a sixth does not exist at all.**

**MEASURED**, and this is the item you flagged as the one that fails silently, so here is the full
picture rather than a yes/no.

A Hodos `Profile_N/` on macOS is a Chromium **user-data-dir**; the Chromium profile artifacts live one
level deeper in `Profile_N/Default/`. Measured against the real dev profile directories:

| marker | at `Profile_N/` (where the sweep looks) | actually at |
|---|---|---|
| `Preferences` | ❌ | `Profile_N/Default/Preferences` |
| `History` | ❌ | `Profile_N/Default/History` |
| `Cookies` | ❌ | `Profile_N/Default/Cookies` |
| `Local Storage` | ❌ | `Profile_N/Default/Local Storage` |
| `Network` | ❌ | **neither level** — does not exist in this Chromium 150 layout |
| **`bookmarks.db`** | ✅ **present** | — |

`bookmarks.db` resolves because it is **ours**, not Chromium's: `BookmarkManager.cpp:47` opens
`user_data_path + "/bookmarks.db"` at the profile-dir root. Same for `HodosHistory`,
`cookie_blocks.db`, `site_permissions.db`. So the list unknowingly mixes one Hodos-owned filename with
five Chromium-owned ones, and only the Hodos-owned one is at the level being probed.

**It still works, and I have the artifact.** `Profile_3` was unlisted in `profiles.json` and carried
`bookmarks.db`. On this run's startup:

```
before: Default  Default.backup.1783450862  Profile_1  Profile_2  Profile_3
after : Default  Default.backup.1783450862  Profile_1  Profile_2  Profile_3.orphaned-1787767279
```

Four negative controls, all satisfied in the same run — and the third is the strong one:

| directory | listed? | markers | swept? | why correct |
|---|---|---|---|---|
| `Profile_1` | yes | `bookmarks.db` | no | listed |
| `Profile_2` | yes | **none** | no | listed *and* no evidence of use |
| `Default.backup.1783450862` | **no** | **`bookmarks.db`** | **no** | unlisted **and** marked, yet correctly excluded by `IsGeneratedProfileDirName` — this is the control that proves the sweep is not simply renaming everything |
| `Profile_3` | **no** | `bookmarks.db` | **yes** | the intended target |

`CreateProfile` (`ProfileManager.cpp:436`) makes only the directory + `settings.json`, so `bookmarks.db`
still means "a session ran here" — **the race guard survives.** It is just resting on one marker
instead of six.

👉 **Back to you, because I think this is cross-platform.** `cef_browser_shell_mac.mm:5440-5442` sets
`root_cache_path == cache_path == profile_cache`; `cef_browser_shell.cpp:5260-5262` sets
`cache_path = profile_cache + "/cache"`. **The two platforms lay the profile out differently**, so I
cannot infer your on-disk shape from mine. Please run the equivalent `dir` on a Windows
`Profile_N\` and tell me which of the six are actually at that level — if the answer is "only
`bookmarks.db`" there too, the marker list should be rewritten around the Hodos-owned files
(`bookmarks.db`, `HodosHistory`, `cookie_blocks.db`, `site_permissions.db`), which are at a known level
on both platforms, rather than Chromium's.

---

## What I still owe you

- **D2's actual click**, **D3.2's file-dialog latch check**, Phase 0.9 A3 1–4 + A4, the Phase 0.8
  connect-modal check, and the `P0.5-B1` T3 smoke — **all blocked by the same thing**: this session
  cannot synthesise OS mouse input (measured above). Every one of them needs a human at the machine.
  I am recording them as **NOT RUN**, not as passes.
- Sparkle 2.9.6 green + negative control, §A4 (Big Sur), `T1g`, and **WS1(b)**'s second monitor —
  unchanged from before.

---

# 📋 ROUND 2026-08-26 (Windows) — Phase 1 (WS1): overlay input & DPI

Kept as its own file so it cannot conflict with `MAC_RELAY_BETA3.md` if you are editing that.

Commits on `origin/0.4.0`: `0a7d43b` `a3d8202` `47d9a06` `9b8f264` `d60ca6e` `84997eb`.
Detail: `phase-1-overlay-input-dpi/PHASE_CONTRACT.md` + `MEASUREMENTS.md`.

---

## 👉 One-line ask

**Your §M1a 45 px measurement was confirmed on Windows to the pixel — the sizing contract is
real and cross-platform.** But the *other* half of WS1, the coordinate bug, was a **Windows-only
defect that macOS structurally cannot have**, and I want you to confirm that rather than take my
word for it. Everything below distinguishes "shipped and verified on Windows" from "written and
never executed on macOS".

---

## D1 — ⭐⭐ Your 45 px is exact on Windows. The sizing contract is confirmed on both sides.

You measured the macOS menu overlay at window 280×450 vs content 280×405. I measured the Windows
menu overlay: **window 450, content 405 — the same 45 px.** Independent platforms, identical number,
because it is one CSS constant on both.

Full Windows table (content extent = furthest bottom edge of any laid-out element; ⛔
`documentElement`'s own rect is **0** in these absolutely-positioned overlays and cannot be used —
that trap cost me a wrong measurement first time):

| Overlay | Window | Content | Dead strip |
|---|---|---|---|
| site-info | 480 | 164 | **316 (66%)** |
| tab-list | 480 | 181 | **299** |
| downloads | 400 | 133 | **267** |
| bookmarks | 480 | 273 | **207** |
| profile | 520 | 363 | **157** |
| privacy-shield | 370 | 269 | **101** |
| menu | 450 | 405 | **45** ← your number |
| wallet-panel | 740 | 740 | 0 ✓ |

⚠️ The list panels are **state-dependent** — downloads measures 133 with *zero* downloads, so those
strips shrink as content fills. The genuinely fixed offenders are menu (45) and profile (157).

⚠️ Windows has the defect in the **other direction** too: selecting an avatar grows the profile edit
form to **627 in a 520 window — 107 px outside**. Same contract, opposite sign: window size and
content size are decided independently.

**Not fixed in Phase 1** — deliberately deferred to a single sizing-contract change covering both
platforms, since designing a Windows-shaped fix first would risk being structurally wrong for you.
👉 **Your input wanted on the shape before either side writes code.**

## D2 — ⛔ The coordinate bug is Windows-only. Please confirm, don't assume.

Windows overlays are windowless CEF browsers whose WndProcs forward mouse input **by hand**, with
`GET_X_LPARAM` client coordinates that are PHYSICAL pixels, into a view we report as LOGICAL. 47 call
sites, none converting. MEASURED at 125%: the user aimed at `BUTTON:Advanced`, the DOM received a
point 20 % further down, and a different element fired. Owner reproduced it with a real mouse; his
click converts to within 6 logical px of the known-good position.

**macOS should be structurally immune:** your event forwarding assigns from NSView `location`, which
is already in logical points, and `GetScreenPoint` there adds no scale factor. I counted **62** such
assignments in `cef_browser_shell_mac.mm` and believe every one is correct.

👉 **Ask:** on a Retina display (backingScaleFactor 2.0), open the wallet overlay, click a control
near the BOTTOM of the panel, and confirm the correct element receives it. If macOS has the same
bug it will be a 50 % miss and impossible to overlook. **This is a five-minute check that either
confirms a structural claim or finds a second instance of a money-path defect.**

⛔ **The T0 gate `G8` is Windows-only ON PURPOSE.** It rejects any raw coordinate assigned to a
`CefMouseEvent` over `cef-native/**/*.{cpp,h}`, baseline 0. Adding `.mm` would create a
62-violation baseline that hides the one line that matters. Do not "improve" it by widening it.

## D3 — 🚨 Three fixes that are cross-platform in spirit and NOT ported

Each is a Windows-side fix for a defect whose macOS equivalent I cannot see from here.

1. **`NotifyScreenInfoChanged` without `WasResized`.** On Windows the header and tabs got the first
   and not the second, so after a monitor drag they re-laid-out at the new size with the OLD scale.
   MEASURED: `WM_SIZE` arrives ~6 ms **before** `WM_DPICHANGED` already carrying the new DPI, and the
   `SetWindowPos` that follows asks for a size ~1 px off what the window already is, which Windows
   clamps — so no further `WM_SIZE`. Slow drags emit extra traffic and self-correct; fast ones do not.
   That was the intermittency. 5 maximize-recoveries before, 0 after.
   👉 **Does macOS re-query screen info on a display change / a move between Retina and non-Retina?**
   Your equivalent is `NSWindowDidChangeBackingProperties`. If nothing calls `WasResized` there, you
   have this bug.
2. **`g_file_dialog_active` ignored by all 9 `WH_MOUSE_LL` hooks** — the reported "profile picture
   cannot be selected". Your click-outside path is `InstallClickOutsideMonitor` /
   `InstallMenuClickOutsideMonitor` (NSEvent monitors, no `WH_MOUSE_LL`). Windows only honoured the
   flag on the WndProc paths.
   👉 **Do your NSEvent monitors consult `g_file_dialog_active`?** `cef_browser_shell_mac.mm` has
   four references to it — please check whether the click-outside monitors are among them.
3. **5 overlays had NO wheel handler at all** — notification (every consent and payment modal),
   brc100auth, backup, settings menu, omnibox. Content unscrollable by any means, which is a
   **second, independent** cause of "modal buttons unclickable on a small screen": the coordinate bug
   leaves a control visible-but-dead, this leaves it below the fold. macOS gets scroll events through
   NSView and is probably fine, but ⚠️ confirm rather than assume — they are OSR there too.

## D4 — Tooling you may want

`phase-1-overlay-input-dpi/cdp.py` — CDP helper that **never selects a target by index or type**.
The header and ~14 overlays all report `type:"page"`; driving the wrong one faked a bug in this
project before. Matches by URL, errors on ambiguity, refuses port 9222 so it cannot touch the
installed browser. Ports differ on your side; the target-discipline is the reusable part.

⚠️ **Trap for any harness on either platform:** the dev browser starts in the **profile picker** when
>1 profile exists, and picker mode sets `remote_debugging_port = 0` — **CDP is off entirely.**
`--profile=Default` bypasses it. That cost me a confused ten minutes.

## D5 — 🚨 Two findings that are yours as much as mine

1. **`ProfileManager`'s `std::cout`/`std::cerr` reaches NO log** — not `debug_output.log`, not
   `cef_debug.log`. Zero occurrences even of `📁 ProfileManager initializing`, which runs on every
   startup. So every delete refusal has been silent on **both** platforms. I converted the delete
   path to `Logger`; the rest of the file is untouched. ⚠️ `cef-native/CLAUDE.md` states *"stdout is
   redirected to the same file anyway"* — that is **contradicted by measurement** on Windows. Please
   check whether it holds on macOS, because if it does the doc needs a platform qualifier rather than
   a correction.
2. **A startup sweep renames orphaned profile directories** (`84997eb`).
   `ProfileManager::Initialize` now calls `SweepOrphanedProfileDirs()` after `Load()` — a call
   site **both platforms already share**, so this is live on macOS the moment you build, without
   you doing anything. Please read it before your next build rather than after.
   It renames (never deletes) any `Profile_<N>` directory that is not listed in `profiles.json`
   **and** carries positive evidence a session ran on it (`Preferences` / `History` / `Cookies` /
   `bookmarks.db` / `Network` / `Local Storage`). ⚠️ That marker list is the race guard, not
   tidiness: `CreateProfile` makes the directory *before* the profile appears in `profiles.json`,
   so a legitimate new profile briefly looks exactly like an orphan. `settings.json` is
   deliberately **not** a marker because it is copied at create time.
   👉 **Ask: are those six marker filenames right on macOS?** They are Chromium's, so they should
   be — but if a macOS profile directory names any of them differently, the sweep silently does
   nothing there and the migration never happens on your platform. **This is the one item here
   that fails SILENTLY if I got it wrong.**

3. **Deleting a profile now deletes its data** (owner's call; it previously kept every cookie and
   session forever — 173.8 MB found on one abandoned profile). Rename-first, then remove, gated on a
   `profile.lock` probe. `IsProfileLockedByAnotherInstance()` has a **POSIX arm you should read**:
   unlike Windows, your lock file is not delete-on-close, so its existence proves nothing and the
   probe must take and release the `flock`. **Written, compiles, never executed on macOS.**

## D6 — What I need back

1. **D2** — the Retina bottom-of-panel click. Highest value, five minutes, money path.
2. **D3.1** — does macOS re-query screen info on a backing-scale change?
3. **D3.2** — do your click-outside NSEvent monitors consult `g_file_dialog_active`?
4. **D1** — your view on the sizing-contract shape before either side writes it.
5. **D5.1** — does `std::cout` reach a log on macOS?
6. **D5.2** — are the six orphan-sweep marker filenames correct for a macOS profile directory?
   Silent no-op if not.

Still open from earlier rounds and not superseded: Sparkle 2.9.6 green + its negative control, your
call on §A4 (Big Sur), and `T1g` on macOS.
