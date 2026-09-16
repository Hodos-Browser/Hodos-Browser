# Hodos + Edwin + Dolphin Milk — Integration Plan

> ℹ️ **PARTNER-FACING / PARTLY SUPERSEDED (2026-06-29).** This is the external-send integration
> overview written for Jake, John, and Calhoun. Its §1–§3 (three-party architecture, bundling intent,
> API-key/cost-mode UX) and §7 (test plan) remain useful, but its **Windows install sequencing (§4.1,
> WSL-based)** has been overtaken by the native-sidecar, no-WSL direction. For the current internal build
> sequencing see `../implementation/IMPLEMENTATION_PLAN_v1.md`, which supersedes §6 here and carries the
> still-valid API-key/cost material forward.

**Audience:** Jake (Edwin), John (Dolphin Milk), Calhoun, Matt (Hodos)
**Status:** Living document. Appended as we learn. Distributed for review and discussion.
**Companion docs:**
- `EDWIN_SETUP_FEEDBACK_FOR_JAKE.md` — engineering feedback to Jake on the Edwin install path
- `ARCHITECTURE_TECHNICAL.md` — three-party architecture, PermissionEngine + envelope synthesis, 7 questions for Jake
- `PRODUCT_OUTLINE_v1.md` — product framing
- `EDWIN_VS_DOLPHIN_MILK_SECURITY.md` — security model comparison
- `CANARY_A1_WALLET_COMPAT.md` — wallet compat verification + 3 small patches
- `CONVERGENCE_NARRATIVE.md` — Route-B pitch story
- `JOHN_DOLPHIN_MILK_LAUNCH_POST.md` — John's 2026-06 public launch post (canonical voice + pitch quotes + new "open discovery overlay" concept)

**Feeds two downstream deliverables:**
1. Pitch deck (Golden application; pitch event late June)
2. Implementation work — sidecar-first bundling, then compiled-in (see §2)

---

## TL;DR

Hodos = the browser + Bitcoin-native wallet + permission engine. Edwin = the on-device assistant with memory and channels. Dolphin Milk = the x402-paid LLM and tool service. The three pieces compose into a single user-facing product: an AI-augmented browser where the assistant pays for its own model calls out of the user's wallet, with the wallet enforcing every payment.

This document describes how the three pieces fit together end-to-end, the bundling architecture (sidecar phase → compiled-in destination), the API-key UX, and the Windows / macOS installation flows.

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

3. **Hodos ↔ Dolphin Milk** — remote, but mediated by Edwin. Hodos doesn't talk to Dolphin Milk directly in the assistant flow; Edwin orchestrates and Hodos is the payment + permission backstop. (Direct Hodos → Dolphin Milk for embedded UI in browser pages is a separate flow — same x402 mechanism.)

See `ARCHITECTURE_TECHNICAL.md` for the deeper walkthrough, including envelope synthesis and the 7 Jake-questions.

---

## 2. Bundling architecture

The integration ships as a single product to the user. Internally it composes the three pieces described in §1. This section defines how those pieces assemble at install + runtime, and how the bundling evolves over time.

### Components and where they live

| Component | Shape |
|-----------|-------|
| Hodos Browser | Existing CEF + Rust wallet + frontend, installed via Hodos installer |
| Edwin gateway | Bundled with Hodos. Sidecar process at first, compiled into Hodos's IPC layer as the architecture matures. Extends Hodos's existing `localhost:31301` / `31302` sidecar pattern. |
| qmd memory engine | Bundled alongside Edwin in the same install package |
| Edwin desktop | **Hidden / not used** — Hodos's own browser UI hosts the assistant chat surface |
| Dolphin Milk | Hosted service at `dolphinmilk.x` (or equivalent) — no install footprint on user machines |

Hodos already runs sidecar processes:
- Rust wallet on `localhost:31301`
- adblock-engine on `localhost:31302`

Edwin gateway becomes the third sidecar on `localhost:18789`. Hodos's process supervisor (today `dev-wallet.ps1` / `dev-wallet.sh`, future installer-managed) starts all three.

### Implementation evolution

**Initial bundling — sidecar.** Edwin gateway runs from Hodos's install dir (not `~/edwinpai`) as a separate bundled binary, started by Hodos's supervisor. Native Linux/macOS execution; Windows via WSL until the Windows-native gateway lands (see §4.1.1, which makes a Windows-native build a structural requirement). Bundle qmd's OpenAI-embeddings fork (Jake's `feat/openai-embeddings` branch) into the same install dir. Wire Hodos's chat panel to Edwin's WebSocket. Wire Edwin's outbound LLM calls to Dolphin Milk via x402. Verify PermissionEngine sees + gates every Dolphin Milk payment — this is the demo's magic moment.

**Target bundling — compiled-in.** Edwin's gateway becomes part of Hodos's wallet IPC layer, shares the same supervisor, shares the same identity (BAP) primitives. qmd may be Rust-native by then, replacing the Node implementation. Channels (WhatsApp, Telegram, etc.) loaded as opt-in plugins. User-facing experience is identical to the sidecar phase; the install footprint shrinks and the IPC surface area drops.

The sidecar architecture must make the migration to compiled-in **incremental** — moving code modules across the binary boundary one at a time — not a rewrite. This is the load-bearing design constraint for the sidecar phase.

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

**The "credits expire" wrinkle (verified live 2026-06-06).** OpenAI's prepaid credit model — used for the embeddings path qmd depends on — expires credits **1 year from purchase**. Users have to either:
- Buy small amounts often (more clicks, easier to forget)
- Buy large amounts and risk losing unused credit if usage tails off
- Enable auto-recharge (works but requires trust in the threshold behavior)

This is a **structural advantage for Hodos's x402-only mode (Mode 1)**: BSV credit in the user's wallet doesn't expire. Money sits there until the user spends it on a model call. This is worth one bullet in the pitch deck — "your AI credit doesn't have a 12-month timer like ChatGPT's API tier." Subtle but real differentiator.

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

**Mode 4: BYO ChatGPT / Claude subscription via OAuth (newly observed 2026-06-06)** — Edwin's `onboard` wizard offers "OpenAI Codex (ChatGPT OAuth)" as a separate auth path from "OpenAI API key." This routes calls through the user's existing ChatGPT subscription rather than per-token API billing. Pros: leverages an already-paid subscription the user has; no separate API setup. Cons: tied to OpenAI specifically (no equivalent for Claude/Gemini today); usage caps follow the ChatGPT tier (Plus/Pro/Team), not the API tier; mixes personal-subscription billing into a business product (relevant for B2B users). **Research item:** add to the cost model. The "user already pays $20/mo for ChatGPT Plus and might want to reuse it" question now has an actual technical answer (yes, via OAuth — at least for OpenAI). Likely a Mode-2 alternative for users with existing subscriptions, not a Mode-1 replacement.

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

### 4.1.0 Hodos pairing UX as a structural UX advantage over standalone Edwin

The 2026-06-07 first-connect session surfaced a concrete Hodos win that wasn't obvious before. Standalone Edwin Desktop's first-connect-to-gateway flow on Windows is brittle because of three independent issues (see `EDWIN_SETUP_FEEDBACK_FOR_JAKE.md` §5.7):

1. The Gateway Mode option is a dead-end on Windows (Edwin gateway is Linux-only; the UI doesn't know that)
2. `http://localhost:18789/` fails with `Test Connection` because Windows prefers IPv6 and gateway listens IPv4-only
3. Auth token has to be hand-grabbed from a JSON5 file in WSL

For Hodos's bundled integration, ALL THREE of these vanish:

| Standalone Edwin issue | Hodos solution |
|------------------------|-----------------|
| Gateway Mode broken on Windows | Hodos installer provisions / supervises the WSL gateway directly (per §4.1.1 Option A). No Mode picker. |
| `localhost` → IPv6 → fail | Hodos's own wallet sidecar already runs on Windows and Hodos's wallet IPC layer manages localhost addressing — same connection-handling pattern handles Edwin. We use 127.0.0.1 throughout. |
| Token hand-extraction from WSL | Hodos's installer reads `~/.edwinpai/edwinpai.json` directly (it's on disk on the same machine) and passes the token to the Hodos chat surface via existing IPC — no human paste required. |

**Translation for the pitch:** "Hodos eliminates a 30-minute first-connect for Edwin on Windows down to zero clicks. The browser owns the integration so you never see the rough edges." Concrete advantage worth a slide.

### 4.1.1 Windows distribution strategy (research item — high priority for Hodos)

The Windows install flow above (WSL2 + Ubuntu + ext4 + native Linux Node + systemd) is realistic for developers but is a hard sell for non-developer Windows users — and non-developers will be the majority of Hodos's audience. The WSL2 step alone (admin PowerShell + reboot + Linux account creation + systemd configuration) is enough friction to lose most consumer users before they ever see Edwin.

Five options for how Hodos handles this in the integration; **research and recommendation needed before sidecar bundling begins**:

| Option | Shape | UX cost | Engineering cost |
|--------|-------|---------|-------------------|
| **A. Hodos installer provisions WSL** | The Hodos `.msi` checks for WSL2, installs it (admin elevation + reboot), then installs Edwin inside it. Single "Install Hodos" click does the whole thing. | Best UX possible given current Edwin architecture. Still requires admin + reboot at install time. | High — installer needs to handle admin elevation, reboot resumption, WSL kernel install, Ubuntu image download, post-reboot continuation, all error paths. |
| **B. Hodos requires WSL pre-installed** | Hodos installs cleanly; Edwin works only if WSL+Ubuntu is already there. First-run wizard detects WSL state and walks user through setup. | Worst UX — pushes the WSL pain onto the user. Lots of drop-off. | Low — just detection + hand-off. |
| **C. Windows-native Edwin build** | Rewrite / re-package Edwin's gateway so it runs as a native Windows process (no WSL needed). Eliminates 9P entirely. | Best long-term UX — Edwin becomes a normal Windows app. | Very high — touches `node-llama-cpp`, `sharp`, `@lydell/node-pty`, all native modules. Likely needs Jake's cooperation upstream. **This is the STRUCTURAL requirement for any Windows user above developer tier — see updated reasoning below.** |
| **D. Docker Desktop (WSL2 backend)** | Edwin ships as a Docker image; Hodos installer pulls Docker Desktop and runs the image. | Mixed — moves Edwin's pain to Docker's pain. Docker Desktop has its own licensing + install weight. | Medium — leverages existing tooling, but Docker Desktop install is itself non-trivial. |
| **E. Rust-native gateway** | Edwin's gateway gets rewritten in Rust over time. Native cross-platform compilation; no Node, no native-module-per-OS issue. | Best long-long-term UX. | Very high — multi-quarter Edwin engineering effort. Aligns with the "compiled-in with Hodos" target bundling described in §2. |

**Matt's instinct (2026-06-06):** "A lot of people will not like" the WSL install path. Worth recommending to Jake (Option C) that he prioritize a Windows-focused build, and worth flagging to John that this is the highest-friction part of the three-party product on Windows.

**Empirical update (2026-06-08) — Option C is no longer just preference, it's structural.** We measured Shad searching a 500-file directory under both conditions:

| Source | Files | Wall-clock per query |
|---|---|---|
| `/mnt/c/Users/archb/Hodos-Browser/` (Windows-side, via 9P) | ~500 | **1m 43s** |
| `~/repos/BRCs/` (WSL ext4 native) | 33 | **0.53s** |

That's a **200× slowdown**, almost entirely waiting on Windows file I/O through the WSL 9P bridge. At 100 seconds per query, Edwin recall against the user's actual content is **not chat-capable**. A 2-minute reply latency reads as broken software to any user.

This rewires Option C's status:
- **Option B (Hodos requires user to install WSL):** broken UX for any user with content on Windows. Cannot ship to non-developer audience.
- **Option A (Hodos installer provisions WSL automatically):** still suffers the 9P problem. Workaround: WSL-side mirror of user content via git-sync layer (designed in `../../DevOps-CICD/WSL_HYBRID_WORKSPACE.md`). Works for our own dev/pitch use; not acceptable for end users.
- **Option C (Windows-native Edwin gateway):** the only path that gives Windows users chat-speed responses against their own documents.

**Therefore Option C moves from "ideal long-term answer" to "required for any Windows-audience product."** The pitch deck should reflect this — "Hodos is making Edwin's Windows experience first-class" is now a measured commitment, not a UX wish.

The interim sync-layer architecture is documented in `../../DevOps-CICD/WSL_HYBRID_WORKSPACE.md` for use during our own pre-Option-C development cycle. It's a developer tool, not a consumer-shippable workaround.

**Most-elegant integration question for Hodos:** if Option A (Hodos provisions WSL) is the near-term reality while we wait for Option C, how do we make the install feel like one click? Concrete sub-questions:
- Can Hodos's installer carry the WSL kernel + a pre-built Ubuntu image as embedded resources, avoiding the post-reboot download step?
- Can the Edwin install happen non-interactively (no `pnpm install` prompts, no first-run wizard) via a baked config?
- Can the systemd + linger setup be scripted entirely from the installer with no user input after the initial admin prompt?

Add to follow-up email to Jake: "What's your stance on a Windows-native gateway build? We're sizing the install UX for non-developer Windows users and the WSL barrier is the single biggest friction."

### 4.1.2 Wallet-resident identity collapses the two-auth-system mess (lessons from 2026-06-07 root-cause investigation)

The 2026-06-07 Test-Connection root-cause investigation (`EDWIN_SETUP_FEEDBACK_FOR_JAKE.md` §5.8) surfaced an architectural fact about Edwin's auth that's worth promoting from "incidental observation" to "design constraint" for the Hodos integration.

**Edwin's gateway today runs two parallel auth systems on the same port, gating different transports, with no unified credential:**

| System | Credential | Scope | Failure |
|---|---|---|---|
| Token auth (`gateway.auth.mode = "token"`) | 48-char hex shared secret in `~/.edwinpai/edwinpai.json` | WebSocket connection. Inside-route HTTP checks. | Wrong → WS rejects. HTTP routes return per-handler 401. |
| BSV auth (`gateway.bsvAuth.enabled`) | Per-request ECDSA signature from a BSV identity (BAP) via `x-bsv-*` headers | Whole HTTP pipeline (when `allowUnauthenticated=false`). | Missing/wrong → top-of-pipeline 401 `{"code":"UNAUTHENTICATED"}`. |

These systems were grafted on at different times. They gate **different transports** with **different mental models**. There is no single "is this caller authorized" answer — it depends on the path and the transport. The probe-vs-wizard root cause we hit is a direct consequence: the probe assumes one system is on, the wizard turns the other on, the silent assumption mismatch produces "Gateway not reachable" against a gateway that's running fine.

**Hodos integration design constraint:** the integrated product MUST present this as **one** auth flow, not two. Concretely:

1. **Wallet identity is the unified credential.** Hodos's BRC-100 wallet holds a BSV identity. That identity signs every gateway-bound request — HTTP or WS. Token auth becomes vestigial in the integrated path; the gateway sees a real BAP identity on every connection. No shared-secret token paste, ever. Edwin's `gateway.bsvAuth.enabled = true` becomes the de facto config for any Edwin instance Hodos brings up.

2. **Pairing replaces discovery-by-URL.** Today the desktop asks the user for a gateway URL + token; the probe then guesses what protocol/auth state the gateway is in by looking at HTTP status codes — a coupling that broke the moment the wizard's default config drifted. Hodos pairing: the wallet signs a pairing handshake, the gateway accepts the wallet's identity key as the desktop's identity going forward. No URL guessing, no probe URL convention, no broken-on-IPv6-localhost. The pairing handshake **is** the discovery.

3. **First-connect is one prompt.** "Trust the Edwin gateway at `<discovered-address>` (identity `02a3…b1`)? Allow it to receive chat traffic signed by your wallet." Approve once. Compare to today: pick a mode (Gateway Mode is a Windows dead-end), paste a URL (default is broken on Windows), paste a token (hand-grabbed from JSON5 in WSL), Test Connection (silently fails because of the auth-assumption mismatch §5.8 documents), Save & Connect (only works if Test Connection passes), navigate to Chat. The five-step sequence becomes one prompt.

4. **The wallet IS the keychain.** Today's gateway token lives plaintext at `~/.edwinpai/edwinpai.json` (`EDWIN_SETUP_FEEDBACK_FOR_JAKE.md` §4 cross-platform keychain note). In the Hodos path it doesn't exist as a separate secret at all — the wallet's identity key (DPAPI-protected on Windows, Keychain-protected on macOS, both via existing Hodos plumbing) is the only credential.

**Translation for the pitch:** "Edwin's first-connect on Windows has a real bug we hit on Wednesday — 30 minutes of friction, root cause is two auth systems with incompatible assumptions about each other. Hodos doesn't fix it — it makes it impossible by having one auth flow driven by the wallet." Concrete advantage slide, alongside the discovery-vs-pairing point in §4.1.0.

**For the Hodos build:** do NOT carry forward the gateway-token model in the integrated path. Even though Edwin currently produces a token and the standalone wizard expects one, the integration should never need it — Hodos's wallet identity (signed via BRC-103, the protocol Edwin's gateway is already built around) is sufficient for both HTTP and WS. If the integration ever requires us to read or paste a token, we've designed the seam wrong.

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

Open question for Jake + John on review: does Edwin's gateway already have a permission layer (e.g., per-channel approve / deny), and if so, does Hodos's UX surface it or stay out of its way?

---

## 6. Implementation sequencing

The work of getting the sidecar bundling (§2) live, in order:

1. **Edwin sidecar bundled with Hodos installer** — native binary on Linux + macOS; Windows via WSL (until the §4.1.1 Option C Windows-native gateway lands).
2. **Hodos chat panel wired to Edwin's WebSocket** — basic prompt → response round-trip on `ws://localhost:18789`.
3. **x402 payment path** — Edwin → Dolphin Milk via Hodos wallet → BEEF payment → response. PermissionEngine cap cascade gates the payment.
4. **Green-dot animation in the tab UI fires on payment** — the user's visible confirmation that the wallet handled it.
5. **PermissionEngine integration polish** — bundled per-domain prompts, scope grants, sub-permission cache wiring (the existing infrastructure from Phase 1.5/1.6 of Hodos's sprint work absorbs Edwin's payment events the same way it absorbs BRC-121 paid retries).

Out of initial bundling scope (target bundling work, or post-launch):

- Compiled-in shape (target bundling — see §2; incremental migration after sidecar is stable)
- Full channel-enablement UX (WhatsApp / Telegram / etc.) — chat + browser-context only at start
- Mode 2/3 API-key UX — Mode 1 (fully x402) is the default surface
- Production notarization / installer signing — added once builds are reproducible

---

## 7. Test plan

Three layers of test for the integration:

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
- **Edwin Desktop distribution model — is the polished binary part of the Founders Circle?** edwinpai.com (observed 2026-06-06) markets a Founders Circle membership ($20/mo, $144/yr, $499 lifetime) that includes "advanced deployment guidance" and "early-access materials." The Desktop source is in the (public) repo, but the homepage doesn't expose a binary download. For Hodos to bundle Edwin Desktop UX, what's the commercial path — license the desktop binary, build our own UI on the public gateway, or membership-tier the experience? This intersects directly with the BSL-1.1 in-house clearance Jake gave verbally.
- Bundling Edwin gateway as a sidecar in Hodos's installer — any licensing constraints? (BSL-1.1 status — verbal OK from 6-02 meeting, written confirmation still pending.)
- Does Edwin's gateway have a per-channel permission layer today, or do we add it as part of the integration?
- Is there a planned Rust-native rewrite of the gateway, or does it stay Node?
- **Windows-native gateway build — feasibility & priority?** (See §4.1.1.) Even partial Windows support — e.g., a non-WSL gateway with reduced channel set — would dramatically improve consumer-Windows install UX.

For John:
- Dolphin Milk's x402 endpoints: full price list + per-call cost model? (Critical for the §3 cost analysis.)
- Embedding endpoints — does Dolphin Milk plan to host these too, or stays chat / tools only?
- Rate-limiting and abuse-prevention at the Dolphin Milk side — does it interact with Hodos's PermissionEngine cap cascade, or is it independent?

For Matt:
- Settings UX for the three modes (§3) — does Hodos's Settings overlay already have a place for this, or do we add a new pane?
- Pitch deck — does the demo flow (green-dot moment) make it onto a slide for the Golden application?

For all three (high-leverage research items surfaced 2026-06-06):

- **Edwin's web_search uses Brave Search API — Hodos can eliminate the API entirely.** Jake's choice of Brave Search (over Google / Bing) aligns Edwin with the same privacy posture Hodos already adopted (Brave adblock lists, Brave-inspired fingerprint farbling per CLAUDE.md `FingerprintProtection.h`). Even better: in the integrated product, instead of Edwin paying for a Brave Search API key, Hodos can intercept Edwin's `web_search` tool calls and serve results from a real in-browser Brave Search page (which Hodos already renders with its own adblock + fingerprint posture). Zero API cost to Edwin, results are real browser-rendered pages (not API-summarized snippets), and the privacy posture is consistent end-to-end. **This is the same pattern as the Edwin-browser-subprocess replacement (next item) — Hodos's browser IS the agent's browser.** Worth one bullet in the pitch deck: "Your AI's web search rides on the same privacy-respecting browser engine you use, not a separate API tier."

- **Edwin's bundled browser subprocess — can Hodos replace it?** Edwin ships an `edwinpai browser` subcommand to "Manage EdwinPAI's dedicated browser (Chrome/Chromium)." This is almost certainly the `playwright-core` dep we saw in the build output — Edwin spawns a Chromium for agentic browsing. **Hodos IS a browser.** If Hodos can expose its CEF browsing surface to Edwin (via the existing IPC interception layer or a new agent-control API), we eliminate the second browser entirely. Benefits: one browser instead of two (resource use, install footprint, user mental model); every Edwin web action gated by Hodos's PermissionEngine (security upside); Edwin gets access to user's actual logged-in browser sessions (UX upside — Edwin can do things in the user's *real* gmail / drive / wherever, not a fresh Chrome with no state). Major question: what's Edwin's browser API surface — Playwright-style page commands, or something higher-level? Worth a dedicated investigation before sidecar bundling scope is finalized.

---

## Appendix: source notes

- 2026-06-02 Jake 1:1 meeting — verbal go-ahead on integration, BSL-1.1 status, AWS pitch
- 2026-06-02 prior Claude session — `EDWIN_SETUP_REPORT.md` + `WINDOWS_SETUP_GUIDE.md`
- 2026-06-03 setup-from-scratch attempt (alongside this doc being written)
- `ARCHITECTURE_TECHNICAL.md`, `EDWIN_VS_DOLPHIN_MILK_SECURITY.md`, `CANARY_A1_WALLET_COMPAT.md`, `CONVERGENCE_NARRATIVE.md` (existing dev-docs)
- `Edwin.txt` (2026-03-13 — older operations guide; useful for the qmd / Shad architecture)
