# DevOps / CI-CD — Canonical Process Home

This folder is the **permanent, canonical home** for Hodos Browser's Process & Procedures (P&P): build, release, signing, auto-update, CEF source-build, dependency verification, and testing strategy. Per `CLAUDE.md` invariant #12, whenever a build/dependency/codec/signing/release/auto-update step surprises us or teaches us something, the lesson is written down **here** and the relevant runbook updated. Treat P&P as code: keep it current or it rots. Sprint docs (e.g. `../0.4.0/`) only point here.

**Status (2026-07-09):** Latest shipped = **`v0.3.0-beta.26` (LATEST/promoted, live).** **Silent auto-update is DONE and PROVEN LIVE on both platforms** (Windows beta.25 → beta.26 through the two-process profile picker on real hardware; macOS beta.21 → beta.22). Release is fully automated: a `v*` tag push produces a draft build; a manual promote gate flips it to Latest and publishes the website (appcast/redirects). Version is **tag-derived**.

---

## Document index

### Build & Release
| Doc | Purpose | Status |
|-----|---------|--------|
| `BUILD_AND_RELEASE.md` | Canonical build + release guide (tag-derived version, draft → manual-promote gate, branch flow) | Living |
| `CEF_BUILD_RUNBOOK.md` | Tier-1 custom Chromium/CEF source-build runbook | **Current** — linchpin for the next CEF rebuild sprint |
| `CEF_VERSION_UPDATE_TRACKER.md` | Institutional-memory log for CEF bumps (toolchain, minos floor, FedCM) | **Current** — CEF 136 baseline; rebuild-target version TBD in the sprint |
| `DEPENDENCY_VERIFICATION.md` | Timeless per-CEF-bump dependency-verification procedure | **Current** |
| `ORG_IDENTITY_SIGNING_MIGRATION.md` | Signing-identity migration gate (Win done CN=Marston; macOS individual→org still pending BEFORE first public signed 0.4.0) | **Active gate** |

### Auto-update (silent — SHIPPED + PROVEN LIVE)
| Doc | Purpose | Status |
|-----|---------|--------|
| `AUTO_UPDATE.md` | Canonical cross-platform auto-update P&P | Living — silent DONE + live (Win+Mac) |
| `WINDOWS_AUTOUPDATE_PLAN.md` | Design-of-record for the Windows silent hybrid updater | **SHIPPED + LIVE** (incl. picker-gate resolution) |
| `SILENT_UPDATE_TEST_PLAN.md` | Staged de-risking ladder + reusable apply/stage test rigs | Ladder climbed; rigs kept as living regression procedure |
| `WALLET_GRACEFUL_EXIT_SPEC.md` | Wallet clean-self-exit subsystem (OD-2) underpinning the updater's image-lock release | Implemented/shipped (synchronous=FULL) |

### Testing
| Doc | Purpose | Status |
|-----|---------|--------|
| `TESTING.md` | Canonical cross-stack testing strategy (living DevOps-home doc, invariant #12) | **Current** |
| `TEST_PLAN.md` | Detailed test catalog / manual-QA checklists (companion to TESTING.md) | **Current** |
| `TRIAGE.md` | Master triage/roadmap for DevOps + 0.4.0 process items (A1–A7, B1–B4) | Living |

### Research (feeds the queued Chromium/CEF rebuild sprint)
| Doc | Purpose | Status |
|-----|---------|--------|
| `research/BRAVE_FORK_FEASIBILITY.md` | Keystone A4 decision — stay on CEF, farbling via Blink patches in our self-build | **Current** — load-bearing for the rebuild |
| `research/A1_BUILD_STRATEGY.md` | Build strategy (local + sccache, M1 mac) feeding A1/A2 | **Current** (revisit figures at CEF-branch selection) |
| `research/A6_AUTO_UPDATE.md` | Auto-update research (EdDSA appcast, security requirements, Velopack rejection) | Reference — Windows went hybrid custom, not WinSparkle-silent |
| `WSL_HYBRID_WORKSPACE.md` | Future runbook for the Edwin recall workspace | **Planned** (execute post-sprint) |

> **Archived (moved to `../0.4.0/archive/`):** the pre-code design + build journals whose subjects have since shipped and stabilized — `AUTOUPDATE_6B_SUPERVISOR_DESIGN.md` (rollback supervisor, live), `AUTOUPDATE_PICKER_GATE_DESIGN.md` (picker-exit-wait fix, shipped `ae5beb6` / proven beta.26), `AUTOUPDATE_SILENT_STATE_WRITER_DESIGN.md` (silent-eligibility writer, flipped ON), `AUTO_UPDATE_AND_SIGNING_0_4_0.md` (2026-06-22 research snapshot, superseded), and `research/A6_SILENT_UPDATE_TEST_PLAN.md` (pre-impl test plan, WinSparkle path dropped). Living behavior for all of these now lives in `AUTO_UPDATE.md` + `WINDOWS_AUTOUPDATE_PLAN.md`.

---

## Branch & Remote Workflow

- **`origin` = development** (BSVArchie fork). **ALL code changes land here first.** Flow: feature branch → `origin/staging` → `origin/main`. `staging` = integration + where internal test builds are fetched from; `main` = blessed release-candidate.
- **`release` = the signed-build remote** (Hodos-Browser org; holds the GitHub signing keys). For a **public** build, push `main` → `release` and run the release workflow there. `release` may be **ahead of** `origin` (release-specific auto-update commits) — tolerated, but **code originates in `origin` first**; `release` only consumes + adds release-specific bits.
- **Rule:** never author feature code directly on `release`. Internal/beta builds are `0.3.x-beta` and stay private; only the deliberate public release is tagged `0.4.0` and pushed to `release`.
- *Open question:* whether `staging` stays a separate branch once `main` has CI-gated PRs — for now KEEP it as the integration / internal-beta branch.

## Two-tier release model

1. **Draft build** — a `v*` tag push runs the release workflow and produces a **draft** GitHub release (verified: appcast + BOTH download redirects in a retry loop).
2. **Manual promote gate** — a deliberate promote flips the draft to **Latest**, publishes the website (appcast / `_redirects` / `index.astro` to hodosbrowser.com via the Cloudflare deploy token), and re-verifies live. Version is **tag-derived** — no hand-edited version strings.

> There is also the deeper **Tier-1 / Tier-2** split: Tier-1 = the (infrequent, hours-long) custom Chromium/CEF **binary** build (`CEF_BUILD_RUNBOOK.md`); Tier-2 = the fast app build that consumes those binaries and produces signed installers (the ~35-min tag-triggered CI). The next sprint exercises Tier-1.

---

## Open process questions

| Question | Status |
|----------|--------|
| **A6 — Silent auto-update** | **SOLVED.** Silent-apply-on-quit shipped + PROVEN LIVE on Windows (through the two-process picker) and macOS. Windows uses a hybrid custom updater (rollback supervisor + CN signer-continuity gate + picker-exit-wait); macOS uses Sparkle. |
| **Signing-identity migration** | macOS individual → org conversion still pending; a mid-stream identity change forces reinstall, so complete it **BEFORE** the first public signed 0.4.0. Windows already CN=Marston Enterprises. See `ORG_IDENTITY_SIGNING_MIGRATION.md`. |
| **A1/A2 — CEF self-build** | The 2-week-build pain + latest-stable sourcing + drift detection is the core of the queued Chromium/CEF rebuild sprint. See `../0.4.0/CHROMIUM_CEF_SPRINT_KICKOFF.md`. |
| `staging` branch longevity | Keep as integration / internal-beta branch for now (revisit once `main` has CI-gated PRs). |
| Binary-delta updates | Deferred — full-package auto-update is proven live; deltas are a later optimization. |

---

## Guiding principle (never regress)

**Auto-update must NEVER force a reinstall.** Verify the REAL N-1 → N update + relaunch before every promote (not proxies). Kill silent drift: pin CI runners/SDKs, guard deploy-target + minos. Known reinstall-forcers = signing-identity migration (hence the gate above). We have restated this repeatedly and regressed it; the silent-update test rigs in `SILENT_UPDATE_TEST_PLAN.md` are the system that makes it repeatable.

## Linux

Linux is **not a current target**. Placeholders only — keep a Linux column in process tables so the eventual port has a home, but no Linux work is scheduled.
