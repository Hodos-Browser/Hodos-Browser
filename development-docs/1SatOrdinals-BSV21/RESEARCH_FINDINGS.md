# 1Sat Ordinals — Research Findings

**Date:** 2026-05-05. Source: research agent, web research against live sources.

Faithful capture of the research report so it survives a context reset. Light editorial framing only.

---

## Section 1 — Protocol mechanics

**1.1 Inscription envelope.** A 1Sat Ordinal is an output of *exactly* 1 satoshi whose locking script contains an inscription envelope of the form:

```
<P2PKH locking script> OP_FALSE OP_IF "ord" <field1> <value1> ... OP_0 <content> OP_ENDIF
```

Hex of the marker is `6f7264` ("ord"). Fields and values must appear as pairs; each must be a single `PUSH_DATA` or `OP_1`–`OP_16`. Content is everything after `OP_0`. Two crucial differences from BTC ordinals: (a) BSV puts the envelope in **outputs**, not Taproot inputs, and (b) BSV has **no 520-byte push limit**, so content is not concatenated across pushes. Only the *first* valid envelope in a transaction is treated as a real ordinal — subsequent ones are ignored. The envelope can sit before, after, or be separated from the locking script by `OP_CODESEPARATOR`. ([protocol spec](https://docs.1satordinals.com/), [BitcoinSchema/1sat-ordinals README](https://github.com/BitcoinSchema/1sat-ordinals/blob/master/README.md))

**1.2 Transfers.** A transfer is a normal P2PKH spend of the 1-sat UTXO to a new owner's address. There is **no on-chain "transfer" opcode** — provenance is purely positional: "the nth satoshi input is transferred to the nth satoshi output." Indexers reconstruct ownership by walking that chain back to the original `origin` outpoint. So to indexers, a transfer is just a normal tx; the wallet doesn't have to talk to anyone special for a transfer to be "valid" — it just has to spend the 1-sat UTXO correctly.

**1.3 BSV20 vs BSV21.** Both are JSON token protocols inscribed inside the 1Sat envelope (mime type `application/bsv-20`).
- **BSV20 v1**: ticker-based ("first-is-first"), `deploy` → `mint` → `transfer`, with `tick`, `max`, `lim` fields. Defaults to 18 decimals.
- **BSV21**: tickerless. A `deploy+mint` operation creates the entire supply in one tx; tokens are addressed by `id = "<txid>_<vout>"` of that mint output. Clean DAG, parallelizable. Default 0 decimals. Required fields: `p:"bsv-20"`, `op:"deploy+mint"`, `amt`. Transfer inscriptions use `op:"transfer"`, `id`, `amt`. **BSV21 is now the dominant standard** because of the cleaner tracking model. ([BSV-21 docs](https://docs.1satordinals.com/fungible-tokens/bsv-21))

**1.4 Purchases — Ordinal Lock vs PSBT.** Two patterns coexist:

- **PSBT / partial-signature listings** (`SIGHASH_SINGLE | SIGHASH_ANYONECANPAY | FORKID`). Seller signs a tx with 1 input (their ordinal) and 1 output (their payment address with the asking price). Buyer adds dummy inputs/outputs (passthroughs that prevent invalid 0-value structure) plus their own funding input, signs all *their* inputs with `SIGHASH_ALL`, and broadcasts. ([partially-signed-transactions.md](https://github.com/BitcoinSchema/1sat-ordinals/blob/master/partially-signed-transactions.md))
- **Ordinal Lock script** — a covenant-style lock (Bitcoin Script using `OP_PUSH_TX`-style introspection) that *requires* the spending tx to pay the seller exactly the listing price to a specific address. Cancellable by the seller's signature. **This is what `1sat.market` actually uses today** — it makes listings part of the on-chain order book rather than off-chain PSBTs. ([js-1sat-ord docs](https://js.1satordinals.com/))

**What this means for Hodos.** Conceptually the work is small for transfers (existing `createAction` builder spends a UTXO to a new P2PKH — that's already done), but inscription envelope construction and the Ordinal Lock script template are new locking-script machinery you don't have today. Hodos's BEEF support is helpful: BSV21 indexers now expect BEEF-formatted submissions in some flows.

---

## Section 2 — Wallet provider JavaScript APIs

**2.1 Yours Wallet (`window.yours`).** Yours is the actively-maintained fork; Panda is its predecessor and the API surface is essentially identical. The `bitcoin-sv` org maintains a fork called `spv-wallet-browser`. ([Yours provider docs](https://yours-wallet.gitbook.io/provider-api), [GitHub](https://github.com/yours-org/yours-wallet))

Methods relevant to ordinals:

```ts
// Connection / discovery
isConnected(): Promise<boolean>
connect(): Promise<{ identityPubKey, ordPubKey, bsvPubKey }>
getAddresses(): Promise<{ bsvAddress, ordAddress, identityAddress }>
getBalance(): Promise<{ bsv, satoshis, usdInCents }>
getExchangeRate(): Promise<number>
getSocialProfile(): Promise<{ displayName, avatar }>
getPubKeys(): Promise<...>

// BSV
sendBsv(payments: PaymentObject[]): Promise<{ txid, rawtx }>
//   PaymentObject: { satoshis, address?, paymail?, data?: string[], script?, inscription? }

// Ordinals (1Sat)
getOrdinals(params?: { from?, limit? }): Promise<Ordinal[] | { ordinals, from }>
//   Ordinal fields: txid, vout, origin, data
inscribe(insc: { address, base64Data, mimeType, map?, satoshis? }[]): Promise<{ txid, rawtx }>
transferOrdinal({ address, origin, outpoint }): Promise<string>  // returns txid
purchaseOrdinal({ outpoint, marketplaceRate?, marketplaceAddress? }): Promise<string>

// Signing / utilities
signMessage({ message, encoding? }): Promise<{ sig, pubKey, address }>
getSignatures(params): Promise<SignatureResponse[]>  // arbitrary input signing
broadcast({ rawtx }): Promise<{ txid }>
encrypt({ message, pubKeys, encoding? }): Promise<string[]>
decrypt({ messages }): Promise<string[]>
```

Sources: Yours/Panda GitBook pages for [sendBsv](https://yours-wallet.gitbook.io/provider-api/the-basics/send-bsv), [getOrdinals](https://yours-wallet.gitbook.io/provider-api/ordinals/get-ordinals), [inscribe](https://yours-wallet.gitbook.io/provider-api/ordinals/inscribe), [transferOrdinal](https://yours-wallet.gitbook.io/provider-api/ordinals/transfer-ordinal), [purchaseOrdinal](https://yours-wallet.gitbook.io/provider-api/ordinals/purchase-ordinal).

**Critical observation:** `purchaseOrdinal` takes only an `outpoint` (and optional marketplace fee fields). The wallet itself fetches the listing UTXO from the indexer, builds the buy tx, signs, and broadcasts — the dApp does not hand it a PSBT. This implies `1sat.market` and the wallet share an assumption that listings are discoverable on-chain via an indexer (the Ordinal Lock model).

**2.2 Panda Wallet (`window.panda`).** Same API surface — Yours is the rebranded continuation. Same derivation paths: BSV `m/44'/236'/0'/0/0`, Ord `m/44'/236'/1'/0/0`. Most ordinal apps detect both `window.yours` and `window.panda`.

**2.3 Other providers.** HandCash (mainstream BSV custodial wallet, doesn't currently expose a 1Sat ordinal API equivalent), Twetch (deprecated), Aym (had a marketplace called "the Bazaar" — focuses on its own ecosystem, not a generic provider). For ordinal-app targets, **assume Yours/Panda is the de facto standard**.

**2.4 Is there a formal standard?** No formal BRC for the provider API. It's a *de facto* standard set by Panda → Yours, and most dApps integrate against it directly (no abstraction layer like ethers/web3 wagmi). The naming convention parallels MetaMask's `window.ethereum`. There is no equivalent of EIP-1193 in BSV land.

**What this means for Hodos.** To be a drop-in for ordinal dApps, Hodos needs to inject `window.yours` (and likely `window.panda` as an alias) with this exact method surface. **This is the single most important integration spec in this report.** Every ordinal dApp written against Yours will Just Work if Hodos shadows that surface.

---

## Section 3 — HTTP request shapes during a typical purchase

There is no published HTTP capture for `1sat.market`'s exact flow — the marketplace front-end is closed-source. Tracing the request flow from the Yours `purchaseOrdinal` source, the js-1sat-ord library, and the GorillaPool indexer docs, here's the model:

**3.1 Page → marketplace backend.** When the user lands on a listing page:

```
GET https://1sat.market/api/listings/<outpoint>
  → returns: { outpoint, price (satoshis), seller_address, lock_script_hex, content_url }
```
(Exact path is unverified — the marketplace API is undocumented publicly. The data shown on the listing page is what the wallet eventually needs.)

**3.2 Page → wallet (JS API).** The page calls:

```js
const txid = await window.yours.purchaseOrdinal({
  outpoint: "0640087e862c2eec40ea216032221d9af3e3688d9644ed32a4a9e389a2894a84_0",
  marketplaceRate: 0.05,
  marketplaceAddress: "17dyCLLqGoJNgzDKkVd8c9NkXhjzxius62"
});
```

That's it from the dApp side. Everything below happens *inside the wallet*.

**3.3 Wallet → indexer (look up the listing UTXO).**

```http
GET https://ordinals.gorillapool.io/api/txos/<txid>_<vout>
  → { outpoint, satoshis, script, owner, listing: { price, payout, ... }, origin }
```

Plus the wallet pulls the buyer's funding UTXOs:

```http
GET https://ordinals.gorillapool.io/api/utxos/address/<bsvAddress>
```

**3.4 Wallet builds, signs, broadcasts.** It constructs a tx that spends the Ordinal Lock UTXO + buyer-funding UTXOs, has outputs for (a) the ordinal going to buyer's `ordAddress`, (b) the seller's payout (enforced by the Ordinal Lock script), (c) optional `marketplaceRate` fee output, (d) change. Signs buyer inputs with `SIGHASH_ALL | FORKID`. Broadcasts:

```http
POST https://api.whatsonchain.com/v1/bsv/main/tx/raw
  Content-Type: application/json
  { "txhex": "01000000..." }
  → { "txid": "..." } (or 200 OK with txid string)
```

Yours is more likely using **GorillaPool's ARC** at `https://arc.gorillapool.io/v1/tx` (ARC accepts BEEF) but WhatsOnChain remains a fallback.

**3.5 Wallet returns to dApp:** the resolved txid. The dApp then typically polls the indexer or refreshes its listings view.

**Indexer subscription model (real-time).** GorillaPool exposes an SSE stream:

```http
GET https://ordinals.gorillapool.io/api/subscribe?address=<addr>&lock=<scriptHash>
  Accept: text/event-stream
```

Used for "new mempool tx for this address/lock-hash" notifications — the mechanism by which a marketplace detects "your listing just got bought." ([1Sat public APIs](https://docs.1satordinals.com/public-apis))

**What this means for Hodos.** The buy flow is *entirely* a wallet-side operation. The dApp doesn't need to know about Bitcoin transactions at all — it only needs the JS API. Hodos's existing `createAction`+BEEF builder is the right substrate; you'd add an Ordinal-Lock-aware unlock path and an indexer client.

---

## Section 4 — Indexer infrastructure

**4.1 Canonical indexer.** **GorillaPool** runs the de-facto canonical 1Sat indexer at `https://ordinals.gorillapool.io/api/`. The open-source version is `shruggr/1sat-indexer` on GitHub (described on the repo as "not yet functioning" — the production deployment is more advanced than the OSS snapshot). [docs](https://docs.1satordinals.com/public-apis)

Documented endpoints:

| Path | Purpose |
|---|---|
| `GET /api/inscriptions/txid/:txid` | Inscriptions in a tx |
| `GET /api/inscriptions/origin/:origin` | Inscription metadata for an origin |
| `GET /api/files/inscriptions/:origin` | Raw content (image/json/etc.) |
| `GET /api/utxos/address/:address` | Ordinal UTXOs for an address |
| `GET /api/utxos/address/:address/inscriptions` | UTXOs + their inscription data, batched |
| `GET /api/utxos/lock/:lock` | UTXOs by locking-script hash (used to find Ordinal Lock listings) |
| `GET /api/subscribe?address=...&lock=...` | SSE feed |
| `https://plugins.whatsonchain.com/api/plugin/main/:txid/:vout` | WoC fallback |

**4.2 JungleBus.** A "BSV firehose" run by GorillaPool. It crawls every block + mempool tx, parses, and pushes filtered streams to subscribers via webhooks/streams. The 1sat-indexer itself consumes JungleBus — your wallet does *not* need to. A wallet only needs the indexer's REST/SSE API. JungleBus matters only if Hodos wants to run its own 1Sat indexer, which would be massive over-scope. ([JungleBus](https://junglebus.gorillapool.io/), [js-junglebus repo](https://github.com/GorillaPool/js-junglebus))

**4.3 Data model.** REST + SSE. No GraphQL on the canonical GorillaPool indexer. Responses are plain JSON.

**What this means for Hodos.** Pick one indexer (GorillaPool), wrap it in a thin `OrdinalsIndexerClient` Rust struct, and use the SSE feed for live wallet-balance updates. Don't run your own indexer.

---

## Section 5 — Sigma auth ↔ 1Sat Ordinals relationship

**Same team.** Both Sigma (`BitcoinSchema/sigma`) and 1Sat Ordinals (`BitcoinSchema/1sat-ordinals`) live under the **BitcoinSchema** GitHub org. Same maintainers.

**Functional relationship:** Sigma is a *signing scheme* for transaction-embedded data, distinct from ordinal ownership.

- **Ordinal ownership is purely a function of holding the 1-sat UTXO.** No Sigma needed to own, transfer, list, or buy.
- **Sigma is an *optional* signing layer** that some 1Sat library functions accept (`createOrdinals`, `sendOrdinals`, `createOrdListings`, `transferOrdToken`, `deployBsv21Token`). When supplied with an `idKey`, Sigma appends a signature in `OP_RETURN` data tying the inscription/transfer to a Sigma identity, providing **replay protection and provenance/curation**.
- **Sigma supports both BSM and BRC-77** as inner signing primitives. BRC-77 uses derived child keys (BRC-42-style) — which Hodos *already implements* in `crypto/brc42`.
- **`1sat.market` does not require Sigma to list.** Listing requires owning the UTXO. Sigma shows up as an optional creator/curator stamp on inscriptions and as a way to do app-level identity ("logged-in as X" with no separate auth server).

**What this means for Hodos.** Treat them as orthogonal: ordinals need indexer + script-template work; Sigma needs a small BRC-77 signing helper plus an `OP_RETURN` builder. They can be implemented in either order.

---

## Section 6 — Hodos integration estimate

What Hodos has today that helps: BEEF-aware `createAction` builder, BRC-29/Paymail send paths, BRC-103 mutual auth, SQLite-backed UTXO management, `SyncHttpClient`.

**View-only (display ordinals owned by user) — Size: S**
- New components: `OrdinalsIndexerClient` (REST wrapper for GorillaPool); UTXO classifier flagging ordinal vs fungible; `Ordinal` row type + repo; UI panel calling indexer's `/utxos/address/<addr>/inscriptions`, rendering thumbnails from `/files/inscriptions/<origin>`.
- No new crypto, no new transaction builders.
- ~1 sprint.

**Transfer (send ordinal to another address) — Size: S–M**
- Reuses existing `createAction` builder with one new constraint: lock the 1-sat ordinal UTXO into a specific output, fund fees from non-ordinal UTXOs, never accidentally spend an ordinal UTXO for fees.
- New: separate "ordinal address" derivation path (`m/44'/236'/1'/0/0` per Yours convention).
- ~1 sprint.

**Buy from a marketplace (interact with 1sat.market) — Size: M–L**
- New: **Ordinal Lock** script template (lock + unlock paths). Covenant script with `OP_PUSH_TX`-style introspection for the price-output check. Reference: `js-1sat-ord` `purchaseOrdListing`.
- New: ability to spend a UTXO whose locking script is *not* P2PKH — Hodos's signer must accept arbitrary lock scripts.
- New: `purchaseOrdinal({ outpoint, marketplaceRate?, marketplaceAddress? })` JS API method exposed via V8 injection. For compatibility, alias as `window.yours` / `window.panda`.
- Optional: PSBT support — required if you want legacy PSBT-listed marketplaces, not needed for `1sat.market`.
- ~2 sprints.

**List for sale — Size: M**
- Inverse of buy: Ordinal Lock listing tx + cancel listing tx. Both are templates added to the script library plus new builder paths in `handlers.rs`.
- ~1.5 sprints.

**Inscribe (mint) — Size: S–M**
- 1-sat output with `<P2PKH> + <inscription envelope>`. Script-template work + a JS `inscribe()` method.
- BSV21 deploy/transfer is a JSON-content variant of the same.
- ~1 sprint.

**Total to ordinal-feature-parity with Yours Wallet: ~5–7 sprints.** Hardest piece is the Ordinal Lock covenant script and its unlock construction; everything else reuses Hodos primitives.

**Where you might get blocked / unclear:**
- `1sat.market`'s exact backend contract (closed source)
- The Ordinal Lock script bytes (need to copy from `js-1sat-ord`/`go-1sat-ord` source)
- Whether `purchaseOrdinal`'s `marketplaceRate` is enforced on-chain or just observed off-chain

---

## Sources

- [1Sat Ordinals Protocol Spec](https://docs.1satordinals.com/) · [GitHub README](https://github.com/BitcoinSchema/1sat-ordinals/blob/master/README.md)
- [Partially Signed Transactions](https://github.com/BitcoinSchema/1sat-ordinals/blob/master/partially-signed-transactions.md)
- [BSV-21](https://docs.1satordinals.com/fungible-tokens/bsv-21)
- [Public APIs (GorillaPool)](https://docs.1satordinals.com/public-apis)
- Yours Wallet: [Provider API root](https://yours-wallet.gitbook.io/provider-api), [sendBsv](https://yours-wallet.gitbook.io/provider-api/the-basics/send-bsv), [getOrdinals](https://yours-wallet.gitbook.io/provider-api/ordinals/get-ordinals), [inscribe](https://yours-wallet.gitbook.io/provider-api/ordinals/inscribe), [transferOrdinal](https://yours-wallet.gitbook.io/provider-api/ordinals/transfer-ordinal), [purchaseOrdinal](https://yours-wallet.gitbook.io/provider-api/ordinals/purchase-ordinal)
- [js-1sat-ord docs](https://js.1satordinals.com/) · [GitHub](https://github.com/BitcoinSchema/js-1sat-ord)
- [yours-org/yours-wallet GitHub](https://github.com/yours-org/yours-wallet) · [bitcoin-sv/spv-wallet-browser fork](https://github.com/bitcoin-sv/spv-wallet-browser)
- [shruggr/1sat-indexer](https://github.com/shruggr/1sat-indexer) · [JungleBus](https://junglebus.gorillapool.io/) · [js-junglebus](https://github.com/GorillaPool/js-junglebus)
- [BitcoinSchema/sigma](https://github.com/BitcoinSchema/sigma) · [Sigma docs](https://docs.sigmaidentity.com/)
- [1sat.market](https://1sat.market/)
