//! `CallClass` — outbound HTTP timeout policy enum.
//!
//! Single source of truth for `reqwest::Client::builder().timeout(...)` values
//! across the wallet. Tag every outbound call with the class that matches its
//! failure semantics; tune the class's timeout in one place if a class proves
//! systematically too tight (or too loose).
//!
//! ## Classes
//!
//! | Class | Budget | When to use |
//! |---|---|---|
//! | `IndexerSync` | 8s | Indexer call in front of a UI-blocking request; has a Services-chain fallback. |
//! | `IndexerAsync` | 15s | Background indexer call (Monitor task, cache refresh); has a Services-chain fallback. |
//! | `IndexerBulk` | 30s | Indexer call returning a big payload (BEEF ancestry, gap-limit sweep); has a Services-chain fallback. |
//! | `ThirdPartyNoFallback` | 240s | Third-party service (cert, paymail, MessageBox, overlay) with no alternate provider. Single attempt. |
//!
//! ## Why 240s for `ThirdPartyNoFallback`
//!
//! - 8s/15s budgets assume a provider chain absorbs one tier's failure. Third
//!   parties have no chain — a tight timeout just fails the user.
//! - The CEF outer cap for cert endpoints is 300s
//!   (see `cef-native/src/core/HttpRequestInterceptor.cpp::postHttpTimeout`).
//!   240s wallet inner + 300s CEF outer = 60s buffer — the wallet's structured
//!   response always wins over CEF's outer fallback.
//! - History: 8s (1.6d.A regression) → 60s → 90s → 120s → 240s. Each bump came
//!   after live observation of SocialCert response times exceeding the prior
//!   ceiling. 2026-05-27 testing showed `/signCertificate` regularly taking
//!   100-120s and occasionally exceeding 120s, causing first-attempt timeouts
//!   that then required a SocialCert frontend auto-retry. Bumping to 240s
//!   should let the first attempt complete normally even on degraded backend
//!   days.
//! - This is a class-level setting — it applies to all third-party-no-fallback
//!   sites (paymail, MessageBox/AuthFetch, overlay SHIP/lookup/submit, cert
//!   handlers). Intentional: site-specific overrides do not belong in the
//!   wallet. If a single host needs different semantics, redesign the class
//!   matrix rather than special-casing a hostname.
//! - Per-host overlay submit now parallelizes (see `src/overlay/mod.rs::submit_to_topic`),
//!   so a single slow overlay host hanging for the full 240s does not block
//!   the user — it just keeps running in the background drain task.

use std::time::Duration;

/// Outbound HTTP timeout policy class. See module docs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallClass {
    /// 8s — indexer call, UI is waiting, has chain fallback.
    IndexerSync,
    /// 15s — indexer call, background work (Monitor, cache), has chain fallback.
    IndexerAsync,
    /// 30s — indexer call, large payload (BEEF ancestry, gap scan), has chain fallback.
    IndexerBulk,
    /// 240s — third-party service with no alternate provider; single attempt.
    /// CEF outer cap for cert endpoints is 300s, giving 60s buffer — see module docs.
    ThirdPartyNoFallback,
}

impl CallClass {
    /// The reqwest total-request timeout for this class.
    pub fn timeout(&self) -> Duration {
        match self {
            CallClass::IndexerSync => Duration::from_secs(8),
            CallClass::IndexerAsync => Duration::from_secs(15),
            CallClass::IndexerBulk => Duration::from_secs(30),
            CallClass::ThirdPartyNoFallback => Duration::from_secs(240),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indexer_sync_is_8s() {
        assert_eq!(CallClass::IndexerSync.timeout(), Duration::from_secs(8));
    }

    #[test]
    fn indexer_async_is_15s() {
        assert_eq!(CallClass::IndexerAsync.timeout(), Duration::from_secs(15));
    }

    #[test]
    fn indexer_bulk_is_30s() {
        assert_eq!(CallClass::IndexerBulk.timeout(), Duration::from_secs(30));
    }

    #[test]
    fn third_party_no_fallback_is_240s() {
        assert_eq!(
            CallClass::ThirdPartyNoFallback.timeout(),
            Duration::from_secs(240)
        );
    }

    #[test]
    fn classes_have_distinct_timeouts() {
        // Sanity: a regression that collapses two classes to the same value
        // would defeat the whole point. Note this isn't required forever, but
        // catches an accidental edit collision today.
        let all = [
            CallClass::IndexerSync,
            CallClass::IndexerAsync,
            CallClass::IndexerBulk,
            CallClass::ThirdPartyNoFallback,
        ];
        for (i, a) in all.iter().enumerate() {
            for b in all.iter().skip(i + 1) {
                assert_ne!(
                    a.timeout(),
                    b.timeout(),
                    "{:?} and {:?} share a timeout — collapse not allowed",
                    a,
                    b
                );
            }
        }
    }

    #[test]
    fn third_party_does_not_exceed_cef_cert_cap() {
        // CEF's outer cap for cert endpoints is 300s (see
        // cef-native/src/core/HttpRequestInterceptor.cpp::postHttpTimeout —
        // the 300000ms branch for /acquireCertificate and /proveCertificate).
        // ThirdPartyNoFallback must stay below it (with margin) so the wallet's
        // structured response always wins over CEF's outer fallback. The
        // /signCertificate POST inside acquireCertificate is the dominant
        // consumer of this budget.
        const CEF_CERT_OUTER_CAP_SECS: u64 = 300;
        assert!(
            CallClass::ThirdPartyNoFallback.timeout() < Duration::from_secs(CEF_CERT_OUTER_CAP_SECS),
            "ThirdPartyNoFallback ({}s) exceeds CEF cert cap ({}s) — CEF will cancel before our timeout fires",
            CallClass::ThirdPartyNoFallback.timeout().as_secs(),
            CEF_CERT_OUTER_CAP_SECS,
        );
    }
}
