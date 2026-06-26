# Post-beta.16 Sequenced Plan — 0.4.0 Hardening & Release-Health Phase

**Date:** 2026-06-25 · **Revised:** 2026-06-26 (decisions locked; reorganized around the Update-Stability Program) · **Branch:** `0.4.0` · **Anchor commit:** `2ac4f64`

## Context

beta.16 is **live** (release pipeline fully automated: `v*` tag → build → draft-first verify → promote-latest → website publish). Two problems hurt people *today*: (1) **macOS auto-update is broken** — the beta.16 mac binary is stamped `minos = macOS 26` (Tahoe) and refuses to relaunch on real users' machines; (2) the **installed browser closes when a dev build is launched**. The dev/prod runtime split (ports 31401/31402, `dev.` pipe prefix, `HodosBrowserDev/` data root) is already shipped — neither problem is a port/pipe collision.

This document was produced by a **multi-agent research + adversarial-review workflow**, then walked through decision-by-decision with the owner (2026-06-26). **All 9 open decisions are now resolved** (see table) and the plan is reorganized around a single owner-set priority: **auto-update must never break in a way that forces a reinstall.**

---

## Track 0 — The Update-Stability Program  ⭐ TOP PRIORITY

> **Owner principle (2026-06-26):** *Auto-update must NEVER break such that a user has to uninstall + reinstall.* This ranks **above** shipping speed and feature work. Buggy-but-updatable is fine (we patch); forcing a redownload is not — it destroys credibility and blocks marketing. Worth a large, deliberate time investment. See `feedback_update_stability_principle.md` and `DevOps-CICD/CEF_VERSION_UPDATE_TRACKER.md`.

**Why we keep regressing this (the real root cause).** Auto-update is a chain of ~8 independent preconditions that must *all* hold, and we keep discovering them one at a time *in production* (the beta.9→14 macOS XPC/comparator/CFBundleVersion saga; the build-number-scores-0 bug; `windows-latest`→2025; beta.16's `minos`=26). They share **one root cause**:

1. **We verify *proxies*, not the real update.** CI checks "asset present / signature non-empty / appcast names the version" — all of which pass *while the update is broken*. We have **never installed N−1 and actually updated to N on a real machine before promoting.** A single "actually perform the update + relaunch before promote" gate would have caught **every** historical breakage.
2. **Silent drift** — floating runners/SDKs move preconditions under us with zero code change.

**The two pillars (build these once; never skip them):**

```
 PILLAR A — Verify the REAL update         PILLAR B — Kill silent drift
 ┌───────────────────────────────┐         ┌──────────────────────────────────┐
 │ Before EVERY promote:         │         │ Pin every runner + SDK + dep      │
 │  install N−1 → point at new   │         │ Explicit deployment target +      │
 │  appcast → auto-update to N →  │         │   minos guard (fails closed)      │
 │  confirm relaunch into N,      │         │ No floating macos-latest /        │
 │  BOTH platforms, OS at floor.  │         │   windows-latest, ever            │
 │ Fail-closed. Mac sub-floor =   │         │ Re-validate pins each Chromium    │
 │  mandatory MANUAL step for now.│         │   bump + when GitHub retires image│
 └───────────────────────────────┘         └──────────────────────────────────┘
```

**Known, deliberately-managed reinstall-forcers** (so the principle is honest, not magic): signing-identity/cert changes — the pending personal→Marston Enterprises migration (`ORG_IDENTITY_SIGNING_MIGRATION.md`) is the big one and **must be a planned, tested transition**; and a user's OS being genuinely too old for the new Chromium (their OS aging out — the deployment-target work just makes us *honest* about it). Outside this short list, "users never reinstall" is achievable.

**Everything below nests under Track 0.** T5 (mac min-version) is the **first installment** of Pillar B; the real-update gate is the missing piece of Pillar A and is the single highest-leverage new build we can make.

---

## Decisions locked (2026-06-26)

| # | Decision | Resolution |
|---|----------|-----------|
| 1 | macOS minimum version | **11.0 (Big Sur)**; 10.15 retired (CEF 136 dropped Catalina). Final value = `max(11.0, measured CEF-framework minos)` — confirm via `vtool` on Mac. |
| 2 | macOS runner | **`macos-15`**, pinned (current stable). Never `macos-latest`. Backward-compat comes from the deployment target, not the runner. |
| 3 | Mac user count → T5 urgency | Handful of users, none likely on beta.16 anyway. **T5 stays top** (it's the active break + Track-0 anchor). |
| 4 | Dev/prod crash fix | **Durable rename to `HodosBrowserDev.exe`/`.app` — its own phase** (not the scoped-kill band-aid). Released artifact name stays *exactly* `HodosBrowser`. |
| 5 | PAT → GitHub App token | **Do it, later, low priority** (security hygiene, off the update-stability critical path). |
| 6 | Delta updates | **Defer both platforms.** (See T2 — Windows premise falsified; mac unnecessary once silent background download works.) |
| 7 | Website-clone auth details | **Safe defaults** (public clone + authenticated push; rely on ~1h token auto-expiry; no explicit revoke). Implementation footnote of #5. |
| 8 | `msvc-dev-cmd` Node-20 warning | **Accept the warning** + a **Sept 16 2026 tripwire** to re-check. Don't swap (toolchain-churn risk). |
| 9 | Sparkle CVE gate | **"CVE-2026-47121" is unverified/likely fabricated — do not cite.** Real delta-safety fixes live in Sparkle **2.9.2**; require ≥ 2.9.2 (use 2.9.3) *if* mac delta is ever revisited. |

---

## Thread 1 — Dev + installed crash → **durable rename (own phase)**  *(Windows + mac launchers)*

**Root cause (verified).** Not a C++ crash, not a port/pipe collision. The dev launchers **kill the installed browser by image name** before launching dev: `win_build_run.ps1:27` `Stop-Process -Name "HodosBrowser" -Force`, `win_build_run.sh:27` `taskkill //F //IM HodosBrowser.exe`, `mac_build_run.sh:54` `pkill -9 HodosBrowser`. Both builds emit `HodosBrowser`, so the kill takes down the installed browser's whole process tree (CEF spawns subprocesses from the same exe, `cef_browser_shell.cpp:4015`). It is also a **data-integrity** bug: force-killing the running prod browser can corrupt its history/bookmarks/cookies SQLite. Every runtime resource is verified dev/prod-split (`SingleInstance.cpp:49`, ports, data root, AUMID, DevTools), so the launcher is the sole suspect.

**Decision: durable rename, its own phase** (owner: "real long-term solutions, not shortcuts"). HODOS_DEV-gated CMake `OUTPUT_NAME` → `HodosBrowserDev`. Scope differs by platform:
- **Windows:** small — one gated `OUTPUT_NAME` + update launcher kill/launch targets. Dev safeguard keys on the *path* (`AppPaths.h:20-24`), not the exe name, so it survives.
- **macOS:** the focus work — ripples through the app bundle name, `CFBundleExecutable`, the 5 CEF helper bundles, and ad-hoc signing.
- **Hard guardrail (Track-0):** the *released* artifact must stay **exactly** `HodosBrowser.exe`/`HodosBrowser.app` (auto-update, WinSparkle, Start-menu shortcuts, mac notarization, appcast all key on it). The rename must be strictly dev/build-config-gated, with a **verification step that builds a release artifact and confirms the name is unchanged.**
- **Possible coordination:** the separately-deferred internal CMake *target* rename (`HodosBrowserShell`→`HodosBrowser`, 119 occurrences) could be folded into this phase. Decide at phase kickoff.

**Open gate before the phase closes.** Confirm the installed browser does **not** also close when the dev exe is launched manually (HODOS_DEV=1, no launcher). High-confidence it doesn't (resources are split), but verify; if it does, reopen toward Chromium shared-memory/GPU-cache.

**Interim (optional, owner's call):** a one-line scoped filter in the *local* launchers to avoid corrupting your own prod data before the phase lands — framed as "don't bleed while building the real fix," not a substitute for the rename.

**Effort.** Rename phase: ~1–2 hrs Windows, a focused day on macOS (bundle/helper/signing + the release-name verification). **Confidence:** High.

---

## Thread 5 — macOS minimum-version regression  *(Track-0 Pillar B, first installment; needs Mac to validate)*

**Root cause.** beta.16's mac binary is stamped `LC_BUILD_VERSION minos = macOS 26` because the build floats on `macos-latest` (Tahoe) and the 10.15 intent never applies: `CMakeLists.txt:72` uses `set(... CACHE STRING ...)` **without `FORCE`** and **after `project()`** → silent no-op → clang emits no `-mmacosx-version-min` → `minos` defaults to the **build host's SDK**. The loader then rejects relaunch on every user below that OS. `LSMinimumSystemVersion` (plist) is ignored in favor of the Mach-O `minos`. **Honest floor is 11.0** (CEF 136 dropped Catalina), pending measurement.

**Fix (decisions #1/#2 applied):**
- **(a)** Pin runner `release.yml:313 macos-latest → macos-15`.
- **(b)** Apply the target for real: `-DCMAKE_OSX_DEPLOYMENT_TARGET=<floor>` on the configure command line (`release.yml:441`) **and** job-level `env: MACOSX_DEPLOYMENT_TARGET: <floor>` so the CEF wrapper, cargo, and sub-cmakes inherit one floor; make `CMakeLists.txt:72` self-defending (`FORCE` + non-10.15 default); fix `mac_build_run.sh:19`; update `LSMinimumSystemVersion` in `Info.plist:24` + `helper-Info.plist.in:22`.
- **(c)** **Measure the real floor first:** `vtool -show-build` on the CEF framework; `<floor> = max(11.0, measured)`. Do not hard-code 11.0 sight-unseen (an under-stamped exe passes the loader, then dyld fails to load a higher-minos framework → launch crash).
- **(d) CI guard (Pillar B):** after build, read `minos` of the main exe, all 5 helpers, and both Rust binaries; **FAIL unless each ≥ the CEF framework minos** (inequality, not `== 11.0`).
- **(e) Manual sub-floor gate (Pillar A):** CI runs on macOS-26 and **cannot** reproduce a sub-floor rejection — launch + auto-update + **relaunch** the notarized `.app` on a **real macOS < 26 machine (owner's 15.7.5) BEFORE `promote --latest`.**
- **(f)** Confirm notarytool/stapler still pass on the macos-15 SDK on the first pinned release.
- **(g)** Floating-runner audit — **separate follow-up PR**: pin `test.yml` floating runners (`:46 windows-latest`, `:76`/`:94 ubuntu-latest`).

**Flagged.** "11.0" is **inferred, not measured** (the hard prerequisite in step c). **Effort:** ~1–2 hrs edits (Windows-editable), but guard + sub-floor relaunch **require a Mac + a release round-trip.** **Confidence:** High.

---

## Thread 4 — Deferred hardening: (a) CI signature re-verify (b) GitHub App token  *(CI-only)*

**(a) Crypto-verify is Track-0 Pillar A work** — promote it accordingly. Today the publish job only **greps that signatures are present/non-empty** (`release.yml:1044-1045`); it never verifies them against the client's embedded keys (DSA PEM `AutoUpdater.cpp:95-114`, `SUPublicEDKey` `Info.plist:40`). Add a crypto-verify step between "Verify appcast advertises this version" (`:1046`) and "Promote" (`:1051`).
- **CRITICAL:** do **not** fail-closed on all three signatures — Windows EdDSA is absent by design today (`soft_skip()` `release.yml:244`). Matrix: **MANDATORY** Win-DSA (inverse of `release.yml:215`: `openssl dgst -sha1 -verify dsa_pub.pem ...`) + **MANDATORY** mac-EdDSA; **CONDITIONAL** Win-EdDSA (verify only if present). Read keys **from source at runtime** (no hardcoded second copy). Prefer openssl-3 SPKI Ed25519 over a ship-time `pip install`. Validate once against beta.16 known-good (PASS) + a corrupted sig (FAIL). When commit **3b** flips the client to EdDSA-only, the matrix flips — land coordinated.

**(b) PAT → GitHub App token (decision #5: later, low priority).** Swap `WEBSITE_DEPLOY_TOKEN` (long-lived PAT, `release.yml:1080`) for `actions/create-github-app-token@v2`, minted before "Update website" (`:1078`), App with **Contents:RW installed ONLY on `hodosbrowser.com`**, owner pinned to literal `Hodos-Browser`. Decision #7: keep public clone + authenticated push, rely on ~1h auto-expiry. Sits *after* promote → can't break a release. One-time org-admin task to create + install the App.

**Effort.** (a) ~½ day, **prioritize as Pillar A**. (b) ~½ day + org-admin, **low priority**. Both CI-only, fail-closed. **Confidence:** High.

---

## Thread 3 — CI hygiene: Node-20 + floating-action sweep  *(CI-only)*

Most actions are already Node-24. Risk set: `ilammy/msvc-dev-cmd@v1` (no Node-24 release exists), `softprops/action-gh-release@v2` (v3.0.1 is Node 24), `test.yml` on `checkout@v4`.
- **Wave 1 (ship first):** `test.yml checkout@v4 → @v6`. Pure CI gate, off the artifact path.
- **GUARDRAIL:** do **NOT** touch `release.yml:97 node-version:'20'` — that's the **frontend build toolchain** (Vite), not the Actions runtime. A naive "kill all Node 20" sweep bricks the build. Fence it off.
- **Wave 2 (canary):** `action-gh-release@v2 → @v3.0.1` (`release.yml:997`) — confirm v3 `files:`/`generate_release_notes` semantics first, then canary on `v0.3.0-beta.17` and confirm the verify gates go GREEN.
- **Wave 3 (decision #8): ACCEPT the `msvc-dev-cmd` warning** + Sept 16 2026 tripwire. Don't swap (would risk the VS2022 v143 toolset on the pinned `windows-2022` runner).
- Leave `azure/trusted-signing-action` and the artifact actions untouched.

**Effort.** Windows-doable YAML; Wave 1 ~2 min, Wave 2 ~2 min + canary, Wave 3 zero. Land in `origin` (feature→staging→main) before any `release` canary. **Confidence:** High.

---

## Thread 2 — Delta / differential updates  →  **DEFER BOTH (decision #6)**

Not a bug — a capability gap, and the **wrong lever for the actual goal.** Once silent *background* download works (Track-0 / silent-install track), download *size* is invisible to the user, so deltas optimize a number nobody sees.
- **Windows — REJECTED (not just deferred).** The whole-file-manifest premise is **falsified**: CI re-signs every `*.dll` (incl. ~200 MB `libcef.dll`) with a fresh RFC3161 timestamp every release (`release.yml:155-157`), so the whole-file hash changes even when Chromium doesn't → re-downloads the dominant file anyway. Only true byte-level diffing (`zstd --patch-from`/bsdiff/Courgette) or a PE-aware hash excluding the cert blob would ever pay off. Also carries real ship-broken-update risk (no version guard, in-place overwrite of loaded modules).
- **macOS — DEFER.** Gated on T5 (full path must work first) and on Sparkle ≥ 2.9.2 (the real delta-safety fixes — *not* the fabricated CVE). Revisit only after the silent-update track proves background download already removes the friction.

**Confidence:** High (Windows reversal well-evidenced).

---

## Sequenced execution order

Lead with Track 0. Tags: `[Win]` / `[Mac]` / `[CI-only]`; ⚑ = needs a release to validate.

| # | Step | Track/Thread | Platform | Validate |
|---|------|--------------|----------|----------|
| 1 | **Stop the bleeding (optional interim):** local-launcher scoped filter so dev runs don't corrupt installed prod data before the rename phase. | T1 | `[Win]`+`[Mac]` local | none |
| 2 | **Measure the real mac floor:** `vtool -show-build` on the CEF framework. Set `<floor> = max(11.0, measured)`. | T5 / Pillar B | `[Mac]` | local |
| 3 | **T5 fix (a/b/d):** pin `macos-15`, real `-DCMAKE_OSX_DEPLOYMENT_TARGET` + job-env, FORCE in CMake, fix `mac_build_run.sh`, plists, add the `≥framework` minos guard. | T5 / Pillar B | `[Win]`-edit, `[Mac]`-validate | ⚑ release |
| 4 | **Build the real-update gate (Pillar A) — the highest-leverage new work:** scripted N−1→N update+relaunch check; **mandatory manual sub-floor Mac relaunch (owner's 15.7.5) + notarytool/stapler before `promote --latest`.** | Track 0 | `[Mac]` + `[CI-only]` | ⚑ release |
| 5 | **T4(a) crypto-verify (Pillar A):** mandatory Win-DSA + mac-EdDSA, conditional Win-EdDSA; keys from source; validate vs beta.16 + corrupted sig; per-enclosure grep hardening. | T4 / Track 0 | `[CI-only]` | ⚑ canary |
| 6 | **T3 Wave 1:** `test.yml checkout@v6`; fence OFF `release.yml:97`. | T3 | `[CI-only]` | CI |
| 7 | **Dev-rename phase (T1):** HODOS_DEV-gated `HodosBrowserDev` rename, Win + mac bundle/helper/signing, **release-name-unchanged verification.** Its own focused phase. | T1 | `[Win]`+`[Mac]` | ⚑ release-artifact name check |
| 8 | **T3 Wave 2:** `action-gh-release@v3.0.1` after confirming v3 semantics; canary on beta.17. | T3 | `[CI-only]` | ⚑ beta |
| 9 | **T4(b) GitHub App token** (low priority): create App, swap PAT, owner-pinned, safe-default clone/expiry. | T4 | `[CI-only]` + org-admin | ⚑ release |
| 10 | **T5(g) floating-runner audit** (separate PR): pin `test.yml` runners. | T5 / Pillar B | `[CI-only]` | CI |
| 11 | **T3 Wave 3:** accept `msvc-dev-cmd` warning + Sept 16 2026 tripwire. | T3 | `[Win]` | none |
| — | **T2 delta updates — deferred** (Win rejected; mac after silent-update track + Sparkle ≥ 2.9.2). | T2 | — | — |

**Dependencies.** Steps 2→3→4 are the user-facing mac chain and must run in order. Step 4 (real-update gate) is the keystone of Track 0 — every subsequent ⚑ release should run through it. T4(a)/T2/grep-hardening all touch the same fail-closed appcast verify block — coordinate so `edSignature` becomes per-enclosure once. T4(a)'s matrix is coupled to commit 3b (DSA→EdDSA flip).

**Highest-leverage:** Step 4 (the real-update gate) is the single change that converts "we hope it updates" into "we proved it updates" — it would have caught every historical breakage. Steps 2–3 fix the active mac break. Everything else is fail-closed pipeline hardening that cannot ship a broken update.

---

## Execution prerequisites (not decisions — Mac-bound measurements)

1. **`vtool` the CEF framework minos** (Step 2) — sets the real `<floor>`. The only remaining unknown in the number.
2. **A real sub-26 Mac in the loop** (owner's 15.7.5) for the mandatory relaunch gate until it's automatable.
3. **Org-admin slot** to create + install the GitHub App (Step 9, low priority).

## Verification / test plan (per CLAUDE.md Testing Standards + Track 0)

| Change | Level | Windows | macOS |
|--------|-------|---------|-------|
| Real-update gate (Track 0) | **Release-gating** | scripted N−1→N update+relaunch GREEN | **manual relaunch-after-update on real macOS 15.7.5** before promote |
| T5 min-version | Thorough + release | N/A (config authored on Win) | CI asserts each minos ≥ framework; notarytool/stapler pass |
| Dev-rename phase | Standard | installed survives `win_build_run`; dev launches clean; **release artifact still named `HodosBrowser.exe`** | installed `.app` survives; dev launches clean; release `.app` name unchanged |
| T3/T4 CI changes | Standard | beta-tag canary: full build + signed installer + appcast verify GREEN | mac build + sign + appcast both enclosures |

**Parity gate (the new non-negotiable):** every release must verify the **full update→relaunch path on both Windows and a sub-runner-OS macOS machine before `promote --latest`.** Draft-first/verify/promote only protects users if the real update is part of *verify* — proxies are not enough. None of these threads touch browser-core, the gold-pill payment IPC, wallet signing, or the HODOS_DEV split.
