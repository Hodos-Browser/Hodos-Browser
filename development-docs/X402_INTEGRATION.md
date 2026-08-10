# x402 Integration — what we have, what's missing, what it costs

> **Created:** 2026-08-07 · **Updated:** 2026-08-08 (governance verified §7; freshness rule under revision §4a; **facilitator code read at head `9808154` §4c**; Cloudflare §6a)
> **Status:** Research complete. Not scheduled. No code written. **Item 7 of §3 is now explicitly blocked on spec — do not "fix" it.**
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
| 7 | Reconcile the payment-reuse cache with the freshness window (see §4) | `pay_402` | ⏸️ **BLOCKED on spec — do not fix yet.** The rule is under active revision; see §4a |

Everything reusable stays reused: the permission engine gate (`dispatch_payment`), the gold-pill IPC, `PaidContentCache`, the modal flow, activity logging.

**Estimate: days, not weeks** — items 4 and 7 are the only ones needing real thought.

---

## 4. ⚠️ The freshness-window / reuse-cache collision

**This is the one place our implementation may actually break under x402, and it is worth fixing before anyone depends on it.**

- x402 spec rule 4: `derivationSuffix` must be within a **30-second symmetric window** of the *verifier's* clock. Future-dated beyond the window is also rejected.
- We cache and reuse a minted payment for `PAY402_REUSE_TTL_MS = 25_000` (`handlers.rs`) for the same (URL, sats).

That leaves **~5 seconds** to absorb network RTT to the facilitator **plus clock skew between our clock and theirs**. Internet clock skew alone routinely exceeds that. A reused payment at 24s age against a verifier running 4s fast is rejected as stale — and it would present as intermittent, hard-to-reproduce 402 loops.

### 4a. ⏸️ Status: DO NOT FIX YET — the spec rule itself is under revision (2026-08-08)

**This collision exists only under x402.** Plain BRC-121 specifies no freshness window at all, so `PAY402_REUSE_TTL_MS = 25_000` is not a live bug in what we ship today. Tuning it now would mean tuning against a rule that is actively being challenged on the PR.

Raised as spec feedback on PR #2890 (2026-08-07). **`andyrowe` (bsv.cx, an independent live `exact`-on-BSV implementer on the plain-P2PKH addressing variant) replied 45 min later with a stronger reframe**, and the two positions have converged:

> `derivationSuffix` is being asked to do double duty — **anti-replay and freshness** — and the clock-skew squeeze is the symptom.

### 4b. What `derivationPrefix` / `derivationSuffix` actually are — and why the double duty is the bug

Neither field exists to carry time. Both exist to **derive a one-time key**. BRC-29 never pays to a fixed reusable address; for each payment the sender and recipient independently derive a fresh key from a shared invoice number:

```
2-3241645161d8-{derivationPrefix} {derivationSuffix}
 │      │                    └─ keyID: two base64 strings joined by a space
 │      └─ protocol ID (BRC-29 payments)
 └─ security level
```

| Field | Real job | Encoding |
|---|---|---|
| `derivationPrefix` | **Uniqueness** — fresh randomness ⇒ fresh key ⇒ no on-chain linkage between payments (this is where the privacy comes from) | base64(8 random bytes) |
| `derivationSuffix` | **More keyID string.** Being a timestamp is convention, not a derivation requirement | base64(utf8(decimal Unix ms)) |

Both are transmitted with the payment because the recipient needs them to re-derive the spending key — **without them the output is unspendable.** x402 then gave the suffix a *second* job: the verifier decodes it and checks it against its own clock. That second job is what forces two machines to agree on the time in order to move money, and it is the source of every failure mode in §4.

### 4c. ✅ Verified against the implementation (2026-08-08) — `facilitator/scheme.ts`

Read at head `9808154` (`bsv-blockchain/x402` @ `feat/bsv-exact-scheme`), path `typescript/packages/mechanisms/bsv/src/exact/facilitator/scheme.ts`. Three findings; two of them **correct earlier text in this doc**.

**① ⚠️ CORRECTION — `isMerge` is NOT a replay signal on its own.** An earlier draft of this section said "txid-dedup + `isMerge` catch resubmission." That is imprecise. The code requires a **conjunction**:

```ts
const newlyInternalized = typeof result.satoshis === "number" && result.satoshis > 0;
if (result.isMerge && !newlyInternalized) {
  return this.failure(network, payer, "duplicate_settlement");
}
```

with an explanatory comment: *"`isMerge` alone is not a replay: self-payments (same wallet creates and internalizes) report `isMerge: true` with newly internalized satoshis on first settle."* The distinction is correct and load-bearing — **do not restate it the loose way.**

**② ⭐ The dedup cache already outlives the freshness window by ~20×.** This is the strongest evidence that the timestamp is not bounding replay state:

```ts
const SETTLEMENT_CACHE_TTL_FLOOR_MS = 600_000;   // 10 minutes
const ttl = Math.max(SETTLEMENT_CACHE_TTL_FLOOR_MS, windowMs);
```

The spec prose ("dedup record covering at least `paymentWindow + maxTimeoutSeconds`") reads as though a longer freshness window would force proportionally longer retention — **the implementation disproves that.** A hard 10-minute floor holds txids while the window rejects payloads at 30s. A more generous freshness rule costs *nothing extra* in retention up to that floor.

**③ The asymmetry andyrowe asked for partly exists already** — it just doesn't reach `/verify`:

```ts
if (age < -this.paymentWindowMs) reject                    // future: paymentWindow only
if (age > this.paymentWindowMs + settleBudgetMs) reject     // past: paymentWindow + settle budget
```

Per the spec, the past-side extension applies **at settlement**. Our 25s reuse TTL bites at **`/verify`**, where it's a flat 30s. *(Not fully verified: the callers of `checkTimestamp` were not read, so "settleBudgetMs is 0 at verify" comes from spec prose, not call sites.)*

**Revised ask — smaller and harder to refuse than a new field:** extend the existing past-side allowance to `/verify`, and state explicitly that a client MAY reuse a payload within the window. Given the 10-minute dedup floor, reuse is safe — a replayed payload is caught by txid whether it is 2s or 200s old. A server-issued absolute `expiresAt` (one clock, payer skew drops out) remains the cleaner long-term shape, but it is no longer the minimum viable fix.

⚠️ **Caveat andyrowe raised himself:** bsv.cx pins amount *and* output at issuance because it uses server-issued single-use invoices. In the BRC-29 flow the **payer** generates prefix/suffix, so the output isn't known until minting. Doesn't block the proposal — the two jobs still separate — but it is the open question on the thread.

**Bottom line: hold the code.** If either the past-side extension or `expiresAt` lands, the collision disappears and §4's human-in-the-loop concern below stops being live. Keep the mapping current.

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

### 6a. Cloudflare entered the demand side (2026-08-04) — why open question #3 is about demand, not the merge

Cloudflare announced [Cloudflare Wallets + cloudflare.pay](https://blog.cloudflare.com/wallets/) ([press release](https://www.cloudflare.com/press/press-releases/2026/cloudflare-gives-ai-agents-an-identity-and-a-wallet/)). **Live today: handle reservation only**; wallets, on/off-ramp and Virtual Wallets are "in the coming months," and the seller-side **Monetization Gateway is waitlist/preview**.

- **Custody: UNDISCLOSED.** Not stated in the blog or press release. Do not repeat "custodial" as fact — though account-linked balances plus geographic on/off-ramps strongly imply hosted custody (*inference, unverified*).
- **Chains for the wallet: undisclosed.** But [Cloudflare's x402 docs](https://developers.cloudflare.com/agents/x402/) cover Base, Ethereum, Polygon, Optimism, Arbitrum, Avalanche, Solana, Aptos, Stellar, Sui — **USDC settlement**, and *"`https://x402.org/facilitator` is the public facilitator operated by Coinbase and is used in all Cloudflare examples."*
- **No public wallet API** for third-party integration today.

**Why this matters to us more than the PR does.** x402 is asset-agnostic in spec and highly concentrated in practice (USDC / Base / Coinbase facilitator). BSV appears **nowhere** in [docs.x402.org's supported networks](https://docs.x402.org/core-concepts/network-and-token-support). If Cloudflare becomes how sites turn on 402, the accepted-asset list is effectively set upstream of us — and our payment path never fires, not for protocol reasons but because **no merchant advertises `bsv:mainnet`**. The threat is demand-side aggregation, not the wallet product.

Where Cloudflare does *not* compete: they shipped no browser, and their model is agent-first and custody-hosted. Our differentiators — user-present consent, the gold pill, the per-domain permission engine, non-custodial keys in-process — are orthogonal to it. For headless/server-side agents, however, a browser wallet is largely redundant in their model.

### 6b. Structural advantages over the mobile browser

Both stem from embedding the engine rather than wrapping a WebView:
1. **We read 402 response headers natively.** Its own docs note WebView native navigations don't expose response headers, so it **re-fetches the URL** to read them — a duplicate request per 402.
2. **Persistent paid-content cache.** Ours is SQLite-backed with `Cache-Control` TTL and a 500 MB LRU; its is 30 minutes in memory, injected via `document.write()`.

---

## 7. Governance — resolved 2026-08-08

**Both circulating claims are true; they describe different layers.** Verified at primary sources:

- **2026-04-02** — [Linux Foundation announces it will launch the x402 Foundation](https://www.linuxfoundation.org/press/linux-foundation-is-launching-the-x402-foundation-and-welcoming-the-contribution-of-the-x402-protocol). Protocol described as "initially developed by **Coinbase, Cloudflare, and Stripe**."
- **2026-07-14** — [Operational launch](https://www.linuxfoundation.org/press/linux-foundation-announces-operational-launch-of-x402-foundation-to-standardize-internet-native-payments-for-ai-agents-and-applications). Coinbase formally transfers the protocol. 40 member orgs.

⚠️ **The LF supplies a neutral legal/organizational home. It did NOT change who decides.** [`TSC.md`](https://raw.githubusercontent.com/x402-foundation/x402/main/TSC.md) lists exactly three organizations on the Technical Steering Committee:

| Org | Representative |
|---|---|
| Coinbase, Inc. | Erik Reppel |
| Cloudflare, Inc. | Rohin Lohe |
| Stripe, Inc. | Steve Kaliski |

`CONTRIBUTING.md`: *"Merging contributions is at the discretion of the x402 Foundation team, based on the risk of the contribution and the quality of implementation."*

**BSV Association is an Associate Member** — the lowest of three tiers, with Cardano Foundation, Casper, Japan Contents Blockchain Initiative, OMA3. Premier members include Circle (USDC issuer), Solana Foundation, Stellar Development Foundation, Ripple, Monad, plus Coinbase/Cloudflare/Stripe. **Do not assume LF governance implies outsider-neutral merit review** — that inference does not hold.

### ⚠️ Contribution process — PR #2890 does not match it

`CONTRIBUTING.md` mandates a **three-PR workflow** for a new chain: spec PR first → **merged** → reference implementation in a single SDK → additional SDKs. PR #2890 is one 43-file, +4,362-line PR carrying spec + implementation + examples + 103 tests together. As of 2026-08-08: **31 comments, 0 review comments, 0 reviews, no requested reviewers, `mergeable_state: unstable`**, three weeks open.

CONTRIBUTING also warns that contributions *"that show clear signs of unreviewed AI output... may be closed without detailed review."* The PR discloses AI assistance (correctly), but paired with a monolithic diff that is a risk factor. Most thread comments are content-free ecosystem cheerleading — **never add to that; only implementer-grade technical comments help** (see [[project_x402_brc121_ecosystem_2026_08_07]]).

**The §4a freshness fix is the natural small spec-only PR** that would fit the documented workflow.

---

## 8. Open questions

1. Does PR #2890 merge, and in what shape? Structural mismatch with the 3-PR workflow (§7) is the leading explanation for the silence.
2. Does the freshness rule adopt server-issued `expiresAt` (§4a)? **This is the live one** — it determines whether item 7 in §3 is work at all.
3. Do we implement the adapter speculatively, or wait for merge? **Lean: wait — but the reason matters.** Nothing upstream *gates* us: Hodos is a C++/Rust client that doesn't consume the TS SDK, so we could emit a conforming `PAYMENT-SIGNATURE` today. **The real gate is demand — no server advertises `bsv:mainnet`.** A merge alone won't unblock us; a paying server would.
4. Should we propose header-name alignment to BRC-121 itself, or let x402 supersede it? Probably the latter.
5. ~~Governance~~ — **resolved, see §7.**

---

## References

- Spec: [`specs/schemes/exact/scheme_exact_bsv.md`](https://github.com/bsv-blockchain/x402/blob/feat/bsv-exact-scheme/specs/schemes/exact/scheme_exact_bsv.md) (PR branch)
- [BRC-121 Simple 402 Payments](https://bsv.brc.dev/payments/0121) · [BRC-29](https://bsv.brc.dev/payments/0029) · [BRC-42](https://bsv.brc.dev/key-derivation/0042) · [BRC-62 BEEF](https://bsv.brc.dev/transactions/0062) · [BRC-95 Atomic BEEF](https://bsv.brc.dev/transactions/0095)
- Our demo + live test target: `demos/brc121-402/README.md`, `https://now.bsvblockchain.tech`
- [x402.org](https://x402.org) · [x402-foundation/x402](https://github.com/x402-foundation/x402)
