# UX Design Considerations

This document outlines user experience design considerations for HodosBrowser wallet interactions. These designs prioritize user understanding, security, and informed consent.

---

## Table of Contents

1. [Wallet Creation](#1-wallet-creation)
2. [Wallet Recovery](#2-wallet-recovery)
3. [Wallet Interface and Rendering](#3-wallet-interface-and-rendering)
4. [Privileged Identity Access](#4-privileged-identity-access)
5. [Blind Message Attacks](#5-blind-message-attacks)

---

## 1. Wallet Creation

### Overview

Wallet creation is the user's first interaction with the system. It must be:
- Simple enough for non-technical users
- Secure enough to protect real funds
- Educational without being overwhelming

### Design Considerations

**TODO**: Design the following flows:

- [ ] Mnemonic generation and display
- [ ] Mnemonic verification step (confirm user wrote it down)
- [ ] Password/PIN creation for local encryption
- [ ] Backup reminder scheduling
- [ ] First-run tutorial/onboarding

### Security Requirements

- Mnemonic must never be stored in plaintext after initial display
- User must confirm they've backed up before proceeding
- Clear warnings about mnemonic loss = fund loss

---

## 2. Wallet Recovery

### Overview

Recovery flows must balance security with usability. Users in recovery mode are often stressed (lost device, forgotten password).

### Design Considerations

**TODO**: Design the following flows:

- [ ] Mnemonic entry interface (word-by-word vs full paste)
- [ ] Autocomplete for BIP-39 words
- [ ] Error handling for invalid mnemonics
- [ ] Progress indication during wallet restoration
- [ ] Handling partial recovery (some data restored, some lost)

### Security Requirements

- Rate limiting on recovery attempts
- Clear indication of what data is being restored
- Warning if recovering into an existing wallet (overwrite risk)

---

## 3. Wallet Interface and Rendering

### Overview

The wallet overlay must communicate complex blockchain concepts in accessible terms.

### Design Considerations

**TODO**: Design the following components:

- [ ] Balance display (confirmed vs unconfirmed)
- [ ] Transaction history with status indicators
- [ ] Address management UI
- [ ] Certificate display and management
- [ ] Basket/token organization views
- [ ] Settings and preferences

### Rendering Considerations

- Overlay positioning and sizing
- Keyboard navigation support
- Screen reader accessibility
- Dark/light mode theming
- Responsive layout for different window sizes

---

## 4. Privileged Identity Access

### Overview

When applications request access to the user's master identity key (privileged access), users must understand the privacy implications and make an informed choice.

### When This Prompt Appears

- Application requests `getPublicKey(identityKey=true, privileged=true)`
- Application requests certificate with `privileged=true`
- Any operation requiring master key exposure

### Prompt Design

```
┌─────────────────────────────────────────────────────────────┐
│  🔐 Identity Access Request                                 │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  "example.com" is requesting access to your                 │
│  MASTER IDENTITY KEY                                        │
│                                                             │
│  ⚠️  Privacy Implications:                                  │
│  • This app and others can link your activity               │
│  • Your identity becomes trackable across apps              │
│                                                             │
│  ✅ Benefits:                                               │
│  • Certificates work across all apps                        │
│  • Easier identity verification                             │
│                                                             │
│  ┌─────────────┐  ┌──────────────────────────────┐         │
│  │   Deny      │  │  Allow (use master key)      │         │
│  └─────────────┘  └──────────────────────────────┘         │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### Design Principles

1. **Default to Privacy**: Non-privileged (app-scoped) identity should be the default
2. **Clear Trade-offs**: User must understand what they're giving up and gaining
3. **Reversibility**: Make it clear this is a per-request decision, not permanent
4. **Remember Choice**: Option to "Always allow for this app" with easy revocation

### Implementation Status

- [ ] Prompt UI component
- [ ] Integration with permission system
- [ ] Preference storage for "remember" choices
- [ ] Revocation UI in settings

---

## 5. Blind Message Attacks

### Overview

"Blind message attacks" occur when applications request the wallet to sign, encrypt, or perform cryptographic operations on data the user cannot understand or verify. This is a significant security risk.

### Attack Vectors

1. **Signing arbitrary data**: App presents "Sign this message" but data could be a transaction
2. **Hidden transaction details**: Transaction appears legitimate but has hidden outputs
3. **Misleading descriptions**: App describes action as "Login" but requests fund transfer
4. **Rapid-fire requests**: Flood user with prompts hoping for accidental approval

---

### 5.1 Notification System

**Purpose**: Alert users to potentially dangerous operations.

**TODO**: Design notification system for:

- [ ] Transaction signing requests (show amount, recipient)
- [ ] Message signing requests (show message content when possible)
- [ ] Encryption requests (show what data is being encrypted)
- [ ] Certificate operations (show certificate details)
- [ ] Unusual patterns (rapid requests, high values)

**Design Considerations**:

- Notifications must be non-dismissible for high-risk operations
- Clear visual hierarchy: routine operations vs. dangerous ones
- Audio/haptic feedback for critical alerts
- Notification history for audit purposes

---

### 5.2 Guard Rails

**Purpose**: Prevent users from making dangerous mistakes.

**TODO**: Implement guard rails for:

- [ ] **Value thresholds**: Extra confirmation for transactions above X satoshis
- [ ] **Frequency limits**: Warn if many requests in short time period
- [ ] **New counterparty warnings**: First transaction to unknown address
- [ ] **Protocol restrictions**: Block known-dangerous protocol patterns
- [ ] **Domain verification**: Warn for suspicious/phishing domains

**Guard Rail Levels**:

| Level | Behavior | Example |
|-------|----------|---------|
| Info | Show notice, auto-proceed | Small routine transaction |
| Warning | Show notice, require click | First transaction to new address |
| Confirmation | Show details, require explicit approval | Transaction above threshold |
| Block | Prevent action, explain why | Known malicious pattern |

---

### 5.3 Auto-Approve Engine

**Purpose**: Reduce friction for legitimate, low-risk operations while maintaining security.

**TODO**: Design auto-approve rules for:

- [ ] **Trusted domains**: User-configured list of always-trusted apps
- [ ] **Protocol whitelisting**: Specific protocol+keyID combinations
- [ ] **Value limits**: Auto-approve below X satoshis per transaction/day
- [ ] **Rate limits**: Max N auto-approvals per minute/hour
- [ ] **Certificate types**: Auto-approve viewing certain certificate types

**Auto-Approve Configuration UI**:

```
┌─────────────────────────────────────────────────────────────┐
│  ⚡ Auto-Approve Settings                                   │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  Trusted Apps:                                              │
│  ┌─────────────────────────────────────────────────────┐   │
│  │ ✓ toolbsv.com         [Remove]                      │   │
│  │ ✓ handcash.io         [Remove]                      │   │
│  │ + Add trusted app...                                │   │
│  └─────────────────────────────────────────────────────┘   │
│                                                             │
│  Transaction Limits:                                        │
│  • Auto-approve up to: [____1000____] sats per tx          │
│  • Daily auto-approve limit: [___10000___] sats            │
│                                                             │
│  Rate Limits:                                               │
│  • Max auto-approvals: [__5__] per minute                  │
│  • Cooldown after limit: [__60__] seconds                  │
│                                                             │
│  ┌─────────────────┐                                       │
│  │  Save Settings  │                                       │
│  └─────────────────┘                                       │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

**Security Considerations**:

- Auto-approve must NEVER apply to:
  - Master identity key access
  - Certificate issuance
  - Transactions above threshold
  - Unknown/new domains
- Auto-approve settings should require password/PIN to modify
- Clear audit log of all auto-approved actions

---

## Implementation Priority

| Feature | Priority | Complexity | Status |
|---------|----------|------------|--------|
| Privileged Identity Prompt | High | Medium | Not Started |
| Value Threshold Guard Rails | High | Low | Not Started |
| Transaction Notifications | High | Medium | Partial |
| Auto-Approve Engine | Medium | High | Not Started |
| Wallet Creation Flow | Medium | Medium | Basic |
| Wallet Recovery Flow | Medium | Medium | Basic |
| Blind Message Detection | Low | High | Not Started |

---

## References

- [BRC-100 Wallet Interface Specification](../reference/BRC100_spec.md)
- [Security and Process Isolation Analysis](../SECURITY_AND_PROCESS_ISOLATION_ANALYSIS.md)
- [App-Scoped Identity Implementation](./APP_SCOPED_IDENTITY_IMPLEMENTATION.md)
