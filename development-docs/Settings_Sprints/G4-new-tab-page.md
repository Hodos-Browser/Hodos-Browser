# G4: New Tab Page & Homepage Defaults

**Status**: Not Started
**Complexity**: Medium-High (multi-phase)
**Estimated Phases**: 4

---

## Current State

- **Homepage**: Set to `coingeek.com` by default, configurable via General settings
- **New tab**: Opens `metanetapps.com` (hardcoded in C++ `TabManager::CreateTab()`)
- **No new tab page**: There is no custom Hodos new tab page — new tabs just navigate to an external URL
- **No separation**: Homepage and new tab page are not independently configurable
- **No right-click "Set as homepage"** option

---

## What Needs to Happen

### Phase 1: New Tab Page Component

**Goal**: Create a branded new tab page that loads when the user opens a new tab (Ctrl+T or "+" button).

**Design**:
```
┌─────────────────────────────────────────────┐
│                                             │
│              [Hodos Logo]                   │
│                                             │
│     ┌─────────────────────────────┐         │
│     │  🔍  Search or enter URL    │         │
│     └─────────────────────────────┘         │
│                                             │
│     ┌────┐  ┌────┐  ┌────┐  ┌────┐         │
│     │ 🌐 │  │ 🌐 │  │ 🌐 │  │ 🌐 │         │
│     │Site│  │Site│  │Site│  │Site│         │
│     └────┘  └────┘  └────┘  └────┘         │
│     ┌────┐  ┌────┐  ┌────┐  ┌────┐         │
│     │ 🌐 │  │ 🌐 │  │ 🌐 │  │ 🌐 │         │
│     │Site│  │Site│  │Site│  │Site│         │
│     └────┘  └────┘  └────┘  └────┘         │
│                                             │
│        🛡️ 142 trackers blocked today        │
│                                             │
└─────────────────────────────────────────────┘
```

**Changes needed**:
- [ ] Create `NewTabPage.tsx` component in `frontend/src/pages/`
- [ ] Add route `/newtab` in `App.tsx`
- [ ] Hodos logo centered (gold on dark background)
- [ ] Search bar that uses default search engine (ties into G1)
  - Non-URL input → search with default engine
  - URL input → navigate directly
- [ ] Most-visited site tiles (6-8 tiles in a grid)
  - Fetch from `HistoryManager` via IPC (most visited by frecency)
  - Display favicon + site name
  - Click → navigate to site
- [ ] Privacy stats line: "X trackers blocked today" (data from AdblockCache blocked counts)
- [ ] Dark background (#121212) matching Hodos brand
- [ ] `document.title = 'New Tab'`
- [ ] `document.body.style.margin = '0'`

**C++ changes**:
- [ ] Change `TabManager::CreateTab()` default URL from `metanetapps.com` to `http://127.0.0.1:5137/newtab`
- [ ] Add IPC handler `get_most_visited` → returns top N sites by visit count from HistoryManager
- [ ] Add `toDisplayUrl` mapping: `/newtab` → `hodos://newtab`

**Design decisions**:
- Background: solid dark (#121212) or subtle gradient/pattern? Start with solid, iterate.
- Number of tiles: 8 (2 rows of 4) is the sweet spot — Chrome uses 8-10, Brave uses 6
- Should tiles be removable? (Chrome lets you X them out — nice but Phase 2+)
- Should search bar auto-focus? (Yes — matches Chrome/Brave behavior)
- Favicon source: use Google's favicon service (`https://www.google.com/s2/favicons?domain=...&sz=64`) or cache locally?

### Phase 2: Homepage vs New Tab Separation

**Goal**: Let users independently configure homepage (browser launch) and new tab page behavior.

**Changes needed**:
- [ ] Add `browser.newTabPage` setting: `"default"` (Hodos new tab) | `"blank"` | `"homepage"` | custom URL
- [ ] Add UI in General settings: "New tab page" dropdown/selector
- [ ] Update `TabManager::CreateTab()` to read this setting
- [ ] Homepage setting controls browser launch URL (already works)
- [ ] New tab setting controls Ctrl+T / "+" button URL

**Options for new tab page setting**:
| Option | Behavior |
|--------|----------|
| Hodos New Tab (default) | Shows the branded new tab page with search + tiles |
| Blank page | Opens `about:blank` |
| Homepage | Opens whatever homepage is set to |
| Custom URL | Opens a user-specified URL |

### Phase 3: Right-Click "Set as Homepage"

**Goal**: Users can right-click on a tab or in the address bar area to set the current page as their homepage.

**Changes needed**:
- [ ] Add "Set as homepage" to tab context menu (right-click on tab)
- [ ] Add "Set as homepage" to page context menu (right-click on page background)
- [ ] Handler: read current tab URL → update `browser.homepage` setting via SettingsManager
- [ ] Show toast/confirmation: "Homepage set to example.com"

**C++ changes (Windows only for now)**:
- [ ] Add `MENU_ID_SET_AS_HOMEPAGE` to context menu in `simple_handler.cpp`
- [ ] In `OnContextMenuCommand`, read active tab URL and call `SettingsManager::SetHomepage()`
- [ ] Send IPC notification to header for toast display
- [ ] When implementing macOS support, update `development-docs/macos-port/MAC_PLATFORM_SUPPORT_PLAN.md` with equivalent context menu handling

### Phase 4: Polish & Customization (Optional/Future)

**Goal**: Let users customize the new tab page.

**Changes needed**:
- [ ] Remove individual tiles (click X)
- [ ] Add custom shortcut tiles
- [ ] Background image selection (upload or choose from presets)
- [ ] Toggle privacy stats on/off
- [ ] Toggle search bar on/off

---

## Architecture Considerations

**Most-visited data**: `HistoryManager` already tracks visit counts. Need a query like:
```sql
SELECT url, title, COUNT(*) as visits FROM history
GROUP BY url ORDER BY visits DESC LIMIT 8
```

**Favicon approach**:
- **Quick (Phase 1)**: Use Google's favicon API: `https://www.google.com/s2/favicons?domain=example.com&sz=64`
- **Better (Phase 2+)**: Cache favicons locally in profile directory
- **Privacy concern**: Google favicon API leaks visited sites to Google. For a privacy browser, local caching is preferred long-term.

**Search bar vs address bar**: The new tab search bar is separate from the toolbar address bar. Typing in either should work the same way. When the user starts typing in the new tab search bar, consider focusing the address bar instead (Chrome does this).

**Performance**: New tab page should load instantly. Pre-fetch most-visited data and cache it.

---

## Test Checklist

- [ ] Open new tab → Hodos new tab page appears with logo, search bar, tiles
- [ ] Click a tile → navigates to that site
- [ ] Type in search bar → searches with default engine (or navigates if URL)
- [ ] Most-visited tiles update as user browses
- [ ] Address bar shows `hodos://newtab`
- [ ] Tab title shows "New Tab"
- [ ] Privacy stats show accurate blocked count
- [ ] Homepage setting still controls browser launch separately
- [ ] Right-click tab → "Set as homepage" → setting updates
- [ ] Verify dark theme matches rest of browser

---

**Last Updated**: 2026-02-28
