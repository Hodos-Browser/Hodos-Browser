//! BSV Overlay Services client
//!
//! Handles submission of transactions to the BSV overlay network.
//! Uses SHIP (Service Host Interconnect Protocol) to discover overlay hosts
//! dynamically, with hardcoded fallbacks for reliability.
//!
//! SHIP discovery flow:
//! 1. Query SLAP trackers for `ls_ship` service with desired topics
//! 2. Parse SHIP advertisement outputs (PushDrop scripts) from response
//! 3. Extract host URLs and topic mappings
//! 4. Submit BEEF to discovered hosts
//! 5. Fall back to hardcoded hosts if discovery fails

use log::{debug, info, warn};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

pub mod ship_cache;

use ship_cache::ShipDiscoveryCache;

// ═══════════════════════════════════════════════════════════════
// Constants
// ═══════════════════════════════════════════════════════════════

/// Default SLAP trackers for SHIP host discovery (mainnet).
/// These are the well-known overlay nodes that index SHIP advertisements.
/// Matches the BSV SDK's DEFAULT_SLAP_TRACKERS list.
const DEFAULT_SLAP_TRACKERS: &[&str] = &[
    "https://overlay-us-1.bsvb.tech",
    "https://overlay-eu-1.bsvb.tech",
    "https://overlay-ap-1.bsvb.tech",
    "https://users.bapp.dev",
];

/// Hardcoded fallback hosts per topic (used when SHIP discovery fails).
/// Maps topic name → list of known hosts.
fn fallback_hosts_for_topic(topic: &str) -> Vec<&'static str> {
    match topic {
        "tm_identity" => vec![
            "https://overlay-us-1.bsvb.tech",
            "https://overlay-eu-1.bsvb.tech",
            "https://overlay-ap-1.bsvb.tech",
        ],
        _ => vec![
            "https://overlay-us-1.bsvb.tech",
            "https://overlay-eu-1.bsvb.tech",
            "https://overlay-ap-1.bsvb.tech",
        ],
    }
}

/// Topic for identity certificates
pub const TOPIC_IDENTITY: &str = "tm_identity";

// Per-call timeouts now sourced from `crate::services::CallClass`:
//   - SHIP discovery + overlay submit/lookup → `CallClass::ThirdPartyNoFallback` (90s)
// See `services/call_class.rs` for the policy table.
//
// SHIP host discovery cache lives in `ship_cache::ShipDiscoveryCache` on
// `AppState`. Hosts for `tm_identity` are kept warm by
// `monitor::task_refresh_ship_cache` so publish/unpublish never blocks on
// SHIP discovery during normal usage.

// ═══════════════════════════════════════════════════════════════
// Public API
// ═══════════════════════════════════════════════════════════════

/// Submit a BEEF transaction to the overlay network for the `tm_identity` topic.
///
/// Discovery strategy:
/// 1. Try SHIP discovery to find all hosts serving `tm_identity`
/// 2. Submit to ALL discovered hosts (overlays are idempotent)
/// 3. If discovery finds no hosts or all fail, fall back to hardcoded hosts
///
/// Returns Ok(true) if at least one host accepted the transaction.
/// Returns Ok(false) if all hosts rejected (but no network error).
/// Returns Err on total failure.
pub async fn submit_to_identity_overlay(
    ship_cache: &Arc<ShipDiscoveryCache>,
    beef_bytes: &[u8],
) -> Result<bool, String> {
    submit_to_topic(ship_cache, TOPIC_IDENTITY, beef_bytes, false).await
}

/// Like `submit_to_identity_overlay`, but returns as soon as ANY host responds
/// (success, rejection, or per-host error) rather than waiting for the first
/// definitive `Ok(true)`. Use this when the caller verifies the outcome via a
/// separate `/lookup` query — e.g. unpublish, where overlay-express's STEAK is
/// ambiguous for removals and `lookup_published_certificate` is the actual
/// confirmation step.
pub async fn submit_to_identity_overlay_early_return(
    ship_cache: &Arc<ShipDiscoveryCache>,
    beef_bytes: &[u8],
) -> Result<bool, String> {
    submit_to_topic(ship_cache, TOPIC_IDENTITY, beef_bytes, true).await
}

/// Get all known overlay lookup endpoints for identity resolution.
/// Combines SHIP-discovered hosts with hardcoded fallbacks, deduplicates.
/// Returns URLs with `/lookup` path appended.
pub async fn get_identity_lookup_endpoints(ship_cache: &Arc<ShipDiscoveryCache>) -> Vec<String> {
    let discovered = discover_hosts_for_topic(ship_cache, TOPIC_IDENTITY).await;
    let fallbacks = fallback_hosts_for_topic(TOPIC_IDENTITY);

    let mut host_set: HashSet<String> = HashSet::new();
    for h in discovered { host_set.insert(h); }
    for h in fallbacks { host_set.insert(h.to_string()); }

    host_set.into_iter()
        .map(|h| format!("{}/lookup", h))
        .collect()
}

/// Submit a BEEF transaction to all overlay hosts serving a given topic.
///
/// Phase 1.6d polish (2026-05-27): parallel submission with early-return + bg drain.
///
/// Overlays do not gossip among themselves (no GASP), so we must reach every
/// known host eventually. But the user only needs ONE host to admit for the
/// publish to be considered successful — overlays are idempotent and any
/// subsequent /lookup will hit at least the admitting host's index. So:
///
/// 1. Discover hosts (SHIP cache hit, ~instant per Step 1)
/// 2. Spawn parallel submissions to ALL hosts via `JoinSet`
/// 3. Return `Ok(true)` as soon as ANY host accepts (typically 2–5s)
/// 4. Move the remaining `JoinSet` into a `tokio::spawn`'d drain task so
///    slow/hung hosts continue to receive the BEEF in the background. Since
///    overlays are idempotent, late submissions to already-admitted txs are
///    no-ops on the overlay side.
///
/// Failure path: if all hosts complete without acceptance, return `Ok(false)`
/// (everything rejected cleanly) or `Err` (last network error). The user
/// waits for the SLOWEST host in this case — acceptable because it only
/// happens when the publish actually failed.
pub async fn submit_to_topic(
    ship_cache: &Arc<ShipDiscoveryCache>,
    topic: &str,
    beef_bytes: &[u8],
    early_return_on_any_response: bool,
) -> Result<bool, String> {
    if beef_bytes.is_empty() {
        return Err("Empty BEEF bytes — nothing to submit".to_string());
    }

    let client = reqwest::Client::builder()
        .timeout(crate::services::CallClass::ThirdPartyNoFallback.timeout())
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    // Step 1: Merge SHIP-discovered hosts with hardcoded fallbacks (SDK pattern).
    let discovered_hosts = discover_hosts_for_topic(ship_cache, topic).await;
    let fallbacks = fallback_hosts_for_topic(topic);

    let mut host_set: HashSet<String> = HashSet::new();
    for h in &discovered_hosts { host_set.insert(h.clone()); }
    for h in &fallbacks { host_set.insert(h.to_string()); }
    let hosts: Vec<String> = host_set.into_iter().collect();
    let n_hosts = hosts.len();

    info!("Overlay: {} host(s) for '{}' ({} discovered + {} fallback, {} unique)",
        n_hosts, topic, discovered_hosts.len(), fallbacks.len(), n_hosts);

    // Step 2: Spawn parallel submissions. Share large/owned values via Arc so
    // each spawned task captures cheaply (and so they live past this function
    // when we move the JoinSet into the bg-drain task).
    let beef = std::sync::Arc::new(beef_bytes.to_vec());
    let topics_json = std::sync::Arc::new(serde_json::json!([topic]).to_string());
    let topic_owned = topic.to_string();

    let mut set: tokio::task::JoinSet<(String, Result<bool, String>)> = tokio::task::JoinSet::new();
    for host in hosts {
        let client = client.clone();
        let beef = std::sync::Arc::clone(&beef);
        let topics_json = std::sync::Arc::clone(&topics_json);
        set.spawn(async move {
            let result = submit_beef_to_host(&client, &host, &beef, &topics_json).await;
            (host, result)
        });
    }

    // Step 3: Drain the set until we see the first Ok(true) OR exhaust all hosts.
    let mut last_error = String::new();

    // Helper: spawn a bg-drain task taking ownership of the remaining JoinSet.
    // JoinSet aborts its tasks on Drop, so we MUST move it into a spawned task
    // to let remaining submissions complete naturally.
    fn drain_in_background(set: tokio::task::JoinSet<(String, Result<bool, String>)>, topic_bg: String) {
        if set.is_empty() { return; }
        tokio::spawn(async move {
            let mut set = set;
            while let Some(joined) = set.join_next().await {
                match joined {
                    Ok((h, Ok(true)))  => info!("Overlay (bg): {} also accepted for '{}'", h, topic_bg),
                    Ok((h, Ok(false))) => warn!("Overlay (bg): {} rejected for '{}'", h, topic_bg),
                    Ok((h, Err(e)))    => warn!("Overlay (bg): {} error for '{}': {}", h, topic_bg, e),
                    Err(e)             => warn!("Overlay (bg): task panic for '{}': {}", topic_bg, e),
                }
            }
            info!("Overlay (bg): all background submissions for '{}' completed", topic_bg);
        });
    }

    let mut any_response_seen = false;

    while let Some(joined) = set.join_next().await {
        match joined {
            Ok((host, Ok(true))) => {
                let remaining = set.len();
                info!(
                    "Overlay: ✅ {} accepted for '{}' (early return; {} task(s) continue in background)",
                    host, topic_owned, remaining,
                );
                drain_in_background(set, topic_owned.clone());
                return Ok(true);
            }
            Ok((host, Ok(false))) => {
                warn!("Overlay: {} rejected for '{}'", host, topic_owned);
                any_response_seen = true;
            }
            Ok((host, Err(e))) => {
                warn!("Overlay: {} error for '{}': {}", host, topic_owned, e);
                last_error = e;
                any_response_seen = true;
            }
            Err(e) => warn!("Overlay: task panic for '{}': {}", topic_owned, e),
        }

        // For unpublish-style callers: as soon as ANY host has responded (success,
        // rejection, or per-host error), return — caller will verify via /lookup.
        // The remaining hosts continue draining in the background so every overlay
        // still receives the BEEF (we don't gossip — must reach all hosts eventually).
        if early_return_on_any_response && any_response_seen {
            let remaining = set.len();
            info!(
                "Overlay: first response received for '{}' (early-return-on-any-response; {} task(s) continue in background)",
                topic_owned, remaining,
            );
            drain_in_background(set, topic_owned.clone());
            // Return Ok(false) so the caller knows to verify via lookup.
            // (Not Err — the submit phase itself didn't fail, it's just that no
            // host returned a definitive Ok(true), which is normal for unpublish.)
            return Ok(false);
        }
    }

    // All hosts completed without acceptance.
    warn!("Overlay: all {} host(s) completed for '{}' without acceptance", n_hosts, topic_owned);
    if last_error.is_empty() {
        Ok(false)
    } else {
        Err(format!("All overlay hosts failed for '{}'. Last error: {}", topic_owned, last_error))
    }
}

/// Per-host lookup helper. Returns:
/// - `Ok(Some((beef, idx)))` — cert found on this host
/// - `Ok(None)` — host returned 200 but cert not present
/// - `Err(msg)` — network error, non-200 status, or parse failure (skip this host)
async fn lookup_certificate_on_host(
    client: reqwest::Client,
    host: String,
    body: serde_json::Value,
    serial_number_b64: String,
) -> Result<Option<(Vec<u8>, usize)>, String> {
    let url = format!("{}/lookup", host);
    debug!("Overlay lookup: POST {} for serialNumber {}", url, serial_number_b64);

    let response = client
        .post(&url)
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("HTTP error from {}: {}", host, e))?;

    if response.status().as_u16() != 200 {
        return Err(format!("HTTP {} from {}", response.status(), host));
    }

    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("JSON parse error from {}: {}", host, e))?;

    let outputs = json.get("outputs").and_then(|v| v.as_array());
    if let Some(outputs) = outputs {
        if !outputs.is_empty() {
            if let Some(first) = outputs.first() {
                let output_index = first.get("outputIndex").and_then(|v| v.as_u64()).unwrap_or(0) as usize;

                use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
                let beef_bytes = if let Some(s) = first.get("beef").and_then(|v| v.as_str()) {
                    BASE64.decode(s).ok()
                } else if let Some(arr) = first.get("beef").and_then(|v| v.as_array()) {
                    Some(arr.iter().filter_map(|v| v.as_u64().map(|n| n as u8)).collect::<Vec<u8>>())
                } else {
                    None
                };

                if let Some(beef_bytes) = beef_bytes.filter(|b| !b.is_empty()) {
                    info!("Overlay lookup: found certificate on {} ({} bytes BEEF, outputIndex {})", host, beef_bytes.len(), output_index);
                    return Ok(Some((beef_bytes, output_index)));
                }
                // BEEF couldn't be decoded — treat as "not a definitive answer" so we let other hosts respond
                return Err(format!("Could not decode BEEF from {}", host));
            }
        }
    }

    debug!("Overlay lookup: certificate not found on {}", host);
    Ok(None)
}

/// Query the overlay to check if a certificate is published.
///
/// Queries by serialNumber for exact match.
/// Returns the BEEF bytes and output index if found.
///
/// Parallelizes across all known hosts (SHIP-discovered + hardcoded fallback)
/// and returns on the first definitive answer:
/// - First `Ok(Some)` (cert found) → return immediately, bg-drain rest
/// - First `Ok(None)` (cert not present) → return immediately, bg-drain rest
/// - All hosts errored → return Err
///
/// This is the verify-step companion to `submit_to_topic`. Before this change,
/// a sequential `for host in &hosts` loop could block the user for up to the
/// per-host timeout (240s) if the first host iterated was slow/dead.
pub async fn lookup_published_certificate(
    serial_number_b64: &str,
) -> Result<Option<(Vec<u8>, usize)>, String> {
    let client = reqwest::Client::builder()
        .timeout(crate::services::CallClass::ThirdPartyNoFallback.timeout())
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let body = serde_json::json!({
        "service": "ls_identity",
        "query": {
            "serialNumber": serial_number_b64
        }
    });

    let discovered = discover_hosts_for_service("ls_identity").await;
    let fallbacks = fallback_hosts_for_topic(TOPIC_IDENTITY);
    let mut host_set: HashSet<String> = HashSet::new();
    for h in discovered { host_set.insert(h); }
    for h in fallbacks { host_set.insert(h.to_string()); }
    let hosts: Vec<String> = host_set.into_iter().collect();
    let n_hosts = hosts.len();

    // Spawn parallel lookups via JoinSet.
    let mut set: tokio::task::JoinSet<(String, Result<Option<(Vec<u8>, usize)>, String>)> =
        tokio::task::JoinSet::new();
    for host in hosts {
        let client = client.clone();
        let body = body.clone();
        let serial = serial_number_b64.to_string();
        set.spawn(async move {
            let result = lookup_certificate_on_host(client, host.clone(), body, serial).await;
            (host, result)
        });
    }

    let mut last_error = String::new();

    while let Some(joined) = set.join_next().await {
        match joined {
            Ok((host, Ok(answer))) => {
                let remaining = set.len();
                info!(
                    "Overlay lookup: ✅ {} returned definitive answer for '{}' (early return; {} task(s) drain in background)",
                    host,
                    if answer.is_some() { "found" } else { "not found" },
                    remaining,
                );
                // Move remaining JoinSet into a spawned drain task — same pattern as submit_to_topic.
                if remaining > 0 {
                    tokio::spawn(async move {
                        let mut set = set;
                        while let Some(joined) = set.join_next().await {
                            match joined {
                                Ok((h, Ok(Some(_)))) => debug!("Overlay lookup (bg): {} also found cert", h),
                                Ok((h, Ok(None)))    => debug!("Overlay lookup (bg): {} also reports not found", h),
                                Ok((h, Err(e)))     => debug!("Overlay lookup (bg): {} error: {}", h, e),
                                Err(e)              => debug!("Overlay lookup (bg): task panic: {}", e),
                            }
                        }
                    });
                }
                return Ok(answer);
            }
            Ok((host, Err(e))) => {
                warn!("Overlay lookup: {} error: {}", host, e);
                last_error = e;
            }
            Err(e) => warn!("Overlay lookup: task panic: {}", e),
        }
    }

    Err(format!(
        "All {} overlay host(s) failed during lookup. Last error: {}",
        n_hosts,
        if last_error.is_empty() { "no hosts responded".to_string() } else { last_error },
    ))
}

/// Represents a certificate found on the overlay network.
#[derive(Debug, Clone)]
pub struct OverlayCertificateOutput {
    pub beef_bytes: Vec<u8>,
    pub output_index: usize,
    pub serial_number: Option<String>,
    pub publish_txid: Option<String>,
    pub host: String,
}

/// Query ALL overlay nodes for certificates matching a given identity key.
///
/// Returns all certificate outputs found across all nodes, deduplicated by serialNumber.
/// Used for stale certificate cleanup — need to see everything the overlay has for us.
pub async fn lookup_certificates_by_identity_key(
    identity_key_hex: &str,
    certifiers: &[&str],
) -> Result<Vec<OverlayCertificateOutput>, String> {
    let client = reqwest::Client::builder()
        .timeout(crate::services::CallClass::ThirdPartyNoFallback.timeout())
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let body = serde_json::json!({
        "service": "ls_identity",
        "query": {
            "identityKey": identity_key_hex,
            "certifiers": certifiers
        }
    });

    // Merge discovered + fallback hosts
    let discovered = discover_hosts_for_service("ls_identity").await;
    let fallbacks = fallback_hosts_for_topic(TOPIC_IDENTITY);
    let mut host_set: HashSet<String> = HashSet::new();
    for h in discovered { host_set.insert(h); }
    for h in fallbacks { host_set.insert(h.to_string()); }
    let hosts: Vec<String> = host_set.into_iter().collect();

    let mut results: Vec<OverlayCertificateOutput> = Vec::new();
    let mut seen_serials: HashSet<String> = HashSet::new();

    for host in &hosts {
        let url = format!("{}/lookup", host);
        debug!("Overlay cleanup lookup: POST {} for identityKey {}", url, &identity_key_hex[..16]);

        let response = match client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                warn!("Overlay cleanup lookup failed for {}: {}", host, e);
                continue;
            }
        };

        if response.status().as_u16() != 200 {
            warn!("Overlay cleanup lookup returned HTTP {} from {}", response.status(), host);
            continue;
        }

        let json: serde_json::Value = match response.json().await {
            Ok(j) => j,
            Err(e) => {
                warn!("Overlay cleanup lookup JSON parse failed from {}: {}", host, e);
                continue;
            }
        };

        let outputs = match json.get("outputs").and_then(|v| v.as_array()) {
            Some(o) => o,
            None => continue,
        };

        use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};

        for output in outputs {
            let output_index = output.get("outputIndex").and_then(|v| v.as_u64()).unwrap_or(0) as usize;

            // BEEF can be base64 string OR number array (SDK format)
            let beef_data = match output.get("beef") {
                Some(d) => d,
                None => continue,
            };
            let beef_bytes = if let Some(s) = beef_data.as_str() {
                match BASE64.decode(s) {
                    Ok(b) if !b.is_empty() => b,
                    _ => continue,
                }
            } else if let Some(arr) = beef_data.as_array() {
                let bytes: Vec<u8> = arr.iter().filter_map(|v| v.as_u64().map(|n| n as u8)).collect();
                if bytes.is_empty() { continue; }
                bytes
            } else {
                continue;
            };

            // Extract serialNumber and publish txid from the BEEF
            let (serial, publish_txid) = extract_cert_info_from_beef(&beef_bytes, output_index);
            let serial_str = serial.clone().unwrap_or_default();

            // Dedup by serialNumber
            if !serial_str.is_empty() && seen_serials.contains(&serial_str) {
                continue;
            }
            if !serial_str.is_empty() {
                seen_serials.insert(serial_str);
            }

            results.push(OverlayCertificateOutput {
                beef_bytes,
                output_index,
                serial_number: serial,
                publish_txid,
                host: host.clone(),
            });
        }

        info!("Overlay cleanup: found {} certificate(s) on {}", outputs.len(), host);
    }

    info!("Overlay cleanup: {} unique certificate(s) found across all nodes", results.len());
    Ok(results)
}

/// Extract the serialNumber and publish txid from a certificate embedded in BEEF.
/// Returns (serial_number, publish_txid) — either or both may be None.
fn extract_cert_info_from_beef(beef_bytes: &[u8], output_index: usize) -> (Option<String>, Option<String>) {
    let beef = match crate::beef::Beef::from_bytes(beef_bytes) {
        Ok(b) => b,
        Err(_) => return (None, None),
    };
    let tx_bytes = match beef.transactions.last() {
        Some(b) => b,
        None => return (None, None),
    };

    // Compute txid from the raw transaction bytes (double SHA-256, reversed)
    use sha2::{Sha256, Digest};
    let hash1 = Sha256::digest(tx_bytes);
    let hash2 = Sha256::digest(&hash1);
    let mut txid_bytes = hash2.to_vec();
    txid_bytes.reverse();
    let publish_txid = Some(hex::encode(&txid_bytes));

    let parsed_tx = match crate::beef::ParsedTransaction::from_bytes(tx_bytes) {
        Ok(t) => t,
        Err(_) => return (None, publish_txid),
    };
    if output_index >= parsed_tx.outputs.len() {
        return (None, publish_txid);
    }

    let script_bytes = &parsed_tx.outputs[output_index].script;
    let fields = match decode_pushdrop_fields(script_bytes) {
        Some(f) if !f.is_empty() => f,
        _ => return (None, publish_txid),
    };

    // Field 0 is the certificate JSON
    let serial = serde_json::from_slice::<serde_json::Value>(&fields[0])
        .ok()
        .and_then(|j| j.get("serialNumber").and_then(|v| v.as_str()).map(String::from));

    (serial, publish_txid)
}

// ═══════════════════════════════════════════════════════════════
// SHIP Discovery
// ═══════════════════════════════════════════════════════════════

/// Discover overlay hosts serving a specific topic via SHIP protocol.
///
/// Delegates to `state.ship_cache` for stale-while-revalidate semantics.
/// See `ship_cache::ShipDiscoveryCache` for the per-call decision tree
/// (fresh hit / stale + bg refresh / block-fetch on miss). The Monitor's
/// `task_refresh_ship_cache` keeps `tm_identity` warm so this call is
/// nearly always a synchronous cache hit during normal usage.
async fn discover_hosts_for_topic(ship_cache: &Arc<ShipDiscoveryCache>, topic: &str) -> Vec<String> {
    ship_cache.get_hosts(topic).await
}

/// Discover overlay hosts serving a specific lookup service.
/// Uses SLAP tracker discovery (same mechanism as SHIP but for lookup services).
/// For now, returns hardcoded hosts since SLAP service discovery follows the same
/// pattern and the known hosts serve both SHIP and SLAP.
async fn discover_hosts_for_service(_service: &str) -> Vec<String> {
    // SLAP discovery would query for "ls_slap" service similar to SHIP.
    // For now, the known overlay hosts serve both broadcast and lookup.
    // Future: implement full SLAP discovery here.
    Vec::new() // Empty = will use fallback
}

/// Per-tracker hard timeout for SHIP discovery (POST /lookup).
///
/// SLAP trackers should respond in well under a second; 5s leaves generous
/// headroom for the cold-DNS, cold-TLS, busy-server case while bounding worst-
/// case discovery wallclock at ~5s even when an individual tracker (notably
/// `users.bapp.dev`) is taking 90-130s. Trackers that exceed this are skipped
/// for the current call; whatever the other trackers returned still gets used.
const SHIP_TRACKER_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

/// Query SLAP trackers for SHIP advertisements matching the given topics.
///
/// SHIP discovery protocol:
/// 1. POST /lookup to each SLAP tracker (in parallel, each with `SHIP_TRACKER_TIMEOUT`)
/// 2. Request: { "service": "ls_ship", "query": { "topics": ["tm_identity"] } }
/// 3. Response: { "type": "output-list", "outputs": [{ "beef": "...", "outputIndex": N }] }
/// 4. Parse BEEF outputs to extract SHIP advertisement scripts
/// 5. Decode PushDrop scripts to get protocol/domain/topic
///
/// Returns map of host_url → Set<topic>. Trackers that fail or time out are
/// skipped silently — results from the trackers that did respond are still
/// merged and returned.
///
/// Pre-2026-05-27 this ran the trackers SEQUENTIALLY with a 240s reqwest
/// timeout, so a single slow tracker could block the whole discovery call
/// for up to 4 minutes (and 4×240s=16min worst case across all 4). Combined
/// with `ship_cache`'s blocking MISS branch, that made cold-start publish
/// take 132s+. This rewrite caps each tracker at 5s in parallel, so worst-
/// case discovery wallclock is ~5s.
pub(super) async fn query_ship_advertisements(topics: &[String]) -> HashMap<String, HashSet<String>> {
    let client = reqwest::Client::builder()
        .timeout(crate::services::CallClass::ThirdPartyNoFallback.timeout())
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());

    let query_body = Arc::new(serde_json::json!({
        "service": "ls_ship",
        "query": {
            "topics": topics
        }
    }));
    let topics_owned: Arc<Vec<String>> = Arc::new(topics.to_vec());

    // Spawn one task per tracker, each with its own hard timeout. JoinSet
    // collects results as they arrive; trackers that exceed `SHIP_TRACKER_TIMEOUT`
    // are aborted and counted as failures.
    let mut set: tokio::task::JoinSet<(&'static str, Option<HashMap<String, HashSet<String>>>)> =
        tokio::task::JoinSet::new();

    for tracker in DEFAULT_SLAP_TRACKERS {
        let tracker: &'static str = *tracker;
        let client = client.clone();
        let query_body = Arc::clone(&query_body);
        let topics_owned = Arc::clone(&topics_owned);

        set.spawn(async move {
            let url = format!("{}/lookup", tracker);
            debug!("SHIP discovery: querying {} for topics {:?}", url, topics_owned.as_slice());

            let fetch = async {
                let response = client
                    .post(&url)
                    .header("Content-Type", "application/json")
                    .json(query_body.as_ref())
                    .send()
                    .await
                    .map_err(|e| format!("HTTP error: {}", e))?;

                if response.status().as_u16() != 200 {
                    return Err(format!("HTTP {}", response.status()));
                }

                let json: serde_json::Value = response
                    .json()
                    .await
                    .map_err(|e| format!("JSON parse error: {}", e))?;

                let answer_type = json.get("type").and_then(|v| v.as_str()).unwrap_or("");
                if answer_type != "output-list" {
                    return Err(format!("unexpected type '{}'", answer_type));
                }

                let outputs = json
                    .get("outputs")
                    .and_then(|v| v.as_array())
                    .cloned()
                    .unwrap_or_default();

                info!("SHIP discovery: {} returned {} advertisement(s)", tracker, outputs.len());

                let mut tracker_hosts: HashMap<String, HashSet<String>> = HashMap::new();
                for output in &outputs {
                    if let Some((domain, topic)) = parse_ship_advertisement(output, topics_owned.as_slice()) {
                        tracker_hosts.entry(domain).or_default().insert(topic);
                    }
                }
                Ok::<_, String>(tracker_hosts)
            };

            match tokio::time::timeout(SHIP_TRACKER_TIMEOUT, fetch).await {
                Ok(Ok(hosts)) => (tracker, Some(hosts)),
                Ok(Err(e)) => {
                    warn!("SHIP discovery: {} failed: {}", tracker, e);
                    (tracker, None)
                }
                Err(_) => {
                    warn!(
                        "SHIP discovery: {} exceeded {}s tracker timeout — skipping",
                        tracker,
                        SHIP_TRACKER_TIMEOUT.as_secs()
                    );
                    (tracker, None)
                }
            }
        });
    }

    // Merge results from all trackers. Slow/failed trackers contribute nothing,
    // which is fine — the surviving trackers' results still give us a usable
    // host set. Different trackers may have different SHIP advertisements; we
    // union them (matches the SDK's tracker-merge behavior).
    let mut all_hosts: HashMap<String, HashSet<String>> = HashMap::new();
    while let Some(joined) = set.join_next().await {
        if let Ok((_tracker, Some(tracker_hosts))) = joined {
            for (domain, topic_set) in tracker_hosts {
                all_hosts.entry(domain).or_default().extend(topic_set);
            }
        }
    }

    if !all_hosts.is_empty() {
        info!("SHIP discovery: found {} host(s): {:?}",
            all_hosts.len(),
            all_hosts.keys().collect::<Vec<_>>());
    }

    all_hosts
}

/// Parse a single SHIP advertisement output from a SLAP tracker response.
///
/// SHIP advertisements are PushDrop scripts with 4 fields:
///   [0] protocol: "SHIP" or "SLAP"
///   [1] identityKey: hex-encoded pubkey
///   [2] domain: URL of the overlay host
///   [3] topicOrService: topic name (e.g., "tm_identity")
///
/// Returns Some((domain, topic)) if the advertisement matches our desired topics.
fn parse_ship_advertisement(
    output: &serde_json::Value,
    desired_topics: &[String],
) -> Option<(String, String)> {
    // The output has "beef" (base64 or hex) and "outputIndex"
    let beef_data = output.get("beef")?;
    let output_index = output.get("outputIndex").and_then(|v| v.as_u64()).unwrap_or(0) as usize;

    // Try to get BEEF bytes (could be base64 string or number array)
    let beef_bytes = if let Some(s) = beef_data.as_str() {
        // Base64 encoded
        use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
        BASE64.decode(s).ok()?
    } else if let Some(arr) = beef_data.as_array() {
        // Number array
        arr.iter().filter_map(|v| v.as_u64().map(|n| n as u8)).collect::<Vec<u8>>()
    } else {
        return None;
    };

    if beef_bytes.is_empty() {
        return None;
    }

    // Parse BEEF to extract the transaction
    let beef = crate::beef::Beef::from_bytes(&beef_bytes).ok()?;
    if beef.transactions.is_empty() {
        return None;
    }

    // Get the last transaction (the one containing the advertisement)
    let tx_bytes = beef.transactions.last()?;

    // Parse transaction to get the output script
    let parsed_tx = crate::beef::ParsedTransaction::from_bytes(tx_bytes).ok()?;
    if output_index >= parsed_tx.outputs.len() {
        return None;
    }

    let script_bytes = &parsed_tx.outputs[output_index].script;

    // Decode PushDrop fields from the script
    let fields = decode_pushdrop_fields(&script_bytes)?;
    if fields.len() < 4 {
        return None;
    }

    // Field 0: protocol ("SHIP" or "SLAP")
    let protocol = String::from_utf8(fields[0].clone()).ok()?;
    if protocol != "SHIP" {
        return None;
    }

    // Field 1: identityKey (hex pubkey — skip for now, used for verification)
    // Field 2: domain (URL of the overlay host)
    let domain = String::from_utf8(fields[2].clone()).ok()?;
    if domain.is_empty() || !domain.starts_with("http") {
        return None;
    }

    // Field 3: topic name
    let topic = String::from_utf8(fields[3].clone()).ok()?;

    // Check if this topic is one we're looking for
    if !desired_topics.iter().any(|t| t == &topic) {
        return None;
    }

    debug!("SHIP advertisement: {} serves '{}' (protocol: {})", domain, topic, protocol);
    Some((domain, topic))
}

/// Decode PushDrop fields from a locking script.
///
/// PushDrop format: <pubkey> OP_CHECKSIG <field1> <field2> ... OP_DROP ... OP_DROP
/// Extracts the pushed data fields between OP_CHECKSIG and the OP_DROPs.
fn decode_pushdrop_fields(script: &[u8]) -> Option<Vec<Vec<u8>>> {
    let mut fields = Vec::new();
    let mut i = 0;

    // Skip the initial pubkey push + OP_CHECKSIG
    // Format: OP_PUSHBYTES_33 <33 bytes> OP_CHECKSIG
    if script.len() < 35 {
        return None;
    }
    if script[0] != 0x21 { // OP_PUSHBYTES_33
        return None;
    }
    i = 34; // Skip 1 (opcode) + 33 (pubkey)
    if i >= script.len() || script[i] != 0xac { // OP_CHECKSIG
        return None;
    }
    i += 1;

    // Parse data pushes until we hit OP_DROP (0x75) or end
    while i < script.len() {
        let opcode = script[i];

        if opcode == 0x75 { // OP_DROP
            break;
        }

        // Data push opcodes
        if opcode == 0x00 {
            // OP_0
            fields.push(Vec::new());
            i += 1;
        } else if opcode <= 0x4b {
            // OP_PUSHBYTES_N (1-75 bytes)
            let len = opcode as usize;
            i += 1;
            if i + len > script.len() { return None; }
            fields.push(script[i..i + len].to_vec());
            i += len;
        } else if opcode == 0x4c {
            // OP_PUSHDATA1
            i += 1;
            if i >= script.len() { return None; }
            let len = script[i] as usize;
            i += 1;
            if i + len > script.len() { return None; }
            fields.push(script[i..i + len].to_vec());
            i += len;
        } else if opcode == 0x4d {
            // OP_PUSHDATA2
            i += 1;
            if i + 2 > script.len() { return None; }
            let len = u16::from_le_bytes([script[i], script[i + 1]]) as usize;
            i += 2;
            if i + len > script.len() { return None; }
            fields.push(script[i..i + len].to_vec());
            i += len;
        } else if opcode == 0x4e {
            // OP_PUSHDATA4
            i += 1;
            if i + 4 > script.len() { return None; }
            let len = u32::from_le_bytes([script[i], script[i + 1], script[i + 2], script[i + 3]]) as usize;
            i += 4;
            if i + len > script.len() { return None; }
            fields.push(script[i..i + len].to_vec());
            i += len;
        } else {
            // Unknown opcode — stop parsing
            break;
        }
    }

    if fields.is_empty() {
        None
    } else {
        Some(fields)
    }
}

// ═══════════════════════════════════════════════════════════════
// BEEF Submission
// ═══════════════════════════════════════════════════════════════

/// Submit BEEF to a single overlay host.
///
/// POST {host}/submit
/// Content-Type: application/octet-stream
/// X-Topics: ["tm_identity"]
/// Body: raw BEEF bytes
///
/// Returns Ok(true) if the host accepted (topic admitted outputs).
/// Returns Ok(false) if 200 but no outputs admitted.
/// Returns Err on HTTP error.
async fn submit_beef_to_host(
    client: &reqwest::Client,
    host: &str,
    beef_bytes: &[u8],
    topics_json: &str,
) -> Result<bool, String> {
    let url = format!("{}/submit", host);
    let header_hex = if beef_bytes.len() >= 8 { hex::encode(&beef_bytes[..8]) } else { hex::encode(beef_bytes) };
    info!("Overlay: POST {} ({} bytes, header: {}, topics: {})", url, beef_bytes.len(), header_hex, topics_json);

    let response = client
        .post(&url)
        .header("Content-Type", "application/octet-stream")
        .header("X-Topics", topics_json)
        .body(beef_bytes.to_vec())
        .send()
        .await
        .map_err(|e| format!("HTTP request failed: {}", e))?;

    let status = response.status().as_u16();
    let body = response.text().await.unwrap_or_default();

    if status != 200 {
        return Err(format!("HTTP {}: {}", status, body));
    }

    // Parse STEAK (Submitted Transaction Execution AcKnowledgement)
    //
    // Format: { "topic_name": { outputsToAdmit: [], coinsToRetain: [], coinsRemoved?: [] } }
    //
    // IMPORTANT: The overlay-express `onSteakReady` callback sends the HTTP response BEFORE
    // Phase 3 (storage mutation) completes. This means `coinsRemoved` may be absent even
    // for successful removals — the field is only populated in Phase 3 which runs after
    // the response is sent. This is a known limitation of the overlay-express implementation.
    //
    // Therefore:
    //   outputsToAdmit > 0  → definitive publish success
    //   coinsRemoved > 0    → definitive removal success (rare — may not arrive due to callback race)
    //   both empty           → AMBIGUOUS: could be rejection, dupe, or removal-in-progress
    //
    // Callers must verify removal separately via /lookup if confirmation is needed.
    if let Ok(steak) = serde_json::from_str::<serde_json::Value>(&body) {
        info!("Overlay: STEAK from {}: {}", host, body);

        if let Some(obj) = steak.as_object() {
            for (topic, topic_data) in obj {
                let outputs_to_admit = topic_data.get("outputsToAdmit").and_then(|v| v.as_array());
                let coins_to_retain = topic_data.get("coinsToRetain").and_then(|v| v.as_array());
                let coins_removed = topic_data.get("coinsRemoved").and_then(|v| v.as_array());

                let admitted_count = outputs_to_admit.map(|a| a.len()).unwrap_or(0);
                let removed_count = coins_removed.map(|a| a.len()).unwrap_or(0);
                let coins_removed_present = topic_data.get("coinsRemoved").is_some();

                info!("Overlay: STEAK '{}': outputsToAdmit={}, coinsToRetain={:?}, coinsRemoved={} (field present: {})",
                    topic, admitted_count,
                    coins_to_retain.map(|a| a.len()),
                    removed_count,
                    coins_removed_present,
                );

                let retained_count = coins_to_retain.map(|a| a.len()).unwrap_or(0);

                if admitted_count > 0 {
                    info!("Overlay: ✅ {} new output(s) admitted for '{}' on {}", admitted_count, topic, host);
                    return Ok(true);
                }
                if retained_count > 0 {
                    info!("Overlay: ✅ {} coin(s) retained for '{}' on {}", retained_count, topic, host);
                    return Ok(true);
                }
                if removed_count > 0 {
                    info!("Overlay: ✅ {} coin(s) explicitly removed for '{}' on {}", removed_count, topic, host);
                    return Ok(true);
                }
                // Empty response — ambiguous (rejection, dupe, or removal with early callback)
                warn!("Overlay: STEAK from {} has empty outputsToAdmit and no coinsRemoved — ambiguous (could be rejection, dupe, or removal-in-progress)", host);
            }
        }

        return Ok(false);
    }

    warn!("Overlay: Could not parse STEAK from {}: {}", host, body);
    Ok(false)
}
