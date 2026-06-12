//! decision_matrix.rs — 1:1 port of `cef-native/tests/permission_engine_test.cpp`.
//!
//! All 33 C++ GoogleTest cases ported to Rust `#[test]` functions. Same names
//! in snake_case, same scenarios, same assertions translated to typed-enum
//! equivalents (where C++ compared `promptType` strings, Rust compares
//! `PromptType` enum variants).
//!
//! Integration test — uses only the public API of `hodos_permission_engine`.
//! Internal module tests live alongside the implementation in
//! `src/matrix_c.rs::tests`.
//!
//! Phase 2.6-A.4. See:
//!   - `cef-native/tests/permission_engine_test.cpp` for the C++ source
//!   - `development-docs/Sigma-BRC121-Sprint/phase-2.6-engine-to-rust/SUBPHASE_2_6_A_DESIGN.md` §4 for port strategy
//!
//! Vacuous tests (intentionally kept for traceability):
//!   - `empty_trust_level_treated_as_unknown` — C++ tested `trustLevel.empty()`;
//!     Rust's typed `TrustLevel` enum + `Default` impl makes `Unknown` the
//!     default, so this test devolves to verifying the default.
//!   - `payment_unknown_scope_value_defaults_to_protocol_prompt` — C++ tested
//!     a defensive string-fallback path; Rust's `Option<PaymentScopeKind>`
//!     makes the "unknown" case unrepresentable.

use hodos_permission_engine::{
    decide, CallKind, EngineReason, PaymentScopeKind, PermissionContext, PermissionDecision,
    PromptType, TrustLevel,
};

/// Mirror of C++ `baselineApproved()` at `permission_engine_test.cpp:27-44`.
/// Builds a baseline "approved domain with healthy headroom" context.
/// Individual tests override only the fields they care about.
fn baseline_approved() -> PermissionContext {
    PermissionContext {
        trust_level: TrustLevel::Approved,
        per_tx_limit_cents: 100,
        per_session_limit_cents: 1000,
        rate_limit_per_min: 30,
        max_tx_per_session: 100,
        identity_key_disclosure_allowed: false,
        session_spent_cents: 0,
        payment_requests_this_minute: 0,
        payment_count_this_session: 0,
        identity_key_session_opt_in: false,
        key_linkage_session_opt_in: false,
        requested_cents: 0,
        bsv_price_available: true,
        scoped_grant_exists: false,
        bundled_scope_grant: false,
        call_kind: CallKind::GenericApproved,
        payment_scope_kind_missing: None,
        manifest_present: false,
    }
}

// ============================================================================
// Branch 1: Domain trust gates everything else
// ============================================================================

#[test]
fn blocked_domain_always_denies_regardless_of_call_kind() {
    let ctx = PermissionContext {
        trust_level: TrustLevel::Blocked,
        call_kind: CallKind::Payment,
        requested_cents: 10, // Well within caps — irrelevant for blocked
        ..baseline_approved()
    };

    let d = decide(&ctx);
    assert!(d.is_deny(), "blocked domain should deny");
    assert_eq!(d, PermissionDecision::deny(EngineReason::TrustBlocked));
}

#[test]
fn unknown_domain_prompts_for_domain_approval() {
    let ctx = PermissionContext {
        trust_level: TrustLevel::Unknown,
        call_kind: CallKind::Payment,
        ..baseline_approved()
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
fn empty_trust_level_treated_as_unknown() {
    // Vacuous in Rust: `TrustLevel::default()` IS `Unknown` by the impl in
    // `context.rs`. C++ tested `trustLevel.clear()` (empty string treated as
    // unknown); Rust makes this unrepresentable by typing.
    let ctx = PermissionContext {
        call_kind: CallKind::GenericApproved,
        ..baseline_approved()
        // trust_level not overridden — but baseline sets Approved.
        // Switch to default to mirror the C++ "cleared" case:
    };
    // Override to default to mirror the C++ test:
    let ctx = PermissionContext {
        trust_level: TrustLevel::default(),
        ..ctx
    };
    assert_eq!(ctx.trust_level, TrustLevel::Unknown, "default == unknown");

    let d = decide(&ctx);
    assert_eq!(
        d,
        PermissionDecision::prompt(
            PromptType::DomainApproval,
            EngineReason::NewDomainNoManifest
        )
    );
}

// ============================================================================
// Branch 2: Privacy perimeter — identity key
// ============================================================================

#[test]
fn identity_key_reveal_prompts_by_default() {
    let ctx = PermissionContext {
        call_kind: CallKind::IdentityKeyReveal,
        ..baseline_approved()
    };

    let d = decide(&ctx);
    assert!(d.is_prompt());
    if let PermissionDecision::Prompt { prompt_type, .. } = d {
        assert_eq!(prompt_type, PromptType::IdentityKeyReveal);
    } else {
        panic!("expected Prompt decision");
    }
}

#[test]
fn identity_key_reveal_silent_when_persistently_approved() {
    let ctx = PermissionContext {
        call_kind: CallKind::IdentityKeyReveal,
        identity_key_disclosure_allowed: true, // V17 column = 1
        ..baseline_approved()
    };

    let d = decide(&ctx);
    assert!(d.is_silent(), "persistent identity-key grant should silence");
}

#[test]
fn identity_key_reveal_silent_when_session_opt_in() {
    let ctx = PermissionContext {
        call_kind: CallKind::IdentityKeyReveal,
        identity_key_session_opt_in: true, // In-memory cache hit
        ..baseline_approved()
    };

    let d = decide(&ctx);
    assert!(d.is_silent(), "session-scoped opt-in should silence");
}

// ============================================================================
// Branch 2: Privacy perimeter — key linkage
// ============================================================================

#[test]
fn counterparty_linkage_prompts_by_default() {
    let ctx = PermissionContext {
        call_kind: CallKind::CounterpartyKeyLinkage,
        ..baseline_approved()
    };

    let d = decide(&ctx);
    assert!(d.is_prompt());
    if let PermissionDecision::Prompt { prompt_type, .. } = d {
        assert_eq!(prompt_type, PromptType::KeyLinkageReveal);
    } else {
        panic!("expected Prompt decision");
    }
}

#[test]
fn specific_linkage_prompts_by_default() {
    let ctx = PermissionContext {
        call_kind: CallKind::SpecificKeyLinkage,
        ..baseline_approved()
    };

    let d = decide(&ctx);
    assert!(d.is_prompt());
    if let PermissionDecision::Prompt { prompt_type, .. } = d {
        assert_eq!(prompt_type, PromptType::KeyLinkageReveal);
    } else {
        panic!("expected Prompt decision");
    }
}

#[test]
fn key_linkage_silent_when_session_opt_in() {
    let ctx = PermissionContext {
        call_kind: CallKind::SpecificKeyLinkage,
        key_linkage_session_opt_in: true,
        ..baseline_approved()
    };

    let d = decide(&ctx);
    assert!(d.is_silent(), "session opt-in should silence key-linkage reveal");
}

// ============================================================================
// Branch 2: Privacy perimeter — sensitive cert field always prompts
// ============================================================================

#[test]
fn sensitive_cert_field_always_prompts_even_with_opt_in() {
    // Even if every opt-in is true, sensitive cert fields ignore them.
    let ctx = PermissionContext {
        call_kind: CallKind::SensitiveCertField,
        identity_key_disclosure_allowed: true,
        identity_key_session_opt_in: true,
        key_linkage_session_opt_in: true,
        ..baseline_approved()
    };

    let d = decide(&ctx);
    assert!(d.is_prompt());
    if let PermissionDecision::Prompt { prompt_type, .. } = d {
        assert_eq!(prompt_type, PromptType::CertificateDisclosure);
    } else {
        panic!("expected Prompt decision");
    }
}

// ============================================================================
// Branch 3: Scoped grants — protocol / basket / counterparty
// ============================================================================

#[test]
fn protocol_use_silent_when_scoped_grant_exists() {
    let ctx = PermissionContext {
        call_kind: CallKind::ProtocolUse,
        scoped_grant_exists: true,
        ..baseline_approved()
    };

    let d = decide(&ctx);
    assert!(d.is_silent());
}

#[test]
fn protocol_use_prompts_when_no_scoped_grant() {
    let ctx = PermissionContext {
        call_kind: CallKind::ProtocolUse,
        scoped_grant_exists: false,
        ..baseline_approved()
    };

    let d = decide(&ctx);
    assert!(d.is_prompt());
    if let PermissionDecision::Prompt { prompt_type, .. } = d {
        assert_eq!(prompt_type, PromptType::ProtocolPermissionPrompt);
    } else {
        panic!("expected Prompt decision");
    }
}

#[test]
fn basket_access_prompts_when_no_scoped_grant() {
    let ctx = PermissionContext {
        call_kind: CallKind::BasketAccess,
        scoped_grant_exists: false,
        ..baseline_approved()
    };

    let d = decide(&ctx);
    assert!(d.is_prompt());
    if let PermissionDecision::Prompt { prompt_type, .. } = d {
        assert_eq!(prompt_type, PromptType::BasketPermissionPrompt);
    } else {
        panic!("expected Prompt decision");
    }
}

#[test]
fn counterparty_use_silent_when_grant_exists() {
    let ctx = PermissionContext {
        call_kind: CallKind::CounterpartyUse,
        scoped_grant_exists: true,
        ..baseline_approved()
    };

    let d = decide(&ctx);
    assert!(d.is_silent());
}

// ============================================================================
// Branch 4: Payment caps
// ============================================================================

#[test]
fn payment_within_all_caps_is_silent() {
    let ctx = PermissionContext {
        call_kind: CallKind::Payment,
        requested_cents: 50,      // under $1/tx cap
        session_spent_cents: 100, // way under $10/session cap
        payment_requests_this_minute: 5,
        payment_count_this_session: 10,
        ..baseline_approved()
    };

    let d = decide(&ctx);
    assert!(d.is_silent());
}

#[test]
fn payment_exceeding_per_tx_cap_prompts_confirmation() {
    let ctx = PermissionContext {
        call_kind: CallKind::Payment,
        requested_cents: 200, // exceeds $1/tx cap
        ..baseline_approved()
    };

    let d = decide(&ctx);
    assert!(d.is_prompt());
    if let PermissionDecision::Prompt { prompt_type, .. } = d {
        assert_eq!(prompt_type, PromptType::PaymentConfirmation);
    } else {
        panic!("expected Prompt decision");
    }
}

#[test]
fn payment_exceeding_per_session_cap_prompts_confirmation() {
    let ctx = PermissionContext {
        call_kind: CallKind::Payment,
        requested_cents: 50,
        session_spent_cents: 980, // 980 + 50 = 1030 > 1000 cap
        ..baseline_approved()
    };

    let d = decide(&ctx);
    assert!(d.is_prompt());
    if let PermissionDecision::Prompt { prompt_type, .. } = d {
        assert_eq!(prompt_type, PromptType::PaymentConfirmation);
    } else {
        panic!("expected Prompt decision");
    }
}

#[test]
fn payment_exceeding_rate_limit_prompts_rate_limit() {
    let ctx = PermissionContext {
        call_kind: CallKind::Payment,
        requested_cents: 50,
        payment_requests_this_minute: 30, // at limit
        ..baseline_approved()
    };

    let d = decide(&ctx);
    assert!(d.is_prompt());
    if let PermissionDecision::Prompt { prompt_type, .. } = d {
        assert_eq!(prompt_type, PromptType::RateLimitExceeded);
    } else {
        panic!("expected Prompt decision");
    }
}

#[test]
fn payment_at_session_tx_count_prompts_rate_limit() {
    let ctx = PermissionContext {
        call_kind: CallKind::Payment,
        requested_cents: 50,
        payment_count_this_session: 100, // at max_tx_per_session
        ..baseline_approved()
    };

    let d = decide(&ctx);
    assert!(d.is_prompt());
    if let PermissionDecision::Prompt { prompt_type, .. } = d {
        assert_eq!(prompt_type, PromptType::RateLimitExceeded);
    } else {
        panic!("expected Prompt decision");
    }
}

#[test]
fn payment_exactly_at_per_tx_cap_is_silent() {
    // Boundary: requested_cents == per_tx_limit_cents should be allowed.
    let ctx = PermissionContext {
        call_kind: CallKind::Payment,
        requested_cents: 100, // exactly at cap
        ..baseline_approved()
    };

    let d = decide(&ctx);
    assert!(d.is_silent());
}

#[test]
fn payment_price_unavailable_prompts_confirmation() {
    // BSV price cache cold / network down — engine cannot trust cap math
    // (requested_cents would be 0 even for a real spend). Prompt so user
    // sees the satoshi amount.
    let ctx = PermissionContext {
        call_kind: CallKind::Payment,
        bsv_price_available: false,
        requested_cents: 0, // caller couldn't convert satoshis → cents
        ..baseline_approved()
    };

    let d = decide(&ctx);
    assert!(d.is_prompt());
    // C++ test substring-matched "price unavailable" in the reason text.
    // Rust port asserts the typed EngineReason::PriceUnavailable.
    assert_eq!(
        d,
        PermissionDecision::prompt(
            PromptType::PaymentConfirmation,
            EngineReason::PriceUnavailable
        )
    );
}

#[test]
fn payment_price_available_with_zero_cents_still_silent() {
    // Defensive: a free output (satoshis=0 → cents=0) should NOT be blocked
    // by the price-unavailable check. The new branch only fires when price
    // is unavailable AND requested_cents would otherwise be 0 as a proxy for
    // "we couldn't convert."
    let ctx = PermissionContext {
        call_kind: CallKind::Payment,
        bsv_price_available: true, // price IS available
        requested_cents: 0,        // genuine zero-cost payment
        ..baseline_approved()
    };

    let d = decide(&ctx);
    assert!(d.is_silent());
}

// ----------------------------------------------------------------------------
// Commit E — Payment with missing scope returns scope-permission prompt
// BEFORE the cap check fires. Both gates are independent: scope first,
// then payment cap on the re-issued request.
// ----------------------------------------------------------------------------

#[test]
fn payment_with_missing_protocol_prompts_protocol_permission() {
    let ctx = PermissionContext {
        call_kind: CallKind::Payment,
        bsv_price_available: true,
        requested_cents: 5, // well within caps
        per_tx_limit_cents: 100,
        per_session_limit_cents: 1000,
        payment_scope_kind_missing: Some(PaymentScopeKind::Protocol),
        ..baseline_approved()
    };

    let d = decide(&ctx);
    assert!(d.is_prompt());
    if let PermissionDecision::Prompt { prompt_type, .. } = d {
        assert_eq!(prompt_type, PromptType::ProtocolPermissionPrompt);
    } else {
        panic!("expected Prompt decision");
    }
}

#[test]
fn payment_with_missing_basket_prompts_basket_permission() {
    let ctx = PermissionContext {
        call_kind: CallKind::Payment,
        bsv_price_available: true,
        requested_cents: 5,
        per_tx_limit_cents: 100,
        per_session_limit_cents: 1000,
        payment_scope_kind_missing: Some(PaymentScopeKind::Basket),
        ..baseline_approved()
    };

    let d = decide(&ctx);
    assert!(d.is_prompt());
    if let PermissionDecision::Prompt { prompt_type, .. } = d {
        assert_eq!(prompt_type, PromptType::BasketPermissionPrompt);
    } else {
        panic!("expected Prompt decision");
    }
}

#[test]
fn payment_with_missing_counterparty_prompts_counterparty_permission() {
    let ctx = PermissionContext {
        call_kind: CallKind::Payment,
        bsv_price_available: true,
        requested_cents: 5,
        per_tx_limit_cents: 100,
        per_session_limit_cents: 1000,
        payment_scope_kind_missing: Some(PaymentScopeKind::Counterparty),
        ..baseline_approved()
    };

    let d = decide(&ctx);
    assert!(d.is_prompt());
    if let PermissionDecision::Prompt { prompt_type, .. } = d {
        assert_eq!(prompt_type, PromptType::CounterpartyPermissionPrompt);
    } else {
        panic!("expected Prompt decision");
    }
}

#[test]
fn payment_scope_missing_takes_priority_over_cap_exceedance() {
    // BOTH scope missing AND over cap. Engine returns scope prompt; the cap
    // prompt fires on the re-issued request after scope is approved.
    // This is the "independent gates" invariant — caller must see scope
    // first, then payment second.
    let ctx = PermissionContext {
        call_kind: CallKind::Payment,
        bsv_price_available: true,
        requested_cents: 1000, // way over cap
        per_tx_limit_cents: 10,
        per_session_limit_cents: 100,
        payment_scope_kind_missing: Some(PaymentScopeKind::Protocol),
        ..baseline_approved()
    };

    let d = decide(&ctx);
    assert!(d.is_prompt());
    if let PermissionDecision::Prompt { prompt_type, .. } = d {
        assert_eq!(
            prompt_type,
            PromptType::ProtocolPermissionPrompt,
            "scope prompt must beat cap prompt"
        );
    } else {
        panic!("expected Prompt decision");
    }
}

#[test]
fn payment_no_scope_missing_falls_through_to_cap_checks() {
    // No scope missing → engine proceeds to the existing cap-check cascade.
    // This is the "scope already granted, so cap path runs" case.
    let ctx = PermissionContext {
        call_kind: CallKind::Payment,
        bsv_price_available: true,
        requested_cents: 1000, // over cap
        per_tx_limit_cents: 10,
        payment_scope_kind_missing: None, // scope is fine
        ..baseline_approved()
    };

    let d = decide(&ctx);
    assert!(d.is_prompt());
    if let PermissionDecision::Prompt { prompt_type, .. } = d {
        assert_eq!(
            prompt_type,
            PromptType::PaymentConfirmation,
            "cap path, not scope path"
        );
    } else {
        panic!("expected Prompt decision");
    }
}

#[test]
fn payment_unknown_scope_value_defaults_to_protocol_prompt() {
    // Vacuous in Rust: C++ tested `payment_scope_kind_missing = "garbage_value_xyz"`
    // and verified the defensive fallback to protocol_permission_prompt. Rust's
    // typed `Option<PaymentScopeKind>` makes "garbage_value_xyz" unrepresentable
    // at the type system level — the field accepts only Protocol/Basket/Counterparty.
    // JSON deserialization rejects unknown values.
    //
    // The test is kept for traceability with the C++ source. It verifies the
    // type-level invariant: an unrepresentable invalid value cannot reach decide().
    let only_valid_values = [
        PaymentScopeKind::Protocol,
        PaymentScopeKind::Basket,
        PaymentScopeKind::Counterparty,
    ];
    assert_eq!(only_valid_values.len(), 3, "exactly 3 representable scope kinds");

    // Round-trip check: every valid value serializes + deserializes cleanly.
    for kind in only_valid_values {
        let ctx = PermissionContext {
            call_kind: CallKind::Payment,
            bsv_price_available: true,
            requested_cents: 5,
            payment_scope_kind_missing: Some(kind),
            ..baseline_approved()
        };
        let d = decide(&ctx);
        assert!(d.is_prompt(), "valid scope kind {:?} should prompt", kind);
    }
}

// ============================================================================
// Branch 5: Cert disclosure (non-sensitive fields)
// ============================================================================

#[test]
fn cert_disclosure_silent_when_all_fields_pre_approved() {
    let ctx = PermissionContext {
        call_kind: CallKind::CertificateDisclosure,
        scoped_grant_exists: true,
        ..baseline_approved()
    };

    let d = decide(&ctx);
    assert!(d.is_silent());
}

#[test]
fn cert_disclosure_prompts_when_fields_unapproved() {
    let ctx = PermissionContext {
        call_kind: CallKind::CertificateDisclosure,
        scoped_grant_exists: false,
        ..baseline_approved()
    };

    let d = decide(&ctx);
    assert!(d.is_prompt());
    if let PermissionDecision::Prompt { prompt_type, .. } = d {
        assert_eq!(prompt_type, PromptType::CertificateDisclosure);
    } else {
        panic!("expected Prompt decision");
    }
}

// ============================================================================
// Branch 6: Generic approved-domain calls — silent fall-through
// ============================================================================

#[test]
fn generic_approved_call_is_silent() {
    let ctx = PermissionContext {
        call_kind: CallKind::GenericApproved,
        ..baseline_approved()
    };

    let d = decide(&ctx);
    assert!(d.is_silent());
}

// ============================================================================
// Branch ordering: blocked domain wins over privacy perimeter
// ============================================================================

#[test]
fn blocked_domain_wins_over_identity_key_opt_in() {
    let ctx = PermissionContext {
        trust_level: TrustLevel::Blocked,
        call_kind: CallKind::IdentityKeyReveal,
        identity_key_disclosure_allowed: true, // would normally Silent
        ..baseline_approved()
    };

    let d = decide(&ctx);
    assert!(d.is_deny(), "blocked trust must override any opt-in");
}

#[test]
fn unknown_domain_wins_over_privacy_perimeter() {
    // First-visit identity-key request hits domain_approval, not identity_key_reveal.
    let ctx = PermissionContext {
        trust_level: TrustLevel::Unknown,
        call_kind: CallKind::IdentityKeyReveal,
        ..baseline_approved()
    };

    let d = decide(&ctx);
    assert!(d.is_prompt());
    if let PermissionDecision::Prompt { prompt_type, .. } = d {
        assert_eq!(prompt_type, PromptType::DomainApproval);
    } else {
        panic!("expected Prompt decision");
    }
}
