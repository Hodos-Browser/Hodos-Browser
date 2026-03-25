#!/usr/bin/env node
// Test our Rust wallet's certificate acquisition against the local test server.
// This does the FULL BRC-103 authenticated flow, just like our Rust code.

import { createRequire } from 'module';
const require = createRequire(import.meta.url);
const sdkPath = '../reference/ts-brc100/node_modules/@bsv/sdk';

const {
  ProtoWallet, PrivateKey, Utils, Random, Hash,
  MasterCertificate, Certificate
} = require(sdkPath);
const { KeyDeriver } = require(sdkPath + '/dist/cjs/src/wallet/KeyDeriver.js');
const { AESGCM } = require(sdkPath + '/dist/cjs/src/primitives/AESGCM.js');

// Client wallet
const CLIENT_PRIV_HEX = 'e8f32e723decf4051aefac8e2c93c9c5b214313817cdb01a1494b917c8436b35';
const clientPrivKey = PrivateKey.fromString(CLIENT_PRIV_HEX, 16);
const clientWallet = new ProtoWallet(clientPrivKey);
const clientDeriver = new KeyDeriver(clientPrivKey);

const SERVER_URL = 'http://127.0.0.1:8099';

async function main() {
  const clientPubKey = (await clientWallet.getPublicKey({ identityKey: true })).publicKey;
  console.log('Client pubkey:', clientPubKey);

  // ========== STEP 1: Use SDK's AuthFetch to do the full BRC-103 flow ==========
  // But first, let's try the SIMPLEST approach: use the SDK's own Peer/AuthFetch
  // to send the CSR, which would tell us if the problem is in our BRC-103 implementation
  // or in the CSR content.

  // Actually, let's do it step by step, mimicking our Rust code but using SDK primitives.

  // STEP 1: Initial handshake
  console.log('\n=== STEP 1: Initial Handshake ===');

  // Create client nonce for initial request (counterparty = 'self')
  const { hmac: initialHmac } = await clientWallet.createHmac({
    data: Array.from(Random(16)),
    protocolID: [2, 'server hmac'],
    keyID: Utils.toUTF8(Array.from(Random(16))),
    counterparty: 'self'
  });
  console.log('Initial handshake skipped — testing CSR directly...');

  // Instead of doing the full BRC-103 handshake, let's test the signCertificate
  // endpoint directly using the SDK's AuthFetch:
  const { AuthFetch } = require(sdkPath + '/dist/cjs/src/auth/clients/AuthFetch.js');

  // Server pubkey (from server output)
  const serverPubKey = '02cab461076409998157f05bb90f07886380186fd3d88b99c549f21de4d2511b83';
  const serverPubKeyObj = PrivateKey.fromString('dc38f15198fc8cd92a920fd07fc715d223dbca120e523e636a7b835aa932ce36', 16).toPublicKey();

  // ========== STEP 2: Create CSR using SDK (reference implementation) ==========
  console.log('\n=== STEP 2: Create CSR (SDK method) ===');

  // Create nonce for CSR (counterparty = server)
  const { createNonce } = require(sdkPath);
  const csrNonce = await createNonce(clientWallet, serverPubKey);
  console.log('CSR nonce (SDK):', csrNonce.substring(0, 30) + '...');

  // Create encrypted fields using SDK
  const { certificateFields, masterKeyring } = await MasterCertificate.createCertificateFields(
    clientWallet,
    serverPubKey,
    { cool: 'true' }
  );
  console.log('Encrypted fields:', Object.keys(certificateFields));
  console.log('Master keyring:', Object.keys(masterKeyring));

  const certType = 'jVNgF8+rifnz00856b4TkThCAvfiUE4p+t/aHYl1u0c=';

  // ========== STEP 3: Send CSR via SDK's AuthFetch ==========
  console.log('\n=== STEP 3: Send via SDK AuthFetch ===');

  const csrBody = {
    clientNonce: csrNonce,
    type: certType,
    fields: certificateFields,
    masterKeyring
  };
  console.log('CSR body keys:', Object.keys(csrBody));
  console.log('CSR body JSON length:', JSON.stringify(csrBody).length);

  try {
    const authFetch = new AuthFetch(clientWallet);
    const response = await authFetch.fetch(`${SERVER_URL}/signCertificate`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(csrBody)
    });
    const responseText = await response.text();
    console.log('Response status:', response.status);
    console.log('Response body:', responseText.substring(0, 200));
    if (response.status === 200) {
      console.log('\n✅ SDK AuthFetch + SDK CSR: SUCCESS');
    } else {
      console.log('\n❌ SDK AuthFetch + SDK CSR: FAILED');
    }
  } catch (e) {
    console.log('AuthFetch error:', e.message);
  }

  // ========== STEP 4: Create CSR mimicking Rust code ==========
  console.log('\n=== STEP 4: Send CSR mimicking Rust code ===');

  // Mimic Rust's create_nonce_with_hmac
  const firstHalf = Array.from(Random(16));
  const keyID = Utils.toUTF8(firstHalf); // js_to_utf8 equivalent
  const symKey = clientDeriver.deriveSymmetricKey(
    [2, 'server hmac'],
    keyID,
    serverPubKeyObj
  );
  const symKeyStripped = symKey.toArray();
  const hmac = Hash.sha256hmac(symKeyStripped, firstHalf);
  const rustNonce = Utils.toBase64([...firstHalf, ...hmac]);
  console.log('Rust-simulated nonce:', rustNonce.substring(0, 30) + '...');

  // Mimic Rust's field encryption
  const fieldKey = Array.from(Random(32));
  const fieldIV = Array.from(Random(32));
  const fieldValueBytes = Utils.toArray('true', 'utf8');
  const { result: fieldCT, authenticationTag: fieldTag } = AESGCM(fieldValueBytes, [], fieldIV, fieldKey);
  const encryptedFieldB64 = Utils.toBase64([...fieldIV, ...fieldCT, ...fieldTag]);

  // Strip field key leading zeros
  let strippedFieldKey = [...fieldKey];
  while (strippedFieldKey.length > 1 && strippedFieldKey[0] === 0) strippedFieldKey.shift();

  // BRC-2 encrypt revelation key for server
  const revSymKey = clientDeriver.deriveSymmetricKey(
    [2, 'certificate field encryption'],
    'cool',
    serverPubKeyObj
  );
  const revKey32 = revSymKey.toArray('be', 32);
  const revIV = Array.from(Random(32));
  const { result: revCT, authenticationTag: revTag } = AESGCM(strippedFieldKey, [], revIV, revKey32);
  const encryptedRevKeyB64 = Utils.toBase64([...revIV, ...revCT, ...revTag]);

  const rustCsrBody = {
    clientNonce: rustNonce,
    type: certType,
    fields: { cool: encryptedFieldB64 },
    masterKeyring: { cool: encryptedRevKeyB64 }
  };

  try {
    const authFetch2 = new AuthFetch(clientWallet);
    const response2 = await authFetch2.fetch(`${SERVER_URL}/signCertificate`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(rustCsrBody)
    });
    const responseText2 = await response2.text();
    console.log('Response status:', response2.status);
    console.log('Response body:', responseText2.substring(0, 200));
    if (response2.status === 200) {
      console.log('\n✅ SDK AuthFetch + Rust-simulated CSR: SUCCESS');
    } else {
      console.log('\n❌ SDK AuthFetch + Rust-simulated CSR: FAILED');
    }
  } catch (e) {
    console.log('AuthFetch error:', e.message);
  }

  console.log('\n=== DONE ===');
}

main().catch(e => {
  console.error('FATAL:', e.message);
  console.error(e.stack);
  process.exit(1);
});
