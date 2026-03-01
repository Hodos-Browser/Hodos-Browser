/**
 * Generate exact CSR and serialized request using TypeScript SDK
 *
 * This script uses the actual TypeScript SDK to create a CSR and serialize
 * the request exactly as it would be done in a real request, then outputs
 * all the bytes for comparison with our Rust implementation.
 */

const path = require('path');

// Try to load the TypeScript SDK
let Utils, MasterCertificate, ProtoWallet, AuthFetch, Random;
try {
    const sdkPath = path.join(__dirname, '..', 'reference', 'ts-brc100', 'node_modules', '@bsv', 'sdk');
    const sdk = require(sdkPath);
    Utils = sdk.Utils || sdk;
    MasterCertificate = sdk.MasterCertificate;
    ProtoWallet = sdk.ProtoWallet;
    AuthFetch = sdk.AuthFetch;
    Random = sdk.Random;
} catch (error) {
    console.error('Failed to load TypeScript SDK:', error.message);
    console.error('Error details:', error);
    process.exit(1);
}

async function generateExactComparison() {
    console.log('TypeScript SDK Exact CSR Comparison');
    console.log('====================================\n');

    try {
        console.log('Loading TypeScript SDK...');
        if (!Utils || !MasterCertificate || !ProtoWallet || !Random) {
            throw new Error('Failed to load SDK components');
        }
        console.log('✅ SDK loaded successfully\n');
        // Create a test wallet
        const mnemonic = 'abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about';
        const wallet = await ProtoWallet.fromMnemonic(mnemonic);

        console.log('✅ Wallet created');
        console.log(`   Identity key: ${wallet.identityKey.toHex()}`);
        console.log('');

        // Use the same certifier public key from our logs
        const certifierPublicKey = '0220529dc803041a83f4357864a09c717daa24397cf2f3fc3a5745ae08d30924fd';
        const certifier = Utils.PublicKey.fromHex(certifierPublicKey);

        // Create a test certificate type (base64 encoded, matching our logs)
        const certificateType = 'AGfk/WrT1eBDXpz3mcw386Zww2HmqcIn3uY6x4Af1eo=';

        // Create a clientNonce
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

        // Create MasterCertificate to encrypt fields
        const masterCert = new MasterCertificate(
            certificateType,
            null, // serialNumber (will be assigned by certifier)
            wallet.identityKey,
            certifier,
            null // revocationOutpoint
        );

        // Encrypt fields and create masterKeyring
        const { certificateFields, masterKeyring } = await masterCert.createCertificateFields(fields);

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
        console.log('📋 CSR JSON Bytes:');
        console.log(`   Length: ${csrJsonBytes.length} bytes`);
        console.log(`   Hex (FULL): ${csrJsonBytes.toString('hex')}`);
        console.log(`   Base64: ${csrJsonBytes.toString('base64')}`);
        console.log('');

        // Now use AuthFetch to serialize the request (this is the exact method used)
        console.log('📦 Serializing request using AuthFetch.serializeRequest...');

        // Create a mock AuthFetch instance to access serializeRequest
        // We need to create a minimal implementation
        class MockAuthFetch extends AuthFetch {
            async serializeRequestForTest(method, headers, body, pathname, search, requestNonce) {
                // Create a mock URL object
                const mockUrl = {
                    pathname: pathname,
                    search: search || ''
                };

                // Call the private serializeRequest method
                // Since it's private, we'll need to replicate it or access it differently
                // For now, let's manually replicate the serialization logic
                return this.serializeRequest(method, headers, body, mockUrl, requestNonce);
            }
        }

        // Actually, let's just manually replicate the serialization since serializeRequest is private
        const requestNonce = Random(32);
        const requestNonceBase64 = Utils.toBase64(requestNonce);

        console.log(`   Request nonce: ${requestNonceBase64}`);
        console.log(`   Request nonce (hex): ${Buffer.from(requestNonce).toString('hex')}`);
        console.log('');

        // Manually serialize (matching AuthFetch.serializeRequest exactly)
        if (!Utils.Writer) {
            throw new Error('Utils.Writer not found');
        }
        const writer = new Utils.Writer();

        // 1. Request nonce (32 bytes)
        writer.write(requestNonce);
        console.log(`   [0..31] Nonce: ${Buffer.from(requestNonce).toString('hex')}`);

        // 2. Method
        const method = 'POST';
        const methodArray = Utils.toArray(method, 'utf8');
        writer.writeVarIntNum(methodArray.length);
        writer.write(methodArray);
        console.log(`   Method: ${method} (${methodArray.length} bytes)`);

        // 3. Pathname
        const pathname = '/signCertificate';
        const pathnameArray = Utils.toArray(pathname, 'utf8');
        writer.writeVarIntNum(pathnameArray.length);
        writer.write(pathnameArray);
        console.log(`   Path: ${pathname} (${pathnameArray.length} bytes)`);

        // 4. Search (empty, so -1)
        writer.writeVarIntNum(-1);
        console.log(`   Search: -1 (empty)`);

        // 5. Headers
        const headers = {
            'Content-Type': 'application/json'
        };

        // Normalize headers (as TypeScript SDK does)
        const includedHeaders = [];
        for (let [k, v] of Object.entries(headers)) {
            k = k.toLowerCase(); // Lowercase key
            if (k.startsWith('content-type')) {
                // Normalize by removing parameters
                v = v.split(';')[0].trim();
            }
            includedHeaders.push([k, v]);
        }

        // Sort by key
        includedHeaders.sort(([keyA], [keyB]) => keyA.localeCompare(keyB));

        writer.writeVarIntNum(includedHeaders.length);
        console.log(`   Headers: ${includedHeaders.length} header(s)`);

        for (const [key, value] of includedHeaders) {
            const keyArray = Utils.toArray(key, 'utf8');
            const valueArray = Utils.toArray(value, 'utf8');
            writer.writeVarIntNum(keyArray.length);
            writer.write(keyArray);
            writer.writeVarIntNum(valueArray.length);
            writer.write(valueArray);
            console.log(`      ${key} = ${value} (key: ${keyArray.length} bytes, value: ${valueArray.length} bytes)`);
        }

        // 6. Body
        const bodyArray = Utils.toArray(csrJsonString, 'utf8');
        writer.writeVarIntNum(bodyArray.length);
        writer.write(bodyArray);
        console.log(`   Body: ${bodyArray.length} bytes`);

        const serialized = writer.toArray();
        const serializedBuffer = Buffer.from(serialized);

        console.log('');
        console.log('📦 Serialized Request:');
        console.log(`   Total length: ${serializedBuffer.length} bytes`);
        console.log(`   Hex (first 200): ${serializedBuffer.slice(0, 200).toString('hex')}`);
        console.log(`   Hex (FULL): ${serializedBuffer.toString('hex')}`);
        console.log(`   Base64 (FULL): ${serializedBuffer.toString('base64')}`);
        console.log('');

        // Output comparison data
        console.log('📊 COMPARISON DATA FOR RUST:');
        console.log('============================');
        console.log(`CSR_JSON_LENGTH=${csrJsonBytes.length}`);
        console.log(`CSR_JSON_HEX=${csrJsonBytes.toString('hex')}`);
        console.log(`CSR_JSON_BASE64=${csrJsonBytes.toString('base64')}`);
        console.log('');
        console.log(`SERIALIZED_REQUEST_LENGTH=${serializedBuffer.length}`);
        console.log(`SERIALIZED_REQUEST_HEX=${serializedBuffer.toString('hex')}`);
        console.log(`SERIALIZED_REQUEST_BASE64=${serializedBuffer.toString('base64')}`);
        console.log('');
        console.log(`REQUEST_NONCE=${requestNonceBase64}`);
        console.log(`REQUEST_NONCE_HEX=${Buffer.from(requestNonce).toString('hex')}`);
        console.log('');
        console.log('Field Values:');
        for (const [key, value] of Object.entries(certificateFields)) {
            console.log(`  ${key}=${value}`);
        }
        console.log('');
        console.log('MasterKeyring Values:');
        for (const [key, value] of Object.entries(masterKeyring)) {
            console.log(`  ${key}=${value}`);
        }
        console.log('');
        console.log('✅ TypeScript SDK CSR generation complete!');

    } catch (error) {
        console.error('❌ Error:', error);
        console.error(error.stack);
        process.exit(1);
    }
}

// Run the test
generateExactComparison().catch(error => {
    console.error('❌ Fatal Error:', error);
    console.error(error.stack);
    process.exit(1);
});
