# G3: Bookmark Bar

**Status**: Not Started
**Complexity**: High (multi-phase)
**Estimated Phases**: 4-5

---

## Current State

- UI toggle exists in `GeneralSettings.tsx` — "Show bookmark bar" switch
- Setting persists to `settings.json` via `SettingsManager`
- **Not wired**: Toggle controls nothing — no bookmark bar UI exists
- **Backend exists**: `BookmarkManager` in C++ has add/edit/delete/reorder/folder support
- Bookmark import from Chrome/Brave exists (`ProfileImporter`)
- Ctrl+D shortcut exists in `simple_handler.cpp` but bookmarks to C++ backend only — no UI feedback
- No frontend bookmark components exist

---

## What Needs to Happen

### Phase 1: Research & Design Decisions

Before implementation, several UX decisions need to be made:

**Layout & positioning**:
- [ ] Where does the bookmark bar sit? Below address bar (Chrome-style)?
- [ ] How does it affect the webview height? (Need to resize tab HWNDs)
- [ ] Fixed height or auto-hide?

**Data model questions**:
- [ ] Flat list only or folders? (BookmarkManager supports folders already)
- [ ] Max visible items before overflow menu?
- [ ] Favicon display — fetch and cache favicons?

**Interaction patterns**:
- [ ] Click to navigate (same tab or new tab?)
- [ ] Right-click context menu (edit, delete, open in new tab)
- [ ] Drag-and-drop reordering?
- [ ] "Other bookmarks" overflow folder?

**Ctrl+D experience**:
- [ ] What happens when user presses Ctrl+D? Popup editor? Toast confirmation?
- [ ] Edit bookmark name/URL/folder before saving?
- [ ] Visual feedback that bookmark was added?

### Phase 2: Bookmark Bar Component

**Goal**: Render a horizontal bar below the address bar showing bookmarks.

**Changes needed**:
- [ ] Create `BookmarkBar.tsx` component
- [ ] Read bookmarks from C++ via IPC (`bookmark_get_all` or similar)
- [ ] Render as horizontal list of clickable items with favicons
- [ ] Handle overflow (items that don't fit → ">" overflow menu)
- [ ] Show/hide based on `showBookmarkBar` setting
- [ ] Adjust header HWND height when bar is visible/hidden

**C++ changes needed**:
- [ ] IPC handlers: `bookmark_get_bar_items`, `bookmark_add`, `bookmark_remove`, `bookmark_edit`
- [ ] Resize header HWND when bookmark bar toggles (taller toolbar = less webview space)
- [ ] Forward bookmark data to React via IPC

### Phase 3: Add/Edit/Delete UI

**Goal**: Users can manage bookmarks from the bar.

**Changes needed**:
- [ ] Ctrl+D popup: small overlay to name/edit bookmark before saving
- [ ] Right-click context menu on bookmark items
- [ ] Edit dialog (name, URL, folder)
- [ ] Delete with confirmation
- [ ] Star icon in address bar showing "bookmarked" state for current page

### Phase 4: Drag-and-Drop Reordering (Optional)

**Goal**: Users can reorder bookmarks by dragging.

**Changes needed**:
- [ ] HTML5 drag-and-drop on bookmark items
- [ ] Visual drop indicator
- [ ] Persist new order to C++ backend
- [ ] Folder drag-into support

### Phase 5: Bookmark Manager Page (Optional/Future)

**Goal**: Full-page bookmark management (like chrome://bookmarks).

**Changes needed**:
- [ ] `BookmarkManagerPage.tsx` with tree view
- [ ] Search, bulk operations, import/export
- [ ] Route: `/bookmarks` (displayed as `hodos://bookmarks`)

---

## Architecture Considerations

**Header HWND resizing**: The bookmark bar lives inside the header HWND. Currently the header is fixed at ~104px (tab bar + toolbar). Adding a bookmark bar means:
- Header height becomes dynamic (104px without bar, ~134px with bar)
- Tab webview HWNDs need to be repositioned when bar toggles
- This is a C++ change in `WndProc` WM_SIZE handling

**Bookmark data flow**:
```
BookmarkManager (C++) ← SQLite DB
    ↓ IPC
React BookmarkBar ← renders items
    ↓ click
navigate(url) via cefMessage
```

**Performance**: If user has hundreds of bookmarks, only bar-level items should load on startup. Folder contents load on click.

---

## Dependencies

- `BookmarkManager` C++ backend — exists, needs IPC exposure
- `ProfileImporter` bookmark import — exists, populates BookmarkManager
- Header HWND dynamic resizing — new C++ work
- Favicon fetching/caching — new infrastructure (or use default icon)

---

## Test Checklist

- [ ] Toggle "Show bookmark bar" → bar appears/disappears
- [ ] Webview resizes correctly when bar toggles
- [ ] Click bookmark → navigates to URL
- [ ] Ctrl+D → adds current page to bookmarks with edit popup
- [ ] Right-click bookmark → context menu with edit/delete
- [ ] Bookmark persists across browser restart
- [ ] Imported bookmarks (from Chrome/Brave) appear in bar
- [ ] Overflow menu works when too many bookmarks for bar width
- [ ] Verify per-profile bookmark isolation

---

**Last Updated**: 2026-02-28
