#!/usr/bin/env node
// Test the exact certificate acquisition flow that our Rust wallet performs
// This simulates what our Rust code does and verifies each step with the SDK
//
// Run: node test_cert_flow.mjs

import { createRequire } from 'module';
const require = createRequire(import.meta.url);
const sdkPath = '../reference/ts-brc100/node_modules/@bsv/sdk';
const { PrivateKey, SymmetricKey, Utils, Hash, Random } = require(sdkPath);
const { KeyDeriver } = require(sdkPath + '/dist/cjs/src/wallet/KeyDeriver.js');
const { AESGCM, AESGCMDecrypt } = require(sdkPath + '/dist/cjs/src/primitives/AESGCM.js');

// Test keys (same as our Rust integration tests)
const clientPrivKeyHex = 'e8f32e723decf4051aefac8e2c93c9c5b214313817cdb01a1494b917c8436b35';
const serverPrivKeyHex = 'c9afa9d845ba75166b5c215767b1d6934e50c3db36e89b127b8a622b120f6721';

const clientPrivKey = PrivateKey.fromString(clientPrivKeyHex, 16);
const serverPrivKey = PrivateKey.fromString(serverPrivKeyHex, 16);
const clientPubKey = clientPrivKey.toPublicKey();
const serverPubKey = serverPrivKey.toPublicKey();

console.log(`Client pubkey: ${clientPubKey.toString()}`);
console.log(`Server pubkey: ${serverPubKey.toString()}`);

const clientDeriver = new KeyDeriver(clientPrivKey);
const serverDeriver = new KeyDeriver(serverPrivKey);

// ============================================================
// TEST 1: Nonce creation and verification
// Simulates what our Rust create_nonce_with_hmac() does
// ============================================================
console.log("\n=== TEST 1: Nonce creation (client) + verification (server) ===");

// Use known "random" bytes so we can compare with Rust
const firstHalf = [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08,
                    0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F, 0x10];

// Step 1: keyID = Utils.toUTF8(firstHalf)
const keyID = Utils.toUTF8(firstHalf);
console.log(`keyID string: ${JSON.stringify(keyID)} (${keyID.length} chars)`);
console.log(`keyID codePoints: [${[...keyID].map(c => '0x' + c.codePointAt(0).toString(16)).join(', ')}]`);
// Convert keyID to UTF-8 bytes (this is what goes into the invoice number)
const keyIDBytes = Utils.toArray(keyID, 'utf8');
console.log(`keyID UTF-8 bytes: [${keyIDBytes.map(b => b.toString(16).padStart(2, '0')).join(' ')}]`);

// Step 2: Derive symmetric key using BRC-42
// Client creates nonce with server as counterparty
const clientSymKey = clientDeriver.deriveSymmetricKey(
  [2, 'server hmac'],
  keyID,
  serverPubKey
);

const symKey32 = clientSymKey.toArray('be', 32);
const symKeyStripped = clientSymKey.toArray();
console.log(`\nClient symmetric key (32-byte): ${symKey32.map(b => b.toString(16).padStart(2, '0')).join('')} (${symKey32.length} bytes)`);
console.log(`Client symmetric key (stripped): ${symKeyStripped.map(b => b.toString(16).padStart(2, '0')).join('')} (${symKeyStripped.length} bytes)`);

// Step 3: HMAC with STRIPPED key (matching SDK's createHmac: key.toArray())
const hmac = Hash.sha256hmac(symKeyStripped, firstHalf);
console.log(`HMAC (stripped key): ${hmac.map(b => b.toString(16).padStart(2, '0')).join('')}`);

// Step 3b: HMAC with FULL 32-byte key (what our Rust code does when NOT stripping)
const hmac32 = Hash.sha256hmac(symKey32, firstHalf);
console.log(`HMAC (full 32-byte key): ${hmac32.map(b => b.toString(16).padStart(2, '0')).join('')}`);

if (hmac.toString() === hmac32.toString()) {
  console.log(`HMACs match (key has no leading zeros) ✅`);
} else {
  console.log(`HMACs DIFFER! Key has leading zeros - stripping matters! ⚠️`);
}

// Step 4: Build nonce
const nonceBytes = [...firstHalf, ...hmac];
const nonceBase64 = Utils.toBase64(nonceBytes);
console.log(`\nNonce (base64): ${nonceBase64}`);

// Step 5: Server verifies the nonce
console.log("\n--- Server verification ---");
const decodedNonce = Utils.toArray(nonceBase64, 'base64');
const data = decodedNonce.slice(0, 16);
const providedHmac = decodedNonce.slice(16);
const serverKeyID = Utils.toUTF8(data);

// Server derives symmetric key with CLIENT as counterparty
const serverSymKey = serverDeriver.deriveSymmetricKey(
  [2, 'server hmac'],
  serverKeyID,
  clientPubKey
);

const serverSymKeyStripped = serverSymKey.toArray();
console.log(`Server symmetric key (stripped): ${serverSymKeyStripped.map(b => b.toString(16).padStart(2, '0')).join('')} (${serverSymKeyStripped.length} bytes)`);

const expectedHmac = Hash.sha256hmac(serverSymKeyStripped, data);
console.log(`Server expected HMAC: ${expectedHmac.map(b => b.toString(16).padStart(2, '0')).join('')}`);
console.log(`Client provided HMAC: ${providedHmac.map(b => b.toString(16).padStart(2, '0')).join('')}`);

const match = expectedHmac.toString() === providedHmac.toString();
console.log(`Nonce verification: ${match ? '✅ PASS' : '❌ FAIL'}`);

// ============================================================
// TEST 2: Random nonce verification (stress test)
// ============================================================
console.log("\n=== TEST 2: Random nonce stress test (100 attempts) ===");
let noncePass = 0, nonceFail = 0;
for (let i = 0; i < 100; i++) {
  const rFirstHalf = Random(16);
  const rKeyID = Utils.toUTF8(rFirstHalf);

  // Client creates
  const rClientKey = clientDeriver.deriveSymmetricKey([2, 'server hmac'], rKeyID, serverPubKey);
  const rHmac = Hash.sha256hmac(rClientKey.toArray(), rFirstHalf);

  // Server verifies
  const rServerKey = serverDeriver.deriveSymmetricKey([2, 'server hmac'], rKeyID, clientPubKey);
  const rExpected = Hash.sha256hmac(rServerKey.toArray(), rFirstHalf);

  if (rExpected.toString() === rHmac.toString()) {
    noncePass++;
  } else {
    nonceFail++;
    if (nonceFail <= 3) {
      console.log(`  FAIL #${nonceFail}: firstHalf=[${rFirstHalf.map(b=>b.toString(16).padStart(2,'0')).join(' ')}]`);
      console.log(`    clientKey: ${rClientKey.toArray().map(b=>b.toString(16).padStart(2,'0')).join('')}`);
      console.log(`    serverKey: ${rServerKey.toArray().map(b=>b.toString(16).padStart(2,'0')).join('')}`);
    }
  }
}
console.log(`Results: ${noncePass} pass, ${nonceFail} fail`);

// ============================================================
// TEST 3: Field encryption (client) + decryption (server)
// Simulates MasterCertificate.createCertificateFields + decryptFields
// ============================================================
console.log("\n=== TEST 3: Field encryption/decryption ===");

const fieldName = "cool";
const fieldValue = "true";
const fieldValueBytes = Utils.toArray(fieldValue, 'utf8');
console.log(`Field '${fieldName}' = ${fieldValue} (${fieldValueBytes.length} bytes)`);

// Step 1: Generate random symmetric key for field
const fieldKey = SymmetricKey.fromRandom();
const fieldKey32 = fieldKey.toArray('be', 32);
const fieldKeyStripped = fieldKey.toArray();
console.log(`\nField symmetric key (32-byte): ${fieldKey32.map(b=>b.toString(16).padStart(2,'0')).join('')} (${fieldKey32.length} bytes)`);
console.log(`Field symmetric key (stripped): ${fieldKeyStripped.map(b=>b.toString(16).padStart(2,'0')).join('')} (${fieldKeyStripped.length} bytes)`);

// Step 2: Encrypt field value with the 32-byte key (like our Rust code does)
// Our Rust: encrypt_brc2(field_value_bytes, &field_symmetric_key) - uses full 32 bytes
const iv = Random(32);
const { result: cipher, authenticationTag } = AESGCM(fieldValueBytes, [], iv, fieldKey32);
const encryptedField = [...iv, ...cipher, ...authenticationTag];
console.log(`Encrypted field: ${encryptedField.length} bytes`);

// Step 3: Server decrypts with SDK's SymmetricKey.decrypt (uses stripped key internally)
try {
  const serverFieldKey = new SymmetricKey(fieldKeyStripped);
  const decryptedField = serverFieldKey.decrypt(encryptedField);
  const decryptedStr = Utils.toUTF8(decryptedField);
  console.log(`Server decrypt (stripped key): ${decryptedStr} ${decryptedStr === fieldValue ? '✅' : '❌'}`);
} catch (e) {
  console.log(`Server decrypt FAILED: ${e.message} ❌`);
}

// Step 4: Also test with SymmetricKey.encrypt (which uses 32-byte padded key)
const sdkEncrypted = fieldKey.encrypt(fieldValueBytes);
try {
  const sdkDecrypted = fieldKey.decrypt(sdkEncrypted);
  const sdkStr = Utils.toUTF8(sdkDecrypted);
  console.log(`SDK roundtrip: ${sdkStr} ${sdkStr === fieldValue ? '✅' : '❌'}`);
} catch (e) {
  console.log(`SDK roundtrip FAILED: ${e.message} ❌`);
}

// ============================================================
// TEST 4: BRC-2 revelation key encryption/decryption
// Client encrypts revelation key FOR the server using BRC-42
// Server decrypts with its own key
// ============================================================
console.log("\n=== TEST 4: BRC-2 revelation key encryption ===");

// Client encrypts revelation key for server
// Invoice: "2-certificate field encryption-cool"
const clientRevelationKey = clientDeriver.deriveSymmetricKey(
  [2, 'certificate field encryption'],
  fieldName,
  serverPubKey
);

console.log(`Client BRC-2 key (32-byte): ${clientRevelationKey.toArray('be', 32).map(b=>b.toString(16).padStart(2,'0')).join('')}`);
console.log(`Client BRC-2 key (stripped): ${clientRevelationKey.toArray().map(b=>b.toString(16).padStart(2,'0')).join('')}`);

// Encrypt the stripped field key bytes with BRC-2
// Our Rust: encrypt_brc2(revelation_key_bytes, &brc2_symmetric_key)
// But deriveSymmetricKey returns SymmetricKey, and SDK uses encrypt() method
// The SDK's Wallet.encrypt() does: key.encrypt(plaintext)
// key.encrypt() uses this.toArray('be', 32) for encryption

// Simulate what our Rust code does:
// 1. Derive symmetric key via BRC-42 → 32-byte x-coordinate
// 2. Encrypt with encrypt_brc2 using the full 32-byte key
const brc2Key32 = clientRevelationKey.toArray('be', 32);
const revKeyIV = Random(32);
const { result: revCipher, authenticationTag: revTag } = AESGCM(fieldKeyStripped, [], revKeyIV, brc2Key32);
const encryptedRevKey = [...revKeyIV, ...revCipher, ...revTag];
console.log(`Encrypted revelation key: ${encryptedRevKey.length} bytes`);

// Server decrypts
const serverRevelationKey = serverDeriver.deriveSymmetricKey(
  [2, 'certificate field encryption'],
  fieldName,
  clientPubKey
);

console.log(`Server BRC-2 key (32-byte): ${serverRevelationKey.toArray('be', 32).map(b=>b.toString(16).padStart(2,'0')).join('')}`);
console.log(`Server BRC-2 key (stripped): ${serverRevelationKey.toArray().map(b=>b.toString(16).padStart(2,'0')).join('')}`);

const keysMatch = clientRevelationKey.toArray('be', 32).toString() === serverRevelationKey.toArray('be', 32).toString();
console.log(`BRC-2 keys match (ECDH symmetry): ${keysMatch ? '✅' : '❌'}`);

// Server tries to decrypt using SDK's SymmetricKey.decrypt (uses stripped key)
try {
  const decryptedRevKey = serverRevelationKey.decrypt(encryptedRevKey);
  console.log(`Server decrypted revelation key: [${decryptedRevKey.map(b=>b.toString(16).padStart(2,'0')).join(' ')}] (${decryptedRevKey.length} bytes)`);
  console.log(`Original (stripped): [${fieldKeyStripped.map(b=>b.toString(16).padStart(2,'0')).join(' ')}] (${fieldKeyStripped.length} bytes)`);
  const revMatch = decryptedRevKey.toString() === fieldKeyStripped.toString();
  console.log(`Revelation key match: ${revMatch ? '✅' : '❌'}`);

  if (revMatch) {
    // Now decrypt the field value using the decrypted revelation key
    const recoveredFieldKey = new SymmetricKey(decryptedRevKey);
    try {
      const recoveredField = recoveredFieldKey.decrypt(encryptedField);
      const recoveredStr = Utils.toUTF8(recoveredField);
      console.log(`End-to-end field decrypt: ${recoveredStr} ${recoveredStr === fieldValue ? '✅' : '❌'}`);
    } catch (e) {
      console.log(`End-to-end field decrypt FAILED: ${e.message} ❌`);
    }
  }
} catch (e) {
  console.log(`Server revelation key decrypt FAILED: ${e.message} ❌`);
}

// ============================================================
// TEST 5: Verify AES key padding behavior
// ============================================================
console.log("\n=== TEST 5: AES key padding verification ===");

// Create a key with a leading zero
const keyWith0 = [0x00, ...Random(31)];
const keyWithout0 = keyWith0.slice(1);
console.log(`Key with zero:    [${keyWith0.slice(0,4).map(b=>b.toString(16).padStart(2,'0')).join(' ')}...] (${keyWith0.length} bytes)`);
console.log(`Key without zero: [${keyWithout0.slice(0,4).map(b=>b.toString(16).padStart(2,'0')).join(' ')}...] (${keyWithout0.length} bytes)`);

const testPlain = [0x74, 0x72, 0x75, 0x65]; // "true"
const testIV = Random(32);
const { result: testCipher, authenticationTag: testTag } = AESGCM(testPlain, [], testIV, keyWith0);

// Try decrypt with stripped key (31 bytes)
const dec31 = AESGCMDecrypt(testCipher, [], testIV, testTag, keyWithout0);
console.log(`Encrypt with 32-byte key, decrypt with 31-byte key: ${dec31 === null ? '❌ FAILED' : '✅ OK'}`);

// Try decrypt with full key (32 bytes)
const dec32 = AESGCMDecrypt(testCipher, [], testIV, testTag, keyWith0);
console.log(`Encrypt with 32-byte key, decrypt with 32-byte key: ${dec32 === null ? '❌ FAILED' : '✅ OK'}`);

// Test via SymmetricKey class
const skWith0 = new SymmetricKey(keyWith0);
console.log(`SymmetricKey.toArray('be',32): [${skWith0.toArray('be',32).slice(0,4).map(b=>b.toString(16).padStart(2,'0')).join(' ')}...] (${skWith0.toArray('be',32).length} bytes)`);
console.log(`SymmetricKey.toArray():        [${skWith0.toArray().slice(0,4).map(b=>b.toString(16).padStart(2,'0')).join(' ')}...] (${skWith0.toArray().length} bytes)`);

const skEncrypted = skWith0.encrypt(testPlain);
try {
  const skDecrypted = skWith0.decrypt(skEncrypted);
  console.log(`SymmetricKey roundtrip: ${JSON.stringify(skDecrypted) === JSON.stringify(testPlain) ? '✅' : '❌'}`);
} catch (e) {
  console.log(`SymmetricKey roundtrip FAILED: ${e.message} ❌`);
}

console.log("\n=== Done ===");
