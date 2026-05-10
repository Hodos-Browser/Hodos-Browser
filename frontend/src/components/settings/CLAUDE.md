# Settings Components
> Section-level content panels for the full-page Settings overlay.

## Overview

These components render the content area of `SettingsPage.tsx` — each one corresponds to a sidebar tab in the settings overlay. They share a common layout system (`SettingsCard` + `SettingRow`) and get their state from the `useSettings()` hook, which communicates with the C++ backend via `cefMessage.send()` IPC.

The settings overlay runs as a separate CEF subprocess (see root CLAUDE.md overlay architecture). These components are never rendered inside `MainBrowserView.tsx`.

## Components

| Component | Purpose | Settings Keys Used |
|-----------|---------|-------------------|
| `SettingsCard` | Reusable card wrapper with gold title | (layout only) |
| `SettingRow` | Label + description + control row layout | (layout only) |
| `GeneralSettings` | Startup behavior, search engine selection | `browser.restoreSessionOnStart`, `browser.searchEngine` |
| `PrivacySettings` | Ad blocking, cookies, fingerprinting, DNT, browsing data | `privacy.adBlockEnabled`, `privacy.thirdPartyCookieBlocking`, `privacy.fingerprintProtection`, `privacy.doNotTrack`, `privacy.clearDataOnExit` |
| `DownloadSettings` | Download folder picker, save prompt toggle | `browser.downloadsPath`, `browser.askWhereToSave` |
| `AboutSettings` | Static version info and project description | (none — read-only) |

> **Note on Wallet defaults:** there is no `WalletSettings` section here. The "Wallet" sidebar entry in `SettingsPage.tsx` has `externalAction: 'wallet'` so clicking it opens the wallet overlay, where the canonical default-limits editor lives at `wallet/ApprovedSitesTab.tsx` (Default Limits section). A previous `WalletSettings.tsx` component was deleted in Phase 1.5 Step 0 (2026-05-09) — it had been written but never wired up, so removing it eliminated dead code and a potential drift surface.

## Exports

| File | Export | Type |
|------|--------|------|
| `SettingsCard.tsx` | `SettingsCard` | Named |
| `SettingsCard.tsx` | `SettingRow` | Named |
| `GeneralSettings.tsx` | `GeneralSettings` | Default |
| `PrivacySettings.tsx` | `PrivacySettings` | Default |
| `DownloadSettings.tsx` | `DownloadSettings` | Default |
| `AboutSettings.tsx` | `AboutSettings` | Default |

## Component Details

### SettingsCard
- **Purpose:** Dark-themed `Paper` wrapper with a gold-colored title heading.
- **Props:** `title: string`, `children: React.ReactNode`
- **Notes:** Used by all section components. Title can be empty string (used in PrivacySettings for collapsible sections).

### SettingRow
- **Purpose:** Horizontal row layout: label + optional description on left, control widget on right.
- **Props:** `label: string`, `description?: string`, `control: React.ReactNode`
- **Notes:** Bottom border separates rows; last child has no border. The `control` slot accepts any React element (Switch, Select, Button, native `<input>`).

### GeneralSettings
- **Hooks:** `useSettings()`
- **Cards:** Startup (session restore toggle), Search Engine (DuckDuckGo/Google select)
- **IPC:** `settings_set` via `updateSetting()`

### PrivacySettings
- **Hooks:** `useSettings()`, `useCookieBlocking()`
- **Cards:** Shields (ad blocking + 3rd-party cookies), Fingerprinting, Tracking (DNT), Blocked Domains (collapsible list), Block Log (collapsible list with clear), Browsing Data (clear on exit + link to history)
- **IPC:** `settings_set` via `updateSetting()`, `menu_action` with `['history']` for "Manage browsing data" link
- **Local state:** `domainsExpanded`, `logExpanded` for collapsible sections
- **Helper:** `formatRelativeTime()` — module-level function (not exported) that converts timestamps to "Just now" / "Xm ago" / "Xh ago" / "Xd ago" strings
- **Cookie blocking data:** Destructures `blockedDomains`, `blockLog`, `fetchBlockList`, `fetchBlockLog`, `clearBlockLog`, `unblockDomain` from `useCookieBlocking()`. Fetches block list and log (100 entries) on mount.
- **Notes:** Most complex component (217 lines). Blocked domains show source tags ("Tracker" for `default` source, "User" for manual) with unblock buttons. Block log shows cookie domain, page URL (truncated at 80 chars), reason ("Domain" for `blocked_domain`, "3rd-party" otherwise), and relative timestamp.

### DownloadSettings
- **Hooks:** `useSettings()`
- **Cards:** Download Location (folder picker + save prompt toggle)
- **IPC:** `download_browse_folder` opens native OS folder picker; result arrives via `window.onDownloadFolderSelected` callback set by C++
- **Global declaration:** Extends `Window` interface with optional `onDownloadFolderSelected?: (path: string) => void`
- **Notes:** Registers `window.onDownloadFolderSelected` on mount via `useCallback` + `useEffect`. Cleans up (sets to `undefined`) on unmount. The `handleBrowse` function guards on `window.cefMessage?.send` existence before calling.

### AboutSettings
- **Hooks:** None (static content)
- **Cards:** Version Information (browser, engine, wallet backend, protocol), About (description + link)
- **Notes:** Version is hardcoded as "Hodos Browser 1.0.0". Engine shows "Chromium (CEF 136)". No IPC calls. Does not import `SettingRow` — uses custom `Box` layout for key-value pairs.

## Data Flow

```
SettingsPage.tsx
  └─ useSettings()  ←→  C++ via cefMessage IPC
       │
       │  settings_get_all  →  C++ reads settings.json
       │  onSettingsResponse ←  C++ sends AllSettings object
       │  settings_set       →  C++ writes to settings.json + applies
       │
       ├─ GeneralSettings    (browser.*)
       ├─ PrivacySettings    (privacy.* + useCookieBlocking)
       ├─ DownloadSettings   (browser.downloads* + folder picker IPC)
       └─ AboutSettings      (static)
                              (Wallet sidebar entry is externalAction —
                               opens wallet overlay; defaults edited in
                               wallet/ApprovedSitesTab.tsx)
```

Settings are optimistically updated in local React state for responsiveness, then persisted by C++ backend.

## Patterns

- **Layout composition:** Every section uses `SettingsCard` > `SettingRow` for consistent spacing and styling. Exception: `AboutSettings` uses raw `Box` layout since it displays key-value pairs, not toggle/input rows.
- **Settings keys:** Dot-notation strings like `'privacy.adBlockEnabled'` passed to `updateSetting()` which splits on `.` to update the correct nested field.
- **Dark theme:** Hardcoded dark colors (`#121212` bg, `#1e1e1e` cards, `#a67c00` gold accent, `#e0e0e0` text, `#888` secondary text, `#333` borders). No theme provider — inline MUI `sx` props throughout.
- **Native inputs for number fields:** when a settings section needs numeric input, use `<input type="number">` instead of MUI `TextField`, following CEF overlay input guidelines. Style inline with focus/blur border color changes (see `DownloadSettings` for an example).
- **Collapsible sections:** PrivacySettings uses MUI `Collapse` with click-to-expand headers for blocked domains and block log lists. Expand icon rotates 180deg via CSS transition.
- **IPC callbacks:** DownloadSettings registers `window.onDownloadFolderSelected` — a pattern where C++ calls a globally-registered JS function after a native dialog completes.

## IPC Messages

| Message | Direction | Used By | Purpose |
|---------|-----------|---------|---------|
| `settings_get_all` | JS → C++ | `useSettings` hook | Load all settings on mount |
| `settings_set` | JS → C++ | All sections via `updateSetting()` | Persist a single setting change |
| `download_browse_folder` | JS → C++ | `DownloadSettings` | Open native folder picker dialog |
| `menu_action` | JS → C++ | `PrivacySettings` | Open history page (with arg `['history']`) |

| Callback | Direction | Used By | Purpose |
|----------|-----------|---------|---------|
| `window.onSettingsResponse` | C++ → JS | `useSettings` hook | Delivers full settings object |
| `window.onDownloadFolderSelected` | C++ → JS | `DownloadSettings` | Delivers selected folder path |

## MUI Dependencies

These components use MUI components where CEF compatibility allows:

| MUI Component | Used In | Notes |
|---------------|---------|-------|
| `Paper` | `SettingsCard` | Card container |
| `Typography` | All | Text rendering |
| `Box` | All | Layout |
| `Switch` | General, Privacy, Download | Toggle controls |
| `Select` + `MenuItem` | General | Search engine dropdown |
| `Button` | Download, Privacy | Folder browse, unblock, clear log |
| `Chip` | Privacy | Source tags on blocked domains and log entries |
| `Collapse` | Privacy | Expandable sections |
| `ExpandMoreIcon` | Privacy | Collapse toggle indicator |
| `OpenInNewIcon` | Privacy | "Manage browsing data" link |
| `DeleteSweepIcon` | Privacy | Clear block log button |
| `FolderOpenIcon` | Download | Browse button icon |

## Related

- `../../../CLAUDE.md` — Root project docs, overlay architecture, CEF input patterns
- `../../pages/SettingsPage.tsx` — Parent page that renders these components via sidebar navigation
- `../../hooks/useSettings.ts` — Settings state hook (`AllSettings`, `BrowserSettings`, `PrivacySettings`, `WalletSettings` interfaces — note the `WalletSettings` *interface* still exists in this hook; only the React component file of the same name was removed)
- `../../hooks/useCookieBlocking.ts` — Cookie blocking hook used by `PrivacySettings`
