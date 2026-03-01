# Phase 3a: BRC-29 Peer-to-Peer Payments

**Date:** 2026-02-26
**Status:** 📋 Planning
**Priority:** High
**Prerequisite:** Phase 2 (User Notifications) for permission prompts
**Depends On:** BRC-33 Message Relay (✅ Complete), BRC-42 Key Derivation (✅ Complete)

---

## Overview

Implement BRC-29 peer-to-peer payments allowing users to send and receive BSV using identity public keys (not just addresses or paymail). This enables direct wallet-to-wallet payments without relying on payment servers.

---

## Goals

1. **Send to identity key** — User can paste a 33-byte compressed pubkey and send BSV
2. **Receive via identity key** — User can share their identity key to receive payments
3. **Unified send field** — Auto-detect address type (legacy, paymail, identity key)
4. **Background polling** — Automatically fetch incoming payments from MessageBox
5. **Notifications** — Alert user when payment received

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                        Hodos Browser                             │
├─────────────────────────────────────────────────────────────────┤
│  ┌──────────────┐         ┌─────────────────────────────────┐  │
│  │   Frontend   │◄───────►│      Rust Wallet Server         │  │
│  │              │         │                                 │  │
│  │ • Send form  │         │  ┌───────────────────────────┐  │  │
│  │ • Receive UI │         │  │  BRC-29 Payment Handler   │  │  │
│  │ • Notify     │         │  │  • constructPayment()     │  │  │
│  │              │         │  │  • processIncoming()      │  │  │
│  └──────────────┘         │  └───────────────────────────┘  │  │
│                           │                                 │  │
│                           │  ┌───────────────────────────┐  │  │
│                           │  │  MessageBox Poller        │  │  │
│                           │  │  • poll payment_inbox     │  │  │
│                           │  │  • process & acknowledge  │  │  │
│                           │  └───────────┬───────────────┘  │  │
│                           └──────────────┼──────────────────┘  │
└──────────────────────────────────────────┼─────────────────────┘
                                           │
                                           ▼
                              ┌────────────────────────┐
                              │   MessageBox Server    │
                              │  (External Service)    │
                              │  • Store messages      │
                              │  • Deliver on poll     │
                              └────────────────────────┘
```

---

## Specification

### BRC-29 Payment Message Format

```typescript
interface BRC29PaymentMessage {
  protocol: '3241645161d8';           // Magic number for BRC-29
  senderIdentityKey: string;          // Sender's 33-byte compressed pubkey (hex)
  derivationPrefix: string;           // Unique per-payment (random hex)
  transactions: BRC29Transaction[];   // One or more transaction envelopes
}

interface BRC29Transaction {
  // Standard BRC-8 transaction envelope fields
  rawTx: string;                      // Raw transaction hex
  inputs?: TransactionInput[];        // Input metadata
  mapiResponses?: MapiResponse[];     // Broadcast responses
  proof?: MerkleProof;                // SPV proof if confirmed
  
  // BRC-29 extension
  outputs: {
    [outputIndex: string]: {
      suffix: string;                 // derivationSuffix for this output
    }
  };
}
```

### Key Derivation (BRC-42)

Invoice number format: `"2-3241645161d8-{derivationPrefix} {derivationSuffix}"`

- Security Level: `2` (counterparty-specific)
- Protocol ID: `3241645161d8` (BRC-29 magic)
- Key ID: `{derivationPrefix} {derivationSuffix}`

---

## Implementation Plan

### 3a.1: Send to Identity Key

#### API Endpoint

**`POST /wallet/send-to-identity`**

```typescript
// Request
interface SendToIdentityRequest {
  recipientIdentityKey: string;  // 33-byte compressed pubkey (hex)
  amount: number;                // Satoshis
  description?: string;          // Optional memo
}

// Response
interface SendToIdentityResponse {
  success: boolean;
  txid?: string;
  messageId?: string;  // MessageBox message ID
  error?: string;
}
```

#### Implementation

```rust
pub async fn send_to_identity(
    state: web::Data<AppState>,
    req: web::Json<SendToIdentityRequest>,
) -> HttpResponse {
    log::info!("💸 /wallet/send-to-identity called");
    log::info!("   Recipient: {}...", &req.recipient_identity_key[..16]);
    log::info!("   Amount: {} sats", req.amount);
    
    // 1. Validate recipient identity key
    let recipient_pubkey = match validate_identity_key(&req.recipient_identity_key) {
        Ok(pk) => pk,
        Err(e) => return error_response("INVALID_RECIPIENT", &e.to_string())
    };
    
    // 2. Generate derivation values
    let derivation_prefix = generate_random_hex(16);  // 16 bytes = 32 hex chars
    let derivation_suffix = generate_random_hex(8);   // 8 bytes = 16 hex chars
    
    // 3. Derive payment public key for recipient
    let invoice_number = format!("2-3241645161d8-{} {}", derivation_prefix, derivation_suffix);
    
    let payment_pubkey = derive_child_public_key(
        &state.get_private_key()?,    // Our private key
        &recipient_pubkey,             // Recipient's identity pubkey
        &invoice_number
    )?;
    
    // 4. Create P2PKH output script
    let output_script = create_p2pkh_script(&payment_pubkey);
    
    // 5. Build transaction via createAction
    let tx_result = state.create_action(CreateActionParams {
        description: req.description.clone().unwrap_or("BRC-29 payment".into()),
        outputs: vec![ActionOutput {
            script: output_script,
            satoshis: req.amount,
            description: Some("Payment output".into()),
            basket: None,
            tags: None,
        }],
        labels: vec!["brc29-sent".into()],
        ..Default::default()
    }).await?;
    
    // 6. Construct BRC-29 payment message
    let payment_message = BRC29PaymentMessage {
        protocol: "3241645161d8".into(),
        sender_identity_key: state.get_identity_key()?,
        derivation_prefix,
        transactions: vec![BRC29Transaction {
            beef: tx_result.beef,
            outputs: hashmap! {
                "0".into() => OutputDerivation { suffix: derivation_suffix }
            }
        }]
    };
    
    // 7. Send via MessageBox
    let message_box_client = state.get_message_box_client()?;
    let send_result = message_box_client.send_message(SendMessageParams {
        recipient: req.recipient_identity_key.clone(),
        message_box: "payment_inbox".into(),
        body: serde_json::to_string(&payment_message)?,
        skip_encryption: Some(false),  // Encrypt by default
    }).await?;
    
    HttpResponse::Ok().json(json!({
        "success": true,
        "txid": tx_result.txid,
        "messageId": send_result.message_id
    }))
}
```

### 3a.2: Receive via Identity Key

#### Display Identity Key

Update Light Wallet receive modal to show identity key option.

```typescript
// Frontend component
function ReceiveModal() {
  const [identityKey, setIdentityKey] = useState<string>('');
  const [legacyAddress, setLegacyAddress] = useState<string>('');
  
  useEffect(() => {
    // Fetch identity key on mount
    fetch('/getPublicKey', {
      method: 'POST',
      body: JSON.stringify({ identityKey: true })
    })
    .then(r => r.json())
    .then(data => setIdentityKey(data.publicKey));
  }, []);
  
  const generateLegacyAddress = async () => {
    const response = await fetch('/wallet/new-address');
    const data = await response.json();
    setLegacyAddress(data.address);
  };
  
  return (
    <Modal title="Receive BSV">
      <Section>
        <Label>🔑 Your Identity Key</Label>
        <Description>For BRC-29 compatible wallets and apps</Description>
        <CopyableField value={identityKey} />
        <QRCode value={identityKey} />
      </Section>
      
      <Divider>OR</Divider>
      
      <Section>
        <Label>📍 Legacy Address</Label>
        <Description>For exchanges and traditional wallets</Description>
        {legacyAddress ? (
          <>
            <CopyableField value={legacyAddress} />
            <QRCode value={legacyAddress} />
          </>
        ) : (
          <Button onClick={generateLegacyAddress}>
            Generate New Address
          </Button>
        )}
      </Section>
    </Modal>
  );
}
```

### 3a.3: Background MessageBox Poller

#### Rust Implementation

```rust
use tokio::time::{interval, Duration};

pub struct MessageBoxPoller {
    state: Arc<AppState>,
    client: MessageBoxClient,
    poll_interval: Duration,
}

impl MessageBoxPoller {
    pub fn new(state: Arc<AppState>, host: &str) -> Self {
        Self {
            state: state.clone(),
            client: MessageBoxClient::new(MessageBoxClientOptions {
                host: Some(host.into()),
                wallet_client: None,  // We'll sign manually
                ..Default::default()
            }),
            poll_interval: Duration::from_secs(30),
        }
    }
    
    pub async fn start(self) {
        log::info!("📬 Starting MessageBox poller (interval: {:?})", self.poll_interval);
        
        let mut interval = interval(self.poll_interval);
        
        loop {
            interval.tick().await;
            
            if let Err(e) = self.poll_and_process().await {
                log::warn!("MessageBox poll error: {}", e);
            }
        }
    }
    
    async fn poll_and_process(&self) -> Result<()> {
        // 1. List messages in payment_inbox
        let messages = self.client.list_messages(ListMessagesParams {
            message_box: "payment_inbox".into()
        }).await?;
        
        if messages.is_empty() {
            return Ok(());
        }
        
        log::info!("📨 Found {} incoming payment(s)", messages.len());
        
        // 2. Process each message
        let mut processed_ids = Vec::new();
        
        for message in messages {
            match self.process_payment(&message).await {
                Ok(()) => {
                    processed_ids.push(message.message_id.clone());
                    
                    // Notify frontend
                    self.state.notify_payment_received(&message).await;
                }
                Err(e) => {
                    log::error!("Failed to process payment {}: {}", message.message_id, e);
                }
            }
        }
        
        // 3. Acknowledge processed messages
        if !processed_ids.is_empty() {
            self.client.acknowledge_message(AcknowledgeMessageParams {
                message_ids: processed_ids
            }).await?;
        }
        
        Ok(())
    }
    
    async fn process_payment(&self, message: &PeerMessage) -> Result<()> {
        // 1. Parse BRC-29 payment message
        let payment: BRC29PaymentMessage = serde_json::from_str(&message.body)?;
        
        // 2. Validate protocol
        if payment.protocol != "3241645161d8" {
            return Err(anyhow!("Unknown payment protocol: {}", payment.protocol));
        }
        
        // 3. Process each transaction
        for tx_envelope in &payment.transactions {
            self.process_transaction(&payment, tx_envelope).await?;
        }
        
        Ok(())
    }
    
    async fn process_transaction(
        &self,
        payment: &BRC29PaymentMessage,
        tx_envelope: &BRC29Transaction
    ) -> Result<()> {
        let our_private_key = self.state.get_private_key()?;
        
        // For each output marked for us
        for (output_index_str, output_info) in &tx_envelope.outputs {
            let output_index: usize = output_index_str.parse()?;
            
            // Derive the private key for this output
            let invoice_number = format!(
                "2-3241645161d8-{} {}",
                payment.derivation_prefix,
                output_info.suffix
            );
            
            let child_private_key = derive_child_private_key(
                &our_private_key,
                &payment.sender_identity_key,
                &invoice_number
            )?;
            
            // Verify the script matches what we'd generate
            let expected_pubkey = child_private_key.to_public_key();
            let expected_script = create_p2pkh_script(&expected_pubkey);
            
            // Parse transaction to verify
            let tx = Transaction::from_beef(&tx_envelope.beef)?;
            let actual_script = &tx.outputs[output_index].script;
            
            if hex::encode(&expected_script) != hex::encode(actual_script) {
                return Err(anyhow!("Script mismatch for output {}", output_index));
            }
            
            // Internalize the output
            self.state.internalize_action(InternalizeActionParams {
                tx: tx_envelope.beef.clone(),
                outputs: vec![InternalizeOutput {
                    output_index: output_index as u32,
                    protocol: "wallet payment".into(),
                    insertion_proof: false,
                    payment_remittance: Some(PaymentRemittance {
                        derivation_prefix: payment.derivation_prefix.clone(),
                        derivation_suffix: output_info.suffix.clone(),
                        sender_identity_key: payment.sender_identity_key.clone(),
                    }),
                    basket: None,
                    tags: Some(vec!["brc29-received".into()]),
                }],
                labels: vec!["brc29-received".into()],
                description: format!("Payment from {}", &payment.sender_identity_key[..16]),
            }).await?;
            
            log::info!("✅ Internalized output {} from tx", output_index);
        }
        
        Ok(())
    }
}
```

### 3a.4: Unified Send Address Detection

```typescript
// Frontend utility
type RecipientType = 'address' | 'paymail' | 'identity';

interface ResolvedRecipient {
  type: RecipientType;
  original: string;
  // For address type
  address?: string;
  // For paymail type
  paymail?: string;
  resolvedAddress?: string;
  // For identity type
  identityKey?: string;
}

async function resolveRecipient(input: string): Promise<ResolvedRecipient> {
  const trimmed = input.trim();
  
  // 1. Check for identity key (33-byte compressed pubkey = 66 hex chars)
  if (/^(02|03)[0-9a-fA-F]{64}$/.test(trimmed)) {
    return {
      type: 'identity',
      original: trimmed,
      identityKey: trimmed
    };
  }
  
  // 2. Check for paymail format (something@something.something)
  if (/^[a-zA-Z0-9._-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$/.test(trimmed)) {
    try {
      const resolved = await resolvePaymail(trimmed);
      return {
        type: 'paymail',
        original: trimmed,
        paymail: trimmed,
        resolvedAddress: resolved.address
      };
    } catch (e) {
      throw new Error(`Failed to resolve paymail: ${e.message}`);
    }
  }
  
  // 3. Check for legacy BSV address
  if (/^[13][a-km-zA-HJ-NP-Z1-9]{25,34}$/.test(trimmed)) {
    return {
      type: 'address',
      original: trimmed,
      address: trimmed
    };
  }
  
  throw new Error('Invalid recipient format. Use address, paymail, or identity key.');
}

// Visual indicator component
function RecipientTypeIndicator({ type }: { type: RecipientType | null }) {
  if (!type) return null;
  
  const indicators = {
    address: { icon: '📍', label: 'Legacy Address' },
    paymail: { icon: '📧', label: 'Paymail' },
    identity: { icon: '🔑', label: 'Identity Key' }
  };
  
  const { icon, label } = indicators[type];
  
  return (
    <span className="recipient-type-badge">
      {icon} {label}
    </span>
  );
}
```

### 3a.5: Payment Notifications

```typescript
// Frontend notification system
interface PaymentNotification {
  id: string;
  type: 'received' | 'sent' | 'confirmed';
  amount: number;
  counterparty: string;  // Identity key (truncated) or paymail
  timestamp: number;
  txid: string;
}

// Poll for new payments (or use WebSocket when available)
function usePaymentNotifications() {
  const [notifications, setNotifications] = useState<PaymentNotification[]>([]);
  const [unreadCount, setUnreadCount] = useState(0);
  
  useEffect(() => {
    const poll = async () => {
      const response = await fetch('/wallet/notifications');
      const data = await response.json();
      
      if (data.newPayments?.length > 0) {
        // Add to notifications
        setNotifications(prev => [...data.newPayments, ...prev]);
        setUnreadCount(prev => prev + data.newPayments.length);
        
        // Show toast for most recent
        const latest = data.newPayments[0];
        toast.success(
          `Received ${formatSats(latest.amount)} from ${truncateKey(latest.counterparty)}`
        );
      }
    };
    
    const interval = setInterval(poll, 5000);
    return () => clearInterval(interval);
  }, []);
  
  return { notifications, unreadCount, markRead: () => setUnreadCount(0) };
}

// Badge component
function WalletButton() {
  const { unreadCount } = usePaymentNotifications();
  
  return (
    <button className="wallet-button">
      <WalletIcon />
      {unreadCount > 0 && (
        <span className="badge">{unreadCount}</span>
      )}
    </button>
  );
}
```

---

## Database Changes

### New Tables

```sql
-- Track incoming BRC-29 payments
CREATE TABLE brc29_incoming_payments (
    id INTEGER PRIMARY KEY,
    message_id TEXT NOT NULL UNIQUE,
    sender_identity_key TEXT NOT NULL,
    derivation_prefix TEXT NOT NULL,
    total_amount INTEGER NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',  -- pending, processed, failed
    received_at INTEGER NOT NULL,
    processed_at INTEGER,
    error_message TEXT
);

-- Track outgoing BRC-29 payments
CREATE TABLE brc29_outgoing_payments (
    id INTEGER PRIMARY KEY,
    recipient_identity_key TEXT NOT NULL,
    derivation_prefix TEXT NOT NULL,
    txid TEXT NOT NULL,
    message_id TEXT,
    total_amount INTEGER NOT NULL,
    status TEXT NOT NULL DEFAULT 'sent',  -- sent, delivered, confirmed
    sent_at INTEGER NOT NULL
);
```

---

## Configuration

### MessageBox Server

```rust
// In AppState or config
pub struct MessageBoxConfig {
    /// MessageBox server URL
    pub host: String,  // Default: "https://messagebox.babbage.systems"
    
    /// Poll interval in seconds
    pub poll_interval_secs: u64,  // Default: 30
    
    /// Enable background polling
    pub enable_polling: bool,  // Default: true
}
```

### User Settings (Future)

- Custom MessageBox server URL
- Poll interval adjustment
- Notification preferences (sound, toast, badge)

---

## Testing Plan

### Unit Tests

- [ ] BRC-29 payment message construction
- [ ] BRC-29 payment message parsing
- [ ] Key derivation for payments
- [ ] Script verification

### Integration Tests

- [ ] Send to identity key (end-to-end)
- [ ] Receive via identity key (end-to-end)
- [ ] MessageBox polling
- [ ] Address type detection

### Manual Tests

- [ ] Send to self (test both send and receive)
- [ ] Send to Metanet Desktop (interoperability)
- [ ] Receive from Metanet Desktop
- [ ] Invalid identity key handling
- [ ] Network failure handling

---

## Implementation Checklist

### Backend (Rust)

- [ ] `POST /wallet/send-to-identity` endpoint
- [ ] `MessageBoxPoller` background task
- [ ] BRC-29 payment message types
- [ ] Payment processing and internalization
- [ ] Notification endpoint for frontend
- [ ] Database tables for payment tracking

### Frontend

- [ ] Update Send form with unified recipient field
- [ ] Address type detection and indicators
- [ ] Update Receive modal with identity key display
- [ ] Payment notification system
- [ ] Badge count on wallet button
- [ ] Toast notifications

### External Integration

- [ ] MessageBox client library
- [ ] Paymail resolution (BRC-28)
- [ ] Configure public MessageBox server

---

## Timeline

| Task | Estimate |
|------|----------|
| Send to identity endpoint | 1.5 days |
| MessageBox poller | 1 day |
| Receive UI updates | 0.5 days |
| Unified send field | 1 day |
| Notifications | 0.5 days |
| Testing | 1 day |
| **Total** | **5.5 days** |

---

## Open Questions

1. **MessageBox server** — Use public (`babbage.systems`) or self-host?
2. **Encryption** — Always encrypt messages? (Spec allows plaintext)
3. **Refund flow** — How to handle rejected payments?
4. **Rate limiting** — Prevent inbox spam?
5. **Multiple outputs** — Support splitting into multiple outputs?

---

## References

- [BRC-29: Simple Authenticated BSV P2PKH Payment Protocol](https://bsv.brc.dev/payments/0029)
- [BRC-33: PeerServ Message Relay Interface](https://bsv.brc.dev/peer-to-peer/0033)
- [BRC-42: BSV Key Derivation Scheme](https://bsv.brc.dev/key-derivation/0042)
- [BRC-8: Everett-style Transaction Envelopes](https://bsv.brc.dev/transactions/0008)
- [@bsv/message-box-client](https://github.com/bsv-blockchain/message-box-client)

---

**End of Document**
