# Paymail Implementation Guide

## What is Paymail?

Paymail replaces cryptographic Bitcoin addresses with human-readable email-like identifiers:

```
Traditional: 1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa
Paymail:     alice@example.com
```

**Core benefits:**
- Human-readable and memorable
- No blockchain scanning required (direct transaction delivery)
- Capability-based extensibility
- DNS-based service discovery

---

## How Paymail Works

### Address Format

```
<alias>@<domain.tld>
```
- `alias`: Alphanumeric, periods, hyphens only
- `domain.tld`: Standard domain name

### Service Discovery (2 steps)

**Step 1: DNS SRV Lookup**

Query `_bsvalias._tcp.<domain.tld>` to find the paymail server:

```
_bsvalias._tcp.example.com → paymail.example.com:443
```

If no SRV record exists, fall back to `https://<domain.tld>/.well-known/bsvalias`

**Step 2: Capability Discovery**

GET `https://<host>:<port>/.well-known/bsvalias`:

```json
{
  "bsvalias": "1.0",
  "capabilities": {
    "pki": "https://paymail.example.com/api/v1/id/{alias}@{domain.tld}",
    "paymentDestination": "https://paymail.example.com/api/v1/address/{alias}@{domain.tld}",
    "2a40af698840": "https://paymail.example.com/api/v1/p2p-destination/{alias}@{domain.tld}",
    "5f1323cddf31": "https://paymail.example.com/api/v1/receive-tx/{alias}@{domain.tld}",
    "5c55a7fdb7bb": "https://paymail.example.com/api/v1/beef/{alias}@{domain.tld}"
  }
}
```

---

## BRC Specifications Overview

| BRC | Name | Purpose | Priority for HodosBrowser |
|-----|------|---------|---------------------------|
| **BRC-28** | P2P Payment Destinations | Request payment scripts from paymail | **High** - Core sending |
| **BRC-70** | Paymail BEEF Transactions | Send BEEF-formatted transactions | **High** - SPV compliance |
| **BRC-29** | Simple P2PKH Payments | BRC-42 derived payment addresses | **Already implemented** |
| **BRC-27** | Direct Payment Protocol | Invoice-based payments | Medium - E-commerce |
| **BRC-54** | Hybrid Payment Mode | Multi-token payments (BSV + tokens) | Low - Token support |
| **BRC-85** | PIKE | Secure key exchange with TOTP | Low - Contact verification |

---

## BRC-28: P2P Payment Destinations (Core)

This is the fundamental paymail payment flow.

### Capability IDs
- `2a40af698840` - P2P Payment Destination (request output scripts)
- `5f1323cddf31` - P2P Transaction (submit raw transaction)

### Payment Flow

```
┌─────────────┐                           ┌─────────────┐
│   Sender    │                           │  Recipient  │
│   Wallet    │                           │   Paymail   │
└──────┬──────┘                           └──────┬──────┘
       │                                         │
       │  1. DNS SRV lookup                      │
       │─────────────────────────────────────────►
       │                                         │
       │  2. GET /.well-known/bsvalias           │
       │─────────────────────────────────────────►
       │         capabilities response           │
       │◄─────────────────────────────────────────
       │                                         │
       │  3. POST /p2p-destination (amount)      │
       │─────────────────────────────────────────►
       │         output scripts + reference      │
       │◄─────────────────────────────────────────
       │                                         │
       │  4. Build transaction with outputs      │
       │                                         │
       │  5. POST /receive-tx (hex + metadata)   │
       │─────────────────────────────────────────►
       │         txid + confirmation             │
       │◄─────────────────────────────────────────
       │                                         │
       │  6. Recipient broadcasts to network     │
       │                                         │
```

### Request: P2P Payment Destination

**POST** to capability `2a40af698840`:

```json
{
  "satoshis": 10000
}
```

**Response:**

```json
{
  "outputs": [
    {
      "script": "76a914...88ac",
      "satoshis": 10000
    }
  ],
  "reference": "payment-ref-123"
}
```

### Request: P2P Transaction

**POST** to capability `5f1323cddf31`:

```json
{
  "hex": "0100000001...",
  "metadata": {
    "sender": "bob@wallet.com",
    "pubkey": "02abc123...",
    "signature": "304402...",
    "note": "Payment for coffee"
  },
  "reference": "payment-ref-123"
}
```

**Response:**

```json
{
  "txid": "abc123...",
  "note": "Payment received"
}
```

---

## BRC-70: Paymail BEEF Transactions

Same as BRC-28 P2P transactions but with BEEF format instead of raw hex.

### Capability ID
- `5c55a7fdb7bb` - BEEF Transaction endpoint

### Request Format

**POST** to capability `5c55a7fdb7bb`:

```json
{
  "beef": "0100beef...",
  "metadata": {
    "sender": "bob@wallet.com",
    "pubkey": "02abc123...",
    "signature": "304402...",
    "note": "Payment for coffee"
  },
  "reference": "payment-ref-123"
}
```

**Advantage:** Recipient can perform SPV validation before broadcasting.

---

## BRC-27: Direct Payment Protocol (DPP)

Invoice-based payment system for e-commerce scenarios.

### Flow

```
1. Merchant creates invoice → PaymentTerms
2. Customer wallet requests PaymentTerms
3. Wallet displays payment options to user
4. User authorizes → Payment message
5. Merchant validates → PaymentACK
6. Optional redirect to confirmation page
```

### PaymentTerms (from merchant)

```json
{
  "network": "bitcoin-sv",
  "version": "1.0",
  "creationTimestamp": 1704844800,
  "expirationTimestamp": 1704848400,
  "paymentUrl": "https://merchant.com/pay/invoice123",
  "beneficiary": {
    "name": "Coffee Shop",
    "email": "pay@coffeeshop.com",
    "avatar": "https://coffeeshop.com/logo.png"
  },
  "modes": {
    "bsv": {
      "outputs": [
        { "script": "76a914...88ac", "satoshis": 5000 }
      ]
    }
  }
}
```

### Payment (from customer)

```json
{
  "modeId": "bsv",
  "mode": {
    "transactions": ["0100000001..."]
  },
  "originator": {
    "name": "Bob",
    "paymail": "bob@wallet.com"
  },
  "memo": "Large latte"
}
```

### PaymentACK (from merchant)

```json
{
  "modeId": "bsv",
  "mode": {
    "transactionIds": ["abc123..."]
  },
  "redirectUrl": "https://merchant.com/thank-you"
}
```

---

## BRC-85: PIKE (Proven Identity Key Exchange)

Secure contact establishment with TOTP verification to prevent MITM attacks.

### Use Case

When Alice wants to exchange keys with Bob through potentially compromised servers:

1. Both parties generate ECDH key pairs
2. Exchange public keys through paymail infrastructure
3. Both derive shared secret independently
4. Exchange TOTPs out-of-band (phone, in-person) to verify
5. If TOTPs match, keys are trusted

### When to Implement

- Contact list with verified identities
- End-to-end encrypted messaging
- High-security payment channels

**Priority for HodosBrowser:** Low - nice-to-have for contact verification

---

## Implementation Plan for HodosBrowser

### Phase 1: Paymail Resolution (Client-Side Only)

**Goal:** Send payments to paymail addresses

**Components:**

| Component | Location | Purpose |
|-----------|----------|---------|
| DNS SRV resolver | Rust | Query `_bsvalias._tcp` records |
| Capability fetcher | Rust | GET `/.well-known/bsvalias` |
| P2P destination client | Rust | Request output scripts |
| P2P transaction client | Rust | Submit transactions |

**New Rust Files:**

```
rust-wallet/src/paymail/
├── mod.rs              # Module exports
├── resolver.rs         # DNS SRV + capability discovery
├── p2p_destination.rs  # BRC-28 payment destination requests
└── p2p_transaction.rs  # BRC-28/70 transaction submission
```

**New Endpoints:**

| Endpoint | Method | Purpose |
|----------|--------|---------|
| `/paymail/resolve` | POST | Resolve paymail → capabilities |
| `/paymail/getDestination` | POST | Get payment scripts for amount |
| `/paymail/sendTransaction` | POST | Submit tx to recipient's paymail |

### Phase 2: BEEF Integration

**Goal:** Use BRC-70 for SPV-compliant paymail payments

- Detect if recipient supports `5c55a7fdb7bb` capability
- If yes, send BEEF format instead of raw hex
- Reuse existing BEEF generation from `signAction`

### Phase 3: DPP Support (Optional)

**Goal:** Handle invoice-based payments

- Parse `paymentUrl` from QR codes or links
- Fetch PaymentTerms
- Display payment options in UI
- Submit Payment and handle PaymentACK

### Phase 4: Paymail Hosting (Optional/Future)

**Goal:** Let users have their own paymail address

This requires running a server, so it's more relevant for the SDK/server version:

- Host `/.well-known/bsvalias` endpoint
- Implement P2P destination endpoint (generate addresses)
- Implement P2P transaction endpoint (receive payments)

---

## Implementation Details

### DNS SRV Resolution in Rust

```rust
// Using trust-dns-resolver crate
use trust_dns_resolver::TokioAsyncResolver;

async fn resolve_paymail_host(domain: &str) -> Result<(String, u16)> {
    let resolver = TokioAsyncResolver::tokio_from_system_conf()?;

    let srv_name = format!("_bsvalias._tcp.{}", domain);
    let response = resolver.srv_lookup(&srv_name).await;

    match response {
        Ok(lookup) => {
            if let Some(record) = lookup.iter().next() {
                Ok((record.target().to_string(), record.port()))
            } else {
                // Fallback to domain:443
                Ok((domain.to_string(), 443))
            }
        }
        Err(_) => {
            // No SRV record, use domain directly
            Ok((domain.to_string(), 443))
        }
    }
}
```

### Capability Discovery

```rust
#[derive(Debug, Deserialize)]
pub struct PaymailCapabilities {
    pub bsvalias: String,
    pub capabilities: HashMap<String, String>,
}

async fn get_capabilities(host: &str, port: u16) -> Result<PaymailCapabilities> {
    let url = format!("https://{}:{}/.well-known/bsvalias", host, port);
    let response = reqwest::get(&url).await?.json().await?;
    Ok(response)
}

// Capability IDs
const P2P_DESTINATION: &str = "2a40af698840";
const P2P_TRANSACTION: &str = "5f1323cddf31";
const BEEF_TRANSACTION: &str = "5c55a7fdb7bb";
```

### P2P Payment Destination Request

```rust
#[derive(Debug, Serialize)]
pub struct PaymentDestinationRequest {
    pub satoshis: u64,
}

#[derive(Debug, Deserialize)]
pub struct PaymentDestinationResponse {
    pub outputs: Vec<PaymailOutput>,
    pub reference: String,
}

#[derive(Debug, Deserialize)]
pub struct PaymailOutput {
    pub script: String,  // Hex-encoded locking script
    pub satoshis: u64,
}

async fn get_payment_destination(
    capabilities: &PaymailCapabilities,
    paymail: &str,
    satoshis: u64,
) -> Result<PaymentDestinationResponse> {
    let template = capabilities.capabilities
        .get(P2P_DESTINATION)
        .ok_or("P2P destination not supported")?;

    let (alias, domain) = parse_paymail(paymail)?;
    let url = template
        .replace("{alias}", &alias)
        .replace("{domain.tld}", &domain);

    let response = reqwest::Client::new()
        .post(&url)
        .json(&PaymentDestinationRequest { satoshis })
        .send()
        .await?
        .json()
        .await?;

    Ok(response)
}
```

---

## Integration with Existing Code

### Where Paymail Fits

```
User enters: "Send 10000 sats to alice@example.com"
                    │
                    ▼
         ┌──────────────────┐
         │ Parse recipient  │
         │ Is it a paymail? │
         └────────┬─────────┘
                  │
      ┌───────────┴───────────┐
      │                       │
      ▼                       ▼
┌───────────┐           ┌───────────┐
│  Paymail  │           │ Standard  │
│  address  │           │  address  │
└─────┬─────┘           └─────┬─────┘
      │                       │
      ▼                       │
┌─────────────────┐           │
│ Resolve paymail │           │
│ Get destination │           │
└────────┬────────┘           │
         │                    │
         ▼                    │
┌─────────────────┐           │
│ Build outputs   │           │
│ from response   │◄──────────┘
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ createAction    │  (existing)
│ signAction      │  (existing)
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ Submit via P2P  │  (new for paymail)
│ OR broadcast    │  (existing for addresses)
└─────────────────┘
```

### Frontend Changes

```typescript
// frontend/src/hooks/usePaymail.ts

interface PaymailResolution {
  host: string;
  port: number;
  capabilities: Record<string, string>;
  supportsBeef: boolean;
}

async function resolvePaymail(paymail: string): Promise<PaymailResolution> {
  const response = await fetch('http://localhost:3301/paymail/resolve', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ paymail })
  });
  return response.json();
}

async function sendToPaymail(paymail: string, satoshis: number, note?: string) {
  // 1. Resolve paymail
  const resolution = await resolvePaymail(paymail);

  // 2. Get payment destination
  const destination = await fetch('http://localhost:3301/paymail/getDestination', {
    method: 'POST',
    body: JSON.stringify({ paymail, satoshis })
  }).then(r => r.json());

  // 3. Create and sign transaction using existing flow
  const action = await createAction({
    outputs: destination.outputs.map(o => ({
      script: o.script,
      satoshis: o.satoshis
    }))
  });

  // 4. Submit to recipient's paymail server
  await fetch('http://localhost:3301/paymail/sendTransaction', {
    method: 'POST',
    body: JSON.stringify({
      paymail,
      reference: destination.reference,
      beef: action.beef,  // or hex if BEEF not supported
      note
    })
  });
}
```

---

## Dependencies

### New Rust Crates

```toml
# Cargo.toml additions
trust-dns-resolver = "0.23"  # DNS SRV resolution
```

### Existing Crates (Already Available)

- `reqwest` - HTTP client
- `serde` / `serde_json` - Serialization
- `secp256k1` - Signature for metadata

---

## Testing Strategy

### Unit Tests

1. Paymail address parsing (`alice@example.com` → `("alice", "example.com")`)
2. Capability URL template substitution
3. Output script validation

### Integration Tests

1. Resolve known paymail providers (HandCash, MoneyButton archives)
2. Request payment destinations from test paymails
3. End-to-end: send to paymail, verify recipient receives

### Real-World Test Targets

| Provider | Paymail Domain | Notes |
|----------|----------------|-------|
| HandCash | handcash.io | Popular wallet |
| Centbee | centbee.com | Mobile wallet |
| RelayX | relayx.io | Exchange/wallet |
| Simply Cash | simply.cash | Web wallet |

---

## Open Questions

1. **Do we need to host our own paymail?**
   - For receiving: Yes, eventually
   - For sending: No, client-only is sufficient

2. **DNS resolution in browser context?**
   - Browsers can't do DNS SRV lookups directly
   - Rust backend handles all resolution

3. **What if recipient doesn't support BEEF?**
   - Fall back to raw hex via `5f1323cddf31`
   - Check capabilities first

4. **Identity linkage with BRC-100?**
   - Paymail public key should match BRC-100 identity key
   - Need to decide on key derivation strategy

---

## References

- [BRC-28: Paymail Payment Destinations](https://bsv.brc.dev/payments/0028)
- [BRC-70: Paymail BEEF Transactions](https://bsv.brc.dev/payments/0070)
- [BRC-27: Direct Payment Protocol](https://bsv.brc.dev/payments/0027)
- [BRC-54: Hybrid Payment Mode](https://bsv.brc.dev/payments/0054)
- [BRC-85: PIKE](https://bsv.brc.dev/peer-to-peer/0085)
- [Paymail Specification (original)](https://docs.moneybutton.com/docs/paymail-overview.html)

---

**Created**: January 9, 2025
**Status**: Research/Planning
