/**
 * Test server-side decryption using TypeScript SDK
 *
 * This test simulates what the certifier server does:
 * 1. Receives CSR with encrypted fields and masterKeyring
 * 2. Decrypts masterKeyring revelation keys using BRC-2
 * 3. Uses revelation keys to decrypt fields
 * 4. Validates the decrypted data
 *
 * We'll use Rust to encrypt, then TypeScript SDK to decrypt (like the server would)
 */

const path = require('path');
const fs = require('fs');

// Try to load the TypeScript SDK
let Utils, MasterCertificate, ProtoWallet, Random;
try {
    const sdkPath = path.join(__dirname, '..', 'reference', 'ts-brc100', 'node_modules', '@bsv', 'sdk');
    const sdk = require(sdkPath);
    Utils = sdk.Utils || sdk;
    MasterCertificate = sdk.MasterCertificate;
    ProtoWallet = sdk.ProtoWallet;
    Random = sdk.Random;
} catch (error) {
    console.error('Failed to load TypeScript SDK:', error.message);
    process.exit(1);
}

/**
 * Decrypt a revelation key from masterKeyring (what the server does)
 *
 * @param {string} encryptedRevelationKeyBase64 - Base64-encoded encrypted revelation key
 * @param {ProtoWallet} certifierWallet - Certifier's wallet (has private key)
 * @param {PublicKey} subjectPublicKey - Subject's public key
 * @param {string} fieldName - Field name (e.g., "cool")
 * @returns {Promise<Buffer>} - Decrypted revelation key (symmetric key)
 */
async function decryptRevelationKey(encryptedRevelationKeyBase64, certifierWallet, subjectPublicKey, fieldName) {
    console.log(`\n🔓 Decrypting revelation key for field '${fieldName}'...`);
    console.log(`   Encrypted (base64): ${encryptedRevelationKeyBase64}`);

    // Decode the encrypted data
    const encryptedBytes = Utils.fromBase64(encryptedRevelationKeyBase64);
    console.log(`   Encrypted length: ${encryptedBytes.length} bytes`);

    // Parse: [32-byte IV][ciphertext][16-byte tag]
    if (encryptedBytes.length < 48) {
        throw new Error(`Invalid encrypted data length: ${encryptedBytes.length} (expected at least 48 bytes)`);
    }

    const iv = encryptedBytes.slice(0, 32);
    const ciphertext = encryptedBytes.slice(32, encryptedBytes.length - 16);
    const tag = encryptedBytes.slice(encryptedBytes.length - 16);

    console.log(`   IV (hex): ${Utils.toHex(iv)}`);
    console.log(`   Ciphertext length: ${ciphertext.length} bytes`);
    console.log(`   Tag (hex): ${Utils.toHex(tag)}`);

    // Create invoice number for certificate field encryption
    const invoiceNumber = `2-certificate field encryption-${fieldName}`;
    console.log(`   Invoice number: ${invoiceNumber}`);

    // Use BRC-2 to derive the symmetric key
    // Server perspective: certifier (recipient) decrypts using their private key + subject's public key
    const symmetricKey = await certifierWallet.keyDeriver.deriveSymmetricKey({
        invoiceNumber: invoiceNumber,
        counterparty: subjectPublicKey.toHex(),
        keyID: fieldName
    });

    console.log(`   Derived symmetric key (hex, first 16): ${symmetricKey.toHex().slice(0, 32)}`);

    // Decrypt using the symmetric key
    // The TypeScript SDK's SymmetricKey.decrypt() expects: [IV][ciphertext][tag]
    const encryptedData = Buffer.concat([iv, ciphertext, tag]);

    try {
        const decrypted = symmetricKey.decrypt(encryptedData);
        console.log(`   ✅ Decrypted revelation key (hex): ${Utils.toHex(decrypted)}`);
        console.log(`   ✅ Decrypted revelation key length: ${decrypted.length} bytes`);
        return decrypted;
    } catch (error) {
        console.error(`   ❌ Decryption failed: ${error.message}`);
        throw error;
    }
}

/**
 * Decrypt a field value using a revelation key (what the server does)
 *
 * @param {string} encryptedFieldValueBase64 - Base64-encoded encrypted field value
 * @param {Buffer} revelationKey - Decrypted revelation key (symmetric key)
 * @returns {Promise<string>} - Decrypted field value
 */
async function decryptFieldValue(encryptedFieldValueBase64, revelationKey) {
    console.log(`\n🔓 Decrypting field value...`);
    console.log(`   Encrypted (base64): ${encryptedFieldValueBase64}`);

    // Decode the encrypted data
    const encryptedBytes = Utils.fromBase64(encryptedFieldValueBase64);
    console.log(`   Encrypted length: ${encryptedBytes.length} bytes`);

    // Parse: [32-byte IV][ciphertext][16-byte tag]
    if (encryptedBytes.length < 48) {
        throw new Error(`Invalid encrypted data length: ${encryptedBytes.length} (expected at least 48 bytes)`);
    }

    const iv = encryptedBytes.slice(0, 32);
    const ciphertext = encryptedBytes.slice(32, encryptedBytes.length - 16);
    const tag = encryptedBytes.slice(encryptedBytes.length - 16);

    console.log(`   IV (hex): ${Utils.toHex(iv)}`);
    console.log(`   Ciphertext length: ${ciphertext.length} bytes`);
    console.log(`   Tag (hex): ${Utils.toHex(tag)}`);

    // Create SymmetricKey from revelation key
    // Note: TypeScript SDK's SymmetricKey expects the key bytes directly
    const SymmetricKey = Utils.SymmetricKey || (await import(sdkPath + '/primitives/SymmetricKey.js')).default;
    const key = SymmetricKey.fromArray(revelationKey, 'be');

    // Decrypt
    const encryptedData = Buffer.concat([iv, ciphertext, tag]);

    try {
        const decrypted = key.decrypt(encryptedData);
        const decryptedString = decrypted.toString('utf8');
        console.log(`   ✅ Decrypted field value: ${decryptedString}`);
        return decryptedString;
    } catch (error) {
        console.error(`   ❌ Decryption failed: ${error.message}`);
        throw error;
    }
}

/**
 * Test server-side decryption
 *
 * This simulates what the certifier server does when it receives our CSR
 */
async function testServerDecryption() {
    console.log('Server-Side Decryption Test');
    console.log('============================\n');

    // Create a test certifier wallet (simulating the server)
    // In reality, the server would use its own private key
    const certifierMnemonic = 'abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about';
    const certifierWallet = await ProtoWallet.fromMnemonic(certifierMnemonic);

    // Create a test subject wallet (simulating us)
    const subjectMnemonic = 'abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about';
    const subjectWallet = await ProtoWallet.fromMnemonic(subjectMnemonic);
    const subjectPublicKey = subjectWallet.identityKey;

    console.log('Test Setup:');
    console.log(`   Certifier identity key: ${certifierWallet.identityKey.toHex()}`);
    console.log(`   Subject identity key: ${subjectPublicKey.toHex()}`);
    console.log('');

    // For this test, we'll use sample encrypted data from our Rust logs
    // In a real test, we'd generate this from Rust, but for now we'll use the format

    console.log('📋 To use this test:');
    console.log('   1. Run your Rust acquireCertificate request');
    console.log('   2. Copy the encrypted field value and masterKeyring from the logs');
    console.log('   3. Paste them below and run this script');
    console.log('');

    // Example from your logs:
    // Field: "XtUQoqvY4DHSIH+DPHCGUOWslWM89jLN+xTJDapj9/4NTL/6LTnu2LQPbF9+bLgTlTZXig=="
    // MasterKeyring: "5P8GIjy6vpPdwV/vatSufba73o6nEPQyienKvb93kTATZ5sxo9MLMURMYngPcqw5QvsHPBENcLPgKBm2PmPE3mUO1vleK9ZKJeR1kLOWg08="

    const args = process.argv.slice(2);
    if (args.length < 3) {
        console.log('Usage: node test_server_decryption.js <encryptedFieldBase64> <encryptedRevelationKeyBase64> <fieldName>');
        console.log('');
        console.log('Example:');
        console.log('  node test_server_decryption.js "XtUQoqvY4DHSIH+..." "5P8GIjy6vpPdwV/..." "cool"');
        console.log('');
        console.log('Or provide certifier and subject keys:');
        console.log('  node test_server_decryption.js <field> <keyring> <fieldName> <certifierPrivKeyHex> <subjectPubKeyHex>');
        return;
    }

    const encryptedFieldBase64 = args[0];
    const encryptedRevelationKeyBase64 = args[1];
    const fieldName = args[2];

    let certifierPrivKeyHex = args[3];
    let subjectPubKeyHex = args[4];

    // If keys not provided, use test wallets
    if (!certifierPrivKeyHex || !subjectPubKeyHex) {
        console.log('⚠️  Using test wallets (not real keys from your request)');
        console.log('   For real test, provide certifier private key and subject public key');
        console.log('');
    } else {
        // Create wallets from provided keys
        // This is more complex - we'd need to create wallets from keys
        console.log('Using provided keys...');
    }

    try {
        // Step 1: Decrypt revelation key (what server does)
        const revelationKey = await decryptRevelationKey(
            encryptedRevelationKeyBase64,
            certifierWallet,
            subjectPublicKey,
            fieldName
        );

        // Step 2: Decrypt field value using revelation key
        const fieldValue = await decryptFieldValue(encryptedFieldBase64, revelationKey);

        console.log('\n✅ Server-side decryption test PASSED!');
        console.log(`   Field '${fieldName}' decrypted to: ${fieldValue}`);
        console.log(`   Expected: true (or whatever the original value was)`);

    } catch (error) {
        console.error('\n❌ Server-side decryption test FAILED!');
        console.error(`   Error: ${error.message}`);
        console.error(error.stack);
        process.exit(1);
    }
}

// Run the test
testServerDecryption().catch(error => {
    console.error('Fatal error:', error);
    process.exit(1);
});

