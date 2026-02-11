# User Notifications Interface Implementation Plan

## Overview

**Interface Type**: Popup modal (and related notification surfaces)
**Purpose**: Present permission and consent prompts to the user, governed by whitelist and escalation rules. Ensures sensitive operations (payments, identity, certificates) require explicit user action while reducing noise for trusted or low-risk cases.

**Status**: 📋 Planning Phase
**Last Updated**: 2026-02-11

---

## Interface Description

The User Notifications system:

- Shows **modals** when a site requests a sensitive action (payment, signing, certificate, PII).
- Can show **lightweight notifications** for first-time or low-risk requests (informational, dismissible).
- Uses **quiet indicators** for routine requests from trusted sites (no popup).
- Is driven by **domain whitelist**, **auto-approve rules**, and **escalation level** so the right level of interruption is used.

**Display Context**:

- **Modal**: Blocking prompt for payments, signing, certificate operations, identity disclosure.
- **Toast / inline notification**: Optional for “first-time site” or informational notices.
- **No UI**: Trusted domain + low-risk operation (handled by backend/indicator only).

---

## Escalation Levels (When to Show What)

Aligns with [Design Philosophy](./helper-2-design-philosophy.md) and [UX Considerations](./helper-3-ux-considerations.md).

| Level | Interruption | When to use | Example |
|-------|--------------|-------------|---------|
| **Quiet** | None | Routine requests from trusted sites; public key (non-PII); low-risk | Trusted app requests pubkey on load |
| **Notification** | Minimal (dismissible) | First-time site; routine from new site; informational | “example.com is requesting access” (one-time notice) |
| **Modal (Escalation Consent)** | Requires action | Certificates/PII, payments/transactions, identity disclosure, high-risk | “Approve payment of 1,000 sats to …?” |

**Escalation Consent** (high-risk): For payments, signing, certificate operations, and identity disclosure, the UI must require explicit user action (Allow / Deny / Block site). These prompts should be non-dismissible without a choice; see [Design Philosophy – Non-Annoying Permission Model](./helper-2-design-philosophy.md).

---

## Notification Types (To Implement)

Planned notification/prompt types (details to be refined per phase):

1. **Transaction / payment requests** – Show amount, recipient, optional memo; Approve / Deny / Block site.
2. **Message signing requests** – Show message content when safe; Approve / Deny.
3. **Encryption requests** – Show what data is being encrypted (summary); Approve / Deny.
4. **Certificate operations** – Show certificate type and “Why is this needed?”; Approve / Deny.
5. **Unusual patterns** – Rapid requests, high value, first-time counterparty: extra confirmation or warning.

**Design rules** (from [UX Considerations](./helper-3-ux-considerations.md)):

- Non-dismissible for high-risk operations.
- Clear hierarchy: routine vs dangerous.
- Optional: audio/haptic for critical alerts.
- Notification history for audit (outline only; implement in phase when needed).

---

## Requirements

### Functional Requirements

- [ ] Modal for transaction/payment approval (amount, recipient, site).
- [ ] Modal for message signing approval (message preview when possible).
- [ ] Modal for encryption approval (summary of data).
- [ ] Modal for certificate operations (type + short explanation).
- [ ] Optional lightweight notification for first-time or low-risk (dismissible).
- [ ] Integrate with domain whitelist and auto-approve rules (no modal when allowed by policy).
- [ ] Options: Allow Once / Always Allow / Deny / Block This Site where applicable.
- [ ] Clear pending state (e.g. “Waiting for your approval”) and result (approved / denied).

### Non-Functional Requirements

- [ ] Same notification format and patterns across types (consistency).
- [ ] Immediate feedback (pending → approved/denied).
- [ ] Accessible (keyboard, screen reader).
- [ ] Works with HTTP interceptor flow (request paused until user responds).

---

## Frontend Implementation (Outline)

### Component Structure

**Location**: `frontend/src/components/` (e.g. `UserNotificationModal.tsx`, `EscalationConsentModal.tsx`, or shared `PermissionModal.tsx`).

**Props (conceptual)**:

```typescript
interface UserNotificationModalProps {
  open: boolean;
  type: 'payment' | 'sign' | 'encrypt' | 'certificate' | 'info';
  site: string;
  payload: PaymentRequest | SignRequest | EncryptRequest | CertificateRequest; // type-specific
  onAllow: () => void;
  onDeny: () => void;
  onBlockSite?: () => void;
  allowOnce?: boolean;
  allowAlways?: boolean;
}
```

**State**: Open/closed, loading (sending response), error.

### Integration with Interceptor

- C++ interceptor pauses request and shows overlay (same pattern as [HTTP Interceptor Flow Guide](./HTTP_INTERCEPTOR_FLOW_GUIDE.md)).
- Frontend receives request via `window.*` or message; renders modal; sends Allow/Deny/Block via `window.cefMessage.send(...)`.
- Backend records decision and optional whitelist/blocklist update.

---

## CEF-Native / Backend (Outline)

- Reuse overlay + message pattern from BRC-100 auth and domain whitelist.
- New message types for: payment_request, sign_request, encrypt_request, certificate_request.
- Wallet/backend: persist “Block This Site” and “Always Allow” per domain + permission type (detailed schema later).

---

## Permissions Research (Outline – For Later Phase)

These are **outline questions** only; detailed research and design in the phase when we implement each.

**Payment requests**

- What exact payload does the wallet API receive (amount, address, memo, expiry)?
- How do we display “first-time counterparty” or “high value” warnings?
- How do we integrate with value thresholds and guard rails in [UX Considerations](./helper-3-ux-considerations.md)?

**Certificates**

- What certificate types do we support (auth, identity, custom)?
- What do we show in the modal (type, issuer, “Why is this needed?” link)?
- Which certificate operations are auto-approvable (e.g. read-only) vs always require consent?

**Other**

- Rate limiting and “rapid-fire” request handling.
- Notification history storage and audit UI (when needed).

---

## Related Documents

- [Design Philosophy](./helper-2-design-philosophy.md) – Escalation levels, non-annoying permissions.
- [UX Considerations](./helper-3-ux-considerations.md) – Notification system, guard rails, auto-approve.
- [HTTP Interceptor Flow Guide](./HTTP_INTERCEPTOR_FLOW_GUIDE.md) – How requests are paused and resumed.
- [Activity Status Indicator](./phase-5-activity-status-indicator.md) – Passive activity vs active notifications.

---

**End of Document**
