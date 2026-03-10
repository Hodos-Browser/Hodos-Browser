/**
 * Test AuthFetch headers to see exactly what the TypeScript SDK sends
 *
 * This script uses AuthFetch to make a request and logs all headers
 * so we can compare with our Rust implementation.
 */

const path = require('path');
const http = require('http');

// Load TypeScript SDK
let Utils, ProtoWallet, AuthFetch, Random, sdk, Mnemonic, HD, MasterCertificate;
try {
    const sdkPath = path.join(__dirname, '..', 'reference', 'ts-brc100', 'node_modules', '@bsv', 'sdk');
    sdk = require(sdkPath);
    Utils = sdk.Utils || sdk;
    MasterCertificate = sdk.MasterCertificate;
    ProtoWallet = sdk.ProtoWallet;
    AuthFetch = sdk.AuthFetch;
    Random = sdk.Random;

    // Load compat modules
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

// Create a test server to capture headers
let requestCount = 0;
let certifierPublicKey = '0220529dc803041a83f4357864a09c717daa24397cf2f3fc3a5745ae08d30924fd';
const testServer = http.createServer((req, res) => {
    requestCount++;
    const parsedUrl = require('url').parse(req.url, true);
    const pathname = parsedUrl.pathname;

    console.log('\n' + '='.repeat(60));
    console.log(`📥 REQUEST #${requestCount} RECEIVED`);
    console.log('='.repeat(60));
    console.log(`Method: ${req.method}`);
    console.log(`Path: ${pathname}`);

    const headers = {};
    for (const [key, value] of Object.entries(req.headers)) {
        headers[key.toLowerCase()] = value;
    }

    console.log('\n📋 Headers (x-bsv-auth-*):');
    let hasAuthHeaders = false;
    for (const [key, value] of Object.entries(headers)) {
        if (key.startsWith('x-bsv-auth')) {
            console.log(`   ${key}: ${value}`);
            hasAuthHeaders = true;
        }
    }
    if (!hasAuthHeaders) {
        console.log('   (none)');
    }

    // Read body
    let body = '';
    req.on('data', chunk => {
        body += chunk.toString();
    });

    req.on('end', () => {
        if (pathname === '/.well-known/auth' || pathname === '/initialRequest') {
            // Handle initialRequest
            console.log('\n📦 Initial Request Body:');
            try {
                const parsed = JSON.parse(body);
                console.log(JSON.stringify(parsed, null, 2));

                // Generate server nonce and response
                const serverNonce = Utils.toBase64(Random(32));
                const serverIdentityKey = certifierPublicKey;

                // Create a simple response (we won't sign it properly, just for testing)
                const response = {
                    version: "0.1",
                    messageType: "initialResponse",
                    identityKey: serverIdentityKey,
                    nonce: serverNonce,
                    signature: [] // Empty signature for testing
                };

                res.writeHead(200, { 'Content-Type': 'application/json' });
                res.end(JSON.stringify(response));
                console.log('\n✅ Initial response sent');
            } catch (e) {
                res.writeHead(400, { 'Content-Type': 'application/json' });
                res.end(JSON.stringify({ error: 'Invalid JSON' }));
            }
        } else if (pathname === '/signCertificate') {
            // This is the actual CSR request - capture all headers
            console.log('\n📋 ALL Headers for /signCertificate:');
            for (const [key, value] of Object.entries(headers)) {
                console.log(`   ${key}: ${value}`);
            }

            console.log('\n📦 CSR Body:');
            console.log(`   Length: ${body.length} bytes`);
            try {
                const parsed = JSON.parse(body);
                console.log('\n📦 Parsed CSR:');
                console.log(`   clientNonce: ${parsed.clientNonce}`);
                console.log(`   type: ${parsed.type}`);
                console.log(`   fields: ${Object.keys(parsed.fields || {}).length} field(s)`);
                console.log(`   masterKeyring: ${Object.keys(parsed.masterKeyring || {}).length} key(s)`);
            } catch (e) {
                console.log(`   (not JSON or parse error: ${e.message})`);
            }

            res.writeHead(200, {
                'Content-Type': 'application/json',
                'x-bsv-auth-identity-key': certifierPublicKey
            });
            res.end(JSON.stringify({ success: true, certificate: {}, serverNonce: Utils.toBase64(Random(32)) }));

            console.log('\n✅ CSR response sent');
            console.log('='.repeat(60));

            // Close server after CSR request
            setTimeout(() => {
                testServer.close();
                process.exit(0);
            }, 100);
        } else {
            res.writeHead(404, { 'Content-Type': 'application/json' });
            res.end(JSON.stringify({ error: 'Not found' }));
        }
    });
});

async function testAuthFetchHeaders() {
    console.log('🔍 Testing AuthFetch Headers');
    console.log('='.repeat(60));
    console.log('');

    // Start test server
    const PORT = 3002;
    testServer.listen(PORT, () => {
        console.log(`✅ Test server listening on port ${PORT}`);
        console.log('');
    });

    // Wait a moment for server to start
    await new Promise(resolve => setTimeout(resolve, 500));

    // Create wallet
    console.log('🔧 Creating test wallet...');
    const mnemonicStr = 'abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about';
    const mnemonic = new Mnemonic(mnemonicStr);
    mnemonic.mnemonic2Seed();
    const hdWallet = HD.fromSeed(mnemonic.seed);
    const wallet = new ProtoWallet(hdWallet.privKey);

    const identityKeyResult = await wallet.getPublicKey({ identityKey: true });
    console.log(`   Identity key: ${identityKeyResult.publicKey}`);
    console.log('');

    // Create AuthFetch instance
    console.log('🔧 Creating AuthFetch instance...');
    const authClient = new AuthFetch(wallet);
    console.log('   ✅ AuthFetch created');
    console.log('');

    // Create test CSR body (matching what we send)
    const certifierPublicKey = '0220529dc803041a83f4357864a09c717daa24397cf2f3fc3a5745ae08d30924fd';
    const certificateType = 'AGfk/WrT1eBDXpz3mcw386Zww2HmqcIn3uY6x4Af1eo=';

    console.log('🔧 Creating certificate fields...');
    const { certificateFields, masterKeyring } = await MasterCertificate.createCertificateFields(
        wallet,
        certifierPublicKey,
        { cool: true }
    );
    console.log('   ✅ Certificate fields created');
    console.log('');

    // Use AuthFetch to make the request (it will handle initialRequest automatically)
    console.log('📤 Making request with AuthFetch...');
    console.log(`   URL: http://localhost:${PORT}/signCertificate`);
    console.log(`   Method: POST`);
    console.log('');
    console.log('   Note: AuthFetch will first send initialRequest to /.well-known/auth');
    console.log('   Then it will send the authenticated /signCertificate request');
    console.log('');

    try {
        // Create CSR body (AuthFetch will handle the clientNonce internally)
        // Actually, we need to create it the same way Wallet.acquireCertificate does
        const clientNonce = await sdk.createNonce(wallet, certifierPublicKey);

        const csrBody = {
            clientNonce,
            type: certificateType,
            fields: certificateFields,
            masterKeyring: masterKeyring
        };

        const response = await authClient.fetch(`http://localhost:${PORT}/signCertificate`, {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json'
            },
            body: JSON.stringify(csrBody)
        });

        console.log('\n✅ Request completed');
        console.log(`   Status: ${response.status}`);

    } catch (error) {
        console.error('\n❌ Request failed:', error.message);
        // Don't exit on error - we want to see the headers even if auth fails
    }
}

// Run test
testAuthFetchHeaders().catch(error => {
    console.error('❌ Fatal error:', error);
    console.error(error.stack);
    process.exit(1);
});
