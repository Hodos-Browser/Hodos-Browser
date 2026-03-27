#!/usr/bin/env node
// Full certificate acquisition flow test:
// 1. Simulates our Rust code's nonce creation
// 2. Tests SDK verifyNonce (what the server does)
// 3. Simulates our Rust code's field encryption
// 4. Tests SDK MasterCertificate.decryptFields (what the server does)
//
// This tests the EXACT operations that fail on the server, using the SDK.
// No server needed — just runs both sides in-process.

import { createRequire } from 'module';
const require = createRequire(import.meta.url);
const sdkPath = '../reference/ts-brc100/node_modules/@bsv/sdk';

const {
  ProtoWallet, PrivateKey, Utils, SymmetricKey, Random,
  MasterCertificate, Certificate, verifyNonce, createNonce, Hash
} = require(sdkPath);
const { KeyDeriver } = require(sdkPath + '/dist/cjs/src/wallet/KeyDeriver.js');
const { AESGCM } = require(sdkPath + '/dist/cjs/src/primitives/AESGCM.js');

// ========== SETUP ==========
// Our wallet (client)
const CLIENT_PRIV_HEX = 'e8f32e723decf4051aefac8e2c93c9c5b214313817cdb01a1494b917c8436b35';
const clientPrivKey = PrivateKey.fromString(CLIENT_PRIV_HEX, 16);
const clientWallet = new ProtoWallet(clientPrivKey);
const clientDeriver = new KeyDeriver(clientPrivKey);

// Server wallet (certifier)
const SERVER_PRIV_HEX = 'dc38f15198fc8cd92a920fd07fc715d223dbca120e523e636a7b835aa932ce36';
const serverPrivKey = PrivateKey.fromString(SERVER_PRIV_HEX, 16);
const serverWallet = new ProtoWallet(serverPrivKey);
const serverDeriver = new KeyDeriver(serverPrivKey);

async function main() {
  const clientPubKey = (await clientWallet.getPublicKey({ identityKey: true })).publicKey;
  const serverPubKey = (await serverWallet.getPublicKey({ identityKey: true })).publicKey;
  console.log('Client pubkey:', clientPubKey);
  console.log('Server pubkey:', serverPubKey);

  // ========== TEST A: SDK createNonce + verifyNonce ==========
  console.log('\n=== TEST A: SDK createNonce + SDK verifyNonce ===');
  {
    // Client creates nonce for server (counterparty = server's key)
    const nonce = await createNonce(clientWallet, serverPubKey);
    console.log('SDK nonce:', nonce.substring(0, 30) + '...');

    // Server verifies (counterparty = client's key)
    try {
      const valid = await verifyNonce(nonce, serverWallet, clientPubKey);
      console.log('SDK verifyNonce:', valid ? '✅ PASS' : '❌ FAIL');
    } catch (e) {
      console.log('SDK verifyNonce THREW:', e.message, e.code || '');
    }
  }

  // ========== TEST B: Rust-simulated createNonce + SDK verifyNonce ==========
  console.log('\n=== TEST B: Rust-simulated nonce + SDK verifyNonce ===');
  {
    // Simulate our Rust create_nonce_with_hmac EXACTLY
    const firstHalf = Array.from(Random(16));
    console.log('firstHalf (hex):', firstHalf.map(b => b.toString(16).padStart(2, '0')).join(''));

    // Rust: js_to_utf8(&first_half)
    const keyID = Utils.toUTF8(firstHalf);
    console.log('keyID:', JSON.stringify(keyID), `(${keyID.length} chars)`);

    // Rust: derive_symmetric_key_for_hmac with server's key as counterparty
    const serverPubKeyObj = clientPrivKey.toPublicKey(); // wait no, we need the server's pubkey
    const symKey = clientDeriver.deriveSymmetricKey(
      [2, 'server hmac'],
      keyID,
      PrivateKey.fromString(SERVER_PRIV_HEX, 16).toPublicKey()
    );

    // Rust: strip leading zeros
    const symKeyStripped = symKey.toArray(); // BigNumber.toArray() already strips
    console.log('symKey stripped (' + symKeyStripped.length + ' bytes):',
      symKeyStripped.map(b => b.toString(16).padStart(2, '0')).join(''));

    // Rust: hmac_sha256(stripped_key, first_half)
    const hmac = Hash.sha256hmac(symKeyStripped, firstHalf);
    console.log('HMAC:', hmac.map(b => b.toString(16).padStart(2, '0')).join(''));

    // Rust: base64(firstHalf + hmac)
    const nonce = Utils.toBase64([...firstHalf, ...hmac]);
    console.log('Nonce:', nonce.substring(0, 30) + '...');

    // Now verify using SDK (what the server does)
    try {
      const valid = await verifyNonce(nonce, serverWallet, clientPubKey);
      console.log('SDK verifyNonce:', valid ? '✅ PASS' : '❌ FAIL');
    } catch (e) {
      console.log('SDK verifyNonce THREW:', e.message, e.code || '');

      // Debug: what does the server compute?
      const buffer = Utils.toArray(nonce, 'base64');
      const data = buffer.slice(0, 16);
      const hmacFromNonce = buffer.slice(16);

      console.log('  Server sees data (hex):', data.map(b => b.toString(16).padStart(2, '0')).join(''));
      console.log('  Server sees hmac (hex):', hmacFromNonce.map(b => b.toString(16).padStart(2, '0')).join(''));

      const serverKeyID = Utils.toUTF8(data);
      console.log('  Server keyID:', JSON.stringify(serverKeyID));
      console.log('  Our keyID:   ', JSON.stringify(keyID));
      console.log('  keyIDs match:', serverKeyID === keyID ? '✅' : '❌');

      // Server derives symmetric key
      const serverSymKey = serverDeriver.deriveSymmetricKey(
        [2, 'server hmac'],
        serverKeyID,
        PrivateKey.fromString(CLIENT_PRIV_HEX, 16).toPublicKey()
      );
      const serverStripped = serverSymKey.toArray();
      console.log('  Server symKey stripped:', serverStripped.map(b => b.toString(16).padStart(2, '0')).join(''));
      console.log('  Client symKey stripped:', symKeyStripped.map(b => b.toString(16).padStart(2, '0')).join(''));
      console.log('  Keys match:', serverStripped.every((b, i) => b === symKeyStripped[i]) && serverStripped.length === symKeyStripped.length ? '✅' : '❌');

      const expectedHmac = Hash.sha256hmac(serverStripped, data);
      console.log('  Server expected HMAC:', expectedHmac.map(b => b.toString(16).padStart(2, '0')).join(''));
      console.log('  HMACs match:', expectedHmac.every((b, i) => b === hmacFromNonce[i]) ? '✅' : '❌');
    }
  }

  // ========== TEST C: Rust-simulated field encryption + SDK decryptFields ==========
  console.log('\n=== TEST C: Rust-simulated field encryption + SDK decryptFields ===');
  {
    const fieldName = 'cool';
    const fieldValue = 'true';
    const fieldValueBytes = Utils.toArray(fieldValue, 'utf8');

    // Rust: Generate random 32-byte field key
    const fieldKey = Array.from(Random(32));
    console.log('Field key (hex):', fieldKey.map(b => b.toString(16).padStart(2, '0')).join(''));

    // Rust: Encrypt field value with field key using full 32-byte key
    const fieldIV = Array.from(Random(32));
    const { result: fieldCT, authenticationTag: fieldTag } = AESGCM(fieldValueBytes, [], fieldIV, fieldKey);
    const encryptedFieldValue = [...fieldIV, ...fieldCT, ...fieldTag];
    const encryptedFieldB64 = Utils.toBase64(encryptedFieldValue);
    console.log('Encrypted field value:', encryptedFieldB64.substring(0, 40) + '...');

    // Rust: Strip field key (leading zeros)
    let strippedFieldKey = [...fieldKey];
    while (strippedFieldKey.length > 1 && strippedFieldKey[0] === 0) strippedFieldKey.shift();
    console.log('Stripped field key (' + strippedFieldKey.length + ' bytes):',
      strippedFieldKey.map(b => b.toString(16).padStart(2, '0')).join(''));

    // Rust: BRC-2 encrypt revelation key for server
    // Invoice: "2-certificate field encryption-cool" (no serial number for master cert)
    const revSymKey = clientDeriver.deriveSymmetricKey(
      [2, 'certificate field encryption'],
      fieldName,
      PrivateKey.fromString(SERVER_PRIV_HEX, 16).toPublicKey()
    );

    const revKey32 = revSymKey.toArray('be', 32);
    console.log('BRC-2 sym key for revelation (hex):', revKey32.map(b => b.toString(16).padStart(2, '0')).join(''));

    // Rust: encrypt_brc2(strippedFieldKey, revKey32)
    const revIV = Array.from(Random(32));
    const { result: revCT, authenticationTag: revTag } = AESGCM(strippedFieldKey, [], revIV, revKey32);
    const encryptedRevKey = [...revIV, ...revCT, ...revTag];
    const encryptedRevKeyB64 = Utils.toBase64(encryptedRevKey);
    console.log('Encrypted revelation key:', encryptedRevKeyB64.substring(0, 40) + '...');

    // Build CSR-like objects
    const fields = { [fieldName]: encryptedFieldB64 };
    const masterKeyring = { [fieldName]: encryptedRevKeyB64 };

    // SERVER SIDE: MasterCertificate.decryptFields
    try {
      const decrypted = await MasterCertificate.decryptFields(
        serverWallet,
        masterKeyring,
        fields,
        clientPubKey
      );
      console.log('SDK decryptFields:', JSON.stringify(decrypted));
      console.log('Result:', decrypted.cool === 'true' ? '✅ PASS' : '❌ FAIL (got: ' + decrypted.cool + ')');
    } catch (e) {
      console.log('SDK decryptFields THREW:', e.message);

      // Debug: try manual decrypt
      console.log('\n  Manual decrypt debug:');
      const serverRevSymKey = serverDeriver.deriveSymmetricKey(
        [2, 'certificate field encryption'],
        fieldName,
        PrivateKey.fromString(CLIENT_PRIV_HEX, 16).toPublicKey()
      );
      const serverRevKey32 = serverRevSymKey.toArray('be', 32);
      const serverRevKeyStripped = serverRevSymKey.toArray();
      console.log('  Server BRC-2 key (32):', serverRevKey32.map(b => b.toString(16).padStart(2, '0')).join(''));
      console.log('  Client BRC-2 key (32):', revKey32.map(b => b.toString(16).padStart(2, '0')).join(''));
      console.log('  Keys match:', serverRevKey32.every((b, i) => b === revKey32[i]) ? '✅' : '❌');

      // Try SymmetricKey.decrypt
      try {
        const revKeyDecrypted = serverRevSymKey.decrypt(encryptedRevKey);
        console.log('  Manual revelation key decrypt: OK (' + revKeyDecrypted.length + ' bytes)');
      } catch (e2) {
        console.log('  Manual revelation key decrypt FAILED:', e2.message);
      }
    }
  }

  // ========== TEST D: Full ProtoWallet.createHmac + verifyHmac flow ==========
  console.log('\n=== TEST D: ProtoWallet createHmac/verifyHmac symmetry ===');
  {
    const testData = Array.from(Random(16));
    const testKeyID = Utils.toUTF8(testData);

    // Client creates HMAC with server as counterparty
    const { hmac } = await clientWallet.createHmac({
      data: testData,
      protocolID: [2, 'server hmac'],
      keyID: testKeyID,
      counterparty: serverPubKey
    });
    console.log('Client HMAC:', hmac.map(b => b.toString(16).padStart(2, '0')).join(''));

    // Server verifies with client as counterparty
    try {
      const { valid } = await serverWallet.verifyHmac({
        data: testData,
        hmac,
        protocolID: [2, 'server hmac'],
        keyID: testKeyID,
        counterparty: clientPubKey
      });
      console.log('Server verifyHmac: valid=' + valid + ' ✅');
    } catch (e) {
      console.log('Server verifyHmac THREW:', e.message, '❌');
    }
  }

  // ========== TEST E: ProtoWallet encrypt/decrypt flow ==========
  console.log('\n=== TEST E: ProtoWallet encrypt/decrypt symmetry ===');
  {
    // Client encrypts
    const plaintext = Array.from(Utils.toArray('hello', 'utf8'));
    const { ciphertext } = await clientWallet.encrypt({
      plaintext,
      protocolID: [2, 'certificate field encryption'],
      keyID: 'cool',
      counterparty: serverPubKey
    });
    console.log('Encrypted:', ciphertext.length, 'bytes');

    // Server decrypts
    try {
      const { plaintext: decrypted } = await serverWallet.decrypt({
        ciphertext,
        protocolID: [2, 'certificate field encryption'],
        keyID: 'cool',
        counterparty: clientPubKey
      });
      console.log('Decrypted:', Utils.toUTF8(decrypted), decrypted.length === plaintext.length ? '✅' : '❌');
    } catch (e) {
      console.log('Decrypt THREW:', e.message, '❌');
    }
  }

  console.log('\n=== DONE ===');
}

main().catch(e => {
  console.error('FATAL:', e.message);
  console.error(e.stack);
  process.exit(1);
});
