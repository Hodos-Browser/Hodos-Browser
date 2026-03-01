# D1: Download Settings

**Status**: Not Started
**Complexity**: Low-Medium
**Estimated Phases**: 2

---

## Current State

- UI exists in `DownloadSettings.tsx` — text input for download folder path
- Setting persists to `settings.json` via `SettingsManager::SetDownloadsPath()`
- **Not wired**: `OnBeforeDownload()` in `simple_handler.cpp` calls `callback->Continue("", true)` — empty path (system default) and always shows Save As dialog
- No "Ask where to save" toggle exists in UI
- No folder picker button — user must type path manually

---

## What Needs to Happen

### Phase 1: Wire Settings + Folder Picker

**Goal**: Default download folder is used, and users can browse for a folder via native file dialog.

**Changes needed**:

**C++ — Read settings in OnBeforeDownload**:
- [ ] In `OnBeforeDownload()`, read `SettingsManager::GetBrowserSettings().downloadsPath`
- [ ] If path is set and valid, use it as the base path: `callback->Continue(downloadsPath + "/" + suggested_name, show_dialog)`
- [ ] If path is empty, fall back to current behavior (empty string = system default)

**C++ — Folder picker IPC**:
- [ ] Add `download_browse_folder` IPC handler in `OnProcessMessageReceived()`
- [ ] Use `CefBrowserHost::RunFileDialog()` with `FILE_DIALOG_OPEN_FOLDER` mode
- [ ] Return selected path to frontend via response message
- [ ] Frontend updates the text input with the selected path

**Frontend — Add Browse button**:
- [ ] Add "Browse" button next to the download path text input in `DownloadSettings.tsx`
- [ ] Button sends `download_browse_folder` IPC
- [ ] Listen for response and update input + save setting

**Design decisions**:
- Validate folder exists before saving? (Recommended: yes, or at least warn)
- What if saved folder no longer exists at download time? (Fall back to system default + show Save As)

### Phase 2: "Ask where to save" Toggle

**Goal**: Toggle controls whether Save As dialog appears on every download.

**Changes needed**:

**C++ — Read setting in OnBeforeDownload**:
- [ ] Read `SettingsManager::GetBrowserSettings().askWhereToSave` (new field, default: `true`)
- [ ] Pass as `show_dialog` parameter: `callback->Continue(path, askWhereToSave)`
- [ ] If `askWhereToSave` is false, downloads go silently to the default folder

**Frontend — Add toggle**:
- [ ] Add "Ask where to save each file" toggle in `DownloadSettings.tsx`
- [ ] Persists via `updateSetting('browser.askWhereToSave', value)`

**SettingsManager — New field**:
- [ ] Add `askWhereToSave` bool to `BrowserSettings` struct (default: `true`)
- [ ] Add getter/setter in `SettingsManager`
- [ ] Add to JSON serialization/deserialization

**Design decisions**:
- If "ask" is off but no default folder is set, should we force the dialog? (Recommended: yes — need a valid path for silent downloads)
- Should the toggle be disabled until a default folder is set? (Good UX hint)

---

## Architecture Notes

**CEF folder picker**: `CefBrowserHost::RunFileDialog(FILE_DIALOG_OPEN_FOLDER, title, default_path, accept_filters, callback)` — async, returns selected path via `CefRunFileDialogCallback::OnFileDialogDismissed()`. Must run on UI thread.

**OnBeforeDownload flow**:
```cpp
void SimpleHandler::OnBeforeDownload(
    CefRefPtr<CefBrowser> browser,
    CefRefPtr<CefDownloadItem> download_item,
    const CefString& suggested_file_name,
    CefRefPtr<CefBeforeDownloadCallback> callback) {

    auto& settings = SettingsManager::GetInstance().GetBrowserSettings();
    std::string path = settings.downloadsPath;
    bool show_dialog = settings.askWhereToSave;

    if (!path.empty()) {
        // Append suggested filename to folder path
        path += "/" + suggested_file_name.ToString();
    }

    callback->Continue(path, show_dialog);
}
```

---

## Test Checklist

- [ ] Set download folder via Browse button → folder picker opens → selected path appears in input
- [ ] Download a file → Save As dialog starts in the selected folder (not system default)
- [ ] Clear download folder → downloads use system default folder again
- [ ] Toggle "Ask where to save" OFF → download goes silently to default folder
- [ ] Toggle "Ask where to save" ON → Save As dialog appears again
- [ ] "Ask" OFF with no folder set → Save As dialog appears anyway (fallback)
- [ ] Settings persist across browser restart
- [ ] Invalid/deleted folder path → graceful fallback to Save As dialog

---

**Last Updated**: 2026-02-28
