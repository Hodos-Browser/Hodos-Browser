# Post-Silent-Proof Fix List (owner-reported, 2026-07-06)

**Context:** The Windows silent auto-update is PROVEN end-to-end on real signed builds
(beta.19 → beta.20 auto-applied through the real public pipeline; wallet intact). This list
is the polish + bug backlog the owner surfaced *during* that proof. It is intended for a
dedicated next-session **deep-dive workflow** (task-agents to investigate → fix → optimize).

Everything below is owner-observed on real builds. Group A (update UX) and Group C (Windows-10
cluster) are the highest priority; B and D are UX polish; E is verification.

---

## A. Silent-update presentation (Windows) — mechanism works, the *look* is rough

The update WORKS; users just shouldn't see the seams. Two issues on the apply-boot:

- **A1 — the "Hodos is updating…" splash appears too SLOWLY.** On the reopen (apply-boot),
  there is a multi-second gap before the splash shows. It was long enough that the owner was
  tempted to click again, thinking nothing was happening. **Fix:** the splash must appear
  *immediately* — the very first thing on the apply-boot, before the heavy backup/install work —
  so the user gets instant feedback. Investigate where the splash is created in the apply path
  (helper `update-helper/splash.h` / `RunApplyTransaction`) and hoist it to the front.
- **A2 — a terminal/console window flashes at the end of the update.** Right as the update
  finishes, the splash closes and a console window opens, logs a few lines, and closes — visible
  to the user. **Bad look; users must never see a console.** **Fix:** find what spawns with a
  console at the tail of the apply (candidates: the helper's final relaunch, a `cmd /c` cleanup,
  the installer, or the relaunched browser inheriting a console) and make it fully windowless
  (`CREATE_NO_WINDOW` / `DETACHED_PROCESS`, or a GUI-subsystem exe). Target end-to-end feel:
  splash appears instantly → work happens under it → browser launches. No console, ever.

## B. Bookmarks (cross-platform)

- **B1 — bookmark overlay doesn't show the current page URL (with the star) on FIRST open.**
  Have to close it and reopen to get it to populate with the current page. Seen on the owner's
  machine too (not just Win10). Suspect a timing/IPC race: the overlay is shown before it has
  received/read the active tab's URL on first creation. Check whether the recent overlay
  slow-PC self-close guard (`483fc1e`) interacts with first-open population.
- **B2 — bookmark modal only closes via the X.** It should also close on click-outside
  (backdrop) or clicking elsewhere, like the other dropdown overlays. **Fix:** add the
  click-outside close (mouse-hook / backdrop handler) consistent with the other overlays.

## C. ⭐ Windows-10 "works once then dead" cluster (owner's mom's machine) — LIKELY SHARED ROOT CAUSE

All three below are the SAME shape — an overlay/button works exactly once (or not at all) and
is then dead — and all on the **mom's Windows 10** machine, **NOT reproducible on the owner's
(Win11) machine.** Strong signal of one shared Windows-10 overlay/window-lifecycle or
environment cause. **Investigate C1–C3 TOGETHER, and specifically on a real Windows 10 setup.**

- **C1 — bookmark button dead after first use.** Opens the overlay once (also without the
  URL/star, per B1), owner closes it, then the button does NOTHING on every later click — the
  overlay never reopens.
- **C2 — Site Controls button does nothing** (the other toolbar buttons work).
- **C3 — profile picker shows only on the FIRST launch**, then silently opens the default
  profile on every subsequent launch (picker never appears again).
- Owner asked if **Windows 10** is the factor — very likely yes. Candidate causes to probe: an
  overlay HWND created once and never recreated/re-shown; a `WH_MOUSE_LL` hook that isn't
  re-installed; a WS_POPUP/OSR or DWM behavior difference on Win10 vs Win11; a one-shot state
  guard that never resets. Confirm the OS version and reproduce on Win10.

## D. Profile picker — performance + UX (owner's machine)

- **D1 — slow profile launch.** Picker opens → select a profile → *everything closes* → a
  multi-second gap before the browser window appears. Looks broken. The gap is the process
  handoff: the picker spawns a new `HodosBrowser.exe --profile=…`, the picker exits, and the new
  process cold-boots. **Fix:** optimize so the selected profile launches promptly (pre-warm /
  overlap the handoff / cut cold-boot cost). Overlaps the earlier ~2s-first-paint startup work
  (`development-docs/0.4.0/STARTUP_OPTIMIZATION.md`).
- **D2 — consider a small popup modal instead of the full-window picker.** Owner's steer:
  do this ONLY if it improves feel/perf and fits the functional changes above — NOT a
  cosmetic-polish task. A lighter modal may reduce the D1 handoff cost, which is the real reason
  to consider it. Don't over-invest in looks right now.

## E. macOS auto-update — Sparkle works; the AUTOMATIC scheduled check doesn't fire promptly

**Updated diagnosis (2026-07-06, after owner retest):** Sparkle is linked + working — manual
"Check for updates" sees beta.20 on beta.19. But NEITHER Automatic (silent) NOR Notify fired on
their own within a ~15-min window, even after switching to Automatic. Key realization: the
"Automatic vs Notify" setting controls *what happens when an update is found* (download silently
vs prompt), NOT *how often Sparkle checks*. The check FREQUENCY is `SUScheduledCheckInterval`,
which our `Info.plist` does NOT set → Sparkle uses its default (~1 day). So the automatic check
simply wasn't due yet in the test window — for either mode. **Likely not a regression and not
broken** (mac auto-update was never verified before; we're discovering the default cadence).

**Fixes / investigation:**
1. **Set `SUScheduledCheckInterval` in `cef-native/Info.plist`** to something sane (Chrome-like,
   e.g. a few hours = 3600–14400s), so updates land promptly in production AND are testable in
   minutes. Verify the full automatic path E2E: launch → scheduled check fires → background
   download → install-on-quit → relaunch is the new version.
2. ✅ **Install PATH CONFIRMED (2026-07-06):** owner ran manual "Check for updates → install" and
   it downloaded, installed, and relaunched as **beta.20**. So the ENTIRE mac update path
   (feed → download → Ed25519 verify → install-on-quit → relaunch) works. The ONLY remaining mac
   gap is the automatic check CADENCE (fix #1 below). This item is now small + well-scoped.
3. Consider a **check-on-launch / shortened first-run interval** so a freshly-installed mac
   picks up a waiting update quickly rather than after a full interval.
4. Get the mac log if needed: `~/Library/Application Support/HodosBrowser/logs/debug_output.log`
   → `Auto-updater initialized (... mode=...)` confirms the mode; Sparkle's own scheduling logs
   go to Console.app / unified logging.

## E2. macOS update MODE — the legacy-collapse design question (secondary)

- **RESOLVED unknown:** Sparkle IS linked + working on mac. On the retest, "Check for updates"
  correctly detected beta.20 and knew the app was on beta.19. So the #7 "is Sparkle linked?"
  unknown is answered — yes. macOS install also confirmed (drag-to-Applications; About beta.18 =
  beta.19 installed).
- **Why silent didn't fire:** the Mac is almost certainly in **"Notify me"** mode, not
  "Automatic." Manual check finds the update but nothing auto-installs-on-quit. **Likely cause:**
  the Mac had an old **beta.8** install; on beta.19's first launch the one-time
  MOST-CONSERVATIVE cross-profile collapse (#1/#7 "never surprise-silent") found the legacy
  profile's notify/legacy-bool setting and set the global mode to **notify**. The owner's Windows
  machine had no such legacy setting, so it stayed silent and updated on its own. Setting the Mac
  to "Automatic" should make it silently update (retest pending owner confirmation of the mode).
- **⭐ DESIGN QUESTION for the deep-dive (this is the real item, not a bug):** the conservative
  collapse means EVERY machine upgrading from an older/legacy install lands on **notify**, and the
  user must manually opt into "Automatic" to get silent auto-update. That is the *safe* behavior
  we deliberately chose, but it works against the owner's goal of "everyone's apps just update."
  Decide: is legacy-upgraders→notify the right call, or too conservative? Options to weigh — keep
  as-is (safe, users opt in); OR only collapse to notify when there's an EXPLICIT notify/off
  choice (treat a legacy `autoUpdateEnabled=true` as silent-eligible rather than notify); OR a
  one-time in-app nudge inviting existing users to turn on Automatic. Whatever is chosen must not
  re-introduce a surprise-silent path ([[feedback_update_stability_principle]]). Confirm the mode
  hypothesis first (owner: Settings → Software updates dropdown; or the mac
  `~/Library/Application Support/HodosBrowser/logs/debug_output.log` line
  `Auto-updater initialized (... mode=...)`).

---

## Suggested deep-dive bundling

1. **Windows-10 cluster (C1–C3)** — one investigation, shared root cause, test on Win10. *(highest — real functional breakage for a real user)*
2. **Update presentation (A1, A2)** — instant splash + kill the console flash. *(highest — most-visible seam of the now-working silent feature)*
3. **Profile picker (D1 perf + D2 modal)** — one bundle; modal only if it helps perf/functional.
4. **Bookmarks (B1 first-open populate, B2 click-outside close)** — may share code with C1.
5. **macOS Sparkle verification (E)** — pending retest outcome.
