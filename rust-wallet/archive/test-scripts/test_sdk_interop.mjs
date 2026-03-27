#!/usr/bin/env node
// Quick test to compare SDK's toUTF8 + HMAC with our Rust implementation
// Run: node test_sdk_interop.mjs

// Import SDK utils from the reference project
import { createRequire } from 'module';
const require = createRequire(import.meta.url);

// Use the SDK from the reference project
const sdkPath = '../reference/ts-brc100/node_modules/@bsv/sdk';
const { Utils, Hash, PrivateKey, SymmetricKey, Random } = require(sdkPath);

// Also import the internal key deriver
const { KeyDeriver } = require(sdkPath + '/dist/cjs/src/wallet/KeyDeriver.js');

// Test 1: toUTF8 with various byte patterns
console.log("=== Test 1: toUTF8 comparison ===");
const testCases = [
  // Normal ASCII
  [0x48, 0x65, 0x6C, 0x6C, 0x6F], // "Hello"
  // 2-byte sequences
  [0xC0, 0x80], // overlong NUL
  [0xC2, 0xA3], // £
  // 3-byte producing surrogate range (0xD800)
  [0xED, 0xA0, 0x80],
  // 4-byte with code_point < 0x10000 (THE BUG CASE)
  [0xF0, 0x80, 0x80, 0x80], // code_point = 0
  [0xF0, 0x80, 0x80, 0xBF], // code_point = 63
  [0xF0, 0x80, 0x81, 0x80], // code_point = 64
  [0xF0, 0x8F, 0xBF, 0xBF], // code_point = 0xFFFF (max BMP in 4-byte)
  // 4-byte with code_point >= 0x10000 (should work correctly)
  [0xF0, 0x90, 0x80, 0x80], // code_point = 0x10000
  [0xF0, 0x90, 0x80, 0xBF], // code_point = 0x1003F
  // Continuation bytes at start (should be skipped)
  [0x80, 0x41], // continuation + 'A'
  [0xBF, 0x42], // continuation + 'B'
  // Mixed problematic sequence
  [0xF0, 0x80, 0x80, 0x80, 0x41], // 4-byte (cp=0) + 'A'
];

for (const bytes of testCases) {
  const str = Utils.toUTF8(bytes);
  // Convert string back to UTF-8 bytes using toArray
  const roundtrippedBytes = Utils.toArray(str, 'utf8');
  console.log(`Input:  [${bytes.map(b => '0x' + b.toString(16).padStart(2, '0')).join(', ')}]`);
  console.log(`String: ${JSON.stringify(str)} (length: ${str.length}, codePoints: [${[...str].map(c => '0x' + c.codePointAt(0).toString(16)).join(', ')}])`);
  console.log(`Bytes:  [${roundtrippedBytes.map(b => '0x' + b.toString(16).padStart(2, '0')).join(', ')}]`);
  console.log(`Hex:    ${roundtrippedBytes.map(b => b.toString(16).padStart(2, '0')).join('')}`);
  console.log();
}

// Test 2: Full nonce HMAC comparison
console.log("\n=== Test 2: Nonce HMAC with known bytes ===");
// Use a deterministic "random" 16 bytes for testing
const knownFirstHalf = [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08,
                         0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F, 0x10];

// Use a known private key for testing
const testPrivKeyHex = 'e8f32e723decf4051aefac8e2c93c9c5b214313817cdb01a1494b917c8436b35';
const testPrivKey = PrivateKey.fromString(testPrivKeyHex, 16);
const testPubKey = testPrivKey.toPublicKey();
console.log(`Test private key: ${testPrivKeyHex}`);
console.log(`Test public key: ${testPubKey.toString()}`);

// Derive keyID using toUTF8
const keyID = Utils.toUTF8(knownFirstHalf);
console.log(`\nKeyID from toUTF8([01..10]): ${JSON.stringify(keyID)}`);
console.log(`KeyID hex bytes: ${Utils.toArray(keyID, 'utf8').map(b => b.toString(16).padStart(2, '0')).join('')}`);

// Create KeyDeriver for HMAC computation
const keyDeriver = new KeyDeriver(testPrivKey);

// Derive symmetric key using same params as createNonce
// counterparty='self' means use own public key
const symmetricKey = keyDeriver.deriveSymmetricKey(
  [2, 'server hmac'],
  keyID,
  'self'
);

console.log(`\nSymmetric key (full): ${symmetricKey.toArray('be', 32).map(b => b.toString(16).padStart(2, '0')).join('')}`);
console.log(`Symmetric key (stripped): ${symmetricKey.toArray().map(b => b.toString(16).padStart(2, '0')).join('')}`);

// Compute HMAC
const hmac = Hash.sha256hmac(symmetricKey.toArray(), knownFirstHalf);
console.log(`HMAC: ${hmac.map(b => b.toString(16).padStart(2, '0')).join('')}`);

// Full nonce
const nonceBytes = [...knownFirstHalf, ...hmac];
const nonceBase64 = Utils.toBase64(nonceBytes);
console.log(`Nonce (base64): ${nonceBase64}`);

// Test 3: With counterparty as a specific public key (simulating certifier)
console.log("\n=== Test 3: Nonce HMAC with counterparty ===");
const counterpartyPrivKeyHex = 'c9afa9d845ba75166b5c215767b1d6934e50c3db36e89b127b8a622b120f6721';
const counterpartyPrivKey = PrivateKey.fromString(counterpartyPrivKeyHex, 16);
const counterpartyPubKey = counterpartyPrivKey.toPublicKey();
console.log(`Counterparty public key: ${counterpartyPubKey.toString()}`);

// Client creates nonce with counterparty's pubkey
const symmetricKeyWithCounterparty = keyDeriver.deriveSymmetricKey(
  [2, 'server hmac'],
  keyID,
  counterpartyPubKey
);
console.log(`\nClient symmetric key (full): ${symmetricKeyWithCounterparty.toArray('be', 32).map(b => b.toString(16).padStart(2, '0')).join('')}`);
console.log(`Client symmetric key (stripped): ${symmetricKeyWithCounterparty.toArray().map(b => b.toString(16).padStart(2, '0')).join('')}`);

const hmacWithCounterparty = Hash.sha256hmac(symmetricKeyWithCounterparty.toArray(), knownFirstHalf);
console.log(`Client HMAC: ${hmacWithCounterparty.map(b => b.toString(16).padStart(2, '0')).join('')}`);

// Server verifies: uses its own key with client's pubkey as counterparty
const serverKeyDeriver = new KeyDeriver(counterpartyPrivKey);
const serverSymmetricKey = serverKeyDeriver.deriveSymmetricKey(
  [2, 'server hmac'],
  keyID,
  testPubKey  // client's public key
);
console.log(`\nServer symmetric key (full): ${serverSymmetricKey.toArray('be', 32).map(b => b.toString(16).padStart(2, '0')).join('')}`);
console.log(`Server symmetric key (stripped): ${serverSymmetricKey.toArray().map(b => b.toString(16).padStart(2, '0')).join('')}`);

const serverHmac = Hash.sha256hmac(serverSymmetricKey.toArray(), knownFirstHalf);
console.log(`Server HMAC: ${serverHmac.map(b => b.toString(16).padStart(2, '0')).join('')}`);

const match = hmacWithCounterparty.toString() === serverHmac.toString();
console.log(`\nHMAC match (ECDH symmetry): ${match ? '✅ YES' : '❌ NO'}`);

// Test 4: BRC-2 field encryption (for comparing with Rust)
console.log("\n=== Test 4: BRC-2 field encryption ===");
const fieldKey = SymmetricKey.fromRandom();
const fieldKeyHex = fieldKey.toArray('be', 32).map(b => b.toString(16).padStart(2, '0')).join('');
console.log(`Field symmetric key (32 bytes): ${fieldKeyHex}`);
console.log(`Field symmetric key stripped: ${fieldKey.toArray().map(b => b.toString(16).padStart(2, '0')).join('')}`);

// Test 5: Random 16-byte nonce - check for 4-byte sequence issues
console.log("\n=== Test 5: Random nonces - checking for problematic bytes ===");
let problematic = 0;
let total = 1000;
for (let i = 0; i < total; i++) {
  const firstHalf = Random(16);
  const str1 = Utils.toUTF8(firstHalf);
  // Roundtrip: bytes -> string -> bytes
  const roundtripped = Utils.toArray(str1, 'utf8');
  // Check if any byte in 0xF0 range exists AND the 4-byte codepoint < 0x10000
  let has4ByteSmallCP = false;
  let skip = 0;
  for (let j = 0; j < firstHalf.length; j++) {
    if (skip > 0) { skip--; continue; }
    const b = firstHalf[j];
    if (b >= 0xf0 && b <= 0xf7) {
      skip = 3;
      const b2 = firstHalf[j+1] || 0;
      const b3 = firstHalf[j+2] || 0;
      const b4 = firstHalf[j+3] || 0;
      const cp = ((b & 0x07) << 18) | ((b2 & 0x3f) << 12) | ((b3 & 0x3f) << 6) | (b4 & 0x3f);
      if (cp < 0x10000) has4ByteSmallCP = true;
    } else if (b >= 0xe0 && b <= 0xef) { skip = 2; }
    else if (b >= 0xc0 && b <= 0xdf) { skip = 1; }
  }
  if (has4ByteSmallCP) {
    problematic++;
    if (problematic <= 3) {
      console.log(`  Problematic nonce #${problematic}: [${firstHalf.map(b => b.toString(16).padStart(2,'0')).join(' ')}]`);
      console.log(`    String: ${JSON.stringify(str1)}`);
      console.log(`    Roundtripped bytes: ${roundtripped.map(b => b.toString(16).padStart(2,'0')).join(' ')}`);
    }
  }
}
console.log(`\nProblematic nonces (4-byte with CP < 0x10000): ${problematic}/${total} = ${(problematic/total*100).toFixed(1)}%`);

console.log("\n=== Done ===");
