/**
 * Test: Can TypeScript SDK decrypt our Rust-encrypted data?
 *
 * This test takes encrypted data from our Rust logs and tries to decrypt it
 * using the TypeScript SDK (simulating what the server does).
 *
 * Usage:
 *   node test_decrypt_rust_encrypted.js
 *
 * Or provide values from logs:
 *   node test_decrypt_rust_encrypted.js <encryptedField> <encryptedKeyring> <fieldName>
 */

const path = require('path');

// Load TypeScript SDK
let Utils, ProtoWallet, SymmetricKey;
try {
    const sdkPath = path.join(__dirname, '..', 'reference', 'ts-brc100', 'node_modules', '@bsv', 'sdk');
    const sdk = require(sdkPath);
    Utils = sdk.Utils || sdk;
    ProtoWallet = sdk.ProtoWallet;
    SymmetricKey = sdk.SymmetricKey;
} catch (error) {
    console.error('Failed to load TypeScript SDK:', error.message);
    process.exit(1);
}

async function testDecryption() {
    console.log('Testing TypeScript SDK Decryption of Rust-Encrypted Data');
    console.log('========================================================\n');

    // Get values from command line or use example from logs
    const args = process.argv.slice(2);

    let encryptedFieldBase64, encryptedKeyringBase64, fieldName;
    let certifierPrivKeyHex, subjectPubKeyHex;

    if (args.length >= 3) {
        encryptedFieldBase64 = args[0];
        encryptedKeyringBase64 = args[1];
        fieldName = args[2];
        certifierPrivKeyHex = args[3];
        subjectPubKeyHex = args[4];
    } else {
        console.log('📋 Paste values from your Rust logs:');
        console.log('   - Encrypted field value (base64)');
        console.log('   - Encrypted masterKeyring value (base64)');
        console.log('   - Field name (e.g., "cool")');
        console.log('   - Certifier private key (hex) - from server');
        console.log('   - Subject public key (hex) - your identity key');
        console.log('');
        console.log('Example from your logs:');
        console.log('   Field: "XtUQoqvY4DHSIH+DPHCGUOWslWM89jLN+xTJDapj9/4NTL/6LTnu2LQPbF9+bLgTlTZXig=="');
        console.log('   Keyring: "5P8GIjy6vpPdwV/vatSufba73o6nEPQyienKvb93kTATZ5sxo9MLMURMYngPcqw5QvsHPBENcLPgKBm2PmPE3mUO1vleK9ZKJeR1kLOWg08="');
        console.log('   Field name: "cool"');
        console.log('');
        console.log('Usage:');
        console.log('   node test_decrypt_rust_encrypted.js <field> <keyring> <fieldName> [certifierPrivKey] [subjectPubKey]');
        return;
    }

    console.log('Test Parameters:');
    console.log(`   Field name: ${fieldName}`);
    console.log(`   Encrypted field (base64, first 50): ${encryptedFieldBase64.substring(0, 50)}...`);
    console.log(`   Encrypted keyring (base64, first 50): ${encryptedKeyringBase64.substring(0, 50)}...`);
    console.log('');

    // Create wallets from keys (if provided) or use test wallets
    let certifierWallet, subjectPublicKey;

    if (certifierPrivKeyHex && subjectPubKeyHex) {
        console.log('Creating wallets from provided keys...');
        // Create certifier wallet from private key
        // Note: ProtoWallet.fromPrivateKey might not exist, so we might need to use a different approach
        try {
            // For now, we'll use test wallets and note that real keys are needed
            const testMnemonic = 'abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about';
            certifierWallet = await ProtoWallet.fromMnemonic(testMnemonic);
            const subjectWallet = await ProtoWallet.fromMnemonic(testMnemonic);
            subjectPublicKey = subjectWallet.identityKey;
            console.log('⚠️  Using test wallets - provide real keys for accurate test');
        } catch (error) {
            console.error('Failed to create wallets:', error.message);
            return;
        }
    } else {
        console.log('Using test wallets...');
        const testMnemonic = 'abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about';
        certifierWallet = await ProtoWallet.fromMnemonic(testMnemonic);
        const subjectWallet = await ProtoWallet.fromMnemonic(testMnemonic);
        subjectPublicKey = subjectWallet.identityKey;
        console.log('⚠️  NOTE: For real test, you need the actual certifier private key and subject public key');
    }

    console.log(`   Certifier identity: ${certifierWallet.identityKey.toHex()}`);
    console.log(`   Subject identity: ${subjectPublicKey.toHex()}`);
    console.log('');

    try {
        // Step 1: Decrypt revelation key from masterKeyring
        console.log('Step 1: Decrypting revelation key from masterKeyring...');
        console.log('   (This is what the server does)');

        const encryptedKeyringBytes = Utils.fromBase64(encryptedKeyringBase64);
        console.log(`   Encrypted keyring length: ${encryptedKeyringBytes.length} bytes`);

        // Parse: [32-byte IV][ciphertext][16-byte tag]
        if (encryptedKeyringBytes.length < 48) {
            throw new Error(`Invalid encrypted data: ${encryptedKeyringBytes.length} bytes (need at least 48)`);
        }

        const iv = encryptedKeyringBytes.slice(0, 32);
        const ciphertext = encryptedKeyringBytes.slice(32, encryptedKeyringBytes.length - 16);
        const tag = encryptedKeyringBytes.slice(encryptedKeyringBytes.length - 16);

        console.log(`   IV (hex, first 16): ${Utils.toHex(iv).substring(0, 32)}...`);
        console.log(`   Ciphertext length: ${ciphertext.length} bytes`);
        console.log(`   Tag (hex): ${Utils.toHex(tag)}`);

        // Create invoice number for BRC-2 decryption
        const invoiceNumber = `2-certificate field encryption-${fieldName}`;
        console.log(`   Invoice number: ${invoiceNumber}`);

        // Derive symmetric key using BRC-2 (server perspective)
        // Server uses: certifier private key + subject public key
        const symmetricKey = await certifierWallet.keyDeriver.deriveSymmetricKey({
            invoiceNumber: invoiceNumber,
            counterparty: subjectPublicKey.toHex(),
            keyID: fieldName
        });

        console.log(`   Derived symmetric key (hex, first 32): ${symmetricKey.toHex().substring(0, 64)}...`);

        // Decrypt revelation key
        const encryptedData = Buffer.concat([iv, ciphertext, tag]);
        const revelationKey = symmetricKey.decrypt(encryptedData);

        console.log(`   ✅ Revelation key decrypted!`);
        console.log(`   Revelation key (hex): ${Utils.toHex(revelationKey)}`);
        console.log(`   Revelation key length: ${revelationKey.length} bytes`);
        console.log('');

        // Step 2: Decrypt field value using revelation key
        console.log('Step 2: Decrypting field value using revelation key...');

        const encryptedFieldBytes = Utils.fromBase64(encryptedFieldBase64);
        console.log(`   Encrypted field length: ${encryptedFieldBytes.length} bytes`);

        // Parse: [32-byte IV][ciphertext][16-byte tag]
        if (encryptedFieldBytes.length < 48) {
            throw new Error(`Invalid encrypted data: ${encryptedFieldBytes.length} bytes (need at least 48)`);
        }

        const fieldIv = encryptedFieldBytes.slice(0, 32);
        const fieldCiphertext = encryptedFieldBytes.slice(32, encryptedFieldBytes.length - 16);
        const fieldTag = encryptedFieldBytes.slice(encryptedFieldBytes.length - 16);

        console.log(`   IV (hex, first 16): ${Utils.toHex(fieldIv).substring(0, 32)}...`);
        console.log(`   Ciphertext length: ${fieldCiphertext.length} bytes`);
        console.log(`   Tag (hex): ${Utils.toHex(fieldTag)}`);

        // Create SymmetricKey from revelation key
        const fieldKey = SymmetricKey.fromArray(revelationKey, 'be');

        // Decrypt field value
        const fieldEncryptedData = Buffer.concat([fieldIv, fieldCiphertext, fieldTag]);
        const fieldValue = fieldKey.decrypt(fieldEncryptedData);
        const fieldValueString = fieldValue.toString('utf8');

        console.log(`   ✅ Field value decrypted!`);
        console.log(`   Decrypted value: ${fieldValueString}`);
        console.log('');

        console.log('✅✅✅ SUCCESS! TypeScript SDK can decrypt our Rust-encrypted data!');
        console.log(`   Field '${fieldName}' = ${fieldValueString}`);
        console.log('');
        console.log('This means our encryption format is correct and the server should be able to decrypt it.');
        console.log('If the server still returns 500, the issue is likely:');
        console.log('  1. Server is using different keys than expected');
        console.log('  2. Server has a bug in its decryption logic');
        console.log('  3. Server expects something else we\'re not providing');

    } catch (error) {
        console.error('\n❌❌❌ FAILED! TypeScript SDK cannot decrypt our Rust-encrypted data!');
        console.error(`   Error: ${error.message}`);
        console.error(error.stack);
        console.log('');
        console.log('This means there\'s a mismatch in our encryption format.');
        console.log('We need to fix the encryption to match TypeScript SDK exactly.');
        process.exit(1);
    }
}

testDecryption().catch(error => {
    console.error('Fatal error:', error);
    process.exit(1);
});

