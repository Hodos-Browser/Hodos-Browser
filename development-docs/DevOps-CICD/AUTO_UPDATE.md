# Auto-Update — Canonical Process & Procedures

**Consolidated:** 2026-06-16 (merges the old `AUTO_UPDATE_IMPLEMENTATION_PLAN.md` + `research/A6_AUTO_UPDATE.md` + `research/A6_SILENT_UPDATE_TEST_PLAN.md`)
**Owner:** DevOps/CI-CD · **Canonical home:** `development-docs/DevOps-CICD/`
**Per root CLAUDE.md Invariant #12** — keep current; append to §8 Lessons Learned each time.
**Research inputs (reference, retained):** `research/A6_AUTO_UPDATE.md`, `research/A6_SILENT_UPDATE_TEST_PLAN.md`.

---

## 0. What shipped (2026-07-09) — silent auto-update is DONE + PROVEN LIVE

> **This supersedes the "notify-first / not implemented / WinSparkle-DSA" framing in the sections below.** Silent install-on-quit with automatic relaunch is **built, shipped, and proven live on real hardware** on BOTH platforms:
> - **Windows:** silent update **beta.25 → beta.26** applied through the two-process profile picker on real hardware (beta.26 = LATEST/live). Windows uses a **hybrid custom updater** (WinSparkle for detection only → own the download/stage → apply on next launch), NOT WinSparkle's own install UI.
> - **macOS:** silent update **beta.21 → beta.22** proven live (Sparkle config-only path).
>
> Key pieces that landed across the beta.19→beta.26 saga:
> - **Signer-continuity CN gate (beta.23)** — the apply-phase verifies the staged build's Authenticode signer **Subject CN** matches (not the Azure Trusted Signing *leaf thumbprint*, which rotates ~every 3 days and was false-rejecting good updates).
> - **External rollback supervisor** — a separate supervisor process guards the apply/relaunch so a failed swap rolls back instead of bricking the install.
> - **Picker-gate fix (beta.26, commit `ae5beb6`)** — the picker-spawned `--profile` child now **waits for the picker process to exit** before the sole-instance check, so the multi-process CEF picker tree no longer blocks the silent apply on multi-profile machines.
> - **`promote.yml` redirect-verify retry** — the promote gate wraps the appcast + BOTH download-redirect checks in a retry loop (killed a false-red where the redirect lagged the appcast).
> - **`BUILD_AND_RELEASE` tag-derived version + draft→manual-promote gate** — releases build as drafts, are gate-verified, then manually promoted to Latest.
>
> **Default is now SILENT** (not notify). Profile picker + per-profile-wallet architecture is **SHELVED** (wallet stays SHARED); the same-process picker refactor that would make the picker-gate permanent is **deferred**.
>
> The sections below (§2–§7) are retained as the **design rationale and P&P** that produced this outcome; where they say "pending / target / not implemented," read §0 for the shipped reality. File:line anchors below are **unverified this pass** (drift-risk).

---

## 1. Decision (settled)
**Keep Sparkle (macOS) + WinSparkle (Windows). Do NOT unify on Velopack.**
- A single proven cross-platform C++ *silent* updater does not exist (2026). Velopack is closest but **its update feed is not cryptographically signed** (only per-package SHA) → a compromised CDN/MITM can serve a malicious `releases.json`. **Disqualifying for a money-handling browser.** Sparkle/WinSparkle sign the appcast with **EdDSA** — a full server compromise still can't forge an update without the offline private key.
- Archetype precedent: **OBS Studio** (C++, CEF-using, 60M+ installs) uses exactly Sparkle + WinSparkle.
- We already ship both → this is mostly **config + a Windows EdDSA bump, not a replacement.**
- *Revisit Velopack only if* it ships cryptographic feed signing + earns a named C++/macOS production record + 12 months v1.x field time.

## 2. Current state (shipped 2026-07-09 — see §0)
> ⚠️ Reconciliation: earlier revisions of this table said the silent path and Windows apply were "pending." **That is now stale — silent update is shipped + proven live on both platforms (§0).** Table updated to shipped reality.

| Piece | State |
|-------|-------|
| `AutoUpdater` singleton (`cef-native/.../AutoUpdater.cpp` + `_mac.mm`) | ✅ built, wired |
| Settings UI (check-for-updates, on/off toggle) | ✅ built |
| Per-user install dir (`%LOCALAPPDATA%`, no UAC) | ✅ done (`.iss`) — prerequisite for unattended update |
| CI signing (Azure Trusted Signing Win; Apple notarytool Mac) | ✅ working in `release.yml` |
| macOS Sparkle version | ✅ **2.9.0** (safe vs the 2025 XPC CVEs; ≥2.7.2 bar met) |
| macOS appcast signing | ✅ **EdDSA** |
| **macOS silent install-on-quit** | ✅ **shipped + proven live** (beta.21→22), Sparkle config-only path |
| **Windows silent update** | ✅ **shipped + proven live** (beta.25→26 through the two-process picker). **Hybrid custom updater** (WinSparkle detection only → own download/stage → apply on next launch + external rollback supervisor + CN signer-continuity gate + picker-exit wait). |
| **Windows appcast / signing** | ✅ Azure Trusted Signing; apply-phase verifies signer **Subject CN** (leaf thumbprint rotates ~3d → not used for continuity) |
| **Appcast publish safety** | ✅ `promote.yml` draft→manual-promote gate verifies appcast + BOTH download redirects in a retry loop before promotion to Latest |
| Signing identity | 🟠 macOS individual→org signing migration is a separate GATE (`ORG_IDENTITY_SIGNING_MIGRATION.md`) — do before first *public* signed 0.4.0; Windows already CN=Marston Enterprises |

## 3. How silent update works ("Chrome model") — SHIPPED

> ✅ **This is now the shipped behavior on both platforms (§0).** The Windows design followed `AUTO_UPDATE_AND_SIGNING_0_4_0.md` (2026-06-22): because **WinSparkle has NO silent install-on-quit / apply-on-next-launch API** (`win_sparkle_check_update_without_ui()` still shows a dialog; `installerArguments` only silences the installer, not WinSparkle's own window), Windows uses a **hybrid custom updater** — WinSparkle for detection only → own the download → apply staged Inno `/VERYSILENT` on next launch, with child-process shutdown + ProfileLock/picker-exit sequencing + external rollback supervisor. macOS is the **config-only** Sparkle path. The flow below is what runs in production:
```
Browser running → every 24h, fetch appcast.xml (HTTPS) → compare version
  → if newer: verify EdDSA signature → download installer silently to staging
  → user quits browser naturally → on shutdown, apply staged update silently → relaunch new version
```
Honest limit: a browser **cannot hot-swap its own running binary**, so *some* relaunch is unavoidable; the goal is to make it **unnoticeable** (apply-on-quit, no "restart now" nag). "Update while the app is closed for weeks" needs a scheduled task / LaunchAgent — a **later** add (~1 wk), not 0.4.0.

## 4. Security requirements (non-negotiable — bake in)
1. **EdDSA appcast signing on BOTH platforms.** macOS ✅; **Windows must migrate DSA→EdDSA** (needs WinSparkle ≥0.9.x; latest **0.9.3**). Private keys stay **offline**, never unencrypted on the build server.
2. **HTTPS appcast only** (plain HTTP was the 2016 MITM bug; Sparkle enforces ATS).
3. **Monotonic version** → Sparkle/WinSparkle silently reject downgrades (anti-rollback).
4. **Version pins:** Sparkle **≥2.7.2** (fixes 2025 CVE-2025-0509 / -10015 / -10016) — we're on 2.9.0 ✅. WinSparkle **≥0.9.3**.
5. **Two 2026 Sparkle CVEs are UNPATCHED through 2.9.1** — CVE-2026-47122 (appcast-item injection) + CVE-2026-47121 (binary-delta symlink traversal). **Both are LOCAL-only** (need existing code execution; 47121 needs a malicious delta, which EdDSA+HTTPS block remotely). **→ Not a hard blocker for silent.** Mitigate: **ship full updates (binary-deltas OFF)** to dodge 47121, and **monitor Sparkle releases** to patch the moment a fix ships. *(This reconciles SPRINT_0_4_0_MASTER_PLAN open-question #14: mitigate, don't block.)*
6. **Fail-closed feed publish** — an unsigned / failed-signature build must NOT be promotable to the live appcast (fix the `|| true` push).

## 5. Implementation (the work — SHIPPED)
> ✅ The Windows plan below evolved: rather than reaching zero-UI *through* WinSparkle, we built the **hybrid custom updater** (WinSparkle detection only → own download/stage → apply on next launch). That path is what shipped and is proven live (§0). Retained here as design rationale.

### 5.1 Windows pass
- WinSparkle is used for **detection only** in the shipped hybrid design; the apply is our own staged Inno `/VERYSILENT` path on next launch (so the WinSparkle EdDSA-version blocker was routed around, not on the critical apply path).
- **Silent path — Hodos uses Inno Setup, NOT NSIS.** ⚠️ (The research docs say NSIS `/S`; that was an assumption — our installer is `installer/hodos-browser.iss`.) The staged installer runs `/VERYSILENT /SP- /SUPPRESSMSGBOXES`, with Inno `AppMutex` + `CloseApplications=yes` so the running CEF browser and all child subprocesses close (Windows Restart Manager) before file replacement. Per-user install already avoids UAC ✅.
- **Signer-continuity = Subject CN** (not Azure Trusted Signing leaf thumbprint, which rotates ~every 3 days and false-rejected good updates — beta.23 fix).
- **Two-process picker sequencing** — the picker-spawned `--profile` child waits for the picker process to exit before the sole-instance check, so the multi-process CEF picker tree doesn't block the silent apply (beta.26 fix, commit `ae5beb6`).
- **External rollback supervisor** guards the apply/relaunch so a failed swap rolls back rather than bricking the install.

### 5.2 macOS pass
- `Info.plist`: `SUEnableAutomaticChecks=YES` (suppress 2nd-launch prompt) + `SUAutomaticallyUpdate=YES` (install on quit) + `SUAllowsAutomaticUpdates=YES`; keep `SUVerifyUpdateBeforeExtraction` on (needs EdDSA).
- Ensure the app bundle is **user-owned** (install to `~/Applications`, not root `/Applications`) so silent install needs no auth.
- **Binary-deltas OFF** (CVE-2026-47121 mitigation) — ship full packages.

### 5.3 Signing / identity / feed pass
- Keep **Azure Trusted Signing** (Windows) — instant SmartScreen reputation; EV no longer needed. ✅
- ✅ **Appcast promote is fail-closed** — `promote.yml` builds releases as drafts, then a manual-promote gate verifies the appcast + BOTH download redirects in a retry loop before flipping to Latest. `BUILD_AND_RELEASE` version is tag-derived.
- 🟠 **macOS signing identity → org (Marston Enterprises)** still a pending GATE before the first *public* signed 0.4.0 (`ORG_IDENTITY_SIGNING_MIGRATION.md`); Windows is already CN=Marston Enterprises. A mid-stream signing-identity change **resets** accrued reputation and forces reinstall — do it before the first public build.
- **`sparkle:channel` discipline** — tag beta items with a channel; an **unchannelled beta auto-ships to ALL stable users** (this is the `generate-appcast.py` regression 24b2522→2eda476).

## 6. Test plan (retained as the release gate — passed live for beta.22 / beta.26)
> ✅ The macOS (beta.21→22) and Windows (beta.25→26) live-update proofs satisfied this acceptance set. Keep running it as the standing gate before every promote (per the update-stability principle — verify the REAL N−1→N update, not proxies). Full step-by-step in `research/A6_SILENT_UPDATE_TEST_PLAN.md`. Run macOS on the M1, Windows on the 32 GB box. Acceptance:
- [ ] **macOS:** check → download → install-on-quit → relaunch to new version, **zero prompts**.
- [ ] **Windows:** equivalent **zero-UI** path (or a documented decision to fall back).
- [ ] **Tampered/unsigned update REJECTED** on both (flip a byte / random signature → no install, no crash).
- [ ] **Downgrade REJECTED** on both (publish a lower signed version → skipped).
- [ ] HTTPS appcast, EdDSA verified, **private keys offline**, Sparkle ≥2.7.2, **binary-deltas decision recorded**.
- [ ] Wallet data, bookmarks, history preserved across update.
- [ ] No-network = graceful failure; tested Win 10 + Win 11.

## 7. Decisions (resolved)
- ✅ **Silent is the default** on both platforms — proven live (§0). Windows zero-UI reached via the hybrid custom updater, so the notify fallback and the **Velopack-on-Windows fallback** are both moot.
- ✅ **Binary-deltas OFF** — ship full packages (CVE-2026-47121 mitigation) until a patched Sparkle ships.
- ⏳ **"Update while app closed"** scheduled task / LaunchAgent — still deferred past 0.4.0 (nice-to-have, not on the critical path).
- ⏳ **Same-process picker refactor** — would make the picker-exit-wait fix (beta.26) permanent/structural instead of a sequencing workaround; deferred (profile-picker architecture is shelved pending market feedback).

## 8. Lessons Learned (append per Invariant #12)
- *(2026-06-16, consolidation)* Research docs assumed an **NSIS** silent installer; reality is **Inno Setup** — silent flag is `/VERYSILENT`, not NSIS `/S`. Don't propagate the NSIS assumption.
- *(2026-06-16)* The two 2026 Sparkle CVEs are **local-only** → they don't block silent; mitigate with deltas-off + monitoring. (Corrects an earlier "hard blocker" framing.)
- *(2026-07-09)* **WinSparkle can't do silent apply — build a hybrid updater.** WinSparkle detection only → own the download/stage → apply staged Inno `/VERYSILENT` on next launch. This is what shipped and is proven live (beta.25→26).
- *(2026-07-09)* **Signer-continuity must compare Subject CN, not the leaf thumbprint.** Azure Trusted Signing rotates the signing **leaf certificate ~every 3 days**, so a thumbprint-equality gate false-rejected every good update ("signer changed"). Compare the Subject CN instead (beta.23 fix).
- *(2026-07-09)* **The two-process profile picker blocks the silent apply.** The picker's ~8-process CEF tree was still tearing down when the picker-spawned `--profile` child ran the apply → sole-instance count > 1 → defer → never applies. Fix: the child **waits for the picker process to exit** before the sole-instance check (beta.26, commit `ae5beb6`). A same-process picker would fix this structurally (deferred).
- *(2026-07-09)* **Promote gates need retry loops.** The redirect-verify in `promote.yml` false-reds when the download redirect lags the appcast; wrap appcast + BOTH redirect checks in one retry loop.

## 9. Code map (files this touches)
> Anchors below are **unverified this pass** (drift-risk). Roles updated to shipped reality.

| File | Role |
|------|------|
| `cef-native/include/core/AutoUpdater.h` / `src/core/AutoUpdater.cpp` / `AutoUpdater_mac.mm` | the singleton (WinSparkle detection + hybrid apply / Sparkle wrappers) |
| Windows hybrid updater (download→stage→apply-on-launch + external rollback supervisor + CN signer gate + picker-exit wait) | the shipped Windows silent path |
| `cef-native/include/core/SettingsManager.h` | `UpdateSettings` (silent default) |
| `cef-native/src/handlers/simple_handler.cpp` | `check_for_updates` / `update_settings_changed` IPC |
| `frontend/src/components/settings/AboutSettings.tsx` | version + check button + update-mode UI |
| `installer/hodos-browser.iss` | per-user dir ✅; `/VERYSILENT` + `AppMutex` + `CloseApplications` ✅ |
| `scripts/build-release.ps1` | stage `WinSparkle.dll` |
| `scripts/generate-appcast.py` | appcast signatures + `sparkle:channel` |
| `.github/workflows/release.yml` + `promote.yml` | sign both platforms; draft→manual-promote gate; verify appcast + BOTH download redirects in a retry loop before Latest |
