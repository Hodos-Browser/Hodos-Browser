# Hodos Browser — Build & Release Guide

**Created:** 2026-03-20
**Last Updated:** 2026-04-27
**Purpose:** How to build installers, sign code, ship updates, and manage releases

> **Single source of truth.** This file in `development-docs/` is the canonical build & release guide. Any out-of-tree copies (e.g. older `Marston Enterprises/Hodos/Hodos_CI_CD/` snapshots) are deprecated — edit only this one.

---

## Current Status (2026-04-27)

**Next release: TBD**
**Last shipped: `v0.3.0-beta.7`**

| Component | Status |
|-----------|--------|
| Windows installer (Inno Setup) | WORKING — signed via Azure Artifact Signing |
| Windows portable zip | WORKING |
| macOS DMG | WORKING — signed + notarized + stapled in CI |
| GitHub Actions CI/CD | WORKING — tag-triggered, builds both platforms |
| Website (hodosbrowser.com) | LIVE — download links active for beta.7 |
| Auto-update (Windows/WinSparkle) | WORKING — verified end-to-end on beta.4 → beta.5 |
| Auto-update (macOS/Sparkle 2) | FIXED in beta.6 — Sparkle EdDSA signing path corrected; verify before each release |
| Appcast generation | INTEGRATED — CI generates appcast.xml as release artifact |
| Install directory | `{localappdata}\HodosBrowser` (per-user, no UAC for updates) |
| AV reputation (SmartScreen) | DEGRADED — Microsoft transitioned us to `EOC CA 03` intermediate in March 2026; reputation accumulation regressed industry-wide. See §2.5.1. |

### How to Release a New Version — Complete Checklist

**Before you start:** Decide on the version number. Format: `X.Y.Z-beta.N` (e.g., `0.3.0-beta.8`).

---

#### Step 1: Bump version

For beta increments, only one file needs to change. For major/minor bumps, also update CMakeLists.txt.

| File | What to change | Example | When |
|------|---------------|---------|------|
| `frontend/src/components/settings/AboutSettings.tsx` | `APP_VERSION` constant | `const APP_VERSION = '0.3.0-beta.8';` | Every release |
| `installer/hodos-browser.iss` (default fallback) | `#define AppVersion` | `"0.3.0-beta.8"` | Every release (cosmetic — CI overrides via `/DAppVersion`, but keep in sync for clarity) |
| `cef-native/CMakeLists.txt` | `MACOSX_BUNDLE_*_VERSION` (macOS only) | `"0.3.1"` | Major/minor bumps only |
| `rust-wallet/Cargo.toml` | `version` | `"0.3.1"` | Major/minor bumps only |

**Automatic — no manual change needed:**
- `cef-native/cef_browser_shell.cpp` — uses `APP_VERSION` define injected by CMake from the CI tag
- The signed installer's `OutputBaseFilename` — derived from `/DAppVersion` in CI

#### Step 2: Commit and push

```bash
git add -A
git commit -m "Bump version to X.Y.Z-beta.N"

# Merge to main (if working on a feature branch)
git checkout main
git merge <your-branch>

# Push to dev repo (private)
git push origin main

# Push to release repo (public — may need: git pull release main first if diverged)
git push release main
```

#### Step 3: Tag and trigger CI build

```bash
git tag vX.Y.Z-beta.N
git push release vX.Y.Z-beta.N
```

CI takes ~35 min (beta.6 = 19 min, beta.7 = ~28 min — varies). Monitor at: https://github.com/Hodos-Browser/Hodos-Browser/actions

#### Step 4: Get DSA signature from CI logs

After build completes, find the signature in the CI logs:

```bash
# Using gh CLI:
"/mnt/c/Program Files/GitHub CLI/gh.exe" run list --repo Hodos-Browser/Hodos-Browser --limit 1 --json databaseId --jq '.[0].databaseId'
"/mnt/c/Program Files/GitHub CLI/gh.exe" run view <RUN_ID> --repo Hodos-Browser/Hodos-Browser --log 2>&1 | grep "DSA signature:"
```

The signature looks like: `MD0CHQCvt9FfZ6Q9Co/s...`

#### Step 5: Update appcast.xml on website

```bash
python3 scripts/generate-appcast.py \
  --version "X.Y.Z-beta.N" \
  --windows-url "https://github.com/Hodos-Browser/Hodos-Browser/releases/download/vX.Y.Z-beta.N/HodosBrowser-X.Y.Z-beta.N-setup.exe" \
  --windows-size 0 \
  --windows-signature "<PASTE DSA SIGNATURE FROM STEP 4>" \
  --macos-url "https://github.com/Hodos-Browser/Hodos-Browser/releases/download/vX.Y.Z-beta.N/HodosBrowser-X.Y.Z-beta.N.dmg" \
  --macos-size 0 \
  --output "C:\Users\archb\Marston Enterprises\Hodos\website\public\appcast.xml"

cd "C:\Users\archb\Marston Enterprises\Hodos\website"
git add public/appcast.xml
git commit -m "Update appcast.xml for vX.Y.Z-beta.N with DSA signature"
git push
```

Wait 1–2 min for Cloudflare to deploy. Verify at: https://hodosbrowser.com/appcast.xml

#### Step 6: Update website download links

Edit the website repo at `C:\Users\archb\Marston Enterprises\Hodos\website`:

| File | What to change |
|------|----------------|
| `public/_redirects` | Update `/download/win` and `/download/mac` URLs to point at `vX.Y.Z-beta.N` GitHub Release assets |
| `src/pages/index.astro` | Update `const version = "X.Y.Z-beta.N"` (the version text shown to users) |

```bash
cd "C:\Users\archb\Marston Enterprises\Hodos\website"
git add -A
git commit -m "Update download links and version to vX.Y.Z-beta.N"
git push
```

#### Step 7: Publish the GitHub Release

Go to: https://github.com/Hodos-Browser/Hodos-Browser/releases
- Find the draft release for `vX.Y.Z-beta.N`
- Verify all 5 assets are present (installer .exe, portable .zip, macOS .dmg, appcast.xml, SHA256SUMS.txt)
- Click **Publish release**

#### Step 8: Anti-virus / reputation seeding

Submit signed binaries to anti-virus vendors and reputation services **immediately after publishing**. Chromium-based executables trigger heuristic false positives — pre-submission builds reputation. See §2.5 for full detail; this is the operational summary.

**Pre-step: verify which intermediate CA signed this build** (run on Windows in PowerShell):

```powershell
$file = "$env:USERPROFILE\Downloads\HodosBrowser-X.Y.Z-beta.N-setup.exe"
$sig = Get-AuthenticodeSignature $file
$chain = New-Object Security.Cryptography.X509Certificates.X509Chain
[void]$chain.Build($sig.SignerCertificate)
$chain.ChainElements | ForEach-Object { Write-Host "  -> $($_.Certificate.Subject)" }
```

If the chain shows `Microsoft ID Verified CS EOC CA 03`, you're on the post-March-2026 regression CA — call this out in MS Defender submissions. Detail in §2.5.1.

**Submit to all of these (every release):**

| Service | URL | What to submit | File state | Notes |
|---|---|---|---|---|
| VirusTotal | https://www.virustotal.com/ | Windows installer + portable zip | Raw .exe (not zipped) | 70+ AV engines. Public report. Save the URL. |
| Microsoft Defender | https://www.microsoft.com/en-us/wdsi/filesubmission | Windows installer | Raw .exe (not zipped) | Seeds SmartScreen reputation. Choose "Software developer" / "Incorrectly detected." Submission ID arrives via email. |
| Google Safe Browsing | https://safebrowsing.google.com/safebrowsing/report_error/ | Download page URL | (URL, not file) | Only if Chrome blocks the download. |
| Norton (only if flagged) | https://submit.norton.com/ | Windows installer | **ZIP'd .exe** (Norton requires archived; not password-protected; max 500 MB) | Pick "False positive" → "File". ~48h turnaround. Email fallback: `avsubmit@symantec.com`. |

**Process:**
1. Download release artifacts from GitHub Releases (same URLs users will hit).
2. Run cert-chain check (above).
3. Upload to VirusTotal — note detection count.
4. Submit to MS Defender (every release, regardless of VT result).
5. If Norton flags the build in the wild, submit to Norton portal too.
6. Track every submission in the release issue per §2.5.2 (vendor, date, submission ID, outcome).

**Timeline:** VT scan ~2 min. MS Defender review 1–3 business days; submission instant. Norton ~48h.

#### Step 9: Verify auto-update

- Users with the previous version installed should see an "Update available" dialog on next browser launch
- Or: Settings > About > "Check for updates" button
- WinSparkle downloads the installer, verifies the DSA signature, and prompts to install

---

**What's automatic (CI handles):**
- Windows + macOS builds
- Azure code signing (Windows executables)
- macOS code signing + notarization attempt
- DSA signing of Windows installer for WinSparkle
- Appcast.xml generation with DSA signature (as release artifact)
- Version injection into C++ binary via `-DAPP_VERSION=` CMake flag

**What's still manual:**
- Bumping `APP_VERSION` in frontend AboutSettings.tsx (Step 1)
- Copying appcast.xml (with DSA sig) to website repo (Step 5)
- Updating `_redirects` and `index.astro` in website (Step 6)
- Publishing the draft release on GitHub (Step 7)
- AV submissions (Step 8)

**Important notes:**
- The release repo is `Hodos-Browser/Hodos-Browser` (public). Dev repo is `BSVArchie/Hodos-Browser` (private).
- CI triggers on tags pushed to the release repo only.
- If `git push release main` is rejected, run `git pull release main` first to merge any divergence.
- Install directory is `{localappdata}\HodosBrowser` (per-user, no UAC) since v0.2.0.
- Dependabot is enabled via GitHub UI on the dev repo (no `.github/dependabot.yml` checked in). Manage dependency updates manually via the PRs it raises.
- DSA private key is in GitHub Secret `WINSPARKLE_DSA_PRIVATE_KEY`. EdDSA key in `SPARKLE_EDDSA_PRIVATE_KEY`.
- Local key files are in `external/keys/` (gitignored). Back them up securely.

---

## Quick Reference

| Item | Value |
|------|-------|
| Version format | MAJOR.MINOR.PATCH-prerelease (semver) |
| Git tag format | `v0.1.0-beta.1`, `v1.0.0` |
| Dev repo | `BSVArchie/Hodos-Browser` (private) |
| Release repo | `Hodos-Browser/Hodos-Browser` (public) |
| Website repo | `Hodos-Browser/hodosbrowser.com` |
| Website URL | `https://hodosbrowser.com` |
| Appcast URL | `https://hodosbrowser.com/appcast.xml` |
| CI dashboard | `https://github.com/Hodos-Browser/Hodos-Browser/actions` |
| Release drafts | `https://github.com/Hodos-Browser/Hodos-Browser/releases` |
| Website local path | `C:\Users\archb\Marston Enterprises\Hodos\website` |
| DSA keys local path | `C:\Users\archb\Hodos-Browser\external\keys\` |

---

## 1. Prerequisites & Costs

### 1.1 Purchases Made

| Item | Cost | Vendor | Status |
|------|------|--------|--------|
| Azure Artifact Signing | ~$120/yr ($9.99/mo) | Microsoft Azure | DONE |
| Apple Developer Program | $99/yr | Apple | DONE (individual account) |
| Domain (hodosbrowser.com) | ~$10/yr | Cloudflare | DONE |

### 1.2 Windows Code Signing — DECIDED: Azure Artifact Signing

Chose Azure Artifact Signing (formerly Trusted Signing) over OV/EV certificates:
- **Instant SmartScreen trust** (like EV) — no warnings from day one
- **No hardware tokens** — fully cloud-managed
- **Native GitHub Action** — `azure/trusted-signing-action`
- **~$120/yr** — cheaper than OV (~$280/yr) or EV (~$400-600/yr)

> ⚠️ **Caveat (added 2026-04-27):** Since Microsoft's March 2024 SmartScreen policy change, even EV-equivalent certs no longer give *instant* clearance — reputation must build per file-hash + cert-thumbprint. Compounded by Microsoft's March 2026 transition to new intermediate CAs (`EOC CA 03`, `AOC CA 03`, `EOC CA 04`), which broke accumulated reputation industry-wide. See §2.5.1.

Configuration:
- Account: `Hodos-Signing` (West Central US)
- Profile: `Hodos-signing`
- Endpoint: `https://wcus.codesigning.azure.net/`
- Certificate: `CN=Marston Enterprises, O=Marston Enterprises, L=Peyton, S=Colorado, C=US`

### 1.3 GitHub Infrastructure

**Organization:** `hodos-browser`

| Repository | Purpose |
|------------|---------|
| `Hodos-Browser/Hodos-Browser` | Main browser source code (public release repo) |
| `BSVArchie/Hodos-Browser` | Dev repo (private) |
| `Hodos-Browser/hodosbrowser.com` | Website (Cloudflare Pages) |

---

## 2. Windows Build & Installer

### 2.1 Installer Format

**Tool:** Inno Setup 6

| Why Inno Setup | |
|----------------|--|
| Easy to learn | Pascal-based scripting |
| Professional results | Modern UI, used by VLC, 7-Zip |
| Code signing | Native support |
| Well-documented | Extensive help + community |

### 2.2 Installation Directory

**Decision:** Per-user install (Chrome/Brave model)

```
Installation:     %LOCALAPPDATA%\HodosBrowser\
User Data:        %APPDATA%\HodosBrowser\
Start Menu:       %APPDATA%\Microsoft\Windows\Start Menu\Programs\Hodos Browser\
```

**Why per-user:**
- No admin/UAC prompts
- Auto-update can write without elevation
- Industry standard for browsers

### 2.3 Directory Structure

```
%LOCALAPPDATA%\HodosBrowser\
├── HodosBrowser.exe          # Main browser
├── hodos-wallet.exe          # Rust wallet backend
├── hodos-adblock.exe         # Adblock backend
├── WinSparkle.dll            # Auto-update library
├── libcef.dll                # CEF library
├── chrome_elf.dll            # Chrome helper
├── *.pak, *.dat, *.bin       # CEF resources
├── locales\                  # Language files
└── resources\                # Frontend HTML/CSS/JS

%APPDATA%\HodosBrowser\
├── Default\                  # Default profile
│   ├── wallet.db             # Wallet database
│   ├── Cache\                # Browser cache
│   └── ...
├── Profile 1\                # Additional profiles
└── adblock\                  # Adblock data
```

### 2.4 Inno Setup Script

Full script at: `installer/hodos-browser.iss`

Key sections:
```iss
[Setup]
AppName=Hodos Browser
AppVersion={#AppVersion}
DefaultDirName={localappdata}\HodosBrowser
PrivilegesRequired=lowest              ; No admin needed
SignTool=signtool                      ; Code signing
Compression=lzma2/ultra64

[Files]
Source: "build\Release\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs

[Registry]
; Default browser registration
Root: HKCU; Subkey: "Software\RegisteredApplications"; ...
Root: HKCU; Subkey: "Software\Clients\StartMenuInternet\HodosBrowser"; ...

[Run]
Filename: "{app}\HodosBrowser.exe"; Flags: nowait postinstall
```

### 2.5 Code Signing & AV reputation

> **Important:** Sign ALL binaries, not just the installer. Unsigned DLLs inside a signed installer still trigger antivirus and SmartScreen warnings.

**Files that must be signed:**
- `HodosBrowser.exe` (main browser)
- `hodos-wallet.exe` (Rust wallet)
- `hodos-adblock.exe` (adblock engine)
- `libcef.dll` and other CEF DLLs
- CEF subprocess executable
- `HodosBrowser-X.Y.Z-beta.N-setup.exe` (the installer itself)

**Sign command:**
```powershell
signtool sign /f "hodos-codesign.pfx" /p "$PASSWORD" `
  /tr http://timestamp.sectigo.com /td sha256 /fd sha256 `
  "HodosBrowser-X.Y.Z-beta.N-setup.exe"
```

**Verify signature:**
```powershell
signtool verify /pa "HodosBrowser-X.Y.Z-beta.N-setup.exe"
```

**In CI/CD:** Azure Trusted Signing credentials stored as GitHub Secrets (`AZURE_TENANT_ID`, `AZURE_CLIENT_ID`, `AZURE_CLIENT_SECRET`).

**Pre-release antivirus / SmartScreen seeding:** Chromium-based installers and Rust binaries trigger heuristic AV detections. Each vendor maintains its own definitions, so submission to one does **not** propagate to others. Do all three steps below for every public release (skip on diag/internal builds):

1. **VirusTotal (pre-flight check)** — https://www.virustotal.com/
   - Upload `HodosBrowser-X.Y.Z-setup.exe` and review which engines flag it.
   - This is a *scanner*, not a whitelist submission — it tells you who to chase, but doesn't clear anything.

2. **Microsoft Defender (whitelist + reputation)** — https://www.microsoft.com/en-us/wdsi/filesubmission
   - Submit signed binaries (installer + all signed exes/DLLs from above).
   - Choose "Software developer" submission type.
   - Provide the cert thumbprint and a build/release URL so Microsoft can correlate future builds.
   - This seeds SmartScreen reputation and clears Defender false positives.

3. **Norton (only when Norton specifically flags us)** — https://submit.norton.com/
   - Pick "False positive" → "File" (not URL) at the top of the form.
   - Upload the installer in a ZIP or RAR (Norton requires the file to be archived; **must not be password-protected**; max 500 MB).
   - Provide the **Detection name** and **Alert ID** from the affected user's alert popup if you have them — this is in the bottom-right of the Norton alert that fires when the user tries to install.
   - Norton typically pushes updated definitions within ~48 hours. They send a tracking number you can use to follow up via the community portal.
   - **Email fallback** if the portal returns "internal error": `avsubmit@symantec.com` with the zipped installer attached.

If a different vendor (BitDefender, ESET, Kaspersky, McAfee, etc.) flags us in the wild, look up that vendor's developer-submission portal — there's no shared whitelist across the industry. Track per-vendor submissions in the release issue.

#### 2.5.1 Cert chain verification (run before every submission)

Verify which intermediate CA signed the installer — useful to know whether you're on the regression-affected `EOC CA 03` cohort:

```powershell
$file = "$env:USERPROFILE\Downloads\HodosBrowser-X.Y.Z-setup.exe"
$sig = Get-AuthenticodeSignature $file
$chain = New-Object Security.Cryptography.X509Certificates.X509Chain
[void]$chain.Build($sig.SignerCertificate)
$chain.ChainElements | ForEach-Object { Write-Host "  -> $($_.Certificate.Subject)" }
```

**As of March 2026**, Azure Trusted Signing transitioned new releases to the intermediate CA `Microsoft ID Verified CS EOC CA 03`. Files signed under this CA experienced a SmartScreen reputation regression — accumulated reputation from earlier CAs did **not** carry over. Reference: https://learn.microsoft.com/en-us/answers/questions/5855708/trusted-signing-regression-in-smartscreen-reputati

If we land back on `EOC CA 02` (the pre-regression CA) on a future release, reputation should accrue normally again. Mention the regression in MS Defender submissions while we're stuck on `EOC CA 03`.

**Confirmed for beta.7:** signed via `EOC CA 03` (regression cohort).

#### 2.5.2 Per-release submission tracking

Maintain a row in the release issue (or a separate tracking file) for every release:

| Vendor | Submitted | Submission ID | File state | Outcome |
|---|---|---|---|---|
| VirusTotal | YYYY-MM-DD | (report URL) | raw .exe | X/72 engines flagged |
| MS Defender | YYYY-MM-DD | uuid from email | raw .exe | pending → cleared |
| Norton | only if flagged | tracking number | .zip (Norton requires) | pending → cleared |

Submission IDs come from the confirmation email (Microsoft) or the success screen (Norton). The Microsoft portal's "submission details" page sometimes shows "Unable to access submission details" right after submit — that's a portal display bug; the email confirmation is authoritative.

**Submission discipline:** submit to MS Defender for **every** public release, not only when flagged. This builds *publisher* reputation independently of *file* reputation.

**beta.7 submission record:**
- VirusTotal: submitted 2026-04-26 (clean per scan)
- MS Defender: submitted 2026-04-26, ID `d02ca5bc-d3af-4932-89b2-8a766a37ae90`
- Norton: not yet flagged in beta.7 wild; portal was broken on beta.6, no submission landed

#### 2.5.3 Reputation-building strategy (per-release)

Reputation in SmartScreen is keyed on file hash + cert thumbprint, accumulated via telemetry from real successful installs. To accelerate it:

1. Within the first 24h after publish, get 5–10 trusted internal testers (different machines / networks) to download and install
2. Have each tester click "Run anyway" past any SmartScreen warning and complete the install
3. Early successful installs are weighted heavily; spreading them across machines/IPs helps more than one tester running it 10 times

Don't fake telemetry by clicking through with a script — Microsoft does deduplicate suspicious patterns.

#### 2.5.4 Required: VERSIONINFO in all signed exes (beta.8 forward)

Empty version metadata is a heuristic red flag for AV engines and SmartScreen. **All** of our signed Windows binaries must embed `VERSIONINFO`:

- `HodosBrowser.exe` — embed via `cef-native/hodos.rc` (currently has only an icon block; needs a `VERSIONINFO` block, version pulled from CMake `-DAPP_VERSION=`)
- `hodos-wallet.exe` — add `winresource` build-dependency in `rust-wallet/Cargo.toml` + `build.rs` that calls `winresource::WindowsResource::new().set(...).compile()`
- `hodos-adblock.exe` — same pattern in `adblock-engine/`

Required fields per binary:

| Field | Value |
|---|---|
| `FileDescription` | "Hodos Browser" / "Hodos Wallet Backend" / "Hodos Adblock Engine" |
| `FileVersion` | `0.3.0.7` (Win32 4-part) for tag `v0.3.0-beta.7` |
| `ProductName` | "Hodos Browser" |
| `ProductVersion` | `0.3.0-beta.7` (semver string) |
| `CompanyName` | "Marston Enterprises" |
| `LegalCopyright` | "© 2026 Marston Enterprises. All rights reserved." |
| `OriginalFilename` | matches the exe name |

Verify post-build via `(Get-Item HodosBrowser.exe).VersionInfo` — every field should be populated, not "Unknown".

### 2.6 Build Script

```powershell
# scripts/build-installer.ps1
param(
    [string]$Version = "1.0.0",
    [switch]$Sign
)

# 1. Build browser
cmake --build build --config Release

# 2. Build Rust backends
cargo build --release --manifest-path rust-wallet/Cargo.toml
cargo build --release --manifest-path adblock-engine/Cargo.toml

# 3. Create installer
& "C:\Program Files (x86)\Inno Setup 6\ISCC.exe" installer/hodos-browser.iss

# 4. Sign if requested
if ($Sign) {
    signtool sign /f $env:CERT_PATH /p $env:CERT_PASSWORD `
      /tr http://timestamp.sectigo.com /td sha256 /fd sha256 `
      "dist\HodosBrowser-$Version-setup.exe"
}
```

---

## 3. macOS Build & Installer

### 3.1 App Bundle Structure

```
Hodos.app/
└── Contents/
    ├── Info.plist                  # App metadata
    ├── MacOS/
    │   └── Hodos                   # Main executable
    ├── Frameworks/
    │   └── Chromium Embedded Framework.framework/
    ├── Resources/
    │   ├── hodos.icns              # App icon
    │   ├── hodos-wallet            # Rust wallet
    │   ├── hodos-adblock           # Adblock engine
    │   └── resources/              # Frontend
    └── _CodeSignature/
```

### 3.2 Code Signing

> **Important:** `codesign --deep` can miss nested frameworks. Explicitly sign `Chromium Embedded Framework.framework` before signing the outer `.app` bundle.

```bash
# Sign CEF framework first
codesign --force --options runtime \
  --sign "Developer ID Application: Marston Enterprises (TEAMID)" \
  "Hodos.app/Contents/Frameworks/Chromium Embedded Framework.framework"

# Then sign the app bundle
codesign --force --deep --options runtime \
  --entitlements macos/entitlements.plist \
  --sign "Developer ID Application: Marston Enterprises (TEAMID)" \
  Hodos.app
```

### 3.2.1 Required Entitlements

CEF requires specific hardened runtime entitlements or the app will crash on launch with no useful error. The `macos/entitlements.plist` must include:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <!-- CEF V8 JIT requires this -->
    <key>com.apple.security.cs.allow-unsigned-executable-memory</key>
    <true/>
    <!-- Loading CEF framework -->
    <key>com.apple.security.cs.disable-library-validation</key>
    <true/>
    <!-- JavaScript JIT compilation -->
    <key>com.apple.security.cs.allow-jit</key>
    <true/>
</dict>
</plist>
```

### 3.3 Notarization

**For CI/CD, use App Store Connect API key** (avoids Apple ID password and 2FA issues):

```bash
# Recommended: API key method (for CI/CD)
xcrun notarytool submit Hodos.dmg \
  --key "AuthKey_XXXX.p8" \
  --key-id "XXXX" \
  --issuer "XXXX-XXXX-..." \
  --wait

# Alternative: Apple ID method (for local builds)
xcrun notarytool submit Hodos.dmg \
  --apple-id "you@email.com" \
  --team-id "XXXXXXXXXX" \
  --password "app-specific-password" \
  --wait

# Staple the ticket
xcrun stapler staple Hodos.dmg
```

### 3.4 DMG Creation

```bash
create-dmg \
  --volname "Hodos Browser" \
  --window-size 600 400 \
  --icon "Hodos.app" 175 190 \
  --app-drop-link 425 190 \
  "dist/Hodos-$VERSION.dmg" \
  "build/Release/Hodos.app"
```

---

## 4. Auto-Update System (design)

> See also: the *implementation status* addendum at the end of this file (post-2026-03-30 reality).

### 4.1 Architecture

**Decision:** Silent auto-update by default (Chrome model)

```
Browser running (background, every 24h)
    ↓
Fetch appcast.xml from hodosbrowser.com
    ↓
Compare version to installed
    ↓
If newer: download silently to staging folder
    ↓
When user closes browser naturally: apply update
    ↓
Next launch: new version running (no interruption)
```

**User never sees popups.** Updates happen invisibly.

### 4.1.1 Auto-Update Library

***Decided:*** WinSparkle (Windows) + Sparkle 2 (macOS). Velopack was considered but not adopted.

| Option | Platforms | Delta Updates | Integration | Notes |
|--------|-----------|---------------|-------------|-------|
| **WinSparkle + Sparkle** | Win (WinSparkle) + Mac (Sparkle 2.x) | Mac only | C++ (WinSparkle), Obj-C (Sparkle) | Two separate libraries, proven and mature. **Currently in use.** |
| **Velopack** | Win + Mac + Linux | Yes (built-in) | **Has Rust SDK** (`velopack` crate) | Single cross-platform library; not adopted. |

### 4.2 User Settings

***Decided:*** Update settings live under **Settings → About**. "Notify me" mode uses a popup dialog (WinSparkle's native one).

```
┌─────────────────────────────────────────────────────────┐
│ Updates                                                  │
├─────────────────────────────────────────────────────────┤
│ ● Update automatically (recommended)                    │
│   Updates install silently when you close the browser   │
│                                                         │
│ ○ Notify me when updates are available                  │
│   (Native WinSparkle dialog)                            │
│                                                         │
│ Current version: 0.3.0-beta.7                           │
│ [Check for updates now]                                 │
└─────────────────────────────────────────────────────────┘
```

### 4.3 WinSparkle Integration (Windows)

**Library:** [WinSparkle](https://winsparkle.org/) — C++, ~100KB

```cpp
#include <winsparkle.h>

void InitializeAutoUpdate() {
    win_sparkle_set_app_details(L"marstonenterprises.com", L"Hodos Browser", L"1.0.0");
    win_sparkle_set_appcast_url(L"https://hodosbrowser.com/appcast.xml");
    win_sparkle_set_automatic_check_for_updates(1);
    win_sparkle_set_update_check_interval(86400); // 24 hours
    win_sparkle_init();
}

// On browser exit, check for staged update
void OnBrowserExit() {
    if (HasStagedUpdate()) {
        ShellExecute(NULL, "open", GetStagedInstallerPath(),
                     "/SILENT /UPDATE /RELAUNCH", NULL, SW_HIDE);
    }
    win_sparkle_cleanup();
}
```

### 4.4 Sparkle Integration (macOS)

**Framework:** [Sparkle 2.x](https://sparkle-project.org/)

```xml
<!-- Info.plist -->
<key>SUFeedURL</key>
<string>https://hodosbrowser.com/appcast-mac.xml</string>
<key>SUAutomaticallyUpdate</key>
<true/>
<key>SUPublicEDKey</key>
<string>YOUR_EDDSA_PUBLIC_KEY</string>
```

### 4.5 Appcast Format

```xml
<?xml version="1.0" encoding="utf-8"?>
<rss version="2.0" xmlns:sparkle="http://www.andymatuschak.org/xml-namespaces/sparkle">
  <channel>
    <title>Hodos Browser Updates</title>

    <!-- Windows -->
    <item>
      <title>Version 1.1.0</title>
      <sparkle:version>1.1.0</sparkle:version>
      <sparkle:os>windows</sparkle:os>
      <pubDate>Wed, 20 Mar 2026 12:00:00 +0000</pubDate>
      <sparkle:releaseNotesLink>https://hodosbrowser.com/releases/1.1.0</sparkle:releaseNotesLink>
      <enclosure
        url="https://github.com/Hodos-Browser/Hodos-Browser/releases/download/v1.1.0/HodosBrowser-1.1.0-setup.exe"
        sparkle:dsaSignature="SIGNATURE_HERE"
        length="85000000"
        type="application/octet-stream"/>
    </item>

    <!-- macOS -->
    <item>
      <title>Version 1.1.0</title>
      <sparkle:version>1.1.0</sparkle:version>
      <sparkle:os>macos</sparkle:os>
      <sparkle:minimumSystemVersion>10.15</sparkle:minimumSystemVersion>
      <enclosure
        url="https://github.com/Hodos-Browser/Hodos-Browser/releases/download/v1.1.0/Hodos-1.1.0.dmg"
        sparkle:edSignature="SIGNATURE_HERE"
        length="120000000"
        type="application/octet-stream"/>
    </item>
  </channel>
</rss>
```

### 4.6 Update Signing

**Windows (DSA):**
```bash
# Generate keys (one-time)
openssl dsaparam -genkey 2048 -out dsa_priv.pem
openssl dsa -in dsa_priv.pem -pubout -out dsa_pub.pem

# Sign installer
openssl dgst -sha1 -binary "HodosBrowser-1.1.0-setup.exe" | \
  openssl dgst -sha1 -sign dsa_priv.pem | openssl enc -base64
```

**macOS (EdDSA):**
```bash
# Generate keys (one-time)
./Sparkle.framework/Resources/generate_keys

# Sign DMG
./Sparkle.framework/Resources/sign_update "Hodos-1.1.0.dmg"
```

### 4.7 Hosting

**MVP:** GitHub Releases (free, CDN, 2GB file limit)

**Scale:** Cloudflare R2 (free egress, cheap storage) — when millions of downloads/month

---

## 5. CI/CD Pipeline

### 5.1 GitHub Actions Overview

| Workflow | Trigger | What it Does |
|----------|---------|--------------|
| `ci.yml` | Push/PR to main | Rust check, test, clippy; Frontend build, lint |
| `release.yml` | Tag `v*` | Build, sign, create installer, publish to Releases |

### 5.2 CI Workflow (ci.yml)

> ***Needs a decision:*** Should CI run Rust tests on both `windows-latest` and `macos-latest`? Currently only Windows. Since we ship on both platforms, running on both catches platform-specific issues on PRs rather than at release time. Trade-off is doubled GitHub Actions minutes.

```yaml
name: CI

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

jobs:
  rust-wallet:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
        with:
          workspaces: rust-wallet
      - run: cargo check --manifest-path rust-wallet/Cargo.toml
      - run: cargo test --manifest-path rust-wallet/Cargo.toml
      - run: cargo clippy --manifest-path rust-wallet/Cargo.toml -- -D warnings

  adblock-engine:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
        with:
          workspaces: adblock-engine
      - run: cargo check --manifest-path adblock-engine/Cargo.toml
      - run: cargo test --manifest-path adblock-engine/Cargo.toml

  security-scan:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo install cargo-audit
      - run: cargo audit --manifest-path rust-wallet/Cargo.toml
      - run: cargo audit --manifest-path adblock-engine/Cargo.toml
      - uses: actions/setup-node@v4
        with:
          node-version: 20
      - run: npm ci
        working-directory: frontend
      - run: npm audit --audit-level=high
        working-directory: frontend

  frontend:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with:
          node-version: 20
          cache: 'npm'
          cache-dependency-path: frontend/package-lock.json
      - run: npm ci
        working-directory: frontend
      - run: npm run build
        working-directory: frontend
      - run: npm run lint
        working-directory: frontend
```

### 5.3 Release Workflow (release.yml)

```yaml
name: Release

on:
  push:
    tags:
      - 'v*'

jobs:
  # Gate: run full test suite before building installers
  test-gate:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo test --manifest-path rust-wallet/Cargo.toml
      - run: cargo test --manifest-path adblock-engine/Cargo.toml

  build-windows:
    needs: [test-gate]
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4
        with:
          submodules: recursive

      - uses: ilammy/msvc-dev-cmd@v1
      - uses: dtolnay/rust-toolchain@stable

      - name: Cache CEF
        uses: actions/cache@v4
        with:
          path: cef-binaries
          key: cef-windows-136

      - name: Build
        run: |
          cmake -B build -G "Visual Studio 17 2022" -A x64
          cmake --build build --config Release
          cargo build --release --manifest-path rust-wallet/Cargo.toml
          cargo build --release --manifest-path adblock-engine/Cargo.toml

      - name: Create Installer
        run: iscc installer/hodos-browser.iss

      - name: Sign
        env:
          CERT_BASE64: ${{ secrets.WINDOWS_CERT_BASE64 }}
          CERT_PASSWORD: ${{ secrets.WINDOWS_CERT_PASSWORD }}
        run: |
          [IO.File]::WriteAllBytes("cert.pfx", [Convert]::FromBase64String($env:CERT_BASE64))
          signtool sign /f cert.pfx /p $env:CERT_PASSWORD `
            /tr http://timestamp.sectigo.com /td sha256 /fd sha256 `
            dist\HodosBrowser-*-setup.exe
          Remove-Item cert.pfx

      - uses: actions/upload-artifact@v4
        with:
          name: windows-installer
          path: dist/HodosBrowser-*-setup.exe

  build-macos:
    runs-on: macos-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable

      - name: Import Certificates
        env:
          MACOS_CERT_BASE64: ${{ secrets.MACOS_CERT_BASE64 }}
          MACOS_CERT_PASSWORD: ${{ secrets.MACOS_CERT_PASSWORD }}
        run: |
          echo $MACOS_CERT_BASE64 | base64 --decode > cert.p12
          security create-keychain -p "" build.keychain
          security import cert.p12 -k build.keychain -P "$MACOS_CERT_PASSWORD" -T /usr/bin/codesign
          security default-keychain -s build.keychain

      - name: Build
        run: |
          cmake -B build -DCMAKE_BUILD_TYPE=Release
          cmake --build build
          cargo build --release --manifest-path rust-wallet/Cargo.toml
          cargo build --release --manifest-path adblock-engine/Cargo.toml

      - name: Sign & Notarize
        env:
          APPLE_ID: ${{ secrets.APPLE_ID }}
          APPLE_TEAM_ID: ${{ secrets.APPLE_TEAM_ID }}
          APPLE_APP_PASSWORD: ${{ secrets.APPLE_APP_PASSWORD }}
        run: |
          ./scripts/sign-macos.sh
          ./scripts/create-dmg.sh
          ./scripts/notarize-macos.sh

      - uses: actions/upload-artifact@v4
        with:
          name: macos-dmg
          path: dist/Hodos-*.dmg

  publish:
    needs: [build-windows, build-macos]
    runs-on: ubuntu-latest
    steps:
      - uses: actions/download-artifact@v4

      - name: Generate Checksums
        run: sha256sum */* > SHA256SUMS.txt

      - uses: softprops/action-gh-release@v1
        with:
          files: |
            windows-installer/*
            macos-dmg/*
            SHA256SUMS.txt
          draft: true
          generate_release_notes: true
```

### 5.4 Required Secrets

**Azure Trusted Signing (current):**

| Secret | Purpose | How to Get |
|--------|---------|------------|
| `AZURE_TENANT_ID` | Azure AD tenant | Azure Portal |
| `AZURE_CLIENT_ID` | Service principal | Azure Portal |
| `AZURE_CLIENT_SECRET` | Service principal secret | Azure Portal |

**macOS:**

| Secret | Purpose | How to Get |
|--------|---------|------------|
| `MACOS_CERT_BASE64` | Apple cert (base64 P12) | Export from Keychain |
| `MACOS_CERT_PASSWORD` | P12 password | Set when exporting |
| `APPLE_TEAM_ID` | 10-char team ID | Developer portal |
| `APPLE_API_KEY_ID` | App Store Connect API key ID | App Store Connect → Users → Keys |
| `APPLE_API_ISSUER` | API key issuer UUID | App Store Connect → Users → Keys |
| `APPLE_API_KEY_P8_BASE64` | API key .p8 file (base64) | Download once from App Store Connect |

**Auto-update signing:**

| Secret | Purpose |
|--------|---------|
| `WINSPARKLE_DSA_PRIVATE_KEY` | Sign Windows installers for WinSparkle verification |
| `SPARKLE_EDDSA_PRIVATE_KEY` | Sign macOS DMGs for Sparkle 2 verification |

---

## 6. Version Management

### 6.1 Semantic Versioning

`MAJOR.MINOR.PATCH`

| Component | When to Increment | Example |
|-----------|-------------------|---------|
| **MAJOR** | Breaking changes (wallet DB incompatibility, protocol changes) | 1.0.0 → 2.0.0 |
| **MINOR** | New features (BRC support, new UI) | 1.0.0 → 1.1.0 |
| **PATCH** | Bug fixes, security patches | 1.0.0 → 1.0.1 |

**Pre-release:** `1.0.0-beta.1`, `1.0.0-rc.1`
**Build metadata:** `1.0.0+abc1234` (internal builds)

### 6.2 Version sources of truth

The beta suffix lives in **two** places (not in `rust-wallet/Cargo.toml`, despite older docs claiming otherwise):

1. **`frontend/src/components/settings/AboutSettings.tsx`** — `APP_VERSION` constant (user-visible Settings → About)
2. **The git tag** (`vX.Y.Z-beta.N`) — drives CI build, installer name, GitHub Release

`rust-wallet/Cargo.toml` and `cef-native/CMakeLists.txt` carry only the unsuffixed `MAJOR.MINOR.PATCH` and only change on major/minor bumps. `cef-native/cef_browser_shell.cpp` reads `APP_VERSION` injected at build time via `-DAPP_VERSION=` from CMake.

### 6.3 Release Process

See the 9-step checklist at the top of this file ("How to Release a New Version — Complete Checklist") — that's the authoritative process. Don't use the older 5-line snippet that floated around in earlier doc revisions.

---

## 7. Dependency Updates

### 7.1 CEF/Chromium Upgrades

**Cadence:** Quarterly, or immediately for security patches

**Process:**
1. Check CEF releases at https://cef-builds.spotifycdn.com/index.html
2. Download new binaries (or rebuild from source for codecs)
3. Rebuild `libcef_dll_wrapper`
4. Run full test suite
5. Test video playback (codec check)

**Subscribe to:** CEF announcements, Chromium security releases

### 7.2 Adblock List Updates

**Current:** Background task fetches updated lists every 6 hours
- EasyList, EasyPrivacy, uBlock Filters, uBlock Privacy
- Hosted upstream; no CDN intermediary

### 7.3 Rust/npm Dependencies

**Automation:** Dependabot is enabled via GitHub UI on the dev repo (no `.github/dependabot.yml` checked into the tree). It raises weekly PRs for `cargo` (rust-wallet, adblock-engine) and `npm` (frontend), plus GitHub Actions and the Azure trusted-signing-action.

**Review PRs weekly.** Don't auto-merge — review changelog for breaking changes.

---

## 8. Developer Certificate Setup

### 8.1 CI/CD (Recommended)

Certificates stored as GitHub Secrets. Workflow decodes and uses them. No manual handling.

### 8.2 Local Signing (Windows)

```powershell
# Set environment variables
$env:CERT_PATH = "C:\certs\hodos-codesign.pfx"
$env:CERT_PASSWORD = "your-password"

# Run build script with signing
.\scripts\build-installer.ps1 -Version "1.0.0" -Sign
```

**Sharing with team:** OV certificate (.pfx) can be shared securely. No device limits.

### 8.3 Local Signing (macOS)

```bash
# Import certificate to Keychain
security import cert.p12 -k ~/Library/Keychains/login.keychain-db -P "password"

# Verify identity available
security find-identity -v -p codesigning
```

**Sharing with team:** Export .p12 from Keychain, share securely. Or add devs to Apple Developer team.

---

## 9. Beta Testing Checklist

### 9.1 Installation

- [ ] Fresh install on clean Windows machine (no prior Hodos data)
- [ ] Fresh install on clean macOS machine
- [ ] Installer creates correct directory structure (`%LOCALAPPDATA%\HodosBrowser\` / `/Applications/Hodos.app`)
- [ ] Start Menu shortcut created (Windows) / appears in Applications (macOS)
- [ ] Desktop shortcut offered during install (optional)
- [ ] App icon displays correctly (not default/generic Windows shell icon)
- [ ] App launches successfully after install without errors
- [ ] SmartScreen behavior documented (warning text if OV cert, no warning if Azure Trusted Signing)
- [ ] macOS Gatekeeper: no "unidentified developer" warning (notarization verified)
- [ ] Install over existing version — user data preserved, app updates cleanly

### 9.2 Uninstallation

- [ ] Uninstaller accessible from Add/Remove Programs (Windows) / drag to Trash (macOS)
- [ ] All application files removed from install directory
- [ ] User data handling: prompt "Keep user data?" or document expected behavior
- [ ] No orphaned registry entries (Windows)
- [ ] No orphaned processes after uninstall (check Task Manager / Activity Monitor)
- [ ] Start Menu / Desktop shortcuts removed
- [ ] Reinstall after uninstall works cleanly

### 9.3 Process Lifecycle & Shutdown

- [ ] Close browser window — main process exits within 2-3 seconds
- [ ] Close browser — Rust wallet backend (`hodos-wallet.exe`) stops cleanly
- [ ] Close browser — adblock engine (`hodos-adblock.exe`) stops cleanly
- [ ] No orphaned processes visible in Task Manager / Activity Monitor after close
- [ ] Close while media is playing — audio stops promptly (known: may take 1-2 seconds currently)
- [ ] Close during active download — download cancelled, temp files cleaned up
- [ ] Force-kill main process — background processes also terminate (no zombies)
- [ ] System tray: verify no leftover tray icon after close (if applicable)

### 9.4 Auto-Update Flow

- [ ] Silent update: release new version, verify auto-download in background
- [ ] Update applies on browser close/restart — new version running after relaunch
- [ ] Update preserves user data (wallet, bookmarks, history, settings, profiles)
- [ ] Update preserves domain permissions and wallet unlock state
- [ ] Downgrade scenario: what happens if user manually installs older version?
- [ ] Update while browser is in use — no disruption until close
- [ ] Verify update signature validation (tampered installer rejected)
- [ ] "Check for updates now" button works in Settings

### 9.5 Multi-Instance & Profile Behavior

- [ ] Launch Profile A, then launch Profile B — both run independently
- [ ] Close Profile A while Profile B running — wallet backend stays alive for B
- [ ] Verify cookie/session isolation between profiles
- [ ] Log into site X on Profile A — not logged in on Profile B
- [ ] Stress test: active browsing on both profiles simultaneously
- [ ] Two profiles accessing wallet simultaneously — no database lock errors

### 9.6 Code Signing Verification

- [ ] Windows: Right-click .exe → Properties → Digital Signatures → "Marston Enterprises"
- [ ] Windows: All .exe and .dll files in install directory are signed (not just installer)
- [ ] Windows: Cert chain check via `Get-AuthenticodeSignature` (see §2.5.1)
- [ ] macOS: `codesign -dv --verbose=4 Hodos.app` shows valid signature
- [ ] macOS: `spctl --assess --type exec Hodos.app` returns "accepted"
- [ ] macOS: `stapler validate Hodos.app` confirms notarization ticket stapled

### 9.7 First-Run Experience

- [ ] First launch shows wallet setup prompt (no crash, no blank screen)
- [ ] Default homepage / new tab page loads correctly
- [ ] Browser is responsive within 3 seconds of launch
- [ ] No white flash during page load (dark background renders immediately)
- [ ] Default settings are sensible (adblock on, privacy protections on, etc.)

### 9.8 Platform-Specific

**Windows:**
- [ ] Works on Windows 10 (minimum supported version)
- [ ] Works on Windows 11
- [ ] High DPI / 4K display: UI scales correctly, no blurry text
- [ ] Windows Defender: no false-positive malware warnings (submit to Microsoft pre-release per §2.5)

**macOS:**
- [ ] Works on macOS 10.15 Catalina (minimum supported version)
- [ ] Works on latest macOS
- [ ] Apple Silicon (M1/M2/M3) — native arm64 or Rosetta 2 runs without issues
- [ ] Retina display: UI renders at correct resolution

### 9.9 Known Issues to Track

- [ ] App icon: practice installer build shows default Windows shell icon — need custom icon in Inno Setup config
- [ ] Audio delay on close: closing while video plays, audio continues for 1-2 seconds (low priority, CEF subprocess shutdown timing)

---

## 10. Post-Release Metrics

After publishing a release (Step 7), check these dashboards. The marketing-owned tracking doc lives at `Hodos/marketing/metrics/DOWNLOAD_METRICS.md` — add a new row to the running log there after each release.

### Quick check URLs

| Source | What it shows | URL |
|---|---|---|
| GitHub Releases UI | Per-asset download counts (source of truth for installs) | https://github.com/Hodos-Browser/Hodos-Browser/releases |
| Cloudflare Web Analytics | Landing page visits, countries, referrers | https://dash.cloudflare.com → Web Analytics → hodosbrowser.com |
| Cloudflare Pages Metrics | Aggregate request count (no per-path on Free plan; per-URL Traffic analytics requires CF Pro) | https://dash.cloudflare.com → Workers & Pages → hodosbrowser-com → Metrics |
| Cloudflare Email Routing | Inbound volume to `contact@`, `support@`, `hello@` | https://dash.cloudflare.com → Email |
| VirusTotal | Detection rate on latest release | Report URL from Step 8 |

### Pull GitHub download counts

```bash
gh api repos/Hodos-Browser/Hodos-Browser/releases \
  --jq '.[] | {tag: .tag_name, assets: [.assets[] | {name, downloads: .download_count}]}'
```

### Interpret the numbers

- First ~dozen downloads per release are typically: Matt's test installs, VirusTotal, AV scanners, and auto-update fetches from prior-version installs. See the marketing doc for the subtraction formula.
- `/appcast.xml` 24h hit count ≈ active install base (every running Hodos checks daily).
- Cloudflare Pages Metrics gives aggregate request counts only on the Free plan — not per-path. Redirect hits (`/download/*`) and appcast polls are NOT separately visible in CF Web Analytics (JS beacon doesn't fire on 302s or XML). Installer download counts (the real signal) live on GitHub Releases — pull via the `gh api` command above. Per-URL CF analytics unlocks on CF Pro ($20/mo).

---

## Appendix A: File Checklist

```
Hodos-Browser/
├── installer/
│   ├── hodos-browser.iss            # Inno Setup script
│   └── assets/
│       └── hodos.ico                # Installer icon
├── macos/
│   ├── Info.plist                   # App metadata
│   ├── entitlements.plist           # Code signing entitlements
│   └── hodos.icns                   # macOS app icon
├── scripts/
│   ├── build-installer.ps1          # Windows build
│   ├── sign-macos.sh                # macOS signing
│   ├── create-dmg.sh                # DMG creation
│   ├── notarize-macos.sh            # Apple notarization
│   └── generate-appcast.py          # Appcast generator
├── .github/
│   └── workflows/
│       └── release.yml              # Release builds
└── external/
    └── winsparkle/                  # WinSparkle (gitignored, downloaded by CI)
```

---

## Appendix B: Quick Commands

```powershell
# Build Windows installer (local)
.\scripts\build-installer.ps1 -Version "0.3.0-beta.7" -Sign

# Create release (after Step 1 version bump + commit)
git tag v0.3.0-beta.7
git push release v0.3.0-beta.7

# Sign manually
signtool sign /f cert.pfx /p "password" /tr http://timestamp.sectigo.com /td sha256 /fd sha256 "file.exe"

# Verify signature + cert chain (PowerShell native — no signtool on PATH needed)
$sig = Get-AuthenticodeSignature "HodosBrowser-X.Y.Z-beta.N-setup.exe"
$chain = New-Object Security.Cryptography.X509Certificates.X509Chain
[void]$chain.Build($sig.SignerCertificate)
$chain.ChainElements | ForEach-Object { Write-Host "  -> $($_.Certificate.Subject)" }
```

```bash
# Build macOS DMG (local)
./scripts/sign-macos.sh
./scripts/create-dmg.sh
./scripts/notarize-macos.sh

# Verify signature
codesign -dv --verbose=4 Hodos.app
spctl --assess --type exec Hodos.app
```

---

## Auto-Update System — implementation status (added 2026-03-30)

> Captures the actual implemented state, separate from the §4 design doc above.

### Architecture

| Platform | Library | Mechanism |
|----------|---------|-----------|
| Windows | WinSparkle 0.8.1 | Checks appcast.xml, shows native update dialog, downloads installer, requests app shutdown |
| macOS | Sparkle 2 | Same pattern — fixed in beta.6 (EdDSA signing path corrected) |

### How It Works

1. On startup, `AutoUpdater::Initialize()` configures WinSparkle with version, appcast URL, and DSA public key
2. If auto-check enabled (default: yes), WinSparkle checks `https://hodosbrowser.com/appcast.xml` every 24h
3. If a newer version exists in the appcast, WinSparkle shows a native "Update Available" dialog
4. User clicks "Install Update" → WinSparkle downloads the installer → requests app shutdown via callback
5. Callback posts `WM_CLOSE` to main window → graceful `ShutdownApplication()` runs → installer launches

### Key Files

| File | Purpose |
|------|---------|
| `cef-native/include/core/AutoUpdater.h` | Cross-platform singleton API |
| `cef-native/src/core/AutoUpdater.cpp` | Windows implementation (WinSparkle) |
| `cef-native/src/core/AutoUpdater_mac.mm` | macOS implementation (Sparkle 2) |
| `scripts/generate-appcast.py` | Generates appcast.xml from release artifacts |
| `external/winsparkle/WinSparkle-0.8.1/` | WinSparkle library (gitignored, downloaded by CI) |
| `external/keys/dsa_priv.pem` | DSA private key for signing (gitignored → GitHub Secret) |

### Appcast Format

```xml
<?xml version="1.0" encoding="utf-8"?>
<rss version="2.0" xmlns:sparkle="http://www.andymatuschak.org/xml-namespaces/sparkle">
  <channel>
    <title>Hodos Browser Updates</title>
    <item>
      <title>Version 0.3.0-beta.7</title>
      <sparkle:version>0.3.0-beta.7</sparkle:version>
      <sparkle:os>windows</sparkle:os>
      <enclosure url="https://github.com/Hodos-Browser/Hodos-Browser/releases/download/v0.3.0-beta.7/HodosBrowser-0.3.0-beta.7-setup.exe"
                 sparkle:dsaSignature="..." length="110587768" type="application/octet-stream"/>
    </item>
  </channel>
</rss>
```

### GitHub Secrets Needed

| Secret | Purpose | Status |
|--------|---------|--------|
| `WINSPARKLE_DSA_PRIVATE_KEY` | Sign Windows installers for WinSparkle verification | DONE — verified end-to-end on beta.4 → beta.5 |
| `SPARKLE_EDDSA_PRIVATE_KEY` | Sign macOS DMGs for Sparkle 2 verification | DONE — fix landed in beta.6 (correct `sign_update` path) |

### Settings

Users can toggle auto-update in **Settings → About → "Check for updates automatically"**. Manual check via "Check for updates" button. Setting stored in `settings.json` as `browser.autoUpdateEnabled`.

### Release Workflow Changes

CI (`release.yml`) now:
1. Downloads WinSparkle 0.8.1 zip before CMake configure
2. Generates `appcast.xml` as a release artifact (alongside installer, portable zip, DMG)
3. Signs installer with DSA private key (Windows) / DMG with EdDSA key (macOS)
4. Pushes appcast.xml as release artifact (manual copy to website repo per Step 5)
