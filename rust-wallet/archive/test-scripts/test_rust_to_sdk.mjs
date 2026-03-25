#!/usr/bin/env node
// Test: Can the SDK decrypt what our Rust AES-GCM produces?
// We use known key/IV/plaintext, encrypt with our Rust-equivalent logic, then SDK decrypts.

import { createRequire } from 'module';
const require = createRequire(import.meta.url);
const sdkPath = '../reference/ts-brc100/node_modules/@bsv/sdk';
const { PrivateKey, Utils, SymmetricKey } = require(sdkPath);
const { KeyDeriver } = require(sdkPath + '/dist/cjs/src/wallet/KeyDeriver.js');
const { AESGCM, AESGCMDecrypt } = require(sdkPath + '/dist/cjs/src/primitives/AESGCM.js');

// ======== Test 1: Direct AES-GCM ========
console.log("=== Test 1: Direct AES-GCM (32-byte IV) ===");

// Known values
const key = new Uint8Array(32);
for (let i = 0; i < 32; i++) key[i] = 0x42; // All 0x42
const iv = new Uint8Array(32);
for (let i = 0; i < 32; i++) iv[i] = i;
const plaintext = Utils.toArray("Hello, BRC-2!", 'utf8');

// Encrypt with SDK (simulating what our Rust does)
const { result: ciphertext, authenticationTag } = AESGCM(plaintext, [], Array.from(iv), Array.from(key));
const encrypted = [...iv, ...ciphertext, ...authenticationTag];
console.log(`Encrypted: ${encrypted.length} bytes`);
console.log(`IV: ${Array.from(iv).map(b=>b.toString(16).padStart(2,'0')).join('')}`);
console.log(`Ciphertext: ${ciphertext.map(b=>b.toString(16).padStart(2,'0')).join('')}`);
console.log(`Tag: ${authenticationTag.map(b=>b.toString(16).padStart(2,'0')).join('')}`);

// Decrypt with SDK's SymmetricKey.decrypt (which uses stripped key)
const symKey = new SymmetricKey(Array.from(key));
try {
  const decrypted = symKey.decrypt(encrypted);
  const decryptedStr = Utils.toUTF8(decrypted);
  console.log(`Decrypted: ${decryptedStr} ${decryptedStr === "Hello, BRC-2!" ? '✅' : '❌'}`);
} catch (e) {
  console.log(`Decrypt FAILED: ${e.message} ❌`);
}

// ======== Test 2: BRC-2 full flow (encrypt → decrypt with ECDH key) ========
console.log("\n=== Test 2: BRC-2 full flow (client encrypt → server decrypt) ===");

const clientPrivHex = 'e8f32e723decf4051aefac8e2c93c9c5b214313817cdb01a1494b917c8436b35';
const serverPrivHex = 'c9afa9d845ba75166b5c215767b1d6934e50c3db36e89b127b8a622b120f6721';

const clientPriv = PrivateKey.fromString(clientPrivHex, 16);
const serverPriv = PrivateKey.fromString(serverPrivHex, 16);
const clientPub = clientPriv.toPublicKey();
const serverPub = serverPriv.toPublicKey();
const clientDeriver = new KeyDeriver(clientPriv);
const serverDeriver = new KeyDeriver(serverPriv);

// Client encrypts a revelation key for the server
const fieldName = "cool";
const revelationKeyBytes = [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08,
                            0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f, 0x10,
                            0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18,
                            0x19, 0x1a, 0x1b, 0x1c, 0x1d, 0x1e, 0x1f, 0x20];

// Client: derive symmetric key
const clientSymKey = clientDeriver.deriveSymmetricKey(
  [2, 'certificate field encryption'],
  fieldName,
  serverPub
);

const clientKey32 = clientSymKey.toArray('be', 32);
console.log(`Client sym key (32-byte): ${clientKey32.map(b=>b.toString(16).padStart(2,'0')).join('')}`);

// Client: encrypt with full 32-byte key (matching our Rust encrypt_brc2)
const encIV = Array.from({length: 32}, (_, i) => (i * 7 + 3) % 256); // deterministic IV for testing
const { result: encCT, authenticationTag: encTag } = AESGCM(revelationKeyBytes, [], encIV, clientKey32);
const encryptedRevKey = [...encIV, ...encCT, ...encTag];
console.log(`Encrypted revelation key: ${encryptedRevKey.length} bytes`);

// Server: derive symmetric key (should match due to ECDH symmetry)
const serverSymKey = serverDeriver.deriveSymmetricKey(
  [2, 'certificate field encryption'],
  fieldName,
  clientPub
);

const serverKey32 = serverSymKey.toArray('be', 32);
const serverKeyStripped = serverSymKey.toArray();
console.log(`Server sym key (32-byte): ${serverKey32.map(b=>b.toString(16).padStart(2,'0')).join('')}`);
console.log(`Server sym key (stripped, ${serverKeyStripped.length} bytes): ${serverKeyStripped.map(b=>b.toString(16).padStart(2,'0')).join('')}`);
console.log(`Keys match: ${clientKey32.every((b,i) => b === serverKey32[i]) ? '✅' : '❌'}`);

// Server: decrypt using SymmetricKey.decrypt (uses stripped key internally)
try {
  const decryptedRevKey = serverSymKey.decrypt(encryptedRevKey);
  const match = decryptedRevKey.length === revelationKeyBytes.length &&
    decryptedRevKey.every((b, i) => b === revelationKeyBytes[i]);
  console.log(`Server decrypted revelation key: ${match ? '✅ matches' : '❌ mismatch'}`);
  if (!match) {
    console.log(`  Expected: [${revelationKeyBytes.map(b=>b.toString(16).padStart(2,'0')).join(' ')}]`);
    console.log(`  Got:      [${decryptedRevKey.map(b=>b.toString(16).padStart(2,'0')).join(' ')}]`);
  }
} catch (e) {
  console.log(`Server decrypt FAILED: ${e.message} ❌`);
  // Try manual decrypt with both key variants
  const extractedIV = encryptedRevKey.slice(0, 32);
  const extractedCT = encryptedRevKey.slice(32, -16);
  const extractedTag = encryptedRevKey.slice(-16);

  const dec32 = AESGCMDecrypt(extractedCT, [], extractedIV, extractedTag, clientKey32);
  console.log(`  Manual decrypt with full 32-byte key: ${dec32 ? 'OK' : 'FAILED'}`);

  const decStripped = AESGCMDecrypt(extractedCT, [], extractedIV, extractedTag, serverKeyStripped);
  console.log(`  Manual decrypt with stripped key: ${decStripped ? 'OK' : 'FAILED'}`);
}

// ======== Test 3: Simulate the EXACT certificate flow ========
console.log("\n=== Test 3: Full certificate acquisition simulation ===");

// Step 1: Client creates nonce (matching our Rust code)
const firstHalf = [0x41, 0x42, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48,
                   0x49, 0x4A, 0x4B, 0x4C, 0x4D, 0x4E, 0x4F, 0x50]; // ASCII ABCDEFGHIJKLMNOP
const keyID = Utils.toUTF8(firstHalf);
console.log(`Nonce keyID: "${keyID}"`);

// Client derives HMAC key with SERVER as counterparty
const clientHmacKey = clientDeriver.deriveSymmetricKey([2, 'server hmac'], keyID, serverPub);
const hmac = require(sdkPath + '/dist/cjs/src/primitives/Hash.js').sha256hmac(clientHmacKey.toArray(), firstHalf);
const nonceBytes = [...firstHalf, ...hmac];
const nonceBase64 = Utils.toBase64(nonceBytes);
console.log(`Client nonce (base64): ${nonceBase64}`);

// Step 2: Server verifies nonce (matching signCertificate.ts)
const serverNonceBuffer = Utils.toArray(nonceBase64, 'base64');
const serverData = serverNonceBuffer.slice(0, 16);
const serverHmac = serverNonceBuffer.slice(16);
const serverKeyID = Utils.toUTF8(serverData);

const serverHmacKey = serverDeriver.deriveSymmetricKey([2, 'server hmac'], serverKeyID, clientPub);
const expectedHmac = require(sdkPath + '/dist/cjs/src/primitives/Hash.js').sha256hmac(serverHmacKey.toArray(), serverData);
const nonceValid = expectedHmac.every((b, i) => b === serverHmac[i]);
console.log(`Server nonce verification: ${nonceValid ? '✅ PASS' : '❌ FAIL'}`);

// Step 3: Client encrypts field
const fieldValue = "true";
const fieldValueBytes = Utils.toArray(fieldValue, 'utf8');

// Generate random field key (32 bytes)
const fieldKey = new Uint8Array(32);
for (let i = 0; i < 32; i++) fieldKey[i] = (i * 13 + 7) % 256; // deterministic

// Encrypt field value with field key
const fieldIV = Array.from({length: 32}, (_, i) => (i * 11 + 5) % 256);
const { result: fieldCT, authenticationTag: fieldTag } = AESGCM(fieldValueBytes, [], fieldIV, Array.from(fieldKey));
const encryptedFieldValue = [...fieldIV, ...fieldCT, ...fieldTag];

// Strip field key (matching our Rust)
let strippedFieldKey = Array.from(fieldKey);
while (strippedFieldKey.length > 1 && strippedFieldKey[0] === 0) strippedFieldKey.shift();

// Encrypt stripped field key for server using BRC-2
const revKeySymKey = clientDeriver.deriveSymmetricKey([2, 'certificate field encryption'], 'cool', serverPub);
const revKeyIV = Array.from({length: 32}, (_, i) => (i * 17 + 11) % 256);
const { result: revKeyCT, authenticationTag: revKeyTag } = AESGCM(strippedFieldKey, [], revKeyIV, revKeySymKey.toArray('be', 32));
const encryptedRevKeyFull = [...revKeyIV, ...revKeyCT, ...revKeyTag];

// Build CSR
const csr = {
  clientNonce: nonceBase64,
  type: "test-cert-type",
  fields: { cool: Utils.toBase64(encryptedFieldValue) },
  masterKeyring: { cool: Utils.toBase64(encryptedRevKeyFull) }
};
console.log(`CSR constructed with ${Object.keys(csr).length} fields`);

// Step 4: Server decrypts fields (matching MasterCertificate.decryptFields)
const serverRevSymKey = serverDeriver.deriveSymmetricKey([2, 'certificate field encryption'], 'cool', clientPub);
console.log(`Server BRC-2 key matches client: ${serverRevSymKey.toArray('be', 32).every((b, i) => b === revKeySymKey.toArray('be', 32)[i]) ? '✅' : '❌'}`);

try {
  // Decrypt revelation key
  const encRevKeyBytes = Utils.toArray(csr.masterKeyring.cool, 'base64');
  const decryptedRevKey = serverRevSymKey.decrypt(encRevKeyBytes);
  console.log(`Server decrypted revelation key (${decryptedRevKey.length} bytes): ${decryptedRevKey.every((b, i) => b === strippedFieldKey[i]) ? '✅ matches' : '❌ mismatch'}`);

  // Decrypt field value using revelation key
  const fieldSymKey = new SymmetricKey(decryptedRevKey);
  const encFieldBytes = Utils.toArray(csr.fields.cool, 'base64');
  const decryptedField = fieldSymKey.decrypt(encFieldBytes);
  const decryptedStr = Utils.toUTF8(decryptedField);
  console.log(`Server decrypted field 'cool' = "${decryptedStr}" ${decryptedStr === "true" ? '✅' : '❌'}`);
} catch (e) {
  console.log(`Server decrypt FAILED: ${e.message} ❌`);
}

console.log("\n=== Done ===");
