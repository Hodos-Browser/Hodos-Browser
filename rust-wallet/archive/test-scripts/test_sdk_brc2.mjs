#!/usr/bin/env node
// Test BRC-2 encryption/decryption interop
// Specifically tests the decrypt path that the server uses (SymmetricKey with stripped keys)

import { createRequire } from 'module';
const require = createRequire(import.meta.url);
const sdkPath = '../reference/ts-brc100/node_modules/@bsv/sdk';
const { PrivateKey, SymmetricKey, Utils } = require(sdkPath);
const { KeyDeriver } = require(sdkPath + '/dist/cjs/src/wallet/KeyDeriver.js');
const { AESGCM, AESGCMDecrypt } = require(sdkPath + '/dist/cjs/src/primitives/AESGCM.js');

// Test: Encrypt with 32-byte key, decrypt with stripped key
console.log("=== Test: Key stripping in SymmetricKey.encrypt/decrypt ===");

// Create a key with a leading zero byte
const keyWithZero = new SymmetricKey([
  0x00, 0x39, 0xa3, 0x60, 0x13, 0x30, 0x15, 0x97,
  0xda, 0xef, 0x41, 0xfb, 0xe5, 0x93, 0xa0, 0x2c,
  0xc5, 0x13, 0xd0, 0xb5, 0x55, 0x27, 0xec, 0x2d,
  0xf1, 0x05, 0x0e, 0x2e, 0x8f, 0xf4, 0x9c, 0x85
]);

console.log(`Key toArray('be', 32): [${keyWithZero.toArray('be', 32).map(b=>b.toString(16).padStart(2,'0')).join(' ')}] (${keyWithZero.toArray('be', 32).length} bytes)`);
console.log(`Key toArray():         [${keyWithZero.toArray().map(b=>b.toString(16).padStart(2,'0')).join(' ')}] (${keyWithZero.toArray().length} bytes)`);

// Test encrypt then decrypt with this key
const plaintext = [0x74, 0x72, 0x75, 0x65]; // "true"
try {
  const encrypted = keyWithZero.encrypt(plaintext);
  console.log(`\nEncrypt succeeded. Ciphertext: ${encrypted.length} items`);
  const decrypted = keyWithZero.decrypt(encrypted);
  console.log(`Decrypt succeeded. Plaintext: [${decrypted.map(b=>b.toString(16).padStart(2,'0')).join(' ')}]`);
  console.log(`Match: ${JSON.stringify(plaintext) === JSON.stringify(decrypted) ? '✅' : '❌'}`);
} catch(e) {
  console.log(`ERROR: ${e.message}`);
}

// Test: Encrypt with 32-byte key (padded), decrypt with stripped key explicitly
console.log("\n=== Test: Explicit encrypt with 32-byte / decrypt with 31-byte ===");
const key32 = keyWithZero.toArray('be', 32); // 32 bytes with leading zero
const key31 = keyWithZero.toArray();          // 31 bytes without leading zero

const iv = Array.from({length: 32}, () => Math.floor(Math.random() * 256));
const { result: cipher, authenticationTag } = AESGCM(plaintext, [], iv, key32);
console.log(`Encrypted with 32-byte key: cipher=${cipher.length} bytes, tag=${authenticationTag.length} bytes`);

// Try decrypt with 31-byte key
const decResult31 = AESGCMDecrypt(cipher, [], iv, authenticationTag, key31);
console.log(`Decrypt with 31-byte key: ${decResult31 === null ? '❌ FAILED (null)' : '✅ succeeded'}`);

// Try decrypt with 32-byte key
const decResult32 = AESGCMDecrypt(cipher, [], iv, authenticationTag, key32);
console.log(`Decrypt with 32-byte key: ${decResult32 === null ? '❌ FAILED (null)' : '✅ succeeded'}`);

// Test: BRC-2 derive + encrypt/decrypt cycle (simulating our Rust->Server flow)
console.log("\n=== Test: BRC-2 derivation key length ===");
const clientPrivKey = PrivateKey.fromString('e8f32e723decf4051aefac8e2c93c9c5b214313817cdb01a1494b917c8436b35', 16);
const serverPrivKey = PrivateKey.fromString('c9afa9d845ba75166b5c215767b1d6934e50c3db36e89b127b8a622b120f6721', 16);
const clientPubKey = clientPrivKey.toPublicKey();
const serverPubKey = serverPrivKey.toPublicKey();

console.log(`Client pubkey: ${clientPubKey.toString()}`);
console.log(`Server pubkey: ${serverPubKey.toString()}`);

// Client encrypts revelation key for server (field "cool")
const clientDeriver = new KeyDeriver(clientPrivKey);
const clientSymKey = clientDeriver.deriveSymmetricKey(
  [2, 'certificate field encryption'],
  'cool',
  serverPubKey
);
console.log(`\nClient derived symmetric key (be,32): ${clientSymKey.toArray('be', 32).map(b=>b.toString(16).padStart(2,'0')).join('')} (${clientSymKey.toArray('be', 32).length} bytes)`);
console.log(`Client derived symmetric key (strip): ${clientSymKey.toArray().map(b=>b.toString(16).padStart(2,'0')).join('')} (${clientSymKey.toArray().length} bytes)`);

// Encrypt with full 32-byte key (like SymmetricKey.encrypt does)
const revelationKey = Array.from({length: 32}, () => Math.floor(Math.random() * 256));
const encryptedRev = clientSymKey.encrypt(revelationKey);
console.log(`Encrypted revelation key: ${encryptedRev.length} items`);

// Server decrypts (uses its own derived key)
const serverDeriver = new KeyDeriver(serverPrivKey);
const serverSymKey = serverDeriver.deriveSymmetricKey(
  [2, 'certificate field encryption'],
  'cool',
  clientPubKey
);
console.log(`Server derived symmetric key (be,32): ${serverSymKey.toArray('be', 32).map(b=>b.toString(16).padStart(2,'0')).join('')} (${serverSymKey.toArray('be', 32).length} bytes)`);
console.log(`Server derived symmetric key (strip): ${serverSymKey.toArray().map(b=>b.toString(16).padStart(2,'0')).join('')} (${serverSymKey.toArray().length} bytes)`);

// Keys should match (ECDH symmetry)
const keysMatch = clientSymKey.toArray('be', 32).toString() === serverSymKey.toArray('be', 32).toString();
console.log(`Symmetric keys match: ${keysMatch ? '✅' : '❌'}`);

// Server tries to decrypt
try {
  const decryptedRev = serverSymKey.decrypt(encryptedRev);
  console.log(`Server decryption: ✅ succeeded`);
  const revMatch = revelationKey.toString() === decryptedRev.toString();
  console.log(`Revelation key match: ${revMatch ? '✅' : '❌'}`);
} catch(e) {
  console.log(`Server decryption: ❌ FAILED - ${e.message}`);
}

// NOW the key test: simulate what our Rust code does
// Rust encrypts with the FULL 32-byte x-coordinate
// Can the SDK decrypt with the stripped key?
console.log("\n=== Test: Simulated Rust encrypt (32-byte key) -> SDK decrypt (stripped key) ===");
const fullKey = clientSymKey.toArray('be', 32);
const strippedKey = clientSymKey.toArray();
console.log(`Full key length: ${fullKey.length}, Stripped length: ${strippedKey.length}`);

if (fullKey.length !== strippedKey.length) {
  console.log("⚠️  Key lengths differ! This WILL cause decrypt failure.");
  // Manually encrypt with the full 32-byte key
  const testIV = Array.from({length: 32}, () => Math.floor(Math.random() * 256));
  const { result: testCipher, authenticationTag: testTag } = AESGCM(revelationKey, [], testIV, fullKey);
  const testEncrypted = [...testIV, ...testCipher, ...testTag];

  // Try to decrypt with stripped key (simulating server behavior)
  const testDecResult = AESGCMDecrypt(testCipher, [], testIV, testTag, strippedKey);
  console.log(`Decrypt with stripped key: ${testDecResult === null ? '❌ FAILED' : '✅ succeeded'}`);

  // Also try with full key
  const testDecResult2 = AESGCMDecrypt(testCipher, [], testIV, testTag, fullKey);
  console.log(`Decrypt with full key: ${testDecResult2 === null ? '❌ FAILED' : '✅ succeeded'}`);
} else {
  console.log("Keys are same length - no strip issue here.");
}

// Stress test: Check how often ECDH produces keys with leading zeros
console.log("\n=== Test: Leading zero frequency in derived symmetric keys ===");
let zeroCount = 0;
let testCount = 100;
for (let i = 0; i < testCount; i++) {
  const rk = PrivateKey.fromRandom();
  const rpk = PrivateKey.fromRandom().toPublicKey();
  const kd = new KeyDeriver(rk);
  const sk = kd.deriveSymmetricKey([2, 'certificate field encryption'], `field${i}`, rpk);
  if (sk.toArray('be', 32)[0] === 0) {
    zeroCount++;
  }
}
console.log(`Keys with leading zero: ${zeroCount}/${testCount}`);

console.log("\n=== Done ===");
