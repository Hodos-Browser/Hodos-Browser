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
///   2. **Bundled scope grant → Silent** (Phase 2.6-D Fix #4). If the user
///      ticked "Allow this site to perform wallet operations without
///      prompting each time" on the connect modal,
///      `domain_permissions.bundled_scope_grant=1` and ProtocolUse +
///      BasketAccess are silent. Protected baskets are NOT silenced here —
///      `dispatch_scoped_grant` overrides `bundled_scope_grant` to false for
///      basket access against `default`/`backup-*`/`admin *`.
///   3. **Matching V18 row → Silent**. Per-call explicit grant from a prior
///      "Always allow" prompt.
///   4. Otherwise → Prompt with the appropriate scope modal.
///
/// Mirrors C++ `DecideScopedGrant` (PermissionEngine.cpp:83-113) with the
/// Fix #3 / Fix #4 deltas.
fn decide_scoped_grant(ctx: &PermissionContext) -> PermissionDecision {
    // Fix #3 — CounterpartyUse is silent for approved domains.
    if ctx.call_kind == CallKind::CounterpartyUse {
        return PermissionDecision::silent(EngineReason::SilentCounterpartyDefault);
    }

    // Fix #4 — bundle-grant covers ProtocolUse + BasketAccess on approved
    // domains where the user opted into the bundled grant on the connect
    // modal. dispatch_scoped_grant has already cleared protected baskets.
    if ctx.bundled_scope_grant {
        return PermissionDecision::silent(EngineReason::SilentBundledScopeGrant);
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

    // ── Phase 2.6-D Fix #4 — bundled_scope_grant covers ProtocolUse + BasketAccess ──

    #[test]
    fn fix4_bundle_grant_silences_protocol_use_without_v18_row() {
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
            PermissionDecision::silent(EngineReason::SilentBundledScopeGrant)
        );
    }

    #[test]
    fn fix4_bundle_grant_silences_basket_access_without_v18_row() {
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
            PermissionDecision::silent(EngineReason::SilentBundledScopeGrant)
        );
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
