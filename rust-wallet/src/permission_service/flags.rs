//! `EngineFlags` — process-wide engine config.
//!
//! Phase 2.6-A through 2.6-B carried a 5-flag-per-CallKind-class struct here
//! (`HODOS_ENGINE_RUST_<CLASS>` env vars). Phase 2.6-C kickoff dropped that
//! migration model: each class becomes Rust-authoritative the instant its
//! sub-commit lands, with no env-var fallback path. See memory
//! `phase26-2-6-c-kickoff-2026-06-03` Q1.
//!
//! What survives in this struct:
//!   - `shadow_log_enabled` — diagnostic only (Phase 2.6-B). When true,
//!     `/engine/shadow-decide` writes rows to `engine_shadow_log`. Independent
//!     of any class flag; turning it on does NOT make any Rust engine path
//!     authoritative. Env var: `HODOS_ENGINE_SHADOW_LOG`.
//!
//! What was removed in 2.6-C.2 cleanup:
//!   - `privacy_perimeter`, `scoped_grant`, `payment`, `cert_disclosure`,
//!     `domain_trust` — the 5 per-class booleans and their env-var readers.
//!   - `any_enabled()`, `is_enabled_for(CallKind)` — gating accessors.
//!   - `HODOS_ENGINE_RUST_*` env vars.
//!
//! The CallKind → flag-class string mapping survives as a free function so
//! `permission_service::audit::build_shadow_entry` keeps a stable
//! `call_kind_class` column value during the shadow-log lifetime.
//!
//! See: PHASE_2_6_ENGINE_TO_RUST.md §LD3 (now superseded by kickoff Q1).

use hodos_permission_engine::CallKind;

#[derive(Debug, Clone, Copy)]
pub struct EngineFlags {
    /// Phase 2.6-B: when true, `/engine/shadow-decide` accepts comparison POSTs
    /// from the C++ engine and writes rows to `engine_shadow_log`. When false,
    /// the handler short-circuits with 204 No Content so even a misconfigured
    /// C++ client with shadow ON can't pollute the table. Env var:
    /// `HODOS_ENGINE_SHADOW_LOG`.
    pub shadow_log_enabled: bool,
}

impl Default for EngineFlags {
    /// Shadow OFF — diagnostic infrastructure stays opt-in.
    fn default() -> Self {
        Self {
            shadow_log_enabled: false,
        }
    }
}

impl EngineFlags {
    /// Read flags from environment variables at process start.
    pub fn from_env() -> Self {
        Self {
            shadow_log_enabled: read_bool_env("HODOS_ENGINE_SHADOW_LOG"),
        }
    }

    /// CallKind → flag class name (kebab-case for audit/shadow logs). The
    /// class taxonomy survives the per-class flag removal because the shadow
    /// log table still groups disagreements by class for diagnostics.
    pub fn class_name_for(kind: CallKind) -> &'static str {
        match Self::class_for(kind) {
            FlagClass::PrivacyPerimeter => "privacy_perimeter",
            FlagClass::ScopedGrant => "scoped_grant",
            FlagClass::Payment => "payment",
            FlagClass::CertDisclosure => "cert_disclosure",
            FlagClass::DomainTrust => "domain_trust",
        }
    }

    /// CallKind → flag class mapping. Single source of truth.
    fn class_for(kind: CallKind) -> FlagClass {
        match kind {
            CallKind::IdentityKeyReveal
            | CallKind::CounterpartyKeyLinkage
            | CallKind::SpecificKeyLinkage
            | CallKind::SensitiveCertField => FlagClass::PrivacyPerimeter,
            CallKind::ProtocolUse | CallKind::BasketAccess | CallKind::CounterpartyUse => {
                FlagClass::ScopedGrant
            }
            CallKind::Payment => FlagClass::Payment,
            CallKind::CertificateDisclosure => FlagClass::CertDisclosure,
            CallKind::DomainTrust | CallKind::GenericApproved => FlagClass::DomainTrust,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FlagClass {
    PrivacyPerimeter,
    ScopedGrant,
    Payment,
    CertDisclosure,
    DomainTrust,
}

/// Read a boolean env var. "1" or "true" (case-insensitive) → true; anything
/// else (including unset, empty, or unrelated value) → false.
fn read_bool_env(name: &str) -> bool {
    match std::env::var(name) {
        Ok(v) => matches!(v.to_lowercase().as_str(), "1" | "true"),
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_has_shadow_off() {
        let f = EngineFlags::default();
        assert!(!f.shadow_log_enabled);
    }

    #[test]
    fn class_mapping_covers_all_call_kinds_correctly() {
        // Privacy perimeter (4 kinds)
        assert_eq!(EngineFlags::class_name_for(CallKind::IdentityKeyReveal), "privacy_perimeter");
        assert_eq!(EngineFlags::class_name_for(CallKind::CounterpartyKeyLinkage), "privacy_perimeter");
        assert_eq!(EngineFlags::class_name_for(CallKind::SpecificKeyLinkage), "privacy_perimeter");
        assert_eq!(EngineFlags::class_name_for(CallKind::SensitiveCertField), "privacy_perimeter");

        // Scoped grant (3 kinds)
        assert_eq!(EngineFlags::class_name_for(CallKind::ProtocolUse), "scoped_grant");
        assert_eq!(EngineFlags::class_name_for(CallKind::BasketAccess), "scoped_grant");
        assert_eq!(EngineFlags::class_name_for(CallKind::CounterpartyUse), "scoped_grant");

        // Payment (1 kind)
        assert_eq!(EngineFlags::class_name_for(CallKind::Payment), "payment");

        // Cert disclosure (1 kind — sensitive cert lives under privacy_perimeter)
        assert_eq!(EngineFlags::class_name_for(CallKind::CertificateDisclosure), "cert_disclosure");

        // Domain trust (2 kinds)
        assert_eq!(EngineFlags::class_name_for(CallKind::DomainTrust), "domain_trust");
        assert_eq!(EngineFlags::class_name_for(CallKind::GenericApproved), "domain_trust");
    }
}
