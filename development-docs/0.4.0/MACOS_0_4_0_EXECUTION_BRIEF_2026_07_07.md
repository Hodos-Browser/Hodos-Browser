# macOS Execution Brief — 0.4.0 post-silent fixlist + picker UI (2026-07-07)

**For:** Claude on the Mac. **Flow:** owner pushes `origin/0.4.0` → you pull → review this
brief → execute M1–M3 → report back (build result + Phase E cadence result + picker
screenshot). Companion tracking doc: `MACOS_PORT_0_4_0.md`.

**Context (what landed on Windows this session, local on 0.4.0):** the POST_SILENT_FIXLIST
deep-dive (A1/A2 update-UX, C1/C2 Win10 overlay hardening, B1/B2 bookmark+profile-panel
click-outside/first-open, E mac cadence) + a full **profile-picker UI redesign** (launcher
window + tiles). Commits `6112d78 … 6d358b8`. **Almost all of it is Windows-only** (`#ifdef
_WIN32` or Windows-only files), so your build should be unaffected — but confirm. Your real
work is **Phase E verify** and **picker-window mac parity**.

---

## M1 — Build verify (confirm the Windows changes didn't leak)
Pull `origin/0.4.0`, build the mac shell + frontend (`cd cef-native && ./mac_build_run.sh`,
`cd frontend && npm run build`). The following are all Windows-only and should NOT affect the
mac build — if any breaks the mac compile, it's an accidental cross-platform leak, fix/report:
- A1/A2 update splash + console fix — Windows `update-helper/` + the `#ifdef
  HODOS_SILENT_AUTOUPDATE` shell splash (`cef_browser_shell.cpp`). `splash.h` is Win32-only.
- C1/C2/B2 overlay hardening — `IsOverlayEffectivelyVisible` (DWM cloaked), `SWP_FRAMECHANGED`
  (`my_overlay_render_handler.cpp` Windows block), the WH_MOUSE_LL hook installs
  (`simple_app.cpp` Windows overlay fns), profile-panel hook — **all Windows-only.**
- **DO NOT port these to mac.** Mac overlays use `NSPanel` + `resignKey`/`WasXxxJustHidden`,
  a different close mechanism — they don't have the Win10 "works once then dead" bug and
  don't need the cloaked/hook/frame-flush fixes.

## M2 — Phase E: verify the auto-update check now fires
`cef-native/Info.plist` now has `SUScheduledCheckInterval = 10800` (3h) — committed, it's a
cross-platform file so it's already in your tree. Sparkle is confirmed linked + the manual
check + install-on-quit already work; this key was the missing piece (default was ~1 day).
Verify the AUTOMATIC path end-to-end:
1. Build + install the app. To test without waiting 3h, force the scheduled check due:
   `defaults delete com.hodosbrowser.app SULastCheckTime` (or `defaults write
   com.hodosbrowser.app SUScheduledCheckInterval -int 120` for a 2-min interval), relaunch.
2. With an older version installed and a newer on the live feed: confirm scheduled check
   fires → downloads → installs-on-quit → relaunches as the new version.
3. Confirm the mode in `~/Library/Application Support/HodosBrowser/logs/debug_output.log`
   (`Auto-updater initialized (... mode=...)`). E2 (legacy-collapse → notify) stays AS-IS.

## M3 — Picker-window mac parity (the real port) ⭐
The **React** launcher redesign (`ProfilePickerOverlayRoot.tsx`, `isPickerWindow` branch:
Hodos logo top-left, "Choose Hodos Profile", gold-bordered profile TILES paging left/right
with arrow buttons, gold-glow backdrop, top-right X that sends the existing `exit` IPC) is
**cross-platform — it renders on mac automatically.** What's missing on mac is the **window
size**: on Windows the picker-mode main window was shrunk to a small centered launcher; on
mac `g_main_window` is still full-size, so the mac launcher would fill the screen.

**Windows reference** (`cef_browser_shell.cpp`, in the `g_picker_mode` branch right after the
work-area rect is computed):
```cpp
if (g_picker_mode) {
    int pw = width * 60 / 100;  if (pw > 980) pw = 980;  if (pw > width)  pw = width;
    int ph = height * 78 / 100; if (ph > 660) ph = 660;  if (ph > height) ph = height;
    rect.left += (width - pw) / 2;  rect.top += (height - ph) / 2;
    width = pw; height = ph;   // header view fills this in picker mode
}
```
**Mac task:** in `cef_browser_shell_mac.mm`, find where `g_main_window` is created
(`NSWindow … initWithContentRect:`) and, when `g_picker_mode`, use a **small centered content
rect** mirroring the above: `pw = min(0.60*screenW, 980)`, `ph = min(0.78*screenH, 660)`,
centered on the visible frame (`[NSScreen mainScreen].visibleFrame`). The header view already
fills the whole window in picker mode (`cef_browser_shell_mac.mm:2543-2567`), so only the
window frame needs to shrink+center. Consider a non-resizable / no-zoom style so it reads as a
launcher (optional; match the feel). Verify:
- Launch with >1 profile, no `--profile` → small centered launcher, logo + tiles + glow.
- The `isMac` traffic-light left-padding (`paddingLeft: 86` in the picker page) clears the
  mac window controls; the top-right X coexists with the top-left traffic lights (fine).
- Tile click launches the profile + closes the launcher; X does a clean exit.
- >4 profiles → arrows page the tile strip.

## Report back
Build result (M1 clean?), Phase E cadence result (did the scheduled check fire + install?),
and the picker mac parity (done + a screenshot of the launcher). Note anything that needed a
mac-specific deviation.

---
**Not in scope here:** the same-process picker refactor (`PROFILE_PICKER_SAME_PROCESS_PLAN.md`,
still deferred), the Apple org-signing migration (separate gate), and the Windows-side
adversarial review of the picker-window sizing (owner/Windows-Claude).
