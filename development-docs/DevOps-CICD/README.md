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

## Document index (everything in this folder)

> This folder is the **canonical, permanent home** for build/release/update/process docs (per root
> CLAUDE.md Invariant #12). Layer-specific details stay in each layer's `CLAUDE.md`; cross-layer
> build/release/process concerns live here. Re-org consolidated here 2026-06-16.

| Doc | Purpose | State |
|-----|---------|-------|
| `README.md` (this file) | Process home + index + terminology + two-tier model + open questions | living |
| `TRIAGE.md` | Master backlog: every item → category/size/0.4.0/deps/research-depth | 🚧 live |
| `CEF_BUILD_RUNBOOK.md` | **Tier-1** full CEF/Chromium build (A1/A2/A3/A5): env → depot_tools → automate-git → GN flags → deps → verify. Now includes the merged-in from-source steps + LTS cadence | ✅ WORKING |
| `CEF_VERSION_UPDATE_TRACKER.md` | CEF version cadence / per-bump changelog | reference (living log) |
| `BUILD_AND_RELEASE.md` | **Tier-2** app build & release: installers, signing, AV/SmartScreen, release checklist | WORKING (some PLANNED/stale claims now flagged inline — reconciled 2026-06-16) |
| `AUTO_UPDATE.md` | Auto-update CANONICAL (consolidates old impl-plan + `research/A6_*`) | built; notify-only — silent + Windows EdDSA pending |
| `DEPENDENCY_VERIFICATION.md` | Procedure: verify Hodos-owned deps on **every CEF bump** | 🆕 procedure |
| `TESTING.md` | CANONICAL cross-stack testing strategy (the audit↔CI overlap, done once): census, pyramid, CI gating, coverage, anti-gaming, secret-log gate, capped live-e2e harness | 🆕 strategy |
| `TEST_PLAN.md` | Detailed test plan/catalog + manual QA checklists: ts-sdk vectors to port, Vitest blueprint, e2e/adblock/C++ test inventory, reconciled census. The PLAN that TESTING.md (strategy) points to | ⚠️ inherited (was `UNIT_TESTING.md`), reconciled 2026-06-16, mostly unverified/proposed |
| `WSL_HYBRID_WORKSPACE.md` | Dev-environment strategy: repo location + WSL/Windows split + GitHub-mediated sync | 📋 planned |
| `scripts/` | `build_hodos_cef.bat`, `build_hodos_cef_mac.sh` (Tier-1 build scripts) | reference |
| `research/BRAVE_FORK_FEASIBILITY.md` | Keystone spike — build-from-Brave vs upstream CEF; Widevine path | ✅ done (2026-06-01) — verdict: STAY ON CEF |
| `research/A1_BUILD_STRATEGY.md` | A1 self-build pain-reduction research (caching / remote build) | research |
| `research/A6_AUTO_UPDATE.md` | Auto-update research input (folded into `AUTO_UPDATE.md`) | research (retained) |
| `research/A6_SILENT_UPDATE_TEST_PLAN.md` | Silent-update test plan research input (folded into `AUTO_UPDATE.md`) | research (retained) |

**Related, out of this folder (cross-referenced):**

| Doc | Purpose |
|-----|---------|
| `../../.github/workflows/release.yml` | Tag-triggered release CI (Windows + macOS build/sign/notarize/publish) |
| `../../scripts/build-release.ps1`, `../../scripts/generate-appcast.py` | Release orchestrator + appcast generator |
| `../../build-instructions/` | Platform-specific first-time build setup |
| `../0.4.0/SPRINT_0_4_0_MASTER_PLAN.md` | 0.4.0 master plan (PIPE-* pipeline items, §3 / §7.3 build research) |

## Source-of-truth code pointers

- Version sources (5, manual): `frontend/src/components/settings/AboutSettings.tsx` (`APP_VERSION`),
  `rust-wallet/Cargo.toml`, `cef-native/CMakeLists.txt` (`-DAPP_VERSION`),
  `installer/hodos-browser.iss`, git tag.
- Tests (verified 2026-06, see `TESTING.md` §2 / `TEST_PLAN.md`): `rust-wallet/` ~491 (inline + `tests/`),
  `adblock-engine/` 23, `cef-native/tests/` 39 (GoogleTest, opt-in `-DHODOS_BUILD_TESTS=ON`),
  `frontend/e2e/` 54 (Playwright), Vitest **0**. ⚠️ Pass-status NOT verified (no recent run on record).
- Dev launchers: root `dev-wallet.{ps1,sh}`, `dev-adblock.{ps1,sh}`, `cef-native/win_build_run.ps1`,
  `cef-native/mac_build_run.sh` (all gate on `HODOS_DEV=1`).

---

## Terminology (settle this once)

We are a **CEF-based browser that does custom Chromium builds** — not "CEF vs our own Chromium."
CEF's `automate-git.py` pulls the full Chromium source, applies the CEF embedding layer, and compiles
`libcef`, which our `cef-native/` shell drives. We self-build for **proprietary codecs**
(`proprietary_codecs=true ffmpeg_branding=Chrome`) — confirmed in `scripts/build_hodos_cef*.{bat,sh}`.
Farbling (B1) is a renderer-layer patch that rides on this same build. Extensions (B4) are
chrome-layer and are NOT unlocked by self-building. See `research/BRAVE_FORK_FEASIBILITY.md`.

## Branch & Remote Workflow (canonical)

> Mirrored concisely in root `CLAUDE.md → "Branch & Remote Workflow"`.

**Remotes:**
- **`origin`** (`github.com/BSVArchie/Hodos-Browser`, = `personal`) — **development.** ALL code changes land here first.
- **`release`** (`github.com/Hodos-Browser/Hodos-Browser`) — **the signed-build repo**; GitHub holds the code-signing keys here. The actual public `BUILD_AND_RELEASE` runs here.

**Flow:**
1. Author on a **feature branch** in `origin` → merge to **`origin/staging`** (integration; internal test builds are fetched from here) → merge to **`origin/main`** (blessed release-candidate; what `release` pulls from).
2. **Internal / beta test builds** run the full (eventually new) pipeline but are versioned **`0.3.x-beta`** and stay **private** (fetched locally for testing; NOT the newest GitHub release).
3. When ready for a **public** release: push `main` → **`release`**, tag it the real version (e.g. `0.4.0`), and run the signed `BUILD_AND_RELEASE` there.

**Rules / notes:**
- **Code originates in `origin` first**, always. Never author feature code directly on `release`.
- **`release` may be AHEAD of `origin`** — currently mostly release-specific auto-update commits. That's tolerated. ⚠️ **TODO before executing the new `AUTO_UPDATE.md` plan:** diff `release` vs `origin` for the auto-update code, learn what differs, then re-implement in `origin` (don't fork logic onto `release`).
- *Open question:* keep `staging` as a separate branch once `main` has CI-gated PRs? For now **keep it** as the integration / internal-beta branch.

## The two-tier model (A5 — to be fully documented)

- **Tier 1 — binary build:** build CEF/Brave Chromium **binaries** (infrequent, expensive, hours).
  Publish to the `cef-binaries` GitHub release.
- **Tier 2 — app release:** consume prebuilt binaries; build shell + Rust + frontend; sign; ship.
  This is what today's ~35 min tag-triggered CI already does — it's the **fast bug-fix path**.

## Open process questions (resolved as research lands)

- A1: self-build is mandatory (codecs — settled). Real question: how to make it not take ~2 weeks —
  caching (sccache), remote/cloud build execution (GitHub-hosted runners can't), reproducible runbook.
- A2: how to source latest-stable Chromium/CEF + detect what a new version breaks (we are **~12 mo behind**; M136 predates the M138 LTS program — see `CEF_BUILD_RUNBOOK.md` §1)?
- A3: post-CEF dependency-bump process (deps are pinned to current CEF).
- A6: Omaha 4 (silent background updates) vs Sparkle (notify-only) — feasible for a small team?
- A7: where tests run (GitHub pre-build gate / cloud / local), on which platforms, naming conventions.

## Maintenance policy

When a change alters the build, test, version, or release process, update the relevant doc here in
the **same commit**. Don't let this folder drift from `BUILD_AND_RELEASE.md` or the CI workflow.

## Linux

Linux is **not a current target**. Placeholders only — keep a Linux column in process tables so the
eventual port has a home, but no Linux work is scheduled.
