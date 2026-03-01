# D1: Download Settings — Detailed Implementation Plan

**Status**: Not Started  
**Complexity**: Low-Medium  
**Estimated Time**: 2-4 hours  
**Dependencies**: None

---

## Executive Summary

Wire the download folder setting to actual download behavior and add a folder picker button. Currently, downloads always show the Save As dialog with the system default location — the user's configured path is ignored.

---

## Current State Analysis

### What Exists
- **UI**: Text input in `DownloadSettings.tsx` for download folder path
- **Persistence**: `SettingsManager::SetDownloadsPath()` saves to `settings.json`
- **Backend**: `BrowserSettings.downloadsPath` field (default: empty string)
- **Download Handler**: `OnBeforeDownload()` in `simple_handler.cpp` — calls `callback->Continue("", true)` (always shows Save As)

### What's Missing
- `OnBeforeDownload()` doesn't read the settings
- No folder picker button (user must type path manually)
- No "Ask where to save" toggle
- No validation that the folder exists

---

## Phase 1: Wire Settings + Folder Picker (2 hours)

### Step 1: Update OnBeforeDownload to Use Settings

**File**: `src/handlers/simple_handler.cpp`

```cpp
bool SimpleHandler::OnBeforeDownload(CefRefPtr<CefBrowser> browser,
                                     CefRefPtr<CefDownloadItem> download_item,
                                     const CefString& suggested_name,
                                     CefRefPtr<CefBeforeDownloadCallback> callback) {
    CEF_REQUIRE_UI_THREAD();
    
    auto& settings = SettingsManager::GetInstance().GetBrowserSettings();
    std::string downloadsPath = settings.downloadsPath;
    bool askWhereToSave = true; // Phase 2: read from settings.askWhereToSave
    
    std::string fullPath;
    
    if (!downloadsPath.empty()) {
        // Validate folder exists
        if (std::filesystem::exists(downloadsPath) && 
            std::filesystem::is_directory(downloadsPath)) {
            // Construct full path: folder + filename
            fullPath = downloadsPath;
            // Ensure path ends with separator
            if (fullPath.back() != '/' && fullPath.back() != '\\') {
#ifdef _WIN32
                fullPath += "\\";
#else
                fullPath += "/";
#endif
            }
            fullPath += suggested_name.ToString();
        } else {
            // Folder doesn't exist — fall back to Save As dialog
            LOG_WARN_BROWSER("Download folder not found: " + downloadsPath);
            askWhereToSave = true; // Force dialog
        }
    }
    
    LOG_INFO_BROWSER("📥 OnBeforeDownload: " + suggested_name.ToString() +
                     " -> " + (fullPath.empty() ? "(Save As dialog)" : fullPath));
    
    // Continue with download
    // Empty fullPath = system default + Save As dialog
    // Non-empty fullPath with askWhereToSave=true = dialog starts in that folder
    // Non-empty fullPath with askWhereToSave=false = silent download to that path
    callback->Continue(fullPath, askWhereToSave);
    return true;
}
```

**Required include**:
```cpp
#include <filesystem>
#include "include/core/SettingsManager.h"
```

### Step 2: Add Folder Picker IPC Handler

**File**: `simple_handler.cpp` — in `OnProcessMessageReceived()`

```cpp
} else if (message_name == "download_browse_folder") {
    CEF_REQUIRE_UI_THREAD();
    
    // Get current path as starting point
    auto& settings = SettingsManager::GetInstance().GetBrowserSettings();
    std::string currentPath = settings.downloadsPath;
    if (currentPath.empty()) {
        // Default to user's Downloads folder
        char path[MAX_PATH];
        if (SUCCEEDED(SHGetFolderPathA(NULL, CSIDL_PERSONAL, NULL, 0, path))) {
            currentPath = path;
        }
    }
    
    // CEF folder dialog is async — use a callback
    class FolderDialogCallback : public CefRunFileDialogCallback {
    public:
        FolderDialogCallback(CefRefPtr<CefBrowser> browser) : browser_(browser) {}
        
        void OnFileDialogDismissed(
            const std::vector<CefString>& file_paths) override {
            
            CefRefPtr<CefProcessMessage> response = 
                CefProcessMessage::Create("download_folder_selected");
            CefRefPtr<CefListValue> args = response->GetArgumentList();
            
            if (!file_paths.empty()) {
                args->SetString(0, file_paths[0]);
            } else {
                args->SetString(0, ""); // User cancelled
            }
            
            browser_->GetMainFrame()->SendProcessMessage(PID_RENDERER, response);
        }
        
    private:
        CefRefPtr<CefBrowser> browser_;
        IMPLEMENT_REFCOUNTING(FolderDialogCallback);
    };
    
    browser->GetHost()->RunFileDialog(
        FILE_DIALOG_OPEN_FOLDER,
        CefString("Select Download Folder"),
        CefString(currentPath),
        std::vector<CefString>(),
        new FolderDialogCallback(browser)
    );
```

### Step 3: Update Frontend Download Settings

**File**: `frontend/src/components/settings/DownloadSettings.tsx`

```tsx
import React, { useEffect } from 'react';
import { Typography, Box, Button } from '@mui/material';
import { SettingsCard, SettingRow } from './SettingsCard';
import { useSettings } from '../../hooks/useSettings';

const DownloadSettings: React.FC = () => {
  const { settings, updateSetting } = useSettings();

  // Listen for folder picker response
  useEffect(() => {
    const handleMessage = (event: MessageEvent) => {
      if (event.data.type === 'download_folder_selected' && event.data.path) {
        updateSetting('browser.downloadsPath', event.data.path);
      }
    };
    
    window.addEventListener('message', handleMessage);
    return () => window.removeEventListener('message', handleMessage);
  }, [updateSetting]);

  const handleBrowse = () => {
    window.cefMessage?.send('download_browse_folder', []);
  };

  return (
    <Box>
      <Typography variant="h5" sx={{ mb: 3, color: '#e0e0e0' }}>
        Downloads
      </Typography>

      <SettingsCard title="Download Location">
        <SettingRow
          label="Default download folder"
          description={settings.browser.downloadsPath || 'System default (Downloads folder)'}
          control={
            <Box sx={{ display: 'flex', gap: 1, alignItems: 'center' }}>
              <input
                type="text"
                value={settings.browser.downloadsPath}
                onChange={(e) => updateSetting('browser.downloadsPath', e.target.value)}
                placeholder="System default"
                style={{
                  width: 200,
                  padding: '6px 10px',
                  border: '1px solid #444',
                  borderRadius: 4,
                  backgroundColor: '#2a2a2a',
                  color: '#e0e0e0',
                  fontSize: '0.85rem',
                  outline: 'none',
                }}
              />
              <Button
                variant="outlined"
                size="small"
                onClick={handleBrowse}
                sx={{
                  color: '#a67c00',
                  borderColor: '#a67c00',
                  '&:hover': { borderColor: '#c99a00', bgcolor: 'rgba(166,124,0,0.1)' },
                }}
              >
                Browse
              </Button>
            </Box>
          }
        />
      </SettingsCard>
    </Box>
  );
};

export default DownloadSettings;
```

---

## Phase 2: "Ask Where to Save" Toggle (1 hour)

### Step 1: Add Setting to SettingsManager

**File**: `include/core/SettingsManager.h`

```cpp
struct BrowserSettings {
    // ... existing fields ...
    bool askWhereToSave = true;  // NEW
};

// Update JSON serialization macro:
NLOHMANN_DEFINE_TYPE_NON_INTRUSIVE_WITH_DEFAULT(BrowserSettings,
    homepage, searchEngine, zoomLevel, showBookmarkBar, 
    downloadsPath, restoreSessionOnStart, askWhereToSave)  // ADD askWhereToSave
```

Add setter:
```cpp
void SetAskWhereToSave(bool ask);
```

### Step 2: Update OnBeforeDownload

```cpp
bool askWhereToSave = settings.askWhereToSave;

// If "ask" is disabled but no folder is set, force the dialog anyway
if (!askWhereToSave && downloadsPath.empty()) {
    LOG_WARN_BROWSER("No download folder set — forcing Save As dialog");
    askWhereToSave = true;
}
```

### Step 3: Add Toggle to Frontend

```tsx
<SettingRow
  label="Ask where to save each file"
  description="Show a dialog to choose the download location for every file"
  control={
    <Switch
      checked={settings.browser.askWhereToSave ?? true}
      onChange={(e) => updateSetting('browser.askWhereToSave', e.target.checked)}
      size="small"
      disabled={!settings.browser.downloadsPath} // Disable if no folder set
    />
  }
/>
```

---

## Gaps & Edge Cases

| Gap | Resolution |
|-----|------------|
| Invalid folder path at download time | Fall back to Save As dialog + log warning |
| Folder deleted between setting and download | Same — graceful fallback |
| Path with special characters | Let filesystem handle it |
| Network paths (UNC) | Should work on Windows |
| Path too long | Windows MAX_PATH limit (~260) — rare edge case |

---

## Test Checklist

### Phase 1: Folder Selection
- [ ] Click "Browse" → native folder picker opens
- [ ] Select a folder → path appears in text input
- [ ] Path persists after browser restart
- [ ] Download a file → Save As dialog starts in selected folder

### Phase 1: Fallback Behavior
- [ ] Clear download folder → downloads use system default
- [ ] Set invalid/deleted folder path → Save As dialog appears (graceful fallback)

### Phase 2: Ask Where to Save Toggle
- [ ] Toggle OFF + folder set → downloads go silently to folder
- [ ] Toggle ON + folder set → Save As dialog appears, starting in folder
- [ ] Toggle OFF + NO folder → Save As dialog forced (can't download silently without a path)
- [ ] Toggle disabled when no folder is set (good UX)

### Persistence
- [ ] All settings persist across browser restart

---

## Files to Modify

| File | Changes |
|------|---------|
| `src/handlers/simple_handler.cpp` | Read settings in OnBeforeDownload, add folder picker IPC |
| `include/core/SettingsManager.h` | Add `askWhereToSave` field + JSON serialization |
| `src/core/SettingsManager.cpp` | Add setter implementation |
| `frontend/src/components/settings/DownloadSettings.tsx` | Browse button + toggle |

---

**Last Updated**: 2026-02-28
