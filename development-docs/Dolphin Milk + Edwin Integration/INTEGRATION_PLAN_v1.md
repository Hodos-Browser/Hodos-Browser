# Hodos + Edwin + Dolphin Milk — Integration Plan v1

**Audience:** Jake (Edwin), John (Dolphin Milk), Matt (Hodos)
**Status:** Living first-draft. Appended as we learn. Sent to both Jake AND John when v1 is ready.
**Companion docs:**
- `EDWIN_SETUP_FEEDBACK_FOR_JAKE.md` — engineering feedback to Jake on the Edwin install path
- `ARCHITECTURE_TECHNICAL.md` — three-party architecture, PermissionEngine + envelope synthesis, 7 questions for Jake
- `PRODUCT_OUTLINE_v1.md` — product framing
- `EDWIN_VS_DOLPHIN_MILK_SECURITY.md` — security model comparison
- `CANARY_A1_WALLET_COMPAT.md` — wallet compat verification + 3 small patches
- `CONVERGENCE_NARRATIVE.md` — Route-B pitch story

**Feeds three downstream deliverables:**
1. Pitch deck updates (Golden, June 25)
2. PoC implementation plan (June 12 selection → June 25 pitch window)
3. Test plan

---

## TL;DR

Hodos = the browser + Bitcoin-native wallet + permission engine. Edwin = the on-device assistant with memory and channels. Dolphin Milk = the x402-paid LLM and tool service. The three pieces compose into a single user-facing product: an AI-augmented browser where the assistant pays for its own model calls out of the user's wallet, with the wallet enforcing every payment.

This document describes how the three pieces fit together end-to-end, the v1 (PoC) vs v2 (productionized) bundling strategies, the API-key UX, and the Windows / macOS installation flows.

---

## 1. The three-party architecture

```
┌─ Hodos Browser ─────────────────────────────────────────────────┐
│                                                                  │
│   Browser UI (CEF)                                              │
│       ↕                                                          │
│   PermissionEngine (Rust)  ←──── per-call / per-domain / per-    │
│       ↕                          session policy; auto-approve    │
│   Rust wallet (BRC-100 + BSV signing + DPAPI/Keychain)          │
│       ↕                                                          │
│   x402 client (BRC-29 BEEF payment, BRC-103 AuthFetch)          │
│                                                                  │
└────────┬─────────────────────────────────────────────────────────┘
         │                                  ↑
         │ ws:// + local IPC                 │ ws:// + paid LLM API
         ↓                                  │
┌─ Edwin (gateway) ──────────────────┐    ┌─ Dolphin Milk ────────┐
│                                    │    │                        │
│   Gateway (Node, port 18789)       │    │   x402-paid endpoints  │
│   + Memory (qmd + shad-context)    │    │   for chat / tools     │
│   + Channels (optional)            │    │   (HTTP 402 → retry    │
│   + Agent runtime (pi-* SDK)       │    │    with BEEF payment)  │
│                                    │    │                        │
└────────────────────────────────────┘    └────────────────────────┘
```

**The three integration points:**

1. **Hodos ↔ Edwin** — local. Hodos talks to Edwin's gateway over a local WebSocket (`ws://localhost:18789`). Hodos contributes the wallet + permission UX; Edwin contributes the assistant brain + memory. Edwin authenticates to the gateway with a BAP identity Hodos manages.

2. **Edwin ↔ Dolphin Milk** — remote. Edwin's agent loop wants to call LLMs and tools. Instead of holding API keys, it makes x402-style HTTP calls to Dolphin Milk endpoints. The server returns 402, Edwin asks Hodos's wallet to pay via BRC-121 (paid retry with BEEF payment headers), Hodos's PermissionEngine gates the payment, Dolphin Milk returns the LLM response.

3. **Hodos ↔ Dolphin Milk** — remote, but mediated by Edwin in v1. Hodos doesn't talk to Dolphin Milk directly in the assistant flow; Edwin orchestrates and Hodos is the payment + permission backstop. (Direct Hodos → Dolphin Milk for embedded UI in browser pages is a separate flow — same x402 mechanism.)

See `ARCHITECTURE_TECHNICAL.md` for the deeper walkthrough, including envelope synthesis and the 7 Jake-questions.

---

## 2. v1 (PoC for Golden June 25 pitch) vs v2 (post-pitch productionized)

### v1 — separate bundled binaries

| Component | Shape in v1 |
|-----------|-------------|
| Hodos Browser | Existing CEF + Rust wallet + frontend, installed via Hodos installer |
| Edwin gateway | **Separate bundled binary** invoked as a subprocess by Hodos at first launch, similar to how Hodos already manages the Rust wallet + adblock-engine sidecars |
| Dolphin Milk | Hosted service at `dolphinmilk.x` (or equivalent) — no install footprint on user machines |
| qmd memory engine | Bundled alongside Edwin in the same install package |
| Edwin desktop | **Hidden / not used** in v1 — Hodos's own browser UI hosts the assistant chat surface |

The v1 PoC reuses Hodos's existing sidecar pattern. Hodos already runs:
- Rust wallet on `localhost:31301`
- adblock-engine on `localhost:31302`

Edwin gateway becomes the third sidecar on `localhost:18789`. Hodos's process supervisor (today `dev-wallet.ps1` / `dev-wallet.sh`, future installer-managed) starts all three.

**v1 critical-path tasks (June 12 selection → June 25 pitch):**
- Edwin gateway runs from Hodos's install dir (not `~/edwinpai`) — but on **Linux/macOS only** in v1. On Windows we install Edwin inside WSL and Hodos talks to it over `ws://localhost:18789` (the mirrored networking path).
- Bundle qmd OpenAI fork (Jake's `feat/openai-embeddings` branch) into the same install dir
- Wire Hodos's chat panel to Edwin's WebSocket
- Wire Edwin's outbound LLM calls to Dolphin Milk via x402
- Verify PermissionEngine sees + gates every Dolphin Milk payment (this is the demo magic moment)

### v2 — compiled-in

| Component | Shape in v2 |
|-----------|-------------|
| Hodos Browser | Same CEF shell |
| Edwin core (gateway + agent loop) | Compiled in to Hodos's build pipeline, exposed as a Rust crate or in-process subprocess |
| qmd | Same — but possibly Rust-native by then, replacing the Node implementation |
| Channels (WhatsApp, Telegram, etc.) | Loaded as plugins; opt-in per channel |
| Dolphin Milk | Same hosted service |

v2 collapses the sidecar boundary: Edwin's gateway becomes part of Hodos's wallet IPC layer, shares the same supervisor, shares the same identity (BAP) primitives. The user-facing experience is identical to v1 but the install footprint shrinks and the IPC surface area drops.

**v2 is captured here as the destination, not the v1 deliverable.** The v1 architecture should make v1 → v2 incremental (move code modules across the binary boundary), not a rewrite.

---

## 3. API-key UX strategy

### The problem

Edwin requires an OpenAI (or equivalent) API key today for embeddings via qmd (`text-embedding-3-small`, 1536-dim). Dolphin Milk's value prop is *eliminating* API keys via x402. So which is it — does the user need a key or not?

Matt's framing: this isn't a binary choice. The right product is a hybrid where the default path is friction-free (x402 covers everything) and the user can opt into bringing their own key when it's economically beneficial.

### The cost question (TBD — research item)

We need a real cost model, not vibes:

| Workload | x402 per-call cost (est) | API subscription cost (est) | Breakeven point |
|----------|--------------------------|------------------------------|------------------|
| Chat (Sonnet-class) | _TBD_ | $20/mo Claude Pro = unlimited UI, but no API access. API tier billed per-token. | _TBD_ |
| Chat (Opus-class) | _TBD_ | Same as above; API tier expensive at scale | _TBD_ |
| Embeddings (small) | _TBD_ | ~$0.02 / 1M tokens (OpenAI text-embedding-3-small) | Likely API wins at very low usage |
| Embeddings (large) | _TBD_ | More expensive but higher quality | _TBD_ |
| Tool calls (search etc) | _TBD_ | Varies per provider | _TBD_ |

Open question: does ChatGPT Plus / Claude Pro give the user API access? Generally **no** — subscription tier and API tier are billed separately. So a user paying $20/mo for Claude Pro can't reuse that key for Edwin without a separate API tier setup. This is a UX wrinkle worth surfacing in the settings flow.

### The UX

Three modes Hodos's settings should support:

**Mode 1: Fully x402 (default for new users)**
- No API key needed; all LLM + tool + embedding calls pay through the wallet via Dolphin Milk
- Lowest friction; highest per-call cost
- Default; lets us pitch "no signup, no API keys, just install and go"

**Mode 2: BYO API key (power users)**
- User pastes their own OpenAI / Anthropic / Gemini key
- Lowest per-call cost at scale; user manages their own billing relationship
- Settings UI: "Use my own API key for [chat / embeddings / both]" with cost-comparison transparency ("at your current usage, X mode is cheaper by $Y/mo")

**Mode 3: Mixed (probably the sweet spot)**
- User brings an OpenAI key for embeddings (where API is dramatically cheaper)
- x402 covers chat + tools (where the API premium isn't worth the signup friction)
- This may end up being the recommended default after the cost model is built

The settings panel needs to make the cost trade-off legible — not just "paste a key here" but "you've spent $X this month via x402; if you'd had an OpenAI key it would have been $Y."

### Acquisition UX for users who pick Mode 2 or 3

For the user who decides they want their own key, the settings panel should:
- Link directly to the provider's API-key page (with screenshots? embedded tutorial?)
- Optionally walk through the signup with an in-browser overlay (Hodos already has overlay infrastructure)
- Detect existing Claude Pro / ChatGPT Plus subscriptions and explain why the user needs a separate API setup
- Validate the pasted key with a probe call before saving

### Prereq taxonomy for the installer

| Bucket | What | Auto / user / platform-conditional |
|--------|------|----------------------------------|
| Hodos itself | CEF runtime, Rust wallet, adblock-engine, frontend bundle | Auto-install |
| Edwin gateway (Linux/macOS) | gateway binary, Node embedded runtime, qmd binary | Auto-install |
| Edwin gateway (Windows) | WSL2 + Ubuntu + Edwin install inside WSL | **Platform-conditional** — Windows installer needs to provision WSL if not present |
| Embeddings provider key | OpenAI key (Mode 2/3) | **User-provide** if Mode 2/3; otherwise N/A (Mode 1 uses x402) |
| BSV in wallet | Funded wallet for x402 payments | **User-provide** (Hodos onboarding already handles this) |
| OS admin password | Firewall rule (Windows mirrored networking only) | **User-provide**, only triggered on Windows mirrored mode |

---

## 4. Installation flows — Windows vs macOS

### 4.1 Windows install flow

Numbered Check / Action / Verify pattern (per the EDWIN_SETUP_FEEDBACK_FOR_JAKE.md §2.1 idempotency principle):

```
Step 1 — WSL2 + Ubuntu
  Check:  `wsl -l -v` shows Ubuntu, VERSION 2
  Action: `wsl --install -d Ubuntu` (admin PowerShell, reboot if prompted)
  Verify: `wsl --list --verbose` reports Ubuntu / VERSION 2 / running or stopped

Step 2 — systemd in WSL
  Check:  inside Ubuntu, `systemctl is-system-running` returns 'running' or 'degraded'
  Action: `sudo tee /etc/wsl.conf <<EOF` with `[boot]\nsystemd=true`, then `wsl --shutdown` from PowerShell
  Verify: same check as above, after restart

Step 3 — Mirrored networking (Win11; skip on Win10)
  Check:  `Get-Content $env:USERPROFILE\.wslconfig | Select-String mirrored`
  Action: write `[wsl2]\nnetworkingMode=mirrored` to `~/.wslconfig`, then `wsl --shutdown`
  Verify: `wsl ip addr show` includes Windows host IP (or skip — verify by Step 7 reachability)

Step 4 — Linux Node via nvm (inside Ubuntu)
  Check:  `realpath $(command -v node)` starts with $HOME/.nvm and `node -v` is v22+
  Action: install nvm, `nvm install 22`, `nvm alias default 22`
  Verify: `which node` resolves under ~/.nvm; `node -v` reports v22.x

Step 5 — pnpm
  Check:  `command -v pnpm` returns a path
  Action: `corepack enable && corepack prepare pnpm@latest --activate`
  Verify: `pnpm -v` prints a version

Step 6 — Edwin (clone + build)
  Check:  `[ -f ~/edwinpai/dist/index.js ]`
  Action: `git clone https://github.com/jonesj38/edwin.git ~/edwinpai && cd ~/edwinpai && pnpm install && pnpm build && npm install -g .`
  Verify: `edwinpai --version` prints a version

Step 7 — Edwin first-run config
  Check:  `[ -f ~/.edwinpai/edwinpai.json ]`
  Action: `edwinpai setup` (interactive)
  Verify: config file exists; `gateway.bind` set to `lan` (or `loopback` if mirrored networking is on)

Step 8 — qmd OpenAI fork
  Check:  `qmd --version` reports 2.5.x (the fork, not 1.1)
  Action: clone jonesj38/qmd feat/openai-embeddings, install with bun, symlink into ~/.local/bin
  Verify: `qmd --version` reports 2.5.x

Step 9 — Service install + linger
  Check:  `systemctl --user is-active edwinpai-gateway.service` reports 'active'
  Action: `edwinpai daemon install && edwinpai daemon start && loginctl enable-linger "$USER"`
  Verify: `ss -ltn | grep 18789` shows listening; `systemctl --user is-active` reports active

Step 10 — Network reachability from Windows
  Check:  in PowerShell, `Test-NetConnection -ComputerName localhost -Port 18789` reports TcpTestSucceeded: True
  Action: if False AND mirrored networking is on, run the admin firewall command:
          New-NetFirewallHyperVRule -Name 'EdwinPAI-Gateway-18789' -DisplayName 'EdwinPAI Gateway 18789' \
              -Direction Inbound -VMCreatorId '{40E0AC32-46A5-438A-A0B2-2B479E8F2E90}' \
              -Protocol TCP -LocalPorts 18789 -Action Allow
          (else, recheck `gateway.bind` in edwinpai.json — should be 'lan')
  Verify: same check as above, after the rule is added

Step 11 — Hodos installs + connects to Edwin
  Check:  Hodos sidecar supervisor lists Edwin at port 18789
  Action: install Hodos via its standard installer; the supervisor wires the connection
  Verify: Hodos chat panel opens; first message round-trips through Edwin and gets a response
```

### 4.2 macOS install flow

Same step-numbering for parity with Windows. Steps 1–3 collapse to a single "install Edwin natively" since there's no WSL layer:

```
Step 1 — macOS prereqs
  Check:  `command -v brew` and `xcode-select -p`
  Action: install Homebrew + Xcode command-line tools if missing
  Verify: both checks pass

Step 2 — Node + pnpm
  Check:  `node -v` ≥ v22 AND `pnpm -v` succeeds
  Action: `brew install node@22 pnpm` (or nvm)
  Verify: both checks pass

Step 3 — Edwin (clone + build)
  Same as Windows Step 6 but natively in ~/edwinpai

Step 4 — Edwin first-run config
  Same as Windows Step 7. `gateway.bind: loopback` is fine on macOS — no WSL bridge to worry about.

Step 5 — qmd OpenAI fork
  Same as Windows Step 8

Step 6 — Service install + launchd
  Check:  `launchctl list | grep edwinpai`
  Action: `edwinpai daemon install && edwinpai daemon start` — uses launchd on macOS
  Verify: `lsof -i :18789` shows Edwin listening; `launchctl list` includes the service

Step 7 — Hodos installs + connects to Edwin
  Same as Windows Step 11; no firewall rule needed (no Hyper-V firewall on macOS)
```

**macOS-specific items:**
- Apple Silicon vs Intel matters for native modules (`node-llama-cpp`, `sharp`, `@lydell/node-pty`, `@matrix-org/matrix-sdk-crypto-nodejs`). Likely fine since pnpm installs platform-correct binaries, but worth a verification pass.
- macOS Gatekeeper / notarization is the equivalent gotcha to Windows SmartScreen — Hodos's installer must be notarized for the Edwin sidecar binaries to launch without scary "unidentified developer" prompts.
- macOS Keychain is the natural place for the gateway token + API keys. (See EDWIN_SETUP_FEEDBACK_FOR_JAKE.md §4 — same OS-keychain story applies on both platforms.)

---

## 5. Permission model integration (Hodos PermissionEngine ↔ Edwin)

Hodos's PermissionEngine (Rust, `rust-wallet/src/permission_service.rs` and engine port) is what gates every BRC-100 wallet operation today. The integration question: how does Edwin's agent activity flow through it?

Two boundaries to gate:

1. **Edwin → Dolphin Milk (paid LLM calls)** — every x402 retry is a BRC-29 payment. PermissionEngine already gates these via the cap cascade (per-tx / per-session / per-domain / max-tx-per-session). Edwin doesn't get a free pass; Dolphin Milk calls hit the same engine cascade as any other paid HTTP request. *This is the core demo: when Edwin says "let me think about that for a sec," the user sees the wallet's green-dot animation fire as the payment auto-approves under the user's cap.*

2. **Edwin → user data (memory, channels, browser tabs)** — local-only, but still privacy-sensitive. Today Edwin has open access to its own memory dir, the channels it's enabled, and (via Hodos integration) the browser's tab content. We need to decide whether PermissionEngine gates this OR whether Edwin's gateway has its own permission layer for memory / channel access.

Open question for Jake + John in v1 review: does Edwin's gateway already have a permission layer (e.g., per-channel approve / deny), and if so, does Hodos's UX surface it or stay out of its way?

---

## 6. PoC implementation plan (June 12 selection → June 25 pitch)

13 days to ship a working demo. Critical path:

| Day | Milestone |
|-----|-----------|
| 12  | (Selection day) — kick off |
| 13–15 | Edwin sidecar bundled with Hodos installer (Linux + macOS); Windows uses WSL path |
| 16–18 | Hodos chat panel wired to Edwin WebSocket; basic prompt → response round-trip |
| 19–21 | x402 path: Edwin → Dolphin Milk via Hodos wallet → BEEF payment → response. PermissionEngine cap cascade gates the payment. |
| 22–23 | Demo dress rehearsals; the green-dot moment timed and visible |
| 24  | Buffer for issues |
| 25  | **Pitch** |

What we're NOT doing in 13 days:
- v2 compiled-in shape (it's the destination, not the v1 deliverable)
- Full channel-enablement UX (WhatsApp / Telegram / etc.); v1 demos chat + browser-context only
- Mode 2/3 API-key UX (Mode 1 is the default + v1 demo)
- Production notarization / installer signing (TBD for post-pitch)

---

## 7. Test plan

Three layers of test for the v1 integration:

### 7.1 Unit / component tests
- Hodos PermissionEngine cap cascade — already 25 tests in `cef-native/tests/permission_engine_test.cpp`; extend with Dolphin Milk payment scenarios
- Edwin gateway WebSocket auth handshake — Hodos's WS client correctly handles `connect.challenge` and 401 + retry
- x402 payment flow — Hodos's wallet correctly mints a BRC-29 BEEF, attaches BRC-121 headers, and submits the paid retry to Dolphin Milk

### 7.2 Integration tests (the demo path)
- Bring up all three: Hodos browser, Edwin gateway (sidecar), Dolphin Milk endpoint (mocked or live)
- Issue a chat prompt from Hodos's chat panel
- Verify: WebSocket message arrives at Edwin → Edwin's agent loop calls Dolphin Milk → 402 returned → BRC-121 retry → 200 with response → Edwin streams response back → Hodos renders
- Verify: PermissionEngine cap cascade fires on the payment; the green-dot animation fires in the tab UI
- Verify: payment shows up in Hodos's activity log with the correct amount and domain

### 7.3 Platform-parity tests
- Windows: full install flow (Section 4.1) end-to-end on a fresh Win11 VM
- macOS: full install flow (Section 4.2) end-to-end on a fresh macOS VM
- Both: Hodos's existing standard verification basket (youtube, x.com, github, etc. per CLAUDE.md Testing Standards) still works after Edwin sidecar is added — no regressions on the browser-core feature set

---

## 8. Open questions

For Jake:
- Bundling Edwin gateway as a sidecar in Hodos's installer — any licensing constraints? (BSL-1.1 status — verbal OK from 6-02 meeting, written confirmation still pending.)
- Does Edwin's gateway have a per-channel permission layer today, or do we add it as part of the integration?
- Is there a planned v2 Rust-native rewrite of the gateway, or does it stay Node?

For John:
- Dolphin Milk's x402 endpoints: full price list + per-call cost model? (Critical for the §3 cost analysis.)
- Embedding endpoints — does Dolphin Milk plan to host these too, or stays chat / tools only?
- Rate-limiting and abuse-prevention at the Dolphin Milk side — does it interact with Hodos's PermissionEngine cap cascade, or is it independent?

For Matt:
- Settings UX for the three modes (§3) — does Hodos's Settings overlay already have a place for this, or do we add a new pane?
- Pitch deck — does the demo flow (green-dot moment) make it onto a slide before June 25?

---

## Appendix: source notes

- 2026-06-02 Jake 1:1 meeting — verbal go-ahead on integration, BSL-1.1 status, AWS pitch
- 2026-06-02 prior Claude session — `EDWIN_SETUP_REPORT.md` + `WINDOWS_SETUP_GUIDE.md`
- 2026-06-03 setup-from-scratch attempt (alongside this doc being written)
- `ARCHITECTURE_TECHNICAL.md`, `EDWIN_VS_DOLPHIN_MILK_SECURITY.md`, `CANARY_A1_WALLET_COMPAT.md`, `CONVERGENCE_NARRATIVE.md` (existing dev-docs)
- `Edwin.txt` (2026-03-13 — older operations guide; useful for the qmd / Shad architecture)
