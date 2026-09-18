# x402 Integration — what we have, what's missing, what it costs

> **Created:** 2026-08-07 · **Updated:** 2026-09-18 (**§3a added** — the optional `resource` field in the v2 `PaymentPayload`: why the facilitator-correlation problem Brave closed in `bx402` PR #105 cannot arise on the BSV scheme, the v1→v2 structural change that makes the field optional, and the decision to omit it with a check at §9 target B). Previous: 2026-09-14 (sweep at head `30a30e8` — still open/`blocked`, 1 review, BSV changes since 09-02 are wording-only; daily upstream merges are automated; merge authority, the Cardano three-PR precedent, two older BSV spec PRs #1844/#1004, and repo provenance recorded, §7). Previous: 2026-08-26 (**BRC-166 read in full** from PR #231 — wire format, `payloadFormat.kind` mode discriminator, and "Overpayment MUST be accepted" now specified, closing §9 target B's open question, §9b; **BRC-120 name collision recorded**, §1). Previous: 2026-08-25 (status re-verified — still open/`blocked`; andyrowe's approval remains the only review; no new comments; 11 commits, head `d14bff7` — an upstream merge plus a prettier-formatting fix to the BSV constants test, §7). Previous: 2026-08-24 (heads `6cfa20a`/`36c1fda` — merge-only, no BSV files, §4a/§8 re-checks carry over; unit/lint workflows still never run, §7) · 2026-08-21 (first review on the PR — andyrowe **APPROVED**, non-maintainer, §7; head `53a8fdf` — merge-only, no BSV files; fork CI identified as a concrete component of `blocked`, §7) · 2026-08-19 (CAIP-2 `bsv` namespace **registered** §5; PR status re-verified at head `f8813af` §7; freshness rule re-checked — still 30s symmetric at `/verify`, §4a/§4c; test matrix §9; decision record §10; draft comment §11) · 2026-08-08 (governance §7; facilitator code read at `9808154` §4c; Cloudflare §6a)
> **Status:** Research complete. **Decision recorded (§10): items 1,2,3,5,6 → a `beta.4`/feature branch; hold 4 and 7. Not a beta.3 tail item.** No code written. **Item 7 of §3 remains blocked on spec — do not "fix" it.**
> **Context:** [x402-foundation/x402 PR #2890](https://github.com/x402-foundation/x402/pull/2890) — `feat(bsv): add exact scheme support for BSV`, by sirdeggen (Deggen), **open**, 26 commits, head `30a30e8` (2026-09-12; re-verified 2026-09-14).
> **TL;DR:** We already ship the hard part. x402 support is a **serialization adapter**, not new crypto.

---

## 1. The relationship: x402 ⊃ BRC-121

> ⚠️ **NAME COLLISION — two unrelated protocols are called "x402" (recorded 2026-08-26).** This
> document is about the **x402 Foundation** one throughout. The other is **[BRC-120: x402 Stateless
> Settlement-Gated HTTP Protocol](https://bsv.brc.dev/payments/0120)** by Rui Da Silva (Merkle
> Works) — **merged in the BRC registry**, BSV-native, and **frozen at v1.0** in the
> `merkleworks-x402-spec` repo (BRC-120 assigns the number and conformance rules; it does not
> restate the wire format). They share a number and an idea, not a wire format; they are **not
> interoperable and neither is a profile of the other.**
>
> | | **BRC-120** (Merkle Works) | **This document** (x402-F) |
> |---|---|---|
> | Challenge header | `X402-Challenge` | `PAYMENT-REQUIRED` |
> | Proof header | `X402-Proof` | `PAYMENT-SIGNATURE` |
> | Replay control | nonce UTXO + RFC 8785 canonical JSON binding | per-invoice address / server-side invoice state |
> | Extensibility | none — frozen, conformance all-or-nothing | `scheme` |
>
> Surfaced by BRC-166 §2, which puts the warning before anything else because "implementers who
> miss this will build the wrong thing." **Practical consequence for us:** when anyone in the BSV
> ecosystem says "x402" unqualified, establish which one they mean before acting — and never
> describe our BRC-121 work as "BRC-120 compliant."

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

### 3a. `resource` in the payload — why the facilitator-correlation problem does not reach us (2026-09-18)

**Recorded so nobody re-derives it.** The v2 `PaymentPayload` carries an optional `resource` object, and the BSV scheme's own example fills it with a query string: `"resource": {"url": "http://api.example.com/weather?city=San%20Francisco"}` *(read: `scheme_exact_bsv.md` §`PAYMENT-SIGNATURE` Header Payload)*. Item 3 builds that object, so the field is real in the shape we serialize.

**On EVM rails this is a disclosure channel.** Brave's `bx402` [PR #105](https://github.com/brave-experiments/bx402/pull/105) (merged 2026-09-18) blanks `resource.url` to `""` before the payload leaves for the facilitator, because their production facilitator is Coinbase-hosted and would otherwise learn what was searched alongside the paying wallet. It is a 12-line change in `src/x402.ts` with 87 lines of test *(read: the diff)*.

**It cannot arise on BSV as specified.** `scheme_exact_bsv.md` defines the facilitator as the recipient's own BRC-100 wallet — in-process in the resource server, or self-hosted by the recipient. Verification rule 2 requires the facilitator to hold the `payTo` identity key, and §Facilitator Deployment puts a shared multi-tenant facilitator out of scope. ⇒ The party receiving `resource` is the origin, which served the request and already knows the URL. Item 5's refusal of non-`bsv:*` networks closes the other direction: we never present a payment to an EVM facilitator as a client.

⚠️ **The v1 → v2 change is what makes any of this optional.** In v1, `resource` was a **required string inside `PaymentRequirements`**, and `/verify` received that object — the facilitator had to see it. In v2 it moved to a `ResourceInfo` object, required in `PaymentRequired` but **optional in `PaymentPayload`**, and the `/verify` body's separate `paymentRequirements` object has no `resource` field at all *(read: `x402-specification-v1.md` §5.1.2 and `-v2.md` §5.2.2, §7.1)*. Anyone reasoning from a v1 implementation will reach the wrong conclusion.

⇒ **The default for item 3: omit `resource`.** It costs nothing, it is spec-legal, and it keeps the field from becoming a habit if a later deployment ever does put a third party on that hop. ⭐ **Verify it at §9 target B (`bsv.cx`)** — an implementer's requirement we cannot see would fail loudly as a verify refusal, which is Brave's own stated caution and the reason they want a testnet canary before production. If B rejects a payment without the field, send it and record that here. Item 3 stays size **S** either way.

**Estimate: days, not weeks** — items 4 and 7 are the only ones needing real thought.

---

## 4. ⚠️ The freshness-window / reuse-cache collision

**This is the one place our implementation may actually break under x402, and it is worth fixing before anyone depends on it.**

- x402 spec rule 4: `derivationSuffix` must be within a **30-second symmetric window** of the *verifier's* clock. Future-dated beyond the window is also rejected.
- We cache and reuse a minted payment for `PAY402_REUSE_TTL_MS = 25_000` (`handlers.rs`) for the same (URL, sats).

That leaves **~5 seconds** to absorb network RTT to the facilitator **plus clock skew between our clock and theirs**. Internet clock skew alone routinely exceeds that. A reused payment at 24s age against a verifier running 4s fast is rejected as stale — and it would present as intermittent, hard-to-reproduce 402 loops.

### 4a. ⏸️ Status: DO NOT FIX YET — the spec rule itself is under revision (2026-08-08)

**This collision exists only under x402.** Plain BRC-121 specifies no freshness window at all, so `PAY402_REUSE_TTL_MS = 25_000` is not a live bug in what we ship today. Tuning it now would mean tuning against a rule that is actively being challenged on the PR.

**Re-checked 2026-08-19 at head `f8813af` — the rule has NOT changed.** Commit history of the PR branch: `ecb9235` (initial) → `1dad246` "harden exact scheme verification and replay defense" (2026-07-21) → `bf4f969` (ttn/tstn + bip122 refusal) → `9808154` (merge main; the head §4c was read at) → `fa6f8ad` (merge upstream/main, 2026-08-19) → `f8813af` (adapt to upstream `SchemeNetworkServer`/`MoneyParser` API changes, 2026-08-19) → `53a8fdf` (merge upstream/main, 2026-08-21 — **merge-only for BSV**: `compare f8813af...53a8fdf` touches only `docs/dev-tools/facilitators.md` (+1 line) and a new `specs/schemes/batch-settlement/scheme_batch_settlement_svm.md` (+1,530); no BSV files, so every `f8813af` re-check in this section carries over unchanged) → `6cfa20a` (merge upstream/main, 2026-08-22) → `36c1fda` (merge upstream/main, 2026-08-24 — both likewise merge-only for BSV: `compare 53a8fdf...36c1fda` touches only upstream EVM/Sei-stablecoin, docs, and spec-template files; no BSV files, so the re-checks continue to carry over). `1dad246` **predates** the §4c read and its spec diff touched rule 1 (prefix ≥ 8 bytes, `asset` must be `BSV`) and the replay-protection prose (dedup record ≥ `paymentWindow + maxTimeoutSeconds`, sticky-session note) — **rule 4 itself is unchanged**: 30 s symmetric, past side extended by `maxTimeoutSeconds` only at settlement. The two 2026-08-19 commits touch `server/scheme.ts` and `moneyParser.ts` only — no facilitator freshness change. `DEFAULT_PAYMENT_WINDOW_MS = 30_000` still in `constants.ts`. **Item 7 stays blocked.** Neither the past-side-at-verify extension nor `expiresAt` has been adopted, and no maintainer has engaged the thread on it.

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

Per the spec, the past-side extension applies **at settlement**. Our 25s reuse TTL bites at **`/verify`**, where it's a flat 30s. ~~*(Not fully verified: the callers of `checkTimestamp` were not read…)*~~ **Closed 2026-08-19, read at head `f8813af`:** `checkTimestamp(suffix, requirements, phase)` computes `settleBudgetMs = phase === "settle" && maxTimeoutSeconds > 0 ? maxTimeoutSeconds*1000 : 0` — so at `/verify` it is **exactly 0** and the window is a flat ±`paymentWindowMs`. Confirmed from the code, not the prose.

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
- **Networks — ✅ REGISTERED 2026-08-18.** `bsv:mainnet`, `bsv:testnet`, `bsv:ttn`, `bsv:tstn` are now registered CAIP-2 identifiers: [ChainAgnostic/namespaces#190](https://github.com/ChainAgnostic/namespaces/pull/190) ("Add bsv namespace (BSV Blockchain)", authored by sirdeggen, approved by bumblefudge + obstropolos, **merged 2026-08-18 17:23 UTC by obstropolos**, merge commit `9bac6ac`; andyrowe commented on it 2026-08-07). `bsv:mainnet` is no longer a *proposed* identifier. `bip122:<genesis-hash>` is **strictly wrong** for BSV, not merely ambiguous-by-convention — BSV shares the genesis hash byte-for-byte with BTC and BCH, so `bip122:000000000019d6689c085ae165831e93` cannot name the chain. Item 5 in §3 (refuse `bip122:*`) is now a spec-backed requirement and a registry-backed one. **Independent confirmation:** andyrowe switched bsv.cx production to emit `network: "bsv:mainnet"` on 2026-08-18 and reports it round-trips end to end (their settlement keys off the broadcast tx, not the network string).

---

## 6. Ecosystem position

| Product | Platform | Browser? | Wallet? | 402 | x402 |
|---|---|---|---|---|---|
| **Hodos** | Windows/macOS desktop | ✅ CEF native | ✅ Rust, in-process | ✅ BRC-121 | ❌ |
| `bsv-blockchain/bsv-browser` | iOS/Android | ✅ RN WebView | ✅ on-device | ✅ BRC-121 | ❌ |
| `bsv-blockchain/bsv-desktop` | Win/mac/Linux | ❌ | ✅ Electron, HTTPS on **:2121** | ❌ | ❌ |
| **bsv.cx** (andyrowe) | server-side service | ❌ | n/a (merchant side) | ❌ | ✅ **live on mainnet** — plain-P2PKH `exact` variant, `network:"bsv:mainnet"` since 2026-08-18 |

**Nobody ships the BRC-29/BRC-42 `exact` scheme on BSV as a client yet** — PR #2890 is the reference implementation and it is unmerged. **Correction to earlier wording:** x402-on-BSV *is* live in one form — bsv.cx runs the plain-P2PKH addressing variant (the "extension path" in the PR's reviewer note 2, server-issued single-use invoices, absolute `expiresAt`), so it is an independent implementer on a *different* addressing variant, not a competitor on ours. Hodos would be the first *client* of the BRC-29-derivation `exact` scheme. Profile: `Marston Enterprises/Hodos/Marketing/Profiles/bsv/andyrowe.md`.

### 6a. Cloudflare entered the demand side (2026-08-04) — why open question #3 is about demand, not the merge

Cloudflare announced [Cloudflare Wallets + cloudflare.pay](https://blog.cloudflare.com/wallets/) ([press release](https://www.cloudflare.com/press/press-releases/2026/cloudflare-gives-ai-agents-an-identity-and-a-wallet/)). **Live today: handle reservation only**; wallets, on/off-ramp and Virtual Wallets are "in the coming months," and the seller-side **Monetization Gateway is waitlist/preview**.

- **Custody: UNDISCLOSED.** Not stated in the blog or press release. Do not repeat "custodial" as fact — though account-linked balances plus geographic on/off-ramps strongly imply hosted custody (*inference, unverified*).
- **Chains for the wallet: undisclosed.** But [Cloudflare's x402 docs](https://developers.cloudflare.com/agents/x402/) cover Base, Ethereum, Polygon, Optimism, Arbitrum, Avalanche, Solana, Aptos, Stellar, Sui — **USDC settlement**, and *"`https://x402.org/facilitator` is the public facilitator operated by Coinbase and is used in all Cloudflare examples."*
- **No public wallet API** for third-party integration today.

**Why this matters to us more than the PR does.** x402 is asset-agnostic in spec and highly concentrated in practice (USDC / Base / Coinbase facilitator). BSV appears **nowhere** in [docs.x402.org's supported networks](https://docs.x402.org/core-concepts/network-and-token-support). If Cloudflare becomes how sites turn on 402, the accepted-asset list is effectively set upstream of us — and our payment path never fires, not for protocol reasons but because **no merchant advertises `bsv:mainnet`**. The threat is demand-side aggregation, not the wallet product.

Where Cloudflare does *not* compete: they shipped no browser, and their model is agent-first and custody-hosted. Our differentiators — user-present consent, the gold pill, the per-domain permission engine, non-custodial keys in-process — are orthogonal to it. For headless/server-side agents, however, a browser wallet is largely redundant in their model.

### 6a½. Brave (added 2026-08-22)

**BAT Roadmap 4.0 (2026-07-09)** commits Brave — 122M MAU — to first-class browser handling of HTTP 402: on a 402 response with an **x402 or MPP** (Machine Payments Protocol, agent payments) body, the browser pays for protected content. Rails are GENIUS-compliant stablecoins via BravePay (self-custody, `.brave` addresses); BAT survives via buybacks; Brave Search API to accept x402/MPP; wallet prototypes Fall 2026; experiments repo `brave-experiments/bx402`. Eich publicly: “all hail the HTTP 402 response code” (2026-08-19). **Read**: the browser-pays-402 architecture is now validated by the largest privacy-browser vendor — and the demand-side risk of §6a gains a browser-side twin: if Brave + Cloudflare normalize 402-with-stablecoin, the accepted-asset list consolidates around USDC-class assets from both ends. Our §6b differentiators are unaffected (BSV rails, BRC-29 derivation, per-domain permission engine, non-custodial in-process keys), but comparisons must target this roadmap, not 2024 Brave. Profile: `Marston Enterprises/Hodos/Marketing/Profiles/browser-tech/brave.md`.

### ⭐ 6a¾. Orthogonal — an operator's field report, and it validates our roadmap (added 2026-09-02)

Source: Christian Pickett (co-founder/CEO, ex-**payments at Coinbase**, ex-billing at Vercel),
*"What Onboarding 700+ x402-Enabled API Endpoints Are Teaching Us About Agent Commerce"*,
`orthogonal.com/blog/x402-agent-commerce-700-api-endpoints`. Orthogonal is YC W26, **$4.3M seed led
by Pantera**, aggregating paid APIs behind one integration for agents. Post banner is
**Orthogonal × Coinbase × x402**.

⭐ **This is the first operator-scale account of x402 in production we have seen**, and three of its
findings land directly on our open questions.

| Their finding | What it means for us |
|---|---|
| ⭐⭐ **Their stated next priorities are *"governance and spend controls, limits, self-service API onboarding, and a control plane so companies can manage agent capabilities with confidence."*** | **That is our auto-approve engine.** A Pantera-funded x402 company, at 700+ endpoints, says the missing layer is per-agent spend governance — the thing Hodos already ships and video 4 is about. ⭐ **Strongest external validation of the permission engine we have** |
| ⭐ **The friction they name is wallet management** — x402 became viable ~Jan 2026 when agents "could stand up and control their own wallets", but there was *"not an easy way to set up x402 at the time without managing your own wallet"* | **The obstacle they routed around by becoming an aggregator is the thing we ship.** A browser with an in-process wallet removes that setup step rather than intermediating it. ⚠️ Note the trade-off honestly: their answer scales to non-wallet-holders, ours does not |
| ⭐ **Refunds are unspecified.** *"Facilitators did not have support for managing refunds, so each API that implemented v1 had to manage its own refund policy"* | **A gap our §3 does not mention at all.** ⭐ And BRC-166's shape partly pre-empts it: the **payer does not broadcast — the origin does, after producing the resource** (§9b), so a failed fulfilment means no payment rather than a refund. Worth raising as a genuine BSV-side advantage, carefully |
| **Dynamic/request-based pricing** was *"not native to x402 v1, but came later in v2"* | ⭐ **Independent corroboration of the v1/v2 split** found in the 09-02 sweep. Two sources now |
| **No shared discovery surface** existed; they built natural-language search and request-time routing | Upstream is now building this too (#3309, `bazaar` discovery indexing). Not our layer, but it is where they say the value sits |
| Demand shape: **contact enrichment dominates** (ContactOut, Fiber AI, Tomba) | Agents went at the highest-friction manual workflows first. A demand signal for §8 q3, though not a BSV one |

⭐ **Their thesis in one line, and it is worth internalising:** *x402 made agent payments possible;
the value is in what sits above it — **discovery, pricing, and fulfilment**.* ⚠️ **That is an argument
that the protocol layer is not the differentiator** — which cuts against treating the #2890 merge as
important, and for treating our permission engine as the asset.

⛔ **Do not over-read it.** "700+ endpoints" is a **catalogue count, not traffic**; the one outcome
number given (*"87% of company updates surfaced were independently grounded, up from near zero"*) is
a single customer over two weeks. **Same caution as the bsv.cx read in §9 — live is not trafficked.**
⚠️ **And it is a Coinbase-lineage, stablecoin-rail account** — no BSV anywhere in it. Evidence for
x402 demand generally, **not** for our variant.

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

`CONTRIBUTING.md` mandates a **three-PR workflow** for a new chain: spec PR first → **merged** → reference implementation in a single SDK → additional SDKs. PR #2890 is one 43-file, +4,368-line PR carrying spec + implementation + examples + 121 tests together (103 at open; 121 after `bf4f969`).

**Status snapshot — 2026-08-19 (verified via GitHub API):** **37 issue comments, 0 review comments, 0 reviews, no requested reviewers, `mergeable_state: blocked`** (was `unstable` on 2026-08-08 — GitHub's `blocked` means branch protection is unsatisfied, usually required reviews and/or checks; *which* protection is not exposed by the API), **6 commits**, head `f8813af`, ~4.5 weeks open. Still **zero maintainer engagement** from the TSC orgs. The branch is being tended: sirdeggen rebased onto `upstream/main` (`fa6f8ad`) and adapted to upstream `SchemeNetworkServer`/`MoneyParser` API changes (`f8813af`) on 2026-08-19 — so the PR is not rotting, but nothing has moved on the review side. *Prior snapshot 2026-08-08: 31 comments, `unstable`, three weeks open.*

**Status snapshot — 2026-08-21 (verified via GitHub API):** 37 issue comments (none new since andyrowe's 2026-08-18 CAIP-2 comment), 0 review comments, **1 review — the PR's first: andyrowe APPROVED, 2026-08-19 23:50 UTC.** The review is implementer-grade: it confirms the `f8813af` adaptation tracks upstream cleanly (`defaultAssetTransferMethod = "default"` + the single `authorization` flow lines up with the other non-EVM mechanisms — near/aptos/hedera/keeta) and that widening the WhatsOnChain parser to `string | number` keeps the guards intact (empty string → 0 and non-numeric → NaN both still rejected). ⚠️ **andyrowe is not a maintainer** — this is peer review from the other live BSV implementer (bsv.cx, §5/§9 target B), not TSC engagement, which remains **zero**. **7 commits**, head `53a8fdf` (2026-08-21 merge of upstream/main; merge-only for BSV — §4a), diff unchanged at 43 files / +4,368 −7. Still `mergeable_state: blocked`, and one component of `blocked` is now identifiable: andyrowe's review notes the fork's CI sits at `action_required`, and on `53a8fdf` only `check-verified-commits` and `labeler` have run — **the unit/lint workflows have never executed on this PR** because fork PRs need maintainer approval to run workflows. So even the mechanical prerequisite to merging is gated on a maintainer touching the PR. Context datapoint: upstream merged a 1,530-line SVM batch-settlement scheme spec into `main` in this same window — the repo is actively absorbing non-EVM scheme work while #2890 waits, consistent with the structural-mismatch reading below.

**Status snapshot — 2026-08-24 (verified via GitHub API):** no movement on the review side. 37 issue comments (latest still 2026-08-18), 0 review comments, still exactly **1 review** — andyrowe's APPROVED of 2026-08-19 (above). **9 commits**, head `36c1fda` (2026-08-24), after `6cfa20a` (2026-08-22) — both merges of upstream/main, merge-only for BSV (§4a); diff unchanged at 43 files / +4,368 −7. Still `mergeable_state: blocked` (`mergeable: MERGEABLE`). On `36c1fda` only `labeler` and `check-verified-commits` have run (both pass); the unit/lint workflows have **still never executed** on this PR, and the Vercel deploy check fails pending Coinbase-team authorization — the maintainer-gated workflow approval identified on 2026-08-21 remains the mechanical blocker. TSC engagement remains **zero**. The branch is still being tended: sirdeggen merged upstream/main on 08-19, 08-21, 08-22, and 08-24.

**Status snapshot — 2026-08-25 (verified via GitHub API):** unchanged on the review side — 37 issue comments (latest still 2026-08-18), 0 review comments, still exactly **1 review** (andyrowe's APPROVED of 2026-08-19). `mergeable: MERGEABLE`, `mergeStateStatus: BLOCKED`, diff unchanged. **11 commits**, head `d14bff7` (2026-08-25 09:17 UTC): `410e8f9` merges upstream/main and `d14bff7` is `style(bsv): fix prettier formatting in constants test` — one line removed from `typescript/packages/mechanisms/bsv/test/unit/constants.test.ts`, described as a pre-existing formatting issue caught by lint. **No behavioural change to the BSV mechanism; §2/§4 wire-format and freshness findings carry over unchanged, and no action is required on our side.** Note the sequencing: the fix was authored against a lint rule the PR's own CI has still never run (the maintainer-gated workflow approval of 2026-08-21 remains the mechanical blocker), i.e. the author is pre-emptively clearing checks he cannot yet execute. Upstream `main` continues to absorb non-BSV scheme work in the same window (Sei stablecoins, SVM smart-wallet limits, Python payment flow, settlement-pending auto-recovery), consistent with the structural-mismatch reading above. TSC engagement remains **zero**.

**Related, not part of #2890 — a second BSV x402 surface (noted 2026-08-25):** Dylan Murray (BSV Association; GitHub `galt-tr`, X `@CrosTheRubicon`) maintains [`galt-tr/a2a-bsv`](https://github.com/galt-tr/a2a-bsv), *"BSV Payment Scheme for Google A2A x402 Protocol — Agent-to-Agent Micropayments via BRC-100."* Same BRC-100/BRC-29 substrate as #2890 and our BRC-121 implementation, aimed at Google's Agent-to-Agent protocol rather than Coinbase's x402 repo. Tracking only — no evaluation done, no contact made. Profile: `Hodos/Marketing/Profiles/bsv/dylan-murray.md`.

**Of the two open maintainer calls** (as framed by mrz1836 on 2026-08-09: network identifier, and the recipient-wallet facilitator model), **one is now closed from outside the PR:** the network identifier is settled by the CAIP-2 registration (§5). The facilitator-model call remains.

CONTRIBUTING also warns that contributions *"that show clear signs of unreviewed AI output... may be closed without detailed review."* The PR discloses AI assistance (correctly), but paired with a monolithic diff that is a risk factor. Most thread comments are content-free ecosystem cheerleading — **never add to that; only implementer-grade technical comments help** (see [[project_x402_brc121_ecosystem_2026_08_07]]).

**The §4a freshness fix is the natural small spec-only PR** that would fit the documented workflow.

### ⭐ Sweep 2026-09-02 — the PR is on life support while the spec moves under it

Verified via GitHub API. **Headline: sirdeggen has merged nothing. He has exactly one PR in the repo
(#2890) and it is still open.**

| | State at 2026-09-02 |
|---|---|
| **#2890** | OPEN, `mergeable=MERGEABLE`, `mergeStateStatus=`**`BLOCKED`**, head `9d43df6`, updated 09-02 |
| **Reviews** | ⛔ **Still 1** — andyrowe's, 2026-08-19. **Fourteen days, no second review; TSC engagement still zero** |
| **Comments** | 37 — unchanged |
| ⛔ **CI** | **Only three checks have ever run**: `check-verified-commits` ✅, `labeler` ✅, `Vercel` ❌ *"Authorization required to deploy"*. **Unit tests and lint have still never executed.** The fork-CI `action_required` block from the 08-21 sweep is unchanged |

**Commits since the last substantive one (`d14bff7`, 08-25, a prettier fix):** five, **all**
`Merge remote-tracking branch 'upstream/main'` — 08-26, 08-27, 08-28, 09-01, 09-02.

⭐ **Verified still merge-only for BSV.** `compare d14bff7...9d43df6` = **300 files changed, not one
matching `bsv`.** So §4a's freshness rule and every earlier code reading **carry over unchanged** —
open question 2 re-checked, no change.

#### ⚠️ But the surface around it is moving fast, and that is the new risk

The repo merged **15 PRs in the six days to 09-02** (phdargen, CarsonRoscoe, wnjoon, mintlify). Three
strands touch our future implementation:

| Landing upstream | Why it matters to us |
|---|---|
| ⭐ **`EXTENSION-RESPONSES`** (#3270 → #3278, #3306 py, #3301 go) | A **facilitator → resource-server sidechannel header**, deliberately **excluded from JSON serialization "so it cannot reach buyers through `PAYMENT-RESPONSE`"** — an architectural privacy boundary that did not exist when #2890 was written |
| ⭐ **builder-code `a` field attribution** (#3313 ts, #3302 go, docs #3315) | An app-attribution field now **validated on v2**, with distinct v1 behaviour. New required-field surface |
| **v1 / v2 split being enforced** | #3313 and #3315 both distinguish them explicitly. BRC-166 targets `x402Version 2` (§9b) |

⛔ **The risk, plainly: #2890 was written against an earlier wire surface, its author merges 300-file
upstream deltas into it weekly, and CI has never once run on the result.** Nobody — including him —
currently knows whether the BSV scheme still passes.

⭐ **Two consequences for us.** Keep reading from **the spec, not his branch**; and **do not treat a
future merge as evidence the code works** — on this PR, merging would prove only that a maintainer
approved it, never that a test ran.

⭐ **No change to the §10 decision.** Still do not implement. The gate is still a paying BRC-29
`exact` server, not a merge.

### Sweep 2026-09-14 — head `30a30e8`, plus who actually merges

Verified via GitHub API unless marked *inference*.

**The commit link is to the fork, shown through the upstream URL.** `github.com/x402-foundation/x402/commit/30a30e8…`
resolves because GitHub shares objects between a repo and its forks. The commit lives on
`bsv-blockchain/x402@feat/bsv-exact-scheme`. `compare main...30a30e8` on the foundation repo =
**ahead 26, behind 0** — nothing from #2890 is on foundation `main`.

| | State at 2026-09-14 |
|---|---|
| **#2890** | OPEN, `mergeable_state: blocked`, **26 commits**, head `30a30e8` (2026-09-12 09:07 UTC), `author_association: NONE` |
| **Reviews** | Still 1 — andyrowe, 2026-08-19. 26 days without a second review; TSC engagement still zero |
| **Comments** | 37 — none since 2026-08-18 |
| **CI** | Now visible as named runs: `Lint`, `Run Unit Tests`, `Format`, `Package Lock`, `Check Go`, `Check Python` all **`action_required`** (queued awaiting maintainer approval, never executed). `check-verified-commits` ✅, `labeler` ✅, `Vercel` ❌ "Authorization required to deploy" |

**What `30a30e8` is:** `Merge remote-tracking branch 'upstream/main'`. Its first-parent diff is six
upstream files — Go and Python `bazaar` facilitator route-template decode fixes and their changelog
entries. No BSV file.

**BSV-file changes since the 09-02 sweep (read via path-filtered commit history, because
`compare` caps at 300 files):**

- `92fc793` (sirdeggen, 09-03) — `docs(bsv): note CAIP-2 bsv namespace is registered`. Wording only
  in `scheme_exact_bsv.md`, the package README and a doc comment in `constants.ts`: "registration in
  progress" → "registered", plus a link to `bsv/caip2.md`. No normative change.
- `23fb0cb` (09-08) — `pnpm-lock.yaml` regenerated after an upstream merge (+2 −1).
- Nothing under `go/` or `python/` for BSV.
- `DEFAULT_PAYMENT_WINDOW_MS = 30_000` still in `constants.ts` at `30a30e8`; `facilitator/scheme.ts`
  untouched since `d14bff7`. **§4a freshness rule unchanged; item 7 stays blocked.**

**The daily merges are automated.** 17 of the 26 commits are authored and committed as `Claude
<noreply@anthropic.com>` (GitHub maps that email to the `claude` account), almost all at 09:07–09:20
UTC, one per day. sirdeggen's own two commits on 09-03 merge from a remote named `foundation/main`;
the automated ones merge from `upstream/main`. *Inference:* a scheduled Claude Code job on a
separate clone with push rights to the `bsv-blockchain` fork keeps the branch current; Deggen
steps in by hand for content changes. This matters for CONTRIBUTING's warning about "unreviewed AI
output" (§7): the PR history is now mostly bot merges, while its tests have never run.

#### Who can merge

- **Merges are done by `phdargen`.** Of the 150 most recent merged PRs (all after the 2026-07-14
  operational launch), phdargen merged 142 and CarsonRoscoe 8. Neither profile names an employer;
  do not guess one.
- **Deggen cannot merge.** #2890 is his only PR in the repo; his association is `NONE`. He pushes to
  the BSV Association fork and waits for a maintainer, like any outside contributor.
- `CODEOWNERS` has network-maintainer teams for `evm`, `svm`, `stellar`, `aptos` and `avm`, under a
  `core` team that owns `/specs/`. **There is no `bsv` team**, so a BSV spec defaults to `core`.
- The only public org member is `lgalabru` (Ludo Galabru, Solana Foundation). Private membership is
  not visible to us.
- `TSC.md` unchanged: Coinbase (Erik Reppel), Cloudflare (Rohin Lohe), Stripe (Steve Kaliski).

#### ⭐ The Cardano precedent confirms the three-PR reading

Cardano went through the documented workflow and landed. Spec-only **#1093 merged 2026-04-23**;
implementation **#2537** (fabianbormann, 99 files, +18,673, 108 commits) opened 2026-06-01, went
through repeated inline review from phdargen and a Cardano-side reviewer in late June, and **merged
2026-09-09 by phdargen**; docs (#3429) and SDK follow-ups (#3430) merged the same day. A large
implementation PR is acceptable. What #2890 lacks is a merged spec PR ahead of it.

#### ⚠️ Two older BSV spec PRs we had not recorded

| PR | Author | Opened | Shape | State |
|---|---|---|---|---|
| **#1844** `spec: add exact scheme for BSV network` | sgbett (Simon Bettison) | 2026-03-27 | Spec only, 1 file, `specs/schemes/exact/scheme_exact_bsv.md` — **the same path as #2890** — with a different design: no facilitator, the resource server broadcasts to ARC | Open, idle since 2026-04-02 |
| **#1004** `spec: Add draft TXID payment payload specification` | alftom | 2026-01-21 | Spec draft, 2 files | Open, idle since 2026-04-27 |

Read so far: titles, descriptions, file lists only — neither spec body has been read. Consequence:
a maintainer turning to BSV will find three open proposals, two of which write the same file with
incompatible settlement models. *Inference:* that is another reason a maintainer would leave BSV
alone until the BSV side presents one spec.

#### Repository provenance — it is the Linux Foundation's x402 Foundation repo

- Repo created **2025-02-21**; first commit `e01a090` by erik, **2025-02-16**. It is the original
  Coinbase repo, moved: the `x402-foundation` org was created **2026-04-01**, and `coinbase/x402`
  is now a **fork of** `x402-foundation/x402` created 2026-04-02, the day of the LF announcement.
- Chain of links: the LF operational-launch press release points to `x402.org`; `docs.x402.org`
  links to `github.com/x402-foundation/x402`; the repo's homepage is `x402.org`. The GitHub org
  itself is not domain-verified, and the LF press release does not link the repo directly.
- ~6,600 stars, ~2,000 forks, 1,218 commits on `main`.

**No change to the §10 decision.**

---

## 8. Open questions

1. Does PR #2890 merge, and in what shape? **Narrowed 2026-08-19.** The network-identifier question is no longer part of this — CAIP-2 `bsv` is registered (§5), so the only remaining *technical* maintainer call is the **recipient-wallet facilitator model** (PR reviewer note 2: the facilitator is the merchant's own BRC-100 wallet rather than a third-party service). Structural mismatch with the 3-PR workflow (§7) remains the leading explanation for the silence; the 2026-08-19 rebase shows the author is keeping it mergeable, so "abandoned" is not the explanation. **2026-08-21:** the review-side silence is no longer total — andyrowe approved (the PR's first review, §7) — but that is peer review, not the maintainer call this question turns on; don't over-read it.
2. Does the freshness rule adopt server-issued `expiresAt` or the past-side-at-verify extension (§4a/§4c)? **This is the live one** — it determines whether item 7 in §3 is work at all. **Re-checked 2026-08-19: no change at head `f8813af`; re-checked 2026-08-21: `53a8fdf` is merge-only, no BSV files (§4a), so no change; re-checked 2026-08-24: `6cfa20a`/`36c1fda` likewise merge-only (§4a), so no change; re-checked 2026-09-14 at `30a30e8`: only wording changes to BSV files, `DEFAULT_PAYMENT_WINDOW_MS` still 30 000 (§7), so no change.**
3. Do we implement the adapter speculatively, or wait for merge? **Decided 2026-08-19 — see §10.** Nothing upstream *gates* us: Hodos is a C++/Rust client that doesn't consume the TS SDK, so we could emit a conforming `PAYMENT-SIGNATURE` today. The demand gate has **partly** moved: bsv.cx now advertises `bsv:mainnet` in production (plain-P2PKH variant) — so there is a live server that exercises the network identifier and amount exactness, though not the BRC-29 `exact` payload (§9). A merge alone still won't unblock us; a paying BRC-29 `exact` server would, and until then Deggen's example server from the PR branch is the only one.
4. Should we propose header-name alignment to BRC-121 itself, or let x402 supersede it? Probably the latter.
5. ~~Governance~~ — **resolved, see §7.**

---

## 9. Test matrix for the eventual implementation (written 2026-08-19)

Three targets exist. None of them covers everything; together they cover every build item except 7 (blocked anyway).

| Target | What it is | Exercises §3 items | Does **not** exercise | Pass criterion |
|---|---|---|---|---|
| **A. `now.bsvblockchain.tech`** (live, BSVA) | Plain **BRC-121**: `x-bsv-sats` / `x-bsv-server` headers, no JSON requirements, **no `network` field** | **None of 1–7 positively.** This is the **non-regression gate**: item 1's detection must *not* fire on the header form, and the existing five-header retry must be byte-identical to today | everything x402 | Existing BRC-121 flow unchanged: same headers out, same paid page back, `PaidContentCache` hit on reload. Any x402 code path touched = fail |
| **B. `bsv.cx`** (live, andyrowe, mainnet) | x402 **`exact` on BSV, plain-P2PKH variant** — JSON `PaymentRequirements` with `network:"bsv:mainnet"`, `asset:"BSV"`, server-issued single-use invoice, absolute `expiresAt`; pays to a P2PKH **address**, not a BRC-29-derived key | **1** (x402-shaped 402 detection), **2** (parse `accepted{}`), **5** (network/asset acceptance — `bsv:mainnet` accepted, `bip122:*` refused; this is the only live target where `network` is present), **4** (amount exactness — andyrowe states the amount is pinned at issuance; whether over-payment is *rejected* is unverified by us and is exactly what this target should establish). ⚠️ Requires a **P2PKH-address `payTo` branch** that is *not* in §3 today; if we don't build that branch, B tests 1/2/5 and then stops with a clean "unsupported addressing variant" refusal — which is itself a useful test | **3** (our `PaymentPayload` is the BRC-29 shape; bsv.cx doesn't consume `derivationPrefix`/`derivationSuffix`), **6** only partially (their settlement response shape must be checked — don't assume it is the spec's `SettlementResponse`), **7** | Either: (i) challenge parsed, network accepted, then explicit refusal on P2PKH `payTo` with a user-visible reason — no payment minted; or (ii) if the P2PKH branch is built: exact-amount payment accepted, resource returned, and a deliberately over-paid request **rejected** by the server (that is the behavioural check for item 4) |
| **C. Deggen's example server, run locally** from the PR branch: `examples/typescript/servers/advanced/all_networks.ts` + `examples/typescript/facilitator/advanced/all_networks.ts` (package `@x402/bsv`, `typescript/packages/mechanisms/bsv`) | The **full BRC-29 `exact` scheme** — the only target that consumes our actual payload | **1, 2, 3, 5, 6** fully; **4** (facilitator rule 6 enforces equality — over-payment is rejected at `/verify`); **7** *observably* (set a reuse within 25 s and watch for `invalid_exact_bsv_payload_timestamp_out_of_window` — but **do not tune against it**, per §4a) | real-world demand; a remote facilitator's clock (local run = zero skew, so 7's failure mode is *masked* unless you skew the clock deliberately) | `/verify` → `/settle` → `SettlementResponse{success:true, payer:<our identity key>, transaction:<txid>, network:"bsv:mainnet"}`; activity log shows the settlement; `bip122:000000000019d6689c085ae165831e93` challenge refused without minting; over-payment refused by server |

Order of execution when the time comes: **A first** (prove nothing broke), **C second** (prove the scheme works end to end with zero skew), **B last** (prove the network identifier and exactness against someone else's production code). Run C twice — once with system clock true, once with it deliberately +35 s — and record the second result in §4 rather than fixing it.

Things to keep in mind for target B specifically: **as of 2026-08-26 the wire format is no longer unread** — it is specified in BRC-166, read below. The earlier caveat ("andyrowe's comments describe the architecture, not the bytes") is retired.

#### §9b. BRC-166 read in full — 2026-08-26 (PR #231, `payments/0166.md` @ `38d7e6e`, 555 lines)

andyrowe's **BRC-166: P2PKH Payments for HTTP 402** specifies exactly the bsv.cx profile. It states its own place relative to #2890 up front: #2890 "should land first"; this is the plain-P2PKH addressing mode #2890's Facilitator Deployment appendix explicitly places **out of scope** ("supporting third-party facilitators without recipient wallets would require either a different addressing mode (e.g. plain P2PKH to a static address in `payTo`) …"). Networks aligned to `bsv:mainnet` per CAIP-2 #190; `bip122` "MUST NOT be emitted" — same conclusion as our §5, independently.

**Wire format (fills the gaps §9 target B listed as unread):**

* **Challenge:** status `402` + `PAYMENT-REQUIRED: base64(JSON(PaymentRequired))`, `x402Version: 2`, `accepts[]` — the same envelope as #2890.
* **`PaymentRequirements`:** `scheme:"exact"`, `network:"bsv:mainnet"`, `amount` (satoshis, decimal **string**), `asset:"BSV"`, `payTo` (P2PKH **address**), `maxTimeoutSeconds`.
* **`extra` — REQUIRED:** `chain`, `unit`, `invoiceId`, **`lockingScriptHex`** (the exact script the origin matches — publishing the script not just the address removes address-encoding as a failure mode), `expiresAt` (RFC 3339), and **`payloadFormat.kind` which MUST be `"p2pkh-rawtx"`**. OPTIONAL: `perCallSats`, `submitUrl`.
* ⭐ **`payloadFormat.kind` is the discriminator.** The spec names it as "the discriminator that distinguishes this profile from the BRC-29/BRC-42 `exact` mode of #2890 … a payer keys its choice of payload off this field." **This is the branch condition our §3 P2PKH work needs** — we no longer have to guess how a client tells the two `exact` modes apart. Both are `scheme:"exact"`; the mode lives in `extra.payloadFormat.kind`.
* **Payment:** retransmit the identical request with `PAYMENT-SIGNATURE: base64(JSON(PaymentPayload))` — **same header as #2890**. Payload: `{x402Version:2, accepted:{…}, payload:{lockingScriptHex, rawtx}}` where `rawtx` is a hex-encoded **fully signed** transaction. Origins MUST accept `lockingScriptHex` from either `payload` or `accepted.extra` and MUST compare **case-insensitively**.
* **The payer SHOULD NOT broadcast** — the origin broadcasts, after producing the resource (§5.8, "produce-then-broadcast", so a handler failure means the payer was never charged).
* **Settlement response:** `PAYMENT-RESPONSE: base64({success:true, transaction:"<txid>", network, extensions:{status:"mempool"}})`. **No `extra`, no token — the scheme is credential-less, pay-per-call.** Failure emits `{success:false, network, errorReason}` alongside the error status.
* **Authorization is on mempool acceptance (0-conf)**, declared in `extensions.status` so the risk is visible up front.

**✅ Resolves the open question §9 target B was created to answer.** We had "whether over-payment is *rejected* is unverified by us." **BRC-166 §5.6 step 6: "Overpayment MUST be accepted."** Outputs to `lockingScriptHex` must sum to **at least** `amount`; *under*-payment is `402 insufficient_funds` reporting both figures. So the behavioural check we planned for target B is answered by the spec — amount exactness is a floor, not an equality. Our §3 item 4 (amount exactness) must not assume equality against this target.

**Other normative points worth carrying into any implementation:** settlement is **idempotent per invoice** (an already-settled invoice re-serves the resource rather than charging twice — "agents retry by default"); resource-match is checked **before** the idempotent-replay shortcut, so a cheap invoice cannot be presented at an expensive endpoint; a fresh address per invoice is REQUIRED and address reuse is forbidden (this is what makes attribution work with no identity layer); "already known to mempool" on broadcast MUST be treated as success; concurrent settlements MUST be serialized.

**Nothing here changes our §10 decision or our BRC-29 payload shape** — BRC-166 is the *other* addressing mode. It changes two things only: the P2PKH branch is now specifiable from a document instead of reverse-engineered, and the over-payment question is closed.

---

## 10. Decision record — 2026-08-19 (do not implement yet)

**Decision: build items 1, 2, 3, 5, 6 on a `beta.4` feature branch (or a branch started now and merged when PR state is clearer). Hold 4 and 7. This is NOT a beta.3 tail item.**

| Item | Decision | Reason |
|---|---|---|
| 1 Detect x402-shaped 402 | **Build (beta.4 branch)** | Additive; spec shape stable across all 6 PR commits; B and C both exercise it |
| 2 Parse `accepted{}` | **Build** | Same |
| 3 Serialize `PaymentPayload` / `PAYMENT-SIGNATURE` | **Build** | Field names unchanged since `ecb9235`; byte-compatible derivation already proven (§2d); only C exercises it |
| 5 Refuse non-`bsv:*`, `bip122:*`, `asset != BSV` | **Build** | Now registry-backed (§5) *and* spec-backed (rule 1 after `1dad246`, rule 3, and `bf4f969`). Zero risk of reversal |
| 6 Parse `SettlementResponse` | **Build** | Read-only / UI; shape stable |
| **4 Strict amount equality** | **HOLD — decide deliberately** | It is a **behavioural change for BRC-121 users**: BRC-121 tolerates overpayment and `pay_402` has never been required to be exact (the 1000-sat Hodos fee is a separate output, which *should* leave the payment output exact — unconfirmed). Enforcing exact equality is correct for x402 and harmless for BRC-121 *only if* our payment output is already exact — §5 says "verify first" and nobody has. Decide with data from target A, not by assertion. If the payment output is already exact, item 4 is a no-op assertion and can join the branch; if not, it needs its own review |
| **7 Reuse-cache vs freshness** | **HOLD — blocked on spec** | §4a. Rule unchanged at `f8813af`. Any change now tunes against a rule under challenge |

**Why not beta.3:** beta.3 is the shipping build. Landing a new payment path there with **no paying BRC-29 `exact` server in the wild to validate against** (bsv.cx is the other addressing variant; C is a local example) would ship untested-in-production code on the money path. The branch can be kept green against C; it merges to a release when either PR #2890 merges or a real server advertises the BRC-29 scheme.

**Branch hygiene:** gate the whole adapter behind a build-time or runtime flag so A (non-regression) can be run with the adapter compiled in but disabled. Never mint before `dispatch_payment` approval (§4, human-in-the-loop note) — that ordering carries over unchanged.

---

## 11. Draft PR comment — post ONLY after the implementation round-trips (do not post now)

Per §7: implementer-grade only; no cheerleading; no statement of intent. Model is andyrowe's register (data point, not proposal). Fill every `<…>` from a real run; if a cell can't be filled from a run, delete the sentence rather than soften it.

> Implementer data point, same register as the earlier one from us: Hodos (desktop browser, native C++/Rust BRC-100 wallet, in-process) now emits the BRC-29 `exact` payload from this spec alongside its existing BRC-121 header flow.
>
> - Detects the JSON `PaymentRequirements` form and the `x-bsv-sats` header form on the same 402 path; the BRC-121 path is unchanged (non-regression against `now.bsvblockchain.tech`).
> - Accepts `bsv:mainnet` / `bsv:testnet` / `bsv:ttn` / `bsv:tstn` per the registered CAIP-2 namespace (ChainAgnostic/namespaces#190); **refuses `bip122:*`** (including the shared-genesis `bip122:000000000019d6689c085ae165831e93`) and `asset != "BSV"` without minting.
> - Round-trips against `<target: the PR branch example server + facilitator at commit <sha>, locally / bsv.cx>`: `/verify` → `/settle` → `SettlementResponse{success:true}`; payer identity key and txid match ours. `<N>` payments, `<0>` failures. `<If tested against bsv.cx: state explicitly that this was the plain-P2PKH variant and what was/wasn't exercised.>`
> - Payload derivation is the same byte-compatible one reported on 2026-08-07 (8-byte base64 prefix, base64(utf8(ms)) suffix, invoice `2-3241645161d8-{prefix} {suffix}`); no change was needed on the crypto side to go from BRC-121 to this scheme — it is serialization only.
> - Freshness: `<one sentence of fact only, e.g. "with the local facilitator clock deliberately +35 s the verify step returned invalid_exact_bsv_payload_timestamp_out_of_window as expected; we have not changed our 25 s reuse TTL pending the open question above.">` — *delete this bullet if the test wasn't run.*
>
> Nothing here is a proposal; confirming the spec as written is implementable from a non-TS client and that the registered identifiers work end to end.

**Checks before posting:** (1) every number came from a logged run; (2) no "we plan to"; (3) no "+1"; (4) no mention of Hodos features unrelated to the payload; (5) re-read §7 — if a maintainer has engaged the thread since, adjust tone to answer what they asked, not to add volume.

---

## References

- Spec: [`specs/schemes/exact/scheme_exact_bsv.md`](https://github.com/bsv-blockchain/x402/blob/feat/bsv-exact-scheme/specs/schemes/exact/scheme_exact_bsv.md) (PR branch)
- [BRC-121 Simple 402 Payments](https://bsv.brc.dev/payments/0121) · [BRC-29](https://bsv.brc.dev/payments/0029) · [BRC-42](https://bsv.brc.dev/key-derivation/0042) · [BRC-62 BEEF](https://bsv.brc.dev/transactions/0062) · [BRC-95 Atomic BEEF](https://bsv.brc.dev/transactions/0095)
- Our demo + live test target: `demos/brc121-402/README.md`, `https://now.bsvblockchain.tech`
- [ChainAgnostic/namespaces#190 — Add bsv namespace](https://github.com/ChainAgnostic/namespaces/pull/190) — **merged 2026-08-18**, `9bac6ac` · [andyrowe's confirmation comment on #2890](https://github.com/x402-foundation/x402/pull/2890#issuecomment-5331880396) (2026-08-18) · bsv.cx (live plain-P2PKH `exact` on BSV; not yet read at the wire level by us)
- PR branch example server/facilitator for §9 target C: `examples/typescript/{servers,facilitator,clients}/advanced/all_networks.ts` at `f8813af` (unchanged at `53a8fdf`, `36c1fda`)
- [x402.org](https://x402.org) · [x402-foundation/x402](https://github.com/x402-foundation/x402)
