# BSV Authentication Integration Guide

> **Status note (2026-05-05):** This guide remains relevant for the Phase 4 demo. Both auth surfaces it documents — BRC-103/104 direct wallet auth and Sigma Identity OAuth 2.0 — work in Hodos: BRC-103/104 is what we already implement, and Sigma OAuth works as a normal OAuth provider (like Google/X/GitHub login) with no special Hodos code. The originally planned "Sigma OAuth interception with substituted Hodos identity" is **cancelled** (iframe-signer architecture blocks it — see `../FACT_CHECK_RESULTS.md` Q3 and the sprint `OPEN_QUESTIONS.md` OQ#1). The OAuth-as-provider content below is the current truth and should be the basis of the demo's "Sign in with Sigma" button.

> Guide for web developers to add BSV wallet authentication to their apps.
> Two methods: BRC-103/104 (direct wallet auth) and Sigma Identity (OAuth 2.0 style).
> Saved for future Hodos demo video build.

## WHAT TO BUILD

A clean, modern auth page with:

1. **Two auth buttons:**
   - "Sign in with BSV Wallet" (BRC-103/104 protocol)
   - "Sign in with Sigma Identity" (OAuth 2.0 flow)

2. **Account creation flow** (after first-time auth):
   - User has just proven their identity via crypto signature
   - Show optional MFA setup screen:
     - [ ] Add email address (for account recovery / 2FA codes)
     - [ ] Add password (as backup auth method)
     - [ ] Enable TOTP authenticator app (Google Authenticator, Authy)
   - These are OPTIONAL — the crypto key IS the primary auth
   - Store user preferences in your database

3. **Identity key request dialog:**
   - After auth succeeds, show the user a permission prompt:
     "This app would like to know your public identity key.
      This allows the app to identify you. You can decline."
     - [Allow] [Deny]
   - If allowed, fetch the identity key from the wallet
   - If denied, create an anonymous session (app cannot identify user across visits)

## BRC-103/104 IMPLEMENTATION (BSV Wallet Auth)

This is a mutual authentication protocol. Your server and the user's
BSV wallet prove identity to each other using cryptographic signatures.

### Flow:

```
Your App                          User's BSV Wallet
   |                                    |
   |--- POST /.well-known/auth -------->|
   |    {                               |
   |      version: "0.1",              |
   |      messageType: "initialRequest",|
   |      identityKey: <YOUR_SERVER_PUBKEY>,
   |      initialNonce: <random 32 bytes, base64>
   |    }                               |
   |                                    |
   |<--- Response ----------------------|
   |    {                               |
   |      version: "0.1",              |
   |      messageType: "initialResponse",
   |      identityKey: <USER_PUBKEY>,   |  ← app-scoped, unique per app
   |      initialNonce: <wallet_nonce>, |
   |      yourNonce: <echo_your_nonce>, |
   |      signature: <DER_bytes>        |  ← ECDSA proof
   |    }                               |
   |                                    |
   |--- Verify signature -------------->|
   |    (proves wallet owns the key)    |
```

### Server-side implementation:

```javascript
const crypto = require('crypto');
const express = require('express');

// Your server's identity key pair (generate once, store securely)
// Use secp256k1 - same curve as Bitcoin
const serverKeyPair = generateSecp256k1KeyPair(); // You need a secp256k1 library

app.post('/.well-known/auth', (req, res) => {
  const { messageType, identityKey, initialNonce } = req.body;

  if (messageType === 'initialRequest') {
    // 1. Store the client's nonce and identity key
    const clientNonce = initialNonce;
    const clientIdentityKey = identityKey;

    // 2. Generate our nonce
    const serverNonce = crypto.randomBytes(32).toString('base64');

    // 3. Sign: SHA256(decode(clientNonce) || decode(serverNonce))
    //    Using BRC-42 derived child key (see BRC-42 section below)
    const nonceConcat = Buffer.concat([
      Buffer.from(clientNonce, 'base64'),
      Buffer.from(serverNonce, 'base64')
    ]);
    const hash = crypto.createHash('sha256').update(nonceConcat).digest();
    const signature = signWithChildKey(hash, clientIdentityKey);

    // 4. Store session: map clientIdentityKey → nonces
    sessions.set(clientIdentityKey, { clientNonce, serverNonce });

    res.json({
      version: '0.1',
      messageType: 'initialResponse',
      identityKey: serverKeyPair.publicKey, // Your server's public key
      initialNonce: serverNonce,
      yourNonce: clientNonce,
      signature: Array.from(signature) // DER-encoded ECDSA signature
    });
  }
});
```

### BRC-42 Key Derivation (used for signing):

BRC-42 derives unique child keys per counterparty + protocol:
1. ECDH shared secret = server_privkey × client_pubkey
2. HMAC = HMAC-SHA256(shared_secret, invoice_number)
3. Child private key = server_privkey + HMAC_scalar (mod N)

Invoice format (BRC-43): "{securityLevel}-{protocolID}-{keyID}"
Example: "2-auth message signature-{clientNonce} {serverNonce}"

Use the `@bsv/sdk` npm package which implements BRC-42:
```javascript
const { PrivateKey, PublicKey, HD } = require('@bsv/sdk');
// Or use: https://github.com/bitcoin-sv/ts-sdk
```

### After auth - requesting identity key:

```javascript
// After successful BRC-103 auth, the wallet returns an app-scoped
// identity key. This key is UNIQUE TO YOUR APP — the user's other
// apps see a different key. This prevents cross-app tracking.
//
// The identity key from the auth response IS the user's identifier
// for your app. You don't need to request it separately.
//
// But if you want the user's MASTER identity key (same across all apps),
// you must explicitly request it and the user must approve:

app.post('/api/request-identity', (req, res) => {
  // This triggers a permission prompt in the user's wallet
  // The wallet will ask: "App wants your public identity key. Allow?"
  // Return the request to the frontend to forward to the wallet
  res.json({
    requestType: 'identityKey',
    reason: 'To create your profile and let others find you',
    required: false // User can decline
  });
});
```

## SIGMA IDENTITY IMPLEMENTATION (OAuth 2.0 Style)

Sigma Identity is an OAuth 2.0-compatible BSV auth service by GorillaPool.
Documentation: https://docs.sigmaidentity.com/

### Flow:

```javascript
// 1. Redirect user to Sigma Identity login
const SIGMA_AUTH_URL = 'https://auth.sigmaidentity.com';
const CLIENT_ID = 'your-registered-client-id'; // Register at sigmaidentity.com

app.get('/auth/sigma', (req, res) => {
  const state = crypto.randomBytes(16).toString('hex');
  req.session.oauthState = state;

  const params = new URLSearchParams({
    client_id: CLIENT_ID,
    redirect_uri: `${YOUR_DOMAIN}/auth/sigma/callback`,
    response_type: 'code',
    state: state,
    scope: 'identity' // Request identity scope
  });

  res.redirect(`${SIGMA_AUTH_URL}/authorize?${params}`);
});

// 2. Handle callback
app.get('/auth/sigma/callback', async (req, res) => {
  const { code, state } = req.query;

  // Verify state matches
  if (state !== req.session.oauthState) {
    return res.status(403).send('Invalid state');
  }

  // Exchange code for token
  const tokenResponse = await fetch(`${SIGMA_AUTH_URL}/token`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      grant_type: 'authorization_code',
      code,
      client_id: CLIENT_ID,
      client_secret: CLIENT_SECRET,
      redirect_uri: `${YOUR_DOMAIN}/auth/sigma/callback`
    })
  });

  const { access_token } = await tokenResponse.json();

  // 3. Fetch user identity
  const userResponse = await fetch(`${SIGMA_AUTH_URL}/userinfo`, {
    headers: { Authorization: `Bearer ${access_token}` }
  });

  const user = await userResponse.json();
  // user.publicKey — the user's BSV public key
  // user.paymail — if available

  // Create or find user in your database
  createOrLoginUser(user);
});
```

## ACCOUNT CREATION UI (After First Auth)

After the user authenticates via either method, show this:

```
┌─────────────────────────────────────────────┐
│  Welcome! Your account is secured by your   │
│  cryptographic key. Optionally add backup   │
│  authentication methods:                    │
│                                             │
│  ☐ Add email address                        │
│    └─ For account recovery & notifications  │
│    └─ [email input field]                   │
│                                             │
│  ☐ Add a password                           │
│    └─ As a backup login method              │
│    └─ [password input field]                │
│    └─ [confirm password field]              │
│                                             │
│  ☐ Enable authenticator app (TOTP)          │
│    └─ Google Authenticator, Authy, etc.     │
│    └─ [Show QR code for TOTP setup]         │
│                                             │
│  These are optional. Your crypto key is     │
│  your primary authentication method.        │
│                                             │
│  [Skip for now]        [Save preferences]   │
└─────────────────────────────────────────────┘
```

## DATABASE SCHEMA

```sql
CREATE TABLE users (
  id TEXT PRIMARY KEY,           -- UUID
  identity_key TEXT UNIQUE,      -- BSV public key (app-scoped from BRC-103)
  master_identity_key TEXT,      -- Master key (only if user allowed)
  auth_method TEXT NOT NULL,     -- 'brc103' or 'sigma'
  email TEXT,                    -- Optional
  password_hash TEXT,            -- Optional (bcrypt)
  totp_secret TEXT,              -- Optional (base32 TOTP secret)
  identity_key_shared BOOLEAN DEFAULT FALSE,
  created_at TIMESTAMP DEFAULT NOW(),
  last_login TIMESTAMP
);

CREATE TABLE sessions (
  id TEXT PRIMARY KEY,
  user_id TEXT REFERENCES users(id),
  token TEXT NOT NULL,
  created_at TIMESTAMP DEFAULT NOW(),
  expires_at TIMESTAMP NOT NULL
);
```

## KEY REFERENCES

- BRC-103 spec: https://bsv.brc.dev/peer-to-peer/0103
- BRC-104 (HTTP transport): https://bsv.brc.dev/peer-to-peer/0104
- BRC-42 (key derivation): https://bsv.brc.dev/key-derivation/0042
- BRC-43 (invoice format): https://bsv.brc.dev/key-derivation/0043
- BRC-52 (identity certificates): https://bsv.brc.dev/peer-to-peer/0052
- BSV SDK (TypeScript): https://github.com/bitcoin-sv/ts-sdk
- Sigma Identity docs: https://docs.sigmaidentity.com/
- Sigma Identity auth: https://auth.sigmaidentity.com

## IMPORTANT NOTES

- The identity key returned from BRC-103 auth is APP-SCOPED — it's
  unique to your app. The same user authenticating with a different
  app gets a different key. This is a privacy feature that prevents
  cross-app tracking.
- If you need the user's MASTER identity key (same across all apps),
  you must request it explicitly and the user must approve. Only do
  this if you have a genuine need (e.g., user wants a public profile).
- All crypto signing happens in the user's wallet, never on your server.
- Use the @bsv/sdk npm package for secp256k1 operations and BRC-42.
- For TOTP, use the 'otpauth' or 'speakeasy' npm package.
- Hash passwords with bcrypt, never store plaintext.
