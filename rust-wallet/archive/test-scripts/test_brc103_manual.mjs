#!/usr/bin/env node
// Test BRC-103 flow manually (mimicking Rust code) against local test server.
// This tests the BRC-103 transport layer separately from the crypto.

import { createRequire } from 'module';
const require = createRequire(import.meta.url);
const sdkPath = '../reference/ts-brc100/node_modules/@bsv/sdk';

const {
  ProtoWallet, PrivateKey, Utils, Random, Hash,
  MasterCertificate, Certificate, createNonce
} = require(sdkPath);
const { KeyDeriver } = require(sdkPath + '/dist/cjs/src/wallet/KeyDeriver.js');
const { AESGCM } = require(sdkPath + '/dist/cjs/src/primitives/AESGCM.js');

const CLIENT_PRIV_HEX = 'e8f32e723decf4051aefac8e2c93c9c5b214313817cdb01a1494b917c8436b35';
const clientPrivKey = PrivateKey.fromString(CLIENT_PRIV_HEX, 16);
const clientWallet = new ProtoWallet(clientPrivKey);
const clientDeriver = new KeyDeriver(clientPrivKey);

const SERVER_URL = 'http://127.0.0.1:8099';

async function main() {
  const clientPubKey = (await clientWallet.getPublicKey({ identityKey: true })).publicKey;
  console.log('Client pubkey:', clientPubKey);

  // ========== STEP 1: Initial handshake (mimicking Rust's initial_request) ==========
  console.log('\n=== STEP 1: Initial Handshake ===');

  // Create client nonce for initial request (counterparty = 'self')
  const initialNonce = await createNonce(clientWallet, 'self');
  console.log('Initial nonce:', initialNonce.substring(0, 30) + '...');

  // Build initial AuthMessage (what Rust sends to /.well-known/auth)
  const initialMessage = {
    version: '0.1',
    messageType: 'initialRequest',
    identityKey: clientPubKey,
    initialNonce: initialNonce
  };

  // Create request nonce (requestId) for initial request
  const initialRequestNonce = Utils.toBase64(Random(32));

  // Send initial request via raw fetch
  const initialResp = await fetch(`${SERVER_URL}/.well-known/auth`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      'x-bsv-auth-version': '0.1',
      'x-bsv-auth-identity-key': clientPubKey,
      'x-bsv-auth-nonce': initialRequestNonce,
      'x-bsv-auth-request-id': initialRequestNonce,
    },
    body: JSON.stringify(initialMessage)
  });

  const initialRespHeaders = {};
  initialResp.headers.forEach((v, k) => { initialRespHeaders[k] = v; });
  const initialRespBody = await initialResp.text();
  console.log('Initial response status:', initialResp.status);

  // Parse initial response (it's in the auth headers, not the body)
  const serverIdentityKey = initialRespHeaders['x-bsv-auth-identity-key'];
  const serverNonce = initialRespHeaders['x-bsv-auth-nonce'];
  const serverYourNonce = initialRespHeaders['x-bsv-auth-your-nonce'];
  console.log('Server identity key:', serverIdentityKey);
  console.log('Server nonce:', serverNonce?.substring(0, 30) + '...');
  console.log('Server yourNonce:', serverYourNonce?.substring(0, 30) + '...');
  console.log('Our initialNonce:', initialNonce?.substring(0, 30) + '...');

  if (!serverIdentityKey || !serverNonce) {
    // Try parsing body as JSON (different middleware versions return differently)
    console.log('Initial response body:', initialRespBody.substring(0, 200));
    const parsed = JSON.parse(initialRespBody);
    console.log('Parsed:', Object.keys(parsed));
    return;
  }

  // ========== STEP 2: Create CSR ==========
  console.log('\n=== STEP 2: Create CSR ===');

  // Create CSR nonce (counterparty = server)
  const csrNonce = await createNonce(clientWallet, serverIdentityKey);
  console.log('CSR nonce:', csrNonce.substring(0, 30) + '...');

  // Create encrypted fields using SDK
  const { certificateFields, masterKeyring } = await MasterCertificate.createCertificateFields(
    clientWallet,
    serverIdentityKey,
    { cool: 'true' }
  );

  const certType = 'jVNgF8+rifnz00856b4TkThCAvfiUE4p+t/aHYl1u0c=';
  const csrBody = {
    clientNonce: csrNonce,
    type: certType,
    fields: certificateFields,
    masterKeyring
  };
  const csrBodyJson = JSON.stringify(csrBody);
  console.log('CSR body length:', csrBodyJson.length);

  // ========== STEP 3: Build serialized request (mimicking Rust code) ==========
  console.log('\n=== STEP 3: Build serialized request ===');

  // Request nonce for signCertificate
  const csrRequestNonce = Utils.toBase64(Random(32));
  const csrRequestNonceBytes = Utils.toArray(csrRequestNonce, 'base64');

  const writer = new Utils.Writer();

  // 1. Write 32-byte request nonce
  writer.write(csrRequestNonceBytes);

  // 2. Method
  const method = Utils.toArray('POST');
  writer.writeVarIntNum(method.length);
  writer.write(method);

  // 3. Pathname
  const pathname = Utils.toArray('/signCertificate');
  writer.writeVarIntNum(pathname.length);
  writer.write(pathname);

  // 4. Search (-1)
  writer.writeVarIntNum(-1);

  // 5. Headers
  const headers = [['content-type', 'application/json']];
  headers.sort((a, b) => a[0].localeCompare(b[0]));

  writer.writeVarIntNum(headers.length);
  for (const [k, v] of headers) {
    const keyBytes = Utils.toArray(k, 'utf8');
    writer.writeVarIntNum(keyBytes.length);
    writer.write(keyBytes);

    const valueBytes = Utils.toArray(v, 'utf8');
    writer.writeVarIntNum(valueBytes.length);
    writer.write(valueBytes);
  }

  // 6. Body
  const bodyBytes = Utils.toArray(csrBodyJson, 'utf8');
  writer.writeVarIntNum(bodyBytes.length);
  writer.write(bodyBytes);

  const serializedRequest = writer.toArray();
  console.log('Serialized request length:', serializedRequest.length);

  // ========== STEP 4: Sign serialized request ==========
  console.log('\n=== STEP 4: Sign ===');

  // KeyID: requestNonce + " " + serverNonce
  // CRITICAL: This is the server's session nonce (from their initial response),
  // not the server's general nonce from the signCertificate response
  const signKeyID = `${csrRequestNonce} ${serverNonce}`;

  // Derive child private key via BRC-42
  // protocolID: [2, 'auth message signature'], counterparty: server
  const signatureResult = await clientWallet.createSignature({
    data: Hash.sha256(serializedRequest),
    protocolID: [2, 'auth message signature'],
    keyID: signKeyID,
    counterparty: serverIdentityKey
  });

  const signatureHex = signatureResult.signature.map(b => b.toString(16).padStart(2, '0')).join('');
  console.log('Signature:', signatureHex.substring(0, 30) + '...');

  // ========== STEP 5: Send signCertificate request ==========
  console.log('\n=== STEP 5: Send signCertificate ===');

  const csrResp = await fetch(`${SERVER_URL}/signCertificate`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      'x-bsv-auth-version': '0.1',
      'x-bsv-auth-identity-key': clientPubKey,
      'x-bsv-auth-nonce': csrRequestNonce,
      'x-bsv-auth-your-nonce': serverNonce,
      'x-bsv-auth-request-id': csrRequestNonce,
      'x-bsv-auth-signature': signatureHex
    },
    body: csrBodyJson
  });

  const csrRespHeaders = {};
  csrResp.headers.forEach((v, k) => { csrRespHeaders[k] = v; });
  const csrRespBody = await csrResp.text();
  console.log('Response status:', csrResp.status);
  console.log('Response body:', csrRespBody.substring(0, 200));

  if (csrResp.status === 200) {
    console.log('\n✅ Manual BRC-103 flow: SUCCESS');
  } else {
    console.log('\n❌ Manual BRC-103 flow: FAILED');

    // Now try with SDK AuthFetch for comparison
    console.log('\n=== COMPARISON: SDK AuthFetch ===');
    const { AuthFetch } = require(sdkPath + '/dist/cjs/src/auth/clients/AuthFetch.js');
    const authFetch = new AuthFetch(clientWallet);
    try {
      const sdkResp = await authFetch.fetch(`${SERVER_URL}/signCertificate`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: csrBodyJson
      });
      const sdkRespBody = await sdkResp.text();
      console.log('SDK AuthFetch status:', sdkResp.status);
      console.log('SDK AuthFetch body:', sdkRespBody.substring(0, 200));
    } catch (e) {
      console.log('SDK AuthFetch error:', e.message);
    }
  }

  console.log('\n=== DONE ===');
}

main().catch(e => {
  console.error('FATAL:', e.message);
  console.error(e.stack);
  process.exit(1);
});
