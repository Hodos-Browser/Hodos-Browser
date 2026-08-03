# Review: OpNS (On-chain Name System) + 1sat.name

**Reviewed:** 2026-07-15
**Author of OpNS:** David Case (@shruggr) / b-open-io / GorillaPool ecosystem
**Prompted by:** networking meeting with shruggr (2026-07-14) + the question "does this solve our decentralized-naming problem?"
**Method:** two independent research passes — one code-level (cloned + read the `op-ns` sCrypt contract, the Go indexer, and the `opns-overlay` UI), one product/adoption-level (queried the live 1sat.name + `mine.shruggr.cloud` + `api.1sat.app` APIs directly). Where the two passes disagreed, the conflict is called out below.

> **Confidence tags:** `[CODE]` = verified by reading contract/indexer/SDK source · `[LIVE]` = measured against a live API today · `[SPEC]` = from the OP-standard gitbook · `[INFERRED]` = our synthesis · `[UNVERIFIED]` = could not confirm.

---

## TL;DR

OpNS is **the strongest on-chain, human-readable, self-sovereign, *payable* naming primitive that exists on BSV today**, and it fills the exact gap our `Xanaverse-Contracts-Review/REVIEW.md` flagged: Xanaverse's covenant gives you unique *numbers*; OpNS gives you unique *human-readable names* with the **same consensus-level uniqueness guarantee**. It's shruggr's work, so adopting it aligns with our "be the client + ecosystem partner, not the operator" stance.

**The headline finding (a correction to our first prior):** name-mint uniqueness is **enforced by Bitcoin Script / consensus via a stateful sCrypt covenant** — *not* by indexer goodwill. A duplicate claim is an invalid transaction, the same way a double-spend is invalid. This is **stronger than our own Open Paymail design** (which relies on "first-seen-in-a-block, indexer-enforced").

**But** the real-world state tempers everything downstream of that:
- Registration today runs through a **centralized, closed-source paid miner** (`mine.shruggr.cloud`), flat **0.25 BSV (~$3)** per name regardless of length. `[LIVE]`
- **Adoption is essentially zero** — even `shruggr`, `1sat`, `bitcoin`, `satoshi` are unclaimed in the live mine tree. `[LIVE]`
- The marketed "fair-mint PoW anti-squatting" is, in practice, **capital-bound not compute-bound** (buy names at $3 each in a loop). `[INFERRED]`
- Ownership/identity resolution *after* the mint reverts to ordinary indexer-convention trust (ORDFS).

**Verdict:** genuinely well-architected primitive; **essentially pre-adoption**. Posture → **engage, prototype (read-only), and watch** (detail in `README.md`). Don't build our own; don't depend on OpNS as a shipped naming layer yet.

---

## How OpNS actually works

### The mine tree (char-by-char growth) `[CODE]`

A name is not "registered" in a database — it is **grown one character at a time** as a chain of Bitcoin transactions. Each character is a transaction that spends a 1-satoshi stateful-covenant UTXO (a node in a "mine tree") and produces three new 1-sat outputs:

| Output | Role |
|---|---|
| 0 — `selfOutput` | same prefix node, with the claimed-character bitmap updated (this char now taken) |
| 1 — `spawnOutput` | a NEW child node for `prefix + char`, fresh empty bitmap |
| 2 — `tokenOutput` | the actual 1-sat **ordinal** inscription: content = `prefix+char` string, content-type `application/op-ns`, locked to your key |

Registering `cat` = **three chained transactions** (`"" → c → ca → cat`), each spending the `spawnOutput` of the prior step. Only the final step's `tokenOutput` is "the name" you keep; intermediate partial-name ordinals (`c`, `ca`) are minted as a side effect and usually discarded.

- **Genesis UTXO:** `58b7558ea379f24266c7e2f5fe321992ad9a724fd7a87423ba412677179ccb25:0` — hard-coded in the indexer. Every registration descends, transitively, from this one UTXO.
- The inscription embeds a 33-byte tag + the 36-byte genesis outpoint, so the indexer can distinguish "real" OpNS inscriptions from lookalikes with the same content-type.

### Proof-of-work — what it's actually for `[CODE]` `[SPEC]`

Each character claim requires solving a chained Hashcash puzzle:

```
hash = SHA256d( prevPow(32) || char(1) || nonce(32) )
valid iff top DIFFICULTY bits of reversed(hash) == 0
```

- `DIFFICULTY = 22` — **hard-coded constant**, not scaled by name length or demand. ≈ 4.19M hashes/char average — sub-second on a laptop. PoW cost is **linear** in name length.
- `prevPow` chains each char's proof to the entire prior mining history of that tree path, so you can't precompute.
- **PoW is the anti-squatting / rate-limiter, NOT the uniqueness mechanism.** Its only job is to make it "impossible to instantly claim the entire domain space" `[SPEC]`. The spec frames it as re-introducing Bitcoin-style fair-mint energy cost (which BSV's near-zero fees otherwise remove — the "unfair pre-mint" problem it cites for BSV21 tokens).

### Uniqueness — enforced by consensus `[CODE]` — the crux

Inside the covenant's `mint()`:
```
validateChar(char):
  mask = 1 << char
  assert( (mask & this.claimed) == 0, 'char already claimed' )   // consensus-enforced
```
plus an output-commitment check (`hash256(outputs) == ctx.hashOutputs`) that forces the transaction's outputs to exactly match the required self/spawn/token structure. **Any tampering makes the unlocking script fail Bitcoin script verification — the transaction is invalid and unminable, exactly like a double-spend.** This is genuine Script/consensus enforcement evaluated by every validating node, *not* an indexer convention.

**Collision resolution** `[CODE]`: it's a pure UTXO double-spend — two people racing the same prefix are trying to spend the *same* covenant UTXO; whichever transaction a miner confirms wins, the other becomes invalid. **No tiebreak rule is needed** (not "highest PoW", not "lowest height/tx-index"). Solving the PoW off-chain is necessary but not sufficient — you still have to win the ordinary Bitcoin confirmation race.

### The name after mint: identity binding + resolution `[CODE]` `[SPEC]`

- The finished name is a **normal 1-sat ordinal you own** (transferable, sellable like any ordinal — the covenant's job ends at the mint).
- You bind an identity to it via **MAP metadata** `opns.idKey` on a self-transfer. This binding is **convention, not consensus** (a plain ordinal transfer with metadata, interpreted by ORDFS + the resolver). It accepts: a BRC-100 identity **pubkey** (→ one-shot BRC-29 derived addresses, MessageBox-capable), a **P2PKH address** (fixed, legacy), or **empty string** (clears binding).
- **Paymail resolution:** `name@domain` → OpNS overlay lookup for the name's origin → **ORDFS** forward-crawl to the current outpoint → read latest `opns.idKey` → BRC-29 derive a destination. **Resolution is handled by ORDFS, not the overlay** `[CODE]`. Any operator running the reference `opns-overlay` server automatically becomes a paymail domain for every claimed name.

---

## Scalability `[CODE]`

**No *global* singleton bottleneck** like Xanaverse's single-registry-UTXO (which caps ALL registrations at ~6/block). Disjoint names mint fully in parallel.

**But there IS a scoped bottleneck:**
- The **root UTXO serializes the first character of every name** — only 37 possible first characters (`a-z`, `0-9`, `-`), claimed one at a time off the genesis chain, until the root is exhausted and splits into ≤37 independent subtrees.
- Beyond depth 1, contention is scoped strictly to **shared prefixes**: `cat` and `dog` never contend after their first diverging character; `cat` and `car` contend only over the `ca` node's next-char claim.
- **Each name = N sequential transactions** (one per character) — heavier on-chain footprint and latency than a single-transaction PushDrop registration (our Open Paymail model).

**Indexer side scales fine:** overlay admission is O(1) per tx (it trusts consensus-valid ingestion rather than re-running the covenant), lookups are O(name length). Cold-start genesis crawl is O(all mints ever) via JungleBus, parallelized; live sync is delta-pull.

**Concrete scale number** `[CODE]`: as of 2026-07-10 the production overlay index held ~42,157 outputs — but that **mixes mine-tree nodes with actual name ordinals**, so it overstates the real name count. There is no published throughput benchmark.

---

## Cost & adoption reality `[LIVE]` (measured 2026-07-15)

- **Cost:** `GET mine.shruggr.cloud/price?name=<anything>` returned identical `25000000 sats` (**0.25 BSV, ~$3**) for every name tested — `a`, `bitcoin`, `zzzzzzzzzz`, empty string. Flat fee, any length.
- **You don't mine it yourself.** The browser does **not** grind PoW in the shipped product; mining is delegated to a **separate, closed-source, paid orchestrator** (`go-opns-mint` / `mine.shruggr.cloud`, BRC-103 authenticated). A client-side/WASM miner is noted as "later, not shipped."
- **Adoption is near-zero.** Probing the live overlay (`api.1sat.app/1sat/opns/mine/:name`): only `bo` (prefix of `bopen`) was found. `bitcoin`, `satoshi`, `alice`, `bob`, `shruggr`, `1sat`, `opns`, single chars `a`/`b`/`s` — **all "No outpoint found."** No public dashboard/leaderboard. The sole contract repo (`op-enheimer/op-ns`) has 7 stars, 0 forks, 0 issues, no third-party builders found.
- **X/Twitter sentiment is a blind spot** — no X API access in the research passes, so "no public critiques found" is *not* a clean bill of health.

---

## Weaknesses & open questions

1. **Everything downstream of the mint is indexer-convention trust** `[CODE]`. Uniqueness at birth is consensus-enforced, but *current ownership* (after transfers) and *what key a name resolves to* depend on ORDFS/the overlay correctly tracking spend-chains + MAP metadata. The reference implementation's own notes document live "stale node" drift between operators.
2. **Anti-squatting is currently capital-bound, not compute-bound** `[INFERRED]`. The spec sells PoW as fair-mint energy cost, but because mining is a flat paid service, a funded actor can buy many names at ~$3 each. No expiry, no auction, no graduated pricing in the open protocol.
3. **Resale identity-binding — the one place our two research passes conflicted, reconciled:**
   - Pass A (protocol docs): "binding follows the name, not the person" — payments keep flowing to the seller until re-registration; resolver behavior here is "undecided."
   - Pass B (`@1sat/actions` SDK source): `opnsList` and `opnsTransfer` **automatically clear the `opns.idKey` binding** on sale/transfer.
   - **Reconciled read:** safe on the *happy path* (official SDK marketplace/transfer functions clear the binding); the footgun is a **raw ordinal transfer that bypasses the SDK helpers**, where the resolver's fallback is the genuinely-undecided part. → confirm with shruggr.
4. **"Which genesis tree is canonical" is convention, not consensus** `[INFERRED]`. Someone could deploy a rival identical contract with a different genesis outpoint; only ecosystem/indexer agreement makes the real one real. Uniqueness is airtight *within* a tree.
5. **Names never expire** `[SPEC]` — permanent claims, no recycling path for abandoned/squatted names.
6. **Centralized, closed-source, unaudited miner** is the practical registration path today; true P2P overlay peering is explicitly **deferred** (static leader model).
7. **Spec is thin and actively reshaping** — the gitbook omits the difficulty constant, hash construction, and collision semantics (only the source has them); docs.1satordinals.com is mid-rewrite.

---

## How OpNS maps to our framework (`README.md`)

| Our 4 required properties | OpNS |
|---|---|
| Human-readable | ✅ real strings (beats Xanaverse's numbers) |
| Unique | ✅ **consensus/covenant-enforced at mint** (strongest rung) |
| Non-fungible | ✅ it's a 1-sat ordinal — can't be duplicated |
| Self-sovereign | ✅ you hold the key/UTXO |

| Our 3 uniqueness approaches | Where OpNS lands |
|---|---|
| Centralized registry (ORDnet `.web3`) | ❌ not this — ORDnet's name→TXID map is a private DB |
| **sCrypt covenant (Xanaverse)** | ✅ **OpNS's family** — but for *names* not numbers, and a fan-out tree instead of one choke-point registry |
| Federated overlay + micropayments (our Open Paymail lean) | Partial — *post-mint* resolution/ownership still rides on overlay/ORDFS trust |

**Net:** OpNS ≈ "Xanaverse's on-chain-uniqueness strength, applied to human-readable names" — the thing our own review said nobody had built.

## Comparison to adjacent naming systems

Via **Zooko's Triangle** (decentralized / secure / human-readable — historically "pick two"; CoinGeek's BSV-naming survey uses this framing and *doesn't even mention OpNS yet*):

| System | Decentralized | Chain-enforced-secure | Human-readable | Note |
|---|---|---|---|---|
| BSV **Paymail** handles | ❌ (revocable by domain owner) | ✅ | ✅ | domain-bound |
| **NBDomain / Allegory** | ✅ | ❌ (not chain-enforced) | ✅ | "Back to Genesis" scaling issues |
| **ORDnet `.web3`** | ❌ name layer (centralized DB) | ❌ | ✅ | content on-chain, *name* off-chain |
| **ENS** (Ethereum) | ✅ (smart-contract) | ✅ | ✅ | money claims names; annual rent |
| **Xanaverse UserRegistry** | ✅ | ✅ (SMT covenant) | ❌ (numbers) | identity-number, not names |
| **OpNS** | ✅ | ✅ (covenant, *at mint*) | ✅ | **all three at mint** — but centralized paid miner + pre-adoption today |

OpNS is the one that plausibly gets all three of Zooko's corners at the mint layer — its weaknesses are *operational/economic* (miner centralization, squatting economics, adoption), not *architectural*.

---

## Recommendation for Hodos

1. **Don't build our own name registry.** OpNS is the right architectural bet and it's shruggr's — we're aligned.
2. **Don't depend on it as a shipped naming layer yet** — it isn't one (near-zero adoption, centralized paid miner, unresolved economics).
3. **Engage + prototype (read-only) + watch** (see `README.md` for the posture, and `../../Sigma-BRC121-Sprint/phase-3-ordinals/` for the ordinals-adjacent check):
   - **Cheap, reversible first step:** prototype **read-only OpNS resolution** in Hodos's send form / omnibox (name → `opns.idKey` → BRC-29 address). Low lift, dogfoods the primitive, gives a concrete artifact to discuss with shruggr — without betting on adoption that isn't there.
   - **Open design decision for the planning phase (do NOT hardcode now):** resolve via shruggr's **hosted `1sat.app` overlay**, or **independently** via JungleBus / our own / other **federated-overlay** endpoints? The overlay + registrar are open-source / self-hostable, so the independent path is open — but avoiding a single-hosted-endpoint dependency needs its own research. Tracked in `../../Sigma-BRC121-Sprint/phase-3-ordinals/README.md`. (Note: `@1sat.app` = paymail domain; `1sat.name` = registrar UI.)
   - The open questions here (capital-bound squatting, no expiry, resale-binding resolver policy, centralized miner, canonical-genesis convention) **are literally our README's "how does the ecosystem converge on a naming solution" questions.** We're positioned as a *design partner*, not just a consumer.
4. **Possible OpNS × Xanaverse synthesis** (worth raising with both shruggr and Calhoun, since both are aligned builders): they're **complementary primitives that bind to the same BRC-100 identity key** —
   - **Xanaverse UserRegistry** = one canonical, unique on-chain *number* per identity key (Sybil-resistance / "verified forever #43" anchor).
   - **OpNS** = a human-readable *name* bound to that same identity key.
   - Layered: `matt` (OpNS) → resolves to identity key → that key also holds Xanaverse number `#43` (a reputation/verification badge). Name + verified-uniqueness-number + payable address, all keyed to one identity. This is a concrete instance of the hybrid our README already floats ("covenant for identity-key uniqueness + human-readable names on top").

---

## Reference links

**OpNS protocol / spec**
- OP-NS protocol: https://op0-2.gitbook.io/op-standard/protocols/op-ns
- Why OP (fair-mint rationale): https://op0-2.gitbook.io/op-standard/overview/why-op
- What is Proof of Work: https://op0-2.gitbook.io/op-standard/foundations/what-is-proof-of-work
- 1Sat name-service docs: https://docs.1satordinals.com/name-service/opns.md · payments: https://docs.1satordinals.com/name-service/payments.md

**Source code**
- `op-ns` sCrypt contract: https://github.com/op-enheimer/op-ns
- 1sat-sdk (`@1sat/actions` opns module — `opnsRegister`/`opnsDeregister`/`opnsList`/`opnsTransfer`): https://github.com/b-open-io/1sat-sdk
- 1sat-stack (Go indexer, `pkg/opns/`, `pkg/template/opns/`): https://github.com/b-open-io/1sat-stack
- opns-overlay (reference overlay server + the 1sat.name UI): https://github.com/b-open-io/opns-overlay
- shruggr/1sat-indexer (foundational indexer): https://github.com/shruggr/1sat-indexer

**Live endpoints**
- Registrar/resolver UI: https://1sat.name
- Overlay name lookup: `GET https://api.1sat.app/1sat/opns/mine/:name`
- Mining price (closed-source orchestrator): `GET https://mine.shruggr.cloud/price?name=<name>`
- Genesis outpoint: `58b7558ea379f24266c7e2f5fe321992ad9a724fd7a87423ba412677179ccb25:0`

**Context / comparison**
- CoinGeek — Naming protocols atop Bitcoin SV (Zooko's Triangle): https://coingeek.com/naming-protocols-atop-bitcoin-sv/
- Typosquatting 3.0 — squatting in Blockchain Naming Systems (arXiv, general, not OpNS-specific): https://arxiv.org/abs/2411.00352

**Related Hodos docs**
- `README.md` (this folder) — problem statement + current direction
- `Xanaverse-Contracts-Review/REVIEW.md` — the identity-*number* covenant (complementary primitive)
- `Paymail/OPEN_PAYMAIL_PROTOCOL.md` — our own overlay paymail design (OpNS is a stronger-uniqueness live instance of much of this)
- `Domain-Names/WEBSITES_ON_CHAIN_RESEARCH.md` — ORDnet `.web3`, Babbage Metanet URI, on-chain content
- `../../Sigma-BRC121-Sprint/phase-3-ordinals/README.md` — the ordinals-adjacent naming check
