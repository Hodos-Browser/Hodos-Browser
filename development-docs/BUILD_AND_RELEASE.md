# Hodos Browser — Build & Release Guide

**Created:** 2026-03-20
**Last Updated:** 2026-03-24
**Purpose:** How to build installers, sign code, ship updates, and manage releases

---

## Current Status (2026-03-24)

**First beta release shipped: `v0.1.0-beta.1`**

| Component | Status |
|-----------|--------|
| Windows installer (Inno Setup) | WORKING — signed with Azure Artifact Signing |
| Windows portable zip | WORKING |
| macOS DMG | WORKING — signed, notarization pending (Apple slow) |
| GitHub Actions CI/CD | WORKING — tag-triggered, builds both platforms |
| Website (hodosbrowser.com) | LIVE — download links active |
| Auto-update (WinSparkle/Sparkle) | NOT STARTED — next priority |

### How to Release a New Version

```bash
# 1. Commit and push changes
git push origin main
git push release main

# 2. Tag and push (triggers CI build)
git tag v0.1.0-beta.2
git push release v0.1.0-beta.2

# 3. Wait ~35 min, then go to GitHub Releases
# 4. Review draft release, click Publish
# 5. Update website download links if needed
```

---

## Quick Reference

| Item | Value |
|------|-------|
| Version format | MAJOR.MINOR.PATCH-prerelease (semver) |
| Git tag format | `v0.1.0-beta.1`, `v1.0.0` |
| GitHub org | `Hodos-Browser` |
| Main repo | `Hodos-Browser/Hodos-Browser` (public) |
| Website repo | `Hodos-Browser/hodosbrowser.com` |
| Website URL | `https://hodosbrowser.com` |
| Appcast URL | `https://hodosbrowser.com/appcast.xml` (not yet implemented) |

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

Configuration:
- Account: `Hodos-Signing` (West Central US)
- Profile: `Hodos-signing`
- Endpoint: `https://wcus.codesigning.azure.net/`
- Certificate: `CN=Marston Enterprises, O=Marston Enterprises, L=Peyton, S=Colorado, C=US`

### 1.3 GitHub Infrastructure

**Organization:** `hodos-browser`

| Repository | Purpose |
|------------|---------|
| `hodos-browser/hodos` | Main browser source code |
| `hodos-browser/hodosbrowser.com` | Website (GitHub Pages or Cloudflare Pages) |

**Setup steps:**
1. Create org at github.com/organizations/new
2. Free plan (sufficient for public repos)
3. Enable 2FA requirement
4. Add verified domain after DNS setup

**Migration approach:** Work on current local repo, push to GitHub org when ready for MVP release.

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

Full script at: `installer/HodosSetup.iss`

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

### 2.5 Code Signing

> **Important:** Sign ALL binaries, not just the installer. Unsigned DLLs inside a signed installer still trigger antivirus and SmartScreen warnings.

**Files that must be signed:**
- `HodosBrowser.exe` (main browser)
- `hodos-wallet.exe` (Rust wallet)
- `hodos-adblock.exe` (adblock engine)
- `libcef.dll` and other CEF DLLs
- CEF subprocess executable
- `HodosSetup-x.x.x.exe` (the installer itself)

**Sign command:**
```powershell
signtool sign /f "hodos-codesign.pfx" /p "$PASSWORD" `
  /tr http://timestamp.sectigo.com /td sha256 /fd sha256 `
  "HodosSetup-1.0.0.exe"
```

**Verify signature:**
```powershell
signtool verify /pa "HodosSetup-1.0.0.exe"
```

**In CI/CD:** Store certificate as base64-encoded GitHub Secret, decode during workflow.

**Pre-release:** Submit signed binaries to Microsoft's malware analysis portal before first public release. Chromium-based executables often trigger heuristic antivirus detections. Pre-submitting seeds reputation.

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
& "C:\Program Files (x86)\Inno Setup 6\ISCC.exe" installer/HodosSetup.iss

# 4. Sign if requested
if ($Sign) {
    signtool sign /f $env:CERT_PATH /p $env:CERT_PASSWORD `
      /tr http://timestamp.sectigo.com /td sha256 /fd sha256 `
      "dist\HodosSetup-$Version.exe"
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

## 4. Auto-Update System

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

***Needs a decision***

| Option | Platforms | Delta Updates | Integration | Notes |
|--------|-----------|---------------|-------------|-------|
| **WinSparkle + Sparkle** | Win (WinSparkle) + Mac (Sparkle 2.x) | Mac only | C++ (WinSparkle), Obj-C (Sparkle) | Two separate libraries, proven and mature |
| **Velopack** | Win + Mac + Linux | Yes (built-in) | **Has Rust SDK** (`velopack` crate) | Single cross-platform library, modern successor to Squirrel.Windows, supports GitHub Releases as backend |

**WinSparkle/Sparkle** is the proven approach (documented below in §4.3-4.4). **Velopack** is newer but has first-class Rust support which fits our stack, does delta updates on both platforms (WinSparkle does not), and is one library instead of two. Worth evaluating.

### 4.2 User Settings

***Needs a decision:*** Where should update settings live? Settings → General, or Settings → Privacy & Security?

***Needs a decision:*** For "Notify me" mode, should the notification be a popup dialog or a small toolbar badge?

```
┌─────────────────────────────────────────────────────────┐
│ Updates                                                  │
├─────────────────────────────────────────────────────────┤
│ ● Update automatically (recommended)                    │
│   Updates install silently when you close the browser   │
│                                                         │
│ ○ Notify me when updates are available                  │
│   [Popup / Small badge] ← needs decision                │
│                                                         │
│ Current version: 1.2.3                                  │
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
        url="https://github.com/hodos-browser/hodos/releases/download/v1.1.0/HodosSetup-1.1.0.exe"
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
        url="https://github.com/hodos-browser/hodos/releases/download/v1.1.0/Hodos-1.1.0.dmg"
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
openssl dgst -sha1 -binary "HodosSetup-1.1.0.exe" | \
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
        run: iscc installer/HodosSetup.iss
      
      - name: Sign
        env:
          CERT_BASE64: ${{ secrets.WINDOWS_CERT_BASE64 }}
          CERT_PASSWORD: ${{ secrets.WINDOWS_CERT_PASSWORD }}
        run: |
          [IO.File]::WriteAllBytes("cert.pfx", [Convert]::FromBase64String($env:CERT_BASE64))
          signtool sign /f cert.pfx /p $env:CERT_PASSWORD `
            /tr http://timestamp.sectigo.com /td sha256 /fd sha256 `
            dist\HodosSetup-*.exe
          Remove-Item cert.pfx
      
      - uses: actions/upload-artifact@v4
        with:
          name: windows-installer
          path: dist/HodosSetup-*.exe

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

**If using OV/EV certificate (Sectigo):**

| Secret | Purpose | How to Get |
|--------|---------|------------|
| `WINDOWS_CERT_BASE64` | OV cert (base64 PFX) | `base64 -i cert.pfx` |
| `WINDOWS_CERT_PASSWORD` | PFX password | From Sectigo |

**If using Azure Trusted Signing:**

| Secret | Purpose | How to Get |
|--------|---------|------------|
| `AZURE_TENANT_ID` | Azure AD tenant | Azure Portal |
| `AZURE_CLIENT_ID` | Service principal | Azure Portal |
| `AZURE_CLIENT_SECRET` | Service principal secret | Azure Portal |

**macOS (required regardless of Windows choice):**

| Secret | Purpose | How to Get |
|--------|---------|------------|
| `MACOS_CERT_BASE64` | Apple cert (base64 P12) | Export from Keychain |
| `MACOS_CERT_PASSWORD` | P12 password | Set when exporting |
| `APPLE_TEAM_ID` | 10-char team ID | Developer portal |
| `APPLE_API_KEY_ID` | App Store Connect API key ID | App Store Connect → Users → Keys |
| `APPLE_API_ISSUER` | API key issuer UUID | App Store Connect → Users → Keys |
| `APPLE_API_KEY_P8_BASE64` | API key .p8 file (base64) | Download once from App Store Connect |

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

### 6.2 Single Source of Truth

Version lives in `rust-wallet/Cargo.toml`. Derive everywhere else:

```powershell
# scripts/get-version.ps1
$cargo = Get-Content rust-wallet/Cargo.toml | Select-String 'version = "(.+)"'
$version = $cargo.Matches.Groups[1].Value
```

Update frontend `package.json`, Inno Setup script, C++ resources from this source.

### 6.3 Release Process

```bash
# 1. Update version in Cargo.toml
# 2. Update CHANGELOG.md
# 3. Commit
git commit -am "Release v1.1.0"

# 4. Tag
git tag -a v1.1.0 -m "Release 1.1.0"

# 5. Push (triggers release workflow)
git push origin main --tags
```

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

**Current (MVP):** Lists bundled at build time

**Post-MVP:** Background task fetches updated lists every 6 hours
- EasyList, EasyPrivacy: `! Expires: 4 days` header
- Host on CDN or use upstream URLs directly

### 7.3 Rust/npm Dependencies

**Automation:** Enable Dependabot in GitHub repo settings

```yaml
# .github/dependabot.yml
version: 2
updates:
  - package-ecosystem: "cargo"
    directory: "/rust-wallet"
    schedule:
      interval: "weekly"
  
  - package-ecosystem: "cargo"
    directory: "/adblock-engine"
    schedule:
      interval: "weekly"
  
  - package-ecosystem: "npm"
    directory: "/frontend"
    schedule:
      interval: "weekly"
```

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
- [ ] Windows Defender: no false-positive malware warnings (submit to Microsoft pre-release)

**macOS:**
- [ ] Works on macOS 10.15 Catalina (minimum supported version)
- [ ] Works on latest macOS
- [ ] Apple Silicon (M1/M2/M3) — native arm64 or Rosetta 2 runs without issues
- [ ] Retina display: UI renders at correct resolution

### 9.9 Known Issues to Track

- [ ] App icon: practice installer build shows default Windows shell icon — need custom icon in Inno Setup config
- [ ] Audio delay on close: closing while video plays, audio continues for 1-2 seconds (low priority, CEF subprocess shutdown timing)

---

## Appendix A: File Checklist

```
Hodos-Browser/
├── installer/
│   ├── HodosSetup.iss              # Inno Setup script
│   └── assets/
│       └── hodos.ico               # Installer icon
├── macos/
│   ├── Info.plist                  # App metadata
│   ├── entitlements.plist          # Code signing entitlements
│   └── hodos.icns                  # macOS app icon
├── scripts/
│   ├── build-installer.ps1         # Windows build
│   ├── sign-macos.sh               # macOS signing
│   ├── create-dmg.sh               # DMG creation
│   ├── notarize-macos.sh           # Apple notarization
│   └── generate-appcast.py         # Appcast generator
├── .github/
│   ├── workflows/
│   │   ├── ci.yml                  # PR checks
│   │   └── release.yml             # Release builds
│   └── dependabot.yml              # Dependency updates
└── external/
    └── winsparkle/                 # WinSparkle (submodule or copy)
```

---

## Appendix B: Quick Commands

```powershell
# Build Windows installer (local)
.\scripts\build-installer.ps1 -Version "1.0.0" -Sign

# Create release
git tag -a v1.0.0 -m "Release 1.0.0"
git push origin main --tags

# Sign manually
signtool sign /f cert.pfx /p "password" /tr http://timestamp.sectigo.com /td sha256 /fd sha256 "file.exe"

# Verify signature
signtool verify /pa "file.exe"
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
