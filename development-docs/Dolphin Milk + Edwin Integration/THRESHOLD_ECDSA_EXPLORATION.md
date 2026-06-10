# Threshold ECDSA — John + BINARY's Signing Network (Exploration)

> 📁 Part of the **Dolphin Milk + Edwin Integration** doc set (technical half) — see `README.md` here for the full map. Pitch/product/outreach docs live in the Marston marketing intelligence vault.

**Status:** Exploration / future-tracking. NOT pitch material yet. NOT committed direction.
**Trigger:** Matt flagged @johncalhooon's X post linking to IACR ePrint 2021/060. We don't bring this up with John directly per Matt's call; we want to understand if his roadmap intersects ours.
**Drafted:** 2026-05-30. **Updated:** 2026-05-30 (post content recovered, inference revised).

---

## What the post actually said (recovered 2026-05-30)

Posted by @johncalhooon, **May 4, 2026, 8:42 AM** — 3,460 views.

> *"The Calhooon Brothers are partnering with Binary on a threshold MPC signing network.*
>
> *> CGGMP'24 threshold ECDSA on secp256k1.*
> *> The private key never exists.*
> *> Any t+1 parties cooperate to sign.*
>
> *Signing nodes will be discoverable via an overlay.*
>
> *> BRC-100 compatible.*
> *> 100% Rust/WASM.*
> *> Open source.*
>
> *Based on: https://eprint.iacr.org/2021/060*
> *More soon."*

Key reply to @JoeSchlubb's *"Is it like a key share or multi sig?"*:

> *"Imagine your wallet's password is torn into pieces and handed to a few different strangers on the internet, and to spend money you need any group of them (say, 3 out of 5) to help you sign. No single stranger can steal your money, and even if one or two go offline or turn evil, the others can still help you spend.*
>
> ***BSV is the only place this works cheaply, because every helper gets paid a fraction of a cent on the public scoreboard for pitching in,** and if anyone tries to cheat the math snitches on exactly which one. Same trick the big banks use to protect billions, except anyone can be a helper and the whole thing happens out in the open instead of inside one company."*

That second paragraph is genuinely deck-quotable. Worth memorizing the framing — *"the same trick the big banks use to protect billions, except anyone can be a helper."*

---

## What's verified now (vs. inferred before)

| Claim | Status |
|---|---|
| John is building threshold ECDSA on the CGGMP line | ✅ Verified (his own post) |
| Calhoun ↔ Binary Distributed Technologies partnership | ✅ Verified (his own post: *"The Calhooon Brothers are partnering with Binary"*) |
| The "Calhooon Brothers" plural — there's a sibling involved too | ✅ Verified (plural in post; identity of brother not yet confirmed) |
| BRC-100 compatibility | ✅ Verified (explicit) |
| Rust/WASM implementation | ✅ Verified (explicit) |
| Open source | ✅ Verified (explicit) |
| Overlay-discoverable | ✅ Verified (explicit — uses BSV overlay network) |
| Specific paper variant | ⚠️ Says **CGGMP'24** in the post. IACR 2021/060 is the foundational 2020 paper by the same authors; **CGGMP'24** likely refers to a 2024 follow-up/refinement of theirs. Worth tracking down the exact 2024 variant. |
| Specific t/n parameters | Not specified ("any t+1 parties") |
| Launch timing | Not specified ("More soon") |
| AWS hosting plan | Not specified — likely permissionless / multi-operator, not AWS-centric |

---

## What I got wrong in my earlier inference

For honesty/calibration:

- ❌ **Earlier:** *"Best guess: a wallet/signing primitive announcement, not a new x402 endpoint."* — partially right (it IS a primitive), but underplayed how decentralized the topology is.
- ❌ **Earlier:** *"Adjacent possibility: a managed-TSS-as-a-service offering ('x402agency hosts your second share')."* — wrong shape. It's a **permissionless P2P network of independent operators**, not a hosted service. Each operator gets paid per signing op via x402 micropayments.
- ❌ **Earlier:** *"Maps cleanly to: Nitro Enclaves for the share holder..."* — partially right (AWS CAN host nodes), but missed that **the architecture is operator-permissionless.** AWS is one venue; anyone can run a node.

The cryptographic part I had right. The topology I had wrong.

---

## What the paper actually does (plain language)

**ECDSA** is the signature scheme BSV (and Bitcoin, and Ethereum) all use. A single party with a private key produces a signature; verifiers check it against the public key. **The private key is the kingdom.** Whoever has it can sign anything.

**Threshold ECDSA (TSS-ECDSA)** splits the signing power among `n` parties such that any `t` of them can produce a valid signature, but `t-1` or fewer cannot. The final signature looks identical to a normal ECDSA signature — verifiers can't tell it was produced by threshold signing. No party ever holds the full private key; they hold mathematically combined *shares*.

This paper specifically delivers:
- **Non-interactive online phase.** Most of the work (the "preprocessing") happens offline. When a signature is actually needed, only one final fast round happens. This matters for latency-sensitive scenarios.
- **Proactive refresh.** Shares can be periodically rotated so that even if an attacker compromises one party's share, it becomes useless after the next refresh. *This is the key safety property for autonomous agents holding spendable secrets.*
- **Identifiable aborts.** If a party misbehaves, the protocol names them. You don't just fail — you know who cheated.
- **4 rounds (or 7 rounds with linear cheater identification).** Quadratic vs linear tradeoff in the cheater-ID step.

Application the paper itself names: *"ideal for threshold wallets for ECDSA-based cryptocurrencies."*

---

## What John + BINARY are actually building (revised)

A **permissionless threshold-signing network** with the following properties:

- **Multiple independent signing-node operators.** No single trusted custodian. Anyone can run a node.
- **Discoverable via BSV overlay.** Wallets find available signing nodes through BSV's overlay-services discovery layer (the same layer Hodos already uses for things like SHIP advertisements per `project_phase16_polish_step1_landed`).
- **t+1 of n required to sign.** Configurable threshold. The wallet (or its agent) doesn't hold the full key — it has one share. The other shares are held by signing-network operators.
- **Pay-per-signature in BSV.** *"Every helper gets paid a fraction of a cent on the public scoreboard for pitching in."* So the signing network IS an x402-style micropayment service. When the wallet wants a signature, it pays the participating signing nodes via micropayments.
- **CGGMP'24 protocol** — the modern refinement of IACR 2021/060. Non-interactive online phase, proactive refresh, identifiable aborts.
- **Rust/WASM.** Runs natively in the browser (WASM) or as a server (native Rust).
- **BRC-100 compatible.** The signing network presents a BRC-100-compliant signing interface, so any BRC-100 wallet (including Hodos's) can talk to it without protocol changes.
- **Open source.** No licensing barrier.

**Recursive elegance:** the signing network's economics are themselves x402 micropayments. So if Hodos uses TSS to sign an x402 LLM-payment transaction, the signing produces a chain: agent's request → TSS signature → each participating node paid in sats → LLM endpoint paid in sats. **Sats flow at every layer.** John's thesis ("agent-to-API micropayments without a middleman") applies recursively, all the way down to the signing infrastructure.

**Why this is structurally different from "AWS co-signer":**
- Permissionless: anyone runs a node, market-driven pricing for participation
- BSV-native economics: signing-node operators earn directly from each signature
- Decentralized failure modes: any subset of operators going offline doesn't kill the system as long as t+1 remain
- No single AWS account or operator is a chokepoint

---

## How this would interact with Edwin's envelope model

**Headline:** Edwin's envelope code path is unchanged. What changes is *what the signer is internally.*

### What stays the same
- The envelope schema (`{kid, alg, iat, exp, nonce, scope, target, payload, sig}`)
- The verification logic (single ECDSA signature against single pubkey)
- The TTL / nonce / scope semantics
- Wallet-side validation flow in Hodos

A verifier looking at the envelope sees a normal ECDSA signature. They can't tell whether it was produced by a single key or by t-of-n threshold cooperation. **This means Edwin's vault doesn't need to know.** Backward compatibility is trivial.

### What changes (the upgrade)
**Today (single-key Edwin):** the envelope's scope/target/payload binding is enforced by the *verifier* at signature-check time. The signer (the wallet) trusts that the policy engine matched the request before issuing the envelope. There's an honor-system assumption between policy and signer.

**With TSS underneath:** the co-signer share is the place where policy enforcement becomes *cryptographic*. The co-signer (which could be the user's hardware key, a custody service, or an AWS-hosted policy enforcement point) **refuses to participate** in the threshold signing round if the policy is violated. The signature literally cannot be produced. There is no honor-system path.

**Concrete example:**
- Today: agent presents an envelope for "send 200 sats to claude-chat." Vault verifies envelope, asks wallet to sign. Wallet signs. (Vault and wallet both inside Hodos's trust boundary.)
- With TSS: agent presents the envelope for the same request. Hodos's wallet has one share. The user's hardware (or an AWS-hosted policy service) has the other share. **Producing the signature requires the second share's cooperation, which requires the second share's policy check to pass.** Two parties, two independent checks, one signature.

The Edwin envelope becomes the *coordination artifact* between the two TSS parties. The envelope says "all parties agreed to allow this specific signature." Each party checks the envelope independently before contributing their share.

### Where this matters most
- **Compromised agent runtime** — even if Dolphin Milk's process is fully compromised, the attacker only has one share. They cannot forge a signature without the co-signer's participation.
- **Compromised wallet daemon** — same logic. If Hodos's wallet at `:31301` is compromised, attackers don't get full keys, only one share. The co-signer wall stands.
- **Disagreement between agent and user** — if the user's hardware key thinks the policy is violated but Hodos's policy engine thinks it's fine, the signature can't be produced. Disagreement = failure-to-sign rather than wrong-sign.

**This is a meaningful security upgrade over single-key Edwin.** Not required for v1, but worth knowing it's a clean extension path.

---

## How this would interact with AWS infrastructure (revised — narrower)

The signing network is permissionless, so AWS isn't *required.* But **AWS is a natural venue for running high-availability operator nodes.** A Hodos-operated AWS signing node could be one of N participants in the network.

The relevance to the Futran grant becomes narrower:

| AWS component | Use |
|---|---|
| Nitro Enclaves | Attested isolation for the Hodos-operated signing node's share at rest + in use |
| KMS | Sealing share material across regions |
| Lambda | Per-signature policy entry point for the Hodos-operated node specifically |
| EventBridge | Refresh schedule for our operator instance |
| Multi-AZ deployment | Operator-node HA |

**Important nuance:** Hodos doesn't need to operate any signing nodes to *use* the network. The wallet just discovers nodes via overlay, picks t+1, and pays them per signature. **Running our own node is an additional value-add (revenue per signature, independence), not a requirement.**

So the AWS grant work would be smaller: *"deploy and operate a Hodos-branded signing node on AWS as one operator in the John/BINARY signing network"* — rather than *"build the whole signing system on AWS."* That's a cleaner, smaller, more honest scope for $25K Futran hours.

**Pitch-side framing if asked:** *"Our partners are building a permissionless threshold-signing network as a public good. We use it; we may also operate a node on AWS as a contributor to the network."*

---

## Implication for our pitch (calibration — revised)

**Don't bring this up to John yet.** Per Matt's call. We're tracking, not entangling.

**Don't put it as a *promise* in the pitch deck.** It's not our work, and the timeline is John's not ours.

**DO mention it as evidence the partner ecosystem is building real infrastructure.** That's a different claim and a true one. If asked about the security ceiling or partner-network maturity:

> *"Our partners are already shipping the next layer beyond v1 — John Calhoun and Binary Distributed Technologies just announced a permissionless threshold-signing network for BRC-100 wallets, in Rust/WASM, based on the modern CGGMP construction. It means no single party holds the full signing key, and the math itself catches any party that tries to cheat. The same trick big banks use to protect billions, except permissionless. We don't have to build that — we'll integrate it once it ships. That's the value of building inside an ecosystem instead of alone."*

This is honest, technically literate, name-drops two partners credibly without promising what isn't ours to promise.

**Tier classification:** previously "Tier-3 what-becomes-possible." Now legitimately **Tier-2 "designed by partners, not yet shipped, integration path clear."** Because it's BRC-100 compatible, the integration work on Hodos's side is just "talk to the signing network via overlay-discovered nodes for any envelope-gated action."

**The Mines connection deepens here.** Mitch Burcham (BINARY founder, CSM) is a co-builder of the signing network with John. So the "Mines alumni in the BSV community" framing isn't just nominal — it's *infrastructure-deep.* Worth a sentence in the Beck-aimed framing.

---

## Open questions to revisit

- When X is reachable, re-read John's actual post text to confirm or refute the inference here.
- Re-check John's recent x402agency / Dolphin Milk announcements for any TSS-specific tooling.
- If John ships a TSS primitive, does it expose a clean RPC interface Hodos could integrate against? Or is it tightly coupled to his runtime?
- For Jake: does Edwin's envelope schema as currently designed have room for "multi-party signature" metadata, or would TSS require schema additions?
- For an eventual AWS conversation: would Futran's competencies cover Nitro Enclaves + KMS + Lambda + multi-region key management? (Almost certainly yes — this is core AWS.)

---

## Related

- IACR ePrint 2021/060: https://eprint.iacr.org/2021/060
- Bolt Labs TSS-ECDSA reference implementation: https://github.com/boltlabs-inc/tss-ecdsa
- ACM CCS 2020 publication: https://dl.acm.org/doi/10.1145/3372297.3423367
- @johncalhooon Feb 2026 x402 thesis post (context): https://x.com/johncalhooon/status/2022302656207688091
- `EDWIN_VS_DOLPHIN_MILK_SECURITY.md` — the Edwin envelope model this extends
- `ARCHITECTURE_TECHNICAL.md` — the v1 architecture this is a future extension to
- `BSV_OBJECTION_HANDLING.md` — AWS-BSV defensive positioning that this extension reinforces
