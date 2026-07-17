# Architecture Options — Both Ways (Edwin in Hodos)

> 📁 Part of the **Dolphin Milk + Edwin Integration** doc set — see `README.md`.
> **Created:** 2026-06-26 by a 6-agent architecture workflow, grounded in the code study (`EDWIN_NATIVE_PACKAGING_FINDINGS.md`), the existing design (`ARCHITECTURE_TECHNICAL.md`), the casual-user lessons (`LESSONS_LEARNED_EDWIN_INSTALL.md`), and the two browser-research rounds (`UX_EDWIN_ASSISTANT_COMMUNICATION.md` §5, `BROWSER_AI_IMPLEMENTATION_STUDY.md`).
> **This is a STUDY, not a decision.** Every option is presented with honest pros/cons for Matt to weigh — no winner is selected. The synthesis is below; full structured per-decision analyses are in the appendix.

---

# Hodos + Edwin: Architecture Options for Design Discussion

**Status:** Planning study — options and trade-offs only. No path is recommended here.
**Audience:** Matt Archbold, Jake (EdwinPAI), John (Dolphin Milk) — pre-design-conversation read.
**Date:** 2026-06-26

---

## Overview

Five open architecture decisions stand between the current state (Edwin running in WSL on Windows, casual users hitting the 9P bridge failure) and the product north star (install Hodos, AI just works). These decisions are not independent: choices in D1 (packaging) constrain D2 (which assistant), which constrains D3 (form factor), and D4 (search posture) and D5 (monetization) both depend on D2 and D3.

This document lays out each decision neutrally: what's at stake, what the options are with their mechanisms and honest trade-offs, and what questions must be answered — by Matt, by Jake, or by Jake and John together — before a path can be committed to.

The transport-binding discovery (the Hodos wallet can back Edwin's IdentityCore via the same pattern Jake already uses in desktop-binding.ts) significantly shifts the D1/D2 calculus relative to earlier planning docs. That finding is threaded through each relevant section.

Sources are cited by local file path (all under `C:\Users\archb\Hodos-Browser\development-docs\Dolphin Milk + Edwin Integration\`) and by URL. Access date: 2026-06-26 throughout.

---

## Decision Dependency Map

```
D2 — Which assistant runtime
 ├─ Constrains D1: Edwin-only or three-party requires Node bundling;
 │   Dolphin Milk-only can skip Node entirely.
 ├─ Constrains D3: The assistant chosen determines what UI is available
 │   to render in the panel.
 ├─ Constrains D4: Edwin handles NL queries differently than Dolphin Milk;
 │   the search posture design changes per assistant.
 └─ Constrains D5: The x402/envelope payment path is wired through whichever
     agent runs; micropayment feasibility depends on D2's outcome.

D1 — Packaging (gates D2 viability)
 ├─ Option A (Jake native companions) and Option B (Hodos wallet transport)
 │   determine whether v1 is blocked on Jake's Windows build calendar.
 └─ The transport-binding insight changes D1's Jake-dependency profile:
     Option B now requires only a Jake PR review, not a Jake binary build.

D3 — Form factor (depends on D1 + D2)
 ├─ Panel options C and A require Edwin's sidecar to serve or accept UI queries;
 │   must be settled after D2.
 └─ IPC bridge design (how the panel calls the wallet's PermissionEngine)
     depends on which panel option is chosen.

D4 — Search posture (depends on D2 + D3)
 ├─ Options A and B require Edwin to accept structured query input — new IPC;
     Jake must build or approve this API surface.
 └─ Per-cite micropayment trigger architecture (D5) depends on whether D4
     produces an explicit citation event (blend) or inferred synthesis (AI-first).

D5 — Monetization (depends on D2 + D4)
 ├─ Micropayment-only requires x402/BSV per-query and per-cite triggers;
 │   these only exist in the payment chain if D2 and D4 produce them.
 └─ Subscription billing is less dependent on D2/D4 choices but requires
     knowing which party's treasury split formula applies.
```

**Which to settle first.** D2 has the broadest downstream impact: nearly every other decision changes depending on whether the runtime is Edwin-only, Dolphin Milk-only, or both. D1 is the enabling gate for D2's Edwin path. D3, D4, and D5 can be scoped once D2 and D1 are directionally aligned.

---

## D1 — Packaging and Native Runtime

**Why it matters.** The WSL/9P bridge is the root cause of every "Edwin feels broken on Windows" user experience (LESSONS_LEARNED_EDWIN_INSTALL.md §1). Fixing it means running Edwin's Node gateway natively on Windows and macOS, managed by the Hodos C++ shell as a localhost sidecar. This decision determines: (a) whether v1 shipping is blocked on Jake building Windows native companions, (b) which party owns the cryptographic signing implementation, and (c) when x402 BSV micropayments can be envelope-gated end-to-end. Options C and D are structurally not viable for v1 (see below); the real fork is A vs. B.

### Options

#### Option A — Bundled Node runtime + Jake's per-platform native companions

| Field | Detail |
|---|---|
| **Mechanism** | Hodos bundles pinned Node.js v22.12 LTS + Edwin's dist/ (18 MB) + pruned node_modules inside its installer. Jake builds and publishes per-platform native companions for @edwinpai/identity-core and shad-core (win32-x64, win32-arm64, darwin-arm64, darwin-x64). Hodos C++ shell spawns `node dist/index.js` on a localhost port (:18789), health-checked and restarted on crash, killed on browser exit — same subprocess pattern as the wallet (:31301) and adblock engine (:31302). EDWINPAI_IDENTITY_CORE_MODULE env override points Edwin at the bundled companion path. |
| **Pros** | Full Edwin fidelity — Jake's own tested crypto impl; no semantic divergence risk. Clean responsibility split: Hodos packages, Jake owns crypto. Jake's build pipeline for per-platform companions already exists (prepare-platform-packages scripts). BSL licensing respected — binaries distributed via Jake's authorized publish. Established pattern (VS Code, many Node tools ship bundled runtimes). |
| **Cons** | Hard timeline block on Jake: v1 cannot ship until Jake publishes win32-x64 + darwin native companions for identity-core and shad-core. Three-way release coordination (Hodos + Edwin + companion version). win32-arm64 for sharp is uncertain (fewer than 30 downloads/month, flagged for possible removal — sharp-libvips issue #238). BSL licensing may require per-user click-through in the Hodos installer. Pruned node_modules must be re-verified on every Edwin version bump. |
| **Dependencies** | Jake must build and publish per-platform native companions for identity-core and shad-core on Windows (x64 minimum, arm64 aspirational) and macOS. Jake must keep EDWINPAI_IDENTITY_CORE_MODULE override supported. Hodos needs a Windows CI runner. |
| **Best-when** | Jake is already planning Windows Edwin native installs for his own distribution and companion builds are coming regardless of Hodos's timeline. Also the right long-term steady state once that infrastructure exists. |
| **Effort** | Hodos: Medium (bundling toolchain, subprocess management, version-pinning, installer). Jake dependency: High — blocks v1 shipping. Risk: timeline slippage if Jake's native CI is not imminent. |

#### Option B — Bundled Node sidecar + Hodos Rust wallet transport (no Jake native companion required for signing)

| Field | Detail |
|---|---|
| **Mechanism** | Same Node bundling layer as Option A. Key difference: instead of loading Jake's native identity-core companion, Edwin is initialized with `createNodeIdentityCoreBinding(transport)` wired to the Hodos wallet (:31301). The wallet gains 4 new HTTP endpoints implementing NodeIdentityCoreTransport (signHttpRequest, signEnvelope, verifyEnvelope, getPublicKey) — mapping onto its existing secp256k1 + DPAPI/Keychain + BRC-42 key stack. Precedent: desktop-binding.ts in Edwin's own codebase already proves a Rust backend can implement this pattern. EDWINPAI_IDENTITY_CORE_MODULE or a startup hook loads the transport binding. shad-core: either disabled in v1 (B1) or Jake publishes shad-core Windows builds only (B2 — narrower ask than full Option A). |
| **Pros** | Eliminates the single biggest Option A blocker: no Jake Windows native companions needed for signing in v1. Architecturally coherent with ARCHITECTURE_TECHNICAL.md design: wallet is already the specified envelope-issuance authority. desktop-binding.ts proves Jake already ships and uses this transport pattern. Two-way instead of three-way release coordination. Faster v1/demo timeline. Transport interface surface is small and stable (four methods). Migration path to Option A exists later without Edwin-side changes. |
| **Cons** | Hodos must faithfully implement envelope semantics (nonce generation, TTL, payload hashing, secp256k1 ECDSA format) or downstream Edwin code and x402 endpoints will silently reject requests — security-relevant correctness requirement. shad-core/recall still unresolved without a transport binding path for the vector store. Transport path must be explicitly blessed by Jake as supported — if unstable, a future Edwin refactor could break Hodos's implementation. Edwin's test suite tests identity-core with Jake's native companion; the transport path has untested edge cases. Round-trip latency (Edwin → wallet HTTP → back) on every signed request could add perceptible overhead. |
| **Dependencies** | Jake's explicit confirmation that createNodeIdentityCoreBinding / NodeIdentityCoreTransport is a supported, stable integration path. Hodos wallet must implement the 4-method transport interface in Rust (maps to existing capabilities). shad-core story: v1 without recall, or a narrower Jake ask for shad-core Windows builds only. |
| **Best-when** | Jake has not yet built Windows native companions and there is no near-term timeline for them; or shipping a v1 demo quickly takes priority; or as a stepping-stone to Option A once Jake's companion builds are available; or Jake explicitly prefers this integration pattern. |
| **Effort** | Hodos: Medium-High (implement NodeIdentityCoreTransport in Rust, validate envelope semantics, write integration tests). Jake dependency: Low-Medium (API stability confirmation, not new native builds). Risk: envelope fidelity risk is security-relevant and must be rigorously tested before shipping to real users. |

#### Option C — Single-binary compile (Node SEA / yao-pkg / bun build --compile)

| Field | Detail |
|---|---|
| **Mechanism** | Attempt to pack Edwin's ESM gateway + deps into a single self-contained executable. Node SEA (stable since Node 22): requires CJS entry-point transpilation; native .node files cannot be embedded and must be shipped alongside. yao-pkg v6: same CJS constraint; same native addon limitation. bun build --compile: supports ESM natively but uses JavaScriptCore not V8 — node-pty ConPTY behavior untested and may fail silently. In all cases, Edwin's dynamic skill loading (74 skills discovered at runtime from filesystem paths) is incompatible with static bundling. |
| **Pros** | Appears cleaner on the surface (one executable). Slight tamper resistance. Avoids a visible node.exe in the install directory. |
| **Cons** | The "single binary" claim is false for Edwin's profile: native .node files (node-pty, sharp, sqlite-vec, napi-rs/canvas, matrix-sdk-crypto) cannot be embedded — the user gets a binary plus a folder of DLLs. ESM entry point requires additional CJS transpilation. Dynamic skill loading is architecturally incompatible with static bundling. bun runtime incompatibilities with node-pty/ConPTY are untested. Does not solve the identity-core native companion problem. Significantly harder to debug in production. |
| **Dependencies** | Same Jake native companion dependency as Option A for signing. Additional transpilation pipeline or verified bun compatibility. Skill loading must be redesigned. |
| **Best-when** | Edwin's architecture changes to static plugin loading, drops native addons entirely, and Jake provides Windows companions — none of which are true as of June 2026. |
| **Effort** | High effort, high risk. Produces a folder distribution anyway with higher build complexity than A or B. Effectively not viable for Edwin's current architecture. |

#### Option D — Ship Edwin as a separately installed app; Hodos connects over localhost

| Field | Detail |
|---|---|
| **Mechanism** | Hodos does not bundle Edwin. At startup, Hodos checks whether Edwin is running on its default port. If absent, shows a first-run panel linking to Jake's own distribution. Integration is purely API-level. Edwin is installed, updated, and managed entirely by Jake's own installer — which does not currently exist for Windows (install.sh is curl-pipe-bash, Unix-only). |
| **Pros** | Zero Hodos build dependency on Edwin internals. Independent update paths. No BSL licensing navigation. Lowest Hodos engineering effort. |
| **Cons** | Directly fails the casual-user north star: "install Hodos and AI just works" is impossible if the user must separately discover, download, and install Edwin. Edwin has no Windows native installer today. No control over Edwin configuration from Hodos. Version compatibility is user-managed. PermissionEngine envelope integration becomes very hard if Hodos does not own the Edwin process. No practical x402 monetization integration. |
| **Dependencies** | Jake must ship a native Windows installer for Edwin (does not yet exist). Edwin's HTTP gateway API must remain stable across versions Hodos does not control. |
| **Best-when** | Early prototype for power users who already run Edwin natively, or as a future "bring your own Edwin instance" advanced setting alongside a bundled default. Not viable as the v1 casual-user experience. |
| **Effort** | Low Hodos effort; fails the product north star entirely for casual users. |

### Key Trade-offs

The real fork is A vs. B. C is ruled out for Edwin's current architecture (still produces a folder distribution, higher build risk, no benefit). D is ruled out as a primary v1 path (fails the casual-user north star).

**A vs. B is a single dimension dressed up as multiple questions: who owns the signing implementation, and who gates the timeline?** In A, Jake's closed native binary is the signing authority — exact fidelity, no semantic risk, but v1 ships only after Jake builds Windows companions. In B, the Hodos Rust wallet is the signing authority via the transport binding — unblocks shipping immediately, but Hodos must correctly implement envelope semantics (a security-relevant task) and needs Jake's explicit blessing of the transport path as supported.

The natural migration path is B-then-A: ship v1 with the wallet transport (no Jake native dependency), then replace the transport backing with Jake's native companion when he publishes Windows builds. Edwin's code does not need to change for this migration.

shad-core/recall is a partial exception even in Option B: the transport binding solves identity (signing/envelopes) but not the vector store. Options: (1) ship v1 without recall, (2) ask Jake only for shad-core Windows builds (narrower than full Option A). This must be decided early because it affects v1 feature scope.

### Open Questions for Matt / Jake

- **Jake:** Has Windows native companion development for identity-core and shad-core already started? What is a realistic timeline? This single answer determines whether Option A or Option B must go first.
- **Jake:** Do you explicitly bless createNodeIdentityCoreBinding / NodeIdentityCoreTransport as a stable, supported integration surface for Hodos? Or is the transport binding internal and likely to change?
- **Jake:** Does shad-core have a transport binding path analogous to identity-core's desktop-binding.ts? If not: is the plan for v1 Hodos to run without recall, or does Jake plan to publish shad-core Windows builds?
- **Jake:** What is the stable Edwin API surface for this integration — current beta.8/beta.9, or post-refactor main (qmd backend, pruned extensions)?
- **Matt:** Is Edwin-without-recall acceptable for v1 (shad-core disabled, just assistant + signing)? This unlocks Option B without any Jake native builds.
- **Matt:** Is win32-arm64 (Qualcomm Snapdragon laptops) a day-one requirement, or deferred? sharp's win32-arm64 prebuild support is marginal and may not exist (sharp-libvips issue #238).
- **Matt/Jake:** Can Hodos bundle and redistribute Jake's native companion binaries in the installer, or does each user need a separate BSL license click-through? This affects installer UX and may favor Option B for initial ship.

*Sources: EDWIN_NATIVE_PACKAGING_FINDINGS.md; LESSONS_LEARNED_EDWIN_INSTALL.md; ARCHITECTURE_TECHNICAL.md; nodejs.org/api/single-executable-applications.html; github.com/nodejs/help/issues/5129; github.com/lovell/sharp-libvips/issues/238; npmjs.com/package/@lydell/node-pty; alexgarcia.xyz/sqlite-vec/installation.html*

---

## D2 — Which Runtime Is the In-Browser Assistant

**Why it matters.** This sets the subprocess count, the upstream partnership surface, the install footprint, the day-one assistant capability, and the x402 monetization architecture. It gates: (1) how soon a working PoC can ship without Jake's closed native builds, (2) whether the casual user gets EdwinPAI's full skill/recall experience or a thinner x402-native agent, (3) whether Hodos has one or two upstream partners to coordinate, and (4) which parts of ARCHITECTURE_TECHNICAL.md's three-party design are realized vs. deferred. The transport-binding discovery shifts Option A's viability compared to what earlier docs assumed.

### Options

#### Option A — Edwin-only

| Field | Detail |
|---|---|
| **Mechanism** | Edwin runs as a fourth managed subprocess (C++ shell spawns bundled Node runtime → Edwin dist/index.js, :8090, same health-check/restart pattern as wallet and adblock). Edwin is the sole AI assistant: conversational turns, 74+ hot-reloadable skills, qmd/shad recall, LLM routing, BRC-103 signed-request gateway. IdentityCore is backed by the Hodos Rust wallet transport (D1 Option B) rather than Jake's native companion — four transport methods wired through the existing secp256k1/DPAPI/BRC-42 stack. x402 payments: Edwin calls wallet/createAction → PermissionEngine → wallet issues envelope + signs BSV transaction → Edwin carries x402 payment header. No Dolphin Milk subprocess. |
| **Pros** | Single agent subprocess. Matt runs Edwin daily — proven experience with known capability. 74+ skills available immediately; hot-reload means Jake ships new skills without Hodos rebuilds. qmd/shad recall designed for the browser-assistant use case. Transport-binding removes Jake's Win/Mac native core as hard identity blocker. Multi-channel inbox (WhatsApp, Signal, Telegram, Discord) differentiator. Single upstream relationship. EDWINPAI_IDENTITY_CORE_MODULE override is testable in isolation. |
| **Cons** | Node.js sidecar (18MB dist + pruned node_modules + bundled Node runtime) carries meaningfully higher idle memory and startup time than an all-Rust lineup. Single-binary packaging is not viable for Edwin's profile (D1 Option C analysis). x402 payment handling is Edwin making standard HTTP calls with a BSV payment header — functional but not purpose-built as Dolphin Milk's BRC-29 client. shad-core (recall vector store) is not solved by the transport-binding path — still depends on Jake's native build or a separate Hodos Rust implementation. Edwin's narrate-then-stop agent loop may need upstream PRs for browser-assistant UX tuning. BRC-18 on-chain proof of every agent decision (Dolphin Milk feature) is absent. |
| **Dependencies** | Jake: bless createNodeIdentityCoreBinding(transport) path in open-source TypeScript. shad-core: separate Jake decision (native build or Hodos defers recall). Hodos: Node runtime bundling for Win/Mac, subprocess wrapper, wallet transport implementation (~2-3 weeks Rust). No John/Dolphin Milk dependency. |
| **Best-when** | Jake confirms the transport-binding path quickly; Matt prioritizes full EdwinPAI experience (skills, recall, multi-channel inbox) over x402 protocol fidelity; v1 proposition is "a real full-featured AI assistant in your browser"; timeline is tighter and fewer upstream parties means fewer coordination risks. |
| **Effort** | Medium. Hodos: Node runtime bundling, subprocess wrapper, Rust wallet transport. Jake: reviewing and blessing transport-binding PR in TypeScript — this is the critical-path item. Risk: if Jake does not bless quickly, fallback is waiting on Jake's Win/Mac native builds for identity-core and shad-core. |

#### Option B — Dolphin Milk-only

| Field | Detail |
|---|---|
| **Mechanism** | Dolphin Milk (Rust binary, :8080) is the sole agent subprocess, managed by Hodos C++ shell as specified in ARCHITECTURE_TECHNICAL.md. Edwin's contribution is spec reference only: SignedEnvelope / SignEnvelopeInput / VerifyEnvelopeOptions types from Edwin's open-source types.ts inform the Hodos Rust wallet's envelope implementation. No Edwin Node sidecar at runtime. Dolphin Milk makes BRC-29 x402 calls → Hodos PermissionEngine → wallet issues envelope + signs BSV transaction → Dolphin Milk carries x402 payment header. The assistant UI is Dolphin Milk's /ui/ surface or a Hodos-native overlay calling Dolphin Milk's HTTP API. |
| **Pros** | All-Rust subprocess architecture: wallet + Dolphin Milk + adblock — homogeneous, fast startup, low idle memory. Dolphin Milk's x402 plumbing is purpose-built (BRC-29 payment construction, x-bsv-payment header, x402agency.com marketplace routing). Zero Node runtime dependency. Zero Jake binary dependency at runtime (Jake's role is spec author, not binary publisher). BRC-18 on-chain proof of every agent decision is built in. Smallest installer footprint. Apache 2.0 license on Dolphin Milk — bundling is free; coordination with John is version-cadence only. If v1 pitch is "the browser that pays for AI natively via micropayments," Dolphin Milk delivers the x402 mechanism with least ceremony. |
| **Cons** | Dolphin Milk is an x402 agentic task runner, not a general conversational AI with a skill ecosystem — casual users asking "summarize this page," "help me write this email," or "what did I read last week?" may get a thinner experience. No Edwin skill ecosystem (74+ skills): every browser-relevant capability would need to be built in Dolphin Milk's framework. No qmd/shad recall. No multi-channel inbox. Matt does not run Dolphin Milk as his daily assistant; the EdwinPAI experience he has battle-tested is absent. Edwin's BRC-103 signed-request gateway security differentiator is absent. If Dolphin Milk's assistant UX is immature for general conversational tasks, the casual user sees a bare-bones experience regardless of x402 correctness — this needs empirical validation before committing. |
| **Dependencies** | John: Win/Mac Dolphin Milk binary builds on a coordinated version cadence. Hodos: wallet Rust envelope implementation (referencing Edwin types.ts as spec, 2-4 weeks), subprocess wrapper, three Canary A1 wallet-shim patches already planned. No Jake runtime dependency. Key validation dependency: someone needs to run Dolphin Milk as an actual daily assistant to confirm its conversational maturity before v1 — analogous to the Edwin install session in LESSONS_LEARNED. |
| **Best-when** | v1 is primarily the x402 micropayment story and the assistant capability needed is agentic x402 tasks rather than general conversational AI with recall and skills; Jake's timeline is uncertain; Hodos strategically prefers homogeneous all-Rust subprocess lineup; Matt is explicitly OK shipping without the EdwinPAI skill/recall experience in v1; Dolphin Milk's assistant UX is validated as sufficient for casual users. |
| **Effort** | Medium-low (for Hodos). Hardest Hodos work: wallet envelope implementation in Rust (2-4 weeks). Subprocess wrapper follows existing wallet/adblock pattern. No Node runtime packaging. Risk: Dolphin Milk's maturity as a conversational assistant is the largest unknown — an empirical question requiring a hands-on session. |

#### Option C — Both (three-party): Edwin as assistant layer + Dolphin Milk as x402 runtime + Hodos wallet as vault

| Field | Detail |
|---|---|
| **Mechanism** | ARCHITECTURE_TECHNICAL.md's designed three-party system with the transport-binding discovery applied. Three managed subprocesses: Hodos wallet (:31301, key custody + PermissionEngine + envelope gate + IdentityCore transport for Edwin), Dolphin Milk (:8080, x402 agent runtime, BRC-29 payments, BRC-18 on-chain proofs), Edwin (:8090, Node sidecar, conversational assistant, skills, recall, BRC-103 gateway). Two sub-variants: C1 = Edwin is the user-facing front door (delegates x402 agentic tasks to Dolphin Milk); C2 = Dolphin Milk is primary agent orchestrator (calls Edwin's skills/conversation API for NL tasks). Both variants: PermissionEngine governs ALL agent requests from both Edwin and Dolphin Milk. |
| **Pros** | Maximum capability per party's strength: x402 protocol handling (Dolphin Milk), full assistant skills/recall/multi-channel (Edwin), key custody and trust gate (Hodos wallet). BRC-18 on-chain proof + Edwin envelope spec + Edwin skills = most comprehensive audit trail and capability combination. Full three-party trust model from ARCHITECTURE_TECHNICAL.md is realized. Transport-binding collapses the vault question: both agents trust the same Hodos wallet. Two upstream partners provides redundancy. Monetization fee-split architecture is most complete. Product narrative is most defensible long-term. |
| **Cons** | Two agent subprocesses from two upstream partners — coordination overhead doubled. Edwin-to-Dolphin Milk handoff protocol (IPC/HTTP contract) does not exist yet — designing it is non-trivial engineering not in current docs. C1 vs. C2 orchestration must be settled before any code is written; requires Jake and John agreement and may change both projects. Capability overlap risk: both subprocesses can initiate x402 wallet calls independently. Memory footprint: Edwin Node sidecar (~200-400MB with skills loaded) + Dolphin Milk Rust + Hodos wallet = three subprocesses on a typical 8GB laptop. The casual user must see one seamless assistant — building that seamless presentation layer is additional engineering. v1 scope risk: three-party complexity may not fit a tight pitch window. Three-party protocol coordination is operationally harder and grows as a maintenance burden. |
| **Dependencies** | Jake: bless transport-binding OR publish Win/Mac native builds for identity-core AND shad-core, AND agree on Edwin-Dolphin Milk handoff protocol. John: Win/Mac Dolphin Milk binary builds AND agree on handoff protocol. Hodos: two subprocess wrappers, wallet Rust transport + envelope implementation, PermissionEngine extension, handoff protocol design. Jake + John + Matt must align on C1 vs. C2 before any integration code is written. |
| **Best-when** | Both Jake and John are confirmed partners with explicit Win/Mac build commitments before v1 begins; product pitch explicitly features both EdwinPAI and Dolphin Milk as named components; Hodos has sufficient engineering capacity for three-party integration and the handoff protocol; monetization fee-split is designed from day one; timeline for v1 is not tightly constrained (3-6 months vs. 1-2 months). |
| **Effort** | High. Two subprocess wrappers, wallet Rust transport + envelope implementation, PermissionEngine C++ extension, Edwin-Dolphin Milk handoff protocol design and implementation. Jake + John buy-in on an undefined protocol is the primary risk; if C1/C2 orchestration is not settled early, integration rework is likely. |

### Key Trade-offs

**Assistant depth vs. x402 protocol fidelity.** Edwin has the assistant depth (74+ skills, recall, multi-channel inbox, BRC-103 gateway, the experience Matt runs daily). Dolphin Milk has the x402 protocol fidelity (BRC-29, x-bsv-payment header, BRC-18 audit trail). You cannot fully get both without Option C's complexity. Options A and B each sacrifice one dimension.

**The transport-binding discovery shifts Option A's viability.** Earlier docs assumed Option A required Jake's Win/Mac native cores as a hard dependency. The code study found createNodeIdentityCoreBinding(transport) lets any async object back IdentityCore, and desktop-binding.ts already proves a Rust backend can do this. Option A's Jake dependency is now a PR against open-source TypeScript — a materially different negotiation than requesting a closed-source binary build. However, shad-core (recall vector store) is not solved by this path; it remains Jake's binary or a separate Hodos Rust implementation.

**Process homogeneity vs. capability breadth.** All-Rust (Option B) gives a lean, consistent subprocess lineup matching Hodos's existing wallet/adblock pattern. Adding Edwin (Options A, C) introduces a Node runtime, complex installer packaging, and meaningfully higher idle memory. On a typical 8GB Windows laptop this difference is real.

**Upstream dependency count vs. feature set.** Option B minimizes Jake dependency (spec reference only). Option A has one active upstream (Jake). Option C has two active upstreams (Jake + John) plus a new integration contract between them. More partners = more capability = more coordination risk.

**Note on the x402 ecosystem.** The x402 Foundation (Coinbase/Anthropic/AWS/Google/Vercel) implements EVM stablecoins (USDC on Base/Polygon/Solana). Dolphin Milk implements a BSV-flavored x402 using BRC-29. This is architecturally correct for the Hodos/BSV stack but means the x402 marketplace is effectively self-contained within the BSV/Dolphin Milk ecosystem rather than interoperable with the broader Coinbase x402 Foundation. This does not break any option but is relevant context for the monetization narrative.

### Open Questions for Matt / Jake / John

- **Jake:** Will you bless createNodeIdentityCoreBinding(transport) where the Hodos Rust wallet backs IdentityCore — yes or no? This is the pivotal question for Option A. If yes, Win/Mac native identity-core build is no longer on the critical path.
- **Jake:** What is shad-core's Win/Mac native build status? Transport-binding solves identity but not recall. Does shad-core have a Windows/macOS native package in the pipeline, or should Hodos plan to defer recall or implement it independently in Rust?
- **Jake + John:** In Option C, who is the primary agent orchestrator — Edwin (C1) or Dolphin Milk (C2)? This affects every integration seam and requires both parties' agreement.
- **John:** Is Dolphin Milk's assistant UX mature enough for general conversational tasks (summarize this page, help me write this email, what did I read last week)? Option B needs an equivalent hands-on session to what LESSONS_LEARNED documents for Edwin before Matt can commit to it as the primary user-facing assistant.
- **Matt:** What is the v1 minimum assistant capability the casual user must experience? "Full EdwinPAI with skills and recall" takes Option B off the table. "Agentic x402 tasks with basic Q&A" makes Option B viable.
- **Matt:** What is the memory budget constraint for the AI assistant subprocess on target hardware (e.g., 8GB RAM Windows 11 laptop)? Edwin Node sidecar with skills loaded vs. Dolphin Milk Rust binary are meaningfully different.
- **Matt:** For v1 PoC/pitch scope, is there a hard deadline (e.g., the AWS competition window from INTEGRATION_RESEARCH_KICKOFF.md)? Option C's complexity may not fit a tight deadline.

*Sources: ARCHITECTURE_TECHNICAL.md; EDWIN_NATIVE_PACKAGING_FINDINGS.md; LESSONS_LEARNED_EDWIN_INSTALL.md; BROWSER_AI_IMPLEMENTATION_STUDY.md §H; UX_EDWIN_ASSISTANT_COMMUNICATION.md §5; blockeden.xyz/blog/2025/10/26/x402-protocol... ; allium.so/blog/x402-explained...*

---

## D3 — v1 UX Form Factor

**Why it matters.** This sets every casual user's first impression of Hodos's AI — discoverability, screen comfort, idle RAM hit, and how tightly Edwin's own UI versus a Hodos-native interface is exposed. It gates D1 (bundling) and D2 (wallet transport): a side-panel approach requires Edwin to expose a localhost UI route; a full-page approach sidesteps that. Getting this wrong means either shipping too late (Options D, B with full multi-surface polish) or shipping something casual users cannot find (Option E alone, or a panel with Edwin's unreformed desktop UI). The monetization UX (cost transparency, budget caps, x402 micro-payment consent) also lives here — the form factor determines whether Hodos can intercept and surface it clearly, or whether it is buried inside Edwin's UI. The UX doc explicitly designates Option C (localhost SPA in panel) as "the likely v1 surface" and Options B and E as "later, not iteration 1."

### Options

#### Option A — Persistent native sidebar panel (Hodos-built chat UI, Edwin as HTTP backend)

| Field | Detail |
|---|---|
| **Mechanism** | Hodos C++ shell creates a dedicated CEF panel docked to the right of the browser window — same overlay mechanism as the wallet panel. Hodos builds its own React or CEF-native chat UI inside this panel; the UI talks to Edwin via SSE stream and REST at localhost. Edwin is a backend only. Panel persists across tab navigation and collapses/pins. Edwin sidecar starts lazily on first panel invocation. |
| **Pros** | Industry-consensus form factor: Comet, Chrome Gemini sidebar, Edge Copilot (original), Brave Leo, Opera Aria all validated this pattern. Persistent context across tab switches. Native OS panel cannot be blocked by ad-blockers or z-index conflicts. Hodos fully controls cost/payment transparency UX (LESSONS_LEARNED §3 requirement trivially met). SSE streaming from Edwin's localhost port is standard HTTP. Lazy-start mitigates idle RAM concern. |
| **Cons** | Hodos must design and ship a full chat UI — more frontend work, risk of shipping a weaker UX than Edwin already has. Panel consumes permanent screen real estate; Edge's May 2026 lesson (retiring Copilot Mode → ~18% idle RAM reduction) is real data. Diverges from Edwin's roadmap UI — Jake updates Edwin's interface; Hodos tracks API contract changes rather than UI changes. No fallback to Edwin's existing UI — if Hodos's chat UI is incomplete at launch, the experience is entirely Hodos's to own. |
| **Dependencies** | Hodos frontend capacity to build and maintain a chat UI. Edwin's HTTP streaming API (already exists). Hodos's existing CEF overlay system (already exists for wallet panel). Lazy-start subprocess management (already planned in ARCHITECTURE_TECHNICAL.md). |
| **Best-when** | Hodos has frontend bandwidth to build and maintain a native chat UI before v1 ships; the team wants full control over cost transparency, BSV payment consent UX, and Edwin's settings exposure; Edwin's existing UI is judged inadequate for the casual-user bar. |
| **Effort** | Medium-high. CEF overlay mechanics are already in Hodos; the new work is the chat UI itself. Risk: UI quality is Hodos's responsibility; launching with a thinner UI than Edwin's fuller interface is a step backward in feature richness. |

#### Option B — Ambient injection (omnibox NL handler + right-click + keyboard shortcut, no permanent panel)

| Field | Detail |
|---|---|
| **Mechanism** | No persistent panel. Edwin surfaces through three ambient entry points: (1) address-bar heuristic router detects NL input and routes to Edwin; (2) right-click context menu on selected text ("Ask Edwin about this"); (3) global keyboard shortcut opens a transient floating panel, dismissed after the interaction. Edwin sidecar starts on first invocation. |
| **Pros** | Lowest idle RAM — no persistent panel, no renderer overhead when Edwin is not active. Edge May 2026 real-world lesson directly supports this: dissolving the persistent Copilot sidebar saved ~18% idle RAM. Normalizes Edwin as a browser-native capability rather than a separate app users consciously open. Lower visual noise for privacy-conscious users. |
| **Cons** | Hardest to discover for new users — LESSONS_LEARNED §5 identifies discoverability as the single most important casual-user requirement. No persistent conversation context across tab navigation. Omnibox ML router requires at minimum a heuristic intent classifier — non-trivial CEF extension work. Multi-surface implementation (three distinct entry points) is highest engineering complexity for the coverage achieved. Edge adopted this after years with a persistent sidebar — Edge users already knew Copilot existed; Hodos v1 users have zero prior Edwin awareness. |
| **Dependencies** | Omnibox extension API in CEF (requires testing for adequate hooks). Right-click context menu extension (straightforward in CEF). Shortcut manager (already exists in Hodos). Edwin HTTP API for transient panel. |
| **Best-when** | v2 or later, after v1 established user awareness of Edwin. Or as a complement layered on top of Options A, C, or D — the ambient entry points add significant value as supplements, not as the sole discovery mechanism. |
| **Effort** | High for standalone deployment. Medium-low if implemented as a supplement to A, C, or D. Risk: if shipped alone, casual-user discoverability fails. |

#### Option C — Localhost-hosted SPA in CEF side panel (render Edwin's existing UI; UX doc's "likely v1")

| Field | Detail |
|---|---|
| **Mechanism** | Edwin's Node/native sidecar serves its existing React SPA at http://localhost:\<port\>/sidecar (or a route Jake exposes). Hodos loads this URL inside a dedicated CEF panel frame — isolated from the user's main browsing profile. The panel container is a thin shell; all UI is Edwin's. A "Full page" expand button can optionally open Edwin in a Hodos tab, giving both panel convenience and desktop richness. Comet (perplexity.ai/sidecar in Chrome's Side Panel API) is the architecture precedent, served from localhost instead of the cloud. |
| **Pros** | Fastest time-to-ship: Edwin's existing UI is reused as-is. No Hodos frontend team needs to build a chat UI. UX doc §1 explicitly calls this "the likely v1 surface." Edwin's UI stays in sync with Jake's updates without Hodos rebuilding. Rapid iteration: UI changes (Jake pushing Edwin updates) do not require Hodos browser rebuilds. Full-page-localhost variant is essentially zero-effort and gives the rich desktop experience. |
| **Cons** | Edwin's existing UI was designed as a desktop app — may not be optimized for a narrow side-panel viewport; Jake may need to expose a sidecar-specific layout. Embedded browser frame for the panel carries an extra CEF renderer process overhead. Same-origin considerations: cross-panel communication with Hodos C++ (for wallet payment consent, PermissionEngine notifications) requires an explicit IPC bridge (hodos:// custom scheme or WebSocket). Hodos has limited control over Edwin's UX inside the panel — the cheap-safe-defaults, cost transparency, and budget cap requirements (LESSONS_LEARNED §3) must be satisfied by Edwin's UI, not Hodos's. If Edwin's UI still shows the LESSONS_LEARNED §4 gaps (empty Sources tab, jargon-heavy Skills panel showing 0, misleading status surfaces), the casual-user experience fails immediately under the Hodos brand. UI branding is Edwin's, not Hodos's. |
| **Dependencies** | Jake must expose a sidecar-ready UI route (possibly /sidecar or a responsive layout flag). Hodos builds a thin panel frame container (straightforward CEF work). IPC bridge for payment/permission events from Edwin's localhost UI to Hodos's C++ shell. Edwin's UI must meet the casual-user quality bar from LESSONS_LEARNED §4-5 before Hodos ships. |
| **Best-when** | Speed to v1 is the top priority; Jake's UI is trusted to be good enough (or will be fixed before Hodos ships); Hodos wants to minimize frontend investment and keep coupling at the API layer. Also the right choice for an early demo or PoC — it gets something working immediately. |
| **Effort** | Low-medium for Hodos. The panel container is thin CEF work. The real effort is on Jake's side (sidecar UI route, responsive layout, fixing LESSONS_LEARNED UX gaps). Risk: Hodos ships a casual-user experience that reflects Edwin's current setup/onboarding gaps, which are not fully in Hodos's control. |

#### Option D — CEF WebUI first-party panel (Brave Leo pattern, deepest Chromium integration)

| Field | Detail |
|---|---|
| **Mechanism** | Edwin's panel is implemented as a Chromium WebUI — an internal page (like chrome://settings or brave://leo) rendered in a privileged first-party context. Requires custom CEF/Chromium source patches to add the WebUI resource handler, controller, and bindings. Brave Leo uses this architecture. |
| **Pros** | First-party trust level: cannot be blocked by content blockers or interfered with by web pages. Stable Chromium Side Panel API behavior. Hodos fully owns the UI and communication layer. Tightest possible integration with PermissionEngine and wallet IPC. |
| **Cons** | WebUI registration requires custom Chromium source patches — not a standard CEF API. Tight coupling to Chromium version: WebUI APIs can change between Chromium releases. Slowest iteration speed: UI changes require a browser rebuild cycle. Highest engineering cost of all five options. Brave has dedicated Chromium engineers; Hodos as a small shop may underestimate the maintenance burden. |
| **Dependencies** | Deep CEF/Chromium source expertise. Custom WebUI resource handler, controller, and bindings. Significant C++ development. Hodos's Chromium version stability. |
| **Best-when** | Hodos has dedicated CEF engineers and is explicitly targeting Brave-tier browser maturity as a v2 or v3 goal. Not appropriate as a v1 form factor for a small team. May be the right architecture to plan toward, with v1 shipping Option C and migrating to D as the team grows. |
| **Effort** | High. Highest of all five options. Risk: significant scope creep; Chromium version coupling creates a long-term maintenance tax. |

#### Option E — Omnibox / answer-engine-first (address bar as primary AI surface)

| Field | Detail |
|---|---|
| **Mechanism** | The Hodos address bar is extended into a three-mode input: navigate (URL), search (query → search engine), and AI-answer (NL → Edwin inline answer in an omnibox dropdown or dedicated results panel below the bar). Kagi's Quick Answer, Dia's tri-modal omnibox, and Atlas's address-bar handler are industry precedents. No separate panel at all. |
| **Pros** | Most natural browser interaction surface — users already use the address bar constantly. Zero screen real estate overhead when idle. Positions Hodos as an answer engine, the direction every major player is converging toward. Highly discoverable. |
| **Cons** | UX doc §1 explicitly states this is NOT iteration 1: "Address/search bar (omnibox) — AI answer-engine-style interaction directly in the bar. Later; the big incumbents are pushing hard here." Omnibox space is constrained for rich multi-turn conversations, payment consent dialogs, and streaming responses. No conversation persistence. Complex CEF work: the omnibox is one of the least extensible parts of CEF without source patches. Kagi's Quick Answer is a supplement to search, not a replacement — even the most search-forward AI browser keeps both. |
| **Dependencies** | CEF omnibox extension API (may require source patches). Intent classifier (NL vs URL vs query). Edwin API for inline answer generation. Integration with Hodos's existing search configuration. |
| **Best-when** | v2 or later, layered on top of whichever panel option ships in v1. Not a standalone v1 form factor. |
| **Effort** | High for standalone deployment. Medium if added as a supplement in v2. |

### Key Trade-offs

**Speed-to-v1 vs. UX ownership.** Option C can ship the fastest because Edwin's UI is already built — but Hodos's control over the casual-user experience depends entirely on Jake's UI quality, especially whether the LESSONS_LEARNED gaps have been fixed (misleading empty tabs, jargon setup, no cost guardrails). Option A gives Hodos full control but requires building and maintaining a chat UI. Option D gives the deepest integration but is a large-team's work.

**Idle RAM vs. discoverability.** Edge's real-world lesson (18% RAM reduction from going ambient) is genuine data. But Edge users already knew Copilot existed for years before Microsoft dissolved the sidebar. Hodos v1 users have no prior Edwin awareness — the panel is the discovery mechanism, not just a convenience. Ambient-only (Option B) is the right v2 pattern, not v1.

**Edwin's existing UI as asset or liability.** If Edwin's UI has solved the LESSONS_LEARNED casual-user problems (cheap defaults, no jargon, honest status surfaces), Option C is a strong choice. If the Sources tab still shows "file not found," if cost guardrails are absent, if the Skills panel reads 0 on first launch — Option C ships those problems under the Hodos brand, and Option A (Hodos builds the UI) becomes the safer path.

**Payment and cost transparency ownership.** LESSONS_LEARNED §3 is unambiguous: casual users need cheap safe defaults and visible cost with budget caps. Whoever owns the UI owns this responsibility. Option A gives it entirely to Hodos. Option C gives it to Jake's Edwin UI (which currently lacks it). This is a non-trivial product decision, not just a UI detail.

**Full-page-localhost vs. in-browser chrome.** A full-page localhost tab is essentially zero implementation effort and gives Edwin's richest desktop UI, but breaks the page-context relationship — the user leaves their browsing context, losing the ability to summarize or reference what they were looking at. An in-browser panel (Options A, C, D) keeps Edwin alongside the page. These are not mutually exclusive: a "Full page" expand button on a side panel satisfies both.

### Open Questions for Matt / Jake

- Does Edwin currently expose a sidecar-ready UI route at localhost (a responsive or panel-optimized layout), or only the full desktop app UI? If the latter, Option C requires Jake to build that route before Hodos can use it.
- Has Edwin's UI addressed the LESSONS_LEARNED §4-5 gaps before Hodos ships? If not, Option C ships those gaps under the Hodos brand.
- What is Hodos's actual frontend team capacity for v1? If there is not dedicated bandwidth for a polished chat UI, Option A's advantage is theoretical; Option C is the only path that ships on time.
- Full-page tab vs. panel as primary surface: should v1 ship a panel (user can use Edwin while browsing), or is "navigate to localhost tab" acceptable for the first iteration, with a panel in v2?
- Who owns the cost/payment transparency UX? If Edwin's UI does not surface cost per query, budget caps, and the BSV x402 consent event in a user-friendly way, does Hodos intercept and wrap it (requires Option A), or does Hodos defer and ship without those guardrails in v1?
- For Options C and A: what is the IPC mechanism for Edwin's panel UI to trigger Hodos's native payment-consent modal? The panel loads from localhost — a web origin. Does this go through a Hodos-registered custom scheme (hodos://), a localhost webhook, or a WebSocket from Edwin's sidecar to Hodos?

*Sources: BROWSER_AI_IMPLEMENTATION_STUDY.md §H3; UX_EDWIN_ASSISTANT_COMMUNICATION.md §1; ARCHITECTURE_TECHNICAL.md §2; LESSONS_LEARNED_EDWIN_INSTALL.md §3-5; EDWIN_NATIVE_PACKAGING_FINDINGS.md §1; ghacks.net May 2026 (Edge Copilot Mode retirement, ~18% idle RAM reduction); BROWSER_AI_IMPLEMENTATION_STUDY.md Appendix: Perplexity Comet*

---

## D4 — Search Posture

**Why it matters.** The omnibox is where users spend roughly 30% of browser interactions. How Hodos routes that intent determines: (a) when Edwin gets invoked and with what quality bar, (b) whether the per-cite x402 micropayment model has a natural trigger point, (c) how privacy-conscious users perceive being "handled" by AI vs. given control, and (d) the gap between Hodos's "native AI browser" positioning and its v1 reality. The publisher micropayment flywheel — per-cite payments only trigger naturally if the answer layer explicitly attributes sources — is an architectural property of the search routing, not something that can be bolted on later without a redesign.

### Options

#### Option A — Blend: three-mode omnibox with Edwin as an answer layer on top of search results

| Field | Detail |
|---|---|
| **Mechanism** | Three explicit omnibox modes — navigate (URL/domain), search (traditional results from a configurable provider), and AI (Edwin answer). Default mode is user-configurable; no mode is forced. Edwin is also available as an answer layer on top of the SERP: the browser feeds the query plus an AX tree snapshot of the SERP to Edwin via localhost IPC; Edwin returns a synthesized summary with citations rendered in a panel above or alongside the results. Per-cite micropayment fires when Edwin synthesizes from a source page: PermissionEngine checks if the cited domain has a BSV address; an x402 payment triggers within the daily budget cap. Payments outside the cap prompt the user. |
| **Pros** | User control is native to the design — Dia's validated omnibox pattern; DuckDuckGo's 30% install surge post-Google I/O confirms this drives acquisition among Hodos's target users. Per-cite micropayment has the most natural trigger point: Edwin explicitly names sources, so payment fires on a clear, attributable event. Uses existing PermissionEngine + x402 architecture — the payment-per-cite path is an extension of the existing envelope-gated flow, not a new system. No proprietary search index required for v1 — Hodos delegates retrieval to a search API (Brave Search, DuckDuckGo); Edwin layers synthesis. Fallback always available: traditional results are right there. Matches industry convergence (every privacy-forward player keeps the traditional search box as a first-class path while adding AI as parallel option). Privacy signal is strong: AI invocation is explicit, page context is only shared with Edwin on trigger. |
| **Cons** | More v1 complexity than traditional-only: three modes require routing logic in the CEF shell, a UI affordance for mode display/switching, and user education. Edwin's answer quality depends on what context it can access — without a real-time web crawler, Edwin synthesizes from the SERP page content (AX tree snapshot), which is shallower than fetching and reading actual source pages. Per-cite micropayment bootstrapping problem: most source pages do not have BSV addresses in v1, so the payment fires to zero or few sites and the model's killer feature is invisible. Search provider dependency: pricing/availability changes at that provider are a business dependency Hodos does not control. Three-mode UX requires more design polish to feel coherent for non-technical users. |
| **Dependencies** | Configurable search provider API (Brave Search API is a strong fit given its independent index and B2B growth). Edwin's natively running sidecar (D1/D2). Edwin to accept query + optional SERP context via localhost IPC and return structured answer + citations (new protocol, requires Jake alignment). PermissionEngine extended to recognize "AI cite" events as payment triggers. BSV address discovery mechanism for cited pages (may start as a no-op in v1). |
| **Best-when** | v1 ships before Edwin answer quality can support being the primary search surface; Hodos's target users are privacy-conscious but want control over when AI answers; the publisher micropayment ecosystem is early and needs to grow organically; Matt wants to position Hodos as "AI on your terms" rather than "AI instead of search." |
| **Effort** | Medium. New C++ work in the CEF shell for omnibox routing logic and mode-switching UI. New Edwin IPC protocol for query + context ingestion and structured answer + citation response. Extension of PermissionEngine to recognize per-cite payment events. Estimated: 3-6 weeks of new browser work on top of the already-planned Edwin sidecar integration, plus Jake alignment on the Edwin API surface. |

#### Option B — AI-answer-first: Edwin is the primary query surface (Comet/Atlas style)

| Field | Detail |
|---|---|
| **Mechanism** | The omnibox's primary action for any non-navigational input is Edwin. A lightweight heuristic classifier (or small local model) in the CEF shell distinguishes URLs/domains (direct navigation) from everything else (Edwin). Edwin returns a synthesized answer with cited sources, rendered as the primary browser content. Traditional search is available via an explicit toggle, secondary button, or keyword prefix ("search: ...") but is not the default. Per-cite micropayments fire from Edwin's citation output. Per-query AI cost (Edwin's inference via x402) is also billed at the moment of answer. |
| **Pros** | Strongest product differentiation: Hodos does not look like a browser with AI bolted on. Perplexity ($20B valuation, $500M ARR) and OpenAI Atlas have validated AI-first search at commercial scale. Cleaner UX for users who have already adopted AI-first search: no mode switching. Per-query micropayment model is simpler and more consistent. Forces a clear, differentiated value proposition from day one. Eliminates the search provider dependency. Citation-attribution payment model has a stronger story: every Edwin answer contains explicit citations, and those citations are the payment events. |
| **Cons** | Edwin's current capability is as a chat assistant / agent over local and cloud AI, not a real-time web search engine with a crawler and index. Positioning it as the primary answer surface before that quality is established will produce unreliable or stale answers for time-sensitive queries — a bad first impression on the browser's primary action is very hard to recover from. No proprietary real-time index means Edwin must either call a search API (re-introducing provider dependency), rely on training cutoff (stale), or use AX tree snapshot (only works for page-context queries) — all three have quality ceilings. DuckDuckGo's 30% install surge post-Google I/O demonstrates that even the privacy-forward user base includes a meaningful segment that resists AI-default search. Amazon injunction against Perplexity (March 2026, N.D. Cal.) established that AI-first summarization at scale carries CFAA risk when it displaces user visits to source pages. Navigational queries handled by an AI classifier introduce failure modes and latency that do not exist in a traditional omnibox. Misclassification feels broken to users. Citation attribution from AI synthesis is technically harder: Edwin must reliably report which source URLs it drew from for per-cite payment attribution — structured output requirement that does not currently exist. |
| **Dependencies** | Edwin must reliably handle arbitrary web queries at a quality bar appropriate for primary search — requires real-time web access or strong pre-indexed knowledge base. Edwin's citation output must be structured (source URL + sentence-level attribution) for per-cite payment attribution. A reliable lightweight query classifier in the CEF shell. Jake to expose a "search query" API on Edwin with structured citations. Content freshness strategy is a prerequisite. |
| **Best-when** | Edwin answer quality has been validated on a representative sample of real user queries; a real-time data freshness mechanism is in place; the publisher micropayment ecosystem has matured enough that citation-attribution payments can be verified in practice; Hodos is positioned as a full AI-browser product and the target user cohort has shifted to AI-native searchers. |
| **Effort** | High. Requires a query classifier in the CEF shell, content freshness mechanism for Edwin, structured citation output from Edwin (new API contract with Jake), per-cite payment attribution from AI synthesis output, and an answer-rendering UI that is not a traditional SERP. Quality and reliability bar to ship AI-first as the default is significantly higher. Rough estimate: 3-4 months of additional work beyond what is planned. |

#### Option C — Traditional-search-only for v1, defer the answer engine

| Field | Detail |
|---|---|
| **Mechanism** | v1 ships a conventional omnibox routing to a configurable search provider (DuckDuckGo default, user-selectable: Brave Search, Kagi, Google). No AI integration in the search/omnibox flow. Edwin is accessible as a separate surface via the agent overlay, a keyboard shortcut, or a toolbar button — siloed from search. Micropayments are deferred from the search flow entirely; they apply to Edwin overlay interactions but do not trigger from search results. Traditional SERP behavior: provider's result page loaded in the main browser tab. |
| **Pros** | Lowest v1 complexity and fastest path to a working browser. Zero risk of Edwin quality problems affecting the primary browser action. DuckDuckGo's 30% install surge and Vivaldi's 140% Norway growth prove that a traditional search browser with strong privacy credentials grows a user base without AI in the search flow. Preserves optionality: blend or AI-first can be added in v1.1 or v2 once Edwin quality is validated. No search provider API dependency beyond a configurable URL. Sidesteps the AI-answer legal exposure (summarization without explicit consent) for v1. If Edwin's overlay is complex or unreliable, users still have a working browser — the failure mode of AI is isolated from the failure mode of the browser. |
| **Cons** | Hodos positions itself as a "native AI browser" but the primary browser interaction surface (the search box) contains no AI in v1 — the positioning and the product are misaligned at launch. Traditional search in 2026 means competing in an index already being disrupted by AI Mode. No per-cite micropayment trigger in the search flow means the core monetization model's most natural driver does not activate from the primary user action. The "Edwin as separate overlay" pattern is exactly what Edge Copilot Mode used from 2023-2025 — Microsoft retired it in May 2026 explicitly because users treated it as a separate thing rather than a browser primitive. No incremental revenue model beyond the Edwin overlay micropayments. |
| **Dependencies** | Minimal. Configurable search provider URL (Brave Search, DuckDuckGo — no API key required for web search redirect). Edwin as a separate overlay (already planned regardless of D4 choice). |
| **Best-when** | v1 must ship fast and Edwin integration is not yet reliable enough for the primary search path; the team wants to validate browser UX (install, startup, wallet, ad-block) before layering AI into search; or the decision is to position v1 as a privacy browser first and an AI browser in v1.1. |
| **Effort** | Low. Traditional search is standard browser behavior. The only new work is the provider-selection UI in settings and the default provider choice. Estimated incremental effort: days, not weeks. |

### Key Trade-offs

**Quality ceiling vs. differentiation.** Option B puts Edwin in the primary search role before a real-time index or content freshness mechanism exists. If Edwin gives stale or wrong answers to common search queries, the browser's most important action is broken. Option A manages this by keeping traditional search as reliable ground truth while Edwin layers synthesis on top. Option C avoids the risk entirely but makes the "native AI browser" claim hollow in v1.

**User control vs. AI boldness.** Forced AI intermediation (Google AI Mode) drove DuckDuckGo installs up 30% among exactly Hodos's target users. But Hodos is pro-AI done privately — the real question is not "AI or no AI" but "who decides when AI answers." Option A makes user control the design principle. Option B bets that users who install Hodos want AI-first. Option C defers the bet. None is wrong; they serve different segments within the privacy-conscious-but-pro-AI user base.

**Micropayment trigger naturalness.** The per-cite x402 payment model fits Option A most naturally: Edwin explicitly names sources from a traditional SERP, and payment fires on a clear attribution event. In Option B, citation attribution from AI synthesis must be structured output from Edwin (harder, more failure modes). In Option C, there is no per-cite trigger in the search flow at all. The publisher-side adoption problem (most pages lack BSV addresses in v1) affects all three options equally — but the trigger architecture matters for when the flywheel can actually start.

**Architectural dependency on Jake.** Options A and B both require Edwin to accept queries via a new IPC/HTTP protocol and return structured answers with citations — new API surface Jake must build or approve. Option C requires nothing from Jake for the search posture specifically. If Jake's bandwidth is constrained or his refactor is mid-flight, Options A and B gate on his availability.

**Trajectory lock-in.** Moving from C to A (adding blend) is straightforward. Moving from A to B (making AI the default) requires validating quality first, then shipping a default-change. Moving from B back to A if B fails is a more visible product retreat. The ordering C → A → B is the lowest-risk trajectory; the ordering B → A if B fails is highest risk to user trust.

### Open Questions for Matt / Jake

- **Jake:** Does Edwin currently have a "search query" API — NL question in, structured answer + source URLs out? If not, what is the effort to add one? Options A and B both require this; Option C does not.
- **Jake:** What is Edwin's current data freshness mechanism for queries about recent events? Does it call an external search API for live results, or rely on the model's training cutoff? This directly determines whether Option B is viable for general web queries in v1.
- **Matt:** What search provider does Hodos intend to use for the "search" mode in Option A, and is there a budget for a paid search API (Brave Search API at $5/1000 requests, Kagi API, etc.)? DuckDuckGo's HTML endpoint is free but rate-limited and not production-reliable at scale.
- **Matt:** What is the minimum quality bar for Edwin answers before they can appear in the primary omnibox flow? Who evaluates this and how — informal testing, a benchmark, or user feedback in a private beta?
- **Matt:** For the per-cite micropayment model — what is the plan for discovering whether a source page has a BSV address (e.g., a `<meta name='bsv-address'>` field, or publisher opt-in via some other mechanism)? This determines when the payment flywheel can actually fire.
- **Jake/Matt:** Does Hodos's legal picture for Option B (AI-answer-first) account for the Amazon injunction against Perplexity (March 2026, U.S. District Court N.D. Cal.)? Option A (Edwin on top of search, with source links prominent) is structurally lower risk.
- **Matt:** Is the v1 target a privacy-focused general user (who expects a search box to work like a search box) or an AI-forward early adopter (who wants the answer engine experience)? Both can be justified for Hodos's audience but they serve different points on the pro-AI/privacy spectrum.

*Sources: UX_EDWIN_ASSISTANT_COMMUNICATION.md §5 C, D, F; BROWSER_AI_IMPLEMENTATION_STUDY.md §H2-H3, H5; ARCHITECTURE_TECHNICAL.md §2-4; LESSONS_LEARNED_EDWIN_INSTALL.md §5; INTEGRATION_RESEARCH_KICKOFF.md §3-4; techcrunch.com/2026/05/26/duckduckgo-installs-are-up-30...; cnbc.com/2026/03/10/amazon-wins-court-order-to-block-perplexitys...; searchenginejournal.com/perplexity-launches-comet-plus-shares-revenue...; techcrunch.com/2026/05/19/google-search-as-you-know-it-is-over/*

---

## D5 — Monetization Model

**Why it matters.** This determines Hodos's revenue model, ops sustainability, and whether the BSV/x402 differentiator is real or theoretical. It gates: (1) what Hodos can fund and staff from day one, (2) how the PermissionEngine budget-cap UX is designed, (3) whether publishers are paid per-cite (the structural gap no incumbent fills) or whether that remains a roadmap promise, (4) how the Edwin treasury split flows to Jake, and (5) whether casual users can onboard without understanding crypto at all. The precedent set by ARCHITECTURE_TECHNICAL.md §9 (Matt's instinct: 60/40 Hodos/Edwin on agent-authorized transactions) is the starting point, but it is explicitly flagged as an open design question for Jake.

### Options

#### Option A — Micropayment-only (x402/BSV per-query, per-cite, per-agent-action)

| Field | Detail |
|---|---|
| **Mechanism** | Every AI inference call, every publisher citation, and every agent action is individually priced and paid via a BSV satoshi micropayment at HTTP-layer (x402). User loads a BSV wallet at browser install. PermissionEngine's Silent/Prompt/Deny budget-cap logic governs each payment without requiring per-click user approval. Fiat-denominated display ('$0.001/query') with BSV settlement at current rate via a price oracle. Publisher per-cite payments sent directly to a BSV address embedded in x402 headers on the publisher's page — no enrollment, no pool, no 90-day settlement. Envelope-aware fee split routes a portion to Edwin treasury on each agent-authorized transaction (ARCHITECTURE_TECHNICAL.md §2). |
| **Pros** | Purest structural differentiator: the only browser model that pays publishers per-cite in real time with no enrollment, no pool, no intermediary rake — fills the precise gap Perplexity's $42.5M batch pool and BAT's 8-year failed creator-registration wall leave open. Payment IS the consent mechanism: x402 payment replaces "are you sure?" dialogs for agent actions — cryptographic, auditable, unforgeable. Zero monthly commitment for casual/occasional users — pay only what you use. On-chain audit trail (BRC-18 + envelope hash) of every agent action, inference call, and publisher payment. Aligns incentives with BSV's technical thesis: sub-cent, instant, permissionless HTTP payments. Publisher revenue starts flowing the moment a publisher adds a BSV address to their x402 header — no account creation, no custodial partner, no KYC wall. |
| **Cons** | Szabo's mental transaction costs: even invisible micropayments carry psychological weight if users sense money is leaving on every click — a behavioral problem 25 years of attempts have not solved without budget-cap abstraction. Two-sided cold-start: publishers must have BSV addresses and serve x402 headers; if they don't participate, per-cite story is fiction — Maxthon shipped VPoint in 2020, the loop never closed in 6 years; BAT failed this loop after 8 years and 100M MAU. x402 Foundation (Coinbase/Anthropic/AWS/Google/Vercel) implements EVM stablecoins — BSV is NOT in the foundation's reference implementation; Hodos maintains a parallel BSV x402 implementation off the main industry rails. Revenue unpredictability: zero subscription floor means ops costs must be funded from per-query margins — thin and volatile if LLM inference eats most of each payment. BSV price volatility creates wallet-value anxiety. Envelope overhead per query: 20 queries = 20 envelope operations. The '$20 lesson' risk: without tight defaults and hard caps, users can drain a wallet faster than expected. LLM provider BSV support is currently mediated through x402agency.com — Hodos is dependent on a third-party intermediary for the core inference payment rail. |
| **Dependencies** | Jake must publish Win/Mac native identity-core binaries (envelope signing path). x402agency.com must sustain BSV support for LLM inference routing. Publisher cold-start requires a specific vertical where publishers already have BSV addresses (1Sat Ordinals content is the candidate). BSV price oracle integrated into wallet for fiat-denominated display. PermissionEngine budget-cap defaults tuned for per-query micropayment safety. |
| **Best-when** | Hodos's launch user base is concentrated BSV-native early adopters who already have funded wallets; a specific content vertical (1Sat Ordinals, BSV DApp ecosystem) provides the publisher-side bootstrap so per-cite payments are real immediately; the team can sustain on thin per-query margin while publisher adoption grows. |
| **Effort** | Medium-High. Per-query envelope + x402/BSV payment path is already partially designed in ARCHITECTURE_TECHNICAL.md. Hard new work: fiat-denominated display with price oracle, publisher-side x402 header discovery and payment routing, PermissionEngine budget-cap defaults tuned for micropayment safety, wallet top-up UX with clear burn-rate display. Publisher cold-start is a go-to-market problem, not an engineering one, but it is the dominant risk. |

#### Option B — Subscription (fixed monthly BSV deposit / flat tier)

| Field | Detail |
|---|---|
| **Mechanism** | User pays a fixed monthly fee (e.g., $10 USD equivalent) billed in BSV from the Hodos wallet at renewal. Fee is deposited into a "credits bucket" tracked in the wallet. Hodos operates a privacy-preserving proxy (Brave Leo pattern) that pays LLM providers in bulk at negotiated rates — not via per-query x402 micropayments, but via periodic settlement. From the user's perspective: unlimited Edwin queries up to tier limits; no per-query decisions. BSV-denominated renewal auto-triggers from wallet at month boundary (a single periodic payment, not thousands of micropayments). Fiat-pegged amount ('$10/month equivalent in BSV at current rate') smooths BSV volatility at the billing event. Edwin treasury split applies to the subscription fee directly (e.g., 60/40 Hodos/Edwin per ARCHITECTURE_TECHNICAL.md §9). |
| **Pros** | Proven at scale: Kagi profitable at $10-25/month with ~37-person team at 72K subscribers; Brave Leo Premium at $14.99/month contributes to $100M ARR; DuckDuckGo Privacy Pro at $9.99-$19.99/month — model validated for privacy-conscious AI users (kagi.com/stats, June 26, 2026). Eliminates Szabo's mental transaction cost problem entirely at the user layer: one monthly decision instead of thousands of per-query micro-decisions. Revenue predictability: Hodos can plan infrastructure and Jake's treasury split around a recurring floor. Easiest casual-user onboarding: "it's like Kagi for AI, $10/month" is a mental model non-crypto users immediately understand. Hodos proxy model allows contractual no-train guarantees with LLM providers. Publisher cold-start problem is deferred — publishers receive nothing directly in v1. Single monthly wallet transaction reduces envelope/signing overhead to one operation per renewal cycle. |
| **Cons** | Abandons the per-cite publisher payment differentiator entirely: Hodos takes the subscription fee, publishers receive nothing directly, and the structural gap that makes Hodos architecturally unique is not filled — the BSV native story becomes "we use BSV to collect subscriptions." Competitive disadvantage against well-funded incumbents (Brave Leo, DuckDuckGo Pro, Kagi) requires matching model quality, proxy infrastructure, and support. Requires Hodos to operate and maintain a cloud proxy infrastructure — operational burden Brave (100M MAU, $100M ARR) can absorb but a seed-stage browser team may not. BSV is just a payment rail for a known model: no architectural differentiation from paying the same subscription in any other crypto. Churn risk as AI subscription market saturates: users with ChatGPT Plus, Claude Pro, and Kagi already may resist a fourth AI subscription. Delayed publisher ecosystem: if Hodos runs subscription-only for two years, publisher payment infrastructure is never built. |
| **Dependencies** | BSV-denominated subscription billing with auto-renewal from wallet. Hodos proxy infrastructure (or negotiated bulk rate with Dolphin Milk / x402agency.com). Contractual no-train agreements with LLM providers. Tier enforcement logic in the wallet credits system. No dependency on publisher BSV adoption. |
| **Best-when** | Hodos's early user base extends beyond BSV-native users to privacy-conscious mainstream users unfamiliar with crypto mechanics; the team prioritizes revenue predictability and sustainability over architectural purity; publisher cold-start cannot be solved in year one; direct competition with Kagi/Brave on their own terms is the strategic choice. |
| **Effort** | Medium. Subscription billing is known engineering. BSV-denominated renewal with fiat-pegging is slightly novel but tractable. Main complexity: proxy infrastructure — if Hodos negotiates a bulk rate with x402agency.com and batches costs, proxy ops can be deferred. Edwin treasury split on subscriptions requires a simple percentage calculation at renewal time. |

#### Option C — Both in parallel (subscription funds ops + micropayment as differentiator)

| Field | Detail |
|---|---|
| **Mechanism** | Two co-existing tracks: (1) Subscription tier ("Hodos AI Standard"): $8-12/month BSV equivalent, covers all standard Edwin inference queries, Hodos pays LLM providers in bulk, casual users never see a per-query payment, provides ops revenue floor. (2) Micropayment overlay ("Hodos AI Pay-as-you-go" and publisher payments): non-subscribers pay per-query via x402/BSV micropayments with a pre-loaded wallet balance; subscribers receive a "publisher payment" option — when Edwin cites a BSV-enabled publisher, the wallet can optionally send a per-cite micropayment to that publisher's BSV address; agent actions above the subscriber's included tier trigger x402 micropayments for overage. Brave's dual-stack model (Leo Premium + BAT creator tipping) is the prior art. Edwin treasury split: subscription fee splits on renewal (60/40); micropayments split on each envelope-gated transaction. |
| **Pros** | Brave's own model proves dual-stack is commercially viable: Leo Premium plus BAT creator payments co-exist for 100M MAU without confusing the core user base. Subscription revenue floor funds ops; micropayment track funds the publisher ecosystem bootstrap separately. Different user segments served simultaneously: casual users buy the subscription; BSV-native power users load a wallet and pay per-query; privacy maximalists use pay-as-you-go. Publisher cold-start can begin with the optional "fund publishers when Edwin cites them" feature even at subscriber scale. Micropayment track provides real UX for the x402/BSV differentiator without requiring it to be the only revenue model. PermissionEngine budget caps serve dual purpose (security layer + user-facing spending control). Edwin treasury split flows on both tracks. Kagi's "no use no pay" fairness principle can be implemented for pay-as-you-go users. |
| **Cons** | Two billing systems to build, maintain, and support — roughly doubles the billing surface area. User confusion is the primary UX risk: "am I on subscription or pay-as-you-go? Does Edwin's answer about my email cost me extra? Did the subscription cover that agent action or did it come out of my wallet?" Brave's BAT lesson: even with 100M MAU and 8 years of investment, BAT creator tipping reaches only ~2M verified publishers against millions of Brave-visited sites — running subscription + micropayments does not solve publisher cold-start, it just defers it with a different mechanism. The "optional publisher payment" feature for subscribers risks becoming the only micropayment use case if pay-as-you-go doesn't find adoption. More complex Jake dependency: both tracks require identity-core native binaries. Potential margin confusion if the two pricing models produce different effective per-query costs that users compare unfavorably. Highest engineering effort of the three options; startup team risk of building two things and executing neither well. |
| **Dependencies** | Everything from Option A (micropayment track) plus everything from Option B (subscription track). Both Jake's native identity-core binaries for Win/Mac AND Dolphin Milk x402agency.com BSV support. Publisher-side BSV header discovery for the optional per-cite payment feature. Clear UX design — which costs hit which track — must be finished before any billing ships. Pricing of both tiers must be validated so they do not cannibalize each other. |
| **Best-when** | Hodos has enough engineering capacity to build and maintain two billing tracks without diluting quality on either; a publisher vertical that will receive per-cite BSV payments has been validated (to make the micropayment track non-fictional); the BSV-native early adopter base and the crypto-agnostic privacy-conscious base are both reachable in year one; Brave's dual-stack is the explicit strategic model studied carefully rather than reinvented. |
| **Effort** | High. Full subscription infrastructure + full micropayment infrastructure + clear UX separating them + Edwin treasury split on both tracks + publisher payment routing. Likely a phased sequence: ship subscription-only first (Medium effort), add pay-as-you-go micropayment track second, add optional publisher payment feature third. The phased approach turns Option C into Option B followed by Option A incrementally, which reduces simultaneous risk. |

### Key Trade-offs

**Revenue predictability vs. differentiator purity.** Subscription gives Hodos a revenue floor to fund ops; micropayment-only generates zero predictable revenue and depends on per-query volume. The differentiator (per-cite publisher payments, the structural gap no incumbent fills) requires micropayments to be real, not roadmap. These pull in opposite directions: the commercially safer choice is subscription; the architecturally distinctive choice is micropayment-first.

**Szabo's mental-transaction-cost mitigation vs. transparency.** The only proven solution to the micro-decision problem is moving the decision to budget-setting (set a daily/monthly cap once, execute silently within it). Casual users should NEVER see a per-query cost prompt for standard queries — PermissionEngine's Silent path plus fiat-denominated budget caps is the mechanism. But invisibility at execution requires trust that the budget is correct, which requires excellent real-time spend dashboards. The '$20 lesson' shows that without hard caps AND visibility, users get surprised. All three options must solve this; they differ in whether the cap is a subscription boundary or a pre-approved wallet balance.

**Publisher cold-start timing vs. architectural completeness.** The per-cite publisher payment is the strongest structural argument for x402/BSV in a browser. But it requires publishers to have BSV addresses in their x402 headers — the same loop Maxthon failed to close in 6 years, BAT in 8 years. Option A makes publisher cold-start a launch-blocking requirement. Option B defers it indefinitely. Option C attempts it as a progressive enhancement. The honest question is whether Hodos has a credible publisher bootstrapping wedge (1Sat Ordinals content is the obvious candidate) or whether publisher cold-start will remain fictional.

**x402/BSV off the main rails.** The x402 Foundation (Coinbase/Anthropic/AWS/Vercel) implements EVM stablecoins, not BSV. Hodos's x402/BSV is a parallel implementation. Dolphin Milk routes through x402agency.com which appears to support BSV, but Hodos is not in the main x402 ecosystem. This limits which AI providers can be reached directly via x402/BSV without an intermediary. The differentiator claim depends on x402agency.com or future BSV support in the x402 Foundation — neither is guaranteed.

**Casual-user accessibility.** Option A requires users to understand wallet loading and accept that money leaves per query. Option B presents as a familiar SaaS subscription. Option C requires users to understand both. Hodos's stated north star is "easy for a casual, non-technical user." Micropayment-first is the hardest casual-user experience. Fiat-denominated display and pre-approved budget caps narrow the gap significantly, but the onboarding story ("load your wallet with $20 of BSV before you can ask a question") is fundamentally harder than "subscribe with a credit card for $10/month."

**Edwin treasury split.** ARCHITECTURE_TECHNICAL.md §9 identifies the split as an open design question for Jake. The formula differs by model: subscription gives Jake a predictable monthly percentage; micropayments give Jake a per-transaction percentage. Jake's incentives likely favor micropayments (aligned with Dolphin Milk's x402 architecture and his own envelope system) but may also value subscription predictability.

### Open Questions for Matt / Jake / John

- **Publisher cold-start bootstrapping wedge:** Is the 1Sat Ordinals content ecosystem large enough and willing enough to bootstrap per-cite BSV payments in year one? Without a credible answer, Option A's publisher payment story is fictional at launch.
- **x402agency.com BSV support scope:** Does x402agency.com support BSV denomination for ALL the LLM providers Hodos needs (Claude, GPT-5, open models)? Or is it limited to specific providers? This determines whether micropayment-only is architecturally complete or still dependent on intermediary choices.
- **Jake's Edwin treasury split preference:** ARCHITECTURE_TECHNICAL.md §9 identifies the split as open. Jake's view matters because subscription vs. micropayment changes when the split triggers and Jake's own Edwin revenue model depends on which track Hodos leads with. Does Jake prefer a predictable monthly subscription split or a per-transaction micropayment split?
- **Subscription infrastructure ownership:** If Option B or C is chosen, who operates the privacy proxy that pays LLM providers in bulk? Hodos running its own proxy is a significant ops commitment for a small team. Alternative: negotiate a bulk rate with Dolphin Milk / x402agency.com. Which path is Jake/John willing to support?
- **BSV volatility fiat-pegging mechanism:** For subscription billing, the renewal amount should be fiat-denominated ('$10/month equivalent in BSV'). What BSV price oracle does Hodos use, and how is it secured against oracle manipulation? This needs a clear policy before any billing ships.
- **Phased sequencing if Option C:** If both is the goal, what ships first — subscription (Option B) or micropayment pay-as-you-go (Option A)? Shipping subscription first provides revenue to fund micropayment track development, but risks the micropayment track never being built (BAT lesson). Does Matt have a clear phase sequence in mind?

*Sources: UX_EDWIN_ASSISTANT_COMMUNICATION.md §5 B (monetization thesis, Szabo mental transaction costs, BAT precedent, Mozilla Coil $3 experiment, Maxthon VPoint, Kagi/Brave/DuckDuckGo subscription facts); ARCHITECTURE_TECHNICAL.md §2, §3, §9; LESSONS_LEARNED_EDWIN_INSTALL.md §3, §5; BROWSER_AI_IMPLEMENTATION_STUDY.md §F, §H cross-cutting; github.com/x402-foundation/x402; aws.amazon.com/blogs/industries/x402-and-agentic-commerce...; brave.com/blog/bat-roadmap-3-0/; kagi.com/stats (72,586 members as of June 26, 2026)*

---

## Cross-Cutting Observations

**The transport-binding insight reshapes D1 and D2 together.** The discovery of `createNodeIdentityCoreBinding(transport)` and `desktop-binding.ts` means that the Jake-dependency on Windows native companion builds is now a Jake PR review against open-source TypeScript (D1 Option B), not a request for Jake to ship a closed-source binary to a platform he has not yet targeted. This materially changes the A-vs-B analysis in both D1 and D2. Any discussion that treats D1 Option B as a "workaround" rather than a "designed integration path" should re-read EDWIN_NATIVE_PACKAGING_FINDINGS.md on this point. The gap that remains even with Option B is shad-core (the vector store for recall) — that is still not solved by the transport binding.

**The casual-user north star is the consistent test.** Every decision should be evaluated against: "can a non-technical user install Hodos and have AI just work?" D1 Option D fails this test outright (separate install). D3 Option B fails it as a standalone v1 choice (ambient-only is undiscoverable for new users). D5 Option A with no budget-cap UX fails it (the $20 lesson). Wherever the answer to the north-star test is "not without additional work," that additional work must be explicitly scoped and owned.

**The upstream-PR-to-Jake constraint is structural.** Hodos does not fork Edwin; changes go upstream. This means: (a) any new Edwin API surface (structured citation output for D4, sidecar UI route for D3, search query endpoint) requires Jake's bandwidth, not just Hodos's; (b) the decision sequence must account for Jake's refactor in progress (qmd backend, pruned extensions) — building against a moving API surface risks rework; (c) Jake's responses to the consolidted questions below are not advisory, they are blocking inputs.

**Process homogeneity has a real value that should not be dismissed.** The Hodos wallet and adblock engine are both Rust subprocesses. Adding Edwin (a Node sidecar) breaks that homogeneity: Node's runtime security model, its npm supply chain surface, and its restart behavior under the C++ shell are all different from a Rust binary. This is not an argument for any specific option, but it is a real engineering maintenance consideration — particularly for a small team that already has to manage two upstream partners if Option C (three-party) is chosen in D2.

**The IPC bridge between Edwin's localhost panel and the Hodos C++ shell is a non-trivial design problem that spans D3 and D4.** If Edwin's UI (D3 Option C) needs to trigger the native payment-consent modal (PermissionEngine's Prompt path from ARCHITECTURE_TECHNICAL.md §3), and that UI is a web origin loading from localhost, it cannot call Hodos C++ APIs directly. The mechanism — a Hodos-registered custom scheme (hodos://), a localhost webhook, or a WebSocket from Edwin's sidecar to the shell — must be designed before any panel or payment UI is built. This design decision is currently unresolved in ARCHITECTURE_TECHNICAL.md and is a concrete output needed from the Jake design conversation.

**The x402/BSV position relative to the x402 Foundation requires an honest framing.** The x402 Foundation (Coinbase/Anthropic/AWS/Google/Vercel) has established EVM stablecoins as the reference implementation. Hodos's x402/BSV is an independent implementation of the same HTTP 402 concept. This is architecturally viable but means "pay any AI provider with BSV via x402" depends on x402agency.com as a gateway intermediary for non-BSV-native providers. The strength of the differentiator claim in D5 is bounded by the scope and reliability of that intermediary's support — a fact that should be surfaced honestly in any v1 pitch.

**The D5 publisher cold-start problem is not a v-next problem, it is a v1 design question.** If the per-cite publisher payment is real (not just roadmap), then the specific publisher vertical that bootstraps it must be identified and confirmed before v1 ships. The 1Sat Ordinals content ecosystem is the most plausible candidate given the existing BSV-native community, but this requires validation with actual content publishers, not just an architectural assumption. If that validation cannot be done before design lock-in, Option B or phased Option C (subscription first) is the honest choice.

---

## What to Ask Jake

The following questions appear across multiple decisions and are the minimum required inputs from Jake before architecture can be locked. They should be the agenda for the Jake design conversation.

**1. Transport-binding blessing (blocks D1 Option B, D2 Option A)**
Will you explicitly support `createNodeIdentityCoreBinding(transport)` and the `NodeIdentityCoreTransport` interface as a stable, public integration path — not an internal detail? Yes or no. If yes, the Hodos Rust wallet implementation of the four-method interface (signHttpRequest, signEnvelope, verifyEnvelope, getPublicKey) is a supported Hodos-side engineering task. If no, D1 Option A (waiting on Jake's native companion builds) is the only path, and v1 shipping is gated on Jake's Windows CI calendar.

**2. Windows native companion build timeline (blocks D1 Option A viability)**
Have Windows native companion builds for identity-core and shad-core (win32-x64 minimum) started? What is a realistic timeline for a published npm package? This single answer determines whether Option A is a v1 option or a v2 option.

**3. Shad-core / recall gap (affects D1 and D2 regardless of transport-binding answer)**
Does shad-core have a transport binding path analogous to identity-core's desktop-binding.ts? If not: should Hodos plan to (a) ship v1 without recall, (b) wait for Jake to publish shad-core Windows builds, or (c) implement the vector recall layer independently in Rust? This affects v1 feature scope and must be decided before any integration contract is written.

**4. Edwin API stability target (affects all decisions)**
What is the stable Edwin API surface for this integration — current beta.8/beta.9, or post-refactor main (qmd backend, pruned extensions)? If Jake's refactor changes the HTTP gateway API or the IdentityCore interface, Hodos building against beta.x now risks rework. Jake needs to designate a "Hodos integration branch" or API stability commitment before Hodos's integration code begins.

**5. Sidecar UI route and LESSONS_LEARNED gaps (affects D3)**
Will Edwin expose a sidecar-specific UI route (a responsive or panel-optimized layout) before Hodos ships? Have the specific LESSONS_LEARNED §4-5 gaps been fixed in Edwin's current UI — the Sources/Skills/Workflows tabs showing empty or misleading state on first launch, the absence of cost guardrails, the jargon setup flows? If not, D3 Option C (reusing Edwin's existing UI in the panel) ships those gaps under the Hodos brand.

**6. Search query API (affects D4)**
Does Edwin currently have a "search query" API — NL question in, structured answer + source URLs out — suitable for omnibox integration? If not, what is the effort to add one, and is it on Jake's near-term roadmap? Options A and B in D4 both require this; Option C does not.

**7. Edwin treasury split preference (affects D5)**
ARCHITECTURE_TECHNICAL.md §9 identifies the monetization split as an open design question. What is Jake's view on subscription split (predictable monthly percentage) vs. micropayment split (per-envelope transaction percentage)? Does Jake have a financial model for Edwin's operating costs that informs what a sustainable split looks like?

**8. BSL licensing and redistribution (affects D1)**
Can Hodos bundle and redistribute Jake's native companion binaries inside the Hodos installer under the BSL terms, or does each user need to accept a separate click-through? This affects installer UX and may be a meaningful factor in the D1 Option A vs. Option B decision.

---

## Appendix — raw per-decision analyses (structured)

> Captured 2026-06-26 by the architecture workflow. Each decision analyzed independently; pros/cons verbatim.

### D1 — How to bundle and run Edwin natively (no WSL) on Windows + macOS as a Hodos-managed localhost sidecar

**Why it matters.** This decision gates whether the casual-user north star is achievable at all. The WSL/9P bridge is the root of every "Edwin feels broken on Windows" experience; the fix is running Edwin's Node gateway natively, managed by the Hodos C++ shell the same way the wallet and adblock engine already are. The choice also determines whether v1 shipping is blocked on Jake building Windows native companions (a timeline gate Hodos does not control), and which party owns the cryptographic signing implementation — Jake's closed native binary or a Hodos Rust transport that speaks the same interface. Getting this wrong either blocks the product (too Jake-dependent), produces a broken UX (too complex for casual users), or creates a subtle security regression (transport semantics diverge from Jake's native impl). The choice also sets the monetization unlock timeline: x402 BSV micropayments through the Hodos wallet cannot be properly envelope-gated until the signing layer is settled.

#### Option: A — Bundled Node runtime + pruned node_modules + dist, with Jake's per-platform native companions

*Mechanism.* Hodos bundles a pinned Node.js v22.12 LTS binary (node.exe / node) inside its installer, alongside Edwin's built dist/ (18 MB), a production-pruned node_modules tree, and per-platform native companion packages that Jake builds and publishes for @edwinpai/identity-core and shad-core (win32-x64, win32-arm64, darwin-arm64, darwin-x64). The Hodos C++ shell spawns 'node dist/index.js' on a localhost port (e.g. :18789) as a managed subprocess — health-checked, restarted on crash, killed on browser exit — using the same subprocess-management pattern already in use for the wallet (:31301) and adblock engine (:31302). The user never sees Node, a port, or a daemon. All other native deps use their published prebuilts: @lydell/node-pty ships win32-x64 + win32-arm64 + darwin prebuilts (confirmed); sharp ships win32-x64 + darwin prebuilts (win32-arm64 is marginal — fewer than 30 downloads/month, listed for possible removal); sqlite-vec ships win32-x86_64 + darwin prebuilts via npm auto-download. EDWINPAI_IDENTITY_CORE_MODULE env hook lets Hodos point Edwin at the bundled companion's exact path, avoiding any runtime resolution ambiguity. Bundle size estimate: Node binary ~75 MB, dist/ 18 MB, pruned node_modules (production deps only, no Playwright/test tooling/dev channel SDKs) ~100–200 MB, native companions ~5–15 MB each — total installed footprint ~250–400 MB, compressed installer ~80–150 MB.

**Pros:**
- Full Edwin fidelity: envelope signing, BRC-103 signed requests, and shad-core recall use Jake's own tested implementation — no risk of semantic divergence in the crypto layer
- Clean split of responsibilities: Hodos is the packager and process manager; Jake owns all cryptographic code; a future Jake security fix drops in with a version bump, no Hodos code change needed
- The build pipeline for per-platform native companions already exists in Edwin's repo (identity-core:prepare-platform-packages, smoke, audit scripts); Jake needs to add Windows targets, not build a new system from scratch
- EDWINPAI_REQUIRE_NATIVE_PROTECTED_CORES + EDWINPAI_IDENTITY_CORE_MODULE env hooks already exist for exactly this bundled-path override — minimal Edwin code change needed
- BSL licensing on Jake's native companion is fully respected: the binary is distributed by Jake's authorized publish, not re-packaged by Hodos from source
- Well-established distribution pattern: VS Code and many non-Electron Node-based tools ship a bundled node binary + files; Windows Authenticode-signing and macOS notarization work on the bundled executable and companion .node files the same way as any signed binary

**Cons:**
- Hard timeline block on Jake: shipping native Windows support requires Jake to build and publish win32-x64 + win32-arm64 + darwin native companions for identity-core and shad-core; nothing Hodos can do will unblock this if Jake deprioritizes it
- Three-way release coordination: every Edwin update that touches identity-core or shad-core requires a coordinated Hodos installer update that bundles the matching companion version; a mismatch causes IdentityCoreUnavailableError at runtime, which must be caught and surfaced as a friendly message rather than a silent hang
- win32-arm64 is uncertain for sharp (sharp-libvips issue #238 flags fewer than 30 downloads/month, possible removal); a Windows ARM64 Hodos build may have to either drop image features or bundle libvips from source
- BSL licensing may require per-user license acceptance for Jake's native companion binary — needs legal clarity on whether Hodos's installer can redistribute without a separate click-through
- Pruning node_modules correctly requires maintaining a Hodos-side build script that runs 'npm prune --omit=dev' against Edwin's exact pinned version; this must be re-verified on every Edwin version bump to avoid silent breakage from newly-promoted devDependencies
- If Jake's CI doesn't yet run Windows native builds, Hodos must either wait or stand up a Windows CI runner on Jake's behalf — either option requires Jake's time

**Dependencies.** Jake must build and publish per-platform native companions for @edwinpai/identity-core and shad-core on Windows (x64 minimum, arm64 aspirational) and macOS (arm64 + x64). Jake must keep EDWINPAI_IDENTITY_CORE_MODULE override path supported. Hodos needs a Windows CI runner in its build pipeline to test the pruned bundle.

**Best when.** Jake actively plans to support Windows Edwin native installs anyway (for his own app distribution), so the companion builds are coming regardless and Hodos just needs to coordinate timing. Also the right long-term steady state once that infrastructure exists.

**Effort/risk.** Hodos effort: Medium (bundling toolchain, subprocess management, version-pinning scripts, installer integration, update pipeline). Jake dependency: High — blocks v1 shipping until Windows native companions are published. Risk: timeline slippage if Jake's native CI is not imminent.

#### Option: B — Bundled Node sidecar, IdentityCore backed by Hodos Rust wallet transport (no Jake native companion needed for signing)

*Mechanism.* Same as Option A for the Node/gateway packaging layer (bundled Node binary + pruned node_modules + dist/). The key difference: instead of loading Jake's native identity-core companion, Edwin is initialized with createNodeIdentityCoreBinding(transport) wired to the already-running Hodos wallet (:31301). The wallet gains 3–4 new HTTP endpoints implementing the NodeIdentityCoreTransport interface — signHttpRequest, signEnvelope, verifyEnvelope, getPublicKey — which map almost exactly onto the wallet's existing secp256k1 + DPAPI/Keychain + BRC-42 key-derivation stack and the envelope-issuance endpoints already specified in ARCHITECTURE_TECHNICAL.md §4. This pattern is proven: desktop-binding.ts in Edwin's own codebase already does this for the Rust/Tauri Desktop backend (snake_case field names in the binding reveal a Rust struct on the other side). The EDWINPAI_IDENTITY_CORE_MODULE env hook or a Hodos-controlled startup hook loads the transport binding rather than the native companion. Edwin signs all BRC-103 requests and envelopes via HTTP to the wallet; from Edwin's perspective the interface contract is identical to Jake's native. Shad-core/recall: either (B1) disabled in v1 — Edwin runs without shad-core, sqlite-vec handles the vector store layer without the shad-core overlay — or (B2) Jake is asked only to publish shad-core Windows builds (narrower ask than full Option A). Installed footprint same as Option A minus Jake's companion binaries (~10–20 MB smaller).

**Pros:**
- Eliminates the single biggest Option A blocker: Hodos does not need Jake's Windows native companions to ship v1, because the wallet already has all the cryptographic primitives (secp256k1, DPAPI/Keychain, BRC-42 key derivation, BRC-100 signing)
- Architecturally coherent with the existing ARCHITECTURE_TECHNICAL.md design: Hodos wallet is already specified as the envelope-issuance and verification authority; wiring Edwin's IdentityCore through the wallet is not a workaround but the designed integration
- desktop-binding.ts establishes that Jake already ships and uses a transport-backed IdentityCore for the Tauri desktop; Hodos is the second client of this pattern, not the first test
- Two-way instead of three-way release coordination: Hodos + Edwin only; no separate companion version to track
- Enables a faster v1 / demo timeline — can ship something real before Jake has Windows native builds ready
- The transport interface surface is small and stable: four methods (signHttpRequest, signEnvelope, verifyEnvelope, getPublicKey) with defined types in node-binding.ts and types.ts
- If Jake later ships Windows native companions, the wallet transport can be replaced with the companion binding transparently from Edwin's perspective — migration path to Option A exists without Edwin-side changes

**Cons:**
- Hodos must faithfully implement envelope semantics: nonce generation, TTL enforcement, payload hashing, secp256k1 ECDSA signature format must exactly match Jake's native implementation or downstream Edwin code (and x402 endpoints verifying envelopes) will silently reject or behave differently
- shad-core/recall still needs resolution: if shad-core is also a native companion with no transport binding, recall is either unavailable in v1 (acceptable) or requires Jake's build anyway (partial reversion to Option A dependency)
- The transport path must be blessed by Jake as a supported integration pattern, not just a convenient API surface — if Jake decides the transport binding is internal/unstable, a future Edwin refactor could break Hodos's implementation
- Every envelope-semantic edge case (clock skew, nonce replay guard timing, sub-key derivation path) that Jake's native companion handles must be correctly replicated in the Rust wallet; subtle bugs here are security-relevant
- Edwin's test suite tests identity-core with Jake's native companion; the transport path may have untested edge cases that only manifest at runtime in the Hodos integration
- If the transport binding adds meaningful round-trip latency (Edwin → wallet HTTP → back) on every signed request, and Edwin makes many signed requests per interaction, this could add perceptible latency compared to in-process native signing

**Dependencies.** Jake's explicit confirmation that createNodeIdentityCoreBinding / NodeIdentityCoreTransport is a supported, stable integration path. Hodos wallet must implement the 4-method transport interface (incremental Rust, maps to existing wallet capabilities). Shad-core story must be decided: v1 without recall, or a narrower ask to Jake for shad-core Windows builds only.

**Best when.** Jake has not yet built Windows native companions and there is no near-term timeline for them; or when shipping a v1 demo/proof-of-concept quickly takes priority; or as an explicit stepping-stone to Option A once Jake's companion builds are available. Also best if Jake explicitly prefers this integration pattern over bundling his closed binaries.

**Effort/risk.** Hodos effort: Medium-High (implement NodeIdentityCoreTransport in Rust wallet, validate envelope semantics against Jake's spec, write integration tests). Jake dependency: Low-Medium (only needs API stability confirmation, not new native builds). Risk: envelope fidelity risk is security-relevant and must be tested rigorously before shipping.

#### Option: C — Single-binary compile (Node SEA / yao-pkg / bun build --compile)

*Mechanism.* Attempt to pack Edwin's ESM gateway + dependencies into a single self-contained executable using one of three tools. Node SEA (--build-sea, stable since Node 22, one-step in Node 25.5+): requires transpiling Edwin's ESM entry to CommonJS first (Node SEA currently supports only CJS entry points per official docs and nodejs/help issue #5129); native .node files cannot be embedded in the blob and must be shipped alongside the executable, written to a temp file and dlopen'd at runtime. yao-pkg v6 (actively maintained fork of vercel/pkg, last release a month before this writing): same CJS-only constraint in standard mode; enhanced SEA mode uses Node's native SEA; native addons sit alongside. bun build --compile: supports ESM natively (no CJS transpilation step); also cannot embed native .node files; bun uses JavaScriptCore not V8, so Node-specific API behavior (especially conpty via node-pty on Windows) may not match Node.js runtime behavior. In all three cases: dynamic skill/plugin loading (Edwin loads 74 skills from filesystem paths discovered at runtime) cannot be statically analyzed by the bundler, meaning either all skills must be eagerly pre-bundled (breaking the dynamic loading architecture) or the skill loader must be excluded from bundling (reintroducing a folder of .js files alongside the binary).

**Pros:**
- Distribution looks cleaner on the surface — one executable file vs a folder structure, which may reduce user confusion if someone navigates to the install directory
- Slightly harder for a curious user to inspect or tamper with the bundled Edwin code (bytecode protection in pkg standard mode)
- Avoids shipping a visible node.exe in the installation, which can trigger end-user or IT security questions about 'why is there a Node.js server on my machine'

**Cons:**
- The 'single binary' claim is false for Edwin's dependency set: native .node files (node-pty, sharp, sqlite-vec, napi-rs/canvas, matrix-sdk-crypto) cannot be embedded and must sit alongside the executable as platform-specific shared libraries; the user gets a binary + a folder of .dlls/.dylibs, not a truly single file
- ESM entry point requires an additional transpilation step for Node SEA and pkg (CJS conversion via tsdown/rolldown — Edwin already has tsdown in its build, but adding this step to Hodos's build pipeline adds fragility on every Edwin version bump)
- Dynamic skill loading is architecturally incompatible with static bundling: Edwin discovers skills by reading a directory at runtime; a bundler cannot know which skill files to include; either the skill system is broken or skills must be kept as loose files alongside the binary (collapsing back to a folder distribution)
- bun's JavaScriptCore runtime is not 100% compatible with Node.js APIs: node-pty uses Windows ConPTY via native code that was built against Node-API/V8; behavior under bun's runtime is untested and may silently fail or behave differently
- Does not solve the identity-core native companion problem at all — Jake's .node file still cannot be embedded and must sit alongside; the signing layer dependency is unchanged relative to Option A
- Harder to debug in production: without a clear node.exe + source structure, crash investigation and log analysis is significantly more difficult; Edwin's own diagnostics assume a standard Node runtime
- Adds a new build step to Hodos's CI that must be re-validated on every Edwin version bump and cannot be automated without risk

**Dependencies.** Same Jake native companion dependency as Option A for identity-core and shad-core. Additional: transpilation pipeline from ESM to CJS (for SEA/pkg) or verified bun compatibility (for bun --compile). Dynamic skill loading must be redesigned or excluded from the bundle.

**Best when.** Edwin's architecture changes to static plugin loading, drops native addons entirely, and Jake provides Windows companions (making the .node problem moot). In Edwin's current state (June 2026), this option does not reach 'single binary' and produces a folder distribution with higher build complexity than Options A or B.

**Effort/risk.** High effort, high risk. The build complexity is significant, the benefits are cosmetic (still a folder), and the failure modes are subtle (skill loading silently broken, bun runtime incompatibilities). Not recommended for Edwin's current architecture.

#### Option: D — Ship Edwin as a separately installed app; Hodos connects to it over localhost

*Mechanism.* Hodos does not bundle Edwin at all. Instead, Hodos's C++ shell checks whether Edwin is running on its default port (18789) at startup. If present, the Hodos agent overlay connects to it. If absent, Hodos shows a first-run panel: 'To use the AI assistant, install EdwinPAI — [Download]', linking to Jake's own distribution. Edwin is installed, updated, and managed entirely by Jake's own installer (which currently does not exist for Windows — the current install.sh is curl-pipe-bash, Unix-only). The integration is purely API-level: Hodos speaks the Edwin HTTP gateway API as a client; no subprocess management, no bundled Node, no native companion build coordination. Users who already have Edwin installed get automatic Hodos integration.

**Pros:**
- Zero Hodos build dependency on Edwin internals: no bundling toolchain to maintain, no version-pinning scripts, no native companion coordination
- Users can update Edwin independently of Hodos; Hodos stays compatible as long as the Edwin HTTP API is stable
- Hodos does not need to navigate BSL licensing on Jake's native companion binaries
- Lowest Hodos engineering effort of any option
- Power users and developers who already run EdwinPAI benefit immediately

**Cons:**
- Directly fails the casual-user north star: 'install Hodos and AI just works' is impossible if the user must separately discover, download, and install Edwin
- Edwin has no Windows native installer today (install.sh is curl-pipe-bash, Unix-only, assumes Node pre-installed); until Jake ships a Windows .msi or similar, the separate-install path for casual users is the same WSL pain that made this problem worth solving
- No control over Edwin configuration from within Hodos: cheap defaults, budget caps, guided setup, and honest status surfaces (the LESSONS_LEARNED checklist) cannot be enforced if Hodos doesn't own the Edwin install
- Version compatibility is user-managed: if a user upgrades Edwin to a version that changes the HTTP API, Hodos integration silently breaks
- PermissionEngine envelope integration becomes much harder: if Hodos doesn't own the Edwin process, wiring the wallet's envelope-issuance into Edwin's request path requires either an Edwin plugin (Jake's work) or a proxy shim (added complexity with no user benefit)
- No practical monetization integration: the x402 BSV payment envelope chain cannot be enforced end-to-end if Edwin is a separately managed process the user could configure to bypass Hodos's wallet

**Dependencies.** Jake must ship a native Windows installer for Edwin (does not yet exist). Edwin's HTTP gateway API must remain stable across versions Hodos doesn't control.

**Best when.** As a very early prototype for developer/power users who already run Edwin natively, or as a future 'bring your own Edwin instance' advanced setting alongside a bundled default path. Not viable as the v1 casual-user experience.

**Effort/risk.** Low Hodos effort, but fails the product north star. Viable only as a fallback or power-user mode alongside one of the bundled options.

**Key trade-offs.** The core fork is Option A vs Option B, with C and D effectively ruled out for v1 (C produces a folder distribution anyway with higher build risk; D fails the casual-user north star entirely).

A vs B is a single dimension dressed up as multiple questions: WHO OWNS THE SIGNING IMPLEMENTATION, and WHO GATES THE TIMELINE?

In Option A, Jake's closed native binary is the signing authority — exact fidelity, no semantic risk, but v1 ships only after Jake builds Windows native companions (a hard external dependency on Jake's release calendar). In Option B, Hodos's Rust wallet is the signing authority via the transport binding — unblocks shipping immediately, but Hodos must correctly implement envelope semantics (a security-relevant implementation task), and this path needs Jake's explicit blessing as a supported integration pattern.

Shad-core/recall is a partial exception: even Option B may require Jake to publish a Windows shad-core build (the transport binding exists for identity-core; it is not confirmed for shad-core). The pragmatic v1 resolution is to ship without recall (shad-core disabled) and add it later — but this must be decided early because it affects the feature scope of the v1 pitch.

The natural migration path is B-then-A: ship v1 with the Hodos wallet transport (Option B, no Jake native dependency), validate the integration end-to-end, then replace the transport backing with Jake's native companion (Option A) when he publishes Windows builds. Edwin's code does not need to change for this migration — only the backing of the IdentityCore interface swaps. This path requires that Option B's transport implementation be correct enough to stake the v1 on.

The size/distribution question is a non-issue in practice: bundled Node (~75 MB) + dist/ (18 MB) + pruned node_modules (est. 100–200 MB) is in the same order of magnitude as any modern browser extension system or productivity app. Win32-arm64 for sharp is a real concern (may need to defer or bundle libvips from source), but it is not a blocker for the Intel/AMD Windows install that covers the vast majority of current Windows machines.

**Open questions:**
- Jake: Has Windows native companion development (win32-x64 + darwin-arm64 at minimum) for identity-core and shad-core already started, or is it unplanned? What is a realistic timeline? This is the sole question that determines whether Option A or Option B must go first.
- Jake: Do you explicitly bless the createNodeIdentityCoreBinding / NodeIdentityCoreTransport path as a supported, stable integration surface for Hodos? Or is the transport binding internal and likely to change during the current refactor? (If the answer is 'stable and supported', Option B is safe to build on.)
- Jake: Does shad-core have a transport binding path analogous to identity-core's desktop-binding.ts? If not, is the plan for v1 Hodos to run without shad-core/recall, or to require Jake to publish a shad-core Windows native companion before recall works?
- Jake: What is the stable Edwin API surface for this integration — current beta.8/beta.9, or post-refactor main (qmd backend, pruned extensions)? Building against a moving target risks rework if major interfaces change mid-integration.
- Matt: For v1 scope, is Edwin-without-recall acceptable (shad-core disabled, just the assistant + signing)? This unlocks Option B without any Jake native builds, but the LESSONS_LEARNED 'recall' feature (index your files) won't be available. Is that a v1 deferral or a hard requirement?
- Matt: Is win32-arm64 (Windows on ARM, e.g. Qualcomm Snapdragon laptops) a day-one requirement for the Hodos + Edwin integration, or deferred to a later build target? Sharp's win32-arm64 prebuild support is marginal and may not exist.
- Matt: On Option B's timeline advantage vs security risk — how much envelope-fidelity testing would be required before shipping v1 to real users? This determines whether B's 'faster' advantage is real (weeks saved) or illusory (validation time erases the gain).
- Matt/Jake: BSL licensing on Jake's native companion — can Hodos bundle and redistribute the native companion binaries in the Hodos installer, or does each user need to accept a separate license click-through? This affects installer UX and may favor Option B (transport, no BSL binary to redistribute) for the initial ship.

**Sources:** <https://nodejs.org/api/single-executable-applications.html — Node.js SEA official docs: CJS-only entry point limitation, native addon asset bundling constraints> · <https://github.com/nodejs/help/issues/5129 — Node SEA + ESM + native addons issue thread: confirms ESM-first projects require CJS transpilation step for SEA> · <https://joyeecheung.github.io/blog/2026/01/26/improving-single-executable-application-building-for-node-js/ — Node.js SEA 2026 improvements overview (--build-sea one-step command in Node 25.5)> · <https://github.com/lovell/sharp-libvips/issues/238 — sharp win32-arm64 prebuild tracking issue: fewer than 30 downloads/month, flagged for possible removal> · <https://sharp.pixelplumbing.com/install/ — sharp official install docs: prebuilt binary coverage, Windows support notes> · <https://www.npmjs.com/package/@lydell/node-pty — @lydell/node-pty npm: confirms prebuilts for win32-x64, win32-arm64, darwin; conpty-only on Windows; never calls node-gyp> · <https://github.com/lydell/node-pty — @lydell/node-pty GitHub: version 1.2.0-beta.12 (April 2026), prebuild branch details, Win arm64 package existence> · <https://github.com/yao-pkg/pkg — yao-pkg/pkg GitHub: actively maintained vercel/pkg fork, v6.20.x, supports Node 22+, SEA-enhanced mode> · <https://deepwiki.com/yao-pkg/pkg/3.1-single-executable-applications-(sea) — yao-pkg SEA mode: pkg . --sea uses Node native SEA under the hood; same CJS + native addon constraints apply> · <https://alexgarcia.xyz/sqlite-vec/installation.html — sqlite-vec installation: npm package auto-downloads prebuilt loadable extensions for win-x86_64 (.dll), macOS (.dylib); disable with SQLITE3_VEC_PREBUILT=0> · <https://github.com/asg017/sqlite-vec/releases — sqlite-vec releases: confirms win32-x86_64 prebuilt loadable extension published per release> · <https://www.npmjs.com/package/@lydell/node-pty-win32-arm64 — @lydell/node-pty-win32-arm64 npm package exists and is published separately> · <C:\Users\archb\Hodos-Browser\development-docs\Dolphin Milk + Edwin Integration\EDWIN_NATIVE_PACKAGING_FINDINGS.md — primary source for Edwin internals: transport binding, native dep surface, protected core architecture, bundle size facts> · <C:\Users\archb\Hodos-Browser\development-docs\Dolphin Milk + Edwin Integration\LESSONS_LEARNED_EDWIN_INSTALL.md — field findings from Windows install: WSL/9P as root cause, casual-user requirements checklist> · <C:\Users\archb\Hodos-Browser\development-docs\Dolphin Milk + Edwin Integration\ARCHITECTURE_TECHNICAL.md — three-party design, PermissionEngine + envelope synthesis, Jake agenda items>

### D2 — Which runtime is the in-browser assistant: Edwin-only, Dolphin Milk-only, or both (three-party)

**Why it matters.** This decision sets the subprocess count, the upstream partnership surface, the install footprint, the casual-user assistant capability on day one, and the x402 monetization architecture. It gates: (1) how soon Matt can ship a working PoC without Jake's closed native builds, (2) whether the casual user gets EdwinPAI's full skill/recall experience or a thinner x402-native agent, (3) whether Hodos has one or two upstream partners to coordinate, and (4) which parts of ARCHITECTURE_TECHNICAL.md's three-party envelope/PermissionEngine design are actually realized vs. deferred. The transport-binding discovery (Hodos wallet can implement IdentityCore transport, removing the need for Jake's per-platform native companion for IDENTITY — though not for shad-core/recall) significantly shifts the Option A calculus compared to what the existing docs assumed.

#### Option: Option A — Edwin-only: Edwin Node sidecar is the assistant; vault via Hodos wallet transport; no Dolphin Milk

*Mechanism.* Edwin runs as a fourth managed subprocess alongside the wallet and adblock engine — the C++ shell spawns a bundled Node runtime pointing at Edwin's dist/index.js on a localhost port, using the same health-check/restart pattern already in use. Edwin is the sole AI assistant: conversational turns, 74+ hot-reloadable skills, qmd/shad recall, LLM routing, BRC-103 signed-request gateway. The IdentityCore vault is NOT Jake's closed-source native companion: instead, Hodos's existing Rust wallet implements the createNodeIdentityCoreBinding(transport) interface (signHttpRequest / signEnvelope / verifyEnvelope / getPublicKey) against its existing secp256k1 / DPAPI / BRC-100 key stack. Edwin signs through Hodos. The EDWINPAI_IDENTITY_CORE_MODULE env override wire this cleanly. For x402 payments: Edwin calls wallet/createAction → PermissionEngine decides → wallet issues envelope + signs BSV transaction → Edwin carries the x402 payment header to the LLM endpoint. No Dolphin Milk subprocess. shad-core (recall vector store) is a separate question — it either needs Jake's native build or a parallel Hodos Rust implementation.

**Pros:**
- Single agent subprocess instead of two: simpler process management, simpler failure recovery for v1
- Matt runs EdwinPAI daily — it is the proven assistant experience he has confidence in; bundling it is a packaging problem, not an unknown-UX bet
- 74+ skills immediately available in the browser from day one; hot-reload means Jake can ship new skills without a Hodos rebuild
- qmd/shad recall (index files once, retrieve relevant snippets) is already designed for the browser-assistant use case — it is exactly the right architecture for a browser-native personal assistant
- Transport-binding discovery removes Jake's Win/Mac native core as a hard blocker for the identity/signing surface — Hodos implements the transport, Jake only needs to bless a pattern in open-source TypeScript, not ship a new closed-source binary
- Multi-channel inbox (WhatsApp, Signal, Telegram, Discord) is a differentiator Dolphin Milk does not have
- Jake partnership is a single relationship, single codebase — simpler negotiation than managing two upstream partners simultaneously
- EDWINPAI_IDENTITY_CORE_MODULE override makes the transport swap testable in isolation before any Hodos C++ work

**Cons:**
- Node.js sidecar (18MB dist + pruned node_modules + bundled Node runtime) vs a lean Rust binary — meaningfully higher idle memory footprint and startup time; on an 8GB RAM Windows laptop this matters
- Node SEA is a dead-end for Edwin's profile (ESM + multiple native addons + dynamic skill loading); the only viable packaging is bundled-runtime-sidecar, which makes the installer more complex
- x402 payment execution is Edwin making standard HTTP calls with a BSV payment header — the protocol handling is functional but not purpose-built the way Dolphin Milk's BRC-29 x402 client is
- shad-core (recall vector store) is NOT solved by the transport-binding path — it still depends on Jake's native build OR a separate Hodos Rust implementation of the vector store; this is an unresolved gap
- Edwin's narrate-then-stop agent loop (verified in the code study) may need upstream PRs to Jake for browser-assistant UX tuning where tasks require looping or multi-turn tool continuations
- BRC-18 on-chain proof of every agent decision (the cryptographic audit trail) is a Dolphin Milk feature — absent from Edwin out of the box; Hodos would need to add it separately if required
- No John (Dolphin Milk) partnership; Hodos loses an Apache-2.0-licensed, x402-native Rust agent that is already BRC-100-compatible (Canary A1 verified)
- Edwin's skills and recall add to startup cost: if the user never uses recall, they are still paying the shad-core memory and startup tax

**Dependencies.** Jake: bless the createNodeIdentityCoreBinding(transport) path in open-source TypeScript (discussion/PR, not a binary build). shad-core: separate Jake decision — native build for Win/Mac OR Hodos implements vector recall in Rust. Hodos engineering: Node runtime bundling for Win/Mac (complex installer), subprocess wrapper (follows existing wallet/adblock pattern), wallet transport implementation (~2-3 weeks Rust). No John/Dolphin Milk dependency.

**Best when.** Jake confirms the transport-binding path quickly; Matt prioritizes delivering the full EdwinPAI assistant experience (skills, recall, multi-channel inbox) over raw x402 protocol fidelity; v1 casual-user value proposition is 'a real, full-featured AI assistant in your browser'; timeline is tighter and fewer upstream parties means fewer coordination risks; shad-core has a viable story (Jake builds Win/Mac OR Hodos defers recall to a later version).

**Effort/risk.** Medium. Hodos work: Node runtime bundling for Win/Mac installer (precedented but complex), subprocess wrapper following existing pattern, Rust wallet transport implementation. Jake work: reviewing and blessing the transport-binding PR in TypeScript — this is the critical-path item. Risk: if Jake does not bless transport-binding quickly, the fall-back is waiting on Jake's Win/Mac native builds for identity-core AND shad-core, which adds indeterminate timeline risk.

#### Option: Option B — Dolphin Milk-only: Rust x402 agent is the assistant; Edwin contributes only the vault/envelope spec

*Mechanism.* Dolphin Milk (Rust binary, port 8080) is the sole agent/assistant subprocess, managed by the Hodos C++ shell exactly as ARCHITECTURE_TECHNICAL.md specifies. Edwin's contribution is purely a spec reference: the SignedEnvelope / SignEnvelopeInput / VerifyEnvelopeOptions types from Edwin's open-source types.ts are the schema reference for Hodos's Rust implementation of envelope issuance and verification in the wallet. No Edwin Node sidecar runs at runtime. The Hodos wallet implements the envelope semantics natively in Rust using its existing secp256k1/DPAPI/BRC-100 key stack — conceptually equivalent to what the transport-binding path would do in Option A, but with no Edwin gateway in the loop at all. Dolphin Milk makes BRC-29 x402 calls → Hodos PermissionEngine decides → wallet issues envelope + signs BSV transaction → Dolphin Milk carries the x402 payment header. The assistant UI is Dolphin Milk's /ui/ surface or a Hodos-native overlay calling Dolphin Milk's HTTP API.

**Pros:**
- All-Rust subprocess architecture: Hodos wallet + Dolphin Milk + adblock — process management is homogeneous, startup is fast, idle memory is low
- Dolphin Milk's x402 plumbing is purpose-built: BRC-29 payment construction, x-bsv-payment header protocol handling, x402agency.com marketplace routing — this is the x402 story, not an add-on
- Zero Node runtime dependency: no Node bundled, no pruned node_modules, no ESM/native-addon/SEA packaging complexity
- Zero Jake binary dependency at runtime: Hodos implements the envelope spec in Rust referencing Edwin's open-source types as a spec; Jake's role is spec author, not binary publisher
- BRC-18 on-chain proof of every agent decision is built into Dolphin Milk per ARCHITECTURE_TECHNICAL.md §7 — audit trail is native
- Smallest installer footprint of the three options
- Apache 2.0 license for Dolphin Milk means bundling is free; coordination with John is version-cadence only, not a binary-publishing dependency on a closed-source companion
- If the v1 pitch is 'the browser that pays for AI natively via micropayments', Dolphin Milk delivers the x402 mechanism with the least ceremony

**Cons:**
- Dolphin Milk is an x402 agentic task runner, not a general conversational AI with a skill ecosystem — the casual user asking 'summarize this page', 'help me write this email', or 'what did I read last week?' may get a thinner experience than EdwinPAI provides
- No Edwin skill ecosystem (74+ skills): every browser-relevant skill capability would need to be built in Dolphin Milk's framework or added as a Hodos overlay, representing significant engineering not accounted for in the existing docs
- No qmd/shad recall: the file/knowledge-indexing capability that makes Edwin valuable as a personal assistant is absent
- No multi-channel inbox (WhatsApp, Signal, Telegram, Discord) — this is Edwin's feature, not Dolphin Milk's
- Matt does not run Dolphin Milk as his daily assistant; the EdwinPAI experience he has battle-tested and has confidence in is absent from this option
- Edwin's BRC-103 signed-request gateway (the security differentiator named in LESSONS_LEARNED §6) is absent; the security backbone is purely Hodos's Rust envelope implementation, which is unproven in production
- If Dolphin Milk's assistant UX is immature for general conversational tasks, the casual user sees a bare-bones experience regardless of how correct the x402 plumbing is — this is a validation risk that requires testing Dolphin Milk as an actual assistant before committing
- John must publish and maintain Win/Mac Dolphin Milk binaries on a coordinated release schedule — different upstream from Jake but still an upstream dependency requiring a partnership agreement

**Dependencies.** John: Win/Mac Dolphin Milk binary builds on a coordinated version cadence. Hodos engineering: wallet Rust envelope implementation (referencing Edwin types.ts as spec, 2-4 weeks), subprocess wrapper (same pattern as wallet/adblock), three Canary A1 wallet-shim patches already planned. No Jake runtime dependency. Key validation dependency: someone (Matt or a Hodos tester) needs to run Dolphin Milk as an actual daily assistant to confirm its conversational/general-assistant maturity before v1.

**Best when.** v1 is primarily the x402 micropayment story and the assistant capability needed is agentic x402 tasks rather than general conversational AI with recall and skills; Jake's timeline for native builds or transport-binding blessing is uncertain; Hodos strategically prefers a homogeneous all-Rust subprocess lineup; Matt is explicitly OK shipping without the EdwinPAI skill/recall experience in v1 and adding it later; Dolphin Milk's assistant UX is validated as sufficient for casual users.

**Effort/risk.** Medium-low (for Hodos). The hardest Hodos work is the wallet envelope implementation in Rust (2-4 weeks). Subprocess wrapper follows the existing wallet/adblock pattern. No Node runtime packaging. Risk: Dolphin Milk's maturity as a conversational assistant is the largest unknown — this is an empirical question that needs a hands-on session with John, analogous to the Edwin install session in LESSONS_LEARNED.

#### Option: Option C — Both (three-party): Edwin as assistant layer + Dolphin Milk as x402 runtime + Hodos wallet as vault

*Mechanism.* This is ARCHITECTURE_TECHNICAL.md's designed three-party system, with the transport-binding discovery applied to collapse the vault into the Hodos wallet. Three managed subprocesses: Hodos wallet (:31301, key custody + PermissionEngine + envelope gate — also IdentityCore transport backend for Edwin), Dolphin Milk (:8080, x402 agent runtime, BRC-29 payments, BRC-18 on-chain proofs), Edwin (:8090 or similar, Node sidecar, conversational assistant, skills, recall, BRC-103 gateway). Two sub-variants exist: C1 = Edwin is the user-facing front door (user talks to Edwin; Edwin delegates x402 agentic tasks to Dolphin Milk via a defined IPC/HTTP protocol); C2 = Dolphin Milk is the primary agent orchestrator (Dolphin Milk calls Edwin's skills/conversation API for tasks requiring natural language or recall). In both sub-variants the PermissionEngine/envelope gate in the Hodos wallet governs ALL agent requests — both Edwin's wallet calls and Dolphin Milk's wallet calls. The monetization fee-split (ARCHITECTURE_TECHNICAL.md §7, §9 item 7) is most fully realized here: x402 call → Dolphin Milk routes → wallet signs → fee split path possible for Edwin treasury.

**Pros:**
- Maximum capability per party's strength: x402 protocol handling (Dolphin Milk), full assistant skills/recall/multi-channel (Edwin), key custody and trust gate (Hodos wallet) — no capability is sacrificed
- BRC-18 on-chain proof (Dolphin Milk) + Edwin envelope spec (Hodos wallet Rust) + Edwin skills (Edwin Node) = the most comprehensive audit trail and assistant capability combination
- The full three-party trust model from ARCHITECTURE_TECHNICAL.md is realized: PermissionEngine governs all agent activity; every signing path is envelope-gated regardless of which agent initiated it
- Transport-binding collapses the vault question cleanly: both Edwin and Dolphin Milk trust the same Hodos wallet as vault, reducing the number of distinct crypto implementations
- Two upstream partners (Jake + John) provides redundancy: if one feature area stalls, the other's piece still ships and delivers value
- Monetization fee-split architecture is most complete: three-way value attribution (Hodos operating revenue, Edwin treasury, Dolphin Milk/John ecosystem) is structurally supported
- The product narrative is most defensible long-term: 'the only browser with a real AI assistant (Edwin), an x402 payment engine (Dolphin Milk), and a user-sovereign cryptographic vault (Hodos wallet)' — each party's brand and differentiator is preserved

**Cons:**
- Two agent subprocesses requiring two sets of Win/Mac native builds from two upstream partners — coordination overhead is doubled; if either Jake's timeline or John's timeline slips, the full three-party experience slips
- The Edwin-to-Dolphin-Milk handoff protocol (how does Edwin request a Dolphin Milk x402 task? how does Dolphin Milk return the result?) does not exist yet — designing and implementing this new IPC/HTTP contract is a non-trivial engineering surface not accounted for in current docs
- C1 vs C2 sub-variant ambiguity: who is the primary orchestrator is an architectural decision that affects every integration seam and must be settled before any code is written; this requires Jake AND John agreement and potentially changes both projects
- Capability overlap risk: both Edwin and Dolphin Milk can independently initiate x402 wallet calls; without a clear ownership rule, there is potential for double-routing, race conditions, or ambiguous UX (which subprocess is 'doing' the task the user sees?)
- Memory footprint: Edwin Node sidecar (estimated 200-400MB with skills loaded) + Dolphin Milk Rust binary + Hodos wallet = three subprocesses consuming real RAM; on an 8GB laptop this may be noticeable to the user
- The user-facing presentation must hide all of this complexity — the casual user should see one seamless assistant, not 'Edwin' and 'Dolphin Milk' as separate things; building that seamless presentation layer is additional engineering
- v1 scope risk: shipping both subprocesses with a defined integration contract makes the PoC substantially larger; the AWS pitch window in INTEGRATION_RESEARCH_KICKOFF.md suggests timeline pressure that three-party complexity may not accommodate
- Three-party protocol coordination (BRC schema updates, envelope schema versions, x402 API changes at x402agency.com) is operationally harder than two-party and grows as a maintenance burden over time

**Dependencies.** Jake: bless transport-binding OR publish Win/Mac native builds for identity-core AND shad-core, AND agree on the Edwin-Dolphin Milk handoff protocol. John: Win/Mac Dolphin Milk binary builds AND agree on the Edwin-Dolphin Milk handoff protocol. Hodos engineering: two subprocess wrappers, wallet Rust transport + envelope implementation, PermissionEngine extension, Edwin-Dolphin Milk handoff protocol design and implementation, dual Win/Mac native build coordination. Jake + John + Matt must align on C1 vs C2 (who orchestrates) before any integration code is written.

**Best when.** Both Jake and John are confirmed partners before v1 begins with explicit commitments on Win/Mac build timelines; the product pitch explicitly features both EdwinPAI AND Dolphin Milk as named components; Hodos has sufficient engineering capacity for the three-party integration and the handoff protocol design; the monetization fee-split model with three-way attribution is designed from day one; timeline for v1 is not tightly constrained (3-6 months vs. 1-2 months).

**Effort/risk.** High. Hodos work: two subprocess wrappers, wallet Rust transport + envelope implementation, PermissionEngine C++ extension, Edwin-Dolphin Milk handoff protocol design and implementation. Jake work: native builds OR transport-blessing, handoff protocol agreement. John work: Win/Mac Dolphin Milk builds, handoff protocol agreement. Risk: coordination overhead between two upstream maintainers on an undefined protocol is the primary risk; if the C1/C2 orchestration question is not settled early, integration rework is likely.

**Key trade-offs.** ASSISTANT DEPTH vs. X402 PROTOCOL FIDELITY: Edwin has the assistant depth (74+ skills, recall, multi-channel inbox, BRC-103 gateway, the experience Matt runs daily). Dolphin Milk has the x402 protocol fidelity (BRC-29 payment construction, x-bsv-payment header, purpose-built agent runtime, BRC-18 on-chain audit trail). You cannot fully get both without Option C's complexity. Options A and B each sacrifice one dimension.

THE TRANSPORT-BINDING DISCOVERY SHIFTS OPTION A'S VIABILITY: The existing docs assumed Option A required Jake's Win/Mac native cores as a hard dependency. The code study found createNodeIdentityCoreBinding(transport) lets ANY async object back IdentityCore, and desktop-binding.ts already proves a Rust backend can do it. This means Option A's Jake dependency is now a PR against open-source TypeScript (not a closed-source build request) — a materially different negotiation. However, shad-core (recall vector store) is NOT solved by this path; it remains Jake's binary or a separate Hodos Rust implementation.

PROCESS HOMOGENEITY vs. CAPABILITY BREADTH: All-Rust (Option B) gives a lean, consistent subprocess lineup that matches Hodos's existing wallet/adblock pattern. Adding Edwin (Options A, C) adds the full assistant capability but introduces a Node runtime, complex installer packaging, and meaningfully higher idle memory. On a typical 8GB Windows laptop, the difference between 'Rust agent binary' and 'Node sidecar with skills loaded' is real.

UPSTREAM DEPENDENCY COUNT vs. FEATURE SET: Option B minimizes Jake dependency (spec reference only). Option A has one active upstream (Jake). Option C has two active upstreams (Jake + John) and introduces a new integration contract between them. More partners = more capability = more coordination risk. The upstream-PR constraint (no rewrites; changes go to Jake) makes this asymmetric: Hodos can control its own packaging work, but cannot force Jake's or John's build timelines.

NOTE ON X402 ECOSYSTEM: The real-world x402 protocol (Coinbase, x402 Foundation) is primarily Base/Solana stablecoin-based (156K weekly transactions as of early 2026). Dolphin Milk implements a BSV-flavored x402 using BRC-29. This is architecturally correct for the Hodos/BSV stack but means the 'x402 marketplace' is effectively self-contained within the BSV/Dolphin Milk ecosystem rather than interoperable with the broader Coinbase x402 Foundation. This does not break any option but is relevant context for the monetization narrative.

**Open questions:**
- JAKE: Will you bless the createNodeIdentityCoreBinding(transport) path where the Hodos Rust wallet backs IdentityCore — yes or no? This is the pivotal question for Option A. If yes, the Win/Mac native identity-core build is no longer on the critical path. If no, Option A requires waiting on your Win/Mac native companion builds for identity-core.
- JAKE: What is shad-core's Win/Mac native build status? The transport-binding path solves identity (signing/envelopes) but NOT recall (vector store). Does shad-core have a Windows/macOS native package in the pipeline, or should Hodos plan to defer recall to a later version or implement it independently in Rust?
- JAKE + JOHN: In Option C, who is the primary agent orchestrator — Edwin (C1) or Dolphin Milk (C2)? This affects every integration seam and must be settled before any handoff protocol code is written. This requires Jake's and John's agreement and may require changes to both projects.
- JOHN: Is Dolphin Milk's assistant UX mature enough for general conversational tasks (summarize this page, help me write this email, what did I read last week) — or is it primarily an x402 agentic task runner? The LESSONS_LEARNED experience (running Edwin for two weeks) was the evidence base for Option A's capability; Option B needs an equivalent hands-on session with Dolphin Milk before Matt can commit to it as the primary user-facing assistant.
- MATT: What is the v1 minimum assistant capability the casual user must experience? If it is 'full EdwinPAI with skills and recall', Option B is off the table. If it is 'agentic x402 tasks with basic Q&A', Option B is viable and Option A is over-engineered for v1.
- MATT: What is the memory budget constraint for the AI assistant subprocess on target hardware (e.g., 8GB RAM Windows 11 laptop)? Edwin Node sidecar with skills loaded vs. Dolphin Milk Rust binary are meaningfully different. If memory is a hard constraint, Option B wins on process homogeneity; if capability is the priority, Option A wins despite the footprint.
- MATT + JOHN: Is the 'x402 marketplace' in Dolphin Milk interoperable with the broader Coinbase x402 Foundation ecosystem (Base/Solana stablecoins), or is it a BSV-specific implementation of the HTTP 402 concept? The answer affects the monetization narrative and whether x402agency.com is a first-party-only marketplace or an open one.
- MATT: For v1 PoC/pitch scope, is there a hard deadline (e.g., the AWS competition window from INTEGRATION_RESEARCH_KICKOFF.md)? Option C's complexity may not fit a tight deadline. Options A and B both have cleaner v1 scope boundaries.

**Sources:** <ARCHITECTURE_TECHNICAL.md — C:\Users\archb\Hodos-Browser\development-docs\Dolphin Milk + Edwin Integration\ARCHITECTURE_TECHNICAL.md (the three-party design, PermissionEngine/envelope synthesis, BRC standards table, monetization §9 item 7)> · <EDWIN_NATIVE_PACKAGING_FINDINGS.md — C:\Users\archb\Hodos-Browser\development-docs\Dolphin Milk + Edwin Integration\EDWIN_NATIVE_PACKAGING_FINDINGS.md (transport-binding discovery, native dep surface, shad-core gap, packaging options)> · <LESSONS_LEARNED_EDWIN_INSTALL.md — C:\Users\archb\Hodos-Browser\development-docs\Dolphin Milk + Edwin Integration\LESSONS_LEARNED_EDWIN_INSTALL.md (casual-user requirements, WSL pain, what Edwin does well)> · <BROWSER_AI_IMPLEMENTATION_STUDY.md §H — C:\Users\archb\Hodos-Browser\development-docs\Dolphin Milk + Edwin Integration\BROWSER_AI_IMPLEMENTATION_STUDY.md (sidecar patterns H1-H5, prompt injection threat, x402 consent mechanism analysis)> · <UX_EDWIN_ASSISTANT_COMMUNICATION.md §5 — C:\Users\archb\Hodos-Browser\development-docs\Dolphin Milk + Edwin Integration\UX_EDWIN_ASSISTANT_COMMUNICATION.md (monetization research, x402 gap analysis, micropayment as consent mechanism)> · <x402 protocol — https://blockeden.xyz/blog/2025/10/26/x402-protocol-the-http-native-payment-standard-for-autonomous-ai-commerce/ (x402 ecosystem context: Coinbase-originated, Base/Solana-primary, 156K weekly transactions by early 2026)> · <x402 V2 — https://www.allium.so/blog/x402-explained-the-internet-native-payments-standard-for-apis-data-and-agent-commerce/ (V2 December 2025 features: reusable sessions, multi-chain support, service discovery)> · <Node.js SEA limitations — https://github.com/nodejs/help/issues/5129 (ESM + native addons + dynamic loading = SEA is not viable for Edwin; runtime-sidecar is the correct packaging approach)> · <Node.js SEA 2026 — https://www.hirenodejs.com/blog/nodejs-single-executable-applications-2026 (confirms SEA limitations with native addons requiring temp-file dlopen workarounds)>

### D3 — v1 UX form factor: how a casual user invokes and talks to Edwin inside Hodos

**Why it matters.** This decision sets every casual-user's first impression of Hodos's AI — discoverability, screen comfort, idle RAM hit, and how tightly Edwin's own UI versus a Hodos-native interface is exposed. It also gates D1 (bundling) and D2 (wallet transport): a side-panel approach requires Edwin to expose a localhost UI route; a full-page approach sidesteps that. Getting this wrong means either shipping too late (Options D, B with full multi-surface polish) or shipping something that casual users cannot find or that feels broken (Option E alone, or a panel with Edwin's unreformed desktop UI). The monetization UX (cost transparency, budget caps, x402 micro-payment consent) also lives here — the form factor determines whether Hodos can intercept and surface it clearly, or whether it is buried inside Edwin's UI.

#### Option: A — Persistent Native Sidebar Panel (Hodos-built chat UI, Edwin as HTTP backend)

*Mechanism.* Hodos C++ shell creates a dedicated CEF panel (not in the web-content area) docked to the right of the browser window — same overlay mechanism already used for the wallet panel. Hodos builds its own React or CEF-native chat UI inside this panel; the UI talks to Edwin via SSE stream and REST calls at localhost:<port>. Edwin is a backend only. Panel persists across tab navigation and collapses/pins. Edwin sidecar starts lazily on first panel invocation, not at browser launch.

**Pros:**
- Industry consensus form factor: Comet, Chrome Gemini sidebar, Edge Copilot (original), Brave Leo, Opera Aria all validated this as the primary AI pattern (BROWSER_AI_IMPLEMENTATION_STUDY.md §H3).
- Persistent context across tab switches — user can reference a page, navigate, then return to Edwin's conversation without losing it.
- Native OS panel cannot be blocked by ad-blockers or hidden by z-index conflicts; clear trust signal to users who recognize native UI.
- Hodos fully controls cost/payment transparency UX — the LESSONS_LEARNED §3 'cost guardrails' requirement is trivially met because Hodos owns the UI layer entirely.
- SSE streaming from Edwin's localhost port into the panel is standard HTTP — no new protocol work.
- Lazy-start pattern (ARCHITECTURE_TECHNICAL.md §8: Dolphin Milk agent as managed subprocess) is already planned and mitigates idle RAM concern.

**Cons:**
- Hodos must design and ship a full chat UI — not reusing Edwin's existing interface means more frontend work and the risk of shipping a weaker UX than Edwin already has.
- Panel consumes permanent screen real estate when open; Edge's May 2026 lesson (retired Copilot Mode → 18% idle RAM reduction) suggests persistent panels have a real user-perception cost. [FACT — ghacks.net, May 2026]
- Idle RAM: even with lazy-start, the Edwin sidecar process and the panel's CEF renderer both consume memory when the panel is open; must test and tune.
- Diverges from Edwin's roadmap UI — Jake updates Edwin's interface; Hodos has to track changes to Edwin's API contract (not its UI), then surface them in its own UI.
- No fallback to Edwin's existing UI — if Hodos's chat UI is incomplete at launch, the experience is entirely on Hodos to own.

**Dependencies.** Hodos frontend capacity to build a chat UI. Edwin's HTTP streaming API (already exists). Hodos's existing CEF overlay system (already exists for wallet panel). Lazy-start subprocess management (already planned per ARCHITECTURE_TECHNICAL.md).

**Best when.** Hodos has frontend bandwidth to build and maintain a native chat UI before v1 ships; the team wants full control over cost transparency, BSV payment consent UX, and Edwin's settings exposure (especially the 'cheap safe defaults' requirement from LESSONS_LEARNED §5); or Edwin's existing UI is judged inadequate for the casual-user bar.

**Effort/risk.** Medium-high. CEF overlay mechanics are already in Hodos; the new work is the chat UI itself. Risk: UI quality is Hodos's responsibility; launching with a thin chat UI vs. Edwin's fuller interface is a step backward in feature richness.

#### Option: B — Ambient Injection (omnibox NL handler + right-click + keyboard shortcut, no permanent panel)

*Mechanism.* No persistent panel. Edwin surfaces through three ambient entry points: (1) address-bar ML/heuristic router that detects natural-language input and routes it to Edwin instead of a search engine; (2) right-click context menu on selected text ('Ask Edwin about this'); (3) global keyboard shortcut that opens a transient floating panel, which dismisses after the interaction. Edwin sidecar starts on first invocation. No panel visible when idle.

**Pros:**
- Lowest idle RAM of all options — no persistent panel, no renderer overhead when Edwin is not active.
- Edge May 2026 real-world lesson directly supports this pattern: dissolving the persistent Copilot sidebar saved ~18% idle RAM and was motivated by users treating it as a 'separate thing they had to open' rather than a browser primitive. [FACT — ghacks.net, May 2026]
- Normalizes Edwin as a browser-native capability rather than a distinct app users consciously open.
- Lower visual noise for users who want AI only occasionally — consistent with Hodos's privacy-conscious, non-forced-AI positioning.

**Cons:**
- Hardest to discover for new users — the single most important requirement from LESSONS_LEARNED §5 is 'it just works on install'; ambient patterns require users to already know Edwin is there.
- No persistent conversation context visible across tab navigation — transient panel dismisses, context is lost.
- Omnibox ML router requires at minimum a heuristic intent classifier (URL vs query vs natural language) — this is non-trivial CEF extension work even at the heuristic level.
- Multi-surface implementation (three distinct entry points, each with its own UX) is highest engineering complexity of all options for the coverage achieved.
- Edge adopted this after years with a persistent sidebar — Edge users already knew about Copilot; Hodos v1 users have zero prior awareness of Edwin.

**Dependencies.** Omnibox extension API in CEF (requires testing that CEF exposes adequate hooks for NL routing). Right-click context menu extension (straightforward in CEF). Shortcut manager (already exists in Hodos). Edwin HTTP API for transient panel.

**Best when.** Hodos is building v2 or later, after v1 established user awareness of Edwin. Or as a complement layered on top of Options A/C/D — the ambient entry points add significant value as supplements, but not as the sole discovery mechanism.

**Effort/risk.** High for standalone deployment. Medium-low if implemented as a supplement to A, C, or D. Risk: if shipped alone, casual-user discoverability fails.

#### Option: C — Localhost-hosted SPA in CEF Side Panel (render Edwin's existing UI; the UX doc's 'likely v1')

*Mechanism.* Edwin's Node/native sidecar serves its existing React SPA at http://localhost:<port>/sidecar (or an equivalent route Jake exposes). Hodos loads this URL inside a dedicated CEF panel frame — a separate embedded browser frame from the user's main browsing profile, isolated from their cookies/session. The panel container is a thin shell; all UI is Edwin's. This is Comet's architecture (perplexity.ai/sidecar in Chrome's Side Panel API) but from localhost instead of the cloud. Edwin's full-page desktop UI can also be optionally opened in a Hodos tab via a 'Full page' expand button, giving both the panel convenience and the desktop richness.

**Pros:**
- Fastest time-to-ship: Edwin's existing UI is reused as-is. No Hodos frontend team needs to build a chat UI. UX doc §1 explicitly calls this 'the likely v1 surface.'
- Edwin's UI stays in sync with Jake's updates without Hodos rebuilding; Hodos's coupling is to Edwin's API contract, not its UI implementation.
- Rapid iteration: UI changes (Jake pushing updates to Edwin) do not require Hodos browser rebuilds — the panel just reloads the localhost SPA.
- Modern React/Tailwind tooling available for any UI extensions Jake adds.
- Full-page-localhost variant (open Edwin in a tab) is essentially zero-effort and gives the rich desktop experience described in UX doc §1 as a complement.
- Comet validated this architecture at production scale — the difference is localhost-served (private) vs. cloud-served (Perplexity's servers). [FACT — BROWSER_AI_IMPLEMENTATION_STUDY.md Appendix: Perplexity Comet]

**Cons:**
- Edwin's existing UI was designed as a desktop app — it may not be optimized for a narrow side-panel viewport. Jake may need to expose a sidecar-specific layout or responsive breakpoints.
- Embedded browser frame for the panel carries a CEF renderer process overhead: one extra process with its own heap for the panel.
- Same-origin considerations: the panel is loading from localhost, which is a different origin from Hodos's browser APIs. Cross-panel communication with Hodos C++ (for wallet payment consent, PermissionEngine notifications) requires an explicit IPC bridge.
- Hodos has limited control over Edwin's UX inside the panel — the 'cheap safe defaults,' cost transparency, and budget cap requirements (LESSONS_LEARNED §3) must be satisfied by Edwin's UI, not Hodos's. If Edwin's defaults are still misconfigured, the casual-user experience degrades.
- If Edwin's UI shows the 'empty tabs' / jargon-heavy status surfaces from LESSONS_LEARNED §4 (Sources tab reads missing YAML, Skills panel shows 0), the casual-user experience fails immediately. This is a risk Hodos cannot fully mitigate without Edwin-side fixes.
- UI branding is Edwin's, not Hodos's — visual identity of the AI feature is Jake's product, not Hodos's.

**Dependencies.** Jake must expose a sidecar-ready UI route (possibly /sidecar or a responsive layout flag). Hodos builds a thin panel frame container (straightforward CEF work). IPC bridge for payment/permission events from Edwin's localhost UI to Hodos's C++ shell. Edwin's UI must meet casual-user quality bar (LESSONS_LEARNED §4-5) — or Hodos must accept shipping with Edwin's current UX gaps.

**Best when.** Speed to v1 is the top priority; Jake's UI is trusted to be good enough (or will be fixed before Hodos ships); Hodos wants to minimize frontend investment and keep the coupling at the API layer. Also the right choice for an early demo or PoC — it gets something working immediately.

**Effort/risk.** Low-medium for Hodos. The panel container is thin CEF work. The real effort is on Jake's side (sidecar UI route, responsive layout, fixing LESSONS_LEARNED UX gaps). Risk: Hodos ships a casualuser experience that reflects Edwin's current setup/onboarding gaps, which are not fully in Hodos's control.

#### Option: D — CEF WebUI First-Party Panel (Brave Leo pattern, deepest Chromium integration)

*Mechanism.* Edwin's panel is implemented as a Chromium WebUI — an internal page (like chrome://settings or brave://leo) rendered in a privileged first-party context. Registration requires custom CEF/Chromium source patches to add the WebUI resource handler, controller, and bindings. The panel uses web technologies (HTML/CSS/JS) but runs at browser trust level with access to internal Chromium APIs unavailable to any web page. Brave Leo uses this architecture for its AI panel. [FACT — BROWSER_AI_IMPLEMENTATION_STUDY.md §H3, citing Brave GitHub issues]

**Pros:**
- First-party trust level: the panel cannot be blocked by content blockers, interfered with by web pages, or treated as a third-party web resource.
- Stable Chromium Side Panel API resize/pin/dismiss behavior — the panel behaves identically to Chrome's built-in side panels.
- Hodos fully owns the UI, the communication layer, and the payment/consent UX — no dependency on Edwin's UI quality or Jake's frontend choices.
- Tightest possible integration with Hodos's PermissionEngine and wallet IPC — the panel is inside the browser process trust boundary.

**Cons:**
- WebUI registration is not a simple CEF API — it requires custom Chromium source patches (or CEF extensions that are not standard). This is the most complex CEF integration of all options.
- Tight coupling to Chromium version: WebUI APIs can change between Chromium releases; every Hodos Chromium update risks breaking the panel.
- Slowest iteration speed: UI changes require a browser rebuild cycle, not just restarting Edwin's sidecar.
- Highest engineering cost of all five options; only justified if the team has deep Chromium/CEF expertise and long-term commitment to maintaining the integration.
- Brave is a much larger team with dedicated Chromium engineers; Hodos as a small shop may underestimate the maintenance burden.

**Dependencies.** Deep CEF/Chromium source expertise. Custom WebUI resource handler, controller, and bindings. Significant C++ development and testing effort. Hodos's Chromium version stability (CEF pinning strategy).

**Best when.** Hodos has dedicated CEF engineers and is explicitly targeting Brave-tier browser maturity as a v2 or v3 goal. Not appropriate as a v1 form factor for a small team. May be the right architecture to plan toward, with v1 shipping Option C and migrating to D as the team grows.

**Effort/risk.** High. Highest of all five options. Risk: significant scope creep; Chromium version coupling creates a long-term maintenance tax.

#### Option: E — Omnibox / Answer-Engine-First (address bar as primary AI surface)

*Mechanism.* The Hodos address bar is extended into a three-mode input: navigate (URL detection), search (query → search engine), and AI-answer (natural language → Edwin inline answer displayed in an omnibox dropdown or a dedicated results panel below the bar). Kagi's Quick Answer, Dia's tri-modal omnibox, and Atlas's address-bar handler are industry precedents. Edwin sidecar provides the answers; the UI is Hodos's omnibox extension. No separate panel at all — the answer engine IS the entry point.

**Pros:**
- Most natural browser interaction surface — users already use the address bar constantly; no new behavior to learn.
- Zero screen real estate overhead when idle — the omnibox collapses after use.
- Positions Hodos as an answer engine first, which is the direction every major player (Google AI Mode, Atlas, Comet) is converging toward.
- Highly discoverable — every user types in the address bar.

**Cons:**
- UX doc §1 explicitly states this is NOT iteration 1: 'Address/search bar (omnibox) — AI answer-engine-style interaction directly in the bar. Later; the big incumbents are pushing hard here.' This decision is already documented as deferred.
- Omnibox space is constrained — rich multi-turn conversations, payment consent dialogs, and streaming responses are poor fits for a dropdown below the address bar.
- No conversation persistence: each omnibox invocation is stateless; the context loss problem is acute.
- Requires replacing or competing with Hodos's existing search workflow without a complete substitute — the risk of breaking users' current search behavior is real.
- Complex CEF work: the omnibox is one of the least extensible parts of CEF without source patches; a full answer-engine takeover requires significant custom C++ work.
- Kagi's Quick Answer (closest analog) is available as a supplement to search, not a replacement — even the most search-forward AI browser keeps both.

**Dependencies.** CEF omnibox extension API (may require source patches). Intent classifier (NL vs URL vs query). Edwin API for inline answer generation. Integration with Hodos's existing search configuration.

**Best when.** v2 or later, layered on top of whichever panel option ships in v1. Or if Hodos explicitly decides to reposition as a search replacement product, which the UX doc explicitly defers. Not a standalone v1 form factor.

**Effort/risk.** High for standalone deployment. Medium if added as a supplement in v2. Risk: shipping as the sole entry point in v1 breaks casual-user discoverability and conflicts with users' existing search habits.

**Key trade-offs.** **Speed-to-v1 vs. UX ownership.** Option C (localhost SPA in panel) can ship the fastest because Edwin's UI is already built — but Hodos's control over the casual-user experience is entirely dependent on Jake's UI quality, especially the LESSONS_LEARNED gaps (misleading empty tabs, jargon setup, no cost guardrails). Option A gives Hodos full control but requires building and maintaining a chat UI. Option D gives the deepest integration but is a large team's work.

**Idle RAM vs. discoverability.** Edge's real-world lesson (18% RAM reduction from going ambient) is genuine data. But Edge users already knew Copilot existed for years before Microsoft dissolved the sidebar. Hodos v1 users have no prior Edwin awareness — the panel is the discovery mechanism, not just a convenience. Ambient-only (Option B) is the right v2 pattern, not v1.

**Edwin's existing UI as an asset or liability.** If Edwin's UI (the desktop app) has solved the casual-user problems from LESSONS_LEARNED (cheap defaults, no jargon, honest status surfaces), then Option C is a strong choice. If it has not — if the Sources tab still shows 'file not found,' if think levels are still opaque, if there are no cost guardrails — then Option C inherits those problems, and Option A (Hodos builds the UI) is the safer path for the Hodos brand.

**Full-page-localhost vs. in-browser chrome.** A full-page localhost tab (navigate to http://localhost:PORT in a Hodos tab) is essentially zero implementation effort and gives users Edwin's richest desktop UI. But it breaks the page-context relationship: the user leaves their browsing context to use Edwin, losing the ability to reference or summarize what they were looking at. An in-browser panel (Options A, C, D) keeps Edwin alongside the page. These are not mutually exclusive — a 'Full page' expand button on a side panel satisfies both.

**Payment/cost transparency ownership.** LESSONS_LEARNED §3 is unambiguous: casual users need cheap safe defaults and visible cost with budget caps. Whoever owns the UI owns this responsibility. Option A gives it entirely to Hodos. Option C gives it to Jake's Edwin UI (which currently lacks it). Options B and E (ambient/omnibox) have no natural home for persistent budget visibility. This is a non-trivial product decision, not just a UI detail — it affects Jake's development roadmap if Option C is chosen.

**Hodos overlay system maturity.** The wallet panel already uses the overlay/panel pattern. The incremental work to add an agent panel is architecturally small (ARCHITECTURE_TECHNICAL.md §2: 'Agent overlay' is explicitly listed as 'NEW — to be built'). The question is whether Hodos puts Edwin's UI or its own UI inside that overlay frame.

**Open questions:**
- Does Edwin currently expose a sidecar-ready UI route at localhost (a responsive or panel-optimized layout), or only the full desktop app UI? If the latter, Option C requires Jake to build that route before Hodos can use it. What is Jake's timeline and appetite for that work?
- Has Edwin's UI addressed the LESSONS_LEARNED §4-5 gaps — the Sources/Skills/Workflows tabs showing empty or misleading state on first launch, no cost guardrails, jargon setup flows — before Hodos ships? If not, Option C ships those gaps to Hodos's users under the Hodos brand, and Option A (Hodos-built UI) becomes the safer choice for user quality.
- What is Hodos's actual frontend team capacity for v1? If there is not dedicated bandwidth to build a polished chat UI, Option A's 'full control' advantage is theoretical; Option C's 'reuse Edwin's UI' is the only path that ships on time.
- Full-page tab vs. panel as primary surface: should v1 ship a panel (user can use Edwin while browsing) or is a 'navigate to localhost tab' acceptable as the first iteration, with a panel in v2? This is a product question about whether page-context awareness (summarize current page) is a v1 requirement or a later feature.
- Who owns the cost/payment transparency UX? If Edwin's UI does not surface cost per query, budget caps, and the BSV x402 consent event in a user-friendly way (LESSONS_LEARNED §3), does Hodos intercept and wrap it (which requires Option A), or does Hodos defer and ship without those guardrails in v1?
- For Options C and A: what is the IPC mechanism for Edwin's panel UI to trigger Hodos's native payment-consent modal (the PermissionEngine Prompt path from ARCHITECTURE_TECHNICAL.md §3)? The panel loads from localhost — it is a web origin. It cannot call Hodos C++ APIs directly. Does this go through a Hodos-registered custom scheme (hodos://), a localhost webhook, or a WebSocket from Edwin's sidecar to Hodos?

**Sources:** <C:\Users\archb\Hodos-Browser\development-docs\Dolphin Milk + Edwin Integration\BROWSER_AI_IMPLEMENTATION_STUDY.md §H3 (Options A–D form-factor mechanics, Comet SPA pattern, Brave Leo WebUI confirmation, Edge idle-RAM finding) — accessed 2026-06-26> · <C:\Users\archb\Hodos-Browser\development-docs\Dolphin Milk + Edwin Integration\UX_EDWIN_ASSISTANT_COMMUNICATION.md §1 (Matt's vision: full-page localhost as 'likely v1'; omnibox as 'later, not iteration 1') — accessed 2026-06-26> · <C:\Users\archb\Hodos-Browser\development-docs\Dolphin Milk + Edwin Integration\ARCHITECTURE_TECHNICAL.md §2 (Agent overlay as CEF subprocess rendering Dolphin Milk /ui/ in v1; managed subprocess pattern) — accessed 2026-06-26> · <C:\Users\archb\Hodos-Browser\development-docs\Dolphin Milk + Edwin Integration\LESSONS_LEARNED_EDWIN_INSTALL.md §3-5 (cost guardrails gap, empty-tab UX failures, casual-user requirements list) — accessed 2026-06-26> · <C:\Users\archb\Hodos-Browser\development-docs\Dolphin Milk + Edwin Integration\EDWIN_NATIVE_PACKAGING_FINDINGS.md §1, §3 (Edwin Node/ESM sidecar facts; no Tauri/Electron in main repo; localhost sidecar packaging approach) — accessed 2026-06-26> · <BROWSER_AI_IMPLEMENTATION_STUDY.md Appendix: Perplexity Comet — SPA at perplexity.ai/sidecar loaded in Chrome's native Side Panel API; SSE + WebSocket dual-channel [FACT, Zenity reverse engineering writeup cited in study]> · <ghacks.net, May 2026 — Microsoft Edge retires Copilot Mode, integrates AI into Edge chrome, reduces memory footprint ~18% [FACT cited in BROWSER_AI_IMPLEMENTATION_STUDY.md §H3 Option B and UX study §B Microsoft row]> · <UX_EDWIN_ASSISTANT_COMMUNICATION.md §5 Matrix row: Microsoft Edge — 'ambient not sidebar' post-May 2026; Brave Leo — 'Leo sidebar + Answer with AI in SERP'; Opera Neon — 'Persistent AI panel' [FACT rows, all sourced]>

### D4 — Search posture: blend vs AI-answer-first vs traditional-only

**Why it matters.** The omnibox is where users spend roughly 30% of their browser interactions. How Hodos routes that intent determines: (a) when Edwin gets invoked and with what quality bar, (b) whether the per-cite x402 micropayment model has a natural trigger point, (c) how privacy-conscious users perceive being "handled" by AI vs. given control, and (d) the gap between Hodos's "native AI browser" positioning and its v1 reality. Getting this wrong in either direction — forcing AI answers before Edwin quality justifies it, or hiding AI behind a traditional search box — either erodes user trust early or makes the core differentiator invisible. The decision also gates the publisher micropayment flywheel: per-cite payments only trigger naturally if the answer layer explicitly attributes sources, which is an architectural property of the search routing, not something that can be bolted on later without a redesign.

#### Option: Option A — Blend: three-mode omnibox with Edwin as an answer layer on top of search results

*Mechanism.* The CEF omnibox exposes three explicit modes — navigate (URL/domain), search (traditional results from a configurable provider such as Brave Search API or DuckDuckGo), and AI (Edwin answer). The default mode is user-configurable; no mode is forced. In the blend sub-variant, Edwin is also available as an answer layer on top of the traditional search results page: the browser feeds the query plus an AX tree snapshot of the SERP to Edwin via localhost IPC, and Edwin returns a synthesized summary with citations rendered in a panel above or alongside the results. Per-cite micropayment fires when Edwin synthesizes from a source page: the PermissionEngine checks if the cited domain has a BSV address, and an x402 payment triggers silently within the user's daily budget cap. Payments outside the cap get a prompt. Edwin is invoked only on explicit user action (AI mode or the answer-layer trigger), never ambient. The answer panel renders Edwin's response with numbered citations; users can click citations to visit the source page directly.

**Pros:**
- User control is native to the design — the three-mode toggle matches Dia's validated omnibox pattern and positions Hodos against Google's forced AI Mode without being anti-AI; DuckDuckGo's 30% install surge confirms this drives acquisition among the exact users Hodos targets
- Per-cite micropayment has the most natural trigger point of any option: Edwin explicitly names which sources it synthesized from, so the x402 payment fires on a clear, attributable event rather than requiring inference from AI output
- Uses the existing PermissionEngine + x402 architecture already designed in ARCHITECTURE_TECHNICAL.md; the payment-per-cite path is an extension of the existing envelope-gated payment flow, not a new system
- No proprietary search index required for v1 — Hodos delegates retrieval to a search provider (Brave Search API, DuckDuckGo) and Edwin layers synthesis on top; the dependency is a search API call, not a crawler/index build
- Fallback is always available: if Edwin gives a poor answer, the user's traditional search results are right there; the blend mode does not remove the SERP, it adds to it
- Matches the industry convergence point (§5 D of UX study): every privacy-forward player — Brave, Kagi, DuckDuckGo, Firefox — keeps the traditional search box as a first-class path while adding AI as a parallel option; Hodos is in good company architecturally
- Privacy signal is strong: AI invocation is explicit, page context is only shared with Edwin on trigger (matching H2 Option E from BROWSER_AI_IMPLEMENTATION_STUDY.md), and the payment ledger gives users a readable audit trail of what AI did on their behalf

**Cons:**
- More v1 complexity than traditional-only: three omnibox modes require routing logic in the CEF shell, a UI affordance for mode display/switching, and user education; risk that the mode-switching UX confuses non-technical users before it becomes intuitive
- Edwin's answer quality over search results depends entirely on what context it can access — without a real-time web crawler, Edwin synthesizes from the SERP page content (AX tree snapshot), which is shallower than fetching and reading the actual source pages; answer quality ceiling is lower than Perplexity or Atlas
- Per-cite micropayment bootstrapping problem: most source pages do not have BSV addresses in v1, so the payment fires to zero or few sites and the model's killer feature is invisible until publisher adoption grows (the Maxthon VPoint failure mode)
- Search provider dependency: the 'search' mode requires a configurable third-party search API (Brave Search, DuckDuckGo, Kagi); pricing/availability changes at that provider are a business dependency Hodos does not control
- Three-mode UX requires more design polish than a simple search box; non-technical users may not understand which mode they are in or why their query produced an AI answer vs. links

**Dependencies.** Configurable search provider API for the 'search' mode (Brave Search API is the best fit given Brave's B2B API growth and independent index). Edwin sidecar running natively on Windows/Mac (Decision 1, requires Jake's native builds). Edwin to accept query + optional SERP context via localhost IPC and return structured answer + citations (new protocol on Edwin's existing HTTP API, requires Jake alignment). PermissionEngine extended to recognize 'AI cite' events as payment triggers. BSV address discovery mechanism for cited pages (may start as a no-op in v1, firing only when BSV address is present in page metadata).

**Best when.** v1 ships before Edwin answer quality can support being the primary search surface; Hodos's target users are privacy-conscious but want control over when AI answers, not a forced AI-first experience; the publisher micropayment ecosystem is early and needs to grow organically; engineering capacity for v1 is constrained and a reliable fallback is needed; Matt wants to position Hodos as 'AI on your terms' rather than 'AI instead of search'.

**Effort/risk.** Medium. New C++ work in the CEF shell for omnibox routing logic and mode-switching UI. New Edwin IPC protocol for query + context ingestion and structured answer + citation response. Extension of PermissionEngine to recognize per-cite payment events. The per-cite payment path reuses existing envelope-gated x402 architecture — no new payment system. Estimated: 3-6 weeks of new browser work on top of the already-planned Edwin sidecar integration, plus Jake alignment on the Edwin API surface.

#### Option: Option B — AI-answer-first: Edwin is the primary query surface (Comet/Atlas style)

*Mechanism.* The omnibox's primary action for any non-navigational input is Edwin. The CEF shell runs a lightweight heuristic classifier (or small local model) on each omnibox entry: URLs and domain-shaped inputs trigger direct navigation; everything else routes to Edwin via localhost IPC. Edwin returns a synthesized answer with cited sources, rendered as the primary browser content (full-page answer panel or an answer overlay over a minimal background). Traditional search is available via an explicit toggle, secondary button, or keyword prefix ('search: ...' or pressing Tab) but is not the default. Per-cite micropayments fire from Edwin's citation output: for each source page Edwin names, the PermissionEngine checks for a BSV address and triggers an x402 payment. Per-query AI cost (Edwin's inference via x402) is also billed at the moment of answer. Results render as: Edwin answer panel (primary), with a 'See traditional results' affordance secondary.

**Pros:**
- Strongest product differentiation: Hodos does not look like a browser with AI bolted on; the AI is the answer surface, which is the market trajectory Perplexity ($20B valuation, $500M ARR) and OpenAI Atlas have validated at commercial scale
- Cleaner UX for users who have already adopted AI-first search: no mode switching, no decision about when to invoke AI vs. use the search box
- Per-query micropayment model is simpler and more consistent than blend: every AI query bills once for inference, no ambiguity about when to fire a payment vs. when not to
- Forces a clear, differentiated value proposition from day one: 'this browser gives you answers, not links' is a memorable and distinctive claim
- Eliminates the search provider dependency: if Edwin handles all queries, Hodos is not dependent on DuckDuckGo or Brave Search API pricing or availability
- The citation-attribution payment model has a stronger story here: every Edwin answer contains explicit citations, and those citations are the payment events — the model is more legible than blend where some queries go to search (no payment) and some go to Edwin (payment)

**Cons:**
- Edwin's current capability is as a chat assistant / agent over local and cloud AI, not a real-time web search engine with a crawler and index; positioning it as the primary answer surface before that quality is there will produce unreliable or stale answers for time-sensitive queries, and a bad first impression on the browser's primary action is very hard to recover from
- No proprietary real-time index means Edwin must either (a) call a search API to fetch fresh context (re-introducing the provider dependency), (b) rely on the model's training cutoff (answers become stale), or (c) use Edwin's AX tree snapshot of the user's current page (only works for page-context queries, not novel questions) — all three have quality ceilings that Perplexity and Atlas solve with hundreds of millions in crawler infrastructure
- DuckDuckGo's 30% install surge post-Google I/O and Firefox Nova's AI kill switch demonstrate that even the privacy-forward user base includes a meaningful segment that resists AI-default search; forcing AI-first without an escape hatch risks alienating users who came to Hodos for control
- Legal exposure: the Amazon injunction against Perplexity (March 2026) established that AI-first summarization of third-party content at scale carries CFAA risk; this risk is real when AI answers replace rather than supplement the visit to the source page
- Navigational queries ('go to amazon.com', 'open my bank') handled by an AI classifier introduce failure modes and latency that do not exist in a traditional omnibox; misclassification feels broken to users
- Citation attribution from AI synthesis is technically harder than from explicit link clicks: Edwin must reliably report which source URLs it drew from for each sentence of its answer in order to fire the correct per-cite micropayment; this is a structured output requirement on Edwin's API that does not currently exist
- If Edwin quality is insufficient in v1 but the AI-first posture has already been shipped, fixing it requires a product pivot (reverting the default) which damages trust more than never having shipped AI-first in the first place

**Dependencies.** Edwin must reliably handle arbitrary web queries at a quality bar appropriate for primary search — this requires either real-time web access (calling a search API for fresh context, which re-introduces provider dependency) or a strong pre-indexed knowledge base. Edwin's citation output must be structured (source URL + sentence-level attribution) for per-cite payment attribution. A reliable lightweight query classifier in the CEF shell (navigational vs. AI) — heuristic-based is feasible, ML-based adds complexity. Jake to expose a 'search query' API on Edwin that accepts a natural language question and returns answer + structured citations. Content freshness strategy is a prerequisite to shipping AI-answer-first as the default.

**Best when.** Edwin answer quality has been validated on a representative sample of the queries real Hodos users will issue; a real-time data freshness mechanism (search API integration or live web fetch in Edwin) is in place; the publisher micropayment ecosystem has matured enough that citation-attribution payments can be verified in practice; Hodos is positioned as a full AI-browser product (not a general browser with AI features) and the target user cohort has fully shifted to AI-native searchers.

**Effort/risk.** High. Requires a query classifier in the CEF shell (new ML/heuristic subsystem), content freshness mechanism for Edwin (either a search API integration inside Edwin or a live-fetch layer), structured citation output from Edwin (new API contract with Jake), per-cite payment attribution from AI synthesis output (harder than from explicit link click), and an answer-rendering UI that is not a traditional SERP. The quality and reliability bar to ship AI-first as the default is also significantly higher than for blend, meaning more testing and iteration before v1. Rough estimate: 3-4 months of additional work beyond what is planned, plus Jake's buy-in on the Edwin API surface.

#### Option: Option C — Traditional-search-only for v1, defer the answer engine

*Mechanism.* v1 ships a conventional omnibox routing to a configurable search provider (DuckDuckGo default, user-selectable: Brave Search, Kagi, Google). No AI integration in the search/omnibox flow. Edwin is accessible as a separate surface via the planned agent overlay, a keyboard shortcut, or a toolbar button — but the two are siloed: search results and Edwin chat are independent. Users who want AI answers open the Edwin overlay and ask there; the search box is just a search box. Micropayments are deferred from the search flow entirely; they apply to Edwin overlay interactions (per-query AI billing via x402) but do not trigger from search results. Traditional SERP behavior: provider's result page loaded in the main browser tab.

**Pros:**
- Lowest v1 complexity and fastest path to a working browser — traditional search is standard browser infrastructure; no new architecture required beyond search provider URL configuration
- Zero risk of Edwin quality problems affecting the primary browser action; users judge the browser on its browser behavior, and AI is a bonus feature they discover separately
- DuckDuckGo's 30% install surge and Vivaldi's 140% Norway growth prove that a traditional search browser with strong privacy credentials grows a user base without AI in the search flow; this option leaves the acquisition lane open
- Preserves optionality: the blend or AI-first posture can be added in v1.1 or v2, once Edwin quality is validated on real user queries and the publisher micropayment ecosystem has some traction
- No search provider API dependency beyond a configurable URL; no new IPC protocol for Edwin query ingestion; no citation-attribution complexity in v1
- Sidesteps the AI-answer legal exposure (summarization without explicit consent) entirely for v1
- Casual-user principle from LESSONS_LEARNED: if the Edwin AI overlay is complex or unreliable, users still have a working browser; the failure mode of AI is isolated from the failure mode of the browser

**Cons:**
- The most significant con: Hodos positions itself as a 'native AI browser' but the primary browser interaction surface (the search box) contains no AI in v1 — the positioning and the product are misaligned at launch, which confuses users and weakens the story
- Traditional search in 2026 = searching an index that is already being disrupted by AI Mode; shipping a traditional-only search box into a market where Google, Brave, Kagi, DuckDuckGo, and Atlas all have AI in the search flow is a deliberate decision to look like a 2023 product
- No per-cite micropayment trigger in the search flow means the core monetization model's most natural driver (AI cites a page, micropayment fires) does not activate from the primary user action; micropayments are only possible in the Edwin overlay (lower user session volume)
- The 'Edwin as separate overlay' model is the pattern Edge Copilot Mode used from 2023-2025 — Microsoft retired it in May 2026 explicitly because users treated it as a separate thing rather than a browser primitive; shipping this pattern in late 2026 is shipping a deprecated UX design
- No search deal revenue at niche scale: the revenue from search default deals (Mozilla's model) requires meaningful query volume to be attractive to providers; a small browser cannot negotiate meaningful search deal revenue; this option has no incremental revenue model on top of the Edwin overlay micropayments

**Dependencies.** Minimal. Configurable search provider URL (Brave Search, DuckDuckGo, etc. — no API key required for web search redirect). Edwin as a separate overlay (already planned). No new dependencies introduced by this option. The incremental dependency is the search provider redirect URL, which is a one-line configuration.

**Best when.** v1 must ship fast and the Edwin integration is not yet reliable enough to be in the primary search path; the team wants to validate browser UX (install, startup, wallet, ad-block) before layering AI into the search flow; Edwin quality cannot be vouched for on arbitrary user queries yet; or the decision is to position v1 as a privacy browser first and an AI browser in v1.1.

**Effort/risk.** Low. Traditional search is standard browser behavior. The only new work relative to an existing browser is the provider-selection UI in settings and the default provider choice. Edwin as a separate overlay is already planned regardless of the search posture decision. Estimated incremental effort: days, not weeks.

**Key trade-offs.** 1. QUALITY CEILING vs. DIFFERENTIATION. Option B puts Edwin in the primary search role before a real-time index or content freshness mechanism exists. If Edwin gives stale or wrong answers to common search queries, the browser's most important action is broken and first impressions are very hard to recover. Option A manages this by keeping traditional search as the reliable ground truth while Edwin layers synthesis on top. Option C avoids the risk entirely but makes the 'native AI browser' claim hollow in v1.

2. USER CONTROL vs. AI BOLDNESS. The research shows that forced AI intermediation (Google AI Mode) drove DuckDuckGo installs up 30% among exactly Hodos's target users. But Hodos is pro-AI done privately — so the real question is not 'AI or no AI' but 'who decides when AI answers.' Option A makes user control the design principle. Option B bets that users who install a browser called Hodos want AI-first. Option C defers the bet entirely. None is wrong; they serve different user segments within the privacy-conscious-but-pro-AI category.

3. MICROPAYMENT TRIGGER NATURALNESS. The per-cite x402 payment model fits Option A most naturally: Edwin explicitly names sources from a traditional SERP, and the payment fires on a clear attribution event. In Option B, citation attribution from AI synthesis must be structured output from Edwin (harder, requires Jake API work, more failure modes). In Option C, there is no per-cite trigger in the search flow at all. The publisher-side adoption problem (most pages lack BSV addresses in v1) affects all three options equally, but the trigger architecture matters for when the flywheel can actually start.

4. ARCHITECTURAL DEPENDENCY ON JAKE. Options A and B both require Edwin to accept queries via a new IPC/HTTP protocol and return structured answers with citations — new API surface Jake must build or approve. Option C requires nothing from Jake for the search posture specifically. If Jake's bandwidth is constrained or his refactor (qmd backend, pruned extensions) is mid-flight, Options A and B gate on his availability.

5. POSITIONING COHERENCE vs. SHIPPING SPEED. Option C is the fastest to ship and the least risky technically, but risks shipping a product where the AI claim is invisible in the primary user flow. Option B ships the boldest AI-first claim but risks shipping it before it's ready. Option A threads the needle at medium cost, but requires more design work to make the three modes feel coherent and intuitive for non-technical users.

6. TRAJECTORY LOCK-IN. The search posture decision affects how the browser feels to users from day one. Moving from C to A (adding blend) is straightforward. Moving from A to B (making AI the default) requires validating quality first, then shipping a default-change (usually a minor update). Moving from B back to A (if AI-first fails) is a more visible product retreat. The ordering C → A → B is the lowest-risk trajectory; the ordering B → A if B fails is highest risk to user trust.

**Open questions:**
- Jake: Does Edwin currently have a 'search query' API — structured natural language question in, structured answer + source URLs out? If not, what is the effort to add one? Options A and B both require this interface; Option C does not.
- Jake: What is Edwin's current data freshness mechanism for queries about recent events? Does it call an external search API for live results, or does it rely on the underlying model's training cutoff? This directly determines whether Option B (AI-answer-first) is viable for general web queries in v1.
- Matt: What search provider does Hodos intend to use for the 'search' mode in Option A, and is there a budget for a paid search API (Brave Search API at $5/1000 requests, Kagi API, etc.)? DuckDuckGo's HTML endpoint is free but rate-limited and not production-reliable at scale.
- Matt: What is the minimum quality bar for Edwin answers before they can appear in the primary omnibox flow (either as the default in Option B, or as the AI-mode result in Option A)? Who evaluates this and how — informal testing, a benchmark, or user feedback in a private beta?
- Matt: For the per-cite micropayment model — what is the plan for discovering whether a source page has a BSV address? Is there a standard metadata field (e.g., <meta name='bsv-address'>) that Hodos looks for, or does it depend on publisher opt-in via some other mechanism? This determines when the payment flywheel can actually fire.
- Jake/Matt: The Amazon injunction against Perplexity (March 2026, U.S. District Court N.D. Cal.) established that AI summarization of third-party content at scale carries CFAA risk when it displaces the user's visit to the source. Does Hodos's legal picture for Option B (AI-answer-first) account for this? Option A (Edwin on top of search, with source links prominent) is structurally lower risk than Option B (Edwin as the answer, SERP optional).
- Matt: Is the v1 target a privacy-focused general user (who expects a search box to work like a search box) or an AI-forward early adopter (who wants the answer engine experience)? The right answer affects whether Option A or B is the correct opening posture — both can be justified for Hodos's audience, but they serve different points on the pro-AI/privacy spectrum.

**Sources:** <UX_EDWIN_ASSISTANT_COMMUNICATION.md §5 C (Search-to-Answer Trajectory) and §5 D (Form-Factor Consensus) and §5 F(1) (Replace Search Now/Later/Blend?) — research cutoff 2026-06-26 — internal Hodos planning doc> · <UX_EDWIN_ASSISTANT_COMMUNICATION.md §5 B (Monetization-Disruption Thesis) — Mozilla Coil experiment: $3 in 10 months on 1.1M pageviews; Maxthon VPoint near-zero adoption; x402 as structural gap — all [FACT]-tagged with inline citations> · <UX_EDWIN_ASSISTANT_COMMUNICATION.md §5 A (At-a-Glance Table) — Perplexity $500M ARR, DuckDuckGo 30% install surge post-Google I/O, Dia three-mode omnibox, Google AI Mode global default May 2026 — all [FACT]-tagged> · <BROWSER_AI_IMPLEMENTATION_STUDY.md §H3 (How the Assistant UI Should Surface) — three omnibox mode options, sidebar vs. ambient, localhost SPA pattern> · <BROWSER_AI_IMPLEMENTATION_STUDY.md §H2 (How CEF Should Feed Page Context to Edwin) — AX tree snapshot via CEF CDP as recommended mechanism for query + context ingestion> · <BROWSER_AI_IMPLEMENTATION_STUDY.md §H5 (Privacy-Broker Pattern) and Cross-Cutting Recommendations — prompt injection risks, UI conflation anti-pattern, x402 as consent mechanism> · <ARCHITECTURE_TECHNICAL.md §2 (Process layout), §3 (Request flow + PermissionEngine sequence diagram), §4 (PermissionEngine + envelope synthesis) — Hodos CEF shell architecture, three-subprocess model, envelope-gated payment flow> · <INTEGRATION_RESEARCH_KICKOFF.md §3 (Open decisions), §4a (UI/UX options) — Decision 3 framing, omnibox keyword agent: pattern already sketched> · <LESSONS_LEARNED_EDWIN_INSTALL.md §5 (What casual users concretely require) — cheap safe defaults, no jargon, honest status surfaces> · <https://www.cnbc.com/2026/03/10/amazon-wins-court-order-to-block-perplexitys-ai-shopping-agent.html — Amazon injunction against Perplexity March 2026 (CFAA risk for AI-answer-first)> · <https://techcrunch.com/2026/05/26/duckduckgo-installs-are-up-30-as-users-reject-being-force-fed-googles-ai-search/ — DuckDuckGo 30% install surge post-Google I/O AI Mode> · <https://www.coindesk.com/markets/2026/03/11/coinbase-backed-ai-payments-protocol-wants-to-fix-micropayment-but-demand-is-just-not-there-yet — x402 adoption state March 2026 (119M transactions on Base, $600M annualized volume, demand-side challenge)> · <https://aws.amazon.com/blogs/industries/x402-and-agentic-commerce-redefining-autonomous-payments-in-financial-services/ — x402 agentic commerce architecture overview> · <https://searchenginejournal.com/perplexity-launches-comet-plus-shares-revenue-with-publishers/554596/ — Perplexity $42.5M publisher pool (batch settlement vs. per-interaction x402)> · <https://techcrunch.com/2026/05/19/google-search-as-you-know-it-is-over/ — Google AI Mode global default May 2026>

### D5 — Monetization model: micropayment-only vs. subscription vs. both in parallel

**Why it matters.** This decision determines Hodos's revenue model, ops sustainability, and whether the BSV/x402 differentiator is real or theoretical. It gates: (1) what Hodos can fund and staff from day one, (2) how the PermissionEngine budget-cap UX is designed, (3) whether publishers are paid per-cite (the structural gap no incumbent fills) or whether that remains a roadmap promise, (4) how the Edwin treasury split flows to Jake, and (5) whether casual users can onboard without understanding crypto at all. Getting this wrong in either direction — micropayment purity that never finds publishers, or subscription-only that abandons the architectural differentiator — leaves Hodos in Maxthon's position: correct technology, no commercial ecosystem.

#### Option: A — Micropayment-only (x402/BSV per-query, per-cite, per-agent-action)

*Mechanism.* Every AI inference call, every publisher citation, and every agent action is individually priced and paid via a BSV satoshi micropayment at HTTP-layer (x402). The user loads a BSV wallet at browser install (one-time or recurring top-up). PermissionEngine's existing Silent/Prompt/Deny budget-cap logic (daily-cap, per-task-cap) governs each payment without requiring per-click user approval. EnvelopeSpec from the PermissionEngine includes the per-request amount; the Hodos wallet signs and broadcasts it. Display is fiat-denominated ('$0.001/query') with BSV settlement at current rate via a price oracle already in the wallet layer. Publisher per-cite payments are sent directly to a BSV address embedded in x402 headers on the publisher's page — no enrollment, no pool, no 90-day settlement. LLM inference is paid to x402agency.com endpoints (already confirmed BSV-compatible via Dolphin Milk routing). The Envelope-aware fee split (ARCHITECTURE_TECHNICAL.md §2) routes a portion to Edwin treasury on each agent-authorized transaction.

**Pros:**
- Purest structural differentiator: the only browser model that pays publishers per-cite in real time with no enrollment, no pool, no intermediary rake — fills the precise gap Perplexity's $42.5M batch pool and BAT's 8-year failed creator-registration wall leave open
- Payment IS the consent mechanism: x402 payment replaces 'are you sure?' dialogs for agent actions — cryptographic, auditable, unforgeable, and already architected into the envelope flow
- Zero monthly commitment for casual/occasional users — pay only what you actually use, which is the Kagi 'no use no pay' principle taken to its logical sub-cent extreme
- On-chain audit trail (BRC-18 + envelope hash) of every agent action, every inference call, every publisher payment — verifiable forensically, the strongest sovereignty claim in the industry
- Aligns incentives with BSV's technical thesis: sub-cent, instant, permissionless HTTP payments at the protocol layer — distinguishable architecturally from Brave BAT (attention token), Opera MiniPay (P2P stablecoin), and Perplexity publisher pool
- PermissionEngine + EnvelopeSpec already designed to support per-query authorization — minimal new plumbing if the Silent path is tuned correctly
- Publisher revenue starts flowing the moment a publisher adds a BSV address to their x402 header — no account creation, no custodial partner, no KYC wall (unlike BAT)

**Cons:**
- Szabo's mental transaction costs: even invisible micropayments carry psychological weight if users sense money is leaving on every click — the UX problem is behavioral, not just technical, and 25 years of attempts have not solved it without budget-cap abstraction
- Two-sided cold-start: publishers must have BSV addresses and serve x402 headers for per-cite payments to work; if publishers don't participate, the per-cite story is fiction — Maxthon shipped VPoint in 2020, the loop never closed in six years; BAT failed this loop after eight years and 100M MAU
- x402 Foundation (Coinbase/Anthropic/AWS/Google/Vercel) implements EVM stablecoins (USDC on Base/Polygon/Solana) — BSV is NOT in the foundation's reference implementation; Hodos would be maintaining a parallel BSV x402 implementation off the main industry rails, limiting which AI providers can be reached natively via x402/BSV
- Revenue unpredictability for Hodos: zero subscription floor means ops costs (infrastructure, engineering, support) must be funded from per-query margins — if LLM inference eats most of each payment, Hodos margin is thin and volatile
- BSV price volatility: even with fiat-denominated display, a BSV price crash between top-up and spend creates wallet-value anxiety; a BSV price spike makes existing wallet balances unexpectedly valuable but future top-ups expensive
- Envelope overhead per query: every micropayment requires envelope issuance, signing, TTL, nonce-replay guard — 20 queries in a session = 20 envelope operations; the Silent path is fast but crypto overhead is non-zero vs. subscription's single monthly transaction
- The '$20 lesson' risk: without tight defaults and prominent caps, users can drain a wallet faster than expected — casual users have no intuition for satoshi-level burn rates even with fiat display
- LLM provider BSV support is currently mediated through x402agency.com; direct x402/BSV integration with Anthropic, OpenAI, etc. is not documented as available — Hodos is dependent on a third-party intermediary for the core inference payment rail

**Dependencies.** Jake must publish Win/Mac native identity-core binaries (envelope signing path); x402agency.com must sustain BSV support for LLM inference routing; publisher cold-start requires a specific vertical where publishers already have BSV addresses (1Sat Ordinals content is the candidate); BSV price oracle integrated into wallet for fiat-denominated display; PermissionEngine budget-cap defaults tuned for per-query micropayment safety

**Best when.** Hodos's launch user base is concentrated BSV-native early adopters who already have funded wallets and understand the payment model; a specific content vertical (1Sat Ordinals, BSV DApp ecosystem) provides the publisher-side bootstrap so per-cite payments are real immediately; the team can sustain on thin per-query margin while publisher adoption grows; the focus is proving the architecture is right, not scaling to casual users in year one

**Effort/risk.** Medium-High. The per-query envelope + x402/BSV payment path is already partially designed in ARCHITECTURE_TECHNICAL.md. The hard new work is: (1) fiat-denominated display with price oracle, (2) publisher-side x402 header discovery and payment routing, (3) PermissionEngine budget-cap defaults tuned for micropayment safety rather than just security, (4) wallet top-up UX with clear burn-rate display. Publisher cold-start is a go-to-market problem, not an engineering one, but it is the dominant risk.

#### Option: B — Subscription (fixed monthly BSV deposit / flat tier)

*Mechanism.* User pays a fixed monthly fee (e.g., $10 USD equivalent) billed in BSV from their Hodos wallet at renewal. Fee is deposited into a 'credits bucket' tracked in the wallet. Hodos operates a privacy-preserving proxy (Brave Leo pattern) that pays LLM providers in bulk at negotiated rates — not via per-query x402 micropayments to individual endpoints, but via periodic settlement. From the user's perspective: unlimited Edwin queries up to tier limits; no per-query decisions. BSV-denominated renewal auto-triggers from wallet at month boundary (a single periodic payment, not thousands of micropayments). Fiat-pegged amount ('$10/month equivalent in BSV at current rate') smooths BSV volatility at the billing event. PermissionEngine budget caps remain as a secondary security layer (daily spending limits on agent actions) but are invisible for standard queries. Edwin treasury split applies to the subscription fee directly: e.g., 60/40 Hodos/Edwin per ARCHITECTURE_TECHNICAL.md §9.

**Pros:**
- Proven at scale: Kagi profitable at $10-25/month with ~37-person team at 72K subscribers; Brave Leo Premium at $14.99/month contributes to $100M ARR; DuckDuckGo Privacy Pro at $9.99-$19.99/month — the model is validated for privacy-conscious AI users
- Eliminates Szabo's mental transaction cost problem entirely at the user layer: one monthly decision instead of thousands of per-query micro-decisions — subscription is the historically proven solution to micropayment friction
- Revenue predictability: Hodos can plan infrastructure, staff, and Jake's treasury split around a recurring BSV subscription floor rather than per-query margin variability
- Easiest casual-user onboarding: 'it's like Kagi for AI, $10/month' is a mental model non-crypto users immediately understand; BSV is the payment rail but the user experience is identical to any SaaS subscription
- Hodos proxy model allows contractual no-train guarantees with LLM providers negotiated in bulk — Brave Leo's model shows privacy-conscious users trust this pattern
- Not dependent on publisher BSV adoption: publisher cold-start problem is deferred; publishers receive nothing directly in v1 (can be added later without changing the billing model)
- Single monthly wallet transaction reduces envelope/signing overhead to one operation per renewal cycle, minimizing Jake's identity-core dependency in the critical path

**Cons:**
- Abandons the per-cite publisher payment differentiator entirely: Hodos takes the subscription fee, publishers receive nothing directly, and the structural gap that makes Hodos architecturally unique (vs. Perplexity's batch pool) is not filled — the BSV native story becomes 'we use BSV to collect subscriptions'
- Competitive disadvantage against well-funded incumbents: competing on subscription price/features against Brave Leo ($14.99), DuckDuckGo Pro ($9.99-$19.99), and Kagi ($10-25) requires matching their model quality, proxy infrastructure, and support — all require significant capital that Hodos as a small team may not have
- Requires Hodos to operate and maintain a cloud proxy infrastructure similar to Brave's model — operational burden that Brave (100M MAU, $100M ARR) can absorb but a seed-stage browser team cannot easily sustain
- BSV is just a payment rail for a known model: no architectural differentiation from paying the same subscription in any other crypto — the wallet's unique x402/envelope capabilities go unused for the primary revenue stream
- Churn risk as AI subscription market saturates: users with subscriptions to ChatGPT Plus, Claude Pro, and Kagi already may resist a fourth AI subscription for a browser they're newly switching to
- Tier confusion: does the subscription cover agent actions, or do expensive agent tasks (multi-step research, purchases) cost extra? If extra, the mental-transaction-cost problem returns for the premium use cases
- Delayed publisher ecosystem: if Hodos runs subscription-only for two years, publisher payment infrastructure is never built, making the micropayment differentiator a permanent future roadmap item rather than a shipped product

**Dependencies.** BSV-denominated subscription billing with auto-renewal from wallet; Hodos proxy infrastructure (or negotiated bulk rate with Dolphin Milk/x402agency.com); contractual no-train agreements with LLM providers; tier enforcement logic in the wallet credits system; NO dependency on publisher BSV adoption; Jake identity-core binaries needed for initial wallet setup but not per-query signing

**Best when.** Hodos's early user base extends beyond BSV-native users to privacy-conscious mainstream users unfamiliar with crypto mechanics; the team prioritizes revenue predictability and infrastructure sustainability over architectural purity; publisher cold-start cannot be solved in year one and a working product is more important than a complete vision; direct competition with Kagi/Brave on their own terms is the strategic choice

**Effort/risk.** Medium. Subscription billing is known engineering. BSV-denominated renewal with fiat-pegging is slightly novel but tractable. The main complexity is operating a proxy infrastructure — if Hodos instead negotiates a bulk rate with x402agency.com and batches the per-query costs, proxy ops can be deferred. Edwin treasury split on subscriptions requires a simple percentage calculation at renewal time.

#### Option: C — Both in parallel (subscription funds ops + micropayment as differentiator)

*Mechanism.* Two co-existing tracks: (1) Subscription tier ('Hodos AI Standard'): $8-12/month BSV equivalent. Covers all standard Edwin inference queries. Hodos pays LLM providers in bulk. Casual users never see a per-query payment. The subscription provides the revenue floor for ops. (2) Micropayment overlay ('Hodos AI Pay-as-you-go' and publisher payments): non-subscribers pay per-query via x402/BSV micropayments with a pre-loaded wallet balance. Subscribers receive a 'publisher payment' option: when Edwin cites a BSV-enabled publisher, the wallet can optionally send a per-cite micropayment to that publisher's BSV address — even subscribers can opt in to fund the publisher ecosystem. Agent actions above the subscriber's included tier (expensive multi-step research, purchases) trigger x402 micropayments for the overage. Brave's dual-stack model (Leo Premium + BAT creator tipping) is the prior art. Edwin treasury split: subscription fee splits on renewal (60/40); micropayments split on each envelope-gated transaction. PermissionEngine budget caps govern both tracks: subscription provides the monthly cap for standard use; micropayments are individually capped per request per the existing Silent/Prompt/Deny flow.

**Pros:**
- Brave's own model proves dual-stack is commercially viable: Leo Premium ($14.99/month) plus BAT creator payments co-exist for 100M MAU without confusing the core user base — the subscription is the product, BAT is the architecture
- Subscription revenue floor funds ops and infrastructure; micropayment track funds the publisher ecosystem bootstrap separately — neither track depends on the other to generate cash in year one
- Different user segments served simultaneously: casual users buy the subscription and never think about micropayments; BSV-native power users load a wallet and pay per-query; privacy maximalists who distrust subscriptions use pay-as-you-go micropayments
- Publisher cold-start can begin with the optional 'fund publishers when Edwin cites them' feature, even at subscriber scale — gives publishers a reason to add BSV payment headers without requiring user adoption of micropayment-first billing
- Micropayment track provides real UX for the x402/BSV differentiator without requiring it to be the only revenue model — can grow alongside subscription rather than replacing it
- PermissionEngine budget caps serve dual purpose: security layer for agent actions (existing design), AND user-facing spending control for micropayment track (same logic, additional UX surface)
- Edwin treasury split flows on both tracks: subscription renewal gives Jake a predictable monthly income, per-query micropayments give Jake a per-action income aligned with Dolphin Milk's x402 architecture
- Kagi's 'no use no pay' fairness principle can be implemented for pay-as-you-go users; subscription users get a monthly value guarantee; two fairness models for two segments

**Cons:**
- Two billing systems to build, maintain, and support: subscription management (renewal, tier enforcement, credit tracking) AND micropayment wallet (top-up UX, per-query authorization, publisher payment routing) — roughly doubles the billing surface area
- User confusion is the primary UX risk: 'am I on subscription or pay-as-you-go? Does Edwin's answer about my email cost me extra? Did the subscription cover that agent action or did it come out of my wallet?' — requires very clear UX labeling of which track every cost hits
- Brave's BAT lesson: even with 100M MAU and 8 years of investment, BAT creator tipping reaches only ~2M verified publishers against millions of Brave-visited sites — running subscription + micropayments does not solve the publisher cold-start problem, it just defers it with a different mechanism
- The 'optional publisher payment' feature for subscribers risks becoming the only micropayment use case if the pay-as-you-go track does not find adoption — feature sprawl without a clear revenue story
- More complex Jake dependency: both tracks require identity-core native binaries (subscription renewal = envelope-gated wallet action; per-query micropayments = envelope per request); native binaries on Win/Mac must be published before either track ships
- Potential margin confusion: if subscription price must cover ops cost at average query volume, and micropayment price must also cover inference cost, the two pricing models may produce different effective per-query costs that users compare unfavorably
- Highest engineering effort of the three options; startup team risk of building two things instead of one and executing neither well

**Dependencies.** Everything from Option A (micropayment track) plus everything from Option B (subscription track). Both Jake's native identity-core binaries for Win/Mac AND Dolphin Milk x402agency.com BSV support. Publisher-side BSV header discovery for the optional per-cite payment feature. Clear UX design (which costs hit which track) must be finished before any billing ships. Pricing of both tiers must be validated so they do not cannibalize each other (subscription users should not easily calculate that pay-as-you-go would be cheaper for their usage pattern).

**Best when.** Hodos has enough engineering capacity to build and maintain two billing tracks without diluting quality on either; the team has validated at least one publisher vertical that will receive per-cite BSV payments (to make the micropayment track non-fictional); the BSV-native early adopter base and the crypto-agnostic privacy-conscious base are both reachable in year one and the team can design UX that serves both without alienating either; Brave's dual-stack is the explicit strategic model and the team studies it carefully rather than reinventing it

**Effort/risk.** High. Full subscription infrastructure + full micropayment infrastructure + clear UX separating them + Edwin treasury split on both tracks + publisher payment routing. Likely a 2-3 sprint sequence: ship subscription-only first (Medium effort), add micropayment pay-as-you-go track second, add optional publisher payment feature third. The phased approach turns Option C into Option B followed by Option A incrementally, which reduces simultaneous risk.

**Key trade-offs.** 1. Revenue predictability vs. differentiator purity: Subscription gives Hodos a revenue floor to fund ops and staff; micropayment-only generates zero predictable revenue and depends on per-query volume. The differentiator (per-cite publisher payments, the structural gap no incumbent fills) requires micropayments to be real, not roadmap. These pull in opposite directions: the commercially safer choice is subscription; the architecturally distinctive choice is micropayment-first.

2. Szabo's mental-transaction-cost mitigation vs. transparency: The only proven solution to the micro-decision problem is moving the decision to budget-setting (set a daily/monthly cap once, execute silently within it). This means casual users should NEVER see a per-query cost prompt for standard queries — PermissionEngine's Silent path plus fiat-denominated budget caps is the mechanism. But invisibility at execution requires trust that the budget is correct, which requires excellent real-time spend dashboards. The '$20 lesson' shows that without hard caps AND visibility, users get surprised in both directions. All three options must solve this; they differ in whether the cap is a subscription boundary or a pre-approved wallet balance.

3. Publisher cold-start timing vs. architectural completeness: The per-cite publisher payment is the strongest structural argument for x402/BSV in a browser. But it requires publishers to have BSV addresses in their x402 headers — the same loop Maxthon failed to close in 6 years, BAT in 8 years. Option A makes publisher cold-start a launch-blocking requirement. Option B defers it indefinitely. Option C attempts it as a progressive enhancement. The honest question is whether Hodos has a credible publisher bootstrapping wedge (1Sat Ordinals content is the obvious candidate) or whether publisher cold-start will remain fictional.

4. x402/BSV off the main rails: The x402 Foundation (Coinbase/Anthropic/AWS/Vercel) implements EVM stablecoins, not BSV. Hodos's x402/BSV is a parallel implementation. Dolphin Milk routes through x402agency.com which appears to support BSV, but Hodos is not in the main x402 ecosystem. This limits which AI providers can be reached directly via x402/BSV without an intermediary. The differentiator claim ('pay any AI provider with BSV') depends on x402agency.com or on future BSV support in the x402 Foundation — neither is guaranteed.

5. Casual-user accessibility: Option A requires users to understand wallet loading and accept that money leaves per query; Option B presents as a familiar SaaS subscription; Option C requires users to understand both. Hodos's stated north star is 'easy for a casual, non-technical user.' Micropayment-first is the hardest casual-user experience. Fiat-denominated display and pre-approved budget caps narrow the gap significantly, but the onboarding story ('load your wallet with $20 of BSV before you can ask a question') is fundamentally harder than 'subscribe with a credit card for $10/month.'

6. Edwin treasury split: ARCHITECTURE_TECHNICAL.md §9 identifies monetization split as an open design question for Jake. The split formula differs by model: subscription gives Jake a predictable monthly percentage; micropayments give Jake a per-transaction percentage. Jake's incentives likely favor micropayments (aligned with Dolphin Milk's x402 architecture and his own envelope system) but may also value subscription predictability for Edwin's own financial planning.

**Open questions:**
- Publisher cold-start bootstrapping wedge: Is the 1Sat Ordinals content ecosystem large enough and willing enough to bootstrap per-cite BSV payments in year one? Without a credible answer, Option A's publisher payment story is fictional at launch. (Matt can validate this with the BSV community directly.)
- x402agency.com BSV support scope: Does x402agency.com (the current Dolphin Milk LLM routing endpoint) support BSV denomination for ALL the LLM providers Hodos needs (Claude, GPT-5, open models)? Or is it limited to specific providers? This determines whether micropayment-only is architecturally complete or still dependent on intermediary choices. (Jake or John can confirm.)
- Jake's Edwin treasury split preference: ARCHITECTURE_TECHNICAL.md §9 identifies the monetization split as open (Matt's instinct: 60/40 Hodos/Edwin on agent-authorized transactions). Jake's view matters because: (a) subscription vs. micropayment changes when the split triggers, (b) Jake's own Edwin revenue model depends on which track Hodos leads with. Does Jake prefer a predictable monthly subscription split or a per-transaction micropayment split? (Agenda item for Jake meeting.)
- Subscription infrastructure ownership: If Option B or C is chosen, who operates the privacy proxy that pays LLM providers in bulk? Hodos running its own proxy (Brave Leo pattern) is a significant ops commitment for a small team. Alternative: negotiate a bulk rate with Dolphin Milk / x402agency.com and let them aggregate — Hodos just pays a monthly lump sum and they route queries. Which path is Jake/John willing to support? (Affects ops cost and privacy architecture.)
- BSV volatility fiat-pegging mechanism: For subscription billing, the renewal amount should be fiat-denominated ('$10/month equivalent in BSV'). What BSV price oracle does Hodos use, and how is it secured against oracle manipulation? If BSV price doubles mid-month, does the subscriber's renewal automatically halve in BSV terms? This needs a clear policy before any billing ships.
- Phased sequencing if Option C: If both is the goal, what ships first — subscription (Option B) or micropayment pay-as-you-go (Option A)? Shipping subscription first provides revenue to fund micropayment track development, but risks the micropayment track never being built (BAT lesson). Shipping micropayment first maintains architectural purity but generates no predictable revenue. Does Matt have a clear phase sequence in mind?

**Sources:** <C:\Users\archb\Hodos-Browser\development-docs\Dolphin Milk + Edwin Integration\UX_EDWIN_ASSISTANT_COMMUNICATION.md — §5 §B monetization thesis, Szabo mental transaction costs, BAT precedent, Mozilla Coil $3 experiment, Maxthon VPoint, Kagi/Brave/DuckDuckGo subscription facts, honest micropayment risks (all [FACT] tagged and cited to primary sources)> · <C:\Users\archb\Hodos-Browser\development-docs\Dolphin Milk + Edwin Integration\ARCHITECTURE_TECHNICAL.md — §2 envelope-aware fee split, §3 request flow with PermissionEngine budget caps, §9 open monetization design question for Jake (60/40 split instinct)> · <C:\Users\archb\Hodos-Browser\development-docs\Dolphin Milk + Edwin Integration\LESSONS_LEARNED_EDWIN_INSTALL.md — §3 the $20 OpenAI credit lesson, §5 casual-user requirements (budget cap with alerts, cheap safe defaults, fiat-first display)> · <C:\Users\archb\Hodos-Browser\development-docs\Dolphin Milk + Edwin Integration\BROWSER_AI_IMPLEMENTATION_STUDY.md — §F Maxthon VPoint failure analysis, §H cross-cutting recommendations (x402 payment as consent mechanism, subscription + micropayment not either/or), Brave Leo Premium + BAT dual-stack fact, Kagi fair-pricing viral trust signal, Opera MiniPay 13M wallets vs. desktop content gap, Mozilla Coil failure> · <https://github.com/x402-foundation/x402 — x402 protocol foundation (EVM/stablecoin reference implementation; BSV not in foundation scope)> · <https://aws.amazon.com/blogs/industries/x402-and-agentic-commerce-redefining-autonomous-payments-in-financial-services/ — AWS Bedrock AgentCore x402 integration (May 2026); confirms x402 policy-based spending controls and audit trail> · <https://bitcoinmagazine.com/technical/szabos-micropayments-and-mental-transaction-costs-25-years-later — Szabo mental transaction cost 25-year retrospective; confirms the behavioral problem persists even with technical improvements> · <https://brave.com/blog/bat-roadmap-3-0/ — BAT Roadmap 3.0 on-chain era; confirms BAT + Leo Premium dual-stack architecture and structural limits of creator tipping> · <https://kagi.com/stats — Kagi live subscriber stats (72,586 members as of June 26, 2026; profitable at ~53K); Lightning BTC payment option confirms subscription + crypto-payment combination is viable>

