# A6 — Silent Auto-Update (Research + Verification Pass)

**Created:** 2026-06-01 · **Status:** ✅ VERIFIED (3-agent verification pass done 2026-06-01)
**Method:** primary-sourced (Sparkle/WinSparkle/Velopack repos+docs+issues, NVD/CERT CVEs, Chromium
Updater docs, OBS Studio release notes). Verification reversed the earlier "Velopack on Windows" lean.

> Owner constraints: prefer ONE unified updater if a proven one exists; security non-negotiable;
> silent UX is the goal. Prior history: a Rust cross-platform updater (Velopack) was evaluated and
> passed on for immaturity — the verification CONFIRMS that instinct was right.

---

## TL;DR (verified) — keep the Sparkle + WinSparkle split; it's the secure, proven choice

- **Do NOT unify on Velopack.** A single proven cross-platform C++ silent updater **does not exist**
  in 2026. Velopack is closest but is **disqualified for now on security + maturity** (below).
- **Keep Sparkle (macOS) + WinSparkle (Windows)** — the mature pattern, used by **OBS Studio** (C++,
  CEF-using, 60M+ installs: the exact archetype). Both already do **independent EdDSA signing** — the
  security property Velopack lacks. **We already ship both**, so this is mostly **CONFIG, not a
  replacement.**
- **Enable silent updates:**
  - **macOS / Sparkle:** silent install-on-quit is a config flip (well-supported). ~1–2 days +
    hands-on test. See `A6_SILENT_UPDATE_TEST_PLAN.md`.
  - **Windows / WinSparkle:** functionally-silent via `win_sparkle_check_update_without_ui()` + a
    per-user **silent NSIS installer** (`/S` via `sparkle:installerArguments`). This is the **weaker
    silent story** — historically a sticking point — so **verify hands-on**. If it can't reach true
    silent, *then* revisit Velopack-on-Windows.
- **Achievable bar:** "open the browser → already the new version, zero clicks." "Updates while the
  app is closed for weeks" needs a scheduled task/LaunchAgent (later, +~1 wk) or Omaha (no).

## Why NOT Velopack (the unified candidate) — disqualified for now

| Concern | Finding |
|---------|---------|
| **Security — the dealbreaker** | Velopack's **update feed/manifest is NOT cryptographically signed** (only per-package SHA hashes). A compromised CDN/S3 bucket or MITM can serve a malicious `releases.json` → the client installs it. Sparkle/WinSparkle sign the appcast with **EdDSA**: even a full server compromise can't forge an update without the offline private key. **For a browser handling real BSV money, this gap is disqualifying.** |
| Maturity | **v1.0 released 2026-05-26 (5 days before this eval).** Spent 2+ yrs in 0.0.x. |
| Bus factor | One maintainer = **51% of all commits**; macOS is the lower-priority platform. |
| macOS track record | **No named C++/macOS production users.** Issue #204 is exactly the **symlink-in-`.app`-Frameworks** failure mode CEF's bundle would trigger. New-macOS-version regressions (e.g. Tahoe) recur. |
| C++ SDK | Thin FFI wrapper; least-mature language path; no combined apply+restart call. |

**Revisit Velopack only when:** it ships cryptographic feed signing (≈ Sparkle's EdDSA), earns a
named C++/macOS production track record, and has 12+ months of v1.x field time. Until then, the split
is both safer and cheaper (we already have it).

## Why the "OS security makes it impossible" fear was overstated (still true)
- **Windows per-user `%LOCALAPPDATA%` install = no UAC.** In-place update needs zero elevation. (HIGH)
- **macOS same-team-signed self-update = no App Management prompt**; Gatekeeper doesn't quarantine our
  own running app's downloads. (HIGH)
- SmartScreen is an *initial-installer* reputation concern, not an auto-update blocker. (HIGH)
- The real blocker was never the OS — it was **WinSparkle's notify-first default** + us never enabling
  Sparkle's silent mode. Both are fixable in config.

## Security requirements (non-negotiable — bake into the implementation)
1. **EdDSA appcast signing on BOTH platforms.** Sparkle: `SUPublicEDKey` + `sign_update`. WinSparkle:
   EdDSA (default since v0.9.x — **confirm we're on EdDSA, not deprecated DSA**). Private keys stay
   **offline**, never on the build server unencrypted.
2. **HTTPS appcast only** (Sparkle enforces ATS; plain HTTP was the 2016 MITM bug).
3. **Monotonic `CFBundleVersion`** → Sparkle/WinSparkle silently reject downgrades (anti-rollback).
4. **Pin Sparkle ≥ 2.7.2** (fixes the 2025 CERT Polska XPC CVEs: CVE-2025-10015/10016, CVE-2025-0509);
   prefer latest 2.9.x. WinSparkle ≥ 0.9.3.
5. **Two 2026 Sparkle CVEs are currently UNPATCHED through 2.9.1** — CVE-2026-47122 (appcast-item
   injection) and CVE-2026-47121 (binary-delta symlink traversal). Both **local-only** (need existing
   code execution; 47121 needs a malicious delta, which EdDSA+HTTPS blocks remotely). **Mitigate:**
   consider **disabling binary deltas** (ship full updates) to dodge 47121, and **monitor Sparkle
   releases** to patch the moment a fix ships.

## Recommendation

| Platform | Updater | Action | Effort |
|----------|---------|--------|--------|
| **macOS** | **Sparkle** (keep) | Enable silent install-on-quit (`SUAutomaticallyUpdate=YES`, `SUEnableAutomaticChecks=YES`); ensure user-owned bundle; pin ≥2.7.2; run test plan | ~1–2 days |
| **Windows** | **WinSparkle** (keep) | `win_sparkle_check_update_without_ui()` + per-user silent NSIS (`/S`); confirm EdDSA; **verify silent hands-on** | ~2–3 days + test |
| **Both** | — | Offline EdDSA keys, HTTPS appcast, monotonic version, delta-off mitigation, CVE monitoring | folded in |
| **Later** | scheduled task / LaunchAgent | "update while app closed" (true Chrome parity) | +~1 wk |
| **Parked** | Velopack | Revisit when it has feed signing + macOS/C++ track record | — |

## Honest unknowns / hands-on items
- **WinSparkle true-silent** needs hands-on confirmation — it's the weaker half; if it can't hit
  zero-UI, Velopack-on-Windows is the fallback (accepting its feed-signing gap, or build-our-own).
- Confirm our current vendored **Sparkle version** (must be ≥2.7.2) and whether we're on **EdDSA**
  (not legacy DSA) on both platforms.
- Confirm macOS app bundle is **user-owned** (not root `/Applications`) so silent install needs no auth.
- Decide binary-deltas on/off (off = dodge CVE-2026-47121, at cost of larger downloads).
