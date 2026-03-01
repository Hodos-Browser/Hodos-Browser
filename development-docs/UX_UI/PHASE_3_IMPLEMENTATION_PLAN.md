# Phase 3: Light Wallet & Peer Payments — Implementation Plan

**Date:** 2026-02-26
**Status:** 📋 Planning
**Target Start:** After Phase 2 (User Notifications)
**Estimated Duration:** 2-3 weeks

---

## Executive Summary

Phase 3 combines:
1. **Light Wallet Polish** — branding, UX improvements, QR codes
2. **BRC-29 Peer Payments** — send/receive to identity keys
3. **Unified Send** — auto-detect address, paymail, or identity key
4. **Background Polling** — auto-fetch incoming payments
5. **Notifications** — payment received alerts

---

## Phase Structure

```
Phase 3
├── 3.0: Light Wallet Polish (existing doc: phase-3-light-wallet.md)
│   ├── Hodos branding
│   ├── Button states
│   ├── QR codes
│   └── Micro UX fixes
│
├── 3a: BRC-29 Peer Payments (new doc: phase-3a-brc29-peer-payments.md)
│   ├── Send to identity key
│   ├── Receive via identity key
│   ├── MessageBox polling
│   └── Payment notifications
│
├── 3b: Unified Send Address (part of 3a)
│   ├── Address type detection
│   ├── Paymail resolution (BRC-28)
│   └── Visual feedback
│
└── 3c: Receive UX Update (part of 3a)
    ├── Identity key display
    ├── Legacy address generation
    └── QR for both
```

---

## Prerequisites

| Dependency | Status | Notes |
|------------|--------|-------|
| Phase 2 (User Notifications) | 📋 Planning | Permission prompts for payments |
| BRC-33 Message Relay | ✅ Complete | In-memory + SQLite storage |
| BRC-42 Key Derivation | ✅ Complete | `crypto/brc42.rs` |
| BRC-100 Wallet (core) | ✅ 93% | `createAction`, `internalizeAction` |

**Can Start Without Phase 2:** Yes, for non-permission-gated features (3.0 polish, receive UI). Full 3a requires Phase 2 for payment confirmation prompts.

---

## Sprint Breakdown

### Sprint 3.1: Light Wallet Polish (3-4 days)

**Focus:** Existing wallet overlay improvements

| Task | Est | Owner | Details |
|------|-----|-------|---------|
| Hodos branding pass | 0.5d | FE | Gold accents, logo in header |
| Button states | 0.5d | FE | Hover, pressed, disabled, loading |
| Send form validation | 0.5d | FE | Inline errors, amount validation |
| Progress indicators | 0.5d | FE | Broadcasting → Confirmed states |
| Receive QR code | 0.5d | FE | `qrcode.react` library |
| Copy feedback | 0.25d | FE | "Copied!" toast on address copy |
| Empty states | 0.25d | FE | No transactions message |
| Testing & polish | 0.5d | FE | Cross-browser, responsive |

**Deliverables:**
- [ ] Polished wallet overlay matching Hodos brand
- [ ] All buttons have proper states
- [ ] QR code in receive section
- [ ] Send shows progress/confirmation

---

### Sprint 3.2: Receive UX Update (2 days)

**Focus:** Identity key + legacy address in receive modal

| Task | Est | Owner | Details |
|------|-----|-------|---------|
| Receive modal redesign | 0.5d | FE | Two-section layout |
| Identity key display | 0.25d | FE | Fetch from `/getPublicKey` |
| Identity key QR | 0.25d | FE | Separate QR from legacy |
| Legacy address generation | 0.25d | BE | New endpoint or use existing |
| Legacy address QR | 0.25d | FE | QR for address |
| Copy buttons for both | 0.25d | FE | Copy identity key, copy address |
| Testing | 0.25d | Both | Verify QR scans work |

**Deliverables:**
- [ ] Receive modal shows identity key prominently
- [ ] "Generate Legacy Address" button
- [ ] QR codes for both
- [ ] Copy to clipboard for both

---

### Sprint 3.3: Unified Send Field (2-3 days)

**Focus:** Auto-detect recipient type

| Task | Est | Owner | Details |
|------|-----|-------|---------|
| Address type detection | 0.5d | FE | Regex patterns for each type |
| Visual indicators | 0.25d | FE | 📍📧🔑 badges |
| Paymail resolver client | 0.5d | BE | BRC-28 resolution |
| Frontend paymail call | 0.25d | FE | Async resolution + loading |
| Identity key validation | 0.25d | BE | 33-byte pubkey check |
| Error handling | 0.25d | FE | Invalid format messages |
| Send routing logic | 0.5d | FE | Route to correct send method |
| Testing | 0.5d | Both | All three types |

**Deliverables:**
- [ ] Single "Send To" field accepts all formats
- [ ] Visual indicator shows detected type
- [ ] Paymail resolves to address in real-time
- [ ] Identity key validated before send

---

### Sprint 3.4: BRC-29 Send to Identity (3 days)

**Focus:** Send payments to identity public keys

| Task | Est | Owner | Details |
|------|-----|-------|---------|
| BRC-29 types/structs | 0.25d | BE | `BRC29PaymentMessage`, etc. |
| Key derivation for payment | 0.5d | BE | Invoice number, child pubkey |
| P2PKH script generation | 0.25d | BE | From derived pubkey |
| Transaction construction | 0.5d | BE | Use `createAction` |
| Payment message assembly | 0.25d | BE | BRC-29 format |
| MessageBox send | 0.5d | BE | External server integration |
| `/wallet/send-to-identity` endpoint | 0.25d | BE | REST API |
| Frontend integration | 0.25d | FE | Call new endpoint |
| Testing with self | 0.25d | Both | Send to own identity key |

**Deliverables:**
- [ ] `POST /wallet/send-to-identity` endpoint working
- [ ] Payment message sent via MessageBox
- [ ] Transaction broadcast to network
- [ ] Frontend calls endpoint for identity sends

---

### Sprint 3.5: BRC-29 Receive via MessageBox (3 days)

**Focus:** Background polling and payment processing

| Task | Est | Owner | Details |
|------|-----|-------|---------|
| MessageBox client setup | 0.5d | BE | Connect to external server |
| Poller background task | 0.5d | BE | Tokio interval task |
| Payment message parsing | 0.25d | BE | Deserialize BRC-29 |
| Key derivation for receive | 0.5d | BE | Derive privkey to verify |
| Script verification | 0.25d | BE | Match expected vs actual |
| `internalizeAction` call | 0.25d | BE | Add UTXO to wallet |
| Message acknowledgment | 0.25d | BE | Remove from inbox |
| Error handling | 0.25d | BE | Log failures, retry logic |
| Testing with Metanet Desktop | 0.25d | Both | Interoperability test |

**Deliverables:**
- [ ] Background poller runs every 30s
- [ ] Incoming payments processed automatically
- [ ] UTXOs appear in wallet balance
- [ ] Messages acknowledged after processing

---

### Sprint 3.6: Payment Notifications (2 days)

**Focus:** Alert user when payment received

| Task | Est | Owner | Details |
|------|-----|-------|---------|
| Notification endpoint | 0.25d | BE | `/wallet/notifications` |
| Payment event storage | 0.25d | BE | Track new payments |
| Frontend polling | 0.25d | FE | Check for new payments |
| Badge on wallet button | 0.25d | FE | Unread count |
| Toast notifications | 0.25d | FE | "Received X sats" |
| Notification list | 0.25d | FE | View recent payments |
| Mark as read | 0.25d | Both | Clear badge |
| Sound option | 0.25d | FE | Settings toggle |

**Deliverables:**
- [ ] Badge shows unread payment count
- [ ] Toast appears on new payment
- [ ] Can view notification list
- [ ] Mark as read clears badge

---

## Database Migrations

### V15: Raw Key Recovery Support (Phase 1d)

```sql
ALTER TABLE wallets ADD COLUMN key_source TEXT DEFAULT 'mnemonic';
ALTER TABLE wallets ADD COLUMN raw_key_encrypted BLOB;
```

### V16: BRC-29 Payment Tracking (Phase 3a)

```sql
CREATE TABLE brc29_incoming_payments (
    id INTEGER PRIMARY KEY,
    message_id TEXT NOT NULL UNIQUE,
    sender_identity_key TEXT NOT NULL,
    derivation_prefix TEXT NOT NULL,
    total_amount INTEGER NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    received_at INTEGER NOT NULL,
    processed_at INTEGER,
    error_message TEXT
);

CREATE TABLE brc29_outgoing_payments (
    id INTEGER PRIMARY KEY,
    recipient_identity_key TEXT NOT NULL,
    derivation_prefix TEXT NOT NULL,
    txid TEXT NOT NULL,
    message_id TEXT,
    total_amount INTEGER NOT NULL,
    status TEXT NOT NULL DEFAULT 'sent',
    sent_at INTEGER NOT NULL
);

CREATE TABLE payment_notifications (
    id INTEGER PRIMARY KEY,
    type TEXT NOT NULL,  -- 'received', 'sent', 'confirmed'
    payment_id INTEGER NOT NULL,
    amount INTEGER NOT NULL,
    counterparty TEXT NOT NULL,
    txid TEXT,
    read INTEGER DEFAULT 0,
    created_at INTEGER NOT NULL
);

CREATE INDEX idx_notifications_unread ON payment_notifications(read) WHERE read = 0;
```

---

## API Endpoints Summary

### New Endpoints

| Method | Path | Purpose |
|--------|------|---------|
| POST | `/wallet/send-to-identity` | Send BSV to identity pubkey |
| POST | `/wallet/resolve-paymail` | Resolve paymail to address |
| GET | `/wallet/notifications` | Get unread payment notifications |
| POST | `/wallet/notifications/read` | Mark notifications as read |

### Updated Endpoints

| Method | Path | Change |
|--------|------|--------|
| POST | `/getPublicKey` | Already returns identity key |
| POST | `/wallet/new-address` | Ensure returns fresh address |

---

## Frontend Components

### New Components

```
frontend/src/components/
├── wallet/
│   ├── UnifiedSendField.tsx      # Address/paymail/identity input
│   ├── RecipientTypeBadge.tsx    # Visual indicator (📍📧🔑)
│   ├── ReceiveModal.tsx          # Identity key + legacy address
│   ├── PaymentNotifications.tsx  # Notification list
│   ├── NotificationBadge.tsx     # Unread count badge
│   └── QRCodeDisplay.tsx         # Reusable QR component
```

### Updated Components

```
frontend/src/components/
├── WalletOverlayRoot.tsx         # Branding, states
├── wallet-panel/
│   ├── WalletPanelLayout.tsx     # Polish pass
│   ├── SendSection.tsx           # Unified send, progress
│   ├── ReceiveSection.tsx        # New receive modal
│   └── TransactionList.tsx       # Empty states
```

---

## Configuration

### MessageBox Settings

```rust
// config.rs or environment
pub struct MessageBoxConfig {
    /// External MessageBox server URL
    pub host: String,
    // Default: "https://messagebox.babbage.systems"
    
    /// Polling interval (seconds)
    pub poll_interval: u64,
    // Default: 30
    
    /// Enable/disable background polling
    pub enabled: bool,
    // Default: true
}
```

### Environment Variables

```bash
# .env or runtime config
MESSAGEBOX_HOST=https://messagebox.babbage.systems
MESSAGEBOX_POLL_INTERVAL=30
MESSAGEBOX_ENABLED=true
```

---

## Testing Strategy

### Unit Tests

- [ ] Address type detection (all three formats)
- [ ] BRC-29 message construction
- [ ] BRC-29 message parsing
- [ ] Key derivation (send and receive)
- [ ] Script verification

### Integration Tests

- [ ] Send to identity → receive in same wallet
- [ ] Send to external identity (mock MessageBox)
- [ ] Paymail resolution (mock server)
- [ ] Notification creation on receive

### End-to-End Tests

- [ ] Full send flow (identity key)
- [ ] Full receive flow (background poll)
- [ ] Notification appears after receive
- [ ] Interop with Metanet Desktop

### Manual Testing Checklist

- [ ] Send to self using identity key
- [ ] Send to Metanet Desktop user
- [ ] Receive from Metanet Desktop
- [ ] All address types in unified field
- [ ] QR codes scan correctly
- [ ] Notifications badge updates
- [ ] Toast appears on receive

---

## Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| MessageBox server unavailable | Medium | High | Graceful fallback, retry logic |
| Key derivation mismatch | Low | High | Extensive testing, spec compliance |
| Paymail resolution failures | Medium | Medium | Cache resolved addresses, timeout |
| Notification spam | Low | Low | Rate limiting, dedup |

---

## Success Criteria

### Phase 3.0 (Light Wallet Polish)
- [ ] Wallet matches Hodos brand guidelines
- [ ] All interactive elements have proper states
- [ ] Send shows progress through confirmation
- [ ] QR code works with standard wallet apps

### Phase 3a (BRC-29 Peer Payments)
- [ ] Can send to any valid identity pubkey
- [ ] Background poller processes incoming payments
- [ ] Payments appear in balance within 60s of send
- [ ] Interoperable with Metanet Desktop

### Phase 3b (Unified Send)
- [ ] Single field accepts all three formats
- [ ] Visual indicator shows detected type
- [ ] Paymail resolves without page reload

### Phase 3c (Notifications)
- [ ] Badge shows on new payment
- [ ] Toast notification appears
- [ ] Can clear/mark as read

---

## Timeline Summary

| Sprint | Duration | Focus |
|--------|----------|-------|
| 3.1 | 3-4 days | Light Wallet Polish |
| 3.2 | 2 days | Receive UX Update |
| 3.3 | 2-3 days | Unified Send Field |
| 3.4 | 3 days | BRC-29 Send |
| 3.5 | 3 days | BRC-29 Receive |
| 3.6 | 2 days | Notifications |
| **Total** | **15-17 days** | ~3 weeks |

---

## References

- [phase-3-light-wallet.md](./phase-3-light-wallet.md) — Original Light Wallet doc
- [phase-3a-brc29-peer-payments.md](./phase-3a-brc29-peer-payments.md) — BRC-29 details
- [phase-3-peer-payments-research.md](./phase-3-peer-payments-research.md) — Research findings
- [helper-4-branding-colors-logos.md](./helper-4-branding-colors-logos.md) — Hodos brand assets

---

**End of Document**
