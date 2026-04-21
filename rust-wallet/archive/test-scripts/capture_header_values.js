/**
 * Capture and compare exact header values from TypeScript SDK vs Rust wallet
 *
 * This script:
 * 1. Simulates what TypeScript SDK's AuthFetch sends (by manually constructing headers)
 * 2. Shows the exact format and values expected
 * 3. Provides a template to compare with Rust wallet output
 */

const path = require('path');

// Load TypeScript SDK
let Utils, ProtoWallet, Random, sdk, Mnemonic, HD, MasterCertificate;
try {
    const sdkPath = path.join(__dirname, '..', 'reference', 'ts-brc100', 'node_modules', '@bsv', 'sdk');
    sdk = require(sdkPath);
    Utils = sdk.Utils || sdk;
    MasterCertificate = sdk.MasterCertificate;
    ProtoWallet = sdk.ProtoWallet;
    Random = sdk.Random;

    // Load compat modules
    const compatPath = path.join(__dirname, '..', 'reference', 'ts-brc100', 'node_modules', '@bsv', 'sdk', 'dist', 'cjs', 'src', 'compat');
    const MnemonicModule = require(path.join(compatPath, 'Mnemonic.js'));
    Mnemonic = MnemonicModule.default || MnemonicModule;
    const HDModule = require(path.join(compatPath, 'HD.js'));
    HD = HDModule.default || HDModule;
} catch (error) {
    console.error('Failed to load TypeScript SDK:', error.message);
    process.exit(1);
}

/**
 * Simulate what AuthFetch/Peer.toPeer() generates for headers
 */
async function generateExpectedHeaders() {
    console.log('🔍 Generating Expected Header Values (TypeScript SDK)');
    console.log('='.repeat(70));
    console.log('');

    // Create wallet (same as Rust test)
    const mnemonicStr = 'abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about';
    const mnemonic = new Mnemonic(mnemonicStr);
    mnemonic.mnemonic2Seed();
    const hdWallet = HD.fromSeed(mnemonic.seed);
    const wallet = new ProtoWallet(hdWallet.privKey);

    const identityKeyResult = await wallet.getPublicKey({ identityKey: true });
    const identityKey = identityKeyResult.publicKey;

    console.log('📋 Test Parameters:');
    console.log(`   Wallet identity key: ${identityKey}`);

    // Certifier (server) public key
    const certifierPublicKey = '0220529dc803041a83f4357864a09c717daa24397cf2f3fc3a5745ae08d30924fd';
    console.log(`   Certifier public key: ${certifierPublicKey}`);
    console.log('');

    // Simulate initialRequest/initialResponse flow
    console.log('🔧 Simulating initialRequest/initialResponse...');

    // Client nonce (from initialRequest)
    const clientNonce = Utils.toBase64(Random(32));
    console.log(`   Client nonce (initialRequest): ${clientNonce}`);
    console.log(`   Client nonce length: ${clientNonce.length} chars (base64, 32 bytes)`);

    // Server nonce (from initialResponse)
    const serverNonce = Utils.toBase64(Random(32));
    console.log(`   Server nonce (initialResponse): ${serverNonce}`);
    console.log(`   Server nonce length: ${serverNonce.length} chars (base64, 32 bytes)`);
    console.log('');

    // Now simulate the /signCertificate request
    console.log('🔧 Simulating /signCertificate request...');

    // 1. Generate request nonce (for serialized request - first 32 bytes)
    const requestNonce = Random(32);
    const requestNonceBase64 = Utils.toBase64(requestNonce);
    console.log(`   Request nonce (for serialization): ${requestNonceBase64}`);
    console.log(`   Request nonce (hex): ${Buffer.from(requestNonce).toString('hex')}`);
    console.log(`   Request nonce length: ${requestNonceBase64.length} chars`);
    console.log('');

    // 2. Generate signing nonce (for keyID - separate from request nonce!)
    // This is what Peer.toPeer() uses for the keyID
    const signingNonce = Utils.toBase64(Random(32));
    console.log(`   Signing nonce (for keyID): ${signingNonce}`);
    console.log(`   Signing nonce length: ${signingNonce.length} chars`);
    console.log(`   ⚠️  NOTE: Signing nonce is DIFFERENT from request nonce!`);
    console.log('');

    // 3. Create CSR body
    const certificateType = 'AGfk/WrT1eBDXpz3mcw386Zww2HmqcIn3uY6x4Af1eo=';
    const { certificateFields, masterKeyring } = await MasterCertificate.createCertificateFields(
        wallet,
        certifierPublicKey,
        { cool: true }
    );

    const csrBody = {
        clientNonce,  // Original client nonce from initialRequest
        type: certificateType,
        fields: certificateFields,
        masterKeyring: masterKeyring
    };

    const csrBodyString = JSON.stringify(csrBody);
    console.log(`   CSR body length: ${csrBodyString.length} bytes`);
    console.log('');

    // 4. Serialize request (as AuthFetch does)
    console.log('🔧 Serializing request (as AuthFetch.serializeRequest)...');

    const writer = new Utils.Writer();

    // Write request nonce (32 bytes)
    writer.write(requestNonce);

    // Write method
    const method = 'POST';
    const methodBytes = Utils.toArray(method, 'utf8');
    writer.writeVarIntNum(methodBytes.length);
    writer.write(methodBytes);

    // Write pathname
    const pathname = '/signCertificate';
    const pathnameBytes = Utils.toArray(pathname, 'utf8');
    writer.writeVarIntNum(pathnameBytes.length);
    writer.write(pathnameBytes);

    // Write search (-1 for empty)
    writer.writeVarIntNum(-1);

    // Write headers (only content-type, normalized)
    const headers = [['content-type', 'application/json']];
    writer.writeVarIntNum(headers.length);
    for (const [key, value] of headers) {
        const keyBytes = Utils.toArray(key.toLowerCase(), 'utf8');
        const valueBytes = Utils.toArray(value.toLowerCase(), 'utf8');
        writer.writeVarIntNum(keyBytes.length);
        writer.write(keyBytes);
        writer.writeVarIntNum(valueBytes.length);
        writer.write(valueBytes);
    }

    // Write body
    const bodyBytes = Utils.toArray(csrBodyString, 'utf8');
    writer.writeVarIntNum(bodyBytes.length);
    writer.write(bodyBytes);

    const serializedRequest = writer.toArray();
    const serializedRequestBuffer = Buffer.from(serializedRequest);

    console.log(`   Serialized request length: ${serializedRequestBuffer.length} bytes`);
    console.log(`   Serialized request (hex, first 100): ${serializedRequestBuffer.slice(0, 100).toString('hex')}...`);
    console.log('');

    // 5. Calculate request ID (first 32 bytes of serialized request, base64)
    const requestIdBytes = serializedRequestBuffer.slice(0, 32);
    const requestIdBase64 = Utils.toBase64(requestIdBytes);
    console.log(`   Request ID (first 32 bytes, base64): ${requestIdBase64}`);
    console.log(`   Request ID (hex): ${requestIdBytes.toString('hex')}`);
    console.log(`   Request ID length: ${requestIdBase64.length} chars`);
    console.log('');

    // 6. Create signature (simulate Peer.toPeer() signing)
    console.log('🔧 Creating signature (simulating Peer.toPeer())...');

    // KeyID format: "{signingNonce} {serverNonce}"
    const keyID = `${signingNonce} ${serverNonce}`;
    console.log(`   KeyID: "${keyID}"`);
    console.log(`   KeyID length: ${keyID.length} chars`);
    console.log('');

    // ProtocolID: [2, 'auth message signature']
    const protocolID = [2, 'auth message signature'];
    console.log(`   ProtocolID: [${protocolID[0]}, '${protocolID[1]}']`);
    console.log('');

    // Invoice number (BRC-42 key derivation)
    // This would be calculated using BRC-42 with the certifier as counterparty
    console.log(`   Counterparty (for BRC-42): ${certifierPublicKey}`);
    console.log(`   Invoice number: (calculated via BRC-42 key derivation)`);
    console.log('');

    // Hash the serialized request
    const Hash = sdk.Hash || Utils.Hash;
    const requestHash = Hash.sha256(serializedRequest);
    console.log(`   Request hash (SHA256): ${Utils.toHex(requestHash)}`);
    console.log(`   Request hash length: ${requestHash.length} bytes`);
    console.log('');

    // 7. Generate expected headers
    console.log('📋 EXPECTED HEADERS (TypeScript SDK Format):');
    console.log('='.repeat(70));
    console.log('');

    const expectedHeaders = {
        'x-bsv-auth-version': '0.1',
        'x-bsv-auth-identity-key': identityKey,
        'x-bsv-auth-nonce': signingNonce,  // Signing nonce (for keyID), NOT request nonce!
        'x-bsv-auth-your-nonce': serverNonce,  // Server's nonce from initialResponse
        'x-bsv-auth-request-id': requestIdBase64,  // First 32 bytes of serialized request
        'x-bsv-auth-signature': 'SIGNATURE_HEX_HERE'  // Would be actual signature
    };

    console.log('Header Name                    | Value');
    console.log('-'.repeat(70));
    for (const [name, value] of Object.entries(expectedHeaders)) {
        const displayValue = value.length > 60 ? value.substring(0, 60) + '...' : value;
        console.log(`${name.padEnd(30)} | ${displayValue}`);
    }
    console.log('');

    // 8. Show format details
    console.log('📋 HEADER VALUE FORMATS:');
    console.log('='.repeat(70));
    console.log('');
    console.log('x-bsv-auth-version:');
    console.log('   Type: String');
    console.log('   Value: "0.1"');
    console.log('   Format: Plain string, no encoding');
    console.log('');
    console.log('x-bsv-auth-identity-key:');
    console.log('   Type: String (hex)');
    console.log('   Value: Public key in hex format (66 chars for compressed)');
    console.log(`   Example: ${identityKey}`);
    console.log(`   Length: ${identityKey.length} chars`);
    console.log('');
    console.log('x-bsv-auth-nonce:');
    console.log('   Type: String (base64)');
    console.log('   Value: Signing nonce (for keyID), NOT request nonce!');
    console.log(`   Example: ${signingNonce}`);
    console.log(`   Length: ${signingNonce.length} chars (base64 of 32 bytes)`);
    console.log('   ⚠️  CRITICAL: This is the signing nonce, not the request nonce!');
    console.log('');
    console.log('x-bsv-auth-your-nonce:');
    console.log('   Type: String (base64)');
    console.log('   Value: Server\'s nonce from initialResponse');
    console.log(`   Example: ${serverNonce}`);
    console.log(`   Length: ${serverNonce.length} chars (base64 of 32 bytes)`);
    console.log('');
    console.log('x-bsv-auth-request-id:');
    console.log('   Type: String (base64)');
    console.log('   Value: First 32 bytes of serialized request (base64)');
    console.log(`   Example: ${requestIdBase64}`);
    console.log(`   Length: ${requestIdBase64.length} chars (base64 of 32 bytes)`);
    console.log('   ⚠️  CRITICAL: This must match the first 32 bytes of the serialized request!');
    console.log('');
    console.log('x-bsv-auth-signature:');
    console.log('   Type: String (hex)');
    console.log('   Value: DER-encoded ECDSA signature of serialized request');
    console.log('   Format: Hex string (no 0x prefix)');
    console.log('   Length: Variable (typically 70-72 chars for DER signature)');
    console.log('');

    // 9. Comparison checklist
    console.log('✅ COMPARISON CHECKLIST:');
    console.log('='.repeat(70));
    console.log('');
    console.log('Compare your Rust wallet headers with the above:');
    console.log('');
    console.log('1. x-bsv-auth-version:');
    console.log('   ✓ Should be exactly "0.1" (string)');
    console.log('');
    console.log('2. x-bsv-auth-identity-key:');
    console.log('   ✓ Should be your wallet\'s identity key (hex, 66 chars)');
    console.log(`   ✓ Should match: ${identityKey}`);
    console.log('');
    console.log('3. x-bsv-auth-nonce:');
    console.log('   ✓ Should be base64 string (44 chars)');
    console.log('   ✓ Should be the SIGNING nonce (for keyID), NOT request nonce');
    console.log('   ✓ KeyID format: "{x-bsv-auth-nonce} {x-bsv-auth-your-nonce}"');
    console.log('');
    console.log('4. x-bsv-auth-your-nonce:');
    console.log('   ✓ Should be base64 string (44 chars)');
    console.log('   ✓ Should match server\'s nonce from initialResponse');
    console.log('');
    console.log('5. x-bsv-auth-request-id:');
    console.log('   ✓ Should be base64 string (44 chars)');
    console.log('   ✓ Should be first 32 bytes of serialized request (base64)');
    console.log('   ✓ Must match: base64(serialized_request[0..31])');
    console.log('');
    console.log('6. x-bsv-auth-signature:');
    console.log('   ✓ Should be hex string (no 0x prefix)');
    console.log('   ✓ Should be DER-encoded ECDSA signature');
    console.log('   ✓ Should sign: SHA256(serialized_request)');
    console.log('');

    return {
        expectedHeaders,
        requestNonceBase64,
        signingNonce,
        serverNonce,
        requestIdBase64,
        identityKey,
        serializedRequestBuffer
    };
}

// Run
generateExpectedHeaders().then(result => {
    console.log('='.repeat(70));
    console.log('✅ Header value generation complete!');
    console.log('');
    console.log('Next steps:');
    console.log('1. Check your Rust wallet logs for the actual header values');
    console.log('2. Compare each header value with the expected format above');
    console.log('3. Pay special attention to:');
    console.log('   - x-bsv-auth-nonce (should be signing nonce, not request nonce)');
    console.log('   - x-bsv-auth-request-id (must match first 32 bytes of serialized request)');
    console.log('');
}).catch(error => {
    console.error('❌ Error:', error);
    console.error(error.stack);
    process.exit(1);
});
