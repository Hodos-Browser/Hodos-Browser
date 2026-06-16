# DevOps / CI-CD — Process Home

**Created:** 2026-06-01
**Status:** 🚧 Phase 0 — being stood up (docs + research only, no source code)
**Charter:** the planning session that created this folder is captured in
`C:\Users\archb\.claude\plans\we-are-working-on-cuddly-pillow.md`

This is the **permanent home** for how Hodos is built, tested, versioned, and shipped — the
full dev lifecycle: branching/coordination, bug triage, feature intake, refactor planning,
version bumping, CI gates, release, and post-release. Layer-specific details stay in each layer's
`CLAUDE.md`; **cross-layer build/release/process concerns live here.**

> This folder is being assembled from machinery that already exists but was scattered at the top
> level of `development-docs/`. Existing docs are **linked, not yet moved** — physical
> consolidation is a later dedicated step so no references break.

---

## Build / release / update docs (now consolidated in THIS folder)

> Moved here 2026-06-16 (docs re-org). This folder is the **canonical, permanent home** for build/release/update Process & Procedures (per root CLAUDE.md Invariant #12).

| Topic | Doc | State |
|-------|-----|-------|
| Tier-2 build & release (signing, AV) | `BUILD_AND_RELEASE.md` | WORKING (has stale claims — reconcile on next touch) |
| Auto-update design (WinSparkle / Sparkle 2) | `AUTO_UPDATE_IMPLEMENTATION_PLAN.md` → consolidating into `AUTO_UPDATE.md` | built; notify-only (silent pending) |
| CEF version cadence / bump log | `CEF_VERSION_UPDATE_TRACKER.md` | reference (living log) |
| Tier-1 CEF build guide | `CEF_BUILD_FROM_SOURCE_GUIDE.md` (to be merged into `CEF_BUILD_RUNBOOK.md`) | reference |
| Build scripts | `scripts/build_hodos_cef.bat`, `scripts/build_hodos_cef_mac.sh` | reference |
| Dependency verification (every CEF bump) | `DEPENDENCY_VERIFICATION.md` | 🆕 procedure |
| Release CI | `../../.github/workflows/release.yml` | WORKING (tag-triggered) |
| Release orchestrator | `../../scripts/build-release.ps1`, `../../scripts/generate-appcast.py` | WORKING |
| Build instructions | `../../build-instructions/` | reference |

## Source-of-truth code pointers

- Version sources (5, manual): `frontend/src/components/settings/AboutSettings.tsx` (`APP_VERSION`),
  `rust-wallet/Cargo.toml`, `cef-native/CMakeLists.txt` (`-DAPP_VERSION`),
  `installer/hodos-browser.iss`, git tag.
- Tests: `rust-wallet/tests/` (cargo, ~55), `cef-native/tests/` (GoogleTest, ~46, opt-in
  `-DHODOS_BUILD_TESTS=ON`), `frontend/e2e/` (Playwright, ~6). `adblock-engine/` = none.
- Dev launchers: root `dev-wallet.{ps1,sh}`, `dev-adblock.{ps1,sh}`, `cef-native/win_build_run.ps1`,
  `cef-native/mac_build_run.sh` (all gate on `HODOS_DEV=1`).

---

## Initiative documents (this folder)

| Doc | Purpose | Status |
|-----|---------|--------|
| `TRIAGE.md` | Master backlog: every item → category/size/0.4.0/deps/research-depth | 🚧 live |
| `CEF_BUILD_RUNBOOK.md` | Tier-1 full-build checklist (A1/A2/A3/A5): latest Chromium → codec flags → farbling patches → deps → verify | 🚧 draft skeleton |
| `research/BRAVE_FORK_FEASIBILITY.md` | Keystone spike — build-from-Brave vs upstream CEF; Widevine path | ✅ done (2026-06-01) — verdict: STAY ON CEF |

## Terminology (settle this once)

We are a **CEF-based browser that does custom Chromium builds** — not "CEF vs our own Chromium."
CEF's `automate-git.py` pulls the full Chromium source, applies the CEF embedding layer, and compiles
`libcef`, which our `cef-native/` shell drives. We self-build for **proprietary codecs**
(`proprietary_codecs=true ffmpeg_branding=Chrome`) — confirmed in `scripts/build_hodos_cef*.{bat,sh}`.
Farbling (B1) is a renderer-layer patch that rides on this same build. Extensions (B4) are
chrome-layer and are NOT unlocked by self-building. See `research/BRAVE_FORK_FEASIBILITY.md`.

## The two-tier model (A5 — to be fully documented)

- **Tier 1 — binary build:** build CEF/Brave Chromium **binaries** (infrequent, expensive, hours).
  Publish to the `cef-binaries` GitHub release.
- **Tier 2 — app release:** consume prebuilt binaries; build shell + Rust + frontend; sign; ship.
  This is what today's ~35 min tag-triggered CI already does — it's the **fast bug-fix path**.

## Open process questions (resolved as research lands)

- A1: self-build is mandatory (codecs — settled). Real question: how to make it not take ~2 weeks —
  caching (sccache), remote/cloud build execution (GitHub-hosted runners can't), reproducible runbook.
- A2: how to source latest-stable Chromium/CEF + detect what a new version breaks (we are ~6 mo behind)?
- A3: post-CEF dependency-bump process (deps are pinned to current CEF).
- A6: Omaha 4 (silent background updates) vs Sparkle (notify-only) — feasible for a small team?
- A7: where tests run (GitHub pre-build gate / cloud / local), on which platforms, naming conventions.

## Maintenance policy

When a change alters the build, test, version, or release process, update the relevant doc here in
the **same commit**. Don't let this folder drift from `BUILD_AND_RELEASE.md` or the CI workflow.

## Linux

Linux is **not a current target**. Placeholders only — keep a Linux column in process tables so the
eventual port has a home, but no Linux work is scheduled.
