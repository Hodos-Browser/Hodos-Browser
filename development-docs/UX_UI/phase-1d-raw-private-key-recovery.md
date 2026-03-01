# Phase 1d: Raw Private Key Recovery

**Date:** 2026-02-26
**Status:** 📋 Planning
**Priority:** Medium
**Prerequisite:** Phase 1 (Initial Setup/Recovery) ✅ Complete
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
    log::info!("🔑 /wallet/recover-from-key called");
    
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

### Option B: New Column for Key Type

```sql
ALTER TABLE wallets ADD COLUMN key_source TEXT DEFAULT 'mnemonic';
-- Values: 'mnemonic', 'raw_key', 'hardware'

ALTER TABLE wallets ADD COLUMN raw_key_encrypted BLOB;
-- Only populated when key_source = 'raw_key'
```

**Recommendation:** Option B — cleaner separation, future-proof for hardware wallets.

### Migration

```sql
-- V15 migration
ALTER TABLE wallets ADD COLUMN key_source TEXT DEFAULT 'mnemonic';
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
│  ⚠️ WARNING                                 │
│  Importing a raw private key means you      │
│  will NOT have a recovery phrase backup.    │
│  Store your private key securely!           │
│                                             │
│  Enter your private key:                    │
│  ┌─────────────────────────────────────┐   │
│  │ [                                  ]│   │
│  │ Hex (64 chars) or WIF format       │   │
│  └─────────────────────────────────────┘   │
│                                             │
│  Format detected: ✅ Valid WIF (compressed) │
│                                             │
│  ☑️ I understand there is no recovery       │
│     phrase for this wallet                  │
│                                             │
│  [Cancel]                    [Import Key]   │
│                                             │
└─────────────────────────────────────────────┘
```

### Component Updates

**File:** `frontend/src/components/WalletSetupModal.tsx`

```typescript
// Add new recovery method
type RecoveryMethod = 'mnemonic' | 'backup' | 'privateKey' | 'centbee';

// New component for private key input
function PrivateKeyRecoveryStep({ onComplete, onBack }: StepProps) {
  const [privateKey, setPrivateKey] = useState('');
  const [format, setFormat] = useState<'unknown' | 'hex' | 'wif'>('unknown');
  const [isValid, setIsValid] = useState(false);
  const [understood, setUnderstood] = useState(false);
  
  const validateKey = (key: string) => {
    // Hex format: 64 hex characters
    if (/^[0-9a-fA-F]{64}$/.test(key)) {
      setFormat('hex');
      setIsValid(true);
      return;
    }
    
    // WIF format: starts with 5, K, or L
    if (/^[5KL][1-9A-HJ-NP-Za-km-z]{50,51}$/.test(key)) {
      setFormat('wif');
      setIsValid(true);
      return;
    }
    
    setFormat('unknown');
    setIsValid(false);
  };
  
  const handleImport = async () => {
    const response = await fetch('/wallet/recover-from-key', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ privateKey })
    });
    
    const result = await response.json();
    if (result.success) {
      // Show warning toast
      toast.warning(result.warning);
      onComplete();
    } else {
      toast.error(result.error);
    }
  };
  
  return (
    <div className="private-key-recovery">
      <WarningBanner>
        Importing a raw private key means you will NOT have a recovery 
        phrase backup. Store your private key securely!
      </WarningBanner>
      
      <Input
        label="Enter your private key"
        value={privateKey}
        onChange={(e) => {
          setPrivateKey(e.target.value);
          validateKey(e.target.value);
        }}
        placeholder="Hex (64 chars) or WIF format"
        type="password"  // Hide by default
      />
      
      {format !== 'unknown' && (
        <StatusBadge valid={isValid}>
          Format: {format.toUpperCase()} {isValid ? '✅' : '❌'}
        </StatusBadge>
      )}
      
      <Checkbox
        checked={understood}
        onChange={setUnderstood}
        label="I understand there is no recovery phrase for this wallet"
      />
      
      <ButtonRow>
        <Button variant="secondary" onClick={onBack}>Cancel</Button>
        <Button 
          variant="primary" 
          onClick={handleImport}
          disabled={!isValid || !understood}
        >
          Import Key
        </Button>
      </ButtonRow>
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

- [ ] Add `key_source` and `raw_key_encrypted` columns (V15 migration)
- [ ] Implement `validate_private_key()` function
- [ ] Implement `POST /wallet/recover-from-key` endpoint
- [ ] Update `WalletDatabase` for raw key storage
- [ ] Add unit tests

### Frontend

- [ ] Add "Private Key" option to recovery method selection
- [ ] Create `PrivateKeyRecoveryStep` component
- [ ] Add format detection and validation
- [ ] Add warning banner and confirmation checkbox
- [ ] Update recovery flow routing

### Documentation

- [ ] Update user documentation
- [ ] Add warning about no mnemonic backup
- [ ] Document supported formats

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

---

**End of Document**
