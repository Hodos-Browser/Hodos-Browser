# Maxthon: Forensic Post-Mortem of the BSV-Native Browser

> 📁 Part of the **Dolphin Milk + Edwin Integration** doc set — see `README.md`. Forensic deep-dive companion to the main study docs.
> **Created:** 2026-06-28 by a research workflow (web-cited). **STUDY, not a decision** — options & trade-offs, no winner picked. Claim tags **[FACT]/[VISION]/[INFERRED]/[SPECULATION]/[UNVERIFIED]** preserved.

## Purpose & Bottom Line

Maxthon built the most complete prior-art attempt at a BSV-native browser, assembling all the pieces Hodos is now contemplating — a native wallet (VBox), a micropayment rail (VPoint), blockchain identity/domains (NBdomain), AI assistant (AIChat), and DApp APIs — between 2020 and 2023. The experiment failed to close the economic loop: after three years of public development, 85% of beta users held no BSV, users reported "no application scenarios except buying VPoint," the developer portal is now unreachable, and Maxthon's 2026 public positioning has silently dropped VBox, VPoint, and BSV entirely in favor of "privacy + cloud sync + AI." The cold-start failure was structural, not incidental — Maxthon tried to build a two-sided marketplace (users with wallets, publishers with BSV integrations) simultaneously from a sub-1% market-share browser with a China-origin trust deficit. For Hodos, Maxthon is genuinely instructive: the technical patterns were directionally correct; the sequencing, branding, ecosystem depth, and trust architecture were wrong.

---

## 1. Technical Architecture

### 1.1 VBox: The In-Browser BSV Wallet / Identity Manager

[FACT] VBox was the headline feature of Maxthon 6 beta, described as "a blockchain identity manager and simple wallet." [FACT, source: Maxthon blog] Its core mechanism was a public/private key pair generated and stored locally in the browser. The user-facing claim was that the identity was "immutably saved on the Bitcoin SV blockchain so nobody can delete that identity."

**Wiring into the browser core:** [INFERRED from primary sources] VBox was implemented as a privileged browser extension or native component that exposed a JavaScript signing API to web pages. The documented flow was: a web application sends a SHA-256 hash of data to VBox via a browser API; VBox performs a double-hash (hash of the hashed string) and signs it with the stored private key, returning the signature to the web app. [FACT, source: Maxthon blog Part 1, June 2020] This is a browser-mediated signing pattern, equivalent to a WebAuthn-style flow but using BSV key pairs instead of FIDO2 credentials. No C++ source code was ever made public; the internal architecture was not disclosed. Whether VBox ran as a native module inside the Chromium process or as a privileged extension context is [UNVERIFIED].

**Wallet functionality:** [FACT] VBox also functioned as a basic wallet — it could hold a BSV balance, and users could "recharge" it by purchasing BSV and transferring it in. The wallet's primary function appeared to be signing and small payments to BSV apps, not general-purpose sending. [INFERRED] The data storage design used local files for key material with optional cloud sync (encrypted); the cloud sync endpoint was Maxthon's own servers.

**Custom protocols:** [FACT, source: CoinGeek 2020] Maxthon 6 implemented two custom URL protocols: `tx://` (for viewing BSV transactions) and `nb://` (for accessing NBdomain blockchain-hosted websites). These were browser-level protocol handlers, not web-standard features.

**Developer API surface:** [FACT] Maxthon published a developer documentation portal at `v.maxthon.com/doc`. [FACT, verified 2026-06-28] That portal now returns a connection refused error — it is not reachable. The developer API exposed at minimum: an identity signing call, a payment request interface, and protocol handlers for blockchain data. No open-source SDK was found. All documentation was proprietary.

### 1.2 VPoint: The Micropayment / Points Mechanism

[FACT] VPoint was described officially as "an instant payment interface for BSV apps." [FACT, source: Maxthon 6 Beta Test Report, July 2020] Users encountered errors "recharging Vpoint," which confirms VPoint required a top-up step — users had to buy VPoints (presumably by depositing BSV). [INFERRED] VPoint appears to have been a points layer on top of BSV, not direct on-chain BSV per action. The recharge model means a user funded a balance, and VPoint debits then happened against that balance. Whether VPoint debits were settled on-chain in real time, batched, or were purely off-chain accounting is [UNVERIFIED] — no technical documentation confirming the settlement mechanism was found.

**Was it real BSV per action?** [INFERRED from evidence] Almost certainly not, in the sense that individual user clicks or article reads did not result in individual BSV blockchain transactions. The "recharge" model (buy points first) and the complaint that there were "no application scenarios except buying VPoint" both suggest VPoint was closer to a gift card or prepaid balance than a per-action micropayment rail. [FACT] Jeff Chen's own framing from a CoinGeek interview was that "users care about the pain points a product can resolve, not the token it uses" — the phrasing suggests VPoint was intentionally abstracted away from raw BSV UX. [SPECULATION] The settlement architecture may have been custodial, with Maxthon acting as a payment processor clearing balances in bulk, which would undermine the trustless/self-sovereign premise.

**Publisher/merchant side:** [FACT] The VPoint API required publishers to integrate Maxthon's signing and payment API. [FACT, source: Beta Test Report] Users complained that there were "no application scenarios except buying VPoint" — meaning essentially no publishers had integrated VPoint acceptance in any meaningful way by mid-2020. [INFERRED] No publisher of scale (media company, tool provider, game) ever publicly announced VPoint integration.

### 1.3 NBdomain: Browser-Native On-Chain Domains

[FACT] NBdomain launched November 4, 2020 with the `.b` top-level domain. [FACT] It was developed by Jeff Chen (the same person who is Maxthon's CEO — not a separate team). [FACT, source: CoinGeek NBdomain launch article] Compatible wallets at launch were: Maxthon VBox, Volt Wallet, and DotWallet.

**Technical mechanism:** [FACT] NBdomain is a distributed on-chain database on BSV. Domain registration is a BSV transaction; once registered, the domain is permanently owned by the registrant's BSV key and cannot be revoked. [FACT] The `nb://` protocol in Maxthon's browser resolved these domains without a third-party DNS server — resolution happened through BSV node queries. [FACT] NBdomain supported subdomains, content association (a domain could point to on-chain content), and a Global ID concept linking the domain to a payment address. [FACT] Jeff Chen described it: "All .B domains are permanent. Once registered, it belongs to you — no one, even the creator like us, can take it from you." [INFERRED] Adoption was minimal: no registration count was ever published; the compatible wallet list was tiny; no major website ever adopted a .b domain publicly.

### 1.4 AIChat (uchat): The AI Assistant

[FACT] Maxthon launched AIChat in July 2023 — three years after the BSV blockchain push began. [FACT, source: Maxthon blog, July 2023] The AI assistant offered page summarization, content drafting, and Q&A. [FACT] Maxthon claimed "all AIChat interactions occur locally and no personal data is sent to external servers." [FACT] AIChat was offered as a free trial, then paywalled behind "Maxthon Diamond" credits. [INFERRED] No LLM is named in the original announcement — the underlying model is not disclosed. [FACT] No BSV or VPoint integration with AIChat was mentioned in any source found: AI and crypto were siloed features. [FACT] In May 2025, Maxthon announced a strategic partnership with uuGPT.com for AI capabilities — suggesting the original AIChat implementation was being replaced or supplemented. [FACT] The search term "uchat" does not appear in any primary Maxthon source — the correct term is "AIChat." The term "uchat" used in the research prompt may be a misremembering or community nickname.

### 1.5 DApp Integration

[FACT] Maxthon 6 exposed APIs "for third-party developers to utilise VBox to easily create applications that interface with a user's BSV-based blockchain identity." [INFERRED from Beta Test Report] Zero metrics on the number of DApps built were ever published. [FACT] A group of demo application developers was mentioned in 2020 press materials, but no apps shipped publicly at scale. [FACT] The BSV developer ecosystem itself was thin: a 2019 HackerNoon audit of 136 BSV projects found "the vast majority are sub-par," with "broken and abandoned projects" common, "continued development is low," and only a handful of projects (MoneyButton, Handcash, Bitcom) showing genuine merit. [FACT, source: Jordan Mack / HackerNoon] The entire supply side that Maxthon needed to attract was already underdeveloped before the browser launched.

### 1.6 The "Blockchain Browser" / Metanet Positioning

[FACT] The Metanet concept — a BSV-native Internet where data and payments flow on-chain — was conceived by Craig S. Wright, Chief Scientist of nChain. [FACT, source: BSV Association announcement, Feb 2020] Maxthon adopted the Metanet framing wholesale as the rationale for Maxthon 6: BSV was not just a payment layer but the substrate for a new Internet. [FACT] Jimmy Nguyen of the Bitcoin Association quoted support for Maxthon 6 at CoinGeek London in February 2020; Jeff Chen spoke at that conference. [INFERRED] No financial investment from nChain or Ayre Group in Maxthon was ever confirmed in any primary source found — the relationship appeared to be promotional/ecosystem rather than capital.

---

## 2. Timeline (2017–2026)

### Phase 1: Behavioral Token Mining / ICO Era (2017–2019)

[FACT] Maxthon's first blockchain move preceded BSV. In September 2017, Jeff Chen — not under the Maxthon corporate banner but through a separate vehicle — launched the LivesToken (LVT) private sale and subsequently the Symbiosism Chain concept. [FACT] LivesToken was an ERC-20 token (Ethereum, not BSV) with 1 billion total supply, 35% allocated to ICO. [FACT] The core value proposition was behavioral token mining: browsing time, clicks, comments, and shares would earn LVT. [FACT, source: Prnewswire 2017] Maxthon distributed a "mining edition" browser and a Lives Wallet extension compatible only with Maxthon. [INFERRED] LVT never achieved significant exchange listing or ecosystem traction; the project's Symbiosism Economy Foundation continued promoting it through at least 2025 (extension still listed in Maxthon Extension Center) but the token never became a meaningful revenue stream. [INFERRED] This 2017 attempt established a pattern: behavioral token rewards for browsing, browser-as-mining-tool, ICO to fund the ecosystem — a pattern Maxthon would attempt again, differently, with BSV in 2020.

### Phase 2: BSV Pivot and Maxthon 6 Launch (2019–2021)

- **February 2020:** [FACT] At CoinGeek London, Maxthon announced the world's first BSV-powered browser, with an alpha expected "late February 2020" and beta "March 2020."
- **June 12, 2020:** [FACT] Maxthon 6 public beta launched, including VBox, VPoint (nascent), and the `nb://` protocol.
- **July 2020:** [FACT] Beta test report published: 1,437 participants, 120 questionnaires. Key statistics: 15% held BSV; among the 85% who did not, over 60% cited "no convenient purchase channel" as the barrier; users complained "no application scenarios except buy VPoint."
- **July 27, 2020:** [FACT] Maxthon 6 NBdomain protocol integration announced.
- **November 4, 2020:** [FACT] NBdomain `.b` TLD officially launches.
- **November 30, 2020:** [FACT] Maxthon 6 official release.
- **2021:** [INFERRED from absence of documentation] VPoint and DApp ecosystem failed to grow. No developer success stories, no publisher announcements, no usage metrics published. The developer portal (v.maxthon.com/doc) references continued but no SDK or framework was open-sourced.

### Phase 3: Quiet Abandonment of BSV Identity (2022–2023)

- **August 2022:** [FACT] Maxthon blog highlights a free VPN service — no blockchain mention. [INFERRED] The marketing mix had begun pivoting away from blockchain as primary identity.
- **July 2023:** [FACT] AIChat launched, presented as a standalone privacy feature with no BSV/VPoint integration.
- **2023–2024:** [FACT] Maxthon blogs pivot to writing about "crypto-friendly browser" in a generic multi-chain sense, not BSV-specific. LivesToken mining resurfaces in 2024 and 2025 content. [INFERRED] The BSV-native identity was effectively sunset by this point.

### Phase 4: Privacy + AI Rebranding (2024–2026)

- **2024:** [FACT] Maxthon version 7.x updates to Chromium 130+; positioning is now "privacy-first browser with cloud sync, AI, and Web3 readiness."
- **January 2026:** [FACT] Version 7.5.2.5000 upgrades to Chromium 140 kernel. [FACT, source: Maxthon blog 2026] Positioning in 2026 comparative reviews: "The Versatile Cloud-Powered Browser." VBox, VPoint, and BSV are not mentioned in any 2026 comparative or review content.
- **May 2025:** [FACT] Maxthon announces strategic collaboration with uuGPT.com for AI capabilities — no BSV connection.
- **Current (2026):** [FACT] The 2026 Maxthon self-review identifies its own Web3 features as a "weakness" for general users: "Web3 features feel niche for general users." BSV-specific features are absent from all public positioning materials reviewed.

---

## 3. Forensic Failure Analysis

### 3.1 The Two-Sided Cold Start: The Core Structural Failure

[FACT] Maxthon's own beta data shows the cold-start problem clearly: at launch, 85% of users held no BSV. Of those, over 60% cited no convenient purchase channel — not disinterest in crypto, but friction in the onboarding path. [INFERRED] Without users who already held BSV, VPoint had no natural funders. Without funded VPoint users, publishers had no revenue incentive to integrate VPoint APIs. Without publisher integrations, users had "no application scenarios except buying VPoint." This is a textbook two-sided marketplace cold-start: neither side of the marketplace found a reason to show up without the other side already present. [FACT, source: Beta Test Report, July 2020] This was documented in Maxthon's own internal beta data within weeks of launch — and no published remediation strategy ever addressed it.

### 3.2 VPoint Was a Points Wrapper, Not a Micropayment Rail

[INFERRED, strongly supported by evidence] VPoint's "recharge" model meant users purchased a prepaid credit balance — not that they sent BSV directly to publishers per action. This created multiple compounding problems:

1. **The "buy to use" barrier is the same as crypto onboarding.** A user who needed to buy BSV and recharge VPoint before they could pay a penny for content faced the same UX cliff as opening a crypto exchange account.
2. **The per-action micropayment vision never materialized on-chain.** Individual user actions (reading an article, watching a video) almost certainly did not result in individual BSV transactions — the economic premise of the Metanet.
3. **Publishers could not verify real per-action revenue.** If VPoint was custodial and batched, publishers were trusting Maxthon as an intermediary, not receiving trustless BSV.
4. **Users had zero reason to hold a VPoint balance without publishers to spend it on.** The cart was before the horse by design.

### 3.3 The Developer Portal is Dead

[FACT, verified 2026-06-28] `v.maxthon.com/doc` returns a connection refused error. This is the URL cited in CoinGeek's 2020 coverage as the location of developer documentation. Its death is a direct indicator of the BSV developer program's abandonment. [INFERRED] Without living documentation, no new developer could ever integrate VPoint or VBox after the portal went offline — making the abandoned portal a reliable proxy for when Maxthon internally decided the program was over.

### 3.4 The China-Origin Trust Deficit

[FACT] In July 2016, security researchers from Fidelis Cybersecurity and Exatel found Maxthon browser versions 4.4.5 transmitting: ad blocker status, websites visited, searches conducted, and applications installed — to servers in Beijing, over unencrypted HTTP. [FACT] The collection continued even after users explicitly opted out of the "User Experience Improvement Program." [FACT] Maxthon's CEO characterized the incident as "a bug that was fixed." [FACT, source: Wikipedia] Fidelis's CSO stated the data "contains almost everything you would want in conducting a reconnaissance operation." [FACT, source: multiple 2024-2025 reviews] Independent assessments in 2024–2025 continue to document this history and note Maxthon's closed-source status prevents independent audits. [FACT] Maxthon's Chinese corporate origin subjects it to Chinese data governance laws that conflict with privacy expectations of users in EU, US, and Singapore markets.

**Why this compounded the failure:** Maxthon was attempting to build a self-sovereign identity and payment product (VBox, VPoint) on top of a browser with a documented history of covert data exfiltration. The privacy claim of VBox — "your identity, owned by you, on blockchain" — was directly undermined by a browser that had demonstrably violated user privacy at the OS level. No sophisticated user evaluating VBox for financial activity could trust the platform.

### 3.5 Revenue Context: A Small Company's Bet

[FACT, source: Getlatka 2024] Maxthon's revenue is approximately $12M with ~79 employees. [FACT] PitchBook reports 78 total employees. [FACT] The company has raised $6M in disclosed funding over its lifetime. This is a small, self-funded company making a platform bet on a niche blockchain ecosystem. [INFERRED] With $12M revenue and 79 staff, Maxthon had limited runway to sustain a multi-year developer ecosystem program simultaneously with core browser maintenance, AI development, and cloud services. The resource constraint explains why the developer program was quietly abandoned rather than explicitly pivoted.

### 3.6 Market Share Erosion

[FACT] Maxthon's global desktop market share is under 0.3%. [FACT] Independent data documents a combined 60% year-over-year usage drop in Maxthon's lightweight-browser segment. [FACT, source: Enterprise Apps Today] The "670 million users" claim on Maxthon's own website is [UNVERIFIED] marketing; the "100 million monthly visitors" figure likewise [UNVERIFIED]. [INFERRED] A blockchain DApp ecosystem requires critical mass of users for two-sided network effects. A browser with sub-0.3% share cannot generate that mass independently of the broader BSV ecosystem — which itself was documented as having a sparse, largely abandoned DApp portfolio.

### 3.7 The Ecosystem Was Always Jeff Chen

[FACT] Jeff Chen is simultaneously: Maxthon CEO, creator of NBdomain, and co-creator of LivesToken/Symbiosism Chain. [INFERRED] This is not an ecosystem — it is one person's projects. A healthy platform attracts independent builders who each have independent business interests in the network's success. Maxthon's blockchain offerings were all controlled by the same individual. When Chen's attention (or Maxthon's resources) shifted, every dependent project went dark together.

### 3.8 BSV Ecosystem Thinness

[FACT, source: Jordan Mack / HackerNoon] A 2019 examination of 136 BSV projects concluded "the claims [of a thriving ecosystem] are false," with the vast majority of projects described as "copy and paste clones," "broken and abandoned," or "forks of existing open source projects." [FACT] Only MoneyButton, Handcash, and Bitcom were highlighted as showing genuine merit. [INFERRED] Maxthon launched its VPoint DApp marketplace into this thin ecosystem. The supply-side of the marketplace (BSV developers building content apps) was never large enough to fill a browser-native platform.

### 3.9 Behavioral Token Mining: Regulatory and Trust Liability

[FACT] The LivesToken (2017) and Symbiosism/LivesToken revival (2024–2025) both positioned token rewards for browsing behavior as a value proposition. [INFERRED] Behavioral mining schemes faced regulatory scrutiny in 2017–2018 (SEC guidance on ICOs) and have continued to attract scrutiny. [FACT] LivesToken is an ERC-20 token — not BSV — making it an entirely separate technical track from Maxthon's BSV identity work, suggesting fragmented technical strategy. [INFERRED] Users receiving token rewards for browsing have speculative, not utility, value orientation: they want to sell the token, not spend it on content. This is a fundamentally different incentive structure from the self-sustaining micropayment loop that was theorized.

---

## 4. What This Means for Hodos (Options, Not a Pick)

### Patterns Worth Copying

**The VBox signing API concept is correct.** Exposing a browser-native signing API — where a web page can request a signature from the user's wallet key without the user ever leaving the browser — is exactly the right abstraction for seamless, password-free BSV identity. The pattern (web app sends hash → browser wallet signs → returns signature) is reusable. Hodos, with Edwin as a local sidecar, has a natural implementation path: Edwin exposes a local signing API that the Hodos browser injects as a privileged JavaScript object, similar to how MetaMask injects `window.ethereum`. The key lesson is that this should be implemented as a well-documented, open API from day one — not a closed proprietary system.

**The `nb://` and `tx://` protocol handler approach is technically sound.** Custom URL scheme handlers registered at the OS level, giving the browser first-class status as a BSV client, is a good pattern. Hodos should consider registering handlers for `1sat://`, `bsv://`, or equivalent schemes to enable direct linking into BSV content from any application.

**NBdomain's permanence model is a genuine user benefit.** The concept of a blockchain domain name you own forever and cannot be taken away (vs. annual DNS renewal) is a real value proposition — particularly for identity anchoring. The failure was adoption, not concept. Hodos, building on the existing BSV identity stack (BAP, 1Sat), can inherit the identity pattern without reinventing it.

**Sequencing: Identity before payments.** Maxthon tried to ship VBox (identity), VPoint (payments), NBdomain (domains), and a DApp marketplace simultaneously. A tighter sequencing — start with identity/login only, prove utility, then layer payments — would have been lower friction. Hodos should validate the signing API use case (log into BSV apps without a password) before layering micropayments.

### Patterns to Avoid

**Blockchain-first branding.** Maxthon's "Blockchain Browser" / "Built on BSV" identity attracted only BSV community members and alienated mainstream users who associated "blockchain browser" with scam/volatile territory. [FACT] Only ~2% of Maxthon's own beta users described themselves as knowledgeable about blockchain. Hodos's framing of "native built-in AI" is already better positioned than "blockchain browser." The payments and identity features should be described in user-benefit terms ("pay without passwords," "read paywalled content for fractions of a cent") not technology terms.

**Points wrappers and custodial balance models.** If VPoint was a custodial top-up balance, it recreated every problem of centralized payment processors — user data held by Maxthon, trust in Maxthon's settlement, Maxthon as single point of failure. Hodos's Edwin sidecar, connected to a Rust BSV wallet subprocess, should be designed to produce actual on-chain transactions for payments — not a points ledger managed by the app.

**Launching a developer marketplace before you have developers.** Maxthon published API docs and a developer portal before any DApps existed. The portal is now dead. Hodos's monetization loop should start with a single concrete use case that the Hodos team itself controls — for example, paywalled AI assistant queries (x402), or access to Hodos-built search or summarization features — before opening a general developer platform.

**Behavioral token mining.** Rewarding browsing with tokens creates speculative incentive structures, not utility demand. Users who earn tokens want to sell them; users who want to sell tokens push price down; falling price makes the rewards worthless; the loop collapses. Hodos's x402 micropayment model, where a user pays a small amount of real BSV for a real service and the service provider is immediately paid, is structurally superior because it is utility-first.

**Opaque ownership + closed source + potential data routing to jurisdiction of concern.** This is Maxthon's unfixable problem. Hodos's privacy architecture needs to be auditable — open source where possible, CEF process isolation enforced, no silent data exfiltration to any server, and a clear data residency story. The Edwin local sidecar model (running on localhost, no cloud required) is already a stronger trust architecture than anything Maxthon offered.

**Monoculture ecosystem (single person = all projects).** If Hodos's BSV DApp ecosystem depends entirely on Hodos-built projects, it will have the same fragility. The x402 standard and 1Sat APIs should be positioned as genuinely open — with the aim that independent builders using any BSV wallet can interoperate with Hodos users.

**Integrating AI and payments in separate silos.** Maxthon added AIChat in 2023 with no BSV integration — it monetized AI separately via Diamond credits (proprietary points again). The unique differentiator of Hodos is that Edwin (the AI) is native AND the BSV wallet is native, and they are wired together: the AI can initiate payments, query on-chain data, and use x402-gated APIs as tools. Maxthon never made this connection. It is the primary architectural lesson.

---

## Open Questions

1. **Was VPoint's settlement on-chain or custodial?** No primary source confirms whether individual VPoint micropayments resulted in BSV blockchain transactions or were netted off-chain. If the latter, was this technically because BSV fees in 2020 were still too high for sub-cent transactions? What does this imply for Hodos's x402 at current fee levels?

2. **What was the actual BSV onramp Maxthon offered (or failed to offer)?** The beta data shows 60%+ of non-holders cited "no convenient purchase channel." What specifically did the VPoint recharge flow look like, and how many steps did it require? This is directly analogous to Hodos's wallet onboarding UX challenge.

3. **Did any publisher ever successfully integrate VPoint, and what was their experience?** No public case study was found. Were any pilots attempted with media companies, tool providers, or game developers? If someone has first-hand knowledge of a VPoint integration attempt, it would be extremely valuable.

4. **Is NBdomain's `.b` registry still live and resolving?** The domain registration data is on-chain and theoretically permanent. Are any `.b` domains actively in use? This affects whether Hodos should build on NBdomain, the existing BAP/1Sat identity stack, or wait for something new.

5. **What did Maxthon's internal team learn?** No post-mortem from Jeff Chen or any Maxthon employee has been published about the BSV pivot. Jeff Chen's 2020 interviews were bullish and vision-heavy; there are no candid failure retrospectives available.

6. **Did the China-origin trust problem materially reduce adoption of VBox for financial use cases specifically?** The data shows low BSV adoption but does not isolate trust from friction as the primary barrier. Western vs. Asian user behavior may differ substantially here.

7. **What is the counterfactual cost of Maxthon's developer portal?** Building and maintaining a developer API and portal for a program that attracted essentially zero external developers is a real cost. What does a minimum-viable developer integration for Hodos look like that avoids the sunk-cost trap of building infrastructure before demand exists?

8. **Does Edwin's local-sidecar model solve the trust problem that killed VBox, or does it merely relocate it?** VBox's key material was stored locally and synced to Maxthon's cloud. Edwin is local, but the Rust wallet subprocess and its key management are Hodos-controlled binaries. What is the auditable, open-source path for user trust in the wallet?

---

## Sources

- [Maxthon 6: The Browser for the Next Generation Internet Built on Bitcoin SV (BSV) — PR Newswire, June 2020](https://www.prnewswire.com/news-releases/maxthon-6-the-browser-for-the-next-generation-internet-built-on-bitcoin-sv-bsv-301080919.html)
- [Maxthon Announces World's First Bitcoin SV (BSV) Powered Internet & Blockchain Browser — PR Newswire, Feb 2020](https://www.prnewswire.com/news-releases/maxthon-announces-worlds-first-bitcoin-sv-bsv-powered-internet--blockchain-browser-300997572.html)
- [Maxthon 6: The Browser for the Next Generation Internet Built on Bitcoin SV (BSV) — CoinGeek, June 2020](https://coingeek.com/maxthon-6-the-browser-for-the-next-generation-internet-built-on-bitcoin-sv-bsv/)
- [Maxthon 6, the Blockchain Browser (Part 1) — Maxthon Blog, June 2020](https://blog.maxthon.com/2020/06/07/maxthon-6-blockchain-browser-part-1/)
- [Maxthon 6 Beta Test Report — Maxthon Blog, July 2020](https://blog.maxthon.com/2020/07/03/3563-2/) (key stats: 15% BSV holders, user complaints re VPoint, 120 questionnaire respondents)
- [Maxthon 6 Supports NBdomain Protocol — Maxthon Blog, July 2020](https://blog.maxthon.com/2020/07/27/maxthon-6-supports-nbdomain-protocol/)
- [NBdomain Officially Launches — CoinGeek, Nov 2020](https://coingeek.com/nbdomain-officially-launches/)
- [Finally, .B Domains — NBdomain on Medium](https://nbdomain.medium.com/finally-b-domains-69544fa611fa)
- [How Maxthon's Bitcoin-Based NBDomain Will Save You Money — CoinGeek](https://coingeek.com/how-maxthons-bitcoin-based-nbdomain-will-save-you-money/)
- [Maxthon 6 Enables Every Website to Conduct Bitcoin Transactions — CoinGeek](https://coingeek.com/maxthon-6-enables-every-website-to-conduct-bitcoin-transactions/) (confirms v.maxthon.com/doc as developer API reference)
- [Maxthon CEO Jeff Chen Reveals BSV Features of Maxthon 6 — CoinGeek](https://coingeek.com/maxthon-ceo-jeff-chen-reveals-bsv-features-of-maxthon-6/)
- [LivesToken(LVT) Private Placement Introduction — Maxthon Blog, Sept 2017](https://blog.maxthon.com/2017/09/13/livestokenlvts-private-placement-introduction-symbiosism-economy-realization/)
- [Maxthon Backed New Cryptocurrency LivesToken (LVT) Launches — PR Newswire, Oct 2017](https://www.prnewswire.com/news-releases/maxthon-backed-new-cryptocurrency-livestoken-lvt-launches-private-sale-and-bounty-program-300536292.html)
- [Maxthon Leads Worldwide Browsers to Embrace Blockchain Technology — PR Newswire, 2018](https://www.prnewswire.com/news-releases/maxthon-leads-worldwide-browsers-to-embrace-blockchain-technology-by-introducing-browser-mining-mechanism-for-cryptocurrency-300577466.html)
- [AIChat on Maxthon for Intelligent Browsing — Maxthon Blog, July 2023](https://blog.maxthon.com/2023/07/29/aichat-on-maxthon-for-intelligent-browsing-maxthon-browser/)
- [Maxthon's Crypto-Friendly Browser — Maxthon Blog, Jan 2025](https://blog.maxthon.com/2025/01/20/maxthons-crypto-friendly-browser-2/)
- [Maxthon Announces Strategic Collaboration with uuGPT.com — Maxthon Blog, May 2025](https://blog.maxthon.com/2025/05/22/maxthon-announces-strategic-collaboration-with-uugpt-com/)
- [Maxthon vs. The Field — 2026 Edition — Maxthon Blog, Feb 2026](https://blog.maxthon.com/2026/02/21/maxthon-vs-the-field-2025-edition/) (no VBox/VPoint/BSV mention; "Web3 features feel niche for general users")
- [MAXTHON BROWSER REVIEW — Maxthon Blog, March 2026](https://blog.maxthon.com/2026/03/07/maxthon-browser-2/) (no VBox/VPoint/BSV mention; privacy-productivity positioning)
- [Maxthon — Wikipedia](https://en.wikipedia.org/wiki/Maxthon) (2016 data exfiltration scandal, company history, ownership)
- [How Maxthon Hit $12M Revenue with a 79 Person Team in 2024 — Getlatka](https://getlatka.com/companies/maxthon/competitors)
- [Maxthon Statistics 2023 — Enterprise Apps Today](https://www.enterpriseappstoday.com/stats/maxthon-statistics.html)
- [Examining Bitcoin SV's Developer Ecosystem Claims — Jordan Mack / HackerNoon](https://medium.com/hackernoon/examining-bitcoin-svs-developer-ecosystem-claims-830cbb6f12da) (136 BSV projects audited; "vast majority sub-par")
- [BSV Blockchain — Maxthon Announces World's First BSV Browser](https://bsvblockchain.org/maxthon-announces-world-first-bsv-blockchain-powered-internet-browser/)
- [BSV Browser: An Elegant Step Toward a More Web3 World — CoinGeek](https://coingeek.com/bsv-browser-an-elegant-step-toward-a-more-web3-world/) (BSV Association's own browser attempt, 2024–2026, 50+ downloads on Play Store)
- [Founder of Maxthon Browser Creates New Non-Profit Mining Browser — Medium / Asia Token Fund](https://medium.com/@blockasia/founder-of-maxthon-browser-creates-new-non-profit-mining-browser-55a2f7c3f89f)
