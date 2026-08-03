# Dependency Verification — Procedure (run on every CEF bump)

**Created:** 2026-06-16 · **Owner:** DevOps/CI-CD · **Canonical home:** `development-docs/DevOps-CICD/`
**Per root CLAUDE.md Invariant #12** — keep this current; append lessons learned each time.

> **Why this exists.** The hard part of a CEF/Chromium bump is **not** Chromium's *internal* dependencies (`automate-git.py`/gclient resolve those for the pinned branch automatically). It's **Hodos's *own* dependencies** staying compatible with the new CEF's C++ ABI, toolchain, and headers. This procedure makes that a repeatable, auditable checklist instead of tribal knowledge — runnable by a small team or a small team of AI agents, with verification at each step.

## When to run
- Every **milestone jump** (new CEF LTS branch, e.g. M150 → M156) — full pass.
- Every **quarterly security point-release** within the pinned LTS — lighter pass (most deps unchanged; confirm nothing shifted).
- See `CEF_BUILD_RUNBOOK.md` for the surrounding build flow and the LTS cadence rationale.

## The dependency inventory (Hodos-owned)
| Layer | Dependency | Where pinned |
|-------|-----------|--------------|
| CEF binding | `libcef_dll_wrapper` (must match `libcef` version exactly) | CEF binary distrib + `cef-native/CMakeLists.txt` |
| C++ libs (vcpkg) | nlohmann-json, sqlite3, OpenSSL, quirc, + others | `vcpkg.json` / vcpkg baseline |
| Toolchain | MSVC / Windows SDK (Win); Xcode/clang + min macOS (Mac); C++ std version | build env + CMake |
| Frontend | React, react-dom, react-router, Vite, TypeScript, MUI/Emotion | `frontend/package.json` + lockfile |
| Rust | wallet + adblock crates | `Cargo.toml` + `Cargo.lock` |

## Per-dependency checklist — answer IN WRITING for each
For **every** dependency above, record:
1. **What is it + current version?** (and the new/target version if changing.)
2. **Is it compatible with the new CEF/Chromium ABI + toolchain?** (compiler, CRT, Windows SDK, C++ std, min-macOS all match what the new CEF was built against?)
3. **Is this the right version — and *why* this one?** (pinned for a reason? a transitive constraint? matches what CEF expects?)
4. **What else does bumping it affect?** (ripple to other deps, changed APIs, behavior changes, removed/renamed symbols.)
5. **Any conflict?** (two deps wanting different versions of a shared lib; ABI mismatch; duplicate symbols.)
6. **Verification performed** (compiles? links? unit/integration tests pass? smoke test?) — record the result.
7. **Decision + record** so the next bump starts from a known baseline, not from scratch.

## Output
- A short table appended to `CEF_VERSION_UPDATE_TRACKER.md` (the living log): each dep, old→new version, verdict, notes.
- Any surprise/breakage → **document the lesson here** and update the runbook (Invariant #12).

---

## DEP-1a..d — the silent-drift pins (landed 2026-08-03, pre-`7871`-build)

Before this pass, **four of the five inventory layers floated.** The checklist above asks "is this
the right version and why" — but there was no pinned answer to compare against, so a re-run could
silently get different versions with no diff anywhere in the repo. These pins give each layer a
declared version so drift becomes a reviewable change.

**Principle: every pin below records what the floating command already resolved to on 2026-08-03.**
None of them is an upgrade. That is deliberate — a pin and a bump must never land in the same
commit, or a build break is ambiguous between the two.

| ID | Layer | Was | Now | Where |
|---|---|---|---|---|
| **DEP-1a** | C++ / vcpkg | classic mode, whatever the runner image shipped | manifest mode, `builtin-baseline` + exact `overrides` (nlohmann-json 3.12.0#2, sqlite3 3.53.4, openssl 3.6.3) | `cef-native/vcpkg.json` |
| **DEP-1b** | Installer | `choco install innosetup` (floating) | `--version=6.7.1` | `release.yml` |
| **DEP-1c** | macOS libs | `brew install openssl nlohmann-json sqlite3` (floating) | `brew bundle --file=Brewfile` | `/Brewfile` |
| **DEP-1d** | Rust toolchain | `dtolnay/rust-toolchain@stable` ×4 | `channel = "1.97.1"` | `rust-wallet/`, `adblock-engine/rust-toolchain.toml` |
| *(extra)* | Rust crates | `actix-web = "4.9"` caret, held only by `Cargo.lock` | `= "=4.11.0"` | `rust-wallet/Cargo.toml` |
| *(extra)* | CI runners | commented-out `*-latest` in disabled `test.yml` stubs | pinned images | `test.yml` |

### Verification performed
- **Rust:** `cargo build --release` green on 1.97.1 for **both** workspaces (wallet 3m00s). **Both
  `Cargo.lock` files unchanged** — proving the `actix-web` exact pin records the existing resolution
  rather than moving it.
- **vcpkg / Inno / Brew: NOT verifiable locally.** These only execute in the release workflow. Their
  first real exercise is the next release build — treat a failure there as *this* change, not as a
  CEF-bump symptom.

### Lessons
- **A crate pin without a compiler pin is half a pin.** `adblock-engine` already had exact crate
  pins (`adblock = "=0.10.3"`, `rmp = "=0.8.14"`) chosen to hold an MSRV-sensitive graph together,
  while the toolchain that had to satisfy that MSRV floated. Pin both or neither.
- **Chocolatey lags upstream — check the packaging source, not the project.** Inno Setup upstream is
  on 7.0.x (7.0.0 released 2026-05-18), but Chocolatey's `innosetup` package tops out at **6.7.1**.
  Pinning to the upstream-latest would have produced an uninstallable version string. Always read
  the version list of the *channel you install through*.
- **Commented-out YAML still drifts.** Disabled job stubs carrying `*-latest` are a copy-paste
  source that re-introduces floating images the moment someone enables the job. Pin dead code too.
- **Adding `vcpkg.json` changes vcpkg's MODE, not just its versions.** Manifest mode auto-activates
  from the file's mere presence next to `CMakeLists.txt`, so the CI step that ran a classic
  `vcpkg install` had to be replaced in the same commit — otherwise classic and manifest installs
  fight over the same `find_package` resolution. Also note `builtin-baseline` must exist in the
  runner's vcpkg git history; the workflow now fetches that exact commit before configure.

## Automation goal (0.4.0 target)
This checklist should become **scripted + test-gated** so it runs the same way every time:
- a script that enumerates the pinned versions across all 5 layers and diffs against the new CEF's expected toolchain,
- compile + link + `cargo test` + `ctest` + frontend tests as the pass/fail gate,
- a generated report that drops into the version tracker.
Until automated, run it by hand against this checklist and record results.
