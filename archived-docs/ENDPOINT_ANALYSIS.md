# COMPREHENSIVE ENDPOINT COMPARISON ANALYSIS

## Table 1: Standard BRC-100 Wallet Endpoints

| Our Go Wallet | Metanet Desktop | Status | Analysis |
|---------------|-----------------|--------|----------|
| `POST /getVersion` | `POST /getVersion` | ✅ **SAME** | Both return wallet version and capabilities |
| `POST /getPublicKey` | `POST /getPublicKey` | ✅ **SAME** | Both return current public key |
| `POST /isAuthenticated` | `POST /isAuthenticated` | ✅ **SAME** | Both check authentication status |
| `POST /createSignature` | `POST /createSignature` | ✅ **SAME** | Both create signatures for data |
| `POST /verifySignature` | `POST /verifySignature` | ✅ **SAME** | Both verify signatures |
| `POST /createAction` | `POST /createAction` | ✅ **SAME** | Both create BRC-100 actions |
| `POST /signAction` | `POST /signAction` | ✅ **SAME** | Both sign BRC-100 actions |
| `POST /abortAction` | `POST /abortAction` | ✅ **SAME** | Both abort pending actions |
| `POST /listActions` | `POST /listActions` | ✅ **SAME** | Both list pending actions |
| `POST /internalizeAction` | `POST /internalizeAction` | ✅ **SAME** | Both internalize actions |
| `POST /listOutputs` | `POST /listOutputs` | ✅ **SAME** | Both list UTXOs |
| `POST /relinquishOutput` | `POST /relinquishOutput` | ✅ **SAME** | Both relinquish outputs |
| `POST /revealCounterpartyKeyLinkage` | `POST /revealCounterpartyKeyLinkage` | ✅ **SAME** | Both reveal counterparty keys |
| `POST /revealSpecificKeyLinkage` | `POST /revealSpecificKeyLinkage` | ✅ **SAME** | Both reveal specific keys |
| `POST /encrypt` | `POST /encrypt` | ✅ **SAME** | Both encrypt data |
| `POST /decrypt` | `POST /decrypt` | ✅ **SAME** | Both decrypt data |
| `POST /createHmac` | `POST /createHmac` | ✅ **SAME** | Both create HMAC |
| `POST /verifyHmac` | `POST /verifyHmac` | ✅ **SAME** | Both verify HMAC |
| `POST /acquireCertificate` | `POST /acquireCertificate` | ✅ **SAME** | Both acquire certificates |
| `POST /listCertificates` | `POST /listCertificates` | ✅ **SAME** | Both list certificates |
| `POST /proveCertificate` | `POST /proveCertificate` | ✅ **SAME** | Both prove certificates |
| `POST /relinquishCertificate` | `POST /relinquishCertificate` | ✅ **SAME** | Both relinquish certificates |
| `POST /discoverByIdentityKey` | `POST /discoverByIdentityKey` | ✅ **SAME** | Both discover by identity key |
| `POST /discoverByAttributes` | `POST /discoverByAttributes` | ✅ **SAME** | Both discover by attributes |
| `POST /waitForAuthentication` | `POST /waitForAuthentication` | ✅ **SAME** | Both wait for authentication |
| `POST /getHeight` | `POST /getHeight` | ✅ **SAME** | Both get blockchain height |
| `POST /getHeaderForHeight` | `POST /getHeaderForHeight` | ✅ **SAME** | Both get header for height |
| `POST /getNetwork` | `POST /getNetwork` | ✅ **SAME** | Both get network info |
| `POST /processAction` | ❌ **NOT IN METANET** | ⚠️ **DIFFERENT** | We have this, Metanet doesn't |

## Table 2: Authentication Endpoints Analysis

| ToolBSV Called | Our Implementation | Metanet Desktop | Status | Analysis |
|----------------|-------------------|-----------------|--------|----------|
| `/brc100-auth` | ❌ **MISSING** | ❌ **MISSING** | 🚨 **CRITICAL GAP** | ToolBSV expects this but neither we nor Metanet Desktop have it! |
| `/brc100/auth/challenge` | ✅ **HAS** | ❌ **MISSING** | ⚠️ **DIFFERENT** | We have BRC-100 specific endpoints, Metanet doesn't |
| `/brc100/auth/authenticate` | ✅ **HAS** | ❌ **MISSING** | ⚠️ **DIFFERENT** | We have BRC-100 specific endpoints, Metanet doesn't |
| `/brc100/auth/type42` | ✅ **HAS** | ❌ **MISSING** | ⚠️ **DIFFERENT** | We have BRC-100 specific endpoints, Metanet doesn't |

## Table 3: HTTP Request Handling Analysis

| Aspect | Our Implementation | Metanet Desktop | Analysis |
|--------|-------------------|-----------------|----------|
| **HTTP Server** | Go HTTP server on `localhost:3301` | Tauri HTTP server on `localhost:3321` | ✅ **SAME CONCEPT** |
| **Request Interception** | CEF `HttpRequestInterceptor` | Tauri event system | ✅ **SAME CONCEPT** |
| **Request Routing** | Direct to Go handlers | Frontend TypeScript switch statement | ⚠️ **DIFFERENT IMPLEMENTATION** |
| **CORS Handling** | Go middleware | Tauri automatic | ⚠️ **DIFFERENT IMPLEMENTATION** |
| **Authentication Flow** | Domain whitelist + approval modal | Direct wallet interaction | ⚠️ **DIFFERENT IMPLEMENTATION** |

## Table 4: Missing Endpoints Analysis

### Missing from Our Implementation
| Missing Endpoint | Metanet Desktop Has | Impact |
|------------------|-------------------|---------|
| ❌ **None!** | ✅ **All standard endpoints covered** | ✅ **Good coverage** |

### Missing from Metanet Desktop
| Missing Endpoint | Our Implementation Has | Impact |
|------------------|----------------------|---------|
| ❌ `/brc100/auth/*` endpoints | ✅ **We have these** | ⚠️ **ToolBSV might need these** |
| ❌ `/brc100-auth` | ❌ **We also don't have this** | 🚨 **Critical gap for ToolBSV** |

## Table 5: Our Additional Endpoints (Not in Metanet Desktop)

| Endpoint | Purpose | Status |
|----------|---------|--------|
| `GET /health` | Health check | ✅ **Our addition** |
| `GET /utxo/fetch` | Fetch UTXOs for address | ✅ **Our addition** |
| `POST /transaction/create` | Create unsigned transaction | ✅ **Our addition** |
| `POST /transaction/sign` | Sign transaction | ✅ **Our addition** |
| `POST /transaction/broadcast` | Broadcast transaction to BSV network | ✅ **Our addition** |
| `POST /transaction/send` | Send complete transaction | ✅ **Our addition** |
| `GET /transaction/history` | Get transaction history | ✅ **Our addition** |
| `GET /wallet/status` | Check if unified wallet exists | ✅ **Our addition** |
| `POST /wallet/create` | Create new unified wallet | ✅ **Our addition** |
| `POST /wallet/load` | Load existing unified wallet | ✅ **Our addition** |
| `GET /wallet/info` | Get complete wallet information | ✅ **Our addition** |
| `POST /wallet/markBackedUp` | Mark wallet as backed up | ✅ **Our addition** |
| `GET /wallet/addresses` | Get all addresses | ✅ **Our addition** |
| `POST /wallet/address/generate` | Generate new address | ✅ **Our addition** |
| `GET /wallet/address/current` | Get current address | ✅ **Our addition** |
| `GET /wallet/balance` | Get total balance | ✅ **Our addition** |
| `GET /brc100/status` | BRC-100 service status | ✅ **Our addition** |
| `POST /brc100/identity/generate` | Generate identity certificate | ✅ **Our addition** |
| `POST /brc100/identity/validate` | Validate identity certificate | ✅ **Our addition** |
| `POST /brc100/identity/selective-disclosure` | Create selective disclosure | ✅ **Our addition** |
| `POST /brc100/session/create` | Create authentication session | ✅ **Our addition** |
| `POST /brc100/session/validate` | Validate session | ✅ **Our addition** |
| `POST /brc100/session/revoke` | Revoke session | ✅ **Our addition** |
| `POST /brc100/beef/create` | Create BRC-100 BEEF transaction | ✅ **Our addition** |
| `POST /brc100/beef/verify` | Verify BRC-100 BEEF transaction | ✅ **Our addition** |
| `POST /brc100/beef/broadcast` | Convert and broadcast BEEF | ✅ **Our addition** |
| `POST /brc100/spv/verify` | Verify identity with SPV | ✅ **Our addition** |
| `POST /brc100/spv/proof` | Create SPV identity proof | ✅ **Our addition** |
| `WS /brc100/ws` | WebSocket for real-time BRC-100 communication | ✅ **Our addition** |
| `WS /socket.io/` | Babbage-compatible WebSocket | ✅ **Our addition** |
| `GET /api/brc-100/aliases` | Get wallet aliases (Archie) | ✅ **Our addition** |
| `GET /api/brc-100/transactions` | Get BRC-100 transactions | ✅ **Our addition** |
| `POST /.well-known/auth` | Babbage authentication | ✅ **Our addition** |

## Table 6: Domain Whitelist Endpoints (Our Unique Feature)

| Endpoint | Purpose | Status |
|----------|---------|--------|
| `POST /domain/whitelist/add` | Add domain to whitelist | ✅ **Our unique feature** |
| `GET /domain/whitelist/check` | Check if domain is whitelisted | ✅ **Our unique feature** |
| `POST /domain/whitelist/record` | Record request from domain | ✅ **Our unique feature** |
| `GET /domain/whitelist/list` | List all whitelisted domains | ✅ **Our unique feature** |
| `POST /domain/whitelist/remove` | Remove domain from whitelist | ✅ **Our unique feature** |

---

## 🚨 CRITICAL FINDINGS

### 1. Multi-Site Authentication Patterns (8 Sites Tested)
- **Babbage Sites** (peerpay, thryll, coinflip): Use Socket.IO + `/.well-known/auth` with **universal nonce verification failure**
- **Standard BRC-100 Sites** (toolbsv, coolcert): Use direct HTTP endpoints with **port/endpoint mismatches**
- **Metanet-Only Sites** (dropblocks): Expect Metanet Desktop specifically
- **Non-Wallet Sites** (marscast, paymail): Don't require wallet integration

### 2. The `/brc100-auth` Mystery
- **ToolBSV calls**: `/brc100-auth`
- **Neither we nor Metanet Desktop have this endpoint**
- **This suggests**: ToolBSV might be expecting a different wallet implementation

### 3. Universal Nonce Verification Failure
- **All Babbage sites show**: `"Initial response nonce verification failed from peer: [PUBLIC_KEY]"`
- **Our `/.well-known/auth` handler**: Successfully receives and signs nonces
- **Issue**: Client-side verification is failing despite correct signatures

### 4. Port Compatibility Issues
- **Our Implementation**: `localhost:3301`
- **Metanet Desktop**: `localhost:3321`
- **ToolBSV Expected**: `127.0.0.1:5137` (from debug log)
- **CoolCert Expected**: `localhost:3321` (Metanet Desktop port)

### 5. Authentication Flow Categories
- **Metanet Desktop**: Uses direct wallet calls (no BRC-100 auth endpoints)
- **Our Implementation**: Has BRC-100 auth endpoints (`/brc100/auth/*`)
- **ToolBSV**: Expects `/brc100-auth` (hybrid approach?)
- **Babbage Sites**: Use custom `/.well-known/auth` protocol

---

## 🎯 RECOMMENDATIONS

### Priority 1: Fix Universal Nonce Verification Issue
1. **Debug Babbage nonce verification** - All Babbage sites fail with same error despite correct signatures
2. **Investigate signature format** - Client expects different format than what we're providing
3. **Check public key matching** - Ensure client and server public keys are identical

### Priority 2: Add Missing Endpoints
4. **Add `/brc100-auth` endpoint** - ToolBSV expects this but neither we nor Metanet Desktop have it
5. **Add port 3321 compatibility** - CoolCert expects Metanet Desktop port
6. **Consider multi-port support** - Support both 3301 (our port) and 3321 (Metanet port)

### Priority 3: Standard BRC-100 Compatibility
7. **Study ToolBSV's source code** - To understand what `/brc100-auth` should actually do
8. **Map `/brc100-auth` to existing endpoints** - It might be a wrapper around standard endpoints
9. **Test with Metanet Desktop** - Verify what endpoints it actually provides

---

## 📊 COMPATIBILITY MATRIX

| **Site** | **Category** | **API Injection** | **Authentication** | **Endpoints Called** | **Our Status** | **Issues** |
|----------|--------------|-------------------|-------------------|---------------------|----------------|------------|
| **peerpay.babbage.systems** | Babbage | ✅ Success | Socket.IO + `/.well-known/auth` | Socket.IO handshake | ⚠️ Partial | Nonce verification fails |
| **toolbsv.com** | Standard BRC-100 | ✅ Success | HTTP endpoints | `POST /getVersion`, `/brc100-auth` | 🚨 Failed | Missing `/brc100-auth` |
| **thryll.online** | Babbage | ✅ Success | Socket.IO + `/.well-known/auth` | Socket.IO handshake | 🚨 Failed | Nonce verification fails |
| **coinflip.babbage.systems** | Babbage | ✅ Success | Socket.IO + `/.well-known/auth` | Socket.IO handshake | 🚨 Failed | Nonce verification fails |
| **marscast.babbage.systems** | Non-wallet | ✅ Success | None detected | None | ✅ Working | No wallet integration needed |
| **dropblocks.org** | Metanet-only | ✅ Success | Expects Metanet Desktop | None | ✅ Working | Expects different wallet |
| **coolcert.babbage.systems** | Standard BRC-100 | ✅ Success | HTTP endpoints | `POST /acquireCertificate` | 🚨 Failed | Port mismatch (3321 vs 3301) |
| **paymail.us** | Non-wallet | ✅ Success | None detected | None | ✅ Working | No wallet integration needed |

## 📊 SUMMARY

- **API Injection**: ✅ **100% Success** across all sites
- **Standard BRC-100 Endpoints**: ✅ **100% Compatible** with Metanet Desktop
- **Babbage Compatibility**: 🚨 **Universal nonce verification failure** across all Babbage sites
- **ToolBSV Compatibility**: 🚨 **Missing `/brc100-auth` endpoint**
- **Port Compatibility**: ⚠️ **Need multi-port support** (3301, 3321, 5137)
- **Overall Architecture**: ✅ **Sound foundation, but needs critical fixes**
