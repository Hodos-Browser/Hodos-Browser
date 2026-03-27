#!/usr/bin/env node
// Minimal mock of the SocialCert server to identify exactly where our CSR fails.
// This uses the SDK's crypto primitives but without the full wallet-toolbox setup.
//
// Usage:
//   1. Start this server: node test_server_mock.mjs
//   2. Point our Rust wallet's acquire_certificate to http://localhost:8099
//   3. Watch the console output for exactly which step fails
//
// The server uses a known test private key and does NOT require wallet-toolbox.

import { createRequire } from 'module';
const require = createRequire(import.meta.url);
const sdkPath = '../reference/ts-brc100/node_modules/@bsv/sdk';
const { PrivateKey, Utils, Hash, SymmetricKey, Random, PublicKey } = require(sdkPath);
const { KeyDeriver } = require(sdkPath + '/dist/cjs/src/wallet/KeyDeriver.js');
const { AESGCM, AESGCMDecrypt } = require(sdkPath + '/dist/cjs/src/primitives/AESGCM.js');

import http from 'http';

// Server's private key - use the CoolCert example key
const SERVER_PRIV_HEX = 'dc38f15198fc8cd92a920fd07fc715d223dbca120e523e636a7b835aa932ce36';
const serverPrivKey = PrivateKey.fromString(SERVER_PRIV_HEX, 16);
const serverPubKey = serverPrivKey.toPublicKey();
const serverDeriver = new KeyDeriver(serverPrivKey);

console.log(`Mock certifier server`);
console.log(`Server public key: ${serverPubKey.toString()}`);
console.log(`Listening on http://localhost:8099`);

// Implement verifyNonce manually (matching SDK's verifyNonce)
function verifyNonce(nonceBase64, clientPubKeyHex) {
  console.log(`\n--- verifyNonce ---`);
  console.log(`  Nonce (base64): ${nonceBase64}`);
  console.log(`  Client identity key: ${clientPubKeyHex}`);

  const buffer = Utils.toArray(nonceBase64, 'base64');
  console.log(`  Decoded nonce length: ${buffer.length} bytes`);

  if (buffer.length !== 48) {
    console.log(`  ERROR: Expected 48 bytes, got ${buffer.length}`);
    return false;
  }

  const data = buffer.slice(0, 16);
  const hmac = buffer.slice(16);
  console.log(`  Data (first 16 bytes, hex): ${data.map(b => b.toString(16).padStart(2, '0')).join('')}`);
  console.log(`  HMAC (32 bytes, hex): ${hmac.map(b => b.toString(16).padStart(2, '0')).join('')}`);

  // keyID = Utils.toUTF8(data)
  const keyID = Utils.toUTF8(data);
  console.log(`  keyID from Utils.toUTF8: ${JSON.stringify(keyID)} (${keyID.length} chars)`);
  console.log(`  keyID codePoints: [${[...keyID].map(c => 'U+' + c.codePointAt(0).toString(16).toUpperCase().padStart(4, '0')).join(', ')}]`);

  // Re-encode keyID to UTF-8 bytes to see what goes into the invoice number
  const keyIDBytes = Utils.toArray(keyID, 'utf8');
  console.log(`  keyID re-encoded UTF-8 bytes (hex): ${keyIDBytes.map(b => b.toString(16).padStart(2, '0')).join('')}`);
  console.log(`  keyID re-encoded UTF-8 length: ${keyIDBytes.length} bytes`);

  // Check if data roundtrips through toUTF8 -> toArray('utf8')
  const roundtrip = keyIDBytes.every((b, i) => i < data.length && b === data[i]) && keyIDBytes.length === data.length;
  console.log(`  Data roundtrip (toUTF8 -> toArray utf8 == original): ${roundtrip ? 'YES' : 'NO ⚠️'}`);
  if (!roundtrip) {
    console.log(`    Original data (hex): ${data.map(b => b.toString(16).padStart(2, '0')).join('')}`);
    console.log(`    Roundtripped (hex):  ${keyIDBytes.map(b => b.toString(16).padStart(2, '0')).join('')}`);
  }

  // Derive symmetric key using BRC-42 (server with client as counterparty)
  const clientPubKey = PublicKey.fromString(clientPubKeyHex);
  const symKey = serverDeriver.deriveSymmetricKey(
    [2, 'server hmac'],
    keyID,
    clientPubKey
  );

  const symKey32 = symKey.toArray('be', 32);
  const symKeyStripped = symKey.toArray();
  console.log(`  Symmetric key (32-byte, hex): ${symKey32.map(b => b.toString(16).padStart(2, '0')).join('')}`);
  console.log(`  Symmetric key (stripped, ${symKeyStripped.length} bytes, hex): ${symKeyStripped.map(b => b.toString(16).padStart(2, '0')).join('')}`);

  // Compute expected HMAC with stripped key
  const expectedHmac = Hash.sha256hmac(symKeyStripped, data);
  console.log(`  Expected HMAC (hex): ${expectedHmac.map(b => b.toString(16).padStart(2, '0')).join('')}`);
  console.log(`  Provided HMAC (hex): ${hmac.map(b => b.toString(16).padStart(2, '0')).join('')}`);

  const match = expectedHmac.every((b, i) => b === hmac[i]) && expectedHmac.length === hmac.length;
  console.log(`  HMAC match: ${match ? '✅ PASS' : '❌ FAIL'}`);

  if (!match) {
    // Try with full 32-byte key too
    const expectedHmac32 = Hash.sha256hmac(symKey32, data);
    const match32 = expectedHmac32.every((b, i) => b === hmac[i]) && expectedHmac32.length === hmac.length;
    console.log(`  HMAC match (full 32-byte key): ${match32 ? '✅ would match' : '❌ also fails'}`);
  }

  return match;
}

// Implement decryptFields manually (matching SDK's MasterCertificate.decryptFields)
function decryptFields(masterKeyring, fields, clientPubKeyHex) {
  console.log(`\n--- decryptFields ---`);
  console.log(`  Client identity key: ${clientPubKeyHex}`);
  console.log(`  Fields: ${JSON.stringify(Object.keys(fields))}`);
  console.log(`  MasterKeyring keys: ${JSON.stringify(Object.keys(masterKeyring))}`);

  const clientPubKey = PublicKey.fromString(clientPubKeyHex);
  const decrypted = {};

  for (const fieldName of Object.keys(fields)) {
    console.log(`\n  Field '${fieldName}':`);

    // Step 1: Decrypt revelation key from masterKeyring
    const encryptedRevKey = Utils.toArray(masterKeyring[fieldName], 'base64');
    console.log(`    Encrypted revelation key: ${encryptedRevKey.length} bytes`);

    // getCertificateFieldEncryptionDetails: protocolID=[2,'certificate field encryption'], keyID=fieldName
    const symKey = serverDeriver.deriveSymmetricKey(
      [2, 'certificate field encryption'],
      fieldName,
      clientPubKey
    );

    const symKey32 = symKey.toArray('be', 32);
    const symKeyStripped = symKey.toArray();
    console.log(`    BRC-2 symmetric key (32-byte, hex): ${symKey32.map(b => b.toString(16).padStart(2, '0')).join('')}`);
    console.log(`    BRC-2 symmetric key (stripped, ${symKeyStripped.length} bytes, hex): ${symKeyStripped.map(b => b.toString(16).padStart(2, '0')).join('')}`);

    // Decrypt using SymmetricKey.decrypt (which uses stripped key internally)
    try {
      const revelationKey = symKey.decrypt(encryptedRevKey);
      console.log(`    Decrypted revelation key: [${revelationKey.map(b => b.toString(16).padStart(2, '0')).join(' ')}] (${revelationKey.length} bytes)`);

      // Step 2: Decrypt field value using revelation key
      const encryptedFieldValue = Utils.toArray(fields[fieldName], 'base64');
      console.log(`    Encrypted field value: ${encryptedFieldValue.length} bytes`);

      const fieldSymKey = new SymmetricKey(revelationKey);
      console.log(`    Field SymmetricKey (stripped, ${fieldSymKey.toArray().length} bytes): ${fieldSymKey.toArray().map(b => b.toString(16).padStart(2, '0')).join('')}`);

      try {
        const decryptedBytes = fieldSymKey.decrypt(encryptedFieldValue);
        const decryptedStr = Utils.toUTF8(decryptedBytes);
        console.log(`    Decrypted field value: ${decryptedStr} ✅`);
        decrypted[fieldName] = decryptedStr;
      } catch (e2) {
        console.log(`    ❌ Field value decryption FAILED: ${e2.message}`);
        // Try manual decrypt with full 32-byte padded key
        try {
          const iv = encryptedFieldValue.slice(0, 32);
          const ct = encryptedFieldValue.slice(32, -16);
          const tag = encryptedFieldValue.slice(-16);
          const stripped = fieldSymKey.toArray();
          console.log(`    Manual decrypt attempt:`);
          console.log(`      IV: ${iv.map(b=>b.toString(16).padStart(2,'0')).join('')}`);
          console.log(`      Ciphertext: ${ct.length} bytes`);
          console.log(`      Tag: ${tag.map(b=>b.toString(16).padStart(2,'0')).join('')}`);
          console.log(`      Key (stripped): ${stripped.map(b=>b.toString(16).padStart(2,'0')).join('')}`);

          const dec = AESGCMDecrypt(ct, [], iv, tag, stripped);
          if (dec) {
            console.log(`    Manual decrypt succeeded: ${Utils.toUTF8(dec)}`);
          } else {
            console.log(`    Manual decrypt also FAILED (null result)`);
          }
        } catch (e3) {
          console.log(`    Manual decrypt exception: ${e3.message}`);
        }
        return null;
      }
    } catch (e) {
      console.log(`    ❌ Revelation key decryption FAILED: ${e.message}`);

      // Try manual decryption to debug
      if (encryptedRevKey.length >= 48) {
        const iv = encryptedRevKey.slice(0, 32);
        const ct = encryptedRevKey.slice(32, -16);
        const tag = encryptedRevKey.slice(-16);
        console.log(`    Manual decrypt attempt:`);
        console.log(`      IV (hex): ${iv.map(b=>b.toString(16).padStart(2,'0')).join('')}`);
        console.log(`      Ciphertext: ${ct.length} bytes`);
        console.log(`      Tag (hex): ${tag.map(b=>b.toString(16).padStart(2,'0')).join('')}`);
        console.log(`      Key stripped (hex): ${symKeyStripped.map(b=>b.toString(16).padStart(2,'0')).join('')}`);
        console.log(`      Key full (hex): ${symKey32.map(b=>b.toString(16).padStart(2,'0')).join('')}`);

        // Try with full 32-byte key
        const dec32 = AESGCMDecrypt(ct, [], iv, tag, symKey32);
        console.log(`      Decrypt with full 32-byte key: ${dec32 === null ? 'FAILED' : 'OK (' + dec32.length + ' bytes)'}`);

        // Try with stripped key
        const decStripped = AESGCMDecrypt(ct, [], iv, tag, symKeyStripped);
        console.log(`      Decrypt with stripped key: ${decStripped === null ? 'FAILED' : 'OK (' + decStripped.length + ' bytes)'}`);
      }
      return null;
    }
  }

  return decrypted;
}

const server = http.createServer((req, res) => {
  // Handle CORS
  res.setHeader('Access-Control-Allow-Origin', '*');
  res.setHeader('Access-Control-Allow-Headers', '*');
  res.setHeader('Access-Control-Allow-Methods', '*');
  res.setHeader('Access-Control-Expose-Headers', '*');

  if (req.method === 'OPTIONS') {
    res.writeHead(200);
    res.end();
    return;
  }

  // Collect request body
  let body = '';
  req.on('data', chunk => body += chunk);
  req.on('end', () => {
    console.log(`\n${'='.repeat(80)}`);
    console.log(`${req.method} ${req.url}`);
    console.log(`Headers:`);
    for (const [key, value] of Object.entries(req.headers)) {
      console.log(`  ${key}: ${value}`);
    }

    // Handle initialRequest (returns server's identity key and nonce)
    if (req.url === '/initialRequest' || req.url?.includes('initialRequest')) {
      console.log(`\n--- initialRequest: returning server identity ---`);
      const response = {
        identityKey: serverPubKey.toString(),
      };
      console.log(`Response: ${JSON.stringify(response)}`);
      res.writeHead(200, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify(response));
      return;
    }

    // For signCertificate, skip BRC-103 auth (we just want to test the CSR itself)
    if (req.url === '/signCertificate' || req.url?.includes('signCertificate')) {
      console.log(`\n--- signCertificate ---`);
      console.log(`Body length: ${body.length}`);

      let parsed;
      try {
        parsed = JSON.parse(body);
      } catch (e) {
        console.log(`❌ Failed to parse JSON body: ${e.message}`);
        console.log(`Raw body (first 500): ${body.substring(0, 500)}`);
        res.writeHead(400, { 'Content-Type': 'application/json' });
        res.end(JSON.stringify({ status: 'error', description: 'Invalid JSON' }));
        return;
      }

      console.log(`Parsed CSR fields: ${JSON.stringify(Object.keys(parsed))}`);

      const { clientNonce, type, fields, masterKeyring } = parsed;

      // Get client identity from header (BRC-103 sets this)
      const clientIdentityKey = req.headers['x-bsv-auth-identity-key'];
      console.log(`Client identity key (from header): ${clientIdentityKey}`);

      if (!clientIdentityKey) {
        console.log(`❌ No x-bsv-auth-identity-key header!`);
        console.log(`Available headers: ${Object.keys(req.headers).join(', ')}`);
        res.writeHead(401, { 'Content-Type': 'application/json' });
        res.end(JSON.stringify({ status: 'error', description: 'No identity key header' }));
        return;
      }

      // Validate args
      if (!clientNonce) {
        console.log(`❌ Missing clientNonce`);
        res.writeHead(400);
        res.end(JSON.stringify({ status: 'error', description: 'Missing clientNonce' }));
        return;
      }
      if (!type) {
        console.log(`❌ Missing type`);
        res.writeHead(400);
        res.end(JSON.stringify({ status: 'error', description: 'Missing type' }));
        return;
      }
      if (!fields) {
        console.log(`❌ Missing fields`);
        res.writeHead(400);
        res.end(JSON.stringify({ status: 'error', description: 'Missing fields' }));
        return;
      }
      if (!masterKeyring) {
        console.log(`❌ Missing masterKeyring`);
        res.writeHead(400);
        res.end(JSON.stringify({ status: 'error', description: 'Missing masterKeyring' }));
        return;
      }

      console.log(`\nCSR contents:`);
      console.log(`  clientNonce: ${clientNonce} (${clientNonce.length} chars)`);
      console.log(`  type: ${type}`);
      console.log(`  fields: ${JSON.stringify(fields)}`);
      console.log(`  masterKeyring: ${JSON.stringify(masterKeyring)}`);

      // Step 1: Verify nonce
      console.log(`\n${'='.repeat(40)}`);
      console.log(`STEP 1: Verify client nonce`);
      console.log(`${'='.repeat(40)}`);
      const nonceValid = verifyNonce(clientNonce, clientIdentityKey);

      if (!nonceValid) {
        console.log(`\n❌ NONCE VERIFICATION FAILED — this is the error!`);
        res.writeHead(500, { 'Content-Type': 'application/json' });
        res.end(JSON.stringify({ status: 'error', code: 'ERR_INTERNAL', description: 'Nonce verification failed' }));
        return;
      }

      // Step 2: Decrypt fields
      console.log(`\n${'='.repeat(40)}`);
      console.log(`STEP 2: Decrypt fields`);
      console.log(`${'='.repeat(40)}`);
      const decryptedFields = decryptFields(masterKeyring, fields, clientIdentityKey);

      if (!decryptedFields) {
        console.log(`\n❌ FIELD DECRYPTION FAILED — this is the error!`);
        res.writeHead(500, { 'Content-Type': 'application/json' });
        res.end(JSON.stringify({ status: 'error', code: 'ERR_INTERNAL', description: 'Field decryption failed' }));
        return;
      }

      console.log(`\n✅ ALL STEPS PASSED!`);
      console.log(`Decrypted fields: ${JSON.stringify(decryptedFields)}`);

      // Return success (mock signed certificate)
      res.writeHead(200, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify({
        certificate: {
          type,
          subject: clientIdentityKey,
          serialNumber: 'mock-serial',
          fields,
          certifier: serverPubKey.toString(),
          signature: 'mock-signature'
        },
        serverNonce: 'mock-server-nonce'
      }));
      return;
    }

    // Default: echo request
    console.log(`Unknown endpoint: ${req.url}`);
    console.log(`Body: ${body.substring(0, 200)}`);
    res.writeHead(404);
    res.end(JSON.stringify({ error: 'Not found' }));
  });
});

server.listen(8099);
