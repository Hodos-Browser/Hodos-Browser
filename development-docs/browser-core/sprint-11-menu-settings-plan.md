# Sprint 11: Menu Button UX + Full-Page Settings — Implementation Plan

**Created**: 2026-02-25
**Status**: Planning
**Estimated Duration**: 3-4 days
**Dependencies**: Sprint 9 (Settings Persistence) complete, Sprint 10 (Scriptlet Compatibility) recommended

---

## Problem Statement

The current browser UX has two issues:

1. **No menu button**: Hodos Browser has no three-dot/hamburger menu. Users expect a central menu for browser actions (New Tab, Print, Zoom, Find, Developer Tools, Settings, etc.). Currently these are scattered across keyboard shortcuts, right-click context menus, and individual toolbar icons.

2. **Overlay settings panel**: The Settings UI (Sprint 9a) is a small overlay popup — limited space, not scalable for the growing number of settings. Every major browser uses a full-page settings tab (`chrome://settings`, `brave://settings`, `about:preferences`). Users expect this pattern.

---

## Research Summary

### Menu Button Patterns Across Browsers

All four major browsers (Chrome, Brave, Firefox, Edge) place the menu button as the **rightmost toolbar element** (or second-to-rightmost, before the profile avatar). The menu is a dropdown panel with:

- **Section groups** separated by dividers
- **Icons** alongside each item for accessibility
- **Keyboard shortcuts** displayed right-aligned (e.g., `Ctrl+T` next to "New tab")
- **Inline zoom controls** (not a submenu — a row with `-`, `100%`, `+`, and fullscreen button)
- **Submenus** indicated by right-pointing arrows (e.g., "More tools >")
- **Settings** always near the bottom, followed by **Exit**

**Common grouping order**:
1. Tab/Window creation (New Tab, New Window)
2. Content access (History, Bookmarks, Downloads)
3. Page actions (Print, Find, Zoom, Save)
4. Tools/Developer
5. Settings + Help + Exit

### Full-Page Settings Patterns

All browsers open settings as a **new tab** (not a popup/overlay):
- If a settings tab is already open, the browser **focuses it** instead of opening a duplicate
- URL routing enables deep-linking: `chrome://settings/privacy` goes directly to the Privacy section
- **Left sidebar** (~220-280px) lists categories with icons
- **Main content area** (~720-840px max-width, centered) shows settings cards
- **Search bar** at the top filters across all categories
- Settings are grouped into **cards** with section headings

### Brave's Additions to Chrome Settings

Brave adds several sidebar categories: Shields, Rewards, Social media blocking, Web3, Leo AI, Sync. This shows that browsers with custom features (like our BSV wallet) can add their own settings categories alongside standard ones.

---

## Architecture Decision

### Menu Button: Overlay Panel (Consistent with Existing Pattern)

The menu button will open as an **OSR overlay panel** (same pattern as Privacy Shield, Downloads, etc.). This is simpler than creating a new CEF subprocess and consistent with existing overlay architecture.

**Alternative considered**: Opening a native Win32 popup menu. Rejected because it wouldn't match the Hodos branding/theme and would require separate macOS implementation.

### Full-Page Settings: Tab-Based (React Route)

Settings will open as a **new tab** navigating to `http://127.0.0.1:5137/settings-page` (dev) or equivalent internal URL. This uses the existing tab infrastructure with React Router for section navigation.

**Key decisions**:
- The settings **overlay** (Sprint 9a) is **retired** — replaced by the full-page settings tab
- The settings gear icon in the toolbar is **removed** — settings is accessed via the menu button
- The three-dot menu button **replaces** the settings icon position in the toolbar
- History and Downloads toolbar icons are **removed** — accessed via menu (reduces toolbar clutter)

---

## Implementation Plan

### 11a: Three-Dot Menu Button + Overlay Panel (Day 1, ~6 hours)

**Goal**: Add a three-dot menu button to the toolbar that opens a dropdown panel with standard browser actions.

#### Step 1: Add Menu Button to MainBrowserView

**File**: `frontend/src/pages/MainBrowserView.tsx`

Replace the Settings gear icon with a three-dot menu icon (MUI `MoreVertIcon`). Remove dedicated History and Downloads toolbar icons (they'll move into the menu).

```tsx
// Toolbar layout (simplified):
// [Back] [Forward] [Refresh] [Address Bar] [Shield] [Menu ⋮] [Profile]

<IconButton onClick={handleMenuClick} title="Menu">
    <MoreVertIcon />
</IconButton>
```

#### Step 2: Create MenuOverlay Component

**File**: `frontend/src/components/MenuOverlay.tsx` (NEW)

The menu is a React component rendered inside an overlay panel. It follows the universal browser menu pattern:

```tsx
const MenuOverlay = ({ onClose, onAction }) => {
    return (
        <Box sx={{
            width: 280,
            bgcolor: '#1e1e1e',
            color: '#e0e0e0',
            py: 0.5,
            borderRadius: 1,
        }}>
            {/* Section 1: Tab/Window */}
            <MenuItem icon={<AddIcon />} label="New Tab" shortcut="Ctrl+T"
                onClick={() => onAction('new_tab')} />
            <MenuItem icon={<OpenInNewIcon />} label="New Window" shortcut="Ctrl+N"
                onClick={() => onAction('new_window')} />

            <Divider />

            {/* Section 2: Content Access */}
            <MenuItem icon={<HistoryIcon />} label="History" shortcut="Ctrl+H"
                onClick={() => onAction('history')} />
            <MenuItem icon={<BookmarkIcon />} label="Bookmarks" shortcut="Ctrl+D"
                onClick={() => onAction('bookmarks')} />
            <MenuItem icon={<DownloadIcon />} label="Downloads" shortcut="Ctrl+J"
                onClick={() => onAction('downloads')} />

            <Divider />

            {/* Section 3: Page Actions */}
            <ZoomRow currentZoom={zoom} onZoomIn={...} onZoomOut={...}
                onReset={...} onFullscreen={...} />
            <MenuItem icon={<PrintIcon />} label="Print..." shortcut="Ctrl+P"
                onClick={() => onAction('print')} />
            <MenuItem icon={<SearchIcon />} label="Find in Page" shortcut="Ctrl+F"
                onClick={() => onAction('find')} />

            <Divider />

            {/* Section 4: Tools */}
            <SubmenuItem icon={<BuildIcon />} label="More Tools">
                <MenuItem label="Developer Tools" shortcut="F12"
                    onClick={() => onAction('devtools')} />
                <MenuItem label="View Page Source" shortcut="Ctrl+U"
                    onClick={() => onAction('view_source')} />
                <MenuItem label="Task Manager" shortcut="Shift+Esc"
                    onClick={() => onAction('task_manager')} />
            </SubmenuItem>

            <Divider />

            {/* Section 5: Settings + Exit */}
            <MenuItem icon={<SettingsIcon />} label="Settings"
                onClick={() => onAction('settings')} />
            <MenuItem icon={<HelpIcon />} label="Help"
                onClick={() => onAction('help')} />
            <MenuItem icon={<ExitIcon />} label="Exit"
                onClick={() => onAction('exit')} />
        </Box>
    );
};
```

#### Step 3: Inline Zoom Controls

The zoom row is a special menu item with inline controls (matching Chrome/Brave/Firefox pattern):

```tsx
const ZoomRow = ({ currentZoom, onZoomIn, onZoomOut, onReset, onFullscreen }) => (
    <Box sx={{ display: 'flex', alignItems: 'center', px: 2, py: 0.5, height: 36 }}>
        <ZoomOutIcon sx={{ mr: 1, fontSize: 18 }} />
        <IconButton size="small" onClick={onZoomOut}>
            <RemoveIcon fontSize="small" />
        </IconButton>
        <Typography sx={{ mx: 1, minWidth: 40, textAlign: 'center', fontSize: 13 }}>
            {currentZoom}%
        </Typography>
        <IconButton size="small" onClick={onZoomIn}>
            <AddIcon fontSize="small" />
        </IconButton>
        <Box sx={{ flex: 1 }} />
        <IconButton size="small" onClick={onFullscreen}>
            <FullscreenIcon fontSize="small" />
        </IconButton>
    </Box>
);
```

#### Step 4: Menu Action Dispatch

Each menu action sends an IPC message to C++ or triggers a React action:

| Action | Implementation |
|--------|---------------|
| `new_tab` | `cefMessage.send('tab_create', ['about:blank'])` |
| `new_window` | `cefMessage.send('new_window', [])` (new IPC) |
| `history` | `cefMessage.send('tab_create', ['http://127.0.0.1:5137/history'])` |
| `bookmarks` | `cefMessage.send('tab_create', ['http://127.0.0.1:5137/bookmarks'])` (new route) |
| `downloads` | `cefMessage.send('download_panel_show', [offset])` |
| `print` | `cefMessage.send('print', [])` (new IPC → active tab `CefBrowserHost::Print()`) |
| `find` | Toggle FindBar via existing React state |
| `devtools` | `cefMessage.send('devtools', [])` (new IPC → active tab `ShowDevTools()`) |
| `view_source` | `cefMessage.send('view_source', [])` (new IPC) |
| `settings` | `cefMessage.send('tab_create', ['http://127.0.0.1:5137/settings-page'])` |
| `exit` | `cefMessage.send('exit', [])` (new IPC → `PostQuitMessage(0)`) |
| `zoom_in/out/reset` | `cefMessage.send('zoom_in/out/reset', [])` |

#### Step 5: C++ IPC Handlers

**File**: `cef-native/src/handlers/simple_handler.cpp`

Add new IPC handlers for actions that need C++ involvement:

```cpp
// New IPC handlers
if (message_name == "new_window") {
    // Launch new browser instance
    LaunchNewInstance();
}
else if (message_name == "print") {
    auto tab = TabManager::GetInstance().GetActiveTab();
    if (tab && tab->browser) {
        tab->browser->GetHost()->Print();
    }
}
else if (message_name == "devtools") {
    auto tab = TabManager::GetInstance().GetActiveTab();
    if (tab && tab->browser) {
        CefWindowInfo windowInfo;
        windowInfo.SetAsPopup(nullptr, "Developer Tools");
        CefBrowserSettings settings;
        tab->browser->GetHost()->ShowDevTools(windowInfo, nullptr, settings, CefPoint());
    }
}
else if (message_name == "zoom_in") {
    auto tab = TabManager::GetInstance().GetActiveTab();
    if (tab && tab->browser) {
        double level = tab->browser->GetHost()->GetZoomLevel();
        tab->browser->GetHost()->SetZoomLevel(level + 0.5);
    }
}
else if (message_name == "zoom_out") {
    auto tab = TabManager::GetInstance().GetActiveTab();
    if (tab && tab->browser) {
        double level = tab->browser->GetHost()->GetZoomLevel();
        tab->browser->GetHost()->SetZoomLevel(level - 0.5);
    }
}
else if (message_name == "zoom_reset") {
    auto tab = TabManager::GetInstance().GetActiveTab();
    if (tab && tab->browser) {
        tab->browser->GetHost()->SetZoomLevel(0.0);
    }
}
else if (message_name == "exit") {
#ifdef _WIN32
    PostMessage(g_hwnd, WM_CLOSE, 0, 0);
#elif defined(__APPLE__)
    // TODO: macOS quit
#endif
}
```

#### Step 6: Menu Overlay HWND/Panel

**Option A (Recommended)**: Render the menu as a React component inside the **header browser** using absolute positioning, rather than creating a new overlay HWND. This is simpler and avoids the complexity of another OSR overlay.

```tsx
// In MainBrowserView.tsx
{menuOpen && (
    <Box sx={{
        position: 'absolute',
        top: 48, // below toolbar
        right: 8, // aligned to menu button
        zIndex: 1000,
        boxShadow: 4,
    }}>
        <MenuOverlay onClose={() => setMenuOpen(false)} onAction={handleMenuAction} />
    </Box>
)}
```

Click-outside dismiss: use a backdrop element or MUI's `ClickAwayListener`.

**Option B**: Create a new OSR overlay (like Privacy Shield). More isolated but adds HWND complexity.

**Decision**: Use Option A for simplicity. The menu doesn't need process isolation (it's trusted UI, not web content).

#### Verification Checklist (11a)

- [ ] Three-dot icon appears in toolbar
- [ ] Click opens menu dropdown
- [ ] New Tab action creates a new tab
- [ ] History action opens history in a new tab
- [ ] Downloads action opens download panel overlay
- [ ] Zoom +/- changes active tab zoom
- [ ] Print opens print dialog
- [ ] Find activates find bar
- [ ] Developer Tools opens DevTools window
- [ ] Settings opens settings page (see 11b)
- [ ] Exit closes the browser
- [ ] Click outside menu → dismisses
- [ ] Escape key → dismisses
- [ ] Keyboard shortcut labels display correctly

---

### 11b: Full-Page Settings Tab (Day 2-3, ~8 hours)

**Goal**: Replace the overlay-based settings with a full-page settings tab matching the Chrome/Brave/Firefox pattern.

#### Step 1: Create SettingsPage Component

**File**: `frontend/src/pages/SettingsPage.tsx` (NEW)

Full-page settings with sidebar navigation and content area:

```tsx
const SettingsPage = () => {
    const [activeSection, setActiveSection] = useState('browser');
    const [searchQuery, setSearchQuery] = useState('');

    const sections = [
        { id: 'browser', label: 'General', icon: <LanguageIcon /> },
        { id: 'privacy', label: 'Privacy & Security', icon: <ShieldIcon /> },
        { id: 'appearance', label: 'Appearance', icon: <PaletteIcon /> },
        { id: 'search', label: 'Search Engine', icon: <SearchIcon /> },
        { id: 'downloads', label: 'Downloads', icon: <DownloadIcon /> },
        { id: 'wallet', label: 'Wallet', icon: <AccountBalanceWalletIcon /> },
        { id: 'import', label: 'Import Data', icon: <UploadIcon /> },
        { id: 'profiles', label: 'Profiles', icon: <PeopleIcon /> },
        { id: 'about', label: 'About Hodos', icon: <InfoIcon /> },
    ];

    return (
        <Box sx={{ display: 'flex', height: '100vh', bgcolor: '#121212', color: '#e0e0e0' }}>
            {/* Sidebar */}
            <Box sx={{ width: 240, borderRight: '1px solid #333', py: 2, overflowY: 'auto' }}>
                {/* Search */}
                <TextField
                    placeholder="Search settings"
                    value={searchQuery}
                    onChange={(e) => setSearchQuery(e.target.value)}
                    size="small"
                    sx={{ mx: 2, mb: 2, width: 'calc(100% - 32px)' }}
                    InputProps={{ startAdornment: <SearchIcon fontSize="small" /> }}
                />
                {/* Category list */}
                <List>
                    {sections.map(section => (
                        <ListItemButton
                            key={section.id}
                            selected={activeSection === section.id}
                            onClick={() => setActiveSection(section.id)}
                            sx={{
                                borderRadius: 1,
                                mx: 1,
                                '&.Mui-selected': {
                                    bgcolor: 'rgba(166, 124, 0, 0.15)',
                                    color: '#a67c00',
                                },
                            }}
                        >
                            <ListItemIcon sx={{ minWidth: 36, color: 'inherit' }}>
                                {section.icon}
                            </ListItemIcon>
                            <ListItemText primary={section.label} />
                        </ListItemButton>
                    ))}
                </List>
            </Box>

            {/* Main Content */}
            <Box sx={{ flex: 1, overflowY: 'auto', p: 4, maxWidth: 780, mx: 'auto' }}>
                {activeSection === 'browser' && <GeneralSettings />}
                {activeSection === 'privacy' && <PrivacySettings />}
                {activeSection === 'appearance' && <AppearanceSettings />}
                {activeSection === 'search' && <SearchSettings />}
                {activeSection === 'downloads' && <DownloadSettings />}
                {activeSection === 'wallet' && <WalletSettings />}
                {activeSection === 'import' && <ImportSettings />}
                {activeSection === 'profiles' && <ProfileSettings />}
                {activeSection === 'about' && <AboutSettings />}
            </Box>
        </Box>
    );
};
```

#### Step 2: Settings Section Components

Each section is a standalone component rendering settings as cards:

**GeneralSettings** (`frontend/src/components/settings/GeneralSettings.tsx`):
- Homepage URL input
- Startup behavior (blank page / restore session / custom page)
- Show bookmark bar toggle

**PrivacySettings** (`frontend/src/components/settings/PrivacySettings.tsx`):
- Ad blocking toggle (global)
- Scriptlet injection toggle (global)
- Third-party cookie blocking toggle
- Do Not Track toggle
- Clear browsing data button → opens ClearDataDialog
- Site permissions management link

**AppearanceSettings** (`frontend/src/components/settings/AppearanceSettings.tsx`):
- Theme (Dark only for MVP — future: Light/System)
- Default zoom level slider
- Font size (future)

**SearchSettings** (`frontend/src/components/settings/SearchSettings.tsx`):
- Default search engine dropdown (Google, Bing, DuckDuckGo, Brave Search)
- Search suggestions toggle

**DownloadSettings** (`frontend/src/components/settings/DownloadSettings.tsx`):
- Default download location
- Ask where to save each file toggle

**WalletSettings** (`frontend/src/components/settings/WalletSettings.tsx`):
- Auto-approve enabled toggle
- Default per-transaction spending limit
- Default per-session spending limit
- Default rate limit
- Domain permissions management (existing `DomainPermissionsTab` component)

**ImportSettings** (`frontend/src/components/settings/ImportSettings.tsx`):
- Migrated from current `SettingsOverlayRoot.tsx` Import tab
- Auto-detect Chrome/Brave/Edge profiles
- Import bookmarks/history buttons

**ProfileSettings** (`frontend/src/components/settings/ProfileSettings.tsx`):
- Migrated from Sprint 9d profile management
- Create/rename/delete/recolor profiles
- Profile picker on startup toggle

**AboutSettings** (`frontend/src/components/settings/AboutSettings.tsx`):
- Browser version
- CEF/Chromium version
- Build date
- Hodos logo
- Links to project website, privacy policy

#### Step 3: Settings Card Pattern

All settings sections use a consistent card-based layout:

```tsx
const SettingsCard = ({ title, children }) => (
    <Paper sx={{ p: 3, mb: 3, bgcolor: '#1e1e1e', borderRadius: 2 }}>
        <Typography variant="h6" sx={{ mb: 2, color: '#a67c00' }}>
            {title}
        </Typography>
        {children}
    </Paper>
);

const SettingRow = ({ label, description, control }) => (
    <Box sx={{
        display: 'flex',
        justifyContent: 'space-between',
        alignItems: 'center',
        py: 1.5,
        borderBottom: '1px solid #333',
        '&:last-child': { borderBottom: 'none' },
    }}>
        <Box>
            <Typography variant="body1">{label}</Typography>
            {description && (
                <Typography variant="body2" sx={{ color: '#888', mt: 0.25 }}>
                    {description}
                </Typography>
            )}
        </Box>
        {control}
    </Box>
);
```

#### Step 4: Add Route

**File**: `frontend/src/App.tsx`

```tsx
<Route path="/settings-page" element={<SettingsPage />} />
<Route path="/settings-page/:section" element={<SettingsPage />} />
```

The optional `:section` parameter enables deep-linking (e.g., `/settings-page/privacy`).

#### Step 5: Ensure Single Settings Tab

**File**: `cef-native/src/handlers/simple_handler.cpp` or `frontend/src/pages/MainBrowserView.tsx`

When the user clicks "Settings" in the menu:
1. Check if a tab with URL containing `/settings-page` is already open
2. If yes, focus that tab
3. If no, create a new tab

```tsx
const handleOpenSettings = () => {
    // Check existing tabs via IPC or tab state
    const existingSettingsTab = tabs.find(t => t.url.includes('/settings-page'));
    if (existingSettingsTab) {
        cefMessage.send('tab_activate', [existingSettingsTab.id]);
    } else {
        cefMessage.send('tab_create', ['http://127.0.0.1:5137/settings-page']);
    }
};
```

#### Step 6: Retire Settings Overlay

- Remove `g_settings_overlay_hwnd` and related HWND code from `cef_browser_shell.cpp`
- Remove `CreateSettingsOverlay`/`ShowSettingsOverlay`/`HideSettingsOverlay` from `simple_app.cpp`
- Keep `SettingsOverlayRoot.tsx` but mark as deprecated (or delete if no longer referenced)
- Update the existing `settings_panel_show` IPC handler to open a settings tab instead

**Note**: The overlay can be removed gradually. For Sprint 11, both can coexist during transition.

#### Verification Checklist (11b)

- [ ] Menu → Settings opens a new tab with settings page
- [ ] Settings page has sidebar with all categories
- [ ] Clicking sidebar categories switches the content area
- [ ] Deep-link `/settings-page/privacy` goes directly to Privacy section
- [ ] If settings tab already open, clicking Settings focuses it
- [ ] All setting changes persist (use existing `useSettings` hook and `SettingsManager` IPC)
- [ ] Search bar filters settings across categories (bonus — can defer)
- [ ] Hodos branding (gold accent color) applied throughout
- [ ] Responsive at various window sizes (sidebar collapses at narrow widths — bonus)

---

### 11c: Settings Wiring — Making Settings Actually Work (Day 3-4, ~4 hours)

**Goal**: Connect settings to actual browser behavior. Currently settings persist to JSON but don't change behavior.

This addresses the items documented in `working-notes.md #11`.

#### Priority Settings to Wire

| Setting | Where to Wire | How |
|---------|--------------|-----|
| `homepage` | `simple_handler.cpp` - new tab creation | When creating blank tab, load homepage URL instead of `about:blank` |
| `searchEngine` | `GoogleSuggestService` + search URL | Update search query URL prefix based on engine choice |
| `zoomLevel` | `simple_handler.cpp` - `OnAfterCreated` | `browser->GetHost()->SetZoomLevel(level)` for new tabs |
| `downloadsPath` | `simple_handler.cpp` - `OnBeforeDownload` | Set `suggested_name` parent directory |
| `doNotTrack` | `HttpRequestInterceptor.cpp` | Add `DNT: 1` header to all outgoing requests |
| `clearDataOnExit` | `cef_browser_shell.cpp` - shutdown | Call clear functions before CEF shutdown |

#### Implementation Notes

**Homepage**: Modify `tab_create` IPC handler:
```cpp
if (url == "about:blank" || url.empty()) {
    std::string homepage = SettingsManager::GetInstance().GetBrowserSettings().homepage;
    if (!homepage.empty() && homepage != "about:blank") {
        url = homepage;
    }
}
```

**Search Engine**: Modify `GoogleSuggestService` to use configurable search URL:
```cpp
std::string GetSearchUrl(const std::string& query) {
    auto& settings = SettingsManager::GetInstance().GetBrowserSettings();
    if (settings.searchEngine == "duckduckgo") {
        return "https://duckduckgo.com/?q=" + UrlEncode(query);
    } else if (settings.searchEngine == "bing") {
        return "https://www.bing.com/search?q=" + UrlEncode(query);
    } else if (settings.searchEngine == "brave") {
        return "https://search.brave.com/search?q=" + UrlEncode(query);
    }
    return "https://www.google.com/search?q=" + UrlEncode(query);
}
```

**Do Not Track**: In `HttpRequestInterceptor.cpp`, before forwarding requests:
```cpp
if (SettingsManager::GetInstance().GetPrivacySettings().doNotTrack) {
    request->SetHeaderByName("DNT", "1", true);
    request->SetHeaderByName("Sec-GPC", "1", true);
}
```

#### Verification Checklist (11c)

- [ ] Change homepage to `https://example.com` → new tabs open that URL
- [ ] Change search engine to DuckDuckGo → omnibox searches use DDG
- [ ] Change default zoom → new tabs open at that zoom level
- [ ] Enable DNT → check request headers in DevTools for `DNT: 1`
- [ ] Enable clear-on-exit → close browser → reopen → history/cookies cleared

---

## Files Changed Summary

| File | Changes |
|------|---------|
| **NEW** `frontend/src/components/MenuOverlay.tsx` | Three-dot menu dropdown component |
| **NEW** `frontend/src/pages/SettingsPage.tsx` | Full-page settings tab |
| **NEW** `frontend/src/components/settings/GeneralSettings.tsx` | General settings section |
| **NEW** `frontend/src/components/settings/PrivacySettings.tsx` | Privacy & Security section |
| **NEW** `frontend/src/components/settings/AppearanceSettings.tsx` | Appearance section |
| **NEW** `frontend/src/components/settings/SearchSettings.tsx` | Search engine section |
| **NEW** `frontend/src/components/settings/DownloadSettings.tsx` | Downloads section |
| **NEW** `frontend/src/components/settings/WalletSettings.tsx` | Wallet settings section |
| **NEW** `frontend/src/components/settings/ImportSettings.tsx` | Import data section |
| **NEW** `frontend/src/components/settings/ProfileSettings.tsx` | Profile management section |
| **NEW** `frontend/src/components/settings/AboutSettings.tsx` | About Hodos section |
| `frontend/src/pages/MainBrowserView.tsx` | Replace settings icon with three-dot menu, remove History/Downloads icons |
| `frontend/src/App.tsx` | Add `/settings-page` and `/settings-page/:section` routes |
| `cef-native/src/handlers/simple_handler.cpp` | New IPC handlers (print, devtools, zoom, exit, new_window, view_source) |
| `cef-native/src/core/GoogleSuggestService.cpp` | Configurable search engine URL |
| `cef-native/src/core/HttpRequestInterceptor.cpp` | DNT header injection |
| `cef-native/cef_browser_shell.cpp` | Clear-on-exit logic, settings overlay retirement (gradual) |

---

## Cross-Platform Notes

- **Menu component**: Pure React, cross-platform.
- **Settings page**: Pure React, cross-platform.
- **New IPC handlers**: `Print()`, `ShowDevTools()`, `SetZoomLevel()` are all CEF cross-platform APIs. `PostMessage(WM_CLOSE)` needs `#ifdef _WIN32` / `#elif defined(__APPLE__)`.
- **DNT header**: `CefRequest::SetHeaderByName` is cross-platform CEF API.
- **Settings overlay retirement**: Separate HWND cleanup per platform.

---

## Design Notes

### Hodos Branding

- Background: `#121212` (dark)
- Card background: `#1e1e1e`
- Primary accent: `#a67c00` (gold) — used for selected sidebar items, toggles, buttons
- Text: `#e0e0e0` (primary), `#888` (secondary/descriptions)
- Dividers: `#333`

### Menu Width & Position

- Width: 280px (matches Chrome's menu width)
- Position: Anchored to the three-dot button, right-aligned
- Max height: viewport height - 80px (with scrollbar if needed)
- Shadow: MUI elevation 4

### Settings Page Dimensions

- Sidebar: 240px fixed width
- Content: max-width 780px, centered in remaining space
- Card padding: 24px
- Setting row height: ~56px

---

## Risk Mitigation

| Risk | Impact | Mitigation |
|------|--------|------------|
| Menu overlay interferes with header bar input | Can't type in address bar | Use MUI ClickAwayListener, not a full backdrop |
| Settings page doesn't fit in small tabs | Layout breaks | Set min-width on settings page, test at 800px |
| Removing toolbar icons confuses users | Can't find History/Downloads | Keep keyboard shortcuts (Ctrl+H, Ctrl+J), add to menu prominently |
| Settings overlay retirement breaks existing flows | Settings not accessible | Keep both during transition, remove overlay after full-page settings verified |
| Deep-linking race condition | Settings loads at wrong section | Use React Router `useParams` for section, not state |

---

## Post-Sprint Tasks

1. Update `development-docs/browser-core/CLAUDE.md` with Sprint 11 completion
2. Update `00-SPRINT-INDEX.md` status
3. Update root `CLAUDE.md` Key Files table (add SettingsPage.tsx, MenuOverlay.tsx)
4. Test against Standard basket (15 min)
5. Consider: Bookmark bar toggle (from settings) — does the bookmark bar UI exist yet?
6. Consider: Keyboard shortcut for menu button (`Alt+F` like Chrome, or `F10`)

---

*This document was generated based on research into Chrome, Brave, Firefox, and Edge menu button UX patterns, full-page settings architecture, and analysis of the existing Hodos Browser codebase.*
