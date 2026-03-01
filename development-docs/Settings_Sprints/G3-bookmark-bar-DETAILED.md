# G3: Bookmark Bar — Detailed Implementation Plan

**Status**: Not Started  
**Complexity**: High (multi-phase)  
**Estimated Time**: 10-16 hours (across 4-5 phases)  
**Dependencies**: BookmarkManager backend (exists)

---

## Executive Summary

Implement a bookmark bar UI below the address bar. The backend (BookmarkManager with SQLite, add/edit/delete/reorder/folders) exists — this sprint creates the frontend components and wires them to the C++ backend via IPC.

---

## Current State Analysis

### What Exists
- **UI Toggle**: In `GeneralSettings.tsx` — "Show bookmark bar" switch
- **Persistence**: `SettingsManager::SetShowBookmarkBar(bool)` saves to `settings.json`
- **Backend**: `BookmarkManager` in C++ with full CRUD operations:
  - `AddBookmark()`, `GetBookmark()`, `UpdateBookmark()`, `RemoveBookmark()`
  - `SearchBookmarks()`, `GetAllBookmarks()`, `IsBookmarked()`
  - `CreateFolder()`, `ListFolders()`, `UpdateFolder()`, `RemoveFolder()`
  - Returns JSON strings for all operations
- **Ctrl+D**: Exists in C++ but no UI feedback — silently bookmarks
- **Import**: `ProfileImporter` can import bookmarks from Chrome/Brave

### What's Missing
- No `BookmarkBar.tsx` component
- No IPC handlers to expose BookmarkManager to frontend
- Header HWND doesn't resize when bar toggles
- No Ctrl+D popup/editor
- No right-click context menu on bookmarks
- No star icon in address bar

---

## Architecture Overview

### Data Flow

```
BookmarkManager (C++) ← SQLite DB
    ↓ IPC
React BookmarkBar ← renders items
    ↓ click
navigate(url) via cefMessage
```

### Header HWND Resizing

The header HWND currently has fixed height (~104px for tab bar + toolbar). Adding bookmark bar requires:

1. Header height becomes dynamic: 104px (no bar) → ~134px (with bar)
2. Tab webview HWNDs reposition when bar toggles
3. React header component includes bookmark bar conditionally

---

## Phase 1: IPC Handlers (2-3 hours)

### Step 1: Expose BookmarkManager via IPC

**File**: `simple_handler.cpp` — in `OnProcessMessageReceived()`

```cpp
// ==================== Bookmark IPC Handlers ====================

} else if (message_name == "bookmark_get_bar_items") {
    // Get bookmarks for the bar (top-level items, folder_id = -1)
    int limit = 20;
    if (args->GetSize() > 0 && args->GetType(0) == VTYPE_INT) {
        limit = args->GetInt(0);
    }
    
    auto& bm = BookmarkManager::GetInstance();
    std::string json = bm.GetAllBookmarks(-1, limit, 0); // -1 = root level
    
    CefRefPtr<CefProcessMessage> response = 
        CefProcessMessage::Create("bookmark_bar_items");
    response->GetArgumentList()->SetString(0, json);
    browser->GetMainFrame()->SendProcessMessage(PID_RENDERER, response);

} else if (message_name == "bookmark_add") {
    // Args: url, title, folderId (optional), tags (optional)
    std::string url = args->GetString(0).ToString();
    std::string title = args->GetString(1).ToString();
    int folderId = (args->GetSize() > 2) ? args->GetInt(2) : -1;
    std::vector<std::string> tags; // Empty for now
    
    auto& bm = BookmarkManager::GetInstance();
    std::string result = bm.AddBookmark(url, title, folderId, tags);
    
    CefRefPtr<CefProcessMessage> response = 
        CefProcessMessage::Create("bookmark_added");
    response->GetArgumentList()->SetString(0, result);
    browser->GetMainFrame()->SendProcessMessage(PID_RENDERER, response);

} else if (message_name == "bookmark_remove") {
    int64_t id = args->GetInt(0);
    auto& bm = BookmarkManager::GetInstance();
    std::string result = bm.RemoveBookmark(id);
    
    CefRefPtr<CefProcessMessage> response = 
        CefProcessMessage::Create("bookmark_removed");
    response->GetArgumentList()->SetString(0, result);
    browser->GetMainFrame()->SendProcessMessage(PID_RENDERER, response);

} else if (message_name == "bookmark_update") {
    int64_t id = args->GetInt(0);
    std::string title = args->GetString(1).ToString();
    std::string url = args->GetString(2).ToString();
    int folderId = args->GetInt(3);
    std::vector<std::string> tags;
    
    auto& bm = BookmarkManager::GetInstance();
    std::string result = bm.UpdateBookmark(id, title, url, folderId, tags);
    
    CefRefPtr<CefProcessMessage> response = 
        CefProcessMessage::Create("bookmark_updated");
    response->GetArgumentList()->SetString(0, result);
    browser->GetMainFrame()->SendProcessMessage(PID_RENDERER, response);

} else if (message_name == "bookmark_is_bookmarked") {
    std::string url = args->GetString(0).ToString();
    auto& bm = BookmarkManager::GetInstance();
    std::string result = bm.IsBookmarked(url);
    
    CefRefPtr<CefProcessMessage> response = 
        CefProcessMessage::Create("bookmark_status");
    response->GetArgumentList()->SetString(0, result);
    browser->GetMainFrame()->SendProcessMessage(PID_RENDERER, response);

} else if (message_name == "bookmark_get_folders") {
    int parentId = (args->GetSize() > 0) ? args->GetInt(0) : -1;
    auto& bm = BookmarkManager::GetInstance();
    std::string result = bm.ListFolders(parentId);
    
    CefRefPtr<CefProcessMessage> response = 
        CefProcessMessage::Create("bookmark_folders");
    response->GetArgumentList()->SetString(0, result);
    browser->GetMainFrame()->SendProcessMessage(PID_RENDERER, response);
```

---

## Phase 2: Bookmark Bar Component (3-4 hours)

### Step 1: Create BookmarkBar Component

**File**: `frontend/src/components/BookmarkBar.tsx`

```tsx
import React, { useState, useEffect, useRef } from 'react';
import { useSettings } from '../hooks/useSettings';
import './BookmarkBar.css';

interface Bookmark {
  id: number;
  url: string;
  title: string;
  folder_id: number;
  favicon_url?: string;
  position: number;
}

interface BookmarkBarProps {
  onNavigate: (url: string) => void;
}

const BookmarkBar: React.FC<BookmarkBarProps> = ({ onNavigate }) => {
  const { settings } = useSettings();
  const [bookmarks, setBookmarks] = useState<Bookmark[]>([]);
  const [overflowItems, setOverflowItems] = useState<Bookmark[]>([]);
  const [showOverflow, setShowOverflow] = useState(false);
  const barRef = useRef<HTMLDivElement>(null);

  // Fetch bookmarks
  useEffect(() => {
    window.cefMessage?.send('bookmark_get_bar_items', [20]);

    const handleMessage = (event: MessageEvent) => {
      if (event.data.type === 'bookmark_bar_items') {
        try {
          const data = JSON.parse(event.data.json);
          setBookmarks(data.bookmarks || []);
        } catch (e) {
          console.error('Failed to parse bookmarks:', e);
        }
      }
    };

    window.addEventListener('message', handleMessage);
    return () => window.removeEventListener('message', handleMessage);
  }, []);

  // Calculate overflow
  useEffect(() => {
    if (!barRef.current) return;
    
    const barWidth = barRef.current.offsetWidth;
    const items = barRef.current.querySelectorAll('.bookmark-item');
    let totalWidth = 0;
    let overflowIndex = bookmarks.length;
    
    items.forEach((item, index) => {
      totalWidth += (item as HTMLElement).offsetWidth + 8; // 8px gap
      if (totalWidth > barWidth - 50) { // 50px for overflow button
        overflowIndex = Math.min(overflowIndex, index);
      }
    });
    
    if (overflowIndex < bookmarks.length) {
      setOverflowItems(bookmarks.slice(overflowIndex));
    } else {
      setOverflowItems([]);
    }
  }, [bookmarks]);

  const handleClick = (url: string) => {
    onNavigate(url);
  };

  const handleContextMenu = (e: React.MouseEvent, bookmark: Bookmark) => {
    e.preventDefault();
    // TODO: Show context menu with edit/delete options
  };

  // Get favicon URL
  const getFaviconUrl = (url: string): string => {
    try {
      const domain = new URL(url).hostname;
      return `https://www.google.com/s2/favicons?domain=${domain}&sz=16`;
    } catch {
      return '';
    }
  };

  if (!settings.browser.showBookmarkBar) {
    return null;
  }

  const visibleBookmarks = overflowItems.length > 0 
    ? bookmarks.slice(0, bookmarks.length - overflowItems.length)
    : bookmarks;

  return (
    <div className="bookmark-bar" ref={barRef}>
      {visibleBookmarks.map((bookmark) => (
        <div
          key={bookmark.id}
          className="bookmark-item"
          onClick={() => handleClick(bookmark.url)}
          onContextMenu={(e) => handleContextMenu(e, bookmark)}
          title={bookmark.url}
        >
          <img 
            src={getFaviconUrl(bookmark.url)} 
            alt="" 
            className="bookmark-favicon"
            onError={(e) => {
              (e.target as HTMLImageElement).style.display = 'none';
            }}
          />
          <span className="bookmark-title">
            {bookmark.title || new URL(bookmark.url).hostname}
          </span>
        </div>
      ))}
      
      {overflowItems.length > 0 && (
        <div className="bookmark-overflow">
          <button 
            className="overflow-button"
            onClick={() => setShowOverflow(!showOverflow)}
          >
            »
          </button>
          {showOverflow && (
            <div className="overflow-menu">
              {overflowItems.map((bookmark) => (
                <div
                  key={bookmark.id}
                  className="overflow-item"
                  onClick={() => handleClick(bookmark.url)}
                >
                  <img src={getFaviconUrl(bookmark.url)} alt="" />
                  <span>{bookmark.title || bookmark.url}</span>
                </div>
              ))}
            </div>
          )}
        </div>
      )}
    </div>
  );
};

export default BookmarkBar;
```

### Step 2: Create CSS

**File**: `frontend/src/components/BookmarkBar.css`

```css
.bookmark-bar {
  display: flex;
  align-items: center;
  height: 28px;
  background-color: #1a1a1a;
  border-bottom: 1px solid #333;
  padding: 0 8px;
  gap: 4px;
  overflow: hidden;
}

.bookmark-item {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 4px 8px;
  border-radius: 4px;
  cursor: pointer;
  white-space: nowrap;
  max-width: 150px;
  transition: background-color 0.15s;
}

.bookmark-item:hover {
  background-color: rgba(255, 255, 255, 0.1);
}

.bookmark-favicon {
  width: 16px;
  height: 16px;
  flex-shrink: 0;
}

.bookmark-title {
  font-size: 12px;
  color: #e0e0e0;
  overflow: hidden;
  text-overflow: ellipsis;
}

.bookmark-overflow {
  position: relative;
  margin-left: auto;
}

.overflow-button {
  background: none;
  border: none;
  color: #888;
  font-size: 16px;
  cursor: pointer;
  padding: 4px 8px;
  border-radius: 4px;
}

.overflow-button:hover {
  background-color: rgba(255, 255, 255, 0.1);
  color: #e0e0e0;
}

.overflow-menu {
  position: absolute;
  top: 100%;
  right: 0;
  background-color: #2a2a2a;
  border: 1px solid #444;
  border-radius: 4px;
  min-width: 200px;
  max-height: 300px;
  overflow-y: auto;
  z-index: 1000;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
}

.overflow-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 12px;
  cursor: pointer;
}

.overflow-item:hover {
  background-color: rgba(255, 255, 255, 0.1);
}

.overflow-item img {
  width: 16px;
  height: 16px;
}

.overflow-item span {
  font-size: 13px;
  color: #e0e0e0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
```

### Step 3: Integrate with Header

**File**: `frontend/src/components/Header.tsx` (or wherever the header is)

```tsx
import BookmarkBar from './BookmarkBar';

// In render, after the toolbar:
<BookmarkBar onNavigate={handleNavigate} />
```

---

## Phase 3: Header HWND Resizing (2-3 hours)

This is the trickiest part — the header HWND height must be dynamic.

### Option A: React-Driven Height (Recommended)

1. React calculates total header height based on `showBookmarkBar`
2. Sends IPC to notify C++ of new height
3. C++ resizes header HWND and repositions tab HWNDs

**Frontend**:
```tsx
useEffect(() => {
  const headerHeight = settings.browser.showBookmarkBar ? 132 : 104;
  window.cefMessage?.send('set_header_height', [headerHeight]);
}, [settings.browser.showBookmarkBar]);
```

**C++**:
```cpp
} else if (message_name == "set_header_height") {
    int newHeight = args->GetInt(0);
    g_header_height = newHeight;
    
    // Resize header HWND
    SetWindowPos(g_header_hwnd, NULL, 0, 0, 
                 g_window_width, newHeight, 
                 SWP_NOZORDER | SWP_NOMOVE);
    
    // Reposition all tab HWNDs
    auto& tabManager = TabManager::GetInstance();
    for (auto* tab : tabManager.GetAllTabs()) {
        SetWindowPos(tab->hwnd, NULL, 
                     0, newHeight, 
                     g_window_width, g_window_height - newHeight,
                     SWP_NOZORDER);
    }
}
```

### Option B: Toggle IPC

Simpler — just send "bookmark_bar_toggled" and C++ handles the rest:

```cpp
} else if (message_name == "bookmark_bar_toggled") {
    bool visible = args->GetBool(0);
    int headerHeight = visible ? 132 : 104;
    // Same resize logic as Option A
}
```

---

## Phase 4: Add/Edit Bookmark UI (3-4 hours)

### Ctrl+D Popup

When user presses Ctrl+D, show a small popup to name/edit the bookmark before saving.

**Component**: `BookmarkEditor.tsx`

```tsx
interface BookmarkEditorProps {
  url: string;
  initialTitle: string;
  existingBookmarkId?: number;
  onSave: (title: string, url: string, folderId: number) => void;
  onClose: () => void;
}

const BookmarkEditor: React.FC<BookmarkEditorProps> = ({
  url, initialTitle, existingBookmarkId, onSave, onClose
}) => {
  const [title, setTitle] = useState(initialTitle);
  const [selectedFolder, setSelectedFolder] = useState(-1);
  const [folders, setFolders] = useState<Folder[]>([]);

  useEffect(() => {
    window.cefMessage?.send('bookmark_get_folders', [-1]);
    // Listen for folder response...
  }, []);

  return (
    <div className="bookmark-editor-overlay">
      <div className="bookmark-editor">
        <h3>{existingBookmarkId ? 'Edit Bookmark' : 'Add Bookmark'}</h3>
        <input
          type="text"
          value={title}
          onChange={(e) => setTitle(e.target.value)}
          placeholder="Name"
          autoFocus
        />
        <select 
          value={selectedFolder} 
          onChange={(e) => setSelectedFolder(Number(e.target.value))}
        >
          <option value={-1}>Bookmark Bar</option>
          {folders.map(f => (
            <option key={f.id} value={f.id}>{f.name}</option>
          ))}
        </select>
        <div className="editor-buttons">
          <button onClick={() => onSave(title, url, selectedFolder)}>
            {existingBookmarkId ? 'Update' : 'Save'}
          </button>
          <button onClick={onClose}>Cancel</button>
        </div>
      </div>
    </div>
  );
};
```

### Wire Ctrl+D

The existing Ctrl+D in C++ needs to send IPC to frontend instead of silently bookmarking:

```cpp
// In keyboard handler when Ctrl+D pressed:
CefRefPtr<CefProcessMessage> msg = CefProcessMessage::Create("show_bookmark_editor");
msg->GetArgumentList()->SetString(0, currentUrl);
msg->GetArgumentList()->SetString(1, currentTitle);
browser->GetMainFrame()->SendProcessMessage(PID_RENDERER, msg);
```

---

## Phase 5: Context Menu + Star Icon (2 hours)

### Right-Click Context Menu

```tsx
// In BookmarkBar, on right-click:
const handleContextMenu = (e: React.MouseEvent, bookmark: Bookmark) => {
  e.preventDefault();
  setContextMenu({
    x: e.clientX,
    y: e.clientY,
    bookmark: bookmark,
  });
};

// Context menu component
<ContextMenu x={contextMenu.x} y={contextMenu.y}>
  <MenuItem onClick={() => editBookmark(contextMenu.bookmark)}>Edit</MenuItem>
  <MenuItem onClick={() => openInNewTab(contextMenu.bookmark.url)}>Open in New Tab</MenuItem>
  <Divider />
  <MenuItem onClick={() => deleteBookmark(contextMenu.bookmark.id)}>Delete</MenuItem>
</ContextMenu>
```

### Star Icon in Address Bar

Show a star icon next to the URL that's filled if the current page is bookmarked:

```tsx
// In toolbar/address bar area
const [isBookmarked, setIsBookmarked] = useState(false);

useEffect(() => {
  window.cefMessage?.send('bookmark_is_bookmarked', [currentUrl]);
  // Listen for response...
}, [currentUrl]);

<button 
  className={`star-icon ${isBookmarked ? 'filled' : ''}`}
  onClick={() => toggleBookmark()}
>
  {isBookmarked ? '★' : '☆'}
</button>
```

---

## Gaps & Questions

| Gap | Resolution |
|-----|------------|
| Drag-and-drop reordering | Defer to Phase 6 (post-MVP) |
| Folder contents on click | Show dropdown with folder items |
| Bookmark manager page | Defer to separate sprint |
| Favicon caching | Use Google S2 for Phase 1; local cache later |

---

## Test Checklist

### Phase 1 (IPC)
- [ ] `bookmark_get_bar_items` returns bookmarks JSON
- [ ] `bookmark_add` creates new bookmark
- [ ] `bookmark_remove` deletes bookmark
- [ ] `bookmark_is_bookmarked` returns correct status

### Phase 2 (Bar Component)
- [ ] Toggle "Show bookmark bar" → bar appears/disappears
- [ ] Bookmarks display with favicons and titles
- [ ] Click bookmark → navigates to URL
- [ ] Overflow menu works when too many bookmarks

### Phase 3 (Resizing)
- [ ] Webview resizes correctly when bar toggles
- [ ] No visual glitches during resize
- [ ] Works with multiple tabs open

### Phase 4 (Add/Edit)
- [ ] Ctrl+D → popup appears
- [ ] Can edit name before saving
- [ ] Save creates bookmark in bar
- [ ] Edit existing bookmark works

### Phase 5 (Polish)
- [ ] Right-click shows context menu
- [ ] Edit/Delete from context menu work
- [ ] Star icon shows bookmarked state
- [ ] Click star toggles bookmark

---

## Files to Create/Modify

| File | Action | Description |
|------|--------|-------------|
| `frontend/src/components/BookmarkBar.tsx` | **CREATE** | Bookmark bar component |
| `frontend/src/components/BookmarkBar.css` | **CREATE** | Styling |
| `frontend/src/components/BookmarkEditor.tsx` | **CREATE** | Add/edit popup |
| `frontend/src/components/Header.tsx` | MODIFY | Include BookmarkBar |
| `src/handlers/simple_handler.cpp` | MODIFY | Add bookmark IPC handlers |
| `cef_browser_shell.cpp` | MODIFY | Header resize + Ctrl+D handling |

---

**Last Updated**: 2026-02-28
