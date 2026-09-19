# 🍎 Mac relay — beta.3 Phase 11, omnibox cluster (items 2, 3, 4)

**From:** Windows · **Opened:** 2026-09-18 · **Standard:** `HARNESS.md`
**Why this file exists:** root `CLAUDE.md` build rule — *nothing compiles C++ on push*, so every commit
touching `cef-native/**` names its files here and the other platform rebuilds after its next rebase.

---

## ⚠️ Rebuild required — shared C++ touched

| File | Platform split? | Item | What changed |
|---|---|---|---|
| `cef-native/src/handlers/simple_handler.cpp` | ❌ none — pure CEF, no `#ifdef` added | 3 | New `omnibox_navigated` IPC arm, next to the existing `omnibox_hide` arm. Forwards the clicked URL to the **owning window's** `header_browser`. ⛔ Deliberately **not** `SimpleHandler::GetHeaderBrowser()`, which resolves the **primary** window's header — in a second window the URL would land in the wrong address bar |
| `cef-native/src/handlers/simple_render_process_handler.cpp` | ❌ none | 3 | Relays `omnibox_navigated` to the header document as `window.postMessage({type:'omnibox_navigated', url})`, same shape as the existing `omnibox_autocomplete_update` relay |

| `cef-native/src/handlers/simple_app.cpp` | ❌ none — CEF preference API, cross-platform | 9 | 🚨 `--disable-features=Autofill` named a feature that **does not exist** (verified against the Chromium source: `BASE_FEATURE(feature, name, …)` takes the name as its 2nd arg, and none declares `"Autofill"`), while autofill was live and recording form input to `<profile>/Default/Web Data`. Token removed; `DisableChromiumAutofill()` now sets `autofill.profile_enabled` and `autofill.credit_card_enabled` to false in `OnContextInitialized`. ⛔ `GlicActorUi` kept — it is the CEF 150 crash fix |

| `cef-native/src/handlers/simple_handler.cpp` + `include/handlers/simple_handler.h` | ❌ none | 8 | Two new broadcasters, `BroadcastProfilesChanged()` and `BroadcastSettingsChanged()`, called from the profile and settings mutators. 🚨 The settings one replaces a guard on `header_browser_`, a static that was **never assigned** — so changing the search engine did nothing until a restart. That static and its declaration are deleted |

Both files are shared and carry **no** `#ifdef _WIN32` / `#elif defined(__APPLE__)` in the new code,
so the macOS build needs nothing but a rebuild.

🍎 **macOS should re-run item 9's evidence on its own profile**: the `autofill` table lives at
`~/Library/Application Support/HodosBrowserDev/Default/Default/Web Data`. Type a probe value into any
form, submit, quit the browser, and confirm no row is added. ⚠️ And check whether the **installed**
macOS build has already accumulated rows, as the Windows one has.

## 🍎 What macOS should check when it gets here

⚠️ **The measured cause of item 3 is Windows-specific in one respect, and the fix is not.**

The defect: clicking an omnibox suggestion leaves the header's address `<input>` effectively in edit
mode, so the address bar never shows the URL that was clicked. On Windows that is because the omnibox
overlay's WndProc returns **`MA_NOACTIVATE`** on `WM_MOUSEACTIVATE` (deliberate — the dropdown must
not steal the caret from the address bar). 📏 Measured: the header input blurs for ~18 ms and
re-focuses, so the tab-sync effect gets **exactly one run, with the pre-navigation URL**, and is
blocked from then on.

macOS overlays are borderless `NSWindow`s, not `WS_POPUP`, and click-outside is handled by
`InstallClickOutsideMonitor()` — ⬜ **whether the same focus flicker happens there is unmeasured.**
Either way the fix is the right one on both platforms (it is what Chrome does: the bar shows the
destination before the page arrives), and it is in shared code, so macOS gets it for free.

⬜ **Owed from macOS:** re-run the item-3 row on a Mac with the probe's equivalent —
`development-docs/0.4.0-beta.3/phase-11-ui-leftovers/omniboxprobe.py` is Windows-only (it reads
`IsWindowVisible` on the `CEFOmniboxOverlayWindow` HWND via `user32`), so the HWND-layer half of
item 2 needs a macOS analogue before item 2's row can be called green there.

⚠️ Items 2 and 4 are **React-only** (`MainBrowserView.tsx`, `OmniboxOverlayRoot.tsx`) and relay with
the frontend — no rebuild needed for those, but they ride in the same branch.

---

# 🍎 Round 2 — items 5 → 10, added 2026-09-18 end of session

## Rebuild required (shared C++, no platform split added in any of it)

| File | Item | What changed |
|---|---|---|
| `src/handlers/simple_app.cpp` | **9** | `--disable-features=Autofill` removed (it named a feature that **does not exist**), and `DisableChromiumAutofill()` added to `OnContextInitialized` — sets `autofill.profile_enabled` and `autofill.credit_card_enabled` to false via `CefPreferenceManager::SetPreference`. ⛔ `GlicActorUi` kept — it is the CEF 150 hard-crash fix |
| `src/handlers/simple_handler.cpp` + `include/handlers/simple_handler.h` | **8** | `BroadcastProfilesChanged()` and `BroadcastSettingsChanged()`; the `header_browser_` static is **deleted** (it was defined `= nullptr` and assigned nowhere, so the settings broadcast it guarded had never fired) |
| `src/handlers/simple_handler.cpp` + `src/handlers/simple_render_process_handler.cpp` | **3** | `omnibox_navigated` IPC (round 1 above) |

Nothing else in `cef-native/` was touched. Items 2, 4, 5, 6, 7 changed no C++ at all.

## 🚨 Run this on macOS first — item 9, and it is a privacy row

📏 On Windows, autofill was **live** and had been recording form input to
`<profile>/Default/Web Data` → table `autofill`, including two real email addresses, while the code's
own comment claimed it was disabled.

⬜ **Check the same table on macOS**, at
`~/Library/Application Support/HodosBrowserDev/Default/Default/Web Data`:

```sql
SELECT name, value, count FROM autofill;
```

Then type a probe value into any form on any site, submit it, **quit the browser** (the DB is locked
while it runs) and re-read. Pre-fix a row is added; post-fix none is. ⚠️ And check the **installed**
macOS build too — it will have been accumulating rows the same way.

## ⛔ Item 7 route 1 — the guard is a NO-OP on macOS, and the macOS question is different

`frontend/src/App.tsx` now cancels `wheel` events with `ctrlKey` so the browser chrome cannot be
page-zoomed. React-only, relays with the frontend, no rebuild.

⛔ **It does nothing on macOS, and that is expected.** Chromium compiles the whole ctrl+wheel→zoom
path out there:

```cpp
// content/browser/web_contents/web_contents_impl.cc :: HandleWheelEvent
#if !BUILDFLAG(IS_MAC)
  // On platforms other than Mac, control+mousewheel may change zoom. On Mac,
  // this isn't done for two reasons:
  //   -the OS already has a gesture to do this through pinch-zoom
  //   -if a user starts an inertial scroll … and presses control …
```

⬜ **So the macOS question is a different one and is unmeasured:** does a **trackpad pinch** zoom the
browser chrome? Pinch is *page scale* (visual viewport), not page zoom, and whether it reaches a CEF
chrome browser at all is unknown. 👤 The owner's rule applies either way — *"when the user zooms in
they just want to zoom into the page; when they go into Settings to make it bigger, that should be
everything"* — so if pinch does scale the chrome on macOS, it needs its own guard and the `ctrlKey`
test will not be the right predicate.

⚠️ **The same origin trap applies on macOS and is platform-independent**: the header, all overlays
**and** the internal pages a user opens as tabs (`/newtab`, `/settings-page`, `/browser-data`,
`/wallet-panel`) are all served from `127.0.0.1:5137`, so anything that zooms one of them zooms the
chrome. That is why the guard lives in `App.tsx` and not in the header.

## ⬜ Item 7 route 2 — not started, and macOS has no equivalent yet

The header window is sized from monitor DPI only (`GetHeaderHeightPx()` → `GetDpiForWindow`), while
Chromium sizes its **content** by `monitor DPI × the Windows text-scale factor`
(`ScreenWin::GetScaleFactorForHWND`, *"including accessibility adjustments"*). The gap is what clips.
The Windows fix is to read `HKCU\Software\Microsoft\Accessibility\TextScaleFactor`.

⬜ **macOS equivalent unknown.** The analogous setting is Accessibility → Display → larger text, and
whether AppKit folds it into the backing scale the way Windows does has not been checked. Do not
assume the Windows fix ports.

## ⬜ Instruments are Windows-only

`omniboxprobe.py`, `tearoffprobe.py`, `zoomprobe.py` and `item4check.py` all read `user32` HWND state.
`profilerefreshprobe.py` and `settingsrefreshprobe.py` are pure CDP and **should run on macOS as-is**
— they are the two worth trying there first, since item 8 is shared C++.

⭐ Item 5's whole result rests on `OwnOverlayToRequestingWindow`, which is Windows-only
(`GWLP_HWNDPARENT`). macOS overlays are borderless `NSWindow`s, so **the tear-off question is
genuinely open there** and the Windows green says nothing about it.

---

# 🍎 Round 3 — item 1 (launch focus), added 2026-09-19

## 🚨 Rebuild required, and this is the biggest C++ change of the phase

| File | What changed |
|---|---|
| `src/handlers/simple_app.cpp` | Four overlay creators (wallet, profile, bookmarks, tab-list) now pass `WS_POPUP \| (showImmediately ? WS_VISIBLE : 0)` instead of always `WS_VISIBLE` |
| `cef_browser_shell.cpp` | `ShellWindowProc` gains a `WM_SETFOCUS` case that forwards the keyboard to CEF's own window — the header on a new-tab page, the tab otherwise |
| `src/core/TabManager.cpp` | `RegisterTabBrowser` re-asserts that focus when a tab arrives, because `CreateBrowser` is async |
| `src/handlers/simple_handler.cpp` + `.h` | `focus_address_bar` sent once, when the header finishes loading; new `address_bar_focused_once_` member |
| `frontend/src/pages/NewTabPage.tsx` | no longer auto-focuses its search box (React, relays with the frontend) |

## ⛔ Almost all of this is Windows-only by construction — do NOT port it literally

`WS_VISIBLE`, `WM_SETFOCUS`, `DefWindowProc`, `GetGUIThreadInfo`, `::SetFocus` and `CEFHostWindow`
have no macOS analogues. macOS overlays are borderless `NSWindow`s and focus is AppKit's
first-responder chain.

⬜ **What macOS owes is the same QUESTION, not the same patch:** launch the build, click nothing,
type — do the characters land in the address bar? 📏 On Windows the answer was no for four separate
reasons, three of them native.

⭐ **The one genuinely portable finding** is the diagnostic method, and it is worth reusing: the
instrument that settles keyboard delivery is **`OnPreKeyEvent`'s existing log line**, which names the
role that received each keystroke, in `debug_output-<pid>.log`. `document.activeElement` answers a
different question ("which element gets keys once they arrive at this browser") and produced four
consecutive false greens here.

⚠️ And the macOS equivalent of defect 1 is worth checking on its own: does creating an
`NSWindow` for a pre-warmed overlay make it key/main before it is ordered out?

---

# 🍎 Round 4 — a message TO the Mac side, 2026-09-19

## ⛔ The rule that bit Windows today, and it will bit you in the other direction

Your `6775b63` — *"delete the backup-overlay chain on macOS — and the shared shims with it"* — removed
declarations from `include/handlers/simple_handler.h`. Windows had, minutes earlier, added a member
to **that same header** (`address_bar_focused_once_`, for item 1).

`git rebase` merged the two without a conflict. ⛔ **And that means nothing**, because a textual merge
has no idea that a deleted declaration breaks a translation unit. Windows rebuilt before pushing and
it happened to still compile. Next time it will not.

⇒ **Both sides, every session: `git fetch && git rebase origin/0.4.0`, then REBUILD, then push.**
Now recorded as a standing rule in the root `CLAUDE.md`, next to the existing
*"nothing compiles C++ on push"* rule. That one is about warning **you**; this one is about not being
caught by you — and it is symmetric, so it is as much ours as yours.

⚠️ Two practical notes from doing it four times today:

- **Append-vs-append conflicts** are the normal case in the shared docs — `phase-*/README.md`,
  `HUMAN_TEST_QUEUE.md`, and these relay rounds. Both sides append at the end. **Keep both**, in
  history order. ⛔ Do not resolve by taking one side; today's conflict would have silently dropped
  your entire macOS item-9 evidence section.
- Windows is now pushing **several times a day** too. Treat `origin/0.4.0` as moving under you.

## 👍 And thank you for the item-9 trap — it generalises

> *"`SetPreference` persists, so reverting the C++ alone leaves the pref false and the control writes
> nothing."*

⭐ That is the best negative-control note either side has written this sprint, and it is not
macOS-specific. It is now in the Definition of Done for the next Windows session:
**when a fix writes a persisted preference, the control must revert the STATE as well as the CODE.**
Windows' own item-9 control got away with it only because the pref happened to be re-applied at every
launch by `OnContextInitialized`.

## Where Windows is

**10 of 11 Phase 11 items closed.** `W8` (launch-and-type) and `W9` (the omnibox with a real mouse and
keyboard) both **passed with the owner at the keyboard**. Remaining: `W10` (the DPI + text-scale
matrix) and **item 7 route 2**, which `W10` is the evidence for.

⬜ Still owed from macOS, unchanged from rounds 1–3: item 1's equivalent question (launch, click
nothing, type — do the characters land in the address bar?), item 5's tear-off sweep (our result rests
on `OwnOverlayToRequestingWindow`, which is Windows-only), and whether a trackpad **pinch** scales the
chrome, since our ctrl+wheel guard is a no-op on your platform.

---

# 🍎 Round 5 — item 7 route 2 + W10, 2026-09-19

## Rebuild required — but **Windows-only code**

| File | What changed |
|---|---|
| `include/core/LayoutHelpers.h` | new `GetTextScalePercent()` reading `HKCU\Software\Microsoft\Accessibility\TextScaleFactor`; both `GetHeaderHeightPx*()` now apply it alongside the monitor DPI |
| `cef_browser_shell.cpp` | `ShellWindowProc` gains `WM_SETTINGCHANGE`, which re-lays-out when the header's actual height differs from what it should be |

⛔ Both are inside `#ifdef _WIN32` regions or Win32-only files. **Nothing for macOS to port literally.**

## ⬜ What macOS owes: the same question, its own mechanism

📏 Windows defect: Chromium sizes a window's **content** by `monitor DPI × accessibility text scale`
(`screen_win.cc :: GetScaleFactorForDPI` returns `scale * UwpTextScaleFactor::…GetTextScaleFactor()`),
while our header **window** was sized from `GetDpiForWindow()` — monitor DPI only. At Windows text
size 125% on a 125% monitor, `devicePixelRatio` was **1.5625** and **18 CSS px** of header hid behind
the webview. 👤 Owner-observed, then measured, then fixed.

⬜ **Does the macOS header have the equivalent?** The analogous setting is
System Settings → Accessibility → Display → **larger text**, and whether AppKit folds it into the
backing scale the way Windows folds text scale into the device scale factor is **unknown**. The
macOS header height is a fixed `96` too (`headerHeight = 96`, noted in `LayoutHelpers.h`'s own
comment), so if AppKit does compound it, the same clipping exists there.

## ⭐ Two transferable traps from this round

1. **A `static` initialised inside the event handler misses the first event.** Our first
   `WM_SETTINGCHANGE` handler cached the previous factor in a function-local `static`, which
   initialises on *first call* — and the first call **is** the change. 📏 0 firings across a real
   125% → 150% change. Rewritten to compare *what the header should be* against *what it is*: no
   stored state, self-correcting. ⚠️ That also dodged a second trap — Windows writes an approximate
   integer, and the log reads **124%** for a 125% slider, so comparing factors would have been
   fragile.
2. ⛔ **The DPI matrix's own `--force-device-scale-factor` shortcut makes the mouse row VACUOUS.**
   `ClientToViewPoint` reads `GetDpiForWindow`, which that flag does not change — CEF would render
   scaled while the conversion stayed a no-op, and the test would pass without exercising anything.
   Item 6 was therefore run on a **real** 125% monitor. If macOS has an equivalent shortcut, check
   what its own conversion actually reads before trusting it.

## Where Windows is

**Phase 11 is COMPLETE — 11 of 11.** `W8`, `W9` and `W10` all passed with the owner at the keyboard.
Next is 👤 the owner's requested beta.3 item: `TaskSweepReservations`' second verdict
(`TICKET_reservation_can_be_held_indefinitely.md`).
