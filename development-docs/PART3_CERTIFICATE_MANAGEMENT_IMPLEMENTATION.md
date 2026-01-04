# Part 3: Certificate Management - Implementation Guide

> **Status**: ✅ All 6 Certificate Methods IMPLEMENTED - Testing Pending
> **Last Updated**: 2025-01-04
> **Current Phase**: Testing & Validation
> **Prerequisites**: ✅ Part 1 & Part 2 Complete
> **Completed**: ✅ All certificate methods implemented including discovery

## 🎯 Executive Summary

### Implementation Status: Core Features Working ✅

**✅ Completed (2025-12-19)**:
1. ✅ **Certificate Acquisition ('direct' protocol)**: Successfully acquiring certificates from certifier servers
2. ✅ **Certificate Acquisition ('issuance' protocol)**: Working with socialcert.net - certifier creates transaction, we verify and store
3. ✅ **Certificate Signature Verification**: BRC-52 signature verification working (fixed `anyone_private_key` bug)
4. ✅ **Certificate Storage**: Certificates stored in database with encrypted fields and correct transaction IDs
5. ✅ **Database Schema**: Migration v7 complete with `certificates` and `certificate_fields` tables
6. ✅ **Response Format**: Fixed UI display - returning JSON object instead of base64 string
7. ✅ **Transaction ID Extraction**: Fixed "Not on Chain" issue - extract txid from revocationOutpoint when certificate exists on-chain
8. ✅ **All Four Handlers**: `acquireCertificate`, `listCertificates`, `proveCertificate`, `relinquishCertificate` all implemented

**⏳ Testing Needed**:
1. ⏳ **End-to-End Testing**: Test all certificate methods with real-world apps
2. ⏳ **Third-Party Test Vectors**: Coordinate with ecosystem for validation data
3. ⏳ **Selective Disclosure**: `proveCertificate` needs full testing with real verifiers

**🆕 Key Discoveries**:
- Certificates use **PushDrop encoding** (not OP_RETURN)
- Fields are **always encrypted** (mandatory, not optional)
- `acquireCertificate` receives **JSON data directly** (not BEEF parsing)
- Database migration to **separate `certificate_fields` table** implemented
- **Critical Bug Fixed**: `anyone_private_key` was incorrectly initialized as `[1u8; 32]` - fixed to `[0u8; 32]` with last byte = 1
- **UI Display Fix**: Changed response from base64-encoded string to JSON object for proper UI display

**📋 Implementation Progress**:
- ✅ Database migration complete (v7)
- ✅ Certificate infrastructure (parser, verifier, types)
- ✅ BRC-2 encryption implementation
- ✅ Certificate acquisition ('direct' protocol)
- ✅ Certificate acquisition ('issuance' protocol) - certifier creates transaction
- ✅ Certificate storage and retrieval with correct transaction IDs
- ✅ All six certificate handlers implemented:
  - `acquireCertificate` - Acquire from certifier (direct/issuance)
  - `listCertificates` - Query local certificates
  - `proveCertificate` - Selective disclosure keyring
  - `relinquishCertificate` - Mark certificate as deleted
  - `discoverByIdentityKey` - Search by subject public key
  - `discoverByAttributes` - Search by decrypted field values
- ⏳ End-to-end testing with real-world apps
- ⏳ Third-party test vectors for validation

---

## 📊 Current Implementation Status (2025-12-17)

### ✅ What's Working

1. **Certificate Acquisition ('direct' protocol)**
   - Successfully acquiring certificates from certifier servers
   - Certificate data received as JSON and stored in database
   - Response format fixed to return JSON object (not base64) for UI display

2. **Certificate Signature Verification**
   - BRC-52 signature verification implemented and working
   - Fixed critical bug: `anyone_private_key` initialization (was `[1u8; 32]`, now `[0u8; 32]` with last byte = 1)
   - BRC-42 key derivation working correctly
   - Preimage serialization matching TypeScript SDK

3. **Database Storage**
   - Certificates stored in `certificates` table with metadata
   - Encrypted fields stored in `certificate_fields` table
   - Placeholder `certificate_txid` for certificates not yet on-chain (format: `"Not on Chain_<hash>"`)

4. **Certificate Infrastructure**
   - Certificate parser (JSON parsing)
   - Certificate verifier (signature + revocation checking)
   - Certificate types and data structures
   - Database repository for CRUD operations

### ⏳ What Needs Testing/Implementation

1. **End-to-End Testing**
   - ✅ `acquireCertificate` - Working with socialcert.net
   - ⏳ `listCertificates` - Implemented, needs testing
   - ⏳ `proveCertificate` - Implemented, needs testing with real verifiers
   - ⏳ `relinquishCertificate` - Implemented, needs testing

2. **Certificate Discovery (Not Yet Implemented)**
   - ⏳ `discoverByIdentityKey` - Find certificates by identity key
   - ⏳ `discoverByAttributes` - Find certificates by field attributes

3. **Real-World App Testing**
   - Test with various certificate types (cool cert, social cert, etc.)
   - Verify certificate display in UI
   - Test certificate revocation checking
   - Test selective disclosure (`proveCertificate`) with real verifiers

4. **Documentation**
   - ✅ Certificate lifecycle documented
   - ✅ Issuance protocol flow documented (certifier creates transaction)
   - ⏳ Document any differences from TypeScript SDK discovered during testing

### 🔍 Next Steps

1. **Complete Testing**
   - Test `listCertificates` with real-world apps
   - Test `proveCertificate` with real verifiers
   - Test `relinquishCertificate` functionality
   - Verify certificate display in UI works correctly

2. **Implement Certificate Discovery**
   - Implement `discoverByIdentityKey` (Call Code 21)
   - Implement `discoverByAttributes` (Call Code 22)
   - These are needed for some apps (e.g., microblog.bitspv.com uses `discoverByIdentityKey`)

3. **Documentation**
   - Update implementation notes with testing findings
   - Document any protocol differences discovered during real-world testing

**Note**: Core certificate acquisition and storage are working. The certifier creates the blockchain transaction (not the wallet). We verify the certificate signature, check on-chain status, extract the transaction ID from the revocationOutpoint, and store the certificate correctly. All four certificate handlers are implemented and ready for testing.

---

## 📋 Table of Contents

1. [Introduction: Certificate System Design](#introduction-certificate-system-design)
2. [How Blockchain & Cryptography Verify Certificates](#how-blockchain--cryptography-verify-certificates)
3. [Method Breakdown](#method-breakdown)
4. [Research Findings](#research-findings)
5. [Implementation Plan](#implementation-plan)
6. [Dependencies & Prerequisites](#dependencies--prerequisites)
7. [Reference Documentation](#reference-documentation)

---

## Introduction: Certificate System Design

### Why BRC-52 Identity Certificates?

The BRC-52 certificate system provides a **decentralized, privacy-preserving identity verification mechanism** that eliminates the need for centralized certificate authorities. Unlike traditional PKI systems that rely on trusted third parties, BRC-52 certificates leverage the **immutability of the Bitcoin SV blockchain** and **cryptographic proofs** to create verifiable, tamper-proof identity credentials.

### Key Design Principles

1. **Blockchain Anchoring**: Certificates are embedded in blockchain transactions, creating an immutable record of issuance
2. **Cryptographic Verification**: ECDSA signatures from certifiers prove authenticity without requiring a central authority
3. **Privacy by Default**: Certificate fields are encrypted using BRC-2 (AES-GCM) to protect sensitive information
4. **Selective Disclosure**: The `keyring` mechanism allows revealing specific fields without exposing all data
5. **UTXO-Based Revocation**: Certificates can be revoked by spending a specific UTXO, creating a decentralized revocation mechanism

### How It Works

**Certificate Lifecycle**:
1. **Issuance**: A certifier (trusted party) creates a certificate containing identity information
2. **Signing**: The certifier signs the certificate with their private key (ECDSA signature)
3. **Blockchain Embedding**: The certificate is embedded in a transaction output and broadcast to the blockchain
4. **Acquisition**: Wallets parse the certificate from the transaction and verify its authenticity
5. **Storage**: Valid certificates are stored locally for future use
6. **Revelation**: Certificate holders can selectively reveal fields to verifiers using the `keyring`
7. **Revocation**: Certifiers can revoke certificates by spending the `revocationOutpoint` UTXO

---

## How Blockchain & Cryptography Verify Certificates

### Three-Layer Verification System

BRC-52 certificates use a **three-layer verification system** that combines blockchain immutability, cryptographic signatures, and UTXO-based revocation:

#### Layer 1: Blockchain Immutability

**How it works**:
- Certificates are embedded in **transaction outputs** on the Bitcoin SV blockchain
- Once confirmed, the transaction becomes part of the immutable blockchain history
- The certificate's existence and issuance time are permanently recorded
- Anyone can verify a certificate was issued at a specific time by checking the blockchain

**What it proves**:
- ✅ Certificate was issued at a specific block height/time
- ✅ Certificate data has not been modified since issuance
- ✅ Certificate transaction exists and is confirmed on-chain

**Limitation**: Blockchain alone doesn't prove the certificate is **authentic** or **valid** - that requires cryptographic verification.

#### Layer 2: Cryptographic Signature Verification

**How it works**:
- The **certifier** (issuing authority) signs the certificate data using their private key
- The signature covers: `type`, `subject`, `validationKey`, `fields`, `certifier`
- The certificate includes the certifier's **public key** (33-byte compressed)
- Verifiers use the certifier's public key to verify the ECDSA signature

**ECDSA Signature Verification Process**:
```
1. Extract certifier's public key from certificate
2. Reconstruct the signed data (type + subject + validationKey + fields + certifier)
3. Verify ECDSA signature using certifier's public key
4. If signature is valid → certificate is authentic (signed by certifier)
```

**What it proves**:
- ✅ Certificate was signed by the claimed certifier
- ✅ Certificate data has not been tampered with (signature would fail)
- ✅ Certifier's identity is cryptographically verifiable

**Cryptographic Details**:
- **Algorithm**: ECDSA with secp256k1 curve (Bitcoin standard)
- **Signature Format**: DER-encoded ECDSA signature (hex string)
- **Public Key Format**: 33-byte compressed public key (hex string)

#### Layer 3: UTXO-Based Revocation

**How it works**:
- Each certificate includes a `revocationOutpoint` field (format: `"txid.vout"`)
- This points to a specific UTXO (unspent transaction output)
- To revoke the certificate, the certifier **spends** this UTXO
- Wallets check if the revocation UTXO is spent before accepting a certificate

**Revocation Check Process**:
```
1. Extract revocationOutpoint from certificate (e.g., "abc123...def.0")
2. Query blockchain API for UTXO status
3. If UTXO is spent → certificate is REVOKED (reject)
4. If UTXO is unspent → certificate is ACTIVE (accept)
```

**What it proves**:
- ✅ Certificate has not been revoked by the certifier
- ✅ Revocation status is verifiable on-chain (no central authority needed)
- ✅ Real-time revocation checking (spending UTXO immediately revokes certificate)

**Advantages**:
- **Decentralized**: No need for a revocation list server
- **Real-time**: Revocation takes effect as soon as UTXO is spent
- **Transparent**: Anyone can verify revocation status on the blockchain

### Complete Verification Flow

When a wallet receives a certificate via `acquireCertificate`:

```
1. Parse certificate from BEEF transaction
   └─ Extract certificate JSON structure from transaction output

2. Verify blockchain record
   └─ Check transaction exists and is confirmed on-chain
   └─ Verify certificate data matches blockchain record

3. Verify cryptographic signature
   └─ Extract certifier's public key
   └─ Reconstruct signed data
   └─ Verify ECDSA signature
   └─ If invalid → REJECT (certificate is forged)

4. Check revocation status
   └─ Query revocationOutpoint UTXO status
   └─ If spent → REJECT (certificate is revoked)
   └─ If unspent → ACCEPT (certificate is active)

5. Decrypt fields (optional)
   └─ Use BRC-2 decryption with BRC-42 key derivation
   └─ Decrypt certificate fields for storage/display

6. Store in database
   └─ Save certificate metadata and encrypted fields
   └─ Index by identity_key for discovery
```

### Why This Design is Secure

1. **No Single Point of Failure**: No central certificate authority that can be compromised
2. **Cryptographic Proofs**: Mathematical verification of authenticity (not trust-based)
3. **Blockchain Immutability**: Certificate issuance is permanently recorded
4. **Privacy-Preserving**: Fields are encrypted, selective disclosure prevents data leakage
5. **Decentralized Revocation**: UTXO-based revocation doesn't require a central server

---

## Method Breakdown

### Method 17: `acquireCertificate` (Call Code 17)
**Status**: ⏳ Research Phase
**Complexity**: High
**Estimated Time**: 6-8 hours

**What It Does**:
- Receives a BEEF transaction containing a BRC-52 certificate
- Parses the certificate structure from the transaction
- Verifies the certificate's authenticity (signature + revocation check)
- Decrypts certificate fields using BRC-2
- Stores the certificate in the database

**Research Tasks**:
- [ ] **CRITICAL**: Read [BRC-52 spec](https://bsv.brc.dev/peer-to-peer/0052) completely
- [ ] Understand certificate structure (type, subject, validationKey, fields, certifier, signature, keyring)
- [ ] Review certificate parsing logic from transaction outputs
- [ ] Understand ECDSA signature verification process
- [ ] Understand field encryption (BRC-2 encryption for sensitive fields)
- [ ] Review reference implementation in TypeScript SDK (metanet-desktop, ts-brc100)

**Key Questions to Answer**:
- How is the certificate embedded in the transaction? (OP_RETURN? Output script?)
- What exact data is signed by the certifier? (order of fields, encoding)
- How do we extract the certificate from a BEEF transaction?
- What happens if certificate fields are not encrypted? (optional encryption?)
- How do we handle certificate updates/replacements?

---

### Method 18: `listCertificates` (Call Code 18)
**Status**: ⏳ Research Phase
**Complexity**: Low
**Estimated Time**: 2-3 hours

**What It Does**:
- Lists all certificates owned by the wallet
- Supports filtering (by type, certifier, active/relinquished status)
- Returns certificate metadata (may include decrypted fields)

**Research Tasks**:
- [ ] Review BRC-100 spec for `listCertificates` parameters
- [ ] Understand filtering parameters (type, certifier, status)
- [ ] Check if we need to decrypt fields or return encrypted
- [ ] Review return format (what fields to include?)

**Key Questions to Answer**:
- What filtering options are available? (type, certifier, relinquished status?)
- Should we return encrypted fields or decrypt them?
- What metadata should be included? (issuance date, certifier, type?)
- How to handle pagination? (limit/offset?)

---

### Method 19: `proveCertificate` (Call Code 19)
**Status**: ⏳ Research Phase
**Complexity**: Medium
**Estimated Time**: 4-6 hours

**What It Does**:
- Generates a proof that the wallet owns a certificate
- Enables selective disclosure of certificate fields
- Creates revelation keys from the `keyring` for specific fields
- Returns proof data that verifiers can use to validate ownership

**Research Tasks**:
- [ ] Review BRC-100 spec for `proveCertificate` parameters
- [ ] Understand proof format (what does the proof contain?)
- [ ] Review BRC-52 selective disclosure mechanism
- [ ] Understand how `keyring` works for field revelation
- [ ] Check reference implementation

**Key Questions to Answer**:
- What is the proof format? (signature? hash? revelation keys?)
- How does selective disclosure work? (which fields to reveal?)
- How do we generate revelation keys from the keyring?
- What does the verifier need to validate the proof?

---

### Method 20: `relinquishCertificate` (Call Code 20)
**Status**: ⏳ Research Phase
**Complexity**: Low
**Estimated Time**: 1-2 hours

**What It Does**:
- Marks a certificate as relinquished (wallet no longer claims ownership)
- Updates database status (sets `relinquished = 1`, `relinquished_at = NOW()`)
- Does NOT revoke the certificate (only certifier can revoke via UTXO spending)

**Research Tasks**:
- [ ] Review BRC-100 spec for `relinquishCertificate`
- [ ] Understand use cases (when would a user relinquish?)
- [ ] Check if this is permanent or reversible
- [ ] Understand difference between relinquish vs revoke

**Key Questions to Answer**:
- Is relinquishment permanent or can it be reversed?
- What's the difference between relinquish (wallet action) and revoke (certifier action)?
- Should we delete the certificate or just mark it as relinquished?
- Can a relinquished certificate be re-acquired later?

---

## Research Findings

### BRC-100 Method Specifications

#### Method 17: `acquireCertificate` (Call Code 17)

**Parameters** (from BRC-100 spec):
- `type`: Byte Array (Base64 encoded) - Certificate type identifier
- `certifier`: Byte Array (33 bytes) - Certifier's compressed public key
- `fields`: VarInt number + Map - Map of fieldName to fieldValue (both UTF-8 strings)
- `privileged`: Int8 (1 for true, 0 for false, -1 if not provided)
- `privilegedReason`: Int8 Length + UTF-8 String (Optional)
- `acquisitionProtocol`: UInt8 (1 for 'direct', 2 for 'issuance')

**If `acquisitionProtocol` is 'direct' (1)**:
- `serialNumber`: Byte Array (Base64 encoded) - Certificate serial number
- `revocationOutpoint`: Byte Array (32 bytes) + VarInt - Revocation outpoint (TXID + output index)
- `signature`: VarInt Length + Byte Array - Certifier's signature over certificate data
- `keyringRevealer`: Byte Array (33 bytes) or UInt8 (11 for 'certifier') - Revealer's compressed public key
- `keyringForSubject`: VarInt number of entries + Map - Map of fieldName to keyring values

**If `acquisitionProtocol` is 'issuance' (2)**:
- `certifierUrl`: UTF-8 String - Certifier's URL for issuance

**Return Values**:
- Error Code (1 byte): 0 on success, non-zero error code otherwise
- Response Data: `certificate` - Byte Array (serialized certificate binary data)

**TypeScript Implementation Insights** (`reference/ts-brc100/src/signer/methods/acquireDirectCertificate.ts`):
- Validates all required fields for 'direct' protocol
- Creates `TableCertificateX` object with certificate metadata
- Stores certificate fields separately in `TableCertificateField` table
- Each field has: `fieldName`, `fieldValue`, `masterKey` (from keyring)
- Returns `AcquireCertificateResult` with certificate data

---

#### Method 18: `listCertificates` (Call Code 18)

**Parameters** (from BRC-100 spec):
- `certifiers`: VarInt Length + Array - Array of certifier public keys (each 33 bytes)
- `types`: VarInt Length + Array - Array of certificate types (each 32 bytes)
- `limit`: VarInt - Maximum number of certificates to return (-1 if not provided)
- `offset`: VarInt - Number of certificates to skip (-1 if not provided)
- `privileged`: Int8 (1 for true, 0 for false, -1 if not provided)
- `privilegedReason`: Int8 Length + UTF-8 String (Optional)

**Return Values**:
- Error Code (1 byte): 0 on success
- Response Data:
  - `totalCertificates`: VarInt - Total number of certificates matching criteria
  - `certificates`: Array - Array of certificate binary data

**TypeScript Implementation Insights** (`reference/ts-brc100/src/storage/methods/listCertificates.ts`):
- Filters by `userId`, `isDeleted = false`
- Supports partial matching on: `type`, `subject`, `serialNumber`, `certifier`, `revocationOutpoint`, `signature`
- Joins with `certificate_fields` table to get field values
- Returns fields as `Record<string, string>` and keyring as `Record<string, string>`
- Implements pagination with `limit` and `offset`
- Returns `totalCertificates` count (exact count if less than limit, otherwise queries count)

---

#### Method 19: `proveCertificate` (Call Code 19)

**Parameters** (from BRC-100 spec):
- `certificate`: Certificate Struct (see below)
- `fieldsToReveal`: VarInt Length + Array - Array of fieldName strings (UTF-8) to reveal
- `verifier`: Byte Array (33 bytes) - Verifier's compressed public key
- `privileged`: Int8 (1 for true, 0 for false, -1 if not provided)
- `privilegedReason`: Int8 Length + UTF-8 String (Optional)

**Certificate Struct**:
- `type`: Byte Array (Base64 encoded)
- `subject`: Byte Array (33 bytes) - Subject's compressed public key
- `serialNumber`: Byte Array (Base64 encoded)
- `certifier`: Byte Array (33 bytes) - Certifier's compressed public key
- `revocationOutpoint`: Byte Array - Revocation outpoint (TXID + output index)
- `signature`: VarInt Length + Byte Array - Certificate signature
- `fields`: VarInt Length + Map - Map of fieldName to encrypted fieldValue

**Return Values**:
- Error Code (1 byte): 0 on success
- Response Data: `keyringForVerifier` - VarInt number of fields + Map (Map of fieldName to keyring values)

**TypeScript Implementation Insights** (`reference/ts-brc100/src/signer/methods/proveCertificate.ts`):
- First calls `listCertificates` to find the certificate (must match exactly 1 certificate)
- Uses `MasterCertificate.createKeyringForVerifier()` to generate revelation keys
- Takes `storageCert.keyring` (master keyring) and generates verifier-specific keyring
- Returns only the keyring for fields specified in `fieldsToReveal`
- Enables selective disclosure - verifier can only decrypt revealed fields

---

#### Method 20: `relinquishCertificate` (Call Code 20)

**Parameters** (from BRC-100 spec):
- `type`: Byte Array (Base64 encoded) - Certificate type identifier
- `serialNumber`: Byte Array (Base64 encoded) - Certificate serial number
- `certifier`: Byte Array (33 bytes) - Certifier's compressed public key

**Return Values**:
- Error Code (1 byte): 0 on success
- Response Data: None

**TypeScript Implementation Insights**:
- Marks certificate as `isDeleted = true` (soft delete)
- Certificate is no longer returned by `listCertificates`
- Certificate data is retained in database (for audit/history)
- Uses `type`, `serialNumber`, and `certifier` to uniquely identify certificate

---

### Database Schema (TypeScript Reference)

**TableCertificate** (`reference/ts-brc100/src/storage/schema/tables/TableCertificate.ts`):
```typescript
interface TableCertificate {
  certificateId: number        // Primary key
  userId: number              // Foreign key to user
  type: Base64String          // Certificate type (base64)
  serialNumber: Base64String  // Unique serial number
  certifier: PubKeyHex        // 33-byte compressed public key (hex)
  subject: PubKeyHex          // 33-byte compressed public key (hex)
  verifier?: PubKeyHex        // Optional verifier public key
  revocationOutpoint: OutpointString  // "txid.vout" format
  signature: HexString        // DER-encoded ECDSA signature (hex)
  isDeleted: boolean         // Soft delete flag
  created_at: Date
  updated_at: Date
}
```

**TableCertificateField** (`reference/ts-brc100/src/storage/schema/tables/TableCertificateField.ts`):
```typescript
interface TableCertificateField {
  certificateId: number       // Foreign key to certificate
  userId: number             // Foreign key to user
  fieldName: string          // Field name (max 50 bytes)
  fieldValue: string         // Encrypted field value (base64)
  masterKey: Base64String    // Master keyring value for this field
  created_at: Date
  updated_at: Date
}
```

**Key Differences from Our Schema**:
- TypeScript uses separate `certificate_fields` table (one row per field)
- Our schema uses JSON `attributes` column (single JSON object)
- TypeScript stores `masterKey` (keyring) per field
- TypeScript has `verifier` field (for selective disclosure)
- TypeScript uses `isDeleted` (soft delete) vs our `relinquished` boolean

**Recommendation**: Consider migrating to separate `certificate_fields` table for better querying and selective disclosure support.

---

### BRC-52 Identity Certificates

**Specification**: [BRC-52: Identity Certificates](https://bsv.brc.dev/peer-to-peer/0052)

**Certificate Structure**:
```json
{
  "type": "base64_encoded_certificate_type",
  "subject": "33-byte_compressed_public_key_hex",
  "validationKey": "base64_encoded_validation_key",
  "serialNumber": "base64_encoded_serial",
  "fields": {
    "field_name": "base64_encrypted_field_value"
  },
  "certifier": "33-byte_compressed_public_key_hex",
  "revocationOutpoint": "txid.vout",
  "signature": "DER_encoded_ECDSA_signature_hex",
  "keyring": {
    "field_name": "base64_encrypted_revelation_key"
  }
}
```

**Key Fields**:
- `type`: Certificate type identifier (base64-encoded)
- `subject`: Identity key of the certificate holder (33-byte compressed public key)
- `validationKey`: Key used for validation (base64-encoded) - **Note**: Not in BRC-100 spec, may be optional
- `serialNumber`: Unique serial number (base64-encoded)
- `fields`: Encrypted certificate data (BRC-2 encrypted, base64-encoded)
- `certifier`: Public key of the certifier (33-byte compressed)
- `revocationOutpoint`: UTXO that, when spent, revokes the certificate
- `signature`: ECDSA signature over certificate data (DER-encoded, hex)
- `keyring`: Encrypted revelation keys for selective disclosure

**Signature Verification**:
- Certifier signs: `type + subject + validationKey + fields + certifier` (per BRC-52)
- Use certifier's public key to verify ECDSA signature
- Signature format: DER-encoded ECDSA signature (hex string)

**Revocation Mechanism**:
- Check if `revocationOutpoint` UTXO is spent
- If spent → certificate is revoked (reject)
- If unspent → certificate is active (accept)

---

### BRC-2 Data Encryption and Decryption

**Specification**: [BRC-2: Data Encryption and Decryption](https://bsv.brc.dev/wallet/0002)

**Encryption Process**:
1. Compute BRC-43 invoice number from `protocolID`, `keyID`, `counterparty`
2. Use BRC-42 to derive child public key for recipient
3. Use BRC-42 to derive child private key for sender
4. Compute ECDH shared secret between child keys
5. Hash shared secret (X + Y coordinates) with SHA256 → AES-256-GCM key
6. Generate random 256-bit initialization vector (IV)
7. Encrypt data with AES-256-GCM using derived key and IV
8. Prepend IV to ciphertext (32 bytes IV + ciphertext)

**Decryption Process**:
1. Extract IV from first 32 bytes of ciphertext
2. Compute BRC-43 invoice number
3. Use BRC-42 to derive sender's child public key
4. Use BRC-42 to derive recipient's child private key
5. Compute ECDH shared secret
6. Hash shared secret with SHA256 → AES-256-GCM key
7. Decrypt ciphertext using key and IV

**Key Derivation**:
- Uses BRC-42 key derivation (ECDH + HMAC)
- Invoice number: `"<security_level>-<protocol_id>-<key_id>"`
- For `counterparty="self"`: Use sender's own public key

**Test Vectors** (from BRC-2 spec):
- Identity private key: `6a2991c9de20e38b31d7ea147bf55f5039e4bbc073160f5e0d541d1f17e321b8`
- Identity public key: `025ad43a22ac38d0bc1f8bacaabb323b5d634703b7a774c4268f6a09e4ddf79097`
- Counterparty: `0294c479f762f6baa97fbcd4393564c1d7bd8336ebd15928135bbcf575cd1a71a1`
- Protocol: `[2, "BRC2 Test"]`, KeyID: `42`
- Ciphertext (with IV): `[252, 203, 216, 184, ...]` (96 bytes total)
- Plaintext: `"BRC-2 Encryption Compliance Validated!"`

---

### BRC-53 Certificate Creation and Revelation

**Specification**: [BRC-53: Certificate Creation and Revelation](https://bsv.brc.dev/wallet/0053)

**Certificate Creation Request**:
- Wallet requests certificate creation from certifier
- Includes certificate signing request (CSR) with desired fields
- Certifier validates request and signs certificate
- Certificate is returned to wallet

**Certificate Proof Request**:
- Verifier requests proof of certificate ownership
- Wallet generates proof using `keyring` revelation keys
- Selective disclosure: reveal only requested fields
- Proof is returned to verifier for validation

**Key Concepts**:
- **CSR (Certificate Signing Request)**: Request for certificate creation
- **Selective Disclosure**: Reveal specific fields without exposing all data
- **Revelation Keys**: Keys from `keyring` used to prove field ownership

---

### PushDrop Encoding (Certificate Embedding)

**Discovery**: Certificates are embedded in transaction outputs using **PushDrop encoding**

**What is PushDrop?**:
- PushDrop is a protocol for encoding data in Bitcoin output scripts
- Allows storing structured data (like JSON) in locking scripts
- Used for token protocols and certificate storage

**Certificate Embedding Process** (from `identityUtils.ts`):
```typescript
// 1. Extract transaction from BEEF
const tx = Transaction.fromBEEF(output.beef)

// 2. Get output at specified index
const outputScript = tx.outputs[output.outputIndex].lockingScript

// 3. Decode PushDrop-encoded script
const decodedOutput = PushDrop.decode(outputScript)

// 4. Certificate JSON is in first PushDrop field
const certificate: VerifiableCertificate = JSON.parse(
  Utils.toUTF8(decodedOutput.fields[0])
)
```

**Key Points**:
- Certificate is stored as **JSON string** in PushDrop field
- PushDrop encoding allows multiple fields in single output
- Certificate is in `decodedOutput.fields[0]` (first field)
- Need PushDrop decoder library or implementation in Rust

**For `acquireCertificate`**:
- **Note**: 'direct' protocol receives certificate data **directly** (already parsed)
- PushDrop decoding is used for certificate **discovery** (overlay network lookups)
- Not required for basic `acquireCertificate` implementation
- May be needed for `discoverByIdentityKey` and `discoverByAttributes` methods

---

### MasterCertificate Class (from @bsv/sdk)

**Key Functions** (from TypeScript reference):

1. **`createCertificateFields(certifierWallet, subjectPubKey, plaintextFields)`**
   - Encrypts certificate fields using BRC-2
   - Returns: `{ certificateFields: encryptedFields, masterKeyring: masterKeys }`
   - Each field encrypted with random symmetric key
   - Keys encrypted so both certifier and subject can decrypt

2. **`createKeyringForVerifier(subjectWallet, certifierPubKey, verifierPubKey, fields, fieldsToReveal, masterKeyring, serialNumber)`**
   - Generates verifier-specific revelation keys
   - Uses BRC-42 key derivation (likely)
   - Returns keyring with only `fieldsToReveal` fields
   - Enables selective disclosure

3. **`decryptFields(wallet, masterKeyring, encryptedFields, certifierPubKey)`**
   - Decrypts certificate fields using master keyring
   - Subject can decrypt all fields
   - Verifier can decrypt only revealed fields (using verifier-specific keyring)

**Rust Implementation Needed**:
- Need to implement these functions in Rust
- Or find Rust equivalent library
- Core logic: BRC-2 encryption + BRC-42 key derivation

---

### BRC-100 Wallet Interface

**Specification**: [BRC-100: Unified Wallet-to-Application Interface](https://bsv.brc.dev/wallet/0100)

**Group C Methods** (Certificate Management):
- **Call Code 17**: `acquireCertificate` - Acquire and store certificate
- **Call Code 18**: `listCertificates` - List stored certificates
- **Call Code 19**: `proveCertificate` - Generate ownership proof
- **Call Code 20**: `relinquishCertificate` - Mark certificate as relinquished

**Research Needed**:
- [ ] Review exact parameter formats for each method
- [ ] Understand return formats
- [ ] Check error handling requirements
- [ ] Review permission/authorization requirements

---

## Implementation Plan

### Phase 1: Final Research & PushDrop Analysis (Week 1)

**Goal**: Complete PushDrop understanding and finalize implementation approach

**Tasks**:
1. **Review @bsv/sdk PushDrop Implementation** (2-3 hours) ✅ **NEW PRIORITY**
   - Check `reference/ts-brc100/node_modules/@bsv/sdk` for PushDrop source
   - Understand `PushDrop.decode()` implementation details
   - Understand `PushDrop.lock()` and `PushDrop.unlock()` methods
   - Document script parsing logic for Rust translation
   - Understand field extraction from locking scripts

2. **Read BRC-100 Group C Methods** (1-2 hours) ✅ **COMPLETE**
   - ✅ Reviewed `acquireCertificate` parameters
   - ✅ Reviewed `listCertificates` parameters
   - ✅ Reviewed `proveCertificate` parameters
   - ✅ Reviewed `relinquishCertificate` parameters

3. **Review Reference Implementations** (2-3 hours) ✅ **COMPLETE**
   - ✅ Reviewed ts-brc100 TypeScript SDK
   - ✅ Reviewed certificate parsing in `identityUtils.ts`
   - ✅ Reviewed `acquireDirectCertificate` implementation
   - ✅ Reviewed `proveCertificate` implementation

**Deliverable**: PushDrop implementation plan and ready to start coding

**Note**: BRC-52, BRC-2, and BRC-53 specs will be read **during implementation** when we need specific details (signature verification, encryption, keyring generation).

---

### Phase 2: Database Schema Migration (Week 1)

**Goal**: Migrate to separate `certificate_fields` table for better querying and selective disclosure

**Tasks**:
1. **Create Migration for `certificate_fields` Table**
   - Add new migration function `create_schema_v6()` in `migrations.rs`
   - Create `certificate_fields` table with schema:
     ```sql
     CREATE TABLE IF NOT EXISTS certificate_fields (
         id INTEGER PRIMARY KEY AUTOINCREMENT,
         certificate_id INTEGER NOT NULL,
         user_id INTEGER NOT NULL,
         field_name TEXT NOT NULL,
         field_value TEXT NOT NULL,  -- Base64-encoded encrypted value
         master_key TEXT NOT NULL,    -- Base64-encoded keyring value
         created_at INTEGER NOT NULL,
         updated_at INTEGER NOT NULL,
         FOREIGN KEY (certificate_id) REFERENCES certificates(id) ON DELETE CASCADE,
         UNIQUE(certificate_id, field_name)
     )
     ```
   - Add indexes: `idx_certificate_fields_certificate_id`, `idx_certificate_fields_field_name`

2. **Update `certificates` Table Schema**
   - Add missing fields if needed:
     - `type` (TEXT, base64-encoded certificate type)
     - `serial_number` (TEXT, base64-encoded)
     - `certifier` (TEXT, 33-byte hex public key)
     - `subject` (TEXT, 33-byte hex public key) - already exists as `identity_key`?
     - `verifier` (TEXT, optional, 33-byte hex public key)
     - `revocation_outpoint` (TEXT, "txid.vout" format)
     - `signature` (TEXT, DER-encoded ECDSA signature hex)
     - `is_deleted` (BOOLEAN, default 0) - replace `relinquished`?
   - Review current schema and add missing columns
   - Add indexes for common queries

3. **Data Migration** (if existing certificates exist)
   - Parse JSON `attributes` column from existing certificates
   - Split into `certificate_fields` table rows
   - Preserve `master_key` if available in JSON

4. **Update Certificate Repository** (`rust-wallet/src/database/certificate_repo.rs`)
   - `insert_certificate_with_fields()` - Insert certificate + fields
   - `get_certificate_with_fields()` - Retrieve certificate with fields joined
   - `list_certificates()` - Query with field filtering
   - `update_relinquished()` - Mark as `is_deleted = true`
   - `get_by_identity_key()` - Find by subject/identity_key

**Deliverable**: Database schema ready for certificate management with separate fields table

---

### Phase 3: PushDrop Implementation (Week 1-2) ✅ **ANALYSIS COMPLETE**

**Goal**: Implement PushDrop encoding/decoding for certificate discovery (BRC-48)

**Status**: ✅ **Analysis Complete** - Ready for implementation

**Tasks**:
1. **Review @bsv/sdk PushDrop Implementation** ✅ **COMPLETE** (2-3 hours)
   - ✅ Reviewed `reference/ts-brc100/node_modules/@bsv/sdk/src/script/templates/PushDrop.ts`
   - ✅ Understood `PushDrop.decode()` implementation (see analysis above)
   - ✅ Understood `PushDrop.lock()` and minimal encoding logic
   - ✅ Documented script parsing logic and field extraction
   - ✅ Identified script structure for both 'before' and 'after' lock positions

2. **Create PushDrop Module** (`rust-wallet/src/script/pushdrop.rs`) ⏳ **NEXT**
   - `pub fn decode(script: &[u8]) -> Result<PushDropDecoded>` - Decode PushDrop-encoded script
     - Parse script chunks
     - Extract public key (first or last chunk)
     - Extract fields until OP_DROP/OP_2DROP
     - Handle special opcodes (OP_0, OP_1-OP_16, OP_1NEGATE)
   - `pub fn encode(fields: &[Vec<u8>], public_key: &[u8], lock_position: LockPosition) -> Result<Vec<u8>>`
     - Encode fields using minimal encoding
     - Add OP_DROP/OP_2DROP appropriately
     - Combine with pubkey + OP_CHECKSIG
   - `fn parse_script_chunks(script: &[u8]) -> Result<Vec<ScriptChunk>>`
     - Parse script into chunks
     - Handle OP_PUSHDATA1/2/4 for variable-length data
   - `fn create_minimally_encoded_chunk(data: &[u8]) -> Vec<u8>`
     - Implement minimal encoding logic
     - Handle OP_0, OP_1-OP_16, OP_1NEGATE
     - Handle OP_PUSHDATA1/2/4

3. **Bitcoin Script Parser** (`rust-wallet/src/script/parser.rs`)
   - Parse script opcodes and data pushes
   - Handle variable-length data pushes (OP_PUSHDATA1, OP_PUSHDATA2, OP_PUSHDATA4)
   - Extract data before OP_DROP
   - Verify script structure
   - **Note**: May use existing Rust Bitcoin script libraries if available

4. **Testing** (2-3 hours)
   - Test with certificate JSON from `identityUtils.ts` examples
   - Test with multiple fields
   - Test edge cases (empty fields, large fields)
   - Compare output with TypeScript implementation
   - Test both 'before' and 'after' lock positions

**Deliverable**: PushDrop decoder/encoder ready for certificate discovery

**Note**: This phase is **required for certificate discovery** (Part 4), but **not needed for basic `acquireCertificate`** which receives certificate data directly.

**Implementation Details** (from analysis):
- Script structure: `[pubkey, OP_CHECKSIG, field1, field2, ..., OP_DROP]` (for 'before')
- Fields start at chunk index 2 (skip pubkey at 0, OP_CHECKSIG at 1)
- Stop extraction when hitting OP_DROP (0x75) or OP_2DROP (0x6d)
- Handle special opcodes: OP_0 (0x00), OP_1-OP_16 (0x51-0x60), OP_1NEGATE (0x4f)
- Certificate JSON is in `fields[0]` (first field)

---

### Phase 4: Certificate Infrastructure (Week 2)

**Goal**: Build core certificate parsing and verification infrastructure

**Tasks**:
1. **Create Certificate Module** (`rust-wallet/src/certificate/`)
   - `mod.rs` - Module exports
   - `parser.rs` - Certificate parsing from JSON (for `acquireCertificate`)
   - `verifier.rs` - Signature verification and revocation checking
   - `types.rs` - Certificate data structures
   - `selective_disclosure.rs` - Keyring generation for selective disclosure

2. **Implement BRC-2 Encryption Module** (`rust-wallet/src/crypto/brc2.rs`)
   - Encryption function (AES-256-GCM)
   - Decryption function (AES-256-GCM)
   - Key derivation using BRC-42
   - Test vector validation
   - IV handling (32-byte prepended to ciphertext)
   - **Research BRC-2 spec during implementation** if needed

3. **Create Certificate Repository** (`rust-wallet/src/database/certificate_repo.rs`)
   - `insert_certificate_with_fields()` - Store certificate + fields
   - `get_certificate_by_txid()` - Retrieve by transaction ID
   - `list_certificates()` - List with filtering (joins with fields table)
   - `update_relinquished()` - Mark as `is_deleted = true`
   - `get_by_identity_key()` - Find by identity key
   - `get_certificate_fields()` - Get fields for a certificate

**Deliverable**: Certificate infrastructure ready for handler implementation

**Note**: Research BRC-2, BRC-52 signature verification, and BRC-53 keyring generation **during implementation** as needed.

---

### Phase 5: Handler Implementation (Week 2-3)

**Goal**: Implement all four certificate management methods

**Tasks**:
1. **Implement `acquireCertificate`** (6-8 hours)
   - Parse certificate from BEEF transaction
   - Verify signature (ECDSA)
   - Check revocation status (UTXO query)
   - Decrypt fields (BRC-2)
   - Store in database
   - Return certificate data

2. **Implement `listCertificates`** (2-3 hours)
   - Query database with filters
   - Support type, certifier, status filtering
   - Return certificate list
   - Handle pagination (if needed)

3. **Implement `proveCertificate`** (4-6 hours)
   - Query certificate from database
   - Generate proof (selective disclosure)
   - Extract revelation keys from keyring
   - Return proof data

4. **Implement `relinquishCertificate`** (1-2 hours)
   - Update database status
   - Set `relinquished = 1`
   - Set `relinquished_at = NOW()`
   - Return success

**Deliverable**: All four methods implemented and tested

---

### Phase 5: Testing & Integration (Week 3)

**Goal**: Test certificate system with real certificates

**Tasks**:
1. **Unit Tests**
   - Certificate parsing tests
   - Signature verification tests
   - BRC-2 encryption/decryption tests
   - Revocation checking tests

2. **Integration Tests**
   - End-to-end `acquireCertificate` flow
   - Certificate listing with filters
   - Proof generation and validation
   - Relinquishment flow

3. **Real-World Testing**
   - Test with actual BRC-52 certificates from blockchain
   - Test with different certifiers
   - Test revocation checking
   - Test selective disclosure

**Deliverable**: Fully tested certificate management system

---

## Dependencies & Prerequisites

### ✅ Completed Prerequisites

- **Database Schema**: ✅ `certificates` table exists
- **BRC-42 Key Derivation**: ✅ Implemented (`rust-wallet/src/crypto/brc42.rs`)
- **BRC-43 Invoice Numbers**: ✅ Implemented (`rust-wallet/src/crypto/brc43.rs`)
- **BEEF Parsing**: ✅ Implemented (`rust-wallet/src/beef.rs`)
- **ECDSA Signing**: ✅ Available (secp256k1 library)
- **UTXO Checking**: ✅ Available (WhatsOnChain API integration)

### ❌ Missing Dependencies

- **BRC-2 Encryption Module**: ❌ Needs implementation
- **Certificate Parser**: ❌ Needs implementation
- **Certificate Verifier**: ❌ Needs implementation
- **Certificate Repository**: ❌ Needs implementation
- **AES-256-GCM Library**: ❌ Need to add Rust crate (`aes-gcm`)

### Required Rust Crates

Add to `Cargo.toml`:
```toml
[dependencies]
aes-gcm = "0.10"  # AES-256-GCM encryption
base64 = "0.21"   # Base64 encoding (already have?)
```

---

## Reference Documentation

### Official Specifications

1. **[BRC-52: Identity Certificates](https://bsv.brc.dev/peer-to-peer/0052)** ⭐ **PRIMARY**
   - Certificate structure and format
   - Signature verification process
   - Revocation mechanism
   - Selective disclosure

2. **[BRC-2: Data Encryption and Decryption](https://bsv.brc.dev/wallet/0002)** ⭐ **CRITICAL**
   - AES-256-GCM encryption
   - BRC-42 key derivation for encryption
   - Test vectors for validation

3. **[BRC-53: Certificate Creation and Revelation](https://bsv.brc.dev/wallet/0053)**
   - Certificate creation flow
   - Proof generation
   - Selective disclosure mechanism

4. **[BRC-100: Wallet Interface](https://bsv.brc.dev/wallet/0100)**
   - Group C methods (Call Codes 17-20)
   - Parameter formats
   - Return formats
   - Error handling

### Internal Documentation

- `development-docs/GROUP_C_EXECUTION_PLAN.md` - Overall Group C plan
- `development-docs/RUST_WALLET_DB_ARCHITECTURE.md` - Database schema
- `rust-wallet/src/database/migrations.rs` - Certificates table schema

### Reference Implementations

- **metanet-desktop**: https://github.com/BSVArchie/metanet-desktop ⭐ **PRIMARY REFERENCE**
  - TypeScript implementation with all Group C methods
  - Certificate management examples
  - Real-world usage patterns

- **ts-brc100**: `reference/ts-brc100/` - TypeScript SDK
  - BRC-100 method implementations
  - Certificate handling examples

---

## Next Steps

### Immediate Actions

1. **Start Research Phase**
   - [ ] Read BRC-52 spec completely (2-3 hours)
   - [ ] Read BRC-2 spec completely (1-2 hours)
   - [ ] Read BRC-53 spec (1 hour)
   - [ ] Read BRC-100 Group C methods (1-2 hours)
   - [ ] Review reference implementations (2-3 hours)

2. **Document Research Findings**
   - [ ] Answer all key questions for each method
   - [ ] Document certificate parsing approach
   - [ ] Document signature verification process
   - [ ] Document BRC-2 encryption implementation plan
   - [ ] Update this document with findings

3. **Begin Implementation**
   - [ ] Create certificate module structure
   - [ ] Implement BRC-2 encryption module
   - [ ] Implement certificate parser
   - [ ] Implement certificate verifier
   - [ ] Create certificate repository

---

---

## Research Summary & Key Findings

### ✅ Completed Research (2025-12-08)

**1. BRC-100 Method Specifications** ✅
- Reviewed all 4 certificate methods (Call Codes 17-20)
- Documented exact parameter formats and return values
- Identified two acquisition protocols: 'direct' and 'issuance'

**2. TypeScript Implementation Analysis** ✅
- Reviewed `ts-brc100` reference implementation
- Analyzed database schema (separate `certificate_fields` table)
- Understood selective disclosure mechanism via `keyring`
- Identified soft delete pattern (`isDeleted` flag)

**3. Database Schema Comparison** ✅
- Our schema: Single `certificates` table with JSON `attributes` column
- TypeScript schema: Separate `certificates` + `certificate_fields` tables
- **Recommendation**: Consider migrating to separate fields table for better querying

### 🔍 Key Insights Discovered

**1. Acquisition Protocols**:
- **'direct'**: Certificate data provided directly (requires all fields: serialNumber, signature, revocationOutpoint, keyring)
- **'issuance'**: Certificate requested from certifier URL (certifier creates and signs certificate)

**2. Selective Disclosure**:
- `keyring` contains master revelation keys for each field
- `proveCertificate` generates verifier-specific keyring using `MasterCertificate.createKeyringForVerifier()`
- Only fields in `fieldsToReveal` are included in returned keyring
- Verifier can decrypt only revealed fields using their private key

**3. Certificate Storage**:
- Fields stored separately (one row per field in `certificate_fields` table)
- Each field has: `fieldName`, `fieldValue` (encrypted), `masterKey` (keyring)
- Enables efficient querying and selective disclosure

**4. Relinquishment vs Revocation**:
- **Relinquish**: Wallet action - marks certificate as `isDeleted` (soft delete)
- **Revoke**: Certifier action - spends `revocationOutpoint` UTXO on blockchain
- Relinquished certificates are retained in database but not returned by `listCertificates`

### ✅ Critical Questions - Research Findings

**1. Certificate Parsing from BEEF Transactions** ✅ **FULLY ANSWERED**
- **Answer**: Certificates are embedded in transaction outputs using **PushDrop encoding** (BRC-48)
- **PushDrop Pattern**: `<certificate_json> OP_DROP <public_key> OP_CHECKSIG`
- **Implementation** (from `reference/ts-brc100/src/utility/identityUtils.ts` lines 130-135):
  ```typescript
  const tx = Transaction.fromBEEF(output.beef)
  const decodedOutput = PushDrop.decode(tx.outputs[output.outputIndex].lockingScript)
  const certificate: VerifiableCertificate = JSON.parse(Utils.toUTF8(decodedOutput.fields[0]))
  ```
- **Process**:
  1. Extract transaction from BEEF
  2. Get output at specified index
  3. Decode locking script using PushDrop (BRC-48)
  4. Certificate JSON is in `decodedOutput.fields[0]` (first PushDrop field)
  5. Parse JSON to get certificate structure
- **Impact**: Need PushDrop decoder (BRC-48 implementation) for certificate discovery
- **Note**:
  - Certificate is stored as JSON string in PushDrop-encoded output script
  - PushDrop allows multiple fields, certificate is typically in first field
  - **For `acquireCertificate`**: Not needed - certificate data provided directly in request
  - **For discovery methods**: Required to parse certificates from blockchain

**2. Signature Verification Process** ⚠️ **PARTIALLY ANSWERED**
- **BRC-52 says**: Certifier signs certificate data
- **From TypeScript code**: Certificate has `sign()` and `verify()` methods
- **Still Need**: Exact field order and encoding for signature
- **Questions Remaining**:
  - Exact field order: `type + subject + validationKey + fields + certifier`?
  - How are `fields` serialized? (JSON string? Sorted keys?)
  - Is `validationKey` included? (not in BRC-100 parameters)
- **Impact**: Need BRC-52 spec for exact signature verification process
- **Action**: Read BRC-52 spec section on signature creation/verification

**3. Selective Disclosure / Keyring Generation** ✅ **ANSWERED**
- **Answer**: Uses `MasterCertificate.createKeyringForVerifier()` from `@bsv/sdk`
- **Function Signature** (from test code):
  ```typescript
  MasterCertificate.createKeyringForVerifier(
    subjectWallet,           // Wallet of certificate subject
    certifierPubKey,          // Certifier's public key
    verifierPubKey,           // Verifier's public key
    signedCert.fields,        // Encrypted certificate fields
    ['name', 'email'],        // Fields to reveal
    masterKeyring,            // Master keyring from certificate creation
    signedCert.serialNumber   // Certificate serial number
  )
  ```
- **Process** (from `CertificateLifeCycle.test.ts`):
  1. Subject calls `createKeyringForVerifier()` with verifier's public key
  2. Function generates verifier-specific revelation keys
  3. Only fields in `fieldsToReveal` are included in returned keyring
  4. Verifier uses their private key + keyring to decrypt revealed fields
- **Implementation**: Likely uses BRC-42 key derivation (subject + verifier + serialNumber)
- **Impact**: Need to implement or use `@bsv/sdk` equivalent in Rust
- **Action**: Review BRC-53 spec or find Rust implementation of keyring generation

**4. Certificate Field Encryption** ✅ **ANSWERED**
- **Answer**: Fields are **always encrypted** using BRC-2 (AES-GCM)
- **Process** (from `CertificateLifeCycle.test.ts`):
  1. Certifier calls `MasterCertificate.createCertificateFields(certifierWallet, subjectPubKey, plaintextFields)`
  2. Returns: `{ certificateFields: encryptedFields, masterKeyring: masterKeys }`
  3. Fields are encrypted with random symmetric keys
  4. Keys are encrypted using BRC-2 (BRC-42 key derivation) so both certifier and subject can decrypt
- **Encryption Details**:
  - Uses BRC-2 encryption (AES-256-GCM)
  - Key derivation via BRC-42 (ECDH shared secret)
  - Each field has its own encryption key
  - Master keyring contains encrypted keys for subject to decrypt
- **Impact**: Fields are always base64-encoded encrypted values
- **Note**: No plaintext fields - encryption is mandatory

**5. Validation Key Purpose** ⚠️ **PARTIALLY ANSWERED**
- **Observation**: `validationKey` appears in database sync code (`StorageMySQLDojoReader.ts` line 326)
- **BRC-100**: Not in method parameters, stored as `verifier` field in database
- **Hypothesis**: `validationKey` might be the same as `verifier` field (optional)
- **Still Need**: BRC-52 spec explanation
- **Impact**: May be optional field, not required for basic certificate operations
- **Action**: Read BRC-52 spec to confirm

**6. Certificate Acquisition from BEEF** ✅ **ANSWERED**
- **Answer**: For 'direct' protocol, certificate data is provided **directly in request**
- **BRC-100 Spec**: 'direct' protocol requires all certificate fields in request body:
  - `type`, `serialNumber`, `certifier`, `revocationOutpoint`, `signature`
  - `fields` (encrypted), `keyringForSubject`, `keyringRevealer`
- **Process**:
  1. App provides certificate data directly (not from BEEF transaction)
  2. Wallet validates signature and revocation status
  3. Wallet stores certificate in database
- **For 'issuance' protocol**:
  - App provides `certifierUrl` and `fields` (plaintext)
  - Wallet requests certificate from certifier
  - Certifier creates, encrypts, and signs certificate
  - Certificate returned to wallet
- **BEEF Parsing**: Used for certificate discovery (overlay network lookups), not for `acquireCertificate`
- **Impact**: `acquireCertificate` receives JSON certificate data, not BEEF transaction
- **Note**: Certificate may have originally come from BEEF transaction, but by the time it reaches `acquireCertificate`, it's already parsed

**7. Revocation Checking Implementation** ✅ **ANSWERED**
- **Answer**: Check UTXO status via blockchain API (WhatsOnChain)
- **Current Implementation**: We already have UTXO checking in `createAction` and `signAction`
- **Recommendation**:
  - Check revocation status during `acquireCertificate` (before storing)
  - Cache revocation status in database (add `revoked` boolean field)
  - Re-check periodically (e.g., every 24 hours) or on-demand
  - If API unavailable, use cached status (may be stale)
- **Implementation**:
  - Parse `revocationOutpoint` (format: "txid.vout")
  - Query WhatsOnChain API: `/tx/{txid}/outpoint/{txid}/{vout}`
  - If UTXO is spent → certificate is revoked (reject)
  - If UTXO is unspent → certificate is active (accept)
- **Impact**: Straightforward - use existing UTXO checking infrastructure

### 📚 Required Documentation - Iterative Research Approach

**Strategy**: Research specs **during implementation** when we need specific details, rather than blocking on complete understanding upfront.

**Priority 1 - Review Before Starting** (High-Level Understanding):

1. **[BRC-48: Pay to Push Drop](https://bsv.brc.dev/scripts/0048)** ✅ **REVIEWED**
   - ✅ PushDrop script pattern understood
   - ✅ Field extraction process understood
   - ⚠️ Need to review @bsv/sdk implementation for exact parsing logic

2. **[BRC-100: Group C Methods](https://bsv.brc.dev/wallet/0100)** ✅ **REVIEWED**
   - ✅ Method parameters understood
   - ✅ Return formats understood
   - ✅ Error handling requirements understood

**Priority 2 - Research During Implementation** (Specific Details):

1. **[BRC-52: Identity Certificates](https://bsv.brc.dev/peer-to-peer/0052)** ⚠️ **RESEARCH DURING VERIFICATION**
   - ✅ Certificate structure and format (understood from code)
   - ✅ How certificates are embedded in transactions (PushDrop encoding - found!)
   - ⚠️ Signature verification process - **Research when implementing `verifier.rs`**
   - ✅ Revocation mechanism details (UTXO-based - understood)
   - ✅ Field encryption requirements (BRC-2 - understood)
   - ⚠️ `validationKey` purpose - **Research when implementing verification**

2. **[BRC-2: Data Encryption and Decryption](https://bsv.brc.dev/wallet/0002)** ⚠️ **RESEARCH DURING ENCRYPTION**
   - ✅ Complete AES-256-GCM encryption process (have test vectors)
   - ✅ BRC-42 key derivation for encryption (understood)
   - ✅ IV handling (32-byte prepended) - **CONFIRMED**
   - ✅ Test vectors for validation (have test vectors)
   - ⚠️ Specific implementation details - **Research when implementing `brc2.rs`**

3. **[BRC-53: Certificate Creation and Revelation](https://bsv.brc.dev/wallet/0053)** ⚠️ **RESEARCH DURING SELECTIVE DISCLOSURE**
   - ✅ Selective disclosure mechanism (understood from code)
   - ✅ Keyring generation for verifiers (found function signature)
   - ⚠️ How `createKeyringForVerifier()` works internally - **Research when implementing `selective_disclosure.rs`**
   - ⚠️ Revelation key derivation process - **Research when implementing `proveCertificate`**

**Priority 3 - Reference During Implementation** (Implementation Details):

4. **PushDrop Library/Implementation** ✅ **UNDERSTOOD** ([BRC-48: Pay to Push Drop](https://bsv.brc.dev/scripts/0048))
   - ✅ How PushDrop works: `<data> OP_DROP <pubkey> OP_CHECKSIG` pattern
   - ✅ How to decode: Extract fields from locking script, remove with OP_DROP
   - ✅ Certificate extraction: JSON in `decodedOutput.fields[0]`
   - ⚠️ Rust implementation needed: Need to implement BRC-48 decoder or find library
   - Reference: `reference/ts-brc100/src/utility/identityUtils.ts` lines 130-135
   - **Note**: Only needed for certificate discovery methods (Part 4), not for `acquireCertificate`

5. **MasterCertificate Implementation** (from `@bsv/sdk`)
   - `createCertificateFields()` - Field encryption process
   - `createKeyringForVerifier()` - Selective disclosure keyring generation
   - `decryptFields()` - Field decryption process
   - Need to find Rust equivalent or implement

6. **metanet-desktop Certificate Implementation**
   - Certificate parsing from BEEF transactions (PushDrop decoding)
   - Signature verification code
   - URL: https://github.com/BSVArchie/metanet-desktop

**Priority 3 - Reference** (Nice to Have):

7. **BRC-100 Spec - Group C Methods** (Already reviewed ✅)
   - Method parameter formats
   - Return value formats
   - Error handling

8. **Our Existing Code** (Already available ✅)
   - BEEF parsing (`rust-wallet/src/beef.rs`)
   - BRC-42 key derivation (`rust-wallet/src/crypto/brc42.rs`)
   - UTXO checking (WhatsOnChain API integration)

### 📋 Next Steps

1. **Read BRC-52 Spec Completely** (2-3 hours)
   - Understand certificate structure in detail
   - Understand signature verification process
   - Understand how certificates are embedded in transactions

2. **Read BRC-2 Spec Completely** (1-2 hours)
   - Understand AES-GCM encryption process
   - Review test vectors for validation
   - Understand BRC-42 key derivation for encryption

3. **Read BRC-53 Spec** (1 hour)
   - Understand certificate creation flow
   - Understand selective disclosure mechanism
   - Understand `createKeyringForVerifier()` process

4. **Review metanet-desktop Implementation** (2-3 hours)
   - Look for certificate parsing from BEEF transactions
   - Look for signature verification code
   - Look for selective disclosure implementation

5. **Update Implementation Plan** (1 hour)
   - Refine based on research findings
   - Update database schema recommendations
   - Document exact implementation steps

---

---

## 📋 Pre-Implementation Checklist

### ✅ Completed Research

- [x] BRC-100 method specifications (Call Codes 17-20)
- [x] TypeScript implementation analysis (ts-brc100)
- [x] Database schema comparison and migration plan
- [x] Method parameter and return format documentation

### ❓ Critical Questions (Must Answer Before Implementation)

1. **Certificate Parsing**: How is certificate embedded in BEEF transactions?
2. **Signature Verification**: Exact field order and encoding for signature?
3. **Selective Disclosure**: How does keyring generation work? (BRC-42 derivation?)
4. **Field Encryption**: Are fields always encrypted or optional?
5. **Validation Key**: What is `validationKey` used for? (required/optional?)
6. **Acquisition Format**: How does `acquireCertificate` receive certificate data?
7. **Revocation Caching**: Should we cache revocation status?

### 📚 Required Documentation (Priority Order)

**🔴 Priority 1 - Blocking Implementation**:
1. [BRC-52: Identity Certificates](https://bsv.brc.dev/peer-to-peer/0052) - Certificate structure, signature verification, transaction embedding
2. [BRC-2: Data Encryption and Decryption](https://bsv.brc.dev/wallet/0002) - Complete encryption/decryption process
3. [BRC-53: Certificate Creation and Revelation](https://bsv.brc.dev/wallet/0053) - Selective disclosure mechanism

**🟡 Priority 2 - Implementation Details**:
4. metanet-desktop certificate implementation - BEEF parsing, signature verification, selective disclosure
5. ts-brc100 `MasterCertificate.createKeyringForVerifier()` - Keyring generation source code

**🟢 Priority 3 - Reference**:
6. BRC-100 Group C methods (Already reviewed ✅)
7. Our existing BEEF parsing and BRC-42 code (Already available ✅)

### 🗄️ Database Migration Plan

**Decision**: Migrate to separate `certificate_fields` table

**New Schema**:
- `certificates` table: Certificate metadata (type, subject, certifier, signature, etc.)
- `certificate_fields` table: One row per field (fieldName, fieldValue, masterKey)
- Benefits: Better querying, efficient selective disclosure, matches TypeScript reference

**Migration Steps**:
1. Create `certificate_fields` table (migration v6)
2. Update `certificates` table with missing fields
3. Migrate existing data (if any) from JSON `attributes` to separate table
4. Update certificate repository to use new schema

---

---

## 🔍 Research Findings Summary (2025-12-08)

### ✅ Questions Answered

1. **Certificate Parsing**: ✅ Certificates embedded in PushDrop-encoded output scripts
2. **Field Encryption**: ✅ Always encrypted using BRC-2 (AES-GCM), mandatory
3. **Acquisition Format**: ✅ 'direct' protocol receives JSON certificate data directly
4. **Revocation Checking**: ✅ Check UTXO status via WhatsOnChain API (existing infrastructure)

### ⚠️ Questions Partially Answered

5. **Signature Verification**: Need exact field order and encoding from BRC-52 spec
6. **Keyring Generation**: Found function signature, need internal implementation details
7. **Validation Key**: Appears optional, need BRC-52 spec confirmation

### 🆕 New Discoveries

- **PushDrop Encoding**: Certificates use PushDrop encoding in output scripts (not OP_RETURN)
- **MasterCertificate Class**: Key class from `@bsv/sdk` for certificate operations
- **Certificate Lifecycle**: Clear flow from creation → encryption → signing → storage → selective disclosure

### 📋 Remaining Documentation Needs

**Critical** (Blocking Implementation):
1. BRC-52 spec - Signature verification exact process
2. BRC-53 spec - Keyring generation internal details
3. PushDrop library - Rust implementation or decoding logic

**Helpful** (Implementation Details):
4. MasterCertificate Rust equivalent or implementation guide
5. Certificate signature verification test vectors

---

---

## 📊 Research Progress Summary

### ✅ Completed (70%)

1. **BRC-100 Method Specifications** - ✅ Complete
2. **TypeScript Implementation Analysis** - ✅ Complete
3. **Database Schema Design** - ✅ Complete (migration plan ready)
4. **Certificate Parsing Method** - ✅ Found (PushDrop encoding)
5. **Field Encryption Process** - ✅ Understood (BRC-2, always encrypted)
6. **Acquisition Protocol** - ✅ Understood ('direct' vs 'issuance')
7. **Revocation Mechanism** - ✅ Understood (UTXO-based checking)
8. **Selective Disclosure Flow** - ✅ Understood (keyring generation)

### ⚠️ Research During Implementation (30%)

**Strategy**: Research these items **during implementation** rather than blocking on them now. This allows us to:
- Start implementation with current understanding
- Research specific details when we encounter them
- Test and iterate as we learn
- Avoid over-researching before we know what we need

9. **Signature Verification** - ⚠️ Research during `verifier.rs` implementation
   - Will read BRC-52 spec when implementing signature verification
   - Will test with real certificates to understand exact field order
   - Can reference TypeScript `VerifiableCertificate.verify()` implementation

10. **Keyring Generation** - ⚠️ Research during `selective_disclosure.rs` implementation
   - Will read BRC-53 spec when implementing `proveCertificate`
   - Will reference TypeScript `MasterCertificate.createKeyringForVerifier()` implementation
   - Can test with real keyring examples

11. **Validation Key** - ⚠️ Research during certificate verification implementation
   - Will check BRC-52 spec when implementing verification
   - Can test with/without validation key to understand optionality

12. **BRC-2 Encryption Details** - ⚠️ Research during `brc2.rs` implementation
   - Will read BRC-2 spec when implementing encryption/decryption
   - Will test with real encrypted certificate fields
   - Can reference TypeScript BRC-2 implementation

**Benefits of Iterative Research**:
- ✅ Start implementation immediately
- ✅ Research only what we need, when we need it
- ✅ Test-driven research (learn from real examples)
- ✅ Avoid analysis paralysis
- ✅ Faster time to working code

---

## 📚 PushDrop Implementation Analysis (@bsv/sdk Review)

### ✅ PushDrop.decode() Implementation Analysis

**Source**: `reference/ts-brc100/node_modules/@bsv/sdk/src/script/templates/PushDrop.ts`

**Key Function**: `static decode(script: LockingScript)`

**Algorithm**:
```typescript
static decode(script: LockingScript): {
  lockingPublicKey: PublicKey
  fields: number[][]
} {
  // 1. First chunk is the public key (33 bytes)
  const lockingPublicKey = PublicKey.fromString(
    Utils.toHex(script.chunks[0].data)
  )

  // 2. Fields start at index 2 (skip pubkey at 0, OP_CHECKSIG at 1)
  const fields: number[][] = []
  for (let i = 2; i < script.chunks.length; i++) {
    const nextOpcode = script.chunks[i + 1]?.op
    let chunk: number[] = script.chunks[i].data ?? []

    // 3. Handle special opcodes that push values directly
    if (chunk.length === 0) {
      if (script.chunks[i].op >= 80 && script.chunks[i].op <= 95) {
        // OP_1 through OP_16 (0x51-0x60)
        chunk = [script.chunks[i].op - 80]
      } else if (script.chunks[i].op === 0) {
        // OP_0
        chunk = [0]
      } else if (script.chunks[i].op === 0x4f) {
        // OP_1NEGATE
        chunk = [0x81]
      }
    }
    fields.push(chunk)

    // 4. Stop when we hit OP_DROP (0x75) or OP_2DROP (0x6d)
    if (nextOpcode === OP.OP_DROP || nextOpcode === OP.OP_2DROP) {
      break
    }
  }

  return { fields, lockingPublicKey }
}
```

**Script Structure** (for `lockPosition === 'before'`):
```
[pubkey (33 bytes), OP_CHECKSIG (0xac), field1, field2, ..., OP_DROP/OP_2DROP]
```

**Script Structure** (for `lockPosition === 'after'`):
```
[field1, field2, ..., OP_DROP/OP_2DROP, pubkey (33 bytes), OP_CHECKSIG (0xac)]
```

**Key Insights**:
1. **Public Key Location**: First chunk (index 0) for 'before', last chunk for 'after'
2. **OP_CHECKSIG**: Second chunk (index 1) for 'before', second-to-last for 'after'
3. **Fields Extraction**: Start at index 2, continue until OP_DROP/OP_2DROP
4. **Special Opcodes**: OP_0 (0x00), OP_1-OP_16 (0x51-0x60), OP_1NEGATE (0x4f) push values directly
5. **Data Pushes**: OP_PUSHDATA1 (0x4c), OP_PUSHDATA2 (0x4d), OP_PUSHDATA4 (0x4e) for larger data

**For Certificates**:
- Certificate JSON is in `decodedOutput.fields[0]` (first field)
- Convert from `number[]` to UTF-8: `Utils.toUTF8(decodedOutput.fields[0])`
- Parse JSON: `JSON.parse(Utils.toUTF8(decodedOutput.fields[0]))`

### ✅ PushDrop.lock() Implementation Analysis

**Key Function**: `async lock(fields, protocolID, keyID, counterparty, forSelf, includeSignature, lockPosition)`

**Algorithm**:
1. Get public key from wallet
2. Create lock chunks: `[pubkey, OP_CHECKSIG]`
3. Optionally add signature to fields (if `includeSignature === true`)
4. Encode each field using `createMinimallyEncodedScriptChunk()`
5. Add OP_2DROP for pairs of fields, OP_DROP for remaining field
6. Combine based on `lockPosition` ('before' or 'after')

**Minimal Encoding** (`createMinimallyEncodedScriptChunk`):
- Empty or `[0]` → OP_0 (0x00)
- `[1-16]` → OP_1 through OP_16 (0x51-0x60)
- `[0x81]` → OP_1NEGATE (0x4f)
- `length <= 75` → Direct push (opcode = length)
- `length <= 255` → OP_PUSHDATA1 (0x4c) + length byte + data
- `length <= 65535` → OP_PUSHDATA2 (0x4d) + length (2 bytes) + data
- `length > 65535` → OP_PUSHDATA4 (0x4e) + length (4 bytes) + data

### Rust Implementation Plan

**File**: `rust-wallet/src/script/pushdrop.rs`

**Functions Needed**:
1. `pub fn decode(script: &[u8]) -> Result<PushDropDecoded>`
   - Parse script chunks
   - Extract public key (first or last chunk depending on position)
   - Extract fields until OP_DROP/OP_2DROP
   - Handle special opcodes (OP_0, OP_1-OP_16, OP_1NEGATE)

2. `pub fn encode(fields: &[Vec<u8>], public_key: &[u8], lock_position: LockPosition) -> Result<Vec<u8>>`
   - Encode fields using minimal encoding
   - Add OP_DROP/OP_2DROP appropriately
   - Combine with pubkey + OP_CHECKSIG

3. `fn parse_script_chunks(script: &[u8]) -> Result<Vec<ScriptChunk>>`
   - Parse script into chunks
   - Handle OP_PUSHDATA1/2/4 for variable-length data

4. `fn create_minimally_encoded_chunk(data: &[u8]) -> Vec<u8>`
   - Implement minimal encoding logic
   - Handle OP_0, OP_1-OP_16, OP_1NEGATE
   - Handle OP_PUSHDATA1/2/4

**Data Structures**:
```rust
pub struct PushDropDecoded {
    pub locking_public_key: Vec<u8>,  // 33 bytes
    pub fields: Vec<Vec<u8>>,
}

pub enum LockPosition {
    Before,  // [pubkey, OP_CHECKSIG, fields..., OP_DROP]
    After,   // [fields..., OP_DROP, pubkey, OP_CHECKSIG]
}

struct ScriptChunk {
    op: u8,
    data: Option<Vec<u8>>,
}
```

**Dependencies**:
- Need Bitcoin script parser (or implement basic chunk parsing)
- May use existing Rust Bitcoin libraries if available

---

## 📚 Additional Research: Bitcoin Script Standards (BRC-14, BRC-21, BRC-47, BRC-48, BRC-106)

### Summary of Reviewed Documents

**Documents Reviewed**:
1. [BRC-14: Pay to Public Key Hash (P2PKH)](https://bsv.brc.dev/scripts/0014)
2. [BRC-21: Push TX](https://bsv.brc.dev/scripts/0021)
3. [BRC-47: Bare Multi-Signature](https://bsv.brc.dev/scripts/0047)
4. [BRC-48: Pay to Push Drop](https://bsv.brc.dev/scripts/0048) ✅ **CRITICAL FOR CERTIFICATES**
5. [BRC-106: Bitcoin Script ASM Format](https://bsv.brc.dev/scripts/0106)

### Key Findings

#### 1. **BRC-48: Pay to Push Drop** ✅ **MOST RELEVANT**

**What is PushDrop?**:
- **PushDrop** is a Bitcoin script pattern (not a separate protocol)
- Allows embedding arbitrary data in transaction outputs while maintaining spendability
- Standardized in BRC-48 specification

**How PushDrop Works**:
```
<arbitrary_data> OP_DROP <public_key> OP_CHECKSIG
```

**Process**:
1. Data is pushed onto the stack (e.g., certificate JSON)
2. `OP_DROP` removes the data from stack
3. Standard locking script continues (e.g., `OP_CHECKSIG`)
4. Data is embedded but doesn't affect spendability
5. Multiple fields can be embedded: `<field1> <field2> ... <fieldN> OP_DROP <pubkey> OP_CHECKSIG`

**For Certificates**:
- Certificates are stored as JSON in the first PushDrop field
- Pattern: `<certificate_json> OP_DROP <certifier_pubkey> OP_CHECKSIG`
- Allows certificate to be embedded while output remains spendable
- Used in `identityUtils.ts` for certificate discovery from blockchain

**Rust Implementation Needs**:
- Need to decode PushDrop-encoded locking scripts
- Extract fields from script (before `OP_DROP`)
- Convert first field from bytes to UTF-8 JSON string
- Parse JSON to certificate structure

#### 2. **BRC-106: Bitcoin Script ASM Format**

**What is ASM?**:
- **ASM (Assembly)** is a human-readable representation of Bitcoin Script
- Translates hexadecimal bytecode into opcodes and data pushes
- Example: `OP_DUP OP_HASH160 <hash> OP_EQUALVERIFY OP_CHECKSIG`

**Why It Matters**:
- Useful for debugging and analyzing scripts
- Helps understand PushDrop script structure
- Can be used to verify certificate embedding format

**Example P2PKH in ASM**:
```
OP_DUP OP_HASH160 6e751b60fcb566418c6b9f68bfa51438aefbe094 OP_EQUALVERIFY OP_CHECKSIG
```

#### 3. **BRC-14: Pay to Public Key Hash (P2PKH)**

**What is P2PKH?**:
- Most common Bitcoin script type
- Locks funds to hash of public key
- Requires signature and public key to spend

**Relevance to Certificates**:
- Certificates may use P2PKH for the locking portion after PushDrop
- Understanding P2PKH helps understand certificate output structure

#### 4. **BRC-21: Push TX**

**What is Push TX?**:
- Technique to enforce transaction conditions within Bitcoin script
- Allows inspection of entire transaction within contract
- Uses ECDSA signature messages

**Relevance to Certificates**:
- Less directly relevant, but shows advanced Bitcoin script capabilities
- May be used in future certificate verification schemes

#### 5. **BRC-47: Bare Multi-Signature**

**What is Bare Multi-Sig?**:
- Multi-signature script using `OP_CHECKMULTISIG` directly
- Requires M-of-N signatures to unlock funds
- Structure: `<M> <pubkey1> ... <pubkeyN> <N> OP_CHECKMULTISIG`

**Relevance to Certificates**:
- Less directly relevant for basic certificate implementation
- May be used for multi-party certificate issuance in future

### Impact on Certificate Implementation

**✅ Clarified**:
1. **PushDrop is a script pattern** (BRC-48), not a separate protocol
2. **Certificate embedding format**: `<cert_json> OP_DROP <pubkey> OP_CHECKSIG`
3. **ASM format** helps understand and debug script structures
4. **Multiple fields supported**: PushDrop can embed multiple data fields

**⚠️ Still Need**:
1. **Rust PushDrop decoder**: Implementation or library for decoding BRC-48 scripts
2. **Field extraction logic**: How to parse multiple PushDrop fields
3. **Script parsing library**: Need Bitcoin script parser in Rust

**📝 Next Steps**:
- Search for existing Rust PushDrop/Bitcoin script libraries
- Review `@bsv/sdk` PushDrop implementation for decoding logic
- Consider implementing BRC-48 decoder if no library exists

---

**Last Updated**: 2025-12-08
**Status**: 🔬 Research Phase - 70% Complete
**Next Steps**:
1. Read BRC-52 spec for signature verification details (exact field order)
2. Read BRC-53 spec for keyring generation details (BRC-42 derivation)
3. Find or implement PushDrop decoder in Rust (for certificate discovery)
4. Begin implementation with Phase 2 (database migration) - Can start now!
