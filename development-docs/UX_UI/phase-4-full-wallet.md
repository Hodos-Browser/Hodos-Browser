# Full Wallet Interface Implementation Plan

## Overview

**Interface Type**: Full Webview Window
**Purpose**: Complete wallet management interface with all features (addresses, transactions, settings, certificates, etc.)

**Status**: 📋 Planning Phase
**Last Updated**: 2026-02-11

---

## Interface Description

The Full Wallet provides comprehensive wallet management:
- Balance and portfolio overview
- Address management (generate, view, label)
- Transaction history (full list with filtering)
- Send/Receive transactions (advanced)
- Certificate management (BRC-52)
- Settings and preferences
- Backup and recovery
- Multi-address support
- Advanced features

**Design Philosophy**: Complete feature set, organized navigation, detailed views.

**Display Context**:
- Full webview window (dedicated window)
- Full-screen overlay
- Comprehensive interface

---

## Requirements

### Functional Requirements
- [ ] Dashboard/Overview (balance, summary, quick actions)
- [ ] Address management (list, generate, label, delete)
- [ ] Transaction history (list, filter, search, details)
- [ ] Send transaction (advanced form with all options)
- [ ] Receive (addresses, QR codes, labels)
- [ ] Certificate management (view, organize, transfer)
- [ ] Settings (preferences, backup, recovery, security)
- [ ] UTXO management (view, select, consolidate)
- [ ] Advanced features (multi-sig, scripts, etc.)

### Non-Functional Requirements
- [ ] Comprehensive navigation
- [ ] Search and filtering
- [ ] Export capabilities
- [ ] Print support
- [ ] Keyboard shortcuts
- [ ] Responsive design (for different window sizes)
- [ ] Performance (handle large transaction histories)

---

## Frontend Implementation

### Component Structure

**Location**: `frontend/src/pages/FullWalletView.tsx` (or similar)

**Type**: React Component with Routing

**Layout**:
- Header/Navigation bar
- Sidebar navigation (optional)
- Main content area (route-based)
- Footer/Status bar

**Routes**:
- `/wallet` or `/wallet/full` - Main entry
- `/wallet/overview` - Dashboard
- `/wallet/addresses` - Address management
- `/wallet/transactions` - Transaction history
- `/wallet/send` - Send transaction
- `/wallet/receive` - Receive section
- `/wallet/certificates` - Certificate management
- `/wallet/settings` - Settings

**Component Hierarchy**:
```
FullWalletView
├── WalletHeader
├── WalletSidebar (optional)
├── WalletContent (route-based)
│   ├── OverviewPage
│   ├── AddressesPage
│   ├── TransactionsPage
│   ├── SendPage
│   ├── ReceivePage
│   ├── CertificatesPage
│   └── SettingsPage
└── WalletFooter
```

**State Management**:
- Global wallet state (balance, addresses, etc.)
- Current view/route
- Filters and search state
- UI preferences
- Loading states

---

## CEF-Native Implementation

### Window Management

**Full Webview Window**:
- New dedicated window class
- Window HWND: `g_full_wallet_hwnd` (to be added)
- Resizable, movable window
- Persistent window state (size, position)

**Alternative**: Full-screen overlay (if preferred)

### Message Handling

**Existing Messages** (extend/reuse):
- All existing wallet messages
- Window management messages

**New Messages** (potential):
- `wallet_window_open_full` - Open full wallet window
- `wallet_window_close` - Close full wallet window
- `wallet_window_state` - Save/restore window state

---

## Rust Wallet Backend

### Existing Endpoints (Review/Extend)

- All existing wallet endpoints
- Transaction endpoints
- Address endpoints
- Certificate endpoints

### Required Additions

- [ ] Advanced transaction options
- [ ] Address labeling/management
- [ ] Transaction filtering/search
- [ ] Certificate operations
- [ ] Export functionality
- [ ] Settings persistence

### API Extensions

**Potential New Endpoints**:
- `GET /wallet/overview` - Dashboard summary
- `GET /wallet/addresses` (with pagination, filters)
- `GET /wallet/transactions` (with pagination, filters, search)
- `POST /wallet/addresses/label` - Label address
- `GET /wallet/certificates` - List certificates
- `POST /wallet/export` - Export wallet data

---

## Database Considerations

### Current Schema

- Wallet, addresses, transactions, certificates tables exist

### Potential Additions

**Address Labels Table**:
- Address ID
- Label
- Created timestamp
- User notes

**Transaction Labels/Tags**:
- Transaction ID
- Label
- Tags
- Notes

**Settings Table**:
- Setting key
- Setting value
- Category

**Window State Table** (if needed):
- Window type
- Size/position
- User preferences

---

## Triggers

### Primary Triggers

1. **Full Wallet Button** (in header or light wallet)
   - Dedicated button for full wallet
   - Keyboard shortcut (e.g., Ctrl+W)

2. **From Light Wallet** (upgrade option)
   - "Open Full Wallet" link/button

3. **From Settings** (navigation)
   - Settings menu option

### Secondary Triggers

- Direct navigation (URL)
- Command line arguments
- Programmatic triggers

---

## User Interaction Flow

### Navigation Flow

```
1. User opens Full Wallet
   ↓
2. Dashboard/Overview loads
   ↓
3. User navigates to section (addresses, transactions, etc.)
   ↓
4. Section content loads
   ↓
5. User interacts with features
   ↓
6. User can navigate between sections
```

### Send Transaction Flow (Advanced)

```
1. User navigates to Send page
   ↓
2. User selects from address (if multiple)
   ↓
3. User enters recipient address
   ↓
4. User enters amount
   ↓
5. User sets fee (or auto)
   ↓
6. User adds OP_RETURN data (optional)
   ↓
7. User adds label/note
   ↓
8. User reviews transaction details
   ↓
9. User confirms transaction
   ↓
10. Transaction broadcast
    ↓
11. Transaction appears in history
```

### Address Management Flow

```
1. User navigates to Addresses page
   ↓
2. List of addresses displayed
   ↓
3. User can generate new address
   ↓
4. User can label addresses
   ↓
5. User can view address details
   ↓
6. User can set default address
   ↓
7. User can delete unused addresses
```

---

## Design Considerations

**Reference**: [Design Principles](./helper-2-design-philosophy.md)

Key considerations:
- [ ] Comprehensive navigation
- [ ] Information density (detailed views)
- [ ] Search and filtering
- [ ] Performance with large datasets
- [ ] Export/print capabilities
- [ ] Advanced user features
- [ ] Mobile adaptation (if needed)

---

## Testing Requirements

### Unit Tests
- All component rendering
- Navigation logic
- Form validation
- Search/filter logic
- Export functionality

### Integration Tests
- All wallet operations
- Transaction flows
- Address management
- Certificate operations
- Settings persistence

### User Acceptance Tests
- Feature completeness
- Navigation ease
- Performance with large data
- Export functionality
- Mobile/responsive experience

---

## Dependencies

### External Dependencies
- All wallet backend endpoints
- Certificate management system
- Export libraries
- Charting libraries (if dashboard charts)

### Internal Dependencies
- Initial Setup/Recovery (wallet must exist)
- Light Wallet (navigation between)
- User Notifications (for transaction confirmations)

---

## Related Documentation

- [Light Wallet Interface](./phase-3-light-wallet.md) - Quick wallet access
- [Initial Setup/Recovery](./phase-1-initial-setup-recovery.md) - Wallet setup
- [UI/UX Enhancement Guide](./helper-1-implementation-guide-checklist.md) - Frontend architecture
- [Design Principles](./helper-2-design-philosophy.md) - Design guidelines

---

## Open Questions

1. Should this be a full window or full-screen overlay?
2. What navigation pattern (sidebar, tabs, etc.)?
3. What advanced features are essential?
4. How to handle large transaction histories?
5. Should it support multi-wallet?
6. Mobile/responsive requirements?
7. Export formats needed?

---

## Decisions & Notes (2026-02-11)

> **Route Namespace Reservation**
>
> Phase 4 proposes routes like `/wallet/overview`, `/wallet/addresses`, etc. But `/wallet` is
> already used by the existing wallet overlay (`WalletOverlayRoot.tsx`). During Phase 4 planning,
> resolve this conflict:
> - Option A: Move the light wallet overlay to a non-route trigger (overlay system only, no URL route)
> - Option B: Use `/wallet-full/*` for Phase 4 routes
> - Option C: Phase 4 replaces the overlay entirely and owns `/wallet/*`
>
> **Decide during Phase 4 planning session.** In the meantime, Phases 1-3 should NOT add new
> `/wallet/*` routes — use the overlay system instead.

> **WalletContext for State Management**
>
> Phase 4 has 7+ sub-routes sharing state (balance, selected address, transaction filters,
> certificates). The current "no global state library" approach (useState/useEffect + hooks)
> will likely be insufficient. During Phase 4 planning, evaluate whether a `WalletContext`
> (React Context) is needed to share wallet state across sub-routes. This is the recommended
> approach — it avoids a full state library while providing the shared state Phase 4 needs.

> **Scope Clarification: What "UTXO Management" Actually Means**
>
> The original doc listed "UTXO management (view, select, consolidate)" as a feature. In
> practice, this means:
> - **Basket-labeled outputs**: Display PushDrop tokens and basket-grouped outputs separately
>   from regular UTXOs. Provide a button to "spend back to normal" (convert token to regular UTXO).
> - **Certificates**: Display certificates, allow user to delete/revoke (spend the cert output).
> - **Domain permissions**: A settings panel where the user can search for a domain and adjust
>   basic auto-approve parameters (spending limits, certificate levels). Feeds from Phase 0.1 design.
> - **NOT in scope for MVP**: Advanced UTXO selection, manual UTXO consolidation, coin control.
>   These are power-user features for a future sprint.

---

## Implementation Notes

- This is the most complex interface
- Should build on Light Wallet patterns (Phase 3 polish)
- Consider phased implementation (core features first, advanced later)
- Review existing `WalletPanelLayout.tsx` for patterns
- May need significant database query optimizations

---

**End of Document**
