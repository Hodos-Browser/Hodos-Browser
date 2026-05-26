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
//! | `ThirdPartyNoFallback` | 120s | Third-party service (cert, paymail, MessageBox, overlay) with no alternate provider. Single attempt. |
//!
//! ## Why 120s for `ThirdPartyNoFallback`
//!
//! - 8s/15s budgets assume a provider chain absorbs one tier's failure. Third
//!   parties have no chain — a tight timeout just fails the user.
//! - The CEF outer cap on wallet HTTP requests is 120s
//!   (see `cef-native` and [[project_publish_on_acquire_restored]]). At 120s
//!   inner = 120s outer there is **no buffer**: if CEF's cancel wins the race,
//!   the wallet's response never reaches React. Bumped from 90s to 120s after
//!   SocialCert was observed taking 86-90s on degraded days (live smoke on
//!   2026-05-26 hit 86.06s for a successful second attempt while the first
//!   attempt timed out at exactly 90s). If the race becomes a real problem,
//!   bump CEF's outer cap in tandem and restore the buffer here.
//! - This is a class-level setting — it applies to all third-party-no-fallback
//!   sites (paymail, MessageBox/AuthFetch, overlay SHIP/lookup/submit, cert
//!   handlers). Intentional: site-specific overrides do not belong in the
//!   wallet. If a single host needs different semantics, redesign the class
//!   matrix rather than special-casing a hostname.

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
    /// 120s — third-party service with no alternate provider; single attempt.
    /// Rides at the CEF outer cap (no buffer) — see module docs.
    ThirdPartyNoFallback,
}

impl CallClass {
    /// The reqwest total-request timeout for this class.
    pub fn timeout(&self) -> Duration {
        match self {
            CallClass::IndexerSync => Duration::from_secs(8),
            CallClass::IndexerAsync => Duration::from_secs(15),
            CallClass::IndexerBulk => Duration::from_secs(30),
            CallClass::ThirdPartyNoFallback => Duration::from_secs(120),
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
    fn third_party_no_fallback_is_120s() {
        assert_eq!(
            CallClass::ThirdPartyNoFallback.timeout(),
            Duration::from_secs(120)
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
    fn third_party_does_not_exceed_cef_120s_cap() {
        // CEF's outer wallet-request cap is 120s. ThirdPartyNoFallback must
        // not exceed it, otherwise CEF will cancel the request before the
        // wallet's reqwest timeout fires and our error never reaches React.
        //
        // As of 2026-05-26 we deliberately ride AT the cap (120s = 120s, zero
        // buffer) — chosen over 90s after SocialCert was observed taking
        // 86-90s on degraded days. If the race becomes a real problem, bump
        // CEF's outer cap first, then restore a buffer here.
        assert!(
            CallClass::ThirdPartyNoFallback.timeout() <= Duration::from_secs(120),
            "ThirdPartyNoFallback ({}s) exceeds CEF cap (120s) — CEF will cancel before our timeout fires",
            CallClass::ThirdPartyNoFallback.timeout().as_secs()
        );
    }
}
