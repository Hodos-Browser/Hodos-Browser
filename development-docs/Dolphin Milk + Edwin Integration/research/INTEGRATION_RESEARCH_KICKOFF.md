# Edwin + Dolphin Milk → Hodos — Research & Goals Kickoff

> ⚠️ **SUPERSEDED (2026-06-29).** This was the entry brief for the *research* phase. That phase is
> complete and the project has moved to **build** — start instead at `../implementation/IMPLEMENTATION_PLAN_v1.md`.
> Kept here for historical context (north star, what-already-exists, the open decisions as first framed).
> The casual-user north star and the "don't rewrite Edwin — PR upstream" constraint below remain in force.

> 📁 Part of the **Dolphin Milk + Edwin Integration** doc set (technical half) — see `../README.md`.
> **Created:** 2026-06-26. **Phase:** research & planning only — **NO coding yet.**
> **Purpose:** a single "start here" brief so a fresh session (new context) can pick up the integration work cleanly, anchored on the casual-user goal and on what already exists.

---

## 0. First action every new session: check Jake's repo for updates

Jake (`jonesj38`) actively pushes to Edwin, possibly including the Windows fixes we want — so **before any design work, check for upstream updates and report what's new**, to avoid building things Jake is already building.

- **Repo:** `https://github.com/jonesj38/edwin.git`
- **Local checkout:** `~/edwinpai` (WSL), branch `main`, at `v1.0.0-beta.8 +8` commits (`4f8d8b73b "Add B-Open BSV skills"`).
- **Caution:** the local tree is **ahead of origin by 1 commit** (a local "Add B-Open BSV skills" commit) and has **uncommitted `dist/` changes**. So **`git fetch origin` then review `git log --oneline HEAD..origin/main` — do NOT blindly `git pull`** (it could disrupt the working install). Report what changed upstream (especially Windows / native-build / installer / setup-flow work) and whether it affects our plan.

---

## 1. Primary goal (the north star for this whole effort)

**Make it easy for a casual, non-technical user to set up and use.** Easy to install, easy to use, "just works" when they install Hodos. Everything else — features, models, security depth — is in service of that. (See `LESSONS_LEARNED_EDWIN_INSTALL.md` for why this goal is non-negotiable: the current Edwin install is the opposite of easy.)

Secondary goals (already established in the doc set): bundle the agent into Hodos; keep the BSV signed-envelope security model; enable x402 LLM micropayments; preserve the wallet's existing UX safeguards.

---

## 2. What ALREADY exists (don't redo this — build on it)

This folder already holds a substantial technical doc set (`README.md` is the map):

- **`ARCHITECTURE_TECHNICAL.md`** — the three-party architecture is already designed: Hodos C++ shell manages **three subprocesses** — wallet (`:31301`, exists), adblock (`:31302`, exists), and a **NEW agent subprocess (Dolphin Milk, Rust, `:8080`)**. Adding the agent is the *same pattern* already in use. File locations already specified: `%APPDATA%/HodosBrowser/` (Windows), `~/Library/Application Support/HodosBrowser/` (macOS).
- **`INTEGRATION_PLAN_v1.md`** — architecture + implementation sequencing (external-send to Jake + Calhoun).
- **`EDWIN_SETUP_FEEDBACK_FOR_JAKE.md`** — earlier Jake feedback (overlaps the research-bundle Jake draft — merge them).
- **`EDWIN_VS_DOLPHIN_MILK_SECURITY.md`**, **`CANARY_A1_WALLET_COMPAT.md`**, **`DOLPHIN_MILK_INTEGRATION.md`**, **`THRESHOLD_ECDSA_EXPLORATION.md`** — security model, wallet-compat verification, what Dolphin Milk is, future MPC signing.
- Marketing/pitch half lives in `C:\Users\archb\Marston Enterprises\Hodos\marketing\intelligence\features\Dolphin Milk + Edwin Integration\`.

**Answers to Matt's specific questions, already settled by the existing docs:**
- *"Should it be a microservice on a port like the wallet/adblock?"* → **Yes.** That's exactly the documented model (agent subprocess on `:8080`, managed by the C++ shell). Edwin's own gateway already runs this way (port 18789).
- *"Where should files go — an Edwin folder under HodosBrowser appdata?"* → **Yes, correct instinct.** Already specified: `%APPDATA%/HodosBrowser/` (Win) + `~/Library/Application Support/HodosBrowser/` (mac). An `agent/`/`edwin/` subfolder there fits.

---

## 3. The key open decisions this research must resolve

**Constraint + collaboration model (Matt, 2026-06-26):** Edwin is Jake's project (he's a solo dev with his own priorities). We do **not** maintain a divergent fork or rewrite Edwin as our own thing. Instead: if the integration needs an Edwin feature/fix, **we build it and submit it UPSTREAM as a PR; if Jake approves and merges, we pull it back in.** Matt has a direct line to Jake to coordinate. So our Edwin changes flow *through* Jake, keeping our install in sync with his and getting Edwin to where it works well for the integration. (The earlier "extract a lean Rust vault on our side" idea is set aside — contribute upstream instead.)

**Decision 1 — How to run/bundle Edwin so it "just works."** Correcting an earlier over-statement: the Windows pain is **NOT inherent to Node** — it comes from running Edwin under **WSL** while files live on the Windows drive (the slow 9P bridge). Node runs natively on Windows and Mac. So the leading candidate is: **bundle Edwin's Node server and run it NATIVELY on each OS (no WSL), as a Hodos-managed sidecar on a localhost port** — same pattern as the wallet/adblock, just a Node process instead of a Rust binary. The user never sees Node; Hodos starts/stops it. Research questions: do Edwin's native modules have working Windows/Mac prebuilds (worst offender: local-embeddings `node-llama-cpp` — avoidable, we use cloud embeddings)? How big is a bundled Node runtime + Edwin? What would Jake's installer need so native-Windows is a supported path?

**Decision 2 — What IS the in-browser assistant: Edwin, Dolphin Milk, or both?** The existing docs treat **Dolphin Milk** (Rust, x402 agent) as the agent runtime and **Edwin** as the vault/security + assistant layer. But Matt runs EdwinPAI as the assistant and wants to bundle *that*. Clarify early how Edwin and Dolphin Milk relate in the bundled product, and whether v1 ships one, the other, or both.

**Decision 3 — UI/UX: how does the user launch and talk to the assistant inside the browser?** (See §4a.) Shapes how the browser talks to the agent and how results render; worth settling early even if fine details come later.

These gate everything downstream. Much of the first session is understanding the architecture well enough to make them.

---

## 4. Open questions to carry into the deep-research / architecture doc

From `ARCHITECTURE_TECHNICAL.md` §9-10 (Jake/John agenda) plus new ones from this session:

1. **Agent runtime decision** — A/B/C above. (New, highest priority for the casual-user goal.)
2. **SecureVault extraction** — extract an `edwin-vault` Rust crate vs. reimplement the vault interface in Rust from Edwin's schema. (Jake's call.)
3. **What surface of Edwin to target** — Jake is mid-refactor (qmd backend, pruned extensions); target current shipped vs. main?
4. **Casual-user setup flow** — guided/conversational config, sane cheap defaults, budget caps + alerts, honest status surfaces (the `LESSONS_LEARNED` checklist).
5. **Cost model** — default to a cheap model for routine; expensive features opt-in; per-user BYO-key vs. bundled x402 micropayments.
6. **Envelope schema standardization** (propose a BRC?), **TTL vs long-running tasks**, **multi-channel inbox in scope?**, **monetization split** — all from the existing agenda.
7. **macOS parity** — the build must be cross-platform from day one (Hodos `CLAUDE.md` invariant 9).

---

## 4a. UI/UX — how the user launches & talks to the assistant (decide early)

How a casual user actually interacts with the assistant inside Hodos shapes the integration (how the browser talks to the agent, how replies render). Options to weigh:
- **Side panel / chat sidebar** in the browser window (a keystroke away).
- **Omnibox keyword** (e.g. type `agent: summarize this page`).
- **Dedicated overlay** rendering the agent's own web UI (existing docs mention an "Agent overlay" CEF subprocess) vs a **Hodos-native chat UI** calling the agent's HTTP API.
- **Activity surface** in the wallet panel (docs already plan an "Agent activity" view of on-chain proofs).

`ARCHITECTURE_TECHNICAL.md` already sketches an Agent overlay + an `agent:` omnibox keyword — start there. Decide for v1: sidebar, overlay, or omnibox-first? And does it render Edwin's own UI or a Hodos-native chat?

> **This deserves its own study before UX is locked.** See `UX_EDWIN_ASSISTANT_COMMUNICATION.md` — a long-term UX vision (full-page localhost ≈ Edwin Desktop, omnibox/answer-engine, popups/sidebar) plus a broad industry deep-dive (Perplexity Comet, OpenAI Atlas, Brave Leo, Chrome+Gemini, Firefox, Vivaldi, LibreWolf, Maxthon, etc.) to learn principles/vision before we commit. Run that study before finalizing Decision 3.

---

## 5. The deliverable to produce next (in a fresh context)

A **deep-research + architecture design document** for the bundled, casual-user-friendly integration that:
- Resolves the §3 agent-runtime fork with evidence.
- Specifies the install/setup experience end-to-end from the user's POV (download Hodos → agent just works).
- Specifies the process/port/file-layout (building on `ARCHITECTURE_TECHNICAL.md`).
- Carries the security-envelope model through (from `EDWIN_VS_DOLPHIN_MILK_SECURITY.md`).
- Bakes in the `LESSONS_LEARNED` requirements (cheap defaults, no jargon, no YAML, honest status, cost caps).

This is research/design, **not implementation.**

---

## 6. How to resume in a fresh context (read these first)

1. This file.
2. `LESSONS_LEARNED_EDWIN_INSTALL.md` (this folder).
3. `ARCHITECTURE_TECHNICAL.md` + `README.md` (this folder) — and the other 6 docs as needed.
4. Research bundle: `C:\Users\archb\Marston Enterprises\Edwin\research-2026-06-17\` (loop/harness study, know-thyself draft, Jake feedback, OpenClaw comparison).
5. Claude project memory (auto-loaded): `edwin-internals-verified`, `edwin-north-star`, `why-edwin`, `matt-profile`, `working-style`.

**Carry-over facts that cost real effort to learn:** Edwin = Node/pi-agent-core gateway on port 18789; the Windows pain is **WSL/9P, not Node** — run Edwin natively (no WSL) and the bridge problem disappears; recall works but defaults are expensive; the security envelope/vault is the crown jewel; **do NOT rewrite Edwin — pull + integrate Jake's updates.**
