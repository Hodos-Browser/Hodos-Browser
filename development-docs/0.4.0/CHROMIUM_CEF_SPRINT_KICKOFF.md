# 0.4.0 — Doc Consolidation + Chromium/CEF Build Sprint — KICKOFF BRIEF

**Created:** 2026-07-09 · **Owner:** Matthew (Marston Enterprises) · **Lead:** Windows Claude (orchestrator)
**Mode:** Fully autonomous research + documentation. **NO CODE CHANGES. NO builds. NO pushes to main/release/tags.**

> This is the canonical brief for the next session. It has two missions: (1) consolidate + update the
> project docs, and (2) research + design (NOT implement) the sprint to rebuild our custom Chromium/CEF
> from newest stable source with our edits, culminating in a phase-ordered plan to reach `v0.4.0-beta.1`.

---

## 0. Where we are (context — verify against the repo, don't trust memory)

- **Windows silent auto-update is DONE and PROVEN LIVE.** beta.25→beta.26 silently applied through the
  two-process profile picker on real hardware (2026-07-09). beta.26 is shipping Latest. The whole
  silent-update-through-the-picker path is validated. Prior arc: signer-continuity CN gate (beta.23),
  Win10 overlay fixes, global settings, bookmarks, picker-gate v2 (`AUTOUPDATE_PICKER_GATE_DESIGN.md`).
- **Profile picker + wallet architecture = SHELVED** (owner decision). Wallet stays SHARED across profiles
  (one backend on 31301/31401). Per-profile wallet was discussed 2026-07-09: **deferred**; the future
  migration is **non-destructive/opt-in** (existing profiles keep the shared wallet; per-profile wallets are
  additive — never force-split funds), so delay is cheap. Cheap hedges to keep the door open: (a) frame the
  profile UI honestly ("profiles share one wallet"), (b) incrementally route wallet calls through
  `WalletPort()` instead of literal `31301` (interceptor `HttpRequestInterceptor.cpp` still hardcodes it).
  NOT this sprint — noted for later.
- Owner is happy with the current stable state. **This sprint shifts to the Chromium/CEF rebuild.**

## 1. Hard constraints (this whole sprint)

1. **NO code changes.** Research + design + documentation ONLY. Do not edit `.cpp/.h/.rs/.ts` or build files.
2. **NO builds** (no cmake, no cargo build for output — read-only inspection is fine).
3. **NO pushes to `main`, `release`, `staging`, or any tag.** **DO commit + push the docs to
   `origin/0.4.0`** (owner-approved 2026-07-09) so Mac Claude can pull them. Feature branch only. If a
   feature-branch push somehow prompts for approval, SKIP and leave it committed local (owner is away).
4. **Fully autonomous.** Do NOT ask the owner questions (no AskUserQuestion). Where a decision is needed,
   pick the reasonable default, DOCUMENT the assumption + the alternative, and continue. Loop back later.
5. **Adversarial review throughout.** Every design outline, implementation plan, and test plan gets an
   independent skeptic pass (refute-first), then revise; loop-until-dry. Use the Workflow adversarial pattern.
6. **Create reference docs, keep context compact.** Write findings to docs under `development-docs/0.4.0/`
   (and DevOps-CICD where it belongs); summarize results, don't hold everything in context.
7. **Windows Claude is the LEAD** and owns cross-platform flow/deconfliction with Mac Claude via a
   coordination doc (extend `MAC_WINDOWS_RELAY.md` or a new `CHROMIUM_BUILD_RELAY.md`).

## 2. Mission A — Consolidate + update the docs (do this FIRST)

Target folders: **`development-docs/0.4.0/`** and **`development-docs/DevOps-CICD/`** (and their subfolders).

1. **Inventory + review every doc** (fan out; one agent per doc or tight cluster). For each: classify
   **keep-as-is / update / archive**. Apply the CLAUDE.md kickoff discipline — where a doc cites `file:line`,
   verify the code still exists/shape before trusting it; note drift.
2. **Update the live docs** with everything done + learned over the last several sessions (beta.19→beta.26):
   the silent-update saga (signer CN gate, apply supervisor, picker-gate v2 proven live), Win10 overlay
   fixes (F1/F2/F3/F5), global settings, bookmarks favicon/delete, the promote.yml retry hardening,
   BUILD_AND_RELEASE tag-derived/draft-promote flow, the per-profile-wallet deferral reasoning.
3. **Archive stale docs** into a NEW folder **`development-docs/0.4.0/archive/`** (create it; NOT the
   top-level `archived-docs/`). Move (git mv) anything superseded/historical there, leaving a one-line
   reason. Keep archaeology, declutter the active set.
4. Write/refresh a **`development-docs/0.4.0/README.md`** index reflecting current reality (beta.26 shipped;
   silent-update-through-picker proven; what's active vs archived).
5. Commit the doc changes locally on `0.4.0` (see constraint #3 re: pushing).

## 3. Mission B — Research + DESIGN the Chromium/CEF build sprint (NO implementation)

Goal: a phase-ordered, adversarially-reviewed plan to **rebuild our custom Chromium+CEF from newest stable
source with our edits, build locally, test, build for production**, and a checklist of everything needed
before **`v0.4.0-beta.1`**. We build our OWN Chromium+CEF (see `DevOps-CICD/CEF_BUILD_RUNBOOK.md` +
`CEF_VERSION_UPDATE_TRACKER.md`); capability is bounded by patch scale + per-bump churn, not stock CEF.

### Scope of source edits to plan (reconstruct the full list from the docs — this is a starting set)
- **Farbling → source/Blink level. [OWNER-DECIDED: COMMIT to the migration.]** Currently JS-injection
  (`FingerprintProtection.h` + `FingerprintScript.h`, per-domain seed + Canvas/WebGL/Navigator/Audio).
  Design the move to a compiled Chromium/Blink patch — do NOT re-litigate keeping JS-injection (owner
  committed to source/Blink). Master plan item "B1 = Blink-farbling"; `cef/patch/` is the patch mechanism,
  farbling cited as "the first patch." **Study Brave's actual Blink farbling implementation for BOTH Windows
  and macOS** and map it to our `cef/patch/` approach + per-domain exemptions (see Q3).
- **Proprietary codecs.** Our self-build reason is `ffmpeg_branding=Chrome` (proprietary codecs), NOT
  Widevine (`reference_cef_self_build_reason`). Confirm/plan the codec build config on the new version.
- **DRM / Widevine (the Amazon issue) — RESEARCH + REPORT; likely OUT of beta.1 unless CHEAP.** Media works
  on YouTube/X/LinkedIn but an **Amazon.com movie failed to play** — prime suspect is **Widevine CDM
  absence**. **KEY CLUE (owner):** the owner hit the *same* error in **Brave**, and Brave shows a **button
  that fixes it** — that is Brave's **on-demand Widevine CDM install/enable** flow (Brave offers optional
  Widevine as a component download). So the research MUST investigate: (a) confirm our error == missing
  Widevine CDM; (b) study exactly how Brave offers/downloads/enables the Widevine CDM component and whether a
  CEF embedder can replicate it (component-update the Widevine CDM); (c) the licensing reality (Widevine
  redistribution normally needs a Google agreement — find out if there's a free/component-download path like
  Brave's, or if it costs real money). **Owner stance:** if it can't be fixed cheaply ("without paying a
  million dollars"), leave it as-is until later ("until we get rich") — so default OUT of beta.1, document
  the path + cost + which sites break, and only pull it into beta.1 if a genuinely cheap/free path exists.
- **Dependency updates.** Plan the bumps that ride with a Chromium version jump.
- **Version bump 136 → current stable.** We're on CEF 136; target the current stable Chromium/CEF (earlier
  note: ~M149, no LTS exists — CONFIRM current at research time). Plan minos/deployment-target + codec
  branding + runner pins per `CEF_VERSION_UPDATE_TRACKER.md` and the `minos` guard in BUILD_AND_RELEASE.
- **"Other planned edits."** The owner recalls more source edits were planned but not all by name — the
  research must reconstruct the COMPLETE list from `SPRINT_0_4_0_MASTER_PLAN.md`, `CEF_BUILD_RUNBOOK.md`,
  `CEF_VERSION_UPDATE_TRACKER.md`, DevOps `README.md`, and any patch files under `cef/patch/`.

### The owner's specific research questions (answer each in a dedicated doc)
- **Q1 — Mac farbling.** Do we need to do the farbling edits on Mac too, or is one source/Blink patch
  cross-platform (single patch set, built per-OS)? What must Mac Claude do vs. inherit?
- **Q2 — Farbling × adblock.** How does moving farbling into the CEF binaries (source-level) change how our
  adblock engine works? (adblock-engine = separate Rust process on 31302 + C++ `AdblockCache`; cosmetic CSS +
  scriptlet injection.) Any interaction/ordering/breakage?
- **Q3 — Farbling × OAuth pre-approved sites.** How does source-farbling affect our pre-approved/auth sites?
  Today JS-farbling skips auth domains (`FingerprintProtection::IsAuthDomain`, `hodos-unbreak.txt` blanket
  `#@#+js()`, auth-domain skip in injection). If farbling moves to Blink, that per-domain exemption mechanism
  must be re-implemented at source level — plan how.
- **Q4 — Amazon DRM.** Root-cause the Amazon movie error (Widevine? codec? EME?). Investigate Brave's
  "button that fixes it" (on-demand Widevine CDM install) and whether a CEF embedder can replicate it +
  at what licensing cost. Default OUT of beta.1 unless a cheap/free path exists (owner stance above).
- **Q5 — Full edit list.** The complete, reconciled list of every Chromium/CEF source edit + why + platform.

## 4. How to run it (Workflow-driven, autonomous)

You are authorized + expected to use the **Workflow** tool heavily. Suggested shape (adapt as needed):

- **Workflow 1 — "Consolidate & Outline"**
  - Phase A: parallel doc inventory/review (keep/update/archive + drafted updates + drift check).
  - Phase B: apply — create `0.4.0/archive/`, git mv stale docs, apply updates, refresh the `0.4.0/README.md`.
  - Phase C: produce **`CHROMIUM_CEF_BUILD_DESIGN_OUTLINE.md`** — the outline of the build sprint (get newest
    Chromium → source edits [§3] → local build → test → prod build → beta.1 gate list), feeding in Q1–Q5.
  - Adversarial review of the outline → revise.
- **Workflow 2 — "Detailed Implementation Plans" (auto-chain after WF1 completes + you've read its result)**
  - One deep-research track per design area, each producing a detailed, followable plan doc, e.g.:
    Brave farbling patches (Win+Mac); CEF/Chromium 136→stable bump; ffmpeg_branding codecs; Widevine/Amazon
    DRM; dependency bumps; farbling×adblock (Q2); farbling×OAuth-preapproved (Q3); Mac↔Windows build split
    + coordination doc (Q1); local-build→test→prod-build pipeline + test plan.
  - Web research allowed (WebSearch/WebFetch) — e.g. read Brave's Blink farbling source, Chromium build docs.
  - Each plan → independent adversarial/skeptic review → revise → loop; research more where thin
    (loop-until-dry). Write each as its own reference doc; keep context compact.
  - Synthesize a master **`IMPLEMENTATION_ROADMAP_0_4_0_BETA1.md`**: phase-ordered steps, dependencies,
    Windows vs Mac ownership, and the explicit **`v0.4.0-beta.1` readiness checklist**.

Between workflows, read results and update memory + the coordination doc. Compact context as docs land.

## 5. Deliverables (end state)

1. Consolidated, current `development-docs/0.4.0/` + `DevOps-CICD/` doc sets; stale docs in `0.4.0/archive/`;
   refreshed `0.4.0/README.md`.
2. `CHROMIUM_CEF_BUILD_DESIGN_OUTLINE.md` (adversarially reviewed).
3. Per-area detailed implementation plan docs (each reviewed): farbling(Win/Mac), codecs, Widevine/Amazon,
   version bump, deps, farbling×adblock, farbling×OAuth, Mac↔Windows split, build/test/prod pipeline.
4. Answers to Q1–Q5 (in their own docs).
5. `IMPLEMENTATION_ROADMAP_0_4_0_BETA1.md` with phase order + beta.1 readiness checklist.
6. A Windows↔Mac coordination doc for the build phase (Windows Claude = lead).
7. Updated memory so a future session can boot straight into implementation.

**Reminder: this sprint STOPS at plans + docs. No code, no builds, no protected pushes. Implementation is a
later session, gated on the owner's review of these plans.**
