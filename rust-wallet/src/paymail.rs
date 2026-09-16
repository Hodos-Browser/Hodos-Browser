//! Paymail (bsvalias) Client
//!
//! Resolves human-readable paymail addresses (alice@handcash.io, $handle) to
//! P2PKH output scripts via the bsvalias protocol. Supports both P2P payment
//! destinations (instant receiver notification) and basic paymentDestination fallback.
//!
//! Reference: https://docs.moneybutton.com/docs/paymail-overview.html

use log::{debug, info, warn};
use std::collections::HashMap;
use std::sync::RwLock;
use std::time::Instant;

/// Known DNS SRV overrides — avoids needing a DNS resolver crate.
/// HandCash uses cloud.handcash.io but SRV record is on handcash.io.
const SRV_OVERRIDES: &[(&str, &str)] = &[
    ("handcash.io", "cloud.handcash.io"),
];

/// BRFC capability IDs
const CAP_P2P_DESTINATION: &str = "2a40af698840";  // P2P Payment Destination
const CAP_P2P_RECEIVE_TX: &str = "5f1323cddf31";   // P2P Receive Transaction
const CAP_PUBLIC_PROFILE: &str = "f12f968c92d6";    // Public Profile

/// Cache TTL for capability discovery (1 hour)
const CAPABILITY_CACHE_TTL_SECS: u64 = 3600;

/// Paymail client errors
#[derive(Debug, thiserror::Error)]
pub enum PaymailError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Invalid paymail format: {0}")]
    InvalidFormat(String),

    #[error("Capability discovery failed for {0}: {1}")]
    CapabilityDiscovery(String, String),

    #[error("Address resolution failed: {0}")]
    AddressResolution(String),

    #[error("P2P destination failed: {0}")]
    P2PDestination(String),

    #[error("Transaction submission failed: {0}")]
    TransactionSubmission(String),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("No payment capability found for {0}")]
    NoPaymentCapability(String),

    /// beta.3 Phase 10c panel `F3` — the host answered, and its answer is not
    /// one we may sign. ⛔ Distinct from every variant above, which mean "this
    /// path did not work": those may fall back to the basic path, this one may
    /// NOT. Falling back handed a host that had just tried to overbill a second
    /// chance at the same payment, against the owner's stated rule that a
    /// mismatch "must be rejected and user must be notified".
    #[error("{0}")]
    HostViolation(String),
}

impl PaymailError {
    /// True when the host broke a rule we enforce, rather than being unreachable.
    /// The send path must stop rather than try another endpoint on the same host.
    pub fn is_host_violation(&self) -> bool {
        matches!(self, PaymailError::HostViolation(_))
    }
}

/// The one HTTP client every paymail call uses. Named and module-level so the
/// redirect policy below is testable rather than asserted (panel `F2`).
pub(crate) fn build_http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(crate::services::CallClass::ThirdPartyNoFallback.timeout())
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
}

/// Cached capability URLs for a paymail domain
#[derive(Debug, Clone)]
struct CachedCapabilities {
    capabilities: PaymailCapabilities,
    fetched_at: Instant,
}

/// Parsed capability URLs from .well-known/bsvalias
#[derive(Debug, Clone)]
pub struct PaymailCapabilities {
    /// Basic payment destination URL template (paymentDestination)
    pub payment_destination_url: Option<String>,
    /// P2P payment destination URL template (2a40af698840)
    pub p2p_destination_url: Option<String>,
    /// P2P receive transaction URL template (5f1323cddf31)
    pub p2p_receive_tx_url: Option<String>,
    /// Public profile URL template (f12f968c92d6)
    pub public_profile_url: Option<String>,
}

/// P2P payment destination response
#[derive(Debug, Clone)]
pub struct P2PDestination {
    pub outputs: Vec<PaymailOutput>,
    pub reference: String,
}

/// Single output from a P2P destination response
#[derive(Debug, Clone, serde::Deserialize)]
pub struct PaymailOutput {
    #[serde(rename = "script")]
    pub script_hex: String,
    pub satoshis: i64,
}

/// beta.3 Phase 10c (CU-2) — the maximum outputs a paymail host may split a
/// payment across. BRC-29's P2P destination exists so a receiver can split a
/// payment; it does not say how far. 100 is the fix shape's suggestion and is
/// far above any real host's behaviour (HandCash returns 1–3).
pub const MAX_P2P_OUTPUTS: usize = 100;

/// Why a P2P destination response is not safe to sign.
#[derive(Debug, PartialEq, Eq)]
pub enum P2POutputsError {
    Empty,
    TooMany { count: usize },
    NonPositive { index: usize, satoshis: i64 },
    EmptyScript { index: usize },
    ScriptTooLong { index: usize, hex_len: usize },
    SumMismatch { sum: i64, requested: i64 },
}

/// beta.3 Phase 10c panel `F5` — the longest locking script a paymail host may
/// return, in hex characters (2,000 hex = 1,000 bytes). A P2PKH script is 25
/// bytes and the most baroque real receiver script is far under this.
///
/// ⛔ Why this is a money rule and not tidiness: the script is host-supplied and
/// its length is fed straight into fee estimation
/// (`handlers.rs :: create_action_internal` sums `script_hex.len() / 2` into the
/// estimated size, then prices it at the live ARC sat/KB rate). Without a bound,
/// a host that honours the satoshi total to the last unit can still inflate what
/// leaves the wallet, by making the transaction enormous. "May not change the
/// payment" has to mean the debit, not just the outputs.
pub const MAX_SCRIPT_HEX_LEN: usize = 2_000;

impl std::fmt::Display for P2POutputsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            P2POutputsError::Empty => write!(f, "the recipient's server returned no outputs"),
            P2POutputsError::TooMany { count } => write!(
                f, "the recipient's server returned {} outputs (limit {})", count, MAX_P2P_OUTPUTS),
            P2POutputsError::NonPositive { index, satoshis } => write!(
                f, "the recipient's server returned output {} with {} satoshis", index, satoshis),
            P2POutputsError::EmptyScript { index } => write!(
                f, "the recipient's server returned output {} with an empty script", index),
            P2POutputsError::ScriptTooLong { index, hex_len } => write!(
                f, "the recipient's server returned output {} with a {}-character script (limit {})",
                index, hex_len, MAX_SCRIPT_HEX_LEN),
            P2POutputsError::SumMismatch { sum, requested } => write!(
                f, "the recipient's server asked for {} satoshis but you approved {} — payment cancelled, nothing was sent",
                sum, requested),
        }
    }
}

/// beta.3 Phase 10c (CU-2, `P10c-A1`/`A2`/`A4`) — a paymail host may split the
/// payment, but it may not change it.
///
/// ⛔ The bsvalias P2P destination spec (BRFC `2a40af698840`) does NOT state this
/// rule — its own example answers a 1,000,100-satoshi request with outputs of
/// 10,000 + 20,000 — and the reference client (`bitcoin-sv/go-paymail`) does not
/// check it either. 👤 Owner decision 2026-09-15: it is **Hodos's** invariant, the
/// same one Phase 10b enforces for dApp payments — a signature exists only for an
/// amount the user saw. Without it, `x@evil.example` (or a compromised host for an
/// honest recipient) turns a 1,000-satoshi send into a 5,000,000-satoshi one, with
/// no second prompt: the send is an INTERNAL call and the permission gate priced
/// the request body, not the built transaction.
/// See `development-docs/PRIOR_ART.md` 2026-09-15 and the upstream issues in
/// Marston `Standards/BRCs/drafts/peerpay-messagebox-size-and-encoding/`.
pub fn validate_p2p_outputs(outputs: &[PaymailOutput], requested_satoshis: i64) -> Result<(), P2POutputsError> {
    let sum = validate_p2p_outputs_shape(outputs)?;
    if sum != requested_satoshis {
        return Err(P2POutputsError::SumMismatch { sum, requested: requested_satoshis });
    }
    Ok(())
}

/// The half of the check that does NOT depend on the amount: non-empty, bounded
/// count, every output positive with a plausible script. Returns the saturating
/// total so the caller can compare it if it has an amount to compare against.
///
/// ⛔ Separated 2026-09-15 by the Phase 10c adversarial panel (`F1`). `resolve()`
/// probes a handle by asking the host for a **546-satoshi** destination purely to
/// learn "can this alias receive?" — it commits to nothing and signs nothing. The
/// first cut of this phase ran the full check there, so a host that does not echo
/// a probe amount was reported as an **invalid recipient** and could not be paid
/// at all. That is not a hypothetical shape: BRFC `2a40af698840`'s own worked
/// example answers a 1,000,100-satoshi request with 10,000 + 20,000 (`D-4`), and
/// the contract claimed three times that the preview path was untouched.
/// ⇒ the probe gets the shape checks; only a real send gets the total check.
pub fn validate_p2p_outputs_shape(outputs: &[PaymailOutput]) -> Result<i64, P2POutputsError> {
    if outputs.is_empty() {
        return Err(P2POutputsError::Empty);
    }
    if outputs.len() > MAX_P2P_OUTPUTS {
        return Err(P2POutputsError::TooMany { count: outputs.len() });
    }
    let mut sum: i64 = 0;
    for (i, o) in outputs.iter().enumerate() {
        if o.satoshis <= 0 {
            return Err(P2POutputsError::NonPositive { index: i, satoshis: o.satoshis });
        }
        let script = o.script_hex.trim();
        if script.is_empty() {
            return Err(P2POutputsError::EmptyScript { index: i });
        }
        if script.len() > MAX_SCRIPT_HEX_LEN {
            return Err(P2POutputsError::ScriptTooLong { index: i, hex_len: script.len() });
        }
        // Saturating: a hostile host could otherwise overflow the sum to land on
        // the requested amount.
        sum = sum.saturating_add(o.satoshis);
    }
    Ok(sum)
}

/// beta.3 Phase 10c (CU-2) — every capability URL used on the SEND path must be
/// `https://`. A plain-http P2P destination lets anyone on the network swap the
/// outputs, which is the same defect as a lying host with no host to blame.
/// ⚠️ Applied to the send path only; the resolve/preview path is unchanged.
pub fn require_https_capability(url: &str, what: &str) -> Result<(), PaymailError> {
    if url.starts_with("https://") {
        return Ok(());
    }
    // Panel F3: terminal. An http endpoint is not a path that "did not work",
    // it is one we refuse — the send must not retry the same host elsewhere.
    Err(PaymailError::HostViolation(format!(
        "{} endpoint is not https ({}), refusing to use it", what,
        url.split(':').next().unwrap_or("?"))))
}

#[cfg(test)]
mod cu2_validation_tests {
    use super::*;

    fn out(sats: i64) -> PaymailOutput {
        PaymailOutput { script_hex: "76a914".to_string() + &"11".repeat(20) + "88ac", satoshis: sats }
    }

    /// `P10c-A1` — the measured attack shape: a host answering a 500,000-satoshi
    /// request with 5,000,000.
    #[test]
    fn a1_ten_times_the_request_is_refused() {
        let e = validate_p2p_outputs(&[out(5_000_000)], 500_000).unwrap_err();
        assert_eq!(e, P2POutputsError::SumMismatch { sum: 5_000_000, requested: 500_000 });
        assert!(e.to_string().contains("nothing was sent"));
    }

    /// `P10c-A2` — a host MAY split the amount; it may not change it. One satoshi
    /// either way is a mismatch.
    #[test]
    fn a2_exact_sum_split_across_three_outputs_is_accepted() {
        assert!(validate_p2p_outputs(&[out(300), out(400), out(300)], 1000).is_ok());
        assert!(validate_p2p_outputs(&[out(300), out(401), out(300)], 1000).is_err());
        assert!(validate_p2p_outputs(&[out(300), out(399), out(300)], 1000).is_err());
    }

    /// `P10c-A4` — count bound, non-positive values, empty scripts, and an
    /// overflow that would otherwise wrap onto the requested amount.
    #[test]
    fn a4_count_bound_and_bad_values_are_refused() {
        let many: Vec<PaymailOutput> = (0..=MAX_P2P_OUTPUTS).map(|_| out(1)).collect();
        assert_eq!(validate_p2p_outputs(&many, many.len() as i64).unwrap_err(),
                   P2POutputsError::TooMany { count: MAX_P2P_OUTPUTS + 1 });
        assert_eq!(validate_p2p_outputs(&[out(1000), out(0)], 1000).unwrap_err(),
                   P2POutputsError::NonPositive { index: 1, satoshis: 0 });
        assert_eq!(validate_p2p_outputs(&[out(1500), out(-500)], 1000).unwrap_err(),
                   P2POutputsError::NonPositive { index: 1, satoshis: -500 });
        assert_eq!(validate_p2p_outputs(&[], 1000).unwrap_err(), P2POutputsError::Empty);
        let mut blank = out(1000); blank.script_hex = String::new();
        assert_eq!(validate_p2p_outputs(&[blank], 1000).unwrap_err(), P2POutputsError::EmptyScript { index: 0 });
        // i64 overflow must not wrap onto the requested amount
        assert!(validate_p2p_outputs(&[out(i64::MAX), out(i64::MAX), out(1000)], 1000).is_err());
    }

    /// Panel `F1` — the resolve/preview PROBE must not enforce the total.
    /// ⛔ This is the regression the panel caught: the first cut ran the full
    /// check on `resolve()`'s 546-satoshi probe, so a host that answers a probe
    /// with anything other than 546 satoshis — which BRFC 2a40af698840's own
    /// worked example does — was reported as an INVALID RECIPIENT and could not
    /// be paid at all. Shape rules still apply on the probe.
    #[test]
    fn f1_probe_checks_shape_but_not_the_total() {
        // the exact shape that broke: probe asks 546, host answers otherwise
        let answer = [out(10_000), out(20_000)];
        assert!(validate_p2p_outputs_shape(&answer).is_ok(),
                "the probe must accept a host that does not echo the probe amount");
        assert_eq!(validate_p2p_outputs(&answer, 546).unwrap_err(),
                   P2POutputsError::SumMismatch { sum: 30_000, requested: 546 },
                   "a real SEND of 546 must still refuse the same answer");
        // the probe is not a hole: shape rules are still enforced there
        assert!(validate_p2p_outputs_shape(&[]).is_err());
        assert!(validate_p2p_outputs_shape(&[out(0)]).is_err());
        let many: Vec<PaymailOutput> = (0..=MAX_P2P_OUTPUTS).map(|_| out(1)).collect();
        assert!(validate_p2p_outputs_shape(&many).is_err());
    }

    /// Panel `F5` — a host may not inflate the transaction (and so the miner fee
    /// the user pays) with an enormous locking script while honouring the total.
    #[test]
    fn f5_script_length_is_bounded() {
        let mut huge = out(1000);
        huge.script_hex = "ab".repeat(MAX_SCRIPT_HEX_LEN); // 2x the limit in chars
        assert_eq!(validate_p2p_outputs(&[huge], 1000).unwrap_err(),
                   P2POutputsError::ScriptTooLong { index: 0, hex_len: MAX_SCRIPT_HEX_LEN * 2 });
        // an ordinary P2PKH script is nowhere near the bound
        assert!(validate_p2p_outputs(&[out(1000)], 1000).is_ok());
        let mut at_limit = out(1000);
        at_limit.script_hex = "a".repeat(MAX_SCRIPT_HEX_LEN);
        assert!(validate_p2p_outputs(&[at_limit], 1000).is_ok(), "the bound itself is allowed");
    }

    /// Panel `F3` — a broken rule is terminal, so the send path can tell it apart
    /// from "this endpoint did not work" and refuse instead of asking the same
    /// host again on another endpoint.
    #[test]
    fn f3_rule_breaches_are_marked_terminal() {
        let https_err = require_https_capability("http://h/p2p", "P2P destination").unwrap_err();
        assert!(https_err.is_host_violation(), "an http endpoint must be terminal");
        // the variants that mean "unreachable" must NOT be terminal, or a host
        // that is merely down would stop falling back to the basic path
        assert!(!PaymailError::P2PDestination("connection refused".into()).is_host_violation());
        assert!(!PaymailError::AddressResolution("HTTP 404".into()).is_host_violation());
        assert!(!PaymailError::NoPaymentCapability("a@b".into()).is_host_violation());
    }

    /// Panel `F2` — ⛔ the https check validates the URL we were GIVEN. If the
    /// client follows redirects, a host can advertise `https://…` and answer
    /// `302 Location: http://…`, and the destination request travels in cleartext
    /// for an on-path attacker to rewrite — the exact swap this phase exists to
    /// stop. reqwest 0.11's default is `Policy::limited(10)` and it permits an
    /// https→http hop unless `https_only` is set, which nothing in this wallet
    /// sets. This drives a real socket rather than asserting the builder.
    #[tokio::test]
    async fn f2_the_client_does_not_follow_a_redirect() {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let hops = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let hops_srv = hops.clone();
        std::thread::spawn(move || {
            // Serve two requests at most: the first 302s to a plain-http URL on
            // the same listener. A client that follows it arrives a second time.
            for _ in 0..2 {
                let Ok((mut sock, _)) = listener.accept() else { return };
                hops_srv.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                let mut buf = [0u8; 1024];
                let _ = sock.read(&mut buf);
                let body = format!(
                    "HTTP/1.1 302 Found
Location: http://127.0.0.1:{}/followed
Content-Length: 0
Connection: close

",
                    port);
                let _ = sock.write_all(body.as_bytes());
            }
        });

        let client = build_http_client();
        let resp = client.get(format!("http://127.0.0.1:{}/start", port)).send().await.unwrap();

        assert_eq!(resp.status().as_u16(), 302,
                   "the client must hand back the redirect, not follow it");
        assert_eq!(hops.load(std::sync::atomic::Ordering::SeqCst), 1,
                   "the server must have been hit exactly once — a second hit means the hop was followed");
    }

    /// `P10c-A3` — the send path refuses a non-https capability URL.
    #[test]
    fn a3_http_capability_url_is_refused() {
        assert!(require_https_capability("https://example.com/api/p2p", "P2P destination").is_ok());
        assert!(require_https_capability("http://example.com/api/p2p", "P2P destination").is_err());
        assert!(require_https_capability("http://127.0.0.1:8766/p2p", "P2P destination").is_err());
        assert!(require_https_capability("ftp://example.com/p2p", "P2P destination").is_err());
        // no scheme at all
        assert!(require_https_capability("example.com/p2p", "P2P destination").is_err());
    }
}

/// Public profile information
#[derive(Debug, Clone)]
pub struct PaymailProfile {
    pub name: String,
    pub avatar_url: Option<String>,
}

/// Combined resolution result for the resolve endpoint
#[derive(Debug, Clone, serde::Serialize)]
pub struct PaymailResolution {
    pub valid: bool,
    pub name: Option<String>,
    pub avatar_url: Option<String>,
    pub has_p2p: bool,
}

/// Paymail client with capability caching
pub struct PaymailClient {
    http_client: reqwest::Client,
    capability_cache: RwLock<HashMap<String, CachedCapabilities>>,
}

impl PaymailClient {
    /// Create a new PaymailClient. HTTP timeout sourced from
    /// `CallClass::ThirdPartyNoFallback` — paymail hosts are third parties
    /// with no Hodos-side fallback.
    pub fn new() -> Self {
        // beta.3 Phase 10c panel `F2` — ⛔ redirects OFF.
        //
        // `require_https_capability` checks the URL we were given. reqwest 0.11's
        // default policy follows up to 10 redirects and permits an https → http
        // hop unless `https_only` is set (which nothing in this wallet sets), so
        // a host could advertise `https://host/p2p/...` and answer `302 Location:
        // http://host/p2p/...`. The check would pass and the destination request
        // would still travel in cleartext — precisely the swap-the-outputs attack
        // the check exists to stop. `.well-known` discovery is worse: it is
        // hard-coded https, and a redirect to http would put the whole capability
        // set on the wire for an on-path attacker to rewrite.
        //
        // `Policy::none()` rather than `https_only(true)`: a paymail host has no
        // legitimate reason to redirect a machine API, and refusing the hop keeps
        // every URL we validate the URL we actually talk to.
        let http_client = build_http_client();

        Self {
            http_client,
            capability_cache: RwLock::new(HashMap::new()),
        }
    }

    /// Parse a paymail string into (alias, domain).
    ///
    /// Accepts:
    /// - Standard: `alice@handcash.io`
    /// - HandCash handle: `$alice` → `("alice", "handcash.io")`
    pub fn parse_paymail(input: &str) -> Result<(String, String), PaymailError> {
        let trimmed = input.trim();

        // HandCash $handle shorthand
        if trimmed.starts_with('$') {
            let alias = &trimmed[1..];
            if alias.is_empty() || alias.contains('@') || alias.contains(' ') {
                return Err(PaymailError::InvalidFormat(
                    format!("Invalid handle: {}", trimmed),
                ));
            }
            return Ok((alias.to_lowercase(), "handcash.io".to_string()));
        }

        // Standard alias@domain
        let parts: Vec<&str> = trimmed.splitn(2, '@').collect();
        if parts.len() != 2 {
            return Err(PaymailError::InvalidFormat(
                format!("Expected alias@domain, got: {}", trimmed),
            ));
        }

        let alias = parts[0].trim();
        let domain = parts[1].trim();

        if alias.is_empty() {
            return Err(PaymailError::InvalidFormat("Empty alias".to_string()));
        }
        if domain.is_empty() || !domain.contains('.') {
            return Err(PaymailError::InvalidFormat(
                format!("Invalid domain: {}", domain),
            ));
        }

        Ok((alias.to_lowercase(), domain.to_lowercase()))
    }

    /// Resolve the actual host for a paymail domain.
    /// Uses hardcoded SRV overrides for known providers (avoids DNS crate).
    fn discover_host(domain: &str) -> String {
        for &(src, dst) in SRV_OVERRIDES {
            if domain.eq_ignore_ascii_case(src) {
                debug!("PaymailClient: SRV override {} → {}", domain, dst);
                return dst.to_string();
            }
        }
        domain.to_string()
    }

    /// Fetch and cache capability URLs from .well-known/bsvalias
    async fn discover_capabilities(
        &self,
        domain: &str,
    ) -> Result<PaymailCapabilities, PaymailError> {
        // Check cache (read lock, dropped before any async work)
        {
            let cache = self.capability_cache.read().unwrap();
            if let Some(cached) = cache.get(domain) {
                if cached.fetched_at.elapsed().as_secs() < CAPABILITY_CACHE_TTL_SECS {
                    debug!("PaymailClient: capability cache hit for {}", domain);
                    return Ok(cached.capabilities.clone());
                }
            }
        }

        let host = Self::discover_host(domain);
        let url = format!("https://{}/.well-known/bsvalias", host);
        debug!("PaymailClient: discovering capabilities at {}", url);

        let response = self
            .http_client
            .get(&url)
            .send()
            .await
            .map_err(|e| {
                PaymailError::CapabilityDiscovery(domain.to_string(), e.to_string())
            })?;

        let status = response.status().as_u16();
        if status != 200 {
            return Err(PaymailError::CapabilityDiscovery(
                domain.to_string(),
                format!("HTTP {}", status),
            ));
        }

        let json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| {
                PaymailError::CapabilityDiscovery(domain.to_string(), e.to_string())
            })?;

        let caps_obj = json.get("capabilities").unwrap_or(&json);

        let capabilities = PaymailCapabilities {
            payment_destination_url: caps_obj
                .get("paymentDestination")
                .or_else(|| caps_obj.get("5f1323cddf31")) // fallback BRFC
                .and_then(|v| v.as_str())
                .map(String::from),
            p2p_destination_url: caps_obj
                .get(CAP_P2P_DESTINATION)
                .and_then(|v| v.as_str())
                .map(String::from),
            p2p_receive_tx_url: caps_obj
                .get(CAP_P2P_RECEIVE_TX)
                .and_then(|v| v.as_str())
                .map(String::from),
            public_profile_url: caps_obj
                .get(CAP_PUBLIC_PROFILE)
                .and_then(|v| v.as_str())
                .map(String::from),
        };

        info!(
            "PaymailClient: {} capabilities — P2P: {}, basic: {}, profile: {}",
            domain,
            capabilities.p2p_destination_url.is_some(),
            capabilities.payment_destination_url.is_some(),
            capabilities.public_profile_url.is_some(),
        );

        // Update cache (write lock, dropped immediately)
        {
            let mut cache = self.capability_cache.write().unwrap();
            cache.insert(
                domain.to_string(),
                CachedCapabilities {
                    capabilities: capabilities.clone(),
                    fetched_at: Instant::now(),
                },
            );
        }

        Ok(capabilities)
    }

    /// Expand a bsvalias URL template, replacing {alias} and {domain.tld}
    fn expand_url(template: &str, alias: &str, domain: &str) -> String {
        template
            .replace("{alias}", alias)
            .replace("{domain.tld}", domain)
            .replace("{name}", alias)
    }

    /// Get P2P payment destination (preferred path).
    ///
    /// POST to the P2P destination endpoint with the payment amount.
    /// Returns output scripts and a reference string for receiver notification.
    /// Ask a host where to pay, for a payment we intend to sign.
    /// Enforces the full rule, including the total.
    pub async fn get_p2p_destination(
        &self,
        alias: &str,
        domain: &str,
        satoshis: i64,
    ) -> Result<P2PDestination, PaymailError> {
        self.p2p_destination_inner(alias, domain, satoshis, true).await
    }

    /// Ask a host the same question purely to learn whether an alias can receive
    /// (`resolve()`'s existence probe). ⛔ Does **not** enforce the total — see
    /// `validate_p2p_outputs_shape` for why enforcing it here broke real hosts.
    /// Nothing is signed, built or committed on this path.
    async fn p2p_destination_probe(
        &self,
        alias: &str,
        domain: &str,
        satoshis: i64,
    ) -> Result<P2PDestination, PaymailError> {
        self.p2p_destination_inner(alias, domain, satoshis, false).await
    }

    async fn p2p_destination_inner(
        &self,
        alias: &str,
        domain: &str,
        satoshis: i64,
        enforce_total: bool,
    ) -> Result<P2PDestination, PaymailError> {
        let caps = self.discover_capabilities(domain).await?;

        let url_template = caps.p2p_destination_url.ok_or_else(|| {
            PaymailError::P2PDestination(format!("{} does not support P2P", domain))
        })?;

        let url = Self::expand_url(&url_template, alias, domain);
        // 10c (CU-2, `P10c-A3`): the send path never talks to a non-https capability.
        require_https_capability(&url, "P2P destination")?;
        debug!("PaymailClient: P2P destination POST {}", url);

        let body = serde_json::json!({ "satoshis": satoshis });

        let response = self
            .http_client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| PaymailError::P2PDestination(e.to_string()))?;

        let status = response.status().as_u16();
        if status != 200 {
            let text = response.text().await.unwrap_or_default();
            return Err(PaymailError::P2PDestination(format!(
                "HTTP {} — {}",
                status, text
            )));
        }

        let json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| PaymailError::P2PDestination(e.to_string()))?;

        let outputs: Vec<PaymailOutput> = serde_json::from_value(
            json.get("outputs")
                .cloned()
                .ok_or_else(|| PaymailError::P2PDestination("missing outputs".to_string()))?,
        )
        .map_err(|e| PaymailError::P2PDestination(format!("bad outputs: {}", e)))?;

        let reference = json
            .get("reference")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        // 10c (CU-2): a host may split the payment; it may not change it. Checked
        // HERE, before the caller can build anything from these outputs.
        // `enforce_total` is false only for `resolve()`'s existence probe (panel F1).
        let verdict = if enforce_total {
            validate_p2p_outputs(&outputs, satoshis)
        } else {
            validate_p2p_outputs_shape(&outputs).map(|_| ())
        };
        if let Err(e) = verdict {
            warn!("PaymailClient: refusing P2P destination from {} — {}", domain, e);
            // Panel F3: a broken rule is terminal. The send path must not answer
            // it by asking the same host again on another endpoint.
            return Err(PaymailError::HostViolation(e.to_string()));
        }

        info!(
            "PaymailClient: P2P destination OK — {} outputs, ref={}...",
            outputs.len(),
            &reference[..reference.len().min(20)]
        );

        Ok(P2PDestination { outputs, reference })
    }

    /// Resolve a basic payment destination (fallback path).
    ///
    /// POST to the paymentDestination endpoint.
    /// Returns a single output script hex string.
    pub async fn resolve_address(
        &self,
        alias: &str,
        domain: &str,
        satoshis: i64,
        sender_display_name: &str,
    ) -> Result<String, PaymailError> {
        let caps = self.discover_capabilities(domain).await?;

        let url_template = caps.payment_destination_url.ok_or_else(|| {
            PaymailError::NoPaymentCapability(format!("{}@{}", alias, domain))
        })?;

        let url = Self::expand_url(&url_template, alias, domain);
        // 10c (CU-2, `P10c-A3`): send path, https only.
        require_https_capability(&url, "payment destination")?;
        debug!("PaymailClient: basic paymentDestination POST {}", url);

        // Basic paymentDestination requires senderName and dt
        let dt = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();
        let sender_label = format!("{}'s Hodos Wallet", sender_display_name);

        let body = serde_json::json!({
            "senderName": sender_label,
            "senderHandle": sender_label,
            "dt": dt,
            "amount": satoshis,
            "purpose": "payment",
        });

        let response = self
            .http_client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| PaymailError::AddressResolution(e.to_string()))?;

        let status = response.status().as_u16();
        if status != 200 {
            let text = response.text().await.unwrap_or_default();
            return Err(PaymailError::AddressResolution(format!(
                "HTTP {} — {}",
                status, text
            )));
        }

        let json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| PaymailError::AddressResolution(e.to_string()))?;

        // Response has "output" (script hex) field
        let script_hex = json
            .get("output")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                PaymailError::AddressResolution("missing 'output' in response".to_string())
            })?
            .to_string();

        if script_hex.is_empty() {
            return Err(PaymailError::AddressResolution(
                "empty output script".to_string(),
            ));
        }

        info!(
            "PaymailClient: basic resolution OK — script {}...{}",
            &script_hex[..script_hex.len().min(10)],
            &script_hex[script_hex.len().saturating_sub(6)..]
        );

        Ok(script_hex)
    }

    /// Submit a transaction to the receiver's P2P receive-tx endpoint.
    ///
    /// This notifies the receiver that a payment was made. Non-fatal if it fails
    /// since the transaction is already broadcast on-chain.
    pub async fn submit_transaction(
        &self,
        alias: &str,
        domain: &str,
        raw_tx_hex: &str,
        reference: &str,
        sender_label: &str,
    ) -> Result<(), PaymailError> {
        let caps = self.discover_capabilities(domain).await?;

        let url_template = match caps.p2p_receive_tx_url {
            Some(u) => u,
            None => {
                debug!("PaymailClient: no receive-tx endpoint for {}", domain);
                return Ok(());
            }
        };

        let url = Self::expand_url(&url_template, alias, domain);
        // 10c (CU-2, `P10c-A3`): send path, https only.
        require_https_capability(&url, "P2P receive-transaction")?;
        debug!("PaymailClient: submit_transaction POST {}", url);

        let body = serde_json::json!({
            "hex": raw_tx_hex,
            "reference": reference,
            "metadata": {
                "sender": sender_label,
                "note": format!("Payment from {}", sender_label)
            }
        });

        let response = self
            .http_client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| PaymailError::TransactionSubmission(e.to_string()))?;

        let status = response.status().as_u16();
        if status != 200 {
            let text = response.text().await.unwrap_or_default();
            warn!(
                "PaymailClient: receive-tx returned HTTP {} — {}",
                status, text
            );
            return Err(PaymailError::TransactionSubmission(format!(
                "HTTP {}",
                status
            )));
        }

        info!("PaymailClient: receive-tx submitted OK to {}@{}", alias, domain);
        Ok(())
    }

    /// Fetch the public profile for a paymail address.
    /// Returns None on any failure (non-fatal).
    pub async fn get_profile(
        &self,
        alias: &str,
        domain: &str,
    ) -> Option<PaymailProfile> {
        let caps = match self.discover_capabilities(domain).await {
            Ok(c) => c,
            Err(_) => return None,
        };

        let url_template = caps.public_profile_url?;
        let url = Self::expand_url(&url_template, alias, domain);
        debug!("PaymailClient: profile GET {}", url);

        let response = match self.http_client.get(&url).send().await {
            Ok(r) => r,
            Err(_) => return None,
        };

        if response.status().as_u16() != 200 {
            return None;
        }

        let json: serde_json::Value = match response.json().await {
            Ok(j) => j,
            Err(_) => return None,
        };

        let name = json
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let avatar_url = json
            .get("avatar")
            .or_else(|| json.get("avatarUrl"))
            .or_else(|| json.get("avatar_url"))
            .and_then(|v| v.as_str())
            .map(String::from);

        Some(PaymailProfile { name, avatar_url })
    }

    /// Combined validation + profile resolution for the resolve endpoint.
    ///
    /// Returns a PaymailResolution struct (always succeeds at HTTP level;
    /// `valid: false` when the paymail can't be resolved).
    pub async fn resolve(&self, paymail_input: &str) -> PaymailResolution {
        let (alias, domain) = match Self::parse_paymail(paymail_input) {
            Ok(p) => p,
            Err(_) => {
                return PaymailResolution {
                    valid: false,
                    name: None,
                    avatar_url: None,
                    has_p2p: false,
                };
            }
        };

        let caps = match self.discover_capabilities(&domain).await {
            Ok(c) => c,
            Err(e) => {
                debug!("PaymailClient: resolve failed for {}@{}: {}", alias, domain, e);
                return PaymailResolution {
                    valid: false,
                    name: None,
                    avatar_url: None,
                    has_p2p: false,
                };
            }
        };

        let has_p2p = caps.p2p_destination_url.is_some();
        let has_basic = caps.payment_destination_url.is_some();

        if !has_p2p && !has_basic {
            return PaymailResolution {
                valid: false,
                name: None,
                avatar_url: None,
                has_p2p: false,
            };
        }

        // Authoritative validity check: call the actual payment destination
        // endpoint. This is the only reliable way to know if an alias exists
        // because:
        //   - Capability discovery only proves the DOMAIN supports paymail.
        //   - HandCash's public-profile endpoint is unreliable — it returns
        //     HTTP 200 with mismatched data for some non-existent handles
        //     and HTTP 400 for others.
        //   - The payment-destination endpoints return HTTP 400 with
        //     "Paymail not found" for non-existent handles consistently,
        //     and HTTP 200 with a real output script for valid ones.
        //
        // We use a dust amount (546 sats) for the lookup — this does not
        // commit to anything, it just asks "can this paymail receive?".
        //
        // Prefer P2P (BRC-compliant `{satoshis}` body) because HandCash's
        // basic paymentDestination expects a different body format and will
        // reject BRC-standard requests. Fall back to basic only if the
        // domain has no P2P capability.
        let exists = if has_p2p {
            // Panel F1: the PROBE variant — shape checked, total not. This asks
            // "can this alias receive?", it does not approve a payment.
            self.p2p_destination_probe(&alias, &domain, 546)
                .await
                .map(|_| ())
                .map_err(|e| e.to_string())
        } else {
            self.resolve_address(&alias, &domain, 546, "Hodos")
                .await
                .map(|_| ())
                .map_err(|e| e.to_string())
        };

        match exists {
            Ok(_) => {
                // Valid — fetch profile metadata as secondary info (non-fatal).
                // Profile endpoint returns the owner's display name and avatar,
                // which may differ from the alias (e.g. handle "arch" owned by
                // user with display name "Chaddy.b"). This is normal.
                let profile = self.get_profile(&alias, &domain).await;
                PaymailResolution {
                    valid: true,
                    name: profile.as_ref().map(|p| p.name.clone()),
                    avatar_url: profile.and_then(|p| p.avatar_url),
                    has_p2p,
                }
            }
            Err(e) => {
                debug!(
                    "PaymailClient: payment-destination lookup failed for {}@{}: {}",
                    alias, domain, e
                );
                PaymailResolution {
                    valid: false,
                    name: None,
                    avatar_url: None,
                    has_p2p: false,
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_paymail_standard() {
        let (alias, domain) = PaymailClient::parse_paymail("alice@handcash.io").unwrap();
        assert_eq!(alias, "alice");
        assert_eq!(domain, "handcash.io");
    }

    #[test]
    fn test_parse_paymail_uppercase() {
        let (alias, domain) = PaymailClient::parse_paymail("Alice@HandCash.IO").unwrap();
        assert_eq!(alias, "alice");
        assert_eq!(domain, "handcash.io");
    }

    #[test]
    fn test_parse_paymail_handcash_handle() {
        let (alias, domain) = PaymailClient::parse_paymail("$alice").unwrap();
        assert_eq!(alias, "alice");
        assert_eq!(domain, "handcash.io");
    }

    #[test]
    fn test_parse_paymail_handcash_handle_uppercase() {
        let (alias, domain) = PaymailClient::parse_paymail("$Brandon").unwrap();
        assert_eq!(alias, "brandon");
        assert_eq!(domain, "handcash.io");
    }

    #[test]
    fn test_parse_paymail_whitespace() {
        let (alias, domain) = PaymailClient::parse_paymail("  bob@example.com  ").unwrap();
        assert_eq!(alias, "bob");
        assert_eq!(domain, "example.com");
    }

    #[test]
    fn test_parse_paymail_invalid_no_at() {
        assert!(PaymailClient::parse_paymail("notapaymail").is_err());
    }

    #[test]
    fn test_parse_paymail_invalid_empty_alias() {
        assert!(PaymailClient::parse_paymail("@handcash.io").is_err());
    }

    #[test]
    fn test_parse_paymail_invalid_no_dot_domain() {
        assert!(PaymailClient::parse_paymail("alice@localhost").is_err());
    }

    #[test]
    fn test_parse_paymail_invalid_empty_handle() {
        assert!(PaymailClient::parse_paymail("$").is_err());
    }

    #[test]
    fn test_parse_paymail_invalid_handle_with_at() {
        assert!(PaymailClient::parse_paymail("$alice@domain").is_err());
    }

    #[test]
    fn test_discover_host_override() {
        assert_eq!(
            PaymailClient::discover_host("handcash.io"),
            "cloud.handcash.io"
        );
    }

    #[test]
    fn test_discover_host_passthrough() {
        assert_eq!(
            PaymailClient::discover_host("simply.cash"),
            "simply.cash"
        );
    }

    #[test]
    fn test_discover_host_case_insensitive() {
        assert_eq!(
            PaymailClient::discover_host("HandCash.IO"),
            "cloud.handcash.io"
        );
    }

    #[test]
    fn test_expand_url_both_placeholders() {
        let template = "https://example.com/api/v1/address/{alias}@{domain.tld}";
        let result = PaymailClient::expand_url(template, "alice", "example.com");
        assert_eq!(result, "https://example.com/api/v1/address/alice@example.com");
    }

    #[test]
    fn test_expand_url_name_placeholder() {
        let template = "https://example.com/profile/{name}";
        let result = PaymailClient::expand_url(template, "bob", "example.com");
        assert_eq!(result, "https://example.com/profile/bob");
    }

    #[test]
    fn test_expand_url_no_placeholders() {
        let template = "https://example.com/static";
        let result = PaymailClient::expand_url(template, "alice", "example.com");
        assert_eq!(result, "https://example.com/static");
    }
}
