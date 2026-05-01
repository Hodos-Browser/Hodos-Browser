# macOS Sprint Handover — 2026-05-01

This document covers all changes made on branch `fix/auth-login-session-bugs` that need macOS testing and verification. Two commits:

1. `3761e07` — Auth/login session persistence fixes
2. `bc2e4fe` — Profile management UI, lock retry, per-profile taskbar icons

---

## 1. Auth/Login Session Persistence (commit 3761e07)

### What changed

**Files modified (macOS-relevant):**
- `cef_browser_shell_mac.mm` — Added `persist_session_cookies = false` comment confirming it must stay disabled
- `include/core/FingerprintProtection.h` — Expanded `IsAuthDomain()` from 13 to 35+ domains (cross-platform, header-only)
- `src/core/CookieBlockManager.cpp` — Added `AreSameAuthEntity()` same-entity cookie grouping (cross-platform)

### What to test on macOS

**Google login:**
1. Go to google.com → Sign In
2. Complete login flow (may involve accounts.google.com, myaccount.google.com)
3. Verify login persists across page navigations
4. Close browser, reopen → google.com should still be logged in (session cookies)

**X.com (Twitter) login:**
1. Go to x.com → Sign In
2. Complete login (involves twitter.com cookies on x.com domain)
3. Verify session persists — scroll feed, navigate, come back
4. The `AreSameAuthEntity()` function prevents EphemeralCookieManager from deleting twitter.com cookies when on x.com

**GitHub login:**
1. Go to github.com → Sign In
2. Verify session persists across navigation

**Fingerprint protection bypass:**
- Auth domains (google.com, x.com, github.com, microsoft.com, apple.com, etc.) should NOT have canvas/WebGL farbling
- Verify by checking `FingerprintProtection::IsAuthDomain()` is being called in `cef_browser_shell_mac.mm` fingerprint injection path

---

## 2. Profile Management UI (commit bc2e4fe)

### What changed

**C++ backend (cross-platform):**
- `ProfileManager.h` — Added `SetProfileAvatar()`, `SetDefaultProfile()`, `GetDefaultProfileId()`, `defaultProfileId_` member
- `ProfileManager.cpp` — New methods, `Load()`/`Save()` for `defaultProfileId`, `DeleteProfile()` uses `defaultProfileId_` instead of hardcoded "Default", `ParseProfileArgument()` returns `""` when no `--profile=` flag
- `simple_handler.cpp` — Added 3 IPC handlers: `profiles_set_color`, `profiles_set_avatar`, `profiles_set_default`. Updated `profiles_get_all` response to include `defaultProfileId`

**macOS startup (platform-specific change):**
- `cef_browser_shell_mac.mm` (~line 4491) — Changed from `std::string profileId = "Default"` to `std::string profileId = ""`, then falls back to `ProfileManager::GetInstance().GetDefaultProfileId()` when empty. This is the default profile startup integration.

**Frontend (cross-platform):**
- `useProfiles.ts` — Added `defaultProfileId` state, `setProfileColor()`, `setProfileAvatar()`, `setDefaultProfile()` functions
- `ProfilePickerOverlayRoot.tsx` — Added inline edit mode with pencil icon, edit form (name/color/avatar/default/delete), star icon for default profile

### What to test on macOS

**Profile edit UI:**
1. Open profile picker (toolbar avatar icon)
2. Hover a profile → pencil icon appears → click it
3. Rename profile → Save → verify name and avatar initial update
4. Change color → Save → verify avatar circle color changes
5. Upload custom avatar image → Save → verify it displays
6. Cancel → verify changes are discarded

**Default profile:**
1. Create a second profile if none exists
2. Edit a profile → click "Set as default" star → verify star appears
3. Close browser → reopen (no `--profile=` flag) → verify the default profile launches
4. This tests the `cef_browser_shell_mac.mm` startup change

**Delete profile:**
1. With 2+ profiles, edit a non-default profile → click delete (red trash icon)
2. Verify it disappears from list
3. Verify delete is disabled on the default profile
4. Verify delete is disabled when only one profile remains

**Profile switch:**
1. Create 2 profiles → click one to switch → verify new window opens with correct profile

---

## 3. Profile Lock Retry (commit bc2e4fe)

### What changed

- `ProfileLock.cpp` — macOS/Linux path: 6 retries × 500ms `usleep()` on `flock()` failure before returning false

### What to test on macOS

1. Open browser → close it → immediately reopen → should succeed without error
2. The retry gives the previous instance up to 3 seconds to release the lock
3. If possible, try a few rapid close/reopen cycles

---

## 4. WAL Checkpoint (commit bc2e4fe)

### What changed

- `HistoryManager.cpp`, `BookmarkManager.cpp`, `CookieBlockManager.cpp` — Added `PRAGMA wal_checkpoint(RESTART)` before `sqlite3_close()` in each `CloseDatabase()` method

### What to test on macOS

1. Browse a few sites (generates DB writes)
2. Close browser cleanly
3. Check profile directory (`~/Library/Application Support/HodosBrowserDev/Default/`) for stale `-wal` or `-shm` files
4. There should be none (or very small) after clean shutdown

---

## 5. Per-Profile Taskbar Icons — Windows Only, NO macOS Changes

`TaskbarProfile.h/.cpp` is Windows-only (`#ifdef _WIN32`). The `SetupTaskbarProfile()` call in `WindowManager.cpp` is inside the existing `#ifdef _WIN32` block. `cef_browser_shell_mac.mm` has no taskbar icon changes.

Chrome doesn't do per-profile dock icons on macOS either — macOS doesn't support separate dock entries per app instance.

**Nothing to test on macOS for this feature.**

---

## Build on macOS

```bash
cd cef-native && ./mac_build_run.sh
```

Frontend changes are already built (React/TypeScript). The C++ changes need a macOS rebuild.

## Files touched (macOS-relevant only)

| File | What changed |
|------|-------------|
| `cef_browser_shell_mac.mm` | Default profile startup + persist_session_cookies comment |
| `include/core/FingerprintProtection.h` | Auth domain list expanded (header-only, cross-platform) |
| `src/core/CookieBlockManager.cpp` | Same-entity cookie grouping (cross-platform) |
| `include/core/ProfileManager.h` | New method declarations (cross-platform) |
| `src/core/ProfileManager.cpp` | New methods + defaultProfileId (cross-platform) |
| `src/core/ProfileLock.cpp` | Retry logic in macOS/Linux path |
| `src/core/HistoryManager.cpp` | WAL checkpoint (cross-platform) |
| `src/core/BookmarkManager.cpp` | WAL checkpoint (cross-platform) |
| `src/core/CookieBlockManager.cpp` | WAL checkpoint (cross-platform) |
| `src/handlers/simple_handler.cpp` | New IPC handlers (cross-platform) |
| `frontend/src/hooks/useProfiles.ts` | New hook functions (cross-platform) |
| `frontend/src/pages/ProfilePickerOverlayRoot.tsx` | Edit mode UI (cross-platform) |
