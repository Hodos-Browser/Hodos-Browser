# Websites on the Blockchain — Research Outline

**Status:** Initial outline. Not a roadmap yet. Research seed for future feature/design decisions on hosting, addressing, and rendering blockchain-native websites in Hodos.
**Created:** 2026-05-05
**Owner:** Matt
**Type:** Research → product decisions → marketing campaign once shipped

---

## Why this matters for Hodos

The browser is the rendering layer. If the BSV ecosystem converges on a workable "websites natively on-chain" pattern (URI scheme + storage primitives + addressing), Hodos is the natural client to render it. We don't need to *build* the stack from scratch — we need to understand what's emerging, decide which conventions to support natively, and ship the support before users have to install something else to access on-chain content.

This is also a Chapter 5 candidate in the canonical storytelling arc — "the web that was always supposed to exist" — once we have a concrete demo to point at.

---

## Research targets

Each section: what we know, what to dig into, links.

### 1. Unwriter's Bottle browser (history)

**What we know now:**
- Unwriter is a long-running BSV-ecosystem developer; Bottle was an early attempt at a blockchain-native browser/protocol layer.
- Handle spelling needs verification (could be `@_unwriter`, `@unwriter`, or different).
- Earlier Hodos reply (April 2026) referenced the B:// protocol via @staub_u_356; relevant.

**To dig into:**
- Original posts/repos for Bottle browser (probably 2019–2021 era).
- What URI scheme(s) it used (`b://`, others?).
- Why it didn't take off — technical limits, ecosystem timing, abandoned, or still active.
- Lessons applicable to Hodos's potential support of similar primitives.

---

### 2. Maxthon (browser-side context)

**What we know now:**
- Already covered in `memory/reference_hodos_voice_examples.md` Maxthon-reply template (April 11 reply to @eightyATE).
- Maxthon attempted BSV-native browser features around 2020.
- Hodos's framing: "We agree with the vision Maxthon expressed in 2020. Three things have changed since then: Teranode, BRC-100, the overlay network."

**To dig into:**
- What URI schemes / addressing patterns Maxthon supported (e.g., NB:// resolution).
- Why the project stalled or didn't ship final BSV-native features.
- Whether they still maintain any related code/specs.

---

### 3. Jeff Chen's NBDomain system

**What we know now:**
- Referenced peripherally in the Maxthon thread context.
- "NBDomain" = Naming-on-Bitcoin domain, presumably tied to NB:// URI resolution.

**To dig into:**
- Spec / repo / docs (find Jeff Chen's GitHub or Medium).
- How registration works (on-chain? overlay?).
- Whether it's still operational.
- Relationship to ICANN-style DNS — does it bind to existing TLDs or operate parallel?
- Any users / sites currently registered under NBDomain.

---

### 4. Project Babbage — "Metanet" URI scheme (announced May 4, 2026)

**Source:** https://x.com/ProjectBabbage/status/2050951722869330388

**Quote (verbatim, paraphrased from post):**
> "Working on a cool new 'Metanet' URI scheme for hosting websites without an IP address or server, no A or AAAA records needed :) just UHRP, KVStore, overlays, and one public key in your DNS records to bind it to what's on BSV. Then every website will get proof of when it was hosted, while users will get the ability to time travel every version. Details to come!"

**Key technical primitives Babbage names:**
- **UHRP** — Universal Hash Resolution Protocol (content-addressed retrieval)
- **KVStore** — key-value store primitive
- **Overlays** — indexer/aggregator services for retrieval
- **One public key in DNS records** — bridge between traditional DNS and BSV-native addressing
- **Proof of when hosted** — timestamped attestation
- **Version time-travel** — content history navigation

**To dig into:**
- Full spec when "details to come" lands.
- How the DNS-binding-via-pubkey model differs from existing approaches (e.g., DNSSEC, DNSLink for IPFS).
- How UHRP content addressing relates to OrdFS / 1Sat ordinal addressing (apples-to-apples or different paradigms?).
- Compatibility / overlap with React Onchain (#5) and ORDnet (#6).

---

### 5. Dan Wagner — React Onchain (announced Nov 2025)

**Sources:**
- Article: https://medium.com/@dan.wagner06/react-onchain-a-step-towards-a-truly-decentralized-web-47e3095339ca
- Thread: https://x.com/danwag06/status/1986434059346584028 (not yet fetched in detail)

**What we know now:**
- React Onchain deploys complete frontend apps (HTML/JS/CSS/images) directly to BSV as immutable artifacts.
- Single command: `npx react-onchain deploy`.
- Cost: under $0.01 per deployment; updates ~3% of initial cost (unchanged files reused).
- Built on: BSV blockchain, OrdFS, B Protocol, 1Sat Ordinals.
- Includes: dependency analysis with onchain reference rewriting, service worker for video range requests (4K seek), onchain version tagging.
- Status: author-acknowledged beta; static-build frameworks (Vite, CRA, Next.js exports) supported.
- Author identifies as React.js-rooted; affiliation not stated in article.

**To dig into:**
- Full thread (paid x-research call ~350k sats, defer until we're closer to needing it).
- GitHub repo for `react-onchain` package.
- How it compares to Babbage's incoming Metanet URI scheme — competitor or complement?
- Whether the rendered apps work in any browser or require an OrdFS-aware client.
- Test deployment from a Hodos-hosted dev page.

---

### 6. ORDnet (ordnet.io) — also flagged via the BSVSearch thread

**Source thread:** https://x.com/BSVSearch/status/2051433812115222849
**Project site:** https://ordnet.io
**Domain registrar (sub-app):** https://domains.ordnet.io — likely the `.web3` claim/registration UI; needs hands-on inspection to confirm whether registration is on-chain (UTXO/inscription), overlay-indexed, or a centralized DB. This is the most concrete artifact in the ORDnet stack to look at first.
**Lead:** ARTaY (per site).

**What we know now:**
- Inscribes HTML/CSS/JS directly on-chain. Wallet-controlled "permanent" websites.
- Pricing: 600 sats/KB for inscription.
- Live, with multiple sub-products (ORD/browser, ORD/mail, ORD/app, ORD/os).
- Free .web3 domains claimable.
- Content retrieval via transaction IDs OR .web3 domain names.

**BSVSearch's testing observation (from thread):**
- HTML files inscribed on-chain CAN link to other inscribed HTML files → multi-page static sites work.
- ORDnet browser does NOT load WWW (Web2) links/pages — Web3-only rendering.
- Test page: https://ord-rtr-bsv.com/c30faa4c1b0b3f39b1dd8a5898d80991d822bae7e17f00ac263310698d4dd3b5_0 (note: this URL is on `ord-rtr-bsv.com` which CryptoClub @BullRushClub flagged as a "Danish copycat / unrelated" domain — ARTaY's official ORDnet may render the same content at a different URL).

**To dig into:**
- Difference between ord-rtr-bsv.com (alleged copycat) and ordnet.io (alleged real).
- **Visit https://domains.ordnet.io and inspect the registration flow** — does it require a wallet connect / on-chain payment, or is it a free centralized form? What does the resulting `.web3` record look like?
- Whether `.web3` domain registration is on-chain, overlay-based, or centralized.
- Whether Hodos can natively render `.web3` URLs without a separate client.
- Tradeoffs between ORDnet's approach vs. React Onchain vs. Babbage's incoming scheme.
- Identity of @BullRushClub (CryptoClub) — credibility assessment before quoting their guidance.

---

## Cross-cutting questions for Hodos product design

These shape how/whether Hodos supports any of the above natively.

1. **Which URI schemes does Hodos resolve in the address bar?**
   - Today: `http://`, `https://`, plus paymail recognition in the wallet send form.
   - Candidates to add: `b://` (Bitcoin Files / Unwriter), `nb://` (NBDomain), `metanet://` (Babbage incoming), `peerpay://` (Deggen / BRC-125 — wallet-side already discussed), `ord://` or `.web3` (ORDnet), arbitrary content-addressed schemes (UHRP).
   - Decision: support all natively, support a curated subset, or build a generic resolver-plugin model.

2. **Do we render on-chain content directly, or proxy to an external client?**
   - Native rendering = best UX, owned story, requires Hodos to embed retrieval/decoding logic.
   - Proxy = lower lift, dependency on third parties, weaker positioning.

3. **How does Hodos's full-Chromium choice constrain or enable this?**
   - Chromium gives us the engine, network stack, devtools. Custom URI handlers are a known CEF/Chromium pattern.
   - Service workers (e.g., React Onchain's video-range-request handler) work in Chromium natively.

4. **How does any of this interact with our existing on-chain backup token?**
   - If we're already inscribing data on-chain for backups, the same primitives apply for content.
   - Possible synergy: same retrieval/decoding stack serves both backup files and on-chain web content.

5. **DNS-binding-via-pubkey (Babbage's hint) vs. content-addressing only**
   - DNS-binding lets existing domains point to on-chain content (gentle on-ramp).
   - Pure content-addressing (UHRP / ordinals) is more "purely Web3" but more friction for users used to typing names.
   - Likely both matter; Hodos should resolve both.

---

## Decisions to make (eventually)

- Native support for `metanet://` once Babbage publishes the spec — yes/no, when.
- Native rendering for ORDnet `.web3` URLs — yes/no.
- React Onchain–style content (OrdFS-fetched apps) — render natively or require a separate client.
- Whether Hodos publishes its own on-chain content (e.g., the HodosBrowser landing page itself becomes on-chain) as a credibility/dogfooding move.

---

## Marketing campaign tracking section

Used to seed the eventual launch campaign for whatever Hodos ships in this space. Track interested people and posts as they surface; build the audience before the campaign starts.

### Key voices in the on-chain-website conversation

| Handle | Role | Known posts | Notes |
|---|---|---|---|
| @ProjectBabbage | Standards / Metanet URI scheme | https://x.com/ProjectBabbage/status/2050951722869330388 | Spec author; collaborate, don't compete. |
| @BSVSearch | Tester / influencer (YouTube channel) | https://x.com/BSVSearch/status/2051433812115222849 | Has YouTube reach; previously used in Hodos demo (QR scan section); didn't reply to interview ask earlier. Don't pursue at his expense. |
| @BullRushClub (CryptoClub) | BSV-max promotional account | (referenced in BSVSearch thread) | Small (~800 followers), broadcast-only (follows 3 accounts), maximalist tone, promo register. Their ORDnet-vs-ord-rtr-bsv.com "real-vs-copycat" claim is a tip to verify, not authoritative. Don't quote as a source. |
| @danwag06 (Dan Wagner) | React Onchain author | https://x.com/danwag06/status/1986434059346584028 | Active builder; reach out if Hodos starts rendering React Onchain content. |
| ARTaY (ORDnet lead) | ORDnet project lead | (find primary handle) | Direct contact for ORDnet collaboration. |
| @_unwriter / @unwriter | Bottle / B:// originator | (verify handle) | Historical context; may have moved on from BSV. |
| @deggen | BRC-125 URI scheme | https://x.com/deggen/status/2050312196350230877 | Already in active conversation w/ Hodos on URI schemes — natural collaborator on `metanet://` discussion. |
| @ruthheasman | Builder pain on cross-device sync | (her recent appearance on @GavinMehl's show) | Adjacent — sync/portability lens overlaps with on-chain-content lens. |

### Posts to reference in the eventual campaign

| Date | Author | URL | Why relevant |
|---|---|---|---|
| 2025-11 (approx) | @danwag06 | https://x.com/danwag06/status/1986434059346584028 | First public React Onchain announcement. |
| 2026-05-04 | @ProjectBabbage | https://x.com/ProjectBabbage/status/2050951722869330388 | Metanet URI scheme tease. |
| 2026-05-04 | @BSVSearch | https://x.com/BSVSearch/status/2051433812115222849 | Real-tester confirmation that on-chain HTML linking works. |
| 2026-04-11 | @HodosBrowser | https://x.com/HodosBrowser/status/2042950831973241136 | Hodos's own first public engagement on this topic (Maxthon-reply). |

### Campaign hooks (idea bank, not commitments)

- "We render the metanet natively." (When/if Hodos supports `metanet://` URIs.)
- "Hodos's own landing page is on-chain." (If/when we dogfood by inscribing the site.)
- "One browser. Web2 sites, BRC-100 apps, on-chain native content." (Triple-substrate framing.)

---

## Process notes

- **Don't actively work this until at least mid-2026.** The Babbage spec hasn't shipped; React Onchain is in beta; ORDnet is alpha-ish. Premature integration costs more than it gains. Per `project_marketing_mountain_phase_plan.md`, Hodos's focus through ~July 2026 is brand recognition + UX-tightening on what already ships.
- **Watch for the "details to come" Babbage post.** That's the trigger to start design discussions.
- **Update this doc as new posts / projects surface** — the marketing tracking table is the easy place to drop new finds without committing to a full design pass.
