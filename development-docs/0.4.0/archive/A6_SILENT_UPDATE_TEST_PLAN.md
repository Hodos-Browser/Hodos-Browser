# A6 — Silent Auto-Update: Hands-On Verification Test Plan

**Created:** 2026-06-01 · **For:** the implementation phase of A6 (NOT this research phase).
**Why:** the research says silent updates work + are secure; this proves it on real builds before we
trust it. Run macOS on the M1; Windows on the 32 GB box.

> Decision (from `A6_AUTO_UPDATE.md`): keep **Sparkle (macOS) + WinSparkle (Windows)**, enable silent,
> EdDSA-signed, HTTPS appcast. This plan verifies: (1) silent install-on-quit actually fires with no
> prompts, (2) tampered/unsigned updates are REJECTED, (3) downgrades are REJECTED.

---

## Part A — macOS / Sparkle (on the M1)

**Prereqs:** Xcode CLI tools, Python 3, Developer ID cert in Keychain. **Confirm Sparkle ≥ 2.7.2**
(fixes 2025 XPC CVEs); prefer latest 2.9.x. Confirm app bundle will be **user-owned** (install to
`~/Applications`, not root `/Applications`).

1. **Generate EdDSA keys (once):** `generate_keys` (stores private key in login Keychain, prints
   `SUPublicEDKey`). Export + store the private key OFFLINE: `generate_keys -x ~/sparkle_private.key`.
2. **Info.plist (silent config):**
   ```xml
   <key>SUFeedURL</key>            <string>https://<cdn>/appcast.xml</string>
   <key>SUPublicEDKey</key>        <string>BASE64_PUBLIC_KEY</string>
   <key>SUEnableAutomaticChecks</key>   <true/>   <!-- suppresses 2nd-launch prompt -->
   <key>SUAutomaticallyUpdate</key>     <true/>   <!-- install on quit -->
   <key>SUAllowsAutomaticUpdates</key>  <true/>
   <key>SUScheduledCheckInterval</key>  <integer>86400</integer>  <!-- 60 for testing -->
   ```
   (For a LOCAL http test feed, add a temporary `NSAppTransportSecurity` 127.0.0.1 exception — REMOVE
   before shipping; production appcast MUST be HTTPS.)
3. **Build v1** (`CFBundleVersion=1`), archive → Developer ID export (signs+notarizes) → install to
   `~/Applications/`.
4. **Build v2** (`CFBundleVersion=2`), archive/export. Zip with symlinks: `zip -r --symlinks v2.zip
   YourApp.app`. Sign: `sign_update v2.zip` → copy `sparkle:edSignature` + `length`. (Or
   `generate_appcast updates/` to auto-produce the signed appcast.)
5. **Appcast.xml** — one `<item>` with `<sparkle:version>2</sparkle:version>`,
   `<sparkle:minimumSystemVersion>`, and the signed `<enclosure url=… sparkle:edSignature=… length=…>`.
6. **Serve:** `python3 -m http.server 9191` in the updates folder; verify with `curl`.
7. **Observe silent flow** (Console.app filtered to the app): launch v1 from `~/Applications`, force a
   background check (or wait the interval). Expect, with **NO dialogs**: check → download → on Cmd-Q
   install → relaunch shows v2 (`mdls … | grep kMDItemVersion`). *Auth dialog = bundle is root-owned →
   `chown -R $(whoami)` and retry.*
8. **Security: tamper test** — flip a byte in v2.zip, keep the old signature, re-serve, force re-check
   (`defaults delete <bundleid> SULastCheckTime`). Expect **signature-mismatch rejection**, no install,
   no crash. Repeat with a random `sparkle:edSignature` → also rejected.
9. **Security: downgrade test** — with v2 installed, publish a (properly signed) v1 item
   (`<sparkle:version>1`). Force re-check. Expect Sparkle **skips** it (version not greater), no install.
10. **Cleanup:** remove the ATS exception, restore interval to 86400, point `SUFeedURL` at the real
    HTTPS CDN, kill the server, delete test defaults.

---

## Part B — Windows / WinSparkle (on the 32 GB box)

> This is the **weaker silent story** — the real question is whether WinSparkle reaches *zero-UI*. If
> it can't, fall back to Velopack-on-Windows (accepting its feed-signing gap) or a custom updater.

**Prereqs:** WinSparkle ≥ 0.9.3; **confirm EdDSA** (not deprecated DSA) keys; a **per-user silent NSIS
installer** that installs to `%LOCALAPPDATA%` and supports `/S`.

1. **EdDSA keys:** generate WinSparkle EdDSA keypair; embed public key; keep private key OFFLINE.
2. **Silent config:** call `win_sparkle_set_appcast_url(https…)`, set the EdDSA public key, then
   `win_sparkle_check_update_without_ui()` on a background check. In the appcast item, set
   `sparkle:installerArguments` to `/S` (NSIS silent) and ensure the installer is per-user (no UAC).
3. **Build v1** (per-user install) → run it.
4. **Build v2**, produce the silent NSIS installer, EdDSA-sign it, publish the appcast item.
5. **Observe:** trigger the background check. Goal: download + apply + relaunch to v2 **with no visible
   WinSparkle dialog and no UAC**. Record exactly what UI (if any) appears — this is the make-or-break
   observation for keeping WinSparkle vs falling back to Velopack.
6. **Security: tamper test** — serve a tampered installer with the old signature → expect rejection.
7. **Security: downgrade test** — publish an older version → expect it's not offered.
8. **Cleanup / record findings** in `A6_AUTO_UPDATE.md` (WinSparkle silent: achieved? caveats?).

---

## Pass/fail acceptance
- [ ] macOS: check → download → install-on-quit → relaunch to new version, **zero prompts**.
- [ ] Windows: equivalent **zero-UI** path (or documented decision to fall back to Velopack).
- [ ] Tampered/unsigned update **rejected** on both platforms.
- [ ] Downgrade **rejected** on both platforms.
- [ ] HTTPS appcast, EdDSA verified, private keys offline, Sparkle ≥2.7.2, binary-deltas decision made.

## Known security watch-items (carry into prod)
- Sparkle **CVE-2026-47122 / CVE-2026-47121 unpatched** through 2.9.1 (both local-only; 47121 = delta
  traversal). Mitigate via **deltas-off** + **monitor Sparkle releases** and patch on fix.
