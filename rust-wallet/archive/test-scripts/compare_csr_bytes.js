/**
 * Compare CSR bytes from Rust logs with TypeScript SDK output
 *
 * This script helps compare the exact bytes from your Rust implementation
 * (extracted from logs) with what TypeScript SDK would produce.
 *
 * Usage: node compare_csr_bytes.js
 *
 * Paste your Rust log output (hex/base64) and this will help identify differences.
 */

const readline = require('readline');

console.log('CSR Byte Comparison Tool');
console.log('=========================\n');
console.log('This tool helps compare Rust CSR bytes with TypeScript SDK output.');
console.log('Paste your serialized request hex/base64 from your Rust logs.\n');

// Example: Extract from your logs
// From your logs, you should have:
// - Serialized request (hex, FULL)
// - CSR JSON (hex/base64)
// - Request nonce

// For now, let's create a function to parse and compare
function parseSerializedRequest(hexString) {
    const bytes = Buffer.from(hexString, 'hex');
    let offset = 0;

    console.log('Parsing Serialized Request:');
    console.log(`Total length: ${bytes.length} bytes\n`);

    // 1. Nonce (32 bytes)
    if (offset + 32 > bytes.length) {
        throw new Error('Not enough bytes for nonce');
    }
    const nonce = bytes.slice(offset, offset + 32);
    offset += 32;
    console.log(`[0..31] Nonce (32 bytes): ${nonce.toString('hex')}`);
    console.log(`        Base64: ${nonce.toString('base64')}\n`);

    // 2. Method VarInt + Method
    const methodVarInt = readVarInt(bytes, offset);
    offset = methodVarInt.offset;
    const methodLen = methodVarInt.value;
    const method = bytes.slice(offset, offset + methodLen).toString('utf8');
    offset += methodLen;
    console.log(`[${methodVarInt.start}..${methodVarInt.end}] Method VarInt: ${methodVarInt.bytes.toString('hex')} (value: ${methodLen})`);
    console.log(`[${methodVarInt.end + 1}..${offset - 1}] Method: "${method}" (${methodLen} bytes)\n`);

    // 3. Path VarInt + Path
    const pathVarInt = readVarInt(bytes, offset);
    offset = pathVarInt.offset;
    const pathLen = pathVarInt.value;
    const path = bytes.slice(offset, offset + pathLen).toString('utf8');
    offset += pathLen;
    console.log(`[${pathVarInt.start}..${pathVarInt.end}] Path VarInt: ${pathVarInt.bytes.toString('hex')} (value: ${pathLen})`);
    console.log(`[${pathVarInt.end + 1}..${offset - 1}] Path: "${path}" (${pathLen} bytes)\n`);

    // 4. Search VarInt
    const searchVarInt = readVarInt(bytes, offset);
    offset = searchVarInt.offset;
    console.log(`[${searchVarInt.start}..${searchVarInt.end}] Search VarInt: ${searchVarInt.bytes.toString('hex')} (value: ${searchVarInt.value})\n`);

    // 5. Header count VarInt
    const headerCountVarInt = readVarInt(bytes, offset);
    offset = headerCountVarInt.offset;
    const headerCount = headerCountVarInt.value;
    console.log(`[${headerCountVarInt.start}..${headerCountVarInt.end}] Header Count VarInt: ${headerCountVarInt.bytes.toString('hex')} (value: ${headerCount})\n`);

    // 6. Headers
    for (let i = 0; i < headerCount; i++) {
        const keyVarInt = readVarInt(bytes, offset);
        offset = keyVarInt.offset;
        const keyLen = keyVarInt.value;
        const key = bytes.slice(offset, offset + keyLen).toString('utf8');
        offset += keyLen;

        const valueVarInt = readVarInt(bytes, offset);
        offset = valueVarInt.offset;
        const valueLen = valueVarInt.value;
        const value = bytes.slice(offset, offset + valueLen).toString('utf8');
        offset += valueLen;

        console.log(`  Header ${i + 1}:`);
        console.log(`    Key VarInt: ${keyVarInt.bytes.toString('hex')} (value: ${keyLen})`);
        console.log(`    Key: "${key}"`);
        console.log(`    Value VarInt: ${valueVarInt.bytes.toString('hex')} (value: ${valueLen})`);
        console.log(`    Value: "${value}"\n`);
    }

    // 7. Body VarInt + Body
    const bodyVarInt = readVarInt(bytes, offset);
    offset = bodyVarInt.offset;
    const bodyLen = bodyVarInt.value;
    const body = bytes.slice(offset, offset + bodyLen);
    offset += bodyLen;
    console.log(`[${bodyVarInt.start}..${bodyVarInt.end}] Body Length VarInt: ${bodyVarInt.bytes.toString('hex')} (value: ${bodyLen})`);
    console.log(`[${bodyVarInt.end + 1}..${offset - 1}] Body (${bodyLen} bytes):`);
    console.log(`  Hex (first 100): ${body.slice(0, 100).toString('hex')}...`);
    console.log(`  UTF-8 (first 200): ${body.slice(0, 200).toString('utf8')}...\n`);

    return {
        nonce,
        method,
        path,
        searchValue: searchVarInt.value,
        headers: [],
        body: body.toString('utf8'),
        bodyBytes: body
    };
}

function readVarInt(bytes, offset) {
    const start = offset;
    if (offset >= bytes.length) {
        throw new Error('Not enough bytes for VarInt');
    }

    const firstByte = bytes[offset];
    let value, bytesRead;

    if (firstByte < 0xFD) {
        value = firstByte;
        bytesRead = 1;
    } else if (firstByte === 0xFD) {
        if (offset + 3 > bytes.length) throw new Error('Not enough bytes for VarInt');
        value = bytes.readUInt16LE(offset + 1);
        bytesRead = 3;
    } else if (firstByte === 0xFE) {
        if (offset + 5 > bytes.length) throw new Error('Not enough bytes for VarInt');
        value = bytes.readUInt32LE(offset + 1);
        bytesRead = 5;
    } else { // 0xFF
        if (offset + 9 > bytes.length) throw new Error('Not enough bytes for VarInt');
        // For signed VarInt, 0xFF means read 8 bytes as unsigned
        const unsigned = bytes.readBigUInt64LE(offset + 1);
        // Convert from unsigned to signed (if > 2^63, it's negative)
        if (unsigned >= BigInt('0x8000000000000000')) {
            value = Number(unsigned - BigInt('0x10000000000000000'));
        } else {
            value = Number(unsigned);
        }
        bytesRead = 9;
    }

    return {
        start,
        end: start + bytesRead - 1,
        value,
        offset: start + bytesRead,
        bytes: bytes.slice(start, start + bytesRead)
    };
}

// Export for use by other scripts
module.exports = { parseSerializedRequest, readVarInt };

// Example usage - paste your hex from logs here
const exampleHex = process.argv[2];
if (exampleHex) {
    try {
        parseSerializedRequest(exampleHex);
        console.log('\n✅ Parsing complete!');
        console.log('\n📊 Next steps:');
        console.log('   1. Compare each section with TypeScript SDK output');
        console.log('   2. Pay special attention to:');
        console.log('      - VarInt encodings (especially -1 for search)');
        console.log('      - Header key/value casing');
        console.log('      - Body (CSR JSON) field order');
    } catch (error) {
        console.error('❌ Error parsing:', error.message);
        console.error(error.stack);
    }
} else {
    console.log('Usage: node compare_csr_bytes.js <hex_string>');
    console.log('\nExample:');
    console.log('  node compare_csr_bytes.js "deadbeef..."');
    console.log('\nOr use: node extract_and_compare.js --file <log_file>');
    console.log('Or paste your "Serialized request (hex, FULL)" from your Rust logs.');
}
