/**
 * Generate CSR using TypeScript SDK and output exact bytes for comparison
 *
 * This script creates a CSR exactly as the TypeScript SDK does and outputs
 * the exact serialized request bytes, CSR JSON bytes, and all components
 * for byte-for-byte comparison with our Rust implementation.
 */

const path = require('path');
const fs = require('fs');

// Try to load the TypeScript SDK
let Utils, MasterCertificate, ProtoWallet, createNonce, AuthFetch, Random, sdk, Mnemonic, HD;
try {
    const sdkPath = path.join(__dirname, '..', 'reference', 'ts-brc100', 'node_modules', '@bsv', 'sdk');
    sdk = require(sdkPath);
    Utils = sdk.Utils || sdk;
    MasterCertificate = sdk.MasterCertificate;
    ProtoWallet = sdk.ProtoWallet;
    createNonce = sdk.createNonce || (async (wallet, counterparty) => {
        const Random = sdk.Random;
        return Utils.toBase64(Random(32));
    });
    AuthFetch = sdk.AuthFetch;
    Random = sdk.Random;

    // Load compat modules for mnemonic/HD wallet
    const compatPath = path.join(__dirname, '..', 'reference', 'ts-brc100', 'node_modules', '@bsv', 'sdk', 'dist', 'cjs', 'src', 'compat');
    const MnemonicModule = require(path.join(compatPath, 'Mnemonic.js'));
    Mnemonic = MnemonicModule.default || MnemonicModule;
    const HDModule = require(path.join(compatPath, 'HD.js'));
    HD = HDModule.default || HDModule;
} catch (error) {
    console.error('Failed to load TypeScript SDK:', error.message);
    console.error(error.stack);
    process.exit(1);
}

async function generateCSRForComparison() {
    console.log('TypeScript SDK CSR Generation for Comparison');
    console.log('============================================\n');

    // Create a test wallet (we'll use known keys to match our Rust test)
    const mnemonicStr = 'abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about';
    // Use the same approach as test_ts_sdk_server.js
    const mnemonic = new Mnemonic(mnemonicStr);
    mnemonic.mnemonic2Seed(); // Generate seed from mnemonic
    const hdWallet = HD.fromSeed(mnemonic.seed);
    const masterPrivateKey = hdWallet.privKey;
    const wallet = new ProtoWallet(masterPrivateKey);

    // Use the same certifier public key from our logs (as hex string, not PublicKey object)
    const certifierPublicKey = '0220529dc803041a83f4357864a09c717daa24397cf2f3fc3a5745ae08d30924fd';
    // MasterCertificate.createCertificateFields accepts hex string for certifier
    const certifier = certifierPublicKey;

    // Create a test certificate type (base64 encoded, matching our logs)
    const certificateType = 'AGfk/WrT1eBDXpz3mcw386Zww2HmqcIn3uY6x4Af1eo=';

    // Create a clientNonce (matching format from our logs)
    const clientNonce = Utils.toBase64(Random(32));

    console.log('📋 Test Parameters:');
    console.log(`   Certifier public key: ${certifierPublicKey}`);
    console.log(`   Certificate type: ${certificateType}`);
    console.log(`   Client nonce: ${clientNonce}`);
    console.log('');

    // Create certificate fields
    const fields = {
        cool: true
    };

    console.log('🔐 Creating certificate fields and masterKeyring...');

    // Get wallet identity key as hex string
    const identityKeyResult = await wallet.getPublicKey({ identityKey: true });
    const identityKeyHex = identityKeyResult.publicKey;

    // Encrypt fields and create masterKeyring (MasterCertificate.createCertificateFields is a static method)
    // It takes: wallet, certifier (hex string), fields
    const { certificateFields, masterKeyring } = await MasterCertificate.createCertificateFields(
        wallet,
        certifier,
        fields
    );

    console.log('✅ Certificate fields and masterKeyring created');
    console.log('');

    // Build CSR JSON (exactly as TypeScript SDK does)
    const csr = {
        clientNonce: clientNonce,
        type: certificateType,
        fields: certificateFields,
        masterKeyring: masterKeyring
    };

    const csrJsonString = JSON.stringify(csr);
    const csrJsonBytes = Buffer.from(csrJsonString, 'utf8');

    console.log('📋 CSR JSON:');
    console.log(csrJsonString);
    console.log('');
    console.log('📋 CSR JSON Details:');
    console.log(`   Length: ${csrJsonBytes.length} bytes`);
    console.log(`   Hex (first 200): ${csrJsonBytes.slice(0, 200).toString('hex')}`);
    console.log(`   Hex (full): ${csrJsonBytes.toString('hex')}`);
    console.log(`   Base64: ${csrJsonBytes.toString('base64')}`);
    console.log('');

    // Now simulate the request serialization (as AuthFetch does)
    console.log('📦 Serializing request for signing (as AuthFetch does)...');

    // Generate request nonce
    const requestNonce = Random(32);
    const requestNonceBase64 = Utils.toBase64(requestNonce);

    console.log(`   Request nonce: ${requestNonceBase64}`);
    console.log(`   Request nonce (hex): ${requestNonce.toString('hex')}`);
    console.log('');

    // Serialize request (matching AuthFetch.serializeRequest)
    const method = 'POST';
    const pathname = '/signCertificate';
    const search = ''; // Empty search params
    const headers = {
        'content-type': 'application/json'
    };
    const body = csrJsonBytes;

    // Build serialized request
    const serialized = [];

    // 1. Request nonce (32 bytes)
    serialized.push(...requestNonce);
    console.log(`   [0..31] Nonce: ${requestNonce.toString('hex')}`);

    // 2. Method (VarInt length + string)
    const methodBytes = Buffer.from(method, 'utf8');
    const methodVarInt = writeVarIntNum(methodBytes.length);
    serialized.push(...methodVarInt);
    serialized.push(...methodBytes);
    console.log(`   [${serialized.length - methodBytes.length - methodVarInt.length}..${serialized.length - methodBytes.length - 1}] Method VarInt (${methodVarInt.length} bytes): ${Buffer.from(methodVarInt).toString('hex')}`);
    console.log(`   [${serialized.length - methodBytes.length}..${serialized.length - 1}] Method: ${method}`);

    // 3. Path (VarInt length + string)
    const pathBytes = Buffer.from(pathname, 'utf8');
    const pathVarInt = writeVarIntNum(pathBytes.length);
    serialized.push(...pathVarInt);
    serialized.push(...pathBytes);
    console.log(`   [${serialized.length - pathBytes.length - pathVarInt.length}..${serialized.length - pathBytes.length - 1}] Path VarInt (${pathVarInt.length} bytes): ${Buffer.from(pathVarInt).toString('hex')}`);
    console.log(`   [${serialized.length - pathBytes.length}..${serialized.length - 1}] Path: ${pathname}`);

    // 4. Search (VarInt for -1 if empty)
    const searchVarInt = writeVarIntNum(-1);
    serialized.push(...searchVarInt);
    console.log(`   [${serialized.length - searchVarInt.length}..${serialized.length - 1}] Search VarInt (-1, ${searchVarInt.length} bytes): ${Buffer.from(searchVarInt).toString('hex')}`);

    // 5. Headers (VarInt count + header pairs)
    const headerKeys = Object.keys(headers).sort(); // Headers are sorted by key
    const headerCountVarInt = writeVarIntNum(headerKeys.length);
    serialized.push(...headerCountVarInt);
    console.log(`   [${serialized.length - headerCountVarInt.length}..${serialized.length - 1}] Header count VarInt (${headerCountVarInt.length} bytes): ${Buffer.from(headerCountVarInt).toString('hex')}`);

    for (const key of headerKeys) {
        const value = headers[key];
        const keyBytes = Buffer.from(key.toLowerCase(), 'utf8'); // Keys are lowercased
        const valueBytes = Buffer.from(value.toLowerCase(), 'utf8'); // Values are lowercased
        const keyVarInt = writeVarIntNum(keyBytes.length);
        const valueVarInt = writeVarIntNum(valueBytes.length);

        serialized.push(...keyVarInt);
        serialized.push(...keyBytes);
        serialized.push(...valueVarInt);
        serialized.push(...valueBytes);

        console.log(`      Header: ${key} = ${value} (key: ${keyBytes.length} bytes, value: ${valueBytes.length} bytes)`);
    }

    // 6. Body (VarInt length + bytes)
    const bodyVarInt = writeVarIntNum(body.length);
    serialized.push(...bodyVarInt);
    serialized.push(...body);
    console.log(`   [${serialized.length - body.length - bodyVarInt.length}..${serialized.length - body.length - 1}] Body length VarInt (${bodyVarInt.length} bytes): ${Buffer.from(bodyVarInt).toString('hex')}`);
    console.log(`   [${serialized.length - body.length}..${serialized.length - 1}] Body (${body.length} bytes)`);

    const serializedBuffer = Buffer.from(serialized);

    console.log('');
    console.log('📦 Serialized Request:');
    console.log(`   Total length: ${serializedBuffer.length} bytes`);
    console.log(`   Hex (first 200): ${serializedBuffer.slice(0, 200).toString('hex')}`);
    console.log(`   Hex (FULL): ${serializedBuffer.toString('hex')}`);
    console.log(`   Base64 (FULL): ${serializedBuffer.toString('base64')}`);
    console.log('');

    // Output for comparison
    console.log('📊 COMPARISON DATA:');
    console.log('===================');
    console.log(`CSR JSON Length: ${csrJsonBytes.length}`);
    console.log(`CSR JSON Hex: ${csrJsonBytes.toString('hex')}`);
    console.log(`CSR JSON Base64: ${csrJsonBytes.toString('base64')}`);
    console.log('');
    console.log(`Serialized Request Length: ${serializedBuffer.length}`);
    console.log(`Serialized Request Hex: ${serializedBuffer.toString('hex')}`);
    console.log(`Serialized Request Base64: ${serializedBuffer.toString('base64')}`);
    console.log('');
    console.log(`Request Nonce: ${requestNonceBase64}`);
    console.log(`Request Nonce Hex: ${requestNonce.toString('hex')}`);
    console.log('');
    console.log('Field Values:');
    for (const [key, value] of Object.entries(certificateFields)) {
        console.log(`  ${key}: ${value}`);
    }
    console.log('');
    console.log('MasterKeyring Values:');
    for (const [key, value] of Object.entries(masterKeyring)) {
        console.log(`  ${key}: ${value}`);
    }
}

// Helper function to write VarInt (matching TypeScript SDK's writeVarIntNum)
function writeVarIntNum(n) {
    if (n < 0) {
        // Negative numbers: 0xFF followed by 8 bytes of the number
        const bytes = Buffer.allocUnsafe(9);
        bytes[0] = 0xFF;
        // Convert to unsigned 64-bit (two's complement)
        const unsigned = BigInt(n) + BigInt('0x10000000000000000');
        // Write as big-endian 64-bit
        for (let i = 0; i < 8; i++) {
            bytes[8 - i] = Number((unsigned >> BigInt(i * 8)) & BigInt(0xFF));
        }
        return Array.from(bytes);
    } else if (n < 0xFD) {
        return [n];
    } else if (n < 0x10000) {
        return [0xFD, n & 0xFF, (n >> 8) & 0xFF];
    } else if (n < 0x100000000) {
        const bytes = [0xFE];
        for (let i = 0; i < 4; i++) {
            bytes.push((n >> (i * 8)) & 0xFF);
        }
        return bytes;
    } else {
        const bytes = [0xFF];
        for (let i = 0; i < 8; i++) {
            bytes.push(Number((BigInt(n) >> BigInt(i * 8)) & BigInt(0xFF)));
        }
        return bytes;
    }
}

// Run the test
generateCSRForComparison().catch(error => {
    console.error('Error:', error);
    process.exit(1);
});
