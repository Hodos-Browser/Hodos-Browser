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

---

## Comprehensive demo flow — the canonical narrative

Added 2026-05-18 when Phase 1.5 Step 6 closed. The four targeted demos above each validate one piece, but the canonical user-facing story is a single continuous flow that exercises all three permission engine pillars in order. This is what we'd show in a recorded demo, a conference talk, or a "first time using Hodos" walkthrough.

### Pillar 1 — Connect / Auth

User visits a BRC-100 dApp fresh.

1. dApp calls `getPublicKey({ identityKey: true })` to discover the user's identity
2. Hodos shows `domain_approval` modal with the bundled identity-key checkbox (Step 5 work)
3. User clicks **Allow** — domain trust → "approved", V17 column → 1 if checkbox was on
4. dApp gets the user's identity key, displays profile/handle
5. **Validates:** domain_approval flow, V17 identity-key bundling, Step 5 manifest-or-fallback dispatch

### Pillar 2 — Auto-approve engine in action

Same session, user does something on the dApp (post, sign in, generate content).

1. dApp calls `/createSignature`, `/createHmac`, `/encrypt`, etc. with various protocolIDs and counterparties
2. First time each (protocol, counterparty) tuple appears → **scoped permission prompt** (Commit E)
3. User clicks **"Always allow for this site"** on each — V18 wildcard rows written
4. Subsequent calls with the same scopes → engine returns Silent → no prompts (the user-facing UX win)
5. **Validates:** Commit E end-to-end, wildcard keyId resolution, counterparty table lookup (post-fix `d742fc5`), null-safe accessors (post-fix `83935eb`)

**This pillar also serves as Commit F verification.** If any divergence remains between engine decisions and what dApps experience (the original Commit F payload), it surfaces here. If the demo runs cleanly through this pillar, Step 6 is conclusively done.

### Pillar 3 — Payments

Continue same session. User does something that costs money.

**3a. Direct createAction payment (under cap):**
1. dApp calls `/createAction` with a small spend
2. Engine returns Silent (payment within caps)
3. Tab gets the green-dot animation via `payment_success_indicator` IPC
4. Transaction broadcasts, dApp gets the txid
5. Activity tab shows the spend with proper description

**3b. Direct createAction payment (over cap):**
1. dApp calls `/createAction` with a larger spend
2. Engine returns Prompt with `payment_confirmation` modal
3. User reviews satoshi amount, perTx limit, sessionSpent — clicks Approve
4. Same auto-approve mechanics as 3a from here

**3c. BRC-121 paid content:**
1. User clicks a paid article on `now.bsvblockchain.tech`
2. Site returns 402 → Hodos's `TryHandleBrc121_402` kicks in
3. Within cap → silent auto-pay → green dot
4. Over cap → `payment_pending` spinner + over-cap modal (B+3 work)
5. After approval → one-shot URL flag → reload → article loads
6. Activity tab shows "Paid content — now.bsvblockchain.tech" (B+2 work)

**Validates:** Commit B payment gate, B+1/B+2/B+3 polish, BRC-121 integration (Phase 1), green-dot animation chain.

### Pillar 4 (preview, post-Phase-2) — Sigma OAuth

Once Phase 2 lands, this becomes a fourth pillar: BRC-100 dApp delegates auth to Sigma. Not in scope until Phase 2.

### Why a comprehensive flow over four separate demos

The four targeted demos in the table above are good for regression testing and partner validation — each one exercises a specific cluster cleanly. But for **user storytelling**, the comprehensive flow above is the right shape because:

- It mirrors how a real user encounters Hodos (one continuous session, not four separate apps)
- It exposes the seams between pillars where bugs tend to live (the Commit E discovery process today is evidence)
- It's the artifact we'd hand to a stakeholder, journalist, or developer who asks "what does this do?"

The four targeted demos remain as the **regression test suite** for individual flows. The comprehensive flow is the **demo deliverable**.

### Sequencing inside Phase 4

When Phase 4 starts, build the comprehensive flow FIRST. Each pillar becomes the natural scope of one demo build session:

1. Pillar 1 (Connect/Auth) — likely uses an existing BRC-100 dApp or socialcert.net
2. Pillar 2 (Auto-approve engine) — same dApp, exercise scoped permissions
3. Pillar 3 (Payments) — teragun.com (createAction) + now.bsvblockchain.tech (BRC-121)
4. Pillar 4 (Sigma OAuth) — defer until Phase 2

Sequencing AFTER demo flow is built:

5. Targeted demos from the table above, scoped to areas the comprehensive flow doesn't cover well
6. LLM-ready `.md` integration guides for external devs

### Dependencies before Phase 4 can start

- Phase 1.5 closed (currently Step 6 done, Step 7 pending)
- **Phase 1.6 (Indexer Resilience) MUST land first** — otherwise demo flow is unreliable due to WoC timeouts on the publish path. The `socialcert.net` X verification step inside Pillar 1 will fail intermittently until 1.6 is done.

---

## Originally-planned Commit F now lives here

Phase 1.5 Step 6 Commit F was originally "verify no remaining ProtocolUse engine-vs-inline divergence from Commit A's shadow-mode smoke notes." With Commit E (`dc7d8ba`) migrating ProtocolUse to the engine, that divergence is almost certainly already resolved. The verification step folds naturally into Pillar 2 of the comprehensive demo flow above — if dApps work cleanly through scoped permissions, Step 6 is conclusively done.

If the demo flow surfaces a real divergence here, treat it as a Phase 1.5 patch commit (not a new phase), backport the fix, and continue.
