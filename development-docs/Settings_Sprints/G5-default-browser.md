# G5: Set as Default Browser

**Status**: Not Started
**Complexity**: Low
**Estimated Phases**: 1

---

## Current State

- No "Set as default browser" option exists anywhere in the UI
- Users must manually navigate to OS settings to change their default browser

---

## What Needs to Happen

### Phase 1: Default Browser Button

**Goal**: Add a button in Settings > General that opens the OS default apps settings so users can easily set Hodos as their default browser.

**Changes needed**:

**Frontend — GeneralSettings.tsx**:
- [ ] Add "Default Browser" SettingsCard at the top of General settings
- [ ] Show current status if detectable (optional — hard to check reliably)
- [ ] "Make Hodos your default browser" button
- [ ] Button sends IPC: `open_default_browser_settings`

**C++ — IPC handler**:
- [ ] Add `open_default_browser_settings` handler in `simple_handler.cpp`
- [ ] Windows: `ShellExecute(NULL, "open", "ms-settings:defaultapps", NULL, NULL, SW_SHOWNORMAL)`
- [ ] macOS: Open System Settings > Default Browser (or use `LSSetDefaultHandlerForURLScheme` API)

**Design decisions**:
- Should we try to detect if Hodos is already the default? (Nice but complex — Windows uses `IApplicationAssociationRegistration::QueryCurrentDefault`)
- Just opening the OS settings page is the simplest and most reliable approach — Chrome and Firefox both do this
- Button text: "Make Hodos your default browser" (Chrome-style) or "Set as default" (shorter)

---

## Platform Details

### Windows
```cpp
// Opens Windows Settings > Default Apps
ShellExecuteW(NULL, L"open", L"ms-settings:defaultapps", NULL, NULL, SW_SHOWNORMAL);
```

For Hodos to appear in the default apps list, it needs to be registered:
- [ ] Register URL protocol handlers (http, https) during installation
- [ ] Register file associations (.html, .htm, .svg, etc.)
- [ ] Add to `HKEY_LOCAL_MACHINE\SOFTWARE\RegisteredApplications`
- [ ] Add capabilities under `HKEY_LOCAL_MACHINE\SOFTWARE\HodosBrowser\Capabilities`

**Note**: Registration is an installer task, not a settings task. This sprint just opens the settings page. Registration would be part of the installer/packaging sprint.

### macOS
**Deferred** — Windows-only for now. When implementing macOS support, add equivalent logic and update `development-docs/macos-port/MAC_PLATFORM_SUPPORT_PLAN.md` with details.

---

## Test Checklist

- [ ] Click "Make Hodos your default browser" → Windows Settings > Default Apps opens
- [ ] Verify Hodos appears in the browser list (requires protocol registration)
- [ ] macOS: equivalent settings page opens
- [ ] Button is visible and accessible in General settings

---

**Last Updated**: 2026-02-28
