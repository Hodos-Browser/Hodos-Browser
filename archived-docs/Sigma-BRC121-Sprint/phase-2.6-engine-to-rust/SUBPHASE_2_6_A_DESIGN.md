# Sub-phase 2.6-A — Pure Crate + Service Scaffolding (Design)

> **Status:** Design draft 2026-06-02 after kickoff verification. Sister doc to
> [`PHASE_2_6_ENGINE_TO_RUST.md`](./PHASE_2_6_ENGINE_TO_RUST.md) — that one
> carries the strategy + locked decisions; this one carries the *how* for the
> first sub-phase only.
>
> **Approval gate:** Sub-phase 2.6-A code work does not start until the user
> signs off on this design.
>
> **Scope discipline:** This sub-phase produces ZERO production behavior change.
> Engine is dormant — nothing in production calls `PermissionService::decide()`
> yet. The deliverable is scaffolding + tests + dormant infrastructure.

## 0. Glossary

| Term | Meaning |
|---|---|
| Pure crate | `rust-wallet/crates/hodos_permission_engine/` — engine decision logic, no actix/sqlite/http |
| Service module | `rust-wallet/src/permission_service/` — actix-integrated wrapper holding approval map + audit |
| Workspace conversion | Adding `[workspace]` to `rust-wallet/Cargo.toml` to make it both a workspace root AND a package |
| Test port | Translating 33 C++ GoogleTest cases to Rust `#[test]` 1:1 |
| V20 migration | Single new `migrate_v19_to_v20` function creating two tables |
| Dormant | Code exists, types exist, tests pass, but nothing in production calls into it |

## 1. Kickoff verification (2026-06-02)

Findings from the mandatory pre-commit verification per CLAUDE.md phase-kickoff workflow:

| Plan claim | Actual code state | Resolution |
|---|---|---|
| 46+ C++ engine tests | **33** `TEST()` macros at `permission_engine_test.cpp` | Plan corrected to 33 |
| V21 next migration | **V19** is current max (`migrate_v18_to_v19` at `connection.rs:912`) | Plan corrected to V20 |
| Workspace member at `rust-wallet/crates/` | `rust-wallet/Cargo.toml` is single-crate `[package]` | Workspace conversion approach locked: Option A (hybrid `[workspace]` + `[package]`) |
| `EngineFlags` at `src/services/engine_flags.rs` | `services/` module is Phase 1.6 WalletServices facade (thematic mismatch) | Relocated to `src/permission_service/flags.rs` (co-located with consumer) |
| Migrations as separate SQL files | Migrations are Rust functions in `migrations.rs` | Plan corrected — V20 is a new `pub fn migrate_v19_to_v20` |

All other plan-doc claims verified accurate (PermissionEngine.h L32-55 enum, PermissionContext/PermissionDecision shape, AppState facade pattern at main.rs L80-98, services/ module exists, sub-permission tables V17/V18/V19 already in place).

## 2. Workspace conversion approach (Option A)

### Before

`rust-wallet/Cargo.toml`:
```toml
[package]
name = "hodos-wallet"
version = "0.3.0"
edition = "2021"
default-run = "hodos-wallet"

[dependencies]
# ... (~75 lines of deps)
```

### After

`rust-wallet/Cargo.toml`:
```toml
[workspace]
members = [
    ".",                                 # the hodos-wallet package itself
    "crates/hodos_permission_engine",
]
resolver = "2"

[package]
name = "hodos-wallet"
version = "0.3.0"
edition = "2021"
default-run = "hodos-wallet"

[dependencies]
hodos_permission_engine = { path = "crates/hodos_permission_engine" }
# ... (existing deps unchanged)
```

`rust-wallet/crates/hodos_permission_engine/Cargo.toml`:
```toml
[package]
name = "hodos_permission_engine"
version = "0.1.0"
edition = "2021"

[dependencies]
# Intentionally minimal. NO actix, NO sqlite, NO reqwest, NO AppState.
# Pure decision logic only.
serde = { version = "1.0", features = ["derive"] }    # PermissionDecision/Context serialization
serde_json = "1.0"                                    # for promptPayload values
thiserror = "1.0"                                     # error types

[dev-dependencies]
# Test utilities if needed; nothing yet.
```

### Why Option A

1. **Lowest disruption** — `rust-wallet/` directory unchanged, the wallet binary still builds from the same path, dev launcher scripts work unchanged.
2. **Supported pattern** — Cargo natively supports root-package + workspace.
3. **Compile-time enforcement of "no actix"** — workspace boundary means `cargo build` fails if engine crate accidentally takes actix as a dep.
4. **`resolver = "2"`** — required for workspace + package coexistence in current Cargo; prevents resolver behavior change for the hodos-wallet package.

### Risks + mitigations

| Risk | Mitigation |
|---|---|
| `cargo build --release` breaks for the wallet | Smoke test workspace conversion in isolation (sub-commit 2.6-A.1) before any engine code lands |
| Cargo.lock churn from resolver change | Verify `cargo build` produces a binary functionally identical to pre-2.6-A; commit Cargo.lock changes deliberately |
| Dev launcher scripts break (`HODOS_DEV=1` env var enforcement) | Verify `dev-wallet.ps1` and `dev-wallet.sh` still work — they `cargo run -p hodos-wallet`, no change needed |
| IDE / rust-analyzer confusion | Force a `cargo clean` + `cargo build` cycle on first run after conversion |

## 3. File-by-file inventory

### `rust-wallet/crates/hodos_permission_engine/` (NEW crate)

| File | Purpose | Approx LOC |
|---|---|---|
| `Cargo.toml` | Minimal manifest, intentionally no actix/sqlite deps | 15 |
| `src/lib.rs` | Public surface: `pub use context::*; pub use decision::*; pub fn decide(ctx: &PermissionContext) -> PermissionDecision` | 30 |
| `src/context.rs` | `PermissionContext` struct (mirror of C++ `PermissionContext`) + `PermissionCallKind` enum (mirror of C++ enum) + serde impls | 120 |
| `src/decision.rs` | `PermissionDecision` enum (`Silent`/`Prompt(PromptType, EngineReason)`/`Deny(EngineReason)`) + `PromptType` enum + `EngineReason` enum (initial vocabulary per LD2) | 100 |
| `src/matrix_c.rs` | Branch helpers — `decide_privacy_perimeter`, `decide_domain_trust`, `decide_scoped_grant`, `decide_payment`, `decide_cert_disclosure`, `decide_generic_approved`. Translation of C++ `PermissionEngine::Decide()` body | 250 |
| `tests/decision_matrix.rs` | 33 ported tests, 1:1 with C++ tests | 400 |

Engine crate total: ~915 LOC including tests.

### `rust-wallet/src/permission_service/` (NEW module)

| File | Purpose | Approx LOC |
|---|---|---|
| `mod.rs` | Module root: re-exports + `PermissionService` constructor | 40 |
| `state.rs` | `PermissionService` struct with `pending_approvals: Arc<RwLock<HashMap<ApprovalId, PendingApproval>>>` + session counter map placeholder (real impl lands in 2.6-E) | 120 |
| `flags.rs` | `EngineFlags` struct with 5 boolean fields + `from_env()` constructor reading `HODOS_ENGINE_RUST_*` env vars | 60 |
| `context_builder.rs` | Placeholder. Empty `build_context(&AppState, body, headers) -> PermissionContext` returning a default context. Real impl per CallKind class in 2.6-C through 2.6-G | 40 |
| `audit.rs` | `write_audit_entry(repo, ...)` + `write_shadow_entry(repo, ...)` helpers calling the new repos | 80 |
| `handlers.rs` | Empty placeholder. `/engine/shadow-decide` endpoint lands in 2.6-B | 20 |

Service module total: ~360 LOC.

### `rust-wallet/src/database/` (NEW repos)

| File | Purpose | Approx LOC |
|---|---|---|
| `permission_audit_repo.rs` | `PermissionAuditRepository` with `insert(record)`, `purge_older_than(timestamp)` (90-day cutoff per OQ1), `count_recent(domain, since)` | 150 |
| `engine_shadow_repo.rs` | `EngineShadowRepository` with `insert(comparison)`, `query_disagreements(limit)`, `agreement_stats_by_class()` for CLI inspection (OQ6) | 120 |

Repo total: ~270 LOC.

### Extended files (no new logic, scaffolding wiring only)

| File | Change |
|---|---|
| `rust-wallet/Cargo.toml` | Add `[workspace]` table; add `hodos_permission_engine` path dep |
| `rust-wallet/src/database/migrations.rs` | Add `pub fn migrate_v19_to_v20(conn: &Connection) -> Result<()>` (~70 LOC) |
| `rust-wallet/src/database/connection.rs` | Add V19→V20 migration call to `migrate()` at L912 |
| `rust-wallet/src/database/mod.rs` | Re-export new repos |
| `rust-wallet/src/main.rs` | AppState gains `permission: Arc<PermissionService>` field; constructor reads `EngineFlags::from_env()` and passes to `PermissionService::new()` |

Extended files total: ~150 LOC change across 5 files.

### Grand total

**~1700 LOC new code** + ~150 LOC scaffold wiring = sub-phase 2.6-A.

## 4. Test port strategy

### Approach

**1:1 port** — every C++ `TEST()` becomes one Rust `#[test]`. Same test name, same scenario, same expected output. Side-by-side diff during port catches translation drift.

Translation rules:

| C++ | Rust |
|---|---|
| `TEST(PermissionEngine, FooBar)` | `#[test] fn foo_bar()` |
| `PermissionContext ctx;` | `let ctx = PermissionContext::default();` |
| `ctx.callKind = PermissionCallKind::IdentityKeyReveal;` | `let ctx = PermissionContext { call_kind: CallKind::IdentityKeyReveal, ..Default::default() };` |
| `auto d = PermissionEngine::Decide(ctx);` | `let d = hodos_permission_engine::decide(&ctx);` |
| `EXPECT_EQ(d.kind, PermissionDecision::Kind::Silent);` | `assert_eq!(d, PermissionDecision::Silent);` |
| `EXPECT_EQ(d.promptType, "domain_approval");` | `assert!(matches!(d, PermissionDecision::Prompt(PromptType::DomainApproval, _)));` |

### Naming convention

| C++ test name | Rust test name |
|---|---|
| `BlockedDomainAlwaysDeniesRegardlessOfCallKind` | `blocked_domain_always_denies_regardless_of_call_kind` |
| `IdentityKeyRevealPromptsByDefault` | `identity_key_reveal_prompts_by_default` |
| ... | snake_case throughout |

### Test sources

All 33 ported from `cef-native/tests/permission_engine_test.cpp`. Specific test list (for reviewer cross-check):

```
1.  BlockedDomainAlwaysDeniesRegardlessOfCallKind (L52)
2.  UnknownDomainPromptsForDomainApproval         (L63)
3.  EmptyTrustLevelTreatedAsUnknown               (L73)
4.  IdentityKeyRevealPromptsByDefault             (L87)
5.  IdentityKeyRevealSilentWhenPersistentlyApproved (L96)
6.  IdentityKeyRevealSilentWhenSessionOptIn       (L105)
7.  CounterpartyLinkagePromptsByDefault           (L118)
8.  SpecificLinkagePromptsByDefault               (L127)
9.  KeyLinkageSilentWhenSessionOptIn              (L136)
10. SensitiveCertFieldAlwaysPromptsEvenWithOptIn  (L149)
11. ProtocolUseSilentWhenScopedGrantExists        (L166)
12. ProtocolUsePromptsWhenNoScopedGrant           (L175)
13. BasketAccessPromptsWhenNoScopedGrant          (L185)
14. CounterpartyUseSilentWhenGrantExists          (L195)
15. PaymentWithinAllCapsIsSilent                  (L208)
16. PaymentExceedingPerTxCapPromptsConfirmation   (L220)
17. PaymentExceedingPerSessionCapPromptsConfirmation (L230)
18. PaymentExceedingRateLimitPromptsRateLimit     (L241)
19. PaymentAtSessionTxCountPromptsRateLimit       (L252)
20. PaymentExactlyAtPerTxCapIsSilent              (L263)
21. PaymentPriceUnavailablePromptsConfirmation    (L273)
22. PaymentPriceAvailableWithZeroCentsStillSilent (L289)
23. PaymentWithMissingProtocolPromptsProtocolPermission   (L309)
24. PaymentWithMissingBasketPromptsBasketPermission       (L326)
25. PaymentWithMissingCounterpartyPromptsCounterpartyPermission (L340)
26. PaymentScopeMissingTakesPriorityOverCapExceedance     (L354)
27. PaymentNoScopeMissingFallsThroughToCapChecks          (L372)
28. PaymentUnknownScopeValueDefaultsToProtocolPrompt      (L389)
29. CertDisclosureSilentWhenAllFieldsPreApproved (L408)
30. CertDisclosurePromptsWhenFieldsUnapproved    (L417)
31. GenericApprovedCallIsSilent                  (L431)
32. BlockedDomainWinsOverIdentityKeyOptIn        (L443)
33. UnknownDomainWinsOverPrivacyPerimeter        (L453)
```

### Cross-validation

After all 33 pass on the Rust side, run a comparison pass: same context fed to C++ engine via `permission_engine_test.cpp` ground truth → Rust engine outputs match exactly. This is the literal "engine port produced identical decisions" check.

## 5. V20 migration schema

V20 creates two tables in a single migration function. Idempotent CREATE IF NOT EXISTS guards per existing migration pattern (`migrations.rs:525-1060`).

```rust
/// V19 → V20: Permission engine audit log + shadow comparison log.
/// Phase 2.6-A — Engine-to-Rust migration scaffolding. Tables are written
/// to but not read by production code until 2.6-B (shadow) and 2.6-C+ (audit).
pub fn migrate_v19_to_v20(conn: &Connection) -> Result<()> {
    // permission_audit_log — long-lived audit surface.
    // OQ1 RESOLVED: 90-day retention via background purge task.
    // OQ2 RESOLVED: body_hash (sha256 hex, VARCHAR 64) for privacy + provenance.
    conn.execute_batch("
        CREATE TABLE IF NOT EXISTS permission_audit_log (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            approval_id     VARCHAR(32),                       -- 128-bit hex; NULL for Silent decisions
            domain          TEXT    NOT NULL,
            endpoint        TEXT    NOT NULL,
            call_kind       TEXT    NOT NULL,                  -- e.g. 'Payment', 'IdentityKeyReveal'
            engine_reason   TEXT    NOT NULL,                  -- e.g. 'per_tx_limit', 'silent_within_caps'
            decision        TEXT    NOT NULL,                  -- 'silent' | 'prompt' | 'deny'
            user_decision   TEXT,                              -- 'approve' | 'deny' | NULL (still pending)
            body_hash       VARCHAR(64) NOT NULL,              -- sha256 hex of request body
            created_at      INTEGER NOT NULL,                  -- Unix timestamp
            resolved_at     INTEGER,                           -- Unix timestamp; NULL until user resolves
            resolved_via    TEXT                               -- 'modal_approve' | 'modal_deny' | 'timeout' | NULL
        );
        CREATE INDEX IF NOT EXISTS idx_audit_created_at ON permission_audit_log(created_at);
        CREATE INDEX IF NOT EXISTS idx_audit_domain     ON permission_audit_log(domain);
        CREATE INDEX IF NOT EXISTS idx_audit_approval   ON permission_audit_log(approval_id);
    ")?;

    // engine_shadow_log — dropped in 2.6-H cleanup (V21).
    // Comparison rows from C++ (authoritative) vs Rust (shadow) per LD5.
    conn.execute_batch("
        CREATE TABLE IF NOT EXISTS engine_shadow_log (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            call_kind_class TEXT    NOT NULL,                  -- 'privacy_perimeter' | 'scoped_grant' | 'payment' | 'cert_disclosure' | 'domain_trust'
            endpoint        TEXT    NOT NULL,
            domain          TEXT    NOT NULL,
            cpp_decision    TEXT    NOT NULL,                  -- 'silent' | 'prompt' | 'deny'
            rust_decision   TEXT    NOT NULL,
            cpp_reason      TEXT,
            rust_reason     TEXT,
            agreement       INTEGER NOT NULL,                  -- 1 = agree, 0 = disagree
            context_hash    VARCHAR(64) NOT NULL,              -- sha256 of serialized PermissionContext
            observed_at     INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_shadow_observed_at ON engine_shadow_log(observed_at);
        CREATE INDEX IF NOT EXISTS idx_shadow_agreement   ON engine_shadow_log(agreement);
        CREATE INDEX IF NOT EXISTS idx_shadow_class       ON engine_shadow_log(call_kind_class);
    ")?;

    Ok(())
}
```

### Migration runner wiring

In `connection.rs` after L912 (current last call):

```rust
            if version < 20 {
                log::info!("Running migration V19 -> V20 (permission_audit_log + engine_shadow_log)");
                migrations::migrate_v19_to_v20(&self.conn)?;
                self.conn.execute_batch("PRAGMA user_version = 20;")?;
            }
```

## 6. `EngineFlags` struct definition

```rust
// src/permission_service/flags.rs
use std::sync::Arc;

#[derive(Debug, Clone, Copy)]
pub struct EngineFlags {
    pub privacy_perimeter: bool,
    pub scoped_grant:      bool,
    pub payment:           bool,
    pub cert_disclosure:   bool,
    pub domain_trust:      bool,
}

impl Default for EngineFlags {
    /// All flags OFF by default (LD3 — production-safe default per locked decision).
    fn default() -> Self {
        Self {
            privacy_perimeter: false,
            scoped_grant:      false,
            payment:           false,
            cert_disclosure:   false,
            domain_trust:      false,
        }
    }
}

impl EngineFlags {
    /// Reads flags from environment variables. All default to false unless the
    /// matching env var is set to "1" or "true" (case-insensitive).
    ///
    /// Env var names use the prefix HODOS_ENGINE_RUST_ for consistency with the
    /// dev-runbook precedent (HODOS_DEV=1). See PHASE_2_6_ENGINE_TO_RUST.md LD3.
    pub fn from_env() -> Self {
        fn read(name: &str) -> bool {
            match std::env::var(name) {
                Ok(v) => matches!(v.to_lowercase().as_str(), "1" | "true"),
                Err(_) => false,
            }
        }
        Self {
            privacy_perimeter: read("HODOS_ENGINE_RUST_PRIVACY_PERIMETER"),
            scoped_grant:      read("HODOS_ENGINE_RUST_SCOPED_GRANT"),
            payment:           read("HODOS_ENGINE_RUST_PAYMENT"),
            cert_disclosure:   read("HODOS_ENGINE_RUST_CERT_DISCLOSURE"),
            domain_trust:      read("HODOS_ENGINE_RUST_DOMAIN_TRUST"),
        }
    }

    /// Helper for the audit log "call_kind_class" field — maps a CallKind to its flag class.
    pub fn flag_for_call_kind(kind: hodos_permission_engine::CallKind) -> &'static str {
        use hodos_permission_engine::CallKind::*;
        match kind {
            IdentityKeyReveal | CounterpartyKeyLinkage | SpecificKeyLinkage | SensitiveCertField
                => "privacy_perimeter",
            ProtocolUse | BasketAccess | CounterpartyUse
                => "scoped_grant",
            Payment
                => "payment",
            CertificateDisclosure
                => "cert_disclosure",
            DomainTrust | GenericApproved
                => "domain_trust",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_all_off() {
        let f = EngineFlags::default();
        assert!(!f.privacy_perimeter);
        assert!(!f.scoped_grant);
        assert!(!f.payment);
        assert!(!f.cert_disclosure);
        assert!(!f.domain_trust);
    }
    // from_env tests use temp env vars; defer to integration test fixture.
}
```

`PermissionService` holds an `Arc<EngineFlags>` (cheap to clone per request). The `services` field of `AppState` is **not** touched — that stays the Phase 1.6 WalletServices facade.

## 7. Sub-commit breakdown

Six sub-commits, each one focused chunk with build clean between. Mirrors the 5.a-5.f / 6.a-6.f discipline.

| Sub-commit | Scope | Test gate |
|---|---|---|
| **2.6-A.1** | Workspace conversion only. `rust-wallet/Cargo.toml` gains `[workspace]` + `resolver = "2"`. Stub `crates/hodos_permission_engine/Cargo.toml` + empty `lib.rs` so workspace resolves. NO logic. | `cargo build --release` succeeds; `cargo test` passes existing suite unchanged; `dev-wallet.ps1` still runs the wallet |
| **2.6-A.2** | Pure engine crate types. `context.rs`, `decision.rs` with full enums + structs + serde impls. NO `decide()` body yet. | `cargo build -p hodos_permission_engine` succeeds; types compile-check |
| **2.6-A.3** | `matrix_c.rs` + `decide()` body. C++ engine logic translated. | Engine compiles; manual sanity check on a few hand-built contexts (no full test suite yet) |
| **2.6-A.4** | Test port — all 33 tests translated 1:1. | `cargo test -p hodos_permission_engine` passes all 33 tests |
| **2.6-A.5** | `permission_service` module + `EngineFlags` + V20 migration + new repos. Module is dormant — no handlers register yet. | `cargo build` succeeds; V20 migration applies to fresh + existing dev DB; PermissionService::new() returns Ok |
| **2.6-A.6** | AppState wiring — `permission: Arc<PermissionService>` field added; constructor in `main()` builds it from `EngineFlags::from_env()`. | Wallet starts; smoke matrix from Phase 2.5 closure still green; macOS build succeeds |

Estimated time: 6-10 hours focused work across the six sub-commits.

## 8. Open questions — all resolved

| Q | Resolution | Source |
|---|---|---|
| Workspace approach | Option A (hybrid `[workspace]` + `[package]`) | Discussion 2026-06-02 |
| `EngineFlags` location | `permission_service/flags.rs` (co-located) | Discussion 2026-06-02 |
| Migration number | V20 | Verified 2026-06-02 — V19 is max at `connection.rs:912` |
| Test count | 33 | Verified 2026-06-02 — `grep '^TEST(' permission_engine_test.cpp` |
| Audit log retention | 90 days (OQ1 RESOLVED in plan doc) | Discussion 2026-06-02 |
| Audit body column | `body_hash` sha256 VARCHAR(64) (OQ2 RESOLVED in plan doc) | Discussion 2026-06-02 |
| Env var naming convention | `HODOS_ENGINE_RUST_<CLASS>` matching CallKind class names | Mirrors `HODOS_DEV=1` precedent |

## 9. Risks

| Risk | Severity | Mitigation |
|---|---|---|
| Workspace conversion breaks `cargo build` for hodos-wallet package | **High** | Smallest-possible first commit (2.6-A.1 is workspace-only). Smoke `cargo build --release` BEFORE and AFTER the conversion commit. Revert if binary behavior changes. |
| Test port introduces subtle decision divergence (Rust semantics differ from C++ on edge cases) | **Medium** | 1:1 port discipline; side-by-side diff each test during port; reviewer cross-checks the 33-test list |
| V20 migration conflicts with a future migration landing in another branch | **Low (now confirmed)** | Verified 2026-06-02 via grep: no `migrate_v19_to_v20` exists anywhere in the codebase. V19 is genuine max. Coordinate with team if any branch claims V20 before this lands. |
| `resolver = "2"` changes dependency resolution and produces a different binary | **Medium** | Diff Cargo.lock before/after conversion; if dependency tree shifts unexpectedly, investigate before committing. The `hodos-wallet` package is `edition = "2021"` which already implies resolver v2 semantics by default — adding it explicitly should be a no-op. |
| Dev/prod DB safeguard fails after workspace conversion (the `HODOS_DEV=1` guard in main.rs) | **Low** | Safeguard is in `main.rs` runtime check, not Cargo. Verified no behavior change expected. |
| Cargo.lock churn pollutes commits | **Low** | Commit Cargo.lock once per sub-commit; if churn is large, separate Cargo.lock commit from feature commit for cleaner review |
| `EngineFlags::from_env()` reads at startup only; flag flip during wallet runtime not supported | **By design** | Flag flips happen via `HODOS_ENGINE_RUST_<CLASS>=1` env var + wallet restart (matches dev workflow). Runtime flag flip is not a 2.6 use case — could add later if needed |

## 10. Acceptance criteria sweep (mirrors PHASE_2_6 plan doc)

Cumulative after 2.6-A.6:

| # | Criterion | How verified |
|---|---|---|
| 1 | `cargo build --release` clean + existing wallet builds unchanged | Manual build before and after; binary smoke test |
| 2 | `cargo test -p hodos_permission_engine` passes all 33 tests | Test runner output |
| 3 | `cargo test` (root) passes — no regressions | Test runner output |
| 4 | V20 migration applies to fresh + existing dev DB | Manual: fresh DB creation; old DB upgrade |
| 5 | `PermissionService::new()` constructs at startup | Wallet startup log |
| 6 | AppState fields accessible from handlers | Type check + `cargo build` succeeds with a dummy handler reading `state.permission` |
| 7 | All 5 flags default false | Unit test in flags.rs |
| 8 | No production code paths exercise the engine | grep: no call to `state.permission.decide(...)` outside test code |
| 9 | Phase 2.5 closure smoke matrix still green | Real-world smoke per CLAUDE.md "Testing Standards" Minimal tier |
| 10 | macOS parity: pure crate has zero CEF deps | grep: `grep -r 'cef\|CEF\|cef-native' rust-wallet/crates/hodos_permission_engine/` returns empty |
| 11 | Workspace conversion doesn't regress wallet binary | Pre/post diff of `cargo build --release` output |

## 11. Out of scope for 2.6-A

- Shadow infrastructure (POSTs from C++, `/engine/shadow-decide` endpoint) — that's 2.6-B
- Any CallKind class going live (Privacy Perimeter is 2.6-C, etc.)
- Calling `PermissionService::decide()` from any handler
- SessionManager migration (that's 2.6-E)
- Context builder real implementation (placeholder only in 2.6-A; real per-CallKind impl in 2.6-C through 2.6-G)
- BRC-121 path changes (post-2.6 polish)

## 12. Related docs

- [`PHASE_2_6_ENGINE_TO_RUST.md`](./PHASE_2_6_ENGINE_TO_RUST.md) — phase plan + 5 locked decisions
- `cef-native/include/core/PermissionEngine.h` — C++ engine header (port source)
- `cef-native/src/core/PermissionEngine.cpp` — C++ engine body (port source)
- `cef-native/tests/permission_engine_test.cpp` — 33 tests (port source)
- `rust-wallet/src/database/migrations.rs` — migration function host (V20 lands here)
- `rust-wallet/src/database/connection.rs` — migration runner (L912 is current last call)
- `rust-wallet/src/main.rs` — AppState (`permission` field lands here)
- Memory `phase26_plan_drafted_2026_06_02` — sprint state at kickoff
