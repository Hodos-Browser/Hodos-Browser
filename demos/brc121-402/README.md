# BRC-121 Simple HTTP 402 Payment — demo server

Tiny Express server that demonstrates the wallet-side BRC-121 flow:

1. Client GETs `/paid`.
2. Server returns `402 Payment Required` with two headers:
   - `x-bsv-sats: <amount>` — how much to pay.
   - `x-bsv-server: <hex pubkey>` — the server's identity key (BRC-42 derivation target).
3. Hodos's browser intercepts the 402, builds a BRC-29 payment to the server's
   key (with the standard 1000-sat Hodos service-fee output), and retries the
   request with `x-bsv-payment: <atomic BEEF base64>`.
4. Server parses the BEEF, verifies output 0 pays at least the requested sats,
   and returns 200 with the paid content.

Hodos does not broadcast the BEEF; this server *would* broadcast in production
(plug in `@bsv/sdk`'s ARC broadcaster). This demo only validates the BEEF shape.

## Run

```bash
cd demos/brc121-402
npm install
npm start
```

Server listens on `http://localhost:31402` by default. Optional env vars:

- `PORT` — listen port (default `31402`).
- `PRICE_SATS` — amount to charge per visit (default `100`).
- `SERVER_WIF` — pin the server private key in WIF format. If unset, a fresh
  key is generated each launch (good for repeated test runs).

## Endpoints

| Method | Path | Purpose |
|---|---|---|
| GET | `/` | Landing page with server pubkey + a link to `/paid` |
| GET | `/paid` | The 402-gated resource. First request → 402 challenge; retry with valid `x-bsv-payment` → 200 |
| GET | `/health` | JSON with server pubkey, price, and accepted-txid count |

## Real-world test target (production)

A live BRC-121 server exists in the wild: **`https://now.bsvblockchain.tech`**
("The NOW™ Times" paid micro-parody news site). Free landing page at `/`;
402-protected articles at `/articles/<slug>` priced 75–150 sats.

```
$ curl -i https://now.bsvblockchain.tech/articles/runar-playground
HTTP/1.1 402 Payment Required
x-bsv-sats: 75
x-bsv-server: 0373ce63481ace3634e235af7a73742444b2d6abd8742b182ad595a84028d00c00
```

Use this as the canonical real-world Phase 1 acceptance target. The local
demo server here is for offline / CI / dev-machine smoke testing.

## Driving from Hodos

1. Start the Rust wallet (`./dev-wallet.ps1` or the platform equivalent).
2. Start the frontend dev server (`cd frontend && npm run dev`).
3. Start this demo server (`npm start` in this folder).
4. Build + launch Hodos (`cd cef-native && ./win_build_run.sh` or `./mac_build_run.sh`).
5. In Hodos, navigate to `http://localhost:31402/paid`.
6. First visit: approve the domain (existing Hodos approval flow).
7. Subsequent visits should auto-approve silently. The tab badge animates green
   on every successful payment, and the page renders the "✅ Payment accepted"
   content.

## What's intentionally not in v1

This demo is for Phase 1 happy-path validation. It does not:

- Verify SPV / merkle proofs in the BEEF (`tx.verify(...)` is the hook).
- Broadcast the transaction (Hodos sends `noSend=true` BEEF; the server is
  *expected* to broadcast in production).
- Re-derive the BRC-42 child address to confirm output 0 lands at our derived
  P2PKH (a stricter validator would do this — but with `derivation_prefix` /
  `derivation_suffix` not in standard BRC-121 headers, this requires the
  client to send those nonces too, which is outside the v1 spec).

These are useful enhancements for a real BRC-121 server but live outside the
scope of the Phase 1 demo, which exists to validate the round-trip end-to-end.

## Phase context

See:

- `development-docs/Sigma-BRC121-Sprint/phase-1-brc121/README.md` — phase scope.
- `development-docs/Sigma-BRC121-Sprint/_DRAFT_RECOVERED_PLAN.md` — original
  detailed plan with citations to all the existing Hodos code paths reused.
- `rust-wallet/src/handlers.rs` — `pay_402` handler.
- `cef-native/src/core/HttpRequestInterceptor.cpp` — `OnResourceResponse`
  (the 402 detection + auto-approve gate + sync wallet call).
