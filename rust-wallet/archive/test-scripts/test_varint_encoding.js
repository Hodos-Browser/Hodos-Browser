/**
 * Test VarInt encoding for -1 to verify it matches Rust implementation
 */

const path = require('path');

// Try to load the TypeScript SDK
let Utils;
try {
    const sdkPath = path.join(__dirname, '..', 'reference', 'ts-brc100', 'node_modules', '@bsv', 'sdk');
    const sdk = require(sdkPath);
    Utils = sdk.Utils || sdk;
} catch (error) {
    console.error('Failed to load TypeScript SDK:', error.message);
    process.exit(1);
}

console.log('VarInt Encoding Test');
console.log('====================\n');

// Test encoding -1
const writer = new Utils.Writer();
writer.writeVarIntNum(-1);
const encoded = writer.toArray();

console.log('Encoding -1:');
console.log('  Bytes:', encoded.map(b => '0x' + b.toString(16).padStart(2, '0')).join(', '));
console.log('  Hex:', encoded.map(b => b.toString(16).padStart(2, '0')).join(''));
console.log('  Length:', encoded.length, 'bytes');
console.log('  Base64:', Buffer.from(encoded).toString('base64'));

// Test encoding other values for reference
console.log('\nOther test values:');
const testValues = [0, 1, 252, 253, 255, 256, 65535, 65536, 4294967295];
for (const val of testValues) {
    const w = new Utils.Writer();
    w.writeVarIntNum(val);
    const bytes = w.toArray();
    console.log(`  ${val}: [${bytes.map(b => '0x' + b.toString(16).padStart(2, '0')).join(', ')}] (${bytes.length} bytes)`);
}

// Test negative values
console.log('\nNegative values:');
const negValues = [-1, -2, -100];
for (const val of negValues) {
    const w = new Utils.Writer();
    w.writeVarIntNum(val);
    const bytes = w.toArray();
    console.log(`  ${val}: [${bytes.map(b => '0x' + b.toString(16).padStart(2, '0')).join(', ')}] (${bytes.length} bytes)`);
    console.log(`    Hex: ${bytes.map(b => b.toString(16).padStart(2, '0')).join('')}`);
}

