# Dev / Prod Deconfliction — Full Audit (2026-07-14)

**Status:** C2 (launcher-kill) FIXED all 3 launchers (mac needs runtime-verify). C1: owner **dropped the startup pubkey-check** (see decision below) and approved the **Mode-B env safeguard**, now IMPLEMENTED (force-prod scrub in both layers). H1/M1/L1–L3 tracked below.

## Owner decisions (2026-07-14)
- **Storage model confirmed:** the machine's DPAPI (Windows) / Keychain (macOS) is *at-rest convenience encryption of the mnemonic, bound to the OS login* (same pattern as Chrome/Brave "Safe Storage") — NOT identity, NOT a machine lock. The wallet is fully portable: the **mnemonic** recovers it on any machine; the **encrypted export** (`wallet_export`, AES-256-GCM under a password, includes the seed) imports anywhere; even a raw `wallet.db` carries the PIN-encrypted seed (usable elsewhere with the PIN; the DPAPI/Keychain auto-unlock is machine-bound by design and simply falls back to the PIN). The July bug was ONE shared macOS Keychain slot (un-namespaced service name), already fixed @ `a85985f`; files were always separate; Windows was never affected.
- **DROP the startup pubkey-check (C1 durable half).** Rationale: it's insurance, not a fix — the source bug (shared Keychain slot) is fixed at the source, and the benefit to non-dev users is ~nil; not worth a high-risk money/signing-path change. We *prevent* the wrong storage spot rather than *detect* a wrong key. `DEVPROD_C1_STARTUP_PUBKEY_CHECK_DESIGN.md` is retained for the record only (NOT to be built).
- **DO the Mode-B env safeguard (force-prod + warn).** IMPLEMENTED as a bidirectional dev-safeguard in both layers.

## Why this audit exists

Two recent dev/prod isolation failures:
1. **macOS Keychain service-name collision** (`dpapi.rs`) — dev + prod both used Keychain service `"HodosBrowser"`, so creating a dev wallet silently overwrote the production mnemonic → wrong-key signing → backup NULLFAIL. Found + fixed on the machine by Mac Claude @ `a85985f` (service now `HodosBrowserDev` under `HODOS_DEV=1`). No funds lost; ~12.5M sats swept back.
2. **Dev build-and-run killed the running installed production browser** (owner hit this prepping a BSV-dev meeting).

Owner request: enumerate **every** dev/prod shared surface once, prove isolation adversarially (Win + Mac), rank gaps by blast radius, fix systematically — "so we don't keep getting stuck on this stuff."

## Method

Adversarial workflow (`wf_ea9a32b7-4eb`): 8 shared surfaces, each **mapped** from real code then handed to an independent **skeptic** told to refute the isolation claim with a concrete collision path. 16 agents, 0 errors. **6 of 8 surfaces were refuted** — the skeptic found a real collision the map missed. Key correction: the port split (31401/31402) and pipe `dev.` prefix **did land** (prior memory saying "never landed" was stale).

---

## Ranked gap list

### 🔴 C1 — CRITICAL — Leaked/persistent `HODOS_DEV=1` flips the *installed prod* app into the Dev namespace
The two dev-safeguards don't cover the installed binary layout and are **one-directional**:
- Rust `enforce_dev_safeguard` (`rust-wallet/src/main.rs:180-183`) matches only `target/{release,debug}`.
- C++ `AppPaths::EnforceDevSafeguard` (`cef-native/include/core/AppPaths.h:142-146`) matches only `build/bin/{Release,Debug,HodosBrowser}`.

The installed wallet (`%LOCALAPPDATA%\HodosBrowser\hodos-wallet.exe`) and shell (`HodosBrowser.app/Contents/MacOS/`) match **neither** pattern, so the shipped binary has **zero** protection. Both safeguards only force a *dev-path* binary to **set** the var — nothing forces a *prod-path* binary to have it **unset**. The shell reads `HODOS_DEV` once (`AppPaths.h:12 GetAppDirName`, `PortConfig.h:30`) and spawns the wallet with inherited env (Windows `CreateProcessA lpEnvironment=nullptr` `cef_browser_shell.cpp:3538`; macOS `posix_spawn(...,environ)` `cef_browser_shell_mac.mm:5066`), so shell + wallet always agree on the *wrong* namespace — never a catchable mismatch.

**Harm:** a single persistent `HODOS_DEV=1` in a developer's user/shell environment makes the installed prod app resolve every secret-store + data + port access to `HodosBrowserDev`: opens `%APPDATA%\HodosBrowserDev\wallet\wallet.db` and (macOS) Keychain service `HodosBrowserDev`, account `"wallet-mnemonic"` (shared constant, `dpapi.rs:147`). Run the real dev build too → both collide on the same `wallet.db` + same single Keychain item = **the exact `a85985f` overwrite/wrong-key class**, genuine prod wallet orphaned.

**Fix (owner-gated — design v2 done + adversarial-reviewed; awaiting owner sign-off before code):** see `DEVPROD_C1_STARTUP_PUBKEY_CHECK_DESIGN.md`. Review (`wf_01d524e9-807`, 5 lenses) validated the core (non-circular, no encoding false-positive) and made 5 corrections:
- **Durable:** startup master-pubkey check — derive from the auto-unlocked cached mnemonic, compare to `users.identity_key`, and gate at the **DB chokepoint `get_cached_mnemonic()`** (NOT the HTTP handlers — the Monitor signs autonomously and would leak). Never refuse on inconclusive (empty/legacy `users` row). Env-independent; catches Mode A (the `a85985f` class).
- **Defense-in-depth (Mode B / the flip):** make both dev-safeguards bidirectional via the **inverted rule** — `HODOS_DEV=1` is only legitimate from a recognized dev-*build* path; refuse/force-prod otherwise (covers `%LOCALAPPDATA%`, `.app`, AND portable-ZIP in one rule; avoids the `GetAppInstallDir` self-reference). Symmetric in both layers; shell scrubs `HODOS_DEV` before spawning the wallet child.

### 🔴 C2 — CRITICAL — Dev launcher force-kills the running installed prod browser  ✅ FIXED (Win) / ⏳ Mac verify
Dev + prod share `OUTPUT_NAME "HodosBrowser"` (`CMakeLists.txt:348` mac / `:357` win), so a bare image-name kill hit both:
- `cef-native/win_build_run.sh:27` `taskkill //F //IM HodosBrowser.exe` — **the launcher CLAUDE.md documents as canonical.**
- `cef-native/win_build_run.ps1:27` `Stop-Process -Name "HodosBrowser" -Force`.
- `cef-native/mac_build_run.sh:54` `pkill -9 HodosBrowser`.

`//F`/`-9` is ungraceful → bypasses the wallet money-DB flush (`StopWalletServer→SendShutdownRequest`) and orphans prod's `hodos-wallet.exe`/`hodos-adblock.exe`. In-tree precedent for the correct pattern: `SU_CountSelfBrowsers` (`cef_browser_shell.cpp:3851`) matches by **full module path**.

**Fix applied (2026-07-14):** all three launchers now scope the kill to the dev build dir (Windows: `Get-CimInstance Win32_Process` filtered by `ExecutablePath.StartsWith(build\bin\Release)`; macOS: `pkill -9 -f "$SCRIPT_DIR/build/bin/HodosBrowser.app"` + absolute-path launch so argv is path-scoped). Fail-safe direction is "kill nothing," never "kill prod." Windows CIM filter validated read-only (resolves path, runs, matches 0 with no dev instance). **Mac must runtime-verify** (see handoff §).

### 🟠 H1 — HIGH — macOS Sparkle auto-update runs *ungated* in dev
`cef_browser_shell_mac.mm:5601-5629` inits Sparkle every non-picker launch, **no `IsDevEnv` gate**, defaults to `UpdateMode::Silent` (`SettingsManager.h:17`), and `AutoUpdater_mac.mm:162-164` forces `checkForUpdatesInBackground` every launch for any mode ≠ Off. Single public appcast (`Info.plist:37-38 SUFeedURL=https://hodosbrowser.com/appcast.xml`), single bundle id `com.hodosbrowser.app` (`Info.plist:9-10` — only the *helper* plist template carries a `${BUNDLE_ID_SUFFIX}`, not the main app). Net: a dev `.app` silently downloads/install-on-quit-stages the public prod release **and shares Sparkle's `NSUserDefaults` (SULastCheckTime / SUSkippedVersion / auto-check toggle) with the installed prod app** under the same defaults domain — every dev launch rewrites prod's update state.
Windows counterpart: `AutoUpdater::Initialize` (`cef_browser_shell.cpp:5238-5254`) also ungated against the same appcast; `AutoUpdater.cpp:72 win_sparkle_set_app_details` uses identical strings → shared HKCU WinSparkle subtree, but NOTIFY is click-gated → lower.
**Fix:** gate updater init behind `!hodos::IsDevEnv()` (and/or a dev feed URL + dev bundle id / separate defaults domain). Mac implements + verifies; adversarial review before landing (auto-update path).

### 🟡 M1 — MEDIUM — macOS CEF `remote_debugging_port` 9222 not dev-gated
`cef_browser_shell_mac.mm:5412` sets `remote_debugging_port = (profileId=="Default") ? 9222 : 0` with **no `HODOS_DEV` offset**, while Windows `cef_browser_shell.cpp:4754-4756` adds `+100` for dev (dev Default=9322 / prod=9222). `root_cache_path` stops the CefInitialize SingletonLock failure but not the socket collision: dev + prod Default both bind `127.0.0.1:9222`; whichever wins owns it, so a DevTools/CDP client on 9222 can attach to and drive the **prod funded-wallet** browser instead of dev.
**Fix:** mirror the Windows `+100` dev offset on the mac path (gate with `hodos::IsDevEnv()`). Mac verifies.

### ⚪ L1 — LOW — silent-update staging mutex not namespaced
`cef_browser_shell.cpp:5292` `CreateMutexW(nullptr, FALSE, L"Local\\HodosUpdateStaging")` — the one named OS object not routed through `GetAppDirName()`. Compile-gated OFF today (`HODOS_SILENT_AUTOUPDATE` default OFF, `CMakeLists.txt:85`), so no live collision; becomes dev-blocks-prod-staging if both are built with the flag ON. Staging dir itself IS namespaced, so no data corruption. **Fix:** name it `Local\<GetAppDirName()>_UpdateStaging`. (Also the `:5268` comment is stale — claims a shared pending dir that no longer exists.)

### ⚪ L2 — LOW — update-helper hardcodes ports
`cef-native/update-helper/transaction.cpp:310-311,324,488-489` hardcode `HttpPostShutdown(31301)/(31302)` instead of `hodos::WalletPort()/AdblockPort()`. Benign (helper only runs for a prod install) but a latent literal hole. **Fix:** route through `PortConfig.h`.

### ⚪ L3 — LOW — comment/doc drift
`HttpRequestInterceptor.cpp:3722` & `:3766` comments say "port 31301"/"localhost:31301" though the code uses `hodos::WalletPortStr()/WalletBaseUrl()`; `development-docs/0.4.0/CHROMIUM_CEF_SPRINT_KICKOFF.md:23` falsely asserts the interceptor "still hardcodes 31301." No runtime impact; correct to prevent a maintainer "re-fixing" a non-bug.

---

## Verified isolated (held under adversarial refute)

| Surface | Mechanism | Evidence |
|---|---|---|
| Backend ports 31301/31401, 31302/31402 | `PortConfig.h` helpers + interceptor rewrites stray literals to the active port + Rust `wallet_port()`/`adblock_port()` gate | `PortConfig.h:38-39`, `HttpRequestInterceptor.cpp:3740-3741`, `main.rs:120-125` |
| Frontend origin 5137 | Prod **intercepts** `127.0.0.1:5137` via `LocalFileResourceHandler` (serves local files, **no socket bind**); dev uses the real Vite server. Each process resolves it in its own request pipeline → no cross-process collision | `LocalFileResourceHandler.h:22-23` |
| Data dirs, all SQLite DBs, CEF cache/`root_cache_path`, logs, update working area | All rooted at `GetAppDirName()`/`app_dir_name()` → `HodosBrowserDev` vs `HodosBrowser` | `AppPaths.h:11`, `main.rs:107`, `cef_browser_shell.cpp:4465,4730` |
| Single-instance pipe, AnyInstance mutex, ProfileLock, RegistryLock | Pipe `dev.` prefix; mutex `HodosBrowserDev_AnyInstance`; lock/registry keyed per data-root | `SingleInstance.cpp:53-59`, `AppPaths.h:132`, `ProfileManager.cpp:53` |

---

## Fix status

| # | Gap | Blast | Status | Owner |
|---|---|---|---|---|
| C1 | Leaked `HODOS_DEV` flips prod → Dev namespace | Critical | ✅ **Mode-B env safeguard IMPLEMENTED** (force-prod scrub, Rust `main.rs` + C++ `AppPaths.h`, Rust compiles); pubkey-*check* SHELVED by owner. ⏳ Mac verify (mac shell scrub) | Win done; Mac verifies |
| C2 | Launcher kills prod by image name | Critical | ✅ Win fixed + validated; ⏳ Mac verify | Win done; Mac verifies |
| H1 | macOS Sparkle ungated in dev | High | Open | Mac |
| M1 | macOS debug port 9222 not dev-gated | Medium | Open | Mac |
| L1 | Staging mutex not namespaced | Low | Open (compiled out today) | Win |
| L2 | Update-helper hardcodes ports | Low | Open | Win |
| L3 | Comment/doc drift | Low | Open | Win |

**Mode-B implementation note:** the guard is bidirectional in the existing dev-safeguards. `enforce_dev_safeguard()` (`rust-wallet/src/main.rs`) and `AppPaths::EnforceDevSafeguard()` (`cef-native/include/core/AppPaths.h`) now, when a **non-dev-build** binary sees a stray `HODOS_DEV=1`, scrub it (Rust `env::remove_var`; C++ `_putenv_s`/`unsetenv`) + warn, then proceed in the prod namespace. Runs first in each `main()` before any namespace read, so every downstream read (`app_dir_name`/`wallet_port`/`keychain_service`/`GetAppDirName`/`IsDevEnv`) resolves to prod, and every spawned child (wallet, adblock, CEF subprocs) inherits the scrubbed env — so the shell's scrub alone covers the adblock daemon too (adblock's own `main` not touched). Chosen behavior: **force-prod + warn** (least disruptive; flip to hard-refuse trivially by returning false / exiting instead of scrubbing).

---

## ⛑️ MAC CLAUDE — runtime-verify handoff

This Windows session audited the cross-platform **code** but cannot verify macOS **runtime**. Please:

1. **C2 mac launcher (verify the written fix).** `mac_build_run.sh` now does `pkill -9 -f "$SCRIPT_DIR/build/bin/HodosBrowser.app"` and launches via the absolute bundle path. **Confirm at runtime** that with the installed `/Applications/HodosBrowser.app` running, `./mac_build_run.sh` kills ONLY the dev bundle's processes (main + helpers) and leaves the installed app alive. `pkill -f` matches against full argv — verify argv[0] of the dev processes actually contains `build/bin/HodosBrowser.app` (that's why the launch was switched to the absolute path). Adjust the pattern if CEF helpers spawn with a path that doesn't contain the build dir.

2. **C1 Mode-B mac guard (runtime-verify — code already wired).** The Mode-B scrub lives in the shared header `AppPaths::EnforceDevSafeguard()` (`#else unsetenv` branch) and Rust `enforce_dev_safeguard()`. The mac shell ALREADY calls `EnforceDevSafeguard` at `cef_browser_shell_mac.mm:5213`, before the first `GetAppDirName()` at `:5262` — so the fix auto-applies; no new call needed. **Just runtime-verify:** with `HODOS_DEV=1` exported, launch the **installed** prod `.app` directly → confirm it now warns + uses the **prod** (`HodosBrowser`) namespace + Keychain service `HodosBrowser`, NOT `HodosBrowserDev`, and that the spawned wallet child also lands on prod. (Before this fix it would have opened dev data / the dev Keychain.) The Rust wallet's own scrub covers the standalone `dev-wallet.sh` case. Confirm `unsetenv` on mac clears it for the spawned wallet/adblock children too.

3. **H1 mac Sparkle gate (implement + verify).** Gate `cef_browser_shell_mac.mm:5601-5629` Sparkle init behind `!hodos::IsDevEnv()` (or a dev feed/bundle-id + separate `NSUserDefaults` domain). Verify a dev `.app` no longer checks/stages the public release and no longer rewrites the installed app's `SULastCheckTime`/`SUSkippedVersion`. Adversarial review before landing (auto-update path).

4. **M1 mac debug-port offset (implement + verify).** Mirror the Windows `+100` dev offset at `cef_browser_shell_mac.mm:5412`. Verify dev + prod Default no longer collide on `127.0.0.1:9222`.

5. **Blast-radius confirmation.** Confirm the C1/Keychain contamination class is **dev-machines-only** (devs run dev+prod together, may have `HODOS_DEV` in their env). End-user beta testers never set `HODOS_DEV`, so they should be untouched — confirm no macOS tester needs recovery.

Relay back via `origin/0.4.0` docs (this file or a sibling under `development-docs/`).
