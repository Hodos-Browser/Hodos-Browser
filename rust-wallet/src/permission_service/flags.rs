//! `EngineFlags` — per-CallKind-class feature flags for the Phase 2.6 migration.
//!
//! Five flags, one per CallKind class. Each flag controls whether that class's
//! wallet calls flow through the Rust engine (flag ON) or stay on the C++
//! engine (flag OFF). All flags default OFF in every commit until sub-phase
//! 2.6-H final cleanup deletes the flag system entirely along with the C++
//! engine.
//!
//! No production rollout model — Hodos ships as a desktop installer; there's
//! no "10% of users on the new path." Flags are dev-time testing scaffolding
//! only.
//!
//! Env var naming: `HODOS_ENGINE_RUST_<CLASS>` matching the dev runbook
//! `HODOS_DEV=1` precedent. Values "1" or "true" (case-insensitive) enable;
//! anything else (including unset) disables.
//!
//! See: PHASE_2_6_ENGINE_TO_RUST.md §LD3.

use hodos_permission_engine::CallKind;

#[derive(Debug, Clone, Copy)]
pub struct EngineFlags {
    pub privacy_perimeter: bool,
    pub scoped_grant: bool,
    pub payment: bool,
    pub cert_disclosure: bool,
    pub domain_trust: bool,
}

impl Default for EngineFlags {
    /// All flags OFF — production-safe default per LD3.
    fn default() -> Self {
        Self {
            privacy_perimeter: false,
            scoped_grant: false,
            payment: false,
            cert_disclosure: false,
            domain_trust: false,
        }
    }
}

impl EngineFlags {
    /// Read flags from environment variables at process start.
    ///
    /// All default to false unless the matching env var is "1" or "true"
    /// (case-insensitive).
    pub fn from_env() -> Self {
        Self {
            privacy_perimeter: read_bool_env("HODOS_ENGINE_RUST_PRIVACY_PERIMETER"),
            scoped_grant: read_bool_env("HODOS_ENGINE_RUST_SCOPED_GRANT"),
            payment: read_bool_env("HODOS_ENGINE_RUST_PAYMENT"),
            cert_disclosure: read_bool_env("HODOS_ENGINE_RUST_CERT_DISCLOSURE"),
            domain_trust: read_bool_env("HODOS_ENGINE_RUST_DOMAIN_TRUST"),
        }
    }

    /// Returns true iff any flag is ON. Useful for the wallet startup log
    /// to surface that some Rust engine paths are live.
    pub fn any_enabled(&self) -> bool {
        self.privacy_perimeter
            || self.scoped_grant
            || self.payment
            || self.cert_disclosure
            || self.domain_trust
    }

    /// Returns true iff the flag for the CallKind's class is ON.
    ///
    /// This is the per-request gate: at each wallet endpoint that the engine
    /// owns, the handler calls `flags.is_enabled_for(call_kind)` and chooses
    /// between the C++ engine path (false) and the Rust engine path (true).
    pub fn is_enabled_for(&self, kind: CallKind) -> bool {
        match Self::class_for(kind) {
            FlagClass::PrivacyPerimeter => self.privacy_perimeter,
            FlagClass::ScopedGrant => self.scoped_grant,
            FlagClass::Payment => self.payment,
            FlagClass::CertDisclosure => self.cert_disclosure,
            FlagClass::DomainTrust => self.domain_trust,
        }
    }

    /// Map a CallKind to its flag class name (kebab-case for audit/shadow logs).
    pub fn class_name_for(kind: CallKind) -> &'static str {
        match Self::class_for(kind) {
            FlagClass::PrivacyPerimeter => "privacy_perimeter",
            FlagClass::ScopedGrant => "scoped_grant",
            FlagClass::Payment => "payment",
            FlagClass::CertDisclosure => "cert_disclosure",
            FlagClass::DomainTrust => "domain_trust",
        }
    }

    /// CallKind → flag class mapping. Single source of truth — referenced by
    /// both is_enabled_for and class_name_for so the two cannot drift.
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
    fn default_is_all_off() {
        let f = EngineFlags::default();
        assert!(!f.privacy_perimeter);
        assert!(!f.scoped_grant);
        assert!(!f.payment);
        assert!(!f.cert_disclosure);
        assert!(!f.domain_trust);
        assert!(!f.any_enabled());
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

    #[test]
    fn is_enabled_for_respects_class_mapping() {
        let mut f = EngineFlags::default();
        f.payment = true;

        assert!(f.is_enabled_for(CallKind::Payment));
        assert!(!f.is_enabled_for(CallKind::IdentityKeyReveal));
        assert!(!f.is_enabled_for(CallKind::ProtocolUse));
        assert!(f.any_enabled());
    }
}
