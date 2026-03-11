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
| `WalletSettings` | Auto-approve toggle and spending/rate limits | `wallet.autoApproveEnabled`, `wallet.defaultPerTxLimitCents`, `wallet.defaultPerSessionLimitCents`, `wallet.defaultRateLimitPerMin` |
| `AboutSettings` | Static version info and project description | (none — read-only) |

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
- **Helper:** `formatRelativeTime()` converts timestamps to "Xm ago" / "Xh ago" / "Xd ago" strings
- **Notes:** Most complex component. Blocked domains show source tags ("Tracker" vs "User") with unblock buttons. Block log shows cookie domain, page URL (truncated at 80 chars), reason ("Domain" vs "3rd-party"), and relative timestamp.

### DownloadSettings
- **Hooks:** `useSettings()`
- **Cards:** Download Location (folder picker + save prompt toggle)
- **IPC:** `download_browse_folder` opens native OS folder picker; result arrives via `window.onDownloadFolderSelected` callback set by C++
- **Notes:** Declares global `window.onDownloadFolderSelected` for the C++ → JS callback. Cleans up on unmount.

### WalletSettings
- **Hooks:** `useSettings()`
- **Cards:** Auto-Approve (enable toggle), Spending Limits (per-tx cents, per-session cents, rate limit per minute)
- **Notes:** Uses native `<input type="number">` elements per CEF input guidelines (see root CLAUDE.md). Values are in cents — `formatCents()` helper converts to dollar display (e.g., `1000` → `$10.00`). The "Wallet" sidebar entry in `SettingsPage.tsx` has `externalAction: 'wallet'` so it opens the wallet overlay instead of rendering `WalletSettings` inline — this component is currently unused in production but exists for a future inline settings option.

### AboutSettings
- **Hooks:** None (static content)
- **Cards:** Version Information (browser, engine, wallet backend, protocol), About (description + link)
- **Notes:** Version is hardcoded as "Hodos Browser 1.0.0". No IPC calls.

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
       ├─ WalletSettings     (wallet.*)
       └─ AboutSettings      (static)
```

Settings are optimistically updated in local React state for responsiveness, then persisted by C++ backend.

## Patterns

- **Layout composition:** Every section uses `SettingsCard` > `SettingRow` for consistent spacing and styling.
- **Settings keys:** Dot-notation strings like `'privacy.adBlockEnabled'` passed to `updateSetting()` which splits on `.` to update the correct nested field.
- **Dark theme:** Hardcoded dark colors (`#121212` bg, `#1e1e1e` cards, `#a67c00` gold accent, `#e0e0e0` text, `#888` secondary text, `#333` borders). No theme provider — inline MUI `sx` props throughout.
- **Native inputs for number fields:** `WalletSettings` uses `<input type="number">` instead of MUI `TextField`, following CEF overlay input guidelines.
- **Collapsible sections:** PrivacySettings uses MUI `Collapse` with click-to-expand headers for blocked domains and block log lists.
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

## Related

- `../../../CLAUDE.md` — Root project docs, overlay architecture, CEF input patterns
- `../../pages/SettingsPage.tsx` — Parent page that renders these components via sidebar navigation
- `../../hooks/useSettings.ts` — Settings state hook (`AllSettings`, `BrowserSettings`, `PrivacySettings`, `WalletSettings` interfaces)
- `../../hooks/useCookieBlocking.ts` — Cookie blocking hook used by `PrivacySettings`
- `../../../CLAUDE.md` — See "CEF Input Patterns" section for why native inputs are used
