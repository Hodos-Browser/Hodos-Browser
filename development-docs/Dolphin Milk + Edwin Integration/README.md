# Dolphin Milk + Edwin Integration — Technical Docs (Hodos side)

> **This folder holds the engineering/technical half of the Dolphin Milk + Edwin
> integration doc set.** The pitch, product-framing, meeting-prep, and outreach
> half lives in the marketing intelligence vault — see **Companion folder** below.
>
> **Reorganized 2026-06-29:** the previously-flat folder is now split into
> `design/`, `implementation/`, `research/` (+ `research/deep-dives/`), and
> `partner-facing/`. No research was merged or deleted in the reorg — only moved.
> This README is the map. Cross-references between docs are by **filename in prose**
> (not clickable links), so they survive the move.

## Where to start

- **Need to talk to Jake?** → `JAKE_CONVERSATION_PREP.md` — plain-English cheat-sheet for the partner conversation (no jargon). The build's one real blocker is a question for Jake; this explains it.
- **Building?** → `implementation/IMPLEMENTATION_PLAN_v1.md` — the phased build + test plan. **Start here.**
- **Need the architecture?** → `design/ARCHITECTURE_TECHNICAL.md` (the canonical three-party design).
- **Weighing an open decision?** → `design/ARCHITECTURE_OPTIONS_BOTH_WAYS.md` (5 decisions both ways + the Jake agenda).
- **Want the evidence base?** → `research/` and `research/deep-dives/` (all web-cited study material).

## Split principle

- **Dev / technical docs** (architecture, security model, wallet compat, research,
  implementation) **stay here** in the Hodos-Browser repo, version-controlled
  alongside the code they describe.
- **Pitch / product / admin / outreach docs** live in the marketing intelligence
  vault. They are not in this repo.

## Congruence rule

The two halves are a **coupled set**. When a technical doc here changes in a way
that alters product implications, claims, or messaging, update the corresponding
marketing-side doc so the two stay congruent — and vice versa. The marketing-side
README references these docs by **filename**; the 2026-06-29 reorg kept every
filename identical, so those prose references still resolve. (If the marketing
README hard-codes any flat path like `…/Dolphin Milk + Edwin Integration/FOO.md`,
re-point it at the new subfolder during the next congruence pass.)

## Companion folder (marketing / pitch half)

```
C:\Users\archb\Marston Enterprises\Hodos\marketing\intelligence\features\Dolphin Milk + Edwin Integration\
```

See that folder's `README.md` for the full pitch/product/outreach index.

---

## `design/` — the architecture we're building toward

| Doc | What it is | Notes |
|-----|------------|-------|
| `ARCHITECTURE_TECHNICAL.md` | Canonical three-party technical architecture (Hodos + Edwin + Dolphin Milk); process layout, request flow, the PermissionEngine-as-envelope-issuer synthesis, trust boundaries, Jake/John agendas | The reference design |
| `ARCHITECTURE_OPTIONS_BOTH_WAYS.md` | Both-ways pros/cons of the **5 open decisions** (D1 packaging, D2 which-assistant, D3 form factor, D4 search posture, D5 monetization); ends with the consolidated **"What to Ask Jake"** (8 blocking questions) | Study, not a decision |
| `EDWIN_VS_DOLPHIN_MILK_SECURITY.md` | Security-model comparison: Edwin's signed-envelope cryptographic gate vs Dolphin Milk's policy/budget-cap model; the prompt-injection scenario walkthrough | Core security explainer |
| `CANARY_A1_WALLET_COMPAT.md` | Wallet wire-compatibility verification (Hodos `:31301` vs the BRC-100 surface Dolphin Milk calls); the 3 concrete one-line fixes | Technical reference |

## `implementation/` — the build

| Doc | What it is | Notes |
|-----|------------|-------|
| `IMPLEMENTATION_PLAN_v1.md` | The phased build + local-test plan (native sidecar → frontend entry points → comms/auth → profiles → recall → onboarding → x402/signing → hardening). Per-phase scope, dependencies (incl. what's blocked on Jake), build approach, and test approach. | **The current plan** (2026-06-29) |

## `research/` — cited study material (preserved intact)

The evidence base behind the design. All web-cited (see **Shared research conventions** below).

| Doc | What it is |
|-----|------------|
| `LESSONS_LEARNED_EDWIN_INSTALL.md` | Field findings from the Windows/WSL install: WSL-not-Node is the enemy, the $20 cost lesson, setup-flow gaps, the casual-user requirements checklist |
| `EDWIN_NATIVE_PACKAGING_FINDINGS.md` | Code study of Edwin's native-dep surface + the closed-source-native vault (no JS fallback) + the **transport-binding** path (Hodos wallet can back IdentityCore) |
| `UX_EDWIN_ASSISTANT_COMMUNICATION.md` | UX vision + the **industry matrix** (14 players × 9 dims, business/monetization lens); §F UX recommendations + closest-analog copy/avoid |
| `BROWSER_AI_IMPLEMENTATION_STUDY.md` | How 13 browsers technically build their AI and why (architecture/rationale lens); §H = Hodos-sidecar implications as option clusters |
| `DOLPHIN_MILK_INTEGRATION.md` | What Dolphin Milk is (x402-paid LLM/tool agent, BRC-18 proofs, embedded BSV wallet) + the bundling thesis and hard questions |
| `THRESHOLD_ECDSA_EXPLORATION.md` | Future-tracking of John + BINARY's threshold-MPC (CGGMP'24) signing network; how it would extend Edwin's envelope model. Not committed direction |
| `INTEGRATION_RESEARCH_KICKOFF.md` | ⚠️ **Superseded** — the research-phase entry brief; kept for historical context. Build entry is now `implementation/IMPLEMENTATION_PLAN_v1.md` |

### `research/deep-dives/` — forensic deep-dives (created 2026-06-28)

Companion studies that go deep on the pieces Hodos would actually have to build or defend. All web-cited, claim-tagged.

| Doc | What it is |
|-----|------------|
| `DEEPDIVE_X402_BSV_MICROPAYMENTS.md` | How x402 works + the BSV path (BRC-105/29/31/103/100); the EVM-Foundation-vs-BSV split; honest gaps (intermediary dependence, sub-cent reality) |
| `DEEPDIVE_MICROPAYMENT_HISTORY.md` | The micropayment graveyard (Flattr, Coil, Blendle, Scroll, BAT, Lightning) + Szabo's mental-transaction-costs; why agent-to-API is structurally different |
| `DEEPDIVE_AGENTIC_BROWSER_SECURITY.md` | Threat model for an in-browser agent with a wallet in reach: CometJacking et al., OWASP LLM Top-10, isolation models, the 8-layer defense checklist |
| `DEEPDIVE_PRIVATE_AI_CLOUD_ARCHITECTURES.md` | How to route AI calls privately: Apple PCC, Brave proxy/VOPRF/TEE, DuckDuckGo broker, OHTTP, Tinfoil/NEAR TEE; a Tier 0–4 routing stack for Edwin |
| `DEEPDIVE_LOCAL_INFERENCE_FEASIBILITY.md` | What Edwin could run on-device in 2026: models, runtimes, hardware reality, the OS-level-GPU advantage; local/hybrid/cloud tiers |
| `DEEPDIVE_MAXTHON_FORENSIC.md` | Post-mortem of the BSV-native browser (VBox/VPoint/NBdomain): why the economic loop never closed; patterns to copy vs avoid |
| `DEEPDIVE_CASUAL_USER_ONBOARDING_UX.md` | How AI browsers onboard + the per-query cost-control UX nobody has solved; design-against the Edwin install failures; proposed first-run flow options |

## `partner-facing/` — external-send docs

> Written to be shared with external partners. Do **not** add cross-references to
> internal marketing/pitch material inside these.

| Doc | What it is | Notes |
|-----|------------|-------|
| `INTEGRATION_PLAN_v1.md` | Three-party integration overview for Jake + John + Calhoun: architecture, bundling intent, API-key/cost-mode UX, install flows, test plan | Partly superseded — install sequencing overtaken by the native-sidecar direction; see `implementation/` |
| `EDWIN_SETUP_FEEDBACK_FOR_JAKE.md` | Engineering feedback to Jake: Edwin install/setup friction, WSL/9P measurements, two-auth-system root cause, onboarding-wizard observations | External-send (Jake) |

---

## Shared research conventions

The 9 docs under `research/` and `research/deep-dives/` were produced by web-research
workflows and share one set of conventions (stated here once instead of repeated per-doc):

- **"STUDY, not a decision."** Each presents options with honest pros/cons; none picks a winner
  (a few express soft, explicitly-hedged leanings). This honors the research-phase rule:
  study every direction, don't force the call.
- **Claim tags** mark confidence inline: `[FACT]` (with a source URL), `[INFERRED]`,
  `[UNVERIFIED]`, `[SPECULATION]`/`[VISION]`, `[CALCULATED]`.
- **Citations** are inline markdown links plus an end-of-doc `Sources` list (≈20–45 links each),
  stamped to a June 2026 access date.
- **Shared closing scaffold:** most end with "What This Means for Hodos (Options, Not a Pick)" →
  "Open Questions" → "Sources."

### Known cross-doc overlaps (intentional; not redundancy to fix)

The studies were written as independent lenses, so some themes recur. Where to treat each as canonical:

- **Edwin-as-native-sidecar / no-WSL** — canonical in `LESSONS_LEARNED` + `EDWIN_NATIVE_PACKAGING_FINDINGS`;
  echoed by `DEEPDIVE_LOCAL_INFERENCE_FEASIBILITY` and `DEEPDIVE_CASUAL_USER_ONBOARDING_UX`.
- **x402 / BSV mechanics + adoption numbers** — canonical in `DEEPDIVE_X402_BSV_MICROPAYMENTS`;
  recapped in `DEEPDIVE_MICROPAYMENT_HISTORY` §4 and `DEEPDIVE_PRIVATE_AI_CLOUD_ARCHITECTURES`.
- **"Edwin must never hold signing power" / budget-cap-as-consent** — canonical in
  `EDWIN_VS_DOLPHIN_MILK_SECURITY` + `DEEPDIVE_AGENTIC_BROWSER_SECURITY`.
- **Maxthon prior art** — technical post-mortem in `DEEPDIVE_MAXTHON_FORENSIC`;
  strategic copy/avoid in `BROWSER_AI_IMPLEMENTATION_STUDY` §F and `UX_EDWIN_ASSISTANT_COMMUNICATION` §F(4).
- **Privacy tiering & form factor** — `BROWSER_AI_IMPLEMENTATION_STUDY` (§E, §H3, HOW lens) and
  `UX_EDWIN_ASSISTANT_COMMUNICATION` (§E, §D, WHY/UX lens) rank the same players; both feed the D3/D4 decisions.
- The two big studies each keep a **verbatim per-player appendix** — same players, different lens
  (technical vs UX/business). Kept separate on purpose: each appendix is its own doc's sourcing.

---

## Related

- `../DevOps-CICD/WSL_HYBRID_WORKSPACE.md` — interim workspace/sync strategy for running Edwin in WSL during our own dev cycle (not consumer-shippable)
- Project memory (auto-loaded): `edwin-hodos-implementation-kickoff`, `edwin-hodos-integration`, `edwin-vault-native-transport`, `edwin-internals-verified`
