# 0.4.0 Build — Sprint

**Created:** 2026-06-01
**Status:** 🚧 Planning (research phase — no source code yet)
**Last shipped:** `v0.3.0-beta.15` · **Process home:** `../DevOps-CICD/` · **Triage:** `../DevOps-CICD/TRIAGE.md`

Build-specific features and changes targeting the **0.4.0** release. Process/CI work that recurs
every build lives in `../DevOps-CICD/`, not here.

**Scope philosophy (locked):** lean-ambitious. We'll likely do most/all of it, but **do not commit
the big refactors (B1, B2, any Brave migration) until the Brave-fork feasibility research lands.**
Driving principle: *do it the right way the first time — no re-refactor in two months.*

## Phase / item status

> 🔝 **TOP PRIORITY: HelicOps audit review** (`HelicOps/README.md`) — 479 findings to adjudicate
> before finalizing this sprint; results may reprioritize everything. Do this first.

| ID | Item | Type | Size | Status |
|----|------|------|------|--------|
| AUDIT | HelicOps audit review | review | large | 🚧 setup done; deep review pending (`HelicOps/`) |
| B3 | Bookmarks functional | feature | med | ⏳ stub — can start (not Brave-gated) |
| B4 | Extensions + wallet deconfliction | feature | large | ⏳ stub — untrusted docs relocated, need verification |
| B1 | Farbling into Chromium source | refactor | large | 🔓 unblocked; design done (`B1-farbling-design.md`) — persistent per-profile seed |
| B2 | Header → C++ | refactor | large | ⛔ own dedicated planning session (post-A4) |

## Sequencing

1. **A4 Brave-fork feasibility spike** (`../DevOps-CICD/research/BRAVE_FORK_FEASIBILITY.md`) — gates B1/B4 and frames B2.
2. Non-gated work can proceed in parallel: **B3 bookmarks**, and the **A7 test strategy** (process side).
3. After A4: commit the 0.4.0 cut line, then deep-dive B1/B4; **B2 gets its own multi-agent session.**

## Item docs

| Doc | Item |
|-----|------|
| `B1-farbling-in-source.md` | B1 |
| `B2-header-to-cpp.md` | B2 |
| `B3-bookmarks.md` | B3 |
| `B4-extensions.md` | B4 |
| `browser-extensions/` | B4 background research — **UNTRUSTED**, relocated from `../browser-extensions/` |

## What this sprint does NOT do

- No source code until each item completes research → design → its own plan → kickoff → approval.
- Does not touch the in-flight feature work (other context) or the paused Dolphin Milk + Edwin work.
- Does not redefine the release pipeline — that's `../DevOps-CICD/` (Part A).
