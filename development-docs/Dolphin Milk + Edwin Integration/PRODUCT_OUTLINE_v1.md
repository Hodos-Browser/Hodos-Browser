# Dolphin Milk × Edwin × Hodos — Product Outline (v1)

**Status:** First-cut outline for iteration. Not committed product direction.
**Drafted:** 2026-05-29 · **Last updated:** 2026-05-29 (added OpenClaw, Hodos-as-facilitator framing, terminal-user stat)
**Purpose:** Frame the product idea, name the problems, surface the open feasibility questions, and propose the iteration loop. Architecture-light. Marketing-aware.
**Pitch event:** See `PITCH_EVENT.md` — currently UNVERIFIED, awaiting Matt's confirmation. Drives all downstream timelines.

---

## The idea in one paragraph

Bundle **Dolphin Milk's** pay-per-prompt agent runtime + **Edwin's** cryptographic agent-sandbox security model inside **Hodos Browser**, with the user's Hodos wallet as the shared payment + identity rail. A single-install browser where the AI agent can (a) pay for any model on the open market without an account or API key, (b) physically cannot sign anything the user didn't approve because it never holds keys, and (c) installs in three clicks instead of seven terminal commands. This is a **coordinated partnership** between John Calhoun (Dolphin Milk), Jake Jones (Edwin), and Marston (Hodos). Each owns a complementary piece.

---

## What each project already does (verified from sources, 2026-05-29)

### Dolphin Milk — John Calhoun, Apache 2.0

**Mission, in its own words:** *"An open-source AI agent with its own wallet, no API keys, and a cryptographic receipt for every action it takes. Written in Rust. Single binary. You own it."* (README)

- **Pay-per-prompt across many models via x402.** No Claude account, no OpenAI account, no API key needed for any of them. The agent's wallet pays the provider directly. Two LLM endpoints baked in today (`openai-chat.x402agency.com`, `claude-chat.x402agency.com`); image, video, transcription, X-search are all addressable as additional x402 endpoints.
- **~200 sats (~$0.0003) per LLM call** at current prices. Single-image gen ~$0.19. Single-second of video ~$0.19. Same wallet pays them all.
- **BRC-18 hash-chained proof of every agent decision** stored on-chain. Tamper one proof → entire chain breaks.
- **Six-tier budget caps** (per-task / hour / day / week / month / lifetime) + staging threshold (manual approval above N sats) + strict vs. advisory enforcement.
- **Wallet runs as a separate process** — Dolphin Milk never touches private keys directly, only HTTP-calls a wallet at `:3322`. The wallet URL is configurable → **Hodos's `:31301` slots in directly**.
- Already has prompt-injection defense (Aho-Corasick patterns, 4-layer sanitization), BRC-31 mutual auth, BRC-52 capability certificates, log redaction, AES-256-GCM memory encryption.
- Setup today: **7 terminal-first steps**, requires Rust toolchain.

### Edwin — Jake Jones, Business Source License 1.1 (→ Apache 2.0 in 2030), in-house at Marston

**Mission, in its own words:** *"EdwinPAI is a personal AI assistant you run on your own devices. It answers you on the channels you already use (WhatsApp, Telegram, Slack, Discord, Google Chat, Signal, iMessage, WebChat)..."* (README)

- **The thesis that beats Comet on security:** the AI sandbox has **zero access to signing keys**. AI gets opaque key IDs only. Every command goes through `SecureVault` and must be signed in a time-bound envelope (30–60s TTL).
- **Prompt injection becomes a math problem, not a cat-and-mouse game:** injected prompt has no valid signature → gateway rejects the action. The agent can ask for "delete everything" all day; without a wallet-signed envelope it goes nowhere.
- **BRC-42 per-device sub-keys** with perfect forward secrecy. Devices pair via owner-signed invites; each device gets its own scoped sub-identity.
- Multi-channel inbox under one personal-AI brain.
- **Marston already owns it** — no third-party licensing negotiation. BSL-1.1 needs Jake's sign-off for use inside a competing product, but as in-house IP that's tractable in writing.
- Status: pre-release MVP (2026.2.3).

### Hodos Browser — Marston

Per `CLAUDE.md`, existing in-tree:
- Native BRC-100 wallet on `:31301` with 89+ HTTP handlers (signing, payments, BRC-31 auth, BRC-72 linkage, BRC-29 PeerPay, BRC-121 paid content, certificate publishing).
- Overlay UI system (8 overlay types, separate CEF subprocesses, isolated V8 contexts).
- Payment-success-IPC chain (`HttpRequestInterceptor.cpp` — `AsyncHTTPClient::OnRequestComplete` + `firePaymentSuccessIpc` → `simple_render_process_handler.cpp:1051` → `useTabManager.ts:141`) — the tab badge animation that's our load-bearing UX safeguard.
- HTTP interception → forward to `:31301` for wallet endpoints, with `isWalletEndpoint` route table.
- Already runs the Rust wallet + adblock-engine as managed subprocesses. **Adding a third managed subprocess (Dolphin Milk) follows the same pattern, not a novel one.**
- `PermissionEngine` already implements per-domain trust → privacy perimeter → scoped grants → payment caps → cert disclosure → generic. This is the obvious home for "should the wallet sign Edwin's envelope?" logic.

---

## Brief explainers (so this doc reads cold)

| Term | Plain English |
|---|---|
| **x402** | An HTTP 402 ("Payment Required") protocol where a server replies "pay me N sats first" and the client pays in the request header. No accounts, no API keys — just a wallet that can sign a tiny BSV transaction. |
| **BRC-18** | A way of writing a hash of an agent's decision into an OP_RETURN on the blockchain. Each new decision references the prior one, so the chain of decisions is tamper-evident. |
| **BRC-31** | A handshake where two parties prove identity via ECDSA signatures over nonces. Cheap auth without passwords. |
| **BRC-100** | The standard wallet HTTP API used by BSV-native apps. 28 endpoints covering signing, payments, identity. Hodos implements it. Dolphin Milk's `bsv-wallet-cli` implements it. **If they agree on the wire format, Hodos's wallet can drop in for Dolphin Milk's.** |
| **Signed envelope (Edwin)** | A small JSON object: `{kid, alg, iat, exp, nonce, scope, target, payload, sig}`. The user's wallet signs it. Lives 30–60 seconds. The agent never touches the key — it just hands the envelope around. |
| **SecureVault (Edwin)** | An API boundary between the agent and the wallet. The agent says "sign target X for me." The vault checks scope + freshness + caps, then asks the wallet to sign. The agent only sees a result. |

---

## Three problems this stack solves (the marketing wedges)

### 1. Subscription fatigue → pay-per-prompt across all the AIs

**The pain we name:**
- ChatGPT Plus $20/mo + Claude Pro $20/mo + Midjourney $10/mo + Suno $10/mo + Runway $15/mo = **$75/mo of overlapping subscriptions** most users barely touch.
- Trying a new model means a new signup, new credit card, new password, another subscription to remember to cancel.
- Bored of one? You're still paying.
- The "I only make two images a week" user is paying ~$10/image for a $10 plan.

**The Hodos answer:**
- One BSV balance pays every model that supports x402. No accounts, no signups, no credit cards.
- Use Whisper on Monday, Claude on Tuesday, Veo on Friday — same wallet.
- A user running ~20 inferences a week pays **~16,000 sats ≈ $0.22/month** at current prices.
- Cancel = stop spending. There is no "membership."
- New model drops? Use it the day it goes live, no signup.

**Why this works on BSV specifically:** $0.0001/tx fee. Same flow on BTC or ETH would be prohibitively expensive in fees alone. This is a BSV-native story.

### 2. Agent security → math, not trust

**The pain we name:**
- Comet shipped **6 publicly disclosed prompt-injection CVEs in 8 months.** One crafted URL can wipe your Google Drive ("CometJacking"). One screenshot with hidden text can phish you in under 4 minutes.
- OpenAI publicly conceded (Dec 2025) that prompt injection in agentic browsers is *"unlikely to ever"* be fully resolved.
- Comet ships hidden, non-removable extensions ("Comet Analytics," "Comet Agentic"). It silently patched a system-call vulnerability without filing a CVE.
- The fundamental problem: Comet's agent runs with the user's full session privilege. The agent IS the user, to every site it visits.

**The Hodos answer (Edwin's model, applied in-browser):**
- The agent has **zero key access.** Every privileged action — sending BSV, calling an x402 endpoint, mutating an OAuth-gated account — must be signed by the wallet, not by the agent.
- Prompt injection becomes a non-event: the malicious instruction asks for an action, but without a wallet-signed envelope the wallet refuses and Hodos shows the user a prompt.
- Every spent satoshi gets a BRC-18 proof on-chain. The user has a verifiable receipt of what their agent did with their money.
- **Budget caps are enforced at the wallet layer, not the agent layer.** The wallet refuses to over-sign. The agent can't pinky-promise its way past the cap.
- No hidden extensions. No silent patches. Public threat model from day one.

**Marketing positioning:** *"Comet's promises, with the cryptography that makes them true."* Or shorter: *"Agent-driven, wallet-gated."*

### 3. Setup friction → three clicks, not seven terminal steps

**The pain we name:**
- Dolphin Milk today: install Rust → `cargo install bsv-wallet-cli` → `git clone` → `cargo build --release` (~10 min) → `init` → `start` → fund → check payment → chat. **7+ steps, terminal-first, ~95% of non-developer users disqualify at step 1.**
- Edwin today: Node 22 + pnpm + clone + build + `edwin onboard` wizard. Also terminal-first.

**The Hodos answer:**
- Dolphin Milk's binary ships inside the Hodos installer (`bin/dolphin-milk.exe`). Code-signed alongside Hodos.
- Hodos's wallet pre-exists, so funding is the same satoshis the user is already browsing with.
- "Launch Agent" overlay opens a Hodos tab on the bundled agent UI. **First chat in 3 clicks from install.**
- For the "I want to try a new model" use case: type `agent: image of a fox` in the omnibox.

**This is the click-through-rate story.** Subscription fatigue and security are how we win the argument. Setup is how we win adoption.

---

## Hodos Browser as the facilitator (the layer that makes this consumable)

Jake and John are deeply technical and live in Claude Code, terminals, and Rust toolchains. Their reflex is "users can `cargo install` it" because that's how they work. The hard fact:

> **Fewer than 1% of internet users can complete a `cargo build`.** 47M developers (SlashData, 2025) ÷ 5.5B internet users (ITU, 2024) = ~0.85%. Of those developers, only ~49% use Bash regularly (Stack Overflow Developer Survey, 2025), so the actively-terminal-capable population is closer to **0.4% of internet users.**

That's the addressable-market problem with both Dolphin Milk and Edwin as they stand today. Both are excellent products. Both gate themselves to a sub-1% audience by requiring terminal setup. Hodos's job is to **delete that gate** without sacrificing what makes the products good.

**The split-the-difference framing Matt named:** the pitch line *"It splits the difference for power users without locking out casual users"* applies on TWO axes, not one:

| | Power user (developer / Claude Code native) | Casual user (artist, content creator, business pro, everyone else) |
|---|---|---|
| **Economic model** | Bring your own Claude Pro key, OpenAI key, anything. Pay-per-prompt for the rest. | Pure pay-per-prompt with the browser's bundled wallet. Cents per week. |
| **Setup** | Terminal still works. Can disable Hodos and `cargo install` directly. | One installer. Three clicks. Wallet pre-created. |
| **Agent surface** | Web UI + MCP server + custom skill development. | "Type what you want" in the omnibox or click the agent overlay. |
| **Trust model** | Audit the BRC-18 chain themselves; verify on-chain. | "Hodos says it's safe" + green-dot animation on every payment. |

**Why a browser specifically:** coders work in terminals because that's their tool. Business professionals, artists, content creators, researchers, students — **everyone else** — work in browsers. They already have Gmail open in a tab. They already have Drive in a tab. They want their AI agent in a tab too, with reports rendered as a real web UI, not ASCII in a terminal. Hodos delivers the agent into the place those users already live.

**The single-wallet vs. compartmentalized-wallet trade-off:**
- Power users may want per-task wallets (one for shopping, one for banking, one for "experimental agent that might do something dumb"). Hodos's architecture allows this.
- Casual users want one wallet that handles everything. That's what they'd want from a debit card, too.
- **Default: single browser wallet. Power users can opt into compartmentalization.** Same shape as the API-key story: bring complexity if you want it; otherwise the simple path works.

This framing matters in the deck because it's the answer to *"why a browser product when Dolphin Milk and Edwin already exist as standalone binaries?"* — **because the binaries exclude 99% of the people who'd benefit.**

## OpenClaw — the competitive landscape gets sharper

OpenClaw (openclaw.ai) is a self-hosted open-source AI agent that runs on the user's machine, connects via WhatsApp / Telegram / Discord / Slack / iMessage. **3.2M users** as of April 2026, when Anthropic blocked Claude Pro/Max from working with it (OpenAI immediately moved in to capture the users).

**Note the eerie overlap with Edwin's product description.** Both: personal AI on your devices, same messaging channels. Edwin and OpenClaw are competing in the same product category. The difference is what makes Edwin valuable to Hodos:

| Dimension | OpenClaw (incumbent) | Edwin (Hodos partner) |
|---|---|---|
| User base | 3.2M (huge demand validation) | Pre-release MVP |
| Local-data advantage | ✅ Yes — files/keys stay on device | ✅ Yes — same architectural advantage |
| Security model | **Documented failures: indirect prompt injection, one-click RCE in milliseconds, persistent credential theft, data exfil via community skills, no enterprise kill switch** | **Cryptographic key isolation — agent never holds keys, every action requires wallet-signed envelope** |
| Threat coverage | IBM X-Force, Cisco, Barracuda have published security warnings; "CVE lag" — vulns disclosed faster than CVE assignments | Designed to make these classes of attack mathematically impossible (no valid signature → no action) |
| Distribution | Self-install, terminal-based | (today) Self-install. (With Hodos) Bundled. |

**The new competitive frame for the pitch:**
- **OpenClaw** proved the *demand* for a local agent that lives in your apps. Then proved the *security model needs to be different*.
- **Comet** proved the *demand* for an agent in your browser. Then proved the *privilege model needs to be different*.
- **Hodos + Dolphin Milk + Edwin** = both demands answered, both failure modes fixed by construction.

**OpenClaw's "local access" advantage that we should preserve:** the user's filesystem, OAuth tokens, login cookies, wallet, and computation all stay on the user's hardware. No cloud round-trip. This is the *right thing* about OpenClaw and we don't break it. Our agent is also local. We just don't give it the keys.

## Pitch focus — the three pillars (ratified 2026-05-30)

The deck and partner meetings should hammer these three things, in this order, and nothing else.

### Pillar 1 — Security (Edwin Envelope)
- Prompt injection is unsolvable as long as the agent has signing power. Comet shipped 6 CVEs in 8 months; OpenAI publicly admitted it's *"unlikely to ever"* be fully solved.
- Edwin's signed-envelope model removes the agent's signing power. The vault refuses to sign anything without a valid user-issued envelope binding scope + target + payload + TTL.
- **Slide-able claim:** *"The only agentic browser where prompt injection cannot move money."*

### Pillar 2 — AI Economics / Accessibility / Facilitation (Dolphin Milk pay-per-prompt)
- $75/month of stacked AI subscriptions, used 1/10th of the time.
- One BSV balance pays every model that supports x402. No accounts, no signups, no credit cards. Try a new model the day it ships.
- ~$0.22/month for 20 inferences/week vs. $20 for the same usage on Claude Pro alone.
- **Slide-able claim:** *"Stop paying $75/month for the AI subscriptions you barely use."*

### Pillar 3 — UX / Browser as the facilitator (Hodos)
- Three clicks, not seven terminal commands. Fewer than 1% of internet users can complete a `cargo build` (47M devs ÷ 5.5B internet users — SlashData + ITU 2024-25).
- Total assistant environment: bundled wallet + agent + UI; agent can open tabs, do OAuth flows, render reports as rich web UI not terminal text.
- Casual users get bundled defaults. Power users get BYO API keys, per-domain agent isolation, configurable budgets.
- **Slide-able claim:** *"Power users keep their tools. Casual users get something that just works. No one has to live in a terminal."*

The three pillars share one underlying mechanic: **the BSV/BRC-100 wallet at port 31301 is the shared rail.** Security envelopes ride on it, x402 payments ride on it, UX overlays read it. That's the architectural elegance.

---

## Grant-use plan (one bullet for the deck)

The $25K is engineering services from **Futran Solutions**, not cash. The deck should include one bullet:

> *"$25K in Futran engineering hours: AWS-deployable workstreams the partnership doesn't already own — wallet backend infrastructure (e.g., Shamir-secret-shared wallet backup hosted across AWS regions), cross-platform code-signing pipeline, integration testing. Keeps John focused on Dolphin Milk, Jake focused on Edwin, Marston focused on Hodos."*

**Concrete workstream candidate Matt flagged:** *Shamir Secret Sharing for wallet backup hosted on AWS (multi-region).* This is genuinely a Futran fit — pure AWS infrastructure work, novel security feature for Hodos, case-study-worthy, doesn't touch Dolphin Milk or Edwin internals.

This signals:
- We have a clear plan (we'll use the hours)
- We're not asking Futran to do work outside their stack (AWS infrastructure is exactly their sweet spot)
- The partner-product owners stay focused on their respective products

A separate planning conversation will resolve the actual line-item allocation of Futran's ~250 hours ($25K at ~$100/hr commercial rate). Candidates: AWS infrastructure for the wallet backend, Authenticode/notarization automation, integration test harness for the 3-process stack, frontend React work, GTM/business-development services (Futran advertises this as a service line).

See `../../marketing/intelligence/features/Dolphin Milk + Edwin Integration/FUTRAN_SOLUTIONS_PROFILE.md` for what Futran wants out of the engagement (case study, billable hours used, AWS-native architecture, AWS-friendly client conversion).

## BSV defense — when asked, not when leading

We do not lead with "Bitcoin SV." When the audience asks (and Futran/Beck both will), use the AWS-first answer. Full talking points + verbatim quotes in `BSV_OBJECTION_HANDLING.md`. The 30-second stock answer:

> *"AWS published a case study on their official Web3 blog in March 2026: the BSV Association sustained one million transactions per second for two weeks across six AWS regions, using EKS, FSx for Lustre, and RDS. That's roughly fifteen times Visa's peak throughput, sustained — not a benchmark spike. Whatever you've heard about the personalities, the engineering and the AWS partnership are real and current. The reason we chose BSV is the same reason AWS chose to feature it: it's the only public chain where per-prompt micropayments are economically viable."*

The Colorado School of Mines connection (Ishaan Lahoti on the team, Mitch Burcham/BINARY in the community) gives us a credible warm-intro story for Beck. See `../../marketing/intelligence/features/Dolphin Milk + Edwin Integration/NETWORK_CONNECTIONS.md` for verified facts vs. items still requiring confirmation before pitch use.

---

## Open product decisions (parked, not urgent)

Real questions, but they don't need answers for the AWS application or pitch night. Holding here so they don't get lost.

- **Product UI naming** (settings menu label, omnibox keyword): "Edwin Assistant" vs. just "Assistant" vs. something else. Matt has a slight lean toward keeping the proper noun for branding consistency. **Decision deferrable until after partner alignment.**
- **Default model**: `gpt-5-mini` for v1, expose advanced model picker in settings? Or pick at runtime via Dolphin Milk's auto-detection? **Defer until first real demo cycle.**
- **Per-domain agent isolation**: single agent per browser vs. separate agent contexts per domain (banking agent vs. shopping agent). **Defer until UX design conversation.**
- **Trial budget on first-run**: founder-funded ($0.05/user × early users) confirmed by Matt; mechanism (bundled sats vs. faucet vs. credit-card-to-BSV) deferrable until UX design.

---

## "What is the cost of free?" — the rhetorical anchor for the deck

When asked how we compete with Comet's free tier, the stock answer (no research required, gestures at things people already intuit):

> *"Free for now. Then what? Perplexity has a $21B valuation, hidden built-in extensions you can't disable, and an investor table that eventually wants its money back. We charge cents per prompt because that's what it actually costs. No ads. No surveillance. No exit event aimed at your data."*

Short version: ***"Free means you're the product. Or you will be."***

This pairs with the pay-per-prompt economics: a user doing ~20 inferences a week pays ~$0.22/month with Hodos. They pay ~$20-200/month with Comet for unlimited prompts they don't use, *plus* they sign over their browsing data to Perplexity's data flywheel.

## What becomes possible (the long tail to gesture at, not commit to)

A categorization Matt asked for: things the pitch can *list* during Q&A, not things we have to *build* before pitch night. The point is: once the audience understands the BRC-100 wallet + x402 + signed-envelope foundation, the following all fall out naturally:

**Tier 1 — Working today or near-term**
- Pay-per-prompt for any x402-enabled LLM (Claude, GPT, image, video, transcription)
- Bring your own API key per provider (hybrid economic model)
- BRC-18 on-chain audit trail of agent actions
- Wallet-gated budget caps enforced cryptographically
- One-click install via Hodos bundling

**Tier 2 — Designed but not yet built**
- Edwin signed-envelope command bus → cryptographic prompt-injection immunity
- Per-device sub-keys with perfect forward secrecy
- Per-domain agent isolation (separate budgets/scopes per site)
- Agent activity in wallet panel with friendly rendering of OP_RETURN proofs
- Backup/restore of agent memory through the existing wallet backup flow

**Tier 3 — Logical extensions once the foundation lands**
- OAuth-Connected-Agent: agent acts on Gmail/Drive/YouTube via brokered OAuth tokens (downstream feature, see `OAUTH_CONNECTED_AGENT.md`)
- Content signing & tipping for posts (sibling research, `CONTENT_SIGNING_AND_TIPPING.md`)
- Decentralized paymail addressing (sibling research)
- Multi-channel agent via Edwin's WhatsApp/Telegram/Discord surface (Edwin's existing product)
- On-chain certifier registry for trusted x402 endpoints
- Pay-per-prompt across user-to-user agent calls (one user's agent pays another's for a service)
- Agent receipts as cryptographically-verifiable "I did this work" tokens (gig-economy primitive)

**Tier 4 — Adjacent businesses this unlocks**
- An x402 model marketplace where independent fine-tuners get paid per call
- "Notarized agent transcripts" as a primitive auditors and lawyers could trust
- Privacy-preserving B2B agent-to-agent micropayments

The pitch posture on this list: ***"We're building Tier 1 now. Tiers 2–4 are what becomes possible once the foundation lands."*** Don't promise. Gesture.

## How we differ from Comet (the head-to-head)

| Dimension | Comet | Hodos + Dolphin Milk + Edwin |
|---|---|---|
| Payment model | Free with ads OR $20–200/mo subscription | Pay-per-prompt in BSV; no account anywhere |
| Model choice | Perplexity routes the call (Max tier opens model picker) | Any x402 endpoint; new models work the day they go live |
| Agent's key access | Full session privilege — IS the user | Zero — opaque key IDs only, signed envelopes |
| Prompt-injection posture | Patch-and-hope (6 CVEs in 8 months) | Cryptographic — no valid signature → no action |
| Auditability | None published | BRC-18 hash chain on BSV, verifiable offline |
| Hidden extensions | 2 found (Analytics, Agentic) | Zero — transparency commitment |
| Identity model | Perplexity account + Google OAuth | User's BSV wallet IS the identity |
| What you own | Nothing | Wallet, keys, on-chain proofs, exportable backup |
| Open source | No | Dolphin Milk Apache-2.0, Hodos source-available, Edwin BSL→Apache |

Comet's security failures are not bugs — they are the **architectural consequence** of giving an agent the user's session keys. Our story is that we did the harder thing.

---

## The three-party architecture (conceptual, not engineering)

```
┌─────────────────────────────────────────────────────────────┐
│                        HODOS BROWSER                         │
│                                                              │
│   ┌────────────┐    ┌───────────────┐   ┌────────────────┐  │
│   │  CEF tabs  │    │  Hodos        │   │  Agent overlay │  │
│   │  (the web) │    │  BRC-100      │   │  (the agent UI)│  │
│   │            │    │  Wallet       │   │                │  │
│   └────────────┘    │  :31301       │   └────────────────┘  │
│                     │  (EXISTING)   │                       │
│                     └──────┬────────┘                       │
│                            │ signs every action             │
│                            ▼                                │
│            ┌────────────────────────────────────┐           │
│            │  EDWIN security boundary           │           │
│            │  (SecureVault, signed envelopes,   │           │
│            │   per-device sub-keys, scope+TTL)  │           │
│            │                          [NEW]     │           │
│            └────────────┬───────────────────────┘           │
│                         │ approves / rejects calls          │
│                         ▼                                   │
│            ┌────────────────────────────────────┐           │
│            │  DOLPHIN MILK agent runtime        │           │
│            │  (LLM reasoning, tools, x402,      │           │
│            │   BRC-18 proofs, web UI on :8080)  │           │
│            │                          [NEW]     │           │
│            └────────────┬───────────────────────┘           │
└──────────────────────────┼──────────────────────────────────┘
                           │ x402 micropayments
                           ▼
            Open marketplace of LLMs + tools
            (Claude, GPT, Whisper, Veo, image, transcription...)
```

**Existing Hodos pieces we reuse:**
- Wallet at `:31301` (BRC-100) — pays the x402 calls
- Subprocess management (already runs Rust wallet + adblock-engine, same pattern adds Dolphin Milk)
- Overlay system + V8 injection — the agent UI lives in a Hodos overlay
- Payment-success-IPC chain — every x402 call lights the tab badge (mandatory load-bearing UX)
- `PermissionEngine` — the right home for "is this signed envelope in scope?" logic
- HTTP interceptor — handles the localhost:8080 → Hodos UI surfacing

**NEW pieces:**
- Edwin SecureVault between agent and wallet (the security boundary)
- Dolphin Milk subprocess wired to Hodos's wallet URL
- Hodos-native UI shell around the agent (or, v1: just open Dolphin Milk's web UI in a tab)
- Per-platform binary distribution for `dolphin-milk` (Windows / macOS / later Linux)

**Each partner owns:**
- **John Calhoun (Dolphin Milk):** agent runtime, x402 LLM routing, BRC-18 proof chain.
- **Jake Jones (Edwin):** key-isolation sandbox, signed-envelope command bus, per-device sub-identity tree.
- **Matt / Marston (Hodos):** browser + wallet + UX shell, install pipeline, OAuth brokering downstream.

---

## Surface-level feasibility research — the canaries to run

These are the questions we don't know the answer to yet. Each is small and testable.

### Track A — Wallet API compatibility
- **A1.** Does Hodos's BRC-100 surface on `:31301` cover the wallet endpoints Dolphin Milk's `bsv-wallet-cli` client actually calls? Single canary: run `dolphin-milk status --wallet http://localhost:31301` against a running Hodos wallet and grep for what fails.
- **A2.** Does Dolphin Milk's BRC-31 handshake work against Hodos's `well_known_auth` endpoint? If not, what's the shape gap?
- **A3.** BEEF transaction construction parity — does Hodos build the same BEEF shape Dolphin Milk expects for x402 BRC-29 payments?
- **A4.** Currency encoding (sats vs. satoshis vs. BSV, snake_case vs. camelCase) — do field-level shapes match?

### Track B — Edwin's signed-envelope model in a browser context
- **B1.** Where does SecureVault live? Three options: (a) inside the wallet on `:31301` as new endpoints, (b) as its own process between Dolphin Milk and the wallet, (c) inside Hodos's C++ shell as part of `PermissionEngine`. Pros/cons of each.
- **B2.** What does the user-facing approval UX look like? Prompts for every action vs. pre-approved budgets vs. domain-scoped grants. Hodos's `PermissionEngine` already does most of this — fits Edwin's envelope model?
- **B3.** Edwin's 30–60s envelope TTL vs. Dolphin Milk's multi-step tasks that may run minutes — do envelopes refresh, or do we need a long-running-task primitive?
- **B4.** Multi-channel inbox (the WhatsApp/Telegram/Discord part of Edwin) — is that in scope for v1, or do we ship "Edwin's security model without Edwin's messaging surface" first?

### Track C — Distribution, licensing, partnership
- **C1.** Apache-2.0 (Dolphin Milk) → embedding is fine. Partnership with John = coordinated roadmap, possibly co-marketed launch leveraging his x402 marketplace traction.
- **C2.** BSL-1.1 (Edwin) → in-house, but the "competitive product" clause needs explicit clearance from Jake in writing if Hodos+Edwin ships as a product. Tractable but blocking until done.
- **C3.** Cross-platform binary pipeline for `dolphin-milk` (Windows + macOS today; Linux later, per `LINUX_BUILD.md`).
- **C4.** Code-signing pipeline (Windows Authenticode + macOS notarization) for the bundled subprocess.
- **C5.** Conversations with John and Jake before any of this is publicly committed.

### Track D — UX
- **D1.** First-run flow. Empty wallet → "Agent" overlay opens → minimum credible "try it" budget. Options: (a) bundled trial sats, (b) one-click credit-card → BSV via an x402 partner endpoint, (c) "send to this address" QR.
- **D2.** Single agent per browser vs. per-domain agents. Comet runs one agent everywhere → blast radius is the whole session. Per-domain agents (banking agent vs. shopping agent) is safer but more UI. Lean per-domain, but cost it.
- **D3.** Where do agent-paid x402 transactions appear in the wallet panel? New "Agent" sub-type? Mixed with browsing payments? The existing `paid_url` field already establishes a pattern.
- **D4.** Model picker. Default route (`gpt-5-mini`) for v1; advanced model selection in agent settings. Don't make users pick a model on day one.

### Track E — Watch Comet
- **E1.** Standing record of new Comet CVEs / criticisms. Fuels our security marketing.
- **E2.** Track Comet's monetization shifts. If Perplexity adds a hard paywall, our pay-per-prompt story sharpens.

---

## Proposed iteration loop

Three passes. Each produces one document. Each refines the previous pass's understanding. Iterate until two passes converge.

**Pass 1 — Product** (this doc, expanded into a tight `PRODUCT_VISION.md`)
- User problem, user, end state, marketing wedge. No architecture, no port numbers.
- Deliverable: 2–3 page pitch we could actually run by Jake and John.

**Pass 2 — Architecture** (`ARCHITECTURE_v1.md`)
- 3-party diagram in detail. Process layout. What signs what. Trust boundaries.
- Existing Hodos surfaces reused (wallet endpoints, overlays, IPC chain, `PermissionEngine`).
- NEW vs. EXISTING, Windows vs. macOS marked.
- Deliverable: enough to scope a 2-week spike.

**Pass 3 — Research** (`FEASIBILITY.md`)
- Run the Track-A/B/C/D canaries. Record what worked, what didn't.
- Update product doc if architecture forces a product change.
- Update architecture doc → `ARCHITECTURE_v2.md`.

**Loop:** 1 → 2 → 3 → re-evaluate → 2 → 3 → …

**Exit condition:**
- Architecture stable across two iterations, AND
- Feasibility canaries A1, B1, C2 return green, AND
- Jake and John aligned on the partnership shape in writing.

Then the work splits into a real engineering plan (Hodos-side, Dolphin-Milk-side, Edwin-side) and the marketing one-pager moves into `marketing/intelligence/features/Dolphin Milk Integration/`.

---

## Open questions for Matt (before Pass 2)

1. **Who do we talk to first — John or Jake?** Both partnerships matter; serialize or parallel?
2. **Edwin scope for v1: full or security-only?** Multi-channel inbox is a separate product surface from "browser agent." Including it widens scope significantly.
3. **Trial budget on first-run.** Bundling sats means somebody pays for them. Acceptable per-user cost?
4. **Comet's free tier is a real competitive pressure.** Is "pay-per-prompt" sufficient, or do we need a low-cost optional bundled tier too?
5. **OAuth-Connected-Agent timing.** The downstream feature (`OAUTH_CONNECTED_AGENT.md`) needs Google verification with months of lead time. Do we start the OAuth submission clock during this integration spike, or wait?

---

## Related docs

- `DOLPHIN_MILK_INTEGRATION.md` (sibling) — original research doc, more code-grounded
- `../Possible-MVP-Features/OAUTH_CONNECTED_AGENT.md` — downstream feature, OAuth verification critical path
- `../../../Marston Enterprises/Hodos/marketing/intelligence/FEATURE_PRIORITY.md` — bucket assignment (RESEARCH)
- `../../../Marston Enterprises/Hodos/marketing/intelligence/EFFORT_MATRIX.md#dolphin-milk-integration` — effort scoring
