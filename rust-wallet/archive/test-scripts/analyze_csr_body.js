/**
 * Analyze the CSR JSON body to check for issues
 */

const hex = process.argv[2] || '7b22636c69656e744e6f6e6365223a2242636c6565445249527532352f423976583937363435676b61746f376a62374f72685174743263426972733d222c2274797065223a224147666b2f5772543165424458707a336d63773338365a777732486d7163496e337559367834416631656f3d222c226669656c6473223a7b22636f6f6c223a2248546b5553614b564163642f6556426f68786668464d4155696b746736487a324557767461352f74556d3262686e425242726152576537496a375747763249735738343670673d3d227d2c226d61737465724b657972696e67223a7b22636f6f6c223a22696d6f54596a364f6d435130694470384d4155655342566e72614e2b3037786d7777384f77637778765373374d6c48706e424b54395a6969776f69336e5a41367a5a345838456364594c4959313344356d4a2f2f366e49672f4e57387a75704e594571577a3650635a36343d227d7d';

console.log('Analyzing CSR JSON Body');
console.log('========================\n');

const jsonBytes = Buffer.from(hex, 'hex');
const jsonString = jsonBytes.toString('utf8');

console.log('JSON String:');
console.log(jsonString);
console.log('');

try {
    const parsed = JSON.parse(jsonString);

    console.log('✅ Valid JSON');
    console.log('');

    // Check field order
    const keys = Object.keys(parsed);
    console.log('Field order:', keys.join(', '));
    console.log('');

    const expectedOrder = ['clientNonce', 'type', 'fields', 'masterKeyring'];
    const orderMatches = JSON.stringify(keys) === JSON.stringify(expectedOrder);

    if (orderMatches) {
        console.log('✅ Field order matches TypeScript SDK:', expectedOrder.join(', '));
    } else {
        console.log('❌ Field order mismatch!');
        console.log('   Expected:', expectedOrder.join(', '));
        console.log('   Found:   ', keys.join(', '));
    }
    console.log('');

    // Check structure
    console.log('Structure:');
    console.log('  clientNonce:', typeof parsed.clientNonce, `(${parsed.clientNonce?.length || 0} chars)`);
    console.log('  type:', typeof parsed.type, `(${parsed.type?.length || 0} chars)`);
    console.log('  fields:', typeof parsed.fields, `(${Object.keys(parsed.fields || {}).length} field(s))`);
    console.log('  masterKeyring:', typeof parsed.masterKeyring, `(${Object.keys(parsed.masterKeyring || {}).length} key(s))`);
    console.log('');

    // Check if fields and masterKeyring have matching keys
    const fieldKeys = Object.keys(parsed.fields || {});
    const keyringKeys = Object.keys(parsed.masterKeyring || {});

    console.log('Field/Keyring Key Matching:');
    if (JSON.stringify(fieldKeys.sort()) === JSON.stringify(keyringKeys.sort())) {
        console.log('✅ Fields and masterKeyring have matching keys:', fieldKeys.join(', '));
    } else {
        console.log('❌ Fields and masterKeyring keys do not match!');
        console.log('   Fields:', fieldKeys.join(', '));
        console.log('   Keyring:', keyringKeys.join(', '));
    }
    console.log('');

    // Check JSON stringification (how TypeScript SDK would serialize it)
    const tsStyle = JSON.stringify(parsed);
    console.log('TypeScript SDK JSON.stringify() would produce:');
    console.log(tsStyle);
    console.log('');

    // Compare byte-by-byte
    const tsBytes = Buffer.from(tsStyle, 'utf8');
    const ourBytes = jsonBytes;

    console.log('Byte Comparison:');
    console.log(`  Our JSON length: ${ourBytes.length} bytes`);
    console.log(`  TS SDK length:  ${tsBytes.length} bytes`);

    if (ourBytes.length === tsBytes.length) {
        console.log('✅ Lengths match');
    } else {
        console.log('❌ Length mismatch!');
    }

    // Compare byte-by-byte
    let firstDiff = -1;
    for (let i = 0; i < Math.min(ourBytes.length, tsBytes.length); i++) {
        if (ourBytes[i] !== tsBytes[i]) {
            firstDiff = i;
            break;
        }
    }

    if (firstDiff === -1 && ourBytes.length === tsBytes.length) {
        console.log('✅ Byte-for-byte match with TypeScript SDK!');
    } else if (firstDiff === -1) {
        console.log('⚠️  Bytes match up to length difference');
    } else {
        console.log(`❌ First difference at byte ${firstDiff}`);
        console.log(`   Our byte: 0x${ourBytes[firstDiff].toString(16)} (${String.fromCharCode(ourBytes[firstDiff])})`);
        console.log(`   TS byte:  0x${tsBytes[firstDiff].toString(16)} (${String.fromCharCode(tsBytes[firstDiff])})`);
        console.log(`   Context: "${ourBytes.slice(Math.max(0, firstDiff-10), firstDiff+10).toString('utf8')}"`);
    }
    console.log('');

    // Show hex for comparison
    console.log('Hex Comparison:');
    console.log('  Our hex:', ourBytes.toString('hex'));
    console.log('  TS hex: ', tsBytes.toString('hex'));
    console.log('');

    // Check if the issue might be whitespace or formatting
    const ourCompact = JSON.stringify(JSON.parse(jsonString));
    if (ourCompact === tsStyle) {
        console.log('✅ JSON content is identical (ignoring formatting)');
    } else {
        console.log('⚠️  JSON content differs even after re-parsing');
    }

} catch (error) {
    console.error('❌ Invalid JSON:', error.message);
    console.log('Raw bytes:', jsonBytes.toString('hex'));
}

