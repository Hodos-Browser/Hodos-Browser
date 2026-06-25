# CEF Version Update Tracker

Track features, fixes, and investigations to research when updating the CEF build.

**Current CEF version:** 136 (built from source with `proprietary_codecs=true ffmpeg_branding=Chrome`)

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
4. Run full test suite (Minimal + Standard site verification from CLAUDE.md)
5. Specifically test: Google Sign-In, OAuth flows, media playback, ad blocking, fingerprint protection
6. Update this document with findings
7. Update `CLAUDE.md` x.com media section if codec situation changes
