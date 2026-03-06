# SocialCert: Social Identity Certificates on BSV

> **What:** Certificates that link social platform accounts to BSV public keys  
> **Why:** Enable verified identity for content signing, payments, and trust

---

## Table of Contents

1. [Overview](#overview)
2. [How Social Certificates Work](#how-social-certificates-work)
3. [Authentication Flow: How Does the Certifier Know You Own the Account?](#authentication-flow)
4. [Certificate Structure (BRC-52)](#certificate-structure-brc-52)
5. [Supported Platforms](#supported-platforms)
6. [Extending to Gmail/YouTube](#extending-to-gmailyoutube)
7. [Security Considerations](#security-considerations)
8. [One Account, Multiple Keys?](#one-account-multiple-keys)
9. [Integration with Hodos](#integration-with-hodos)
10. [Open Questions](#open-questions)

---

## Overview

### The Problem

You have a BSV public key: `02a1b2c3d4e5f6...`

Nobody knows who that is. You could claim to be anyone.

### The Solution

A **trusted certifier** verifies you control a social account (X.com, Discord, email) and issues a **certificate** binding that identity to your public key.

```
┌─────────────────────────────────────────────────────────┐
│                    SOCIALCERT                           │
├─────────────────────────────────────────────────────────┤
│  "I, SocialCert (trusted certifier), verify that       │
│   public key 02a1b2c3... is controlled by the same     │
│   person who controls @username on X.com.               │
│                                                         │
│   Signed: SocialCert"                                   │
└─────────────────────────────────────────────────────────┘
```

Now anyone who trusts SocialCert can trust that your public key = your social identity.

---

## How Social Certificates Work

### The Three Parties

```
┌──────────────┐     ┌──────────────┐     ┌──────────────┐
│    USER      │     │  CERTIFIER   │     │   VERIFIER   │
│  (subject)   │     │ (SocialCert) │     │   (anyone)   │
├──────────────┤     ├──────────────┤     ├──────────────┤
│ • Has BSV    │     │ • Trusted    │     │ • Wants to   │
│   key pair   │     │   authority  │     │   verify     │
│ • Has social │     │ • Issues     │     │   identity   │
│   account    │     │   certs      │     │ • Trusts     │
│ • Wants cert │     │ • Signs      │     │   certifier  │
└──────────────┘     └──────────────┘     └──────────────┘
```

### High-Level Flow

```
1. USER → CERTIFIER: "I want a certificate for @myhandle"

2. CERTIFIER → SOCIAL PLATFORM: Initiates OAuth flow

3. USER → SOCIAL PLATFORM: Logs in, authorizes certifier

4. SOCIAL PLATFORM → CERTIFIER: "Yes, this user controls @myhandle"

5. CERTIFIER: Creates certificate binding @myhandle to user's pubkey

6. CERTIFIER → BSV: Publishes certificate on-chain (revocable UTXO)

7. CERTIFIER → USER: Returns certificate
```

---

## Authentication Flow

### How Does the Certifier Know You Own the Account?

This is the critical question. The answer: **OAuth 2.0 Authorization Flow**.

### X.com (Twitter) Authentication

```
┌─────────────────────────────────────────────────────────┐
│                SOCIALCERT OAUTH FLOW                    │
├─────────────────────────────────────────────────────────┤
│                                                         │
│  1. User clicks "Get X.com Certificate" on SocialCert   │
│                          │                              │
│                          ▼                              │
│  2. SocialCert redirects to X.com authorize URL:        │
│     https://x.com/i/oauth2/authorize                    │
│     ?client_id=SOCIALCERT_CLIENT_ID                     │
│     &redirect_uri=https://socialcert.net/callback       │
│     &scope=tweet.read users.read                        │
│     &response_type=code                                 │
│     &code_challenge=...                                 │
│                          │                              │
│                          ▼                              │
│  3. User logs into X.com (if not already)               │
│     X shows: "SocialCert wants to access your account"  │
│     User clicks "Authorize"                             │
│                          │                              │
│                          ▼                              │
│  4. X.com redirects back to SocialCert with auth code   │
│     https://socialcert.net/callback?code=ABC123         │
│                          │                              │
│                          ▼                              │
│  5. SocialCert exchanges code for access token          │
│     POST https://api.x.com/2/oauth2/token               │
│                          │                              │
│                          ▼                              │
│  6. SocialCert calls GET /2/users/me                    │
│     Returns: { "id": "123", "username": "myhandle" }    │
│                          │                              │
│                          ▼                              │
│  7. SocialCert now KNOWS the user controls @myhandle    │
│     Issues certificate linking handle → user's pubkey   │
│                                                         │
└─────────────────────────────────────────────────────────┘
```

### Why This Is Secure

1. **User authenticates directly with X.com** — SocialCert never sees the password
2. **X.com confirms identity** — The API response is authoritative
3. **OAuth is standard** — Battle-tested, widely used
4. **Time-bounded** — Verification happens at certificate issuance time

### What the Certifier Receives

From X.com API (`/2/users/me`):
```json
{
  "data": {
    "id": "123456789",
    "name": "Display Name",
    "username": "myhandle"
  }
}
```

This `username` is what goes into the certificate.

---

## Certificate Structure (BRC-52)

### Binary Format

```
┌─────────────────────────────────────────────────────────┐
│                BRC-52 CERTIFICATE                       │
├─────────────────────────────────────────────────────────┤
│  Type: "social" (certificate type identifier)          │
│  SerialNumber: "abc123..." (unique cert ID)            │
│  Subject: <user's public key>                          │
│  Certifier: <SocialCert's public key>                  │
│  RevocationOutpoint: <TXID:outputIndex>                │
│  Fields: {                                              │
│    "platform": "x.com" (encrypted)                     │
│    "handle": "@myhandle" (encrypted)                   │
│    "userId": "123456789" (encrypted)                   │
│    "certifiedAt": "2026-03-04T..." (encrypted)         │
│  }                                                      │
│  Signature: <certifier's signature over above>         │
└─────────────────────────────────────────────────────────┘
```

### Key Properties

| Property | Description |
|----------|-------------|
| **Encrypted Fields** | Only holder can decrypt; selective disclosure |
| **UTXO-based Revocation** | Spend the outpoint = revoke the certificate |
| **Certifier Signature** | Proves SocialCert issued this |
| **Subject Key** | Links to user's BSV identity |

### Selective Disclosure

When proving identity:
1. User creates "verifiable certificate" for specific verifier
2. Re-encrypts only the fields they want to share
3. Verifier receives proof of specific claims without seeing all data

Example: Prove you own @myhandle without revealing your platform user ID.

---

## Supported Platforms

### Current SocialCert Support

| Platform | Status | OAuth Type |
|----------|--------|------------|
| **X.com (Twitter)** | ✅ Supported | OAuth 2.0 PKCE |
| **Discord** | ✅ Supported | OAuth 2.0 |
| **Email** | ✅ Supported | Verification link |

### OAuth Details by Platform

**X.com:**
- Scopes: `tweet.read`, `users.read`
- Endpoint: `https://api.x.com/2/oauth2/authorize`
- User info: `GET /2/users/me`

**Discord:**
- Scopes: `identify`
- Endpoint: `https://discord.com/api/oauth2/authorize`
- User info: `GET /api/users/@me`

**Email:**
- No OAuth — sends verification link
- User clicks link to prove ownership
- Less secure (email forwarding possible)

---

## Extending to Gmail/YouTube

### Google OAuth Flow

Google's OAuth provides verified identity for Gmail and YouTube:

```
┌─────────────────────────────────────────────────────────┐
│               GOOGLE OAUTH FLOW                         │
├─────────────────────────────────────────────────────────┤
│  1. Redirect to Google authorize URL:                   │
│     https://accounts.google.com/o/oauth2/auth          │
│     ?client_id=YOUR_CLIENT_ID                          │
│     &redirect_uri=https://certifier.example/callback   │
│     &scope=openid email profile                        │
│     &response_type=code                                │
│                                                         │
│  2. User logs in, authorizes                            │
│                                                         │
│  3. Exchange code for ID token + access token           │
│                                                         │
│  4. Validate ID token or call userinfo endpoint:        │
│     GET https://www.googleapis.com/oauth2/v3/userinfo  │
│                                                         │
│  5. Response includes:                                  │
│     {                                                   │
│       "sub": "110248495921238986420",                  │
│       "email": "user@gmail.com",                       │
│       "email_verified": true,                          │
│       "name": "User Name",                             │
│       "picture": "https://..."                         │
│     }                                                   │
└─────────────────────────────────────────────────────────┘
```

### Gmail Certificates

**What you'd certify:**
- `email`: user@gmail.com
- `email_verified`: true (Google confirms ownership)
- `google_id`: unique stable identifier

**Certificate fields:**
```json
{
  "platform": "gmail",
  "email": "user@gmail.com",
  "googleId": "110248495921238986420",
  "certifiedAt": "2026-03-04T..."
}
```

### YouTube Certificates

**Additional scope needed:**
- `https://www.googleapis.com/auth/youtube.readonly`

**What you'd certify:**
- YouTube channel ID
- Channel name
- Subscriber count (optional, time-sensitive)

**API call:**
```
GET https://www.googleapis.com/youtube/v3/channels?mine=true
```

**Response:**
```json
{
  "items": [{
    "id": "UCxxxxx",
    "snippet": {
      "title": "My Channel Name",
      "customUrl": "@mychannelhandle"
    }
  }]
}
```

### Requirements to Add Gmail/YouTube

| Requirement | Details |
|-------------|---------|
| **Google Cloud Project** | Register app, get client ID/secret |
| **OAuth Consent Screen** | Configure scopes, app name |
| **Verification** | Google reviews apps accessing sensitive scopes |
| **Privacy Policy** | Required for OAuth apps |
| **Certifier Backend** | Extend to handle Google OAuth flow |

### Hodos as Certifier?

Could Hodos run its own certification service?

**Pros:**
- No external dependency
- Control over trust model
- Revenue opportunity (charge for certs?)

**Cons:**
- Need to maintain backend service
- Need API access agreements with platforms
- Trust building (new certifier vs. established)

**Recommendation:** Start by integrating with SocialCert, consider running own certifier later.

---

## Security Considerations

### OAuth Attack Vectors

| Attack | Mitigation |
|--------|------------|
| **Phishing** | Users must verify they're on real platform |
| **Token theft** | Short-lived tokens, PKCE flow |
| **Replay attacks** | Nonces, state parameters |
| **Man-in-the-middle** | HTTPS everywhere |

### Certificate Security

| Risk | Mitigation |
|------|------------|
| **Compromised certifier** | Revocation mechanism, multi-certifier trust |
| **Key theft** | User must revoke cert if key compromised |
| **Fake certificates** | Verifiers check certifier signature |
| **Stale certificates** | Time limits? Renewal requirements? |

### Platform Account Compromise

If someone's X.com account is hacked:
1. Hacker could get new certificate for that handle
2. Multiple valid certs for same handle
3. **Mitigation:** Certificate timestamp + reputation over time

---

## One Account, Multiple Keys?

### Can One Social Account Have Multiple Certificates?

**Yes, technically possible:**
- User has X.com account @myhandle
- User has BSV key pair A
- User authenticates, gets cert linking @myhandle → key A
- User creates key pair B
- User authenticates again, gets cert linking @myhandle → key B

**Both certificates are valid** — the certifier verified ownership at issuance time.

### Why Would Someone Do This?

| Use Case | Validity |
|----------|----------|
| **Multiple devices** | Different key per device, same identity |
| **Key rotation** | Old key compromised, get new cert |
| **Delegation** | Give employee cert for company account |
| **Fraud** | Sell/share account access (bad) |

### How to Handle

**Option A: No restrictions**
- Certifier issues cert whenever OAuth succeeds
- Up to verifiers to check timestamps, choose which to trust

**Option B: Revoke old on new**
- New certificate automatically revokes previous
- Single valid cert per handle at any time

**Option C: Certificate limits**
- Certifier limits certs per account per time period
- Prevents rapid multi-key issuance

**Recommendation for Hodos:** Accept any valid cert from trusted certifier. Show timestamp. Let users decide if recent cert is more trustworthy than old one.

---

## Integration with Hodos

### Certificate Acquisition Flow

```
┌─────────────────────────────────────────────────────────┐
│                    HODOS UI                             │
├─────────────────────────────────────────────────────────┤
│  Settings → Identity → Social Certificates              │
│  ┌─────────────────────────────────────────────────────┐│
│  │ Your Certificates:                                  ││
│  │ (none)                                              ││
│  │                                                     ││
│  │ [+ Add X.com]  [+ Add Discord]  [+ Add Gmail]      ││
│  └─────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────┘
           │
           ▼ User clicks "+ Add X.com"
┌─────────────────────────────────────────────────────────┐
│  1. Hodos opens SocialCert in popup/tab                 │
│  2. User authenticates with X.com                       │
│  3. SocialCert issues certificate                       │
│  4. Certificate returned to Hodos (deep link/callback)  │
│  5. Hodos stores certificate in wallet                  │
└─────────────────────────────────────────────────────────┘
```

### Certificate Storage

Stored in Hodos wallet database (BRC-52 format):
- `certificates` table
- Fields: type, subject, certifier, fields, signature, revocationOutpoint
- Indexed by type and certifier for quick lookup

### Using Certificates

**When signing content:**
1. User signs post on X.com
2. Hodos includes certificate reference in signature
3. Verifiers can look up certificate to see identity

**When receiving payment:**
1. Payer sees verified @handle on content
2. Payment sent to public key from certificate
3. User receives in Hodos wallet

### Trust Configuration

```
┌─────────────────────────────────────────────────────────┐
│  Settings → Identity → Trusted Certifiers               │
│  ┌─────────────────────────────────────────────────────┐│
│  │ ✅ SocialCert (socialcert.net)                      ││
│  │    Types: X.com, Discord, Email                     ││
│  │                                                     ││
│  │ ✅ BSV Certificate Authority (bsvca.com)            ││
│  │    Types: Vendor verification                       ││
│  │                                                     ││
│  │ [+ Add Custom Certifier]                            ││
│  └─────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────┘
```

---

## Open Questions

1. **Certifier fees:** Does SocialCert charge for certificates? How does the business model work?

2. **Certificate expiration:** Should certs expire? What if someone loses access to social account but cert is still valid?

3. **Platform bans:** If X.com bans @myhandle, the cert still exists. How to handle?

4. **Cross-certifier trust:** If SocialCert and another certifier both issue certs for @myhandle to different keys, which is canonical?

5. **Hodos-issued certs:** Should Hodos become a certifier itself? What's the legal/regulatory landscape?

6. **Recovery:** If user loses keys, they need new cert. Is there friction in re-certification?

7. **Privacy:** OAuth gives certifier access token. What prevents certifier from scraping user's social data?

---

## References

- SocialCert: [socialcert.net](https://socialcert.net)
- SocialCert Backend (proprietary): [github.com/p2ppsr/socialcert-backend](https://github.com/p2ppsr/socialcert-backend)
- BRC-52 (Identity Certificates): BSV Academy
- BRC-103 (Peer-to-Peer Authentication): BSV Academy
- X.com OAuth 2.0: [developer.x.com](https://developer.x.com)
- Google OAuth 2.0: [developers.google.com/identity](https://developers.google.com/identity)

---

*Last updated: 2026-03-04*
