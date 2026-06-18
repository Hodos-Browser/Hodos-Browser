# 0.4.0 — macOS Port Delta Log

> **Purpose:** Windows-first execution. As each Windows change lands, its macOS-parity delta is recorded here. When the **Mac sprint** starts, pull this doc and implement straight from it, then run Mac smoke + Mac B1-VERIFY.
>
> **How it's filled:** step 6 of the per-chunk harness lifecycle (`ORCHESTRATION_PLAN_0_4_0.md` §1) + a periodic mac-parity sweep workflow (§6).
>
> **Created 2026-06-17. Status: skeleton — populated as Windows work lands.**

---

## How to use this doc (Mac-sprint agent boot)

1. Read `ORCHESTRATION_PLAN_0_4_0.md` §6 + this whole log.
2. For each entry below, open the cited Windows file:fn and the Mac counterpart, implement the delta.
3. Honor CLAUDE.md Invariant #9 (platform conditionals) — Mac code lives in `*_mac.mm` / `#elif defined(__APPLE__)`.
4. Run Mac smoke (Authentication + Video/Media + News categories) + the B1 cross-session login test on macOS.

## Known macOS parity anchors (from prior review)

- Overlays: macOS uses `NSPanel` + `NSWindowDelegate` (`cef_browser_shell_mac.mm`), NOT `WS_POPUP`. Any new overlay needs a Mac creation fn.
- Tabs/windows: `TabManager_mac.mm`, `WindowManager_mac.mm` mirror the Windows APIs.
- HTTP singletons: macOS uses libcurl (`*_mac.cpp` / `SyncHttpClient` libcurl path), not WinHTTP.
- Auto-update: macOS = Sparkle (EdDSA already); Windows = WinSparkle (DSA→EdDSA this sprint, Q9). Mac side mostly unchanged — verify appcast-decouple (Q13) applies to both.

---

## Delta log

> Format per entry:
> ### <chunk id> — <short title> (date, Windows commit)
> - **Windows change:** `file:fn` — what changed.
> - **Mac equivalent:** `file_mac.mm:fn` (or `#elif __APPLE__` block) — what to do.
> - **Risk / notes:** platform-specific gotchas, test to run.

### Wave 0 — secret-log removal (2026-06-17, branch `0.4.0`)
- **Windows change:** `WalletService.cpp::createWallet` — deleted mnemonic `std::cout`. Plus Rust deletions in `crypto/brc2.rs`, `certificate/verifier.rs`, `handlers/certificate_handlers.rs`, `handlers.rs`.
- **Mac equivalent:** **None required.** `WalletService_mac.cpp` (libcurl) never logged the mnemonic — swept all `*_mac.*` + `*.mm` for secret `cout`/`NSLog`/`os_log`, zero siblings. Rust is single cross-platform source (no `_mac` variant).
- **Risk / notes:** Nothing to port. Verified by grep over `*_mac.*` and `*.mm`.

### Wave 0 follow-up — AddressHandler phantom-`privateKey` removal (2026-06-17, branch `0.4.0`)
- **Windows change:** `AddressHandler.cpp` (delete phantom `privateKey` cout + V8 `SetValue`), `simple_app.cpp:479` (legacy injected debug-JS), `frontend/src/types/address.d.ts:4` (type field).
- **Mac equivalent:** **None required.** `AddressHandler.cpp` and `simple_app.cpp` are single cross-platform files; injected JS + TS type are platform-agnostic. No Mac-specific address-gen path.
- **Risk / notes:** Zero functional impact — the `privateKey` field is never returned by Rust nor consumed by JS (phantom).

### Wave 1 Track A — F7 backup/restore path-traversal + internal-only gate; F9 cert malformed-fields panic (2026-06-18, branch `0.4.0`)
- **Windows change:** Pure Rust (platform-agnostic backend). `backup.rs` (`backups_dir_for_db`, `lexical_normalize_abs`, `validate_backup_path`), `handlers.rs` (`wallet_backup` + `wallet_restore`: internal-only `X-Requesting-Domain` gate + path validation before any FS touch), `handlers/certificate_handlers.rs` (`acquire_certificate_issuance` `is_object()` guard).
- **Mac equivalent:** **None required.** Single cross-platform Rust source — no `_mac` variant. Path logic is cross-platform: the `\\?\`/UNC/`\\.\`-rejection test is `#[cfg(windows)]`; the POSIX accept/reject variants already run on the macOS leg.
- **Risk / notes:** At Mac smoke, sanity-check `lexical_normalize_abs`/`validate_backup_path` against a real macOS data path (`~/Library/Application Support/HodosBrowser/backups`) — the unit tests cover the POSIX shape but confirm `data_root()` resolution end-to-end. No Mac code to port.
- **Future (deferred, not built):** the user-facing "copy the file"/cloud-backup buttons must obtain the destination from the **OS save dialog driven by the C++ shell** (authenticated path), not an HTTP body — at which point the `backups/` confinement relaxes for that dialog-returned path. Mac side: native save dialog via `cef_browser_shell_mac.mm`.

### Wave 1 Track A — F6 JS-string-injection hardening (2026-06-18, branch `0.4.0`)
- **Windows change:** New header-only `cef-native/include/core/JsStringEscape.h` (hardened `escapeJsonForJs`); `simple_render_process_handler.cpp` deletes its local `static` copy, `#include`s the header, and routes 3 sites through it (`brc100_auth_request` 5 dApp fields, `tab_list_response`, `omnibox_select`). New GoogleTest `tests/js_string_escape_test.cpp` (15 tests) + `tests/CMakeLists.txt` entry.
- **Mac equivalent:** **None required.** `simple_render_process_handler.cpp` is a single cross-platform file (per `handlers/CLAUDE.md`, all 5 handler files are cross-platform); the new header is pure C++ (no platform code). The encoder behaves identically on macOS.
- **Risk / notes:** Build verified on Windows (encoder 54/54 GoogleTest green; full `HodosBrowserShell` recompiles clean). On the Mac build, the same `hodos_tests` target compiles + runs (no Mac-specific wiring). **Live smoke (deferred to next dev run, both platforms):** BRC-100 auth overlay still populates domain/method/body on a real dApp; a tab whose title contains an apostrophe still renders (was the `tab_list_response` breakout); omnibox arrow-key nav still works.

### Wave 1 Track A — F5 / R1 profile-launch cmd-injection (2026-06-18, branch `0.4.0`)
- **Windows change:** `ProfileManager.h` adds inline `IsValidProfileId` (cross-platform; 9 GoogleTests). `ProfileManager.cpp::LaunchWithProfile` gains a cross-platform validation guard at the top. `simple_handler.cpp` `profiles_switch` IPC validates the id (defense-in-depth). All compile-clean on Windows; the Windows `CreateProcessW` branch is unchanged (validation now guarantees a safe id).
- **⚠️ Mac equivalent — COMPILE-VERIFY REQUIRED ON MAC (this is the core of F5):** `ProfileManager.cpp` `#elif defined(__APPLE__)` branch (~`:435`) replaces `system(cmd)` with **`posix_spawn("/usr/bin/open", argv…)`** (argv `{"/usr/bin/open","-n","-a",appPath,"--args","--profile="+id,nullptr}`) + `waitpid`. New mac includes added: `<spawn.h>`, `<sys/wait.h>`, `<cstring>`, `<cerrno>`, `extern char** environ;`. **This branch is `#elif`-gated so the Windows build did NOT compile it** — on the first Mac build, confirm it compiles and that profile switching still launches a new instance with the right `--profile`. argv is byte-identical to the old shell string, so behavior should match.
- **Risk / notes:** `IsValidProfileId` accepts the legacy `"Profile N"` (space) id form — verified against `GenerateProfileId` — so existing profiles are not locked out. Live smoke (next dev run, **especially macOS**): create/switch profiles; confirm a new instance launches with the correct profile and a malformed id is rejected.
