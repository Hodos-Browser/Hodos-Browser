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
  - ✅ **That exercise happened: `v0.4.0-beta.2`, 2026-08-17.** All three passed — vcpkg resolved the
    manifest baseline, Inno 6.7.1 built the installer, and `brew bundle` provisioned the macOS
    dependencies. Both platform builds went green. *(The run's `publish` job failed later, at the
    draft-release lookup, for an unrelated GitHub-incident reason — see BUILD_AND_RELEASE.md
    "Known CI flake". Nothing dependency-related.)*
  - It was also exercised once more, earlier the same day and with no release attached, by the
    `workflow_dispatch` validation run `31948482218` — which is what that trigger is for.

### Freshness review — 2026-08-17 (the first one)

Queried upstream directly rather than reasoning from the pin dates. **The posture is better than
"we froze and forgot" suggested — most pins are current.** Two real items.

| Dependency | Pinned | Latest upstream | Verdict |
|---|---|---|---|
| **OpenSSL** | `3.6.3` | `4.0.1` | ✅ **CURRENT.** `3.6.3` shipped **2026-06-09, the same day as 4.0.1** — 3.6 is an actively maintained branch getting simultaneous security releases, and we are on its newest patch. "A major behind" is the wrong reading. |
| **SQLite** | `3.53.4` | `3.53.4` | ✅ current |
| **nlohmann-json** | `3.12.0` | `3.12.0` | ✅ current |
| **Rust** | `1.97.1` | `1.97.1` | ✅ current |
| **Node** | ~~`20`~~ → **`22`** | `26.7.0` | ✅ **FIXED 2026-08-17.** Was EOL since 2026-04-30; bumped to 22 (maintained to 2027-04-30). |
| **Sparkle** (macOS updater) | ~~`2.9.3`~~ → **`2.9.6`** | `2.9.6` | ✅ bumped 2026-08-17 — ⛔ **unverified on a real macOS build** |
| **WinSparkle** | tool ~~`0.9.3`~~ → **`0.9.4`**; shipped dll `0.8.1` unchanged | `0.9.4` | ✅ bumped + round-trip verified 2026-08-17 |
| **Inno Setup** | `6.7.1` | `7.1.0` | ⏸️ **deliberate hold** — Chocolatey lags upstream and 7.x is a major compiler change; do not bump without re-validating `hodos-browser.iss` |

#### 🚨 Node 20 is end-of-life

Node 20 reached EOL on **2026-04-30** (`nodejs/Release` schedule: maintenance from 2024-10-22, end
2026-04-30). It has received no security patches since. `release.yml` pins `node-version: '20'` on
both build arms.

⚠️ **Scope it accurately before panicking:** Node is **build-time only**. `npm run build` is
`tsc -b && vite build`, which emits static assets; no Node runtime ships in the installer (nothing
node-shaped appears in `hodos-browser.iss`). So the exposure is **build-toolchain integrity**, not a
vulnerability in the shipped browser. That is a real supply-chain concern for software that handles
money — an EOL toolchain builds the UI that renders wallet state — but it is not a user-facing CVE.

✅ **DONE 2026-08-17 — bumped to Node 22** (maintained to 2027-04-30) on both build arms. Not 24 or
26: 22 is the LTS-track option with the longest runway that is not still moving.

⚠️ **Not yet exercised in CI** — the dev fork's Actions quota is exhausted and `release.yml` only
runs Node on a tag build or a `workflow_dispatch` validation run. The frontend build was verified
**locally** (see below). Treat the first CI frontend build after this as the real confirmation.

#### Sparkle / WinSparkle — bumped 2026-08-17, and what that is and is NOT backed by

⛔ **The Stage-1 update rigs cannot test either of these. Do not read a green rig as covering them.**
Checked before running rather than after:

| Rig | What it actually drives | Covers the bump? |
|---|---|---|
| `test-apply-forward.ps1` / `test-apply-rollback.ps1` | our **custom `hodos-update-helper.exe`** transaction | ❌ never touches WinSparkle |
| `test-update-feed.ps1` | our **custom `UpdateStager`**, with throwaway test-seam keys | ❌ never invokes `winsparkle-tool` |
| any Windows rig | — | ❌ **Sparkle is macOS-only** |

They were also **not runnable** at the time: `hodos_tests.exe` does not exist (needs
`-DHODOS_BUILD_TESTS=ON`), no rig build of the helper exists (needs `-DHODOS_UPDATE_TEST_SEAM=ON`),
and ports **31301/31302/31401 were all listening** — the rigs abort on that by design, because
rollback **POSTs `/shutdown`** and would take down a live wallet.

**What the WinSparkle tool bump IS backed by** — a real functional round-trip against 0.9.4, using
release.yml's exact call shapes:

```
generate-key -f      -> 44-byte key  (32-byte-seed format; SPARKLE_EDDSA_PRIVATE_KEY stays compatible)
public-key   -f      -> derives pubkey   (the call release.yml self-checks against SUPublicEDKey)
sign <file>  -f      -> signature
verify               -> "Valid signature."
```

with negative controls, all three of which correctly **FAILED**: tampered payload, corrupted
signature, wrong public key. Plus the archive-shape check that matters most — release.yml's EdDSA
step **soft-skips** on a missing tool, so a wrong path would silently ship a **DSA-only feed** rather
than fail the build.

**What the Sparkle bump IS backed by** — a layout pre-flight only: `bin/sign_update`,
`Sparkle.framework/Versions/{B,Current}` and `Versions/B/{Autoupdate,Updater.app,XPCServices}` all
present and unchanged in 2.9.6, and `sign_update` still carries `--ed-key-file` and still emits
`sparkle:edSignature`. ⛔ **It has not run on macOS.** The first macOS tag build is the real
confirmation — watch the "EdDSA sign DMG" step, and do a real N−1 → N update on a Mac before
promoting.

#### ⚠️ Why these two matter more per-patch than their small deltas suggest

These are the **auto-update** libraries. A defect there does not break a page — it breaks the
mechanism by which every user receives every future fix, and this project's standing principle is
that auto-update must never brick an install. Being 3 patches behind on Sparkle is a bigger deal than
being a minor behind on a JSON header. Check each release note before bumping, and re-run
`SILENT_UPDATE_TEST_PLAN.md`'s Stage 1 rigs afterwards.

#### The one place a version relationship with the engine really exists — ✅ CLOSED 2026-08-17

The React bundle runs **inside** the shipped Chromium's V8, so its output must be syntax that engine
supports. Nothing tied the Vite build target to the CEF version; it was safe only because Chromium
150 is far newer than anything Vite targets by default — safe **by accident, not by construction**.

Now bound in `frontend/vite.config.ts`:

```ts
build: { target: 'chrome150', ... }
```

⛔ **`build.target` is the load-bearing setting — NOT the `browserslist` field.** Verified before
adding it: this project has **no** postcss, autoprefixer, lightningcss, babel or `plugin-legacy`, so
**nothing currently reads `browserslist`**. It is added to `package.json` as declaration-of-intent
(and so any future tool inherits the right target), but on its own it would have been decorative —
the exact "a doc example is not evidence the code does it" trap this file warns about elsewhere.

**Measured, not assumed** — full frontend build, same tree, target off vs on:

| | total JS emitted |
|---|---|
| Vite default target | 1,052,119 bytes |
| `target: 'chrome150'` | **1,040,689 bytes** |

11,430 bytes smaller (1.09%), **every chunk shrank and every content hash changed** — esbuild stopped
down-levelling syntax Chromium 150 supports natively. The setting is demonstrably live.

⛔ **Bump `target` in lockstep with the CEF pin.** It now also catches the reverse case: a dependency
emitting syntax *newer* than our engine fails at build time instead of surfacing as a blank overlay.

#### Method — repeat this at every engine bump and quarterly

```bash
gh api repos/openssl/openssl/releases/latest      --jq .tag_name
gh api repos/nlohmann/json/releases/latest        --jq .tag_name
gh api repos/rust-lang/rust/releases/latest       --jq .tag_name
gh api repos/sqlite/sqlite/tags                   --jq '.[0].name'
gh api repos/sparkle-project/Sparkle/releases/latest --jq .tag_name
gh api repos/vslavik/winsparkle/releases/latest   --jq .tag_name
gh api repos/jrsoftware/issrc/releases/latest     --jq .tag_name
curl -sS https://raw.githubusercontent.com/nodejs/Release/main/schedule.json   # EOL, not "latest"
```

⛔ **For runtimes, read the EOL schedule, not the latest version number.** Node 20 looked fine by
"is it still widely used?" and was four months past end-of-life. `latest` tells you how far behind
you are; **EOL tells you whether you are getting security fixes at all.**

⛔ **And read the branch, not just the number.** OpenSSL `3.6.3` vs `4.0.1` looks alarming and is
fine. A raw latest-vs-pinned diff will generate false alarms on any project with parallel maintained
branches.

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
