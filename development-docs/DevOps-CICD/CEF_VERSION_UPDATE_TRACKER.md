# CEF Version Update Tracker

Track features, fixes, and investigations to research when updating the CEF build.

**Current CEF version:** 136 (built from source with `proprietary_codecs=true ffmpeg_branding=Chrome`)
**Current macOS floor:** **11.0 (Big Sur)** — CEF 136 dropped 10.15 "Catalina"; published minimum must match (see *"macOS Minimum Deployment Version"* below). Re-check on every Chromium bump.

---

## Must Investigate on Next CEF Update

### Toolchain (MSVC) & Dependency Alignment
- **Priority:** HIGH — build-breaker. Nothing compiles or links if this is wrong, and the errors *look* like our code but aren't.
- **Why:** The compiler toolset (currently **MSVC v143**, shipped by Visual Studio 2022) is a cross-cutting **ABI contract**. **Four things must all sit on the same toolset** or you get linker/ABI failures:
  1. The **CEF binaries** — whether prebuilt download *or* our own full Chromium+CEF source build
  2. The **vcpkg static deps** (`nlohmann-json`, `sqlite3`, …) — compiled per-toolset
  3. Our **C++ shell** code
  4. The **CI runner image** — the `windows-XXXX` / `macos-XX` GitHub label that *provides* the compiler
- **The rule for FULL builds (Chromium/CEF bump), not just shell builds:** when moving to a new stable Chromium/CEF, treat the toolset as a deliberate choice:
  1. Pull the **latest stable Chromium**; note which **MSVC/Clang toolset** its CEF is built with.
  2. Re-validate **every dependency version** against that toolset — vcpkg baseline, CEF wrapper, Inno Setup, Sparkle/WinSparkle, etc. (see `DEPENDENCY_VERIFICATION.md`).
  3. Rebuild the **vcpkg static deps** and the **CEF wrapper** on the chosen toolset.
  4. **Pin the CI runner image** (`runs-on:` in `release.yml` / `ci.yml`) to one that ships that exact toolset — **never `windows-latest` / `macos-latest`**, which float and silently roll the compiler forward under you.
  5. Bump `APP_VERSION` + installer + appcast versions; run the full smoke matrix (CLAUDE.md Testing Standards).
- **Cautionary tale (2026-06-25):** GitHub rolled the `windows-latest` label from the windows-2022 image to **windows-2025**. The `"Visual Studio 17 2022"` CMake generator stopped resolving ("could not find any instance of Visual Studio") and the beta.16 Windows build died at *configure* — before compiling a single file. Pure infra drift, zero code changes. Fix: pin `runs-on: windows-2022`. **Generalize: pin runner images; don't let them float.**
- **References:**
  - `DEPENDENCY_VERIFICATION.md` — per-bump dependency checklist
  - `CEF_BUILD_RUNBOOK.md` — full Chromium+CEF source build
  - `.github/workflows/release.yml` — `runs-on:` pins + the explanatory comment on the windows job
- **Added:** 2026-06-25

### macOS Minimum Deployment Version (published min must match Chromium's floor)
- **Priority:** HIGH — ships a **broken auto-update** if wrong. Sibling of the runner-pin lesson above; same root cause (a floating runner image silently overrode our intent).
- **Why:** Two numbers must agree or mac auto-update breaks:
  1. **The oldest macOS the Chromium/CEF build actually supports** (Chromium raises this every few majors as Apple drops old OSes — e.g. **CEF 136 dropped macOS 10.15 "Catalina"**, so the true floor is **macOS 11.0 "Big Sur"**, *not* the `10.15` our config historically claimed).
  2. **Our published minimum** — `CMAKE_OSX_DEPLOYMENT_TARGET` (`cef-native/CMakeLists.txt`), `LSMinimumSystemVersion` (`cef-native/Info.plist`, `cef-native/mac/helper-Info.plist.in`), and the binary's actual Mach-O `LC_BUILD_VERSION minos`.
- **The two failure modes (both real):**
  - **Published min too HIGH** (the beta.16 bug): the build floated on `macos-latest` = macOS 26 "Tahoe"; the deployment-target intent was a silent CMake no-op, so the **linker stamped the binary's `minos` at the runner's SDK (26)**. Sparkle/the loader then refuses to relaunch on every user below that OS → "requires macOS 26.0 or later" → **dead auto-update**. (This is *why* pinning the runner — see the Toolchain item — and forcing the deployment target both matter.)
  - **Published min too LOW** (claim 11.0 when the framework needs 12.0): the OS *accepts* the update, then dyld fails to load the higher-`minos` CEF framework → **launch crash after update**. Worse than gating honestly.
- **The rule, every Chromium/CEF bump:**
  1. **Look up the new Chromium's oldest supported macOS** (Chromium release notes / "Chrome to drop support for macOS X" announcements).
  2. **Measure the prebuilt CEF framework's real floor** on a Mac: `vtool -show-build "<...>/Chromium Embedded Framework.framework/Chromium Embedded Framework" | awk '/minos/{print $2}'` (or `otool -l | grep -A4 LC_BUILD_VERSION`). Do **not** trust the announcement alone.
  3. **Set our published minimum = `max(Chromium floor, measured framework minos)`** in **all three** places: `CMakeLists.txt` `CMAKE_OSX_DEPLOYMENT_TARGET`, `Info.plist` `LSMinimumSystemVersion`, `helper-Info.plist.in` `LSMinimumSystemVersion`. Keep them identical.
  4. **Apply it for real** — pass `-DCMAKE_OSX_DEPLOYMENT_TARGET=<floor>` on the configure command line (a bare `set(... CACHE ...)` after `project()` is a silent no-op) and export `MACOSX_DEPLOYMENT_TARGET=<floor>` at job level so the CEF wrapper, cargo, and sub-cmakes all inherit one floor.
  5. **Guard it in CI** (the standing per-build check — see `BUILD_AND_RELEASE.md` release checklist): after build, read `minos` of the main exe, all helper apps, and the Rust binaries and **FAIL the build unless each `minos` ≥ the CEF framework's `minos`** (an inequality, not `== <floor>`). CI runs on the newest macOS and *cannot* reproduce a sub-floor loader rejection, so also do a **manual relaunch-after-update on a real machine at/near the floor** before `promote --latest`.
- **⚠️ Runner SDK vs. deployment target — do not conflate (this is what caused beta.16):** these are TWO independent things.
  - **Build runner** (`runs-on:`) = the machine that *compiles* the app. It does **not** decide which users can run it. Best practice: **build on the *current stable, pinned* image** (newest GitHub-supported `macos-NN` you've validated), **never the floating `macos-latest`**. Re-validate and bump the pin on each Chromium bump (and whenever GitHub retires the image — old images *are* eventually removed, so "pin once forever" isn't an option).
  - **Deployment target** (`CMAKE_OSX_DEPLOYMENT_TARGET` / `minos`) = the **minimum-requirements label** that decides backward compatibility. This — not the runner — is what makes one binary run on the floor OS *and everything newer*. Standard Apple practice (and Chrome/Firefox's): **build with the latest SDK, set the deployment target to the oldest OS you support.**
  - **The trap:** if you forget to *explicitly* set + enforce the deployment target, the linker stamps `minos` from the **runner's SDK** — so a newer runner makes the app run on *fewer* machines, not more (the beta.16 "requires macOS 26" failure). Building on a newer runner never *widens* user compatibility; only lowering the deployment target does. So: newest pinned runner is fine and recommended, **provided the explicit target + the `minos` guard are in place.**
- **Decision log:** for **CEF 136**, published floor = **macOS 11.0 (Big Sur)**, pending the §2 `vtool` measurement confirming the framework isn't higher. 10.15 is retired (Chromium dropped Catalina). Runner = **current stable pinned image (`macos-15`)**, not `macos-latest`. Owner approved "Big Sur or newer" + "current stable, pinned runner" 2026-06-26.
- **References:**
  - `cef-native/CMakeLists.txt` (`CMAKE_OSX_DEPLOYMENT_TARGET`), `cef-native/Info.plist` + `cef-native/mac/helper-Info.plist.in` (`LSMinimumSystemVersion`)
  - `.github/workflows/release.yml` — mac build job runner pin + deployment-target flag + the post-build `minos` guard
  - `development-docs/0.4.0/archive/POST_BETA16_PLAN.md` — Thread 5 (full root-cause + fix)
- **Added:** 2026-06-26

### FedCM (Federated Credential Management) Support
- **Priority:** HIGH
- **Why:** Google made FedCM mandatory for "Sign in with Google" as of August 2025. CEF 136 does not implement the browser-level UI (account chooser dialog) that FedCM requires. This breaks "Sign in with Google" on any site that migrated to FedCM-only (no popup/redirect fallback).
- **What to check:**
  - Does the new CEF version include `CefPermissionHandler` methods for FedCM?
  - Is there a `navigator.credentials.get({identity: ...})` handler we can implement?
  - Check Chromium commit history for FedCM-related CEF changes
  - Test: Go to any site with "Sign in with Google" — does the account chooser appear?
- **Workaround (current):** Sites that still support OAuth popup/redirect fallbacks work. Sites that went FedCM-only do not show the Google sign-in button at all.
- **References:**
  - https://developer.chrome.com/docs/identity/fedcm/overview
  - https://developers.google.com/identity/gsi/web/guides/fedcm-migration
  - CEF issue tracker: search "FedCM" or "Federated Credential Management"
- **Added:** 2026-05-01

### Permissions API Updates
- **Priority:** MEDIUM
- **Why:** CEF 136 handles some permissions natively via Chrome bootstrap. Newer CEF versions may add `CefPermissionHandler` methods for notifications, geolocation, camera/mic that we should implement.
- **What to check:**
  - New `CefPermissionHandler` methods
  - Permission persistence APIs
  - Test: Check if notification permissions, camera access work
- **Added:** 2026-05-01

### CefResponseFilter Stability
- **Priority:** LOW
- **Why:** We use `CefResponseFilter` for YouTube ad-key stripping (`AdblockResponseFilter`). This API has had stability issues in some CEF versions.
- **What to check:**
  - Verify YouTube ad blocking still works (response filter streaming)
  - Check if API changed or was deprecated
- **Added:** 2026-05-01

---

## Nice to Have / Research

### Web Bluetooth / Web USB
- CEF may add support for these APIs in newer versions
- Currently not available in CEF 136
- Low priority for a browser focused on BSV/Web3

### COOP/COEP Header Handling
- Cross-Origin-Opener-Policy affects OAuth popup `window.opener` preservation
- Newer Chromium versions have `restrict-properties` mode
- Verify our popup handling still works with stricter COOP defaults

### Codec Updates
- We build CEF from source with proprietary codecs
- Check if build flags changed for H.264/AAC/H.265 support
- Verify media playback on YouTube, Twitch after update

---

## Process for CEF Version Updates

1. Check this document for investigation items
2. Build from source with `proprietary_codecs=true ffmpeg_branding=Chrome`
3. **Align the toolchain** — note the toolset the new CEF is built with; rebuild vcpkg static deps + the CEF wrapper on it; re-run `DEPENDENCY_VERIFICATION.md`; and **re-pin the CI runner images** (`runs-on:`) to one shipping that toolset (never `windows-latest`/`macos-latest`). See *"Toolchain (MSVC) & Dependency Alignment"* above.
4. **Re-check the macOS minimum version** — look up the new Chromium's oldest supported macOS, `vtool`-measure the prebuilt CEF framework's real `minos`, set our published minimum = `max(those)` in `CMakeLists.txt` + both plists, apply it via `-DCMAKE_OSX_DEPLOYMENT_TARGET=` on the configure line, and confirm the CI `minos` guard passes. See *"macOS Minimum Deployment Version"* above.
5. Run full test suite (Minimal + Standard site verification from CLAUDE.md)
6. Specifically test: Google Sign-In, OAuth flows, media playback, ad blocking, fingerprint protection
7. Update this document with findings
8. Update `CLAUDE.md` x.com media section if codec situation changes
