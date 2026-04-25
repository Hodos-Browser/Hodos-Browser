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
use std::sync::Mutex;
use std::time::{Duration, Instant};

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
const TOPIC_IDENTITY: &str = "tm_identity";

/// SHIP host cache TTL (5 minutes, matching TS SDK)
const SHIP_CACHE_TTL: Duration = Duration::from_secs(300);

/// SHIP discovery timeout per tracker
const SHIP_DISCOVERY_TIMEOUT: Duration = Duration::from_secs(5);

/// Overlay submission timeout per host (reduced from 30s since we now submit to 3+ hosts)
const SUBMIT_TIMEOUT: Duration = Duration::from_secs(15);

// ═══════════════════════════════════════════════════════════════
// SHIP Host Discovery Cache
// ═══════════════════════════════════════════════════════════════

/// Cached SHIP discovery results with TTL
struct CachedHosts {
    hosts: HashMap<String, HashSet<String>>, // host_url → Set<topic>
    discovered_at: Instant,
}

use once_cell::sync::Lazy;

/// Global SHIP host cache (topic → cached hosts)
static SHIP_CACHE: Lazy<Mutex<HashMap<String, CachedHosts>>> = Lazy::new(|| Mutex::new(HashMap::new()));

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
pub async fn submit_to_identity_overlay(beef_bytes: &[u8]) -> Result<bool, String> {
    submit_to_topic(TOPIC_IDENTITY, beef_bytes).await
}

/// Get all known overlay lookup endpoints for identity resolution.
/// Combines SHIP-discovered hosts with hardcoded fallbacks, deduplicates.
/// Returns URLs with `/lookup` path appended.
pub async fn get_identity_lookup_endpoints() -> Vec<String> {
    let discovered = discover_hosts_for_topic(TOPIC_IDENTITY).await;
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
/// Uses SHIP discovery with hardcoded fallback.
pub async fn submit_to_topic(topic: &str, beef_bytes: &[u8]) -> Result<bool, String> {
    if beef_bytes.is_empty() {
        return Err("Empty BEEF bytes — nothing to submit".to_string());
    }

    let client = reqwest::Client::builder()
        .timeout(SUBMIT_TIMEOUT)
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    // Step 1: Merge SHIP-discovered hosts with hardcoded fallbacks (SDK pattern).
    // Overlays don't auto-sync (no GASP), so we must submit to ALL known hosts.
    let discovered_hosts = discover_hosts_for_topic(topic).await;
    let fallbacks = fallback_hosts_for_topic(topic);

    let mut host_set: HashSet<String> = HashSet::new();
    for h in &discovered_hosts { host_set.insert(h.clone()); }
    for h in &fallbacks { host_set.insert(h.to_string()); }
    let hosts: Vec<String> = host_set.into_iter().collect();

    info!("Overlay: {} host(s) for '{}' ({} discovered + {} fallback, {} unique)",
        hosts.len(), topic, discovered_hosts.len(), fallbacks.len(), hosts.len());

    // Step 3: Submit to ALL hosts (overlays are idempotent, more coverage is better)
    let mut any_accepted = false;
    let mut last_error = String::new();
    let topics_json = serde_json::json!([topic]).to_string();

    for host in &hosts {
        match submit_beef_to_host(&client, host, beef_bytes, &topics_json).await {
            Ok(true) => {
                info!("Overlay: {} accepted the transaction for '{}'", host, topic);
                any_accepted = true;
            }
            Ok(false) => {
                warn!("Overlay: {} rejected the transaction for '{}'", host, topic);
            }
            Err(e) => {
                warn!("Overlay: {} error for '{}': {}", host, topic, e);
                last_error = e;
            }
        }
    }

    if any_accepted {
        Ok(true)
    } else if last_error.is_empty() {
        Ok(false) // All rejected, no errors
    } else {
        Err(format!("All overlay hosts failed for '{}'. Last error: {}", topic, last_error))
    }
}

/// Query the overlay to check if a certificate is published.
///
/// Queries by serialNumber for exact match.
/// Returns the BEEF bytes and output index if found.
pub async fn lookup_published_certificate(
    serial_number_b64: &str,
) -> Result<Option<(Vec<u8>, usize)>, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let body = serde_json::json!({
        "service": "ls_identity",
        "query": {
            "serialNumber": serial_number_b64
        }
    });

    // Merge discovered + fallback hosts for lookups (same as submit pattern)
    let discovered = discover_hosts_for_service("ls_identity").await;
    let fallbacks = fallback_hosts_for_topic(TOPIC_IDENTITY);
    let mut host_set: HashSet<String> = HashSet::new();
    for h in discovered { host_set.insert(h); }
    for h in fallbacks { host_set.insert(h.to_string()); }
    let hosts: Vec<String> = host_set.into_iter().collect();

    for host in &hosts {
        let url = format!("{}/lookup", host);
        debug!("Overlay lookup: POST {} for serialNumber {}", url, serial_number_b64);

        let response = match client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                warn!("Overlay lookup failed for {}: {}", host, e);
                continue;
            }
        };

        if response.status().as_u16() != 200 {
            warn!("Overlay lookup returned HTTP {} from {}", response.status(), host);
            continue;
        }

        let json: serde_json::Value = match response.json().await {
            Ok(j) => j,
            Err(e) => {
                warn!("Overlay lookup JSON parse failed from {}: {}", host, e);
                continue;
            }
        };

        let outputs = json.get("outputs").and_then(|v| v.as_array());
        if let Some(outputs) = outputs {
            if !outputs.is_empty() {
                if let Some(first) = outputs.first() {
                    let output_index = first.get("outputIndex").and_then(|v| v.as_u64()).unwrap_or(0) as usize;

                    // BEEF can be base64 string OR number array (SDK format)
                    use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
                    let beef_bytes = if let Some(s) = first.get("beef").and_then(|v| v.as_str()) {
                        BASE64.decode(s).ok()
                    } else if let Some(arr) = first.get("beef").and_then(|v| v.as_array()) {
                        Some(arr.iter().filter_map(|v| v.as_u64().map(|n| n as u8)).collect::<Vec<u8>>())
                    } else {
                        None
                    };

                    if let Some(beef_bytes) = beef_bytes.filter(|b| !b.is_empty()) {
                        info!("Overlay lookup: found certificate ({} bytes BEEF, outputIndex {})", beef_bytes.len(), output_index);
                        return Ok(Some((beef_bytes, output_index)));
                    } else {
                        warn!("Overlay lookup: could not decode BEEF from response");
                    }
                }
            }
        }

        debug!("Overlay lookup: certificate not found on {}", host);
        return Ok(None);
    }

    Err("All overlay hosts failed during lookup".to_string())
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
        .timeout(Duration::from_secs(15))
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
/// Queries SLAP trackers for `ls_ship` service with the given topic.
/// Parses SHIP advertisement outputs to extract host URLs.
/// Results are cached for 5 minutes.
///
/// Returns list of host URLs, or empty vec if discovery fails.
async fn discover_hosts_for_topic(topic: &str) -> Vec<String> {
    // Check cache first
    if let Ok(cache) = SHIP_CACHE.lock() {
        if let Some(cached) = cache.get(topic) {
            if cached.discovered_at.elapsed() < SHIP_CACHE_TTL {
                let hosts: Vec<String> = cached.hosts.keys().cloned().collect();
                debug!("Overlay: SHIP cache hit for '{}': {} host(s)", topic, hosts.len());
                return hosts;
            }
        }
    }

    // Query SLAP trackers
    let hosts = query_ship_advertisements(&[topic.to_string()]).await;

    // Cache results (even if empty, to avoid repeated failed queries)
    if let Ok(mut cache) = SHIP_CACHE.lock() {
        cache.insert(topic.to_string(), CachedHosts {
            hosts: hosts.clone(),
            discovered_at: Instant::now(),
        });
    }

    hosts.keys().cloned().collect()
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

/// Query SLAP trackers for SHIP advertisements matching the given topics.
///
/// SHIP discovery protocol:
/// 1. POST /lookup to each SLAP tracker
/// 2. Request: { "service": "ls_ship", "query": { "topics": ["tm_identity"] } }
/// 3. Response: { "type": "output-list", "outputs": [{ "beef": "...", "outputIndex": N }] }
/// 4. Parse BEEF outputs to extract SHIP advertisement scripts
/// 5. Decode PushDrop scripts to get protocol/domain/topic
///
/// Returns map of host_url → Set<topic>.
async fn query_ship_advertisements(topics: &[String]) -> HashMap<String, HashSet<String>> {
    let client = reqwest::Client::builder()
        .timeout(SHIP_DISCOVERY_TIMEOUT)
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());

    let query_body = serde_json::json!({
        "service": "ls_ship",
        "query": {
            "topics": topics
        }
    });

    let mut all_hosts: HashMap<String, HashSet<String>> = HashMap::new();

    for tracker in DEFAULT_SLAP_TRACKERS {
        let url = format!("{}/lookup", tracker);
        debug!("SHIP discovery: querying {} for topics {:?}", url, topics);

        let response = match client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&query_body)
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                warn!("SHIP discovery: {} failed: {}", tracker, e);
                continue;
            }
        };

        if response.status().as_u16() != 200 {
            warn!("SHIP discovery: {} returned HTTP {}", tracker, response.status());
            continue;
        }

        let json: serde_json::Value = match response.json().await {
            Ok(j) => j,
            Err(e) => {
                warn!("SHIP discovery: {} JSON parse error: {}", tracker, e);
                continue;
            }
        };

        // Validate response type
        let answer_type = json.get("type").and_then(|v| v.as_str()).unwrap_or("");
        if answer_type != "output-list" {
            warn!("SHIP discovery: {} returned unexpected type '{}'", tracker, answer_type);
            continue;
        }

        // Parse outputs — each contains a BEEF with a SHIP advertisement
        let outputs = match json.get("outputs").and_then(|v| v.as_array()) {
            Some(o) => o,
            None => {
                debug!("SHIP discovery: {} returned no outputs", tracker);
                continue;
            }
        };

        info!("SHIP discovery: {} returned {} advertisement(s)", tracker, outputs.len());

        for output in outputs {
            match parse_ship_advertisement(output, topics) {
                Some((domain, topic)) => {
                    all_hosts.entry(domain).or_default().insert(topic);
                }
                None => continue,
            }
        }

        // Query ALL trackers and merge results (matching SDK behavior).
        // Different trackers may have different SHIP advertisements.
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
