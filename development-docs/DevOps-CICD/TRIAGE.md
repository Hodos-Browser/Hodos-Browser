# Master Triage — DevOps/CI-CD + 0.4.0

**Created:** 2026-06-01
**Status:** 🚧 live — refine as research lands

Captured from the planning session. **Part A** = every-build process work (lives in this DevOps
home). **Part B** = 0.4.0 build-specific features/changes (live in `../0.4.0/`).

Keystone: **A4 Brave-fork feasibility** gates/colors A1, B1, B2, B4 → it goes **first**.

> ✅ **A4 DECIDED (2026-06-01): STAY ON CEF — do not build from Brave.** See
> `research/BRAVE_FORK_FEASIBILITY.md`. Knock-on: B1 farbling = Blink patches in our own self-built
> CEF (no Brave); A1 self-build is for CODECS (settled), not Widevine; B4 extensions need re-scope
> (CWS extensions/MetaMask infeasible on CEF) + decide jointly with B2 (Chrome-runtime extension path
> fights a custom native header).

> ✅ **Deep dives done (2026-06-01):**
> - **A1** `research/A1_BUILD_STRATEGY.md` — ✅ build LOCAL + sccache; skip cloud (mostly); Mac is
>   **Apple M1** (confirmed) → build macOS on it locally; `concurrent_links` caps + `symbol_level=0`.
> - **A6** `research/A6_AUTO_UPDATE.md` — ✅ VERIFIED (reversed earlier lean): **keep Sparkle +
>   WinSparkle split, do NOT unify on Velopack** (Velopack v1.0 is 5 days old, solo-maintained, and
>   its update FEED isn't cryptographically signed — disqualifying for a money browser). Both Sparkle
>   & WinSparkle have EdDSA signing; OBS (C++/CEF) proves the split. We already ship both → mostly
>   CONFIG. Enable silent (Sparkle install-on-quit; WinSparkle `check_update_without_ui` + silent NSIS
>   — verify hands-on). Pin Sparkle ≥2.7.2; 2 unpatched 2026 CVEs (local-only) → deltas-off + monitor.
>   Test plan: `research/A6_SILENT_UPDATE_TEST_PLAN.md`.
> - **B1** `../0.4.0/B1-farbling-design.md` — Blink Supplement (covers workers) + **persistent
>   per-profile seed** to fix logins; re-implement / use fingerprint-chromium (BSD-3), not Bromite (GPL).

| ID | Item | Category | Size | 0.4.0? | Depends on | Research |
|----|------|----------|------|--------|-----------|----------|
| **A4** | Brave-fork feasibility (build from Brave's tree?) | research/decision | spike | gating | — | **DEEP (keystone)** |
| A1 | Self-build CEF binaries (Win/Mac, Linux placeholder) | process | large | process | A4 | deep |
| A2 | Track latest-stable Chromium/CEF + compat checks | process | med | process | A1 | med |
| A3 | Post-CEF dependency-bump process | process | med | process | A2 | med |
| A5 | Two-tier release flow (binary build vs fast bugfix) | process | med | process | A1 | low-med |
| A6 | True auto-update / Omaha 4 vs Sparkle | process/research | med-lg | maybe later | — | med |
| A7 | Test review + strategy (where/platform/naming/trust) | process | large | process | — | semi-deep |
| B1 | Farbling into Chromium source (Brave-style) | feature/refactor | large | candidate | A4 | deep |
| B2 | Header → C++ (keep exact CSS) | refactor | large | candidate | assess + research | deep (own session) |
| B3 | Bookmarks functional | feature | med | likely | — | light (UX) |
| B4 | Extensions + wallet deconfliction | feature | large | candidate | research (untrusted doc) | deep (security) |

---

## Part A — every-build process

### A1. Self-build CEF binaries (PROPRIETARY CODECS — settled)
Win + Mac now, Linux placeholders. **Why:** stock CEF lacks H.264/AAC/MP3; we build with
`proprietary_codecs=true ffmpeg_branding=Chrome` (confirmed in `scripts/build_hodos_cef*.{bat,sh}`).
Mandatory, not going away. Also the host for farbling patches (B1). Existing scripts noted (Mac built
by someone else; user now has a Mac). **Real A1 work:** make the build not take ~2 weeks — caching
(sccache), remote/cloud execution (GitHub-hosted runners can't do full Chromium builds), reproducible
runbook. See `CEF_BUILD_RUNBOOK.md`. Widevine premium = separate VMP track (§6 of runbook).

### A2. Track latest-stable Chromium/CEF + compat
Currently ~6 months behind. Need: source latest stable, compatibility checks, what breaks, how we'd
know.

### A3. Post-CEF dependency-bump process
Other deps pinned to current CEF, not bumped in isolation; after a CEF bump we update the rest.
Needs a defined process.

### A5. Two-tier release flow (one process doc)
Tier 1 = build CEF/Brave binaries (rare, expensive) → `cef-binaries` release. Tier 2 = fast bug-fix
release consuming prebuilt binaries (today's ~35 min CI already does this). Document both; keep the
fast path fast. `BUILD_AND_RELEASE.md` documents Tier 2 only today.

### A6. True auto-update (vs Sparkle notify-only)
Sparkle/WinSparkle only *notify*; user must manually trigger. Want silent background updates.
Research **Omaha 4** (Chromium-native cross-platform updater behind Chrome's silent updates).
Also informs release **cadence**.

### A7. Comprehensive test review + strategy
Inventory + gaps + trust/audit problem (growing volume). Today: ~55 Rust (`rust-wallet/tests/`),
~46 GoogleTest C++ (`cef-native/tests/`, opt-in), ~6 Playwright (`frontend/e2e/`), adblock = 0.
Decide: where tests run (GitHub gate / cloud / local), which platforms, naming conventions,
auditability. Research Brave + other BSV BRC-100 devs + industry best practice.
> Not yet source-verified: the bookmarks-non-functional claim (B3) and exact test census came from
> exploration agents — confirm against source in each item's kickoff.

---

## Part B — 0.4.0 build-specific (see `../0.4.0/`)

### B1. Farbling into Chromium source (Brave-style)
Current JS-injection farbling (`cef-native/include/core/FingerprintScript.h` +
`FingerprintProtection.h`) is **detectable by some sites (breaks logins)** and suspected slow. Prior
decision: push farbling into Chromium source like Brave. **Tightly coupled to A4** — if we build
from Brave, this may come largely for free.

### B2. Header → C++ (keep exact CSS)
Header loads slowly. Keep exact branding/look. Leaning header-only; overlays stay React. **Own
dedicated multi-agent planning session** to find the *correct* architecture (do it once). Must also
measure *why* the header is slow (React render vs CEF subprocess spawn vs IPC warmup) before
assuming C++ is the fix.

### B3. Bookmarks — make functional
Buttons exist, non-functional today (verify). Research other browsers' bookmark UX.

### B4. Extensions/plugins (security-focused)
Untrusted research moved to `../0.4.0/browser-extensions/` (verify every claim against source).
Likely Chromium-approved extensions. **Wallet deconfliction:** block conflicting BSV wallets (vs our
native wallet), ALLOW Ethereum wallets (MetaMask, etc.). Research other browsers' extension models
for UX + security. **Colored by A4** (CEF's extension API is limited vs full Chromium).
