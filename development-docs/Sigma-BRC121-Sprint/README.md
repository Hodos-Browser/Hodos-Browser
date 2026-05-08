# Sigma Auth + BRC-121 Payments + Ecosystem Demo Sprint

Status: research complete; phase structure revised 2026-05-05; ready to pick up implementation after context clear + b-open-io plugin install.

## Sprint anchors (final)

- **BRC-100 / `window.CWI` alignment + audit** — verify Hodos's surface matches the canonical 28 methods Yours's `brc100-remote` ships.
- **`window.CWI` / `window.yours` / `window.panda` injection** — the load-bearing interop work. Yours is *removing* `window.yours`; we keep it working during the transition.
- **BRC-121 Simple 402 Payments** — pairs naturally with our auto-approve model.
- **1Sat Ordinals support** — promoted from deferred. Final piece after the shim lands.
- **Sigma OAuth as a normal provider** — works in Hodos with no special code (like Google/X/GitHub login). Goes in the demo.
- **Ecosystem demos + LLM-ready dev guides** — distribution + adoption.

**CWI** = **Chrome Wallet Interface** (BRC-100's canonical injected-provider API name).

## Phases (revised 2026-05-05)

| Phase | Folder | Type | Status |
|-------|--------|------|--------|
| **0.1** — BRC-100 Audit | `phase-0.1-brc100-audit/` | research/spec | ✅ Complete (2026-05-06) — `AUDIT_RESULTS.md` |
| **0.2** — `window.yours` Shim Design | `phase-0.2-window-yours-shim-design/` | research/spec | ✅ Complete (2026-05-06) — `SHIM_TRANSLATION_SPEC.md` |
| **1** — BRC-121 implementation | `phase-1-brc121/` | implementation | Ready (small, ~150 LOC, reuses PeerPay) |
| **1.5** — BRC-100 Surface Completion | `phase-1.5-brc100-surface-completion/` | implementation | Scoped (2026-05-06) — gates Phase 2; awaiting user review of `PERMISSION_UX_DESIGN.md` open questions |
| **2** — `window.CWI`/yours/panda shim | `phase-2-window-cwi-shim/` | implementation | Gated on 1.5 (was 0.1 + 0.2; now 1.5 absorbs the implementation prerequisites) |
| **3** — 1Sat Ordinals | `phase-3-ordinals/` | implementation | Gated on Phase 2 (ordinal calls route through the shim) |
| **4** — Demos + LLM dev guides | `phase-4-demos/` | implementation + writing | Gated on 1, 2, 3. Sigma OAuth demo (as normal provider) folded in here. |

**Cancelled:** Sigma OAuth interception (was Phase 2B). Iframe-signer architecture makes it structurally impossible. See `OPEN_QUESTIONS.md` OQ#1.

**Demoted:** BSM/BRC-77 signing primitives (was Phase 2A). Drop unless we want a content-signing demo, in which case fold into Phase 4.

## Load-bearing UX safeguards (preserved across all phases)

These existing features are non-negotiable and survive every refactor in this sprint:

- **Tab payment badge animation** (green dot) fires on every auto-approved payment — `HttpRequestInterceptor.cpp:1656-1681` → `simple_render_process_handler.cpp:1020` → `useTabManager.ts:141`. The user's primary defense against a site spamming auto-approved payments under their nose. Phase 1.5 must keep it firing through the new permission engine; Phase 2 V8 shim payments must trigger it too.
- **Right-click "Manage Site Permissions"** context menu (`MENU_ID_MANAGE_PERMISSIONS` at `simple_handler.cpp:6696`) — quick revoke for any site at any time.
- **`DomainPermissionForm` "Always notify me" toggle** — zeros all limits; the cautious-user opt-in path.
- **Privacy perimeter** — identity-key reveal, key-linkage proofs, sensitive cert fields, large spends ALWAYS prompt regardless of any setting.

## Notes for future ecosystem submission

Captured during sprint design phases for future BRC submissions / community engagement after Phase 4 demos:

- **`wallet-manifest.json` format** — Hodos defines a BSV-native manifest format (Phase 1.5/2). After demos validate the UX, draft a BRC for ecosystem standardization. Crib useful concepts from ERC-7715 (required `expiry`, optional `required` flag, extensible `policies` array) without binding to EVM-specific primitives.
- **Cert-field `sensitivityHints`** — Hodos ships its own classifier (Phase 1.5). Future BRC could let certifiers publish sensitivity metadata alongside type IDs.
- **`bsv:announceProvider`** — multi-provider discovery event (Phase 2). EIP-6963 equivalent. Submit as BRC after we ship.
- **Action registry** — translate protocolIDs to plain verbs in connect prompts. Research due right after Phase 1.5: BSVA standard? de-facto convention? long-term adoption likely?

## Strategic context (one-page summary)

Real-world adoption check from research:

| Surface | Real apps verified today | Effort | Verdict |
|---------|--------------------------|--------|---------|
| BRC-100 (already implemented in spirit) | Babbage catalog, Yours-after-imminent-migration | Audit (Phase 0.1) | High priority — the convergence point |
| **`window.CWI` injection** (`window.yours`/`panda` aliases) | 1sat.market, 3DOrdi, Treechat, ecosystem | M | **Highest leverage** — Yours is *removing* `window.yours` |
| BRC-121 | Zero production servers known | XS (~150 LOC, reuses PeerPay) | Ship — cheap speculative bet |
| 1Sat Ordinals | Many apps need it | L (5–7 sprints worth, can compress with shim approach) | Real phase — gated on shim |
| Sigma OAuth as a normal provider | BSVradar (1 confirmed) | Trivial (zero special code) | Demo it; works |
| ~~Sigma OAuth interception with Hodos identity~~ | n/a | n/a | **CANCELLED** — iframe signer blocks it |

## Top-level docs

- `README.md` (this file) — sprint overview + revised phase structure + strategic context
- `CHECKLIST.md` — cross-phase work checklist (matches new phase structure)
- `OPEN_QUESTIONS.md` — answered Sigma OQs + scope questions Q1–Q19
- `ARCHITECTURE.md` — placeholders for current + sprint plug-in diagrams (TBD)
- `WALLET_PROVIDER_LANDSCAPE.md` — wallet/app inventory; pre-fact-check view, kept as historical
- `YOURS_CWI_MIGRATION.md` — load-bearing reference: 28-method `WalletInterface`, comparison table, permission-model notes
- `BRAVE_WALLET_REFERENCE.md` — patterns to adopt (V8 Proxy, descriptors, EIP-6963 equivalent, etc.)
- `AUTO_APPROVE_RATIONALE.md` — why Hodos's 3-layer auto-approve differs from Brave; talking points for demo + docs
- `_DRAFT_RECOVERED_PLAN.md` — full crashed-session plan (safety net; can be deleted once no longer needed)

## Phase-specific research/findings already in place

- `phase-0-research/FACT_CHECK_RESULTS.md` — 2026-05-05 ecosystem fact-checks (Yours BRC-100 imminent, Treechat surface, BSVradar Sigma block, Zoide irrelevant)
- `phase-2-sigma-auth/RESEARCH_FINDINGS.md` — Sigma protocol + ecosystem deep dive (note: §A4's "BAP-permissive" reading is superseded by `FACT_CHECK_RESULTS.md` Q3 — iframe signer kills it)
- `phase-2-sigma-auth/BRC103_SIGMA_AUTH_GUIDE.md` — developer-facing OAuth integration guide (still useful for Phase 4 demo's "Sign in with Sigma" button)
- `phase-2-sigma-auth/BRC103_SIGMA_COMPARISON_AND_IMPLEMENTATION.md` — original protocol research (Sigma-interception sections obsolete; BRC-77/BSM signing primitives section is reference material if we ever want content signing)
- `phase-3-ordinals/RESEARCH_FINDINGS.md` — 1Sat ordinals protocol, indexer endpoints, integration sizing

## Pick-up-here guide for future sessions

When picking up cold, read in this order:

1. **This `README.md`** — for the phase structure and strategic context
2. **`YOURS_CWI_MIGRATION.md`** — the load-bearing reference for the shim work
3. **`phase-0-research/FACT_CHECK_RESULTS.md`** — what's actually true about the ecosystem
4. **`OPEN_QUESTIONS.md`** — scope decisions still pending
5. **`BRAVE_WALLET_REFERENCE.md`** — security/UX patterns when starting Phase 2

Then jump into the phase folder for whatever you're about to work on.
