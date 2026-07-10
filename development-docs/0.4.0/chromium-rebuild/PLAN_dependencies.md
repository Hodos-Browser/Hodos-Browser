# PLAN — Dependency Bumps Riding the Chromium/CEF Jump (→ v0.4.0-beta.1)

**Status:** DETAILED PLAN (Workflow-2 expansion of `CHROMIUM_CEF_BUILD_DESIGN_OUTLINE.md` §3e / A3 + `DEPENDENCY_VERIFICATION.md`). Research + design only — **NO code, NO builds.**
**Created:** 2026-07-10 · **Owner:** Matthew (Marston Enterprises) · **Lead:** Windows Claude
**Purpose:** The single followable procedure for reconciling **Hodos's own** dependencies — vcpkg static C++ deps, the CEF wrapper, WinSparkle/Sparkle, Inno Setup, and the Rust + frontend stacks — against the toolchain and ABI of the target CEF/Chromium build, with a per-bump verification checklist and an explicit list of what to re-pin. This is the concrete backing for edit-inventory row **DEP-1** in `Q5_full_edit_list.md` and the A3 slice of the outline.

> **Authoritative inputs:** `DevOps-CICD/DEPENDENCY_VERIFICATION.md` (the canonical per-bump checklist this doc operationalizes), `DevOps-CICD/CEF_BUILD_RUNBOOK.md` (Steps 4/5/5.5 — wrapper rebuild + dep reconciliation + drift audit), `DevOps-CICD/CEF_VERSION_UPDATE_TRACKER.md` (Toolchain/Dependency-alignment lesson; the living log this feeds), `build-instructions/WINDOWS_BUILD_INSTRUCTIONS.md` + `MACOS_BUILD_INSTRUCTIONS.md` (the concrete install commands + observed versions), `cef-native/CMakeLists.txt`, `.github/workflows/release.yml` (vcpkg + WinSparkle + Inno steps), `installer/hodos-browser.iss`, `rust-wallet/Cargo.toml`, `adblock-engine/Cargo.toml`, `frontend/package.json`. Siblings: `PLAN_codecs.md`, `Q2_farbling_adblock.md`, `Q4_widevine_amazon_drm.md`, `Q5_full_edit_list.md` (DEP-1 row).

> **TARGET = placeholder.** Exact CEF stable version + branch + milestone + the MSVC/Clang toolset it was built with all resolve from `cef-builds.spotifycdn.com/index.json` + the CEF release notes at plan-execution time (outline §2 / Step 0). Runbook lines 80–85 anchor current stable at **CEF 149 / Chromium 149 (branch 7827)** with **M150-LTS** as the pin candidate. **⚠️ LTS is now a *conditional* default:** the outline's revised Decision #1 (adversarial pass) sets **target = current CEF stable, LTS only if primary sources confirm the LTS program at Step 0** — and flags that in-repo docs still say "no LTS, target M149," to be reconciled at Step 0. Treat the M150-LTS figure here as the runbook's candidate, not a settled pin; target selection is deferred to the VER-* rows regardless. Every "target version" number below is the **current-stable-as-of-2026-07** recommendation; **re-confirm each at execution time** — the value of this doc is the *procedure and the re-pin list*, not the frozen numbers.

---

## 1. What this answers (one screen)

- **Which of our own deps must move when Chromium moves, and why?** The hard part of a CEF bump is **not** Chromium's internal deps (gclient/`automate-git.py` resolve those for the pinned branch). It is **our** deps staying ABI- and toolchain-compatible with the new `libcef` (`DEPENDENCY_VERIFICATION.md` §"Why this exists"). Four things must sit on **one toolset** or you get linker/ABI failures that *look like our bug but aren't*: the CEF binary, the vcpkg static deps, our C++ shell, and the CI runner image (`CEF_VERSION_UPDATE_TRACKER.md`, Toolchain item).
- **Headline recommendation (two parts):**
  1. **Rebuild, don't just re-declare.** The **CEF wrapper** and **every vcpkg static dep** (`nlohmann-json`, `sqlite3`, `openssl`, plus vendored `quirc`) must be **recompiled on the exact MSVC/Clang toolset the target `libcef` was built with** — a version bump of the *source* is optional, a rebuild on the matched toolset is **mandatory**. Same on macOS for the Homebrew deps + the framework's Clang/deployment-target.
  2. **Close the four silent-drift holes first.** Today four dependency inputs are **effectively unpinned** and will bite exactly like the beta.16 `windows-latest`→`windows-2025` drift: (a) `vcpkg install` runs against the **runner's pre-installed vcpkg baseline** (no manifest, no baseline pin), (b) `choco install innosetup` fetches **whatever Inno is newest** (6.7.x today, **7.0 beta already published** — a major with breaking `.iss` changes), (c) macOS `brew install` deps float, (d) there is **no `rust-toolchain.toml`** so `rustc` floats with the runner. Pinning these is cheaper than debugging them post-bump and is the single highest-leverage action in this plan.
- **Scope boundary:** version-bump *mechanics* (branch string, minos, runner pins, file-manifest drift, version single-sourcing) are rows **VER-1..VER-6** in `Q5_full_edit_list.md` §A.7 — this doc owns only the **dependency-compatibility** slice (DEP-1). Where they touch (e.g. the runner pin also fixes the vcpkg-baseline problem) is called out inline.

---

## 2. Dependency inventory — current versions (read in-repo 2026-07-10; ✅ rows pinned, ❌ rows only observed)

The five layers from `DEPENDENCY_VERIFICATION.md` §"The dependency inventory", filled with the **actual current versions** read from the repo:

> **Column note:** "Current version (observed)" = the value actually in force today. For ✅ rows this is a real in-repo pin (lockfile / exact `=` / URL / runner). For ❌ rows the value is **observed/documented, not pinned** — e.g. the vcpkg numbers come from `WINDOWS_BUILD_INSTRUCTIONS.md`'s "Expected output," and the `rustc`/Inno/brew values are whatever the runner/`choco`/`brew` currently serve. Do not read an ❌-row number as a guarantee.

| Layer | Dependency | Current version (observed) | Where set | Pinned reproducibly? |
|---|---|---|---|---|
| CEF binding | `libcef_dll_wrapper` | matches `cef_binary_136.1.6+g1ac1b14+chromium-136.0.7103.114` | built from `cef-binaries/libcef_dll/wrapper/`; version tied 1:1 to `libcef` | ✅ (tied to CEF binary) |
| C++ / vcpkg | `nlohmann-json` | **3.12.0#1** `x64-windows-static` | `release.yml` L172 `vcpkg install …`; `WINDOWS_BUILD_INSTRUCTIONS.md` L61/73 | ❌ **baseline unpinned** (§4-A) |
| C++ / vcpkg | `sqlite3` | **3.51.1** `x64-windows-static` (linked `unofficial::sqlite3::sqlite3`) | same | ❌ **baseline unpinned** |
| C++ / vcpkg | `openssl` | **3.6.0#3** `x64-windows-static` (`/MT` static-CRT — matches `hodos-update-helper`) | same; `OPENSSL_ROOT_DIR` forced to static triplet (`release.yml` L200) | ❌ **baseline unpinned** |
| C++ / vendored | `quirc` (QR decoder) | vendored source, ISC | `cef-native/third_party/quirc/*` (compiled in-tree, `CMakeLists.txt` L126-135) | ✅ (vendored — recompile only) |
| Toolchain | MSVC | **v143 / VS 2022 BuildTools**; `CMAKE_CXX_STANDARD 17` | `release.yml` runner pin + `CMakeLists.txt` L43 | ✅ runner pinned `windows-2022` |
| Toolchain (mac) | Clang / Xcode + `CMAKE_OSX_DEPLOYMENT_TARGET` | floor **11.0** (Big Sur) for CEF 136 | `CMakeLists.txt`, `Info.plist`, `helper-Info.plist.in`; runner `macos-15` | ✅ runner pinned; ⚠️ minos = VER-4 |
| Auto-update (Win) | WinSparkle DLL | **0.8.1** (shipped, DSA-verified) + **0.9.3** tool (EdDSA signing in CI) | `release.yml` L179-184 (hardcoded release URLs) | ✅ URL-pinned |
| Auto-update (mac) | Sparkle | 2.9.x line (EdDSA) | mac build/notary job | ✅ (confirm exact at plan time) |
| Installer (Win) | Inno Setup | **whatever `choco` serves** (6.7.x today) | `release.yml` L160 `choco install innosetup -y` | ❌ **unpinned** (§4-B) |
| Frontend | React / react-dom | **19.1.0** | `frontend/package.json` L21-22 + lockfile | ✅ lockfile |
| Frontend | react-router-dom | **7.6.1** | `package.json` L23 | ✅ lockfile |
| Frontend | MUI / Emotion | `@mui/material` **7.1.1**, icons **7.3.9**, `@emotion/*` **11.14** | `package.json` L15-18 | ✅ lockfile |
| Frontend | TypeScript / Vite | TS **~5.8.3**, Vite **^6.3.5** | `package.json` L37/39 | ✅ lockfile |
| Rust | `hodos-wallet` | edition **2021**; crates via `Cargo.lock` | `rust-wallet/Cargo.toml` | ✅ lockfile |
| Rust | `hodos-adblock` | edition 2021; `adblock =0.10.3` (exact — **Rust-1.85 stability lock**: 0.10.4+ needs unstable `unsigned_is_multiple_of`), `rmp =0.8.14` (exact — **rmp-serde 0.15 compat**) | `adblock-engine/Cargo.toml` L15/17/22 | ✅ exact pins |
| Rust | `actix-web` (adblock server) | **`=4.11.0`** (exact) — in-repo comment: *"pinned to 4.11 — 4.13+ requires Rust 1.88"* → **the load-bearing rustc-ceiling constraint** | `adblock-engine/Cargo.toml` L11-12 | ✅ exact pin |
| Rust | toolchain (`rustc`) | **none declared** — floats with `dtolnay/rust-toolchain@stable` (`release.yml` L142), i.e. the stable channel at build time, not the runner image | (no `rust-toolchain.toml` anywhere) | ❌ **unpinned** (§4-D) |

> **Reading the "reproducibly pinned?" column:** ✅ deps are safe by default on a bump (verify + rebuild only). ❌ deps are the **drift surface** — §4 addresses them first, because a Chromium bump is exactly the moment a floating input silently rolls forward.

---

## 3. Per-dependency plan (answers the `DEPENDENCY_VERIFICATION.md` §"Per-dependency checklist" seven questions for each)

For each dep record, in writing, the checklist's 7 fields. Below is the plan + the recommended answer/action; the implementing session fills the "verification performed" + "decision" fields with real results and appends the table to `CEF_VERSION_UPDATE_TRACKER.md`.

### 3.1 CEF wrapper (`libcef_dll_wrapper`) — **the ABI linchpin**

- **What / current:** the C++ convenience layer that our `cef-native` shell links against; **must match `libcef` version exactly** (`DEPENDENCY_VERIFICATION.md` inventory row 1; `CEF_BUILD_RUNBOOK.md` Step 4).
- **Target / action:** ships **inside** the new CEF binary distribution — do **not** version it independently. On the bump: `cef-binaries/libcef_dll/wrapper` is **replaced** from the target dist, then **rebuilt from scratch**: delete `build/CMakeCache.txt` + `build/`, re-`cmake -G "Visual Studio 17 2022" -A x64 ..`, `cmake --build . --config Release` (runbook Step 4; `WINDOWS_BUILD_INSTRUCTIONS.md` L104-126).
- **Compat / why this one:** it is the same toolset as the target `libcef` by construction. The classic failure is **not rebuilding it** → `"Unsupported CEF version"` at configure/link (runbook Lessons). This is a rebuild, never a "leave it".
- **Ripple:** forces a `cef-native` rebuild against the new headers; any CEF handler-signature change surfaces here as a compile error (loud — good).
- **Verification:** wrapper `.lib` present (`dir Release\libcef_dll_wrapper.lib`); `cef-native` configures + links; smoke-launch (a missing/renamed CEF runtime file = green build / runtime crash — that's the VER-5 file-manifest audit, cross-referenced).
- **Recommended decision:** **replace + full rebuild every bump. Never independently pinned.**

### 3.2 vcpkg static C++ deps — `openssl`, `sqlite3`, `nlohmann-json` (+ vendored `quirc`)

- **What / current:** OpenSSL **3.6.0#3**, sqlite3 **3.51.1**, nlohmann-json **3.12.0#1**, all `x64-windows-static` (`/MT` static-CRT). `quirc` is **vendored** in-tree (recompile, no version decision).
- **Target / action:** the **version can usually carry forward**; the **rebuild on the matched MSVC toolset is mandatory** (`CEF_VERSION_UPDATE_TRACKER.md` Toolchain item, point 2). Concretely: re-run `vcpkg install nlohmann-json:x64-windows-static sqlite3:x64-windows-static openssl:x64-windows-static` **against a pinned vcpkg baseline** (§4-A) on the toolset the target CEF uses. Bump a dep's *version* only if the target toolchain/CEF requires it or there is a security CVE (OpenSSL is the likely candidate — track the OpenSSL 3.x advisory line).
- **Compat / why:** the **static-CRT (`/MT`) triplet is load-bearing** — `release.yml` L166-172 + L200 force `find_package(OpenSSL)` to the static triplet so the `/MT` `hodos-update-helper` links; a `/MD` (dynamic-CRT) dep would break that link. Preserve `x64-windows-static` exactly. C++17 std (`CMAKE_CXX_STANDARD 17`) must remain ≥ what the new CEF headers require (newer CEF may raise to C++20 — **check the target CEF's minimum C++ std**, OQ-3).
- **Ripple:** OpenSSL is the widest **on the C++ side** — used by `cef-native` (`OpenSSL::SSL/Crypto`) and `hodos-update-helper` (`UpdateFs::Sha256FileW`); a CRT or C++-std change ripples to both C++ targets. **No ripple into the Rust layer:** `rust-wallet` builds `reqwest` with `default-features = false, features = ["rustls-tls"]` (`Cargo.toml` L80) → the Rust TLS backend is **rustls, not OpenSSL** (and not Windows SChannel), so the vcpkg OpenSSL bump does **not** touch the Rust graph. (Verify this stays true if the reqwest features change.)
- **Verification:** `vcpkg list` shows the expected versions/triplet; `cef-native` + `hodos-update-helper` + `tests/` all link; `ctest` (the C++ `permission_engine_test` / `manifest` suites) passes.
- **Recommended decision:** **carry versions forward; rebuild on matched toolset; pin the vcpkg baseline (§4-A); bump OpenSSL only for a CVE or a toolchain demand.**

### 3.3 macOS Homebrew deps + framework toolchain

- **What / current:** `brew install openssl nlohmann-json sqlite3` (`MACOS_BUILD_INSTRUCTIONS.md` L40), libcurl from the system (used by `SyncHttpClient`), Keychain (DPAPI-equivalent). Deployment floor 11.0 for CEF 136.
- **Target / action:** re-measure the target framework's real `minos` with `vtool` and set the floor = `max(Chromium floor, measured minos)` in all three places (this is **VER-4**, cross-referenced — kept out of DEP-1 to avoid double-ownership). Rebuild the Homebrew deps under the Xcode/Clang the target CEF framework was built with. **Pin the brew formula versions** (§4-C).
- **Compat / why:** the framework is built with Apple Clang + a deployment target; our deps + shell must not out-run it. A newer Homebrew OpenSSL that raised its own min-macOS above our floor would silently break launch on the floor OS.
- **Verification:** `find_package(OpenSSL)`/nlohmann/sqlite3 resolve (`MACOS_BUILD_INSTRUCTIONS.md` L159-161); `minos` guard passes (VER-4); mac smoke basket.
- **Recommended decision:** **rebuild on matched Xcode; pin brew versions; minos handled by VER-4.**

### 3.4 WinSparkle (Windows auto-update sidecar)

- **What / current:** **0.8.1** DLL shipped (DSA-verified feed) + **0.9.3** tool used only for EdDSA signing in CI — a deliberate **dual-signed DSA→EdDSA transition** (`release.yml` L179-184; `AUTO_UPDATE_AND_SIGNING_0_4_0.md`). Note: Windows now also carries the **custom silent updater** (`hodos-update-helper` + picker-gate) — WinSparkle is no longer the whole update story, so treat its version as **feed-verify/appcast plumbing**, not the apply path.
- **Target / action:** the CEF bump does **not** force a WinSparkle bump — it is independent of the Chromium ABI. **Decouple this decision:** only move off 0.8.1 as part of the deliberate DSA→EdDSA cutover (its own track), not because Chromium moved. If bumped, target the current WinSparkle release (0.9.x line, EdDSA-native) and retire the 0.8.1 DSA path.
- **Compat / why:** WinSparkle links no CEF/Chromium symbols; the only coupling is the `sparkle:version` integer the Windows updater compares (`release.yml` L101) and the EdDSA sidecar signing (`SPARKLE_EDDSA_PRIVATE_KEY`). **⚠️ Auto-update N-1→N gate:** a WinSparkle/signing change is a **reinstall-forcer class** — the real apply test must verify **signer continuity** (Windows CN unchanged = `Marston Enterprises`), per the signing-identity gate. Fold into the P6 auto-update test, not here.
- **Verification:** appcast validates; DSA (0.8.1) **and** EdDSA sidecars both present; a **real N-1→N silent apply** on Windows succeeds with CN continuity (`SILENT_UPDATE_TEST_PLAN.md`).
- **Recommended decision:** **carry 0.8.1/0.9.3 as-is for beta.1; do NOT couple a WinSparkle bump to the Chromium jump; sequence the DSA→EdDSA cutover on its own.**

### 3.5 Sparkle (macOS auto-update)

- **What / current:** Sparkle 2.9.x line (EdDSA). Latest published is **2.9.3** (2026-07). Prior research moved 2.9.0→2.9.3.
- **Target / action:** independent of the Chromium ABI, same as WinSparkle. Confirm the exact embedded Sparkle version; bump within the 2.9.x line only for a security fix. Same **signer-continuity** rule (macOS **Team ID unchanged**) — and note the **individual→org signing migration is PENDING** (`ORG_IDENTITY_SIGNING_MIGRATION.md`); if beta.1 is the first org-signed build, that is the reinstall-forcer to gate, not Sparkle's version.
- **Verification:** mac appcast validates; real N-1→N silent-on-quit apply with Team-ID continuity.
- **Recommended decision:** **carry current 2.9.x; decouple from the bump; verify Team-ID continuity in P6.**

### 3.6 Rust — wallet + adblock crates and the `rustc` toolchain

- **What / current:** both crates edition **2021**; `Cargo.lock` pins the graph; `hodos-adblock` hard-pins three exact crates, each for a **different** reason: `adblock =0.10.3` (**Rust-1.85 stability lock** — 0.10.4+ needs the unstable `unsigned_is_multiple_of`), `rmp =0.8.14` (**rmp-serde 0.15 compat**), and `actix-web =4.11.0` (in-repo comment: *"4.13+ requires Rust 1.88"* — this is the **explicit rustc ceiling**). **No `rust-toolchain.toml`** → `rustc` does **not** float with the runner image; it floats with the `dtolnay/rust-toolchain@stable` CI step (`release.yml` L142), i.e. whatever the stable channel is at build time.
- **Target / action:** Rust does **not** link CEF, so the Chromium ABI does not force a crate bump. The real risk is **platform/toolchain drift** (`CEF_BUILD_RUNBOOK.md` Step 5) + a floating `rustc` that either **(i) refuses to compile the deliberately held-back `adblock`/`rmp`/`actix-web` pins** or **(ii) drifts the effective toolchain outside their MSRV window** — Cargo cannot "force `rmp` forward" (the exact `=` pin + `Cargo.lock` hold it), so the failure mode is compile-refusal / MSRV drift, not an unwanted upgrade. **Add a `rust-toolchain.toml`** pinning the `rustc` channel (§4-D), and note it takes precedence over the `@stable` CI step for workspace cargo invocations. Bump crates only for a CVE (`cargo audit`) or an MSRV demand; keep the exact pins unless intentionally moving the adblock engine.
- **Compat / why:** the wallet is money-handling — crypto crates (`secp256k1`, `openssl`/`rustls`, `aes-gcm`) changes are Invariant #3-adjacent → do not bump silently. The three adblock pins are known-good compat locks; the **`actix-web =4.11.0` "4.13+ needs Rust 1.88" comment is the load-bearing rustc-ceiling evidence** that the channel choice is bounded (see §4-D). A `rustc` bump risks pushing the effective toolchain past a pin's MSRV window (too new for the `unsigned_is_multiple_of` avoidance, or below what a crate demands) — a compile break, not a silent upgrade.
- **Ripple:** a `rustc` MSRV change can cascade through the whole lock; run `cargo build --release` + `cargo test` on both crates on the pinned toolchain.
- **Verification:** `cargo build --release` + `cargo test` green on both crates on the pinned `rustc`; `cargo audit` clean; wallet send/recv smoke; adblock `/health` + a block check.
- **Recommended decision:** **pin `rustc` via `rust-toolchain.toml`; hold crate versions; bump only for CVE/MSRV; NEVER auto-bump crypto crates (ask — Invariant #3).**

### 3.7 Frontend — React / MUI / Vite / TypeScript

- **What / current:** React **19.1**, react-router-dom **7.6.1**, MUI **7.1.1**/icons 7.3.9, Emotion **11.14**, TS **~5.8.3**, Vite **^6.3.5**; all lockfile-pinned.
- **Target / action:** the frontend renders **inside the CEF renderer**, so the only real coupling is **browser-API surface** the new Chromium exposes/removes (a Blink behavior change, not an npm version). No npm bump is *forced* by the Chromium jump. Optionally align Vite/TS to current for security, but **not on the critical path** — hold for beta.1 unless a CVE (`npm audit`) says otherwise.
- **Compat / why:** watch for any JS/TS that relies on a browser API the new Chromium changed (the outline §3e "browser-API-dependent JS" flag) — e.g. a farbled/removed `navigator` field (ties to the C6 value table). Grep the frontend for direct `navigator.*` / `canvas` / `webgl` usage during the F-audit.
- **Verification:** `npm run build` clean (type-check + bundle); render smoke on the target build (header paints, overlays open, wallet flows); `npm test` (Playwright e2e) in the smoke gate against the built browser.
- **Recommended decision:** **hold frontend versions for beta.1; audit browser-API-dependent code against the target Chromium; bump only for a CVE.**

### 3.8 Inno Setup (Windows installer compiler)

- **What / current:** **unpinned** — `choco install innosetup -y` grabs newest (6.7.x today; **7.0.1-beta published 2026-05**). `.iss` uses `WizardStyle=modern`, `#define AppVersion`, standard 6.x directives.
- **Target / action:** **PIN IT** (§4-B). Independent of Chromium, but a `choco`-served **Inno 7 major** could break the `.iss` at compile time on a random future run. Pin to a specific 6.7.x (or a validated 7.x) via `choco install innosetup --version=<x>`.
- **Compat / why:** Inno 7 changed the compiler/IDE and some directives; a silent major bump is the same drift class as the runner-image lesson. The `.iss` is also where **VER-6 version single-sourcing** injects the tag — keep them consistent.
- **Verification:** `ISCC.exe` compiles `hodos-browser.iss` clean; installer runs; DSA + EdDSA sidecar signing intact.
- **Recommended decision:** **pin Inno to a specific 6.7.x for beta.1; evaluate 7.x on its own track, not folded into the bump.**

---

## 4. Close the four silent-drift holes FIRST (highest-leverage re-pins)

These are the ❌ rows in §2. A Chromium bump is precisely when a floating input rolls forward and produces a failure that "looks like our code." Fix these **before** starting the dependency verification pass, so the pass runs against a reproducible baseline.

### 4-A. Pin the vcpkg baseline (currently: runner's pre-installed vcpkg)
- **Problem:** CI runs `vcpkg install …` against `$VCPKG_INSTALLATION_ROOT` (the **runner image's** vcpkg checkout) — so the dep versions are whatever that image's baseline resolves. A runner-image refresh silently rolls OpenSSL/sqlite3/nlohmann-json forward. This is the beta.16 drift class applied to deps.
- **Fix (recommended):** add a **vcpkg manifest** — `cef-native/vcpkg.json` with `"builtin-baseline": "<commit>"` and the three deps + `overrides` pinning exact versions **including `port-version`** (`openssl 3.6.0#3`, `sqlite3 3.51.1`, `nlohmann-json 3.12.0#1` — the `#N` is the vcpkg port-version; pinning only `3.6.0`/`3.12.0` without `"port-version": 3`/`1` is **not** byte-exact), plus a `vcpkg-configuration.json` if using a specific registry commit. Switch CI to manifest mode (`vcpkg install` with no args, toolchain-driven). This makes the dep set reproducible and diffable per bump.
- **Acceptance:** `vcpkg install` from the manifest resolves the recorded versions **and port-versions** deterministically — the manifest makes this true *by construction* (a `builtin-baseline` + `overrides` is reproducible without waiting; a "different days" cross-check tests nothing the manifest doesn't already guarantee). CI asserts `vcpkg list` matches the manifest; the manifest is the single source the verification pass diffs against.

### 4-B. Pin Inno Setup (currently: `choco install innosetup` → newest)
- **Fix:** `choco install innosetup --version=<pinned 6.7.x> -y`. Record the pin next to the runner pin comment in `release.yml`.
- **Acceptance:** installer compiles on a pinned Inno; a future `choco` publishing Inno 7 cannot silently change the build.

### 4-C. Pin macOS Homebrew deps (currently: `brew install` → newest)
- **Fix:** pin formula versions (`brew install openssl@3 …` at a recorded version, or a `Brewfile` with pinned versions) so the mac build is as reproducible as the Windows vcpkg manifest.
- **Acceptance:** mac dep versions recorded + reproducible; minos guard (VER-4) still passes.

### 4-D. Add `rust-toolchain.toml` (currently: `rustc` floats with `dtolnay/rust-toolchain@stable`)
- **Problem:** there is no `rust-toolchain.toml`, so `rustc` is whatever the `dtolnay/rust-toolchain@stable` CI step (`release.yml` L142) resolves at build time — the **stable channel on the build date**, not the runner image. That channel can roll past the MSRV window of the deliberately held-back pins (`adblock =0.10.3` / `rmp =0.8.14` / **`actix-web =4.11.0`, whose comment records "4.13+ requires Rust 1.88"** — the explicit ceiling this pin protects).
- **Fix:** add `rust-toolchain.toml` at the workspace root(s) pinning `channel = "<stable x.y.z>"` (+ components). This locks `rustc` for both `rust-wallet` and `adblock-engine`, protecting the three exact pins above. **A `rust-toolchain.toml` takes precedence over the `@stable` step for workspace cargo invocations**, so also change or remove the `dtolnay/rust-toolchain@stable` step (or pin it to the same version) — otherwise the CI step and the file are redundant/conflicting and the intent is unclear.
- **Acceptance:** `rustc --version` is deterministic in CI and locally, resolving the pinned channel (not the drifting `@stable`); `cargo test` green on the pinned channel.

> **Why these four are in DEP-1 and not VER-*:** they are *dependency-reproducibility* fixes (what versions we consume), distinct from VER-3's *runner* pin (what compiler builds them). VER-3 pins the toolset; §4 pins the deps that ride on it. Both are needed; neither substitutes for the other.

---

## 5. Per-bump verification checklist (the runnable procedure)

Operationalizes `DEPENDENCY_VERIFICATION.md` §"Per-dependency checklist" into ordered, gated steps. Run **after** §4 re-pins land and **after** the target CEF binary + rebuilt wrapper exist (runbook Step 4), i.e. the DEP-1 slot in §6.

1. **Record the target toolset.** From the target CEF release notes + `index.json`: note the MSVC/Clang version the target `libcef` was built with, its minimum C++ std, and (mac) the framework's `vtool` `minos`. This is the contract every dep is checked against.
2. **Rebuild the CEF wrapper** (§3.1) — delete `build/`, reconfigure, rebuild; confirm `.lib`.
3. **Resolve deps against the pinned baseline** (§4-A): manifest-mode `vcpkg install`; `vcpkg list` → confirm `openssl`/`sqlite3`/`nlohmann-json` versions + `x64-windows-static` triplet. macOS: pinned brew (§4-C).
4. **Answer the 7 questions in writing** for each dep (§3) — version, ABI/toolchain compat, why-this-version, ripple, conflicts, verification, decision.
5. **Compile + link gate:** `cef-native`, `hodos-update-helper`, and `cef-native/tests` all configure + build on the matched toolset. A `/MD`↔`/MT` mismatch surfaces here (the OpenSSL static-CRT check).
6. **Compile/unit-test gate:** `ctest` (C++), `cargo test` on both Rust crates (pinned `rustc`, §4-D), `npm run build` (frontend type-check + bundle). `cargo audit` + `npm audit` for CVEs. **Note:** `package.json`'s `"test": "playwright test"` is a **Playwright e2e** suite (`@playwright/test ^1.58.2`) that needs the built browser/dev server + downloaded Playwright browsers — it is an **app-behavior gate, not a unit check**, so it belongs in the smoke gate (step 8), not here. The DEP-1 frontend gate is `npm run build` + a targeted render smoke.
7. **Runtime/manifest cross-check (hand-off to VER-5):** diff the target CEF dist's DLL/`.pak`/`.bin`/`resources`/`locales` against the hardcoded copy-lists in `cef-native/CMakeLists.txt` + the mac framework-embed list. A dep-adjacent runtime file we don't copy = green build / runtime crash.
8. **Smoke gate:** launch the built browser; wallet send/recv; adblock block check; the CLAUDE.md Minimal site basket (youtube/x/github) on **both** OSes. Optionally run `npm test` (Playwright e2e) here against the built browser/dev server, since it exercises app behavior rather than compilation.
9. **Auto-update gate (hand-off to P6):** real N-1→N apply with **signer continuity** (Win CN / mac Team ID unchanged) — WinSparkle/Sparkle version and the pending mac org-signing migration are the reinstall-forcers to watch.
10. **Record.** Append the per-dep old→new/verdict table to `CEF_VERSION_UPDATE_TRACKER.md` (the living log) + document any surprise back into `DEPENDENCY_VERIFICATION.md` (Invariant #12).

> **Milestone vs point-release depth** (`DEPENDENCY_VERIFICATION.md` §"When to run"): a **milestone jump** (M136→target LTS) runs the **full** pass above. A **quarterly security point-release** within the pinned LTS runs a **light** pass — most deps unchanged; confirm nothing shifted (steps 1-3, 5, 8).

---

## 6. Where this sits in the phase plan (DEP-1 slot)

Per `Q5_full_edit_list.md` §C ordering and outline §4: **DEP-1 rides with the bump — "after P2 fetch, before P6"**, i.e. after the target CEF binary is fetched/built and the wrapper is rebuilt (P2), before the P6 test gate. Concretely:

```
P0 provision+pin ─▶ P1 baseline ─▶ P2 bump (VER-1..6 + WRAPPER REBUILD)
                                        │
                        ┌───────────────┴───────────────┐
                        ▼                                ▼
              §4 re-pins + DEP-1              P3 PATCH TOOLCHAIN (PIPE-A1)
         (vcpkg manifest, Inno, brew,        (re-apply/refresh source patches
          rust-toolchain → §5 verify pass)    onto the target Chromium tree)
                        │                                │
                        │                          P3 blocks P4
                        └───────────────┬────────────────┘
                                        ▼
                        P4 farbling ∥ P5 codecs/DRM ──▶ P6 TEST (incl. auto-update signer-continuity gate)
                                                              │
                                                        P7 prod ─▶ [GATE] v0.4.0-beta.1
```

> **P3 is not optional and not deleted from the flow.** Per the outline §4 (L286), **P3 = PATCH TOOLCHAIN (PIPE-A1)** is a real blocking phase between P2 (bump) and P4 (farbling): "P2 … blocks P3+", "P3 before P4." **DEP-1 has no dependency on the patch toolchain**, so it runs in parallel with (or ahead of) P3 — but P3 must not be dropped: the farbling/codec phases (P4/P5) cannot start until the patch toolchain is re-applied to the target Chromium tree.

**Serial note:** §4-A/B/C/D re-pins should land **as their own small commits before DEP-1** so the verification pass runs against a reproducible baseline — pinning *during* the pass reintroduces the drift the pass is trying to catch.

---

## 7. Acceptance criteria (DEP-1 done when)

- [ ] CEF wrapper **rebuilt from the target dist** on the matched toolset; `cef-native` links; no "Unsupported CEF version".
- [ ] vcpkg deps resolve from a **pinned baseline/manifest** (§4-A) at the recorded versions + `x64-windows-static`; two runs are identical.
- [ ] Inno Setup (§4-B), macOS brew (§4-C), and `rustc` (§4-D) all **pinned**; recorded in `release.yml`/`Brewfile`/`rust-toolchain.toml`.
- [ ] Compile + link + `ctest` + `cargo test` (both crates) + `npm run build` all green on the matched toolchains; the Playwright e2e (`npm test`) is run in the smoke gate against the built browser, not counted as a compile/unit gate.
- [ ] `cargo audit` + `npm audit` reviewed; no unaddressed high/critical.
- [ ] The 7-question record filled for every dep and the old→new/verdict table appended to `CEF_VERSION_UPDATE_TRACKER.md`.
- [ ] Crypto crates (wallet) confirmed **not silently bumped** (Invariant #3) — any change explicitly approved.
- [ ] Hand-offs confirmed: VER-4 (minos), VER-5 (file manifest), P6 (auto-update signer continuity) tracked as their own rows, not silently absorbed.

---

## 8. Risks

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| Floating vcpkg baseline rolls a dep forward mid-bump → ABI/CRT mismatch that "looks like our bug" | **Med-High** (unpinned today) | build-breaker, hours lost | §4-A manifest + baseline pin **before** DEP-1 |
| `choco` serves Inno **7.0** → `.iss` fails to compile on a routine build | Med (7.x already published) | red release build | §4-B pin Inno to 6.7.x |
| `/MD` vs `/MT` CRT mismatch (OpenSSL) → `hodos-update-helper` link failure | Med | build-breaker | keep `x64-windows-static`; force `OPENSSL_ROOT_DIR` to static triplet (already done, L200) — re-verify post-bump |
| Target CEF raises minimum C++ std (17→20) | Low-Med | wide recompile | step 1 records the std; bump `CMAKE_CXX_STANDARD` + re-verify deps compile (OQ-3) |
| Floating `rustc` breaks the `adblock =0.10.3`/`rmp =0.8.14` exact-pin combo | Low-Med | adblock engine breakage | §4-D `rust-toolchain.toml` |
| Silent crypto-crate bump in the wallet (money path) | Low | correctness/security | Invariant #3 — hold + ask; `cargo audit` review only |
| WinSparkle/Sparkle or the **pending mac org-signing** change forces a reinstall while the test passes on a proxy | Med | fleet-wide forced reinstall | P6 real N-1→N apply with **signer-continuity** assertion (Win CN / mac Team ID) — not a proxy |
| Framework runtime file added/renamed and not in our copy-list | Med | green build / runtime crash | VER-5 file-manifest drift audit (cross-ref) |

---

## 9. Open questions (with recommended defaults)

| # | Question | Recommended default |
|---|---|---|
| DQ-1 | Adopt a full **vcpkg manifest** (`vcpkg.json` + baseline) or just pin the existing classic-mode install? | **Manifest** — it is the only reproducible option and makes the per-bump diff trivial; small one-time cost. |
| DQ-2 | Bump **OpenSSL** past 3.6.0 on this jump? | **Only for a CVE.** Hold otherwise; the `/MT` static triplet is the load-bearing property, not the point version. Track the OpenSSL 3.x advisory line. |
| DQ-3 | Does the target CEF require **C++20**? | **Look it up at Step 0** from the target CEF's `CMakeLists`/docs. If yes, bump `CMAKE_CXX_STANDARD` to 20 and re-verify the three vcpkg deps + quirc compile; if no, hold at 17. |
| DQ-4 | Bump **WinSparkle 0.8.1 → 0.9.x** now? | **No — decouple.** Sequence the DSA→EdDSA cutover on its own track; do not couple it to the Chromium jump. beta.1 carries the current dual-signed setup. |
| DQ-5 | Bump **Vite/TS/MUI/React** for beta.1? | **No.** Frontend is not ABI-coupled to Chromium; hold lockfile versions; bump only for a `npm audit` CVE. Audit browser-API-dependent JS against the target Chromium instead. |
| DQ-6 | Pin **Inno 7.x** or stay on **6.7.x**? | **6.7.x for beta.1** (validated `.iss` directives); evaluate 7.x separately — a major installer-compiler change is its own test surface. |
| DQ-7 | Where does the **mac org-signing migration** land relative to beta.1? | Confirm before P6: if beta.1 is the first org-signed build, that reinstall is **accepted + announced** (not a silent regression) — see `ORG_IDENTITY_SIGNING_MIGRATION.md`; do not let it masquerade as a routine auto-update. |

---

## 10. Feeds

`Q5_full_edit_list.md` **DEP-1 row** (this is its detailing doc) and the outline §7 readiness checklist (dependency slice). Reconcile the §2 "current pins" table into `CEF_VERSION_UPDATE_TRACKER.md`'s living log at execution time. Hand-offs: **VER-4** (minos), **VER-5** (file-manifest drift), **VER-6** (version single-sourcing into the `.iss`), and the **P6 auto-update signer-continuity gate** are owned by their own rows — this doc references, does not duplicate them.

---

### Sources (primary)
- Hodos canonical checklist: `development-docs/DevOps-CICD/DEPENDENCY_VERIFICATION.md` (the per-bump procedure this operationalizes)
- Hodos build runbook (wrapper rebuild + dep reconciliation + Step 5.5 drift): `development-docs/DevOps-CICD/CEF_BUILD_RUNBOOK.md`
- Hodos toolchain/dependency-alignment lesson: `development-docs/DevOps-CICD/CEF_VERSION_UPDATE_TRACKER.md`
- Observed current pins: `build-instructions/WINDOWS_BUILD_INSTRUCTIONS.md` (vcpkg versions L73-75), `MACOS_BUILD_INSTRUCTIONS.md`, `.github/workflows/release.yml`, `installer/hodos-browser.iss`, `cef-native/CMakeLists.txt`, `rust-wallet/Cargo.toml`, `adblock-engine/Cargo.toml`, `frontend/package.json`
- vcpkg manifest mode + builtin-baseline: https://learn.microsoft.com/vcpkg/concepts/manifest-mode · https://learn.microsoft.com/vcpkg/reference/vcpkg-json
- WinSparkle releases + EdDSA: https://github.com/vslavik/winsparkle/releases · https://github.com/vslavik/winsparkle/issues/187
- Sparkle (macOS) releases (2.9.3 current, 2026-07): https://github.com/sparkle-project/Sparkle/releases · https://sparkle-project.org/documentation/publishing/
- Inno Setup downloads + revision history (6.7.x current, 7.0 beta 2026-05): https://jrsoftware.org/isdl.php · https://jrsoftware.org/files/is6-whatsnew.htm
- CEF version→branch lookup (use `index.json`): https://cef-builds.spotifycdn.com/index.json
- Rust toolchain pinning: https://rust-lang.github.io/rustup/overrides.html#the-toolchain-file

*This doc stops at a followable plan; the implementing session runs §5 against the real TARGET build and fills the verification/decision fields.*
