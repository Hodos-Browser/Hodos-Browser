# Dolphin Milk Integration — Research Doc

> 📁 Part of the **Dolphin Milk + Edwin Integration** doc set (technical half) — see `README.md` here for the full map. Pitch/product/outreach docs live in the Marston marketing intelligence vault.

**Status:** RESEARCH (per `marketing/intelligence/FEATURE_PRIORITY.md`)
**Effort:** L (per `marketing/intelligence/EFFORT_MATRIX.md#dolphin-milk-integration`)
**First logged:** 2026-05-11
**Source repo:** `C:\Users\archb\Dolphin_Milk\rust-bsv-worm` (upstream per README: `https://github.com/Calgooon/rust-bsv-worm`)

> **Downstream dependency:** This integration is a prerequisite for `OAUTH_CONNECTED_AGENT.md` (agent-with-OAuth-tools to act on Google/Gmail/Drive/YouTube on behalf of the user). That feature has a **Google OAuth verification critical path** — 4–16+ weeks of review, plus potential annual CASA assessment cost (~$3K–$15K+/year) for restricted scopes. If pursuing OAuth-Connected-Agent on top of this integration, read the **"Critical path — Google OAuth verification"** section at the top of `OAUTH_CONNECTED_AGENT.md` before locking the timeline here. The verification submission window should open the moment a working demo exists, not at launch.

## The idea in one sentence

Bundle John Calhoun's **Dolphin Milk** open-source AI agent (Rust binary + Lit web UI) inside Hodos so users get a working AI agent with a BSV wallet **without ever touching a terminal**.

## What Dolphin Milk actually is

A single-binary Rust AI agent that:

- **Pays for its own LLM inference via x402** — no API keys, no Claude account, no OpenAI account. The agent's wallet pays ~200 sats per LLM call (~$0.0002).
- **Records on-chain BRC-18 proofs** of every action (think, tool call, decision) as OP_RETURN hash chains. Verifiable audit trail.
- **Ships with an embedded BSV wallet OR connects to an external wallet** on `localhost:3322` (Calhoun's `bsv-wallet-cli`).
- **Serves a Lit-based web UI at `http://localhost:8080/ui/`** with chat, dashboard, budget tracking, proof chain visualization, memory browser.
- **Has an MCP server mode** for Claude Code / Codex integration (`dolphin-milk mcp`).
- **Implements 14 BSV BRC standards** — BRC-18, 29, 31, 33, 42, 46, 48, 52, 56, 60, 77, 78, **100**, 105.
- **38 tools** across reasoning, memory, web, communication, wallet, x402, files, analysis, agent.

**Common misconception to correct:** Dolphin Milk does NOT require a Claude account or an OpenAI account. The "no API keys" claim is real — the agent pays the LLM provider via x402 micropayments using BSV. The user's question "maybe requires a Claude account, I don't know" is answered: **no**.

## Current friction for normal users

Today, to use Dolphin Milk a user must:

1. Install Rust 1.85+ (`rustup update`) — Rust toolchain not in most users' workflow
2. `cargo install bsv-wallet-cli` — installs the external wallet
3. `git clone https://github.com/Calgooon/rust-bsv-worm.git`
4. `cargo build --release` — Rust release build, several minutes first time
5. `./target/release/dolphin-milk init` — terminal command, generates a config and wallet
6. `./target/release/dolphin-milk start` — terminal command, starts the daemon
7. Open `http://localhost:8080/ui/` in a browser
8. Send BSV to the funding address shown
9. Click "Check for payment" — agent internalizes the funding
10. Chat

**Real-world fail modes** for the user we're trying to reach:
- "What's a terminal?"
- "How do I install Rust?"
- "Why isn't `cargo` working?" (PATH issue)
- "I sent BSV but nothing happened" (auto-detection lag)
- "The build is taking forever" (Rust first-build cost)

Steps 1–6 alone disqualify ~95% of non-developer users.

## The Hodos integration thesis

> Hodos already has a Chromium browser + a Rust BSV wallet (BRC-100, port 31301) + a polished overlay UI system + a payment-success notification chain. If we bundle dolphin-milk as a child process and surface its UI through Hodos, users get an agent with a wallet from a single install. No terminal. No Rust install. No separate wallet funding.

The user installs Hodos. They already have a wallet (Hodos's). They click a button — "Launch AI Agent" or `agent:` in the omnibox — and Hodos:

1. Spawns the bundled `dolphin-milk` binary as a child process, pointing it at Hodos's wallet on `:31301` instead of `bsv-wallet-cli` on `:3322`.
2. Waits for the agent's HTTP server to come up on `:8080`.
3. Opens a Hodos tab or overlay to `http://localhost:8080/ui/` — the agent's own web UI.
4. Surfaces the agent's BSV spending against Hodos's wallet (the same wallet that pays for browsing) in Hodos's existing wallet panel transaction history.
5. Surfaces the BRC-18 on-chain proofs as a new "agent activity" view, or as annotated entries in the wallet's transaction history.

User experience: install Hodos → use AI agent. That's it.

## Hard questions to answer before committing

These are the questions a research doc exists to surface, not to resolve.

### 1. Is the wallet API compatible?

Dolphin Milk's wallet client connects to `bsv-wallet-cli` on `:3322`, which exposes a **BRC-100** wallet API (28 endpoints per BRC-100). Hodos's Rust wallet on `:31301` is also BRC-100. **Are the two implementations interoperable at the wire level?** Needs a side-by-side audit of:

- Endpoint coverage (does Hodos implement everything Dolphin Milk calls?)
- Request/response shape compatibility (camelCase vs snake_case, optional fields, error formats)
- Authentication — does Dolphin Milk's wallet client do BRC-31 auth, and does Hodos's `well_known_auth` endpoint accept its handshake?
- Currency/sats vs satoshis vs BSV
- BEEF transaction format (BRC-29 payment construction)

If they're not compatible, options:
- (a) Write a thin adapter shim that translates between the two surfaces. Lowest friction; can ship in `rust-wallet/src/handlers.rs` as additional routes.
- (b) Update Hodos's wallet to be wire-compatible with `bsv-wallet-cli`. Bigger lift, possibly desirable.
- (c) Keep Dolphin Milk on its own embedded wallet and link the two wallets via cross-wallet payment.

Recommended approach for the research stage: build a single canary test that runs Dolphin Milk's `dolphin-milk status` command pointed at Hodos's wallet and see what fails.

### 2. Cross-platform Rust binary distribution

Dolphin Milk is Rust. The Hodos installer would need to bundle:

- `dolphin-milk.exe` (Windows)
- `dolphin-milk` (macOS — universal binary or arm64 + x86_64 separately)
- `dolphin-milk` (Linux — when Linux build lands, see `LINUX_BUILD.md`)

Each must be code-signed (Windows Authenticode, macOS notarization). Each must be auto-updated independently from Hodos. This is a real shipping problem, not a hard one.

### 3. Child-process lifecycle

Hodos would need to:

- Spawn dolphin-milk with the right config (wallet URL = Hodos's, port = some unused port, working dir = under Hodos's profile dir)
- Health-check it (poll `/health` or whatever Dolphin Milk exposes)
- Restart on crash
- Kill cleanly on Hodos shutdown
- Detect port conflicts (what if `:8080` is taken?)
- Manage logs

Patterns already exist in Hodos: the C++ shell already spawns the Rust wallet and adblock-engine as child processes. Adding a third managed child is incremental, not novel.

### 4. UI integration

Two paths:

**(a) Embed the existing Dolphin Milk web UI.** Open `localhost:8080/ui/` as a special Hodos tab (or a Hodos overlay). Pros: zero new UI work; full feature parity with upstream Dolphin Milk. Cons: the UI is Lit-based and styled differently from Hodos's MUI; jarring visual context switch.

**(b) Build a Hodos-native UI on top of Dolphin Milk's HTTP API.** Dolphin Milk exposes 69 routes — Hodos's frontend could call them directly. Pros: visual coherence, integration with Hodos's wallet panel. Cons: lots of new UI work, must keep up with Dolphin Milk's API as it evolves.

Recommended: start with (a) for the integration spike to prove the model works; consider (b) for a polished v2 if the spike validates the user appetite.

### 5. On-chain proof visibility

Every Dolphin Milk action becomes a BRC-18 OP_RETURN proof. These are real BSV transactions paid for from the wallet. They will appear in Hodos's transaction history. What does Hodos's existing transaction list show for "OP_RETURN proof, no value transfer, 1 sat to fee"? Will users be confused? A new transaction sub-type ("agent proof") with friendly rendering in the wallet panel may be required.

Also: should we let users **opt out** of on-chain proofs to save sats? Dolphin Milk's whole pitch is the proofs are the point — but for the Hodos default, costs matter.

### 6. Funding flow

Currently Dolphin Milk's UI says "Send BSV to this address." If we point Dolphin Milk at Hodos's wallet, the agent uses Hodos's UTXOs directly — funding is already handled by Hodos's existing flow. **But** Dolphin Milk's UI still shows the funding step. We'd either need to:

- Detect "wallet has balance, skip funding UI" — modify Dolphin Milk's UI flow (upstream change or fork)
- Hide the funding UI in the Hodos shell — easier but ugly
- Just let it show; users with funded Hodos wallets see balance > 0 and the funding step is a no-op

### 7. What if the user wants to use a different LLM provider?

Dolphin Milk defaults to OpenAI via x402agency endpoints. Users may want Claude, may want a local Ollama, may want to use a paid API key (rare in this audience, but possible). The config supports this — but exposing the config in a non-terminal way means a new Hodos settings panel.

### 8. Licensing and upstream relationship

The repo has a `LICENSE` file (need to confirm contents — likely MIT or similar permissive). Bundling Dolphin Milk in Hodos's installer is fine for permissive licenses. For Hodos to coordinate updates / get bug fixes / influence the roadmap, an explicit conversation with John Calhoun (@johncalhooon) is appropriate.

## Would people like it?

This is a product judgment, not a technical one — but the framing is:

**Pro:**
- "AI agent on your laptop with no API key" is a real differentiator.
- BSV's $0.0001/tx fee structure makes this economically possible (the same agent on BTC/ETH would be prohibitively expensive).
- The proof-chain story is a tangible advantage over "trust me, the chatbot didn't lie."
- It's a story Hodos can tell that no Chromium fork can tell.

**Con:**
- Local LLM inference is what most "AI on your laptop" users want. Dolphin Milk pays for hosted inference; the user still needs an internet connection and BSV to spend.
- The agent is opinionated (38 tools, BRC-18 proof chain) — not a blank-slate chat interface. Users coming from ChatGPT may find it jarring.
- BSV-native UX may confuse users coming from non-BSV crypto.
- If Dolphin Milk evolves fast (it appears to), Hodos's bundled version may drift from upstream.

**Net assessment:** worth a spike. It's a genuine "browser+wallet+agent" story that nobody else can ship today.

## Open questions

- [ ] Wallet API compatibility audit (Hodos's BRC-100 vs Dolphin Milk's expected `bsv-wallet-cli` surface)
- [ ] Cross-platform binary build pipeline for `dolphin-milk` (Windows + macOS today; Linux later)
- [ ] License confirmation (read `Dolphin_Milk/rust-bsv-worm/LICENSE`)
- [ ] Outreach to @johncalhooon about an integration partnership / coordinated launch
- [ ] Decision: embed upstream UI vs. build Hodos-native UI
- [ ] Decision: on-chain proofs default-on vs default-off in Hodos's bundled config
- [ ] Cost model: typical "20-minute chat session" cost in sats, surfaced honestly in the install flow

## Related

- `marketing/profiles/bsv/DolphinMilkAI.md` — promo account profile
- `marketing/profiles/bsv/x402agency.md` — marketplace profile (Dolphin Milk consumes its LLM endpoints)
- `marketing/intelligence/FEATURE_PRIORITY.md` — bucket assignment (RESEARCH)
- `marketing/intelligence/EFFORT_MATRIX.md#dolphin-milk-integration` — full effort scoring
- `marketing/intelligence/ECOSYSTEM_PULSE.md` — week-of-2026-05-11 entry
