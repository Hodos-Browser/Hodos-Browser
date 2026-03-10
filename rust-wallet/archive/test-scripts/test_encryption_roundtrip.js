/**
 * Test: Encrypt with our logic, decrypt with TypeScript SDK
 * This will tell us if our encryption is correct
 */

const path = require('path');
const sdkPath = path.join(__dirname, '..', 'reference', 'ts-brc100', 'node_modules', '@bsv', 'sdk');
const sdk = require(sdkPath);
const { Utils, ProtoWallet, MasterCertificate, Mnemonic, HD, PrivateKey, SymmetricKey } = sdk;

async function testEncryptionRoundtrip() {
    console.log('Testing encryption/decryption roundtrip...\n');

    // Create test server wallet
    const serverMnemonic = 'abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about';
    const mnemonic = new Mnemonic(serverMnemonic);
    mnemonic.mnemonic2Seed();
    const hdWallet = HD.fromSeed(mnemonic.seed);
    const serverMasterPrivateKey = hdWallet.privKey;
    const serverWallet = new ProtoWallet(serverMasterPrivateKey);
    const serverPublicKey = (await serverWallet.getPublicKey({ identityKey: true })).publicKey;

    // Create test client wallet (different mnemonic)
    const clientMnemonic = 'zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo wrong';
    const clientMnem = new Mnemonic(clientMnemonic);
    clientMnem.mnemonic2Seed();
    const clientHdWallet = HD.fromSeed(clientMnem.seed);
    const clientMasterPrivateKey = clientHdWallet.privKey;
    const clientWallet = new ProtoWallet(clientMasterPrivateKey);
    const clientPublicKey = (await clientWallet.getPublicKey({ identityKey: true })).publicKey;

    console.log('Server identity key:', serverPublicKey);
    console.log('Client identity key:', clientPublicKey);
    console.log('');

    // Step 1: Encrypt using TypeScript SDK (what we SHOULD match)
    console.log('Step 1: Encrypting using TypeScript SDK...');
    const fieldValue = 'true';
    const fieldName = 'cool';

    // Create certificate fields using SDK
    const certificateFields = await MasterCertificate.createCertificateFields(
        clientWallet,
        serverPublicKey,  // Certifier public key
        { [fieldName]: fieldValue }
    );

    console.log('Certificate fields result:', JSON.stringify(certificateFields, null, 2));
    const encryptedFields = certificateFields.certificateFields || certificateFields.fields;
    const encryptedMasterKeyring = certificateFields.masterKeyring;

    console.log('SDK encrypted field value:', encryptedFields?.[fieldName]);
    console.log('SDK encrypted revelation key:', encryptedMasterKeyring?.[fieldName]);
    console.log('');

    // Step 2: Try to decrypt using server wallet
    console.log('Step 2: Decrypting using server wallet...');
    try {
        const decryptedFields = await MasterCertificate.decryptFields(
            serverWallet,
            encryptedMasterKeyring,
            encryptedFields,
            clientPublicKey  // Client identity key as counterparty
        );

        console.log('✅ Decryption successful!');
        console.log('Decrypted fields:', decryptedFields);

        if (decryptedFields[fieldName] === fieldValue) {
            console.log('\n🎉 SUCCESS! TypeScript SDK encryption/decryption works correctly.');
            console.log('Now we need to match this exact encryption in our Rust code.');
        } else {
            console.log('\n❌ Value mismatch!');
        }
    } catch (error) {
        console.error('❌ Decryption failed:', error.message);
    }
}

testEncryptionRoundtrip().catch(console.error);
