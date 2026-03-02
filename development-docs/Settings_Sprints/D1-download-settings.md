# D1: Download Settings

**Status**: **COMPLETE** (2026-03-01)
**Complexity**: Low-Medium

---

## Summary

Wired download folder selection and "Ask where to save" toggle to actual behavior. Users can browse for a folder via native Win32 dialog, and the setting controls both silent downloads and Save As dialog initial directory.

---

## What Was Done

### C++ — SettingsManager
- [x] Added `bool askWhereToSave = true` to `BrowserSettings` struct
- [x] Added `SetAskWhereToSave()` setter + JSON serialization

### C++ — OnBeforeDownload (simple_handler.cpp)
- [x] Reads `downloadsPath` and `askWhereToSave` from SettingsManager
- [x] **Ask ON + custom folder**: Win32 `IFileSaveDialog` opens in configured folder (bypasses CEF's dialog which ignores the directory)
- [x] **Ask OFF + custom folder**: Silent download to configured folder
- [x] **Ask ON + no folder**: CEF's default Save As dialog (system Downloads)
- [x] **Ask OFF + no folder**: Silent download to system default
- [x] Invalid/deleted folder → falls back to CEF Save As

### C++ — Folder Picker (download_browse_folder IPC)
- [x] Win32 `IFileOpenDialog` with `FOS_PICKFOLDERS` — button says "Select Folder" (not CEF's "Upload")
- [x] Runs on detached thread to avoid blocking CEF UI thread
- [x] Result relayed via `download_folder_selected` IPC → renderer → `window.onDownloadFolderSelected()` → `updateSetting()`

### Frontend — DownloadSettings.tsx
- [x] Browse button with `FolderOpenIcon` — opens native folder picker
- [x] "Ask where to save each file" toggle (Switch, default ON)
- [x] Displays current path or "System default (Downloads folder)"

---

## Key Gotcha: CEF Save As Dialog Ignores Directory

CEF's `CefBeforeDownloadCallback::Continue(path, showDialog=true)` does NOT honor the directory component of the path — the Save As dialog always opens in Chromium's internal default. Neither `download.default_directory` nor `savefile.default_directory` preferences fix this.

**Solution**: Bypass CEF's dialog entirely on Windows. Use Win32 `IFileSaveDialog` with `SetFolder()` for the initial directory, then call `Continue(selectedPath, false)` to start the download silently to the user's chosen location.

---

## Test Results

- [x] Browse button opens native folder picker with "Select Folder" button
- [x] Selected folder appears in settings and persists
- [x] Save As dialog opens in the configured folder
- [x] "Ask where to save" OFF → download goes silently to set folder
- [x] "Ask where to save" ON → Save As dialog appears in correct folder
- [x] Settings persist across restart

---

**Last Updated**: 2026-03-01
