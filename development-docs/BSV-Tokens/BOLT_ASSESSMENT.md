# BOLT Protocol — Assessment

> **Created:** 2026-08-05
> **Status:** Research complete. **Posture: watch + engage. Do not build on it yet.**
> **Trigger:** the Kurt Wuckert Jr (@kurtwuckertjr) ↔ John Calhoun (@johncalhooon) X debate, 5 Aug 2026.

---

## TL;DR

BOLT's core primitive — **latching** — is genuinely novel and is the most interesting covenant idea in BSV right now. It deserves to be understood. But the thing being *debated publicly* (sharded AMM pools) is not in the specification at all, and the specification carries a **pending patent** that the debate never mentioned.

| Question | Answer |
|---|---|
| Is the primitive real? | **Yes.** Latching genuinely extends PUSHTX beyond its one-ancestor ceiling. |
| Is sharding part of BOLT? | **No.** Zero occurrences of `shard`, `pool`, `AMM`, `liquidity` in the whitepaper. It is an unpublished POC. |
| Does per-transfer size grow with history? | **No — it's flat.** Measured 19,715 → 18,935 → 18,531 B parent→child. |
| Is it cheap? | **No.** ~19 KB/transfer ≈ **$0.025** — 40–65× a BSV21 transfer. |
| Can we legally build on it? | **Informal permission only.** Get it in writing before shipping. |
| Should we adopt it now? | **No.** Watch it; we know the authors. |

---

## What BOLT actually is

**Paper:** *BOLT: A Bitcoin Transaction Latching Mechanism & Token Protocol* — **Frederick Liam Simon Honohan**, 11 Feb 2024, 44pp. `https://boltassociation.com/assets/BOLT-Protocol.pdf` (note: `.com`, **not** `.org`).

> ⚠️ **BOLT is not John Calhoun's.** The public debate was Wuckert vs Calhoun, but Calhoun is building a proof-of-concept *on* BOLT. Honohan is the author; he works with/for **Elas**. Profiles: `Marketing/Profiles/bsv/frederick-honohan.md`, `brendan-lee.md` (also Elas), `john-calhoon.md`.

### The primitive

Ordinary **PUSHTX / PUSHCTX** lets a script inspect its own ScriptContext (sighash preimage) — the executing output's outpoint, its whole `scriptCode`, and the transaction's summary fields. Its hard limit, in the paper's words: *"the history of a chain of UTXOs which is directly accessible by way of the PUSHTX technique at processing time is limited to one transaction only."*

**A "bolt" is a UTXO deliberately left in one transaction so that a later transaction can inspect the transaction containing it.** Three requirements define it:

1. Unlocking at least one additional UTXO depends on simultaneously unlocking the bolt (and/or vice-versa).
2. The **full raw serialised transaction** that created the bolt is used in the calculation.
3. That exposes *every* input's previous-outpoint and `scriptSig`, and every output's script and value, to the executing script.

Because a bolt's inputs may themselves carry ScriptContext blobs, you can walk *further* ancestral locking scripts. Chained: `latchTx` (the requirement is applied) → `unlatchTx` (the bolt is spent, releasing the other UTXO).

### Why it matters — Back-to-Genesis by induction

The paper's headline use case: with two unspent outputs in two distinct transactions plus merkle proofs, a token's ancestry from its genesis outpoint is provable **inductively** — you carry one ancestor, not the whole history. That is a real answer to Back-to-Genesis and it is why BOLT bills itself as SPV-compatible.

**This is the part worth understanding regardless of whether we adopt it.**

---

## The three findings that decide it

### 1. ⛔ Patent — the buried lede

**All 12 content pages carry the same footer:**

> *"UK patent application GB2318902.0 (pending) — available on request. Copyright www.elas.co ©2024"*

The public debate argued entirely about **nChain's** PUSHTX licensing. Kurt asked *"What is the licensing posture?"*; Calhoun replied *"nChain granted use. Also didn't know you could patent an opcode."* (Patents cover *techniques*, not opcodes — the aside misses the point.) **Neither participant appears aware that the BOLT specification asserts a separate pending patent held by Elas.**

**Our position (owner decision, 2026-08-05):** we know Freddy and Brendan, the relationship is good, and they want us to use it. Proceeding on that basis. **But:** informal permission covers *us*; shipping covenant support in a wallet extends the surface to every user and dApp that touches it, and GB2318902.0 is *pending* — claims aren't fixed. **Action: obtain a short written grant from Elas before shipping anything that depends on it.** Cheap, removes the question permanently, and we have two warm channels to ask through.

### 2. The sharding under debate is not in the specification

Verified by full-text search of the extracted whitepaper:

| Term | Occurrences |
|---|---|
| `shard` / `Shard` | **0** |
| `pool` / `Pool` | **0** |
| `AMM` | **0** |
| `liquidity` / `Liquidity` | **0** |

The paper's swap is a **peer-to-peer atomic exchange** ("bureau de change… between recipients of tokens created by two separate issuing entities for an agreed exchange rate"), not an AMM with a pooled-liquidity UTXO. It also states the fungible/non-fungible code examples are *"not fully tested for production release."*

**Pools, sharding, forced exits and the upgrade arm are one person's unpublished POC** — no spec, no audit, one beta repo (`b017`, 0 stars, `v0.0.0-b1`).

### 3. Kurt won the on-chain exchange

Calhoun's evidence was block **957547** (verified: 1,308 txs, 1,079,909 bytes). His three cited transactions, by block index:

| Tx | Role | Block index | Size |
|---|---|---|---|
| `dc0b8cdf85…` | grandparent | **116** | 19,715 B |
| `d3746c3ce1…` | parent | **126** | 18,935 B |
| `fdfe17e0c7…` | forced exit | **1005** | 18,531 B |

Strictly increasing indices ⇒ **serial parent→child inside a single shard**. Kurt's rebuttal — *"Sharding didn't remove the bottleneck, it made four of them… A router that sequences is the operator I'm talking about"* — is correct and **went unanswered**.

Measured: **27 nonstandard txs = 420,865 bytes = 39.0% of the block for 2.1% of its transactions.** (Kurt estimated 45%; the true figure is 39% — direction and magnitude of his point intact.)

**Unverified:** "26 transactions" (I count 27 nonstandard / 24 large); "4 shard chains" (5 size-clusters, full parentage untraced); that these are *pool* rather than plain token transfers; and **"256 forced exits"** — only one txid was ever cited by anyone.

---

## Economics

**Per-transfer size does NOT grow with history** — the three-tx measurement above *decreases* slightly, consistent with the induction design carrying one ancestor rather than accumulating. It is a roughly constant ~19 KB per transfer.

At the live rate (100 sat/KB, BSV $12.93):

| Protocol | Per transfer | Cost |
|---|---|---|
| **BOLT** | ~19 KB | **~1,970 sat ≈ $0.025** |
| BSV21 | ~450–500 B | ~50 sat ≈ $0.0006 |
| Plain P2PKH | ~250 B | ~25 sat ≈ $0.0003 |

**Verdict:** viable for high-value or low-frequency transfers; **prohibitive for the machine-to-machine micropayment story BOLT itself pitches.** That was Kurt's actual economic point and it stands: *"transfer weight IS the product."*

---

## What "forced exit" means

A **forced exit** (unilateral exit / escape hatch) is the L2/rollup concept: can a user withdraw **without the operator's cooperation**, purely by satisfying the covenant's script conditions? Calhoun's claim — *256 exits with operator keys never used* — is an **anti-custody proof**, not a throughput proof. It is the strongest part of his case, and Kurt granted it up front: *"256 forced exits on mainnet with the operator keys never used is real work."*

---

## Relevance to Hodos

**Not for naming.** A namespace needs a *global uniqueness* invariant, which is strictly harder than the fungible-liquidity case that already failed to parallelize here. **OpNS already solves the hard part** (see `Future-Features/Decentralized-Naming/`) with consensus-enforced uniqueness and no patent attached.

**Not for the on-chain backup.** Cost is raw bytes; a covenant wrapper adds bytes and buys semantics we don't need.

**Yes as a technique to understand.** If the ecosystem converges on covenant enforcement over standard token rails — which is exactly where the Aug 2026 thread landed, with shruggr proposing **binary BSV21 ("Shrug")** and saying *"BOLT provenance on existing BSV21 flow could be really powerful"* — then latching is the underlying primitive. Shrug moves token `id`+`amt` out of JSON into locking-script data pushes (~55–62 B vs BSV21's 154 B) specifically so a covenant can byte-compare the 36-byte id against a sighash preimage outpoint. **That is the mechanism that would join BOLT-style enforcement to BSV21 rails.**

**Posture: watch + engage.** We know the authors. Track whether (a) a sharding spec is published, (b) Elas grants a written license, (c) Shrug/binary-BSV21 lands as a BRC.

---

## Sources

- Whitepaper: `https://boltassociation.com/assets/BOLT-Protocol.pdf` (44pp, Honohan, 2024-02-11)
- Block 957547 via WhatsOnChain; ARC policy from `arc.taal.com/v1/policy` + `arc.gorillapool.io/v1/policy` (both 100 sat/KB)
- Debate: `x.com/kurtwuckertjr/status/2085067692646564033` and replies
- nChain White Paper #1605, *PUSHTX & its Building Blocks* (cited by the BOLT paper as prior art)
- Shrug: `https://docs.1satordinals.com/fungible-tokens/shrug.md`

## Handling note

The whitepaper contains an off-topic remark about Charles Hoskinson's ethnicity. It is in the source document, not an extraction artifact. **Do not quote or link the whitepaper in Hodos public-facing content without accounting for this.** It has no bearing on the technical assessment above and is recorded here only so the decision isn't made twice.
