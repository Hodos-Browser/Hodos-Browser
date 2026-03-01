# G5: Set as Default Browser — Detailed Implementation Plan

**Status**: Not Started  
**Complexity**: Low  
**Estimated Time**: 1-2 hours  
**Dependencies**: None (Installer/packaging work is separate)

---

## Executive Summary

Add a button in Settings > General that opens the Windows Default Apps settings page, allowing users to easily set Hodos as their default browser. This sprint focuses on the settings UI — protocol registration is an installer task.

---

## Current State Analysis

### What Exists
- Nothing — no "Set as default browser" option exists

### What's Missing
- UI button in General settings
- IPC handler to open OS settings

---

## Phase 1: Default Browser Button (1 hour)

### Step 1: Add UI to GeneralSettings

**File**: `frontend/src/components/settings/GeneralSettings.tsx`

Add a new SettingsCard at the top:

```tsx
import { Button } from '@mui/material';

// Inside GeneralSettings component, at the top:
<SettingsCard title="Default Browser">
  <SettingRow
    label="Make Hodos your default browser"
    description="Open Windows settings to set Hodos as your default browser"
    control={
      <Button
        variant="contained"
        size="small"
        onClick={() => window.cefMessage?.send('open_default_browser_settings', [])}
        sx={{
          backgroundColor: '#a67c00',
          '&:hover': { backgroundColor: '#c99a00' },
          textTransform: 'none',
        }}
      >
        Set as Default
      </Button>
    }
  />
</SettingsCard>
```

### Step 2: Add IPC Handler (Windows)

**File**: `simple_handler.cpp` — in `OnProcessMessageReceived()`

```cpp
} else if (message_name == "open_default_browser_settings") {
    CEF_REQUIRE_UI_THREAD();
    
#ifdef _WIN32
    // Open Windows Settings > Default Apps
    ShellExecuteW(
        NULL,           // No parent window
        L"open",        // Operation
        L"ms-settings:defaultapps",  // Windows Settings URI
        NULL,           // No parameters
        NULL,           // Default directory
        SW_SHOWNORMAL   // Show normally
    );
    LOG_INFO("Opened Windows Default Apps settings");
#elif defined(__APPLE__)
    // macOS: Would use LSSetDefaultHandlerForURLScheme or open System Preferences
    // Deferred — macOS implementation in future sprint
    LOG_INFO("macOS default browser setting not yet implemented");
#endif
```

**Required include**:
```cpp
#ifdef _WIN32
#include <shellapi.h>
#endif
```

---

## Protocol Registration (Installer Task — Not This Sprint)

For Hodos to appear in the Windows Default Apps list, it needs proper protocol registration. This is typically done by the installer (NSIS, WiX, or similar).

### Required Registry Entries

```
HKEY_LOCAL_MACHINE\SOFTWARE\RegisteredApplications
    "HodosBrowser" = "SOFTWARE\HodosBrowser\Capabilities"

HKEY_LOCAL_MACHINE\SOFTWARE\HodosBrowser
    \Capabilities
        "ApplicationDescription" = "Hodos Browser - A privacy-focused browser with native BSV wallet"
        "ApplicationName" = "Hodos Browser"
        \URLAssociations
            "http" = "HodosBrowserURL"
            "https" = "HodosBrowserURL"
        \FileAssociations
            ".htm" = "HodosBrowserHTML"
            ".html" = "HodosBrowserHTML"
            ".svg" = "HodosBrowserHTML"

HKEY_LOCAL_MACHINE\SOFTWARE\Classes\HodosBrowserURL
    "(Default)" = "Hodos Browser URL"
    "URL Protocol" = ""
    \DefaultIcon
        "(Default)" = "C:\Program Files\HodosBrowser\hodos.exe,0"
    \shell\open\command
        "(Default)" = "\"C:\Program Files\HodosBrowser\hodos.exe\" \"%1\""

HKEY_LOCAL_MACHINE\SOFTWARE\Classes\HodosBrowserHTML
    "(Default)" = "Hodos Browser HTML Document"
    \DefaultIcon
        "(Default)" = "C:\Program Files\HodosBrowser\hodos.exe,0"
    \shell\open\command
        "(Default)" = "\"C:\Program Files\HodosBrowser\hodos.exe\" \"%1\""
```

### NSIS Example (for future installer)

```nsis
; Register as browser
WriteRegStr HKLM "SOFTWARE\RegisteredApplications" "HodosBrowser" "SOFTWARE\HodosBrowser\Capabilities"

WriteRegStr HKLM "SOFTWARE\HodosBrowser\Capabilities" "ApplicationDescription" "Hodos Browser"
WriteRegStr HKLM "SOFTWARE\HodosBrowser\Capabilities" "ApplicationName" "Hodos Browser"
WriteRegStr HKLM "SOFTWARE\HodosBrowser\Capabilities\URLAssociations" "http" "HodosBrowserURL"
WriteRegStr HKLM "SOFTWARE\HodosBrowser\Capabilities\URLAssociations" "https" "HodosBrowserURL"

; URL handler
WriteRegStr HKLM "SOFTWARE\Classes\HodosBrowserURL" "" "Hodos Browser URL"
WriteRegStr HKLM "SOFTWARE\Classes\HodosBrowserURL" "URL Protocol" ""
WriteRegStr HKLM "SOFTWARE\Classes\HodosBrowserURL\shell\open\command" "" '"$INSTDIR\hodos.exe" "%1"'
```

---

## Optional: Detect Default Status

Detecting whether Hodos is already the default browser is complex on Windows and may not be worth the effort for MVP.

### Windows API Approach (Complex)

```cpp
#include <shobjidl.h>

bool IsHodosDefaultBrowser() {
    IApplicationAssociationRegistration* pAAR = nullptr;
    HRESULT hr = CoCreateInstance(
        CLSID_ApplicationAssociationRegistration,
        NULL,
        CLSCTX_INPROC,
        __uuidof(IApplicationAssociationRegistration),
        (void**)&pAAR
    );
    
    if (SUCCEEDED(hr)) {
        LPWSTR pszAppId = nullptr;
        hr = pAAR->QueryCurrentDefault(L"http", AT_URLPROTOCOL, AL_EFFECTIVE, &pszAppId);
        
        if (SUCCEEDED(hr) && pszAppId) {
            bool isDefault = (wcscmp(pszAppId, L"HodosBrowserURL") == 0);
            CoTaskMemFree(pszAppId);
            pAAR->Release();
            return isDefault;
        }
        
        if (pAAR) pAAR->Release();
    }
    
    return false;
}
```

**Decision**: Skip detection for MVP. Just show the button unconditionally.

---

## Test Checklist

### UI
- [ ] "Default Browser" card appears in General settings
- [ ] "Set as Default" button is visible and clickable

### Functionality
- [ ] Click button → Windows Settings > Default Apps opens
- [ ] (After installer work) Hodos appears in the browser list
- [ ] (After installer work) Can select Hodos as default

### Edge Cases
- [ ] Button works on fresh Windows install
- [ ] Button works without admin rights

---

## Files to Modify

| File | Changes |
|------|---------|
| `frontend/src/components/settings/GeneralSettings.tsx` | Add Default Browser card with button |
| `src/handlers/simple_handler.cpp` | Add `open_default_browser_settings` IPC handler |

---

## Future Work (Not This Sprint)

1. **Installer script**: Create NSIS/WiX installer with protocol registration
2. **macOS support**: Implement equivalent for System Preferences
3. **Detection**: Optionally detect and show "Hodos is your default browser" vs "Set as Default"

---

**Last Updated**: 2026-02-28
