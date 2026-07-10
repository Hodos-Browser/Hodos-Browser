# Hodos Browser — 0.4.0 Sprint Docs

**Status (2026-07-09):** Shipping on the beta train — **latest = `v0.3.0-beta.26` (LATEST/promoted, live).** Windows **silent auto-update through the two-process profile picker is DONE and PROVEN LIVE** (beta.25 → beta.26 silently applied on real hardware). macOS silent auto-update was proven live earlier (beta.21 → beta.22). The silent-update saga is **complete**.

Internal/beta builds are versioned `0.3.0-beta.N` and stay private; the deliberate public release will be tagged `0.4.0` and pushed to the `release` remote (see `../DevOps-CICD/BUILD_AND_RELEASE.md` and the Branch/Remote workflow in `../DevOps-CICD/README.md`).

---

## Where we are — the beta.19 → beta.26 arc

The 0.4.0 line converged through the beta train rather than the original wave graph:

- **beta.19 → beta.20** — first silent auto-update proof (Windows), splash + console-flash polish (A1/A2), Win10 overlay dead-button hardening (C1/C2), bookmark first-open + click-outside (B1/B2).
- **beta.21 → beta.22** — macOS silent auto-update proven live; Windows silent regressed (signer-continuity gate compared rotating Azure Trusted-Signing leaf thumbprints).
- **beta.23** — Windows silent regression SOLVED (signer-continuity **CN gate**), Win10 overlay cluster fixes (F1/F2/F3/F5 single-instance handoff), global settings across profiles, bookmark favicon/delete, mac dropdown-button consistency.
- **beta.24** — no-code-change rebuild (thumbprint-gate break-out); promote.yml redirect-verify retry hardened.
- **beta.25 → beta.26** — **picker-gate fix**: the picker-spawned `--profile` child now waits for picker exit before the sole-instance check, unblocking the silent apply on multi-profile installs. **Proven live.** Splash mojibake fixed.

### Shelved (owner decisions, revisit with market feedback)
- **Profile picker + per-profile-wallet architecture = SHELVED.** The wallet stays **SHARED** across profiles.
- **Same-process (Chrome-model) picker refactor = deferred/shelved.**
- **Per-profile wallet = deferred** (would ship later as a non-destructive / opt-in migration, cheap hedges only).

---

## Next: Chromium/CEF rebuild sprint (RESEARCH + DOCS only right now)

The next sprint rebuilds our custom Chromium/CEF from source, bumping **CEF 136 → current stable / LTS** (exact target decided in the rebuild sprint), with source edits:
- **B1 farbling → Blink patch** (owner-committed) — canonical design in `B1-farbling-design.md`.
- Proprietary codecs, dependency verification, version bump, Widevine/DRM investigation.

Full brief: **`CHROMIUM_CEF_SPRINT_KICKOFF.md`** (active — start there). The design outline is in **`CHROMIUM_CEF_BUILD_DESIGN_OUTLINE.md`**, the per-area plans in the `chromium-rebuild/` docs, and the phase-ordered plan in **`IMPLEMENTATION_ROADMAP_0_4_0_BETA1.md`**. Load-bearing inputs: `../DevOps-CICD/CEF_BUILD_RUNBOOK.md`, `../DevOps-CICD/CEF_VERSION_UPDATE_TRACKER.md`, `../DevOps-CICD/DEPENDENCY_VERIFICATION.md`, `../DevOps-CICD/research/BRAVE_FORK_FEASIBILITY.md`.

---

## Scope philosophy

0.4.0 is a **polish + platform-hardening** line, not a feature land-grab. We prefer minimal, reversible changes; the UI-architecture rules in the top-level `CLAUDE.md` (overlays only, never new panels on `MainBrowserView.tsx`) and the security invariants (private keys never in JS, no silent schema/crypto changes) hold. Cross-cutting refactors get a design + adversarial review before code — the silent-update saga is the reference example.

## The B-items (0.4.0 workstreams)

| Item | Scope | Status |
|------|-------|--------|
| **B1 — Farbling in source** | Move fingerprint farbling from injected JS into a Blink source patch | **NEXT SPRINT** — design canonical in `B1-farbling-design.md`; owner-committed |
| **B2 — Header** | Header/omnibox layer | **DONE** as the React-optimization + Header/Omnibox UX pass (native C++ port explicitly rejected); shipped |
| **B3 — Bookmarks** | Bookmarks UI (add/list/delete, favicons) | **SHIPPED** (Windows 2026-06-19; favicon/delete landed beta.23). Mac parity tracked in `MACOS_PORT_0_4_0.md` |
| **B4 — Extensions** | Deferred beyond this sprint | Moved to `../Future-Features/` |

---

## Active docs

| Doc | Purpose |
|-----|---------|
| `CHROMIUM_CEF_SPRINT_KICKOFF.md` | **Active brief** for the next sprint (Chromium/CEF rebuild, farbling→Blink, codecs, version bump) |
| `CHROMIUM_CEF_BUILD_DESIGN_OUTLINE.md` | Design outline of the rebuild sprint (phase order + beta.1 gate) |
| `IMPLEMENTATION_ROADMAP_0_4_0_BETA1.md` | Phase-ordered roadmap + `v0.4.0-beta.1` readiness checklist |
| `chromium-rebuild/` | Per-area deep-research plan docs + the Q1–Q5 answers |
| `B1-farbling-design.md` | Canonical design for the farbling → Blink-patch work (feeds the CEF rebuild) |
| `BROADCAST_AND_EXPLORER_REVIEW.md` | Open scoping/reference for broadcast-freshness + ARCADE + de-hardcode-TAAL work |
| `MACOS_PORT_0_4_0.md` | Living Windows→macOS parity delta log |
| `MAC_WINDOWS_RELAY.md` | Persistent cross-device coordination hub (repointed at the CEF rebuild sprint) |
| `PROFILE_PICKER_UI_REDESIGN.md` | Cosmetic launcher redesign — reconcile whether it landed pre-publish (else pending cosmetic todo) |
| `HelicOps/` | Security-audit sub-archive; `HelicOps/AUDIT_FIX_TRACKER.md` is the live security backlog |

## Archived (see `archive/`)

Completed-phase, shipped-feature, and shelved-plan docs have been moved to `archive/` with a one-line reason (see `archive/README.md`). This includes the header/site-info UX design docs (shipped), the Win10-overlay / single-instance / window-deconfliction investigations (shipped in beta.23), the profile-picker / same-process-refactor / picker-taskbar docs (**shelved**), the post-beta16 / post-beta22 / post-silent-fixlist backlogs (all cleared), the startup-optimization + wallet-UI-bridge migration logs (shipped), the B1-in-source stub, B2-header-to-cpp, B3-bookmarks item cards, the 0.4.0 master plan + orchestration wave graph (superseded by the beta train + CEF kickoff), the Phase-2 bug-hunt log, and the macOS execution briefs/results (done). Superseded auto-update design/build journals from `../DevOps-CICD/` also live in `archive/` (living behavior stayed in the DevOps runbooks).

> DevOps process docs (auto-update, build/release, CEF build) are **not** archived here — they live permanently in `../DevOps-CICD/` as canonical living process docs.
