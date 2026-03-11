# Wallet Tab Components
> Tab content panels for the wallet overlay sidebar navigation

## Overview

These components render the content area for each tab in the wallet overlay panel. They are composed by `WalletPanelPage.tsx` (the overlay host) and switched via `WalletSidebar`. Each tab fetches its own data directly from the Rust wallet backend at `http://127.0.0.1:31301`.

All components use plain CSS classes prefixed with `wd-` (wallet dashboard). No MUI — this follows the CEF overlay rule of using native HTML elements for reliable focus/input handling.

## Components

| Component | File | Purpose |
|-----------|------|---------|
| `WalletSidebar` | `WalletSidebar.tsx` | Navigation sidebar with 5 tabs and Hodos logo |
| `DashboardTab` | `DashboardTab.tsx` | Balance display, receive addresses, send form, recent activity |
| `ActivityTab` | `ActivityTab.tsx` | Full transaction history with pagination and filters |
| `CertificatesTab` | `CertificatesTab.tsx` | BRC-52 identity certificate viewer |
| `ApprovedSitesTab` | `ApprovedSitesTab.tsx` | Domain permission defaults and per-site management |
| `SettingsTab` | `SettingsTab.tsx` | Display name, security keys, rescan, backup, wallet deletion |

## Component Details

### WalletSidebar

- **Props:** `activeTab: number`, `onTabChange: (tabId: number) => void`
- **Exports:** `WalletTab` interface (`{ id: number; label: string; icon: string }`)
- **Tab IDs:** 0=Dashboard, 1=Activity, 2=Certificates, 3=Approved Sites, 4=Settings
- **Contains:** `HodosWalletLogo` SVG component (inline, gold pinwheel + "HODOS WALLET" text)

### DashboardTab

The main wallet view, split into left and right columns.

- **Props:** `onNavigateToActivity: () => void` — callback to switch to Activity tab
- **Left column:**
  - **Balance card** — fetches `/wallet/balance`, shows USD primary + BSV secondary, 10s polling interval (only updates state if values change to avoid re-renders)
  - **PeerPay notification banner** — polls `/wallet/peerpay/status` every 60s, auto-refreshes balance on new incoming payments, dismiss calls `/wallet/peerpay/dismiss` + sends `wallet_payment_dismissed` IPC
  - **Receive section** — split left/right: Identity Key (from `localStorage`) with QR + copy, and Legacy Address (from `/wallet/address/current`) with QR + copy + "New Address" button (`/wallet/address/generate`)
  - **Recent activity** — fetches `/wallet/activity?page=1&limit=5&filter=all`, shows last 5 transactions with relative timestamps
- **Right column:**
  - **Send form** — renders memoized `<TransactionForm>` from `../TransactionForm`; transaction results show success/error banner with WhatsOnChain link (opens via `tab_create` IPC)
- **Local helpers:** `InfoTooltip` (click-to-open tooltip with outside-click dismiss), `formatBsv`, `formatUsd`, `formatUsdCents`, `formatTime`

### ActivityTab

Full paginated transaction history.

- **Props:** none
- **State:** page (1-indexed), filter (`'all' | 'sent' | 'received'`), 10 items per page
- **Endpoint:** `/wallet/activity?page={p}&limit=10&filter={f}` — returns `ActivityResponse` with items, total, page, page_size, current_price_usd_cents
- **USD display:** Uses historical `price_usd_cents` per transaction when available, falls back to current price; shows "(now: $X.XX)" secondary when historical and current differ
- **Actions per item:** Copy TxID button, "View on WhatsOnChain" button (opens via `tab_create` IPC)
- **Interfaces:** `ActivityItem` (txid, direction, satoshis, status, timestamp, description, labels, price_usd_cents, source), `ActivityResponse`, `DirectionFilter`

### CertificatesTab

Read-only BRC-52 identity certificate viewer.

- **Props:** none
- **Endpoint:** `POST /listCertificates` with `{ limit: 100, offset: 0 }`
- **Display:** Table with columns: Type (base64-decoded), Certifier (truncated hash), Subject (truncated hash), Fields count, Serial number
- **Interface:** `Certificate` (type, serial_number, subject, certifier, revocation_outpoint, signature, fields: Record, keyring: Record)

### ApprovedSitesTab

Domain permission management — default limits and per-site overrides.

- **Props:** none
- **Two sections:**
  1. **Default Limits** — fetches/saves to `/wallet/settings` (fields: `default_per_tx_limit_cents`, `default_per_session_limit_cents`, `default_rate_limit_per_min`). Inputs are in USD, stored as cents.
  2. **Per-Site Permissions** — embeds `<DomainPermissionsTab>` from `../DomainPermissionsTab` (MUI-based component — exception to no-MUI rule since it was built earlier)
- **Reset All** — confirmation modal, calls `POST /domain/permissions/reset-all` with current default values
- **Interface:** `DefaultLimits` (defaultPerTxLimitCents, defaultPerSessionLimitCents, defaultRateLimitPerMin)

### SettingsTab

Wallet configuration and danger-zone operations.

- **Props:** none
- **Sections:**
  1. **Display Name** — fetches/saves `sender_display_name` via `/wallet/settings` (GET/POST)
  2. **Security & Keys:**
     - Identity Key — toggle show/hide, copy to clipboard. Fetched via `POST /getPublicKey` with `{ identityKey: true }`
     - Recovery Phrase — PIN-gated reveal via `POST /wallet/reveal-mnemonic`. Shows numbered word grid. PIN must be 4+ digits.
  3. **Wallet Rescan** — `POST /wallet/rescan`, shows results: addresses scanned, new addresses found, new UTXOs found, balance
  4. **Export Backup** — `POST /wallet/export` with password (8+ chars). Downloads `.hodos-wallet` JSON file via blob URL
  5. **Danger Zone** — Two-step wallet deletion: type "DELETE" → enter PIN → `POST /wallet/unlock` to verify → `POST /wallet/delete`. Shows balance warning if funds remain. Calls `window.close()` on success.

## Wallet Backend Endpoints Used

| Endpoint | Method | Used By |
|----------|--------|---------|
| `/wallet/balance` | GET | DashboardTab, SettingsTab |
| `/wallet/address/current` | GET | DashboardTab |
| `/wallet/address/generate` | POST | DashboardTab |
| `/wallet/activity` | GET | DashboardTab, ActivityTab |
| `/wallet/peerpay/status` | GET | DashboardTab |
| `/wallet/peerpay/dismiss` | POST | DashboardTab |
| `/wallet/settings` | GET/POST | ApprovedSitesTab, SettingsTab |
| `/wallet/reveal-mnemonic` | POST | SettingsTab |
| `/wallet/rescan` | POST | SettingsTab |
| `/wallet/export` | POST | SettingsTab |
| `/wallet/unlock` | POST | SettingsTab (delete verification) |
| `/wallet/delete` | POST | SettingsTab |
| `/getPublicKey` | POST | SettingsTab |
| `/listCertificates` | POST | CertificatesTab |
| `/domain/permissions/reset-all` | POST | ApprovedSitesTab |

## Patterns

### Data Fetching
All tabs use `useCallback` + `useEffect` for initial fetch. DashboardTab adds polling intervals (10s for balance, 60s for PeerPay). Balance polling uses refs to compare values and skip state updates when unchanged, preventing unnecessary re-renders.

### CEF IPC
External links open via `(window as any).cefMessage.send('tab_create', url)`. PeerPay dismiss sends `wallet_payment_dismissed` IPC. Components cast `window` to `any` since `cefMessage` is injected by V8 at runtime.

### USD Price Display
Three price contexts exist:
1. **Live price** — from `/wallet/balance` response (`bsvPrice` field)
2. **Historical per-tx price** — `price_usd_cents` on each activity item (recorded at transaction time)
3. **Current price in cents** — `current_price_usd_cents` on activity response (for fallback)

ActivityTab shows historical price as primary and current price as secondary "(now: ...)" when they differ.

### CSS Class Convention
All classes use `wd-` prefix. Styled via `WalletPanel.css` (imported by parent). No inline MUI — native HTML elements throughout (except `DomainPermissionsTab` embedded in ApprovedSitesTab).

## Related

- **Parent page:** `frontend/src/pages/WalletPanelPage.tsx` — overlay host, PIN entry, mnemonic setup, tab switching
- **Legacy panel:** `frontend/src/components/WalletPanel.tsx` — older single-panel wallet (still exists, uses hooks)
- **Transaction form:** `frontend/src/components/TransactionForm.tsx` — send form embedded in DashboardTab
- **Domain permissions:** `frontend/src/components/DomainPermissionsTab.tsx` — MUI table embedded in ApprovedSitesTab
- **Frontend CLAUDE.md:** `frontend/CLAUDE.md` — frontend layer conventions and entry points
- **Root CLAUDE.md:** `CLAUDE.md` — project architecture, overlay lifecycle, CEF input patterns
