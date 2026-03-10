/**
 * TypeScript SDK Interoperability Test
 *
 * This script encrypts data using the TypeScript SDK and outputs test vectors
 * that can be used by the Rust interoperability test.
 *
 * Usage:
 *   1. Make sure you're in the rust-wallet directory
 *   2. Run: node test_interoperability_ts.js
 *   3. Copy the output into the Rust test
 *
 * Prerequisites:
 *   - Node.js installed
 *   - TypeScript SDK available in reference/ts-brc100/node_modules/@bsv/sdk
 */

const path = require('path');
const fs = require('fs');

// Try to load the TypeScript SDK
let SymmetricKey, Utils, Random;

try {
    // Try to load from the reference directory
    const sdkPath = path.join(__dirname, '..', 'reference', 'ts-brc100', 'node_modules', '@bsv', 'sdk');
    const sdk = require(sdkPath);

    SymmetricKey = sdk.SymmetricKey;
    Utils = sdk.Utils || sdk;
    Random = sdk.Random;

    console.log('✅ Loaded TypeScript SDK from reference directory');
} catch (e) {
    console.error('❌ Failed to load TypeScript SDK:', e.message);
    console.error('   Make sure the SDK is installed in reference/ts-brc100/node_modules/@bsv/sdk');
    process.exit(1);
}

function testFieldEncryption() {
    console.log('\n=== Test 1: Field Value Encryption ===');

    // Test with "true" (4 bytes)
    const plaintext = 'true';
    const plaintextBytes = Utils.toArray(plaintext, 'utf8');

    // Generate a random symmetric key (32 bytes)
    const fieldSymmetricKey = SymmetricKey.fromRandom();

    // Encrypt the field value
    const encryptedFieldValue = fieldSymmetricKey.encrypt(plaintextBytes);

    // Convert to base64 (matching CSR format)
    const encryptedBase64 = Utils.toBase64(encryptedFieldValue);

    // Get the key as array (stripped of leading zeros, matching toArray() behavior)
    const keyArray = fieldSymmetricKey.toArray();
    const keyArrayHex = keyArray.map(b => b.toString(16).padStart(2, '0')).join('');

    // Extract components for verification
    const encryptedArray = Array.isArray(encryptedFieldValue) ? encryptedFieldValue : Utils.toArray(encryptedFieldValue, 'base64');
    const iv = encryptedArray.slice(0, 32);
    const ciphertext = encryptedArray.slice(32, encryptedArray.length - 16);
    const authTag = encryptedArray.slice(encryptedArray.length - 16);

    console.log('Plaintext:', plaintext);
    console.log('Plaintext (hex):', plaintextBytes.map(b => b.toString(16).padStart(2, '0')).join(''));
    console.log('Symmetric Key (hex, full 32 bytes):', fieldSymmetricKey.toArray('be', 32).map(b => b.toString(16).padStart(2, '0')).join(''));
    console.log('Symmetric Key (toArray, stripped):', keyArrayHex);
    console.log('Encrypted (base64):', encryptedBase64);
    console.log('IV (hex):', iv.map(b => b.toString(16).padStart(2, '0')).join(''));
    console.log('Ciphertext (hex):', ciphertext.map(b => b.toString(16).padStart(2, '0')).join(''));
    console.log('Auth Tag (hex):', authTag.map(b => b.toString(16).padStart(2, '0')).join(''));
    console.log('Total length:', encryptedArray.length, 'bytes');

    // Test decryption
    const decrypted = fieldSymmetricKey.decrypt(encryptedFieldValue);
    const decryptedStr = Utils.toUTF8(decrypted);
    console.log('Decrypted:', decryptedStr);
    console.log('✅ Decryption successful:', decryptedStr === plaintext);

    return {
        plaintext,
        plaintextHex: plaintextBytes.map(b => b.toString(16).padStart(2, '0')).join(''),
        keyFull: fieldSymmetricKey.toArray('be', 32).map(b => b.toString(16).padStart(2, '0')).join(''),
        keyStripped: keyArrayHex,
        encryptedBase64,
        iv: iv.map(b => b.toString(16).padStart(2, '0')).join(''),
        ciphertext: ciphertext.map(b => b.toString(16).padStart(2, '0')).join(''),
        authTag: authTag.map(b => b.toString(16).padStart(2, '0')).join(''),
        totalLength: encryptedArray.length
    };
}

function testRevelationKeyEncryption() {
    console.log('\n=== Test 2: Revelation Key Encryption (BRC-2) ===');

    // This would require ProtoWallet and BRC-2 encryption
    // For now, we'll just show the format

    console.log('Note: Revelation key encryption requires BRC-2 with BRC-42 key derivation');
    console.log('This test requires ProtoWallet and is more complex');
    console.log('See the Rust test for full BRC-2 encryption test');
}

function testRevelationKeyStripping() {
    console.log('\n=== Test 3: Revelation Key Stripping (toArray behavior) ===');

    // Test various key formats
    const testKeys = [
        { name: 'Key with leading zeros', bytes: [0x00, 0x00, 0x01, 0x02, 0x03] },
        { name: 'Key with no leading zeros', bytes: [0x01, 0x02, 0x03] },
        { name: 'Key all zeros', bytes: [0x00, 0x00, 0x00] },
        { name: 'Single zero', bytes: [0x00] },
        { name: '32-byte key with leading zeros', bytes: new Array(30).fill(0).concat([0x01, 0x02]) },
    ];

    testKeys.forEach(test => {
        const bn = new (require('@bsv/sdk').BigNumber)(test.bytes, 'be');
        const toArrayResult = bn.toArray();
        console.log(`${test.name}:`);
        console.log('  Original:', test.bytes.map(b => b.toString(16).padStart(2, '0')).join(' '));
        console.log('  toArray():', toArrayResult.map(b => b.toString(16).padStart(2, '0')).join(' '));
    });
}

// Run tests
console.log('TypeScript SDK Interoperability Test');
console.log('=====================================\n');

const fieldTest = testFieldEncryption();
testRevelationKeyEncryption();
testRevelationKeyStripping();

// Output test vectors for Rust
console.log('\n=== Test Vectors for Rust Test ===');
console.log('Copy these values into the Rust interoperability test:');
console.log(JSON.stringify(fieldTest, null, 2));

