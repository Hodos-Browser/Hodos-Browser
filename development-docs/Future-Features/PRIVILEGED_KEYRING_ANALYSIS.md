# Privileged Keyring Analysis

**Date:** 2026-02-26
**Status:** Research / Decision Needed
**Priority:** Medium (not blocking MVP)

---

## Executive Summary

BRC-100 specifies a **dual-keyring architecture**: a primary (everyday) keyring and a privileged keyring for sensitive operations. Hodos currently implements only the primary keyring. This document analyzes whether and how to implement the privileged keyring.

---

## 1. Is It Part of BRC Standards?

**Yes, but loosely specified.**

From [BRC-100 §1 (Key Derivation using BKDS)](https://bsv.brc.dev/wallet/0100):

> "Each wallet has an everyday master private key and a corresponding master public key derived from the secp256k1 elliptic curve. The public key is known as the 'identity key'. Additionally, there's a whole secondary 'privileged mode' keyring for sensitive operations, allowing these privileged keys to be treated with higher security than the user's everyday keyring."

**What the spec says:**
- Two keyrings exist: everyday and privileged
- Privileged keys get "higher security" treatment
- Used for "sensitive operations"

**What the spec does NOT say:**
- How to derive the privileged key
- Which operations require privileged mode
- How to transition between modes
- Whether privileged is mandatory or optional

**Conclusion:** It's mentioned in BRC-100 but implementation details are left to wallet vendors. Most BSV wallets (including Metanet Desktop) haven't fully implemented this either.

---

## 2. How Important Is It?

### For MVP: **Not Critical**

| Aspect | Assessment |
|--------|------------|
| User Safety | Low impact — PIN protection already guards the single keyring |
| Compliance | BRC-100 doesn't enforce it; most wallets skip it |
| User Experience | Adds complexity (two PINs? two backups?) |
| Security Benefit | Real but marginal for typical users |

### When It Becomes Important

1. **Enterprise/institutional use** — separate keys for different authorization levels
2. **High-value wallets** — cold storage for privileged, hot wallet for everyday
3. **Hardware wallet integration** — privileged key on hardware, everyday on device
4. **Multi-sig setups** — privileged operations require additional signers

### Recommendation

**Skip for MVP. Implement when:**
- User feedback requests it
- Enterprise features are prioritized
- Hardware wallet integration is added

---

## 3. How Difficult Is It?

### Difficulty: **Medium** (2-3 days of focused work)

### Required Changes

#### A. Database Schema (Low effort)

```sql
-- Add to wallets table or create new table
ALTER TABLE wallets ADD COLUMN privileged_mnemonic TEXT;
ALTER TABLE wallets ADD COLUMN privileged_pin_salt TEXT;
ALTER TABLE wallets ADD COLUMN privileged_mnemonic_dpapi BLOB;

-- Or separate table for cleaner separation
CREATE TABLE privileged_keyrings (
    id INTEGER PRIMARY KEY,
    wallet_id INTEGER NOT NULL REFERENCES wallets(id),
    mnemonic TEXT NOT NULL,           -- Encrypted
    pin_salt TEXT,                    -- Separate PIN from primary
    mnemonic_dpapi BLOB,              -- DPAPI auto-unlock (optional)
    created_at INTEGER NOT NULL,
    UNIQUE(wallet_id)
);
```

#### B. Key Derivation (Medium effort)

```rust
// New struct to manage both keyrings
pub struct DualKeyring {
    primary: KeyringState,
    privileged: Option<KeyringState>,
}

pub enum KeyringState {
    Locked,
    Unlocked { 
        mnemonic: String,
        master_key: ExtendedPrivateKey,
    },
}

impl DualKeyring {
    pub fn get_key(&self, mode: KeyringMode) -> Result<&ExtendedPrivateKey> {
        match mode {
            KeyringMode::Primary => self.primary.get_unlocked_key(),
            KeyringMode::Privileged => {
                self.privileged
                    .as_ref()
                    .ok_or(Error::PrivilegedNotConfigured)?
                    .get_unlocked_key()
            }
        }
    }
}
```

#### C. API Changes (Medium effort)

```rust
// Existing endpoints need mode parameter
#[derive(Deserialize)]
pub struct GetPublicKeyRequest {
    #[serde(rename = "identityKey")]
    pub identity_key: Option<bool>,
    pub protocol_id: Option<String>,
    pub key_id: Option<String>,
    pub counterparty: Option<String>,
    #[serde(rename = "privileged")]
    pub privileged: Option<bool>,  // NEW: Use privileged keyring
}

// New endpoints
POST /wallet/privileged/setup     // Create privileged keyring
POST /wallet/privileged/unlock    // Unlock with separate PIN
POST /wallet/privileged/lock      // Lock privileged keyring
GET  /wallet/privileged/status    // Check if configured/unlocked
```

#### D. Frontend Changes (Medium effort)

- Settings: "Set up Privileged Keyring" option
- Unlock flow: May need to unlock privileged separately
- Visual indicator: Show which mode is active
- Recovery: Backup/restore privileged keyring separately

#### E. Operation Classification (Design effort)

Decide which operations require privileged mode:

| Operation | Mode | Rationale |
|-----------|------|-----------|
| `getPublicKey` (identity) | Primary | Basic identity |
| `createAction` (small tx) | Primary | Everyday spending |
| `createAction` (large tx) | Privileged | Above threshold |
| `encrypt` / `decrypt` | Primary | Normal data ops |
| `createSignature` | Depends | Based on protocol |
| `acquireCertificate` | Privileged | Identity commitment |
| `revealKeyLinkage` | Privileged | Privacy-sensitive |
| Backup export | Privileged | Security-critical |

---

## 4. Implementation Options

### Option A: Same Mnemonic, Different Path (Easiest)

```
Primary:    m/44'/236'/0'/0/...   (current)
Privileged: m/44'/236'/0'/1/...   (new path)
```

**Pros:**
- Single mnemonic backup
- Deterministic — always recoverable
- Minimal UX change

**Cons:**
- Both compromised if mnemonic leaks
- Not true isolation

### Option B: Separate Mnemonic (More Secure)

```
Primary:    Generated mnemonic A
Privileged: Generated mnemonic B (or imported)
```

**Pros:**
- True isolation
- Can store privileged in cold storage
- Compromised primary doesn't expose privileged

**Cons:**
- Two backups required
- More complex UX
- Users may lose one

### Option C: Hardware Wallet for Privileged (Most Secure)

```
Primary:    Software keyring (mnemonic)
Privileged: Hardware wallet (Ledger, Trezor, etc.)
```

**Pros:**
- Best security
- Key never on device
- Industry standard for high-value

**Cons:**
- Requires hardware
- Integration complexity
- Not always available

### Recommendation

**For initial implementation:** Option A (same mnemonic, different path)
- Lowest complexity
- Users already have backup
- Can upgrade to Option B/C later

**For future:** Option C (hardware wallet)
- Natural progression
- Aligns with enterprise needs

---

## 5. Implementation Plan (When Ready)

### Phase 1: Foundation (1 day)
- [ ] Database schema changes
- [ ] `DualKeyring` struct and state management
- [ ] Derive privileged key from different HD path

### Phase 2: API Layer (1 day)
- [ ] Add `privileged` parameter to relevant endpoints
- [ ] New privileged setup/unlock/lock endpoints
- [ ] Operation classification logic

### Phase 3: Frontend (1 day)
- [ ] Settings UI for privileged keyring setup
- [ ] Separate PIN entry for privileged unlock
- [ ] Mode indicator in wallet UI
- [ ] Recovery flow updates

### Phase 4: Testing
- [ ] Unit tests for dual keyring
- [ ] Integration tests for privileged operations
- [ ] UX testing for flow clarity

---

## 6. Decision

### Recommendation: **Defer to Post-MVP**

**Rationale:**
1. Not blocking any current functionality
2. Most BSV wallets don't implement it
3. Single PIN-protected keyring is secure enough for typical use
4. Adds UX complexity without clear user demand
5. Can be added incrementally later

### When to Revisit

- [ ] User feedback requests higher security tiers
- [ ] Enterprise customer requirements
- [ ] Hardware wallet integration work begins
- [ ] Large-value transaction features added

---

## 7. References

- [BRC-100: Unified Wallet Interface](https://bsv.brc.dev/wallet/0100)
- [BRC-42: BSV Key Derivation Scheme](https://bsv.brc.dev/key-derivation/0042)
- [BRC-43: Security Levels, Protocol IDs](https://bsv.brc.dev/key-derivation/0043)

---

**End of Document**
