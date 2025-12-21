# Development Docs

This folder contains feature research, design exploration, and implementation guides. These are **working plans**, not commitments or a roadmap.

## Purpose

Documents here capture thinking at a point in time. They may be:
- Early research into a problem space
- Design options being weighed
- Implementation guides ready for coding
- Archived plans that were superseded or abandoned

Research, design, and planning are iterative. A document may move back and forth between phases as understanding evolves. Implementation is the phase where code, testing, debugging, and optimization occur—it begins only when a plan is ready.

## Status Convention

Each document should have a status block at the top:

```
Status: Research / Exploration
```

| Status | Meaning |
|--------|---------|
| **Research / Exploration** | Gathering information, understanding the problem |
| **Design / Planning** | Defining approach, weighing trade-offs |
| **Ready for Implementation** | Plan is complete and consistent with project architecture |
| **Implemented** | Code exists; document is reference material |
| **Archived / Superseded** | No longer current; kept for historical context |

Status reflects current thinking, not priority or guarantees.

## Before Implementation

Before moving a document to "Ready for Implementation," sanity-check the plan against:
- `CLAUDE.md` — invariants and safety rules
- `PROJECT_OVERVIEW.md` — architecture and security model
- `THE_WHY.md` — design philosophy and trade-offs

If the plan conflicts with these, revise the plan or surface the conflict for discussion.

## How Claude Should Use These Docs

- Treat documents as context and guidance, not mandates
- Do not assume everything described must be implemented
- Prefer the most recent document marked "Ready for Implementation"
- If a plan conflicts with current architecture or invariants, surface the conflict before writing code
- When implementing, follow the plan's approach but adapt to what the code actually requires
