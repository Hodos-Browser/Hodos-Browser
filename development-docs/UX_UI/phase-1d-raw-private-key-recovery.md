# Phase 1d: Raw Private Key Recovery

**Date:** 2026-03-03
**Status:** Planning (scheduled for V&B-1 pass)
**Priority:** Medium
**Prerequisite:** Phase 1 (Initial Setup/Recovery) COMPLETE
**Depends On:** None

---

## Overview

Add the ability to recover a wallet from a raw private key (hex or WIF format), in addition to the existing mnemonic and backup file recovery methods.

---

## Motivation

| Use Case | Description |
|----------|-------------|
| **Power users** | Developers testing with known keys |
| **Emergency recovery** | When only raw key is available (no mnemonic) |
| **BRC-100 wallet migration** | Importing primary key from MetaNet or other BRC-100 wallets |
| **Migration** | Importing from non-BIP39 wallets |
| **Legacy wallets** | Old wallets that used raw keys, not mnemonics |
| **Paper backups** | Some users store raw keys on paper |

---

## Current Recovery Methods

| Method | Status | Source |
|--------|--------|--------|
| Mnemonic (12/24 words) | ✅ Complete | BIP-39 standard |
| Encrypted backup file | ✅ Complete | Hodos JSON export |
| Centbee import | ✅ Complete | External wallet sweep |
| **Raw private key** | ❌ Not implemented | This phase |

---

## Primary vs Privileged Key: Which to Import

BRC-100 specifies two keyrings: **primary (everyday)** and **privileged (sensitive operations)**. MetaNet Desktop exports both as plain text. Users migrating from a BRC-100 wallet need to understand which key to import.

### Why It Matters

| | Import Primary Key | Import Privileged Key |
|---|---|---|
| **Identity preserved?** | Yes -- identity key derives from primary | No -- creates a different identity |
| **Certificates valid?** | Yes -- certs are issued to the identity key | No -- cert issuer doesn't recognize this key |
| **Domain permissions?** | Yes -- sites remember this identity | No -- sites see a stranger |
| **On-chain history?** | Yes -- linked to identity key | No -- orphaned |

The identity key is derived from the primary key. That identity key is what the BRC-100 ecosystem uses to recognize you -- certificates, domain permissions, mutual auth sessions. Importing the privileged key gives you a functional wallet with funds at that key's address, but a **different identity**.

### Key Type Selection

The API supports a `keyType` field to indicate which keyring the imported key belongs to:

| `keyType` Value | Meaning | When to Use |
|----------------|---------|-------------|
| `primary` (default) | Everyday key -- becomes identity key | Most users, BRC-100 migration, general recovery |
| `privileged` | Sensitive ops key -- does NOT become identity | Advanced users who specifically need the privileged keyring |

**Default is `primary`.** The UI should recommend primary for all standard recovery scenarios.

### UI Guidance

When the user selects "Private Key" recovery:

1. **Default state**: Key type selector shows "Primary (Identity Key)" pre-selected
2. **If user selects Privileged**: Show warning: _"Importing a privileged key will create a new wallet identity. Your existing certificates, domain permissions, and on-chain identity from another BRC-100 wallet will NOT transfer. If migrating from a BRC-100 wallet, use your primary key instead."_
3. **BRC-100 migration callout**: When key format is valid, show info text: _"Migrating from MetaNet or another BRC-100 wallet? Use your primary (everyday) key to preserve your identity."_

### Privileged Keyring -- Deferred

Full dual-keyring support (using both primary and privileged keyrings simultaneously) is deferred to post-MVP. See [PRIVILEGED_KEYRING_ANALYSIS.md](../../PRIVILEGED_KEYRING_ANALYSIS.md). For Phase 1d, we support importing **either** key type, but only as a single-keyring wallet. The `keyType` field is stored so we can build on it later.

---

## Specification

### Supported Formats

| Format | Example | Length |
|--------|---------|--------|
| **Hex** | `e8f32e723...` (64 chars) | 32 bytes = 64 hex chars |
| **WIF** | `5HueCGU8r...` or `KwdMAjGm...` | 51-52 chars (uncompressed/compressed) |

### Key Validation

```rust
fn validate_private_key(input: &str) -> Result<PrivateKeyFormat> {
    // Try hex format first
    if input.len() == 64 && input.chars().all(|c| c.is_ascii_hexdigit()) {
        let bytes = hex::decode(input)?;
        validate_secp256k1_scalar(&bytes)?;
        return Ok(PrivateKeyFormat::Hex(bytes));
    }
    
    // Try WIF format
    if input.starts_with('5') || input.starts_with('K') || input.starts_with('L') {
        let decoded = base58check_decode(input)?;
        validate_wif_checksum(&decoded)?;
        let key_bytes = extract_key_from_wif(&decoded)?;
        return Ok(PrivateKeyFormat::Wif { 
            key: key_bytes,
            compressed: input.starts_with('K') || input.starts_with('L')
        });
    }
    
    Err(Error::InvalidPrivateKeyFormat)
}
```

---

## API Design

### New Endpoint

**`POST /wallet/recover-from-key`**

```typescript
// Request
interface RecoverFromKeyRequest {
  privateKey: string;     // Hex (64 chars) or WIF format
  keyType?: 'primary' | 'privileged';  // Default: 'primary' — which BRC-100 keyring this key belongs to
  pin?: string;           // Optional PIN protection
}

// Response (success)
interface RecoverFromKeyResponse {
  success: true;
  identityKey: string;    // Derived public key (hex)
  address: string;        // First receiving address
  warning: string;        // "No mnemonic backup available"
}

// Response (error)
interface RecoverFromKeyError {
  success: false;
  error: string;
  code: 'INVALID_FORMAT' | 'INVALID_KEY' | 'WALLET_EXISTS';
}
```

### Implementation

```rust
pub async fn recover_from_key(
    state: web::Data<AppState>,
    req: web::Json<RecoverFromKeyRequest>,
) -> HttpResponse {
    log::info!("/wallet/recover-from-key called");

    // Default keyType to "primary" if not provided
    let key_type = req.key_type.as_deref().unwrap_or("primary");
    
    // 1. Validate key format
    let key_data = match validate_private_key(&req.private_key) {
        Ok(data) => data,
        Err(e) => return HttpResponse::BadRequest().json(json!({
            "success": false,
            "error": format!("Invalid private key: {}", e),
            "code": "INVALID_FORMAT"
        }))
    };
    
    // 2. Check no wallet exists
    let db = state.db.lock().unwrap();
    if db.wallet_exists()? {
        return HttpResponse::Conflict().json(json!({
            "success": false,
            "error": "Wallet already exists. Delete existing wallet first.",
            "code": "WALLET_EXISTS"
        }));
    }
    
    // 3. Create wallet entry
    // Note: We store a marker instead of mnemonic since there is none
    let wallet = Wallet {
        id: None,
        mnemonic: "[RAW_KEY_IMPORT]".to_string(),  // Marker, not actual mnemonic
        pin_salt: None,
        mnemonic_dpapi: None,
        current_index: 0,
        backed_up: false,  // User must understand no mnemonic backup
        created_at: chrono::Utc::now().timestamp(),
    };
    
    // 4. Derive identity key
    let private_key = PrivateKey::from_bytes(&key_data.to_bytes())?;
    let public_key = private_key.to_public_key();
    let identity_key = hex::encode(public_key.to_compressed());
    
    // 5. Generate first address
    let address = public_key.to_address(Network::Mainnet);
    
    // 6. Store encrypted key (not mnemonic)
    // ... PIN encryption logic if PIN provided ...
    
    // 7. Create user entry
    let user = User {
        user_id: None,
        identity_key: identity_key.clone(),
        active_storage: "local".to_string(),
    };
    
    db.save_wallet(&wallet)?;
    db.save_user(&user)?;
    
    // 8. Cache the key for operations
    state.cache_private_key(private_key);
    
    HttpResponse::Ok().json(json!({
        "success": true,
        "identityKey": identity_key,
        "address": address.to_string(),
        "warning": "This wallet was imported from a raw private key. No mnemonic backup is available. Store your private key securely!"
    }))
}
```

---

## Database Considerations

### Option A: Special Marker in Mnemonic Field

```sql
-- Wallet row for raw key import
INSERT INTO wallets (mnemonic, pin_salt, current_index, backed_up, created_at)
VALUES ('[RAW_KEY_IMPORT]', NULL, 0, 0, 1708956000);
```

- Store encrypted raw key in a new column or separate table
- Marker indicates no mnemonic recovery possible

### Option B: New Columns for Key Source and Type (Recommended)

```sql
ALTER TABLE wallets ADD COLUMN key_source TEXT DEFAULT 'mnemonic';
-- Values: 'mnemonic', 'raw_key', 'hardware'

ALTER TABLE wallets ADD COLUMN key_type TEXT DEFAULT 'primary';
-- Values: 'primary', 'privileged' — which BRC-100 keyring this key belongs to

ALTER TABLE wallets ADD COLUMN raw_key_encrypted BLOB;
-- Only populated when key_source = 'raw_key'
```

**Recommendation:** Option B — cleaner separation, future-proof for hardware wallets and dual-keyring.

The `key_type` column tracks which BRC-100 keyring the imported key belongs to. For mnemonic-based wallets, this is always `'primary'` (the mnemonic derives both keyrings). For raw key imports, it records the user's selection.

### Migration

```sql
-- V15 migration
ALTER TABLE wallets ADD COLUMN key_source TEXT DEFAULT 'mnemonic';
ALTER TABLE wallets ADD COLUMN key_type TEXT DEFAULT 'primary';
ALTER TABLE wallets ADD COLUMN raw_key_encrypted BLOB;
```

---

## Frontend Implementation

### UI Flow

```
┌─────────────────────────────────────────────┐
│  Recover Wallet                             │
├─────────────────────────────────────────────┤
│                                             │
│  Choose recovery method:                    │
│                                             │
│  ┌─────────────────────────────────────┐   │
│  │ 📝 Recovery Phrase (12/24 words)    │   │
│  └─────────────────────────────────────┘   │
│                                             │
│  ┌─────────────────────────────────────┐   │
│  │ 📁 Backup File (.hodos)             │   │
│  └─────────────────────────────────────┘   │
│                                             │
│  ┌─────────────────────────────────────┐   │
│  │ 🔑 Private Key (Hex or WIF)         │   │  ← NEW
│  └─────────────────────────────────────┘   │
│                                             │
│  ┌─────────────────────────────────────┐   │
│  │ 📱 Import from Centbee              │   │
│  └─────────────────────────────────────┘   │
│                                             │
└─────────────────────────────────────────────┘
```

### Private Key Input Screen

```
┌─────────────────────────────────────────────┐
│  Import Private Key                         │
├─────────────────────────────────────────────┤
│                                             │
│  [!] WARNING (amber #e6a200 background)     │
│  Importing a raw private key means you      │
│  will NOT have a recovery phrase backup.    │
│  Store your private key securely!           │
│                                             │
│  Key Type:                                  │
│  (o) Primary (Identity Key)  [default]      │
│  ( ) Privileged (Sensitive Operations)      │
│                                             │
│  [i] Migrating from a BRC-100 wallet?       │
│      Use your primary key to preserve       │
│      your identity. (teal #1a6b6a text)     │
│                                             │
│  Enter your private key:                    │
│  ┌─────────────────────────────────────┐   │
│  │ [                                  ]│   │
│  │ Hex (64 chars) or WIF format       │   │
│  └─────────────────────────────────────┘   │
│                                             │
│  Format detected: Valid WIF (compressed)    │
│  (green #2e7d32)                            │
│                                             │
│  [x] I understand there is no recovery      │
│      phrase for this wallet                 │
│                                             │
│  [Cancel](slate)        [Import Key](gold)  │
│                                             │
└─────────────────────────────────────────────┘
```

**If "Privileged" is selected, show additional warning:**
```
┌─────────────────────────────────────────────┐
│  [!] PRIVILEGED KEY WARNING (amber)         │
│  Importing a privileged key will create a   │
│  new wallet identity. Your certificates,    │
│  domain permissions, and on-chain identity  │
│  from another BRC-100 wallet will NOT       │
│  transfer. Use your primary key instead     │
│  to preserve your identity.                 │
└─────────────────────────────────────────────┘
```

### Branding Notes (Phase 1d)

All Phase 1d UI must follow [helper-2](./helper-2-design-philosophy.md) and [helper-4](./helper-4-branding-colors-logos.md):
- Primary action button (Import Key): gold `#a67c00`
- Cancel button: slate `#4a5568` (secondary)
- Warning banner: amber `#e6a200` background (caution, not error)
- Privileged key warning: amber `#e6a200` background
- BRC-100 migration info: teal `#1a6b6a` text
- Format detection success: green `#2e7d32`
- Format detection error: red `#c62828`
- Input: native `<input>` element (not MUI TextField -- CEF overlay compatibility)
- Button states: hover + pressed + disabled + loading per helper-2
- Use Inter font for all text; Courier New for key display

### Component Updates

**File:** `frontend/src/components/WalletSetupModal.tsx`

```typescript
// Add new recovery method
type RecoveryMethod = 'mnemonic' | 'backup' | 'privateKey' | 'centbee';
type KeyType = 'primary' | 'privileged';

// New component for private key input
function PrivateKeyRecoveryStep({ onComplete, onBack }: StepProps) {
  const [privateKey, setPrivateKey] = useState('');
  const [keyType, setKeyType] = useState<KeyType>('primary');
  const [format, setFormat] = useState<'unknown' | 'hex' | 'wif'>('unknown');
  const [isValid, setIsValid] = useState(false);
  const [understood, setUnderstood] = useState(false);
  const [loading, setLoading] = useState(false);

  const validateKey = (key: string) => {
    if (/^[0-9a-fA-F]{64}$/.test(key)) {
      setFormat('hex');
      setIsValid(true);
      return;
    }
    if (/^[5KL][1-9A-HJ-NP-Za-km-z]{50,51}$/.test(key)) {
      setFormat('wif');
      setIsValid(true);
      return;
    }
    setFormat('unknown');
    setIsValid(false);
  };

  const handleImport = async () => {
    setLoading(true);
    try {
      const response = await fetch('/wallet/recover-from-key', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ privateKey, keyType })
      });
      const result = await response.json();
      if (result.success) {
        toast.warning(result.warning);
        onComplete();
      } else {
        toast.error(result.error);
      }
    } finally {
      setLoading(false);
    }
  };

  // NOTE: Use native <input> elements, NOT MUI TextField (CEF overlay compatibility)
  // NOTE: All colors from helper-4-branding-colors-logos.md
  return (
    <div style={{ padding: '16px' }}>
      {/* Warning banner - amber background */}
      <div style={{ background: '#fff8e1', border: '1px solid #e6a200', borderRadius: '8px', padding: '12px', marginBottom: '16px' }}>
        <span style={{ color: '#e6a200', fontWeight: 600 }}>Warning:</span>
        <span> Importing a raw private key means you will NOT have a recovery phrase backup. Store your private key securely!</span>
      </div>

      {/* Key type selector */}
      <div style={{ marginBottom: '16px' }}>
        <label style={{ fontWeight: 600, display: 'block', marginBottom: '8px' }}>Key Type</label>
        <label style={{ display: 'block', marginBottom: '4px', cursor: 'pointer' }}>
          <input type="radio" name="keyType" value="primary" checked={keyType === 'primary'} onChange={() => setKeyType('primary')} />
          <span style={{ marginLeft: '8px' }}>Primary (Identity Key)</span>
        </label>
        <label style={{ display: 'block', cursor: 'pointer' }}>
          <input type="radio" name="keyType" value="privileged" checked={keyType === 'privileged'} onChange={() => setKeyType('privileged')} />
          <span style={{ marginLeft: '8px' }}>Privileged (Sensitive Operations)</span>
        </label>
      </div>

      {/* BRC-100 migration info - teal text */}
      <div style={{ color: '#1a6b6a', fontSize: '13px', marginBottom: '12px' }}>
        Migrating from MetaNet or another BRC-100 wallet? Use your primary key to preserve your identity.
      </div>

      {/* Privileged key warning - only shown when privileged selected */}
      {keyType === 'privileged' && (
        <div style={{ background: '#fff8e1', border: '1px solid #e6a200', borderRadius: '8px', padding: '12px', marginBottom: '16px' }}>
          <strong>Privileged Key Warning:</strong> Importing a privileged key creates a new wallet identity.
          Your certificates, domain permissions, and on-chain identity from another BRC-100 wallet will NOT transfer.
          Use your primary key instead to preserve your identity.
        </div>
      )}

      {/* Private key input - native <input> for CEF compatibility */}
      <div style={{ marginBottom: '12px' }}>
        <label style={{ fontWeight: 600, display: 'block', marginBottom: '4px' }}>Enter your private key</label>
        <input
          type="password"
          value={privateKey}
          onChange={(e) => { setPrivateKey(e.target.value); validateKey(e.target.value); }}
          placeholder="Hex (64 chars) or WIF format"
          style={{ width: '100%', padding: '10px', border: '1px solid #4a5568', borderRadius: '4px', fontFamily: "'Courier New', monospace" }}
          autoComplete="off"
        />
      </div>

      {/* Format detection - green for valid, red for invalid */}
      {format !== 'unknown' && (
        <div style={{ color: isValid ? '#2e7d32' : '#c62828', fontSize: '13px', marginBottom: '12px' }}>
          Format: {format.toUpperCase()} {isValid ? '(valid)' : '(invalid)'}
        </div>
      )}

      {/* Confirmation checkbox */}
      <label style={{ display: 'flex', alignItems: 'center', marginBottom: '16px', cursor: 'pointer' }}>
        <input type="checkbox" checked={understood} onChange={(e) => setUnderstood(e.target.checked)} />
        <span style={{ marginLeft: '8px' }}>I understand there is no recovery phrase for this wallet</span>
      </label>

      {/* Action buttons - gold primary, slate secondary */}
      <div style={{ display: 'flex', justifyContent: 'flex-end', gap: '12px' }}>
        <button onClick={onBack} style={{ padding: '10px 20px', background: '#4a5568', color: '#fff', border: 'none', borderRadius: '4px', cursor: 'pointer' }}>
          Cancel
        </button>
        <button
          onClick={handleImport}
          disabled={!isValid || !understood || loading}
          style={{ padding: '10px 20px', background: (!isValid || !understood || loading) ? '#ccc' : '#a67c00', color: '#fff', border: 'none', borderRadius: '4px', cursor: (!isValid || !understood || loading) ? 'not-allowed' : 'pointer' }}
        >
          {loading ? 'Importing...' : 'Import Key'}
        </button>
      </div>
    </div>
  );
}
```

---

## Security Considerations

### 1. No Mnemonic = No Standard Backup

Users must understand:
- Cannot use "recovery phrase" to restore
- Must keep original raw key safe
- Export to backup file IS available (encrypted)

### 2. Input Sanitization

- Clear private key from memory after import
- Don't log the key value
- Use secure input field (no autocomplete, hidden text)

### 3. Key Validation

- Verify key is valid secp256k1 scalar (1 < key < curve order)
- Reject obviously weak keys (e.g., 0, 1, very small numbers)

---

## Testing Plan

### Unit Tests

```rust
#[test]
fn test_validate_hex_private_key() {
    let valid_hex = "e8f32e723decf4051aefac8e2c93c9c5b214313817cdb01a1494b917c8436b35";
    assert!(validate_private_key(valid_hex).is_ok());
    
    let invalid_hex = "xyz123";
    assert!(validate_private_key(invalid_hex).is_err());
}

#[test]
fn test_validate_wif_private_key() {
    let valid_wif = "5HueCGU8rMjxEXxiPuD5BDku4MkFqeZyd4dZ1jvhTVqvbTLvyTJ";
    assert!(validate_private_key(valid_wif).is_ok());
    
    let invalid_checksum = "5HueCGU8rMjxEXxiPuD5BDku4MkFqeZyd4dZ1jvhTVqvbTLvyTX";
    assert!(validate_private_key(invalid_checksum).is_err());
}

#[test]
fn test_recover_from_key_creates_wallet() {
    // ... integration test ...
}
```

### Manual Tests

- [ ] Import valid hex key
- [ ] Import valid WIF key (compressed)
- [ ] Import valid WIF key (uncompressed)
- [ ] Reject invalid format
- [ ] Reject when wallet already exists
- [ ] Verify warning is displayed
- [ ] Verify no mnemonic in database
- [ ] Verify derived identity key is correct
- [ ] Verify transactions work with imported key

---

## Implementation Checklist

### Backend (Rust)

- [ ] Add `key_source`, `key_type`, and `raw_key_encrypted` columns (V15 migration)
- [ ] Implement `validate_private_key()` function
- [ ] Implement `POST /wallet/recover-from-key` endpoint with `keyType` field
- [ ] Update `WalletDatabase` for raw key storage
- [ ] Default `keyType` to `'primary'` when not provided
- [ ] Add unit tests

### Frontend

- [ ] Add "Private Key" option to recovery method selection
- [ ] Create `PrivateKeyRecoveryStep` component with key type selector
- [ ] Add format detection and validation
- [ ] Add warning banner and confirmation checkbox
- [ ] Add privileged key warning (shown when privileged selected)
- [ ] Add BRC-100 migration guidance info text
- [ ] Update recovery flow routing
- [ ] Apply Hodos branding (gold buttons, amber warnings, teal info) per helper-4
- [ ] Use native `<input>` elements (CEF overlay compatibility)
- [ ] Follow helper-2 interaction rules (button states, error patterns)

### Documentation

- [ ] Update user documentation
- [ ] Add warning about no mnemonic backup
- [ ] Document supported formats
- [ ] Document primary vs privileged key guidance for BRC-100 migrations

---

## Timeline

| Task | Estimate |
|------|----------|
| Database migration | 0.5 day |
| Backend endpoint | 1 day |
| Frontend UI | 1 day |
| Testing | 0.5 day |
| **Total** | **3 days** |

---

## References

- [BIP-39: Mnemonic code for generating deterministic keys](https://github.com/bitcoin/bips/blob/master/bip-0039.mediawiki)
- [WIF: Wallet Import Format](https://en.bitcoin.it/wiki/Wallet_import_format)
- [secp256k1 curve parameters](https://en.bitcoin.it/wiki/Secp256k1)
- [PRIVILEGED_KEYRING_ANALYSIS.md](../../PRIVILEGED_KEYRING_ANALYSIS.md) -- BRC-100 dual-keyring assessment
- [helper-2-design-philosophy.md](./helper-2-design-philosophy.md) -- Design principles and interaction rules
- [helper-4-branding-colors-logos.md](./helper-4-branding-colors-logos.md) -- Brand colors and typography
- [phase-3-peer-payments-research.md](./phase-3-peer-payments-research.md) -- Section 1: Primary vs Privileged Keys research

---

**End of Document**
