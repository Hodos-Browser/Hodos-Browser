# x402 Integration — what we have, what's missing, what it costs

> **Created:** 2026-08-07
> **Status:** Research complete. Not scheduled. No code written.
> **Context:** [x402-foundation/x402 PR #2890](https://github.com/x402-foundation/x402/pull/2890) — `feat(bsv): add exact scheme support for BSV`, by sirdeggen (Deggen), **open**, 43 files / +4,362 lines.
> **TL;DR:** We already ship the hard part. x402 support is a **serialization adapter**, not new crypto.

---

## 1. The relationship: x402 ⊃ BRC-121

x402 is Coinbase's HTTP-402 payment standard (Erik Reppel, first commits 2025-02-17, public May 2025; now under the x402 Foundation). PR #2890 adds BSV as an `exact`-scheme mechanism, and its spec says so outright:

> *"The mechanism is the [BRC-29](https://bsv.brc.dev/payments/0029) payment protocol as profiled by [BRC-121: Simple 402 Payments](https://bsv.brc.dev/payments/0121)."*

**So x402-on-BSV is BRC-121 in a different envelope.** Same BRC-42 derivation, same BRC-29 payment, same BEEF transport. What changes is how the challenge and the payment are framed on the wire.

We implement **BRC-121 today** (`rust-wallet/src/handlers.rs :: pay_402`, `cef-native/src/core/HttpRequestInterceptor.cpp :: Async402ResourceHandler`). We do **not** implement x402.

---

## 2. Wire format differences

### 2a. The 402 challenge

| | BRC-121 (what we handle today) | x402 `exact`/BSV |
|---|---|---|
| Shape | Two flat HTTP headers | JSON `PaymentRequirements` |
| Amount | `x-bsv-sats: 75` | `accepted.amount: "75"` (decimal string) |
| Recipient | `x-bsv-server: <66-hex identity key>` | `accepted.payTo: <66-hex identity key>` |
| Network | *(implicit — mainnet)* | `accepted.network: "bsv:mainnet"` **(new)** |
| Asset | *(implicit)* | `accepted.asset: "BSV"` **(new)** |
| Settlement budget | *(none)* | `accepted.maxTimeoutSeconds` **(new)** |
| Scheme | *(none)* | `accepted.scheme: "exact"` **(new)** |

### 2b. The payment retry

Ours goes out as **five flat headers**; x402 wants **one JSON object** in a `PAYMENT-SIGNATURE` header.

| Semantic field | Hodos header | x402 `payload` field |
|---|---|---|
| Signed BEEF | `x-bsv-beef` | `transaction` |
| Derivation prefix | **`x-bsv-nonce`** | `derivationPrefix` |
| Derivation suffix | **`x-bsv-time`** | `derivationSuffix` |
| Payer identity key | `x-bsv-sender` | `senderIdentityKey` |
| Payment output index | `x-bsv-vout` | `outputIndex` |

### 2c. ⚠️ Header-name drift between the two shipping BRC-121 clients

**Hodos and `bsv-blockchain/bsv-browser` do not use the same header names.** Both send five headers carrying identical semantics; two of the names differ:

| Field | Hodos | BSV Browser (Deggen) | x402 payload |
|---|---|---|---|
| Derivation prefix | `x-bsv-nonce` | `x-bsv-prefix` | `derivationPrefix` |
| Derivation suffix | `x-bsv-time` | `x-bsv-suffix` | `derivationSuffix` |
| BEEF / sender / vout | `x-bsv-beef` / `x-bsv-sender` / `x-bsv-vout` | same | `transaction` / `senderIdentityKey` / `outputIndex` |

BRC-121 evidently doesn't pin these tightly enough, and two independent implementations drifted. **x402's JSON field names follow BSV Browser's naming, not ours.** This is a decent independent argument *for* x402: a written payload schema removes exactly this ambiguity.

> Practical note: a server built against one client's header names will silently 402-loop the other. Worth testing against `now.bsvblockchain.tech` if we ever depend on cross-client compatibility.

### 2d. Where we're already byte-compatible

Our `pay_402` derivation matches the x402 spec exactly, arrived at independently:

| Spec requirement | Our implementation |
|---|---|
| `derivationPrefix` ≥ 8 random bytes, base64 | `base64(8 random bytes)` ✅ |
| `derivationSuffix` = base64(UTF-8(decimal Unix ms)) | identical ✅ |
| BRC-29 protocol ID `[2, "3241645161d8"]`, keyID `"<prefix> <suffix>"` | invoice `2-3241645161d8-{prefix} {suffix}` ✅ |
| P2PKH over hash160 of BRC-42 child key | ✅ |
| BEEF / Atomic BEEF with SPV ancestry | ✅ |

**Nothing in the crypto or transaction-building layer needs to change.**

---

## 3. What we would actually have to build

Client side only. Settlement (`internalizeAction`) is the *recipient's* wallet — a server concern, not ours.

| # | Work | Where | Size |
|---|---|---|---|
| 1 | Detect an x402-shaped 402 (JSON `PaymentRequirements`) alongside the `x-bsv-sats` form | `HttpRequestInterceptor.cpp :: TryHandleBrc121_402` | S |
| 2 | Parse `accepted{scheme, network, asset, amount, payTo, maxTimeoutSeconds}` | same | S |
| 3 | Serialize the existing five fields into the `PaymentPayload` JSON and send as `PAYMENT-SIGNATURE` | `Async402ResourceHandler` | S |
| 4 | **Enforce strict amount equality** — x402 requires *exactly* `amount` sats; BRC-121 tolerates overpayment | `pay_402` | **M — behavioural change, verify first** |
| 5 | Refuse non-`bsv:*` and ambiguous `bip122:*` networks; refuse `asset != "BSV"` | interceptor | S |
| 6 | Parse `SettlementResponse` (`success`, `payer`, `transaction`, `network`, `errorReason`) for UI/activity-log | interceptor + frontend | S |
| 7 | Reconcile the payment-reuse cache with the freshness window (see §4) | `pay_402` | **M — real bug risk** |

Everything reusable stays reused: the permission engine gate (`dispatch_payment`), the gold-pill IPC, `PaidContentCache`, the modal flow, activity logging.

**Estimate: days, not weeks** — items 4 and 7 are the only ones needing real thought.

---

## 4. ⚠️ The freshness-window / reuse-cache collision

**This is the one place our implementation may actually break under x402, and it is worth fixing before anyone depends on it.**

- x402 spec rule 4: `derivationSuffix` must be within a **30-second symmetric window** of the *verifier's* clock. Future-dated beyond the window is also rejected.
- We cache and reuse a minted payment for `PAY402_REUSE_TTL_MS = 25_000` (`handlers.rs`) for the same (URL, sats).

That leaves **~5 seconds** to absorb network RTT to the facilitator **plus clock skew between our clock and theirs**. Internet clock skew alone routinely exceeds that. A reused payment at 24s age against a verifier running 4s fast is rejected as stale — and it would present as intermittent, hard-to-reproduce 402 loops.

Options, in preference order:
1. **Drop the reuse TTL well below the window** for x402 requests (e.g. 10s), keeping 25s for plain BRC-121.
2. Mint fresh per request when the challenge is x402-shaped.
3. Derive the TTL from the challenge — but the spec exposes `maxTimeoutSeconds` (a *settlement* budget), not the verify window, so this isn't directly available.

Raised as spec feedback on PR #2890 (2026-08-07) — the spec is currently **silent** on whether a client may reuse a payload at all within the window, and a conforming client could reasonably do either.

### Related: human-in-the-loop timing

Our permission modal can sit open a long time (modal timeout is 600s). **We are safe today** because `pay_402` mints the payment *after* `dispatch_payment` approval returns — the freshness clock starts post-approval. Worth preserving deliberately: **never mint before approval**, or the payment can expire while the user reads the dialog. This ordering is an implementation choice the spec doesn't mandate.

---

## 5. Other behavioural deltas

- **Strict equality vs overpayment.** Spec rule 6, verbatim: *"this is stricter than plain BRC-121, which accepts overpayment; x402 exact semantics require equality."* Verify our payment output is exact. (Our 1000-sat Hodos service fee is a *separate output*, so it should not affect the payment output — confirm.)
- **Zero-conf settlement.** The spec is explicit that `success: true` reflects wallet acceptance of a typically-unmined transaction, and that the payer can attempt a double-spend until mined. Irrelevant to us as payer; relevant if we ever run a facilitator.
- **Broadcast responsibility.** In x402 the *facilitator* broadcasts after internalizing. Matches our noSend model — we do **not** broadcast. (Note `bsv-blockchain/bsv-browser` does the opposite: it broadcasts before the retry with `acceptDelayedBroadcast: false`.)
- **Networks.** `bsv:mainnet`, `bsv:testnet`, `bsv:ttn`, `bsv:tstn`, registered via [ChainAgnostic/namespaces#190](https://github.com/ChainAgnostic/namespaces/pull/190). Ambiguous `bip122:*` genesis references MUST be refused — BSV shares a genesis block with BTC and BCH.

---

## 6. Ecosystem position

| Product | Platform | Browser? | Wallet? | 402 | x402 |
|---|---|---|---|---|---|
| **Hodos** | Windows/macOS desktop | ✅ CEF native | ✅ Rust, in-process | ✅ BRC-121 | ❌ |
| `bsv-blockchain/bsv-browser` | iOS/Android | ✅ RN WebView | ✅ on-device | ✅ BRC-121 | ❌ |
| `bsv-blockchain/bsv-desktop` | Win/mac/Linux | ❌ | ✅ Electron, HTTPS on **:2121** | ❌ | ❌ |

**Nobody ships x402-on-BSV yet** — PR #2890 is the reference implementation and it is unmerged. First mover is available.

Two structural advantages we have over the mobile browser, both stemming from embedding the engine rather than wrapping a WebView:
1. **We read 402 response headers natively.** Its own docs note WebView native navigations don't expose response headers, so it **re-fetches the URL** to read them — a duplicate request per 402.
2. **Persistent paid-content cache.** Ours is SQLite-backed with `Cache-Control` TTL and a 500 MB LRU; its is 30 minutes in memory, injected via `document.write()`.

---

## 7. Open questions

1. Does PR #2890 merge, and in what shape? It's a chain integration into a foundation repo where BSV is a newcomer.
2. Does the freshness window get clarified (reuse allowed or mint-fresh)?
3. Do we implement the adapter speculatively, or wait for merge? **Lean: wait for merge, but keep the mapping current** — the cost of waiting is low because we already have every field.
4. Should we propose header-name alignment to BRC-121 itself, or let x402 supersede it? Probably the latter.
5. Governance: the x402 Foundation's relationship to the Linux Foundation (claimed by BSVA 2026-08-07) is **unverified** and materially affects how likely a BSV chain integration is to be judged on merit.

---

## References

- Spec: [`specs/schemes/exact/scheme_exact_bsv.md`](https://github.com/bsv-blockchain/x402/blob/feat/bsv-exact-scheme/specs/schemes/exact/scheme_exact_bsv.md) (PR branch)
- [BRC-121 Simple 402 Payments](https://bsv.brc.dev/payments/0121) · [BRC-29](https://bsv.brc.dev/payments/0029) · [BRC-42](https://bsv.brc.dev/key-derivation/0042) · [BRC-62 BEEF](https://bsv.brc.dev/transactions/0062) · [BRC-95 Atomic BEEF](https://bsv.brc.dev/transactions/0095)
- Our demo + live test target: `demos/brc121-402/README.md`, `https://now.bsvblockchain.tech`
- [x402.org](https://x402.org) · [x402-foundation/x402](https://github.com/x402-foundation/x402)
