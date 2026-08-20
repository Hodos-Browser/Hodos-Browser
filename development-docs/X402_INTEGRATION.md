# x402 Integration — what we have, what's missing, what it costs

> **Created:** 2026-08-07 · **Updated:** 2026-08-19 (CAIP-2 `bsv` namespace **registered** §5; PR status re-verified at head `f8813af` §7; freshness rule re-checked — still 30s symmetric at `/verify`, §4a/§4c; test matrix §9; decision record §10; draft comment §11). Previous: 2026-08-08 (governance §7; facilitator code read at `9808154` §4c; Cloudflare §6a)
> **Status:** Research complete. **Decision recorded (§10): items 1,2,3,5,6 → a `beta.4`/feature branch; hold 4 and 7. Not a beta.3 tail item.** No code written. **Item 7 of §3 remains blocked on spec — do not "fix" it.**
> **Context:** [x402-foundation/x402 PR #2890](https://github.com/x402-foundation/x402/pull/2890) — `feat(bsv): add exact scheme support for BSV`, by sirdeggen (Deggen), **open**, 43 files / +4,368 −7 lines, 6 commits, head `f8813af` (2026-08-19).
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

**Re-checked 2026-08-19 at head `f8813af` — the rule has NOT changed.** Commit history of the PR branch: `ecb9235` (initial) → `1dad246` "harden exact scheme verification and replay defense" (2026-07-21) → `bf4f969` (ttn/tstn + bip122 refusal) → `9808154` (merge main; the head §4c was read at) → `fa6f8ad` (merge upstream/main, 2026-08-19) → `f8813af` (adapt to upstream `SchemeNetworkServer`/`MoneyParser` API changes, 2026-08-19). `1dad246` **predates** the §4c read and its spec diff touched rule 1 (prefix ≥ 8 bytes, `asset` must be `BSV`) and the replay-protection prose (dedup record ≥ `paymentWindow + maxTimeoutSeconds`, sticky-session note) — **rule 4 itself is unchanged**: 30 s symmetric, past side extended by `maxTimeoutSeconds` only at settlement. The two 2026-08-19 commits touch `server/scheme.ts` and `moneyParser.ts` only — no facilitator freshness change. `DEFAULT_PAYMENT_WINDOW_MS = 30_000` still in `constants.ts`. **Item 7 stays blocked.** Neither the past-side-at-verify extension nor `expiresAt` has been adopted, and no maintainer has engaged the thread on it.

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

**Of the two open maintainer calls** (as framed by mrz1836 on 2026-08-09: network identifier, and the recipient-wallet facilitator model), **one is now closed from outside the PR:** the network identifier is settled by the CAIP-2 registration (§5). The facilitator-model call remains.

CONTRIBUTING also warns that contributions *"that show clear signs of unreviewed AI output... may be closed without detailed review."* The PR discloses AI assistance (correctly), but paired with a monolithic diff that is a risk factor. Most thread comments are content-free ecosystem cheerleading — **never add to that; only implementer-grade technical comments help** (see [[project_x402_brc121_ecosystem_2026_08_07]]).

**The §4a freshness fix is the natural small spec-only PR** that would fit the documented workflow.

---

## 8. Open questions

1. Does PR #2890 merge, and in what shape? **Narrowed 2026-08-19.** The network-identifier question is no longer part of this — CAIP-2 `bsv` is registered (§5), so the only remaining *technical* maintainer call is the **recipient-wallet facilitator model** (PR reviewer note 2: the facilitator is the merchant's own BRC-100 wallet rather than a third-party service). Structural mismatch with the 3-PR workflow (§7) remains the leading explanation for the silence; the 2026-08-19 rebase shows the author is keeping it mergeable, so "abandoned" is not the explanation.
2. Does the freshness rule adopt server-issued `expiresAt` or the past-side-at-verify extension (§4a/§4c)? **This is the live one** — it determines whether item 7 in §3 is work at all. **Re-checked 2026-08-19: no change at head `f8813af`.**
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

Things to keep in mind for target B specifically: bsv.cx's wire details (exact `PaymentRequirements` extension fields, settlement response shape, whether `PAYMENT-SIGNATURE` is the header they read) have **not been read by us** — andyrowe's comments describe the architecture, not the bytes. Verify against their endpoint or ask before asserting compatibility. That question ("happy to compare notes on the addressing-variant side") is one he explicitly offered.

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
- PR branch example server/facilitator for §9 target C: `examples/typescript/{servers,facilitator,clients}/advanced/all_networks.ts` at `f8813af`
- [x402.org](https://x402.org) · [x402-foundation/x402](https://github.com/x402-foundation/x402)
