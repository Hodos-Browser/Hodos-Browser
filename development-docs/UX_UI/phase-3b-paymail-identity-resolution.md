# Phase 3b: Paymail Support + Identity Name Resolution

**Status**: IMPLEMENTED — Needs Testing & Refinement
**Completed**: 2026-03-07
**Dependencies**: Phase 3a (BRC-29 PeerPay) — complete

---

## Overview

Add two recipient resolution features to the wallet send form:

1. **Paymail Resolution** — Send BSV to human-readable addresses like `alice@handcash.io`
2. **Identity Name Resolution** — Resolve identity keys (`03abc...`) to human-readable names via BSV Overlay Services

After this phase, the recipient field supports four input types:

| Input | Format | Resolution |
|-------|--------|------------|
| BSV Address | `1...` or `3...` | Direct send (existing) |
| Identity Key | `02.../03...` (66 hex chars) | PeerPay via BRC-29 MessageBox (existing) |
| Paymail | `user@domain.com` | Paymail protocol resolution (NEW) |
| HandCash Handle | `$handle` | Converted to `handle@handcash.io` then paymail (NEW) |

---

## Sprint 1: Paymail Resolution (Rust Backend)

**Goal**: Rust endpoint that resolves a paymail address to a P2PKH output script.

### Paymail Protocol Flow

The paymail protocol (bsvalias) is a 3-step HTTP flow:

#### Step 1: Host Discovery (DNS SRV)

Given paymail `alice@handcash.io`:
- Query DNS SRV record: `_bsvalias._tcp.handcash.io`
- Result: `10 10 443 cloud.handcash.io` (priority, weight, port, target)
- **Fallback**: If no SRV record, use `handcash.io:443`

#### Step 2: Capability Discovery

```
GET https://cloud.handcash.io/.well-known/bsvalias
Accept: application/json
```

Response:
```json
{
  "bsvalias": "1.0",
  "capabilities": {
    "pki": "https://cloud.handcash.io/api/bsvalias/id/{alias}@{domain.tld}",
    "paymentDestination": "https://cloud.handcash.io/api/bsvalias/address/{alias}@{domain.tld}",
    "2a40af698840": "https://cloud.handcash.io/api/bsvalias/p2p-payment-destination/{alias}@{domain.tld}",
    "5f1323cddf31": "https://cloud.handcash.io/api/bsvalias/receive-transaction/{alias}@{domain.tld}",
    "5c55a7fdb7bb": "https://cloud.handcash.io/api/bsvalias/receive-beef/{alias}@{domain.tld}",
    "f12f968c92d6": "https://cloud.handcash.io/api/bsvalias/public-profile/{alias}@{domain.tld}",
    "a9f510c16bde": "https://cloud.handcash.io/api/bsvalias/verifypubkey/{alias}@{domain.tld}/{pubkey}",
    "6745385c3fc0": false
  }
}
```

**Capability BRFC IDs**:

| BRFC ID | Name | Method | Purpose |
|---------|------|--------|---------|
| `pki` | Public Key Infrastructure | GET | Get identity public key for paymail |
| `paymentDestination` | Basic Address Resolution | POST | Get P2PKH output script (simple path) |
| `2a40af698840` | P2P Payment Destination | POST | Get outputs + reference (P2P path) |
| `5f1323cddf31` | P2P Receive Transaction | POST | Submit signed raw tx back to receiver |
| `5c55a7fdb7bb` | P2P Receive BEEF | POST | Submit BEEF-encoded tx to receiver |
| `f12f968c92d6` | Public Profile | GET | Get name + avatar |
| `a9f510c16bde` | Verify Public Key | GET | Verify pubkey belongs to paymail |
| `6745385c3fc0` | Sender Validation | Boolean | Whether sender signature is required |

#### Step 3a: Simple Path — Basic Address Resolution

```
POST https://cloud.handcash.io/api/bsvalias/address/alice@handcash.io
Content-Type: application/json

{
  "senderHandle": "user@hodosbrowser.com",
  "dt": "2026-03-06T12:00:00.000Z",
  "amount": 50000,
  "purpose": "Payment"
}
```

Response:
```json
{
  "output": "76a914f32281faa74e2ac037493f7a3cd317e8f5e9673688ac"
}
```

The `output` is a hex-encoded locking script (P2PKH). Extract the pubkey hash, build a standard P2PKH transaction, broadcast via ARC/GorillaPool.

#### Step 3b: P2P Path — Preferred (Instant Notification to Receiver)

**Request outputs:**
```
POST https://cloud.handcash.io/api/bsvalias/p2p-payment-destination/alice@handcash.io
Content-Type: application/json

{ "satoshis": 50000 }
```

Response:
```json
{
  "outputs": [
    { "script": "76a914...88ac", "satoshis": 50000 }
  ],
  "reference": "payment-ref-abc123"
}
```

**Build & sign transaction** using the output scripts.

**Submit signed transaction back:**
```
POST https://cloud.handcash.io/api/bsvalias/receive-transaction/alice@handcash.io
Content-Type: application/json

{
  "hex": "0100000001...signed_raw_tx_hex...",
  "metadata": {
    "sender": "user@hodosbrowser.com",
    "pubkey": "02abc123...",
    "note": "Payment"
  },
  "reference": "payment-ref-abc123"
}
```

Response:
```json
{ "txid": "abc123...", "note": "Payment received" }
```

### Implementation

**New file**: `rust-wallet/src/paymail.rs`

```rust
pub struct PaymailClient {
    http_client: reqwest::Client,
}

/// Cached capability discovery result
struct PaymailCapabilities {
    pki_url: Option<String>,
    payment_destination_url: Option<String>,
    p2p_destination_url: Option<String>,
    p2p_receive_tx_url: Option<String>,
    p2p_receive_beef_url: Option<String>,
    public_profile_url: Option<String>,
}

impl PaymailClient {
    pub fn new() -> Self;

    /// Resolve DNS SRV record for paymail domain, fallback to domain:443
    async fn discover_host(&self, domain: &str) -> Result<String>;

    /// Fetch and parse .well-known/bsvalias capabilities
    async fn discover_capabilities(&self, host: &str) -> Result<PaymailCapabilities>;

    /// Simple path: resolve paymail to P2PKH output script
    pub async fn resolve_address(&self, paymail: &str) -> Result<PaymailAddress>;

    /// P2P path: get payment destination outputs + reference
    pub async fn get_p2p_destination(&self, paymail: &str, satoshis: u64) -> Result<P2PDestination>;

    /// P2P path: submit signed transaction to receiver
    pub async fn submit_transaction(&self, paymail: &str, tx_hex: &str, reference: &str) -> Result<String>;

    /// Get public profile (name + avatar)
    pub async fn get_profile(&self, paymail: &str) -> Result<Option<PaymailProfile>>;
}

pub struct PaymailAddress {
    pub output_script: Vec<u8>,   // P2PKH locking script
    pub paymail: String,
}

pub struct P2PDestination {
    pub outputs: Vec<PaymailOutput>,
    pub reference: String,
}

pub struct PaymailOutput {
    pub script: Vec<u8>,
    pub satoshis: u64,
}

pub struct PaymailProfile {
    pub name: String,
    pub avatar_url: Option<String>,
}
```

**New endpoint**: `POST /wallet/paymail/send`

```json
Request:  { "paymail": "alice@handcash.io", "amount_satoshis": 50000 }
Response: { "success": true, "txid": "abc123..." }
```

Logic:
1. Parse paymail (`alias@domain`), handle `$handle` → `handle@handcash.io` conversion
2. Discover host → discover capabilities
3. If P2P destination supported (`2a40af698840`):
   - Get P2P destination outputs + reference
   - Build transaction using those outputs
   - Sign transaction
   - Submit signed tx back via receive-transaction endpoint
4. Else fallback to simple path:
   - Get output script via `paymentDestination`
   - Build transaction with that output
   - Sign and broadcast via ARC/GorillaPool (standard path)
5. Record transaction in DB, invalidate balance cache

**New endpoint**: `GET /wallet/paymail/resolve?address=alice@handcash.io`

```json
Response: { "valid": true, "name": "Alice", "avatar_url": "https://..." }
```

For frontend name display in the recipient field. Calls the `public-profile` capability (`f12f968c92d6`) if available.

### DNS SRV Resolution

Use the `trust-dns-resolver` crate (or `hickory-resolver`, its rename):
```toml
[dependencies]
hickory-resolver = "0.24"
```

Fallback logic: if SRV lookup fails or returns no records, use `domain:443`.

### Caching

Cache capability discovery results per domain (in-memory HashMap with 1-hour TTL). The `.well-known/bsvalias` response rarely changes.

### Build & Verify
- `cargo check` in `rust-wallet/`
- Test: resolve `test@handcash.io` or known paymail → verify output script returned

---

## Sprint 2: Paymail Frontend Integration

**Goal**: Detect paymail in recipient field, resolve name, route to paymail send.

### Recipient Detection

**File**: `frontend/src/components/TransactionForm.tsx`

Add paymail regex alongside existing patterns:
```typescript
const IDENTITY_KEY_REGEX = /^(02|03)[0-9a-fA-F]{64}$/;
const BSV_ADDRESS_REGEX = /^[13][a-km-zA-HJ-NP-Z1-9]{25,34}$/;
const PAYMAIL_REGEX = /^[a-zA-Z0-9._-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$/;
const HANDCASH_REGEX = /^\$([a-zA-Z0-9_-]+)$/;  // $handle → handle@handcash.io
```

Detection priority (in `useMemo`):
1. Identity key → PeerPay
2. BSV address → standard send
3. `$handle` → convert to `handle@handcash.io` → paymail
4. `user@domain` → paymail
5. None matched → show validation error

### Name Resolution UI

When paymail is detected in recipient field:
- Debounce 400ms after typing stops
- Call `GET /wallet/paymail/resolve?address=alice@handcash.io`
- Display resolved name below input: "Sending to Alice (alice@handcash.io)"
- Show loading spinner during resolution
- Show error if paymail doesn't resolve

### Send Routing

On form submit, if recipient is paymail:
```typescript
const resp = await fetch('http://127.0.0.1:31301/wallet/paymail/send', {
  method: 'POST',
  headers: { 'Content-Type': 'application/json' },
  body: JSON.stringify({
    paymail: formData.recipient.trim(),
    amount_satoshis: satoshiAmount,
  }),
});
```

### Field Hint Updates

```typescript
<span className="field-hint">
  {isPeerPay ? 'Sending via PeerPay (identity key detected)' :
   isPaymail ? `Sending to ${resolvedName || paymail}` :
   'Enter BSV address, identity key, or paymail'}
</span>
```

### Build & Verify
- `npm run build` in `frontend/`

---

## Sprint 3: Identity Name Resolution (Rust Backend)

**Goal**: Resolve identity keys to human-readable names via BSV Overlay Services.

### How Identity Resolution Works

Users publish BRC-52 identity certificates on-chain as PushDrop tokens to the `tm_identity` overlay topic. Overlay nodes index these and serve queries via the `ls_identity` lookup service.

### The Lookup

```
POST https://overlay-us-1.bsvb.tech/lookup
Content-Type: application/json

{
  "service": "ls_identity",
  "query": {
    "identityKey": "03abc...",
    "certifiers": [
      "03daf815fe38f83da0ad83b5bedc520aa488aef5cbb93a93c67a7fe60406cbffe8",
      "02cf6cdf466951d8dfc9e7c9367511d0007ed6fba35ed42d425cc412fd6cfd4a17"
    ]
  }
}
```

**Default trusted certifiers**:

| Certifier | Identity Key | Trust |
|-----------|-------------|-------|
| Metanet Trust Services | `03daf815fe38f83da0ad83b5bedc520aa488aef5cbb93a93c67a7fe60406cbffe8` | 4 |
| SocialCert | `02cf6cdf466951d8dfc9e7c9367511d0007ed6fba35ed42d425cc412fd6cfd4a17` | 3 |

**Overlay hosts** (discovered via SLAP, but these are the known defaults):
- `https://overlay-us-1.bsvb.tech`
- `https://overlay-eu-1.bsvb.tech`
- `https://overlay-ap-1.bsvb.tech`
- `https://users.bapp.dev`

### Response Format

The response is an `output-list` — Bitcoin transaction outputs in BEEF format containing PushDrop-encoded identity certificates:

```json
{
  "type": "output-list",
  "outputs": [
    {
      "beef": "<base64 BEEF>",
      "outputIndex": 0
    }
  ]
}
```

Each output contains a PushDrop token with a JSON certificate in field[0]:

```json
{
  "type": "<certificate_type_base64>",
  "subject": "03abc...",
  "certifier": "03daf...",
  "serialNumber": "...",
  "revocationOutpoint": "...",
  "fields": {
    "userName": "<encrypted>",
    "profilePhoto": "<encrypted>"
  },
  "signature": "..."
}
```

### Certificate Types and Field Mapping

| Certificate Type | Type ID (base64) | Name Field | Avatar Field |
|-----------------|------------------|------------|--------------|
| xCert (X/Twitter) | `vdDWvftf1H+5+ZprUw123kjHlywH+v20aPQTuXgMpNc=` | `userName` | `profilePhoto` |
| discordCert | `2TgqRC35B1zehGmB21xveZNc7i5iqHc0uxMb+1NMPW4=` | `userName` | `profilePhoto` |
| emailCert | `exOl3KM0dIJ04EW5pZgbZmPag6MdJXd3/a1enmUU/BA=` | `email` | — |
| phoneCert | `mffUklUzxbHr65xLohn0hRL0Tq2GjW1GYF/OPfzqJ6A=` | `phoneNumber` | — |
| identiCert (Gov ID) | `z40BOInXkI8m7f/wBrv4MJ09bZfzZbTj2fJqCtONqCY=` | `firstName` + `lastName` | `profilePhoto` |
| registrant | `YoPsbfR6YQczjzPdHCoGC7nJsOdPQR50+SYqcWpJ0y0=` | `name` | `icon` |
| Unknown | any | Fallback: `firstName`, `lastName`, `name`, `userName`, `email` | `profilePhoto`, `avatar` |

### Decrypting Publicly Revealed Fields

Publicly revealed certificate fields are encrypted with the **'anyone' key** — a well-known private key (`1` / `0x0000...0001`) that anyone can use to decrypt. This is how "selective disclosure" works: the user chooses which fields to reveal publicly by re-encrypting them with the anyone key.

Decryption uses BRC-2 (ECDH + AES-256-GCM):
- Private key: `0x0000000000000000000000000000000000000000000000000000000000000001`
- Public key: `0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798` (generator point)
- Invoice number: from the certificate's protocol and key IDs

**Reuse**: Our existing `decrypt_brc2` and `derive_symmetric_key` from `crypto/brc2.rs`.

### Implementation

**New file**: `rust-wallet/src/identity_resolver.rs`

```rust
const OVERLAY_HOSTS: &[&str] = &[
    "https://overlay-us-1.bsvb.tech",
    "https://overlay-eu-1.bsvb.tech",
];

const TRUSTED_CERTIFIERS: &[&str] = &[
    "03daf815fe38f83da0ad83b5bedc520aa488aef5cbb93a93c67a7fe60406cbffe8",  // Metanet Trust Services
    "02cf6cdf466951d8dfc9e7c9367511d0007ed6fba35ed42d425cc412fd6cfd4a17",  // SocialCert
];

pub struct IdentityResolver {
    http_client: reqwest::Client,
}

impl IdentityResolver {
    /// Query overlay for identity certificates matching an identity key
    pub async fn resolve(&self, identity_key: &str) -> Result<Option<ResolvedIdentity>>;

    /// Parse BEEF outputs into certificates, decrypt public fields, extract name
    fn parse_overlay_response(&self, response: serde_json::Value) -> Result<Vec<IdentityCertificate>>;

    /// Decrypt a publicly revealed field using the 'anyone' key
    fn decrypt_public_field(&self, encrypted: &str, certificate: &IdentityCertificate) -> Result<String>;

    /// Map certificate type to name/avatar fields
    fn extract_display_info(&self, cert: &IdentityCertificate) -> Option<ResolvedIdentity>;
}

pub struct ResolvedIdentity {
    pub name: String,
    pub avatar_url: Option<String>,
    pub certifier_name: String,       // e.g., "SocialCert"
    pub certificate_type: String,     // e.g., "X/Twitter"
    pub identity_key: String,
}
```

**New endpoint**: `GET /wallet/identity/resolve?key=03abc...`

```json
Response: {
  "found": true,
  "name": "alice",
  "avatar_url": "https://...",
  "certifier": "SocialCert",
  "certificate_type": "X/Twitter"
}
```

### Crypto Reuse (DO NOT rewrite)

| Function | File | Purpose |
|----------|------|---------|
| `decrypt_brc2` | `crypto/brc2.rs` | AES-256-GCM decryption |
| `derive_symmetric_key` | `crypto/brc2.rs` | ECDH key derivation for 'anyone' key |

### PushDrop Decoding

PushDrop tokens encode data in Bitcoin script OP_RETURN outputs:
```
OP_0 OP_RETURN <field_0> <field_1> ... <field_n> <signature> OP_DROP ... OP_DROP <locking_script>
```

Field[0] is the JSON certificate. Parse the script, extract push data fields. We may need a small PushDrop decoder (or reuse the script parsing from our BEEF code).

### Caching

Cache resolved identities in memory (HashMap with 10-minute TTL). Identity certificates change infrequently.

### Build & Verify
- `cargo check` in `rust-wallet/`
- Test: resolve a known identity key → verify name returned

---

## Sprint 4: Identity Resolution Frontend Integration

**Goal**: Show resolved names for identity keys in the recipient field.

### Auto-Resolution

When an identity key is detected in the recipient field:
1. Debounce 400ms after the full 66-char key is entered
2. Call `GET /wallet/identity/resolve?key=03abc...`
3. Display resolved name below input:
   - "Sending via PeerPay to alice (X/Twitter, certified by SocialCert)"
4. Show loading spinner during resolution
5. If not found, show: "Sending via PeerPay (identity key detected)"

### Implementation

**File**: `frontend/src/components/TransactionForm.tsx`

Add state for resolved identity:
```typescript
const [resolvedIdentity, setResolvedIdentity] = useState<{
  name: string;
  certifier: string;
  type: string;
} | null>(null);
const [resolving, setResolving] = useState(false);
```

Add effect to resolve when identity key is detected:
```typescript
useEffect(() => {
  if (!isPeerPay) {
    setResolvedIdentity(null);
    return;
  }
  setResolving(true);
  const timer = setTimeout(async () => {
    try {
      const resp = await fetch(`http://127.0.0.1:31301/wallet/identity/resolve?key=${formData.recipient.trim()}`);
      const data = await resp.json();
      if (data.found) {
        setResolvedIdentity({ name: data.name, certifier: data.certifier, type: data.certificate_type });
      } else {
        setResolvedIdentity(null);
      }
    } catch {
      setResolvedIdentity(null);
    }
    setResolving(false);
  }, 400);
  return () => clearTimeout(timer);
}, [isPeerPay, formData.recipient]);
```

Update field hint:
```typescript
<span className="field-hint">
  {resolving ? 'Resolving identity...' :
   resolvedIdentity ? `Sending via PeerPay to ${resolvedIdentity.name} (${resolvedIdentity.type})` :
   isPeerPay ? 'Sending via PeerPay (identity key detected)' :
   isPaymail ? `Sending to ${resolvedName || paymail}` :
   'Enter BSV address, identity key, or paymail'}
</span>
```

### Build & Verify
- `npm run build` in `frontend/`

---

## Active BSV Paymail Providers

| Provider | Domain | P2P Support | Status |
|----------|--------|-------------|--------|
| HandCash | `@handcash.io` | Full (P2P + BEEF) | Active |
| Simply Cash | `@simply.cash` | Basic only | Active |
| Money Button | `@moneybutton.com` | — | Defunct |
| RelayX | `@relayx.io` | — | Defunct/degraded |
| MyPaymail | `@mypaymail.co` | Varies | Active |

---

## Key Files Summary

| File | Action |
|------|--------|
| `rust-wallet/src/paymail.rs` | NEW — Paymail client (DNS SRV, capability discovery, address resolution, P2P send) |
| `rust-wallet/src/identity_resolver.rs` | NEW — BSV Overlay identity lookup, PushDrop decoding, certificate field decryption |
| `rust-wallet/src/handlers.rs` | EDIT — Add `paymail_send`, `paymail_resolve`, `identity_resolve` endpoints |
| `rust-wallet/src/main.rs` | EDIT — Add modules, register routes |
| `rust-wallet/Cargo.toml` | EDIT — Add `hickory-resolver` for DNS SRV |
| `frontend/src/components/TransactionForm.tsx` | EDIT — Unified recipient resolution with dropdown UI |
| `frontend/src/components/wallet/TransactionComponents.css` | NEW — Recipient dropdown styles |

## Crypto Reuse (DO NOT rewrite)

| Function | File | Purpose |
|----------|------|---------|
| `decrypt_brc2` | `crypto/brc2.rs` | Decrypt publicly revealed certificate fields (using 'anyone' key) |
| `derive_symmetric_key` | `crypto/brc2.rs` | ECDH symmetric key for BRC-2 decryption |
| existing BEEF parser | `beef.rs` | Parse BEEF-encoded overlay responses |
| existing tx builder | `handlers.rs` / `transaction/` | Build and sign transactions for paymail outputs |

---

## Implementation Summary (All 4 Sprints Complete)

### What Was Actually Built

**Sprint 1: Paymail Backend** — `rust-wallet/src/paymail.rs`
- `PaymailClient`: parse_paymail ($handle + alias@domain), SRV overrides (handcash.io → cloud.handcash.io), capability cache (1h TTL)
- P2P destination + basic fallback, submit_transaction (receive-tx notification), get_profile, resolve
- `POST /wallet/paymail/send`: P2P preferred → basic fallback → createAction → broadcast → P2P notify (non-fatal)
- `GET /wallet/paymail/resolve?address=`: returns `{valid, name, avatar_url, has_p2p}`, always HTTP 200
- 16 unit tests. No new dependencies (reuses reqwest, serde_json, chrono, thiserror, log)
- Key pattern: `randomize_outputs: false` for P2P (reference depends on output order), `true` for basic
- Sender: `"anonymous@hodosbrowser.com"` (we don't host paymail)

**Sprint 2: Paymail Frontend** — `TransactionForm.tsx`
- Added `PAYMAIL_REGEX`, `isPaymail` memo, debounced resolve via `/wallet/paymail/resolve`
- Paymail submit branch via `/wallet/paymail/send`
- Three-way detection: BSV address → standard send, identity key → PeerPay, paymail → bsvalias

**Sprints 3+4: Unified Recipient Resolution** — Consolidated into a single implementation
- `rust-wallet/src/identity_resolver.rs`: BSV Overlay Services lookup (`ls_identity`), BEEF parse → PushDrop decode → BRC-2 anyone-key decryption. 10-min cache. US/EU overlay fallback. Trusted certifiers: Metanet Trust + SocialCert. Maps 5 cert types to name/avatar fields.
- `GET /wallet/recipient/resolve?input=<value>`: Unified endpoint — auto-detects identity key, paymail, $handle, BSV address. Returns `{type, valid, name, avatar_url, source, has_p2p}`. Old `/wallet/paymail/resolve` still works (no breaking change).
- `TransactionForm.tsx`: Replaced fragmented `paymailInfo`/`isResolvingPaymail` with unified `resolveResult`/`isResolving`/`detectedType`. Single `useEffect` with debounce (500ms paymail, 300ms identity). Frontend `resolveCacheRef` for instant re-display.
- Recipient dropdown: Replaces old `field-hint` span. Absolute positioned below input. Shows: spinner → avatar+name+source+checkmark → "Identity key detected" (unresolved) → red X (failed). BSV addresses: no dropdown.
- CSS: `TransactionComponents.css` (dropdown styles), `WalletPanel.css` (dark theme overrides)

### Testing Status

- [ ] Paymail send to known HandCash handle ($handle)
- [ ] Paymail send to alias@domain format
- [ ] Identity key resolution shows name/avatar from Overlay Services
- [ ] Recipient dropdown appears with spinner during resolution
- [ ] Resolved name + avatar + source badge display correctly
- [ ] BSV address: no dropdown, standard send
- [ ] Invalid paymail: dropdown shows error state
- [ ] P2P vs basic fallback works correctly
- [ ] Standard send (BSV address) unaffected by changes
