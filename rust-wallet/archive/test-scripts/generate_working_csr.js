/**
 * Generate a working CSR using TypeScript SDK with same inputs as Metanet client
 * 
 * This will show us what a working CSR looks like so we can compare with our Rust wallet
 */

const path = require('path');

// Load TypeScript SDK
let Utils, ProtoWallet, Random, sdk, Mnemonic, HD, MasterCertificate, AuthFetch;
try {
    const sdkPath = path.join(__dirname, '..', 'reference', 'ts-brc100', 'node_modules', '@bsv', 'sdk');
    sdk = require(sdkPath);
    Utils = sdk.Utils || sdk;
    MasterCertificate = sdk.MasterCertificate;
    ProtoWallet = sdk.ProtoWallet;
    Random = sdk.Random;
    AuthFetch = sdk.AuthFetch;
    
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

async function generateWorkingCSR() {
    console.log('🔍 Generating Working CSR (TypeScript SDK)');
    console.log('='.repeat(70));
    console.log('');
    
    // Use the same inputs as Metanet client
    const certifier = '0220529dc803041a83f4357864a09c717daa24397cf2f3fc3a5745ae08d30924fd';
    const certificateType = 'AGfk/WrT1eBDXpz3mcw386Zww2HmqcIn3uY6x4Af1eo=';
    const fields = { cool: 'true' };  // Note: Metanet client sends "true" as string
    
    console.log('📋 Input Parameters (matching Metanet client):');
    console.log(`   Certifier: ${certifier}`);
    console.log(`   Type: ${certificateType}`);
    console.log(`   Fields: ${JSON.stringify(fields)}`);
    console.log('');
    
    // Create a test wallet (we'll use a known mnemonic for reproducibility)
    const mnemonicStr = 'abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about';
    const mnemonic = new Mnemonic(mnemonicStr);
    mnemonic.mnemonic2Seed();
    const hdWallet = HD.fromSeed(mnemonic.seed);
    const wallet = new ProtoWallet(hdWallet.privKey);
    
    const identityKeyResult = await wallet.getPublicKey({ identityKey: true });
    console.log(`   Wallet identity key: ${identityKeyResult.publicKey}`);
    console.log('');
    
    // Create clientNonce (as TypeScript SDK does)
    const clientNonce = await sdk.createNonce(wallet, certifier);
    console.log('🔧 Creating certificate fields and masterKeyring...');
    console.log(`   Client nonce: ${clientNonce}`);
    console.log('');
    
    // Create certificate fields and masterKeyring
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
    
    console.log('📋 WORKING CSR (TypeScript SDK):');
    console.log('='.repeat(70));
    console.log('');
    console.log('CSR JSON:');
    console.log(csrJsonString);
    console.log('');
    console.log('CSR JSON Details:');
    console.log(`   Length: ${csrJsonBytes.length} bytes`);
    console.log(`   Hex (full): ${csrJsonBytes.toString('hex')}`);
    console.log(`   Base64 (full): ${csrJsonBytes.toString('base64')}`);
    console.log('');
    
    console.log('📋 Field Values:');
    for (const [key, value] of Object.entries(certificateFields)) {
        console.log(`   ${key}: ${value}`);
        console.log(`      Length: ${value.length} chars`);
    }
    console.log('');
    
    console.log('📋 MasterKeyring Values:');
    for (const [key, value] of Object.entries(masterKeyring)) {
        console.log(`   ${key}: ${value}`);
        console.log(`      Length: ${value.length} chars`);
    }
    console.log('');
    
    console.log('📋 Field Order:');
    const parsed = JSON.parse(csrJsonString);
    console.log(`   Top-level: ${Object.keys(parsed).join(' -> ')}`);
    console.log(`   Fields keys: ${Object.keys(parsed.fields).join(', ')}`);
    console.log(`   MasterKeyring keys: ${Object.keys(parsed.masterKeyring).join(', ')}`);
    console.log('');
    
    console.log('='.repeat(70));
    console.log('✅ Working CSR generated!');
    console.log('');
    console.log('Compare this with your Rust wallet CSR:');
    console.log('   - Field order should match');
    console.log('   - Field values will differ (different encryption keys/IVs)');
    console.log('   - MasterKeyring values will differ (different encryption keys/IVs)');
    console.log('   - But the STRUCTURE and FORMAT should match exactly');
    console.log('');
}

generateWorkingCSR().catch(error => {
    console.error('❌ Error:', error);
    console.error(error.stack);
    process.exit(1);
});

