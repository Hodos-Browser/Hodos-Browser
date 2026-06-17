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
