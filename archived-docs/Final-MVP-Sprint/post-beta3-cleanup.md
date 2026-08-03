# Post-Beta.3 Cleanup Sprint

**Created:** 2026-04-10
**Source:** Bugs found while testing v0.3.0-beta.3 (Windows + macOS via MacinCloud)
**Status:** In progress

---

## Phase 0 — macOS Baseline Testing (DO THIS FIRST)

**Goal:** Before fixing P0 bugs, establish what actually works on macOS and surface any additional bugs we don't know about. Many of the P0 fixes touch multi-window/lifecycle code — we need to know the baseline before changing things.

**Environment:** Rented MacinCloud Mac with v0.3.0-beta.3 installed from the signed DMG.

### Tools: Process Monitoring

macOS equivalent of Windows Task Manager:

- **Activity Monitor** (Applications → Utilities) — GUI, CPU/Memory/Energy tabs
- **Terminal commands:**
  ```bash
  # List all Hodos processes
  ps -ef | grep -i hodos | grep -v grep

  # Watch in real-time
  watch -n 1 'ps -ef | grep -i hodos | grep -v grep'

  # Check wallet/adblock ports
  lsof -i :31301   # wallet
  lsof -i :31302   # adblock

  # Count CEF Helper processes
  pgrep -f "HodosBrowser Helper" | wc -l
  ```

Expected process set on normal startup:
- 1× `HodosBrowserShell` (main browser)
- N× `HodosBrowser Helper` / `HodosBrowser Helper (GPU/Plugin/Renderer)` (CEF subprocesses)
- 1× `hodos-wallet` (Rust, port 31301)
- 1× `hodos-adblock` (Rust, port 31302)

### Test Phases (run in order)

#### Phase A — Startup Baseline (5 min)
| # | Test | Expected | Result |
|---|------|----------|--------|
| A1 | Launch browser from Applications | HodosBrowserShell + Helpers + hodos-wallet + hodos-adblock all spawn | |
| A2 | Count Helper processes after startup | Record baseline number | |
| A3 | `lsof -i :31301` | Shows `hodos-wallet` listening | |
| A4 | `lsof -i :31302` | Shows `hodos-adblock` listening | |

#### Phase D — Shutdown (run BEFORE tab/window tests so you don't leave zombies)
| # | Test | Expected | Result |
|---|------|----------|--------|
| D1 | Cmd+Q from menu bar | All Hodos processes exit within 2s (wallet, adblock, shell, helpers) | |
| D2 | `ps -ef \| grep hodos` after D1 | No Hodos processes remain | |
| D3 | Launch again immediately | No "port in use" errors on 31301/31302 | |
| D4 | Force kill HodosBrowserShell in Activity Monitor | Check if wallet + adblock become orphaned | |
| D5 | After D4, `ps -ef \| grep hodos` | Document what's orphaned; kill manually if so | |

#### Phase B — Single-Window Tab Lifecycle (10 min)
| # | Test | Expected | Result | Related Issue |
|---|------|----------|--------|---------------|
| B1 | Open 3 new tabs (Cmd+T × 3) | +3 Helper processes, tab bar shows 4 tabs | | |
| B2 | Close middle tab (click X) | -1 Helper, other 3 tabs remain | | **#9** |
| B3 | Close another tab | Tab closes, others remain | | **#9** |
| B4 | Close the last tab | On macOS: window closes but app stays in menu bar; on Windows: app exits | | **#9** |
| B5 | Drag tab left/right to reorder | Tabs visually swap | | |
| B6 | Cmd+W shortcut | Closes current tab | | |

#### Phase C — Multi-Window (15 min)
| # | Test | Expected | Result |
|---|------|----------|--------|
| C1 | Cmd+N for 2nd window | New HodosBrowserShell window, +Helpers, tabs independent per window | |
| C2 | Open 2 tabs in each window | Each window has own tab bar | |
| C3 | Tear off tab from window 1 to empty desktop | Becomes 3rd window | |
| C4 | Drag tab from window 3 onto window 2's tab bar | Merges into window 2 | |
| C5 | Close window 2 (red traffic light) | Window 2's Helpers exit, other windows unaffected | |
| C6 | Close all remaining windows | On macOS: app stays in menu bar; on Windows: app exits | |
| C7 | From menu-bar-only state, Cmd+N | New window appears | |

#### Phase E — Profile Isolation (10 min, optional)
| # | Test | Expected | Result |
|---|------|----------|--------|
| E1 | Open profile picker, create new profile | New separate instance opens | |
| E2 | Check `~/Library/Application Support/HodosBrowser/` | Separate Profile_1 folder exists | |
| E3 | Set different wallet in each profile | Wallets are independent | |
| E4 | Close one profile | Other profile unaffected | |
| E5 | Close all profiles | All Hodos processes exit | |

### How to Record Results

For any FAIL or surprising behavior, add a new entry to the bug list at the bottom of this doc with:
- Phase/Test ID (e.g. "Phase B, test B2")
- What happened vs what was expected
- Process list before/after (screenshot of Activity Monitor or `ps -ef` output)
- Steps to reproduce

---

## P0 — Critical macOS Blockers

- [ ] **#9 — Closing any tab kills entire browser on macOS** ⏸ DIAGNOSTIC PARKED 2026-04-11
  - Repro: open browser, click X on any tab, entire window/app closes
  - **Initial suspect (2026-04-10):** `TabManager_mac.mm` or `windowShouldClose:` in `cef_browser_shell_mac.mm:1974` — only the *last*-tab path explicitly calls `ShutdownApplication()`, but Phase 0 confirmed *any*-tab close kills the app, so the simple last-tab hypothesis is incomplete.
  - **Refined hypothesis (2026-04-11, see `~/.claude/plans/graceful-forging-nygaard.md`):** the legacy standalone `"webview"` CEF browser created at `cef_browser_shell_mac.mm:4000-4029` (instead of via `TabManager::CreateTab`) coexists with managed TabManager tabs and creates a lifecycle invariant violation. When a managed tab closes, something in the cascade reaches `windowShouldClose:` → `ShutdownApplication()`. This same legacy browser is the direct cause of Bug #8 (no initial tab visible).
  - **Diagnostic A1 (PARKED, awaiting real Mac 2026-04-11/12):** commit `eb65d30` adds `[NSThread callStackSymbols]` logging to the top of both `windowShouldClose:` delegates (`MainWindowDelegate` and `BrowserWindowDelegate`). CI tag `v0.3.1-diag.1` built a signed/notarized DMG (draft release at https://github.com/Hodos-Browser/Hodos-Browser/releases). When real Mac is in hand: download DMG → run → click `+` 3-4 times → click X on a middle tab → `grep "DIAG-A1" "$HOME/Library/Application Support/HodosBrowser/debug_output.log"` → paste the trace into chat. **Cleanup after**: revert `eb65d30` (callStackSymbols too expensive to leave in), delete draft release, `git push release :refs/tags/v0.3.1-diag.1`.
  - **Planned fix (gated on stack trace from A1):** Phase A2+A3+B1 from `graceful-forging-nygaard.md` — remove `ShutdownApplication()` from delegate close paths, delete the legacy `"webview"` browser, replace with a `TabManager::CreateTab()` seed call. Together these resolve #8 + #9. Phases C/D/E in the same plan additionally cover #11 (Windows profile-in-use race) and the macOS menu-bar-stays-alive convention.
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

- [x] **#3 — Windows maximize covers taskbar** ✓ DONE (commit on post-beta3-cleanup branch)
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

- [x] **#1 — Paymail validation feedback unclear** ✓ DONE (real validity check + owner name warning)
  - In TransactionForm.tsx, after user clicks a Handcash paymail suggestion, we validate it but feedback is unclear
  - Add green checkmark when validation succeeds
  - Maybe a subtle red indicator on failure (not too aggressive)
  - **Difficulty:** Easy

- [x] **#2 — New tab address bar shows `hodos://newtab`** ✓ DONE
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

- [x] **#4 — License → MIT** ✓ DONE (with trademark notice + service fee disclosure). **Still TODO manually**: GitHub branch protection on main + staging.
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

- [ ] **#13 — Cookie consent banner blocking (Brave parity)**
  - Sites still show "Accept / Reject / Necessary only" cookie consent popups
  - Brave hides these via the "Cookie Notices" filter list (sourced from EasyList Cookie / I-don't-care-about-cookies)
  - We don't ship that list — adblock blocks tracker cookies but doesn't hide the consent UI
  - Fix: add the filter-list URL to `adblock-engine/src/engine.rs` filter-list array, bump engine version
  - Validate on common offenders: nytimes.com, theguardian.com, most EU sites

---

## Notes

- All P0 items must be fixed before recommending macOS to public users
- The Day 1 quick wins (#1, #2, #3, #4) can be done in a single context
- macOS items (#7, #8, #9) likely best done together since they all touch `cef_browser_shell_mac.mm` + TabManager_mac.mm
- Use rented Mac (MacinCloud / Mac Mini) to verify each macOS fix

---

## Bugs Found During Phase 0 Testing

*Add new bugs here as they're discovered. Use format:*

```
### #N — Short title
- **Phase/Test:** (e.g. Phase B, test B2)
- **Expected:** ...
- **Actual:** ...
- **Process state:** (any orphaned/missing processes)
- **Repro steps:** 1. ... 2. ... 3. ...
- **Severity:** P0 / P1 / P2
```

---

## Phase 0 Test Run — 2026-04-10 (MacinCloud)

**Tester:** Project owner (first time on macOS)
**Build under test:** v0.3.0-beta.3 signed DMG, run from `~/Desktop` (no admin = no `/Applications` install). Confirmed running from Desktop launches fine.
**Environment caveats:**
- MacinCloud remote was slow and laggy throughout — visual feedback often delayed several seconds, made it hard to tell whether a click did nothing or just hadn't rendered yet.
- **Cmd modifier broken in remote.** Logitech keyboard with `cmd | alt` key + the laptop's built-in keyboard both produced Option-key behavior on the Mac (e.g. Cmd+V → `√`, which is Option+V on macOS). Cause is the remote desktop client mapping Cmd → Option, not anything Hodos-related. **Result: every keyboard-shortcut test (Cmd+Q, Cmd+T, Cmd+W, Cmd+N) is untested.** All shortcut tests should be re-run on a real Mac (purchase planned for this weekend).
- Activity Monitor's process search misbehaved on the remote — clicking a filtered process row dismissed the search instead of selecting. Force-kill testing (D4/D5) was abandoned for this reason.

### Phase A — Startup Baseline ✓ PASS
| Test | Result | Notes |
|---|---|---|
| A1 launch from Desktop | ✓ | window appeared, all 4 process types spawned |
| A2 baseline Helper count | ✓ | **5 Helpers** at clean idle startup (1 main shell + 5 helpers + wallet + adblock) |
| A3 wallet running | ✓ | confirmed via `pgrep -f hodos-wallet` |
| A4 adblock running | ✓ | confirmed via `pgrep -f hodos-adblock` |

### Phase D — Shutdown ✓ partial PASS (clean shutdown only)
| Test | Result | Notes |
|---|---|---|
| D1 Cmd+Q | ⊘ untested | Cmd modifier broken on remote |
| D1 (menu) Quit via HodosBrowser → Quit HodosBrowser | ✓ | window closed, all processes exited within ~2s |
| D2 verify zero leftover | ✓ | wallet/adblock pgrep blank, Helper count 0 |
| D3 immediate relaunch | ✓ | wallet+adblock came back, `example.com` loaded successfully, no port-in-use errors |
| D4 force-kill via Activity Monitor | ⊘ untested | Activity Monitor unusable on remote — see env caveats |
| D5 orphan check after force-kill | ⊘ untested | depends on D4 |

### Phase B — Single-Window Tab Lifecycle (multiple bugs CONFIRMED)
| Test | Result | Notes |
|---|---|---|
| B-pre launch state | **#8 CONFIRMED** | Browser window opens but tab bar is empty until user clicks `+`. No initial tab is created automatically. |
| B1 open 3 new tabs via `+` button | ✓ | Clicking `+` 3 times produced 4 tabs total (1 initial after manual `+`, plus 3 more) |
| B2 close middle tab via X | **#9 CONFIRMED** | Clicking the X on **any** tab — middle, edge, or otherwise — closes the entire browser window, not just that tab. The whole app appears to terminate, not just the window. (Was unable to verify process state after due to remote lag, but window/visual loss is total.) |
| B3 close another tab | n/a — browser already gone after B2 | |
| B4 close last tab | n/a — same | |
| B5 drag tab left/right to reorder | ✓ | tab reorder works visually |
| B6 Cmd+W | ⊘ untested | Cmd modifier broken |

### Phase C — Multi-Window (partial, observation-quality only)
| Test | Result | Notes |
|---|---|---|
| C1 Cmd+N for 2nd window | ⊘ untested | Cmd modifier broken; menu-bar-driven version not exercised |
| C3 tear off tab to new window | ✓ | tear-off worked — dragging a tab away spawned a new window |
| C4 drag tab from one window onto another's tab bar | ✓ probable | merge appeared to work but remote lag made confirmation hard |
| C2/C5/C6/C7 | ⊘ untested | abandoned due to remote slowness |

### Phase E — Profile Isolation
**Not run** — abandoned due to remote slowness. Defer to real-Mac follow-up.

### Summary of confirmed findings vs known bug list

- **#8 — no initial tab on launch** → CONFIRMED in beta.3 on macOS. Repro: launch app, observe empty tab bar.
- **#9 — closing any tab kills entire browser** → CONFIRMED in beta.3 on macOS. Severity feels even worse than the original report — the entire app appears to terminate, not just the window. Repro: launch, click `+` to create a tab, click X on that tab → window vanishes. Also reproduces when there are multiple tabs (closing any of them kills everything).
- **Tear-off works** — useful positive data point, since the tear-off code path is in the same area as #9.
- **Tab reorder works** — same.
- **Clean Quit (via menu) works and is symmetric with relaunch** — wallet/adblock spawn and shut down cleanly, ports recycle.

### What still needs testing (defer to real Mac)

- Cmd+Q, Cmd+T, Cmd+W, Cmd+N keyboard shortcuts
- Force-kill orphan behavior (D4/D5)
- Multi-window via menu bar (Phase C — most tests)
- Profile isolation (Phase E entirely)
- Whether closing the last tab on macOS keeps the app alive in the menu bar (Mac convention) — couldn't even reach this test because #9 fires first

### Recommendation for fix session

Bug #9 is the unblock — fix it first, then most of Phase B/C becomes testable. Once #9 is fixed, immediately also test Cmd+W and last-tab-closes-but-app-stays-alive behavior (the macOS menu-bar convention), since the fix for #9 is likely the same code path.

---

## Session 2026-04-11 — A1 instrumentation parked, MacinCloud abandoned

**Tester:** Project owner
**Goal:** Add diagnostic instrumentation to confirm Bug #9's exact trigger before committing to a fix.

### What was done

1. **Designed full lifecycle redesign plan** at `~/.claude/plans/graceful-forging-nygaard.md` covering bugs #8, #9, #11, #12 as four facets of one underlying lifecycle issue (tabs ≠ windows ≠ session ≠ app process). 5 phases A→E. Phase A1 = diagnostic instrumentation (this session's work). Phases A2/A3/B1 = the actual #8/#9 fix. Phases C/D/E cover #11 + macOS menu-bar convention + graceful server shutdown.
2. **Designed Phase A1 specifically** at `~/.claude/plans/enumerated-brewing-duckling.md` — narrow plan covering only the diagnostic + verification + revert.
3. **Implemented Phase A1** as commit `eb65d30` "WIP: A1 diagnostic instrumentation for macOS Bug #9":
   - `cef-native/cef_browser_shell_mac.mm:1974` — added `[NSThread callStackSymbols]` log at top of `MainWindowDelegate::windowShouldClose:`. Uses `LOG_INFO` macro (already in file). Hardcoded window id `0` because `MainWindowDelegate` (line 1891) has no `window_id` ivar.
   - `cef-native/src/core/WindowManager_mac.mm:70` — added same logging to `BrowserWindowDelegate::windowShouldClose:`. Uses file-local `LOG_INFO_WM` macro (line 15) and `self.window_id` (which exists on this delegate).
   - Both lines tagged `🔍 [DIAG-A1]` for grep-able revert.
4. **Windows build sanity check** — ran `cmake --build build --config Release` from `cef-native/`. Clean build, confirms working tree didn't break Windows (the `.mm` files don't compile on Windows anyway, but the check verifies no other accidental edits).
5. **Pushed branch + tag to release repo** for CI build:
   - `git push release post-beta3-cleanup`
   - `git tag v0.3.1-diag.1 && git push release v0.3.1-diag.1`
   - CI run `24269773284` — Windows + macOS build, ~22 min. Resulting DMG sits as draft release (visible only to maintainers) at https://github.com/Hodos-Browser/Hodos-Browser/releases.

### What was NOT done (and why)

- **Did not run the diagnostic on a Mac.** MacinCloud was procured for this purpose but proved unusably slow even via the native Microsoft Remote Desktop client (browser-based was worse). Tried to navigate to the GitHub releases page on the remote and saw multi-second lag per click. Subscription cancelled the same day. Real Mac purchase planned 2026-04-11/12 to continue.
- **Did not revert the diagnostic commit.** It must stay on the branch until we capture the trace. After we do, `git revert eb65d30` (or equivalent) before any release build. The `[DIAG-A1]` tag makes the lines grep-able if a manual revert is needed.

### What MUST be tested on the real Mac (in this order)

1. **Resume Phase A1 — capture the Bug #9 stack trace.**
   - Download `HodosBrowser-0.3.1-diag.1.dmg` from the draft release at https://github.com/Hodos-Browser/Hodos-Browser/releases
   - Mount, drag `HodosBrowser.app` to Desktop, launch
   - Reproduce: click `+` to spawn 3-4 tabs, then click X on a middle tab → app dies
   - In Terminal: `grep "DIAG-A1" "$HOME/Library/Application Support/HodosBrowser/debug_output.log"`
   - Capture full output (will include AppKit/CEF frame names — those tell us the cascade)
2. **Interpret the trace** to confirm or refute the Phase A hypothesis (legacy `"webview"` browser is the structural cause). Three possible outcomes documented in `~/.claude/plans/enumerated-brewing-duckling.md` Step 4.
3. **Cleanup the diagnostic** before doing any other work:
   - `git revert eb65d30` (or manually remove the two `[DIAG-A1]` blocks)
   - Delete the draft release `v0.3.1-diag.1` on GitHub
   - `git push release :refs/tags/v0.3.1-diag.1` to remove the tag
4. **Then implement the fix** based on the trace finding:
   - If hypothesis confirmed: apply A2+A3+B1 from `graceful-forging-nygaard.md` — removes `ShutdownApplication()` from delegate close paths, deletes legacy `"webview"` browser, replaces with `TabManager::CreateTab()` seed call. Resolves #8 + #9 together.
   - If hypothesis refuted: pivot to whatever the trace actually points at, possibly revisiting `TabManager_mac.mm` close path.
5. **Re-test Phase 0 entries that were untestable on MacinCloud:**
   - All Cmd-key keyboard shortcuts (Cmd+Q, Cmd+T, Cmd+W, Cmd+N) — MacinCloud's RDP client mapped Cmd→Option, blocking every shortcut test
   - Phase D4/D5 force-kill orphan behavior — Activity Monitor unusable on remote
   - Phase C multi-window paths — abandoned due to remote lag
   - Phase E profile isolation — entirely skipped on remote
6. **Then proceed with Phase B (`#8` fix), Phase C (`#11`), Phase D (menu-bar convention), Phase E (graceful server shutdown)** in order, each independently testable per the plan.

### Critical state to preserve across the parking period

- **DO NOT release from the `post-beta3-cleanup` branch** until commit `eb65d30` is reverted. `callStackSymbols` is expensive and would spam the log on every window close.
- **DO NOT delete the draft release** at `v0.3.1-diag.1` until after the trace is captured — that DMG is the build the diagnostic test will use.
- **DO NOT amend or rebase commit `eb65d30`** — it needs to be a clean revert target.
- The plan files at `~/.claude/plans/enumerated-brewing-duckling.md` and `~/.claude/plans/graceful-forging-nygaard.md` are the source of truth for the fix sequence and contain detailed file:line references and rollback notes.
