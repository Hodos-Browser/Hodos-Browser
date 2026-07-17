# x402 + BSV Micropayments — Technical & Ecosystem Deep-Dive

> 📁 Part of the **Dolphin Milk + Edwin Integration** doc set — see `README.md`. Forensic deep-dive companion to the main study docs.
> **Created:** 2026-06-28 by a research workflow (web-cited). **STUDY, not a decision** — options & trade-offs, no winner picked. Claim tags **[FACT]/[VISION]/[INFERRED]/[SPECULATION]/[UNVERIFIED]** preserved.

## Purpose & bottom line

x402 is a real, rapidly institutionalizing HTTP-native payment protocol that lets machines pay machines without accounts, subscriptions, or human friction. Its EVM/stablecoin reference implementation is now under Linux Foundation governance (April 2026) with Mastercard, Visa, Google, AWS, Microsoft, and Stripe as founding members. BSV has its own parallel specification (BRC-105) that is architecturally analogous and technically superior for sub-cent micropayments due to its fee structure — but it is outside the x402 Foundation entirely, bridged only by a single-developer intermediary (x402agency.com). The honest state of play is: EVM x402 is winning the institutional war while BSV x402 is winning the micropayment economics argument, and Hodos must navigate both worlds if it wants to pay AI providers per call and publishers per cite.

---

## 1. The x402 Protocol Itself

### 1.1 Origin and Governance

x402 was launched by Coinbase on **May 6, 2025** as an open-source (MIT) protocol reviving the long-reserved HTTP 402 Payment Required status code. [FACT] The Coinbase Developer Platform (CDP) hosted the first facilitator reference implementation, targeting EVM stablecoins on Base.

On **September 23, 2025**, Coinbase and Cloudflare announced the first **x402 Foundation** as the governing body for the open standard. [FACT] At that stage the founding partners were Coinbase, Cloudflare, Stripe, and early members including Circle, Anthropic, Vercel, Google, and AWS.

On **April 2, 2026**, at MCP Dev Summit North America, the x402 Foundation was formally moved under **Linux Foundation governance**, with a materially expanded member roster. [FACT] Full initial membership: Adyen, Amazon Web Services, American Express, Ampersend.ai, Base, Circle, Cloudflare, Coinbase, Fiserv Merchant Solutions, Google, KakaoPay, Mastercard, Merit Systems, Microsoft, Polygon Labs, PPRO, Shopify, Sierra, Solana Foundation, Stripe, thirdweb, and Visa.

### 1.2 Protocol Flow (Request / Response / Settlement)

The core flow is a 6–12 step HTTP cycle. The definitive version per the GitHub spec and V2 documentation: [FACT]

1. **Client → Server**: standard HTTP request (GET, POST, etc.) to a protected resource.
2. **Server → Client (402)**: `HTTP 402 Payment Required` response with a `PAYMENT-REQUIRED` header containing a base64-encoded JSON object specifying: required amount, asset, network (CAIP-2 identifier), payment address, scheme type (`"exact"`), and expiry.
3. **Client constructs PaymentPayload**: the client (or its agent wallet) reads the requirement object, selects a funded asset, and builds a signed authorization (EIP-3009 `transferWithAuthorization` for USDC on EVM chains; SPL Transfer for Solana; chain-specific equivalents elsewhere).
4. **Client → Server (retry)**: resends the original HTTP request with a `PAYMENT-SIGNATURE` header containing the base64-encoded PaymentPayload.
5. **Server verifies** (locally or via `POST /verify` on the facilitator endpoint): the facilitator checks the signature cryptographic validity, asset/amount/network match, and that the authorization has not been used before.
6. **Server fulfills the request**: delivers the resource body.
7. **Server settles** via `POST /settle` on the facilitator: the facilitator submits the on-chain transfer (or authorized transfer), monitors confirmation, and returns a settlement receipt.
8. **Server returns** `PAYMENT-RESPONSE` header with base64-encoded settlement confirmation in the HTTP response.

**V1 headers** used `X-PAYMENT` and `X-PAYMENT-RESPONSE` (deprecated, X-* pattern). **V2 headers** (December 11, 2025) replaced these with `PAYMENT-REQUIRED`, `PAYMENT-SIGNATURE`, and `PAYMENT-RESPONSE`. [FACT] V2 is backward-compatible with V1.

### 1.3 x402 V2 (December 11, 2025)

V2 was a significant structural update. Key changes: [FACT]

- **Plugin-driven SDK**: developers register chains, assets, and payment schemes as plugins rather than editing SDK internals; lifecycle hooks allow custom logic at payment checkpoints.
- **Wallet-based identity & sessions**: the `@x402/paywall` package supports wallet-controlled session tokens so repeat callers can skip the full on-chain flow (e.g., pre-purchased content), with Sign-In-With-X (CAIP-122 based) forthcoming.
- **Dynamic `payTo` routing**: per-request fund routing to addresses, roles, or callback-based payout logic; enables multi-tenant APIs and marketplace revenue splits.
- **API Discovery extension**: facilitators can crawl x402-enabled endpoints and auto-index pricing/routes/metadata.
- **Multi-chain SDK**: adding new chains requires only a plugin registration, not a core code change.
- **CAIP-2 standardization**: all network and asset identifiers use the Chain Agnostic Improvement Proposal format.

**No BSV support was added in V2.** [FACT]

### 1.4 Supported Networks (as of mid-2026)

Per official x402 documentation: [FACT]

| Category | Networks | Token Type |
|---|---|---|
| EVM | Base, Polygon, Arbitrum, World (+ any `eip155:<chainId>`) | ERC-20 (EIP-3009, Permit2) |
| Solana | SVM via genesis hash | SPL / Token-2022 |
| TON | TVM via workchain | TEP-74 Jetton |
| Algorand | `algorand:<genesisHash>` | ASA |
| Stellar | `stellar:<network>` | SEP-41 (launched March 2026) |
| Aptos | `aptos:<chainId>` | Fungible assets |
| Hedera | `hedera:<network>` | HBAR / HTS |
| Keeta | `keeta:<networkId>` | Native tokens |
| Concordium | `ccd:<genesisHash>` | CCD / PLT |

**BSV is absent from this list.** The Coinbase facilitator specifically handles Base, Polygon, Arbitrum, World, and Solana. The 29+ EVM default assets cover the stablecoin mainstream (USDC, EURC). Notably, **USDT and DAI are excluded** due to EIP-3009 incompatibility — a significant limitation. [FACT]

### 1.5 Transaction Volume and Real Adoption (Honest Account)

- Cumulative transactions on Base: **119 million+** as of March 2026 [FACT]
- Cumulative on Solana: **35 million+** as of March 2026 [FACT]
- Annualized settled volume: **~$600 million** [FACT]
- Daily actual on-chain volume: **~$28,000** [FACT] — a stark contrast to annualized headline figures
- A Chainalysis analysis found that a significant portion of early volume (Q4 2025 spike) was driven by the PING meme coin protocol, not genuine commerce; genuine merchant-to-agent commerce remains nascent [FACT]
- Transaction size distribution: transactions over $1 grew from 49% to 95% of volume from early 2025 to early 2026, while the 10¢–$1 micropayment band collapsed from 46% to 4% — **sub-cent micropayments are not the current x402 use case in practice** [FACT]
- Facilitator latency: verification + settlement adds **500–1,100 ms** per request in the two-phase model [FACT]

---

## 2. The BSV Path: BRC-105, BRC-29, BRC-31, BRC-100, and x402agency.com

### 2.1 Architecture Overview

BSV's micropayment HTTP protocol stack is a layered set of BRC (Bitcoin Request for Comments) standards. From bottom to top:

```
HTTP Transport  ←  BRC-104 (HTTP layer)
Authentication  ←  BRC-103 (mutual auth, supersedes BRC-31/Authrite)
Payment         ←  BRC-105 (HTTP Service Monetization Framework)
                   ↑ builds on BRC-29 (P2PKH payment derivation)
Wallet API      ←  BRC-100 (standard wallet interface)
On-chain data   ←  BRC-18 (OP_FALSE OP_RETURN for receipts/proofs)
Transactions    ←  BRC-62 (BEEF / AtomicBEEF format)
```

### 2.2 BRC-31 / BRC-103: Mutual Authentication (the Session Layer)

**BRC-31 (Authrite)** is the original peer-to-peer mutual authentication protocol. [FACT] It works by:

1. Alice sends an `initialRequest` with her identity public key and a random 256-bit nonce.
2. Bob responds with `initialResponse` containing his identity key, his nonce, and a BRC-43 signature over both nonces using a derived protocol key (`authrite message signature`).
3. Subsequent `general` messages are signed payloads bound to the dual-nonce key, providing replay protection (the key ID `<alice-nonce> <bob-nonce>` is unique per session).
4. Optional certificate exchange via BRC-52 allows selective identity revelation.

**BRC-31 is now the "Authrite-era predecessor."** Modern BSV implementations use **BRC-103** (Peer-to-Peer Mutual Authentication and Certificate Exchange Protocol) which preserves the message structure but updates certificate semantics. [FACT] BRC-31 is referenced in the x402agency.com stack alongside BRC-87.

**Significance for Hodos**: BRC-103/BRC-31 creates a cryptographically authenticated channel between Hodos (client) and an AI provider or publisher server — before any money changes hands. This is structurally superior to the EVM x402 model which has no mandatory session authentication; it allows payment to be provably tied to a specific caller identity.

### 2.3 BRC-29: P2PKH Payment Derivation (the Cryptographic Payment Primitive)

BRC-29 (Simple Authenticated BSV P2PKH Payment Protocol) defines how payments are constructed so each output script is uniquely derived and non-linkable. [FACT]

The derivation uses BRC-42 key derivation:
```
Key ID = "2-3241645161d8-<derivationPrefix> <derivationSuffix>"
```

- `derivationPrefix`: payment-wide random nonce (same for all outputs in one payment)
- `derivationSuffix`: output-unique random value (ensures each UTXO receives a unique, non-linkable key)

The derived public key is converted to a P2PKH locking script (BRC-16). The payment is packaged as an **AtomicBEEF** transaction (BRC-62), which bundles the transaction with its full SPV proof chain.

Recipients validate that P2PKH output scripts match the derived public keys, then call `wallet.internalizeAction()` with the derivation metadata to credit their wallet. This is critical: double-spend protection and validation happen at the wallet layer, not at the application layer.

### 2.4 BRC-105: HTTP Service Monetization Framework (BSV's x402 Equivalent)

BRC-105 is the primary BSV HTTP payment standard. [FACT] Its 402 response/retry cycle maps directly onto the EVM x402 flow but uses BSV-native primitives:

**Server → Client (HTTP 402 response headers):**

| Header | Content |
|---|---|
| `x-bsv-payment-version` | `1.0` |
| `x-bsv-payment-satoshis-required` | Integer; exact satoshi amount |
| `x-bsv-payment-derivation-prefix` | Random nonce; single-use |
| `x-bsv-auth-identity-key` | 33-byte compressed server identity public key (hex) |

**Client → Server (payment submission header):**

| Header | Content |
|---|---|
| `x-bsv-payment` | JSON: `{derivationPrefix, derivationSuffix, transaction}` where `transaction` = AtomicBEEF base64-encoded |

**Six-step flow:**
1. Client establishes BRC-103 authenticated session.
2. Server calculates price for the request.
3. If unpaid: 402 with payment headers.
4. Client calls `wallet.createAction()` (BRC-100) constructing a BSV transaction paying to the derived P2PKH output.
5. Client resubmits HTTP request with `x-bsv-payment` header.
6. Server calls `wallet.internalizeAction()` to validate and accept payment; processes and returns the resource.

**Security mechanisms:**
- The `derivationPrefix` is single-use (stored in wallet database); any replay attempt is rejected. [FACT]
- Mutual BRC-103 authentication prevents MITM payment hijacking. [FACT]
- Underpayment: server checks `x-bsv-payment-satoshis-required` against actual transaction output. [FACT]
- Double-spend: `internalizeAction` performs SPV validation against the UTXO set. [FACT]

**Extensions:**
- **BRC-118**: multipart body transport for large payment payloads (alternative to header transport, no deprecation)
- **BRC-120**: conformance to the "frozen external x402 specification" — the formal bridge to Coinbase-format x402
- **BRC-121**: a smaller, BSV-specific 402 profile (simplified for resource-constrained contexts)

[INFERRED]: BRC-120 and BRC-121 are the architectural answer to BSV-to-EVM x402 interoperability, but they appear to be largely defined on paper rather than widely implemented. The CoinGeek/x402agency reporting does not specifically describe a deployed BRC-120 gateway.

### 2.5 BRC-100: The Wallet Interface

BRC-100 defines the standard wallet-to-application interface that all BSV applications (including Edwin) use to request payment operations. [FACT] It is vendor-neutral and implementation-agnostic. Key methods relevant to micropayments:

- **`createAction(params)`**: builds a BSV transaction; supports `noSend`, `sendWith`, `noSendChange` flags for batched/chained payment workflows; `returnTXIDOnly` flag reduces response size for high-frequency scenarios.
- **`internalizeAction(params)`**: accepts incoming BEEF transactions, validates SPV proofs, credits wallet; handles "wallet payment" (BRC-29 derivation) and "basket insertion" (token tracking) protocols.
- **`getPublicKey(params)`**: derives identity or protocol-specific public keys.
- **`createSignature` / `verifySignature`**: cryptographic signing for authentication.
- **`encrypt` / `decrypt`**: AES-256-GCM for payload confidentiality.

For Hodos's Edwin sidecar: BRC-100 is the interface between Edwin's payment logic and the user's BSV wallet subprocess. Edwin calls `createAction()` to build payment transactions; the wallet subprocess holds keys and signs. The wallet is never exposed to Edwin's Node.js environment.

### 2.6 x402agency.com: The BSV–EVM Bridge

x402agency.com (built by developer **John Calhoun**) is a functioning BSV-native x402 marketplace. [FACT] Key architectural facts:

**What it does:**
- Runs AI agent services (image, video, audio, research) accepting BSV micropayments with no API keys or subscriptions. [FACT]
- Acts as the primary real-world implementation of BRC-105 / x402-on-BSV.
- Hosts the **dolphinmilk** project: an autonomous AI agent that pays for its own LLM inference via BSV micropayments, currently running in production. [FACT]

**Infrastructure:**
- The entire BSV overlay stack was ported from TypeScript to Rust and compiled to WebAssembly. [FACT]
- Runs on **Cloudflare Workers** across 300+ global data centers. [FACT]
- Operating cost: ~**$5/month** (vs. $50–$200/month for traditional servers). [FACT]
- "Byte-for-byte compatible" with the TypeScript reference implementation, verified via 43 differential parity tests. [FACT]

**Permissionless marketplace model:**
- Service providers publish their offerings directly to the BSV blockchain; the marketplace catalog reads service listings from the chain, eliminating approval queues and central databases. [FACT]
- Supports SHIP/SLAP routing, GASP synchronization, BRC-31/BRC-87 token standards, UHRP file hosting, and Agent Registry service discovery. [FACT]

**Auto-refunds:**
- End-to-end refunds are implemented: if a paid API call fails, the server automatically returns the BSV payment. [FACT]

**Roadmap:**
- Payment channels for streaming and high-volume usage are next on the roadmap but not yet deployed. [VISION/ROADMAP]

**The critical gap**: x402agency.com is a **single-developer** project. It is not affiliated with the x402 Foundation; it implements a BSV-specific protocol that is *analogous to* but not interoperable with the EVM x402 spec out of the box. An AI provider implementing standard Coinbase x402 will not accept BSV payments from x402agency without additional bridging logic. [FACT/INFERRED]

---

## 3. The Edwin Envelope + BRC-18 On-Chain Proof Model

### 3.1 BRC-18: Pay to False Return (the On-Chain Data Primitive)

BRC-18 defines the `OP_FALSE OP_RETURN` script template for storing arbitrary data permanently in BSV transactions. [FACT]

Structure:
```
OP_FALSE OP_RETURN <data1> <data2> <data3> ...
```

Key properties:
- Outputs are non-spendable (OP_RETURN) and unspendable (OP_FALSE prefix ensures miner compliance under all script evaluation modes).
- Data chunks are arbitrary bytes pushed as PUSHDATA elements; multiple chunks can encode structured records.
- The **timestamp** and **Merkle proof** linking to the containing block remain as cryptographic proof of existence even if miners subsequently prune the underlying data.
- Established implementations: RUN protocol, BOB (Bitcoin OP_RETURN Bytecode). [FACT]
- Miners may prune the data payload from full nodes, but the BUMP (BRC-74) Merkle proof path remains provable. [FACT]

**Cost**: an OP_FALSE OP_RETURN output adds ~50–100 bytes to a transaction; at 0.1 sat/byte, this costs 5–10 additional satoshis (~$0.000001–$0.000002 USD at current prices) per proof record. Negligible.

### 3.2 Edwin Envelope + BRC-18: The Design Model for Hodos (INFERRED/VISION)

This section describes the logical integration pattern for Hodos, not a shipped implementation. It is assembled from: the BRC-105/BRC-18 specifications, the x402agency "provable-think" pattern (mentioned in CoinGeek reporting as "tamper-evident receipts"), and the general BSV overlay stack design.

**The concept: a dual-output payment transaction**

When Edwin calls a paid AI service (Claude API, x402agency AI agent, or a publisher endpoint), the BSV payment transaction Hodos constructs would contain:

- **Output 0** (P2PKH, spendable): Payment to the provider's derived public key per BRC-29 derivation. This is the actual payment.
- **Output 1** (OP_FALSE OP_RETURN, non-spendable): Audit record encoding: call timestamp, Edwin session ID, provider identity key (BRC-103 identity), request hash (SHA-256 of the prompt/request), response hash (SHA-256 of the response), satoshi amount, derivation prefix, and any citation metadata (for publisher payments).

This creates an immutable on-chain receipt for every paid AI call. The receipt:
- Proves the call happened at a specific block timestamp.
- Binds the payment to the specific request and response by hash.
- Cannot be repudiated by provider or user.
- Is queryable via WhatsOnChain or any BSV data layer (1Sat/OP_RETURN indexers).

**The "Edwin envelope" framing**: Edwin (as a Node.js gateway) would format the BRC-18 payload as a structured JSON or CBOR blob identifying: `{"type":"ai-call","provider":"<pubkey>","model":"<model-id>","req_hash":"<sha256>","resp_hash":"<sha256>","sats":<n>,"ts":<unix>}`. The BRC-100 wallet's `createAction()` builds the transaction with both outputs in a single atomic operation.

**Publisher-cite model**: When Edwin cites a source URL during an AI response, Hodos would trigger a second micropayment to the publisher's BRC-105 endpoint (or an x402 endpoint on EVM chains via x402agency intermediary). The OP_RETURN output would record the URL hash and the citation position (e.g., footnote 3 in response N).

**Critical caveat**: This BRC-18 on-chain proof architecture is [INFERRED] as the logical design from available BSV standards. The user's reference to "BRC-18 on-chain proof model" for Edwin may refer to a specific implementation design in Jake's Edwin codebase that is not publicly documented. BRC-18 per the official BRC specification is a script template standard, not an Edwin-specific protocol. Confirm with Jake whether a specific BRC-18 envelope schema has been defined.

---

## 4. Concrete Numbers

### 4.1 Satoshi Amounts for AI Micropayments

BSV price (as of mid-2026): **~$12–$16 USD** with predicted range $7.98–$15.57 for full year 2026. [FACT] For calculations below, $12 USD/BSV is used as a conservative reference.

At $12/BSV = $0.00000012 per satoshi.

| Use case | Typical satoshi amount | USD equivalent (@ $12/BSV) |
|---|---|---|
| 1 sat (minimum UTXO) | 1 sat | $0.000000012 |
| Nano-payment marker | 100 sats | $0.0000012 |
| Lightweight API call (data query) | 1,000–10,000 sats | $0.00012–$0.0012 |
| Medium AI call (e.g., short inference) | 10,000–100,000 sats | $0.0012–$0.012 |
| Substantive AI call (e.g., 1K tokens Claude) | ~830,000 sats | ~$0.01 (Claude Haiku 4.5 ≈ $0.008/1K tokens) |
| Publisher cite micro-fee | 1,000–50,000 sats | $0.00012–$0.006 |
| Heavy AI call (GPT-4-class, 10K tokens) | 8M–12M sats | ~$0.10–$0.14 |

**Note on Claude API costs (2026)**: Anthropic's current pricing for Claude Haiku 4.5 is approximately $0.008 per 1,000 input tokens and $0.004 per 1,000 output tokens via API. [FACT - from search results]. At $12/BSV, $0.008 = ~66,667 sats. For a 1,000-token exchange, expect 50,000–200,000 sats depending on model tier. BSV transaction fee to carry this payment: ~25 sats (negligible overhead, <0.1%).

### 4.2 BSV Transaction Fees

Standard miner fee: **0.1–1 satoshi per byte** (TAAL recently reduced to 0.5 sat/byte). [FACT]

A typical BRC-105 micropayment transaction:
- 1 input + 1 P2PKH output + 1 OP_RETURN output + change output ≈ **350–450 bytes**
- Fee at 0.1 sat/byte: **35–45 satoshis** ≈ $0.000004–$0.000005 USD

This fee overhead is economically irrelevant for any payment above ~1,000 sats ($0.00012). BSV's fee model enables sub-cent payments with sub-cent overhead — the core advantage over EVM chains where gas fees were historically $0.001–$5+ per transaction even on L2.

### 4.3 Settlement Latency and Finality

| Mechanism | Latency | Finality | Notes |
|---|---|---|---|
| BSV 0-conf (unconfirmed mempool) | Immediate (~100ms propagation) | None (reversible) | Acceptable for low-value calls per BSV community norms; double-spend risk is low but nonzero |
| BSV 1-confirmation | ~10 min | Probabilistic (~90% after 1 block) | Standard for medium-value transfers |
| BSV 6-confirmation | ~60 min | Nakamoto consensus finality | 1-hour true finality per chainspect.app data [FACT] |
| EVM x402 (Base L2) | 500–1,100ms (verification + settlement) | ~200ms on Base (L2 confirmation) | Per PANews analysis [FACT] |
| BSV BRC-105 (with wallet validation) | ~200–500ms (internalizeAction) | 0-conf acceptance | Server trusts 0-conf for micropayments; formal finality waits block |

**Key insight**: For the Edwin per-call model, 0-conf acceptance is the practical approach for micropayments under ~10,000 sats. This is standard BSV merchant practice and is economically rational: the cost of a double-spend attempt (miner coordination) far exceeds the value of a sub-cent API call. [INFERRED from BSV micropayment literature]

**Payment channels** (for streaming high-frequency calls, analogous to Lightning): on the x402agency roadmap but **not yet deployed**. [VISION/ROADMAP] Without payment channels, each Edwin call triggers a full transaction; for AI sessions with dozens of calls per minute, this could produce 1–2 on-chain transactions per second per active user — well within BSV's current throughput but generating real-time chain activity.

### 4.4 Throughput

- BSV real-time TPS as of mid-2026: **~2.75–87 TPS** (live data from chainspect.app; substantial variance) [FACT]
- BSV demonstrated max TPS (lab, 100-block window): **1,975 TPS** [FACT]
- BSV Teranode lab test (October 2025): **1.2 million TPS** on AWS infrastructure [FACT]
- For Hodos scale (say, 100,000 users each making 10 AI calls/day): ~1.16 TPS — comfortably within current live throughput, not a scalability constraint.

---

## 5. Fiat-Denominated Display vs. BSV Settlement

### 5.1 The Volatility Problem

BSV's 2026 price range is projected at $7.98–$15.57, a factor-of-2 swing. [FACT] At the extremes:

- 100,000 sats = **$0.008** (at $8/BSV) or **$0.016** (at $16/BSV)
- A service quoted at "100,000 sats" is effectively a variable-dollar price from the user's perspective.
- For AI call pricing (provider charges in USD), the Hodos client must convert at real-time market rates.

### 5.2 Display Strategies

Three approaches, each with trade-offs:

**A. Display in satoshis only (BSV-native)**
- Shows "100,000 sats" in the UI.
- Pro: stable unit; no live price feed needed; user never sees fiat fluctuations.
- Con: non-technical users have no intuitive grasp of satoshi values; "100,000" sounds large, "$0.001" sounds tiny.
- Verdict: Appropriate for power users, bad for casual users. Conflicts with Hodos north star.

**B. Display in USD equivalent (live conversion)**
- Shows "$0.001" with a live BSV/USD price oracle (e.g., CoinGecko, WhatsOnChain price API).
- Pro: immediately intelligible.
- Con: displayed price fluctuates as BSV price moves; a session started at $0.001/call might be $0.0009 by the end if BSV price rises. Requires live price feed dependency.
- Implementation: Edwin sidecar fetches BSV/USD price every ~60 seconds; converts satoshi amounts before display.
- Verdict: Best UX for casual users; requires price oracle integration.

**C. USD-denominated pricing, BSV settlement at time-of-payment**
- Provider specifies price in USD (e.g., "$0.001/call"); client converts to sats at payment time using live rate.
- Pro: price stability from provider's perspective; user sees stable fiat prices.
- Con: if BSV price drops between user's deposit and payment, the satoshi amount needed increases, potentially draining wallet faster than expected. Reverse if BSV rises.
- This is the model used by Coinbase x402 with stablecoins (USDC = fixed USD value, no volatility problem). BSV lacks a native stablecoin with comparable ecosystem depth. [FACT]
- Verdict: Closest to the EVM x402 UX; feasible but requires robust price oracle and user education about BSV price exposure.

**D. Lock-in at session start**
- Hodos converts a fiat "budget" (e.g., $1.00) to sats at session start; displays remaining budget in both units.
- Pro: user commits a known dollar amount; no mid-session surprise.
- Con: any BSV price move during the session creates a small discrepancy between displayed USD budget and actual purchasing power remaining.
- Verdict: Good pragmatic middle ground for the near term.

### 5.3 Publisher Cite Payments: Pricing Asymmetry

EVM x402 publisher payments are denominated in USDC (stablecoin), eliminating volatility. BSV-based publisher payments would be in sats. Publishers accepting BSV would need to either:

1. Accept BSV volatility as the price of permissionless access (fine for BSV-native publishers).
2. Require the Hodos client to convert via the x402agency intermediary, which could potentially settle in stablecoins on the backend.

This asymmetry is an honest friction point: most mainstream publishers (news sites, academic papers) will implement EVM x402 (USDC) before BSV/BRC-105, if they implement x402 at all.

---

## 6. Honest Gaps

### 6.1 BSV is Not in the x402 Standard

The Linux Foundation x402 specification does not include BSV. [FACT] BSV's UTXO model, Nakamoto consensus finality, and satoshi denomination do not map to the EIP-3009/Permit2 authorization patterns required by EVM x402. The `x402b` proposal (from the Blockonomics analysis) adapts the protocol by using SSE for async confirmation notification and price-lock windows, but this adaptation is a community proposal, not an accepted x402 Foundation standard. [UNVERIFIED — whether any BSV x402 pull request has been submitted to the Linux Foundation repository]

### 6.2 x402agency.com Intermediary Risk

x402agency is a **single developer** (John Calhoun) running an open-source but essentially solo-maintained project. [FACT] This creates:

- **Dependency risk**: if Calhoun is unavailable, the primary BSV-x402 bridge goes dark.
- **Trust assumption**: Hodos would route BSV payments through an intermediary for any non-BSV-native AI provider, adding a settlement hop and a trust dependency.
- **No SLA**: no documented uptime guarantee, no published incident history.
- **Interoperability unproven**: the claim that BRC-120 achieves "conformance to the frozen external x402 specification" has not been independently verified; it is not listed in the Linux Foundation x402 ecosystem. [UNVERIFIED]

### 6.3 EVM x402 vs. BSV x402 Fragmentation

A content publisher implementing EVM x402 (the mainstream path, given Linux Foundation backing) will expect USDC payment authorization in the `PAYMENT-SIGNATURE` header format. A Hodos browser sending BRC-105 headers (`x-bsv-payment`) is speaking a different dialect. There is currently no native gateway in the x402 Foundation stack that translates between them. Hodos would need to:

- Route through x402agency as a payment bridge (centralization, latency, fee overhead).
- Implement the EVM x402 client path with a user-held USDC balance (requires a different wallet, not BSV).
- Wait for a BSV plugin to be contributed to the Linux Foundation x402 SDK (no roadmap signal). [FACT]

### 6.4 BSV Centralization and Miner Risk

BSV has a **Nakamoto Coefficient of 1** and only **10 miners** as of mid-2026. [FACT] This is severe centralization: a single mining entity controls the chain. For payment-receipt finality, this means:

- A rogue miner could, in principle, reorganize recent blocks.
- The immutability of BRC-18 on-chain proof receipts depends on the honesty of the mining majority.
- For micropayments (low value per transaction), this is a low practical risk — the cost to reorganize exceeds the value of any individual call receipt. But it is a systemic trust assumption.

### 6.5 Wallet Funding UX: The Onboarding Problem

Before any micropayment flows, the user needs BSV in their wallet. This requires:

1. Acquiring BSV (CEX: Coinbase, Bitget, Binance; or P2P). [FACT — listed on major exchanges]
2. Withdrawing BSV to the Hodos wallet (Rust subprocess).
3. Understanding that wallet needs sufficient balance for a session budget.

This is the critical UX barrier for casual users. EVM x402 has the same problem with USDC on Base, but the Coinbase ecosystem (Coinbase Wallet, Coinbase Card, on-ramp APIs) provides a smoother on-ramp. BSV's on-ramp ecosystem is thinner: HandCash supports fiat-to-BSV flows, RockWallet supports in-app purchase, but BSV lacks a Coinbase-grade managed wallet product with seamless bank linking. [FACT from wallet survey results]

Potential mitigation paths:
- Prepaid session voucher (user buys a USD-denominated session credit via card; Hodos holds BSV backstop).
- MoonPay or Transak BSV on-ramp embedded in the Hodos onboarding flow. [INFERRED — both support BSV but integration is non-trivial]
- The Yours Wallet BRC-100 rebuild includes an in-wallet on-ramp component. [UNVERIFIED — mentioned in context of roadmap, not confirmed]

### 6.6 Payment Channel Gap (Streaming Payments)

For AI sessions involving many sequential calls (e.g., an extended conversation with Edwin), on-chain transaction-per-call creates:

- ~25–45 sat overhead per call (negligible in dollar terms).
- Real-time chain writes that, at scale, could make user sessions visible as blockchain activity (minor privacy concern, addressable with privacy-forward construction).
- Latency of `createAction()` + `internalizeAction()` for each call (estimated 100–300ms overhead per call in a BRC-100 wallet). [INFERRED from BRC-100 spec complexity]

Payment channels (bidirectional off-chain payment channels with on-chain settlement) would solve this elegantly but are explicitly listed as future roadmap on x402agency. Until deployed, high-frequency call sessions must use on-chain transactions or a batching/credit model.

### 6.7 Micropayment Market Reality

The overall x402 micropayment market ($28K/day actual volume, possible wash trading) indicates that genuine per-call economics have not yet emerged. [FACT] The use case is proven in theory (dolphinmilk runs in production [FACT]) but not yet at consumer scale. Hodos would be an early mover in a genuinely emerging market, which is both an opportunity and a risk.

---

## 7. What This Means for Hodos (Options, Not a Pick)

### Option A: BSV-First (BRC-105 Native), x402agency Bridge for EVM Providers

Hodos implements BRC-105 as its primary payment rail. Edwin uses BRC-100 (`createAction`) to build BSV transactions for every paid call. BRC-18 OP_RETURN outputs provide on-chain audit receipts. For AI providers that only accept EVM x402, Hodos routes through x402agency as a payment bridge (user pays BSV; x402agency settles USDC to the provider).

- Pro: maximum BSV-native alignment; sub-cent fees; on-chain proof; no stablecoin exposure; consistent with Hodos/Jake's BSV identity.
- Con: single-developer intermediary dependency; provider support limited; wallet funding UX friction; BSV price volatility on all payments.

### Option B: Dual Rail (BSV for BSV-native providers, EVM x402 for mainstream providers)

Hodos implements both BRC-105 and EVM x402 client paths. The user holds BSV and a small USDC-on-Base balance (or Hodos manages a backstop). Edwin selects the rail based on provider capability (BSV preferred; EVM x402 fallback).

- Pro: broad provider compatibility; can pay mainstream publishers via EVM x402; not dependent on x402agency for every call.
- Con: two wallet balances to manage (compounded UX friction); Hodos must maintain an EVM key and USDC balance alongside BSV; privacy model differs between rails.

### Option C: EVM x402 Primary (USDC Stablecoin), BSV for On-Chain Proofs Only

Hodos implements the Coinbase/Linux Foundation x402 client in full. Payments are settled in USDC on Base (no BSV volatility). BSV is used only as an audit ledger: BRC-18 OP_RETURN receipts are written after EVM settlement, creating a BSV-anchored audit trail without BSV being the payment medium.

- Pro: maximum provider compatibility; stablecoin eliminates volatility; Linux Foundation ecosystem alignment.
- Con: abandons BSV's fee advantage; requires USDC/Base wallet UX; dilutes BSV-native positioning.

### Option D: Session Budget Model (Fiat On-Ramp → BSV Auto-Provision)

Users fund Hodos with a fiat amount (e.g., $5); Hodos auto-purchases BSV via an embedded on-ramp and holds it in the wallet. All payments are in BSV via BRC-105. Display is in fiat equivalent (live conversion). Budget depletes as calls are made; user tops up with card.

- Pro: hides crypto friction from the casual user; maintains BSV-native rail; addresses the "buy crypto first" barrier.
- Con: on-ramp API integration complexity; regulatory considerations (acting as a fiat-to-crypto exchange interface); BSV price moves between on-ramp and spend.

---

## 8. Open Questions

1. **Is BRC-120 a real deployed implementation or a draft spec?** The GitHub BRC repository lists it but no public deployed implementation bridging BRC-105 to EVM x402 has been independently verified. What is the actual state of BRC-120/BRC-121?

2. **What does Jake's Edwin on-chain proof model actually define?** The user's reference to "BRC-18 on-chain proof model" and "Edwin envelope" is not publicly documented. Is there a specific OP_RETURN schema or BRC extension proposed within Edwin? If not, who designs this?

3. **Does x402agency.com have an SLA, redundancy, or a succession plan?** As the sole BSV-EVM bridge, its reliability and longevity are critical to Option A/B. Has Calhoun indicated open-sourcing the bridge gateway so Hodos could run its own instance?

4. **What is the minimum BSV on-ramp amount compatible with a user's session budget?** At ~50,000–500,000 sats per AI call, a $1–$5 prepaid session budget is feasible at current BSV prices. But what is the minimum wallet balance that Hodos considers viable for a first-time user?

5. **Does the Hodos wallet subprocess expose a BRC-100 compliant interface?** The wallet is a Rust subprocess; does it implement the BRC-100 `createAction`/`internalizeAction` API surface, or does Edwin need to speak a different interface?

6. **Will mainstream publishers adopt x402 at all?** The $28K/day current volume with possible wash trading suggests real publisher adoption has not arrived. What is Hodos's fallback if publishers don't implement x402 within the 1–2 year horizon?

7. **How does Hodos handle the 0-conf double-spend risk for per-call payments?** For very low-value calls, 0-conf acceptance by providers is rational. Does the Hodos/BRC-105 design formally specify 0-conf acceptance thresholds, or leave this to each provider?

8. **Payment channel timeline**: When does x402agency expect to ship payment channels? This changes the per-call session architecture significantly (moving from 1 on-chain tx/call to a single channel-open + channel-close per session).

9. **Privacy of on-chain receipts**: BRC-18 receipts are publicly visible on BSV. Does the audit trail design include encryption of the OP_RETURN payload? A public record of every AI call made by a Hodos user is in tension with the privacy-conscious user base.

10. **BSV exchange availability**: Is BSV listed and liquid enough on major exchanges that a non-technical user in the US or EU could realistically purchase BSV within the Hodos onboarding flow? Coinbase has delisted BSV in some regions historically; current availability should be confirmed.

---

## Sources

- [Coinbase x402 Introduction (May 2025)](https://www.coinbase.com/developer-platform/discover/launches/x402)
- [Coinbase CDP x402 Documentation](https://docs.cdp.coinbase.com/x402/welcome)
- [x402 GitHub Repository (coinbase/x402)](https://github.com/coinbase/x402)
- [x402 Foundation (x402-foundation/x402)](https://github.com/x402-foundation/x402)
- [Cloudflare x402 Foundation Blog (Sep 2025)](https://blog.cloudflare.com/x402/)
- [x402 V2 Launch Announcement](https://www.x402.org/writing/x402-v2-launch)
- [Linux Foundation x402 Announcement (Apr 2, 2026)](https://www.linuxfoundation.org/press/linux-foundation-is-launching-the-x402-foundation-and-welcoming-the-contribution-of-the-x402-protocol)
- [CoinDesk: x402 Joins Linux Foundation](https://www.coindesk.com/tech/2026/04/02/coinbase-s-ai-payments-system-joins-linux-foundation-gathers-support-from-google-stripe-aws-and-others)
- [x402 Network & Token Support (Official Docs)](https://docs.x402.org/core-concepts/network-and-token-support)
- [BRC-105 HTTP Service Monetization Framework (official)](https://bsv.brc.dev/payments/0105)
- [BRC-105 on BSV Hub](https://hub.bsvblockchain.org/brc/payments/0105)
- [BRC-29 Simple Authenticated BSV P2PKH Payment Protocol](https://bsv.brc.dev/payments/0029)
- [BRC-31 Authrite Mutual Authentication](https://bsv.brc.dev/peer-to-peer/0031)
- [BRC-100 Wallet Interface Standard](https://bsv.brc.dev/wallet/0100)
- [BRC-18 Pay to False Return](https://bsv.brc.dev/scripts/0018)
- [BSV BRC GitHub Repository](https://github.com/bitcoin-sv/BRCs)
- [BRC README Index](https://bsv.brc.dev/)
- [CoinGeek: BSV x402 Goes Permissionless with Cloudflare](https://coingeek.com/bsv-x402-marketplace-goes-permissionless-with-5-mo-cloudflare/)
- [CoinGeek: Yours Wallet BRC-100 Rebuild](https://coingeek.com/yours-wallet-gets-a-rebuild-for-brc100-and-ai-agents/)
- [CoinGeek: bOpen Infrastructure](https://coingeek.com/bopen-building-the-infrastructure-for-web3s-agentic-future/)
- [x402agency.com](https://x402agency.com/)
- [Chainspect BSV Performance Data](https://chainspect.app/chain/bsv)
- [Chainalysis: Inside x402 100M Transactions](https://www.chainalysis.com/blog/x402-agentic-payments-adoption/)
- [CoinDesk: x402 Micropayment Demand Reality Check (Mar 2026)](https://www.coindesk.com/markets/2026/03/11/coinbase-backed-ai-payments-protocol-wants-to-fix-micropayment-but-demand-is-just-not-there-yet)
- [PANews: x402 Hidden Problems](https://www.panewslab.com/en/articles/87f007ff-f2c6-4b41-919d-24e26c295912)
- [Blockonomics: Bitcoin Missing from Agentic Commerce](https://insights.blockonomics.co/agenticcommerce-bitcoin/)
- [BSV Transaction Fee Documentation](https://docs.bsvblockchain.org/guides/sdks/ts/examples/example_fee_modeling)
- [AWS: BSV 1M TPS with Teranode](https://aws.amazon.com/blogs/web3/how-the-bsv-association-built-a-million-tps-blockchain-node-using-aws/)
- [xpay Publisher Monetization via x402](https://www.xpay.sh/blog/article/how-publishers-monetize-ai-traffic/)
- [crypto.news: x402 Joins Linux Foundation](https://crypto.news/x402-joins-linux-foundation-with-backing-from-google-stripe-aws/)
- [Wu Blockchain on x402 V2](https://x.com/WuBlockchain/status/1999224448201998638)
