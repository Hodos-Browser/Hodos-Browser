//! Engine output types.
//!
//! Mirrors the C++ `PermissionDecision` at `cef-native/include/core/PermissionEngine.h:109-129`.
//! The decision is consumed by `permission_service` which translates it to the
//! 200 OK / 202 PENDING / 403 FORBIDDEN HTTP response per LD2 wire contract.

use serde::{Deserialize, Serialize};

/// The engine's decision — one of three terminal states.
///
/// C++ uses a struct with `kind` + `promptType` + `reason` strings; Rust uses
/// a sum type so the type system enforces that `prompt_type` only exists on
/// `Prompt` decisions (the C++ shape allows nonsensical `Silent` with non-empty
/// `promptType`, which the Rust port makes unrepresentable).
///
/// Serialization shape (per LD2 with `serde(tag = "kind")`):
///   `{ "kind": "silent", "reason": "silent_within_caps" }`
///   `{ "kind": "prompt", "prompt_type": "payment_confirmation", "reason": "per_tx_limit" }`
///   `{ "kind": "deny",   "reason": "trust_blocked" }`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum PermissionDecision {
    /// Forward to wallet immediately without a modal.
    Silent { reason: EngineReason },
    /// Fire a modal; wait for user approval before forwarding.
    Prompt {
        prompt_type: PromptType,
        reason: EngineReason,
    },
    /// Refuse the request outright.
    Deny { reason: EngineReason },
}

impl PermissionDecision {
    /// Convenience for the common Silent-with-default-reason case.
    pub fn silent(reason: EngineReason) -> Self {
        Self::Silent { reason }
    }

    /// Convenience for building a Prompt decision.
    pub fn prompt(prompt_type: PromptType, reason: EngineReason) -> Self {
        Self::Prompt { prompt_type, reason }
    }

    /// Convenience for building a Deny decision.
    pub fn deny(reason: EngineReason) -> Self {
        Self::Deny { reason }
    }

    /// True iff this decision will produce a 200 OK (no modal, no error).
    pub fn is_silent(&self) -> bool {
        matches!(self, Self::Silent { .. })
    }

    /// True iff this decision will produce a 202 PENDING (modal needed).
    pub fn is_prompt(&self) -> bool {
        matches!(self, Self::Prompt { .. })
    }

    /// True iff this decision will produce a 403 FORBIDDEN.
    pub fn is_deny(&self) -> bool {
        matches!(self, Self::Deny { .. })
    }
}

/// Which modal type to fire on a Prompt decision.
///
/// Drives the React modal dispatch in `BRC100AuthOverlayRoot.tsx`'s
/// type-dispatch (the same mechanism the C++ engine uses today). C++
/// uses raw strings; Rust port uses a typed enum that serializes to
/// the same snake_case strings React already consumes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PromptType {
    /// First-touch from an unknown domain (no manifest).
    DomainApproval,
    /// First-touch from an unknown domain WITH a `.well-known/wallet-manifest.json`.
    /// Bundles all manifest-declared permissions into one approval modal
    /// (Phase 1.5 Step 5).
    ManifestConnectBundle,
    /// Legacy BRC-100 auth modal (kept for compatibility with older flows).
    Brc100Auth,
    /// Spend cap / session cap / max-tx-per-session over the limit.
    PaymentConfirmation,
    /// Rate limit exceeded — variant of payment_confirmation with rate-limit context.
    RateLimitExceeded,
    /// Identity-key reveal — the privacy-perimeter "who is this user" prompt.
    IdentityKeyReveal,
    /// BRC-72 counterparty / specific key linkage reveal.
    KeyLinkageReveal,
    /// Certificate field disclosure (per-field per-domain).
    CertificateDisclosure,
    /// Scoped-grant prompt: protocolID + keyID + counterparty (V18 child table).
    ProtocolPermissionPrompt,
    /// Scoped-grant prompt: basket access (V18 child table).
    BasketPermissionPrompt,
    /// Scoped-grant prompt: counterparty (V18 child table).
    CounterpartyPermissionPrompt,
}

/// Typed reason vocabulary for engine decisions.
///
/// Used for three purposes:
///   1. Engine reasoning visible in `permission_audit_log.engine_reason` column
///   2. Wire-contract `engineReason` field in 202 PENDING / 403 FORBIDDEN bodies (LD2)
///   3. Shadow-mode comparison key for `engine_shadow_log.cpp_reason` / `.rust_reason`
///
/// Per LD2: list grows as branches land. Initial vocabulary covers Matrix C
/// branches 1-6 (privacy perimeter, domain trust, scoped grant, payment, cert
/// disclosure, generic approved). 2.6-A.3 may add variants during the port if
/// the C++ engine emits reasons not yet listed here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EngineReason {
    // ── Silent decisions ──
    /// Payment within all caps OR generic approved-domain call with no extra gate.
    SilentWithinCaps,
    /// Identity-key reveal allowed via V17 column (persistent opt-in).
    SilentIdentityKeyDisclosureAllowed,
    /// Identity-key or key-linkage reveal allowed via session-scoped opt-in cache.
    SilentSessionOptIn,
    /// Scoped grant exists for the requested protocol/basket/counterparty.
    SilentScopedGrantExists,
    /// CounterpartyUse on an approved domain is silent by design — BRC-42
    /// counterparty key derivation is mathematically one-sided (the
    /// counterparty learns nothing) and only reveals what the requesting
    /// dApp already knows (it provided the counterparty pubkey). Avoids the
    /// "prompt-per-recipient" UX collapse seen with token-issuing dApps like
    /// todo.metanet.app. Phase 2.6-D Fix #3 (2026-06-09).
    SilentCounterpartyDefault,
    /// Bundle-grant on first connect covers this scoped call. The user
    /// approved an "allow this site to perform wallet operations without
    /// prompting each time" checkbox on the connect modal, which sets
    /// domain_permissions.bundled_scope_grant=1. Protected baskets still
    /// prompt regardless (dispatch overrides). Phase 2.6-D Fix #4.
    SilentBundledScopeGrant,
    /// Cert disclosure: every requested field has a matching permission row.
    SilentAllCertFieldsApproved,
    /// Generic approved-domain call with no extra gate. The catch-all silent
    /// case at the end of the Matrix C cascade. Used when call_kind is
    /// DomainTrust or GenericApproved on an approved-trust domain.
    SilentGenericApproved,

    // ── Prompt reasons ──
    /// Payment exceeds per_tx_limit_cents.
    PerTxLimit,
    /// Payment would push session_spent_cents over per_session_limit_cents.
    SessionCap,
    /// Payment count this minute exceeds rate_limit_per_min.
    RateLimit,
    /// Payment count this session exceeds max_tx_per_session.
    MaxTxPerSession,
    /// BSV price unavailable — engine can't verify USD cost; prompt for manual review.
    PriceUnavailable,
    /// Privacy-perimeter call with no persistent or session-scoped opt-in.
    PrivacyPerimeterNoGrant,
    /// Scoped-grant missing for protocol/basket/counterparty call.
    ScopedGrantMissing,
    /// Payment body references a protocol scope the site has no grant for.
    PaymentScopeProtocolMissing,
    /// Payment body references a basket scope the site has no grant for.
    PaymentScopeBasketMissing,
    /// Payment body references a counterparty scope the site has no grant for.
    PaymentScopeCounterpartyMissing,
    /// Protected basket (`default`, `backup-*`, `admin *`) — never auto-grant.
    ProtectedBasket,
    /// One or more cert fields lack permission rows.
    CertFieldUnapproved,
    /// First-touch from unknown domain; no manifest fetched.
    NewDomainNoManifest,
    /// First-touch from unknown domain; manifest fetched — bundle prompt.
    NewDomainWithManifest,
    /// Sensitive cert field — always prompt regardless of stored permissions.
    SensitiveCertField,

    // ── Deny reasons ──
    /// `trust_level = 'blocked'` on `domain_permissions`.
    TrustBlocked,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn silent_decision_round_trips() {
        let d = PermissionDecision::silent(EngineReason::SilentWithinCaps);
        let json = serde_json::to_string(&d).unwrap();
        assert!(json.contains("\"kind\":\"silent\""));
        assert!(json.contains("\"reason\":\"silent_within_caps\""));
        let back: PermissionDecision = serde_json::from_str(&json).unwrap();
        assert_eq!(back, d);
    }

    #[test]
    fn prompt_decision_round_trips() {
        let d = PermissionDecision::prompt(
            PromptType::PaymentConfirmation,
            EngineReason::PerTxLimit,
        );
        let json = serde_json::to_string(&d).unwrap();
        assert!(json.contains("\"kind\":\"prompt\""));
        assert!(json.contains("\"prompt_type\":\"payment_confirmation\""));
        assert!(json.contains("\"reason\":\"per_tx_limit\""));
        let back: PermissionDecision = serde_json::from_str(&json).unwrap();
        assert_eq!(back, d);
    }

    #[test]
    fn deny_decision_round_trips() {
        let d = PermissionDecision::deny(EngineReason::TrustBlocked);
        let json = serde_json::to_string(&d).unwrap();
        assert!(json.contains("\"kind\":\"deny\""));
        assert!(json.contains("\"reason\":\"trust_blocked\""));
        let back: PermissionDecision = serde_json::from_str(&json).unwrap();
        assert_eq!(back, d);
    }

    #[test]
    fn predicate_helpers_match_variant() {
        let silent = PermissionDecision::silent(EngineReason::SilentWithinCaps);
        assert!(silent.is_silent());
        assert!(!silent.is_prompt());
        assert!(!silent.is_deny());

        let prompt = PermissionDecision::prompt(PromptType::DomainApproval, EngineReason::NewDomainNoManifest);
        assert!(!prompt.is_silent());
        assert!(prompt.is_prompt());
        assert!(!prompt.is_deny());

        let deny = PermissionDecision::deny(EngineReason::TrustBlocked);
        assert!(!deny.is_silent());
        assert!(!deny.is_prompt());
        assert!(deny.is_deny());
    }

    #[test]
    fn all_prompt_types_serialize_to_expected_strings() {
        // Verifies the snake_case mapping matches what React's BRC100AuthOverlayRoot.tsx
        // dispatches on. If a prompt_type string changes here, the React layer breaks.
        let cases = [
            (PromptType::DomainApproval, "domain_approval"),
            (PromptType::ManifestConnectBundle, "manifest_connect_bundle"),
            (PromptType::Brc100Auth, "brc100_auth"),
            (PromptType::PaymentConfirmation, "payment_confirmation"),
            (PromptType::RateLimitExceeded, "rate_limit_exceeded"),
            (PromptType::IdentityKeyReveal, "identity_key_reveal"),
            (PromptType::KeyLinkageReveal, "key_linkage_reveal"),
            (PromptType::CertificateDisclosure, "certificate_disclosure"),
            (PromptType::ProtocolPermissionPrompt, "protocol_permission_prompt"),
            (PromptType::BasketPermissionPrompt, "basket_permission_prompt"),
            (PromptType::CounterpartyPermissionPrompt, "counterparty_permission_prompt"),
        ];
        for (kind, expected_str) in cases {
            let json = serde_json::to_string(&kind).unwrap();
            assert_eq!(json, format!("\"{}\"", expected_str), "PromptType::{:?} should serialize to \"{}\"", kind, expected_str);
        }
    }

    #[test]
    fn ld2_initial_engine_reason_vocabulary_serializes() {
        // The 11 reasons explicitly named in the Phase 2.6 plan doc LD2.
        // If any of these strings change, the audit log + wire contract drift.
        let ld2_initial = [
            (EngineReason::SilentWithinCaps, "silent_within_caps"),
            (EngineReason::PerTxLimit, "per_tx_limit"),
            (EngineReason::SessionCap, "session_cap"),
            (EngineReason::RateLimit, "rate_limit"),
            (EngineReason::PriceUnavailable, "price_unavailable"),
            (EngineReason::MaxTxPerSession, "max_tx_per_session"),
            (EngineReason::ProtectedBasket, "protected_basket"),
            (EngineReason::TrustBlocked, "trust_blocked"),
            (EngineReason::PrivacyPerimeterNoGrant, "privacy_perimeter_no_grant"),
        ];
        for (reason, expected) in ld2_initial {
            let json = serde_json::to_string(&reason).unwrap();
            assert_eq!(json, format!("\"{}\"", expected));
        }
    }
}
