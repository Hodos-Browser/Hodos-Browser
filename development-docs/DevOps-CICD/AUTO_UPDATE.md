# Auto-Update — Canonical Process & Procedures

**Consolidated:** 2026-06-16 (merges the old `AUTO_UPDATE_IMPLEMENTATION_PLAN.md` + `research/A6_AUTO_UPDATE.md` + `research/A6_SILENT_UPDATE_TEST_PLAN.md`)
**Owner:** DevOps/CI-CD · **Canonical home:** `development-docs/DevOps-CICD/`
**Per root CLAUDE.md Invariant #12** — keep current; append to §8 Lessons Learned each time.
**Research inputs (reference, retained):** `research/A6_AUTO_UPDATE.md`, `research/A6_SILENT_UPDATE_TEST_PLAN.md`.

---

## 1. Decision (settled)
**Keep Sparkle (macOS) + WinSparkle (Windows). Do NOT unify on Velopack.**
- A single proven cross-platform C++ *silent* updater does not exist (2026). Velopack is closest but **its update feed is not cryptographically signed** (only per-package SHA) → a compromised CDN/MITM can serve a malicious `releases.json`. **Disqualifying for a money-handling browser.** Sparkle/WinSparkle sign the appcast with **EdDSA** — a full server compromise still can't forge an update without the offline private key.
- Archetype precedent: **OBS Studio** (C++, CEF-using, 60M+ installs) uses exactly Sparkle + WinSparkle.
- We already ship both → this is mostly **config + a Windows EdDSA bump, not a replacement.**
- *Revisit Velopack only if* it ships cryptographic feed signing + earns a named C++/macOS production record + 12 months v1.x field time.

## 2. Current state (verified in source, fan-2 / readers 2026-06)
> ⚠️ Reconciliation: the old plan's status said "Planning — implement after beta.1." That is **stale.** The updater is substantially **built**; what's pending is the **silent path** and **Windows EdDSA**.

| Piece | State |
|-------|-------|
| `AutoUpdater` singleton (`cef-native/.../AutoUpdater.cpp` + `_mac.mm`) | ✅ built, wired |
| Settings UI (check-for-updates, on/off toggle) | ✅ built (offers on/off, **not** silent-vs-notify yet) |
| Per-user install dir (`%LOCALAPPDATA%`, no UAC) | ✅ done (`.iss`) — prerequisite for unattended update |
| CI signing (Azure Trusted Signing Win; Apple notarytool Mac) | ✅ working in `release.yml` |
| macOS Sparkle version | ✅ **2.9.0** (safe vs the 2025 XPC CVEs; ≥2.7.2 bar met) |
| macOS appcast signing | ✅ **EdDSA** |
| **Windows WinSparkle version** | ❌ **0.8.1 — DSA only** (no EdDSA support) |
| **Windows appcast signing** | ❌ **DSA** (deprecated) at 3 sites: `AutoUpdater.cpp` (`win_sparkle_set_dsa_pub_pem`), `generate-appcast.py`, `release.yml` |
| **Silent install-on-quit** | ❌ not implemented — today is **notify-first** (shows a dialog) |
| **Appcast publish safety** | ❌ push is non-fatal (`\|\| true`) → a bad/unsigned appcast can auto-deploy |
| Signing identity | 🟠 macOS hardcoded "Developer ID Application: Matthew Archbold" (personal, not org) |

## 3. How silent update works (target — "Chrome model")
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

## 5. Implementation in the 0.4.0 build (the work)
### 5.1 Windows pass
- **Bump WinSparkle 0.8.1 → 0.9.3** (`external/winsparkle/`, `CMakeLists.txt`, `build-release.ps1` DLL copy).
- **Migrate DSA → EdDSA** at the 3 sites: `AutoUpdater.cpp` (`win_sparkle_set_eddsa_public_key`), `generate-appcast.py` (emit `sparkle:edSignature`), `release.yml` (sign with EdDSA). Generate keys with the bundled `winsparkle-tool`; private key offline (GitHub Secret).
- **Silent path — Hodos uses Inno Setup, NOT NSIS.** ⚠️ (The research docs say NSIS `/S`; that was an assumption — our installer is `installer/hodos-browser.iss`.) So: `win_sparkle_check_update_without_ui()` on the background check **+ run the Inno installer with `/VERYSILENT /SP- /SUPPRESSMSGBOXES`** via `sparkle:installerArguments`, **+ configure Inno `AppMutex` + `CloseApplications=yes`** so the running CEF browser and all child subprocesses close (Windows Restart Manager) before file replacement — otherwise a locked EXE forces a reboot. Per-user install already avoids UAC ✅.
- ⚠️ `win_sparkle_check_update_without_ui()` is **not** fully UI-less by itself (still shows the "update available" window); zero-UI requires the silent-installer wiring above. **Verify hands-on** (§6) — this is the weaker half; if it can't reach zero-UI, the documented fallback is Velopack-on-Windows (accepting its feed-signing gap) or a custom updater.

### 5.2 macOS pass
- `Info.plist`: `SUEnableAutomaticChecks=YES` (suppress 2nd-launch prompt) + `SUAutomaticallyUpdate=YES` (install on quit) + `SUAllowsAutomaticUpdates=YES`; keep `SUVerifyUpdateBeforeExtraction` on (needs EdDSA).
- Ensure the app bundle is **user-owned** (install to `~/Applications`, not root `/Applications`) so silent install needs no auth.
- **Binary-deltas OFF** (CVE-2026-47121 mitigation) — ship full packages.

### 5.3 Signing / identity / feed pass
- Keep **Azure Trusted Signing** (Windows) — instant SmartScreen reputation; EV no longer needed.
- **Align signing identity to the organization (Marston Enterprises) before wide distribution** — the signer's name is shown to users, and changing identity later **resets** accrued SmartScreen/Gatekeeper reputation.
- **Harden secret-missing to FAIL** (today it warns-and-skips → could ship unsigned).
- **Make appcast generate/push fail-closed** and **decouple** it from the build so a bad appcast can't auto-deploy.
- **`sparkle:channel` discipline** — tag beta items with a channel; an **unchannelled beta auto-ships to ALL stable users** (this is the `generate-appcast.py` regression 24b2522→2eda476).
- Add a **silent-vs-notify** choice to the Settings UI (currently only on/off).

## 6. Test plan (run before enabling silent in production)
> Full step-by-step in `research/A6_SILENT_UPDATE_TEST_PLAN.md`. Run macOS on the M1, Windows on the 32 GB box. Acceptance:
- [ ] **macOS:** check → download → install-on-quit → relaunch to new version, **zero prompts**.
- [ ] **Windows:** equivalent **zero-UI** path (or a documented decision to fall back).
- [ ] **Tampered/unsigned update REJECTED** on both (flip a byte / random signature → no install, no crash).
- [ ] **Downgrade REJECTED** on both (publish a lower signed version → skipped).
- [ ] HTTPS appcast, EdDSA verified, **private keys offline**, Sparkle ≥2.7.2, **binary-deltas decision recorded**.
- [ ] Wallet data, bookmarks, history preserved across update.
- [ ] No-network = graceful failure; tested Win 10 + Win 11.

## 7. Open decisions
- **Silent-vs-notify default** for 0.4.0 (silent is the goal; notify is the safe fallback if Windows zero-UI can't be confirmed).
- **Binary-deltas on/off** — recommend OFF (CVE-2026-47121 mitigation) until a patched Sparkle ships.
- **"Update while app closed"** scheduled task / LaunchAgent — defer past 0.4.0.
- **Velopack-on-Windows fallback** — only if WinSparkle truly can't reach zero-UI.

## 8. Lessons Learned (append per Invariant #12)
- *(2026-06-16, consolidation)* Research docs assumed an **NSIS** silent installer; reality is **Inno Setup** — silent flag is `/VERYSILENT`, not NSIS `/S`. Don't propagate the NSIS assumption.
- *(2026-06-16)* The two 2026 Sparkle CVEs are **local-only** → they don't block silent; mitigate with deltas-off + monitoring. (Corrects an earlier "hard blocker" framing.)
- *(add new lessons here as implementation proceeds…)*

## 9. Code map (files this touches)
| File | Role |
|------|------|
| `cef-native/include/core/AutoUpdater.h` / `src/core/AutoUpdater.cpp` / `AutoUpdater_mac.mm` | the singleton (WinSparkle / Sparkle wrappers) |
| `cef-native/include/core/SettingsManager.h` | `UpdateSettings` (add silent-vs-notify) |
| `cef-native/src/handlers/simple_handler.cpp` | `check_for_updates` / `update_settings_changed` IPC |
| `frontend/src/components/settings/AboutSettings.tsx` | version + check button + update-mode UI |
| `installer/hodos-browser.iss` | per-user dir ✅; add `/VERYSILENT` + `AppMutex` + `CloseApplications` |
| `scripts/build-release.ps1` | stage `WinSparkle.dll` (0.9.3) |
| `scripts/generate-appcast.py` | EdDSA signatures + `sparkle:channel`; fail-closed |
| `.github/workflows/release.yml` | EdDSA sign both platforms; org identity; fail on missing secrets; decoupled fail-closed appcast push |
