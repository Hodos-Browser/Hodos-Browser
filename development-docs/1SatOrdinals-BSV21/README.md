# 1Sat Ordinals + BSV21 — Sprint

**Status:** Not started. Scoped and researched; ready to plan when picked up.

> **Was `Sigma-BRC121-Sprint/phase-3-ordinals/`.** Split into its own sprint 2026-08-15 — it never
> depended on the BRC-100 shim work beyond call routing, and it is large enough to stand alone. The
> parent sprint is archived at `archived-docs/Sigma-BRC121-Sprint/`.

## ⛔ Read before starting this sprint

1. **`development-docs/BSV-Tokens/` — all seven documents.** Pre-existing research this sprint builds
   on. **Per-file trust ratings are at the bottom of this document** ("Existing research to
   evaluate") — two are marked do-not-trust and one must be redone from scratch. Read the ratings
   before you read the docs.
2. **The BRC-147 / BRC-150 decision immediately below** — it supersedes any design in BSV-Tokens that
   predates 2026-08-05.
3. **`development-docs/Future-Features/Decentralized-Naming/`** — OpNS names *are* 1-sat ordinals, so
   naming shares this sprint's substrate. See "Naming & OpNS" below.

Our own BRC draft on name resolution lives outside this repo, at
`Marston Enterprises/Standards/BRCs/drafts/consensus-unique-name-tokens.md`.

---

## ⭐ DECISION (2026-08-05): implement to BRC-147 + BRC-150

> **This doc predates both standards. When we build the 1Sat code, we follow BRC-147 and BRC-150. Owner-approved 2026-08-05.**

Two standards were published (Brandon Cryderman / HandCash — see `Marketing/Profiles/bsv/brandon-cryderman.md`) that define exactly the surface this phase was going to have to invent:

| BRC | What it defines | Why it matters to us |
|---|---|---|
| **[BRC-147](https://github.com/bsv-blockchain/BRCs/blob/master/tokens/0147.md)** — 1Sat Ordinals Basket Profile for BRC-46/BRC-100 | Reserves basket name **`1sat`**; eligibility (`satoshis === 1`); tag vocabulary (`ordinal`, `origin:<outpoint>`, `name:`, `app:`, `collection:`, `creator:`); the `customInstructions` JSON schema; dot↔underscore outpoint normalization | This *is* the "UTXO classification" bullet below. Don't design our own basket/tag scheme. |
| **[BRC-150](https://github.com/bsv-blockchain/BRCs/blob/master/tokens/0150.md)** — 1Sat Provenance Remittance | An offline-verifiable `provenance` object in `customInstructions`: `{v:2, origin, tip, path[], beefB64, contentType}` using **AtomicBEEF (BRC-95)** + local check of the first `ord` envelope | **"Verification does not require a global ordinals indexer."** This is our best mitigation for indexer-dependency risk (see the warning below). |

**We already have every primitive these need** — `output_baskets` + tags, `customInstructions` (BRC-37), BEEF/AtomicBEEF, SPV, and `createAction` accepting arbitrary locking **and** unlocking scripts with the two-phase `signAction` flow. This is closer to wiring than to building.

### Two rules from BRC-147 that are load-bearing for us

1. **Tags are claims, not proof.** "A malicious or buggy sender can attach another inscription's origin to an unrelated 1-sat UTXO." Without a *verifying* BRC-150 package, the receiver **MUST NOT** present sender-supplied `name`/`app`/`origin:` as authoritative. Our UI must distinguish *verified* from *claimed* provenance.
2. **Spending a `1sat` output is NOT a BRC-29 payment.** BRC-147: "a general 'pay' or auto-pay grant **MUST NOT** authorize spending them." → This must be enforced in the **Rust permission engine** (`hodos_permission_engine`), not just the UI. A 1-sat ordinal caught by ordinary coin selection is a **permanently destroyed asset** — spending it into a >1-sat output annihilates the origin. Treat this with the same seriousness as the privacy-perimeter gates.

### ⚠️ Indexer dependency — decide this deliberately (research 2026-08-05)

Discovery is the unsolved half, and the honest picture is worse than the ecosystem markets it:

- `shruggr/1sat-indexer`, `b-open-io/1sat-stack`, `bsv21-overlay` and `1sat-sdk` all carry **NO LICENSE** (= all rights reserved — **not legally forkable or self-hostable**). Only the spec (`BitcoinSchema/1sat-ordinals`) is CC0. JungleBus **server** is closed source.
- BSV21 indexing is **metered and permissioned**: `@1sat/actions` adds a `fee:overlay` output of **1,000 sats per token output**; tokens carry a prepaid balance that halts indexing at zero; live `is_whitelisted` / `is_blacklisted` flags exist.
- BRC-22/24/88 overlays are *architecturally* federated (SHIP/SLAP puts host discovery on-chain), but `1sat-stack` has **zero** hits for `SLAP` / `tm_ship` / `Advertiser` — federation isn't wired up, so hosts are known out-of-band today.

**Consequence:** BRC-150 removes the indexer from **verification**. It does not remove it from **discovery**. Choose the discovery path *before* building on it, not after — this is a strategic decision, not an implementation detail.

### Scope note on BSV21 vs ordinals

BRC-147/150 cover **1Sat ordinals (NFT-style)**. **BSV21 fungible tokens are a separate protocol** (`{"p":"bsv-20","op":"transfer","id":...,"amt":...}`) with **no on-chain enforcement at all** — miners validate the satoshi, never the token amount. See `development-docs/BSV-Tokens/BSV_TOKEN_PROTOCOLS_COMPARISON.md` for the full comparison, and note **OpNS names are ordinals, not BSV21.**

---

Sigma auth (Phase 2) alone unlocks app discovery in Cluster B (users can sign in to 1sat.market with their Hodos identity). Ordinal *transfer* requires substantial new work and is a separate sprint:

- New UTXO classification (1-sat outputs with inscribed data)
- BSV20/21 token indexer integration
- New ordinal transfer transaction builder
- New monitor task for indexer sync
- UI for inscriptions

## Naming & OpNS — adjacent check (added 2026-07-15)

**Small, scoped check — because OpNS names *are* 1-sat ordinals**, Phase 3's ordinal machinery (UTXO classification, indexer/overlay client, transfer builder) is the natural substrate for naming too. This is a *check + possible prototype*, not a commitment to build a full naming feature in this phase.

**Canonical analysis lives outside this sprint:** `development-docs/Future-Features/Decentralized-Naming/OPNS_REVIEW.md` (deep teardown + reference links) and that folder's `README.md` (current direction). Current lean = **OpNS**, posture **"engage, prototype, watch."** Read those first.

### First, understand how the pieces fit together

Before any code, map how these interrelate (they're layers, not competitors):

| Piece | Role | Hodos's relationship |
|---|---|---|
| **JungleBus** | GorillaPool BSV "firehose" that crawls every tx and feeds the indexers | We **do not** consume it directly — the indexer/overlay does |
| **1Sat Ordinals** | Base layer: inscriptions on 1-sat outputs; provenance by origin | Phase 3 core — OpNS names + on-chain content both ride on this |
| **Ordinal publish / inscription** | Minting content on-chain (React Onchain, ORDnet content, etc.) | An OpNS name can *point at* published content |
| **OpNS** | Covenant-secured human-readable **names** that are 1-sat ordinals; bind to a BRC-100 identity key (`opns.idKey`); resolve via **ORDFS** to a payable BRC-29 address | Consume its overlay/ORDFS resolution as a client |
| **ORDnet** | Centralized `.web3` name→TXID DB + on-chain content | OpNS is the *decentralized alternative* to ORDnet's name layer |

### Possible prototype (the concrete, cheap step)

**Read-only OpNS paymail resolver in the send form.** Type an OpNS name → overlay lookup for origin → ORDFS crawl to current outpoint → read `opns.idKey` → BRC-29 derive the destination → populate the send. Read-only, reversible, dogfoods the primitive, and becomes a concrete artifact to design-partner with shruggr on. (Also a natural place to resolve OpNS names in the omnibox for on-chain content.)

### Open question to answer during the check: what integrations work with OpNS *and* Xanaverse?

They're **complementary primitives that bind to the same BRC-100 identity key**:
- **OpNS** → a human-readable **name** on your identity key.
- **Xanaverse `UserRegistry`** → a unique on-chain **number** ("verified forever #43") on your identity key (Sybil-resistance / verification anchor; see `Decentralized-Naming/Xanaverse-Contracts-Review/REVIEW.md`).

Candidate integrations to evaluate:
1. **Send-form / omnibox resolver** (OpNS name → payable address) — the prototype above.
2. **Wallet display + management of OpNS names the user owns** (they're ordinals — falls out of the Phase 3 ordinals UI).
3. **Bind the user's Hodos identity key to an OpNS name** (register/bind flow), and optionally **register that same key in the Xanaverse registry** for a verified-unique number.
4. **The synthesis:** `matt` (OpNS name) → identity key → also holds Xanaverse `#43` → resolves to a BRC-29 payable address. Name + verified-unique-number + payment, all on one identity. Worth raising with both shruggr and Calhoun.

### Open design decisions — resolve in EARLY PLANNING, not now

Flagged so the design-decision phase actually researches them (do **not** hardcode an answer during the check):

- **Resolution dependency — the big one.** Do we resolve OpNS names via shruggr's **hosted `1sat.app` overlay** (his paymail domain is `@1sat.app`; `1sat.name` is the registrar UI), or **independently** — via **JungleBus** and/or our own or other **federated-overlay** endpoints? His overlay + registrar are open-source / self-hostable (confirmed via his 2026-07-15 posts), which keeps the independent/federated path open — but *which* path we take, and whether we avoid a single-hosted-endpoint dependency, needs its own research in the design phase. This ties directly to the federated-overlay + micropayment thesis in `Decentralized-Naming/README.md`. **Research target, not a decision yet.**
- **Do we even want `1sat.app` names**, or only *resolve* them (read-only) while minting/binding stay out of scope? Decide when we plan the build.

**Scope discipline:** for this phase, the *check* (understand the stack + decide feasibility) and at most the *read-only resolver prototype* are in scope. Registration, binding, the Xanaverse synthesis, and the resolution-dependency decision above are engage/watch/plan items, not Phase-3 builds.

## Existing research to evaluate

`development-docs/BSV-Tokens/` contains pre-existing research:

- `BSV_TOKEN_PROTOCOLS_COMPARISON.md` — likely trusted (raw protocol facts)
- `BSV21_1SAT_ORDINALS_ANALYSIS.md` — likely trusted (raw analysis)
- `MNEE_STABLECOIN_IMPLEMENTATION.md` — mixed trust
- `BSV21_PLAN_A_BACKEND.md` / `BSV21_PLAN_B_FRONTEND.md` — design decisions, **do not trust without re-review**
- `BSV21_UX_DESIGN_OUTLINE.md` — **do not trust**, redo from scratch when Phase 3 activates

When this sprint activates, extract the trusted raw research into `research-extracted/` here, and
consider moving the whole `BSV-Tokens/` folder in at that time.
