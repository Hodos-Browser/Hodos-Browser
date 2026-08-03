# AI Handoff Log

**Purpose**: Shared communication between AI assistants across sessions and developers. Every AI assistant should read this at session start and append an entry at session end.

**Format**: Dated entries, most recent at top. Keep entries concise.

---

## Entry Format

```
### YYYY-MM-DD — [Dev Name / AI] — [Brief Title]

**What was done:**
- Bullet points of completed work

**What's blocked:**
- Any blockers or open questions

**What's next:**
- Recommended next steps

**Files changed:**
- List of modified files (helps avoid merge conflicts)
```

---

## Log

### 2026-04-15 — Matt / Claude (Mac) — Turnstile fix, wallet failed-tx rollback hardening, mac tab-bar inset

**What was done:**

1. **Cloudflare Turnstile infinite-loop on whatsonchain.com (macOS) — fixed.** Root cause was a block of "dev-only" Chromium flags that shipped unconditionally in every macOS build (`--disable-web-security`, `--in-process-gpu`, `--disable-gpu-sandbox`, `--allow-running-insecure-content`). Turnstile detected them and refused to complete. Gated the whole block behind `HODOS_MAC_DEV_FLAGS=1` env var so production macOS builds ship a clean Chromium config. `--allow-loopback-in-sandbox` and `--use-mock-keychain` remain unconditional (needed for localhost backends and unsigned-app Keychain bypass; neither is web-visible). Also kept two prior partial mitigations as belt-and-suspenders: `whatsonchain.com` in `FingerprintProtection::IsAuthDomain`, and `OnBeforeBrowse` adblock-scriptlet pre-cache now runs cross-platform (removed Windows-only `#ifdef`). Verified fixed end-to-end on a real tx detail page.

2. **Wallet failed-tx rollback: three-oracle quorum + 5-min threshold.** We hit a real data-integrity bug: a backup tx got killed mid-broadcast (process exit during `/shutdown`), leaving the tx row in `status='unproven'`, inputs reserved with `spent_by=<failed tx id>`, and three ghost change outputs marked `spendable=1`. The DB balance was wrong, the real inputs were unreachable, and `TaskCheckForProofs` wouldn't roll back because ARC (GorillaPool) happened to be mid-outage returning 502, and the existing code only fired rollback on an authoritative ARC 404.

   **Root cause:** ARC was treated as the primary oracle for "does this tx exist," with WoC only as a proof-fetch fallback. When ARC was down, failure detection stalled entirely — even though WoC is equally authoritative for txid-based presence checks.

   **Fix:** Replaced the ARC-404-gated rollback in `task_check_for_proofs.rs` with a three-oracle quorum using independent keyless txid endpoints:
    - WhatsOnChain `/tx/hash/{txid}`
    - JungleBus (GorillaPool non-ARC) `/v1/transaction/get/{txid}`
    - Bitails `/tx/{txid}`

   Per tick, after ARC fails (any error), we query all three in parallel. Any 200 → tx alive, leave alone. All three 404 + age > 5m → `mark_failed` (delete ghosts, restore inputs, invalidate balance cache). Any 5xx/timeout with no 200 → Inconclusive, skip tick. `TaskUnFail`'s existing 6h recovery window catches false positives.

   Verified end-to-end with a synthetic `unproven` row (all-zeros txid, backdated created_at). Monitor tick logged the full path: "ARC unavailable → running oracle quorum → 🧹 404 on WoC, JungleBus, and Bitails after 7m — rolling back → ✅ Failed tx cleaned up." DB row transitioned to `status='failed'`.

   **Manual DB surgery was required to unblock Matt's live wallet today** (before the oracle quorum shipped). Backup at `~/Library/Application Support/HodosBrowser/wallet/wallet.db.backup_20260415_132538` — can be deleted once this is confirmed stable.

   **Known architectural limitation still open:** `/shutdown` handler (`handlers.rs:187-220`) calls `do_onchain_backup().await` synchronously. If CEF's wait-for-shutdown timer expires before the 3-stage broadcast chain (ARC → GorillaPool mAPI → WoC `/tx/raw`) completes, the process is force-killed mid-broadcast and we get exactly the stuck-tx state we just taught Monitor to clean up. The oracle-quorum rollback heals this on next startup within ~6 min, but we should still think about shortening the broadcast retry budget during shutdown or doing the backup outside the shutdown critical path.

3. **macOS tab-bar top inset.** After the recent FullSizeContentView work, tabs visually kissed the window's top edge next to the traffic lights on mac. Added `paddingTop: isMac ? '4px' : 0` and bumped the header `height` to 46 on mac (42 elsewhere) in `TabBar.tsx`. Windows unaffected (guarded by `hodosBrowser.platform === 'macos'`).

**What's blocked:**
- Nothing immediate.

**What's next — INSTRUCTIONS FOR WINDOWS CLAUDE before we build/release a new version:**

Matt wants a Windows AI pass to sanity-check two known/suspected issues before tagging a release:

1. **Background color** — there is a Windows-side rendering issue around background color that hasn't been fully characterized yet. Investigate: what's the actual bug, is it cross-platform or Windows-only, and is the fix localized to React or does it need C++ changes? Likely candidates: `MainBrowserView.tsx`, main window `WM_ERASEBKGND` handling in `cef_browser_shell.cpp`, or the CEF browser `background_color` setting.
2. **DPI thing** — there's a Windows DPI/scaling issue (some UI element wrong size on non-100% DPI, or mismatched between Windows chrome and React). Investigate: which element, which DPI%, Windows Per-Monitor-V2 manifest status, and `GetDpiForWindow` usage. Likely relevant: the app manifest (per-monitor DPI awareness), any hardcoded pixel sizes in C++ overlay positioning, and CEF `device_scale_factor`.

After those two are investigated and fixed (or triaged as post-release), the plan is: build release binaries on Windows, build+notarize on mac, tag, publish. Do not push to origin until the Windows-side review is done — Matt will push after.

**Do not modify anything mac-specific** unless you find a genuine cross-platform regression. All files touched today that have mac-specific paths:
- `cef-native/src/handlers/simple_app.cpp` (mac env-var gating only — no Windows impact)
- `cef-native/src/handlers/simple_handler.cpp` (removed `#ifdef _WIN32` around adblock scriptlet pre-cache; the code now runs on both platforms, which was already guarded at runtime by `g_adblockServerRunning`)
- `cef-native/include/core/FingerprintProtection.h` (added whatsonchain.com to the `IsAuthDomain` allowlist — cross-platform, safe)
- `rust-wallet/src/monitor/task_check_for_proofs.rs` (cross-platform; no OS conditionals)
- `frontend/src/components/TabBar.tsx` (guarded by `isMac`)

**Files changed this session:**
- `cef-native/src/handlers/simple_app.cpp` (mac Chromium flag gating via `HODOS_MAC_DEV_FLAGS`)
- `cef-native/src/handlers/simple_handler.cpp` (cross-platform `OnBeforeBrowse` adblock pre-cache)
- `cef-native/include/core/FingerprintProtection.h` (whatsonchain added to IsAuthDomain)
- `cef-native/cef_browser_shell_mac.mm` (wallet panel URL `ppc`/`ppa` params — Windows parity)
- `frontend/src/components/WalletPanel.tsx` (WoC link via `tab_create` IPC)
- `rust-wallet/src/monitor/task_check_for_proofs.rs` (3-oracle quorum + 5-min rollback)
- `frontend/src/components/TabBar.tsx` (mac tab-bar top inset)
- `development-docs/Final-MVP-Sprint/AI-HANDOFF.md` (this entry)

**Commits (post-beta3-cleanup):**
- `b820a02` macOS: fix Cloudflare Turnstile infinite-loop on whatsonchain
- `1bd9850` macOS: wallet panel PeerPay params + WoC link via tab_create
- `4ef62e3` wallet: TaskCheckForProofs falls back to WoC on any ARC error
- `b65c39b` wallet: short-circuit mark_failed when both ARC and WoC 404 after 30m *(superseded by next commit)*
- `dce3236` wallet: three-oracle quorum for failed-tx rollback, 5-min threshold
- *(pending)* frontend: mac tab-bar top inset
- *(pending)* docs: AI handoff for Windows review

---

### 2026-04-14 — Matt / Claude (Mac) — Bug #9 DIAG-A1 Captured (Outcome A confirmed)

**What was done:**
- Set up the dev environment on the new MBP and ran tasks 1–3 from the prior handoff
- Discovered macOS-specific gap: CMake links with `-no_adhoc_codesign` so the locally built `.app` ships **unsigned**. Apple Silicon refuses to spawn unsigned CEF Helper bundles → browser exited cleanly with no window. Workaround: `codesign --force --deep --sign - HodosBrowserShell.app`. (CI release builds get Developer ID via `--force` so they overwrite ad-hoc; no conflict.) Worth adding a post-build ad-hoc-sign step in CMake for new Mac devs.
- **Bug #9 did NOT reproduce** in the locally built dev binary (ad-hoc signed, launched from CLI) — opened/closed tabs in many positions, no crash.
- Ran the macOS Standard test (~15 min): GitHub login + 2FA ✓, Google ✓, YouTube playback + adblock ✓ (one first-load ad slipped, refresh fixed it — known scriptlet pre-cache race), Twitch streams play ✓ (ads NOT blocked — expected, no Twitch-specific scriptlets), NYT ✓, WhatsOnChain ✓ (no CF challenge fired), nowsecure.nl ✓, g2.com ✓. **x.com login is broken**: enter email, page reloads, never advances. Same code paths as Windows for fingerprint + adblock unbreak — likely x.com bot detection vs our privacy stack (same friction Brave reports).
- Downloaded the `v0.3.1-diag.1.dmg` (notarized release with the DIAG-A1 instrumentation), launched via Finder.
- **Bug #9 DID reproduce** on the notarized release on first middle-tab close. DIAG-A1 stack trace captured — see below.

**Diagnostic result: Outcome A confirmed.**

```
[DIAG-A1] MainWindowDelegate::windowShouldClose called (window 0) — stack:
0  HodosBrowserShell  -[MainWindowDelegate windowShouldClose:]
1  AppKit             -[NSWindow __close]
2  AppKit             -[NSWindow __close]_block_invoke
3  AppKit             -[NSApplication sendAction:to:from:]
4  AppKit             -[NSControl sendAction:to:]
...
8  AppKit             -[NSButtonCell performClick:]   ← tab-close X button click
...
22 Chromium Embedded Framework  ChromeWebAppShortcutCopierMain ...
27 HodosBrowserShell  main + 9688
❌ Last window close requested - shutting down application
```

Trace shows that clicking a tab-close button on the legacy webview's tab triggers `[NSWindow close]` on the **main window** (not the tab). `MainWindowDelegate::windowShouldClose:` then fires, our handler treats it as "last window closed → shut down". Matches the legacy-webview hypothesis exactly.

**Why the dev build doesn't reproduce:** Most likely activation/responder-chain differences between Finder-launch + hardened runtime vs CLI-launch + ad-hoc. Untested but cheap to verify: move the dev `.app` to `/Applications`, `xattr -cr`, double-click from Finder — if it reproduces, that's our iteration loop.

**What's next (recommended, in order):**
1. Implement Phase A2/A3/B1 from `~/.claude/plans/graceful-forging-nygaard.md`:
   - Remove the legacy webview creation at `cef_browser_shell_mac.mm:4000-4029`
   - Seed the first tab via `TabManager::CreateTab()`
   - Wire tab-close buttons to TabManager, not NSWindow
   - Decouple "last tab closed" from "last window closed" (macOS menu-bar app convention)
2. Verify in dev via Finder-launch (move `.app` to `/Applications`) before going to a full notarized rebuild
3. Once green: add a post-build ad-hoc-sign step in `cef-native/CMakeLists.txt` so future Mac devs don't hit the silent-launch problem
4. Logged as new P4 #13: cookie consent banner blocking (Brave parity via EasyList Cookie filter list) — `post-beta3-cleanup.md` updated

**Files changed:**
- `development-docs/Final-MVP-Sprint/AI-HANDOFF.md` (this entry)
- `development-docs/Final-MVP-Sprint/post-beta3-cleanup.md` (added P4 #13)

---

### 2026-04-14 — Matt / Claude — macOS Dev Environment Setup Complete

**What was done:**
- Set up macOS dev environment on new MacBook Pro (first Mac, ARM/Apple Silicon)
- Installed: Xcode CLI tools, Homebrew, cmake, openssl, nlohmann-json, sqlite3, node, gh, Rust, Claude Code
- Cloned repo from `BSVArchie/Hodos-Browser` (personal fork), checked out `post-beta3-cleanup` branch
- Downloaded custom CEF binaries from GitHub release (`cef-binaries` tag on `Hodos-Browser/Hodos-Browser` org repo) — NOT the Spotify CDN builds (ours have proprietary codecs)
- Built all 4 components successfully: rust-wallet, adblock-engine, frontend, cef-native
- Copied 5 Helper bundles into HodosBrowserShell.app/Contents/Frameworks/
- Pushed all local Windows commits to `origin/post-beta3-cleanup` (was 20+ commits ahead of remote)

**What's blocked:**
- Nothing — ready for test run and Bug #9 diagnostic capture

**What's next — INSTRUCTIONS FOR MAC CLAUDE (execute these in order):**

#### Task 1: Test Run — Verify the build works

Open 4 Terminal tabs (`Cmd+T`) and run each in its own tab:

```bash
# Tab 1: Rust Wallet
cd ~/Hodos-Browser/rust-wallet && cargo run --release
# Wait for "Listening on: http://127.0.0.1:31301"

# Tab 2: Adblock Engine
cd ~/Hodos-Browser/adblock-engine && cargo run --release
# Wait for "Listening on: http://127.0.0.1:31302"

# Tab 3: Frontend Dev Server
cd ~/Hodos-Browser/frontend && npm run dev
# Wait for "Local: http://127.0.0.1:5137"

# Tab 4: Launch Browser
cd ~/Hodos-Browser/cef-native/build/bin
./HodosBrowserShell.app/Contents/MacOS/HodosBrowserShell
```

macOS may ask about network access — user should click **Allow**.

If a browser window appears, the build is working. If "unidentified developer" warning: `xattr -cr HodosBrowserShell.app`

#### Task 2: Capture Bug #9 Diagnostic Stack Trace

**Context:** Bug #9 = closing any tab on macOS kills the entire browser. Commit `eb65d30` on this branch added `[NSThread callStackSymbols]` logging (tagged `DIAG-A1`) to `windowShouldClose:` delegates in `cef_browser_shell_mac.mm` and `WindowManager_mac.mm`. The dev build already includes this instrumentation.

**Hypothesis:** A legacy standalone `"webview"` CEF browser created at `cef_browser_shell_mac.mm:4000-4029` (instead of via `TabManager::CreateTab`) coexists with managed tabs and creates a lifecycle invariant violation. When a managed tab closes, the cascade reaches `windowShouldClose:` → `ShutdownApplication()`.

**Steps:**
1. With the browser running from Task 1, click `+` to create 3-4 new tabs
2. Click X on a **middle** tab (not the first or last)
3. The app will crash/close — this IS the bug, it's expected
4. In Terminal, run:
   ```bash
   grep "DIAG-A1" "$HOME/Library/Application Support/HodosBrowser/debug_output.log"
   ```
5. If that returns nothing, try:
   ```bash
   cat "$HOME/Library/Application Support/HodosBrowser/debug_output.log" | tail -100
   ```
6. **Save the full output** — this is the stack trace we need to confirm the hypothesis

**Three possible outcomes:**
- **A) Trace shows `MainWindowDelegate::windowShouldClose:` with tab-close frames** → confirms legacy webview hypothesis. Fix: remove legacy webview, seed via `TabManager::CreateTab()`
- **B) Trace shows `BrowserWindowDelegate::windowShouldClose:`** → multi-window delegate is involved, need to check `WindowManager_mac.mm`
- **C) No DIAG-A1 output at all** → the crash bypasses `windowShouldClose:` entirely, need different instrumentation

#### Task 3: Report Results

Share the diagnostic output with the user. Include:
- Whether the browser window appeared (Task 1 result)
- The full DIAG-A1 grep output (or tail output if grep was empty)
- Which outcome (A, B, or C) matches

#### Background: Key files for the fix (DO NOT modify yet, just for context)
- `cef-native/cef_browser_shell_mac.mm` — legacy webview creation (~line 4000), `MainWindowDelegate::windowShouldClose:` (~line 1974)
- `cef-native/src/core/WindowManager_mac.mm` — `BrowserWindowDelegate::windowShouldClose:`
- `cef-native/src/core/TabManager_mac.mm` — tab lifecycle on macOS
- Bug tracker: `development-docs/Final-MVP-Sprint/post-beta3-cleanup.md`

#### Troubleshooting
- **Permission denied on launch:** `chmod +x HodosBrowserShell.app/Contents/MacOS/HodosBrowserShell`
- **"unidentified developer":** `xattr -cr ~/Hodos-Browser/cef-native/build/bin/HodosBrowserShell.app`
- **App crashes immediately:** Check Helper bundles exist: `ls HodosBrowserShell.app/Contents/Frameworks/ | grep Helper` (should be 5)
- **Port in use:** A previous run didn't clean up. `kill $(lsof -ti :31301) $(lsof -ti :31302)` then retry
- **No debug_output.log:** Check `~/Library/Application Support/HodosBrowser/` exists. If not, the app never got far enough to create it.

**Files changed:**
- `development-docs/Final-MVP-Sprint/AI-HANDOFF.md` (this entry)

---

### 2026-04-13 — Matt / Claude — Wallet Efficiency P0+P1 Complete + UX Fixes

**What was done:**
- Completed ALL remaining P1 items from wallet efficiency sprint:
  - Constant-time comparisons: `subtle` crate, fixed AES-GCM timing oracle + HMAC verify
  - BEEF compaction: parent_transactions cleanup added to task_purge (7-day retention for confirmed txs)
  - Auto dust consolidation: new TaskConsolidateDust monitor task (daily, P2PKH-only guard, tested on treasury wallet — tx confirmed on chain: `151183399bef47719de7fe296a842b6565653dfcc8f54783aaca61768b7e95d9`)
  - Manual trigger endpoint: `POST /wallet/consolidate-dust` with detailed JSON response
- Fixed 2 UX bugs found during testing:
  - Right-click paste in send form (added `onInput` handler for CEF compatibility)
  - Broadcast notification lost on overlay close (wallet_prevent_close IPC + sessionStorage persistence)
- On-chain backup verified after all changes (~68 KB tx size)
- Updated sprint checklist — all P0 + P1 items checked off

**What's blocked:**
- macOS bugs (#7, #8, #9, #11) blocked on real Mac hardware (refurb MBP purchased, needs dev env setup)
- Windows bugs #5, #6 need specific repro info from other PCs

**What's next:**
- Set up macOS dev environment (clone repo, install Rust/Node/CMake, build CEF from source)
- Resume macOS Bug #9 diagnostic (A1 — stack trace capture from `v0.3.1-diag.1` DMG)
- Then fix #8/#9 together (remove legacy webview, seed via TabManager::CreateTab)
- P2/P3 wallet efficiency items are nice-to-haves, not blocking MVP

**Files changed:**
- `rust-wallet/Cargo.toml`, `Cargo.lock` (subtle crate)
- `rust-wallet/src/crypto/aesgcm_custom.rs`, `signing.rs` (constant-time)
- `rust-wallet/src/monitor/task_consolidate_dust.rs` (NEW)
- `rust-wallet/src/monitor/mod.rs`, `task_purge.rs` (task registration + BEEF compaction)
- `rust-wallet/src/handlers.rs`, `main.rs` (consolidate-dust endpoint)
- `frontend/src/components/TransactionForm.tsx`, `WalletPanel.tsx` (UX fixes)
- `development-docs/Final-MVP-Sprint/wallet-efficiency-and-bsv-alignment.md` (checklist update)

---

### 2026-03-09 — Matt / Claude — GitHub Setup & Team Coordination

**What was done:**
- GitHub CLI (`gh`) installed and authenticated (BSVArchie) — works from WSL via credential helper
- Reviewed all GitHub issues (#7-#31) and milestones that John (Calgoon) created
- John's Track A (HTTP/backend): **all 7 issues closed** — SyncHttpClient, WinHTTP port, AdblockCache, singleton init, process auto-launch all done
- Ishaan's Track B (UI/overlays): #20 (Notification overlay) closed, 8 remain open
- Switched to Matt branch, merged origin/main (includes Ishaan's mac fix commit)
- Cleaned up `.claude/settings.local.json` — removed ~30 one-off bash permissions, added comprehensive git/file operation patterns, added deny list for destructive commands
- Reviewed Ishaan's mac fix commit (7d7f287) — confirmed Windows-safe, proper `#ifdef` patterns

**What's blocked:**
- Same as previous entry (send_transaction UX, persist_session_cookies)

**What's next:**
- UX/UI Phase 4 refinement — testing and polishing the wallet dashboard, general UI fixes
- All local changes (~110 files) still unstaged on Matt branch — need to be committed and pushed

**Files changed:**
- `.claude/settings.local.json` (rewritten — clean permission set)

---

### 2026-03-09 — Project Lead / Claude — Sprint Setup

**What was done:**
- Created `Final-MVP-Sprint/` folder with sprint documentation
- `TESTING_GUIDE.md` — 7-tier exploration mission guide for manual testing
- `OPTIMIZATION_PRIORITIES.md` — before/after testing optimization sequencing
- `SECURITY_MINDSET.md` — security philosophy, current posture, dev watch list
- `CLAUDE.md` — AI assistant orientation for this sprint
- Moved `macos-port/` into this folder, updated all references across the repo
- Rust wallet made mac-ready (cross-platform paths, macOS Keychain encryption)
- Adblock engine made mac-ready (test path conditionals)
- Both Rust binaries built and verified on Windows

**What's blocked:**
- macOS C++ build untested (no macOS machine available to project lead)
- `send_transaction` UX bug (black screen) — needs verification post-BEEF-cache-fix. If still broken, needs async IPC conversion (see OPTIMIZATION_PRIORITIES.md item #1)
- `persist_session_cookies` commented out in `cef_browser_shell.cpp` — may cause login sessions to not survive restart. Testing guide Mission 1.3 will reveal this.

**What's next:**
- **macOS dev**: Read `macos-port/MACOS-PORT-HANDOVER.md`, get CEF building on macOS, start Phase 1 (missing overlays)
- **Hardening dev**: Verify send_transaction UX, then start testing guide Tier 1
- **All**: Archive `frontend-ui-ux-cleanup-optimization.md` and `data-storage-and-encryption-review.md` (content captured in sprint docs)

**Files changed:**
- `development-docs/Final-MVP-Sprint/` (new folder, 6 files)
- `rust-wallet/src/main.rs`, `rust-wallet/src/crypto/dpapi.rs`, `rust-wallet/src/bin/extract_master_key.rs`, `rust-wallet/Cargo.toml`
- `adblock-engine/src/engine.rs`
- `CLAUDE.md`, `README.md`, `PROJECT_OVERVIEW.md` (reference updates)
- `build-instructions/BUILD_INSTRUCTIONS.md`, `MACOS_BUILD_INSTRUCTIONS.md`, `WINDOWS_BUILD_INSTRUCTIONS.md` (reference updates)
