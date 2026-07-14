# Mac Claude — Deconfliction Audit Verification Report (2026-07-14)

**Branch:** `0.4.0`  
**Responding to:** `DEVPROD_DECONFLICTION_AUDIT.md` handoff items 1–5

---

## Item 1 — C2 Mac Launcher Kill Scoping: VERIFIED (runtime)

**Test:** Launched installed `/Applications/HodosBrowser.app`, then ran:
```bash
DEV_BUNDLE="$(pwd)/build/bin/HodosBrowser.app"
pgrep -f "$DEV_BUNDLE"   # → matched 0 processes
```
The installed app's argv (`/Applications/HodosBrowser.app/Contents/MacOS/HodosBrowser`) does not contain the `build/bin/HodosBrowser.app` substring. `pkill -f "$DEV_BUNDLE"` is correctly scoped — it will never match the installed app.

The absolute-path launch at `mac_build_run.sh:73` (`"$DEV_BUNDLE/Contents/MacOS/HodosBrowser"`) ensures dev argv[0] always contains `build/bin/HodosBrowser.app`, so the kill pattern reliably matches dev-only.

**Result:** ✅ Confirmed safe. Fail-safe direction is "kill nothing."

---

## Item 2 — C1 Mode-B Env Safeguard: VERIFIED (code review)

**Code path verified (fork review):**
1. `EnforceDevSafeguard(exec_path)` called at `cef_browser_shell_mac.mm:5213–5214`, before the first `GetAppDirName()` at `:5263`.
2. Installed app path (`/Applications/HodosBrowser.app/Contents/MacOS/HodosBrowser`) matches none of the `is_dev_build` patterns (`build/bin/Release`, `build/bin/Debug`, `build/bin/HodosBrowser`), so `is_dev_build = false`.
3. With `HODOS_DEV=1` set and `is_dev_build = false`: the guard calls `unsetenv("HODOS_DEV")` + prints the Mode-B warning banner, then returns `true`.
4. All subsequent reads — `GetAppDirName()` (direct `getenv`), `hodos::IsDevEnv()` (cached `static const bool`, first call happens *after* the scrub), `keychain_service()` in Rust, port selection — resolve to **prod** namespace.
5. Child processes (wallet, adblock, CEF helpers) spawned via `posix_spawn(..., environ)` at `:5066` inherit the scrubbed environment because `unsetenv` modifies the process's `environ` in-place.

**Runtime test was not possible:** the installed `/Applications/HodosBrowser.app` is an older build that predates the `EnforceDevSafeguard` code. Running it with `HODOS_DEV=1` showed `HodosBrowserDev` paths (expected — old binary has no safeguard). A full runtime verification requires installing a build that includes the safeguard.

**Result:** ✅ Code review confirms correct wiring. Runtime verification deferred to next installed build.

---

## Item 3 — H1 Sparkle Gate: IMPLEMENTED + COMPILED

**Change:** `cef_browser_shell_mac.mm:5601`  
- Old: `if (!g_picker_mode) {`  
- New: `if (!g_picker_mode && !hodos::IsDevEnv()) {`  
- Added `else if (hodos::IsDevEnv()) { LOG_INFO("Auto-updater skipped (dev build)"); }`

**Effect:** Dev builds skip the entire Sparkle init block — no `SUUpdater` allocation, no appcast check, no `NSUserDefaults` writes. The installed prod app's `SULastCheckTime` / `SUSkippedVersion` / auto-check toggle are untouched.

Added `#include "include/core/PortConfig.h"` at line 26 (needed for `hodos::IsDevEnv()`).

Build verified: `cmake --build build --config Release` succeeded with zero warnings from the changed lines.

**Result:** ✅ Implemented. Ready for adversarial review.

---

## Item 4 — M1 Debug Port Offset: IMPLEMENTED + COMPILED

**Change:** `cef_browser_shell_mac.mm:5414–5415`  
```cpp
if (hodos::IsDevEnv() && settings.remote_debugging_port != 0)
    settings.remote_debugging_port += 100;
```

**Effect:** Dev Default profile binds 9322; prod Default binds 9222. Non-Default profiles keep `0` (disabled). Mirrors the Windows pattern at `cef_browser_shell.cpp:4754–4756`.

Build verified: same successful build as H1.

**Result:** ✅ Implemented. No socket collision possible between dev and prod Default profiles.

---

## Item 5 — Blast Radius Confirmation

**Findings:**
- Both `HodosBrowser/` and `HodosBrowserDev/` profile directories exist on this machine — expected for a dev machine.
- Neither Keychain entry (`HodosBrowser` nor `HodosBrowserDev` service, `encrypted_mnemonic` account) currently exists — consistent with the prior sweep cleanup.
- There are **no macOS beta testers**. The macOS build is dev-only; end users run the Windows build. No external user has ever had `HODOS_DEV` in their environment.
- The C1/Keychain contamination class (`a85985f`) is confirmed **dev-machines-only**. No recovery needed for any external user.

**Result:** ✅ Blast radius is dev-machines-only. No user impact.

---

## Summary

| # | Item | Method | Result |
|---|------|--------|--------|
| C2 | Launcher kill scoping | Runtime test | ✅ Verified — installed app not matched |
| C1 | Mode-B env safeguard | Code review + fork | ✅ Wiring confirmed correct; runtime deferred to next build |
| H1 | Sparkle gate | Implemented | ✅ `!hodos::IsDevEnv()` gate, compiled |
| M1 | Debug port offset | Implemented | ✅ `+100` offset for dev, compiled |
| 5 | Blast radius | Keychain + profile check | ✅ Dev-machines-only, no user impact |

**Commit:** H1 + M1 code changes in `cef-native/cef_browser_shell_mac.mm` (6 lines added, 1 changed).
