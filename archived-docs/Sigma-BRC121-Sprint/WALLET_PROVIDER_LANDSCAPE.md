# BSV Wallet Provider Landscape

Synthesizes the wallet/app/protocol research into one map for sprint-prioritization decisions.

---

## The three wallet-API conventions in BSV-land

| Convention | Where the API lives | Used by | Hodos status |
|------------|---------------------|---------|--------------|
| **BRC-100** (de jure standard) | JS provider, e.g. `window.hodosBrowser.brc100.*` | Babbage app catalog, MetaNet Desktop apps (archived), BSV Association tools, **Yours-after-migration (reportedly)** | ✓ Already implemented |
| **`window.yours`** (de facto standard) | JS provider injected by Yours Wallet Chrome extension | 1sat.market, 3DOrdi, Treechat, Zoide (if alive), most 1Sat-ecosystem apps | **Not implemented — primary opportunity** |
| **HandCash Connect** (proprietary) | Server-side OAuth + custodial SDK | HandCash apps, custodial games | Not on roadmap (custodial; conflicts with Hodos's self-custody model) |

**Key insight:** dApps don't talk to wallets via HTTP. They call JavaScript methods on a wallet-injected `window.*` object. The wallet's own code does all the HTTP work (indexer fetches, broadcast). So shimming a wallet API is a JS injection task (V8 `OnContextCreated`), not an HTTP interception task.

---

## Wallets

| Wallet | Status | API surface exposed | Open source? | Sprint relevance |
|--------|--------|---------------------|--------------|------------------|
| **Yours Wallet** | Alive, primary target | `window.yours` (de facto std) | Yes (yours-org/yours-wallet) | **Shim source.** Mirror its API. |
| Panda Wallet | Dead — renamed to Yours | `window.panda` (legacy alias) | Yes | Alias `window.yours` to `window.panda` for compat. |
| MetaNet Desktop / BSV Desktop | **Archived 2025-10-27** | BRC-100 reference impl | Yes | Already done — Hodos implements BRC-100. |
| HandCash | Alive, irrelevant for our scope | HandCash Connect SDK (server-side) | Partial | Custodial; out of scope. |
| bitcoin-sv/spv-wallet-browser | Active fork of Yours | `window.yours` (forked) | Yes | Treat as a Yours dialect; same shim covers it. |

---

## Apps (current life signs)

| App | Loads? | Auth | Wallet API | Sprint relevance |
|-----|--------|------|------------|------------------|
| **1sat.market** | Yes | Self-custodial seed entry | None confirmed (likely `window.yours`) | Phase 3 ordinals integration target |
| **3DOrdi** | likely (SPA) | Non-custodial; "interoperable with Yours" per CoinGeek | `window.yours` (presumed) | Phase 3 + `window.yours` shim test target |
| **Treechat** (popular BSV social app) | TBD | TBD | `window.yours` (per user, claims pending fact-check) | `window.yours` shim test target — popularity signal |
| **BSVradar.com** | Yes (user verified) | **Sigma auth (CONFIRMED)** | TBD | **Primary Phase 0 mitmproxy target.** Only confirmed real Sigma user. |
| Zoide | ECONNREFUSED on `.com` (may be wrong URL) | unknown | unknown | Pending fact-check on alternate URLs. |
| FireSat | Renders | unknown | unknown | Marginal. |
| MetaLens | Private beta only | "working on identity" | n/a | Watch list. |
| BitChat Nitro | Hosted instance 404 | n/a | n/a | Dead deploy. |
| 1sat.app | Landing only | "Apple Secure Enclave + BAP" | n/a | **Competitor browser** — read for positioning. |

**Realistic verified-Sigma-user count today: 1 (BSVradar).**

---

## What "shim `window.yours`" actually means

Implement these methods, each as a thin V8 → IPC → Rust handler:

```ts
// Connection / discovery
isConnected()
connect()                  // → { identityPubKey, ordPubKey, bsvPubKey }
getAddresses()             // → { bsvAddress, ordAddress, identityAddress }
getBalance()
getExchangeRate()
getSocialProfile()
getPubKeys()

// BSV transfers
sendBsv(payments[])        // → { txid, rawtx }

// Ordinals (Phase 3 territory)
getOrdinals()
inscribe(items)
transferOrdinal({ address, origin, outpoint })
purchaseOrdinal({ outpoint, marketplaceRate?, marketplaceAddress? })

// Signing / utilities
signMessage({ message, encoding? })
getSignatures(params)      // arbitrary input signing
broadcast({ rawtx })
encrypt({ message, pubKeys })
decrypt({ messages })
```

Source: Yours provider gitbook ([root](https://yours-wallet.gitbook.io/provider-api)).

**Map to Hodos primitives:**
- `connect`, `getAddresses`, `getBalance`, `getExchangeRate` → existing `wallet_*` Rust handlers
- `sendBsv` → existing `create_action_internal`
- `signMessage` → Phase 2A's new `sign_message` handler (BSM/BRC-77)
- `broadcast` → existing broadcast path
- `encrypt`/`decrypt` → existing BRC-2 handlers
- `getSignatures` → existing `sign_action` / `create_action`
- Ordinal methods (`getOrdinals`, `inscribe`, `transferOrdinal`, `purchaseOrdinal`) → require Phase 3 ordinal work (UTXO classification, indexer client, Ordinal Lock script template)

**Conclusion:** `window.yours` shim minus ordinal methods is mostly thin glue over existing Hodos infrastructure. Ordinal methods require the Phase 3 backend.

---

## How a typical 1Sat purchase works (concrete HTTP trace)

```
1. dApp:    window.yours.purchaseOrdinal({ outpoint, marketplaceRate, marketplaceAddress })
   ↓ (JavaScript function call — no HTTP)
2. Wallet:  GET https://ordinals.gorillapool.io/api/txos/<outpoint>
            → { script, owner, listing: { price, payout }, origin }
3. Wallet:  GET https://ordinals.gorillapool.io/api/utxos/address/<buyer_bsv_address>
            (fund the purchase)
4. Wallet:  builds tx — spends Ordinal Lock UTXO + funding UTXOs
            outputs: ordinal → buyer.ordAddress
                     payout → seller (enforced by Ordinal Lock script)
                     marketplace fee → marketplaceAddress (optional)
                     change → buyer
            signs SIGHASH_ALL | FORKID
5. Wallet:  POST https://arc.gorillapool.io/v1/tx  (BEEF; WoC fallback)
            → { txid }
6. Wallet:  returns txid to dApp
```

The dApp itself never makes wallet-level HTTP requests. The contract is purely JS.

---

## How Sigma OAuth works (concrete HTTP trace)

```
1. dApp:    redirects browser to
            GET https://auth.sigmaidentity.com/oauth2/authorize
                ?client_id=...&redirect_uri=...&response_type=code
                &state=...&scope=...&code_challenge=...&code_challenge_method=S256

2. Sigma:   serves login page with in-page ephemeral keypair (THIS IS THE UX PROBLEM)

3. User:    signs bitcoin-auth token
            (token format: pubkey|scheme|timestamp|requestPath|signature)
            (signed body: requestPath + ISO8601 timestamp + SHA256(body) + scheme)
            (default scheme: brc77)

4. Sigma:   POST /sigma/authorize { token }   (or interactive flow continues)
            verifies signature ↔ pubkey ↔ message
            "Sigma Auth acts as a consumer of BAP data rather than a gatekeeper"
            → redirects back to dApp with auth code

5. dApp:    POST https://auth.sigmaidentity.com/api/auth/oauth2/token
            { grant_type, code, client_id, client_secret, redirect_uri }
            → { access_token (ES256 JWT, 30-day TTL), token_type, expires_in }

6. dApp:    GET /api/auth/oauth2/userinfo with Authorization: Bearer ...
            → { pubkey, bap: { idKey, rootAddress, currentAddress, ... }, ... }
```

**Hodos interception point:** step 3. We intercept the `auth.sigmaidentity.com` redirect, sign the bitcoin-auth token with Hodos's wallet-rooted key, and substitute it for the in-page ephemeral key. Per Sigma's docs, the verifier doesn't care that the key wasn't issued by them — it just checks signature ↔ pubkey ↔ message. **But this needs empirical confirmation via mitmproxy capture on BSVradar.com before we ship.**

---

## Strategic posture (working hypothesis)

Per current research, the leverage curve is:

| Surface | Real apps today | Effort | Strategic value |
|---------|----------------|--------|-----------------|
| BRC-100 | Babbage catalog, Yours-after-migration | Done | Locked in |
| **`window.yours` shim (non-ordinal methods)** | 1sat.market, 3DOrdi, Treechat, ecosystem | M | **High right now** |
| BRC-121 | ~zero production servers | XS (~150 LOC) | Cheap speculative bet, ship anyway |
| BSM/BRC-77 signing primitives (Phase 2A) | Useful for content signing/tipping regardless | S (~300 LOC) | Cheap, broadly useful |
| Sigma OAuth interception | 1 confirmed user (BSVradar) | M (interception layer) | Speculative — let demand grow before shipping |
| `window.yours` ordinal methods (full impl) | Many apps | L (5–7 sprints) | High if Hodos wants the ordinal market |

**Working recommendation (pending fact-checks):**
1. Keep BRC-121 (cheap)
2. Keep Phase 2A signing primitives (cheap, broadly useful)
3. **Promote `window.yours` shim (non-ordinal methods) to a near-term phase** — high leverage
4. Demote Phase 2B Sigma interception to "after BSVradar mitmproxy + evidence of more real apps"
5. Phase 3 ordinal full support stays large; `window.yours` shim is the front door, ordinal backend is what plugs in behind it later
6. Track Yours's BRC-100 migration plan — could change everything (pending fact-check Q1)

---

## Open fact-check questions (in flight)

1. Yours's BRC-100 migration plan — additive or replacement, timeline, will `window.yours` survive?
2. Treechat's actual `window.yours.*` call list — view-source the JS bundle
3. BSVradar's Sigma flow detail — what scopes, what client_id, mitmproxy-able?
4. Zoide's actual status — try alternate URLs, GitHub, social signals
