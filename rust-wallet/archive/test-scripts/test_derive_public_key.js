const path = require('path');
// Import from the same location as test_ts_sdk_server.js
const sdkPath = path.join(__dirname, '..', 'reference', 'ts-brc100', 'node_modules', '@bsv', 'sdk');
const sdk = require(sdkPath);
const Utils = sdk.Utils || sdk;
const ProtoWallet = sdk.ProtoWallet;
const PrivateKey = sdk.PrivateKey;

// Load compat modules for mnemonic/HD wallet
const compatPath = path.join(__dirname, '..', 'reference', 'ts-brc100', 'node_modules', '@bsv', 'sdk', 'dist', 'cjs', 'src', 'compat');
const MnemonicModule = require(path.join(compatPath, 'Mnemonic.js'));
const Mnemonic = MnemonicModule.default || MnemonicModule;
const HDModule = require(path.join(compatPath, 'HD.js'));
const HD = HDModule.default || HDModule;

// Test to see what the SDK actually derives when verifying
async function testDerivePublicKey() {
    console.log('='.repeat(80));
    console.log('Testing SDK Key Derivation for Certificate Verification');
    console.log('='.repeat(80));
    console.log('');

    // Create a certifier wallet (simulating the server)
    const certifierMnemonic = 'abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about';
    const mnemonic = new Mnemonic(certifierMnemonic);
    mnemonic.mnemonic2Seed();
    const hdWallet = HD.fromSeed(mnemonic.seed);
    const masterPrivateKey = hdWallet.privKey;
    const certifierWallet = new ProtoWallet(masterPrivateKey);
    const certifierIdentityKey = await certifierWallet.getPublicKey({ identityKey: true });
    const certifierPubkey = certifierIdentityKey.publicKey;

    console.log('📋 Test Parameters:');
    console.log(`   Certifier public key: ${certifierPubkey}`);
    console.log('');

    // Test invoice number (matching what we use)
    const type = 'AGfk/WrT1eBDXpz3mcw386Zww2HmqcIn3uY6x4Af1eo=';
    const serialNumber = 'zAqbEGs/JkXGrKysPcaQQtnATxGaMFkPf49LtESkd9o=';
    const keyID = `${type} ${serialNumber}`;
    const protocolID = [2, 'certificate signature'];

    console.log('📋 Invoice Number Details:');
    console.log(`   ProtocolID: [${protocolID[0]}, '${protocolID[1]}']`);
    console.log(`   KeyID: '${keyID}'`);
    console.log('');

    // Create verifier (matching SDK's verify() method)
    const verifier = new ProtoWallet('anyone');
    console.log('🔍 Step 1: Creating verifier with ProtoWallet("anyone")...');
    const anyonePubkey = await verifier.getPublicKey({ identityKey: true });
    console.log(`   Anyone public key: ${anyonePubkey.publicKey}`);
    console.log('');

    // Get the derived public key using the SDK's method
    console.log('🔍 Step 2: Deriving public key using SDK\'s verifySignature logic...');
    console.log('   This is what verifySignature does internally:');
    console.log(`   - verifier.derivePublicKey(protocolID, keyID, counterparty, forSelf: false)`);
    console.log(`   - protocolID: [${protocolID[0]}, '${protocolID[1]}']`);
    console.log(`   - keyID: '${keyID}'`);
    console.log(`   - counterparty: '${certifierPubkey}'`);
    console.log(`   - forSelf: false (default)`);
    console.log('');

    // Access the KeyDeriver to get the derived public key
    // The SDK's verifySignature internally calls derivePublicKey
    // Let's manually call it to see what it derives
    const derivedPubkey = verifier.keyDeriver.derivePublicKey(
        protocolID,
        keyID,
        certifierPubkey,
        false // forSelf: false
    );

    console.log(`   ✅ Derived public key: ${derivedPubkey.toString()}`);
    console.log(`   Derived public key (hex): ${derivedPubkey.toString()}`);
    console.log('');

    // Now let's also check what happens when signing (to compare)
    console.log('✍️  Step 3: What happens when signing (for comparison)...');
    console.log('   When server signs with counterparty: undefined, the wallet:');
    console.log('   - Treats it as counterparty = certifier_pubkey with forSelf: false');
    console.log('   - Computes: certifier_pubkey.deriveChild(certifier_privkey, invoiceNumber)');
    console.log('');

    // Get the certifier's derived public key (what it uses for signing)
    // Test different counterparty options
    console.log('   Testing different counterparty options for signing:');
    console.log('');

    // Option 1: Using its own public key as counterparty (forSelf: false)
    const certifierDerivedPubkey1 = certifierWallet.keyDeriver.derivePublicKey(
        protocolID,
        keyID,
        certifierPubkey, // Using its own public key as counterparty
        false // forSelf: false
    );
    console.log(`   Option 1 (counterparty=pubkey, forSelf=false): ${certifierDerivedPubkey1.toString()}`);

    // Option 2: Using 'self' as counterparty (forSelf: false)
    const certifierDerivedPubkey2 = certifierWallet.keyDeriver.derivePublicKey(
        protocolID,
        keyID,
        'self',
        false // forSelf: false
    );
    console.log(`   Option 2 (counterparty='self', forSelf=false): ${certifierDerivedPubkey2.toString()}`);

    // Option 3: Using 'self' as counterparty (forSelf: true)
    const certifierDerivedPubkey3 = certifierWallet.keyDeriver.derivePublicKey(
        protocolID,
        keyID,
        'self',
        true // forSelf: true
    );
    console.log(`   Option 3 (counterparty='self', forSelf=true): ${certifierDerivedPubkey3.toString()}`);

    // Option 4: Using its own public key as counterparty (forSelf: true)
    const certifierDerivedPubkey4 = certifierWallet.keyDeriver.derivePublicKey(
        protocolID,
        keyID,
        certifierPubkey,
        true // forSelf: true
    );
    console.log(`   Option 4 (counterparty=pubkey, forSelf=true): ${certifierDerivedPubkey4.toString()}`);
    console.log('');

    // Use the first one for comparison
    const certifierDerivedPubkey = certifierDerivedPubkey1;

    // Compare
    console.log('🔍 Step 4: Comparison...');
    console.log(`   Verifying key (forSelf=false): ${derivedPubkey.toString()}`);
    console.log('');

    const matches = [
        { name: 'Option 1 (counterparty=pubkey, forSelf=false)', key: certifierDerivedPubkey1 },
        { name: 'Option 2 (counterparty=self, forSelf=false)', key: certifierDerivedPubkey2 },
        { name: 'Option 3 (counterparty=self, forSelf=true)', key: certifierDerivedPubkey3 },
        { name: 'Option 4 (counterparty=pubkey, forSelf=true)', key: certifierDerivedPubkey4 },
    ];

    let foundMatch = false;
    for (const option of matches) {
        if (derivedPubkey.toString() === option.key.toString()) {
            console.log(`   ✅ MATCH FOUND: ${option.name}`);
            console.log('   This is what the server uses when signing!');
            foundMatch = true;
            break;
        }
    }

    if (!foundMatch) {
        console.log('   ❌ NO MATCH FOUND!');
        console.log('   This is strange - the SDK verification works, so there must be a match somewhere.');
        console.log('   Maybe the wallet handles counterparty=undefined differently?');
    }
    console.log('');

    // Test with forSelf: true
    console.log('🧪 Step 5: Testing with forSelf: true...');
    const derivedPubkeyForSelf = verifier.keyDeriver.derivePublicKey(
        protocolID,
        keyID,
        certifierPubkey,
        true // forSelf: true
    );

    console.log(`   Derived public key (forSelf: true): ${derivedPubkeyForSelf.toString()}`);
    console.log('');

    // CRITICAL: When server signs with counterparty: undefined, createSignature defaults to 'anyone'!
    // Let's test what happens when we verify with counterparty: 'anyone'
    console.log('🧪 Step 6: Testing with counterparty: "anyone" (matching createSignature default)...');
    console.log('   When server signs: counterparty: undefined → defaults to "anyone"');
    console.log('   So it derives: anyone.derivePrivateKey(...)');
    console.log('   When we verify, we should use: anyone.derivePublicKey(..., "anyone", ...)');
    console.log('');

    // But wait - we can't verify with 'anyone' as counterparty because we need the certifier's key
    // Actually, let me check what the SDK's Certificate.verify() does
    console.log('   Actually, Certificate.verify() uses:');
    console.log('   - verifier = new ProtoWallet("anyone")');
    console.log('   - counterparty: this.certifier (certifier_pubkey)');
    console.log('   - forSelf: undefined (defaults to false)');
    console.log('');

    // So the issue is: when signing, it uses 'anyone' as counterparty
    // But when verifying, it uses certifier_pubkey as counterparty
    // These are different!

    // Unless... maybe the wallet (CWI/XDM) handles counterparty: undefined differently?
    // Maybe it uses the certifier's own public key, not 'anyone'?

    console.log('   The real wallet (CWI/XDM) might handle counterparty: undefined differently');
    console.log('   than ProtoWallet. It might use the certifier\'s own public key.');
    console.log('');
}

testDerivePublicKey().catch(console.error);
