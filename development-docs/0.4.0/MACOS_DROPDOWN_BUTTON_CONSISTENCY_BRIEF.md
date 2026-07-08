# Mac brief — make all dropdown buttons behave consistently (owner request, 2026-07-08)

**For the Mac side (implement + compile + smoke on macOS — this is `.mm`/`#elif __APPLE__` code that
does NOT build on the Windows dev machine).** Owner wants all 7 macOS toolbar dropdown buttons to
behave the same. Investigation found the three "newer" buttons (bookmark / site-info / tab-list) are
already the BEST-behaved and are the reference; the outliers are **menu** and **profile** (and
**download** partially). This brief brings them to the reference pattern.

## Reference pattern (already used by cookie / bookmark / site-info / tab-list on macOS)
For a `*_panel_show` IPC handler mac branch:
```
if (!<window>)              CreateXxxOverlayMacOS(offset);      // first open
else if (IsXxxVisible())    HideXxxOverlayMacOS();              // toggle closed on button
else if (WasXxxJustHidden())/* suppress — click-outside monitor just hid it (0.3s) */;
else                        ShowXxxOverlayMacOS(offset);        // reuse (keep-alive)
```
Plus in `cef_browser_shell_mac.mm`: keep-alive lifecycle (Create once, then `Show`=`orderFront` /
`Hide`=`orderOut`; `Create` destroys any existing first), and a dedicated `NSEvent` click-outside
monitor with a 0.3s just-hidden debounce (so a button-click while open doesn't get hidden-then-reopened).
Bookmark is the cleanest template — mirror it. (Files: `cef-native/src/handlers/simple_handler.cpp`
mac branches; `cef-native/cef_browser_shell_mac.mm` create/show/hide/monitor + the `IsXxxVisible` /
`WasXxxJustHidden` helpers.)

## What to change

### 1. `profile` (closest — helpers already exist, just unused)
- IPC mac branch (`simple_handler.cpp` ~2636-2648): it already has `if (IsVisible) Hide; else Create`.
  Convert to the 4-way reference: add the `WasProfilePanelJustHidden()` suppress branch and a `Show`
  reuse branch. **`WasProfilePanelJustHidden()` is already declared in `.mm` but never called; wire it
  in.** Add/verify a `ShowProfilePanelOverlayMacOS()` reuse path (orderFront the existing panel) — if
  the `.mm` only has Create today, add a Show that reuses the existing NSWindow like bookmark does.
- Net: profile becomes keep-alive + debounced toggle, matching bookmark. Kills its current
  click-outside-then-reopen race + per-open rebuild.

### 2. `menu` (furthest — no visibility logic at all today)
- IPC mac branch (`simple_handler.cpp` ~2693-2696): today it unconditionally calls
  `CreateMenuOverlayMac(...)` every click. Convert to the 4-way reference.
- `.mm` likely needs new helpers mirroring bookmark: `IsMenuOverlayVisible()`,
  `WasMenuOverlayJustHidden()` (0.3s tick), a keep-alive `ShowMenuOverlayMacOS()` (orderFront reuse),
  and switch the click-outside close from the generic `InstallClickOutsideMonitor` (no debounce) to a
  dedicated monitor with the 0.3s debounce (copy bookmark's monitor block). Keep `Create` = build once
  (destroy existing first).

### 3. `download` (partial — toggles fine, but rebuilds every open)
- Lower priority. It has the debounce + `IsVisible→Hide` already, but the IPC path always calls
  `CreateDownloadPanelOverlayMacOS` instead of reusing via `Show`. For full consistency, switch its
  open path to the keep-alive `if(!window)Create; else if(visible)Hide; else if(justHidden)suppress;
  else Show` shape (a `ShowDownloadPanelOverlayMacOS` reuse path exists but is unused on the open path).
- Behaviorally download already toggles correctly, so this is a keep-alive/perf cleanup — do it if
  converging all 7, skip if time-boxed.

## Explicitly do NOT change
- bookmark / site-info / tab-list mac branches — they are the reference, already correct.
- The Windows blocks — Windows made these three OPEN-ONLY (`#ifdef _WIN32`) because Windows can't
  reliably toggle-closed (that desync was the Win10 dead-button bug, fixed separately as F1). Mac keeps
  toggle-to-close because Mac's window tech (NSPanel) doesn't have that bug and toggle is the nicer
  native feel. So there is an intentional platform difference in close-on-button behavior; do not
  "fix" Windows to match Mac or vice-versa. This brief is ONLY about making the MAC buttons consistent
  with EACH OTHER.

## Test (macOS)
Open each of the 7 dropdowns; click the button again while open → closes cleanly (no flicker/reopen);
click outside → closes; reopen → content present (keep-alive reuse). Verify menu + profile no longer
flicker on the click-while-open case.
