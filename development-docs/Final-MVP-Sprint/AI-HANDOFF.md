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
