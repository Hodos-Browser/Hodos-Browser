const path = require('path');
// Import from the same location as test_ts_sdk_server.js
const sdkPath = path.join(__dirname, '..', 'reference', 'ts-brc100', 'node_modules', '@bsv', 'sdk');
const sdk = require(sdkPath);
const Utils = sdk.Utils || sdk;
const ProtoWallet = sdk.ProtoWallet;
const Certificate = sdk.Certificate;

// Load compat modules for mnemonic/HD wallet
const compatPath = path.join(__dirname, '..', 'reference', 'ts-brc100', 'node_modules', '@bsv', 'sdk', 'dist', 'cjs', 'src', 'compat');
const MnemonicModule = require(path.join(compatPath, 'Mnemonic.js'));
const Mnemonic = MnemonicModule.default || MnemonicModule;
const HDModule = require(path.join(compatPath, 'HD.js'));
const HD = HDModule.default || HDModule;

// Test to see what the SDK actually computes when signing vs verifying
async function testSignVerify() {
    console.log('='.repeat(80));
    console.log('Testing SDK Certificate Signing and Verification');
    console.log('='.repeat(80));
    console.log('');

    // Create a test certificate (matching what the server would create)
    const type = 'AGfk/WrT1eBDXpz3mcw386Zww2HmqcIn3uY6x4Af1eo=';
    const serialNumber = 'zAqbEGs/JkXGrKysPcaQQtnATxGaMFkPf49LtESkd9o=';
    const subject = '020b95583e18ac933d89a131f399890098dc1b3d4a8abcdde3eec4a7b191d2521e';
    const certifier = '0220529dc803041a83f4357864a09c717daa24397cf2f3fc3a5745ae08d30924fd';
    const revocationOutpoint = '0000000000000000000000000000000000000000000000000000000000000000.0';
    const fields = {
        cool: 'C1UGmgHf3l5g4s5V90l6oL1cmbDgLQ/bUxOf3jT1VzfsCR+y/l7u3Bs+wftS5BMbP9Ds+w=='
    };

    console.log('📋 Certificate Details:');
    console.log(`   Type: ${type}`);
    console.log(`   SerialNumber: ${serialNumber}`);
    console.log(`   Subject: ${subject}`);
    console.log(`   Certifier: ${certifier}`);
    console.log(`   RevocationOutpoint: ${revocationOutpoint}`);
    console.log('');

    // Create a certifier wallet (simulating the server)
    console.log('🔐 Step 1: Creating certifier wallet (server)...');
    // Use the same approach as test_ts_sdk_server.js
    const certifierMnemonic = 'abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about';
    const mnemonic = new Mnemonic(certifierMnemonic);
    mnemonic.mnemonic2Seed(); // Generate seed from mnemonic
    const hdWallet = HD.fromSeed(mnemonic.seed);
    const masterPrivateKey = hdWallet.privKey;
    const certifierWallet = new ProtoWallet(masterPrivateKey);
    const certifierIdentityKey = await certifierWallet.getPublicKey({ identityKey: true });
    console.log(`   Certifier identity key: ${certifierIdentityKey.publicKey}`);
    console.log('');

    // Create the certificate
    const cert = new Certificate(
        type,
        serialNumber,
        subject,
        certifierIdentityKey.publicKey,
        revocationOutpoint,
        fields
    );

    // Get preimage (what will be signed)
    const preimage = cert.toBinary(false);
    console.log('📝 Step 2: Certificate preimage (what will be signed):');
    console.log(`   Length: ${preimage.length} bytes`);
    console.log(`   Hex (first 64): ${Utils.toHex(preimage.slice(0, 64))}`);
    console.log('');

    // Hash the preimage (using crypto for SHA256)
    const crypto = require('crypto');
    const preimageBuffer = Buffer.from(preimage);
    const hash = crypto.createHash('sha256').update(preimageBuffer).digest();
    console.log('🔐 Hash:');
    console.log(`   SHA256: ${Utils.toHex(Array.from(hash))}`);
    console.log('');

    // Sign the certificate (simulating what the server does)
    console.log('✍️  Step 3: Signing certificate with certifierWallet.createSignature()...');
    console.log('   Calling: certifierWallet.createSignature({');
    console.log(`     data: preimage,`);
    console.log(`     protocolID: [2, 'certificate signature'],`);
    console.log(`     keyID: '${type} ${serialNumber}',`);
    console.log(`     counterparty: undefined (not specified)`);
    console.log(`     forSelf: undefined (not specified)`);
    console.log('   })');
    console.log('');

    try {
        const { signature } = await certifierWallet.createSignature({
            data: preimage,
            protocolID: [2, 'certificate signature'],
            keyID: `${type} ${serialNumber}`
            // Note: counterparty and forSelf are NOT specified
        });

        cert.signature = Utils.toHex(signature);
        console.log(`   ✅ Signature created: ${cert.signature.substring(0, 40)}...`);
        console.log(`   Signature length: ${signature.length} bytes`);
        console.log('');
    } catch (error) {
        console.log(`   ❌ Signing failed: ${error.message}`);
        console.log('');
        return;
    }

    // Now verify the certificate (simulating what we do)
    console.log('🔍 Step 4: Verifying certificate with ProtoWallet("anyone").verifySignature()...');
    console.log('   Calling: verifier.verifySignature({');
    console.log(`     signature: signature,`);
    console.log(`     data: preimage,`);
    console.log(`     protocolID: [2, 'certificate signature'],`);
    console.log(`     keyID: '${type} ${serialNumber}',`);
    console.log(`     counterparty: '${certifierIdentityKey.publicKey}',`);
    console.log(`     forSelf: undefined (defaults to false)`);
    console.log('   })');
    console.log('');

    const verifier = new ProtoWallet('anyone');

    try {
        const result = await verifier.verifySignature({
            signature: Utils.toArray(cert.signature, 'hex'),
            data: preimage,
            protocolID: [2, 'certificate signature'],
            keyID: `${type} ${serialNumber}`,
            counterparty: certifierIdentityKey.publicKey
            // forSelf is NOT specified, so it defaults to false
        });

        console.log(`   ✅ Verification result: ${JSON.stringify(result)}`);
        console.log('   ✅ Signature is VALID!');
        console.log('');

        // Now let's try to understand what happened
        console.log('='.repeat(80));
        console.log('Analysis: What actually happened?');
        console.log('='.repeat(80));
        console.log('');
        console.log('When signing:');
        console.log('  - counterparty = undefined (sent as 0 to wallet)');
        console.log('  - forSelf = undefined (not sent to wallet)');
        console.log('  - Wallet must use some default behavior');
        console.log('');
        console.log('When verifying:');
        console.log('  - counterparty = certifier_pubkey');
        console.log('  - forSelf = undefined (defaults to false)');
        console.log('  - Wallet computes: counterparty.deriveChild(anyone_rootKey, invoiceNumber)');
        console.log('');
        console.log('Since verification works, the wallet must be computing the SAME derived public key');
        console.log('in both cases. This means:');
        console.log('  - When counterparty = undefined, wallet likely uses forSelf: false');
        console.log('  - But what does it use as the counterparty?');
        console.log('    Option 1: Uses its own public key (self)');
        console.log('    Option 2: Uses something else?');
        console.log('');

    } catch (error) {
        console.log(`   ❌ Verification failed: ${error.message}`);
        console.log(`   Error stack: ${error.stack}`);
        console.log('');
    }

    // Let's also test what happens if we explicitly set forSelf: true during verification
    console.log('🧪 Step 5: Testing verification with forSelf: true...');
    try {
        const result = await verifier.verifySignature({
            signature: Utils.toArray(cert.signature, 'hex'),
            data: preimage,
            protocolID: [2, 'certificate signature'],
            keyID: `${type} ${serialNumber}`,
            counterparty: certifierIdentityKey.publicKey,
            forSelf: true
        });

        console.log(`   ✅ Verification with forSelf: true succeeded!`);
        console.log('   This means the wallet signed with forSelf: true!');
        console.log('');
    } catch (error) {
        console.log(`   ❌ Verification with forSelf: true failed: ${error.message}`);
        console.log('   This means the wallet signed with forSelf: false (or something else)');
        console.log('');
    }
}

testSignVerify().catch(console.error);
