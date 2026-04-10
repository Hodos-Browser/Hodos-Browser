# Post-Beta.3 Cleanup Sprint

**Created:** 2026-04-10
**Source:** Bugs found while testing v0.3.0-beta.3 (Windows + macOS via MacinCloud)
**Status:** In progress

---

## P0 — Critical macOS Blockers

- [ ] **#9 — Closing any tab kills entire browser on macOS**
  - Repro: open browser, click X on any tab, entire window/app closes
  - Suspect: `TabManager_mac.mm` or `windowShouldClose` in `cef_browser_shell_mac.mm` (line ~1974)
  - Need to mirror Windows logic: only close window when LAST tab in window closes; on macOS, even then keep app alive (menu bar convention)
  - **Difficulty:** Medium

- [ ] **#8 — macOS opens with no first tab visible until user clicks +**
  - Repro: launch browser on Mac, browser window appears with content but tab bar is empty until + clicked
  - Suspect: `TabManager_mac.mm` initial tab creation, or NSView visibility on first paint
  - **Difficulty:** Easy-Medium

- [ ] **#7 — macOS shows Windows-style min/max/close buttons + tabs misaligned with traffic lights**
  - Three sub-issues:
    - Hide Windows-style buttons in `MainBrowserView.tsx` when running on macOS
    - Add platform detection (no `isMac` exists in frontend yet — need to add via V8 injection or user agent)
    - Shift first tab right by ~80px on macOS to avoid traffic lights
    - Also slightly shift Windows first tab over (UX polish)
  - **Difficulty:** Medium

---

## P1 — Important Bugs

- [ ] **#3 — Windows maximize covers taskbar**
  - Root cause found: `cef_browser_shell.cpp:3438` uses `WS_POPUP | WS_THICKFRAME` instead of `WS_OVERLAPPEDWINDOW`. WS_POPUP doesn't constrain maximize to monitor work area.
  - Fix options: (a) handle `WM_GETMINMAXINFO` to clamp to work area, OR (b) switch to `WS_OVERLAPPEDWINDOW`
  - **Difficulty:** Easy

- [ ] **#5 — Some sites show black background hiding text**
  - Repro: mom's computer, specific site (need URL). Loaded same site in another browser → white background.
  - Inspect element showed no background-color set
  - Suspect: CEF default canvas color differs from Chrome default. May need to inject `html { background: white }` if site doesn't specify
  - **Need:** specific URL to reproduce
  - **Difficulty:** Needs research

- [ ] **#6 — New tab content off-center on some PCs, fixes on maximize then breaks again**
  - Repro: dad's computer only — content offset to right on first paint, fixes when maximized, breaks again on close/reopen
  - Does NOT happen on user's or mom's computer
  - Suspect: DPI scaling issue, initial paint race, or window-size-restore bug
  - **Need:** more diagnostics from dad's PC (monitor DPI, resolution)
  - **Difficulty:** Needs research

- [ ] **#11 — Cloudflare "Verify you are human" fails on macOS**
  - Repro: try opening whatsonchain.com on Mac. Checkbox spins, then resets after a second
  - Works on Windows. We copied Brave's farbling but maybe macOS impl differs
  - Check: `IsAuthDomain()` in `FingerprintProtection.h` — might need to add Cloudflare challenge domains
  - Compare farbling implementation Windows vs macOS
  - **Difficulty:** Medium-Hard

---

## P2 — Polish / UX

- [ ] **#1 — Paymail validation feedback unclear**
  - In TransactionForm.tsx, after user clicks a Handcash paymail suggestion, we validate it but feedback is unclear
  - Add green checkmark when validation succeeds
  - Maybe a subtle red indicator on failure (not too aggressive)
  - **Difficulty:** Easy

- [ ] **#2 — New tab address bar shows `hodos://newtab`**
  - Other browsers show empty address bar on new tab
  - Fix in `MainBrowserView.tsx` or address bar component — clear when URL matches newtab pattern
  - **Difficulty:** Easy

- [ ] **#10 — Wallet payment notification delayed on macOS**
  - On Windows: green dot appears AND payment details ready when wallet panel opens
  - On macOS: green dot appears but payment details take a few seconds to load after panel opens
  - We fixed this on Windows. May need same fix in macOS path.
  - **Difficulty:** Needs research (compare Win vs Mac IPC timing)

---

## P3 — Admin / Process

- [ ] **#4 — License → MIT, GitHub branch protection review**
  - Change LICENSE file to MIT
  - Review GitHub settings: only owner can push/merge to main
  - Branch protection rules
  - **Difficulty:** Easy

---

## P4 — Future Sprint (Separate Context)

- [ ] **#12 — BSV Rust ecosystem evaluation**
  - Read `development-docs/BSV_RUST_ECOSYSTEM_COMPARISON.md`
  - Identify what to implement now to:
    - Keep backup token small
    - Improve handling of miner responses
  - This should be its own sprint with its own context

---

## Notes

- All P0 items must be fixed before recommending macOS to public users
- The Day 1 quick wins (#1, #2, #3, #4) can be done in a single context
- macOS items (#7, #8, #9) likely best done together since they all touch `cef_browser_shell_mac.mm` + TabManager_mac.mm
- Use rented Mac (MacinCloud / Mac Mini) to verify each macOS fix
