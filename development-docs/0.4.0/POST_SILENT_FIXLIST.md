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

## E. macOS silent update — VERIFICATION (pending this session's retest)

- First attempt did NOT auto-update (owner quit before Sparkle finished the background download —
  likely just timing). Retesting with a proper ~10–15 min wait. **If it still doesn't fire after
  a proper wait:** verify `Sparkle.framework` is actually linked/bundled in the shipped build —
  the one open unknown from the #7 mac verification. Check the runtime log for
  `Sparkle 2 controller created (deferred start)` (linked) vs `Sparkle framework not available`
  (`AutoUpdater_mac.mm`), and the Sparkle check-interval + install-on-quit flow. (macOS install
  itself works: drag-to-Applications confirmed; About showed beta.18 = beta.19 installed.)
  **→ Update this section with the retest result before the deep-dive.**

---

## Suggested deep-dive bundling

1. **Windows-10 cluster (C1–C3)** — one investigation, shared root cause, test on Win10. *(highest — real functional breakage for a real user)*
2. **Update presentation (A1, A2)** — instant splash + kill the console flash. *(highest — most-visible seam of the now-working silent feature)*
3. **Profile picker (D1 perf + D2 modal)** — one bundle; modal only if it helps perf/functional.
4. **Bookmarks (B1 first-open populate, B2 click-outside close)** — may share code with C1.
5. **macOS Sparkle verification (E)** — pending retest outcome.
