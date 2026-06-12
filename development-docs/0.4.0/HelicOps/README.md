# HelicOps Audit Review — Start Here

**Created:** 2026-06-09 · **Status:** ✅ Adjudication complete (2026-06-09) — all 479 findings verified; the 3 output docs below are populated. Implementation of `AUDIT_FIX_TRACKER.md` pending.
**Why this is first:** audit findings may change pipeline (Part A) and feature/bugfix (Part B)
priorities, so this review precedes A7 and the rest of the 0.4.0 plan.

## What's here

| Path | What it is | Source |
|------|-----------|--------|
| `HELICOPS_AUDIT_BRIEF.md` | The brief **we** wrote for the auditors (our input) | repo root (moved here) |
| `deliverables/findings.jsonl` | **479 machine-generated findings** — `category, tier, severity, suggested_handling, file, line, snippet` | HelicOps |
| `deliverables/hodos-agent-brief.md` | The brief the audit **agent** actually ran against (~73 KB) | HelicOps |
| `deliverables/hodos-executive-summary.md` | Auditor's human-readable exec rollup | HelicOps |
| `deliverables/hodos-technical-findings.md` | Auditor's technical findings rollup | HelicOps |
| `deliverables/*.html` | Rendered mirrors of the `.md` files (ignore; read the `.md`) | HelicOps |
| `HELICOPS_FEEDBACK.md` | **Output:** itemized feedback for HelicOps (specific false positives, wrong/ambiguous findings, missing context) | ours |
| `AUDIT_FIX_TRACKER.md` | **Output:** findings we AGREE with and will fix before 0.4.0, prioritized | ours |
| `HELICOPS_META_ANALYSIS.md` | **Output:** report-level meta-analysis (good/bad about the report as a whole; signal-to-noise, calibration, coverage) → also sent to HelicOps | ours |

## The stance (non-negotiable)

**Zero-trust on the findings — assume nothing is correct OR incorrect until verified against current
source.** HelicOps is a new tool with new processes; 479 auto-generated findings will include real
issues, false positives, and context-blind flags. The auditors likely lacked context on *why* certain
designs are the way they are (deliberate divergences, load-bearing safeguards). Our job is to
adjudicate each finding, fix what's real, and give the HelicOps team precise feedback.

## Review methodology (run after context clear, with ultrathink)

1. **Orient:** read `HELICOPS_AUDIT_BRIEF.md` (what we asked for), then the exec summary + technical
   findings (the auditor's own prioritization), then sample `findings.jsonl` structure.
2. **Triage 479 findings** by `category` × `tier` × `severity`. Don't go strictly top-to-bottom —
   cluster, because many findings repeat a pattern (one root cause → N findings).
3. **Verify each (or each cluster) against CURRENT source** at the cited `file:line`. Adversarial:
   don't trust the finding, and don't trust our first reaction either. For security findings, default
   to skepticism in BOTH directions.
4. **Classify** each into one of:
   - **AGREE → fix** → add to `AUDIT_FIX_TRACKER.md` (with severity + 0.4.0 in/out).
   - **DISAGREE (false positive / wrong / already-mitigated)** → add to `HELICOPS_FEEDBACK.md` with the
     evidence (cite the code that refutes it).
   - **NEEDS CLARIFICATION (auditor lacked context, or finding is ambiguous)** → `HELICOPS_FEEDBACK.md`
     with the context they were missing.
5. **Cross-check against our known design intent** — load-bearing safeguards and deliberate
   divergences the audit may misread (see "Watch for" below).
6. **Process feedback:** note where the brief/agent-brief should have given more context so HelicOps
   improves (this is a new tool — the feedback is valuable to them and to our next audit).

> **Scale note:** 479 findings is too many for one linear pass. This is a strong candidate for a
> multi-agent verification workflow (fan out: each agent verifies a cluster against source → renders
> AGREE/DISAGREE/CLARIFY with evidence → referee dedups + grades). Confirm the orchestration approach
> with the user before launching (it's substantial). Ultrathink throughout.

## Watch for (things the audit may misread as bugs)

These are deliberate or load-bearing; verify before "fixing":
- **Auto-approve / permission engine** divergences (local SQLite vs on-chain — deliberate, see Phase 1.5).
- **Tab payment badge animation** safeguard (must survive — `payment_success_indicator` IPC chain).
- **TAAL ARC key hardcoded** (intentional, rotated at build time — not an env-var bug).
- **Per-session counters reset on tab close** (by design).
- **HODOS_DEV runtime safeguard**, overlay close-prevention races, etc.
- Anything contradicting `CLAUDE.md` or the memory index — verify against source, docs may be stale.

## Outputs feed the 0.4.0 plan
`AUDIT_FIX_TRACKER.md` becomes part of the 0.4.0 backlog (alongside B-items). `HELICOPS_FEEDBACK.md`
goes back to the HelicOps team. Security-critical AGREE items may reprioritize the whole sprint.
