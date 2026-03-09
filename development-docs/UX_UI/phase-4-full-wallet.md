# Phase 4: Advanced Wallet Dashboard — Implementation Summary

**Date**: March 8, 2026
**Project**: Hodos Browser
**Phase**: 4 - Full Wallet Dashboard
**Status**: IMPLEMENTED — Needs Testing & Refinement

---

## 1. Overview

Phase 4 replaced the old MUI-tab wallet overlay (786 lines in `WalletOverlayRoot.tsx`) with a sidebar + content dashboard layout. The wallet now has five navigable sections with a dark theme, lazy-loaded tabs, and deep-linking support.

---

## 2. What Was Built

### Sprint 4.1: Layout Shell + Dashboard
- `WalletSidebar.tsx` — Left sidebar navigation (5 tabs)
- `DashboardTab.tsx` — Balance card, QR receive, send form (TransactionForm), recent activity (5 most recent)
- Sidebar uses gold highlight on active tab, compact icon+label layout

### Sprint 4.2: Activity Tab
- `ActivityTab.tsx` — Full transaction history with unified sent/received
- Direction filter (All / Sent / Received)
- Pagination: 10 per page, `[<] Page X of Y [>]`
- Copy txid icon + WhatsOnChain external link per row
- USD at transaction time + current USD display

### Sprint 4.3: Certificates + Approved Sites
- `CertificatesTab.tsx` — Extracted from old WalletOverlayRoot, same BRC-52 certificate list
- `ApprovedSitesTab.tsx` — Wraps existing `DomainPermissionsTab` + default auto-approve limit controls (per-tx, per-session, rate limit)

### Sprint 4.4: Settings Tab
- `SettingsTab.tsx` — Display name editor, PIN-gated mnemonic reveal, export backup, delete wallet with 2-step confirmation
- New endpoints: `GET/POST /wallet/settings`, `POST /wallet/reveal-mnemonic`, `POST /domain/permissions/reset-all`

### Sprint 4.5: Dark Theme CSS
- `WalletDashboard.css` — Full dark theme, MUI overrides for embedded DomainPermissionsTab
- Responsive layout, lazy-loaded tabs via `React.lazy()` + `Suspense`

### Unified Activity History (Sprint 4.6, added 2026-03-09)
- V11 DB migration: `price_usd_cents INTEGER` column on `transactions` + `peerpay_received`
- New `GET /wallet/activity` endpoint merging both tables with deduplication, ISO timestamps, pagination, direction filter
- ActivityTab rewrite to use unified endpoint
- DashboardTab recent activity uses `/wallet/activity?page=1&limit=5&filter=all`
- Price snapshot at transaction time recorded in create_action, internalize_action, task_check_peerpay, task_sync_pending

---

## 3. Architecture

### Frontend Files

| File | Purpose |
|------|---------|
| `frontend/src/components/wallet/WalletSidebar.tsx` | Sidebar navigation (Dashboard, Activity, Certificates, Approved Sites, Settings) |
| `frontend/src/components/wallet/DashboardTab.tsx` | Balance + QR + Send form + Recent activity |
| `frontend/src/components/wallet/ActivityTab.tsx` | Full transaction history with pagination, filters, USD |
| `frontend/src/components/wallet/CertificatesTab.tsx` | BRC-52 certificate list (extracted) |
| `frontend/src/components/wallet/ApprovedSitesTab.tsx` | Domain permissions + default limit controls |
| `frontend/src/components/wallet/SettingsTab.tsx` | Display name, mnemonic reveal, backup, delete |
| `frontend/src/components/wallet/WalletDashboard.css` | All dashboard styles, dark theme |

### Backend Endpoints Added

| Method | Path | Handler | Purpose |
|--------|------|---------|---------|
| GET | `/wallet/settings` | `wallet_get_settings` | Read display name + default limits |
| POST | `/wallet/settings` | `wallet_set_settings` | Update display name + default limits |
| POST | `/wallet/reveal-mnemonic` | `wallet_reveal_mnemonic` | PIN-gated mnemonic reveal |
| POST | `/domain/permissions/reset-all` | `domain_permissions_reset_all` | Clear all domain permissions |
| GET | `/wallet/activity` | `wallet_activity` | Unified sent+received activity with pagination |

### Database Changes

| Migration | Table | Change |
|-----------|-------|--------|
| V10 | `settings` | Added `default_per_tx_limit_cents`, `default_per_session_limit_cents`, `default_rate_limit_per_min` |
| V11 | `transactions` | Added `price_usd_cents INTEGER` |
| V11 | `peerpay_received` | Added `price_usd_cents INTEGER` |

### Key Patterns

- **Lazy-loaded tabs**: `React.lazy()` + `Suspense` — each tab is a separate chunk
- **Dark theme**: CSS-only (no MUI ThemeProvider), uses `WalletDashboard.css`
- **Deep-linking**: `?tab=N` query param for direct tab navigation
- **Price snapshot**: `state.price_cache.get_cached().or_else(|| state.price_cache.get_stale()).map(|p| (p * 100.0) as i64)`

---

## 4. Sender Display Name

The wallet stores `sender_display_name` in the `settings` table (default: `"Anonymous"`). This name is sent to paymail recipients as the "from" label (formatted as `"{name}'s Hodos Wallet"`).

**Settings tab UI**: Text field with info tooltip explaining the purpose. Saves on blur or Enter.

**Future (Open Paymail)**: When implemented, `sender_display_name` becomes the paymail registration alias and the "Hodos Wallet" suffix is removed. No schema change needed.

---

## 5. Testing Status

### Needs Testing

- [ ] Dashboard tab: balance displays correctly, QR code generates, send form works
- [ ] Activity tab: shows both sent AND received transactions
- [ ] Activity tab: pagination works (next/prev, page counter)
- [ ] Activity tab: direction filter (All/Sent/Received) resets to page 1
- [ ] Activity tab: USD at tx time displays, current USD shown when different
- [ ] Activity tab: copy txid icon works (shows checkmark 2s)
- [ ] Activity tab: WoC link opens transaction in new browser tab
- [ ] Certificates tab: lists existing certificates
- [ ] Approved Sites tab: shows domain permissions, default limits editable
- [ ] Settings tab: display name editable and persists
- [ ] Settings tab: mnemonic reveal requires correct PIN
- [ ] Settings tab: export backup downloads .hodos file
- [ ] Settings tab: delete wallet has 2-step confirmation, refuses if balance > 0
- [ ] Sidebar navigation: all 5 tabs switch correctly
- [ ] Dark theme: no visual artifacts, all text readable
- [ ] DashboardTab recent activity: shows 5 most recent with USD values

### Known Issues to Investigate

- Send transaction black screen / wallet close (C++ IPC blocking UI thread — see MEMORY.md)
- Activity timestamps may need timezone handling verification
- Historical USD prices only recorded for new transactions (old ones show current price only)

---

## 6. Future Work (Phase 5+)

- Activity Status Indicator (Phase 5, low priority)
- Addresses page (view/label derived addresses)
- Vault section (advanced key management)
- Per-site toggle for identity resolution
- V&B branding alignment pass on earlier phases

---

*This document replaces the original Phase 4 strategic planning doc. All wallet dashboard tasks should be tracked here.*
