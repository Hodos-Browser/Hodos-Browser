/**
 * Generate CSR using TypeScript SDK for comparison with Rust implementation
 *
 * This script creates a CSR exactly as the TypeScript SDK does and outputs
 * the exact JSON string, field order, and byte representation for comparison.
 */

const path = require('path');

// Try to load the TypeScript SDK
let Utils, MasterCertificate, ProtoWallet, createNonce, AuthFetch;
try {
    const sdkPath = path.join(__dirname, '..', 'reference', 'ts-brc100', 'node_modules', '@bsv', 'sdk');
    const sdk = require(sdkPath);
    Utils = sdk.Utils || sdk;
    MasterCertificate = sdk.MasterCertificate;
    ProtoWallet = sdk.ProtoWallet;
    createNonce = sdk.createNonce || (async (wallet, counterparty) => {
        const Random = sdk.Random;
        return Utils.toBase64(Random(32));
    });
    AuthFetch = sdk.AuthFetch;
} catch (error) {
    console.error('Failed to load TypeScript SDK:', error.message);
    process.exit(1);
}

async function generateCSR() {
    console.log('TypeScript SDK CSR Generation Test');
    console.log('==================================\n');

    // Create a test wallet (using a dummy private key)
    const testPrivateKey = '42'.repeat(64); // 128 hex chars = 64 bytes
    const wallet = new ProtoWallet(testPrivateKey);

    // Test values matching our Rust implementation
    const type = 'AGfk/WrT1eBDXpz3mcw386Zww2HmqcIn3uY6x4Af1eo=';
    const certifier = '0220529dc803041a83f4357864a09c717daa24397cf2f3fc3a5745ae08d30924fd';
    const fields = {
        cool: 'true'
    };

    // Create clientNonce (matching TypeScript SDK's createNonce)
    const clientNonce = await createNonce(wallet, certifier);
    console.log('Client Nonce:', clientNonce);
    console.log('');

    // Create certificate fields and masterKeyring (matching TypeScript SDK)
    const { certificateFields, masterKeyring } = await MasterCertificate.createCertificateFields(
        wallet,
        certifier,
        fields
    );

    console.log('Certificate Fields:');
    for (const [key, value] of Object.entries(certificateFields)) {
        console.log(`  ${key}: ${value.substring(0, 50)}... (${value.length} chars)`);
    }
    console.log('');

    console.log('Master Keyring:');
    for (const [key, value] of Object.entries(masterKeyring)) {
        console.log(`  ${key}: ${value.substring(0, 50)}... (${value.length} chars)`);
    }
    console.log('');

    // Create CSR JSON exactly as TypeScript SDK does (line 506-511 in Wallet.ts)
    const csr = {
        clientNonce,
        type,
        fields: certificateFields,
        masterKeyring
    };

    // Serialize using JSON.stringify (matching TypeScript SDK)
    const csrJsonString = JSON.stringify(csr);

    console.log('CSR JSON (TypeScript SDK):');
    console.log(csrJsonString);
    console.log('');
    console.log('CSR JSON length:', csrJsonString.length, 'bytes');
    console.log('CSR JSON (hex):', Buffer.from(csrJsonString, 'utf8').toString('hex'));
    console.log('CSR JSON (base64):', Buffer.from(csrJsonString, 'utf8').toString('base64'));
    console.log('');

    // Verify field order by parsing and checking
    const parsed = JSON.parse(csrJsonString);
    const topLevelKeys = Object.keys(parsed);
    console.log('Top-level field order:', topLevelKeys.join(' -> '));
    console.log('Expected order: clientNonce -> type -> fields -> masterKeyring');
    console.log('Match:', JSON.stringify(topLevelKeys) === JSON.stringify(['clientNonce', 'type', 'fields', 'masterKeyring']) ? '✅' : '❌');
    console.log('');

    // Check nested object key order
    const fieldsKeys = Object.keys(parsed.fields);
    const masterKeyringKeys = Object.keys(parsed.masterKeyring);
    console.log('Fields object keys:', fieldsKeys);
    console.log('MasterKeyring object keys:', masterKeyringKeys);
    console.log('');

    // Show byte-by-byte comparison info
    console.log('=== For Rust Comparison ===');
    console.log('Copy this CSR JSON to compare with Rust output:');
    console.log(csrJsonString);
    console.log('');
    console.log('Expected byte length:', csrJsonString.length);
    console.log('Expected field order: clientNonce, type, fields, masterKeyring');
}

generateCSR().catch(console.error);

