# Convergence Narrative — The Pitch Story

**Status:** Working draft. The Route-B narrative ratified 2026-05-30. Refinement continues through the all-hands meeting + Jake/John meetings.
**Purpose:** The story we tell — to Build AI on AWS judges, to Jake/John in meetings, to Beck/Futran in partner conversations. Distinct from the technical architecture; this is the *positioning*.

---

## The one-paragraph version

Four engineers met at the Babbage BSV Hackathon in Austin, Texas in April 2025. Three of them placed top-three: Ishaan Lahoti (1st, $30K, GitSats — a GitHub bounty escrow on BSV smart contracts), Matt Archbold (2nd, $15K, "Cookies"), and the Calhoun Brothers — John and Matt Calhoun (3rd, $10K, Thryll Arcade). Mitch Burcham was there too, competing with another team. Over the year that followed, each independently built a piece of the same vision:

- **John Calhoun** built **Dolphin Milk** — an open-source Rust agent that pays for its own LLM inference through micropayments, with no API keys
- **Jake Jones** — Head of Infrastructure at the BSV Blockchain Association during Teranode's development — built **Edwin**, a personal AI assistant with the cryptographic security model where the agent literally cannot sign anything without a user-issued envelope
- **John Calhoun + Mitch Burcham (Binary Distributed Technologies)** are now building a **permissionless threshold-signing network** for BRC-100 wallets — Rust/WASM, open source, based on CGGMP'24 — where the private key never exists in one place
- **Matt Archbold (Marston Enterprises)** built **Hodos Browser** — a Web3 browser with the native BRC-100 wallet that all of the above target

**Three of the four components are built and working today.** They've never been integrated, but they were all designed against the same standards (BRC-100, BRC-31, BRC-42), so integration is engineering, not invention. The fourth piece — the threshold-signing network — is the infrastructure layer being built right now by the partner team. **What we're pitching is the engineering work to integrate these into a single consumer product and to deploy a Hodos-operated signing node on AWS as one operator in the permissionless network.**

That's the convergence. Not a coalition formed to win a pitch; a coalition that's been building toward the same vision for a year and now needs the integration push to ship.

---

## Why this story works

It's *true*. Every claim above is verifiable. Hackathon placements are in CoinGeek. Teranode title is in BSV Association announcements. John's threshold-signing announcement is on his public X. Ishaan and John are already in Hodos's beta codebase. None of this is marketing fiction.

It positions Hodos as **the integration layer**, not "another browser." That's a sharper claim than "we make AI safe and affordable" — it says we're the *coordination point* for an ecosystem the BSV community has been building independently.

It honors all four founders. Nobody is a junior partner here. John has the agent runtime AND co-owns the signing network. Jake has Teranode credibility AND owns the security primitive. Mitch has the BINARY infrastructure AND the Mines connection. Matt has the browser AND coordinates the pitch. Four engineers, four pieces, one product.

It makes the grant ask *small and credible*. We're not asking AWS/Futran/Beck to bet on a 4-pillar startup. We're asking for the engineering hours to glue together components that already exist. That's a $25K-sized ask, not a $25M-sized ask.

---

## The pitch framing — three layers, fourth integration

This is the slide structure. Three messages, fourth as the ask.

### Layer 1 — Hodos Browser (consumer surface, built)
*Three clicks, not seven terminal commands.* Native BRC-100 wallet at port 31301. Overlay system. HTTP interception. Already manages the Rust wallet and adblock-engine as subprocesses; adding Dolphin Milk follows the same pattern. **Built. In public beta.**

### Layer 2 — Dolphin Milk (AI economics, built)
*Stop paying $75/month for AI subscriptions you barely use.* Open-source Rust agent. Pays for its own LLM inference via x402 micropayments. No API keys, no accounts. ~$0.22/month for the typical user vs. $20+ for one subscription. **Built. Apache 2.0. John on the team.**

### Layer 3 — Edwin Envelope (cryptographic security, built)
*The only agentic browser where prompt injection cannot move money.* Signed-envelope model. The agent never holds keys; every action requires a wallet-signed envelope binding scope + target + payload + TTL. Comet's 6 prompt-injection CVEs in 8 months have shown that policy-based safety doesn't hold; ours is cryptographic. **Built. Jake on the team — the former Head of Infrastructure at the BSV Association who led Teranode's technical development.**

### Layer 4 — Threshold signing network (infrastructure, being built — what we're pitching)
*Even the wallet's master key doesn't exist in one place.* Permissionless P2P signing network. BRC-100 compatible. Rust/WASM, open source. CGGMP'24 protocol. John Calhoun and Mitch Burcham (BINARY) are building it now — they announced it publicly May 4. **Integration with the other three layers + standing up a Hodos-operated signing node on AWS is the engineering work we'd spend the Futran $25K on.**

---

## The honest framing — "built but not integrated"

The deck needs to be explicit about this. Glossing it would feel dishonest and risk credibility loss in Q&A.

> *"These three pieces are built and working today, but they've never been integrated into one product. The integration isn't research — they all target the same standards. It's engineering. The fourth piece, the signing network, is being built by the partner team right now. What we need are the engineering hours to do the integration and to deploy a Hodos-operated signing node on AWS as part of that network."*

This is the cleanest possible position: confidence in what's built, clarity on what needs work, and a tight scope for the grant.

---

## Why this works for Build AI on AWS specifically

**Futran's worldview:** governance-first AI, AWS-native architectures, billable engineering hours that produce case studies. Our pitch:
- The $25K does AWS-native infrastructure (signing node on AWS, multi-region key sealing via KMS, attested isolation via Nitro Enclaves)
- The case study is *"how AWS infrastructure enabled the first permissionless threshold-signed agentic browser"*
- The integration testing is real Futran work
- Code-signing pipeline (Authenticode + macOS notarization) is real Futran work

**Beck Venture Center's worldview:** deep-tech engineering, Mines-affiliated builders, novel infrastructure, hard-to-replicate moats. Our pitch:
- Three Mines connections in the convergence: Ishaan (CSM 2019, on BINARY's team and a Hodos beta contributor), Mitch (CSM dropout, co-built the signing network), Matt Archbold (took Audrey's Boulder SBDC marketing class)
- Jake's BSV-Association infrastructure architect background — the kind of credential Beck respects
- The signing network is BRC-100 compatible, Rust/WASM, open source — exactly the new-standard-for-new-industry pattern Beck's portfolio funds

**SBDC's worldview:** viable small businesses, US-based, real revenue path. Our pitch:
- Revenue model is concrete: Hodos service fee on transactions + signing-node operator revenue (per-signature micropayments) + future enterprise distribution channels
- Team is real (four engineers + one marketing/finance lead in Matt Calhoun)
- US-based (Marston Enterprises, Colorado)

**For the BSV question specifically:** *"AWS published a million-TPS case study on their own Web3 blog in March 2026. Jake was Head of Infrastructure at the BSV Association during the development that led to that achievement."* That's the AWS-first answer + the team-credential answer in one breath.

---

## What goes on the team slide

Names, with verified facts only:

> **Matt Archbold (Marston Enterprises, BSVArchie)** — Hodos Browser. Built the Chromium fork + Rust BRC-100 wallet. 2nd place at the 2025 Babbage BSV Hackathon in Austin.
>
> **John Calhoun (Calgooon Inc., @johncalhooon)** — Dolphin Milk + x402agency + co-building the threshold-signing network with BINARY. 3rd place at the 2025 Babbage BSV Hackathon (Thryll Arcade, with brother Matt Calhoun).
>
> **Jake Jones (On Chain Innovation Ltd.)** — Edwin. Former Head of Network Infrastructure at the BSV Blockchain Association, leading Teranode's technical development. Project Owner on the BSV Blockchain overlay-services GitHub.
>
> **Mitch Burcham (Binary Distributed Technologies, BINARY)** — Co-building the threshold-signing network with John Calhoun. Colorado School of Mines alum. $50K Babbage hackathon winner. National Director of Blockchain Technologies, Taskforce on Homeland and National Security.
>
> **Ishaan Lahoti (Binary Distributed Technologies)** — Colorado School of Mines (2019). 1st place at the 2025 Babbage BSV Hackathon ($30K, GitSats). Hodos public beta contributor.
>
> **Matt Calhoun (Calgooon Inc.)** — Marketing + finance. Co-built Thryll Arcade with John for the 2025 hackathon.

Six names, six verified credentials, six clear roles. That's a coalition.

---

## What this requires before the application

Three things, in priority order:

1. **All-hands meeting next week.** See `ALL_HANDS_MEETING_PREP.md`. Surface the convergence directly. Get explicit consent from each partner to be named on the application.
2. **Audrey email Monday** + **Jake DM Monday.** Already drafted.
3. **The application content reshapes once the meeting outcome lands.** Don't write the application before the meeting.

---

## What this leaves unresolved (to discuss in the all-hands meeting)

- **Monetization split for envelope-gated wallet actions.** Three-pool model proposed: Hodos service fee ÷ 3 between Hodos, Edwin, and (potentially) signing-network operator pool. Dolphin Milk is separate (already monetized via x402 endpoints). See `ALL_HANDS_MEETING_PREP.md`.
- **TSS as wallet-backup-replacement.** Matt's insight: current Hodos backup is buggy and possibly not economically feasible. TSS replaces backup architecturally. Requires Mitch + John alignment on Hodos as an integration target.
- **Signing-node operator status.** Hodos operates its own node (Futran AWS work), or just integrates as a consumer? Probably both.
- **Naming on application.** Who's OK being named explicitly? Who wants partner-credit-only?
- **The pro-skier comment.** Mitch dropped out of Mines to be a pro skier until he got hurt. Witty line in the pitch for the Boulder/Golden room (Mines + ski-country audience). Holding for Mitch's OK.

---

## Related

- `ALL_HANDS_MEETING_PREP.md` (sibling) — meeting agenda, convergence map, monetization framework
- `ARCHITECTURE_TECHNICAL.md` (sibling) — the 3-layer technical architecture the pitch describes
- `THRESHOLD_ECDSA_EXPLORATION.md` (sibling) — what the 4th layer is
- `../../marketing/intelligence/features/Dolphin Milk + Edwin Integration/NETWORK_CONNECTIONS.md` — verified facts on each team member
- `../../marketing/intelligence/features/Dolphin Milk + Edwin Integration/FUTRAN_SOLUTIONS_PROFILE.md` — what Futran wants
- `../../marketing/intelligence/features/Dolphin Milk + Edwin Integration/BECK_VENTURE_CENTER_PROFILE.md` — what Beck wants
- `BSV_OBJECTION_HANDLING.md` (sibling) — the AWS-first answer for BSV defense
