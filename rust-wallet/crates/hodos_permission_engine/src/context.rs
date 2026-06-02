//! Engine input types.
//!
//! Mirrors the C++ `PermissionContext` at `cef-native/include/core/PermissionEngine.h:61-107`.
//! Plain data — trivially mockable in tests. No actix/sqlite/http imports.
//!
//! Caller (permission_service::context_builder) assembles this from the live
//! DB row, session counters, BSV price cache, and request-specific scope.

use serde::{Deserialize, Serialize};

/// The kind of BRC-100 / wallet call being gated. Engine uses this to classify
/// the request into one of the Matrix C branches before consulting
/// scope-specific data.
///
/// 1:1 with C++ `PermissionCallKind` enum (PermissionEngine.h:32-55).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum CallKind {
    // Privacy perimeter — always-prompt unless persistently opted in.
    /// getPublicKey({identityKey:true}) from external domain.
    IdentityKeyReveal,
    /// /revealCounterpartyKeyLinkage.
    CounterpartyKeyLinkage,
    /// /revealSpecificKeyLinkage.
    SpecificKeyLinkage,
    /// proveCertificate touching a high-sensitivity field.
    SensitiveCertField,

    // Scoped grants (V18 child tables).
    /// Any call carrying protocolID + keyID.
    ProtocolUse,
    /// listOutputs / relinquishOutput with a basket.
    BasketAccess,
    /// Level-2 protocols with a specific counterparty.
    CounterpartyUse,

    // Payment / spending.
    /// createAction / acquireCertificate / sendMessage.
    Payment,

    // Domain trust.
    /// First BRC-100 hit from a fresh origin.
    DomainTrust,

    // Cert disclosure (non-sensitive fields).
    CertificateDisclosure,

    // Catch-all for approved-domain calls with no extra gate.
    GenericApproved,
}

impl Default for CallKind {
    fn default() -> Self {
        // Matches the C++ default at PermissionEngine.h:62.
        Self::GenericApproved
    }
}

/// Coarse domain trust tier from the `domain_permissions` row.
///
/// C++ uses a raw `std::string` here ("unknown" | "approved" | "blocked");
/// Rust uses a typed enum for compile-time exhaustiveness in match arms.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TrustLevel {
    Unknown,
    Approved,
    Blocked,
}

impl Default for TrustLevel {
    fn default() -> Self {
        // Empty trust level in C++ is treated as Unknown (see test
        // EmptyTrustLevelTreatedAsUnknown in permission_engine_test.cpp:73).
        Self::Unknown
    }
}

/// Which sub-permission scope is missing on a Payment call.
///
/// C++ uses `std::string` with values "" | "protocol" | "basket" | "counterparty"
/// (PermissionEngine.h:97-106). The Rust port uses `Option<PaymentScopeKind>`
/// where `None` matches the empty-string case.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PaymentScopeKind {
    Protocol,
    Basket,
    Counterparty,
}

/// Input to `decide()`. Caller assembles this from the live cache row,
/// SessionManager-equivalent counters, BSV price cache, and request-specific
/// scope. Plain data — no actix dependency, trivially mockable.
///
/// 1:1 with C++ `PermissionContext` struct (PermissionEngine.h:61-107).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionContext {
    pub call_kind: CallKind,

    // Domain-level state (from DomainPermissionCache::Permission in C++).
    pub trust_level: TrustLevel,
    pub per_tx_limit_cents: i64,
    pub per_session_limit_cents: i64,
    pub rate_limit_per_min: i64,
    pub max_tx_per_session: i64,
    /// V17 column on `domain_permissions`. When true and call_kind is
    /// IdentityKeyReveal, engine returns Silent without requiring a session opt-in.
    pub identity_key_disclosure_allowed: bool,

    // Session counters (migrate from C++ SessionManager to Rust during 2.6-E).
    pub session_spent_cents: i64,
    pub payment_requests_this_minute: i32,
    pub payment_count_this_session: i32,

    // Privacy-perimeter session opt-ins (transient — in-memory only, cleared on tab close).
    pub identity_key_session_opt_in: bool,
    pub key_linkage_session_opt_in: bool,

    // Request-specific cost (computed by caller for Payment kind).
    pub requested_cents: i64,
    /// True when BSV/USD price cache returned a usable price for the current
    /// request. Payment kind: when false, engine prompts payment_confirmation
    /// with reason=PriceUnavailable so the user can review the satoshi amount
    /// manually instead of silently forwarding a tx whose USD cost is unverified.
    /// Defaults to true so non-payment contexts (which never set this) preserve
    /// existing semantics. Matches C++ default at PermissionEngine.h:90.
    pub bsv_price_available: bool,

    // Scoped-grant evaluation (filled in by caller for ProtocolUse/BasketAccess/CounterpartyUse).
    pub scoped_grant_exists: bool,

    /// Phase 1.5 Step 6 Commit E: for Payment kind, if the createAction body
    /// also references a protocol/basket/counterparty scope the site does NOT
    /// have a grant for, caller sets this field. DecidePayment short-circuits
    /// on this BEFORE the cap checks, returning the matching scope-permission
    /// prompt. After user approves the scope, request is re-issued and this
    /// field comes back `None`, so the cap checks then run.
    pub payment_scope_kind_missing: Option<PaymentScopeKind>,
}

impl Default for PermissionContext {
    fn default() -> Self {
        Self {
            call_kind: CallKind::default(),
            trust_level: TrustLevel::default(),
            per_tx_limit_cents: 0,
            per_session_limit_cents: 0,
            rate_limit_per_min: 0,
            max_tx_per_session: 0,
            identity_key_disclosure_allowed: false,
            session_spent_cents: 0,
            payment_requests_this_minute: 0,
            payment_count_this_session: 0,
            identity_key_session_opt_in: false,
            key_linkage_session_opt_in: false,
            requested_cents: 0,
            // C++ default at PermissionEngine.h:90 — must stay true to preserve
            // existing semantics for non-payment contexts that don't set this.
            bsv_price_available: true,
            scoped_grant_exists: false,
            payment_scope_kind_missing: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_context_matches_cpp_defaults() {
        let ctx = PermissionContext::default();
        assert_eq!(ctx.call_kind, CallKind::GenericApproved);
        assert_eq!(ctx.trust_level, TrustLevel::Unknown);
        assert!(ctx.bsv_price_available, "bsv_price_available defaults to true per C++ semantics");
        assert!(!ctx.identity_key_disclosure_allowed);
        assert_eq!(ctx.payment_scope_kind_missing, None);
    }

    #[test]
    fn call_kind_serializes_as_pascal_case() {
        let json = serde_json::to_string(&CallKind::IdentityKeyReveal).unwrap();
        assert_eq!(json, "\"IdentityKeyReveal\"");
        let back: CallKind = serde_json::from_str(&json).unwrap();
        assert_eq!(back, CallKind::IdentityKeyReveal);
    }

    #[test]
    fn trust_level_serializes_as_lowercase() {
        assert_eq!(serde_json::to_string(&TrustLevel::Approved).unwrap(), "\"approved\"");
        assert_eq!(serde_json::to_string(&TrustLevel::Blocked).unwrap(), "\"blocked\"");
        assert_eq!(serde_json::to_string(&TrustLevel::Unknown).unwrap(), "\"unknown\"");
    }

    #[test]
    fn payment_scope_kind_round_trips() {
        for kind in [PaymentScopeKind::Protocol, PaymentScopeKind::Basket, PaymentScopeKind::Counterparty] {
            let json = serde_json::to_string(&kind).unwrap();
            let back: PaymentScopeKind = serde_json::from_str(&json).unwrap();
            assert_eq!(back, kind);
        }
    }

    #[test]
    fn context_round_trips_through_json() {
        let ctx = PermissionContext {
            call_kind: CallKind::Payment,
            trust_level: TrustLevel::Approved,
            per_tx_limit_cents: 100,
            session_spent_cents: 50,
            requested_cents: 25,
            bsv_price_available: true,
            ..Default::default()
        };
        let json = serde_json::to_string(&ctx).unwrap();
        let back: PermissionContext = serde_json::from_str(&json).unwrap();
        assert_eq!(back.call_kind, CallKind::Payment);
        assert_eq!(back.per_tx_limit_cents, 100);
        assert_eq!(back.session_spent_cents, 50);
        assert_eq!(back.requested_cents, 25);
    }
}
