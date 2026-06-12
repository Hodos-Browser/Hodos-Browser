# Goodwill Bridge Pre-Mint — Web2 → Web3 Launch Mechanism

**Created:** 2026-06-08
**Status:** Idea capture — candidate launch mechanism, not architecture
**Owner:** Matt

## What this doc is

A potential **system-level launch event** for BSV-native naming: pre-mint the existing ICANN namespace (or a curated subset) as on-chain BSV UTXOs locked behind a release covenant. Each UTXO releases to a claimant only on cryptographic proof, via BRC-52 selective-disclosure certificate, that the claimant controls the corresponding Web2 domain — with ICANN (and/or ICANN-accredited registrars) as the canonical attestor.

**This is not a Hodos engineering project.** It is a pitch and a design candidate that Hodos can champion and reference-implement, but that needs to be **coordinated by the BSV Association** in partnership with ICANN (and eventually W3C). See [Strategic positioning](#strategic-positioning) below.

This is **NOT a replacement for the federated-overlay architecture** described in `README.md`. It's a one-time launch mechanism that:

- Pre-populates the namespace with existing brand identities
- Costs in the hundreds to low thousands of dollars in BSV at MVP scope
- Delivers a clean marketing narrative ("your brand is already reserved on BSV")
- Hands ownership to Web2 brand holders via authoritative attestation, not ad-hoc arbitration
- Composes cleanly with the federated overlay, which handles all new names post-launch
- **Generalizes beyond domains** — the same BRC-52 covenant template can later admit attestations from WIPO (trademarks), USPTO, national companies registries, professional licensing bodies, etc.

Captured here so we can revisit when the federated-overlay design is more concrete, and pitch when we get an audience with BSVA.

---

## Why this is interesting

The hybrid does something none of the three approaches in `README.md` (centralized / sCrypt-SMT / federated overlay) does alone — it **separates legacy-identity import from new-identity issuance**, and treats them as different problems with different right answers.

| Problem | Right answer |
|---|---|
| Import the ~150M existing Web2 brand identities so they aren't squatted on day one | Pre-mint + covenant release on DNS proof |
| Issue new names from the unbounded namespace forever after | Federated overlay (BRC-22), per current `README.md` design |
| Discover any name → owner mapping (whether bridged or newly-minted) | Federated overlay — but now overlays only have to *discover*, not enforce uniqueness |

The structural payoff: **discovery overlays stop having to enforce uniqueness for the imported set**, because the chain enforces it via the covenant (one UTXO per name, can't double-spend). Overlays go back to doing the job they're actually good at. The uniqueness-enforcement burden on the federated overlay applies only to names minted after launch.

---

## Strategic positioning

This work is **system-level infrastructure** for the BSV ecosystem, not a Hodos product feature. The right operator is the **BSV Association**, in partnership with **ICANN** and (eventually) **W3C**. Hodos's role is **champion, designer, and reference implementer**, not operator.

### Why BSVA, not Hodos

- **Credibility surface.** ICANN takes meetings with standards bodies and associations, not single-product startups. BSVA is the right counterparty for ICANN; Hodos isn't.
- **Funding.** A serious launch (legal review, attestor infrastructure, ICANN liaison, W3C engagement) is at least mid-six-figures. Out of BSVA's treasury. Out of Hodos's reach.
- **Multi-stakeholder governance.** A bridge that lasts decades needs a governance body. BSVA can credibly hold that role.
- **Posture consistency.** Matches `NARRATIVES.md`: "We're a client. We want partners, not vertical integration. We built it so we don't matter."

### Hodos's actual role

- **Originate the design** (this doc). Publish under our name as the proposal that started the conversation.
- **Reference implementation.** Hodos's wallet already verifies BRC-52 certs and is positioned to render bridged names natively. We can ship the reference rendering client before BSVA picks the proposal up.
- **Small curated demo bridge** (~100 names, friends-and-allies, no ICANN involvement) to prove the mechanic works *before* pitching BSVA. Demonstrates the design isn't theoretical.
- **Champion.** Pitch BSVA when we get an audience. Stay close to the conversation as it moves to ICANN and (later) W3C.
- **Early integrator.** When BSVA + ICANN ship the production bridge, Hodos is the first wallet/browser to render it natively.

That gets our name on the architecture without making us the operator. Same play Vitalik used with ENS — proposed the design, evangelized it, then ENS Labs became a separate organization.

### The pitch escalation ladder

| Stage | Counterparty | What we ask for | Realistic timeline |
|---|---|---|---|
| 1 | BSVA | Adopt the design; fund a pilot | 6–18 months from pitch |
| 2 | ICANN | Bless BRC-52 schema; designate signing authority (ICANN root or delegated to registrars) | 12–24 months after BSVA buy-in |
| 3 | ICANN-accredited registrars | Each registrar issues BRC-52 attestations for its managed domains, DNSSEC-style | Phased opt-in over 1–3 years |
| 4 | W3C / IETF | URI scheme registration; browser-vendor adoption guidance | 3–10 years (IPFS is still waiting after a decade) |
| 5 | WIPO / national IP offices | Trademark attestation for bare names | After domain bridge is mature |

The bridge can ship through BSV-aware wallets long before W3C blesses any URI scheme. **W3C is the endgame for native Chrome/Safari/Firefox support; it's not blocking the launch.**

---

## How it works

1. BSVA broadcasts N UTXOs across a series of batched transactions. Each UTXO encodes one name (or `sha256(name)` to reduce on-chain branding exposure).
2. Each UTXO is locked under a single covenant template that releases the UTXO to a claimant pubkey if and only if:
   - The claimant provides a valid **BRC-52 certificate** attesting "the bearer of pubkey X is the registered controller of domain Y as of block height H"
   - The certificate is signed by an issuer whose pubkey appears in the covenant's approved-issuer set (initially: ICANN root or ICANN-delegated registrar signing keys)
   - (Optional) The certificate is no more than N blocks old to prevent replay
3. The claimant constructs a release transaction spending the UTXO, providing the BRC-52 certificate as a witness. The covenant validates the issuer signature and rebroadcasts the UTXO to a standard P2PKH locked to the claimant's pubkey.
4. After release, the name is a normal self-sovereign BSV UTXO — transferable, sellable, recoverable via standard wallet flows. The covenant is no longer involved.

Names not claimed within the covenant's timeout window (probably 3 years) revert to the federated overlay as available for new registration.

### Why BRC-52 + ICANN dominates ad-hoc DNS TXT proof

Earlier drafts of this doc proposed DNS TXT records signed by an ad-hoc attestor. BRC-52 attestation by ICANN (or registrars) is materially better on every axis:

| Property | DNS TXT + ad-hoc attestor | BRC-52 + ICANN/registrar |
|---|---|---|
| Attestor authority | Whoever we designate | The actual authoritative registry of domain ownership |
| Forgery risk | Compromised DNS hosting reveals attacker | Compromise of ICANN/registrar signing keys is catastrophic but very hard |
| Privacy | Public DNS records expose claim → pubkey binding | Selective disclosure — claimant proves ownership without revealing PII |
| Cross-namespace extensibility | Domains only | Trademarks, government registries, professional licenses — anything an authoritative body would attest |
| User experience | Edit a DNS record (technical) | Click "verify with my registrar" (consumer-grade) |
| Long-term legitimacy | Looks like Hodos rolling its own attestor | Anchored in existing trust hierarchy that already governs domain ownership |

The deeper framing: **BRC-52 is the substrate, ICANN is the first canonical issuer, and the covenant is just "release on valid BRC-52 cert from one of N approved issuers."** The covenant doesn't change as new issuers are added. WIPO for trademarks, national companies registries, professional licensing bodies — they all plug into the same primitive.

That makes this much more than a domain bridge. It's a **general substrate for porting any authoritative real-world identifier onto BSV.**

---

## Cost envelope

Rough numbers on BSV at $30/BSV, ~0.5 sat/byte fee, 18 sats per minted UTXO (output + amortized fee).

| Scope | Names | BSV cost | USD cost | Broadcast time | UTXO-set delta |
|---|---|---|---|---|---|
| **Top 10K brands** (curated) | 10,000 | 180k sats | $0.05 | <1 block | ~500 KB |
| **Tranco top 1M** | 1,000,000 | 18M sats | $5.40 | ~1 block | ~50 MB |
| **All ICANN SLDs, deduped** | ~150M | 2.7B sats | $810 | ~2 blocks | ~7.5 GB |
| **All ICANN SLDs, NOT deduped** | ~370M | 6.7B sats | $2,000 | ~5 blocks | ~18 GB |

Costs are roughly linear up to ~1B UTXOs (~$6K). The cost is not the binding constraint; **legal review and attestor design are.**

---

## Where the design isn't settled

### 1. Attestor model — the strategic question

A covenant on BSV cannot verify domain control on its own — it can only verify a signature. So *somebody* has to attest "the bearer of pubkey X is the registered controller of domain Y" and sign that in a form the covenant accepts.

The strategic options, ordered by long-run legitimacy:

| Attestor model | Trust surface | Realism |
|---|---|---|
| **ICANN root signing key** | One institutional actor; treaty-bound | Cleanest legitimacy; ICANN governance is slow but precedent exists (DNSSEC root key signing ceremonies) |
| **ICANN-accredited registrars, DNSSEC-delegated** | Each registrar attests for its managed domains; ICANN is meta-authority | More realistic politically; opt-in per registrar; closer to how DNSSEC actually works |
| **BSVA m-of-n federation until ICANN engages** | Multi-party honesty; BSVA-affiliated members sign | Realistic bridge state for the 2–5 years before ICANN approval; bounded centralization |
| **Hodos demo-only attestor** | One actor, tiny scope, no production use | Right for the pre-pitch curated demo only |

**Likely trajectory:**
1. Hodos runs a ~100-name demo bridge with Hodos as sole attestor to prove the mechanic works.
2. BSVA picks up the design and runs a 10K-name beta with a BSVA-affiliated attestor federation.
3. ICANN engagement happens in parallel. Once ICANN blesses BRC-52 issuance (by ICANN root or via registrar delegation), the covenant's approved-issuer set is updated and the bridge expands to the full ICANN namespace.

**The covenant template doesn't change across these stages — only the issuer set does.** That's the key architectural property. New attestor authorities (WIPO for trademarks, registrars, national IP offices) plug into the same covenant primitive without redeployment.

### 2. Don't dedup at MVP — and treat bare names as their own phase

`nike.com`, `nike.co.uk`, `nike.cn` dedup to one `nike` — but whose proof wins? Arbitrary priority rules (`.com` first) make us the arbiter of who-gets-which-brand, which is exactly what we're trying to avoid. Real holders of each TLD instance have legitimate, distinct claims on their respective tokens.

**Working answer: don't dedup.** Pre-mint each `<name>.<tld>` as a separate token. Less elegant, no priority disputes, and the UX layer can do post-claim merging if it wants. This also halves the legal surface — we're not deciding "Nike Inc. owns the canonical `nike`," we're just mirroring ICANN's existing namespace.

#### The bare-name question (Phase 4+)

There's a strong intuition (and we share it) that **someone should own just `nike`** as the Web3 base — not `nike.com` or `nike.org`. That instinct is correct AND it's the hardest part of the whole design, for two reasons:

1. **Different attestor authority.** ICANN doesn't own trademarks — WIPO does (international), USPTO does (US), national IP offices do (everywhere else). For `nike` (bare), the attestor chain has to involve trademark offices, not domain registries. Different bodies, different procedures, different timelines, different jurisdictions.
2. **Genuine ambiguity.** `apple` the fruit company vs `apple` the records label vs `apple` the computer company is a real trademark dispute that ICANN's `.com` registry never has to adjudicate (each got a different TLD or country code). The bare-name namespace forces the dispute.

**My read: bare names are Phase 4+, not Phase 1.** Domain-tied tokens (`nike.com`, `nike.org`, etc.) ship first under ICANN attestation. Bare names come later, gated by WIPO / IP-office BRC-52 attestation, with explicit dispute mechanics (ENS-style sunrise + UDRP-equivalent).

That actually strengthens the BSVA pitch: there's a clear roadmap from launch (domain bridge) → trademark bridge → professional-license bridge → general authoritative-identifier substrate. **Each phase uses the same covenant template; only the approved-issuer set grows.**

### 3. Covenant timeout / expiry

If unclaimed names sit in the covenant forever, the UTXO set carries them indefinitely with no path to reuse them. Three options:

- **Eternal lock** — clean, expensive on UTXO set, namespace permanently consumed
- **3-year expiry → release to federated overlay as available** — gives Web2 owners a deadline, returns long-tail to the open registry
- **Sweep to Hodos treasury after expiry** — worst look, makes us the arbiter

Working preference: 3-year covenant expiry, automatic release back to the federated overlay's open registration pool.

### 4. What "name" means on-chain

The covenant holds millions of UTXOs that each represent one name. The literal string can live in the locking script, an OP_RETURN, or a hashed form. Mild preference for `sha256("nike.com")` in the locking script to keep on-chain branding exposure low — name is recoverable from the genesis transaction's OP_RETURN where Hodos publishes the full bridge manifest.

This affects: storage size per UTXO, ability to enumerate names from the chain alone vs needing an indexer, legal exposure from on-chain brand strings.

### 5. Legal review is non-optional before broadcast

Pre-minting `apple`, `cocacola`, etc. — even with a clean release-on-proof covenant — could be construed as use-in-commerce of trademarks. Mitigations under consideration:

- Encode names as `sha256(...)` rather than literal strings on-chain
- Public attestation that all pre-mints are *reservations, not assets*, with no transferable value until claimed
- Instant transfer to verified owners on request (no waiting on the covenant if the brand asks)
- Skip the most legally sensitive names entirely (no `apple` token; let the federated overlay handle high-risk brands case by case)

**ICANN-attested issuance significantly mitigates this risk.** When the attestor is ICANN (or an ICANN-accredited registrar), the bridge isn't "Hodos minting brand names" — it's "ICANN issuing a BRC-52 certificate that BSV happens to honor." The trademark-use-in-commerce exposure shifts substantially. This is another argument for waiting on ICANN partnership before the full bulk import, and for the Hodos demo to use only non-trademark-loaded names.

For the Hodos pre-pitch demo: stick to ~100 names that are obviously not trademarks (test domains, friends-and-allies, BSV-ecosystem org domains). 30 minutes with an IP attorney before broadcast, even for the demo.

### 6. Post-claim Web2 divergence is a feature (but a UX surprise)

After someone claims `nike` via DNS proof, the token is standard P2PKH. If they later let `nike.com` lapse and a squatter acquires it, the Hodos `nike` token stays with the original claimant. This demonstrates the value proposition cleanly — on-chain identity outlives ICANN's revocable rights — but it's a real surprise for first-time users and a real disagreement vector if the brand later reacquires the domain. Worth disclosing upfront in launch copy.

### 7. Scope discipline — the phased roadmap

Launching with all 150M ICANN SLDs at once is technically cheap (~$800) but creates a large attack surface for attestor disputes, support load, and legal scrutiny. The right shape is a multi-year phased rollout that maps onto the BSVA → ICANN → WIPO escalation:

| Phase | Operator | Scope | Attestor | Trigger to next |
|---|---|---|---|---|
| **0** (demo) | Hodos | ~100 non-trademark names | Hodos sole | Proves the mechanic; pitch BSVA |
| **1** (beta) | BSVA | Top 10K SLDs, curated | BSVA m-of-n federation | ICANN approval to issue BRC-52 |
| **2** (production) | BSVA + ICANN | Tranco top 1M SLDs | ICANN root or accredited registrars | Sustained claim throughput |
| **3** (long tail) | BSVA + ICANN registrar network | All 150M ICANN SLDs | Per-registrar delegated signing | Trademark partnerships in place |
| **4** (trademarks) | BSVA + WIPO | Bare names (`nike`, `apple`) | WIPO + national IP offices | Professional licensing partnerships |
| **5** (general substrate) | BSVA + various | Companies registries, licenses | Multiple authoritative issuers | — |

Phase 0 is the Hodos deliverable — closer to "$5 in BSV + a curated CSV" than to "broadcast the internet." Phases 1+ are BSVA-led.

---

## Failure modes worth thinking through

| Mode | What happens | Mitigation |
|---|---|---|
| Attestor goes rogue | Names release to wrong claimants | m-of-n federation; on-chain attestation logs make dishonesty publicly detectable |
| Reorg during bulk pre-mint | Partial broadcast, double-mint risk | Wait for N-block confirmation per batch; idempotent batch IDs |
| Brand holder can't or won't claim | Name sits in covenant; eventually expires | 3-year expiry returns name to open registry |
| Brand holder claims AFTER expiry | Name may already be re-registered by someone else | Disclose timeline clearly; offer support channel for late claims |
| Squatter races a brand holder to claim | Brand holder loses because they didn't set up DNS TXT in time | Generous claim window; possible "brand priority" attestor that pre-validates trademark holders without DNS step |
| Covenant has a bug | UTXOs locked forever, names un-claimable | Multiple independent audits before broadcast; phased rollout catches issues at small scale |

---

## How this composes with the rest of the system

The bridge is a **seed event**, not a substrate. After launch:

- **All resolution** (bridged names + newly-minted names) goes through the same federated overlay
- **All new names** are minted via the overlay's BRC-22 first-come-first-served mechanism with deterministic block-ordering as the uniqueness anchor
- **All ownership** is standard self-sovereign BSV UTXOs — no covenant in the loop after release
- **All discovery** is standard overlay lookup, paid for via micropayments

The bridge populates the initial UTXO set. The overlay is the discovery and ongoing-uniqueness layer for everything that comes after. They aren't competing architectures — they're two phases of the same system.

This division of labor is structurally cleaner than the all-federated design in `README.md`:

| Concern | Pure federated overlay | Federated overlay + Goodwill Bridge |
|---|---|---|
| Initial Web2 brand mapping | Slow organic; squatter risk on day one | Pre-loaded with covenant-enforced rightful-owner release |
| New name uniqueness | Block-ordering + operator convergence | Same |
| Discovery | Federated query | Same |
| Operator trust surface | Uniqueness enforcement + discovery + dispute hints | Discovery only (uniqueness enforced by covenant for bridged names, by block-ordering for new names) |

---

## Open questions

- Should bridged names have a different visual / wallet treatment ("verified Web2-bridged") vs newly-minted names? Or is that gatekeeping?
- If the attestor federation grows to N members, what's the right `m` threshold? Probably depends on N — `ceil(2/3 N)` is a reasonable default.
- Is there a clean way to let a brand holder pre-fund their own claim transaction so they don't need to hold BSV at claim time? (Probably: Hodos sponsors claim-tx fees during the launch window.)
- Should the covenant accept either (DNS TXT proof) OR (signed letter from the brand on company letterhead, attested by Hodos) as a fallback? Brand legal teams don't always control DNS quickly.
- Does the bridge work for **paymails** too, or only for **domain names**? Probably only domains — paymails have no DNS analog. Worth thinking about whether `name@paymail` for an existing brand should be pre-reserved alongside its domain token, with the same DNS proof unlocking both.
- What's the relationship between this and Babbage's incoming Metanet URI scheme (DNS-binding-via-pubkey)? They might be the same primitive expressed differently — Babbage uses DNS records to bind a pubkey; we'd use DNS records to release a covenant to a pubkey. Worth comparing carefully when the spec lands.
- Is there a non-trademark naming subset worth pre-minting that has lower legal exposure? E.g., common first names, surnames from public datasets, common dictionary words. Less "brand" and more "human identity scaffolding."

---

## Why this is in the folder

This isn't the answer to decentralized naming on BSV. The federated-overlay architecture in `README.md` remains the working answer for ongoing-uniqueness, and Hodos doesn't run this even if it ships — BSVA does.

This **is** plausibly the right launch story for whatever the BSV ecosystem eventually ships in this space. A Web2 → Web3 bridge with covenant-enforced rightful-owner release via ICANN-issued BRC-52 attestation is a much better day-one narrative than "anyone can register any name; first come first served," because the latter immediately puts trademark holders in a defensive posture against the system. The bridge lets BSV launch with the existing brand ecosystem already on-side, anchored in authorities (ICANN, eventually WIPO) that the brand ecosystem already recognizes.

The doc captures the design so we can:
- **Pitch BSVA** when we get an audience
- **Ship the Phase 0 demo** ourselves to prove the mechanic before pitching
- **Hand the architecture to BSVA** if they want to run with it, with Hodos as champion + reference client
- **Watch for parallel work** — Babbage's incoming Metanet URI scheme (DNS-binding-via-pubkey) is the closest existing technique and may converge

Worth revisiting when:
- We have a credible BSVA audience scheduled
- The federated overlay design (`README.md`) has converged enough to be the substrate
- Babbage's Metanet URI scheme has shipped and we can compare DNS-binding mechanics
- Phase 0 demo would meaningfully advance a Hodos product launch

Until then, this sits here as a captured design and a pitch-in-waiting.

---

## Related Hodos docs

- `README.md` (this folder) — the federated-overlay architecture this bridge complements, not replaces
- `BOOTSTRAP_PROBLEM.md` (this folder) — bootstrap theory; this bridge is a concrete bootstrap mechanism
- `NARRATIVES.md` (this folder) — public messaging; "your brand is already reserved" is a candidate frame
- `Paymail/OPEN_PAYMAIL_PROTOCOL.md` — paymail-side may want a parallel bridge mechanism
- `Domain-Names/WEBSITES_ON_CHAIN_RESEARCH.md` — Babbage Metanet URI scheme is the closest existing technique
- `Xanaverse-Contracts-Review/REVIEW.md` — sCrypt-SMT covenant pattern; same on-chain enforcement primitive used differently here
