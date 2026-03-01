/**
 * TypeScript SDK Server - Simulates Certifier Server
 *
 * This script acts as a certifier server using TypeScript SDK to:
 * 1. Receive initialRequest from Rust wallet
 * 2. Create initialResponse using TypeScript SDK
 * 3. Receive CSR from Rust wallet
 * 4. Decrypt and validate CSR using TypeScript SDK
 * 5. Output all steps and bytes for comparison
 *
 * This allows us to test if our Rust wallet can successfully communicate
 * with a TypeScript SDK-based server, and compare the exact bytes/flow.
 */

const http = require('http');
const path = require('path');
const url = require('url');

// Load TypeScript SDK
let Utils, ProtoWallet, MasterCertificate, Random, Mnemonic, HD, PrivateKey, SymmetricKey;
try {
    const sdkPath = path.join(__dirname, '..', 'reference', 'ts-brc100', 'node_modules', '@bsv', 'sdk');
    const sdk = require(sdkPath);
    Utils = sdk.Utils || sdk;
    ProtoWallet = sdk.ProtoWallet;
    MasterCertificate = sdk.MasterCertificate;
    Random = sdk.Random;
    PrivateKey = sdk.PrivateKey;
    SymmetricKey = sdk.SymmetricKey;

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

// Helper function to get certificate field encryption details
// This matches Certificate.getCertificateFieldEncryptionDetails from TypeScript SDK
function getCertificateFieldEncryptionDetails(fieldName, serialNumber) {
    return {
        protocolID: [2, 'certificate field encryption'],
        keyID: serialNumber ? `${serialNumber} ${fieldName}` : fieldName
    };
}

// Create certifier (server) wallet
let certifierWallet;
let certifierPublicKey;

async function initializeServer() {
    console.log('Initializing TypeScript SDK Certifier Server...\n');

    // Create a test certifier wallet (this acts as the server)
    // In production, the server would use its actual private key
    const certifierMnemonic = 'abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about';

    // Create mnemonic and get seed
    const mnemonic = new Mnemonic(certifierMnemonic);
    mnemonic.mnemonic2Seed(); // Generate seed from mnemonic

    // Create HD wallet from seed and get master private key
    const hdWallet = HD.fromSeed(mnemonic.seed);
    const masterPrivateKey = hdWallet.privKey;

    // Create ProtoWallet from private key
    certifierWallet = new ProtoWallet(masterPrivateKey);
    const publicKeyResult = await certifierWallet.getPublicKey({ identityKey: true });
    certifierPublicKey = publicKeyResult.publicKey;

    console.log('✅ Certifier (server) wallet created');
    console.log(`   Identity key: ${certifierPublicKey}`);
    console.log('');
}

// Handle initialRequest (BRC-53 Step 1)
async function handleInitialRequest(body) {
    console.log('📥 Received initialRequest');
    console.log('   Body:', JSON.stringify(body, null, 2));
    console.log('');

    const clientIdentityKey = body.identityKey;
    const clientInitialNonce = body.initialNonce;

    console.log('   Client identity key:', clientIdentityKey);
    console.log('   Client initial nonce:', clientInitialNonce);
    console.log('');

    // Generate server nonce
    const serverNonce = Utils.toBase64(Random(32));
    console.log('   Generated server nonce:', serverNonce);
    console.log('');

    // Create initialResponse using TypeScript SDK's wallet.createSignature
    // Data to sign: clientNonce + serverNonce (as base64 strings, concatenated, then converted to array)
    const dataToSign = Utils.toArray(clientInitialNonce + serverNonce, 'base64');

    console.log('   Data to sign length:', dataToSign.length, 'bytes');
    console.log('   Data to sign (hex, first 64):', Utils.toHex(Buffer.from(dataToSign)).substring(0, 128));
    console.log('');

    // Create signature using TypeScript SDK's wallet.createSignature
    const keyID = `${clientInitialNonce} ${serverNonce}`;
    const { signature } = await certifierWallet.createSignature({
        data: dataToSign,
        protocolID: [2, 'auth message signature'],
        keyID: keyID,
        counterparty: clientIdentityKey
    });

    console.log('   Signature (array length):', signature.length);
    console.log('   Signature (hex, first 64):', Utils.toHex(Buffer.from(signature)).substring(0, 128));
    console.log('');

    // Create initialResponse
    const initialResponse = {
        version: '0.1',
        messageType: 'initialResponse',
        identityKey: certifierPublicKey,
        initialNonce: serverNonce,
        yourNonce: clientInitialNonce,
        requestedCertificates: {
            certifiers: [],
            types: {}
        },
        signature: signature // TypeScript SDK sends as array
    };

    console.log('📤 Sending initialResponse');
    console.log('   Response:', JSON.stringify(initialResponse, null, 2));
    console.log('');

    return initialResponse;
}

// Handle CSR (Certificate Signing Request) - BRC-53 Step 2
async function handleCSR(body, headers) {
    console.log('\n' + '='.repeat(80));
    console.log('📥 Received CSR (Certificate Signing Request)');
    console.log('='.repeat(80));

    // Log raw body bytes
    const bodyStr = typeof body === 'string' ? body : JSON.stringify(body);
    const bodyBytes = Buffer.from(bodyStr, 'utf8');
    console.log('   Body length:', bodyBytes.length, 'bytes');
    console.log('   Body (hex, full):', bodyBytes.toString('hex'));
    console.log('   Body (base64, full):', bodyBytes.toString('base64'));
    console.log('');

    // Parse and log JSON structure
    let parsedBody;
    try {
        parsedBody = typeof body === 'string' ? JSON.parse(body) : body;
        console.log('   📋 Parsed JSON Body:');
        console.log(JSON.stringify(parsedBody, null, 2));
        console.log('');

        // Log field order
        console.log('   📋 Field order in JSON:');
        const fields = Object.keys(parsedBody);
        fields.forEach((field, i) => {
            console.log(`      ${i + 1}. "${field}"`);
        });
        console.log('');

        // Log specific fields we care about
        if (parsedBody.messageType) {
            console.log(`   ⚠️  Found messageType: "${parsedBody.messageType}"`);
        }
        if (parsedBody.serverSerialNonce) {
            console.log(`   ⚠️  Found serverSerialNonce: "${parsedBody.serverSerialNonce.substring(0, 50)}..."`);
        }
        if (parsedBody.keyring) {
            console.log(`   ⚠️  Found keyring with ${Object.keys(parsedBody.keyring).length} field(s)`);
        }
        if (parsedBody.masterKeyring) {
            console.log(`   ✅ Found masterKeyring with ${Object.keys(parsedBody.masterKeyring).length} field(s)`);
        }
        console.log('');
    } catch (e) {
        console.log('   ⚠️  Body is not valid JSON:', e.message);
        parsedBody = body;
    }

    console.log('   Headers:', JSON.stringify(headers, null, 2));
    console.log('');

    // Parse CSR - log ALL fields present
    console.log('   📋 All fields present in request body:');
    Object.keys(parsedBody).forEach(key => {
        const value = parsedBody[key];
        if (typeof value === 'object' && value !== null && !Array.isArray(value)) {
            console.log(`     ${key}: object with ${Object.keys(value).length} key(s)`);
        } else if (typeof value === 'string') {
            const preview = value.length > 50 ? value.substring(0, 50) + '...' : value;
            console.log(`     ${key}: "${preview}" (${value.length} chars)`);
        } else {
            console.log(`     ${key}: ${JSON.stringify(value)}`);
        }
    });
    console.log('');

    // Extract expected fields
    const clientNonce = parsedBody.clientNonce;
    const certificateType = parsedBody.type;
    const encryptedFields = parsedBody.fields || {};
    const encryptedMasterKeyring = parsedBody.masterKeyring || parsedBody.keyring || {};

    // Check for unexpected fields
    const expectedFields = ['clientNonce', 'type', 'fields', 'masterKeyring'];
    const unexpectedFields = Object.keys(parsedBody).filter(k => !expectedFields.includes(k));
    if (unexpectedFields.length > 0) {
        console.log('   ⚠️  UNEXPECTED FIELDS (not in TypeScript SDK format):');
        unexpectedFields.forEach(field => {
            console.log(`      - ${field}: ${JSON.stringify(parsedBody[field]).substring(0, 100)}`);
        });
        console.log('');
    }

    console.log('   CSR Fields:');
    console.log(`     clientNonce: ${clientNonce}`);
    console.log(`     type: ${certificateType}`);
    console.log(`     fields: ${Object.keys(encryptedFields).length} field(s)`);
    console.log(`     masterKeyring: ${Object.keys(encryptedMasterKeyring).length} key(s)`);
    console.log('');

    // Get client identity key from auth headers
    const clientIdentityKey = headers['x-bsv-auth-identity-key'];
    if (!clientIdentityKey) {
        throw new Error('Missing x-bsv-auth-identity-key header');
    }

    console.log('   Client identity key:', clientIdentityKey);
    console.log('');

    // Validate BRC-31 signature (this would be done by the server's auth middleware)
    console.log('   ✅ BRC-31 signature validated (assuming valid for this test)');
    console.log('');

    // Verify the client nonce (matching coolcert's behavior)
    console.log('🔍 Verifying client nonce (matching coolcert behavior)...');
    console.log(`   Client nonce: ${clientNonce}`);
    console.log(`   Client identity key (counterparty): ${clientIdentityKey}`);
    console.log(`   Server will derive key using:`);
    console.log(`      - Server's master private key`);
    console.log(`      - Client's public key (counterparty): ${clientIdentityKey}`);
    console.log(`      - Invoice: 2-server hmac-{keyID}`);
    console.log(`   Where keyID = Utils.toUTF8(first 16 bytes of nonce)`);
    try {
        const verifyNoncePath = path.join(__dirname, '..', 'reference', 'ts-brc100', 'node_modules', '@bsv', 'sdk', 'dist', 'cjs', 'src', 'auth', 'utils', 'verifyNonce.js');
        const verifyNonceModule = require(verifyNoncePath);
        const verifyNonce = verifyNonceModule.default || verifyNonceModule.verifyNonce || verifyNonceModule;

        // Decode nonce to get first 16 bytes for keyID
        const nonceBuffer = Utils.toArray(clientNonce, 'base64');
        const firstHalf = nonceBuffer.slice(0, 16);
        const keyID = Utils.toUTF8(firstHalf);
        const keyIDBytes = Buffer.from(keyID, 'utf8');
        console.log(`   First 16 bytes (hex): ${Utils.toHex(Buffer.from(firstHalf))}`);
        console.log(`   KeyID (UTF-8 decoded): "${keyID}" (${keyID.length} chars)`);
        console.log(`   KeyID UTF-8 bytes (hex): ${keyIDBytes.toString('hex')}`);
        console.log(`   KeyID UTF-8 bytes length: ${keyIDBytes.length} bytes`);
        console.log(`   Invoice number will be: 2-server hmac-${keyID}`);
        console.log(`   Invoice number UTF-8 bytes (hex): ${Buffer.from(`2-server hmac-${keyID}`, 'utf8').toString('hex')}`);

        const isValid = await verifyNonce(clientNonce, certifierWallet, clientIdentityKey);
        if (isValid) {
            console.log('   ✅ Client nonce verified successfully!');
        } else {
            console.log('   ❌ Client nonce verification FAILED!');
            console.log('   This means the HMAC key derived by the server does not match the HMAC in the nonce.');
            throw new Error('Client nonce verification failed');
        }
    } catch (error) {
        console.log('   ❌ Error verifying client nonce:', error.message);
        console.log('   Error stack:', error.stack);
        throw error;
    }
    console.log('');

    // Decrypt masterKeyring revelation keys using TypeScript SDK's wallet.decrypt
    console.log('🔓 Decrypting masterKeyring revelation keys using TypeScript SDK...');
    console.log(`   Server will decrypt using:`);
    console.log(`      - Server's private key (certifier wallet)`);
    console.log(`      - Client's public key (counterparty): ${clientIdentityKey}`);
    console.log(`      - Invoice: 2-certificate field encryption-{fieldName}`);

    const decryptedRevelationKeys = {};
    for (const [fieldName, encryptedRevelationKeyBase64] of Object.entries(encryptedMasterKeyring)) {
        console.log(`\n   📊 Field: ${fieldName}`);
        console.log(`   📊 Encrypted revelation key (base64, FULL): ${encryptedRevelationKeyBase64}`);
        console.log(`   📊 Encrypted revelation key length: ${encryptedRevelationKeyBase64.length} chars (base64)`);

        try {
            // Use TypeScript SDK's wallet.decrypt with certificate field encryption details
            // The counterparty is the SUBJECT (client), not the certifier
            // For MasterCertificate, only fieldName is used (no serialNumber)
            const encryptionDetails = getCertificateFieldEncryptionDetails(fieldName);

            console.log(`   📊 Encryption details:`, JSON.stringify(encryptionDetails, null, 2));
            console.log(`   📊 Counterparty (subject): ${clientIdentityKey}`);
            console.log(`   📊 Invoice number will be: 2-certificate field encryption-${fieldName}`);

            const { plaintext: revelationKey } = await certifierWallet.decrypt({
                ciphertext: Utils.toArray(encryptedRevelationKeyBase64, 'base64'),
                ...encryptionDetails,
                counterparty: clientIdentityKey // Subject is the counterparty
            });

            decryptedRevelationKeys[fieldName] = revelationKey;
            console.log(`   ✅ Revelation key decrypted!`);
            console.log(`   📊 Revelation key (hex, FULL): ${Utils.toHex(Buffer.from(revelationKey))}`);
            console.log(`   📊 Revelation key (base64): ${Buffer.from(revelationKey).toString('base64')}`);
            console.log(`   📊 Revelation key length: ${revelationKey.length} bytes`);

        } catch (error) {
            console.error(`   ❌ Failed to decrypt revelation key for field '${fieldName}':`, error.message);
            console.error(`   Error stack:`, error.stack);
            throw error;
        }
    }

    console.log('');

    // Decrypt field values using revelation keys
    console.log('🔓 Decrypting field values using revelation keys...');
    const decryptedFields = {};

    for (const [fieldName, encryptedFieldValueBase64] of Object.entries(encryptedFields)) {
        console.log(`\n   📊 Field: ${fieldName}`);
        console.log(`   📊 Encrypted field value (base64, FULL): ${encryptedFieldValueBase64}`);
        console.log(`   📊 Encrypted field value length: ${encryptedFieldValueBase64.length} chars (base64)`);

        const revelationKey = decryptedRevelationKeys[fieldName];
        if (!revelationKey) {
            throw new Error(`No revelation key found for field '${fieldName}'`);
        }

        console.log(`   📊 Using revelation key (hex, FULL): ${Utils.toHex(Buffer.from(revelationKey))}`);
        console.log(`   📊 Revelation key length: ${revelationKey.length} bytes`);

        try {
            // Create SymmetricKey from revelation key and decrypt
            // SymmetricKey constructor takes the key bytes directly (number array)
            const fieldKey = new SymmetricKey(revelationKey);

            // Decrypt field value
            const encryptedData = Utils.toArray(encryptedFieldValueBase64, 'base64');
            console.log(`   📊 Encrypted data length: ${encryptedData.length} bytes`);
            const decryptedBytes = fieldKey.decrypt(encryptedData);
            const fieldValueString = Utils.toUTF8(decryptedBytes);

            decryptedFields[fieldName] = fieldValueString;
            console.log(`   ✅ Field value decrypted!`);
            console.log(`   📊 Decrypted value: ${fieldValueString}`);
            console.log(`   📊 Decrypted value (hex, FULL): ${Utils.toHex(decryptedBytes)}`);
            console.log(`   📊 Decrypted value length: ${decryptedBytes.length} bytes`);

        } catch (error) {
            console.error(`   ❌ Failed to decrypt field '${fieldName}':`, error.message);
            console.error(`   Error stack:`, error.stack);
            throw error;
        }
    }

    console.log('');
    console.log('✅✅✅ CSR Decryption Complete!');
    console.log('   Decrypted fields:', JSON.stringify(decryptedFields, null, 2));
    console.log('');

    // Return a minimal certificate object that the Rust wallet expects
    // The Rust wallet's parser expects: type, certifier, serialNumber, subject, fields, signature
    // Generate a random serialNumber (32 bytes, base64)
    const serialNumberBytes = Random(32);
    const serialNumber = Utils.toBase64(serialNumberBytes);

    // Create a minimal certificate response
    // Note: This is just for testing - a real server would properly sign the certificate
    // revocationOutpoint must be in format "txid.vout" where txid is 32 bytes (64 hex chars)
    const dummyTxid = '0000000000000000000000000000000000000000000000000000000000000000';
    const certificate = {
        type: certificateType, // Use the type from the CSR
        certifier: certifierPublicKey, // Server's identity key
        serialNumber: serialNumber,
        subject: clientIdentityKey, // Client's identity key
        fields: encryptedFields, // Encrypted fields from CSR (as-is)
        signature: '0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000', // Dummy signature for testing
        revocationOutpoint: `${dummyTxid}.0`, // Format: "txid.vout" (32-byte txid + vout number)
        keyringForSubject: {} // Empty for testing
    };

    console.log('📤 Returning certificate object:');
    console.log(`   type: ${certificate.type}`);
    console.log(`   certifier: ${certificate.certifier}`);
    console.log(`   serialNumber: ${certificate.serialNumber}`);
    console.log(`   subject: ${certificate.subject}`);
    console.log(`   fields: ${Object.keys(certificate.fields).length} field(s)`);
    console.log('');

    // According to BRC-53, the response should have a 'certificate' field
    // But the Rust wallet expects the certificate object directly
    // Return both formats for compatibility
    return {
        status: 'success',
        certificate: certificate
    };
}

// HTTP Server
const server = http.createServer(async (req, res) => {
    const parsedUrl = url.parse(req.url, true);
    const pathname = parsedUrl.pathname;

    // CORS headers
    res.setHeader('Access-Control-Allow-Origin', '*');
    res.setHeader('Access-Control-Allow-Methods', 'GET, POST, OPTIONS');
    res.setHeader('Access-Control-Allow-Headers', 'Content-Type, x-bsv-auth-*');

    if (req.method === 'OPTIONS') {
        res.writeHead(200);
        res.end();
        return;
    }

    // Read request body
    let body = '';
    req.on('data', chunk => {
        body += chunk.toString();
    });

    req.on('end', async () => {
        try {
            // Parse headers
            const headers = {};
            for (const [key, value] of Object.entries(req.headers)) {
                headers[key.toLowerCase()] = value;
            }

            console.log(`\n${'='.repeat(80)}`);
            console.log(`📥 ${req.method} ${pathname}`);
            console.log(`${'='.repeat(80)}`);

            // Log raw request details
            console.log('   Raw request body length:', body.length, 'bytes');
            if (body.length > 0) {
                const bodyBytes = Buffer.from(body, 'utf8');
                console.log('   Raw body (hex, first 200 bytes):', bodyBytes.toString('hex').substring(0, 400));
                console.log('   Raw body (utf8, first 200 chars):', body.substring(0, 200));
            }
            console.log('');

            if (pathname === '/certifierPublicKey' || pathname === '/getCertifierPublicKey') {
                // Return certifier's public key for testing (GET request, no body needed)
                console.log('   Returning certifier public key');
                res.writeHead(200, { 'Content-Type': 'application/json' });
                res.end(JSON.stringify({
                    certifier: certifierPublicKey,
                    message: 'Certifier public key for testing'
                }));

            } else if (pathname === '/.well-known/auth' || pathname === '/initialRequest') {
                // Handle initialRequest
                const requestBody = JSON.parse(body || '{}');
                const response = await handleInitialRequest(requestBody);

                res.writeHead(200, { 'Content-Type': 'application/json' });
                res.end(JSON.stringify(response));

            } else if (pathname === '/signCertificate') {
                // Handle CSR
                let requestBody;
                try {
                    requestBody = JSON.parse(body || '{}');
                } catch (e) {
                    console.error('   ❌ Failed to parse body as JSON:', e.message);
                    console.error('   Raw body:', body);
                    res.writeHead(400, { 'Content-Type': 'application/json' });
                    res.end(JSON.stringify({ error: 'Invalid JSON body' }));
                    return;
                }
                const result = await handleCSR(requestBody, headers);

                res.writeHead(200, { 'Content-Type': 'application/json' });
                res.end(JSON.stringify(result));

            } else {
                res.writeHead(404, { 'Content-Type': 'application/json' });
                res.end(JSON.stringify({ error: 'Not found' }));
            }

        } catch (error) {
            console.error('❌ Server error:', error.message);
            console.error(error.stack);

            res.writeHead(500, { 'Content-Type': 'application/json' });
            res.end(JSON.stringify({
                status: 'error',
                code: 'ERR_INTERNAL',
                description: error.message
            }));
        }
    });
});

// Start server
const PORT = 3001;
initializeServer().then(() => {
    server.listen(PORT, () => {
        console.log(`\n${'='.repeat(60)}`);
        console.log(`TypeScript SDK Certifier Server Running`);
        console.log(`${'='.repeat(60)}`);
        console.log(`Listening on: http://localhost:${PORT}`);
        console.log('');
        console.log('This server simulates a certifier using TypeScript SDK.');
        console.log('Point your Rust wallet to: http://localhost:3001');
        console.log('');
        console.log('The server will:');
        console.log('  1. Handle initialRequest (/.well-known/auth)');
        console.log('  2. Handle CSR (/signCertificate)');
        console.log('  3. Decrypt and validate using TypeScript SDK');
        console.log('  4. Output all steps and bytes for comparison');
        console.log('');
        console.log('📋 Certifier Public Key (use this in acquireCertificate request):');
        console.log(`   ${certifierPublicKey}`);
        console.log('');
        console.log('   Or GET http://localhost:3001/certifierPublicKey to retrieve it');
        console.log('');
    });
}).catch(error => {
    console.error('Failed to initialize server:', error);
    process.exit(1);
});
