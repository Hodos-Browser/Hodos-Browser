# BRC-33 Message Relay Implementation Guide

## Why Message Relay Exists

This document explains the **purpose** of BRC-33 message relay, why it's necessary in the UTXO model, and how applications implement message box systems.

---

## Table of Contents

1. [The Fundamental Problem](#1-the-fundamental-problem)
2. [Why UTXO Makes This Hard](#2-why-utxo-makes-this-hard)
3. [The Message Relay Solution](#3-the-message-relay-solution)
4. [BRC-33 Protocol Design](#4-brc-33-protocol-design)
5. [Real-World Use Cases](#5-real-world-use-cases)
6. [Implementation Architecture](#6-implementation-architecture)
7. [HodosBrowser Current State](#7-hodosbrowser-current-state)
8. [Recommended Improvements](#8-recommended-improvements)
9. [Rust Implementation Patterns](#9-rust-implementation-patterns)

---

## 1. The Fundamental Problem

### The Bitcoin Whitepaper Promise

Bitcoin was designed as **"A Peer-to-Peer Electronic Cash System"** - direct exchange between parties without intermediaries. But there's a catch: Satoshi's original paper notes that **"no existing mechanism allows payment exchange over a communication channel."**

### What "Peer-to-Peer" Actually Means

The blockchain itself is NOT peer-to-peer between users. It's a broadcast network:

```
Alice → Miners → Blockchain ← Miners ← Bob
                    ↓
              (No direct path from Alice to Bob)
```

When Alice wants to pay Bob:
1. Alice creates a transaction giving Bob a UTXO
2. Alice broadcasts to the mining network
3. Miners include it in a block
4. But HOW does Bob know to look for it?

### The Notification Problem

In traditional payment systems, the recipient is notified:
- Credit card: "You received $50 from Alice"
- Bank transfer: "Deposit of $50 from Alice"
- PayPal: "Alice sent you $50"

**In Bitcoin, the blockchain doesn't notify anyone.** It just records transactions. Recipients must:
1. Know a payment is coming
2. Know which address(es) to monitor
3. Continuously scan the blockchain (expensive)

This is the **payment notification problem**.

---

## 2. Why UTXO Makes This Hard

### Account Model vs UTXO Model

**Account Model (Ethereum, banks):**
```
┌─────────────────┐      ┌─────────────────┐
│ Alice: $1000    │ ───▶ │ Alice: $950     │
│ Bob:   $500     │      │ Bob:   $550     │
└─────────────────┘      └─────────────────┘
```
- One address per user
- Easy to monitor: just watch your address
- Server can push notifications when balance changes

**UTXO Model (Bitcoin/BSV):**
```
┌─────────────────────┐      ┌─────────────────────┐
│ UTXO A: 1000 → Alice│      │ UTXO A: SPENT       │
│ UTXO B: 500 → Bob   │ ───▶ │ UTXO B: 500 → Bob   │
│                     │      │ UTXO C: 950 → Alice │
│                     │      │ UTXO D: 50 → Bob    │
└─────────────────────┘      └─────────────────────┘
```
- UTXOs are discrete, one-time outputs
- New addresses for privacy (one per transaction)
- Recipient needs to know:
  - Which transaction contains their payment
  - Which output index is theirs
  - What derivation path unlocks it

### The SPV Challenge

Simplified Payment Verification (SPV) lets light clients verify transactions without downloading the full blockchain. But SPV requires you to **already know which transaction to verify**.

```
SPV Question: "Is transaction X in block Y?"
NOT: "What transactions paid me?"
```

To find "what paid me," you'd need to:
1. Download every block header
2. Request Merkle proofs for all transactions
3. Check each transaction for outputs you control
4. This doesn't scale!

### The Address Derivation Problem

Modern wallets use HD (Hierarchical Deterministic) key derivation:
- `m/0` → address #1
- `m/1` → address #2
- etc.

If Alice pays Bob at `m/47`:
- Bob needs to know to check derivation index 47
- Bob might only be scanning indices 0-45
- Payment sits unnoticed until Bob extends his scan range

Even worse with BRC-42 key derivation:
- Key derived from `counterparty + protocol + keyID`
- Infinite possible combinations
- Can't brute-force scan for payments

---

## 3. The Message Relay Solution

### Core Insight

Since the blockchain can't notify recipients, we need an **out-of-band communication channel** to tell them:
1. A payment was made
2. Where to find it (transaction ID)
3. How to claim it (derivation parameters)

### Why Not Direct Connection?

Why not have Alice connect directly to Bob?

**Problem 1: NAT Traversal**
Most users are behind NAT (Network Address Translation):
```
Alice's Computer ← NAT ← Router ← Internet
                    ↓
     (Alice can reach out, but others can't reach in)
```

**Problem 2: Offline Recipients**
Bob might be:
- Sleeping
- Offline for vacation
- Using a different device
- In airplane mode

Direct connection requires both parties online simultaneously.

**Problem 3: Discovery**
How does Alice even find Bob's IP address? This reintroduces centralized registries.

### Store-and-Forward Architecture

The solution is a **message relay** that provides store-and-forward:

```
       Alice                 Message Relay                  Bob
         │                        │                          │
         │──── Send Message ─────▶│                          │
         │                        │  (stores message)        │
         │                        │                          │
         │                        │     (Bob comes online)   │
         │                        │◀───── List Messages ─────│
         │                        │─────── Messages ────────▶│
         │                        │                          │
         │                        │◀──── Acknowledge ────────│
         │                        │  (deletes message)       │
```

**Key Properties:**
- Asynchronous: parties don't need to be online simultaneously
- Decentralized: multiple relay servers can exist
- Simple: just store messages until retrieved

---

## 4. BRC-33 Protocol Design

### Architecture Overview

BRC-33 builds on BRC-31 (Authrite) authentication to create authenticated message boxes:

```
┌──────────────────────────────────────────────────────────────┐
│                     Message Relay Server                      │
│                                                               │
│  ┌─────────────────────────────────────────────────────────┐ │
│  │                     BRC-31 Authentication                │ │
│  │              (Verifies sender/recipient identity)        │ │
│  └─────────────────────────────────────────────────────────┘ │
│                              │                                │
│  ┌───────────────────────────┴───────────────────────────┐   │
│  │                    Message Boxes                        │  │
│  │                                                         │  │
│  │  Recipient: 02abc123...                                 │  │
│  │  ├── payment_inbox: [msg1, msg2]                        │  │
│  │  ├── certificate_inbox: [msg3]                          │  │
│  │  └── coinflip_game: [msg4, msg5, msg6]                  │  │
│  │                                                         │  │
│  │  Recipient: 02def456...                                 │  │
│  │  ├── payment_inbox: [msg7]                              │  │
│  │  └── notifications: []                                  │  │
│  │                                                         │  │
│  └─────────────────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────────────┘
```

### Why "Message Boxes"?

Different applications need different message streams:
- Payment notifications go to `payment_inbox`
- Game moves go to `coinflip_game`
- Certificate updates go to `certificate_inbox`

This prevents:
- Applications stepping on each other
- Message ordering confusion
- Privacy leaks between apps

### API Endpoints

#### POST /sendMessage

Send a message to a recipient's specific message box.

```json
{
  "message": {
    "recipient": "02abc123...",
    "messageBox": "payment_inbox",
    "body": "{\"txid\": \"...\", \"outputIndex\": 0, ...}"
  }
}
```

**Why recipient is a public key:** Identity is cryptographically verifiable. No need for usernames/passwords.

**Why body is opaque:** Different applications encode different data. The relay doesn't need to understand it.

#### POST /listMessages

Retrieve messages from your message box.

```json
{
  "messageBox": "payment_inbox"
}
```

Response:
```json
{
  "messages": [
    {
      "messageId": 42,
      "sender": "02def456...",
      "body": "{\"txid\": \"...\", ...}"
    }
  ]
}
```

**Why BRC-31 auth for listing:** Server needs to verify you're the legitimate recipient.

#### POST /acknowledgeMessage

Confirm receipt and delete messages.

```json
{
  "messageIds": [42, 43, 44]
}
```

**Why acknowledge?** Messages are for transport, not storage. Once processed, they should be deleted to:
- Free server resources
- Prevent duplicate processing
- Maintain privacy (don't leave message history on server)

### Authentication Flow

BRC-31 (Authrite) provides mutual authentication:

1. Client sends identity key + nonce
2. Server responds with its identity key + signature
3. All subsequent requests include signed authentication headers
4. Server verifies signatures to confirm identity

This means:
- Recipients can't impersonate each other
- Senders can't forge messages
- Messages can form encrypted channels on top

---

## 5. Real-World Use Cases

### Use Case 1: Direct Payments

**Scenario:** Alice wants to pay Bob 1000 satoshis.

**Without Message Relay:**
1. Alice creates transaction
2. Alice broadcasts to network
3. Bob... has no idea
4. Days later, Bob scans blockchain and finds it (maybe)

**With Message Relay:**
1. Alice creates transaction
2. Alice broadcasts to network
3. Alice sends Bob a payment notification:
   ```json
   {
     "type": "payment",
     "txid": "abc123...",
     "outputIndex": 1,
     "derivationPrefix": "m/purpose/coin/account",
     "derivationSuffix": "0/47",
     "amount": 1000
   }
   ```
4. Bob receives notification immediately
5. Bob validates transaction via SPV
6. Bob acknowledges receipt

### Use Case 2: Token Transfers

**Scenario:** Alice sends Bob a game sword token.

The token includes:
- PushDrop data (sword stats, history)
- BRC-42 derived locking key

Bob needs to know:
- The transaction ID
- The protocol ID used for derivation
- Any custom instructions for spending

```json
{
  "type": "token_transfer",
  "txid": "def456...",
  "outputIndex": 0,
  "protocolID": ["2", "game_tokens"],
  "keyID": "sword_42",
  "customInstructions": {
    "history": ["forged_by_blacksmith_npc", "enchanted_fire_20"]
  }
}
```

### Use Case 3: Certificate Issuance

**Scenario:** A certifier issues an identity certificate to Bob.

```json
{
  "type": "certificate_issuance",
  "txid": "ghi789...",
  "outputIndex": 0,
  "certificate": {
    "type": "verified_email",
    "fields": {
      "email": "<encrypted>"
    }
  },
  "decryptionKey": "<for Bob to decrypt fields>"
}
```

### Use Case 4: Multi-Party Games

**Scenario:** A coin flip game between Alice and Bob.

```
Alice → "I commit hash(secret_choice)"
Bob → "I commit hash(my_choice)"
Alice → "My choice was heads, secret was X"
Bob → "My choice was tails, secret was Y"
(Smart contract resolves winner)
```

Each move is a message in the `coinflip_game` message box.

### Use Case 5: Order Confirmations

**Scenario:** E-commerce payment confirmation.

After Bob pays for an item:
```json
{
  "type": "order_confirmation",
  "orderId": "12345",
  "status": "paid",
  "items": [...],
  "shipmentTrackingId": "USPS123456"
}
```

---

## 6. Implementation Architecture

### Federation: Why Multiple Servers?

A single message relay is a single point of failure. BRC-34 and BRC-35 enable federation:

```
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│   Relay Server  │◀───▶│   Relay Server  │◀───▶│   Relay Server  │
│       #1        │     │       #2        │     │       #3        │
└─────────────────┘     └─────────────────┘     └─────────────────┘
         ▲                      ▲                      ▲
         │                      │                      │
         └──────────────────────┼──────────────────────┘
                                │
                          ┌─────┴─────┐
                          │   User    │
                          │  Wallet   │
                          └───────────┘
```

**Benefits:**
- Redundancy: if one server is down, others work
- Competition: servers compete on price/quality
- Privacy: users choose who to trust
- Censorship resistance: no single authority

### WebSocket vs HTTP Polling

**HTTP Polling:**
```
Client: "Any messages?"
Server: "No"
(5 seconds later)
Client: "Any messages?"
Server: "No"
(5 seconds later)
Client: "Any messages?"
Server: "Yes, here's one"
```

**WebSocket:**
```
Client: "Connect and subscribe"
Server: (keeps connection open)
...
Server: "New message arrived!" (pushed immediately)
```

**Trade-offs:**

| Aspect | HTTP Polling | WebSocket |
|--------|-------------|-----------|
| Latency | High (poll interval) | Low (instant push) |
| Server load | Higher (repeated requests) | Lower (one connection) |
| Complexity | Simple | More complex |
| Firewall | Always works | Sometimes blocked |
| Offline tolerance | Natural | Needs reconnection logic |

**Recommendation:** Support both. WebSocket for real-time UX, HTTP for reliability.

### Persistence Strategies

**In-Memory (Current HodosBrowser):**
```rust
messages: Arc<Mutex<HashMap<String, HashMap<String, Vec<Message>>>>>
```
- Fast
- Simple
- Lost on restart
- Doesn't scale

**SQLite (Recommended):**
```sql
CREATE TABLE messages (
    id INTEGER PRIMARY KEY,
    recipient TEXT NOT NULL,
    message_box TEXT NOT NULL,
    sender TEXT NOT NULL,
    body TEXT NOT NULL,
    created_at INTEGER NOT NULL
);

CREATE INDEX idx_recipient_box ON messages(recipient, message_box);
```
- Persistent
- Queryable
- Scales to millions of messages
- Atomic operations

**Why SQLite over PostgreSQL/Redis?**

For a wallet that:
- Runs locally
- Serves one user primarily
- Needs zero configuration
- Must work offline

SQLite is ideal. PostgreSQL adds deployment complexity. Redis adds infrastructure.

---

## 7. HodosBrowser Current State

### What We Have

Location: `rust-wallet/src/message_relay.rs`

```rust
pub struct MessageStore {
    messages: Arc<Mutex<HashMap<String, HashMap<String, Vec<Message>>>>>,
    next_id: Arc<Mutex<u64>>,
}
```

Endpoints in `handlers.rs`:
- `POST /sendMessage` - Store a message
- `POST /listMessages` - Retrieve messages
- `POST /acknowledgeMessage` - Delete messages

### Limitations

1. **In-Memory Only**
   - Messages lost on wallet restart
   - No persistence across sessions
   - Can't handle large message volumes

2. **Single Server**
   - No federation support
   - Single point of failure
   - No redundancy

3. **No Encryption**
   - Messages stored in plaintext
   - Privacy depends on transport only

4. **No WebSocket**
   - Must poll for new messages
   - Higher latency for real-time apps

5. **No Message Expiry**
   - Old messages never auto-deleted
   - Could accumulate indefinitely (in memory)

6. **No Authentication Verification**
   - Extracts identity from headers
   - Doesn't cryptographically verify signatures

---

## 8. Recommended Improvements

### Phase 1: Persistence (Essential)

Add SQLite storage for messages:

```sql
-- New table in migrations.rs
CREATE TABLE IF NOT EXISTS relay_messages (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    recipient TEXT NOT NULL,
    message_box TEXT NOT NULL,
    sender TEXT NOT NULL,
    body TEXT NOT NULL,
    created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    expires_at INTEGER  -- optional expiry
);

CREATE INDEX idx_relay_recipient_box ON relay_messages(recipient, message_box);
CREATE INDEX idx_relay_expires ON relay_messages(expires_at) WHERE expires_at IS NOT NULL;
```

### Phase 2: Message Expiry

Prevent unbounded growth:

```rust
// Auto-delete messages older than 30 days
pub async fn cleanup_expired_messages(pool: &SqlitePool) -> Result<u64> {
    let cutoff = Utc::now().timestamp() - (30 * 24 * 60 * 60);
    sqlx::query("DELETE FROM relay_messages WHERE created_at < ?")
        .bind(cutoff)
        .execute(pool)
        .await
}
```

### Phase 3: WebSocket Support

Add real-time push notifications:

```rust
// Using actix-web-actors
use actix_web_actors::ws;

pub struct MessageWebSocket {
    identity_key: String,
    subscribed_boxes: HashSet<String>,
}

impl StreamHandler<ws::Message> for MessageWebSocket {
    fn handle(&mut self, msg: ws::Message, ctx: &mut Self::Context) {
        match msg {
            ws::Message::Text(text) => {
                // Handle subscribe/unsubscribe
            }
            ws::Message::Close(_) => ctx.stop(),
            _ => {}
        }
    }
}
```

### Phase 4: End-to-End Encryption

Encrypt message bodies with recipient's public key:

```rust
// Sender encrypts
let encrypted_body = encrypt_for_recipient(
    &body,
    &sender_private_key,
    &recipient_public_key,
)?;

// Recipient decrypts
let decrypted_body = decrypt_from_sender(
    &encrypted_body,
    &recipient_private_key,
    &sender_public_key,
)?;
```

This uses BRC-2 ECDH encryption - we already have the primitives in `crypto/brc2.rs`.

### Phase 5: Federation Support

Allow configuring multiple relay servers:

```rust
pub struct FederatedRelayClient {
    servers: Vec<RelayServerConfig>,
}

impl FederatedRelayClient {
    pub async fn send_message(&self, msg: &Message) -> Result<()> {
        // Send to all configured servers for redundancy
        for server in &self.servers {
            server.send(msg).await.ok(); // Best-effort
        }
        Ok(())
    }

    pub async fn list_messages(&self, box_name: &str) -> Result<Vec<Message>> {
        // Aggregate from all servers, deduplicate by message ID
        let mut all_messages = Vec::new();
        for server in &self.servers {
            if let Ok(msgs) = server.list(box_name).await {
                all_messages.extend(msgs);
            }
        }
        deduplicate(all_messages)
    }
}
```

---

## 9. Rust Implementation Patterns

### Pattern 1: Repository Layer

Separate storage logic from handlers:

```rust
// src/database/message_relay_repo.rs

pub struct MessageRelayRepository;

impl MessageRelayRepository {
    pub async fn send_message(
        pool: &SqlitePool,
        recipient: &str,
        message_box: &str,
        sender: &str,
        body: &str,
    ) -> Result<i64, sqlx::Error> {
        let result = sqlx::query(
            "INSERT INTO relay_messages (recipient, message_box, sender, body)
             VALUES (?, ?, ?, ?)"
        )
        .bind(recipient)
        .bind(message_box)
        .bind(sender)
        .bind(body)
        .execute(pool)
        .await?;

        Ok(result.last_insert_rowid())
    }

    pub async fn list_messages(
        pool: &SqlitePool,
        recipient: &str,
        message_box: &str,
    ) -> Result<Vec<RelayMessage>, sqlx::Error> {
        sqlx::query_as::<_, RelayMessage>(
            "SELECT id, sender, body, created_at
             FROM relay_messages
             WHERE recipient = ? AND message_box = ?
             ORDER BY created_at ASC"
        )
        .bind(recipient)
        .bind(message_box)
        .fetch_all(pool)
        .await
    }

    pub async fn acknowledge_messages(
        pool: &SqlitePool,
        recipient: &str,
        message_ids: &[i64],
    ) -> Result<u64, sqlx::Error> {
        // Use a transaction for atomicity
        let mut tx = pool.begin().await?;

        for id in message_ids {
            sqlx::query(
                "DELETE FROM relay_messages WHERE id = ? AND recipient = ?"
            )
            .bind(id)
            .bind(recipient)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(message_ids.len() as u64)
    }
}
```

### Pattern 2: WebSocket with Tokio Channels

Use channels for message broadcasting:

```rust
use tokio::sync::broadcast;

pub struct MessageRelayState {
    // Broadcast channel for new messages
    new_message_tx: broadcast::Sender<NewMessageEvent>,
}

#[derive(Clone)]
pub struct NewMessageEvent {
    pub recipient: String,
    pub message_box: String,
    pub message_id: i64,
}

// When a message is sent, broadcast to all listeners
pub async fn send_message(state: &MessageRelayState, msg: Message) {
    // Store in DB...

    // Notify listeners
    let _ = state.new_message_tx.send(NewMessageEvent {
        recipient: msg.recipient,
        message_box: msg.message_box,
        message_id: msg.id,
    });
}

// WebSocket handler subscribes to broadcast
async fn ws_handler(ws: WebSocket, state: MessageRelayState, identity: String) {
    let mut rx = state.new_message_tx.subscribe();

    while let Ok(event) = rx.recv().await {
        if event.recipient == identity {
            // Send notification to this WebSocket
            ws.send(format!("new_message:{}", event.message_id)).await;
        }
    }
}
```

### Pattern 3: Graceful Degradation

Handle external relay server failures:

```rust
pub async fn send_with_fallback(
    msg: &Message,
    local_store: &MessageStore,
    remote_servers: &[RemoteRelay],
) -> Result<()> {
    // Always store locally
    local_store.send(msg)?;

    // Best-effort send to remote servers
    let mut any_success = false;
    for server in remote_servers {
        match server.send(msg).await {
            Ok(_) => {
                any_success = true;
                log::info!("Sent to {}", server.url);
            }
            Err(e) => {
                log::warn!("Failed to send to {}: {}", server.url, e);
            }
        }
    }

    if remote_servers.is_empty() || any_success {
        Ok(())
    } else {
        Err(Error::AllRemotesFailed)
    }
}
```

---

## Appendix: BRC Reference Links

| BRC | Title | Relevance |
|-----|-------|-----------|
| [BRC-31](https://hub.bsvblockchain.org/brc/peer-to-peer/0031) | Authrite Mutual Authentication | Authentication for relay requests |
| [BRC-33](https://github.com/bitcoin-sv/BRCs/blob/master/peer-to-peer/0033.md) | PeerServ Message Relay | This specification |
| [BRC-34](https://github.com/bitcoin-sv/BRCs/blob/master/peer-to-peer/0034.md) | Message Relay Discovery | Finding relay servers |
| [BRC-35](https://github.com/bitcoin-sv/BRCs/blob/master/peer-to-peer/0035.md) | Message Relay Federation | Multi-server coordination |
| [BRC-41](https://github.com/bitcoin-sv/BRCs/blob/master/payments/0041.md) | Service Monetization | Paid relay services |
| [BRC-50](https://hub.bsvblockchain.org/brc/wallet/0050) | Payment Submission | What to put in payment messages |

---

## Summary

**Why BRC-33 exists:** The UTXO model doesn't notify recipients. Without a communication channel, recipients can't know they've been paid.

**What it solves:**
- Payment notifications
- Token transfer coordination
- Certificate delivery
- Multi-party application state

**How it works:**
- Store-and-forward message boxes
- BRC-31 authenticated endpoints
- Asynchronous, works offline
- Federable for redundancy

**What HodosBrowser needs:**
1. SQLite persistence (essential)
2. Message expiry (important)
3. WebSocket support (nice-to-have)
4. End-to-end encryption (recommended)
5. Federation support (future)
