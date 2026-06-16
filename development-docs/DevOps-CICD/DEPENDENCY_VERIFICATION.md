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

## Automation goal (0.4.0 target)
This checklist should become **scripted + test-gated** so it runs the same way every time:
- a script that enumerates the pinned versions across all 5 layers and diffs against the new CEF's expected toolchain,
- compile + link + `cargo test` + `ctest` + frontend tests as the pass/fail gate,
- a generated report that drops into the version tracker.
Until automated, run it by hand against this checklist and record results.
