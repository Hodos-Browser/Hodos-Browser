# Phase 4 — Ecosystem Demos + LLM Dev Guides

Separate (sub)sprint. Depends on Phase 1 + 2A landing.

Four localhost demo servers + LLM-ready integration `.md` guides:

| Demo | Validates | Sprint role |
|------|-----------|-------------|
| `demo-brc100-createaction` | Cluster A — createAction round-trip + domain-permission overlay | regression smoke test |
| `demo-brc121-402` | Cluster A — 402 → payment → retry | Phase 1 acceptance partner |
| `demo-sigma-oauth` | Cluster B — Sigma OAuth login | Phase 2 acceptance partner |
| `demo-brc29-peerpay` | Cluster A — PeerPay send/receive | regression smoke test |

Each demo: `git clone && npm install && npm start` on configurable localhost port.

Each `.md` guide: copy-pasteable into a developer's Claude / Replit / Cursor session, with code blocks, error handling, expected user flow, Hodos as the test wallet.

**Open scope question (Q3):** demo repo location — `Hodos-Browser/demos/` subdir vs separate `hodos-demos/` repo. Recommendation in plan: separate repo (audience is external BSV devs).
