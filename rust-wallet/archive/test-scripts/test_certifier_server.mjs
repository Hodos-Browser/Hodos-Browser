#!/usr/bin/env node
// Minimal certifier server using REAL SDK auth middleware + ProtoWallet.
// No wallet-toolbox needed — just @bsv/sdk and @bsv/auth-express-middleware.
//
// Usage:
//   1. Start: node test_certifier_server.mjs
//   2. Point Rust wallet acquire_certificate to http://localhost:8099
//      (certifier URL in the request body)
//   3. Watch console for detailed step-by-step output

import { createRequire } from 'module';
const require = createRequire(import.meta.url);

const sdkPath = '../reference/ts-brc100/node_modules/@bsv/sdk';
const { ProtoWallet, PrivateKey, Utils, MasterCertificate, Certificate, verifyNonce, createNonce } = require(sdkPath);
const { createAuthMiddleware } = require('../reference/ts-brc100/node_modules/@bsv/auth-express-middleware/dist/cjs/mod.js');
const express = require('../reference/ts-brc100/node_modules/express');

// Server key — use CoolCert example key
const SERVER_PRIV_HEX = 'dc38f15198fc8cd92a920fd07fc715d223dbca120e523e636a7b835aa932ce36';
const serverPrivKey = PrivateKey.fromString(SERVER_PRIV_HEX, 16);
const serverWallet = new ProtoWallet(serverPrivKey);

async function main() {
  const serverPubKey = (await serverWallet.getPublicKey({ identityKey: true })).publicKey;
  console.log(`Server public key: ${serverPubKey}`);

  const app = express();
  app.use(express.json({ limit: '30mb' }));

  // CORS
  app.use((req, res, next) => {
    res.header('Access-Control-Allow-Origin', '*');
    res.header('Access-Control-Allow-Headers', '*');
    res.header('Access-Control-Allow-Methods', '*');
    res.header('Access-Control-Expose-Headers', '*');
    res.header('Access-Control-Allow-Private-Network', 'true');
    if (req.method === 'OPTIONS') {
      res.sendStatus(200);
    } else {
      next();
    }
  });

  // BRC-103 auth middleware (REAL SDK implementation)
  app.use(createAuthMiddleware({
    wallet: serverWallet,
    logger: console,
    logLevel: 'debug'
  }));

  // signCertificate route
  app.post('/signCertificate', async (req, res) => {
    try {
      const { clientNonce, type, fields, masterKeyring } = req.body;
      const clientIdentityKey = req.auth?.identityKey;

      console.log('\n========== signCertificate DEBUG ==========');
      console.log('req.auth:', JSON.stringify(req.auth));
      console.log('clientIdentityKey:', clientIdentityKey);
      console.log('clientNonce:', clientNonce?.substring(0, 30) + '...');
      console.log('type:', type);
      console.log('fields keys:', fields ? Object.keys(fields) : 'MISSING');
      console.log('masterKeyring keys:', masterKeyring ? Object.keys(masterKeyring) : 'MISSING');

      // Validate params
      if (!clientNonce) throw new Error('Missing clientNonce');
      if (!type) throw new Error('Missing type');
      if (!fields) throw new Error('Missing fields');
      if (!masterKeyring) throw new Error('Missing masterKeyring');
      console.log('STEP 1 [validate params]: PASS');

      // Verify nonce
      try {
        const nonceValid = await verifyNonce(clientNonce, serverWallet, clientIdentityKey);
        console.log('STEP 2 [verifyNonce]: PASS (valid=' + nonceValid + ')');
      } catch (nonceErr) {
        console.log('STEP 2 [verifyNonce]: FAIL -', nonceErr.message, nonceErr.code || '');
        // Detailed debug
        const buffer = Utils.toArray(clientNonce, 'base64');
        console.log('  nonce total bytes:', buffer.length);
        console.log('  data (first 16, hex):', buffer.slice(0, 16).map(b => b.toString(16).padStart(2, '0')).join(''));
        console.log('  hmac (rest, hex):', buffer.slice(16).map(b => b.toString(16).padStart(2, '0')).join(''));
        const keyID = Utils.toUTF8(buffer.slice(0, 16));
        console.log('  keyID:', JSON.stringify(keyID), `(${keyID.length} chars)`);
        console.log('  keyID codepoints:', [...keyID].map(c => 'U+' + c.codePointAt(0).toString(16).toUpperCase().padStart(4, '0')).join(', '));
        // Re-encode to see byte representation
        const keyIDBytes = Utils.toArray(keyID, 'utf8');
        console.log('  keyID re-encoded bytes:', keyIDBytes.map(b => b.toString(16).padStart(2, '0')).join(''));
        console.log('  counterparty:', clientIdentityKey);
        throw nonceErr;
      }

      // Create server nonce
      let serverNonce;
      try {
        serverNonce = await createNonce(serverWallet, clientIdentityKey);
        console.log('STEP 3 [createNonce]: PASS');
      } catch (err) {
        console.log('STEP 3 [createNonce]: FAIL -', err.message);
        throw err;
      }

      // Serial number
      let serialNumber;
      try {
        const { hmac } = await serverWallet.createHmac({
          data: Utils.toArray(clientNonce + serverNonce, 'base64'),
          protocolID: [2, 'certificate issuance'],
          keyID: serverNonce + clientNonce,
          counterparty: clientIdentityKey
        });
        serialNumber = Utils.toBase64(hmac);
        console.log('STEP 4 [serialNumber]: PASS');
      } catch (err) {
        console.log('STEP 4 [serialNumber]: FAIL -', err.message);
        throw err;
      }

      // Decrypt fields
      let decryptedFields;
      try {
        decryptedFields = await MasterCertificate.decryptFields(
          serverWallet,
          masterKeyring,
          fields,
          clientIdentityKey
        );
        console.log('STEP 5 [decryptFields]: PASS');
        console.log('  decrypted:', JSON.stringify(decryptedFields));
      } catch (err) {
        console.log('STEP 5 [decryptFields]: FAIL -', err.message);
        throw err;
      }

      // Check cool field
      if (!decryptedFields.cool || decryptedFields.cool !== 'true') {
        console.log('STEP 6 [field check]: FAIL - cool=' + JSON.stringify(decryptedFields.cool));
        return res.status(400).json({ status: 'error', description: 'Not cool enough!' });
      }
      console.log('STEP 6 [field check]: PASS');

      // Sign certificate
      const signedCert = new Certificate(
        type,
        serialNumber,
        clientIdentityKey,
        serverPubKey,
        'not supported.0',
        fields
      );
      await signedCert.sign(serverWallet);
      console.log('STEP 7 [sign]: PASS');
      console.log('========== signCertificate SUCCESS ==========\n');

      return res.status(200).json({
        certificate: signedCert,
        serverNonce
      });
    } catch (e) {
      console.error('========== signCertificate EXCEPTION ==========');
      console.error(e);
      console.error('================================================\n');
      return res.status(500).json({
        status: 'error',
        code: 'ERR_INTERNAL',
        description: 'An internal error has occurred.'
      });
    }
  });

  const PORT = parseInt(process.env.PORT || '8099', 10);
  app.listen(PORT, () => {
    console.log(`\nTest certifier server listening on http://localhost:${PORT}`);
    console.log(`Using REAL SDK auth middleware + ProtoWallet`);
    console.log(`\nTo test: modify acquire_certificate certifier_url to http://localhost:${PORT}\n`);
  });
}

main().catch(e => {
  console.error('Failed to start server:', e);
  process.exit(1);
});
