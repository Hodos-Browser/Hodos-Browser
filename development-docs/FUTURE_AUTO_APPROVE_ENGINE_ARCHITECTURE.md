# Future: "Cleanest, Fastest, Most Secure" Auto-Approve Engine

> **Status:** Architectural vision — NOT for current sprint. Captured 2026-05-30
> during Phase 2.5 IPC bridge work for back-of-mind / Phase 4+ planning.
>
> This document is the answer to "if we were starting from scratch today,
> knowing what we know, how would we build the auto-approve engine?" — saved
> here so we don't lose the thinking the next time the question comes up.
> Re-evaluate after Phase 2.5 lands and again before any Phase 4 cleanup.

## TL;DR

Engine lives in **Rust**, alongside the keys it protects. C++ becomes a thin
proxy + presentation layer with no business logic. Modal flow becomes an
explicit `202 PENDING + re-issue` state machine instead of implicit closures.

## Today's architecture (what we have)

Two permission systems that must stay aligned:

- **C++ `PermissionEngine`** in browser process — fine-grained gates: per-tx
  limits, per-session caps, rate limiting, scoped grants, privacy perimeter,
  modal dispatch.
- **Rust `check_domain_approved`** in wallet — coarse-grained: approved /
  blocked / unknown.

Alignment between them happens via the `X-Requesting-Domain` HTTP header and
the shared `domain_permissions` SQLite table. Drift between the two systems
is a security bug class — each gate added to one has to be added to the
other, or the C++ side has to consult the Rust side over HTTP.

C++ has `DomainPermissionCache` in-memory mirror of the `domain_permissions`
table so its engine decisions are sub-millisecond. This is the "fast cache"
that would change shape under the future design (see "Performance" below).

## The future design

Every wallet call returns one of three response codes:

| Status | Meaning | What C++ does |
|---|---|---|
| `200 OK` + result | Engine said silent approve, work done | Forward response to renderer |
| `202 PENDING` + prompt context | Engine wants user decision | C++ shows modal; on resolve, re-issues call with `X-User-Approved: <approvalId>` header |
| `403 FORBIDDEN` + reason | Engine denied | Forward error to renderer |

The `approvalId` is a nonce minted by Rust when it returns 202. Rust stores
the pending request indexed by `approvalId`. When C++ re-issues with
`X-User-Approved: <approvalId>`, Rust looks it up, verifies the user's
decision, and either processes the call (if approved) or rejects (if denied).
Approval records get logged to an audit table for traceability.

## Why this is the cleanest

- **One source of truth.** All permission policy in one place, next to the
  keys it's protecting. No more "we changed the gate logic in C++, did we
  remember to mirror it in Rust?"
- **Stateless C++.** Browser process becomes a thin proxy + UI layer.
  Browser-process code has no security logic, which makes it dramatically
  easier to audit. The blast radius of a browser-process bug stops at "wrong
  pixels rendered" instead of "let an unapproved dApp spend the user's coin."
- **API-surface-agnostic.** Future MCP servers, REST clients, mobile
  apps, CLIs — all automatically get the engine because they enter the wallet
  through Rust. No "remember to add gate X to surface Y" coordination.
- **Modal flow is explicit.** The `202 PENDING + re-issue` contract is a
  clean documented state machine. Today's flow uses closures, pending-request
  maps, and timer-based timeouts in a way that's hard to reason about and
  full of subtle race conditions.

## Why this is the most secure

- **Security boundary aligns with key boundary** (the wallet process). Today
  the C++ engine is "between" the renderer and the keys but has its own
  state and cache; if a bug in C++ misroutes a request, that's a security
  incident. Under the future design, the security boundary is one process
  away from the keys, not two.
- **No bypass via different API surface.** Today, anything that reaches the
  Rust wallet via a path that's not CEF's resource interception (e.g.
  Phase 2.5's IPC bridge in its current state) bypasses the C++ engine.
  Under the future design, *every* API surface enters through Rust, so
  there's no "C++ skipped" failure mode.
- **Auditability.** Every approval produces a logged record (`approvalId`,
  domain, endpoint, body, user decision, timestamp). Today's auto-approve
  cascade has logging but isn't structured for audit.

## Performance discussion

Today the C++ `DomainPermissionCache` makes "is this domain approved"
decisions in **<1ms** without touching the wallet. Under the future design,
every wallet call has to go to Rust — but **the wallet call had to go to
Rust anyway** to do the signing/keying work. The cache only matters for
*pre-flight* checks ("should we even inject the shim on this page?"), not
for actual wallet calls.

**Pre-flight pattern preserved via tiny C++ mirror:**

- `should_inject_shim_on_this_page(host)` → checks a small in-memory
  `Set<approved_host>` mirror
- The mirror is updated by a long-lived stream from Rust (similar to the
  existing `domain_permission_invalidate` IPC, but bidirectional)
- Sub-millisecond pre-flight checks preserved
- The mirror contains ONLY the approved-host set — no engine logic, no
  per-tx limits, no rate state. Just a bool. Can't drift from Rust because
  it's append-only from Rust's announcements.

For actual wallet calls (the 95% case):
- Engine decisions in Rust take ~1-5ms (pure CPU + in-memory state)
- The network/IPC roundtrip + signing dominates total latency anyway
- Net change in user-perceived latency: undetectable

## What the migration would look like

Phase 4ish (2-4 week careful refactor, each step shippable and rollback-safe):

1. **Build the engine in Rust as a new module.** Don't touch C++ yet. Module
   is self-contained, no Actix integration, just pure decision logic.
2. **Test the Rust engine against the same test vectors the C++ engine
   passes today.** Cross-validate decisions to confirm parity.
3. **Add the `202 PENDING` response shape to Actix routes** behind a feature
   flag. Default off — existing C++ engine still in charge.
4. **Migrate one endpoint at a time** from "C++ engine + Rust wallet" →
   "Rust engine + wallet". Per-endpoint flag flip with rollback ready.
5. **Last endpoint migrated, C++ engine becomes dead code.** Run for a
   week with both engines in shadow mode (engine in Rust authoritative,
   C++ engine logs disagreements) to catch any divergence.
6. **Delete C++ engine.** Browser process becomes a thin presentation
   layer.

## When to revisit this

Triggers that should prompt re-reading this document:

- Phase 4 planning kicks off
- A new API surface (MCP, REST, mobile, CLI) is being added — that's the
  cleanest moment to commit to engine-in-Rust because the new surface
  doesn't have to reimplement the engine
- C++ engine drift bug (incident where C++ and Rust disagreed about a
  permission decision)
- Engine modification that touches both C++ and Rust simultaneously — if
  we're already touching both sides, doing the migration may be cheaper
  than the change

## Counterarguments (acknowledged + addressed)

| Concern | Response |
|---|---|
| "Loses fast C++-side cache" | Mostly theoretical. Pre-flight checks preserved via tiny mirror; actual calls hit Rust anyway. |
| "Modal coordination across processes is harder" | The `202 PENDING + re-issue` contract is explicit and testable. Today's closure-based flow has more hidden state. |
| "Bigger lift than option 1" | True. ~2-4 weeks careful work. Pays back forever in audit cost + new-surface integration cost. |
| "We might want to keep the engine in browser process for offline mode" | Engine has no offline mode benefit — every wallet call needs the wallet anyway. |
| "Microservice extraction (option 3) gives even cleaner isolation" | Adds IPC overhead without proportional gain. Engine has no business being a separate process — it has to live alongside SOMETHING (keys, UI, or its own process), and the cleanest answer is alongside the keys. |

## Related

- `development-docs/architecture/AUTO_APPROVE_ENGINE.md` — current state
  documentation of the C++ engine (to be filled out as part of Phase 2.5
  sub-phase A)
- `development-docs/architecture/WALLET_API_MAP.md` — endpoint × gate ×
  shim-call mapping (also Phase 2.5 sub-phase A)
- `development-docs/Sigma-BRC121-Sprint/phase-2-window-cwi-shim/PHASE_2_5_IPC_REFACTOR.md`
  — the immediate Phase 2.5 work that surfaced these architectural
  questions
