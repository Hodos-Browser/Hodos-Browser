/**
 * Test CSR Request Serialization
 *
 * This script serializes a CSR request using the TypeScript SDK's AuthFetch
 * and outputs the serialized bytes for comparison with our Rust implementation.
 */

const path = require('path');
const fs = require('fs');

// Load TypeScript SDK
let Utils, Random, Writer;
try {
    const sdkPath = path.join(__dirname, '..', 'reference', 'ts-brc100', 'node_modules', '@bsv', 'sdk');
    const sdk = require(sdkPath);
    Utils = sdk.Utils || sdk;
    Random = sdk.Random;
    Writer = Utils.Writer;
} catch (error) {
    console.error('Failed to load TypeScript SDK:', error.message);
    process.exit(1);
}

// Mock wallet for testing (we only need serialization, not signing)
class MockWallet {
    async getPublicKey() {
        return { publicKey: '03' + '0'.repeat(64) }; // Dummy public key
    }
}

// Simulate AuthFetch.serializeRequest method
async function serializeRequest(method, headers, body, parsedUrl, requestNonce) {
    const writer = new Writer();

    // Write request nonce
    writer.write(requestNonce);

    // Method length
    writer.writeVarIntNum(method.length);
    // Method
    writer.write(Utils.toArray(method));

    // Handle pathname
    if (parsedUrl.pathname.length > 0) {
        const pathnameAsArray = Utils.toArray(parsedUrl.pathname);
        writer.writeVarIntNum(pathnameAsArray.length);
        writer.write(pathnameAsArray);
    } else {
        writer.writeVarIntNum(-1);
    }

    // Handle search params
    if (parsedUrl.search.length > 0) {
        const searchAsArray = Utils.toArray(parsedUrl.search);
        writer.writeVarIntNum(searchAsArray.length);
        writer.write(searchAsArray);
    } else {
        writer.writeVarIntNum(-1);
    }

    // Construct headers to send / sign
    const includedHeaders = [];
    for (let [k, v] of Object.entries(headers)) {
        k = k.toLowerCase();
        if (k.startsWith('x-bsv-') || k === 'authorization') {
            if (k.startsWith('x-bsv-auth')) {
                throw new Error('No BSV auth headers allowed here!');
            }
            includedHeaders.push([k, v]);
        } else if (k.startsWith('content-type')) {
            // Normalize the Content-Type header by removing any parameters
            v = v.split(';')[0].trim();
            includedHeaders.push([k, v]);
        } else {
            throw new Error('Unsupported header in the simplified fetch implementation. Only content-type, authorization, and x-bsv-* headers are supported.');
        }
    }

    // Sort the headers by key
    includedHeaders.sort(([keyA], [keyB]) => keyA.localeCompare(keyB));

    // nHeaders
    writer.writeVarIntNum(includedHeaders.length);
    for (let i = 0; i < includedHeaders.length; i++) {
        // headerKeyLength
        const headerKeyAsArray = Utils.toArray(includedHeaders[i][0], 'utf8');
        writer.writeVarIntNum(headerKeyAsArray.length);
        // headerKey
        writer.write(headerKeyAsArray);
        // headerValueLength
        const headerValueAsArray = Utils.toArray(includedHeaders[i][1], 'utf8');
        writer.writeVarIntNum(headerValueAsArray.length);
        // headerValue
        writer.write(headerValueAsArray);
    }

    // Handle body
    if (body) {
        // Normalize body to number array
        let reqBody;
        if (typeof body === 'object') {
            reqBody = Utils.toArray(JSON.stringify(body), 'utf8');
        } else if (typeof body === 'string') {
            reqBody = Utils.toArray(body, 'utf8');
        } else {
            throw new Error('Unsupported body type');
        }
        writer.writeVarIntNum(reqBody.length);
        writer.write(reqBody);
    } else {
        writer.writeVarIntNum(-1);
    }

    return writer;
}

// Test CSR serialization
async function testCSRSerialization() {
    console.log('🧪 Testing CSR Request Serialization\n');

    // Create a sample CSR request matching what we send
    const requestNonce = Random(32);
    const requestNonceBase64 = Utils.toBase64(requestNonce);

    const csrBody = {
        clientNonce: 'test_client_nonce_base64_32_bytes_here',
        type: 'test_certificate_type',
        fields: {
            'field1': 'encrypted_field1_base64',
            'field2': 'encrypted_field2_base64'
        },
        masterKeyring: {
            'field1': 'encrypted_revelation_key1_base64',
            'field2': 'encrypted_revelation_key2_base64'
        }
    };

    // Parse URL
    const parsedUrl = new URL('https://example.com/signCertificate');

    // Headers (matching what we send)
    const headers = {
        'Content-Type': 'application/json'
    };

    console.log('📋 CSR Request Body:');
    console.log(JSON.stringify(csrBody, null, 2));
    console.log('');

    // Serialize the request
    const writer = await serializeRequest(
        'POST',
        headers,
        csrBody,
        parsedUrl,
        requestNonce
    );

    const serialized = writer.toArray();

    console.log('📦 Serialized Request:');
    console.log(`   Total length: ${serialized.length} bytes`);
    console.log(`   Request nonce (base64): ${requestNonceBase64}`);
    console.log(`   Request nonce (hex, first 32 bytes): ${Utils.toHex(Buffer.from(serialized.slice(0, 32)))}`);
    console.log('');

    // Detailed breakdown
    let offset = 0;

    // 1. Nonce (32 bytes)
    const nonceBytes = serialized.slice(0, 32);
    offset = 32;
    console.log(`   [0..31] Nonce (32 bytes, hex): ${Utils.toHex(Buffer.from(nonceBytes))}`);
    console.log(`   [0..31] Nonce (base64): ${Utils.toBase64(nonceBytes)}`);

    // 2. Method
    const methodVarInt = serialized[offset];
    let methodVarIntLen = 1;
    let methodLen;
    if (methodVarInt < 0xFD) {
        methodLen = methodVarInt;
    } else if (methodVarInt === 0xFD) {
        methodVarIntLen = 3;
        methodLen = serialized[offset + 1] | (serialized[offset + 2] << 8);
    } else if (methodVarInt === 0xFE) {
        methodVarIntLen = 5;
        methodLen = serialized[offset + 1] | (serialized[offset + 2] << 8) | (serialized[offset + 3] << 16) | (serialized[offset + 4] << 24);
    } else {
        methodVarIntLen = 9;
        methodLen = serialized[offset + 1] | (serialized[offset + 2] << 8) | (serialized[offset + 3] << 16) | (serialized[offset + 4] << 24) |
                    (serialized[offset + 5] << 32) | (serialized[offset + 6] << 40) | (serialized[offset + 7] << 48) | (serialized[offset + 8] << 56);
    }
    offset += methodVarIntLen;
    const methodBytes = serialized.slice(offset, offset + methodLen);
    offset += methodLen;
    console.log(`   [${offset - methodVarIntLen - methodLen}..${offset - 1}] Method VarInt (${methodVarIntLen} bytes): ${Utils.toHex(Buffer.from(serialized.slice(offset - methodVarIntLen - methodLen, offset - methodLen)))}`);
    console.log(`   [${offset - methodLen}..${offset - 1}] Method (${methodLen} bytes): ${Utils.toUTF8(methodBytes)}`);

    // 3. Path
    const pathVarInt = serialized[offset];
    let pathVarIntLen = 1;
    let pathLen;
    if (pathVarInt < 0xFD) {
        pathLen = pathVarInt;
    } else if (pathVarInt === 0xFD) {
        pathVarIntLen = 3;
        pathLen = serialized[offset + 1] | (serialized[offset + 2] << 8);
    } else if (pathVarInt === 0xFE) {
        pathVarIntLen = 5;
        pathLen = serialized[offset + 1] | (serialized[offset + 2] << 8) | (serialized[offset + 3] << 16) | (serialized[offset + 4] << 24);
    } else {
        pathVarIntLen = 9;
        pathLen = serialized[offset + 1] | (serialized[offset + 2] << 8) | (serialized[offset + 3] << 16) | (serialized[offset + 4] << 24) |
                  (serialized[offset + 5] << 32) | (serialized[offset + 6] << 40) | (serialized[offset + 7] << 48) | (serialized[offset + 8] << 56);
    }
    offset += pathVarIntLen;
    const pathBytes = serialized.slice(offset, offset + pathLen);
    offset += pathLen;
    console.log(`   [${offset - pathVarIntLen - pathLen}..${offset - 1}] Path VarInt (${pathVarIntLen} bytes): ${Utils.toHex(Buffer.from(serialized.slice(offset - pathVarIntLen - pathLen, offset - pathLen)))}`);
    console.log(`   [${offset - pathLen}..${offset - 1}] Path (${pathLen} bytes): ${Utils.toUTF8(pathBytes)}`);

    // 4. Search (-1)
    const searchVarInt = serialized[offset];
    let searchVarIntLen = 1;
    if (searchVarInt === 0xFF) {
        searchVarIntLen = 9;
    }
    offset += searchVarIntLen;
    console.log(`   [${offset - searchVarIntLen}..${offset - 1}] Search VarInt (-1, ${searchVarIntLen} bytes): ${Utils.toHex(Buffer.from(serialized.slice(offset - searchVarIntLen, offset)))}`);

    // 5. Headers
    const headerCountVarInt = serialized[offset];
    let headerCountVarIntLen = 1;
    let headerCount;
    if (headerCountVarInt < 0xFD) {
        headerCount = headerCountVarInt;
    } else if (headerCountVarInt === 0xFD) {
        headerCountVarIntLen = 3;
        headerCount = serialized[offset + 1] | (serialized[offset + 2] << 8);
    } else if (headerCountVarInt === 0xFE) {
        headerCountVarIntLen = 5;
        headerCount = serialized[offset + 1] | (serialized[offset + 2] << 8) | (serialized[offset + 3] << 16) | (serialized[offset + 4] << 24);
    } else {
        headerCountVarIntLen = 9;
        headerCount = serialized[offset + 1] | (serialized[offset + 2] << 8) | (serialized[offset + 3] << 16) | (serialized[offset + 4] << 24) |
                      (serialized[offset + 5] << 32) | (serialized[offset + 6] << 40) | (serialized[offset + 7] << 48) | (serialized[offset + 8] << 56);
    }
    offset += headerCountVarIntLen;
    console.log(`   [${offset - headerCountVarIntLen}..${offset - 1}] Header count VarInt (${headerCountVarIntLen} bytes): ${Utils.toHex(Buffer.from(serialized.slice(offset - headerCountVarIntLen, offset)))}`);
    console.log(`   Header count: ${headerCount}`);

    for (let i = 0; i < headerCount; i++) {
        // Header key length
        const keyLenVarInt = serialized[offset];
        let keyLenVarIntLen = 1;
        let keyLen;
        if (keyLenVarInt < 0xFD) {
            keyLen = keyLenVarInt;
        } else if (keyLenVarInt === 0xFD) {
            keyLenVarIntLen = 3;
            keyLen = serialized[offset + 1] | (serialized[offset + 2] << 8);
        } else if (keyLenVarInt === 0xFE) {
            keyLenVarIntLen = 5;
            keyLen = serialized[offset + 1] | (serialized[offset + 2] << 8) | (serialized[offset + 3] << 16) | (serialized[offset + 4] << 24);
        } else {
            keyLenVarIntLen = 9;
            keyLen = serialized[offset + 1] | (serialized[offset + 2] << 8) | (serialized[offset + 3] << 16) | (serialized[offset + 4] << 24) |
                      (serialized[offset + 5] << 32) | (serialized[offset + 6] << 40) | (serialized[offset + 7] << 48) | (serialized[offset + 8] << 56);
        }
        offset += keyLenVarIntLen;
        const keyBytes = serialized.slice(offset, offset + keyLen);
        offset += keyLen;

        // Header value length
        const valueLenVarInt = serialized[offset];
        let valueLenVarIntLen = 1;
        let valueLen;
        if (valueLenVarInt < 0xFD) {
            valueLen = valueLenVarInt;
        } else if (valueLenVarInt === 0xFD) {
            valueLenVarIntLen = 3;
            valueLen = serialized[offset + 1] | (serialized[offset + 2] << 8);
        } else if (valueLenVarInt === 0xFE) {
            valueLenVarIntLen = 5;
            valueLen = serialized[offset + 1] | (serialized[offset + 2] << 8) | (serialized[offset + 3] << 16) | (serialized[offset + 4] << 24);
        } else {
            valueLenVarIntLen = 9;
            valueLen = serialized[offset + 1] | (serialized[offset + 2] << 8) | (serialized[offset + 3] << 16) | (serialized[offset + 4] << 24) |
                        (serialized[offset + 5] << 32) | (serialized[offset + 6] << 40) | (serialized[offset + 7] << 48) | (serialized[offset + 8] << 56);
        }
        offset += valueLenVarIntLen;
        const valueBytes = serialized.slice(offset, offset + valueLen);
        offset += valueLen;

        console.log(`   Header ${i + 1}:`);
        console.log(`      Key VarInt (${keyLenVarIntLen} bytes): ${Utils.toHex(Buffer.from(serialized.slice(offset - valueLen - valueLenVarIntLen - keyLen - keyLenVarIntLen, offset - valueLen - valueLenVarIntLen - keyLen)))}`);
        console.log(`      Key (${keyLen} bytes): ${Utils.toUTF8(keyBytes)}`);
        console.log(`      Value VarInt (${valueLenVarIntLen} bytes): ${Utils.toHex(Buffer.from(serialized.slice(offset - valueLen - valueLenVarIntLen, offset - valueLen)))}`);
        console.log(`      Value (${valueLen} bytes): ${Utils.toUTF8(valueBytes)}`);
    }

    // 6. Body
    const bodyLenVarInt = serialized[offset];
    let bodyLenVarIntLen = 1;
    let bodyLen;
    if (bodyLenVarInt < 0xFD) {
        bodyLen = bodyLenVarInt;
    } else if (bodyLenVarInt === 0xFD) {
        bodyLenVarIntLen = 3;
        bodyLen = serialized[offset + 1] | (serialized[offset + 2] << 8);
    } else if (bodyLenVarInt === 0xFE) {
        bodyLenVarIntLen = 5;
        bodyLen = serialized[offset + 1] | (serialized[offset + 2] << 8) | (serialized[offset + 3] << 16) | (serialized[offset + 4] << 24);
    } else {
        bodyLenVarIntLen = 9;
        bodyLen = serialized[offset + 1] | (serialized[offset + 2] << 8) | (serialized[offset + 3] << 16) | (serialized[offset + 4] << 24) |
                  (serialized[offset + 5] << 32) | (serialized[offset + 6] << 40) | (serialized[offset + 7] << 48) | (serialized[offset + 8] << 56);
    }
    offset += bodyLenVarIntLen;
    const bodyBytes = serialized.slice(offset, offset + bodyLen);
    offset += bodyLen;
    console.log(`   [${offset - bodyLenVarIntLen - bodyLen}..${offset - 1}] Body length VarInt (${bodyLenVarIntLen} bytes): ${Utils.toHex(Buffer.from(serialized.slice(offset - bodyLenVarIntLen - bodyLen, offset - bodyLen)))}`);
    console.log(`   [${offset - bodyLen}..${offset - 1}] Body (${bodyLen} bytes, hex, first 200): ${Utils.toHex(Buffer.from(bodyBytes.slice(0, 200)))}`);
    console.log(`   Body (UTF-8): ${Utils.toUTF8(bodyBytes)}`);

    console.log('');
    console.log('📊 Full Serialized Request:');
    console.log(`   Hex (full): ${Utils.toHex(Buffer.from(serialized))}`);
    console.log(`   Base64 (full): ${Utils.toBase64(serialized)}`);
    console.log(`   Length: ${serialized.length} bytes`);
    console.log('');

    // Output JSON for easy comparison
    const output = {
        requestNonce: requestNonceBase64,
        csrBody: csrBody,
        serialized: {
            hex: Utils.toHex(Buffer.from(serialized)),
            base64: Utils.toBase64(serialized),
            length: serialized.length
        },
        breakdown: {
            nonce: Utils.toHex(Buffer.from(nonceBytes)),
            method: Utils.toUTF8(methodBytes),
            path: Utils.toUTF8(pathBytes),
            headerCount: headerCount,
            body: Utils.toUTF8(bodyBytes)
        }
    };

    console.log('📄 JSON Output (for comparison):');
    console.log(JSON.stringify(output, null, 2));

    // Save to file for comparison
    fs.writeFileSync(
        path.join(__dirname, 'csr_serialization_ts_sdk.json'),
        JSON.stringify(output, null, 2)
    );
    console.log('\n✅ Saved to: csr_serialization_ts_sdk.json');
}

// Run the test
testCSRSerialization().catch(error => {
    console.error('❌ Test failed:', error);
    console.error(error.stack);
    process.exit(1);
});
