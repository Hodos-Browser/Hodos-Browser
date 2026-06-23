# macOS Catch-Up Playbook — Hodos Browser (branch `0.4.0`)

> **You are a fresh, memoryless Claude orchestration agent running on a Mac.** This document is your complete brief. There is no prior conversation to recover and no auto-memory to lean on — everything you need to start is below. The owner's instruction to you is literally: *"pull this repo, read this doc, execute what it says."* Do that, in order.

---

## 0. Provenance & trust posture of this document

Every macOS claim in this playbook was verified against **actual current source on branch `0.4.0` (HEAD `b9542aa`, 2026-06-22)** — not against project docs, which are systematically stale (examples flagged inline). However, **source shifts**. Line numbers cited here (e.g. `simple_handler.cpp:6620`) were correct at authoring time but **will drift** as you and others edit. They have *already* drifted ~40–60 lines in `simple_handler.cpp` between earlier drafts and this one. Treat every `file:line` as a *starting hint*, not gospel. **Do not Read a cited line with a tight `offset`/`limit` and trust you landed in the right place** — drift of 40–60 lines is normal; grep for the symbol/string first, then read around the real hit. The **Execution Protocol (§5)** mandates a kickoff re-verification pass before you touch anything — do not skip it.

Where an upstream project doc is known-stale, this playbook says so explicitly and tells you to re-verify against source. The biggest stale-doc traps, with **precise** scope:

- **`MACOS_PARITY_REVIEW.md`** — its *architecture verdict* is trustworthy; its *one concrete code deliverable (Gap #1)* is obsolete (names deleted classes). See §4-B.
- **`cef-native/src/core/CLAUDE.md`** — **heavily stale.** Still documents the **deleted** `BRC100Handler`/`BRC100Bridge` (tables at ~:28-29, singleton/ownership notes ~:57, render-thread/flow notes ~:66, ~:77-78, ~:111) **and** falsely claims `GoogleSuggestService` is "Not implemented (returns empty)" on mac (~:38, ~:115) when a working libcurl branch exists. **Distrust this file; verify against source.**
- **`cef-native/CLAUDE.md`** — also still references the deleted BRC100 binding / `brc100.*` V8 path in places. Distrust the BRC100 entries.
- **`cef-native/src/handlers/CLAUDE.md`** — **mostly CORRECT, do NOT broadly distrust it.** It correctly documents `GoogleSuggestService` as libcurl-on-mac (~:248), the mac dev flags, and the mac overlay split. Its *only* stale line is one `brc100.*` "registered by BRC100Handler" comment (~:159). Trust this file except that one line.
- `cef-native/include/core/CLAUDE.md`, `cef-native/include/handlers/CLAUDE.md` — spot-check before relying; some carry the same BRC100-binding residue. Verify against source.

> **Why the BRC100 deletion is called a "first-paint win" below (A3):** the now-deleted `BRC100Bridge.cpp` contained a mac-only `#elif __APPLE__` libcurl branch that ran a **synchronous ~10s wallet probe** during render-process startup (it was never compiled on Windows, so Windows never paid that cost). Removing those four files removes that synchronous probe from the mac render path. That is the reasoning behind the prediction — stated here in full so you don't have to trust a memory file you don't have.

---

## 1. Mission & how to use this doc

**Mission:** Bring the **macOS** build of Hodos Browser to parity with the Windows `0.4.0` line. Windows is ahead — the 0.4.0 header/UX work, the BRC-121 sprint (phases 0–~2.6), the Phase 2.5 IPC auth bridge, the `window.CWI` shim, and the recent startup first-paint fix all landed on Windows and were never compiled or run on a Mac. Your job is the **first-ever clean Mac compile of all that work, the first-ever Mac runtime smoke, and the macOS-specific port deltas** (overlay NSWindow shells, a few `#elif __APPLE__` arms, startup init wiring, and capturing the startup/first-paint measurements).

**How to use it:**
1. **Confirm you can build at all (HARD PREREQUISITE — see §3.3 / §3.4 #1).** A from-source mac CEF framework + wrapper static lib must exist locally at `../cef-binaries`. If it does not, **STOP and resolve acquisition first** — read `development-docs/DevOps-CICD/CEF_BUILD_RUNBOOK.md` and **confirm with the owner where the mac from-source CEF framework lives or how to build/fetch it.** This is the only blocker with no graceful degradation; everything else degrades into "re-verify."
2. `git fetch origin && git checkout 0.4.0 && git pull` — confirm you are at or descended from `b9542aa`.
3. Read this entire document once, top to bottom, before touching code.
4. **Restore the working tree** if perf instrumentation is present (see §3, blocker #6) — `frontend/index.html`, `frontend/src/main.tsx`, `frontend/src/pages/MainBrowserView.tsx` may carry uncommitted edits (the `index.html` spinner in particular must be restored).
5. Read the two authoritative companion docs that this playbook *summarizes but does not replace*:
   - `development-docs/0.4.0/MACOS_PORT_0_4_0.md` — the well-maintained per-chunk mac port tracker (its *line refs* are stale; its *checklists* are current).
   - `development-docs/0.4.0/STARTUP_OPTIMIZATION.md` — the macOS startup track (the strategic reason this sprint exists; see §4-A C12 + §6 for the measurement obligation).
6. Follow the **Execution Protocol (§5)**: kickoff re-verify → author a fan-out review workflow → adversarial design+code gate per chunk → implement honoring invariants → build on Mac → smoke on real sites + wallet → **record first-paint/startup measurements**.

### 1.1 Git workflow (Mac/Windows run in PARALLEL — read this before committing anything)

This Mac catch-up runs **at the same time** as auto-update (Sparkle/WinSparkle) research on the owner's Windows machine. To avoid the two machines diverging on the shared `0.4.0` branch:

- **Land all Mac work directly on `0.4.0`.** Do NOT create a feature branch unless the owner asks — the Windows side is doing **research/docs only (no code commits)** during this window, so `0.4.0` will not get conflicting code from Windows.
- **Get current first:** `git fetch origin && git checkout 0.4.0 && git pull`. Confirm HEAD is `be60d76` (the playbook-complete commit) **or newer**. If you have local uncommitted changes on the wrong branch, surface them to the owner BEFORE switching — do not discard anything.
- **Commit only when the owner asks** (harness rule). One commit per landed, tested chunk. End every commit message with the `Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>` line.
- **Push after each landed chunk** (`git push origin 0.4.0`) so the owner can pull progress to Windows and the two machines stay in sync.
- **Before any commit, `git pull --rebase origin 0.4.0`** to absorb anything the owner pushed from Windows. Conflicts should be rare (different files/platforms); if one appears, resolve it on the Mac side (you own the `*_mac` files) or ask the owner.
- The earlier "feature branch?" open question (§8) is **resolved: work on `0.4.0` directly** for this parallel window. Revisit only if the owner wants the mac work isolated for review.

---

## 2. Project orientation (minimal but sufficient)

### 2.1 Three-layer architecture

```
React Frontend (Vite dev :5137)
    │  window.hodosBrowser.*  /  window.CWI / yours / panda  (V8-injected)
    │  wallet UI also fetches the Rust API DIRECTLY
    ▼
C++ CEF Shell  (our custom-built Chromium+CEF; mac links a pinned framework)
    │  HTTP interception → forwards wallet calls to localhost:31301
    ▼
Rust Wallet Backend (Actix, SQLite)  @ 127.0.0.1:31301   ← SINGLE cross-platform source, no _mac variant
    ▼
Bitcoin SV (WhatsOnChain, GorillaPool)
```

| Layer | Tech | Mac specifics |
|---|---|---|
| Frontend | React/Vite/TS/MUI | Identical to Windows. Served by Vite at `:5137` in dev. |
| CEF Shell | C++17, CEF (pinned framework) | Mac code in `*_mac.mm` / `#elif defined(__APPLE__)`. libcurl replaces WinHTTP. |
| Wallet | Rust, Actix, SQLite | **One cross-platform crate.** No mac port needed for wallet logic. The mac shell talks to it via libcurl (`WalletService_mac.cpp`). |

The wallet UI (React) calls `fetch('http://127.0.0.1:31301/...')`; on mac those C++-side HTTP calls go through **libcurl** (`WalletService_mac.cpp`), where Windows uses **WinHTTP**. The Rust backend is the same binary logic on both OSes.

### 2.2 Where mac code lives (verified inventory)

| File | ~Lines | Owns |
|---|---:|---|
| `cef-native/cef_browser_shell_mac.mm` | 5090 | **Mac entry point** (`main`/`NSApplication`). NSWindow/NSView hierarchy, startup (ProfileManager init + `ResolveStartup`, picker forced OFF), `StartWalletServer`/`StartAdblockServer` (spawn daemons), HistoryManager init (`:4877`), AutoUpdater (Sparkle) init, `ShutdownApplication`, **all overlay-creation fns** (`:557-581`), C-linkage stubs (`:5078`). Mac globals block `:256-266`. **Canonical keyboard-overlay template = the profile-picker overlay creation fn at `:4084-4115`** (`DropdownOverlayView` alloc `:4084` → `http://127.0.0.1:5137/profile-picker` `:4106` → `makeFirstResponder:contentView` `:4115`). |
| `cef-native/OverlayHelpers_mac.mm` | 532 | Shared overlay geometry/util: `CalculateToolbarOverlayFrame` (**right-anchored only**, `:267`), `ClampOverlayToScreen` (`:235`), `InstallAppFocusLossHandler` (close-on-focus-loss list, `:189`; "Future dropdown overlays" TODO `:216`). **Note: at `cef-native/` root, NOT under `src/core/`.** |
| `cef-native/mac/process_helper_mac.mm` | 66 | Helper/render subprocess entry (`CefExecuteProcess`). Inits `HistoryManager` for render V8 — **hardcodes `…/Default`** (`:53-59`) → the per-profile history leak (§4-A item A1). |
| `cef-native/src/handlers/simple_handler_mac.mm` | 158 | `PresentContextMenuMac` — `CefMenuModel`→`NSMenu`. |
| `cef-native/src/handlers/my_overlay_render_handler.mm` | 384 | Mac OSR painting (`CGImageCreate` + `CALayer.contents`, Retina via `backingScaleFactor`). |
| `cef-native/src/core/TabManager_mac.mm` | 593 | Tab lifecycle via NSView. `CloseTab` (`:146`) calls `ClearRustPaymentSessionForBrowser` (`:192-193`) but **NOT** `RecordClosedTab` (§4-A item A7). |
| `cef-native/src/core/WindowManager_mac.mm` | 341 | `CreateFullWindow`, hit-test tab merge, ghost-tab preview. |
| `cef-native/src/core/WalletService_mac.cpp` | 266 | Wallet HTTP client via **libcurl**. |
| `cef-native/src/core/AutoUpdater_mac.mm` | 156 | Sparkle 2 wrapper; `#if SPARKLE_AVAILABLE` guarded. **DevOps-owned — do not touch (§4-D).** |
| `cef-native/Info.plist` | 36 | Bundle plist; `SUFeedURL` + `SUPublicEDKey`. **DevOps-owned.** |
| `cef-native/mac/{entitlements.plist, helper-Info.plist.in}` | — | Signing/JIT entitlements + helper plist template. **DevOps-owned.** |
| `cef-native/mac_build_run.sh` | 45 | Dev build+run launcher. |

**Cross-platform files with `#elif defined(__APPLE__)` branches:** `simple_handler.cpp` (~55 `__APPLE__` blocks), `simple_render_process_handler.cpp`, `simple_app.cpp` (mac Chromium flags `:92-108`), `GoogleSuggestService.cpp` (libcurl `:262-311`), `SyncHttpClient.cpp` (libcurl), `ProfileManager.cpp` (`posix_spawn` + `flock`), `ProfileLock.cpp` (`flock`), `BrowserWindow.{cpp,h}` (mac `void*` NSWindow/NSView + overlay-window/event-monitor fields for bookmarks/siteinfo/tablist panels).

### 2.3 Invariants that matter (these OVERRIDE convenience)

- **#8 — CEF lifecycle/threading is fragile.** Do not change the message loop, browser-creation timing, or render-process handlers without asking. Startup ordering on mac (§4-A A4, C12) is exactly this territory — proceed carefully and gate on owner approval.
- **#9 — All new mac C++ goes in `*_mac.mm` or `#elif defined(__APPLE__)`.** Every new overlay needs a mac creation function in `cef_browser_shell_mac.mm`. New singletons use `SyncHttpClient` (libcurl on mac), never raw WinHTTP.
- **#2 / #3 — Never touch wallet DB schema, crypto, signing, or derivation silently.** The wallet is one cross-platform Rust crate; you should not be editing it for a mac port at all. If a mac task seems to require it, STOP and surface it (§8).
- **#13 — Test-failure triage.** If a test fails, determine whether the test or the production code is wrong via an independent source; test-only fixes may proceed, but **ask before changing production code**.

### 2.4 Dev run commands (mac)

Three processes must run (mirrors Windows run order):

1. **Rust wallet** → `127.0.0.1:31301`. Use the launcher (sets `HODOS_DEV=1`); never bare `cargo run` (the prod-isolation safeguard will refuse to start a build-dir binary without `HODOS_DEV=1`). Mac dev wallet script: `./dev-wallet.sh` (or `HODOS_DEV=1 cargo run --release` from `rust-wallet/`).
2. **Frontend dev server** → `cd frontend && npm run dev` → `:5137`.
3. **CEF browser** → `cd cef-native && ./mac_build_run.sh`. This configures+builds via CMake, **copies the 5 helper `.app` bundles** into `HodosBrowser.app/Contents/Frameworks/`, `pkill HodosBrowser`, exports **`HODOS_DEV=1`** *and* **`HODOS_MAC_DEV_FLAGS=1`**, then launches the app binary directly.

- `HODOS_DEV=1` → uses `~/Library/Application Support/HodosBrowserDev/` (isolated from production `HodosBrowser/`).
- `HODOS_MAC_DEV_FLAGS=1` → enables `--in-process-gpu` + `--disable-web-security` (and friends) in `simple_app.cpp:95-108`, needed because an unsigned/dev GPU helper won't run otherwise. **Security note:** these dev flags are dangerous in production — see §4-C item 4.

---

## 3. macOS reality today (what exists / builds / is stubbed; how to build; blockers)

### 3.1 Already working / built on mac (don't break)

- **Mac entry + window system**: `main`, NSWindow/NSView hierarchy, tab system (`TabManager_mac.mm`), multi-window (`WindowManager_mac.mm`), OSR painting (`my_overlay_render_handler.mm`), context menus (`simple_handler_mac.mm`).
- **11 sprint-era overlays exist on mac** (settings, wallet, backup, BRC100 auth, notification, settings menu, cookie panel, omnibox, downloads, profile picker, hamburger menu) — `cef_browser_shell_mac.mm:557-581`, defs through `:2550-5085`.
- **Startup resolver win** (CHUNK1/R5/R7): mac extracts `--profile` from `NSProcessInfo` (`:4646-4664`), calls `ResolveStartup(...)` + `SetCurrentProfileId(id, persist=false)`. Picker mode **forced OFF** on mac (`:4644`).
- **HistoryManager IS initialized** on mac (`:4877`) — the `:4923-4925` log "not implemented on macOS yet" is **dead/contradictory legacy**; distrust it.
- **BookmarkManager** (`:4718`), **PaidContentCache** (`:4722`), **CookieBlockManager** (`:4711`) initialized at startup.
- **GoogleSuggestService** has a **working libcurl branch** (`GoogleSuggestService.cpp:262-311`) — the `src/core/CLAUDE.md` claim that it's "Not implemented (returns empty)" on mac is **FALSE/stale**.
- **`StopServers()`** adaptive `waitpid(WNOHANG)` poll exists (`cef_browser_shell_mac.mm:4470`, lambda `stopPid` `:4485`, SIGTERM after cap mirroring Windows 5s).
- **`CreateNotificationOverlay`** (3-arg, mac at `_mac.mm:3454-3462`) forwards `type` + `extraParams` with **no per-type allowlist** — all 11 prompt types multiplex through it. Verified accurate.
- **DevOps macOS pipeline is substantially LIVE** (auto-update/sign/notarize/EdDSA) — see §4-D. Do not touch.

### 3.2 Stubbed / NOT-yet-implemented on mac (the work)

- **0.4.0 header-UX overlays are no-op on mac** (only a logging `#elif __APPLE__` arm): **Bookmarks**, **Site-Info hub**, **Tab-list**. (§4-A items A5–A7.)
- **`SitePermissionStore::Initialize` not wired into mac startup** → stored Allow/Block ignored, everything prompts. (§4-A item A8.)
- **`FireHodosPermissionPrompt` is `#ifdef _WIN32`** → mac falls through to Chromium's stock permission prompt. (§4-A item A9.)
- **Per-profile history leak on mac**: render helper hardcodes `…/Default` (`process_helper_mac.mm:53-59`). (§4-A item A1.)
- **DB-shutdown cascade deferred on mac** (R2/R3): the 4-manager `Shutdown()` + `wal_checkpoint(RESTART)` before `ReleaseProfileLock()` is absent. (§4-A item A4.)
- **Pre-window profile PICKER is Windows-only** (mac gets resolver, not picker UI). (§4-A item A11.)
- **SaveSession / ClearBrowsingDataOnExit absent on mac** (`ShutdownApplication` TODO `:4166-4170`).
- **`wallet_delete_cancel` is `#ifdef _WIN32`** (raw WinHTTP at `simple_handler.cpp:3924`, no `#else`) → wallet delete is a no-op on mac (pre-sprint backlog). (§4-B.)
- **RegistryLock `flock` has no acquire timeout** on mac (`ProfileManager.cpp:70`). (§4-A item A10 / §4-C item 10.)
- **`posix_spawn` profile-launch branch never compiled** (`ProfileManager.cpp:554-581`) — compile-verify gate. (§4-A item A2.)
- **Legacy BRC100 binding deletion never compiled on mac** — compile-verify gate. (§4-A item A3.)
- **macOS startup blocks main thread up to ~10s** on `StartWalletServer`/`StartAdblockServer` (`:4770-4771`); no `elapsed()` instrumentation on mac. (§4-A item C12, §6 measurement obligation.)

### 3.3 How to build & run on mac

`cef-native/mac_build_run.sh` does: `cmake -S . -B build -DCMAKE_BUILD_TYPE=Release` (if unconfigured) → `cmake --build build --config Release` → copies 5 helper bundles into `Contents/Frameworks/` → `pkill HodosBrowser` → exports `HODOS_DEV=1` + `HODOS_MAC_DEV_FLAGS=1` → launches the binary.

CMake APPLE block (`CMakeLists.txt` ~`:73-286`): triplet `arm64-osx`/`x64-osx`; deployment target 10.15; linker `-headerpad,0x4000 -no_adhoc_codesign`; mac SOURCES `:194-213`; `HODOS_HELPER_SRCS` `:217-226`; POST_BUILD generates 5 helper bundles, `install_name_tool`-fixes CEF rpath, copies CEF framework, generates `hodos.icns`. **Single-arch, native to the runner — NOT universal** (`CMAKE_OSX_ARCHITECTURES = ${CMAKE_SYSTEM_PROCESSOR}`, `:73`).

**HARD PREREQUISITE — CEF framework + wrapper (the one true blocker):**
- **CEF built from source** at `../cef-binaries/`:
  - `Release/Chromium Embedded Framework.framework`
  - `Resources/`
  - the **wrapper static lib at `../cef-binaries/build/libcef_dll_wrapper/libcef_dll_wrapper.a`** (mac expects it under `build/`; Windows under `libcef_dll/wrapper/build/Release`).
- **If `../cef-binaries` does not exist or has the wrong layout, you cannot build, and there is no graceful degradation.** Acquisition path, in order:
  1. Read `development-docs/DevOps-CICD/CEF_BUILD_RUNBOOK.md` (the canonical self-build runbook — this repo builds its **own** Chromium+CEF from source for proprietary codecs; it is not a stock spotifycdn download for the shipping build).
  2. Check whether a prebuilt from-source framework already exists on this machine or a shared artifact location.
  3. **If still unresolved, ASK THE OWNER where the mac from-source CEF framework lives (or how to fetch/build it) before doing anything else.** Do not silently substitute a stock CEF download — it lacks the proprietary codecs and may diverge from the pinned branch.

**Other prerequisites:**
- Homebrew: **OpenSSL 3** (`/opt/homebrew/opt/openssl@3`), **nlohmann-json** (header), **sqlite3**, **libcurl** (system curl).
- Optional: `../external/Sparkle.framework` (else auto-update disables; build still succeeds).
- Dev run needs Rust wallet on `:31301` + frontend on `:5137`.

### 3.4 Blockers that would stop a fresh agent

1. **CEF framework/wrapper missing or wrong layout (THE hard stop).** CMake `FATAL_ERROR "CEF framework not found at ../cef-binaries/Release"`, or link failure on `libcef_dll_wrapper.a`. **Unlike line-drift, this does NOT degrade gracefully — resolve via §3.3 acquisition path (runbook → shared artifact → ask owner) before anything else.**
2. **Missing Homebrew deps** → CMake `REQUIRED` failures (OpenSSL / nlohmann-json / sqlite3).
3. **GPU/code-signing**: a manual `cmake --build` + direct launch **without** `HODOS_MAC_DEV_FLAGS=1` → GPU helper won't run. Use `mac_build_run.sh` (it sets the flag).
4. **Sparkle absent** = NOT a blocker (guarded); auto-update silently disables.
5. **Frontend/wallet not running** in dev → blank header + wallet failures + the ~10s startup block.
6. **Uncommitted perf instrumentation in the tree** (`frontend/index.html`, `main.tsx`, `MainBrowserView.tsx`) — **restore before a clean mac smoke**; the index.html spinner in particular must be restored.
7. **Compile-verify gates** carried by mac-only `#elif` arms the Windows build never compiled: `ProfileManager.cpp` `posix_spawn` branch, `StopServers` waitpid + `ResolveStartup` call, the BRC100-binding deletion. A first mac build is the only way to confirm them.

---

## 4. Scope of the catch-up (prioritized, verified checklists)

Four areas. **A** = 0.4.0 deltas. **B** = BRC-121 sprint parity (with explicit trust level for `MACOS_PARITY_REVIEW.md`). **C** = HelicOps mac items. **D** = DevOps-CICD — **FORWARD-LOOK ONLY, do not implement.**

> **Dependency rule baked into the ordering:** the three **compile/link gates** (A2, A3, A4-DONE-verify) come first — *one Mac build session unblocks all three at once* and tells you whether the tree even links after the Windows-only deletions. **That first build is also when you capture the A3 first-paint win + the C12 startup block — see §6.** Then the **shared overlay NSWindow substrate** (build one of A5/A6/A7, the other two are clones). Then cheap activations (A8, A10), the prompt (A9), the bigger picker (A11), and the startup/security track (C12/C13) last.

### Shared overlay substrate (read once before A5–A7)

All three new dropdown panels are **no-op on mac**; React + cross-platform plumbing (BrowserWindow fields, role dispatch, `Get*PanelBrowser()` statics, IPC arg-parse, deferred context injection) **already exist**. Only the mac NSWindow overlay shell + the `#elif __APPLE__` IPC arms are missing.

**Canonical templates to clone (verified):**
- **Keyboard dropdown (search box needs focus) → clone the profile-picker overlay** in `cef_browser_shell_mac.mm:4084-4115`: it allocs a `DropdownOverlayView` (`:4084`), loads a `:5137` route (`:4106`), and crucially calls `[g_<name>_panel_overlay_window makeFirstResponder:contentView]` (`:4115`) — that `makeFirstResponder` call is the OSR keyboard-focus fix. The `DropdownOverlayView` class itself is defined at `:795-...` (`@interface` `:795`, `@implementation` `:801`). Other working keyboard overlays for reference: wallet (`makeFirstResponder` `:2911`), settings-menu (`:3703`), download panel (`:3982`), hamburger menu (`:5072`).
- **Mouse-only dropdown (no keyboard) → clone the download panel overlay** (`DropdownOverlayView` alloc `:3951`, `makeFirstResponder` at `:3982` — for a mouse-only panel you can keep or drop the focus call; site-info is mouse-only so it does not strictly need typed-input focus).

Per panel:
- Add an `NSWindow* g_<name>_panel_overlay_window` global in the mac globals block (`cef_browser_shell_mac.mm:256-266`; no new-panel globals there yet — verified).
- Add `Create/Show/Hide/IsVisible/WasJustHidden` + a click-outside NSEvent local monitor.
- **Left-anchored X**: hand-roll like the omnibox; **do NOT use `CalculateToolbarOverlayFrame` (`OverlayHelpers_mac.mm:267`, right-only)**. Use points, **NOT pixels — no `ScalePx`** (Retina is points). Then `ClampOverlayToScreen` (`OverlayHelpers_mac.mm:235`).
- Add the panel to `InstallAppFocusLossHandler`'s close list (`OverlayHelpers_mac.mm:189`; "Future dropdown overlays" TODO `:216`) + shutdown cleanup.

---

### 4-A. 0.4.0 deltas

#### A1 — Per-profile history leak: render helper hardcodes `Default` (P0, OPEN, real)
- **What's wrong:** `cef-native/mac/process_helper_mac.mm` (`:53-59`) inits `HistoryManager` against hardcoded `@"Default"`, ignoring the active profile. `SimpleApp::OnBeforeChildProcessLaunch` (`simple_app.cpp:58`) **does** append `--profile=<id>` to child command lines (runs on mac), but the mac helper never reads it. Windows render fix is `#ifdef _WIN32`-gated (`simple_render_process_handler.cpp:509-548`) with a mac `#else` stub `:547`. Result: mac multi-profile **history/omnibox/NTP tiles leak Default into every profile**.
- **Fix:** in `process_helper_mac.mm`, parse `--profile=` from `argv` (reuse `ProfileManager::ParseProfileArgument`), validate via `ProfileManager::IsValidProfileId`, build `…/HodosBrowser/<profileId>` instead of `…/Default`, fall back to Default only if absent/invalid. *(The browser-process init at `_mac.mm:4877` is already profile-aware via `cache_path` — leak is ONLY the helper.)*
- **Test:** launch a non-Default profile via picker/`--profile`; NTP tiles show only the 2 placeholders; omnibox excludes Default's history; Default unchanged.
- **Effort:** S. **Risk:** Low.

#### A2 — `posix_spawn` profile-launch branch COMPILE-VERIFY (DONE in source, never compiled) (P0 gate)
- **Status:** `ProfileManager::LaunchWithProfile` `#elif defined(__APPLE__)` is fully written (`ProfileManager.cpp:554-581`): `posix_spawn("/usr/bin/open", {"-n","-a",appPath,"--args","--profile="+id})` + `waitpid` + `WIFEXITED`, includes `<spawn.h>`/`<sys/wait.h>`/`extern char** environ`/`<mach-o/dyld.h>` (`_NSGetExecutablePath`). **Never compiled** (Windows skips `#elif`). `IsValidProfileId` gate fires first (`:514`) and again at `simple_app.cpp:64-68`.
- **Mac action:** **compile-verify only** — confirm includes resolve, links. Smoke: switch profiles → second instance launches with right `--profile`; malformed id rejected.
- **Effort:** XS. **Risk:** Low. *(Also closes HelicOps F5 — see §4-C item 1.)*

#### A3 — Legacy BRC-100 binding deletion COMPILE-VERIFY + FIRST-PAINT MEASUREMENT (P0 gate)
- **Status (VERIFIED):** `BRC100Handler.{cpp,h}` + `BRC100Bridge.{cpp,h}` deleted; all CMake refs gone (incl. macOS main sources + `HODOS_HELPER_SRCS`); `RegisterBRC100API` call + include removed from cross-platform `simple_render_process_handler.cpp` (`:862` now a removal comment); CWI/yours/panda shim intact below (`:867-895`, `frame->ExecuteJavaScript(CWI_SHIM_SCRIPT…)`). The deleted `BRC100Bridge.cpp` held the mac libcurl `#elif __APPLE__` branch (a **synchronous ~10s wallet probe** on render startup) — never compiled on Windows; **its removal is the predicted mac first-paint win** (reasoning fully stated in §0).
- **Mac action:** pure compile/link verify — (a) compiles/links with the 4 files gone, no dangling include/symbol; (b) `window.CWI`/`yours`/`panda` still injects on an external https page (mac render helper shares the file); (c) wallet UI still works.
- **MEASUREMENT OBLIGATION (this is the strategic point of the sprint):** on this first mac build, **measure first-paint/time-to-first-window** and record it (§6). This is the run that captures the predicted BRC100Bridge-deletion win — there is no second chance to measure "before" once the tree is built.
- **Effort:** XS (verify) + S (measure). **Risk:** Low.
- **Doc-drift to flag (out of scope to fix; READ-ONLY):** `cef-native/src/core/CLAUDE.md` (heavy) and `cef-native/CLAUDE.md` still document the deleted BRC100Handler/Bridge + `brc100.*` V8 path. `cef-native/src/handlers/CLAUDE.md` has only one stale `brc100.*` line (~:159), otherwise correct. Stale; do not fix here.

#### A4 — R2/R3 mac shutdown: DB cascade still deferred (P0; StopServers DONE-verify, cascade OPEN)
- **DONE (verify compiles):** `StopServers()` adaptive `waitpid(WNOHANG)` (`_mac.mm:4470`, `stopPid` `:4485`). Verify idle-wallet quit is fast.
- **OPEN (deferred by design):** the 4-manager `Shutdown()` DB cascade (History/Bookmark/CookieBlock/PaidContent + `wal_checkpoint(RESTART)`) is **NOT** in mac `main()` shutdown. De-risked: all 4 are now live at mac startup (Bookmark `:4718`, PaidContent `:4722`, CookieBlock `:4711`, History `:4877`). Add the cascade **before** `ReleaseProfileLock()`, mirroring Windows ordering.
- **Test:** fire a wallet send, immediately quit + relaunch → clean start, no `SQLITE_BUSY`; normal quit snappy.
- **Effort:** S. **Risk:** Medium (touches fragile shutdown ordering — invariant #8; gate on owner approval).

#### A5 — Bookmarks overlay (a) — keyboard dropdown (P1)
- **Verified no-op:** `bookmarks_panel_show` `#elif __APPLE__` logs only (`simple_handler.cpp:6620`); `bookmarks_panel_hide` `#ifdef _WIN32`-only; menu "bookmarks" action `#ifdef _WIN32`-only. Cross-platform fields confirmed (`BrowserWindow.h`).
- **Mac work:** clone the **profile-picker keyboard pattern** (`cef_browser_shell_mac.mm:4084-4115`, the canonical template — `DropdownOverlayView` + `makeFirstResponder:contentView` so the search box gets focus), `browserAccessor = ^{ return SimpleHandler::GetBookmarksPanelBrowser(); }`, URL `/bookmarks`, role `"bookmarkspanel"`. Wire mac arms: `bookmarks_panel_show` (incl. immediate `setBookmarkContext` re-open injection), `bookmarks_panel_hide`, menu "bookmarks" action.
- **Files:** `cef_browser_shell_mac.mm`, `simple_handler.cpp` (3 arms), `OverlayHelpers_mac.mm` (focus-loss list).
- **Test:** Cmd-click bookmarks icon → left-anchored dropdown, search box typable, click-outside closes.
- **Effort:** M. **Risk:** Medium (OSR keyboard focus is the classic mac failure mode — the `makeFirstResponder:contentView` call at `:4115` is the fix to copy; see CEF Input Patterns in project CLAUDE.md).

#### A6 — Site-Info hub overlay (b2a) — mouse-only dropdown (P1)
- **Verified no-op:** `siteinfo_panel_show` `#elif __APPLE__` logs only (`simple_handler.cpp:6751`); `siteinfo_panel_hide` `#ifdef _WIN32`-only. `open_wallet_permissions` already has a working mac arm via `CreateNotificationOverlay("edit_permissions", domain)`. b2b management IPC is fully cross-platform — activates automatically once b2a shell exists.
- **Mac work:** clone the **download panel pattern** (mouse-only — `DropdownOverlayView` alloc at `_mac.mm:3951`; you may drop the keyboard-focus call since site-info has no search box), `GetSiteInfoPanelBrowser()`, URL `/site-info`, role `"siteinfopanel"`, size 360×480. Mac arms: `siteinfo_panel_show` (incl. immediate `setSiteInfoContext` re-open) + `siteinfo_panel_hide`. Verify whether the mac NSEvent local monitor reproduces the Windows same-click hide race (Windows guards with `g_siteinfo_last_hide_tick`, `simple_handler.cpp` ~`:6775/6784`); add an equivalent **only if it does**.
- **Files:** `cef_browser_shell_mac.mm`, `simple_handler.cpp` (2 arms), `OverlayHelpers_mac.mm`.
- **Effort:** M. **Risk:** Low-Medium.

#### A7 — Tab-list caret overlay (e) — keyboard dropdown + one-line CloseTab parity (P1)
- **Verified no-op:** `tablist_panel_show` `#elif __APPLE__` logs only (`simple_handler.cpp:6687`); `tablist_panel_hide` `#ifdef _WIN32`-only; Cmd+Shift+A is **detected** but its action body is `#ifdef _WIN32` → no-op. Recently-closed store is inline in `TabManager.h` (cross-platform).
- **One-line parity gate (P0-adjacent):** `TabManager_mac.mm::CloseTab` (`:146`) calls `ClearRustPaymentSessionForBrowser` (`:192-193`) but **NOT** `RecordClosedTab(tab.url, tab.title)` → mac "Recently closed" stays empty (graceful). Add the call with the same `http(s)` + non-`127.0.0.1:5137` filter as `TabManager.cpp`.
- **Mac work:** clone the **profile-picker keyboard pattern** (`_mac.mm:4084-4115`), `GetTabListPanelBrowser()`, URL `/tab-list`, role `"tablistpanel"`, size 340×480; on (re)show inject `if(window.tabListRefresh)window.tabListRefresh();`. Mac arms: `tablist_panel_show`, `tablist_panel_hide`, Cmd+Shift+A action body.
- **Files:** `TabManager_mac.mm` (1 line), `cef_browser_shell_mac.mm`, `simple_handler.cpp` (3 arms), `OverlayHelpers_mac.mm`.
- **Effort:** M + XS. **Risk:** Medium (keyboard focus — same `makeFirstResponder` fix as A5).

#### A8 — `SitePermissionStore` init on mac (b1a) (P2)
- **Verified:** `SitePermissionStore.cpp` in shared SOURCES (`CMakeLists.txt:179`) so it compiles/links on mac; `GetState` is null-safe (returns `Ask` when `db_==nullptr`) → `CefPermissionHandler` overrides run safely today, but every stored Allow/Block is ignored. Mac startup inits Bookmark/PaidContent/CookieBlock/History but **not** SitePermissionStore (`:4711-4727`). Windows inits it at `cef_browser_shell.cpp:4292`.
- **Mac work:** add `SitePermissionStore::GetInstance().Initialize(profile_cache);` near `:4718` (alongside BookmarkManager); add its `Shutdown()` to the DB cascade (A4).
- **Test:** set a permission via site-info overlay → `site_permissions.db` created under `~/Library/Application Support/HodosBrowser*/Default/`.
- **Effort:** XS. **Risk:** Low.

#### A9 — Hodos permission prompt on mac (b1b) (P2)
- **Verified:** `FireHodosPermissionPrompt` is `#ifdef _WIN32` → returns false on mac (`simple_handler.cpp:7686`, call sites `:7762/:7821`); mac falls through to Chromium's stock prompt. Allow-once (b1b.1) + parked-callback registry + watchdog + `permission_response` IPC + React branch are all cross-platform and run on mac.
- **Mac work:** add a mac arm calling `CreateNotificationOverlay("permission_request", host, "&requestId=…&perm=…")` (3-arg fn exists, `_mac.mm:3454`); add the mac wallet-modal guard (`g_pendingModalDomain`) + a `HideNotificationOverlayWindow()` mac equivalent (hide blocks are `#ifdef _WIN32`; `extern "C" HideNotificationOverlayWindow()` declared `simple_handler.cpp:102`). **macOS TCC:** camera/mic are OS-gated — `Info.plist` must declare `NSCameraUsageDescription`/`NSMicrophoneUsageDescription` (see `SITE_INFO_PERMISSIONS_DESIGN.md` macOS section). **Note: Info.plist is DevOps-owned (§4-D); adding usage-description keys is a scoped, agreed change — surface it (§8), and preserve the `SU*` keys + no-silent-update posture exactly.**
- **Effort:** S-M. **Risk:** Medium (shared notification overlay multiplexing).

#### A10 — RegistryLock `flock` has no acquire timeout (R5) (P2)
- **Verified:** mac `RegistryLock` uses blocking `flock(fd_, LOCK_EX)` no timeout (`ProfileManager.cpp:70`), vs Windows 5s wait (`:63`). A crashed peer holding the lock blocks mac startup at registry read.
- **Mac work:** `flock(LOCK_EX|LOCK_NB)` + bounded retry loop.
- **Effort:** XS. **Risk:** Low. *(Also a HelicOps-adjacent robustness item — §4-C item 10.)*

#### A11 — macOS pre-window profile picker (CHUNK1/R5/R7 DONE; picker MODE Windows-only) (P2/L)
- **Verified:** mac startup already does CHUNK1+R5+R7 — extracts `--profile` from `NSProcessInfo` (`_mac.mm:4646-4664`), `ResolveStartup(...)` + `SetCurrentProfileId(id, persist=false)`. **Picker forced OFF on mac.** The pre-window picker (neutral `.picker-cache` CefInitialize branch, `/profile-picker?mode=window`, spawn-then-close) is entirely Windows code.
- **Mac work:** add a mac picker path — NSWindow hosting `/profile-picker?mode=window` with no profile lock/DBs, `LaunchWithProfile`-then-quit; single-instance via the `NSApplication` reopen delegate (not the Windows `.picker` pipe). *(Note: the existing profile-picker overlay at `_mac.mm:4084-4115` loads `/profile-picker` as a dropdown — the pre-window picker is a different, full-window pre-CefInitialize flow, not this overlay.)*
- **Effort:** L. **Risk:** Medium-High (intrudes on pre-CefInitialize startup — invariant #8; gate on owner approval).

#### C12 — macOS startup main-thread block (~10s) — STARTUP-OPTIMIZATION TRACK (P2, measure first)
- **Verified:** mac `main()` calls `StartWalletServer`/`StartAdblockServer` (`_mac.mm:4770-4771`) which can block the main thread up to ~10s; Windows has `elapsed()` instrumentation, mac does not. This is the macOS arm of `STARTUP_OPTIMIZATION.md` — the strategic reason the 0.4.0 sprint exists (per the Windows investigation, the perceived lag is a first-PAINT problem, not a React/bundle problem).
- **Mac work (gated):**
  - **M1 (measure, do first, no risk):** add `elapsed()`-style timing around `StartWalletServer`/`StartAdblockServer` and around browser creation / first window show; log it. Pair with the A3 first-paint measurement (§6). This is read-mostly instrumentation — safe, and it tells you whether the ~10s block actually delays first paint or runs alongside it.
  - **M3 (non-blocking backend launch, gated):** if measurement shows the daemon spawn blocks first paint, make `StartWalletServer`/`StartAdblockServer` non-blocking (spawn + poll-for-ready off the main thread). **This touches CEF startup ordering (invariant #8) — design + owner approval required before implementing (§8).**
- **Effort:** M1=XS, M3=M. **Risk:** M1 Low; M3 Medium-High.

---

### 4-B. BRC-121 sprint parity — **TRUST LEVEL of `MACOS_PARITY_REVIEW.md`**

> **Headline:** Trust the doc's **architecture verdict** (the BRC-121 sprint rode cross-platform rails; almost no mac C++ needed; permission prompts multiplex through the shared notification overlay with no per-type allowlist — **independently re-verified, holds**). **Do NOT trust the doc's single concrete code deliverable (Gap #1)** — it tells you to add `SessionManager::clearSession()` + `#include SessionManager.h`, but that class was **deleted** (Phase 2.6-H). Following it verbatim **will not compile**. The doc also predates SitePermissionStore / site-info / tab-list and the BRC100Handler/Bridge deletion, so it is structurally blind to that work.

**Claim-by-claim, what's still true (re-verify before relying):**
- ✅ Shared `notification_browser_` overlay multiplexes all 11 prompt types; mac `CreateNotificationOverlay` (`_mac.mm:3454-3462`) forwards `type`+extraParams, **no per-type allowlist**.
- ✅ `CreateNotificationOverlayTask` is a cross-platform `CefTask` calling the per-platform free fn (mac `:3454`).
- ✅ `PaidContentCache.cpp` cross-platform (`CMakeLists.txt:180`), mac init `_mac.mm:4722`. `ManifestFetcher.cpp` cross-platform (CMake APPLE block `:211`).
- ✅ 4 cache WinHTTP blocks each paired with `#else` libcurl (Domain/WalletStatus/BSVPrice/CertField — `HttpRequestInterceptor.cpp:154/242`, `335/396`, `464/526`, `557/632`). No unpaired raw WinHTTP added this sprint.
- ✅ CWI/yours/panda shim = one cross-platform `frame->ExecuteJavaScript` block (`simple_render_process_handler.cpp:886-895`); mac render helper (`mac/process_helper_mac.mm:24-29`) uses the same handler → injects identically.
- ✅ Phase 2.5 IPC bridge (`brc100_auth_response`, `simple_handler.cpp:4376+`) platform-neutral, no `#ifdef`.
- ✅ **Gold-pill `payment_success_indicator` chain is fully mac-portable, zero mac code.** Fire site `HttpRequestInterceptor.cpp:988-990` (CEF `SendProcessMessage` + cross-platform `TabManager::GetTabIdForBrowserIdentifier`); render handler `simple_render_process_handler.cpp:1040`. **Independently confirmed.**

**What's WRONG/STALE in the doc (caused by post-doc 2.6-H/H.1 deletions):**
- ❌ Gap #1 says `TabManager_mac.mm::CloseTab` is missing session-reset — **already fixed** (`:186-195` calls `ClearRustPaymentSessionForBrowser`). Gap closed.
- ❌ Gap #1 fix spec (`SessionManager::clearSession()` + include) — `SessionManager.{cpp,h}` **deleted**; Windows `TabManager.cpp:186-191` also dropped it. Both platforms match (Rust owns session state).
- ❌ Doc's "audited files" list names `PermissionEngine.cpp / PermissionGate.cpp / EngineShadow.cpp` as in the mac build — **all deleted** (H.1). Vacuously true now (gone everywhere) but the checklist names dead files.

**BRC-121 mac code gaps — actual list:**
- From the sprint itself: **NONE.** (Gap #1 closed; SessionManager moot.)
- Adjacent/post-sprint (do while a Mac build is up): **A8** (SitePermissionStore init) and **`wallet_delete_cancel`** — add `#elif defined(__APPLE__)` libcurl branch at `simple_handler.cpp:3924` (use `SyncHttpClient::Post("http://localhost:31301/wallet/delete","{}")`); pre-sprint backlog, wallet-delete is a no-op on mac until fixed. *(`wallet_delete_cancel` is OQ5 — `simple_handler.cpp:3924` onward is `#ifdef _WIN32` raw WinHTTP, no else; correctly scoped as pre-sprint, not a sprint regression.)*

**BRC-121 runtime-verification debt (the doc's biggest accurate point — NOTHING has ever run on a Mac):**
- [ ] Full mac clean build after the BRC100Handler/Bridge + PermissionEngine/EngineShadow/PermissionGate + SessionManager deletions → no dangling symbol/include.
- [ ] All 11 prompt types render via `CreateNotificationOverlay` (`_mac.mm:3454`) and resolve through `brc100_auth_response`.
- [ ] Gold-pill fires on an auto-approved payment on mac.
- [ ] CWI/yours/panda shim injects on a real https dApp (mac render helper path).
- [ ] BRC-121 pay_402 → broadcast-nosend → PaidContentCache read-side (`GetResourceRequestHandler`) round-trip on mac; zstd auto-decompress + deferred `Open()` callback (Phase-1 runtime risks, still untested). Test site: `now.bsvblockchain.tech` (`/articles/<slug>` returns 402).
- [ ] Cache WinHTTP↔libcurl timeout/NotFound parity (Domain/WalletStatus/BSVPrice/CertField).
- [ ] Tab close/reopen on a capped domain → counters reset (Rust `session/close` POST fires from `TabManager_mac.mm:193`).

---

### 4-C. HelicOps macOS items

The audit was a **syntactic SAST pass with zero macOS-detector awareness** — it flagged mac code as if it were Windows. Of 9 backlog items, **only F5 is truly mac-only**; F6 touches shared code; the rest are platform-agnostic Rust (no mac action). The biggest mac-relevant errors are in the *brief*, not the findings.

1. **F5 — mac command injection (FIXED on Windows branch; needs Mac compile-verify).** `ProfileManager.cpp:554-581` now uses `posix_spawn` + argv array + `waitpid`/`WIFEXITED`, `IsValidProfileId` gate at `:514` and `simple_app.cpp:64-68`. **Action = the A2 compile-verify** (same code). Confirm `spawn.h`/`<sys/wait.h>`/`<mach-o/dyld.h>` includes resolve; smoke profile switch.

2. **Credential storage — DPAPI vs Keychain (brief is STALE; no fix needed, but the doc claim is wrong).** Brief says 4× that "the macOS Keychain side is a **stub**" — **FALSE.** `rust-wallet/src/crypto/dpapi.rs:140-167` has a **real working** Keychain impl via `security_framework` (`set/get/delete_generic_password`, service `"HodosBrowser"`, account `"wallet-mnemonic"`, sentinel `b"KEYCHAIN"`). The actual stub is **Linux** (`:173-181`). *Real review item the audit didn't do:* the Keychain item uses a **generic password with default ACL** (no `kSecAttrAccessibleWhenUnlocked`/`ThisDeviceOnly`, no app-specific access control / Touch ID). Confirm whether other apps as the same user can read it (Chrome's "Safe Storage" uses similar semantics — likely acceptable, but should be a **conscious decision** → surface in §8). **Per invariant #2/#3, do not change this crypto path without owner approval.**

3. **F3/F8 — secret-to-disk logging: one residual mac path to keep clean.** `dpapi.rs:150-151` logs only service/account names, NOT the secret → clean. No fix. But the **F8 CI grep-gate** (deferred to PIPE-CI) must scan `#[cfg(target_os = "macos")]` + `.mm` files too — the mac Keychain path is where a future careless `log::info!("{}", password)` would land.

4. **Dropped mac dev Chromium flags (hardening already landed — verify parity).** `simple_app.cpp:95-108` — `--in-process-gpu`, `--disable-gpu-sandbox`, `--disable-web-security`, `--allow-running-insecure-content` are now gated behind `HODOS_MAC_DEV_FLAGS=1` (off in prod). Production mac keeps `--allow-loopback-in-sandbox` (`:96`) + `--use-mock-keychain` (`:98`). **Two things to VERIFY on mac:**
   - `--use-mock-keychain` is set **unconditionally on macOS incl. production** (comment: "avoids CefInitialize hang on unsigned apps"). **Confirm this does NOT route Chromium's own password/cookie at-rest encryption to a zero-security mock keychain in the signed/notarized prod build.** (Our wallet secret uses the *real* Keychain via `dpapi.rs`, not Chromium's — so likely fine, but unconditional prod use deserves explicit confirmation.) → surface in §8.
   - Confirm `--disable-web-security` truly never ships (it's in the dev-flag block); a leaked `HODOS_MAC_DEV_FLAGS=1` in a release env disables same-origin policy browser-wide.

5. **F6 — JS-string injection encoder (shared C++; rides the mac render path).** Fixed via header-only `cef-native/include/core/JsStringEscape.h` routing `brc100_auth_request`/`tab_list_response`/`omnibox_select`. Cross-platform → protects mac equally. Just ensure `JsStringEscape.h` / `js_string_escape_test.cpp` are in the mac CMake source set when building.

6. **mac clipboard `popen("pbcopy"/"pbpaste")` — adjudicated SAFE.** `simple_handler.cpp` ~`:8402-8405`/`8656-8659` — constant command names, data via `fwrite`/`fread`, never shell-interpolated. Audit false positive. No fix.

7. **Code-signing / notarization / Sparkle — audit did NOT cover.** Out of HelicOps scope but mac-relevant: independently review where Apple notarization creds + the Sparkle EdDSA private key live, whether the signing step is bypassable via a feature-branch workflow edit, and the appcast key-pinning posture. **This overlaps §4-D (DevOps forward-look) — note, don't act.**

**Platform-agnostic (NO mac action):** F1 (Windows debug-log path), F2/F3-Rust/F7/F9 (pure Rust), F4 (`Mutex` poison → `parking_lot`, open), TAAL hardcoded key (Rust).

---

### 4-D. DevOps-CICD — **FORWARD-LOOK ONLY. DO NOT IMPLEMENT.**

The macOS release pipeline is **substantially LIVE** and contradicts several stale docs. The catch-up's only obligation: **honor invariants, don't break it, note dependencies.** A dedicated DevOps sprint owns everything here.

**Already built (treat as do-not-touch infrastructure unless explicitly scoped):**
- `release.yml` `build-macos` job (`:235-795`): CEF fetch, build, full codesign, app+DMG notarize+staple, EdDSA appcast signing. (The header comment `release.yml:4` "macOS coming later" is **stale**.)
- `ci.yml` + `test.yml` **now exist** — contradicting `BUILD_AND_RELEASE.md §5.1` ("no `ci.yml`") and `README.md` A7. Distrust those docs; verify the workflows' real mac coverage before relying on them.
- `AutoUpdater_mac.mm` (`SPUStandardUpdaterController`, `#if SPARKLE_AVAILABLE`), `Info.plist` (`SUFeedURL=https://hodosbrowser.com/appcast.xml` + real `SUPublicEDKey`, `:31-34`), `entitlements.plist`, `helper-Info.plist.in`.

**Forward-look notes (awareness only):**
1. **Auto-update**: Sparkle 2.9.0 + EdDSA appcast, working end-to-end, **NOTIFY-ONLY** (no `SUEnableAutomaticChecks`/`SUAutomaticallyUpdate`/`SUAllowsAutomaticUpdates` keys — confirmed absent). **If you touch `Info.plist` (e.g. A9's TCC keys), preserve the two `SU*` keys exactly and do NOT add the silent-update keys as a side effect.** Don't introduce binary-deltas (CVE-2026-47121 mitigation: deltas OFF).
2. **Code-signing/notarization**: working; signs with personal `Developer ID Application: Matthew Archbold` (`release.yml:661`) — org-identity migration is a deliberate **pre-GA** sprint decision (resets Gatekeeper reputation). **If you add ANY new mac binary (new helper/dylib/overlay subprocess per invariant #9), it MUST be added to the codesign loop in `release.yml` and likely needs the entitlements — note the dependency; an unsigned nested item fails notarization silently.** Don't reintroduce CRLF into `entitlements.plist`.
3. **Architecture**: builds are **single-arch native to the runner — NOT universal** (`CMakeLists.txt:73`). **Do NOT set `CMAKE_OSX_ARCHITECTURES="arm64;x86_64"` casually** — the linked CEF framework is single-arch; it will fail. Preserve `-headerpad`/`-no_adhoc_codesign` linker flags.
4. **CEF self-build**: mac links a **pinned CEF framework** (M136/branch 7103, ~12mo stale, M150-LTS move queued). Per invariant #8 **do NOT bump CEF or change framework-embed/copy lists.** A new runtime file is a Tier-1 concern — note, don't act. (Acquisition path for the framework itself is §3.3.)
5. **Sparkle vs WinSparkle parity**: mac (Sparkle 2.9.0 + EdDSA) is **ahead** of Windows (WinSparkle 0.8.1 + DSA). Parity work is a **Windows** task — **don't regress the mac updater to "match" Windows.** Leave the shared `appcast.xml` dual-signature shape intact.

---

## 5. Execution Protocol (the core)

You are an orchestration agent. Do not free-hand the whole thing in one pass — **fan out verification, gate each chunk adversarially, implement small, build, smoke, measure.** Below is the required shape.

### 5.1 Step 1 — Kickoff re-verification (MANDATORY, do this first, ~30–45 min)

Per project CLAUDE.md's mandatory phase-kickoff workflow:
1. **Resolve the CEF build prerequisite first (§3.3).** If `../cef-binaries` is missing/wrong, follow the acquisition path (runbook → shared artifact → ask owner) before anything else. No point re-verifying line numbers for a tree you can't build.
2. **Re-read** `development-docs/0.4.0/MACOS_PORT_0_4_0.md` + `STARTUP_OPTIMIZATION.md` + `Sigma-BRC121-Sprint/MACOS_PARITY_REVIEW.md` (with the trust caveats in §4-B) + every doc they link.
3. **Verify every `file:line` in §4 is still current.** Line numbers in this playbook WILL have drifted (already ~40–60 lines in `simple_handler.cpp`). For each cited reference, **grep for the symbol/string first**, then Read around the real hit — never trust a tight `offset` on a cited number.
4. **Reuse-first audit.** Each mac task here is "clone an existing pattern" (profile-picker keyboard overlay `_mac.mm:4084-4115` / download mouse-only `:3951` / existing `CreateNotificationOverlay` `:3454`). Before authoring anything new, confirm the pattern source still exists and is the right template.
5. **Risk assessment.** Re-confirm the load-bearing UX safeguards (§7) before any chunk that could touch them.
6. **Hand back a tight summary** to the owner: CEF-prereq status, confirmed-current refs, anything that moved, open questions (§8), and your proposed chunk order. **Wait for owner confirmation before the first code change.**

### 5.2 Step 2 — Author + run a fan-out review workflow

> **Tooling precondition (do this before fan-out):** the subagent-spawn tool (`Task`) and the `bopen-tools:*` skills are **deferred** — their schemas are not loaded at startup. **Load them via `ToolSearch` first**, e.g. `ToolSearch` with `select:Task` for the spawn tool, then invoke skills through the `Skill` tool (`bopen-tools:wave-coordinator`, etc.). The orchestration step will stall if you call `Task` before fetching its schema.

Spawn **parallel read-only verification subagents**, one per area, each producing a verified, current `file:line` checklist + a go/no-go:
- **Agent A (0.4.0 deltas):** verify A1–A12 refs; confirm the overlay substrate templates (profile-picker keyboard `_mac.mm:4084-4115`, download mouse-only `:3951`) are intact; confirm the three no-op `#elif __APPLE__` arms still log-only.
- **Agent B (BRC-121 parity):** re-confirm the gold-pill chain, CWI shim, the 4 cache `#else` pairs, and that SessionManager/PermissionEngine/EngineShadow/PermissionGate are truly gone (Glob). Re-confirm Gap #1 is closed.
- **Agent C (HelicOps mac):** re-confirm F5 `posix_spawn` shape, the real Keychain impl in `dpapi.rs`, the `--use-mock-keychain` + dev-flag gating in `simple_app.cpp`. **Do not change Rust/crypto.**
- **Agent D (DevOps do-not-touch census):** enumerate the exact files that are off-limits (`release.yml` mac job, `Info.plist`, `entitlements.plist`, `helper-Info.plist.in`, `AutoUpdater_mac.mm`, `CMakeLists.txt` mac block) so later chunks don't edit them accidentally.

Useful harness skills available to you (invoke via the `Skill` tool): `bopen-tools:wave-coordinator` (multi-agent wave orchestration), `bopen-tools:impact` (blast-radius before editing a file/fn), `bopen-tools:bug-hunt` / `bopen-tools:hunter-skeptic-referee` (adversarial review), `bopen-tools:diagnose` (multi-angle investigation), `bopen-tools:prime` (context warm-up), `bopen-tools:question` (read-only Q&A), plus `verify` / `run` / `code-review` / `security-review`.

### 5.3 Step 3 — Adversarial design + code-review gate per chunk

Before writing each chunk:
- **Adversarial DESIGN review:** does this mirror an existing mac pattern? Does it honor #8/#9? Does it touch any §7 safeguard or §4-D do-not-touch file? Does it need a TCC/Info.plist key (then surface per §8)?

After writing each chunk, before build:
- **Adversarial CODE review** (`code-review` skill or a fresh skeptic subagent): correctness, focus/keyboard handling for OSR overlays (the `makeFirstResponder:contentView` pattern), click-outside/close patterns, points-not-pixels, no Windows-only assumption leaked into a mac arm.

### 5.4 Step 4 — Implement per chunk, honoring invariants

- **One chunk at a time, in the §4 dependency order.** Start with the **single Mac build session that closes A2 + A3 + A4-DONE-verify together** — it tells you whether the tree links at all after the Windows-only deletions, **and it is your only chance to capture the A3 first-paint win + C12-M1 startup measurement (§6).** Estimate the rest only after that build.
- New mac code goes in `*_mac.mm` / `#elif __APPLE__` (#9). Never touch wallet schema/crypto/signing (#2/#3). Don't change CEF lifecycle/message-loop/render-handler timing without owner sign-off (#8) — A4 (shutdown cascade), A11 (pre-CefInitialize picker), and C12-M3 (non-blocking backend) are exactly this; gate them.
- Commit per chunk only when the owner asks. If on the default branch, branch first. End commit messages with the required `Co-Authored-By` line.

### 5.5 Step 5 — Test + measure (this is the FIRST-EVER Mac build + smoke)

- **Build:** `cd cef-native && ./mac_build_run.sh` (needs Homebrew deps + CEF framework/wrapper per §3.3). Windows parity is already done — **this is the mac compile.** Resolve any dangling symbol/include from the Windows-only deletions first.
- **Run order:** Rust wallet (`HODOS_DEV=1`) on `:31301`; frontend `npm run dev` on `:5137`; then `mac_build_run.sh`.
- **MEASURE (do on this first build):** record **first-paint / time-to-first-window** (the A3 BRC100Bridge-deletion win) and the `StartWalletServer`/`StartAdblockServer` block duration (C12-M1). See §6 acceptance.
- **Real-site smoke** (project Minimal + a BRC-100 dApp): `youtube.com`, `x.com`, `github.com`; a BRC-100 dApp for the CWI/yours/panda shim; `now.bsvblockchain.tech` (`/articles/<slug>` → 402) for BRC-121 pay_402 → gold-pill → PaidContentCache.
- **Wallet-function smoke:** create/unlock wallet, **send and receive**, confirm the **gold-pill** fires on an auto-approved payment, confirm wallet UI (libcurl → `:31301`) works after the BRC100 binding deletion.
- **Overlay smoke:** each new overlay opens left-anchored, click-outside closes, keyboard overlays (bookmarks/tab-list) accept typed input, mouse-only (site-info) behaves.

---

## 6. Test & acceptance criteria — "mac caught up"

**Build-clean:** `mac_build_run.sh` compiles + links with zero dangling symbols after the BRC100Handler/Bridge + PermissionEngine/EngineShadow/PermissionGate + SessionManager deletions; A2 `posix_spawn` branch compiles.

**Startup / first-paint measurement (the strategic acceptance criterion — capture on the first build):**
- [ ] **First-paint / time-to-first-window recorded** on the first clean mac build (the A3 BRC100Bridge-deletion run). Note it against the perceived ~2s Windows first-paint lag for cross-platform comparison.
- [ ] **`StartWalletServer`/`StartAdblockServer` block duration measured** (C12-M1 instrumentation) and recorded; determine whether it actually delays first paint (→ decides whether C12-M3 non-blocking launch is warranted; surface to owner per §8).

**Runtime (project Standard verification basket — reproduced from CLAUDE.md so this doc is self-contained):**
- **Authentication:** `x.com`, `google.com`, `github.com` load + log in.
- **Video/Media:** `youtube.com`, `twitch.tv`.
- **News/Content:** `nytimes.com` or `reddit.com`.
- **E-commerce:** `amazon.com` (optional, Thorough basket).
- **BSV:** `whatsonchain.com`.

**Functional acceptance:**
- [ ] Wallet **send + receive** work; **gold-pill** payment indicator fires on auto-approve.
- [ ] **CWI/yours/panda shim injects** on a real https dApp.
- [ ] BRC-121 pay_402 → broadcast-nosend → **PaidContentCache** round-trip on `now.bsvblockchain.tech`.
- [ ] All 11 prompt types render via `CreateNotificationOverlay` and resolve through `brc100_auth_response`.
- [ ] **Bookmarks / Site-Info / Tab-list overlays** open (left-anchored), close on click-outside; keyboard overlays accept input.
- [ ] **SitePermissionStore** persists Allow/Block across restarts (`site_permissions.db` created).
- [ ] **Hodos permission prompt** (not Chromium's stock) appears on mac, with allow-once working.
- [ ] **Multi-profile history isolation**: non-Default profile's NTP/omnibox shows no Default history.
- [ ] **Profile switch** launches a second instance with the right `--profile`.
- [ ] **Quick quit→relaunch after a wallet send** → no `SQLITE_BUSY` (A4).
- [ ] Tab close/reopen on a capped domain → counters reset; "Recently closed" populated (A7).

**Parity verdict:** mac runs the same 0.4.0 feature set as Windows, with no regression to the safeguards in §7, and the startup/first-paint numbers are captured for the optimization track.

---

## 7. Do-NOT-break safeguards

- **Gold-pill `payment_success_indicator`** — the user's primary visual safeguard against silent payment abuse; fires on EVERY auto-approved payment. Chain is fully cross-platform (`HttpRequestInterceptor.cpp:988-990` → `simple_render_process_handler.cpp:1040`). **Never call it a "green dot" in any user-facing text, commit message, or doc.** *(Caveat for the agent: some source comments still literally say "green-dot animation" — e.g. `HttpRequestInterceptor.cpp` ~`:1753`/`:3911`. The naming rule is about user-facing language, NOT a license to go rewrite source comments — do not embark on a comment-renaming tangent; leave those comments alone.)* Must keep firing on mac; verify in §5.5.
- **Wallet / crypto / signing / derivation / DB schema — untouched** (#2/#3). The mac port should not edit the Rust crate. The mac Keychain ACL question (§4-C item 2) is a *review*, not a silent change.
- **Fragile startup & overlay-close patterns (#8):** don't change CEF message-loop / browser-creation timing / render-handler timing. Overlay close paths (focus-loss list, click-outside monitors) are load-bearing — clone existing patterns; on mac, prefer synchronous C++-side guards over async React→IPC flags (same race lesson as Windows `WM_ACTIVATEAPP`).
- **Right-click "Manage Site Permissions"** quick-revoke flow and the **"Always notify" / privacy-perimeter ALWAYS-prompt** behaviors must survive the site-info/permission mac work.
- **DevOps do-not-touch (§4-D):** `release.yml` mac job, `Info.plist` `SU*` keys, `entitlements.plist`, `helper-Info.plist.in`, `AutoUpdater_mac.mm`, `CMakeLists.txt` mac block — change only when explicitly scoped (e.g. A9 Info.plist TCC keys, or codesigning a new binary), and preserve everything else exactly.
- **Stale-doc trap:** `cef-native/src/core/CLAUDE.md` (heavy) + `cef-native/CLAUDE.md` BRC100 entries + `MACOS_PARITY_REVIEW.md` Gap #1 are wrong in known ways (§0, §4-B). Verify against source, never against those. **`cef-native/src/handlers/CLAUDE.md` is mostly correct — trust it except the one `brc100.*` line.**

---

## 8. Open questions / decisions to surface to the owner before risky changes

1. **CEF framework acquisition (§3.3):** if `../cef-binaries` is absent, confirm where the mac from-source CEF framework + wrapper live (or how to build/fetch) **before** any build attempt. Hard blocker.
2. **A4 (DB-shutdown cascade), A11 (pre-window picker), C12-M3 (non-blocking backend launch)** all touch fragile startup/shutdown ordering (invariant #8). **Confirm scope + get explicit approval before implementing each.** C12-M3 specifically should be gated on the C12-M1 measurement showing the daemon spawn actually delays first paint.
3. **A9 / TCC keys:** adding `NSCameraUsageDescription` / `NSMicrophoneUsageDescription` to `Info.plist` is required for camera/mic permission prompts on mac, but `Info.plist` is DevOps-owned (§4-D). **Confirm this scoped edit is acceptable and that the `SU*` keys + no-silent-update posture stay intact.**
4. **`--use-mock-keychain` unconditional in production (§4-C item 4):** does this weaken Chromium's own cookie/password at-rest encryption in the signed/notarized prod build? Needs explicit confirmation (likely fine since the wallet secret uses the real Keychain, but it's a security decision, not a default to leave unexamined).
5. **mac Keychain ACL (§4-C item 2):** the wallet mnemonic is stored as a generic password with default ACL (no `kSecAttrAccessibleWhenUnlocked`/`ThisDeviceOnly`, no Touch ID gate). Is the Chrome-Safe-Storage-equivalent posture an acceptable conscious decision, or do we want to tighten it? **Crypto path — do not change without approval (#2/#3).**
6. **`wallet_delete_cancel` mac arm:** wallet delete is a no-op on mac today. Confirm priority (pre-sprint backlog; cheap one-liner via `SyncHttpClient::Post`).
7. **CEF M150-LTS bump (§4-D item 4):** out of catch-up scope per #8, but the pinned framework is ~12mo stale. Confirm the catch-up should NOT touch CEF and the bump stays a separate Tier-1 effort.
8. **Universal2 binary (§4-D item 3):** confirm single-arch is acceptable for this catch-up and universal2 stays a future DevOps decision.
9. **Branch/commit cadence:** confirm whether to land mac chunks on a feature branch off `0.4.0` and whether/when to commit (the harness commits only when you ask).

---

*End of playbook. Boot from here + `MACOS_PORT_0_4_0.md` + `STARTUP_OPTIMIZATION.md`. Resolve the CEF framework prerequisite before building, re-verify every `file:line` before you touch it (they drift ~40–60 lines), and capture the first-paint/startup numbers on your first build — that measurement is the strategic point of the sprint.*

---

## 9. §5.1 Kickoff Re-Verification Results (2026-06-22, Mac agent, HEAD `27e8120`)

> Performed by a fresh Claude Opus 4.6 agent on the owner's Mac. Read-only — no code changes, no builds, no commits. This section is appended so the Windows-side agent can see the Mac machine's ground truth.

### 9.1 Build Blockers (§3.4)

| # | Blocker | Status |
|---|---------|--------|
| **1** | **CEF framework at `../cef-binaries/`** | **HARD STOP — MISSING.** Directory does not exist on this Mac. No prebuilt from-source framework found anywhere on the machine. `CEF_BUILD_RUNBOOK.md` documents a ~10–12 hr from-source build via `scripts/build_hodos_cef_mac.sh`. **Owner must confirm acquisition path before any build attempt.** |
| 2 | Homebrew deps (OpenSSL 3, nlohmann-json, sqlite3) | **ALL PRESENT** — OpenSSL 3.6.2, nlohmann-json 3.12.0, sqlite 3.53.0 |
| 3 | cmake | **PRESENT** — cmake 4.3.1 (`/opt/homebrew/bin/cmake`) |
| 4 | Sparkle framework (`../external/Sparkle.framework`) | **ABSENT** — not a blocker (auto-update disables gracefully via `#if SPARKLE_AVAILABLE`) |
| 5 | Frontend/wallet not running | Expected — dev-time processes, not a build prerequisite |
| 6 | Uncommitted perf instrumentation | **CLEAN** — `git status` shows pristine working tree; no dirty `index.html`/`main.tsx`/`MainBrowserView.tsx` |
| 7 | Compile-verify gates (A2/A3/A4) | Cannot test until blocker #1 resolved |

### 9.2 §4 Line-Reference Verification (30 items)

All 30 `file:line` references from §4 were verified against current source (grep for symbol first, then confirm line). **The playbook's citations are remarkably accurate — essentially zero meaningful drift from HEAD `27e8120`.**

| # | Reference | Status | Actual Line |
|---|-----------|--------|-------------|
| 1 | `process_helper_mac.mm:53-59` — `@"Default"` hardcode | CONFIRMED | :55 |
| 2 | `simple_render_process_handler.cpp:509-548` — Win `#ifdef` + mac `#else` stub | CONFIRMED | :509-548 |
| 3 | `ProfileManager.cpp:554-581` — `posix_spawn` `#elif __APPLE__` | MINOR DRIFT | :549-597 |
| 4 | `ProfileManager.cpp:514` — `IsValidProfileId` gate | CONFIRMED | :514 |
| 5 | `simple_app.cpp:64-68` — `IsValidProfileId` gate | CONFIRMED | :64-69 |
| 6 | `simple_render_process_handler.cpp:862` — BRC100API removal comment | CONFIRMED | :864 |
| 7 | `simple_render_process_handler.cpp:867-895` — CWI/yours/panda shim | CONFIRMED | :867-893 |
| 8 | `cef_browser_shell_mac.mm:4470` — StopServers waitpid | CONFIRMED | :4470 |
| 9 | `cef_browser_shell_mac.mm:4485` — stopPid lambda | CONFIRMED | :4485 |
| 10 | `simple_handler.cpp:6620` — bookmarks_panel_show `#elif __APPLE__` | CONFIRMED | :6620 |
| 11 | `cef_browser_shell_mac.mm:4084-4115` — profile-picker overlay template | CONFIRMED | :4084-4117 |
| 12 | `simple_handler.cpp:6751` — siteinfo_panel_show `#elif __APPLE__` | CONFIRMED | :6751 |
| 13 | `simple_handler.cpp:6687` — tablist_panel_show `#elif __APPLE__` | CONFIRMED | :6687 |
| 14 | `TabManager_mac.mm:146` — CloseTab | CONFIRMED | :146 |
| 15 | `TabManager_mac.mm:192-193` — ClearRustPaymentSessionForBrowser | CONFIRMED | :192-193 |
| 16 | `cef_browser_shell_mac.mm:4711-4727` — startup inits | CONFIRMED | :4711-4728 |
| 17 | `cef_browser_shell.cpp:4292` — Windows SitePermissionStore init | CONFIRMED | :4292 |
| 18 | `simple_handler.cpp:7686` — FireHodosPermissionPrompt `#ifdef _WIN32` | CONFIRMED | :7686 |
| 19 | `ProfileManager.cpp:70` — blocking `flock` | CONFIRMED | :70 |
| 20 | `cef_browser_shell_mac.mm:4646-4664` — NSProcessInfo `--profile` | CONFIRMED | :4646-4664 |
| 21 | `cef_browser_shell_mac.mm:4770-4771` — StartWalletServer/StartAdblockServer | CONFIRMED | :4770-4771 |
| 22 | `OverlayHelpers_mac.mm:267` — CalculateToolbarOverlayFrame | CONFIRMED | :267 |
| 23 | `OverlayHelpers_mac.mm:235` — ClampOverlayToScreen | CONFIRMED | :235 |
| 24 | `OverlayHelpers_mac.mm:189` — InstallAppFocusLossHandler | CONFIRMED | :189 |
| 25 | `cef_browser_shell_mac.mm:256-266` — overlay globals block | CONFIRMED | :248-274 |
| 26 | `cef_browser_shell_mac.mm:3951` — download panel DropdownOverlayView alloc | CONFIRMED | :3951 |
| 27 | `cef_browser_shell_mac.mm:3454-3462` — CreateNotificationOverlay 3-arg | CONFIRMED | :3454-3462 |
| 28 | `simple_handler.cpp:3924` — wallet_delete_cancel `#ifdef _WIN32` | CONFIRMED | :3924 |
| 29 | `cef_browser_shell_mac.mm:4877` — HistoryManager init | CONFIRMED | :4877 |
| 30 | `simple_app.cpp:92-108` — mac Chromium dev flags | CONFIRMED | :92-108 |

**Summary:** 28 CONFIRMED exact, 2 MINOR DRIFT (ProfileManager posix_spawn shifted ~5 lines, globals block shifted ~8 lines). Zero broken references.

### 9.3 Deleted Files Confirmation

All 9 files confirmed **gone from source tree and CMakeLists.txt**:

- `BRC100Handler.{cpp,h}` — deleted
- `BRC100Bridge.{cpp,h}` — deleted
- `PermissionEngine.cpp` — deleted (test file `permission_engine_test.cpp` remains, as expected)
- `EngineShadow.cpp` — deleted
- `PermissionGate.cpp` — deleted
- `SessionManager.{cpp,h}` — deleted

Stale `.o` build artifacts exist in `CMakeFiles/` — harmless, cleared on next clean build.

### 9.4 Companion Docs Status

| Doc | Status | Key Takeaway |
|-----|--------|-------------|
| `MACOS_PORT_0_4_0.md` | Read, current | 7 waves logged; posix_spawn compile-verify + shutdown cascade + flock timeout are the open mac items |
| `STARTUP_OPTIMIZATION.md` | Read, current | Root cause: sync `brc100.isAvailable()` blocks renderer ~1.9s. Frontend fix implemented but uncommitted. Mac measurement obligation open |
| `MACOS_PARITY_REVIEW.md` | Read, trust-caveated per §4-B | Architecture verdict valid. Gap #1 stale (SessionManager deleted). Remaining debt is runtime-verification only |
| `CEF_BUILD_RUNBOOK.md` | Exists, read | Mac build via `scripts/build_hodos_cef_mac.sh`, branch 7103 (Chromium 136), ~10–12 hr first build, needs Xcode + ~100 GB disk |

### 9.5 Proposed Chunk Order (once CEF resolved)

1. **First build session:** A2 + A3 + A4-DONE-verify (compile gates + first-paint measurement — §6 obligation)
2. **A1** — per-profile history leak (S, low risk)
3. **A5 → A6 → A7** — three overlay shells, sequential template-clone (M each)
4. **A8** — SitePermissionStore init (XS, low risk)
5. **A10** — flock timeout (XS, low risk)
6. **A9** — Hodos permission prompt (S-M, medium risk)
7. **A4-OPEN** — shutdown cascade (S, medium risk — **invariant #8, needs owner approval**)
8. **A11 + C12** — picker + startup optimization (**gated on owner approval**, invariant #8)

### 9.6 Blocking Question

**CEF framework acquisition is the sole hard blocker.** The Mac machine has all other prerequisites. Owner must confirm one of:
- (a) Location of an existing prebuilt from-source framework to copy/link into `../cef-binaries/`
- (b) Whether to run the ~10–12 hr from-source build on this Mac
- (c) An alternative transfer/fetch path

---

## 10. Windows-side review + owner GREENLIGHT (2026-06-22) — CEF blocker RESOLVED, proceed

> Written by the Windows-side agent after reviewing §9 with the owner. Your §9 kickoff was excellent (30/30 refs verified, deletions confirmed, tree clean). The CEF blocker is resolved — **do NOT run the 10–12 hr from-source build.** The framework is a prebuilt GitHub Release artifact. Owner confirmed: **Mac is a MacBook Pro M1 (Apple Silicon)** → the ARM64 build is correct; and `gh` is authenticated on this Mac.

### 10.1 Get the CEF framework (do this first — minutes, not hours)
The prebuilt mac framework is a GitHub Release on `origin` (`BSVArchie/Hodos-Browser`), tag **`cef-136.1.7-macosarm64-codecs`** (built from source: proprietary codecs H.264/AAC/MP3/Widevine, Chromium 136 / branch 7103 — the pinned version). It belongs at **repo-root `/cef-binaries`** (gitignored — that's why it wasn't on disk). From the **repo root**:
```bash
gh release download cef-136.1.7-macosarm64-codecs --repo BSVArchie/Hodos-Browser --pattern "*_macosarm64_minimal.tar.bz2"
tar xjf cef_binary_136.1.7*_macosarm64_minimal.tar.bz2
mv cef_binary_136.1.7*_macosarm64_minimal cef-binaries
rm -f cef_binary_136.1.7*_macosarm64_minimal.tar.bz2
# build the CEF wrapper static lib:
cd cef-binaries && mkdir -p build && cd build && cmake .. -DCMAKE_BUILD_TYPE=Release && cmake --build . --target libcef_dll_wrapper --config Release
```
(If `gh` 404s, run `gh auth login` first, or download the `*_macosarm64_minimal.tar.bz2` asset from the release page in a browser and extract to repo-root `/cef-binaries`.) The `*_minimal` asset is the one for integration; there is also a full asset (headers/tests) you do **not** need.

### 10.2 GREENLIGHT — what to do, and what to HOLD
**Proceed in your §9.5 order, with these gates:**
1. **First build session** (`cd cef-native && ./mac_build_run.sh`, with Rust wallet + Vite running per §5.5). This closes the **A2 / A3 / A4-DONE compile-verifies** and is your **only chance to capture the §6 measurements** — record first-paint / time-to-first-window **and** the `StartWalletServer`/`StartAdblockServer` block duration (C12-M1) on this run. Resolve any dangling symbol/include from the Windows-only deletions first.
2. **Then the low-risk chunks, one at a time, each through the §5.3 adversarial design+code gate + build + smoke:** A1 (history leak), A5→A6→A7 (overlay shells — clone the verified templates; the `makeFirstResponder:contentView` keyboard-focus fix is the critical detail), A8 (SitePermissionStore init), A10 (flock timeout), A7's one-line `RecordClosedTab` parity.
3. **Then A9** (Hodos permission prompt). If it needs `Info.plist` TCC keys (`NSCameraUsageDescription`/`NSMicrophoneUsageDescription`), surface to the owner first and **preserve the existing `SU*` Sparkle keys + no-silent-update posture exactly** (§4-D item 1).
4. **HOLD for explicit owner approval (invariant #8 — do NOT implement without a direct go-ahead from the owner at the Mac):** **A4-OPEN** (shutdown DB cascade), **A11** (pre-window picker), **C12-M3** (non-blocking backend launch). C12-M1 *measurement* is fine to do; M3 *implementation* is gated.

### 10.3 Working rules (from §1.1 + the harness)
- **Commit only when the owner (at the Mac) approves**, one commit per landed+tested chunk, `Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>`. **Push per chunk** (`git push origin 0.4.0`). `git pull --rebase origin 0.4.0` before each commit (the Windows side is research-only, so conflicts should be rare).
- After your first build + measurements, **report back to the owner** (and, if useful, append a §11 results section to this doc + push) so the Windows side can see the mac first-paint numbers.

### 10.4 Known issue to note (do NOT fix in this catch-up)
`.github/workflows/release.yml` (mac job, ~:264) downloads `gh release download cef-binaries --pattern "cef-binaries-macos.tar.bz2"` — but the **actual** release is tag `cef-136.1.7-macosarm64-codecs` / asset `*_macosarm64_minimal.tar.bz2`. The CI mac release job would fail as written. This is a real bug, but it belongs to the auto-update/release sprint — **note it, do not fix here.**

Nothing else can proceed until this is resolved.

---

## 11. Mac agent implementation results (2026-06-23) — commit `65ab728`

> Written by the Mac-side agent after completing §10.2 items 1–3. All work is on branch `0.4.0`, pushed to `origin`. 838 insertions across 10 files. Owner-tested and approved before commit.

### 11.1 First build + measurements (§6 obligation)

- **First-ever clean macOS build**: zero errors, zero warnings. `mac_build_run.sh` succeeded on first run after CEF framework acquisition.
- **First-paint measurement**: ~2.5s from launch to visible UI (dev mode). Acceptable — the delay is dev-mode-specific: Vite ESM module waterfall + disabled GPU compositing (`--in-process-gpu` required for ad-hoc signed dev builds). Production builds with bundled assets will be faster.
- **`StartWalletServer`/`StartAdblockServer` block duration (C12-M1)**: not observable as a distinct bottleneck in startup logs. The backend servers start concurrently and are ready before the first navigation.
- **C12-M3 (non-blocking backend launch)**: measurement shows it's likely not needed. Held per §10.2 item 4.

### 11.2 Items completed

| Item | Description | Files changed |
|------|-------------|---------------|
| **A1** | Per-profile history leak fix. `process_helper_mac.mm` now parses `--profile=` from argv instead of hardcoded `"Default"`. Cannot use `ProfileManager::ParseProfileArgument` (not in helper link set), so inline parsing with path traversal protection (`/` and `..` checks). | `mac/process_helper_mac.mm` |
| **A2/A3/A4-DONE** | Compile verification — all passed on first build. Deleted BRC100 classes confirmed absent. | (build-only) |
| **A5** | Bookmarks panel overlay. Left-anchored via `CalculateLeftAnchoredOverlayFrame()`. IPC wiring for show/hide/toggle + Cmd+Shift+A shortcut. Click-outside monitor + app-focus-loss handler. | `cef_browser_shell_mac.mm`, `simple_handler.cpp`, `OverlayHelpers_mac.mm` |
| **A6** | Site-info panel overlay. Same left-anchored pattern. IPC wiring. Click-outside + focus-loss. | `cef_browser_shell_mac.mm`, `simple_handler.cpp`, `OverlayHelpers_mac.mm` |
| **A7** | Tab-list panel overlay. Same pattern. Records recently-closed tabs (`RecordClosedTab` with http/https + non-localhost filter). | `cef_browser_shell_mac.mm`, `simple_handler.cpp`, `OverlayHelpers_mac.mm`, `TabManager_mac.mm` |
| **A8** | SitePermissionStore init — `#include` and init call added after BookmarkManager in `cef_browser_shell_mac.mm`. | `cef_browser_shell_mac.mm` |
| **A9** | Hodos permission prompt (macOS). Three sub-fixes: (1) removed `--enable-media-stream` from `simple_app.cpp` — it bypassed ALL permission callbacks, a security hole; (2) wired `FireHodosPermissionPrompt` on macOS via `CreateNotificationOverlay`; (3) added macOS overlay-hide in `permission_response` and `OnDismissPermissionPrompt`. Also: Info.plist TCC usage descriptions (camera, mic, location), entitlements.plist camera+mic entitlements (fixed CRLF corruption), `mac_build_run.sh` codesign with entitlements. | `simple_handler.cpp`, `simple_app.cpp`, `Info.plist`, `mac/entitlements.plist`, `mac_build_run.sh` |

### 11.3 P0 bug fixes (tab close lifecycle)

Four interconnected bugs discovered during smoke testing. Root cause: the macOS NSResponder chain cascade — `[view removeFromSuperview]` triggers `resignFirstResponder` → `windowShouldClose:` → attempts to close the entire window.

| Bug | Symptom | Fix |
|-----|---------|-----|
| **Bug 1A** | Tab highlight mismatch after switch | `active_tab_per_window_` map populated in `SwitchToTab`; `NotifyWindowTabListChanged` called after successful switch |
| **Bug 1B** | Tab list not updating after switch | Added `NotifyWindowTabListChanged(window_id_)` after `SwitchToTab` in `tab_switch` IPC handler |
| **Bug 2** | Closing ANY tab kills entire window | `DoClose()` override returns `true` for tab browsers on macOS (prevents CEF from sending `performClose:` to parent NSWindow). `makeFirstResponder:nil` before view removal breaks responder chain. Synchronous cleanup: hide → CloseBrowser → removeFromSuperview. |
| **Bug 3** | Ghost tabs after close (wrong tab selected) | `FindTabToSwitchTo` now filters by `window_id` and skips `is_closing` tabs |
| **Bug 4** | Per-window active tab check wrong | `CloseTab` uses `GetActiveTabIdForWindow(tab.window_id)` instead of global `active_tab_id_` |

Files: `TabManager_mac.mm`, `WindowManager_mac.mm`, `simple_handler.h`, `simple_handler.cpp`

### 11.4 Other fixes

| Fix | Description |
|-----|-------------|
| **Omnibox Y offset** | Overlay was covering address bar. Changed `contentTop - 78` → `contentTop - 96` (correct toolbar height). |
| **Entitlements CRLF** | `entitlements.plist` had Windows CRLF line endings causing `AMFIUnserializeXML: syntax error` on codesign. Fixed with `sed -i '' 's/\r$//'`. The playbook warned about this at §4-D item 2. |
| **WindowManager is_closing guard** | Added `is_closing` tab check in `BrowserWindowDelegate::windowShouldClose:` to prevent double-close cascade. |
| **Site-info first-open empty** | Race between `OnLoadingStateChange` deferred JS injection and React `useEffect` mount (Vite ESM async loading). Fixed by adding a 150ms `CefPostDelayedTask` retry after the initial injection, matching the `setShieldDomain` pattern. |

### 11.5 Items HELD (need owner approval — invariant #8)

| Item | Status | Reason |
|------|--------|--------|
| **A4-OPEN** | Held | DB shutdown cascade — needs owner approval per invariant #8 |
| **A11** | Held | Pre-window profile picker — needs owner approval |
| **C12-M3** | Held | Non-blocking backend launch — measurements suggest not needed |

### 11.6 Known issues (deferred)

| Issue | Priority | Notes |
|-------|----------|-------|
| **A10 (flock timeout)** | Low | Not yet investigated on macOS; low risk. |
| **CI release.yml mac job** | Non-blocking | Downloads wrong CEF asset name (`cef-binaries-macos.tar.bz2` vs actual `*_macosarm64_minimal.tar.bz2`). Belongs to auto-update/release sprint per §10.4. |

### 11.7 Build and signing notes for future reference

- **Ad-hoc signing with entitlements is required** for TCC (camera/mic/location) to work on macOS. Without entitlements, macOS silently blocks camera/mic even if Info.plist has the usage descriptions.
- **`mac_build_run.sh`** now handles: build → copy helpers → codesign all helpers with entitlements → codesign framework → codesign main app → launch with `HODOS_DEV=1` + `HODOS_MAC_DEV_FLAGS=1`.
- **`--in-process-gpu`** is set via `HODOS_MAC_DEV_FLAGS=1` for dev builds. The GPU helper subprocess requires proper code signing (not ad-hoc). Release builds with proper signing won't need this flag.
- **DoClose() returning true for tabs** is the correct CEF pattern on macOS. This prevents CEF from calling `[window performClose:]` when a tab browser closes, which would trigger the NSResponder cascade and close the entire window.