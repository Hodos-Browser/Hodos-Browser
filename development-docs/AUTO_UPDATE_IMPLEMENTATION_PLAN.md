# Hodos Browser — Auto-Update Implementation Plan

**Created:** 2026-03-24
**Status:** Planning — implement after beta.1 testing
**Estimated Effort:** 7 days

---

## Recommendation: WinSparkle (Windows) + Sparkle 2 (macOS)

**Why not Velopack?**
- Replaces our working Inno Setup installer — high risk
- Architectural mismatch — Rust SDK would run in the wallet process, but updates need to be managed by the C++ shell
- Younger project, smaller community
- Can switch later if needed — the `AutoUpdater` singleton abstracts the library

**Why WinSparkle + Sparkle?**
- Battle-tested (WinSparkle since 2012, Sparkle since 2006)
- Works with existing Inno Setup installer
- Integrates directly into C++ layer where it belongs
- Both support silent background checking
- Appcast XML is a simple standard format

---

## How It Works

### Silent Update Flow (Chrome Model)
```
Browser running (background, every 24h)
    |
Fetch appcast.xml from hodosbrowser.com
    |
Compare version to installed
    |
If newer: download installer silently to staging folder
    |
User closes browser naturally
    |
ShutdownApplication() detects staged update
    |
Launch Inno Setup with /SILENT /UPDATE flags
    |
Installer replaces files, relaunches browser
    |
Next launch: new version running (no interruption)
```

**Note:** WinSparkle MVP will show a small "Update available" dialog rather than fully silent. Fully silent requires custom download logic — iterate after MVP.

---

## Implementation Phases

### Phase 1: Infrastructure (Day 1-2)

**Download libraries:**
1. WinSparkle release → `external/winsparkle/` (include/, Release/WinSparkle.dll + .lib)
2. Sparkle 2 release → `external/Sparkle.framework/`

**Generate signing keys:**

Windows DSA (one-time):
```bash
openssl dsaparam -genkey 2048 -out dsa_priv.pem
openssl dsa -in dsa_priv.pem -pubout -out dsa_pub.pem
```
- Private key → GitHub Secret `WINSPARKLE_DSA_PRIVATE_KEY`
- Public key → embed in `AutoUpdater.cpp`

macOS EdDSA (one-time):
```bash
./Sparkle.framework/Resources/generate_keys
```
- Private key → GitHub Secret `SPARKLE_EDDSA_PRIVATE_KEY`
- Public key → `Info.plist` as `SUPublicEDKey`

**Create AutoUpdater singleton:**

`cef-native/include/core/AutoUpdater.h`:
```cpp
class AutoUpdater {
public:
    static AutoUpdater& GetInstance();
    void Initialize(const std::string& version, const std::string& appcastUrl, bool autoUpdate);
    void CheckForUpdatesInteractively();  // Manual "Check for updates" button
    void CheckForUpdatesInBackground();
    bool HasStagedUpdate();
    void ApplyStagedUpdateAndRelaunch();
    void SetAutoUpdateEnabled(bool enabled);
    void Shutdown();
};
```

### Phase 2: Windows Integration (Day 2-3)

**`AutoUpdater.cpp`** — wraps WinSparkle:
- `Initialize()`: `win_sparkle_set_app_details()`, `win_sparkle_set_appcast_url()`, `win_sparkle_init()`
- `CheckForUpdatesInteractively()`: `win_sparkle_check_update_with_ui()`
- Register callbacks: `win_sparkle_set_did_find_update_callback`, `win_sparkle_set_shutdown_request_callback`

**`cef_browser_shell.cpp`** changes:
- After window creation: `AutoUpdater::GetInstance().Initialize(version, appcastUrl, settings.autoUpdate)`
- In `ShutdownApplication()`: after wallet/adblock stop, before exit: `AutoUpdater::GetInstance().ApplyStagedUpdateAndRelaunch()`

**`CMakeLists.txt`** — add to Windows block:
```cmake
target_link_libraries(HodosBrowserShell PRIVATE "${WINSPARKLE_DIR}/Release/WinSparkle.lib")
```

**`build-release.ps1`** — copy `WinSparkle.dll` to staging

**`hodos-browser.iss`** — add `WinSparkle.dll` to `[Files]`, support `/SILENT /UPDATE` flags

**IMPORTANT:** Change `DefaultDirName` from `{autopf}\HodosBrowser` to `{localappdata}\HodosBrowser` for per-user install (no UAC needed for updates)

### Phase 3: macOS Integration (Day 3-4)

**`AutoUpdater_mac.mm`** — wraps Sparkle 2:
- Create `SPUUpdater` with `SPUStandardUserDriver`
- Configure via delegate callbacks

**`cef_browser_shell_mac.mm`** — same initialization/shutdown hooks

**`CMakeLists.txt`** macOS block — link and bundle `Sparkle.framework`

**`Info.plist`** — add `SUFeedURL` and `SUPublicEDKey`

**`release.yml`** macOS signing — sign Sparkle.framework before app bundle

### Phase 4: Frontend Settings UI (Day 4-5)

**IPC handlers in `simple_handler.cpp`:**
- `check_for_updates` → calls `AutoUpdater::CheckForUpdatesInteractively()`
- `update_settings_changed` → calls `AutoUpdater::SetAutoUpdateEnabled()`
- Send `update_status` IPC back to React

**`AboutSettings.tsx`** updates:
- Current version display
- "Check for updates" button
- Status text: "Up to date" / "Checking..." / "Update available (v1.2.0)" / "Downloading..."
- Auto-update toggle (radio: "Update automatically" / "Notify me")

**`SettingsManager.h`** — add:
```cpp
struct UpdateSettings {
    bool autoUpdateEnabled = true;
    int checkIntervalHours = 24;
};
```

### Phase 5: CI/CD and Appcast (Day 5-6)

**Create `scripts/generate-appcast.py`:**
- Takes version, platform, download URL, file size, signature as inputs
- Generates/updates `appcast.xml`

**Appcast XML format:**
```xml
<?xml version="1.0" encoding="utf-8"?>
<rss version="2.0" xmlns:sparkle="http://www.andymatuschak.org/xml-namespaces/sparkle">
  <channel>
    <title>Hodos Browser Updates</title>
    <item>
      <title>Version 0.1.0-beta.2</title>
      <sparkle:version>0.1.0-beta.2</sparkle:version>
      <sparkle:os>windows</sparkle:os>
      <enclosure
        url="https://github.com/Hodos-Browser/Hodos-Browser/releases/download/v0.1.0-beta.2/HodosBrowser-0.1.0-beta.2-setup.exe"
        sparkle:dsaSignature="SIGNATURE"
        length="109000000"
        type="application/octet-stream"/>
    </item>
    <item>
      <title>Version 0.1.0-beta.2</title>
      <sparkle:version>0.1.0-beta.2</sparkle:version>
      <sparkle:os>macos</sparkle:os>
      <enclosure
        url="https://github.com/Hodos-Browser/Hodos-Browser/releases/download/v0.1.0-beta.2/HodosBrowser-0.1.0-beta.2.dmg"
        sparkle:edSignature="SIGNATURE"
        length="173000000"
        type="application/octet-stream"/>
    </item>
  </channel>
</rss>
```

**Host at:** `https://hodosbrowser.com/appcast.xml`

**CI/CD changes to `release.yml`:**
- After signing, compute DSA signature of Windows installer
- After signing, compute EdDSA signature of macOS DMG
- Run `generate-appcast.py`
- Push `appcast.xml` to website repo

### Phase 6: Testing (Day 6-7)

1. Build v0.1.0-beta.2 with auto-update enabled
2. Install v0.1.0-beta.1 (old version)
3. Verify background check finds the update
4. Verify manual "Check for updates" works
5. Verify update downloads and stages correctly
6. Verify update applies on browser close
7. Verify browser relaunches with new version
8. Verify wallet data, bookmarks, history preserved
9. Test with auto-update disabled
10. Test with no network — graceful failure
11. Test on both Windows 10 and Windows 11
12. Test macOS (if notarization is working)

---

## Files To Create

| File | Purpose |
|------|---------|
| `cef-native/include/core/AutoUpdater.h` | Cross-platform singleton header |
| `cef-native/src/core/AutoUpdater.cpp` | Windows implementation (WinSparkle) |
| `cef-native/src/core/AutoUpdater_mac.mm` | macOS implementation (Sparkle 2) |
| `external/winsparkle/` | WinSparkle library (download) |
| `external/Sparkle.framework/` | Sparkle 2 framework (download) |
| `scripts/generate-appcast.py` | Appcast XML generator |

## Files To Modify

| File | Change |
|------|--------|
| `cef-native/CMakeLists.txt` | Link WinSparkle/Sparkle |
| `cef-native/cef_browser_shell.cpp` | Initialize AutoUpdater, apply on shutdown |
| `cef-native/cef_browser_shell_mac.mm` | Same for macOS |
| `cef-native/include/core/SettingsManager.h` | Add `UpdateSettings` |
| `cef-native/src/core/SettingsManager.cpp` | Serialize update settings |
| `cef-native/src/handlers/simple_handler.cpp` | IPC for check_for_updates |
| `frontend/src/components/settings/AboutSettings.tsx` | Update button + toggle |
| `installer/hodos-browser.iss` | Add WinSparkle.dll, /SILENT /UPDATE support, per-user install dir |
| `scripts/build-release.ps1` | Copy WinSparkle.dll to staging |
| `.github/workflows/release.yml` | Signing + appcast generation |

---

## Risks and Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| WinSparkle fully-silent mode incomplete | Users see dialog instead of silent update | Ship with dialog for MVP, iterate to silent later |
| Inno Setup files locked during update | Update fails | Launch installer AFTER ShutdownApplication() completes |
| Version comparison with pre-release tags | Wrong version detected as update | Test `0.1.0-beta.1` → `0.1.0-beta.2` comparison |
| Update fails mid-install | Broken installation | Inno Setup has transactional rollback; keep portable zip available |
| macOS Sparkle + CEF entitlements conflict | Crash on launch after update | Test entitlements carefully |
| WinSparkle.dll flagged by antivirus | Blocks install | Sign WinSparkle.dll with Azure cert |

---

## Key Decision: MVP Approach

For the MVP (next 2-3 days), do a **minimal integration**:

1. WinSparkle with its standard UI dialog (not fully silent)
2. Sparkle 2 with standard UI
3. Manual appcast.xml generation (not automated in CI yet)
4. "Check for updates" button in Settings → About

This gets auto-update working for beta testers in 2-3 days. Full silent mode and CI automation can follow.
