# macOS Phase 3 (#5 half) — fullscreen follows the window that asked. 2026-09-19.

Subject: ONE dev process, window A primary (#N, x=0, 1440x795), window B torn off via
the `tab_tearoff` IPC (x=50, 1340x697). Rig: `p3fsprobe.py`.

## ⛔ ATTRIBUTION — and the instrument that got it wrong first

Windows are identified by **`kCGWindowNumber`, captured before the action**.

📏 The first version of this probe identified them by their **x origin** and reported
**RED on the fixed build**. A window that enters fullscreen moves to **x=0** — which is
also the primary's x — so the two become indistinguishable EXACTLY in the success case.
⭐ Same family as the `ClampOverlayToScreen` trap (relay round k §4): *a discriminator
that collapses when the fix works is not a discriminator.* Twice in two rounds, so the
lesson is not about clamping or about fullscreen — it is about choosing a discriminator
the success case cannot erase.

⚠️ A window in native fullscreen gets its own Space, so the other window leaves
`kCGWindowListOptionOnScreenOnly`. "OFF-SPACE" is evidence, not an error.

## RED — pre-fix, negative control run with the FINAL probe (git stash + rebuild + re-sign)

  ROW 1  menu Fullscreen clicked in B   -> A 1440x795 -> 1440x900, B OFF-SPACE   🚨 A went fullscreen
  ROW 2  requestFullscreen() in B's tab -> A 1440x795 -> 1440x900, B unchanged    🚨 A went fullscreen

## GREEN — post-fix, same subject, same probe

  ROW 1  menu Fullscreen clicked in B   -> B 1340x697 -> 1440x900, A OFF-SPACE    ✅ B went fullscreen
  ROW 2  requestFullscreen() in B's tab -> B 1340x697 -> 1440x900, A UNCHANGED    ✅ B went fullscreen
  both   on exit each window restored to its OWN frame (A 1440x795 @x=0,
         B 1340x697 @x=50) — the per-window pre-fullscreen frame, which was one
         shared global before.

⭐ The RED and GREEN are exact mirror images, which is what makes the control decisive:
the probe demonstrably produces both answers.

## Second, INDEPENDENT discriminator — the log attributes by window id

  Native fullscreen toggle on window 1
  Native fullscreen ENTERED (window 1)      <- the delegate ADDED to secondary windows
  HandleFullscreenChange: ENTER (window 1)
  HandleFullscreenChange: EXIT  (window 1)

⭐ `Native fullscreen ENTERED (window 1)` proves the new `BrowserWindowDelegate`
fullscreen path RUNS, not merely that it exists — secondary windows had no such
delegate method at all before this change.

## Regressions — both clean on the same build

  D-h1 header : both windows innerHeight 104 / #root 104
  D-h2 sweep  : 0/8 overlays ignore the requesting window; y=134; x unchanged
  log         : 0 [ERROR] lines
