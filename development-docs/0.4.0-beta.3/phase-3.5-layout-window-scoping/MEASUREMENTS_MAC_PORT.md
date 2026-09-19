# D-h2 — macOS Phase 3.5 overlay-follows-window. Measured 2026-09-19.

Subject: ONE dev process, window A primary (x=0, 1440x795), window B torn off
via the `tab_tearoff` IPC (x=50, 1340x697). B is FULLY ON SCREEN — see the
clamp trap below. Frames read with CGWindowListCopyWindowInfo (no Accessibility).
Each dropdown driven over CDP from EACH window's own header, attributed by frame.

## RED — pre-fix (`88e6592`), B at x=600

  menu       fromA=(1160,126)  fromB=(1160,126)  SAME
  profile    fromA=(1060,126)  fromB=(1060,126)  SAME
  download   fromA=(1040,126)  fromB=(1040,126)  SAME
  cookie     fromA=(1040,126)  fromB=(1040,126)  SAME
  bookmarks  fromA=(   0,126)  fromB=(   0,126)  SAME
  siteinfo   fromA=(   0,126)  fromB=(   0,126)  SAME
  tablist    fromA=(1100,126)  fromB=(1100,126)  SAME
  omnibox    fromA=( 223,126)  fromB=( 223,126)  SAME
  => 8/8 ignored the requesting window. Reproduces relay round f §3 exactly
     (menu 1160 = A.right-280, omnibox 223).

## GREEN — post-fix, B at x=50 (A/B edges differ by 50 pt, nothing clamps)

  menu       fromA=(1160,126)  fromB=(1110,126)  MOVED -50   = B.right(1390)-280
  profile    fromA=(1060,126)  fromB=(1010,126)  MOVED -50   = 1390-380
  download   fromA=(1040,126)  fromB=( 990,126)  MOVED -50   = 1390-400
  cookie     fromA=(1040,126)  fromB=( 990,126)  MOVED -50   = 1390-400
  bookmarks  fromA=(   0,126)  fromB=(  50,126)  MOVED +50   = B.left
  siteinfo   fromA=(   0,126)  fromB=(  50,126)  MOVED +50   = B.left
  tablist    fromA=(1100,126)  fromB=(1050,126)  MOVED -50   = 1390-340
  omnibox    fromA=( 223,126)  fromB=( 258,126)  MOVED +35   = 50+(1340-924)/2
  => 0/8 ignored the requesting window. Every value equals the predicted
     B-anchored arithmetic, not merely "different from A".

## NEGATIVE CONTROL — pre-fix binary (git stash + rebuild + re-sign), SAME B at x=50

  All eight rows read SAME, i.e. A-anchored, on the identical subject that the
  fixed build measured as 8/8 following. 8/8 ignored the requesting window.
  ⇒ The GREEN above is attributable to the change and to nothing else about the
    subject, the rig or the window placement.

## SINGLE-WINDOW REGRESSION (free, and it is the control on over-conversion)
  Every `fromA` value is BYTE-IDENTICAL pre-fix and post-fix — 1160 / 1060 /
  1040 / 1040 / 0 / 0 / 1100 / 223. An overlay opened from the primary lands
  exactly where it always did, which is what `OverlayHostWindow(nullptr)`
  falling back to `g_main_window` is supposed to guarantee.

## Z-ORDER half — post-fix GREEN on FIRST CREATION, 3/3

  menu     A.z 1->2  B.z 0->1   B STAYS IN FRONT
  cookie   A.z 1->2  B.z 0->1   B STAYS IN FRONT
  omnibox  A.z 1->2  B.z 0->1   B STAYS IN FRONT
  (Both indices shift by one because the overlay itself enters the list at z=0;
   the measurement is the RELATIVE order, B before A.)
  ⚠️ ONE-SIDED THIS ROUND. The RED half is relay round f §3's pre-fix
  measurement — "on the FIRST creation of menu / cookie / bookmarks / omnibox
  from B, window A came in front of B" — taken on the old build at B.x=600 and
  NOT re-run here. Round f also measured that 24 re-opens afterwards moved
  nothing, so the row is only meaningful on a fresh process.

## ⬜ CLOSE SAFETY NET (`ReleaseOverlaysOwnedByMac`) — NOT MEASURED. CODE_READING only.

⛔ And the first attempt produced a FALSE GREEN that had to be thrown away.
The script opened the cookie panel from B, "clicked" B's close button, then
re-opened the panel from A and printed "GREEN — overlay survived B's close".
📏 It had not: the same sample showed `shell windows remaining: 2`. B never
closed, so the assertion could not fail.

CAUSE, verified rather than guessed: `AXIsProcessTrusted()` returns **False**
for this process, so synthetic `CGEventPost` events aimed at another
application are dropped. A coordinate sweep of 8 candidate points on B's
traffic-light confirmed it — none closed the window.
⛔ `window_close` is no help either: that IPC arm is WINDOWS-ONLY
(`simple_handler.cpp` — `HWND` + `PostMessage`, no `__APPLE__` branch), which
is itself worth reporting.
⇒ Needs either an Accessibility grant for Terminal or a human clicking the
   close button. Queued as a human row, NOT claimed as passing.

## ⛔ THE INSTRUMENT TRAP, measured before it could fake a pass
A first GREEN run used B torn off at x=600. Window B is 1340 wide, so its right
edge was 1940 on a 1440-wide screen, and `ClampOverlayToScreen` rewrites any
overflowing overlay to `maxX - w` = 1440 - w. Window A spans the whole screen
(x=0, w=1440), so an A-anchored overlay computes 1440 - w TOO. The clamp and the
defect produce the SAME NUMBER, and all five right-anchored overlays read "SAME"
on a build where the fix was working. Verified arithmetically against
`OverlayHelpers_mac.mm :: ClampOverlayToScreen` before re-placing the window.
⭐ The left-anchored pair (bookmarks, siteinfo) is what exposed it: they moved
while the right-anchored ones did not, which no single explanation covers.
