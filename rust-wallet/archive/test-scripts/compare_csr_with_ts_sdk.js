/**
 * Compare Rust wallet CSR output with TypeScript SDK
 *
 * This script:
 * 1. Generates CSR using TypeScript SDK (reference)
 * 2. Takes Rust wallet CSR output (from logs or test)
 * 3. Compares them byte-for-byte
 */

const path = require('path');
const { execSync } = require('child_process');

// Load TypeScript SDK
let Utils, MasterCertificate, ProtoWallet, Random, sdk, Mnemonic, HD;
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
 * Generate CSR using TypeScript SDK (reference implementation)
 */
async function generateTSReferenceCSR(certifierPublicKey, certificateType, fields) {
    // Create wallet (same mnemonic as Rust test)
    const mnemonicStr = 'abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about';
    const mnemonic = new Mnemonic(mnemonicStr);
    mnemonic.mnemonic2Seed();
    const hdWallet = HD.fromSeed(mnemonic.seed);
    const wallet = new ProtoWallet(hdWallet.privKey);

    // Create clientNonce
    const clientNonce = Utils.toBase64(Random(32));

    // Create certificate fields and masterKeyring
    const { certificateFields, masterKeyring } = await MasterCertificate.createCertificateFields(
        wallet,
        certifierPublicKey,
        fields
    );

    // Build CSR JSON (exact order: clientNonce, type, fields, masterKeyring)
    const csr = {
        clientNonce: clientNonce,
        type: certificateType,
        fields: certificateFields,
        masterKeyring: masterKeyring
    };

    const csrJsonString = JSON.stringify(csr);
    const csrJsonBytes = Buffer.from(csrJsonString, 'utf8');

    return {
        clientNonce,
        csrJsonString,
        csrJsonBytes,
        certificateFields,
        masterKeyring,
        fieldOrder: Object.keys(csr)
    };
}

/**
 * Compare two CSR JSON strings byte-for-byte
 */
function compareCSR(tsCSR, rustCSR) {
    console.log('\n📊 CSR COMPARISON RESULTS');
    console.log('='.repeat(60));

    // 1. Compare field order
    console.log('\n1️⃣  Field Order:');
    const tsOrder = tsCSR.fieldOrder.join(' -> ');
    console.log(`   TypeScript SDK: ${tsOrder}`);

    // Parse Rust CSR to get field order
    let rustOrder = 'UNKNOWN';
    try {
        const rustParsed = JSON.parse(rustCSR.csrJsonString);
        rustOrder = Object.keys(rustParsed).join(' -> ');
        console.log(`   Rust Wallet:   ${rustOrder}`);

        if (tsOrder === rustOrder) {
            console.log('   ✅ Field order MATCHES');
        } else {
            console.log('   ❌ Field order DIFFERS');
            console.log(`   Expected: ${tsOrder}`);
            console.log(`   Got:      ${rustOrder}`);
        }
    } catch (e) {
        console.log(`   ⚠️  Could not parse Rust CSR: ${e.message}`);
    }

    // 2. Compare JSON length
    console.log('\n2️⃣  JSON Length:');
    console.log(`   TypeScript SDK: ${tsCSR.csrJsonBytes.length} bytes`);
    console.log(`   Rust Wallet:   ${rustCSR.csrJsonBytes.length} bytes`);
    if (tsCSR.csrJsonBytes.length === rustCSR.csrJsonBytes.length) {
        console.log('   ✅ Length MATCHES');
    } else {
        console.log('   ❌ Length DIFFERS');
    }

    // 3. Compare JSON bytes (hex)
    console.log('\n3️⃣  JSON Bytes (Hex):');
    const tsHex = tsCSR.csrJsonBytes.toString('hex');
    const rustHex = rustCSR.csrJsonBytes.toString('hex');

    if (tsHex === rustHex) {
        console.log('   ✅ JSON bytes MATCH exactly!');
    } else {
        console.log('   ❌ JSON bytes DIFFER');
        console.log(`   TypeScript SDK (first 100): ${tsHex.substring(0, 100)}...`);
        console.log(`   Rust Wallet (first 100):   ${rustHex.substring(0, 100)}...`);

        // Find first difference
        let diffIndex = -1;
        for (let i = 0; i < Math.min(tsHex.length, rustHex.length); i++) {
            if (tsHex[i] !== rustHex[i]) {
                diffIndex = i;
                break;
            }
        }
        if (diffIndex >= 0) {
            console.log(`   First difference at byte ${diffIndex}`);
            console.log(`   TS:   ${tsHex.substring(Math.max(0, diffIndex - 20), diffIndex + 20)}`);
            console.log(`   Rust: ${rustHex.substring(Math.max(0, diffIndex - 20), diffIndex + 20)}`);
        }
    }

    // 4. Compare field values
    console.log('\n4️⃣  Field Values:');
    try {
        const tsParsed = JSON.parse(tsCSR.csrJsonString);
        const rustParsed = JSON.parse(rustCSR.csrJsonString);

        const tsFields = tsParsed.fields || {};
        const rustFields = rustParsed.fields || {};

        const allFieldNames = new Set([...Object.keys(tsFields), ...Object.keys(rustFields)]);

        for (const fieldName of allFieldNames) {
            const tsValue = tsFields[fieldName];
            const rustValue = rustFields[fieldName];

            if (tsValue === rustValue) {
                console.log(`   ✅ ${fieldName}: MATCHES`);
            } else {
                console.log(`   ❌ ${fieldName}: DIFFERS`);
                console.log(`      TS:   ${tsValue ? tsValue.substring(0, 50) + '...' : 'MISSING'}`);
                console.log(`      Rust: ${rustValue ? rustValue.substring(0, 50) + '...' : 'MISSING'}`);
            }
        }
    } catch (e) {
        console.log(`   ⚠️  Could not compare fields: ${e.message}`);
    }

    // 5. Compare masterKeyring values
    console.log('\n5️⃣  MasterKeyring Values:');
    try {
        const tsParsed = JSON.parse(tsCSR.csrJsonString);
        const rustParsed = JSON.parse(rustCSR.csrJsonString);

        const tsKeyring = tsParsed.masterKeyring || {};
        const rustKeyring = rustParsed.masterKeyring || {};

        const allKeyringNames = new Set([...Object.keys(tsKeyring), ...Object.keys(rustKeyring)]);

        for (const keyringName of allKeyringNames) {
            const tsValue = tsKeyring[keyringName];
            const rustValue = rustKeyring[keyringName];

            if (tsValue === rustValue) {
                console.log(`   ✅ ${keyringName}: MATCHES`);
            } else {
                console.log(`   ❌ ${keyringName}: DIFFERS`);
                console.log(`      TS:   ${tsValue ? tsValue.substring(0, 50) + '...' : 'MISSING'}`);
                console.log(`      Rust: ${rustValue ? rustValue.substring(0, 50) + '...' : 'MISSING'}`);
            }
        }
    } catch (e) {
        console.log(`   ⚠️  Could not compare masterKeyring: ${e.message}`);
    }

    console.log('\n' + '='.repeat(60));
}

/**
 * Main comparison function
 */
async function compareCSRWithTS(rustCSRHex) {
    console.log('🔍 CSR Comparison Tool');
    console.log('='.repeat(60));

    // Test parameters (matching our Rust implementation)
    const certifierPublicKey = '0220529dc803041a83f4357864a09c717daa24397cf2f3fc3a5745ae08d30924fd';
    const certificateType = 'AGfk/WrT1eBDXpz3mcw386Zww2HmqcIn3uY6x4Af1eo=';
    const fields = { cool: true };

    console.log('\n📋 Test Parameters:');
    console.log(`   Certifier: ${certifierPublicKey}`);
    console.log(`   Type: ${certificateType}`);
    console.log(`   Fields: ${JSON.stringify(fields)}`);

    // Generate TypeScript SDK reference
    console.log('\n🔧 Generating TypeScript SDK reference CSR...');
    const tsCSR = await generateTSReferenceCSR(certifierPublicKey, certificateType, fields);
    console.log('   ✅ TypeScript SDK CSR generated');
    console.log(`   Field order: ${tsCSR.fieldOrder.join(' -> ')}`);
    console.log(`   JSON length: ${tsCSR.csrJsonBytes.length} bytes`);

    // Parse Rust CSR (from hex)
    console.log('\n🔧 Parsing Rust wallet CSR...');
    let rustCSR;
    try {
        const rustCSRBytes = Buffer.from(rustCSRHex, 'hex');
        const rustCSRString = rustCSRBytes.toString('utf8');
        rustCSR = {
            csrJsonString: rustCSRString,
            csrJsonBytes: rustCSRBytes
        };
        console.log('   ✅ Rust CSR parsed');
        console.log(`   JSON length: ${rustCSR.csrJsonBytes.length} bytes`);
    } catch (e) {
        console.error(`   ❌ Failed to parse Rust CSR: ${e.message}`);
        process.exit(1);
    }

    // Compare
    compareCSR(tsCSR, rustCSR);
}

// Get Rust CSR hex from command line argument or use example
const rustCSRHex = process.argv[2];

if (!rustCSRHex) {
    console.error('Usage: node compare_csr_with_ts_sdk.js <rust_csr_hex>');
    console.error('\nExample:');
    console.error('  node compare_csr_with_ts_sdk.js 7b22636c69656e744e6f6e6365223a22...');
    console.error('\nOr paste the hex from your Rust wallet logs (CSR JSON bytes)');
    process.exit(1);
}

compareCSRWithTS(rustCSRHex).catch(error => {
    console.error('❌ Error:', error);
    console.error(error.stack);
    process.exit(1);
});
