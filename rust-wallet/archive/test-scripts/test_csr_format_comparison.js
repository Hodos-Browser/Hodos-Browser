/**
 * Test CSR JSON format comparison
 *
 * This script creates a CSR JSON using the same approach as TypeScript SDK
 * and outputs it for comparison with our Rust implementation.
 */

const path = require('path');

// Try to load the TypeScript SDK
let Utils, MasterCertificate, ProtoWallet, createNonce;
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
} catch (error) {
    console.error('Failed to load TypeScript SDK:', error.message);
    process.exit(1);
}

async function testCSRFormat() {
    console.log('CSR Format Comparison Test');
    console.log('==========================\n');

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
    console.log('Client Nonce length:', clientNonce.length, 'chars');
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

    // Check field order
    console.log('Field order in JSON:');
    const fieldOrder = [];
    let inString = false;
    let currentField = '';
    for (let i = 0; i < csrJsonString.length; i++) {
        const char = csrJsonString[i];
        if (char === '"' && csrJsonString[i-1] !== '\\') {
            if (!inString) {
                inString = true;
                currentField = '';
            } else {
                inString = false;
                if (currentField && !currentField.includes(':')) {
                    // Check if this looks like a field name (followed by :)
                    if (csrJsonString.substring(i+1, i+5).trim() === ':') {
                        fieldOrder.push(currentField);
                    }
                }
                currentField = '';
            }
        } else if (inString) {
            currentField += char;
        }
    }
    console.log('  Order:', fieldOrder.join(' -> '));
    console.log('');

    // Parse and show structure
    const parsed = JSON.parse(csrJsonString);
    console.log('Parsed CSR structure:');
    console.log('  clientNonce:', parsed.clientNonce);
    console.log('  type:', parsed.type);
    console.log('  fields:', Object.keys(parsed.fields));
    console.log('  masterKeyring:', Object.keys(parsed.masterKeyring));
    console.log('');

    // Compare with expected Rust output
    console.log('Expected from Rust (from logs):');
    console.log('  Field order: clientNonce -> type -> fields -> masterKeyring');
    console.log('  All fields should be present');
    console.log('  JSON should be compact (no whitespace)');
}

testCSRFormat().catch(console.error);

