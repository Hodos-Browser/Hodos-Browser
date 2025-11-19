# BRC-42/BRC-43 Privacy and Per-App Identity Explained

> **Status**: Technical Analysis
> **Last Updated**: 2025-11-19

## Your Question

**"Can I use the same master public key to create accounts with banking app A and social media app B, but prevent them from associating the accounts?"**

**Short Answer: YES!** This is exactly what BRC-42/BRC-43 enables. ✅

## How BRC-42 Key Derivation Works

### What is BRC-42?

**BRC-42 (BSV Key Derivation Scheme)** is like a **cryptographic Diffie-Hellman key exchange** that creates unique child keys between two parties:

1. **Your Master Private Key** (`m`): Your wallet's root private key
2. **App's Master Public Key**: Each app (ToolBSV, banking app, social media) has their own master public key
3. **ECDH Shared Secret**: `ECDH(your_privkey, app_pubkey)` = shared secret only you and that app can compute
4. **Invoice Number**: A unique identifier for the derivation context (protocol + key ID)

### The Derivation Formula

```rust
// For each app, you derive a unique child key:
child_private_key = derive_child_private_key(
    your_master_private_key,    // Your secret (same for all apps)
    app_master_public_key,       // App's public key (different per app)
    invoice_number               // Context identifier
)
```

**Key Insight**: Even with the same invoice number, different apps produce **completely different child keys** because each app has a different master public key.

## What is the Invoice Number?

The **invoice number** (BRC-43 format) is: `<securityLevel>-<protocolID>-<keyID>`

### In Well-Known Auth (Current Implementation)

From `handlers.rs` line 181-196:

```rust
// Protocol ID (same for all apps using BRC-100 auth)
let protocol_id = "auth message signature";

// Key ID (includes session nonces - makes it session-specific!)
let key_id = format!("{} {}", req.initial_nonce, our_nonce);

// Final invoice number
invoice_number = "2-auth message signature-{theirNonce} {ourNonce}"
```

### Important: Current Implementation is Session-Specific

**⚠️ Current Behavior**: The invoice number includes random nonces, making each authentication session produce a **different child key**. This creates **session keys**, not persistent per-app identities.

**For Persistent Per-App Identity**: You would use a **deterministic key_id** based on the app, like:
```rust
// Hypothetical persistent identity invoice number:
invoice_number = "2-auth message signature-toolbsv"  // Same every time
// OR
invoice_number = "2-auth message signature-banking-app-A"
```

## Privacy Guarantees

### Why Apps Can't Associate Accounts

Even if two apps use the **same invoice number format**, they **cannot** associate your accounts because:

1. **Different Counterparty Keys**: Each app has a different master public key
2. **ECDH Property**: `ECDH(your_privkey, app_A_pubkey) ≠ ECDH(your_privkey, app_B_pubkey)`
3. **Mathematically Independent**: The child keys are cryptographically independent - no way to link them without knowing your master private key

### Example

```rust
// Banking App A
let app_A_pubkey = hex::decode("03AAA...");  // Banking app's master key
let child_key_A = derive_child_private_key(your_privkey, app_A_pubkey, invoice);
// Result: 0x7f3a...

// Social Media App B
let app_B_pubkey = hex::decode("03BBB...");  // Social media app's master key
let child_key_B = derive_child_private_key(your_privkey, app_B_pubkey, invoice);
// Result: 0x9b2c...  // COMPLETELY DIFFERENT!

// Even with SAME invoice number, child keys are unrelated!
```

## Current Implementation Analysis

### How Well-Known Auth Works

1. **Client sends**: Their master public key (identity key) + initial nonce
2. **Server (us) responds**: Our master public key + our nonce + signature
3. **Invoice number**: `"2-auth message signature-{clientNonce} {serverNonce}"`
4. **Child key derived**: Using BRC-42 with both parties' master keys

### The Invoice Number Components

```rust
// From handlers.rs line 181-196
let invoice_number = InvoiceNumber::new(
    SecurityLevel::CounterpartyLevel,  // Level 2: per-counterparty
    "auth message signature",           // Protocol ID (normalized)
    format!("{} {}", req.initial_nonce, our_nonce)  // Key ID (nonces)
);
```

**Security Level**: `CounterpartyLevel` (2) means the key is specific to the counterparty (the app)

**Protocol ID**: `"auth message signature"` is normalized and used for all BRC-100 authentication

**Key ID**: Currently includes both nonces, making it **session-specific**

### For ToolBSV: Is Identity Persistent?

**Question**: "Each time I log into ToolBSV, does it know I am the same person?"

**Answer**: It depends on how ToolBSV stores your identity:

1. **If ToolBSV stores the child public key from first auth**: Then it can recognize you, but only if you use the same nonces (which you don't - they're random)

2. **If ToolBSV stores your master public key**: Then it recognizes you by your master key, not by a derived child key

3. **For persistent identity**: ToolBSV would need to use a **deterministic invoice number** (without nonces), like:
   ```rust
   // Persistent identity invoice number:
   invoice_number = "2-auth message signature-toolbsv"
   // Same every time you authenticate with ToolBSV
   ```

## Recommendation: Persistent Per-App Identity

To achieve **persistent per-app identity** (same identity each time you log into an app), you should:

### Option 1: Deterministic Key ID Per App

```rust
// For ToolBSV
invoice_number = "2-auth message signature-toolbsv"

// For Banking App A
invoice_number = "2-auth message signature-banking-app-a"

// For Social Media App B
invoice_number = "2-auth message signature-social-app-b"
```

**Pros**:
- ✅ Same identity every time you authenticate with same app
- ✅ Different identity per app (cannot be associated)
- ✅ Apps can recognize returning users

**Cons**:
- ⚠️ Need to coordinate with apps on key ID format
- ⚠️ Apps might need to store mapping of key IDs to identities

### Option 2: App-Specific Protocol ID

```rust
// For ToolBSV
invoice_number = "2-toolbsv auth signature-{deterministic_id}"

// For Banking App A
invoice_number = "2-banking app a auth signature-{deterministic_id}"
```

**Pros**:
- ✅ Even stronger separation (different protocol IDs)
- ✅ Apps can't accidentally use wrong protocol ID

**Cons**:
- ⚠️ Requires app-specific protocol ID coordination

### Option 3: Use App's Master Public Key Hash as Key ID

```rust
// Derive deterministic key ID from app's public key
let app_key_hash = sha256(app_master_pubkey)[..16];  // First 16 bytes
let key_id = hex::encode(app_key_hash);

invoice_number = format!("2-auth message signature-{}", key_id);
```

**Pros**:
- ✅ Automatically unique per app (no coordination needed)
- ✅ Deterministic (same app = same key ID)
- ✅ No manual key ID management

**Cons**:
- ⚠️ Key ID is longer (32 hex chars)

## Summary

### What BRC-42/BRC-43 Provides

✅ **Privacy**: Apps cannot associate accounts - each app gets unique child keys
✅ **Deterministic**: Same inputs = same child key (for persistent identity)
✅ **ECDH Security**: Only you and the app can compute the child key
✅ **Flexible**: Invoice number controls the derivation context

### Current Implementation

**What it does**: Creates **session-specific keys** (different each login)
**Why**: Invoice number includes random nonces
**Impact**: Each authentication creates a new child key (not persistent)

### For Persistent Identity

**What you need**: Deterministic invoice number (no random nonces)
**How**: Use app identifier or app's public key hash as key ID
**Result**: Same app = same child key every time = persistent identity

### Privacy Answer

**YES, apps cannot associate accounts!** Even with the same invoice number format:
- Different app public keys → Different ECDH shared secrets → Different child keys
- Mathematically impossible to link without your master private key
- This is the core privacy property of BRC-42

---

## Questions to Consider

1. **Do you want session-specific keys or persistent per-app identities?**
   - Session-specific: Current implementation (random nonces)
   - Persistent: Use deterministic key IDs

2. **How should apps identify returning users?**
   - Store child public key from first auth?
   - Store master public key?
   - Store user-chosen identifier?

3. **Should invoice numbers be standardized?**
   - BRC-100 standard format?
   - App-specific formats?
   - Auto-generated from app's public key?
