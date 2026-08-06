# Decentralized Naming — Problem Statement and Design Direction

**Created:** 2026-05-09
**Status:** Working hypothesis — research ongoing. **Current lean (2026-07-15): OpNS** (posture = *engage, prototype, watch*). See "Current direction" below + `OPNS_REVIEW.md`.
**Owner:** Matt

## Why this folder exists

Two seemingly distinct features Hodos cares about — paymails (human-readable wallet identifiers) and on-chain domain names (`.web3`, NBDomain, hypothetical `.bsv`, etc.) — share the same architectural problem. This folder organizes the research and design thinking for both, because the lessons in either subdomain transfer directly to the other.

```
Decentralized-Naming/
├── README.md                       ← this file (problem statement + design direction)
├── OPNS_REVIEW.md                  ← ⭐ current lean: deep review of OpNS / 1sat.name (2026-07-15)
├── BOOTSTRAP_PROBLEM.md            ← adoption / network-effect strategy
├── NARRATIVES.md                   ← externally-facing messaging
├── Xanaverse-Contracts-Review/     ← the sCrypt-SMT identity-*number* covenant (complementary primitive)
├── Paymail/
│   └── OPEN_PAYMAIL_PROTOCOL.md
└── Domain-Names/
    └── WEBSITES_ON_CHAIN_RESEARCH.md
```

---

## Current direction (updated 2026-07-15)

**We have a strong lean toward OpNS as the naming primitive** — and a posture of **"engage, prototype, and watch."** Full analysis in `OPNS_REVIEW.md`; the short version:

- **OpNS** (David Case / @shruggr / b-open-io) is the strongest on-chain, human-readable, self-sovereign, *payable* naming primitive that exists on BSV today. Its name-mint **uniqueness is enforced by Bitcoin Script / consensus via an sCrypt covenant** — a *stronger* guarantee than our own Open Paymail design — and it delivers human-readable **names**, filling the exact gap the Xanaverse review left open (Xanaverse gives unique *numbers*, not names).
- **But it is essentially pre-adoption**: near-zero names registered, a centralized closed-source paid miner (~0.25 BSV / ~$3 flat per name), no expiry, and squatting economics that are currently capital-bound rather than compute-bound. So it is a promising *primitive*, not a shipped *naming layer* we can depend on yet.

**What "engage, prototype, watch" means concretely:**

| Track | Posture | Why |
|---|---|---|
| **OpNS name resolution** (name → `opns.idKey` → BRC-29 address) | **Prototype** (read-only) | Cheap, reversible, dogfoods the primitive, and gives us a concrete artifact to design-partner with shruggr on. First home = a paymail-style resolver in the send form — see `../../Sigma-BRC121-Sprint/phase-3-ordinals/`. |
| **OpNS as a design collaboration** (squatting economics, no-expiry, resale-binding resolver policy, canonical-genesis convention, client-side miner) | **Engage** | These open questions *are* our "how does the ecosystem converge on a naming solution" questions. We're positioned as a design partner, not just a consumer. |
| **More decentralization at the indexer / overlay layer** (federated overlays, no single leader, micropayment-funded operators) | **Engage + watch** (NOT prototype yet) | Still the right long-term shape (see "The federated overlay path" below), but OpNS's overlay peering is currently a static leader and adoption is thin. Watch how it decentralizes; don't build overlay infra ourselves. |

**Standing rules:** don't build our own name registry; don't depend on OpNS as a shipped layer yet; keep the OpNS × Xanaverse synthesis (name + verified-unique-number, both bound to one BRC-100 identity key) on the table with both shruggr and Calhoun.

---

## The shared problem

A useful naming system needs four properties:

1. **Human-readable** — people can remember, type, share verbally
2. **Unique** — `marston` should map to exactly one identity, not several
3. **Non-fungible** — your name is yours; nobody else can claim it
4. **Self-sovereign** — owner has cryptographic control of their name

Three of these are easy on Bitcoin (BSV). The hard one is **uniqueness**.

Bitcoin Script doesn't enforce uniqueness directly. Miners validate transaction-level rules — is this signature valid, are these inputs unspent, does the script evaluate correctly — not protocol-level semantics. There's nothing stopping someone from broadcasting a transaction that claims `marston.bsv` even if I've already claimed it. The blockchain accepts both as valid transactions. The question of "who really owns marston.bsv" is decided by software that interprets the chain — **indexers**.

That means uniqueness on Bitcoin can be enforced at one of three levels:

| Approach | What it looks like | Who we're trusting | Trade-off |
|---|---|---|---|
| **Centralization** | One operator (ICANN, ORDnet, a single paymail provider) maintains the canonical registry and arbitrates conflicts | The operator | Single point of failure |
| **On-chain via sCrypt SMT covenant** | A singleton UTXO holds a Sparse Merkle Tree root of registered keys. Registration spends the singleton, providing a non-membership proof. Miners validate the proof on-chain. (See `Xanaverse-Contracts-Review/` for a working example of this pattern.) | The protocol + Bitcoin's miners | **Coordination** bottleneck, not a throughput one *(corrected 2026-08-05)* — a sequencer can chain thousands of registrations into one block (ancestor-height limit 10,000 / Teranode 1M), but **whoever sequences is a de-facto centralized coordinator**. Uncoordinated writers get ~1/block plus retry storms. |
| **Federated consensus among indexers** | Many independent operators follow the same deterministic protocol; given the same chain data, they converge on the same answer | The protocol + the federation as a whole | No single bottleneck, scales horizontally; residual operator-honesty trust |

**Earlier drafts of this doc said indexers were "the only way." That was wrong.** sCrypt with covenants and Merkle proofs CAN enforce uniqueness at the network level on the UTXO model. The trade-off is ~~throughput~~ **coordination** *(corrected 2026-08-05)*, not possibility. Centralization remains philosophically misaligned with Bitcoin; the other two are both legitimate decentralized paths with different scaling characteristics.

### ⚠️ The shard-spawn race — why you cannot shard your way out of contention (added 2026-08-05)

The obvious fix for a contended singleton is to **partition the namespace** — shard by a hash prefix of the name, so `shard(name) = H(name) mod N`. The uniqueness argument is sound *as far as it goes*: a deterministic, total, fixed name→shard map means every name lives in exactly one shard, so **shard-local uniqueness ⇒ global uniqueness**, with no cross-shard collision by construction. It also fixes hot-prefix skew, which a literal-character trie suffers from.

**What breaks is shard creation.** If shards are created lazily — shard #7 exists only once someone registers a name hashing to 7 — then shard #7's genesis is itself an unclaimed slot. Two parties concurrently creating shard #7 produce **two valid, non-conflicting transactions**. They don't spend a common UTXO, so first-seen never fires, both confirm, and you have rival shard-7 roots in which `alice` can exist twice.

> **The failure in one line: sharding converts a *double-spend* (which consensus resolves for free) into a *duplicate-creation* (which consensus cannot see at all).** You have re-introduced the original naming problem one level up, at the shard layer.

Only two sound fixes:

| Fix | How | Verdict |
|---|---|---|
| **Pre-create all N shards at genesis** | One deployment fans out all N shard roots from a single genesis outpoint; lazy creation banned | Clean, but N is fixed forever and you pay for N roots up front |
| **Make shard-spawn a contended spend** | Shard root *k* is spawned by spending a parent node whose bitmap marks slot *k* claimed — so spawning twice **is** a double-spend | **Better**, and pay-as-you-grow |

**OpNS already does the second one.** Its char-by-char mine tree is a trie of UTXOs where creating a child means spending the parent's claimed-bitmap. That is not incidental to the design — **it is load-bearing**, and it is the reason OpNS's fan-out is sound where a naive hash-sharded map would not be.

**General rule to carry forward:** *a sound sharded namespace has exactly one uncontended layer (the genesis); every other layer must derive from it by a spend.*

Four secondary problems remain even when spawn is sound: **shard discovery** (shard→tip mapping isn't queryable on-chain — needs an indexer or a genesis walk), **proving non-existence** (needs an SMT non-membership proof per shard — i.e. sharded Xanaverse, ~8 KB per registration), **hot shards** (a tail problem under hashing, survivable given chained spends), and **root trust anchoring** (every shard must be provably descended from one genesis, or rival trees are indistinguishable — OpNS embeds the 36-byte genesis outpoint in every node and the indexer rejects mismatches, but this remains *convention, not consensus*).

**Hybrid designs are possible.** The most interesting one for our use case: use the on-chain SMT-covenant path for identity-key uniqueness (slow, high-integrity, low volume), and federated overlays for human-readable names on top (fast, high-volume, reputation-secured). Each layer plays to its strength. See `Xanaverse-Contracts-Review/REVIEW.md` for detail on how the on-chain piece can work.

> **⭐ Update (2026-07-15): OpNS already implements the covenant approach *for human-readable names*.** The row above ("On-chain via sCrypt SMT covenant") had only produced Xanaverse's identity-*number* registry when this table was written. **OpNS** (shruggr / b-open-io) is a live covenant-secured system that enforces name-mint uniqueness at consensus level and delivers actual human-readable names — a fan-out "mine tree" rather than a single choke-point registry, so it also sidesteps Xanaverse's singleton **sequencer** bottleneck (see the correction above — the old "~6 registrations per block" framing was wrong). It is our current lean. **Full teardown + reference links: `OPNS_REVIEW.md`** — note that doc's "adoption is near-zero" finding was **retracted 2026-08-05**; the tree is substantially claimed.

---

## The federated overlay path (BRC-22)

Bitcoin's federated overlay model — formalized in BRC-22 (Overlay Network Data Synchronization Protocol) — is the most promising direction we've identified. The shape:

- Anyone can run an indexer / overlay service
- All operators implement the same publicly-specified protocol rules
- Given the same chain data, any compliant operator computes the same state independently
- Clients can query any operator, and (if they want extra confidence) cross-check against multiple
- No single operator is canonical; the *protocol* is canonical

This isn't decentralized in the cryptographic-consensus sense (where every miner enforces every rule, like Ethereum's EVM). It's decentralized in the **redundant-infrastructure sense** — closer to how DNS resolvers operate at scale today. There's no single point of failure as long as there are enough independent operators following the same spec.

---

## The incentive problem

Federated overlays don't run themselves. Operating an indexer costs real money — servers, bandwidth, chain sync, ongoing maintenance. Without an incentive layer, only one or two well-funded operators will exist, which collapses back toward effective centralization.

Our working belief: **API micropayments are the natural incentive layer.**

A wallet or application paying small per-query fees to an indexer:

- Aligns operator economics with actual usage
- Lowers the barrier to becoming an operator (you don't need to be venture-funded; a sustainable side business is enough)
- Makes "many small operators" economically viable — which is what gives the federation real decentralization, not just nominal decentralization
- Plays to Bitcoin's structural advantage. Fast, cheap, native micropayments are something this chain can actually do; other chains can't match it

Concrete implementation is design space. Possibilities include per-request HTTP 402, BRC-29 PeerPay flows for one-shot queries, prepaid pools, subscription channels via BRC-100 wallet authorization, etc. The architectural commitment is "wallets pay indexers per query (or batch)"; the protocol shape can iterate.

---

## The "first mint" question — actually deterministic

**Who actually owns `marston.bsv` if two people claim it?**

The blockchain itself answers this deterministically. Block height + transaction index within the block defines a total ordering of all transactions ever broadcast. The protocol rule is just "lowest (block height, tx index) tuple wins." Given the same chain data, every honest indexer applying this rule computes the same answer. There is no protocol-level ambiguity.

The residual question isn't *can the protocol determine the truth* — the protocol is deterministic. The residual question is *is the indexer I'm querying honestly reporting what the protocol computed?* That's a much smaller surface, with a clean defense:

- **Cross-checking.** Query multiple independent indexers. They should all return the same answer. A liar becomes the outlier within seconds.
- **Confirmation depth.** Wait N blocks before treating a claim as final. Reorgs deeper than N become exponentially improbable; for naming, even N=6 is overkill.
- **Public attestations.** Honest indexers sign and publish their computed state regularly. Disagreements between operators become publicly auditable after the fact.
- **Game theory and reputation.** The deepest defense — covered next.

There's no protocol-level attack vector for first-mint when block ordering is the source of truth. The defenses above are about *operator honesty*, not *protocol soundness*. Two distinct concerns; conflating them was a sloppy framing in earlier drafts.

---

## The reputation layer (where this actually works)

The deepest answer is game-theoretic. An indexer that wants to be the long-term backbone of a meaningful naming system has every incentive to behave honestly:

- Wallet builders, application developers, and end users won't integrate with — or pay — operators that have ever been caught lying
- A single demonstrated dishonesty can collapse years of accumulated reputation in days
- Honest operators continue earning micropayment revenue indefinitely
- Dishonest ones cash out once on a single attack and lose the franchise forever

This is the same trust model that underpins DNS resolvers, certificate authorities, payment processors, and clearing houses today. They aren't "trusted" because of cryptographic guarantees — they're trusted because their entire business model depends on continued trust. The expected cost of cheating exceeds the expected benefit when the future income stream is large enough.

**Up-front investment is itself a costly signal.** Operators who invest in real infrastructure, public audit trails, transparent governance, and published track records have skin in the game. Only operators planning to be reputable long-term will make those investments. End users and integrators can use this as a filter: prefer operators who've already sunk significant capital into being reputable.

This is not a perfect mechanism. Bad actors can fake reputation, large operators can leverage market power, and there's always *some* probability of a "rug pull." But across many independent operators with skin in the game, the overall system is robust enough that the residual fraud risk is comparable to (or lower than) what users tolerate from their banks, ISPs, and DNS providers today.

---

## The mutually-beneficial endpoint

The system works when wallets, applications, indexers, and end users all benefit from each other's success:

| Party | What they get | What they pay |
|---|---|---|
| **End users** | Stable, portable, human-readable identity they cryptographically own | A few sats per name lookup (effectively free at the user-experience level) |
| **Wallet builders** | A name layer they can integrate against without having to operate one themselves | A few sats per query, passed through to the user (or absorbed) |
| **Application builders** | Their users have stable identities they can attach state, history, and reputation to | Same |
| **Indexer operators** | Sustainable per-query revenue at scale | Real infrastructure investment, real reputation investment |

When every party profits from the system continuing, attacks on the system are economically self-defeating. That's the goal: not a perfect cryptographic system, but a robust *economic* system that's stable in the way real-world institutions are stable, while preserving the self-sovereignty Bitcoin uniquely enables at the cryptographic layer.

---

## How paymails and domain names fit this frame

Both subfolders below are applications of the same architecture. Insights in one should propagate to the other.

| | Paymail (`matt@hodosbrowser.com`) | Domain Name (`marston.bsv` or `.web3`) |
|---|---|---|
| The thing being uniquely named | A wallet's address-generation endpoint | A piece of on-chain content (HTML inscription) or a wallet address |
| Today's solution | DNS-bound handles (centralized via ICANN registrars) | ORDnet's centralized DB; ENS's smart contract; ICANN parallel/`.web3` |
| What we want | Human-readable, self-sovereign, federated registry with cryptographic ownership | Same |
| Indexer role | Resolves `name@domain.com` → wallet's payment endpoint via federated overlay | Resolves `marston.bsv` → wallet/content TXID via federated overlay |

The deep insight: **paymails and domain names are the same protocol applied to different namespaces.** A solution to one is mechanically a solution to the other.

---

## Open questions to dig into

- Is there a protocol-level way to make first-mint manipulation cryptographically *detectable*, not just commercially disincentivized?
- What's the right granularity for indexer micropayments? Per-query? Per-batch? Subscription channels via BRC-100 authorized recurring payment?
- How do we bootstrap reputation when there are no incumbent operators yet — bootstrap problem in a federation
- Hodos's preferred role: **client + ecosystem partner + active encourager.** We want overlay/indexer providers to build sustainable businesses on top of the protocol. We believe micropayments enable specialization, and indexer infrastructure is the most under-supplied piece of the BSV stack today. We don't want to focus our own time and energy on running an overlay; we'd rather invest in protocol design, client integration, and partnerships. **We will run one reluctantly if other operators don't step up** — but the goal is to never need to. This is a strategic position; see `BOOTSTRAP_PROBLEM.md` for the broader thinking on why ecosystem-builder is a stronger long-term position than vertical-integrator.
- How does this interact with Babbage's incoming Metanet URI scheme (DNS-binding-via-pubkey hint)? Compatible, complementary, or competing?
- What's the minimum viable spec for a federated paymail registry that could ship? Same question for domain names.
- Is there a way to share infrastructure between paymail and domain-name indexers, since they're solving the same problem?

---

## What's in the subfolders

| Subfolder / doc | What's there | What's missing |
|---|---|---|
| `OPNS_REVIEW.md` | ⭐ Current lean. Code-level teardown of OpNS / 1sat.name: char-by-char mine tree, consensus-covenant uniqueness, PoW, cost/adoption reality, weaknesses, comparison, reference links | Live end-to-end mint test; X/Twitter sentiment (blind spot); adoption trend over time |
| `Xanaverse-Contracts-Review/REVIEW.md` | The sCrypt-SMT identity-*number* covenant (unique numbers, not names) — complementary to OpNS | The OpNS × Xanaverse synthesis (name + verified-number on one identity key) is noted, not yet designed |
| `Paymail/OPEN_PAYMAIL_PROTOCOL.md` | Our own open/decentralized paymail proposal (overlay + PushDrop + first-seen-in-block uniqueness) | **Needs a rethink in light of OpNS** — OpNS is a live instance of much of this with a *stronger* (consensus) uniqueness guarantee |
| `Domain-Names/WEBSITES_ON_CHAIN_RESEARCH.md` | Research on on-chain websites, ORDnet, NBDomain, Bottle browser, ENS, Babbage Metanet URI scheme | Should incorporate OpNS as the decentralized alternative to ORDnet's centralized `.web3` name layer |

These docs predate the OpNS review. As the design crystallizes around OpNS, they should be refactored to align with `OPNS_REVIEW.md` and the current-direction block above.
