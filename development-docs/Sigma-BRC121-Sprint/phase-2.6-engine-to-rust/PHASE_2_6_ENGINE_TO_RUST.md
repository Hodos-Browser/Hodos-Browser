# Phase 2.6 — Auto-Approve Engine to Rust

> **Status:** Plan locked 2026-06-02 after architectural discussion session. Multi-week
> work; sub-phases 1–7 pending across multiple focused sessions.
>
> Sequenced immediately after Phase 2.5 closed (2026-06-02). Per
> `feedback_research_first_do_it_once` memory, the architectural research,
> discussion, and decision-locking happened BEFORE any source code work began.
> This doc is the gate for sub-phase 1.
>
> **Approval gate:** Sub-phase 1 work does not start until the user signs off on
> this plan. Open questions in §11 require explicit answers before specific
> sub-phases land.

## TL;DR

The C++ `PermissionEngine` migrates to Rust as the canonical implementation. C++ becomes a thin proxy + presentation layer — it captures origin, forwards wallet calls to Rust, opens modals on `202 PENDING` responses, fires the green-dot animation on `200 OK` payment responses, and runs the `IsInternalOrigin` bypass. Everything else (cap checks, rate limits, session counters, sub-permission lookups, scoped-grant cascade, privacy-perimeter rules, modal-decision logic) moves to Rust.

Engine state (counters, approval map, sub-permission caches) moves with the engine. UI state (modal dispatch, indicator animation, tab-ID translation) stays in C++. The `202 PENDING + re-issue with X-User-Approved` state machine is the new wire contract between the two.

5 feature flags scaffold the migration during dev testing — one per CallKind class — then get deleted in the final cleanup commit alongside the C++ engine.

## Why this exists

Phase 2.5 (closed 2026-06-02) wired external dApp traffic through the IPC bridge so the C++ engine cascade fires for `wallet_call` IPC requests. That solved the CSP+CORS bypass problem. But it left a structural issue exposed: **the C++ engine and Rust's `check_domain_approved` are two independent permission systems that must stay aligned.**

The drift risk is concrete:
- Every gate added to one side has to be added to the other (or the C++ side has to consult the Rust side over HTTP)
- The C++ engine's decisions live in browser-process code, two processes away from the keys it's protecting
- The blast radius of a browser-process bug stops at "wrong pixels rendered" today only because of defense-in-depth on the Rust side — but a missed gate in C++ could let an unapproved dApp signal "I'm approved" to a Rust handler that trusts the upstream check
- New API surfaces (future MCP servers, REST clients, mobile apps) all enter the wallet through Rust, which means they'd skip the C++ engine entirely without a coordinated reimplementation

Phase 2.6 collapses this to one source of truth. All permission policy lives next to the keys it's protecting. C++ engine becomes dead code, then gets deleted.

Per `FUTURE_AUTO_APPROVE_ENGINE_ARCHITECTURE.md` (the vision doc captured 2026-05-30 during Phase 2.5 planning), this was always the planned destination — Phase 2.5 was specifically architected so its 4 locked decisions (D1–D4) are forward-compatible with the Phase 2.6 shape.

## Goal

Port the `PermissionEngine::Decide()` logic to Rust as `hodos_permission_engine::Decide()`. Migrate engine-owned state (SessionManager counters, sub-permission caches, approval map) to Rust. Establish the `202 PENDING + re-issue` wire contract. C++ keeps modal dispatch, payment indicator IPC fire, `IsInternalOrigin` bypass, and `wasAutoApprovedPayment` derivation (local-only, no signaling needed).

End state:
- One source of truth for all permission policy (Rust)
- C++ wallet code has zero security logic — pure proxy + UI
- Every API surface that talks to Rust automatically gets the engine
- Engine decisions are logged to `permission_audit_log` for compliance/forensics

## Out of scope

- BRC-121 paid retry path migration (`TryHandleBrc121_402`) — sequenced as polish commit AFTER all 5 main flags flip. Out of scope for the main 2.6 migration.
- Internal-only wallet endpoints (clusters 1/3/4/6/8/11/13/14 from `WALLET_API_MAP.md`) — stay non-engine-routed. Defense-in-depth `check_domain_approved` sufficient.
- `Set<approved_host>` shim-injection mirror — deferred. The shim is unconditionally injected on every external https main frame today (`simple_render_process_handler.cpp:889-898`); the mirror is net-new architecture, not preservation. Open question if you ever want per-domain shim gating.
- React modal layer rewrite — `BRC100AuthOverlayRoot.tsx:335-440` keeps its `URLSearchParams` consumer; C++ translates JSON → URL query string at the modal-dispatch seam.
- Phase 3 (ordinals) — sequenced after 2.6 closes.

## Locked decisions

Five architectural decisions resolved in the 2026-06-02 discussion session. All are forward-compatible with the Phase 2.5 D1–D4 decisions (which are the literal prep for 2.6).

### LD1 — Module structure: two-layer Rust split

**Locked:** Pure crate + actix-integrated service module.

```
rust-wallet/
├── crates/
│   └── hodos_permission_engine/        ← pure crate (no actix, no sqlite, no http, no AppState)
│       ├── src/
│       │   ├── lib.rs                  ← public: Decide(ctx) -> Decision
│       │   ├── context.rs              ← PermissionContext struct (data in)
│       │   ├── decision.rs             ← PermissionDecision enum + EngineReason enum (data out)
│       │   └── matrix_c.rs             ← branch helpers (privacy_perimeter, payment, scoped_grant, ...)
│       └── tests/                      ← ports C++ engine tests verbatim
└── src/
    └── permission_service/             ← actix-integrated service module
        ├── mod.rs
        ├── state.rs                    ← PermissionService struct (approval map + session counters + audit handle)
        ├── handlers.rs                 ← /engine/shadow-decide endpoint, approval lookup, etc.
        ├── context_builder.rs          ← reads AppState → PermissionContext
        ├── audit.rs                    ← writes engine_shadow_log + permission_audit_log
        └── flags.rs                    ← EngineFlags struct (5 boolean flags + env-var parser)
```

`AppState` gains one new field: `permission: Arc<PermissionService>`. Mirrors the proven Phase 1.6 `services: Arc<WalletServices>` facade pattern at `rust-wallet/src/main.rs:87`.

**Workspace conversion:** `rust-wallet/Cargo.toml` converts from a single-crate `[package]` to a hybrid **workspace + package root** — `[workspace]` table with `members = ["crates/hodos_permission_engine"]` co-exists with the existing `[package]` for `hodos-wallet`. This is a supported Cargo pattern; no repo-root restructure needed.

**Rationale:** Mirrors today's C++ shape (`cef-native/include/core/PermissionEngine.h:18-21` says "PURE LOGIC. No CEF dependencies"). The 33 engine tests in `cef-native/tests/permission_engine_test.cpp` (verified 2026-06-02 — original plan inherited a stale "46+" estimate) port verbatim to the pure crate. Workspace boundary enforces "no actix imports" at compile time (a module could drift over time).

### LD2 — Wire contract: 202 PENDING + re-issue

**Locked:** Every shim-reachable wallet endpoint returns one of three response codes:

| Status | Meaning | What C++ does |
|---|---|---|
| `200 OK` + result | Engine said Silent, wallet processed | Forward response to renderer; derive `wasAutoApprovedPayment` locally; fire indicator if payment |
| `202 PENDING` + envelope | Engine wants user decision | Open modal via translated query-string params; on resolve, re-issue with `X-User-Approved: <approvalId>` header |
| `403 FORBIDDEN` + reason | Engine denied | Forward error to renderer |

`202 PENDING` body shape (versioned):

```json
{
  "status": "pending",
  "approvalId": "<128-bit hex nonce>",
  "promptType": "payment_confirmation|identity_key_reveal|key_linkage_reveal|certificate_disclosure|protocol_permission_prompt|basket_permission_prompt|counterparty_permission_prompt|domain_approval|manifest_connect_bundle|brc100_auth",
  "engineReason": "per_tx_limit|session_cap|rate_limit|silent_within_caps|...",
  "ttlMs": 600000,
  "schemaVersion": 1,
  "promptPayload": { /* type-specific — see table below */ }
}
```

`promptPayload` per type (matches `BRC100AuthOverlayRoot.tsx:335-440`'s `applyParams` consumer exactly):

| `promptType` | `promptPayload` fields |
|---|---|
| `payment_confirmation` / `rate_limit_exceeded` | `satoshis`, `cents`, `exceededLimit`, `perTxLimit`, `perSessionLimit`, `sessionSpent`, `rateLimit`, `maxTxPerSession` |
| `identity_key_reveal` | (empty — modal renders from origin alone) |
| `key_linkage_reveal` | `kind` (`counterparty`\|`specific`), `verifier`, `counterparty`, `protocol`, `keyID` |
| `certificate_disclosure` | `certType`, `certifier`, `fields` (array) |
| `protocol_permission_prompt` | `protocolLevel`, `protocolName`, `protocolKeyId`, `protocolCounterparty` |
| `basket_permission_prompt` | `basket`, `basketAccess` |
| `counterparty_permission_prompt` | `counterparty` |
| `domain_approval` | (empty) |
| `manifest_connect_bundle` | `manifest` (stringified JSON — preserves `applyParams` parser at L425-434) |
| `brc100_auth` (legacy) | (empty) |

**Domain/endpoint/body NOT echoed** — C++ already has them in `PendingAuthRequest` (Phase 2.5 D2). Rust persists them server-side for audit. Round trip is symmetric.

**`approvalId`** — 128-bit hex (32 chars), generated by `rand::random::<u128>()` in Rust, single-use, 10-minute TTL (matches production `kPromptAuthTimeoutMs = 600_000`).

**`engineReason`** — typed enum in the engine crate, kebab-case serialization. Initial draft vocabulary: `per_tx_limit`, `session_cap`, `rate_limit`, `price_unavailable`, `max_tx_per_session`, `protected_basket`, `manifest_required`, `trust_blocked`, `trust_unknown`, `privacy_perimeter_no_grant`, `silent_within_caps`. List grows as branches land.

**`schemaVersion`** — bump on breaking changes only. Additive fields stay on v1 (old consumers ignore unknown fields).

**C++ JSON→query-string translator** — single new helper at the 202 response handler in `simple_handler.cpp`. ~30 lines. Reads `promptPayload`, builds URL query string, invokes existing `CreateNotificationOverlayTask` with `extraParams_` populated. React modal layer (`BRC100AuthOverlayRoot.tsx`) unchanged.

### LD3 — Feature flag structure: 5 flags, one per CallKind class

**Locked:** Five flags matching `PermissionEngine.h:32-55`'s `PermissionCallKind` enum branch structure exactly:

| Flag | Covers CallKinds | Endpoints |
|---|---|---|
| `engine_rust_privacy_perimeter` | `IdentityKeyReveal`, `CounterpartyKeyLinkage`, `SpecificKeyLinkage`, `SensitiveCertField` | `/getPublicKey` (identity-key shape), `/revealCounterpartyKeyLinkage`, `/revealSpecificKeyLinkage`, `/proveCertificate` (sensitive fields) |
| `engine_rust_scoped_grant` | `ProtocolUse`, `BasketAccess`, `CounterpartyUse` | `/createSignature`, `/createHmac`, `/verifyHmac`, `/encrypt`, `/decrypt`, `/wallet/encrypt-bie1`, `/wallet/decrypt-bie1`, `/listOutputs`, `/relinquishOutput`, `/listMessages`, `/acknowledgeMessage` |
| `engine_rust_payment` | `Payment` | `/createAction`, `/signAction`, `/processAction`, `/acquireCertificate`, `/sendMessage`, `/transaction/send` |
| `engine_rust_cert_disclosure` | `CertificateDisclosure` (non-sensitive) | `/proveCertificate` (non-sensitive fields) |
| `engine_rust_domain_trust` | `DomainTrust`, `GenericApproved` | everything else (first-touch `domain_approval` / `manifest_connect_bundle` paths) |

**Mechanism:** env vars read at Rust process start, exposed through `AppState.engine_flags: EngineFlags` (a struct of `bool` fields). Same env-var pattern as `HODOS_DEV=1` in the dev runbook.

**Migration model:** ALL flags default OFF in every commit. Developer flips per-class flag ON during dev testing of that class. Once dev testing is satisfied, flag stays ON. After all 5 are ON and end-to-end smoke passes, the final cleanup commit (sub-phase 7) deletes all 5 flags + the C++ engine in one sweep.

**No production-vs-test distinction.** Hodos ships as a desktop installer — there is no "10% of users on the new path" model. The flags are dev-time testing scaffolding, not a production rollback mechanism. No soak periods; driven by dev-test confidence.

**Sub-phase ordering** (smallest blast radius first):
1. Privacy Perimeter — lowest call volume, highest security value
2. Scoped Grant — moderate volume, well-isolated
3. Payment — highest stakes (money)
4. Cert Disclosure — moderate volume
5. Domain Trust — last (highest volume, catch-all)

### LD4 — SessionManager migration boundary

**Locked:** Engine state moves; UI feedback stays.

| Concern | Lives after 2.6 |
|---|---|
| Session counters (`sessionSpentCents`, `paymentCountThisSession`, `paymentRequestsThisMinute`) | **Rust** (`permission_service::state.rs`) — engine state |
| Cap-checking logic | **Rust** — engine logic |
| Rate-limit 60s sliding window | **Rust** — engine state |
| `wasAutoApprovedPayment` derivation | **C++** — local-only from endpoint + response code + body (`HttpRequestInterceptor.cpp:3683-3694`) |
| `cents` extraction + `BSVPriceCache` | **C++** — avoids unnecessary round-trip; indicator already needs it |
| Green-dot animation fire (`OnWalletCallSuccess`) | **C++** (`HttpRequestInterceptor.cpp:1320-1350`) — UI concern |
| Tab-ID translation (`Tab::id` ≠ `CefBrowser::GetIdentifier()`) | **C++** — CEF internals |

**Session keying:** Rust tracks counters by `(browserId, domain)` tuple — preserves current per-tab-per-domain semantics (matches `SessionManager.h`'s `BrowserSession` shape).

**Tab close hook:** C++ sends new `session_close` IPC to Rust with `browserId` when a tab closes. Rust drops counters for that browserId. Mirrors today's `SessionManager::clearSession(browserId)` semantics exactly. Idle-timeout alternative rejected — would silently expire counters mid-session.

**Why no Rust→C++ signaling for `wasAutoApprovedPayment`:** C++ already derives this locally from data passing through it (`isPaymentKind` from endpoint URL + HTTP success + body error parse). The derivation runs in production today at `HttpRequestInterceptor.cpp:3683-3694`. Adding a response field or header would be redundant signaling.

**Critical invariant:** the payment_success_indicator IPC chain (`OnWalletCallSuccess` → `TabManager::GetTabIdForBrowserIdentifier` → header browser IPC → `useTabManager.ts:141`) MUST survive every sub-phase. Memory `payment_animation_safeguard` is the contract.

### LD5 — Shadow mode: C++ authoritative + Rust async compare

**Locked:** During each sub-phase migration, both engines run on every wallet call. C++ engine stays authoritative until that CallKind class's flag flips. Rust engine runs in shadow — its decision is computed and compared, but doesn't affect the user.

**Fire-and-forget over localhost HTTP** — the hard invariant:

1. C++ engine decides Silent/Prompt/Deny; wallet call proceeds normally
2. C++ posts a single fire-and-forget HTTP POST to `http://127.0.0.1:31301/engine/shadow-decide` on `TID_FILE_USER_BLOCKING` worker thread
3. POST body carries the `PermissionContext` C++ just consumed plus C++'s decision as a hint:
   ```json
   {
     "context": { /* the PermissionContext */ },
     "cppDecision": "silent|prompt|deny",
     "cppPromptType": "payment_confirmation",
     "cppReason": "silent_within_caps"
   }
   ```
4. Rust runs `hodos_permission_engine::Decide(ctx)`, compares to C++'s hint, writes one row to `engine_shadow_log` SQLite table:
   ```
   (call_kind_class, endpoint, domain, cpp_decision, rust_decision,
    cpp_reason, rust_reason, agreement: bool, context_hash, observed_at)
   ```
5. C++ does NOT read Rust's response. Critical path stays exactly as fast as today.

**Inverted after flag flip:** Once a CallKind class's flag flips, Rust becomes authoritative for that class. C++ inline engine still runs in shadow — same comparison logic, just inverted. Same `engine_shadow_log` table, same fire-and-forget pattern.

**Scope:** Shadow runs for ALL 5 CallKind classes from sub-phase 1. Once the infrastructure exists, additional comparisons are free, and more data catches more drift.

**Review:** CLI/SQLite queries during dev (`SELECT * FROM engine_shadow_log WHERE agreement = 0 ORDER BY observed_at DESC LIMIT 50;`). Wallet UI panel only added if disagreement volume warrants a visual surface.

**Cleanup (sub-phase 7):** Shadow POST sites deleted from C++; `/engine/shadow-decide` endpoint deleted from Rust; `engine_shadow_log` table dropped. `permission_audit_log` table (separate — the production audit) persists.

## Reuse audit

Every change in 2.6 maps to existing infrastructure where possible. Net new code is bounded.

| Need | Existing piece | Status |
|---|---|---|
| Pure-logic engine pattern | `cef-native/src/core/PermissionEngine.{h,cpp}` (PURE LOGIC) | **Port verbatim** to `hodos_permission_engine` crate |
| 33 engine unit tests | `cef-native/tests/permission_engine_test.cpp` (verified 2026-06-02) | **Port verbatim** as Rust unit tests in pure crate |
| AppState facade-struct pattern | `Arc<services::WalletServices>` at `main.rs:87` | **Reuse pattern.** Add `permission: Arc<PermissionService>` field |
| ResumeKind::kInternal placeholder | `PendingAuthRequest.h:23-27` (Phase 2.5 reserved this for 2.6) | **Wire up.** Becomes the active resume path for Rust-initiated state |
| `RunPermissionGate(ctx, cb) -> Decision` (Phase 2.5 D1 seam) | `cef-native/include/core/PermissionGate.h` | **Repurpose.** Body changes from "local Decide()" to "POST to Rust, handle 200/202/403" |
| `OpenPromptModal(promptType, ctx, requestId, extraParams)` dispatcher (Phase 2.5 D3) | Free fns in `HttpRequestInterceptor.cpp` | **Reuse.** Called from `202 PENDING` response handler instead of from `RunPermissionGate` callback |
| `OnWalletCallSuccess(...)` indicator helper (Phase 2.5 D4) | `HttpRequestInterceptor.cpp:1320-1350` | **Stays put.** Continues to fire green-dot animation on `200 OK` payment responses |
| `IsInternalOrigin` exact-or-port-suffix match | `HttpRequestInterceptor.cpp:1365-1374` | **Reuse unchanged.** Pre-IPC-bridge bypass — header-less Rust requests skip engine |
| Defense-in-depth Rust gates (`check_domain_approved`, `X-Identity-Key-Approved`, `X-Key-Linkage-Approved`) | `handlers.rs:572-612` + Phase 1.5 Step 1 work | **Reuse unchanged.** Belt-and-suspenders — gates fire AFTER the engine decision |
| `domain_permission_invalidate` IPC chain | `simple_handler.cpp:4411-4447` | **Repoint.** Wallet UI sends to Rust directly via `POST /domain/permissions/invalidate`; Rust drops its internal cache. C++ caches deleted in sub-phase 7 |
| SQLite migrations + repo pattern | `rust-wallet/src/database/` (V20+ migrations) | **Reuse.** New tables `permission_audit_log` + `engine_shadow_log` follow existing patterns |
| Fire-and-forget worker dispatch | `CefPostTask(TID_FILE_USER_BLOCKING, ...)` pattern | **Reuse.** Shadow POST is one more worker task |

**Net new code:**
- `hodos_permission_engine` pure crate (~400 LOC + tests)
- `permission_service` module (~600 LOC including audit/shadow handlers)
- `EngineFlags` struct + env-var parsing (~50 LOC)
- C++ JSON→query-string translator at 202 handler (~30 LOC)
- C++ shadow-POST helper (~40 LOC)
- 2 new SQLite tables (V20 migration — V19 verified as current max at `connection.rs:912`)
- `session_close` IPC handler + chain (~20 LOC C++ + ~20 LOC Rust)

Sub-phase 7 deletes more code than the prior 6 sub-phases added — net LOC trends DOWN across the full migration.

## Sub-phase structure (multi-session plan)

7 sub-phases, each one focused session. Plan handoff doc between sessions so context-clear ↔ context-load is lossless. All sub-phases include cumulative shadow-mode + integration smoke before the sub-phase closes.

| Sub-phase | Status | Deliverable |
|---|---|---|
| 2.6 plan doc | 🚧 this doc (pending sign-off) | This document |
| **2.6-A — Pure crate + service scaffolding** | pending | `hodos_permission_engine` crate built + `permission_service` module + V20 migration + `EngineFlags` + 33 ported tests pass. All flags OFF. No production behavior change. |
| **2.6-B — Shadow infrastructure** | pending | C++ shadow-POST helper + `/engine/shadow-decide` endpoint + `engine_shadow_log` writes. Comparison runs on every wallet call for ALL 5 CallKind classes (engine cascade returns Silent/Prompt/Deny, shadow logs the divergence). C++ engine still authoritative. |
| **2.6-C — Privacy Perimeter CallKind migration** | pending | `engine_rust_privacy_perimeter` flag wired. Rust handles `IdentityKeyReveal`, `CounterpartyKeyLinkage`, `SpecificKeyLinkage`, `SensitiveCertField`. Flag flips ON during dev testing. Shadow inversion log clean. |
| **2.6-D — Scoped Grant CallKind migration** | pending | `engine_rust_scoped_grant` flag wired. Rust handles `ProtocolUse`, `BasketAccess`, `CounterpartyUse`. |
| **2.6-E — Payment CallKind migration + SessionManager move** | pending | `engine_rust_payment` flag wired. SessionManager counters migrate to Rust. `session_close` IPC wired. Green-dot animation invariant preserved end-to-end. |
| **2.6-F — Cert Disclosure CallKind migration** | pending | `engine_rust_cert_disclosure` flag wired. Rust handles `CertificateDisclosure` (non-sensitive). |
| **2.6-G — Domain Trust CallKind migration** | pending | `engine_rust_domain_trust` flag wired. Rust handles `DomainTrust` + `GenericApproved` catch-all. |
| **2.6-H — Cleanup + ship readiness** | pending | Delete C++ engine + all 5 flags + shadow comparison code + 4 C++ cache singletons in one commit. End-to-end smoke matrix runs against real production dApps. Phase 2.6 closure. |

### Smoke obligation discipline

Each sub-phase 2.6-B through 2.6-G needs cumulative real-world smoke before closing — testing the just-migrated CallKind class on at least one real dApp from the standard verification basket (CLAUDE.md "Testing Standards"). Shadow log must show clean (zero disagreements) before the flag flips. Each sub-phase commit message lists the dApps tested + shadow log query results.

Sub-phase 2.6-H is the final readiness smoke against the full thorough verification basket (30-45 min, all categories) per CLAUDE.md.

## Per-sub-phase scope + acceptance criteria

### Sub-phase 2.6-A — Pure crate + service scaffolding

**Files touched (new):**
- `rust-wallet/crates/hodos_permission_engine/Cargo.toml`
- `rust-wallet/crates/hodos_permission_engine/src/lib.rs`
- `rust-wallet/crates/hodos_permission_engine/src/context.rs`
- `rust-wallet/crates/hodos_permission_engine/src/decision.rs`
- `rust-wallet/crates/hodos_permission_engine/src/matrix_c.rs`
- `rust-wallet/crates/hodos_permission_engine/tests/*.rs` (ports 33 C++ engine tests verbatim, 1:1 with `cef-native/tests/permission_engine_test.cpp`)
- `rust-wallet/src/permission_service/mod.rs`
- `rust-wallet/src/permission_service/state.rs`
- `rust-wallet/src/permission_service/context_builder.rs`
- `rust-wallet/src/permission_service/audit.rs`
- `rust-wallet/src/permission_service/handlers.rs` (placeholder)
- `rust-wallet/src/permission_service/flags.rs` (`EngineFlags` struct + env-var parser — co-located with consumer, NOT in `services/`)
- `rust-wallet/src/database/permission_audit_repo.rs`
- `rust-wallet/src/database/engine_shadow_repo.rs`

**Files touched (extended):**
- `rust-wallet/Cargo.toml` (workspace conversion — adds `[workspace] members = ["crates/hodos_permission_engine"]` to existing `[package]`)
- `rust-wallet/src/database/migrations.rs` (adds `pub fn migrate_v19_to_v20(conn: &Connection) -> Result<()>` — creates `permission_audit_log` + `engine_shadow_log` tables. V19 verified as current max at `connection.rs:912`; V20 is genuinely unused)
- `rust-wallet/src/database/connection.rs` (adds V19→V20 migration call to `migrate()` runner at L760+)
- `rust-wallet/src/database/mod.rs` (re-exports `PermissionAuditRepository` + `EngineShadowRepository`)
- `rust-wallet/src/main.rs` (AppState gains `permission: Arc<PermissionService>` field; init at startup)

**Done when:**
1. `cargo build --release` succeeds with new workspace member AND existing `hodos-wallet` package builds unchanged (smoke: workspace conversion does NOT regress the existing build)
2. `cargo test -p hodos_permission_engine` passes all **33 ported tests** (1:1 with C++ engine tests at `cef-native/tests/permission_engine_test.cpp` — verified 2026-06-02 by `grep '^TEST(' permission_engine_test.cpp | wc -l`; any additional Rust-side tests written during port are added on top of the 33)
3. `cargo test` (root) passes — no regressions in existing test suite
4. **V20 migration** applies cleanly to a fresh dev database AND to the user's existing dev database (V19 verified as current max via `migrate_v18_to_v19` at `connection.rs:912` — V20 is genuinely the next unused version, no parallel branch has claimed it)
5. `permission_service::state::PermissionService::new(...)` constructs successfully at startup
6. `AppState.permission` is accessible from `web::Data<AppState>` handlers; `AppState.permission.flags()` returns `EngineFlags` with all 5 booleans
7. All 5 flags default `false` (verified by inspecting `EngineFlags::default()`)
8. No new production code paths exercise the engine (it's dormant — nothing calls `PermissionService::decide()` yet; shadow infrastructure lands in 2.6-B)
9. Existing wallet UI + external dApp smoke unchanged — passes Phase 2.5 closure smoke matrix unchanged (github.com getNetwork, treechat login, github encrypt/decrypt, localhost wallet UI)
10. macOS parity check: pure crate has zero CEF dependencies (verified by `grep -r 'cef\|CEF\|cef-native' rust-wallet/crates/hodos_permission_engine/` returning empty); `cargo build --release` clean on macOS
11. **Workspace conversion smoke:** `cargo build --release -p hodos-wallet` produces an identical binary (modulo Cargo.lock churn) to pre-2.6-A baseline — confirms the `[workspace]` table addition does not change build semantics for the wallet package itself

### Sub-phase 2.6-B — Shadow infrastructure

**Files touched (new):**
- `cef-native/include/core/EngineShadow.h` (shadow-POST helper signature)
- `cef-native/src/core/EngineShadow.cpp` (fire-and-forget POST implementation)

**Files touched (extended):**
- `cef-native/src/core/HttpRequestInterceptor.cpp` (call shadow helper after each engine decision in 5 inline locations — payment branch L2245-2385, identity-key branch, key-linkage branch, scoped-grant branch, cert-disclosure branch)
- `cef-native/src/core/PermissionGate.cpp` (shadow call from `RunPermissionGate` for IPC path)
- `rust-wallet/src/permission_service/handlers.rs` (`/engine/shadow-decide` POST handler)
- `rust-wallet/src/main.rs` (route registration for `/engine/shadow-decide`)

**Done when:**
1. Every wallet call from both HTTP path and IPC path generates exactly one shadow POST
2. Shadow POST runs on `TID_FILE_USER_BLOCKING` — verified by adding a deliberate 500ms sleep in Rust's shadow handler and confirming UI thread + wallet call complete normally without waiting
3. Critical path latency unchanged from pre-2.6-B baseline (measured via existing log timestamps on a `/createAction` call)
4. C++ NEVER reads Rust's shadow response (verified by code review — no `resp.body` consumption after POST)
5. `engine_shadow_log` table gains one row per wallet call with all required columns populated
6. `cpp_decision`, `cpp_promptType`, `cpp_reason` match what the C++ engine actually decided
7. `rust_decision`, `rust_reason` populated by `hodos_permission_engine::Decide(ctx)` against the same context
8. `agreement` column correctly computed as `cpp_decision == rust_decision && cpp_reason == rust_reason`
9. SQLite query `SELECT call_kind_class, agreement, COUNT(*) FROM engine_shadow_log GROUP BY call_kind_class, agreement` returns rows after a 5-minute real-world browse session
10. Initial shadow log shows agreement≈100% for `DomainTrust` and `GenericApproved` CallKinds (the simplest cases — high signal that the engine port is correct at baseline)
11. Disagreements in non-trivial CallKinds are EXPECTED at this point (Rust engine implementation may have subtle differences) — they're the diagnostic signal for 2.6-C through 2.6-G
12. Sub-phase 2.6-A done-when criteria still pass
13. macOS parity: shadow-POST helper compiled and exercised on macOS build

### Sub-phase 2.6-C — Privacy Perimeter CallKind migration

**Files touched (new):**
- (none — extends existing crates/modules)

**Files touched (extended):**
- `rust-wallet/src/handlers.rs` (privacy-perimeter endpoints call `PermissionService::decide()` when `engine_rust_privacy_perimeter` flag is ON; on `Decision::Prompt`, return 202 PENDING with envelope; on `Decision::Silent`, proceed)
- `rust-wallet/src/permission_service/state.rs` (approval map gains `pending_approvals: Arc<RwLock<HashMap<approval_id, PendingApproval>>>` with TTL)
- `rust-wallet/src/permission_service/handlers.rs` (`/engine/approval-lookup` for the X-User-Approved re-issue path)
- `cef-native/src/handlers/simple_handler.cpp` (202 response handler — JSON→query-string translator; PendingAuthRequest enrollment with kInternal resume kind; modal open via existing OpenPromptModal dispatcher)
- `cef-native/src/core/HttpRequestInterceptor.cpp` (re-issue path on X-User-Approved adds header + posts wallet call again)

**Done when:**
1. `engine_rust_privacy_perimeter` flag default `false`; can be flipped via env var
2. With flag OFF: privacy perimeter calls flow through C++ engine exactly as today; shadow log records Rust's decision; no production behavior change
3. With flag ON: privacy perimeter calls flow through Rust engine; Rust returns 200/202/403 per LD2 schema; C++ JSON→query-string translator opens correct modal
4. User Approve on `identity_key_reveal` modal re-issues with `X-User-Approved: <approvalId>` header; Rust looks up approvalId, processes call, returns 200; C++ forwards result
5. User Deny on modal sends error envelope to renderer; no further wallet call
6. `permission_audit_log` table gains one row per approval lifecycle (created → resolved)
7. `approvalId` single-use: re-using a consumed approvalId returns 403 from Rust
8. `approvalId` TTL: a 10min+ old approvalId returns 403 with reason `approval_expired`
9. Shadow log with flag ON shows agreement≈100% for privacy perimeter (C++ inline gate now in shadow agreeing with Rust authoritative)
10. Real-world smoke: test against treechat (identity-key reveal) + a key-linkage-using dApp + a sensitive-cert-field disclosure flow. Each modal opens correctly; Approve resolves; Deny returns clean error.
11. Sub-phase 2.6-B done-when criteria still pass
12. Defense-in-depth: Rust `get_public_key` handler still requires `X-Identity-Key-Approved` header OR `domain_permissions.identity_key_disclosure_allowed=1` (Phase 1.5 Step 1 gate intact)
13. macOS parity: smoke matrix runs on macOS build

### Sub-phase 2.6-D — Scoped Grant CallKind migration

Same shape as 2.6-C, targeting `ProtocolUse`/`BasketAccess`/`CounterpartyUse` endpoints (createSignature, createHmac, encrypt, decrypt, encrypt-bie1, decrypt-bie1, listOutputs, relinquishOutput, listMessages, acknowledgeMessage).

**Done when:**
1. `engine_rust_scoped_grant` flag default `false`; flippable via env var
2. With flag ON: scoped-grant endpoints flow through Rust engine
3. Modal: `protocol_permission_prompt` / `basket_permission_prompt` / `counterparty_permission_prompt` open correctly with translated query-string params
4. Protected basket guardrail enforced: `default`, `backup-*`, `admin *` never auto-grant — always prompt (matches Phase 1.5 Step 6 Commit E behavior)
5. Sub-permission persistence works: after Approve, `domain_protocol_permissions` / `domain_basket_permissions` / `domain_counterparty_permissions` row exists; subsequent calls Silent
6. Shadow log clean with flag ON
7. Real-world smoke: test against treechat (createSignature with yours-legacy-message protocol) + a basket-using dApp + a counterparty-bound BRC-2 encrypt flow
8. Sub-phase 2.6-C done-when criteria still pass

### Sub-phase 2.6-E — Payment CallKind migration + SessionManager move

**Files touched (extended):**
- `rust-wallet/src/handlers.rs` (`create_action`, `sign_action`, `acquire_certificate`, `send_message`, `transaction/send` call PermissionService when flag ON)
- `rust-wallet/src/permission_service/state.rs` (SessionManager equivalent — `session_counters: Arc<RwLock<HashMap<(browser_id, domain), SessionCounters>>>` with 60s rate window)
- `rust-wallet/src/main.rs` (new route `POST /session/close`)
- `cef-native/src/handlers/simple_handler.cpp` (new `session_close` IPC → Rust POST when tab closes; trigger from existing tab-close path in TabManager/SimpleHandler)
- `cef-native/include/core/SessionManager.h` (gradually deprecated — counter writes no-op when `engine_rust_payment` flag is ON)
- `cef-native/src/core/HttpRequestInterceptor.cpp` (`OnWalletCallSuccess` continues to fire indicator IPC + cents extraction stays C++ — does NOT call `recordSpending` when flag ON; that's Rust's job now)

**Done when:**
1. `engine_rust_payment` flag default `false`; flippable via env var
2. With flag OFF: payment path unchanged from 2.5 closure
3. With flag ON: payments flow through Rust engine; cap check / rate limit / max-tx-per-session all enforced server-side
4. Rust increments `sessionSpentCents` + `paymentCountThisSession` + `paymentRequestsThisMinute` on Silent decisions
5. Modal: `payment_confirmation` opens with correct cents + exceeded-limit reason
6. Modal Approve re-issues with `X-User-Approved` → Rust processes payment → Rust records spending → returns 200
7. C++ `OnWalletCallSuccess` fires `payment_success_indicator` IPC on every 200 response to a payment endpoint
8. **GREEN-DOT ANIMATION VISUAL CHECK on real-world smoke** — every auto-approved payment fires the tab animation (matches `payment_animation_safeguard` invariant)
9. Tab close → `session_close` IPC fires from C++ → Rust drops counters for that browserId
10. Closing a tab and reopening to the same domain resets sessionSpent (verified in shadow log + by attempting a payment that previously hit session cap)
11. Per-tab independence: two tabs on same domain have independent counters (key by `(browserId, domain)`)
12. Shadow log clean with flag ON
13. Real-world smoke: at minimum one payment per cap-class (silent within cap; prompt over per-tx; prompt over session; prompt on rate limit; prompt on bsv price unavailable — synthesize last by clearing BSVPriceCache mid-test)
14. BRC-121 path UNCHANGED — still uses inline cascade in `TryHandleBrc121_402` (intentionally deferred to post-2.6 polish)
15. Sub-phase 2.6-D done-when criteria still pass

### Sub-phase 2.6-F — Cert Disclosure CallKind migration

Same shape as 2.6-C, targeting `/proveCertificate` (non-sensitive fields).

**Done when:**
1. `engine_rust_cert_disclosure` flag default `false`; flippable via env var
2. With flag ON: non-sensitive cert disclosure flows through Rust engine
3. Per-field permission check: `cert_field_permissions` rows consulted by Rust; missing fields trigger `certificate_disclosure` modal
4. Sensitive fields STILL go through Privacy Perimeter (`engine_rust_privacy_perimeter` — `SensitiveCertField` CallKind)
5. After Approve: `cert_field_permissions` rows persisted; subsequent calls for same fields Silent
6. Shadow log clean with flag ON
7. Real-world smoke: test against a dApp doing `proveCertificate` with displayName + avatar (low-sensitivity) and confirm Silent after first approval
8. Sub-phase 2.6-E done-when criteria still pass

### Sub-phase 2.6-G — Domain Trust CallKind migration

**Files touched (extended):**
- `rust-wallet/src/handlers.rs` (ALL remaining shim-reachable endpoints — first-touch domain trust check moves to Rust)
- `rust-wallet/src/permission_service/state.rs` (in-memory domain permission cache + manifest fetch coordination)
- `cef-native/src/core/HttpRequestInterceptor.cpp` (remove DomainPermissionCache reads from C++ engine path — these moves to Rust)

**Done when:**
1. `engine_rust_domain_trust` flag default `false`; flippable via env var
2. With flag ON: first-touch `domain_approval` / `manifest_connect_bundle` flows through Rust engine
3. Manifest fetch: Rust calls existing `ManifestFetcher` equivalent (or C++ continues to fetch and bundle into the call — TBD as 2.6-G design refinement; manifest fetching is currently in C++)
4. Blocked-domain check moves to Rust (`trust_level = 'blocked'` → 403 with reason `trust_blocked`)
5. Unknown-domain check moves to Rust (returns 202 with `domain_approval` or `manifest_connect_bundle` per manifest presence)
6. C++ `DomainPermissionCache` no longer consulted by engine path (still consulted by `IsInternalOrigin` check and other ancillary uses)
7. Shadow log clean with flag ON
8. Real-world smoke: connect to a brand-new dApp Hodos has never seen; verify manifest fetch + bundle modal opens; Approve flow persists `domain_permissions` row; subsequent calls flow through approved-trust path
9. Sub-phase 2.6-F done-when criteria still pass
10. **All 5 flags now ON.** Production binary still has C++ engine code + flag checks; cleanup is sub-phase 2.6-H

### Sub-phase 2.6-H — Cleanup + ship readiness

**Files touched (deleted):**
- `cef-native/src/core/PermissionEngine.cpp`
- `cef-native/include/core/PermissionEngine.h`
- `cef-native/tests/permission_engine_test.cpp` (tests now live in `hodos_permission_engine` crate)
- `cef-native/src/core/EngineShadow.cpp`
- `cef-native/include/core/EngineShadow.h`
- `rust-wallet/src/services/engine_flags.rs` (EngineFlags struct + all flag checks)
- All 5 `engine_rust_*` env var reads
- `DomainPermissionCache` / `SubPermissionCache` / `IdentityKeyApprovalCache` / `KeyLinkageApprovalCache` C++ singletons (deleted — Rust owns the state now)
- `SessionManager` C++ singleton (deleted — fully migrated to Rust in 2.6-E)
- `/engine/shadow-decide` Rust handler + route
- `engine_shadow_log` SQLite table (V21 migration drops it — V20 created it in 2.6-A)
- `RunPermissionGate` body collapses to thin proxy (or deleted if the IPC handler in `simple_handler.cpp` inlines the POST)

**Files touched (extended):**
- `cef-native/src/core/HttpRequestInterceptor.cpp` (`AsyncWalletResourceHandler::Open()` collapses dramatically — engine cascade replaced by single POST-to-Rust + response dispatch)
- `cef-native/src/handlers/simple_handler.cpp` (IPC bridge becomes thinner; 202 handler + modal dispatch + X-User-Approved re-issue is the entire shim-traffic path)
- `rust-wallet/src/main.rs` (drops shadow route, drops `engine_flags` field from AppState)
- `development-docs/architecture/AUTO_APPROVE_ENGINE.md` (rewritten to describe Rust engine state-of-world)
- `development-docs/FUTURE_AUTO_APPROVE_ENGINE_ARCHITECTURE.md` (marked HISTORICAL — closure note pointing to 2.6 close memory)
- `CLAUDE.md` "Key Files" table updated to reflect Rust engine ownership

**Done when:**
1. Single commit deletes C++ engine + all 5 flags + shadow comparison code + 4 C++ cache singletons + SessionManager singleton
2. `cargo build --release` clean
3. `cmake --build build --config Release` clean (Windows)
4. `cmake --build build --config Release` clean (macOS)
5. `cargo test` passes — no regressions in any Rust test suite
6. `hodos_tests.exe` C++ test runner has been pruned of `permission_engine_test` references; remaining tests pass
7. **Full thorough verification smoke** (CLAUDE.md "Testing Standards" — 30-45 min, all categories from Authentication / Video-Media / News-Content / E-commerce / Productivity / BSV) — zero regressions vs Phase 2.5 closure baseline
8. Internal wallet UI (localhost:5137) fully functional — wallet creation, backup, restore, send, settings, all overlays
9. External dApp smoke: github.com `getNetwork({})`, treechat login + payment, encrypt/decrypt round-trip, paid BRC-121 content, ordinals dApp — all work
10. **Green-dot animation visible on every auto-approved payment** across the smoke matrix
11. Shadow log table dropped via V21 migration (cleanup commit + migration land together)
12. Architecture docs updated; CLAUDE.md Key Files table reflects Rust engine ownership
13. Memory written: `project_phase26_CLOSED_<date>.md` summarizing migration outcome
14. Phase 2.6 CLOSED. Phase 3 (ordinals) unblocked.

## Risk surface

| Risk | Severity | Mitigation |
|---|---|---|
| Green-dot animation regression during sub-phase 2.6-E (SessionManager migration) | **High** | LD4 explicitly preserves `OnWalletCallSuccess` and the indicator IPC chain in C++. Visual smoke check required in 2.6-E done-when criterion #8. Memory `payment_animation_safeguard` is the contract. |
| Engine drift between Rust and C++ during dev testing (Rust decides differently than C++) | **Medium** | Shadow log surfaces every disagreement. Sub-phase done-when criteria require shadow log clean before flag flip. Disagreements are diagnostic, not silent bugs. |
| Defense-in-depth gate weakening (Rust `get_public_key` gate, `check_domain_approved`) | **High** | LD2 preserves these gates — they fire AFTER the engine decision, not instead of it. Sub-phase 2.6-C done-when criterion #12 explicitly verifies. |
| `IsInternalOrigin` bypass regressions opening wallet UI to engine prompts | **High** | LD3 preserves `IsInternalOrigin` exact-or-port-suffix match. Header-less Rust requests skip engine (matches today's `check_domain_approved` `Ok(None)` behavior). 2.6-A done-when criterion #8 verifies wallet UI smoke unchanged. |
| Approval state leak (approvalId reused, expired ID accepted, audit row missed) | **Medium** | Single-use + atomic-pop semantics in Rust (mirrors `PendingRequestManager::popRequest`). 2.6-C done-when criteria #7 + #8 explicitly verify. |
| Session close hook race (tab closes between Approve and re-issue) | **Medium** | Approval re-issue checks both approval map AND session counters; if session was dropped mid-flight, approval falls through (counter goes to 0 + new session, payment still gated by per-tx cap). |
| Engine crate accidentally takes actix dependency | **Medium** | Workspace member boundary + Cargo.toml manifest enforces. 2.6-A done-when criterion #10 grep verifies. |
| Shadow POST saturates worker pool under load | **Low** | Fire-and-forget single POST per wallet call. Worker pool `TID_FILE_USER_BLOCKING` already handles wallet HTTP + SHIP discovery + indexer calls; one more post is bounded. |
| C++ JSON→query-string translator drops a field that React expects | **Medium** | LD2 includes exhaustive `promptPayload` field table per `promptType`. Field-by-field test against `BRC100AuthOverlayRoot.tsx:335-440` consumer during sub-phase 2.6-C smoke. |
| Subtle race: 202 PENDING returned, C++ enrolls PendingAuthRequest, user Approves before enrollment completes | **Low** | PendingAuthRequest enrollment is synchronous in `OnProcessMessageReceived` on UI thread; React modal can't fire approve_xxx IPC until the modal is open, which is after enrollment. No race window. |
| macOS parity regression (overlay, IPC, threading) | **Medium** | Every sub-phase done-when criterion includes macOS smoke. macOS overlay creation (`cef_browser_shell_mac.mm`) is the highest-risk surface; 2.6-A through 2.6-H must each verify on macOS. |
| Phase 2.5 4 locked decisions break under 2.6 (forward-compat check fails) | **None** | Decisions D1–D4 were specifically chosen for forward compatibility. PHASE_2_5_IPC_REFACTOR.md "Why these survive the Phase 2.6 migration" section captures the mapping. |

## Open questions

These have strong leans documented in the brief at `tmp/PHASE_2_6_ARCHITECTURE_RECOMMENDATIONS.md` but aren't load-bearing for sub-phase structure — settled per-commit during migration.

### OQ1 — `permission_audit_log` retention — RESOLVED 2026-06-02

**Resolved:** **90 days.** Background purge task drops rows older than 90 days from `permission_audit_log`. Revisitable — may extend later or expose as a user-configurable setting if forensics needs change. Schema includes `created_at` index to support efficient purge.

**Original question:** How long do we retain audit log rows?

### OQ2 — `permission_audit_log` body column — RESOLVED 2026-06-02

**Resolved:** **`body_hash`** — sha256 hex (64 chars), `VARCHAR(64)` column. Captures call identity for forensic provenance ("this exact body was submitted at this exact time") without storing raw payload bytes. Privacy-safe even if DB is extracted.

**Original question:** Store request body as `body_hash`, `body_preview`, or nothing?

### OQ3 — `engineReason` enum vocabulary completeness

**Question:** Initial draft list (`per_tx_limit`, `session_cap`, `rate_limit`, `price_unavailable`, `max_tx_per_session`, `protected_basket`, `manifest_required`, `trust_blocked`, `trust_unknown`, `privacy_perimeter_no_grant`, `silent_within_caps`) likely needs additions as each CallKind branch lands.

**Resolution model:** Enum is source of truth — defined in `hodos_permission_engine::decision::EngineReason`. Each sub-phase extends as needed. Doc updated as we go (no doc-update commit needed; the enum is the spec).

### OQ4 — Shim-injection `Set<approved_host>` mirror

**Question:** Deferred per architecture brief — the shim is unconditionally injected on every external https main frame today (`simple_render_process_handler.cpp:889-898`), so the mirror is net-new architecture, not preservation.

**Reopens if:** future requirement for per-domain shim gating (e.g. don't inject `window.CWI` on a domain the user explicitly told us to ignore).

### OQ5 — BRC-121 paid retry path migration

**Question:** Sequenced as polish AFTER all 5 main flags flip. Out of scope for the main 2.6 migration.

**Plan:** `TryHandleBrc121_402` becomes a thin "is this a 402 with BRC-121 headers? if so, call Rust's pay_402" handler. Rust's `pay_402` extends to internally call `hodos_permission_engine::Decide()` for the Payment CallKind. BRC-121-specific inline cap-cascade at `HttpRequestInterceptor.cpp:5022-5108` deleted.

**Resolution gate:** Standalone polish commit after sub-phase 2.6-H closes.

### OQ6 — Shadow log review UI

**Question:** Add a debug-build-only wallet UI panel for browsing `engine_shadow_log`?

**Lean:** CLI-only (SQLite queries) during dev. Build a UI panel only if disagreement volume warrants visual surface.

**Resolution gate:** Sub-phase 2.6-B mid-flight if you find yourself running the SQLite query more than 5 times.

### OQ7 — macOS parity matrix per sub-phase

**Question:** Each sub-phase done-when criterion lists "macOS parity check" — but the matrix isn't explicit. What gets verified?

**Lean:** Per sub-phase:
- `cmake --build build --config Release` succeeds on macOS
- macOS NSWindow / NSPanel overlay creation for any new modal type unchanged
- IPC dispatch + JS bridge unchanged
- Smoke matrix runs end-to-end on macOS (at minimum the just-migrated CallKind class on one production dApp)

**Resolution gate:** Add as explicit sub-section to each sub-phase's done-when list before 2.6-A lands.

### OQ8 — Manifest fetch ownership

**Question:** Today `ManifestFetcher` lives C++-side (`cef-native/src/core/ManifestFetcher.cpp`). Under 2.6-G (Domain Trust migration), Rust needs the manifest to build `manifest_connect_bundle` modal payload. Two options:
- Rust calls a new endpoint that fetches the manifest; C++ stays out of it
- C++ continues to fetch manifest and includes it in the wallet call body or as a header

**Lean:** Migrate `ManifestFetcher` equivalent to Rust during 2.6-G. Removes one cross-process coordination; manifest cache is engine state.

**Resolution gate:** Sub-phase 2.6-G design refinement.

## Out-of-tree assumptions

- `cefMessage.send` IPC pattern reliable across all sub-phases (verified in Phase 2.5 closure smoke)
- `TID_FILE_USER_BLOCKING` pool can absorb shadow POST per wallet call (existing pool sized for blocking I/O)
- SQLite write throughput sufficient for shadow log (low volume — one row per wallet call, even at 10 calls/sec is trivial)
- Rust workspace member crates inherit `rust-wallet`'s edition, lints, and security-relevant Cargo.toml settings (verified in 2.6-A first commit)
- Memory note `cef_self_build_reason` — Hodos's self-built CEF is for proprietary codecs; extension support is out of scope, so third-party-extension-spoofing-localhost-5137 threat model stays deferred
- Memory note `wallet_toolbox_divergence` — Hodos diverges from toolbox by keeping permissions local-first (SQLite) rather than on-chain. 2.6 preserves this divergence; engine state stays local.

## Verification — Phase 2.6 done when

The 2.6-H done-when criteria are the canonical close check. In summary:

1. C++ `PermissionEngine.{h,cpp}` deleted
2. All 5 `engine_rust_*` flags deleted from code
3. C++ shadow-mode infrastructure deleted
4. 4 C++ permission caches (`DomainPermissionCache`, `SubPermissionCache`, `IdentityKeyApprovalCache`, `KeyLinkageApprovalCache`) deleted
5. C++ `SessionManager` singleton deleted
6. `engine_shadow_log` SQLite table dropped (V21 migration in 2.6-H — V20 created it in 2.6-A)
7. Full thorough verification smoke matrix passes (CLAUDE.md "Testing Standards" 30-45 min Thorough tier)
8. Wallet UI smoke unchanged from Phase 2.5 baseline
9. External dApp smoke (github, treechat, BRC-121 paid content, ordinals dApp) all work
10. **Green-dot animation fires on every auto-approved payment** across the smoke matrix
11. Architecture docs updated
12. Memory written summarizing migration outcome
13. macOS parity verified
14. No new build warnings
15. `permission_audit_log` table persists with rows from production usage (the live audit surface — separate from the deleted shadow log)

## Related docs / memories

- [`../../FUTURE_AUTO_APPROVE_ENGINE_ARCHITECTURE.md`](../../FUTURE_AUTO_APPROVE_ENGINE_ARCHITECTURE.md) — the vision doc (will be marked HISTORICAL at 2.6 close)
- [`../../architecture/AUTO_APPROVE_ENGINE.md`](../../architecture/AUTO_APPROVE_ENGINE.md) — current C++ engine state-of-world (will be rewritten at 2.6-H to describe Rust engine)
- [`../../architecture/WALLET_API_MAP.md`](../../architecture/WALLET_API_MAP.md) — 95 endpoints × gate matrix (updated incrementally per sub-phase)
- [`../phase-2-window-cwi-shim/PHASE_2_5_IPC_REFACTOR.md`](../phase-2-window-cwi-shim/PHASE_2_5_IPC_REFACTOR.md) — Phase 2.5 plan + 4 locked decisions D1–D4 that 2.6 inherits
- [`../phase-2-window-cwi-shim/COMMIT_6_DESIGN.md`](../phase-2-window-cwi-shim/COMMIT_6_DESIGN.md) — pattern reference for individual sub-phase design docs
- [`../phase-1.5-brc100-surface-completion/PERMISSION_UX_DESIGN.md`](../phase-1.5-brc100-surface-completion/PERMISSION_UX_DESIGN.md) §3 Matrix C — engine decision logic source
- `tmp/PHASE_2_6_ARCHITECTURE_RECOMMENDATIONS.md` — architecture-reviewer brief that informed the 2026-06-02 discussion + locked decisions
- Memory `phase25-closed-2026-06-02` — Phase 2.5 close + the 4 D1–D4 decisions 2.6 inherits
- Memory `payment_animation_safeguard` — green-dot invariant preserved across every sub-phase
- Memory `feedback_research_first_do_it_once` — the protocol that gated 2.6 kickoff with research before code
- Memory `domain_permission_cache_invalidation` — the IPC chain that gets repointed in sub-phase 2.6-G
- Memory `IsInternalOrigin uses exact-or-port-suffix match` — security tightening preserved
- Memory `brc121_bypasses_permission_engine` — OQ5 the post-2.6 polish item
- Memory `wallet_toolbox_divergence` — local-first permissions stays
