#!/usr/bin/env node
// Test SocialCert using the TypeScript SDK directly.
// If this ALSO fails with 500, the problem is on SocialCert's end.

import { createRequire } from 'module';
const require = createRequire(import.meta.url);
const sdkPath = '../reference/ts-brc100/node_modules/@bsv/sdk';

const {
  ProtoWallet, PrivateKey, Utils, Random, Hash,
  MasterCertificate, createNonce, Certificate
} = require(sdkPath);
const { AuthFetch } = require(sdkPath + '/dist/cjs/src/auth/clients/AuthFetch.js');

// Use the same key as our Rust wallet
const CLIENT_PRIV_HEX = 'be8d816a4c3bb97335a5e03c2590687c4cde7c3c3fb0005b07a1d3a65c7dfc3e';
const clientPrivKey = PrivateKey.fromString(CLIENT_PRIV_HEX, 16);
const clientWallet = new ProtoWallet(clientPrivKey);

const SOCIALCERT_URL = 'https://backend.socialcert.net';
const SOCIALCERT_CERTIFIER_KEY = '02cf6cdf466951d8dfc9e7c9367511d0007ed6fba35ed42d425cc412fd6cfd4a17';

// SocialCert Twitter certificate type
const CERT_TYPE = 'vdDWvftf1H+5+ZprUw123kjHlywH+v20aPQTuXgMpNc=';

async function main() {
  const clientPubKey = (await clientWallet.getPublicKey({ identityKey: true })).publicKey;
  console.log('Client pubkey:', clientPubKey);
  console.log('');

  // Try using SDK's AuthFetch to call SocialCert /signCertificate
  console.log('=== Testing SDK AuthFetch against SocialCert ===');

  const authFetch = new AuthFetch(clientWallet);

  // Create CSR nonce with SocialCert as counterparty
  const csrNonce = await createNonce(clientWallet, SOCIALCERT_CERTIFIER_KEY);
  console.log('CSR nonce:', csrNonce.substring(0, 30) + '...');

  // Create encrypted fields using SDK
  const { certificateFields, masterKeyring } = await MasterCertificate.createCertificateFields(
    clientWallet, SOCIALCERT_CERTIFIER_KEY, {
      userName: 'bsvarchie',
      profilePhoto: 'https://pbs.twimg.com/profile_images/1640477660475432960/MqGVgc-N_normal.jpg'
    }
  );

  const csrBody = {
    clientNonce: csrNonce,
    type: CERT_TYPE,
    fields: certificateFields,
    masterKeyring
  };

  console.log('CSR body keys:', Object.keys(csrBody));
  console.log('CSR fields:', Object.keys(certificateFields));
  console.log('CSR masterKeyring keys:', Object.keys(masterKeyring));
  console.log('');

  try {
    console.log('Sending to:', `${SOCIALCERT_URL}/signCertificate`);
    const resp = await authFetch.fetch(`${SOCIALCERT_URL}/signCertificate`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(csrBody)
    });

    const respBody = await resp.text();
    console.log('Response status:', resp.status);
    console.log('Response body (first 500):', respBody.substring(0, 500));

    if (resp.status === 200) {
      console.log('\n✅ SDK AuthFetch against SocialCert: SUCCESS');
    } else {
      console.log('\n❌ SDK AuthFetch against SocialCert: FAILED');
      console.log('This confirms the issue is on SocialCert\'s end, not ours.');
    }
  } catch (e) {
    console.log('SDK AuthFetch error:', e.message);
    console.log('\n❌ SDK also fails against SocialCert — the issue is on their end');
  }
}

main().catch(e => {
  console.error('FATAL:', e.message);
  console.error(e.stack);
  process.exit(1);
});
