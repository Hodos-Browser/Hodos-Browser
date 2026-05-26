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
//! | `ThirdPartyNoFallback` | 90s | Third-party service (cert, paymail, MessageBox, overlay) with no alternate provider. Single attempt. |
//!
//! ## Why 90s for `ThirdPartyNoFallback`
//!
//! - 8s/15s budgets assume a provider chain absorbs one tier's failure. Third
//!   parties have no chain — a tight timeout just fails the user.
//! - The CEF outer cap on wallet HTTP requests is 120s
//!   (see `cef-native` and [[project_publish_on_acquire_restored]]). The wallet
//!   timeout must finish *before* CEF gives up so the wallet can surface a
//!   meaningful error to React. 90s leaves a 30s buffer.
//! - SocialCert (the slowest known third party) takes ~46-50s on a normal day;
//!   60s caught their slow days but failed during their bad days
//!   (see [[project_phase16_d_d_2_landed]]). 90s gives 40s of additional
//!   headroom over their typical wallclock.

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
    /// 90s — third-party service with no alternate provider; single attempt.
    ThirdPartyNoFallback,
}

impl CallClass {
    /// The reqwest total-request timeout for this class.
    pub fn timeout(&self) -> Duration {
        match self {
            CallClass::IndexerSync => Duration::from_secs(8),
            CallClass::IndexerAsync => Duration::from_secs(15),
            CallClass::IndexerBulk => Duration::from_secs(30),
            CallClass::ThirdPartyNoFallback => Duration::from_secs(90),
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
    fn third_party_no_fallback_is_90s() {
        assert_eq!(
            CallClass::ThirdPartyNoFallback.timeout(),
            Duration::from_secs(90)
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
    fn third_party_fits_under_cef_120s_cap() {
        // CEF's outer wallet-request cap is 120s. ThirdPartyNoFallback must
        // finish before CEF times out, otherwise the wallet's response never
        // reaches React. Leave a margin of >= 15s.
        let buffer = Duration::from_secs(120) - CallClass::ThirdPartyNoFallback.timeout();
        assert!(
            buffer >= Duration::from_secs(15),
            "Buffer between ThirdPartyNoFallback ({}s) and CEF cap (120s) is only {}s — too tight",
            CallClass::ThirdPartyNoFallback.timeout().as_secs(),
            buffer.as_secs()
        );
    }
}
