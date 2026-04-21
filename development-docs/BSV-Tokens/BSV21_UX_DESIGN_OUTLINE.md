# BSV-21 Wallet UI/UX Design Outline

## Status: PENDING DESIGN REVIEW

This document outlines the UI/UX considerations for BSV-21 token support. **No implementation should begin until the design team has reviewed and approved the approach.**

---

## Current Architecture Options

### Option A: Enhanced Wallet Panel
Keep the existing slide-out wallet panel and add token functionality as tabs/sections.

**Pros:**
- Familiar pattern, already implemented
- Quick access from any page
- Good for simple operations (check balance, quick send)

**Cons:**
- Limited space for complex displays
- Difficult to show multiple token types, baskets, history
- May become cluttered

### Option B: Dedicated Wallet Page (localhost)
Create a full wallet management page loaded in the WebView at a localhost URL.

**Pros:**
- Full screen real estate
- Can display complex data (baskets, history, multiple token types)
- Better for power users
- Separation of concerns

**Cons:**
- Extra navigation step
- Need to design full page layout

### Option C: Hybrid (RECOMMENDED)
- **Wallet Panel**: Quick actions - BSV balance, quick send, recent activity
- **Wallet Page**: Full management - tokens, baskets, certificates, history, settings

**User Flow:**
1. Click wallet icon → Panel slides out with summary
2. Panel has "Manage Wallet" button → Opens full wallet page
3. Full page has tabs: Overview | Tokens | Baskets | Certificates | History

---

## Core Concepts to Display

### 1. BSV Balance (Already Exists)
- Total satoshis across all UTXOs
- Display in BSV and USD equivalent

### 2. Token Balances (NEW)
- **What it is**: Sum of token amounts across all UTXOs for each token
- **Example**: You own 3 UTXOs with TEST token → Total = sum of amounts
- **Display**: List of tokens with symbol, icon, balance

### 3. Baskets (Category Labels)
- **What it is**: Tags/labels for organizing UTXOs
- **Default basket**: "default" - where new UTXOs go
- **Custom baskets**: User-created categories (savings, DeFi, etc.)
- **Display**: Folder-like organization, can filter tokens by basket

### 4. Token Types
| Type | Description | Display Considerations |
|------|-------------|----------------------|
| BSV-21 Fungible | Standard tokens (TEST, PEPE) | Show balance, allow send |
| BSV-21 Stablecoin | USD-pegged tokens | Show balance in USD equivalent |
| STAS Tokens | Different protocol | Separate section, different logic |
| NFTs (1Sat Ordinals) | Unique items | Grid display with images |

### 5. Certificates (BRC-52)
- Identity certificates from verifiers
- Display issuer, type, expiry
- Separate section from tokens

---

## Required UI Elements

### Buttons/Actions

| Action | Location | Description |
|--------|----------|-------------|
| Send Token | Token detail / Token row | Transfer tokens to address |
| Receive | Wallet overview | Show address + QR code |
| Sync | Token list header | Refresh from GorillaPool |
| Create Basket | Basket management | Add new category |
| Move to Basket | Token/UTXO context menu | Reassign category |

### Displays/Views

| View | Content | Priority |
|------|---------|----------|
| Token List | All tokens with balances | High |
| Token Detail | Single token info, send form | High |
| Basket List | Categories with counts | Medium |
| Basket Contents | Tokens/UTXOs in a basket | Medium |
| Transaction History | Past sends/receives | Medium |
| Certificate List | Identity certs | Low (Phase 2) |
| NFT Gallery | Ordinal images | Low (Phase 2) |

### Information Cards

| Card | Fields |
|------|--------|
| Token Summary | Icon, Symbol, Balance (formatted), USD value |
| Token Detail | + Token ID, Decimals, Max Supply, Deploy Height |
| Transaction | Type (send/receive), Amount, Address, Date, TXID |
| Basket | Name, Token count, Total value |

---

## User Flows

### Flow 1: Check Token Balance
```
User opens wallet → Sees BSV balance at top
                  → Sees "Tokens" tab/section
                  → Clicks to expand
                  → Sees list of tokens with balances
```

### Flow 2: Send Tokens
```
User selects token → Sees token detail with balance
                   → Clicks "Send"
                   → Enters address + amount
                   → Reviews confirmation
                   → Approves → Transaction sent
                   → Sees success with TXID
```

### Flow 3: Receive Tokens
```
User clicks "Receive" → Sees their address
                      → QR code displayed
                      → Copy button for address
                      → (Later: select basket for incoming)
```

### Flow 4: Organize with Baskets
```
User goes to Baskets → Sees list of baskets
                     → Can create new basket
                     → Can view contents of basket
                     → Can drag/move tokens between baskets
```

### Flow 5: Marketplace Purchase (BRC-100)
```
User visits marketplace website in browser
→ Marketplace requests auth via BRC-100
→ Our wallet shows auth modal (existing)
→ User approves
→ Marketplace requests createAction for purchase
→ Our wallet shows confirmation modal
→ User approves → Transaction signed and sent
→ Token appears in wallet after sync
```

---

## Design Recommendations

### 1. Hierarchical Organization
```
Wallet
├── Overview (BSV + quick stats)
├── Tokens
│   ├── BSV-21 Fungible
│   │   ├── Stablecoins (special display)
│   │   └── Other tokens
│   ├── STAS (if implemented)
│   └── NFTs (grid view)
├── Baskets
│   ├── Default
│   └── Custom baskets...
├── Certificates
└── History
```

### 2. Token Type Visual Distinction
- **Stablecoins**: Show USD symbol, green for positive
- **Regular tokens**: Show token icon/symbol
- **NFTs**: Thumbnail images
- **STAS**: Different badge/indicator

### 3. Balance Display Format
```
Token Balance:     1,234.56789012 TEST
USD Equivalent:    ~$45.67 (if price available)
UTXO Count:        3 UTXOs (for advanced users, optional)
```

### 4. Empty States
- "No tokens yet" with explanation
- "Sync from network" button
- Link to how to get tokens

### 5. Loading States
- Skeleton loaders for token cards
- "Syncing..." indicator
- Last sync timestamp

---

## Questions for Design Team

1. **Panel vs. Page**: Which approach for main wallet UI?
   - Option A: Enhanced panel only
   - Option B: Full page only
   - Option C: Hybrid (panel + page)

2. **Token List Style**: How to display token list?
   - List view (compact, more items visible)
   - Card view (larger, more info per item)
   - Switchable (user preference)

3. **Basket Visibility**: How prominent should baskets be?
   - Top-level tab (equal to tokens)
   - Sub-section under tokens
   - Advanced/settings area

4. **NFT Priority**: When to add NFT display?
   - Phase 1 with tokens
   - Phase 2 after tokens work
   - Future enhancement

5. **Stablecoin Special Treatment**: Should stablecoins have special UI?
   - Display in fiat units
   - Separate section from other tokens
   - Just another token with USD indicator

6. **Transaction History Scope**: What to show?
   - All wallet transactions
   - Per-token history
   - Filterable by type/date

---

## Marketplace Integration Notes

**We do NOT build marketplace UI.**

Marketplaces (like existing 1Sat Ordinals marketplaces) implement BRC-100 to talk to our wallet. The flow is:

1. User browses marketplace website in our browser
2. Marketplace calls `window.hodosBrowser.wallet.*` methods
3. Our existing auth modals appear for approval
4. We sign/broadcast transactions
5. Tokens appear in our wallet after sync

**What we need to ensure:**
- Our wallet panel/page displays tokens we own
- Send functionality works for user-initiated transfers
- Auth modals work for marketplace-initiated requests
- Token sync picks up new tokens after purchases

---

## Implementation Dependencies

| Feature | Depends On |
|---------|------------|
| Token Display | Plan A: `/ordinals/balance` endpoint |
| Token Send | Plan A: `createAction` with token_transfer |
| Token Sync | Plan A: `/ordinals/sync` endpoint |
| Token Metadata | Plan A: `/ordinals/token/{id}` endpoint |
| Basket Display | Existing basket DB (already in Plan A) |
| STAS Support | Future: Separate implementation plan |
| NFT Display | Future: Ordinals inscription parsing |

---

## Next Steps

1. [ ] Design team reviews this document
2. [ ] Decide on Panel vs. Page vs. Hybrid approach
3. [ ] Create wireframes/mockups
4. [ ] Review mockups with dev team
5. [ ] Update BSV21_PLAN_B_FRONTEND.md with approved designs
6. [ ] Begin implementation

---

**Created**: January 2025
**Status**: PENDING DESIGN REVIEW
**Owner**: UX/UI Design Team
**Related**: BSV21_PLAN_A_BACKEND.md, BSV21_PLAN_B_FRONTEND.md
