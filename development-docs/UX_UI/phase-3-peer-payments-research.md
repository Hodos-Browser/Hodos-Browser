# Phase 3: Peer Payments & Light Wallet Polish - Research Document

**Date:** 2026-02-26
**Status:** Research Complete
**Prerequisite:** Phase 2 (User Notifications) for permission prompts

---

## Executive Summary

This document covers research for Phase 3 of the UX/UI implementation:
1. **BRC-29 Peer-to-Peer Payments** — send/receive to identity pubkeys
2. **Unified Send Address Handling** — auto-detect address type (legacy, paymail, identity key)
3. **MessageBox Integration** — background polling for incoming payments
4. **Receive UX** — identity key vs legacy address options
5. **Recovery Enhancements** — raw private key recovery option
6. **Primary vs Privileged Keys** — BRC-100 dual-keyring clarification

---

## 1. Primary vs Privileged Keys (BRC-100)

### What Are They?

BRC-100 specifies **two separate keyrings**:

| Keyring | Purpose | Security Level |
|---------|---------|----------------|
| **Primary (Everyday)** | Normal wallet operations — payments, signing, identity | Standard |
| **Privileged** | Sensitive operations — high-value tx, admin functions | Higher security |

**From BRC-100 spec:**
> "Each wallet has an everyday master private key and a corresponding master public key derived from secp256k1. Additionally, there's a whole secondary 'privileged mode' keyring for sensitive operations, allowing these privileged keys to be treated with higher security than the user's everyday keyring."

### Why Two Keyrings?

1. **Security Isolation** — compromise of everyday key doesn't expose privileged operations
2. **UX Flexibility** — everyday ops can auto-approve; privileged always prompts
3. **Recovery Separation** — can recover one without the other (e.g., privileged from cold storage)

### Hodos Current State

**Currently: Single keyring only** — derived from mnemonic via BIP-39 → HD master key.

The `Wallet` model stores:
- `mnemonic` (encrypted or plain)
- `pin_salt` (for PIN-protected encryption)
- Single identity key derived from master

### Implementation Options for Privileged Key

**Option A: Derive from same mnemonic (different path)**
```
Primary:    m/44'/236'/0'/0/0  (current)
Privileged: m/44'/236'/0'/1/0  (new path)
```
- Pros: Single backup, deterministic
- Cons: Both compromised if mnemonic leaked

**Option B: Separate mnemonic/seed**
- Pros: True isolation, cold storage friendly
- Cons: Two backups, more complex UX

**Option C: Hardware wallet for privileged**
- Pros: Best security, key never on device
- Cons: Requires hardware, not always available

**Recommendation:** Start with Option A for MVP (same mnemonic, different derivation path). Document the path so users can derive privileged key manually if needed.

---

## 2. Recovery from Raw Private Key

### Current Recovery Methods

| Method | Status | Source |
|--------|--------|--------|
| Mnemonic (12/24 words) | ✅ Complete | BIP-39 |
| Encrypted backup file | ✅ Complete | Hodos JSON export |
| Centbee import | ✅ Complete | External wallet sweep |
| **Raw private key (hex)** | ❌ Missing | User request |

### Why Add Raw Key Recovery?

1. **Power users** — developers testing with known keys
2. **Emergency recovery** — when only raw key is available
3. **Migration** — importing from non-BIP39 wallets
4. **Compatibility** — some tools export raw keys, not mnemonics

### Implementation Plan

**New endpoint:** `POST /wallet/recover-from-key`

```typescript
interface RecoverFromKeyRequest {
  privateKey: string;      // 32-byte hex (64 chars) or WIF format
  keyType: 'primary' | 'privileged';  // Which keyring to import
  pin?: string;            // Optional PIN protection
}
```

**UI Flow:**
1. Recovery screen shows new option: "Recover from Private Key"
2. Input field for hex or WIF format
3. Dropdown: "Primary Key" or "Privileged Key" (future)
4. Standard PIN setup
5. Derive identity key, create wallet entry

**Security Warning:** Display prominent warning that raw key recovery provides no backup phrase — user must store the key securely themselves.

---

## 3. Paymail.us Bridge Analysis

### What Is It?

**Paymail Bridge** (`paymail.us`) is a **transitional service** for users without native paymail support:

- Creates `@paymail.us` addresses
- Store-and-forward model — payments held until collected
- Requires BRC-100 wallet (Metanet Desktop) to collect

### How It Works

```
1. User registers alias: alice@paymail.us
2. Someone sends BSV to alice@paymail.us
3. Payment stored on paymail.us server
4. Alice opens paymail.us, clicks "Collect"
5. Payment transferred to her BRC-100 wallet
```

### Should Hodos Use It?

**No — Hodos should implement native paymail instead.**

| Aspect | paymail.us Bridge | Native Paymail |
|--------|-------------------|----------------|
| Custody | Temporary custody by bridge | Direct to user |
| UX | Two-step (send, then collect) | Direct delivery |
| Trust | Requires trusting bridge | Peer-to-peer |
| Domain | Fixed @paymail.us | Custom domain possible |
| Offline | Stored until collected | Real-time (or MessageBox) |

**Recommendation:** 
- Implement native paymail resolution for SENDING (resolve `alice@handcash.io` → address)
- Use BRC-29/MessageBox for RECEIVING (identity key based)
- Skip paymail.us bridge entirely

---

## 4. Unified Send Address System

### Address Type Detection

When user types in "Send To" field, auto-detect format:

| Pattern | Type | Resolution Method |
|---------|------|-------------------|
| `1...` / `3...` / `bc1...` | Legacy address | Direct use |
| `user@domain.tld` | Paymail | BRC-28 resolution |
| `02...` / `03...` (66 hex chars) | Identity pubkey | BRC-29 derivation |

### Implementation Flow

```typescript
async function resolveRecipient(input: string): Promise<ResolvedRecipient> {
  // 1. Check if valid BSV address
  if (isValidBsvAddress(input)) {
    return { type: 'address', address: input };
  }
  
  // 2. Check if paymail format
  if (isPaymailFormat(input)) {
    const resolved = await resolvePaymail(input);  // BRC-28
    return { type: 'paymail', address: resolved.address, paymail: input };
  }
  
  // 3. Check if identity pubkey (33-byte compressed hex)
  if (isIdentityKey(input)) {
    return { type: 'identity', identityKey: input };
    // Actual address derived at send time via BRC-29
  }
  
  throw new Error('Invalid recipient format');
}
```

### UI Feedback

As user types:
- Show detected type icon (📍 address, 📧 paymail, 🔑 identity)
- For paymail: resolve and show profile pic/name if available
- For identity: show truncated key with checkmark
- Red border if invalid format

### Paymail Resolution (BRC-28)

```typescript
async function resolvePaymail(paymail: string): Promise<PaymailInfo> {
  const [alias, domain] = paymail.split('@');
  
  // 1. Fetch capabilities
  const capUrl = `https://${domain}/.well-known/bsvalias`;
  const capabilities = await fetch(capUrl).then(r => r.json());
  
  // 2. Get payment destination
  const p2pUrl = capabilities.capabilities['2a40af698840'];  // P2P dest
  const destUrl = p2pUrl.replace('{alias}', alias).replace('{domain.tld}', domain);
  
  // 3. Request address
  const destination = await fetch(destUrl, {
    method: 'POST',
    body: JSON.stringify({ senderHandle: ourPaymail, amount: satoshis })
  }).then(r => r.json());
  
  return { address: destination.output, paymail };
}
```

---

## 5. BRC-29 Payment Flow (Send to Identity Key)

### Sending Payment

```typescript
async function sendToIdentityKey(recipientKey: string, amount: number) {
  // 1. Generate unique derivation values
  const derivationPrefix = generateRandomHex(16);
  
  // For each output:
  const outputs = [{
    derivationSuffix: generateRandomHex(8),
    amount: amount
  }];
  
  // 2. Derive payment key for recipient
  for (const output of outputs) {
    const invoiceNumber = `2-3241645161d8-${derivationPrefix} ${output.derivationSuffix}`;
    
    // BRC-42: Derive child public key from recipient's identity key
    const childPubKey = deriveChildPublicKey(
      ourPrivateKey,      // Sender's private key
      recipientKey,       // Recipient's identity public key
      invoiceNumber
    );
    
    // Create P2PKH script to derived key
    output.script = createP2PKHScript(childPubKey);
  }
  
  // 3. Build and sign transaction
  const tx = await createAction({
    outputs: outputs.map(o => ({
      script: o.script,
      satoshis: o.amount
    }))
  });
  
  // 4. Create BRC-29 payment message
  const paymentMessage = {
    protocol: '3241645161d8',
    senderIdentityKey: ourIdentityKey,
    derivationPrefix: derivationPrefix,
    transactions: [{
      ...tx.beef,
      outputs: outputs.reduce((acc, o, i) => {
        acc[i] = { suffix: o.derivationSuffix };
        return acc;
      }, {})
    }]
  };
  
  // 5. Send via MessageBox (BRC-33)
  await messageBoxClient.sendMessage({
    recipient: recipientKey,
    messageBox: 'payment_inbox',
    body: JSON.stringify(paymentMessage)
  });
}
```

### Receiving Payment

```typescript
async function processIncomingPayment(message: PeerMessage) {
  const payment = JSON.parse(message.body);
  
  // Validate protocol
  if (payment.protocol !== '3241645161d8') {
    throw new Error('Unknown payment protocol');
  }
  
  // For each transaction in the payment
  for (const txEnvelope of payment.transactions) {
    // For each output marked for us
    for (const [outputIndex, outputInfo] of Object.entries(txEnvelope.outputs)) {
      const invoiceNumber = `2-3241645161d8-${payment.derivationPrefix} ${outputInfo.suffix}`;
      
      // Derive private key to spend this output
      const childPrivKey = deriveChildPrivateKey(
        ourPrivateKey,              // Our master private key
        payment.senderIdentityKey,  // Sender's identity public key
        invoiceNumber
      );
      
      // Verify the script matches
      const expectedScript = createP2PKHScript(childPrivKey.toPublicKey());
      const actualScript = txEnvelope.rawTx.outputs[outputIndex].script;
      
      if (expectedScript !== actualScript) {
        throw new Error('Script mismatch - invalid payment');
      }
      
      // Internalize the output
      await internalizeAction({
        tx: txEnvelope,
        outputs: [{
          outputIndex: parseInt(outputIndex),
          protocol: 'wallet payment',
          paymentRemittance: {
            derivationPrefix: payment.derivationPrefix,
            derivationSuffix: outputInfo.suffix,
            senderIdentityKey: payment.senderIdentityKey
          }
        }]
      });
    }
  }
  
  // Acknowledge message (removes from inbox)
  await messageBoxClient.acknowledgeMessage({
    messageIds: [message.messageId]
  });
}
```

---

## 6. Receive UX Design

### Current Behavior
- User clicks "Receive"
- New address generated and copied to clipboard

### Proposed New Behavior

**Receive Modal/Panel:**

```
┌─────────────────────────────────────────┐
│  Receive BSV                            │
├─────────────────────────────────────────┤
│                                         │
│  🔑 Your Identity Key                   │
│  ┌─────────────────────────────────┐   │
│  │ 02a1b2c3d4e5f6...               │   │
│  │ [Copy] [QR Code]                │   │
│  └─────────────────────────────────┘   │
│  Best for: BRC-29 wallets, apps         │
│                                         │
│  ─────────── OR ───────────             │
│                                         │
│  📍 Generate Legacy Address             │
│  ┌─────────────────────────────────┐   │
│  │ [Generate New Address]          │   │
│  └─────────────────────────────────┘   │
│  For: Traditional wallets, exchanges    │
│                                         │
└─────────────────────────────────────────┘
```

### Key Decisions

1. **Identity key is static** — same key always, no privacy concern (BRC-42 derivation handles privacy)
2. **Legacy addresses are one-time** — generate fresh each time for privacy
3. **Don't auto-generate** — let user choose which type they need
4. **Show QR for both** — mobile-friendly

---

## 7. MessageBox Background Polling

### Architecture

```
┌─────────────────────────────────────────────────────┐
│                   Hodos Browser                      │
├─────────────────────────────────────────────────────┤
│  ┌─────────────┐    ┌──────────────────────────┐   │
│  │   Frontend  │◄───│  Wallet Server (Rust)     │   │
│  │             │    │                          │   │
│  │  [Badge: 2] │    │  ┌────────────────────┐  │   │
│  │             │    │  │ MessageBox Poller  │  │   │
│  └─────────────┘    │  │ (background task)  │  │   │
│                     │  └─────────┬──────────┘  │   │
│                     └────────────┼─────────────┘   │
└──────────────────────────────────┼─────────────────┘
                                   │
                                   ▼
                      ┌────────────────────────┐
                      │  MessageBox Server     │
                      │  (babbage.systems or   │
                      │   self-hosted)         │
                      └────────────────────────┘
```

### Polling Implementation

```rust
// In rust-wallet, new background task
pub struct MessageBoxPoller {
    client: MessageBoxClient,
    poll_interval: Duration,  // Default: 30 seconds
    wallet_identity_key: String,
}

impl MessageBoxPoller {
    pub async fn start(&self) {
        loop {
            match self.poll_inbox().await {
                Ok(payments) => {
                    for payment in payments {
                        if let Err(e) = self.process_payment(payment).await {
                            log::error!("Failed to process payment: {}", e);
                        }
                    }
                }
                Err(e) => log::warn!("MessageBox poll failed: {}", e),
            }
            
            tokio::time::sleep(self.poll_interval).await;
        }
    }
    
    async fn poll_inbox(&self) -> Result<Vec<PeerMessage>> {
        self.client.list_messages(ListMessagesParams {
            message_box: "payment_inbox".to_string()
        }).await
    }
}
```

### Frontend Notification

When payment received:

1. **Badge on wallet icon** — show count of new payments
2. **Toast notification** — "Received 0.001 BSV from 02a1b2..."
3. **Activity indicator** — subtle animation on status bar
4. **Sound** (optional) — configurable in settings

```typescript
// Frontend polling for notification state
useEffect(() => {
  const checkNewPayments = async () => {
    const response = await fetch('/api/notifications/payments');
    const { count, recent } = await response.json();
    
    if (count > lastCount) {
      showToast(`Received ${recent[0].amount} sats`);
      setPaymentBadge(count);
    }
  };
  
  const interval = setInterval(checkNewPayments, 5000);
  return () => clearInterval(interval);
}, []);
```

---

## 8. Implementation Phases

### Phase 3a: Light Wallet Polish (Existing Plan)
- [ ] Balance display improvements
- [ ] Send form polish (amount, address validation)
- [ ] Receive modal redesign
- [ ] Recent transactions list
- [ ] QR code support

### Phase 3b: BRC-29 Peer Payments
- [ ] Identity key display in Receive modal
- [ ] BRC-29 payment message construction (send)
- [ ] BRC-29 payment validation (receive)
- [ ] MessageBox client integration (external server)
- [ ] Background poller implementation
- [ ] Payment notification system

### Phase 3c: Unified Send
- [ ] Address type detection
- [ ] Paymail resolution (BRC-28)
- [ ] Identity key resolution
- [ ] Send to field autocomplete/suggestions
- [ ] Visual feedback for detected type

### Phase 3d: Recovery Enhancements (Phase 1 Update)
- [ ] Add raw private key recovery option
- [ ] UI for key type selection (primary/privileged)
- [ ] Privileged keyring support (future)

---

## 9. External Dependencies

### MessageBox Server

**Options:**
1. **Public server:** `https://messagebox.babbage.systems` (Project Babbage)
2. **Self-hosted:** Run `messagebox-server` package

**For MVP:** Use public server. Add self-hosted option later.

### Overlay Network (for host discovery)

To find where a recipient's MessageBox is hosted:
- BRC-22/24 overlay lookup services
- Or fallback to well-known public server

**For MVP:** Assume all recipients use the public server. Add overlay lookup later.

---

## 10. Open Questions

1. **Multiple MessageBox servers?** — Should users be able to specify their preferred server?
2. **Offline payments?** — How long should MessageBox hold payments? (Current: indefinite with expiry config)
3. **Payment metadata?** — Should we support memo/note fields in BRC-29?
4. **Refund flow?** — If recipient rejects, how to handle refund?
5. **Rate limiting?** — How to prevent inbox spam?

---

## References

- [BRC-29: Simple Authenticated BSV P2PKH Payment Protocol](https://bsv.brc.dev/payments/0029)
- [BRC-33: PeerServ Message Relay Interface](https://bsv.brc.dev/peer-to-peer/0033)
- [BRC-42: BSV Key Derivation Scheme (BKDS)](https://bsv.brc.dev/key-derivation/0042)
- [BRC-100: Unified Wallet Interface](https://bsv.brc.dev/wallet/0100)
- [BRC-103: Peer-to-Peer Mutual Authentication](https://bsv.brc.dev/peer-to-peer/0103)
- [@bsv/message-box-client](https://github.com/bsv-blockchain/message-box-client)
- [Paymail Protocol (BRC-28)](https://bsv.brc.dev/payments/0028)

---

**End of Document**
