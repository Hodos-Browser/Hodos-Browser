//! Matrix C branch dispatch — the decision logic that today lives in C++ at
//! `cef-native/src/core/PermissionEngine.cpp`.
//!
//! Branch order matches Matrix C top-to-bottom per `PERMISSION_UX_DESIGN.md` §3:
//!   1. Domain trust   (blocked → Deny, unknown → Prompt, approved → continue)
//!   2. Privacy perimeter (identity-key, key-linkage, sensitive cert)
//!   3. Scoped grants (protocol, basket, counterparty)
//!   4. Payment caps + scope-missing precedence
//!   5. Cert disclosure (non-sensitive fields)
//!   6. Generic approved → Silent
//!
//! C++ helpers use a fall-through pattern: they return `Silent` with empty
//! reason to signal "no decision, caller continue." Rust port uses
//! `Option<PermissionDecision>` for `decide_domain_trust` because that's the
//! only branch that can legitimately fall through. The other branches are only
//! called when their `CallKind` predicate has already matched, so they always
//! return a decision.

use crate::context::{CallKind, PaymentScopeKind, PermissionContext, TrustLevel};
use crate::decision::{EngineReason, PermissionDecision, PromptType};

/// Run the full cascade. This is the function `lib.rs::decide()` ultimately
/// delegates to. Pure function — same input always produces the same output.
pub(crate) fn decide(ctx: &PermissionContext) -> PermissionDecision {
    // 1. Domain trust gates everything else.
    //    Blocked domains can't even prompt; unknown domains prompt for
    //    domain approval before any other check.
    if let Some(d) = decide_domain_trust(ctx) {
        return d;
    }

    // 2. Privacy perimeter — always-prompt unless persistently opted in.
    //    Takes precedence over scoped/payment gates because privacy-perimeter
    //    calls MUST always prompt (or honor explicit opt-in) regardless of
    //    spending caps in play.
    if is_privacy_perimeter_kind(ctx.call_kind) {
        return decide_privacy_perimeter(ctx);
    }

    // 3. Scoped grants — protocol/basket/counterparty.
    if is_scoped_grant_kind(ctx.call_kind) {
        return decide_scoped_grant(ctx);
    }

    // 4. Payment caps + scope-missing precedence.
    if ctx.call_kind == CallKind::Payment {
        return decide_payment(ctx);
    }

    // 5. Cert disclosure (non-sensitive — sensitive went through privacy perimeter).
    if ctx.call_kind == CallKind::CertificateDisclosure {
        if ctx.scoped_grant_exists {
            // Caller resolves "every requested field pre-approved" via the
            // existing cert_field_permissions table and signals it via
            // scoped_grant_exists. See PermissionEngine.cpp:252.
            return PermissionDecision::silent(EngineReason::SilentAllCertFieldsApproved);
        }
        return PermissionDecision::prompt(
            PromptType::CertificateDisclosure,
            EngineReason::CertFieldUnapproved,
        );
    }

    // 6. Generic approved-domain catch-all.
    PermissionDecision::silent(EngineReason::SilentGenericApproved)
}

/// Branch 1 — domain trust.
///
/// Returns `Some(decision)` when trust forces an immediate decision (blocked or
/// unknown), `None` when trust is approved and the caller should continue the
/// cascade. Mirrors C++ `DecideDomainTrust` (PermissionEngine.cpp:64-81).
fn decide_domain_trust(ctx: &PermissionContext) -> Option<PermissionDecision> {
    match ctx.trust_level {
        TrustLevel::Blocked => Some(PermissionDecision::deny(EngineReason::TrustBlocked)),
        TrustLevel::Unknown => {
            // Phase 2.6-G — a valid wallet-manifest lets us open the richer
            // manifest_connect_bundle modal (permissions declared up-front)
            // instead of a bare domain_approval. The caller fetches the
            // manifest and signals its presence via `manifest_present`.
            if ctx.manifest_present {
                Some(PermissionDecision::prompt(
                    PromptType::ManifestConnectBundle,
                    EngineReason::NewDomainWithManifest,
                ))
            } else {
                Some(PermissionDecision::prompt(
                    PromptType::DomainApproval,
                    EngineReason::NewDomainNoManifest,
                ))
            }
        }
        TrustLevel::Approved => None,
    }
}

/// Branch 2 — privacy perimeter.
///
/// Identity-key reveal honors the V17 persistent column OR the session opt-in
/// cache; key-linkage honors session opt-in only (no persistent column);
/// sensitive cert field ALWAYS prompts (no opt-out per design principle #1
/// — see PermissionEngine.cpp:44-47).
///
/// Caller (the `decide` cascade) only invokes this when `call_kind` is one of
/// the four privacy-perimeter kinds, so the unreachable arm is structural.
fn decide_privacy_perimeter(ctx: &PermissionContext) -> PermissionDecision {
    match ctx.call_kind {
        CallKind::IdentityKeyReveal => {
            if ctx.identity_key_disclosure_allowed {
                PermissionDecision::silent(EngineReason::SilentIdentityKeyDisclosureAllowed)
            } else if ctx.identity_key_session_opt_in {
                PermissionDecision::silent(EngineReason::SilentSessionOptIn)
            } else {
                PermissionDecision::prompt(
                    PromptType::IdentityKeyReveal,
                    EngineReason::PrivacyPerimeterNoGrant,
                )
            }
        }
        CallKind::CounterpartyKeyLinkage | CallKind::SpecificKeyLinkage => {
            if ctx.key_linkage_session_opt_in {
                PermissionDecision::silent(EngineReason::SilentSessionOptIn)
            } else {
                PermissionDecision::prompt(
                    PromptType::KeyLinkageReveal,
                    EngineReason::PrivacyPerimeterNoGrant,
                )
            }
        }
        CallKind::SensitiveCertField => {
            // Sensitive cert fields ALWAYS prompt, no opt-out path.
            // C++ PermissionEngine.cpp:43-51.
            PermissionDecision::prompt(
                PromptType::CertificateDisclosure,
                EngineReason::SensitiveCertField,
            )
        }
        _ => unreachable!("decide_privacy_perimeter called with non-privacy-perimeter CallKind"),
    }
}

/// Branch 3 — scoped grants.
///
/// Decision order on an approved-trust domain (the only trust level that
/// reaches this branch — see `decide_domain_trust`):
///   1. **CounterpartyUse → Silent** (Phase 2.6-D Fix #3). BRC-42 counterparty
///      key derivation is mathematically one-sided and reveals nothing the
///      dApp doesn't already know. Prompting per-counterparty collapses UX
///      on token-issuing dApps that use one counterparty per recipient.
///   2. **Matching V18 row → Silent**. The user approved this exact scope —
///      either by leaving it ticked on the connect screen, or by answering
///      "Always allow" to a prior prompt.
///   3. Otherwise → Prompt with the appropriate scope modal.
///
/// Mirrors C++ `DecideScopedGrant` (PermissionEngine.cpp:83-113) with the
/// Fix #3 delta.
///
/// ⛔ **beta.3 Phase 7c removed the `bundled_scope_grant` arm that used to sit
/// between 1 and 2.** It returned `Silent` for *every* ProtocolUse and
/// BasketAccess while the flag was set — declared or not, ticked or not — so
/// the per-item list on the connect screen was a preview rather than a limit,
/// and the V18 rows those ticks write were never read.
/// (`TICKET_quiet_mode_wider_than_manifest.md`, owner yes 2026-09-07.)
///
/// Three things a future edit needs to know, because each one was measured
/// and each one is a trap:
///
/// - ⛔ **Do not "fix" this by reordering.** Both arms returned `Silent`, so
///   swapping them was a pure no-op in all four input cells. The arm had to
///   go, not move.
/// - ⛔ **Do not narrow it by reading `domain_manifest_snapshots`.** That
///   table is informational only and must never be a decision input
///   (`R-SNAPSHOT` / `P0.8-A11`, owner-approved 2026-08-22) — a site could
///   otherwise widen its own grants by republishing after approval. The V18
///   child tables, i.e. `scoped_grant_exists`, are the authoritative record.
/// - ⚠️ **`PermissionContext.bundled_scope_grant` is deliberately still here**
///   and deliberately unread by this function. It is retained as the *subject*
///   of `p7c_quiet_mode_never_changes_any_scoped_outcome`, which asserts the
///   decision is identical with the flag on and off for every
///   (call_kind, scoped_grant_exists) pair. Delete the field and that guard
///   goes with it, and re-introducing the bug becomes a one-line change nobody
///   catches.
///   ⛔ It is **not** what protects the protected baskets. That guardrail lives
///   in `request_gate.rs :: dispatch_scoped_grant`, which forces
///   `scoped_grant_exists = false` for `default` / `backup-*` / `admin *`
///   before the context is built — unaffected by this phase.
fn decide_scoped_grant(ctx: &PermissionContext) -> PermissionDecision {
    // Fix #3 — CounterpartyUse is silent for approved domains.
    if ctx.call_kind == CallKind::CounterpartyUse {
        return PermissionDecision::silent(EngineReason::SilentCounterpartyDefault);
    }

    // Level-0 protocols are open usage — match the reference wallet, which
    // returns before any permission check. Reached only on an approved domain
    // (an unknown or blocked domain is stopped by `decide_domain_trust`), and
    // only for ProtocolUse: level 1 and 2 still need a grant or a prompt.
    // 📏 Found on zanaadu.com: every "like" signs under `[0, "xanaverse"]`,
    // which its manifest never declares, so the user was asked on every like.
    if ctx.call_kind == CallKind::ProtocolUse && ctx.protocol_security_level == Some(0) {
        return PermissionDecision::silent(EngineReason::SilentProtocolLevelZero);
    }

    if ctx.scoped_grant_exists {
        return PermissionDecision::silent(EngineReason::SilentScopedGrantExists);
    }
    match ctx.call_kind {
        CallKind::ProtocolUse => PermissionDecision::prompt(
            PromptType::ProtocolPermissionPrompt,
            EngineReason::ScopedGrantMissing,
        ),
        CallKind::BasketAccess => PermissionDecision::prompt(
            PromptType::BasketPermissionPrompt,
            EngineReason::ScopedGrantMissing,
        ),
        // CounterpartyUse handled above; this arm is unreachable but kept
        // for exhaustiveness if a future change moves the CounterpartyUse
        // short-circuit behind a feature flag.
        CallKind::CounterpartyUse => unreachable!(
            "CounterpartyUse should have returned Silent above"
        ),
        _ => unreachable!("decide_scoped_grant called with non-scoped-grant CallKind"),
    }
}

/// Branch 4 — payment caps + scope-missing precedence.
///
/// Phase 1.5 Step 6 Commit E: a createAction that references a
/// protocol/basket/counterparty the site doesn't have a grant for prompts for
/// the scope FIRST, before the payment cap check. Both gates apply
/// independently: if scope is missing AND over cap, the user approves scope,
/// the request is re-issued, then the cap path fires (separate prompt). On the
/// re-issue, `payment_scope_kind_missing` comes back `None` (caller observed
/// the grant) and the cap path runs.
///
/// Mirrors C++ `DecidePayment` (PermissionEngine.cpp:115-198).
///
/// Note on the C++ "unknown scope value defaults to protocol prompt" case
/// (PermissionEngine.cpp:142-146): that case is unrepresentable in Rust
/// because `payment_scope_kind_missing` is `Option<PaymentScopeKind>` with
/// only three valid variants. The test
/// `PaymentUnknownScopeValueDefaultsToProtocolPrompt` ports as a vacuous test
/// — see permission_engine_test.cpp:389.
fn decide_payment(ctx: &PermissionContext) -> PermissionDecision {
    // Scope-missing takes priority over cap exceedance.
    if let Some(scope) = ctx.payment_scope_kind_missing {
        return match scope {
            PaymentScopeKind::Protocol => PermissionDecision::prompt(
                PromptType::ProtocolPermissionPrompt,
                EngineReason::PaymentScopeProtocolMissing,
            ),
            PaymentScopeKind::Basket => PermissionDecision::prompt(
                PromptType::BasketPermissionPrompt,
                EngineReason::PaymentScopeBasketMissing,
            ),
            PaymentScopeKind::Counterparty => PermissionDecision::prompt(
                PromptType::CounterpartyPermissionPrompt,
                EngineReason::PaymentScopeCounterpartyMissing,
            ),
        };
    }

    // BSV/USD price unavailable — cannot evaluate caps in cents.
    // C++ guards on `requestedCents == 0` here so a Silent-with-known-cents
    // payment isn't accidentally blocked when the price cache momentarily
    // lapses but the caller already computed cents at an earlier moment.
    // See PermissionEngine.cpp:155-160.
    if !ctx.bsv_price_available && ctx.requested_cents == 0 {
        return PermissionDecision::prompt(
            PromptType::PaymentConfirmation,
            EngineReason::PriceUnavailable,
        );
    }

    // Rate limit first — if exceeded, fire rate-limit prompt regardless of cap.
    // C++ guards on `rateLimitPerMin > 0` to avoid false-positive on the
    // default-zero case (no limit configured). See PermissionEngine.cpp:163.
    if ctx.payment_requests_this_minute as i64 >= ctx.rate_limit_per_min
        && ctx.rate_limit_per_min > 0
    {
        return PermissionDecision::prompt(
            PromptType::RateLimitExceeded,
            EngineReason::RateLimit,
        );
    }

    // Max tx per session. Same zero-guard as rate limit.
    // C++ uses promptType="rate_limit_exceeded" here too, on purpose — the
    // user-facing modal is the same shape. See PermissionEngine.cpp:171-176.
    if ctx.payment_count_this_session as i64 >= ctx.max_tx_per_session
        && ctx.max_tx_per_session > 0
    {
        return PermissionDecision::prompt(
            PromptType::RateLimitExceeded,
            EngineReason::MaxTxPerSession,
        );
    }

    // Per-tx cap. Boundary is strict-greater-than: requested == cap is allowed.
    // See test PaymentExactlyAtPerTxCapIsSilent (permission_engine_test.cpp:263).
    if ctx.requested_cents > ctx.per_tx_limit_cents {
        return PermissionDecision::prompt(
            PromptType::PaymentConfirmation,
            EngineReason::PerTxLimit,
        );
    }

    // Cumulative session cap. Strict-greater-than is intentional same as per-tx.
    if ctx.session_spent_cents + ctx.requested_cents > ctx.per_session_limit_cents {
        return PermissionDecision::prompt(
            PromptType::PaymentConfirmation,
            EngineReason::SessionCap,
        );
    }

    // Within all caps — auto-approve.
    PermissionDecision::silent(EngineReason::SilentWithinCaps)
}

/// True iff the CallKind is one of the four privacy-perimeter kinds.
fn is_privacy_perimeter_kind(kind: CallKind) -> bool {
    matches!(
        kind,
        CallKind::IdentityKeyReveal
            | CallKind::CounterpartyKeyLinkage
            | CallKind::SpecificKeyLinkage
            | CallKind::SensitiveCertField
    )
}

/// True iff the CallKind is one of the three scoped-grant kinds.
fn is_scoped_grant_kind(kind: CallKind) -> bool {
    matches!(
        kind,
        CallKind::ProtocolUse | CallKind::BasketAccess | CallKind::CounterpartyUse
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    // Light sanity tests for the cascade — full 33-test port lands in 2.6-A.4.
    // These prove the branches are reachable and the dispatch ordering matches
    // the C++ cascade.

    #[test]
    fn blocked_trust_returns_deny_regardless_of_call_kind() {
        for kind in [
            CallKind::Payment,
            CallKind::IdentityKeyReveal,
            CallKind::ProtocolUse,
            CallKind::GenericApproved,
        ] {
            let ctx = PermissionContext {
                call_kind: kind,
                trust_level: TrustLevel::Blocked,
                ..Default::default()
            };
            let d = decide(&ctx);
            assert!(d.is_deny(), "blocked trust should deny for CallKind::{:?}", kind);
        }
    }

    #[test]
    fn unknown_trust_prompts_domain_approval() {
        let ctx = PermissionContext {
            call_kind: CallKind::Payment,
            trust_level: TrustLevel::Unknown,
            ..Default::default()
        };
        let d = decide(&ctx);
        assert_eq!(
            d,
            PermissionDecision::prompt(
                PromptType::DomainApproval,
                EngineReason::NewDomainNoManifest
            )
        );
    }

    #[test]
    fn unknown_trust_with_manifest_prompts_connect_bundle() {
        // Phase 2.6-G — a valid manifest upgrades the unknown-domain prompt from
        // domain_approval to manifest_connect_bundle.
        let ctx = PermissionContext {
            call_kind: CallKind::Payment,
            trust_level: TrustLevel::Unknown,
            manifest_present: true,
            ..Default::default()
        };
        let d = decide(&ctx);
        assert_eq!(
            d,
            PermissionDecision::prompt(
                PromptType::ManifestConnectBundle,
                EngineReason::NewDomainWithManifest
            )
        );
    }

    #[test]
    fn approved_payment_within_caps_is_silent() {
        let ctx = PermissionContext {
            call_kind: CallKind::Payment,
            trust_level: TrustLevel::Approved,
            per_tx_limit_cents: 100,
            per_session_limit_cents: 1000,
            requested_cents: 50,
            session_spent_cents: 100,
            ..Default::default()
        };
        let d = decide(&ctx);
        assert_eq!(d, PermissionDecision::silent(EngineReason::SilentWithinCaps));
    }

    #[test]
    fn approved_payment_over_per_tx_prompts() {
        let ctx = PermissionContext {
            call_kind: CallKind::Payment,
            trust_level: TrustLevel::Approved,
            per_tx_limit_cents: 100,
            per_session_limit_cents: 1000,
            requested_cents: 200,
            ..Default::default()
        };
        let d = decide(&ctx);
        assert_eq!(
            d,
            PermissionDecision::prompt(PromptType::PaymentConfirmation, EngineReason::PerTxLimit)
        );
    }

    #[test]
    fn identity_key_with_persistent_grant_is_silent() {
        let ctx = PermissionContext {
            call_kind: CallKind::IdentityKeyReveal,
            trust_level: TrustLevel::Approved,
            identity_key_disclosure_allowed: true,
            ..Default::default()
        };
        let d = decide(&ctx);
        assert!(d.is_silent());
    }

    #[test]
    fn identity_key_without_any_grant_prompts() {
        let ctx = PermissionContext {
            call_kind: CallKind::IdentityKeyReveal,
            trust_level: TrustLevel::Approved,
            ..Default::default()
        };
        let d = decide(&ctx);
        assert_eq!(
            d,
            PermissionDecision::prompt(
                PromptType::IdentityKeyReveal,
                EngineReason::PrivacyPerimeterNoGrant
            )
        );
    }

    #[test]
    fn sensitive_cert_field_always_prompts() {
        // Even with every grant true, sensitive cert field still prompts.
        let ctx = PermissionContext {
            call_kind: CallKind::SensitiveCertField,
            trust_level: TrustLevel::Approved,
            identity_key_disclosure_allowed: true,
            identity_key_session_opt_in: true,
            key_linkage_session_opt_in: true,
            scoped_grant_exists: true,
            ..Default::default()
        };
        let d = decide(&ctx);
        assert_eq!(
            d,
            PermissionDecision::prompt(
                PromptType::CertificateDisclosure,
                EngineReason::SensitiveCertField
            )
        );
    }

    #[test]
    fn scoped_grant_existing_silences_protocol_use() {
        let ctx = PermissionContext {
            call_kind: CallKind::ProtocolUse,
            trust_level: TrustLevel::Approved,
            scoped_grant_exists: true,
            ..Default::default()
        };
        let d = decide(&ctx);
        assert_eq!(
            d,
            PermissionDecision::silent(EngineReason::SilentScopedGrantExists)
        );
    }

    // -- beta.3: level-0 protocols are open (TICKET_level_0_protocol_prompts_after_connect) --

    fn protocol_ctx(trust: TrustLevel, level: Option<u8>) -> PermissionContext {
        PermissionContext {
            call_kind: CallKind::ProtocolUse,
            trust_level: trust,
            scoped_grant_exists: false, // the whole point: undeclared, no V18 row
            protocol_security_level: level,
            ..Default::default()
        }
    }

    #[test]
    fn level0_protocol_is_silent_on_approved_domain_without_a_grant() {
        // 📏 zanaadu.com: createSignature under [0, "xanaverse"], not in the manifest.
        assert_eq!(
            decide(&protocol_ctx(TrustLevel::Approved, Some(0))),
            PermissionDecision::silent(EngineReason::SilentProtocolLevelZero)
        );
    }

    #[test]
    fn level1_and_level2_undeclared_protocols_still_prompt() {
        // ⛔ Negative control. A fix that silenced every protocol would pass the
        // test above too — this is the one that would go red.
        for level in [1u8, 2] {
            assert_eq!(
                decide(&protocol_ctx(TrustLevel::Approved, Some(level))),
                PermissionDecision::prompt(
                    PromptType::ProtocolPermissionPrompt,
                    EngineReason::ScopedGrantMissing
                ),
                "undeclared level-{level} protocol must still prompt"
            );
        }
    }

    #[test]
    fn protocol_with_no_level_recorded_fails_closed() {
        // A caller that never wires the level must not get the exemption.
        assert!(decide(&protocol_ctx(TrustLevel::Approved, None)).is_prompt());
    }

    #[test]
    fn level0_protocol_does_not_bypass_domain_trust() {
        // First contact still asks; a blocked site is still refused.
        let unknown = decide(&protocol_ctx(TrustLevel::Unknown, Some(0)));
        assert!(unknown.is_prompt(), "unknown domain must still be asked: {unknown:?}");
        assert_ne!(unknown, PermissionDecision::silent(EngineReason::SilentProtocolLevelZero));
        assert!(decide(&protocol_ctx(TrustLevel::Blocked, Some(0))).is_deny());
    }

    #[test]
    fn level0_applies_only_to_protocol_use() {
        // A basket call carrying a stray level must still need its grant.
        let ctx = PermissionContext {
            call_kind: CallKind::BasketAccess,
            trust_level: TrustLevel::Approved,
            scoped_grant_exists: false,
            protocol_security_level: Some(0),
            ..Default::default()
        };
        assert!(decide(&ctx).is_prompt());
    }

    #[test]
    fn payment_scope_missing_overrides_cap_check() {
        // Even when the payment would otherwise be over-cap, scope-missing fires first.
        let ctx = PermissionContext {
            call_kind: CallKind::Payment,
            trust_level: TrustLevel::Approved,
            per_tx_limit_cents: 100,
            requested_cents: 5000, // way over cap
            payment_scope_kind_missing: Some(PaymentScopeKind::Protocol),
            ..Default::default()
        };
        let d = decide(&ctx);
        assert_eq!(
            d,
            PermissionDecision::prompt(
                PromptType::ProtocolPermissionPrompt,
                EngineReason::PaymentScopeProtocolMissing
            )
        );
    }

    #[test]
    fn cert_disclosure_with_grant_is_silent() {
        let ctx = PermissionContext {
            call_kind: CallKind::CertificateDisclosure,
            trust_level: TrustLevel::Approved,
            scoped_grant_exists: true,
            ..Default::default()
        };
        let d = decide(&ctx);
        assert_eq!(
            d,
            PermissionDecision::silent(EngineReason::SilentAllCertFieldsApproved)
        );
    }

    #[test]
    fn cert_disclosure_without_grant_prompts() {
        // Phase 2.6-F: a non-sensitive cert disclosure where not every requested
        // field is pre-approved must prompt (certificate_disclosure modal).
        let ctx = PermissionContext {
            call_kind: CallKind::CertificateDisclosure,
            trust_level: TrustLevel::Approved,
            scoped_grant_exists: false,
            ..Default::default()
        };
        let d = decide(&ctx);
        assert_eq!(
            d,
            PermissionDecision::prompt(
                PromptType::CertificateDisclosure,
                EngineReason::CertFieldUnapproved,
            )
        );
    }

    #[test]
    fn generic_approved_falls_through_to_silent() {
        let ctx = PermissionContext {
            call_kind: CallKind::GenericApproved,
            trust_level: TrustLevel::Approved,
            ..Default::default()
        };
        let d = decide(&ctx);
        assert_eq!(
            d,
            PermissionDecision::silent(EngineReason::SilentGenericApproved)
        );
    }

    // ── Phase 2.6-D Fix #3 — CounterpartyUse silent for approved domains ──

    #[test]
    fn fix3_counterparty_use_silent_without_grant() {
        // No V18 row, no bundle grant — still silent. The default UX
        // collapse: token-issuing dApps stop prompting per recipient.
        let ctx = PermissionContext {
            call_kind: CallKind::CounterpartyUse,
            trust_level: TrustLevel::Approved,
            scoped_grant_exists: false,
            bundled_scope_grant: false,
            ..Default::default()
        };
        let d = decide(&ctx);
        assert_eq!(
            d,
            PermissionDecision::silent(EngineReason::SilentCounterpartyDefault)
        );
    }

    #[test]
    fn fix3_counterparty_use_silent_uses_default_reason_not_grant_reason() {
        // When BOTH paths would silence (grant exists AND fix #3 applies),
        // the CounterpartyDefault path wins because it sits earlier in
        // decide_scoped_grant. The reason field surfaces the actual silencing
        // mechanism for audit/debugging.
        let ctx = PermissionContext {
            call_kind: CallKind::CounterpartyUse,
            trust_level: TrustLevel::Approved,
            scoped_grant_exists: true,
            ..Default::default()
        };
        let d = decide(&ctx);
        assert_eq!(
            d,
            PermissionDecision::silent(EngineReason::SilentCounterpartyDefault)
        );
    }

    #[test]
    fn fix3_counterparty_use_blocked_domain_still_denies() {
        // Trust=Blocked never reaches scoped-grant branch — deny short-circuits
        // first. The CounterpartyDefault silent should not override Deny.
        let ctx = PermissionContext {
            call_kind: CallKind::CounterpartyUse,
            trust_level: TrustLevel::Blocked,
            ..Default::default()
        };
        let d = decide(&ctx);
        assert_eq!(d, PermissionDecision::deny(EngineReason::TrustBlocked));
    }

    // ── beta.3 Phase 7c — quiet mode covers only what the user approved ──
    //
    // These two were `fix4_bundle_grant_silences_*_without_v18_row`, asserting
    // the opposite. They are REWRITTEN, not deleted: the input they cover — the
    // flag set with no matching grant — is the exact case Phase 7c changed, so
    // deleting them would have dropped coverage behind a green gate.
    // `P7c-A10`.

    #[test]
    fn p7c_quiet_mode_does_not_silence_undeclared_protocol_use() {
        // The whole point of the phase: flag on, no grant for THIS scope ⇒
        // prompt. Before 7c this returned Silent(SilentBundledScopeGrant).
        let ctx = PermissionContext {
            call_kind: CallKind::ProtocolUse,
            trust_level: TrustLevel::Approved,
            scoped_grant_exists: false,
            bundled_scope_grant: true,
            ..Default::default()
        };
        let d = decide(&ctx);
        assert_eq!(
            d,
            PermissionDecision::prompt(
                PromptType::ProtocolPermissionPrompt,
                EngineReason::ScopedGrantMissing
            )
        );
    }

    #[test]
    fn p7c_quiet_mode_does_not_silence_undeclared_basket_access() {
        let ctx = PermissionContext {
            call_kind: CallKind::BasketAccess,
            trust_level: TrustLevel::Approved,
            scoped_grant_exists: false,
            bundled_scope_grant: true,
            ..Default::default()
        };
        let d = decide(&ctx);
        assert_eq!(
            d,
            PermissionDecision::prompt(
                PromptType::BasketPermissionPrompt,
                EngineReason::ScopedGrantMissing
            )
        );
    }

    #[test]
    fn p7c_approved_scope_is_silent_whether_or_not_quiet_mode_is_on() {
        // The other half of `P7c-A1`, and the pair that makes the two tests
        // above meaningful: a scope the user DID approve must stay silent, and
        // must be silent for the same reason either way. If the flag ever
        // changes this outcome again, it has grown a second meaning.
        for quiet in [true, false] {
            for (kind, label) in [
                (CallKind::ProtocolUse, "ProtocolUse"),
                (CallKind::BasketAccess, "BasketAccess"),
            ] {
                let ctx = PermissionContext {
                    call_kind: kind,
                    trust_level: TrustLevel::Approved,
                    scoped_grant_exists: true,
                    bundled_scope_grant: quiet,
                    ..Default::default()
                };
                assert_eq!(
                    decide(&ctx),
                    PermissionDecision::silent(EngineReason::SilentScopedGrantExists),
                    "{label} with an approved grant must be silent for the grant, \
                     not the flag (quiet mode = {quiet})"
                );
            }
        }
    }

    #[test]
    fn p7c_quiet_mode_never_changes_any_scoped_outcome() {
        // ⭐ The generalisation, and the guard that makes a future reorder
        // pointless: for every (call_kind, scoped_grant_exists) pair, the
        // decision must be IDENTICAL with the flag on and off. Phase 7c's
        // whole claim is that `bundled_scope_grant` no longer decides
        // anything on this branch; this asserts it exhaustively rather than
        // one case at a time.
        for kind in [
            CallKind::ProtocolUse,
            CallKind::BasketAccess,
            CallKind::CounterpartyUse,
        ] {
            for exists in [true, false] {
                let mk = |quiet: bool| PermissionContext {
                    call_kind: kind,
                    trust_level: TrustLevel::Approved,
                    scoped_grant_exists: exists,
                    bundled_scope_grant: quiet,
                    ..Default::default()
                };
                assert_eq!(
                    decide(&mk(true)),
                    decide(&mk(false)),
                    "quiet mode changed the outcome for {kind:?} with \
                     scoped_grant_exists={exists}"
                );
            }
        }
    }

    #[test]
    fn fix4_no_bundle_falls_back_to_v18_lookup_for_protocol() {
        // Without bundle_scope_grant + without V18 row → prompt.
        let ctx = PermissionContext {
            call_kind: CallKind::ProtocolUse,
            trust_level: TrustLevel::Approved,
            scoped_grant_exists: false,
            bundled_scope_grant: false,
            ..Default::default()
        };
        let d = decide(&ctx);
        assert_eq!(
            d,
            PermissionDecision::prompt(
                PromptType::ProtocolPermissionPrompt,
                EngineReason::ScopedGrantMissing
            )
        );
    }

    #[test]
    fn fix4_bundle_grant_does_not_affect_unknown_or_blocked_trust() {
        // Engine reaches decide_scoped_grant only via Approved trust per the
        // cascade order — Unknown/Blocked short-circuit at decide_domain_trust.
        // Verify the bundle grant doesn't leak into those branches.
        let ctx_unknown = PermissionContext {
            call_kind: CallKind::ProtocolUse,
            trust_level: TrustLevel::Unknown,
            bundled_scope_grant: true,
            ..Default::default()
        };
        assert_eq!(
            decide(&ctx_unknown),
            PermissionDecision::prompt(
                PromptType::DomainApproval,
                EngineReason::NewDomainNoManifest
            )
        );

        let ctx_blocked = PermissionContext {
            call_kind: CallKind::BasketAccess,
            trust_level: TrustLevel::Blocked,
            bundled_scope_grant: true,
            ..Default::default()
        };
        assert_eq!(
            decide(&ctx_blocked),
            PermissionDecision::deny(EngineReason::TrustBlocked)
        );
    }

    #[test]
    fn fix4_bundle_grant_does_not_affect_payment_kind() {
        // Bundle grant is a SCOPED-GRANT cascade thing. Payment caps run in a
        // separate branch — make sure bundle_scope_grant=true doesn't silence
        // an over-cap payment.
        let ctx = PermissionContext {
            call_kind: CallKind::Payment,
            trust_level: TrustLevel::Approved,
            bundled_scope_grant: true,
            per_tx_limit_cents: 100,
            requested_cents: 5000, // over cap
            ..Default::default()
        };
        let d = decide(&ctx);
        // Should hit the per-tx cap prompt, NOT a silent bundle path.
        assert!(matches!(d, PermissionDecision::Prompt { .. }));
    }

    #[test]
    fn blocked_wins_over_identity_key_opt_in() {
        // Even with a persistent grant, blocked trust trumps everything.
        let ctx = PermissionContext {
            call_kind: CallKind::IdentityKeyReveal,
            trust_level: TrustLevel::Blocked,
            identity_key_disclosure_allowed: true,
            ..Default::default()
        };
        let d = decide(&ctx);
        assert!(d.is_deny());
    }

    #[test]
    fn unknown_wins_over_privacy_perimeter() {
        // Unknown trust prompts for domain_approval, never reaches identity-key gate.
        let ctx = PermissionContext {
            call_kind: CallKind::IdentityKeyReveal,
            trust_level: TrustLevel::Unknown,
            identity_key_disclosure_allowed: true,
            ..Default::default()
        };
        let d = decide(&ctx);
        assert_eq!(
            d,
            PermissionDecision::prompt(
                PromptType::DomainApproval,
                EngineReason::NewDomainNoManifest
            )
        );
    }

    #[test]
    fn payment_at_boundary_is_silent() {
        // Exactly at per-tx cap → Silent (strict > comparison in C++ + Rust).
        let ctx = PermissionContext {
            call_kind: CallKind::Payment,
            trust_level: TrustLevel::Approved,
            per_tx_limit_cents: 100,
            per_session_limit_cents: 1000,
            requested_cents: 100, // exactly at cap
            ..Default::default()
        };
        let d = decide(&ctx);
        assert_eq!(d, PermissionDecision::silent(EngineReason::SilentWithinCaps));
    }

    #[test]
    fn rate_limit_with_zero_config_does_not_trigger() {
        // rate_limit_per_min = 0 means "no limit configured" — should NOT fire
        // even if payment_requests_this_minute is large. C++ guards on > 0.
        let ctx = PermissionContext {
            call_kind: CallKind::Payment,
            trust_level: TrustLevel::Approved,
            per_tx_limit_cents: 100,
            per_session_limit_cents: 1000,
            requested_cents: 50,
            rate_limit_per_min: 0, // no limit
            payment_requests_this_minute: 9999,
            ..Default::default()
        };
        let d = decide(&ctx);
        assert_eq!(d, PermissionDecision::silent(EngineReason::SilentWithinCaps));
    }

    #[test]
    fn bsv_price_unavailable_with_zero_cents_prompts() {
        let ctx = PermissionContext {
            call_kind: CallKind::Payment,
            trust_level: TrustLevel::Approved,
            bsv_price_available: false,
            requested_cents: 0,
            ..Default::default()
        };
        let d = decide(&ctx);
        assert_eq!(
            d,
            PermissionDecision::prompt(
                PromptType::PaymentConfirmation,
                EngineReason::PriceUnavailable
            )
        );
    }

    #[test]
    fn bsv_price_available_with_zero_cents_is_silent() {
        // 0 cents is under every cap → Silent.
        let ctx = PermissionContext {
            call_kind: CallKind::Payment,
            trust_level: TrustLevel::Approved,
            bsv_price_available: true,
            requested_cents: 0,
            per_tx_limit_cents: 100,
            per_session_limit_cents: 1000,
            ..Default::default()
        };
        let d = decide(&ctx);
        assert_eq!(d, PermissionDecision::silent(EngineReason::SilentWithinCaps));
    }
}
