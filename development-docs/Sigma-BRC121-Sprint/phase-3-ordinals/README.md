# Phase 3 — 1Sat Ordinals (Deferred)

**Status:** Deferred to a later sprint.

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

When Phase 3 activates, extract trusted raw research into `research-extracted/` here. Folder will be moved into this sprint at that time.
