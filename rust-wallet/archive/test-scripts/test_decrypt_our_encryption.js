/**
 * Test script to verify our Rust wallet's encryption
 *
 * This script takes the encrypted values from our logs and tries to decrypt them
 * using the TypeScript SDK, simulating what the server does.
 */

const path = require('path');
const sdkPath = path.join(__dirname, '..', 'reference', 'ts-brc100', 'node_modules', '@bsv', 'sdk');
const sdk = require(sdkPath);
const { Utils, ProtoWallet, MasterCertificate, Mnemonic, HD } = sdk;

// These values from your logs - UPDATE THESE with values from your latest run
const CLIENT_IDENTITY_KEY = '020b95583e18ac933d89a131f399890098dc1b3d4a8abcdde3eec4a7b191d2521e';
const SERVER_IDENTITY_KEY = '0220529dc803041a83f4357864a09c717daa24397cf2f3fc3a5745ae08d30924fd';

// Encrypted values from your latest logs - using the most recent values
// From the logs: Field 'cool' encrypted value and revelation key
const ENCRYPTED_FIELD_VALUE = 'yP/pPJFN181ib339R+sC2m44QAK60ZX1QALu/C+KtchtC/JGJQmNqHs46hdSc+7CiaRGlA==';
const ENCRYPTED_REVELATION_KEY = 'tKqVRCqcRog47rev9VdBjkFpAPC9LlRC4zCQ+j3yxYoIlFzW6eMH4PIBP1ZTkhgf6e5RB5ry3+rscpwPvcmNqQ9xzzHXTxoqBslYQPTIloI=';

async function testDecryption() {
    console.log('Testing decryption of our Rust wallet\'s encrypted values...\n');

    // Create a test server wallet (simulating the certifier server)
    // Use a known mnemonic for testing - same as test_ts_sdk_server.js
    const serverMnemonic = 'abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about';
    const mnemonic = new Mnemonic(serverMnemonic);
    mnemonic.mnemonic2Seed(); // Generate seed from mnemonic

    // Create HD wallet from seed and get master private key
    const hdWallet = HD.fromSeed(mnemonic.seed);
    const masterPrivateKey = hdWallet.privKey;

    // Create ProtoWallet from private key
    const serverWallet = new ProtoWallet(masterPrivateKey);

    console.log('Server wallet created');
    console.log('Server identity key:', (await serverWallet.getPublicKey({ identityKey: true })).publicKey);
    console.log('Client identity key:', CLIENT_IDENTITY_KEY);
    console.log('');

    // Try to decrypt the revelation key
    console.log('Step 1: Decrypting revelation key...');
    console.log('Encrypted revelation key (base64):', ENCRYPTED_REVELATION_KEY);

    try {
        const encryptionDetails = {
            protocolID: [2, 'certificate field encryption'],
            keyID: 'cool'  // No serial number for master keyring
        };

        console.log('Encryption details:', JSON.stringify(encryptionDetails, null, 2));
        console.log('Counterparty (client identity key):', CLIENT_IDENTITY_KEY);
        console.log('Invoice number: 2-certificate field encryption-cool');

        const { plaintext: revelationKey } = await serverWallet.decrypt({
            ciphertext: Utils.toArray(ENCRYPTED_REVELATION_KEY, 'base64'),
            ...encryptionDetails,
            counterparty: CLIENT_IDENTITY_KEY
        });

        console.log('✅ Revelation key decrypted!');
        console.log('Revelation key (hex):', Utils.toHex(Buffer.from(revelationKey)));
        console.log('Revelation key length:', revelationKey.length, 'bytes');
        console.log('');

        // Try to decrypt the field value using the revelation key
        console.log('Step 2: Decrypting field value using revelation key...');
        console.log('Encrypted field value (base64):', ENCRYPTED_FIELD_VALUE);

        const { SymmetricKey } = sdk;
        const fieldKey = new SymmetricKey(revelationKey);
        const encryptedData = Utils.toArray(ENCRYPTED_FIELD_VALUE, 'base64');
        const decryptedBytes = fieldKey.decrypt(encryptedData);
        const fieldValue = Utils.toUTF8(decryptedBytes);

        console.log('✅ Field value decrypted!');
        console.log('Decrypted value:', fieldValue);
        console.log('Expected: true');

        if (fieldValue === 'true') {
            console.log('\n🎉 SUCCESS! Our encryption is correct!');
        } else {
            console.log('\n❌ FAILED! Decrypted value does not match expected "true"');
            console.log('This means our encryption has a bug.');
        }

    } catch (error) {
        console.error('\n❌ DECRYPTION FAILED!');
        console.error('Error:', error.message);
        console.error('Stack:', error.stack);
        console.log('\nThis means the server cannot decrypt our values, which explains the 500 error.');
    }
}

testDecryption().catch(console.error);
