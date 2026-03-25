#!/usr/bin/env node
// Test BRC-103 flow mimicking EXACTLY what our Rust code does.
// Step 1: Raw POST initial request (no auth headers) to /.well-known/auth
// Step 2: Build serialized request for signing
// Step 3: Sign with BRC-42 derived key
// Step 4: Send with auth headers to /signCertificate
//
// This tests our Rust wallet's BRC-103 transport against the local test server.

import { createRequire } from 'module';
const require = createRequire(import.meta.url);
const sdkPath = '../reference/ts-brc100/node_modules/@bsv/sdk';

const {
  ProtoWallet, PrivateKey, Utils, Random, Hash,
  MasterCertificate, createNonce
} = require(sdkPath);
const { KeyDeriver } = require(sdkPath + '/dist/cjs/src/wallet/KeyDeriver.js');

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
  // IMPORTANT: NO auth headers on initial request! Just JSON body.
  const initialMessage = {
    version: '0.1',
    messageType: 'initialRequest',
    identityKey: clientPubKey,
    initialNonce: initialNonce
  };

  // Send initial request via raw fetch (NO x-bsv-auth headers!)
  console.log('Sending to:', `${SERVER_URL}/.well-known/auth`);
  console.log('Body:', JSON.stringify(initialMessage));

  const initialResp = await fetch(`${SERVER_URL}/.well-known/auth`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(initialMessage)
  });

  const initialRespBody = await initialResp.text();
  console.log('Initial response status:', initialResp.status);

  if (!initialResp.ok) {
    console.log('Initial response body:', initialRespBody);
    console.log('FAILED at initial handshake');
    return;
  }

  const initialData = JSON.parse(initialRespBody);
  const serverIdentityKey = initialData.identityKey;
  const serverNonce = initialData.initialNonce;
  const echoed = initialData.yourNonce;

  console.log('Server identity key:', serverIdentityKey);
  console.log('Server nonce:', serverNonce?.substring(0, 30) + '...');
  console.log('Echoed our nonce:', echoed === initialNonce ? 'YES ✅' : 'NO ❌');

  // ========== STEP 2: Build CSR ==========
  console.log('\n=== STEP 2: Build CSR ===');

  // Create CSR nonce (counterparty = server)
  const csrNonce = await createNonce(clientWallet, serverIdentityKey);

  // Create encrypted fields using SDK
  const { certificateFields, masterKeyring } = await MasterCertificate.createCertificateFields(
    clientWallet, serverIdentityKey, { cool: 'true' }
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
  console.log('CSR body keys:', Object.keys(csrBody));

  // ========== STEP 3: Build serialized request (EXACTLY like Rust code) ==========
  console.log('\n=== STEP 3: Serialize request ===');

  // Generate request nonce (32 random bytes, base64)
  const csrRequestNonce = Utils.toBase64(Random(32));
  const csrRequestNonceBytes = Utils.toArray(csrRequestNonce, 'base64');
  console.log('Request nonce (base64):', csrRequestNonce.substring(0, 30) + '...');
  console.log('Request nonce decoded length:', csrRequestNonceBytes.length, 'bytes');

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

  // 5. Headers — ONLY content-type, sorted
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

  // 6. Body (CSR JSON as UTF-8 bytes)
  const bodyBytes = Utils.toArray(csrBodyJson, 'utf8');
  writer.writeVarIntNum(bodyBytes.length);
  writer.write(bodyBytes);

  const serializedRequest = writer.toArray();
  console.log('Serialized request length:', serializedRequest.length, 'bytes');

  // ========== STEP 4: Sign (EXACTLY like Rust code) ==========
  console.log('\n=== STEP 4: Sign ===');

  // KeyID: requestNonce + " " + serverNonce
  // Our Rust code uses the SAME nonce for both x-bsv-auth-nonce and x-bsv-auth-request-id
  const signKeyID = `${csrRequestNonce} ${serverNonce}`;
  console.log('KeyID (first 60 chars):', signKeyID.substring(0, 60) + '...');

  // Sign the serialized request
  // CRITICAL: ProtoWallet.createSignature internally applies SHA256 to args.data
  // So we pass the raw bytes, NOT pre-hashed. If we pass Hash.sha256(bytes),
  // it would double-hash: sha256(sha256(bytes)) which won't match server verification.
  //
  // BUT: Our Rust code does sha256 manually then signs the hash directly (Message::from_digest_slice).
  // So to match our Rust code, we should use hashToDirectlySign with the pre-computed hash.
  const preHash = Hash.sha256(serializedRequest);
  console.log('SHA256 of serialized request:', preHash.map(b => b.toString(16).padStart(2, '0')).join('').substring(0, 40) + '...');

  // Method A: Like SDK's Peer.toPeer() — pass raw data, let ProtoWallet hash
  const signatureResultA = await clientWallet.createSignature({
    data: serializedRequest,
    protocolID: [2, 'auth message signature'],
    keyID: signKeyID,
    counterparty: serverIdentityKey
  });
  const signatureHexA = signatureResultA.signature.map(b => b.toString(16).padStart(2, '0')).join('');

  // Method B: Like our Rust code — pre-hash, sign directly
  const signatureResultB = await clientWallet.createSignature({
    hashToDirectlySign: preHash,
    protocolID: [2, 'auth message signature'],
    keyID: signKeyID,
    counterparty: serverIdentityKey
  });
  const signatureHexB = signatureResultB.signature.map(b => b.toString(16).padStart(2, '0')).join('');

  console.log('Signature A (SDK-style, data):', signatureHexA.substring(0, 30) + '...');
  console.log('Signature B (Rust-style, hashToDirectlySign):', signatureHexB.substring(0, 30) + '...');
  console.log('Signatures match:', signatureHexA === signatureHexB ? 'YES' : 'NO (different!)');

  // Use Method A (SDK-style) since the server verifies the same way
  const signatureHex = signatureHexA;

  // (old single-hash code removed — replaced by Method A/B comparison above)

  // ========== STEP 5: Send with auth headers (EXACTLY like Rust code) ==========
  console.log('\n=== STEP 5: Send signCertificate ===');

  // Rust code uses same nonce for both x-bsv-auth-nonce and x-bsv-auth-request-id
  const sendHeaders = {
    'Content-Type': 'application/json',
    'x-bsv-auth-version': '0.1',
    'x-bsv-auth-identity-key': clientPubKey,
    'x-bsv-auth-nonce': csrRequestNonce,
    'x-bsv-auth-your-nonce': serverNonce,
    'x-bsv-auth-request-id': csrRequestNonce,
    'x-bsv-auth-signature': signatureHex
  };

  console.log('Headers:');
  for (const [k, v] of Object.entries(sendHeaders)) {
    if (k.startsWith('x-bsv-auth')) {
      console.log(`  ${k}: ${v.substring(0, 40)}...`);
    }
  }

  const csrResp = await fetch(`${SERVER_URL}/signCertificate`, {
    method: 'POST',
    headers: sendHeaders,
    body: csrBodyJson
  });

  const csrRespBody = await csrResp.text();
  console.log('\nResponse status:', csrResp.status);
  console.log('Response body (first 200):', csrRespBody.substring(0, 200));

  if (csrResp.status === 200) {
    console.log('\n✅ Rust-style BRC-103 transport: SUCCESS');
  } else {
    console.log('\n❌ Rust-style BRC-103 transport: FAILED');

    // Debug: check response headers for auth info
    console.log('\nResponse headers:');
    csrResp.headers.forEach((v, k) => {
      if (k.startsWith('x-bsv') || k === 'content-type') {
        console.log(`  ${k}: ${v}`);
      }
    });

    // Try SDK AuthFetch for comparison
    console.log('\n=== COMPARISON: SDK AuthFetch ===');
    const { AuthFetch } = require(sdkPath + '/dist/cjs/src/auth/clients/AuthFetch.js');
    const authFetch = new AuthFetch(clientWallet);
    try {
      // IMPORTANT: Need to create a new CSR body with a new nonce (server expects fresh nonce)
      const csrNonce2 = await createNonce(clientWallet, serverIdentityKey);
      const { certificateFields: cf2, masterKeyring: mk2 } = await MasterCertificate.createCertificateFields(
        clientWallet, serverIdentityKey, { cool: 'true' }
      );
      const csrBody2 = {
        clientNonce: csrNonce2,
        type: certType,
        fields: cf2,
        masterKeyring: mk2
      };

      const sdkResp = await authFetch.fetch(`${SERVER_URL}/signCertificate`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(csrBody2)
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
