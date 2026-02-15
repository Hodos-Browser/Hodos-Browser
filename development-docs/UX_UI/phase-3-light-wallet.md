# Light Wallet Interface Implementation Plan

## Overview

**Interface Type**: Modal or Panel
**Purpose**: Quick access wallet interface for common operations (balance, send, receive)

**Status**: 📋 Planning Phase
**Last Updated**: 2026-02-11

---

## Decision (2026-02-11): Phase 3 = Polish of Existing Wallet Overlay

> **Phase 3 is NOT a new component.** The existing `WalletPanelLayout.tsx` / `WalletOverlayRoot.tsx`
> already provides balance, send, receive, and transaction history. Phase 3 is a **polish and
> branding pass** on the existing wallet overlay to bring it up to production quality.
>
> Phase 4 (Full Wallet) will later extend this into a comprehensive management interface.

### What Phase 3 Adds/Fixes

- **Hodos branding**: Replace any default/blue accents with Hodos gold (`#a67c00`) color scheme
- **Hodos logo**: Add `Hodos_Gold_Icon.svg` or `Hodos_Black_Icon.svg` to wallet header
- **Button feedback**: All buttons must have hover, pressed, disabled, and loading states (currently missing)
- **Progress indicators**: Send transaction should show a progress bar / status ("Broadcasting...", "Confirmed")
- **BSV QR code**: Add standard BSV QR code to the receive section so users can scan with their mobile wallet and auto-populate the address
- **Micro UX fixes**: "Copied" feedback on address copy, inline validation on send form, empty state messages
- **Consistent color scheme**: Apply the full Hodos palette (gold, teal, semantic colors) throughout

---

## Interface Description

The Light Wallet (existing wallet overlay) provides quick access to essential wallet functions:
- View balance
- Send transactions
- Receive (show address + **BSV QR code**)
- View recent transactions (limited)
- Quick actions

**Design Philosophy**: Minimal, fast, focused on common tasks without full wallet complexity.

**Display Context**: Overlay panel (existing `WalletOverlayRoot.tsx` pattern)

---

## Requirements

### Functional Requirements
- [ ] Display current balance
- [ ] Generate/display receive address
- [ ] QR code generation for receive address
- [ ] Send transaction (simplified form)
- [ ] View recent transactions (last 5-10)
- [ ] Quick actions (copy address, view full wallet, etc.)
- [ ] Real-time balance updates

### Non-Functional Requirements
- [ ] Fast load time (< 1 second)
- [ ] Minimal UI footprint
- [ ] Responsive design
- [ ] Keyboard shortcuts support
- [ ] Accessible

---

## Frontend Implementation

### Component Structure

**Location**: `frontend/src/components/LightWallet.tsx` (or similar)

**Type**: React Functional Component

**Props**:
```typescript
interface LightWalletProps {
  open: boolean;
  onClose: () => void;
  onOpenFullWallet?: () => void; // Navigate to full wallet
  mode?: 'modal' | 'panel'; // Display mode
}
```

**Component Sections**:
1. Header (balance, quick actions)
2. Send section (simplified form)
3. Receive section (address, QR code)
4. Recent transactions (compact list)
5. Footer (open full wallet link)

**State Management**:
- Current balance
- Selected address
- Recent transactions
- Send form state
- Loading states

---

## CEF-Native Implementation

### Window Management

**If Modal**:
- Use existing overlay window system
- Route: `/light-wallet` (or similar)
- Overlay HWND: `g_light_wallet_overlay_hwnd` (to be added)

**If Panel**:
- Consider side panel implementation
- May need new window type or reuse overlay system

### Message Handling

**Existing Messages** (reuse):
- `get_balance` - Get wallet balance
- `send_transaction` - Send transaction
- `get_current_address` - Get receive address
- `get_transaction_history` - Get recent transactions

**Potential New Messages**:
- `open_full_wallet` - Navigate to full wallet view

---

## Rust Wallet Backend

### Existing Endpoints (Review/Verify)

- `GET /wallet/balance` - Get wallet balance
- `POST /wallet/send` - Send transaction
- `GET /wallet/address/current` - Get current address
- `GET /wallet/transactions` - Get transaction history

### Required Changes

- [ ] Verify endpoints support light wallet use case
- [ ] Optimize for fast responses
- [ ] Support transaction history pagination (first N only)
- [ ] Real-time balance updates (if needed)

---

## Database Considerations

### Current Schema

- Balance cached in `balance_cache` table
- Transactions in transactions table
- Addresses in addresses table

### Potential Optimizations

- [ ] Index transactions by timestamp (for recent queries)
- [ ] Cache balance updates more aggressively
- [ ] Consider materialized views for recent transactions

---

## Triggers

### Primary Triggers

1. **Light Wallet Button** (new button in header)
   - Quick access from main browser view
   - Keyboard shortcut (e.g., Ctrl+Shift+W)

2. **From Full Wallet** (collapsed view option)
   - Switch to light view from full wallet

3. **From Context Menu** (right-click options)
   - Quick wallet access from anywhere

### Secondary Triggers

- URL route navigation
- Programmatic trigger from other components

---

## User Interaction Flow

### View Balance Flow

```
1. User opens Light Wallet
   ↓
2. System fetches current balance
   ↓
3. Balance displayed in header
   ↓
4. Real-time updates (if enabled)
```

### Send Transaction Flow

```
1. User opens Light Wallet
   ↓
2. User clicks "Send" or focuses send form
   ↓
3. User enters recipient address
   ↓
4. User enters amount
   ↓
5. User optionally adds note
   ↓
6. User clicks "Send"
   ↓
7. Transaction confirmation (if enabled)
   ↓
8. Transaction sent
   ↓
9. Success notification
   ↓
10. Balance updates
```

### Receive Flow

```
1. User opens Light Wallet
   ↓
2. User navigates to "Receive" section
   ↓
3. Current address displayed
   ↓
4. QR code generated and displayed
   ↓
5. User can copy address or share QR code
```

---

## Design Considerations

**Reference**: [Design Principles](./helper-2-design-philosophy.md)

Key considerations:
- [ ] Minimal UI (focus on essentials)
- [ ] Fast interactions
- [ ] Clear visual hierarchy
- [ ] Mobile-friendly (responsive)
- [ ] Keyboard navigation
- [ ] Visual balance display

---

## Testing Requirements

### Unit Tests
- Component rendering
- Balance fetching
- Transaction sending
- Address generation
- QR code generation

### Integration Tests
- Balance updates
- Transaction sending flow
- Address management
- Navigation to full wallet

### User Acceptance Tests
- Ease of use
- Speed of common operations
- Clarity of UI
- Mobile experience

---

## Dependencies

### External Dependencies
- Wallet balance API
- Transaction sending API
- Address generation API
- QR code library

### Internal Dependencies
- Wallet must exist (Initial Setup/Recovery)
- Full Wallet (for navigation)

---

## Related Documentation

- [Full Wallet Interface](./phase-4-full-wallet.md) - Complete wallet interface
- [Initial Setup/Recovery](./phase-1-initial-setup-recovery.md) - Wallet setup
- [Design Principles](./helper-2-design-philosophy.md) - Design guidelines
- [CEF Refinement Tracker](../CEF_REFINEMENT_TRACKER.md) - **CR-3 items** (debug overlay removal, settings overlay stale pointer, logging cleanup) can be done alongside Phase 3 polish since both touch overlay and UX code

---

## Open Questions (Updated 2026-02-11)

1. ~~Modal vs Panel~~ → **Resolved**: Existing overlay panel (no change)
2. What transaction fields are essential for "light" send? (address, amount, optional memo — keep current)
3. How many recent transactions to show? (5-10, keep current)
4. ~~Should light wallet support multiple addresses?~~ → No, keep single current address for light wallet
5. ~~Real-time updates or periodic refresh?~~ → Keep existing balance poller pattern
6. ~~Should it replace or complement existing wallet overlay?~~ → **Resolved**: Phase 3 IS the existing overlay, polished

---

## Implementation Notes

- Review existing `WalletPanelLayout.tsx` for patterns
- Consider if this replaces or complements existing wallet overlay
- QR code generation will need a library (e.g., `qrcode.react`)
- Balance caching important for performance

---

**End of Document**
