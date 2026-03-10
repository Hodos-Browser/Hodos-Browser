/**
 * Side-by-side comparison: TypeScript SDK vs what we think it does
 * This will show us EXACTLY what the SDK does so we can match it
 */

const path = require('path');
const sdkPath = path.join(__dirname, '..', 'reference', 'ts-brc100', 'node_modules', '@bsv', 'sdk');
const sdk = require(sdkPath);
const { Utils, ProtoWallet, MasterCertificate, Mnemonic, HD, SymmetricKey } = sdk;

async function sideBySideTest() {
    console.log('='.repeat(80));
    console.log('SIDE-BY-SIDE COMPARISON: TypeScript SDK Encryption');
    console.log('='.repeat(80));
    console.log('');

    // Setup wallets
    const serverMnemonic = 'abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about';
    const serverMnem = new Mnemonic(serverMnemonic);
    serverMnem.mnemonic2Seed();
    const serverHdWallet = HD.fromSeed(serverMnem.seed);
    const serverMasterPrivateKey = serverHdWallet.privKey;
    const serverWallet = new ProtoWallet(serverMasterPrivateKey);
    const serverPublicKey = (await serverWallet.getPublicKey({ identityKey: true })).publicKey;

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

    const fieldName = 'cool';
    const fieldValue = 'true';

    // Use SDK to encrypt
    console.log('STEP 1: TypeScript SDK Encryption');
    console.log('-'.repeat(80));
    const certificateFields = await MasterCertificate.createCertificateFields(
        clientWallet,
        serverPublicKey,
        { [fieldName]: fieldValue }
    );

    const encryptedField = certificateFields.certificateFields[fieldName];
    const encryptedRevelationKey = certificateFields.masterKeyring[fieldName];

    console.log('Field value:', fieldValue);
    console.log('Field name:', fieldName);
    console.log('Encrypted field (base64):', encryptedField);
    console.log('Encrypted revelation key (base64):', encryptedRevelationKey);
    console.log('');

    // Now manually decrypt to see the steps
    console.log('STEP 2: Manual Decryption (to see what SDK does internally)');
    console.log('-'.repeat(80));

    // Decrypt revelation key
    console.log('Decrypting revelation key...');
    const encryptionDetails = {
        protocolID: [2, 'certificate field encryption'],
        keyID: fieldName
    };

    console.log('Invoice number: 2-certificate field encryption-' + fieldName);
    console.log('Counterparty (client):', clientPublicKey);

    const { plaintext: revelationKey } = await serverWallet.decrypt({
        ciphertext: Utils.toArray(encryptedRevelationKey, 'base64'),
        ...encryptionDetails,
        counterparty: clientPublicKey
    });

    console.log('Revelation key decrypted!');
    console.log('Revelation key (hex):', Utils.toHex(Buffer.from(revelationKey)));
    console.log('Revelation key (base64):', Buffer.from(revelationKey).toString('base64'));
    console.log('Revelation key length:', revelationKey.length, 'bytes');
    console.log('');

    // Decrypt field value
    console.log('Decrypting field value...');
    const fieldKey = new SymmetricKey(revelationKey);
    const encryptedData = Utils.toArray(encryptedField, 'base64');
    const decryptedBytes = fieldKey.decrypt(encryptedData);
    const decryptedValue = Utils.toUTF8(decryptedBytes);

    console.log('Field value decrypted!');
    console.log('Decrypted value:', decryptedValue);
    console.log('');

    console.log('='.repeat(80));
    console.log('SUMMARY FOR RUST IMPLEMENTATION:');
    console.log('='.repeat(80));
    console.log('1. Invoice number format: "2-certificate field encryption-' + fieldName + '"');
    console.log('2. Revelation key length:', revelationKey.length, 'bytes');
    console.log('3. Revelation key (hex):', Utils.toHex(Buffer.from(revelationKey)));
    console.log('4. Field value:', fieldValue);
    console.log('5. Encrypted field format: IV (32 bytes) + ciphertext + tag (16 bytes)');
    console.log('6. Encrypted revelation key format: IV (32 bytes) + ciphertext + tag (16 bytes)');
    console.log('');
    console.log('Now compare these with your Rust logs!');
}

sideBySideTest().catch(console.error);
