# Knowledge & memory architecture — where facts live, and what keeps them current

**Filed:** 2026-09-08, from an owner question at the close of beta.3 Phase 8.
**Status:** 🔵 **RESEARCH — deliberately not scheduled.** Owner: *"I don't necessarily want to do
this right now but maybe we should research it and start implementing it at the end of this sprint."*
**Sprint:** end of beta.3, or beta.4 — owner's call. ⛔ Not work until assigned.

## The problem, in one sentence

We have four places a fact can live and no written rule for which one it goes in, so facts land
wherever the session that learned them happened to be looking.

| Store | Versioned? | Visible to Mac Claude? | Visible to a human? | Lifetime |
|---|---|---|---|---|
| Root + per-dir `CLAUDE.md` | ✅ | ✅ | ✅ | permanent |
| `development-docs/` (tickets, contracts, harness) | ✅ | ✅ | ✅ | sprint → permanent |
| **Agent memory** (`~/.claude/…/memory/`) | ❌ | ⛔ **NO** | ⛔ effectively no | per machine, per user |
| Session prompts / relay docs | ✅ | ✅ | ✅ | one round |

🚨 **The load-bearing observation:** agent memory is **per-machine and invisible to the other
agent**. `MAC_RELAY_*.md` exists precisely because memory does not cross machines. Yet a large share
of `MEMORY.md` today is *project* knowledge — closed-phase findings, measured facts about the
codebase — sitting in the one store nobody else can read.

⇒ **Proposed rule to test:** *anything another person or agent would need belongs in the repo.
Agent memory holds only how-I-work-with-this-user and environment traps.*

By that test a large fraction of `MEMORY.md` is misfiled and should migrate into per-directory
`CLAUDE.md` (durable code facts) or `development-docs/` (phase findings).

## Forcing event — this is not hypothetical

`MEMORY.md` hit **24.2 KB against a 24.4 KB read limit** on 2026-09-08 and had to be compacted
mid-session (→ 13.7 KB, overflow moved to `reference_memory_archive_index.md`). It will hit the
ceiling again. Compaction alone is a treadmill; the question is what should never have been there.

## Questions to answer in the research pass

1. **Placement rule.** Ratify or replace the proposed rule above. What is the test a session applies
   in five seconds to decide where a new fact goes?
2. **Migration.** Which existing memory entries are project knowledge? Where does each land?
   ⛔ Do not delete on migration — the memory files carry *why* as well as *what*.
3. **What keeps it current.** CLAUDE.md invariants #11/#12 and §"Context File Maintenance" already
   say to update docs with features. They are **advisory with no gate**, in a project whose culture
   is gates, ratchets and negative controls. Candidate: a sign-off row `preflight.ps1` can check —
   e.g. *every ticket this phase touched has a current status line*, or *root `CLAUDE.md`'s "Last
   reviewed" is not older than N phases*. ⚠️ Working rule 6 applies: the gate is not authored in the
   same commit as the thing it measures.
4. **Ticket queryability.** ⭐ **Cheapest concrete win, probably do this first.** "What is open in
   Phase 8?" currently requires reading `SPRINT_PLAN.md` §4.1 and then opening each ticket. A
   generated `TICKET_INDEX.md` — one row per ticket: phase · severity · status · one line — would
   answer it at a glance. Needs a machine-readable status header on each ticket (most already have
   `**Status:**` and `**Sprint:**` lines; normalise them).
5. **GitHub issues — evaluated and provisionally NO.** Our `TICKET_*.md` files are *richer* than
   issues (evidence tables, negative controls, `GREEN|RED|SUBJECT`) and are versioned beside the
   phase contracts and the code that fixes them. Issues would split the source of truth and lose
   co-location; the only real gain is queryable state, which #4 delivers for far less. ⚠️ Also note
   the two-remote setup (`origin` = dev fork, `release` = signing org) — issues would live on one.
   **Revisit only if a non-Claude collaborator joins.**
6. **Published prior art.** ⛔ Research, do not assume. Known-solid for the *docs* half: ADRs
   (Nygard), Diátaxis, docs-as-code, living documentation, decision logs. ⚠️ For the *agent-memory*
   half there is **no settled best practice** — it is a young area, and a confident answer here
   would be an overclaim. Read before recommending (working rule 5: log what you looked at in
   `PRIOR_ART.md`).

## Non-goals

- ⛔ Not a docs rewrite. The existing structure (root = shape/contracts/pointers, per-dir = inventory)
  is sound and stays.
- ⛔ Not a process for its own sake. If a rule does not prevent a specific failure we have actually
  had, it does not ship.

## Related

- Root `CLAUDE.md` §"Context File Maintenance", invariants #11 / #12
- `development-docs/SCOPING_PROCESS.md` — the existing procedure this would sit beside
- `0.4.0-beta.3/MAC_RELAY_*.md` — the workaround that exists because memory does not cross machines
