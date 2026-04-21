const { Certificate, ProtoWallet, Utils } = require('../reference/ts-brc100/node_modules/@bsv/sdk/dist/cjs/src/index.js');

// Test certificate verification to see what the SDK computes
async function testVerification() {
    console.log('Testing certificate verification with SDK...\n');
    
    // Create a test certificate (matching what the server returns)
    const type = 'AGfk/WrT1eBDXpz3mcw386Zww2HmqcIn3uY6x4Af1eo=';
    const serialNumber = 'zAqbEGs/JkXGrKysPcaQQtnATxGaMFkPf49LtESkd9o=';
    const subject = '020b95583e18ac933d89a131f399890098dc1b3d4a8abcdde3eec4a7b191d2521e';
    const certifier = '0220529dc803041a83f4357864a09c717daa24397cf2f3fc3a5745ae08d30924fd';
    const revocationOutpoint = '0000000000000000000000000000000000000000000000000000000000000000.0';
    const fields = {
        cool: 'C1UGmgHf3l5g4s5V90l6oL1cmbDgLQ/bUxOf3jT1VzfsCR+y/l7u3Bs+wftS5BMbP9Ds+w=='
    };
    const signature = '304402202614640bb0c39ba1269c855041c092aa3d203fce62d471e90c993637258593e702204fa4892a8a466e00c6d5df12e21670f911a7c82b60359b2859484bd71f729216';
    
    const cert = new Certificate(
        type,
        serialNumber,
        subject,
        certifier,
        revocationOutpoint,
        fields
    );
    cert.signature = signature;
    
    console.log('Certificate created:');
    console.log(`  Type: ${type}`);
    console.log(`  SerialNumber: ${serialNumber}`);
    console.log(`  Subject: ${subject}`);
    console.log(`  Certifier: ${certifier}`);
    console.log(`  Signature: ${signature.substring(0, 20)}...`);
    console.log('');
    
    // Create verifier (matching SDK's verify() method)
    const verifier = new ProtoWallet('anyone');
    console.log('Verifier created: ProtoWallet("anyone")');
    console.log('');
    
    // Get verification data (preimage)
    const verificationData = cert.toBinary(false);
    console.log(`Verification data (preimage) length: ${verificationData.length} bytes`);
    console.log(`Verification data (hex, first 64): ${Utils.toHex(verificationData.slice(0, 64))}`);
    console.log('');
    
    // Call verifySignature (matching SDK's verify() method)
    console.log('Calling verifySignature with:');
    console.log(`  protocolID: [2, 'certificate signature']`);
    console.log(`  keyID: '${type} ${serialNumber}'`);
    console.log(`  counterparty: '${certifier}'`);
    console.log(`  forSelf: undefined (defaults to false)`);
    console.log('');
    
    try {
        const result = await verifier.verifySignature({
            signature: Utils.toArray(signature, 'hex'),
            data: verificationData,
            protocolID: [2, 'certificate signature'],
            keyID: `${type} ${serialNumber}`,
            counterparty: certifier
        });
        
        console.log('✅ Verification result:', result);
        console.log('✅ Signature is VALID!');
    } catch (error) {
        console.log('❌ Verification failed:', error.message);
        console.log('❌ Signature is INVALID!');
    }
}

testVerification().catch(console.error);

