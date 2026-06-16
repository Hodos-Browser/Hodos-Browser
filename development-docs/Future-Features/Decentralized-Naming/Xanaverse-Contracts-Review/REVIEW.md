# Review: @xanaverse/contracts (v1.3.0)

**Reviewed:** 2026-05-09
**Source:** https://www.npmjs.com/package/@xanaverse/contracts
**Author:** XanaVerse / johncalhooon (dev@calhounjohn.com)
**Local copy:** `package/` (npm tarball v1.3.0, ~2.7MB unpacked, 170 files)
**License:** MIT
**Built on:** scrypt-ts ^1.4.0

## TL;DR

The package is **mostly a trustless social-media protocol** (posts, upvotes, replies with covenant-enforced creator payments). It includes a `UserRegistry` contract that **does enforce uniqueness on-chain via a Sparse Merkle Tree covenant** — not via indexers — which is a real engineering accomplishment and a partial correction to our earlier framing in `../README.md`.

**However**, the registry has real trade-offs that mean it doesn't fully solve Hodos's decentralized-naming problem:

1. **Singleton UTXO bottleneck:** only one registration per block (BSV's 10-minute average).
2. **Assigns sequential numbers, not human-readable names.** Each user gets `userId: bigint`. No `marston` → identity mapping.
3. **Centralized deployer:** the `treasuryAddress` (registration fees) and `platformAddress` (1% marketplace fees) are baked into the singleton at deployment. Whoever deploys gets the fees and influence over future versions.
4. **Cost per registration ~1500+ sats:** 8KB SMT proof + 1000 sat registration fee.

Verdict: **Genuinely impressive technique. A useful building block. Not a complete solution for Hodos.**

---

## What's actually in the package

| Contract | Purpose | Uniqueness mechanism |
|---|---|---|
| `UserRegistry` | Singleton trustless registration of user numbers | On-chain SMT covenant (the interesting one for our use case) |
| `UserProof` | Tradeable proof-of-ownership for a registered user number, with built-in marketplace | Covenant enforces 99/1 split on sale, signature-gated transfers |
| `PostAnchor` (v0/v1) | Immutable posts with two-tx commit-reveal pattern + signature-validated authorship | N/A (each post is unique by transaction; not a uniqueness problem) |
| `UpvoteProof` | Upvote with covenant-enforced payment to creator | N/A (count derived by querying UTXOs) |
| `ReplyProof` | Reply with covenant-enforced payment to parent post | N/A |
| `DownvoteProof` | Downvote (no payment) | N/A |

**The core social-media architecture** is clever but orthogonal to our naming question. The relevant piece for our R&D is `UserRegistry` + `UserProof`.

---

## How `UserRegistry` works (the technique that matters)

### State stored in a single covenant-locked UTXO:

```typescript
nextUserId: bigint              // 1, 2, 3, ... sequential
registeredKeysRoot: ByteString  // SMT root of all registered identity keys
treasuryAddress: PubKeyHash     // immutable: fee recipient
platformAddress: PubKeyHash     // immutable: marketplace-fee recipient
emptyHash: ByteString           // 32 bytes of zeros (SMT empty leaves)
```

The SMT is **depth 256** — one leaf for every possible `sha256(identityKey)` value. The full SHA-256 address space, so collision probability is effectively zero.

### Registration flow:

1. Registrant constructs a transaction that **spends the current registry UTXO** and creates a new one
2. They provide:
   - Their identity key (which they sign with)
   - A **256-element non-membership proof** showing their key is NOT already in the tree
3. The contract's `register()` covenant runs on-chain (validated by every miner) and:
   - Checks the registrant's signature
   - Walks the SMT proof — verifying each sibling hash bottom-up — to confirm the leaf at `sha256(identityKey)` is empty
   - Computes the new SMT root with the leaf inserted
   - Validates that the next-state UTXO has exactly that new root
   - Validates the treasury fee output exists (1000 sats hardcoded)
   - Validates a `UserProof` UTXO is created for the registrant with `userId = nextUserId`

If any check fails, the entire transaction is rejected by miners. **No indexer is involved in determining uniqueness.** This is genuinely network-enforced.

### Key insight that updates my earlier framing

I previously said: *"Bitcoin Script can't enforce uniqueness directly. Miners validate transaction-level rules, not protocol-level semantics."*

That was incomplete. With sCrypt covenants and an SMT, you can encode "this key has not been registered before" as a transaction-level rule — because non-membership in a Merkle tree IS a verifiable property given a sufficient proof. Miners can validate it. Uniqueness CAN be enforced on-chain on a UTXO model, with caveats.

**The corrected framing:** there are now three approaches to decentralized uniqueness on Bitcoin (BSV), not two:

| Approach | Where uniqueness is enforced | Trade-off |
|---|---|---|
| **Centralized registry** (ORDnet) | One trusted operator's database | Fast, simple, single point of failure |
| **Federated overlay** (BRC-22) | Multiple operators following deterministic protocol; consensus via convergence | Scales horizontally, no single bottleneck, residual operator-honesty trust |
| **sCrypt SMT covenant** (XanaVerse pattern) | Singleton UTXO + miners validating proofs | Truly trustless for the rule; bottlenecked by single-UTXO contention |

`../README.md` should be updated to reflect this third option.

---

## The trade-offs that limit XanaVerse's design for our use case

### 1. Singleton UTXO contention (the structural one)

The entire registry state lives in ONE UTXO. To register, you must spend that UTXO and create a successor. **Only one registration can be confirmed per block** — because two transactions trying to spend the same UTXO are double-spends, and miners only include one.

BSV blocks average ~10 minutes. So:

- 6 registrations per hour
- ~144 per day
- ~52,000 per year

For a niche social-media protocol with slow growth, this is fine. For a name registry that wants to absorb millions of users (or even tens of thousands), it's a hard ceiling.

There are workarounds:
- **Sharding by namespace** (one registry per first letter, etc.). XanaVerse doesn't do this.
- **Per-name UTXOs with deterministic addressing** (the pattern I sketched in `../README.md`). Different design entirely; trades the bottleneck for a different uniqueness mechanism.
- **Off-chain coordination layer** (overlay batches multiple registrations into one transaction). Defeats the "fully on-chain" claim somewhat.

**This bottleneck is the most important architectural caveat.** It's why ENS works on Ethereum — the EVM has parallel state per slot, so two name registrations in the same block don't conflict. BSV's UTXO model with a singleton registry doesn't have that.

### 2. Numbers, not names

Each user gets `userId: bigint` (1, 2, 3...). The mapping `marston → user 17` doesn't exist in the contract. To turn user numbers into human-readable names, you need:

- Either an off-chain mapping (back to indexer territory)
- Or another singleton-with-SMT contract specifically for names (same bottleneck, multiplied)
- Or each `UserProof` UTXO has a mutable `name` field (then you need yet another uniqueness mechanism for names — they're not enforced unique by the existing contract)

So this contract solves **identity-key → unique-number** but not **identity-key → unique-name**. For Hodos's paymail and `.bsv` use cases, we need names. The XanaVerse contracts give us a pattern, not the actual feature.

### 3. The deployer collects fees

The singleton has a `treasuryAddress` and `platformAddress` baked in at deployment. Whoever deploys:
- Receives 1000 sats per registration (treasury)
- Receives 1% of all marketplace transactions (platform)
- Has gradient pull on the namespace because they "are" the registry

This isn't a deal-breaker — ICANN collects fees, ENS DAO collects fees, every namespace has a fee structure — but it means the deployer is a privileged participant. If a thousand people each deployed their own `UserRegistry` clone, you'd have a thousand competing namespaces with no coordination.

So in practice the system requires **someone to be the canonical deployer** that everyone agrees to use, which is a soft form of centralization. Not as bad as ORDnet (because the rules are immutable once deployed and the deployer can't change them), but not as decentralized as the rules-only-deterministic federated model either.

### 4. Cost per registration

Roughly:
- 8 KB SMT proof = ~400 sats at typical BSV fee rates
- 1000 sat registration fee
- 1 sat for the next-state UTXO + 1 sat for the UserProof + change
- Network broadcast and tx assembly overhead

So ~1500+ sats per registration, hardcoded. ORDnet's free 10+ char names are more accessible at the user level. ENS's pricing is much higher in dollar terms but has a tiered structure.

For a paid name service this is fine. For a "free for everyone" service it's a friction point.

### 5. The reclaim escape hatch concern

`UserProof` has a `reclaim()` method that lets the owner spend the UTXO entirely (collapsing it). Per the docs, "the user number is still historically registered" — but the UserProof is gone from the UTXO set. If you're querying current state via the UTXO set, the user number disappears. Indexers would need to walk transaction history to find it.

This isn't broken — it's a design choice for cleanup — but it interacts oddly with the "permanent registration" claim. After reclaim, what does it mean to "own" user 17? You're no longer in the live UTXO set, but no one else can register user 17 either (it's still in the SMT root). It's an orphaned slot. Worth thinking through if we adopt this pattern.

---

## How this compares to the federated-overlay path we sketched

| Property | XanaVerse SMT covenant | Federated overlay (BRC-22) |
|---|---|---|
| Uniqueness enforcement | Network (miners) | Protocol convergence among operators |
| Throughput ceiling | ~6 registrations/block (BSV) | Bounded by chain throughput, not protocol contention |
| Operator trust required | Just trust deterministic miner validation | Trust at least one honest operator (cross-checking detects dishonesty) |
| Deployer privilege | Treasury + platform fee recipients | None inherent (operators compete on quality) |
| Human-readable names | Not built in | Designed for it |
| Fee model | Hardcoded in contract | Per-query micropayments to operators |
| Upgrade path | None (contract is immutable) | Operators can adopt new versions of the protocol; registry data migrates |
| Best for | Identity-numbers and enforced-payment protocols | Human-readable names, high throughput, evolving features |

Both are real options. They're not mutually exclusive — you could use sCrypt-SMT for the identity-key registration layer (where uniqueness is critical and throughput is slow-growing), and federated overlays for the human-readable name layer on top (where throughput matters). That hybrid might actually be the best design.

---

## Recommendation for Hodos

1. **Update `../README.md`** to acknowledge the sCrypt-SMT-covenant approach as a third path. My earlier framing of "indexers are the only way" was incomplete.

2. **Consider XanaVerse's `UserRegistry` pattern as a building block.** Specifically: if Hodos ever wants to assign on-chain unique identifiers to identity keys (separate from human-readable names), this is the proven design. We don't need to build it; we'd potentially fork or partner.

3. **Don't expect XanaVerse to solve the human-readable-name problem.** It's a number registry, not a name registry. Names are a separate (harder) problem on top.

4. **Reach out to John Calhoun.** He's working in adjacent territory; an R&D conversation would surface whether they have plans for human-readable names, what their throughput model is, and whether the social-media protocol or the identity-number primitive is their primary product. Their npm publishing maintainer email is `dev@calhounjohn.com`.

5. **Consider the hybrid design.** sCrypt-SMT for identity-key uniqueness (slow, high integrity), federated overlay for human-readable names on top (fast, high throughput, with reputation defending against name-squat). This isn't a thing anyone has built yet; it might be the actual shape of a working BSV-native naming system.

---

## What was wrong in my earlier framing

In `../README.md` I wrote:

> "Bitcoin Script doesn't enforce uniqueness directly. Miners validate transaction-level rules, not protocol-level semantics. There's nothing stopping someone from broadcasting a transaction that claims `marston.bsv` even if I've already claimed it."

This was incomplete. With sCrypt + SMT proofs, "this key has not been registered before" can be a transaction-level rule that miners validate. The blockchain CAN enforce uniqueness directly given the right covenant design. I was reasoning about what raw Bitcoin Script can do without smart-contract abstractions; sCrypt extends that meaningfully.

The corrected framing is: **the question isn't whether on-chain uniqueness is possible (it is), but which approach scales for the use case at hand.** sCrypt-SMT for low-throughput / high-integrity. Federated overlays for high-throughput / human-readable. Both have legitimate domains.

User's instinct to be skeptical was correct in the right direction (the contract isn't a complete naming solution), but the underlying technique IS legitimate — I owe a clean update to the main README.

---

## Files of interest in `package/dist/src/contracts/`

- `UserRegistry.d.ts` — the singleton + SMT covenant pattern
- `UserProof.d.ts` — tradeable per-user UTXO with marketplace covenant
- `PostAnchor.d.ts` — two-tx commit-reveal pattern with signature-validated authorship (interesting separately for content)
- `UpvoteProof.d.ts` — covenant-enforced payment to creator + transaction parsing for binding the proof to the right post
- `shared/` — utility code (varint parsing, output extraction, etc.)

Worth reading the post/upvote contracts even though they're not naming-relevant — the two-tx commit-reveal pattern with on-chain nonce is a clever privacy mechanism that could be useful elsewhere in Hodos.
