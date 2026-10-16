//! UTXO Fetcher
//!
//! Fetches unspent transaction outputs from blockchain APIs.
//! Based on go-wallet/utxo_manager.go

use serde::{Deserialize, Serialize};

/// A 1-satoshi output is a token carrier, not ordinary spendable value.
///
/// 1Sat Ordinals (BRC-147) and OpNS names are carried by outputs of exactly one
/// satoshi; the asset IS the output. Spending one into a larger output permanently
/// destroys it — BRC-147: *"a general 'pay' or auto-pay grant MUST NOT authorize
/// spending them."* See
/// `development-docs/0.4.0-beta.7/track-2-1sat-ordinals/README.md` §"Two rules from
/// BRC-147 that are load-bearing for us", rule 2.
///
/// ⛔ This is a **defensive floor, not a classification system.** It cannot tell a
/// token from a stray 1-satoshi payment, and it protects nothing at 2 satoshis or
/// above. The real fix is classification on ingest: nothing currently files a token
/// output into a protective basket, which is why `output_repo`'s existing
/// `basket_id IS NULL OR b.name = 'default'` exclusion — which works — never fires
/// for one. That is beta.4 sprint 1.
///
/// ⚠️ Value is the only discriminator available at this layer. Outputs discovered by
/// address sync carry a locking script **synthesised from the address** by
/// [`generate_p2pkh_script_from_address`] in this very module, never one observed on
/// chain, so script-shape checks cannot see what such an output really is.
///
/// ⛔ Lives here, beside the `UTXO` type, because `utxo_fetcher` is declared by both
/// crate roots. `handlers` is binary-only and `recovery` compiles into both, so a
/// floor in `handlers` cannot be reached from every path that needs it.
pub const TOKEN_RESERVED_SATS: i64 = 1;

/// True when an output must never be spent as ordinary value. See [`TOKEN_RESERVED_SATS`].
pub fn is_token_reserved_value(satoshis: i64) -> bool {
    satoshis <= TOKEN_RESERVED_SATS
}

/// Check if a locking script is standard P2PKH (OP_DUP OP_HASH160 <20 bytes> OP_EQUALVERIFY OP_CHECKSIG)
/// This guards against accidentally trying to spend PushDrop tokens or other non-standard scripts.
///
/// ⚠️ B5-T1-P4: an inscription is P2PKH **plus** an envelope, so only the exact
/// 25-byte form passes. A row whose script is unchecked (`script_length` NULL) still
/// holds the address-derived script and passes regardless — for those rows the
/// value floor ([`is_token_reserved_value`]) is what excludes tokens.
pub fn is_p2pkh_script(script: &[u8]) -> bool {
    script.len() == 25
        && script[0] == 0x76  // OP_DUP
        && script[1] == 0xa9  // OP_HASH160
        && script[2] == 0x14  // Push 20 bytes
        && script[23] == 0x88 // OP_EQUALVERIFY
        && script[24] == 0xac // OP_CHECKSIG
}

/// UTXO structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UTXO {
    pub txid: String,
    pub vout: u32,
    pub satoshis: i64,
    pub script: String, // Hex-encoded locking script
    pub address_index: i32, // Which address owns this UTXO (negative = derived, -1 = master)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_instructions: Option<String>, // BRC-29 derivation info for spending derived UTXOs
    /// Whether this UTXO has been confirmed on-chain (height > 0).
    /// Unconfirmed UTXOs (mempool-only) have confirmed = false.
    #[serde(default = "default_true")]
    pub confirmed: bool,
}

fn default_true() -> bool { true }

/// WhatsOnChain API response format (single-address endpoint)
#[derive(Debug, Deserialize)]
struct WhatsOnChainUTXO {
    tx_hash: String,
    tx_pos: u32,
    value: i64,
    /// Block height. 0 or negative = unconfirmed (mempool only).
    ///
    /// MEASURED 2026-08-19: entries from the *unconfirmed* endpoints omit this
    /// field entirely (they carry `status:"unconfirmed"` + `hex` instead), so the
    /// `serde(default)` is load-bearing, not defensive — it is what makes a
    /// mempool UTXO deserialize as height 0.
    #[serde(default)]
    height: i64,
    /// WoC flags an output that a *mempool* transaction already spends.
    ///
    /// ⚠️ The confirmed endpoint keeps returning such an output as "unspent"
    /// until the spending tx confirms. Counting it overstates the balance —
    /// measured on this wallet: a 19,077,803 sat output came back
    /// `isSpentInMempoolTx: true` while our own spend of it sat in the mempool.
    /// Nothing read this field before 2026-08-19.
    #[serde(default, rename = "isSpentInMempoolTx")]
    is_spent_in_mempool_tx: bool,
}

/// WhatsOnChain /unspent/all wrapper response
#[derive(Debug, Deserialize)]
struct WhatsOnChainUnspentAllResponse {
    #[serde(default)]
    result: Vec<WhatsOnChainUTXO>,
    #[serde(default)]
    error: String,
}

/// WhatsOnChain bulk endpoint response item
/// New API uses `result` field; old API used `unspent`. We accept both.
#[derive(Debug, Deserialize)]
struct WhatsOnChainBulkItem {
    address: String,
    #[serde(default)]
    result: Vec<WhatsOnChainUTXO>,
    #[serde(default)]
    unspent: Vec<WhatsOnChainUTXO>,
    #[serde(default)]
    error: String,
}

impl WhatsOnChainBulkItem {
    /// Get UTXOs from whichever field has data (new API uses `result`, old used `unspent`)
    fn utxos(&self) -> &Vec<WhatsOnChainUTXO> {
        if !self.result.is_empty() { &self.result } else { &self.unspent }
    }
}

/// GorillaPool ordinals API response format
#[derive(Debug, Deserialize)]
struct GorillaPoolUTXO {
    txid: String,
    vout: u32,
    satoshis: i64,
    #[serde(default)]
    owner: String,
}

/// Max addresses per bulk API request (WhatsOnChain limit)
const BULK_BATCH_SIZE: usize = 20;

/// Fetch UTXOs for a Bitcoin address from blockchain APIs.
///
/// Tries WhatsOnChain first, falls back to GorillaPool ordinals API.
/// Returns Ok(vec) on success (empty vec = genuinely no UTXOs).
/// Returns Err only if ALL providers failed (API down).
pub async fn fetch_utxos_for_address(address: &str, address_index: i32) -> Result<Vec<UTXO>, String> {
    log::info!("   Fetching UTXOs for address: {}", address);

    let client = reqwest::Client::builder()
        .timeout(crate::services::CallClass::IndexerSync.timeout())
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());

    // Try WhatsOnChain first
    match fetch_utxos_woc(&client, address, address_index).await {
        Ok(utxos) => return Ok(utxos),
        Err(e) => {
            log::debug!("   WoC failed for {}: {}, trying GorillaPool...", address, e);
        }
    }

    // Fallback: GorillaPool ordinals API (different provider)
    match fetch_utxos_gorillapool(&client, address, address_index).await {
        Ok(utxos) => return Ok(utxos),
        Err(e) => {
            log::warn!("   ⚠️  All UTXO APIs failed for {}: {}", address, e);
        }
    }

    Err(format!("All UTXO API providers failed for address {}", address))
}

/// Fetch UTXOs for a single address including unconfirmed (mempool) UTXOs.
///
/// Uses WhatsOnChain single-address API which returns both confirmed and
/// unconfirmed UTXOs (unconfirmed have height=0). No GorillaPool fallback
/// since GorillaPool doesn't expose unconfirmed.
pub async fn fetch_utxos_single_address_with_unconfirmed(address: &str, address_index: i32) -> Result<Vec<UTXO>, String> {
    let client = reqwest::Client::builder()
        .timeout(crate::services::CallClass::IndexerSync.timeout())
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());
    fetch_utxos_woc(&client, address, address_index).await
}

/// Fetch UTXOs from WhatsOnChain
async fn fetch_utxos_woc(client: &reqwest::Client, address: &str, address_index: i32) -> Result<Vec<UTXO>, String> {
    let url = format!("https://api.whatsonchain.com/v1/bsv/main/address/{}/unspent/all", address);

    let response = client.get(&url).send().await
        .map_err(|e| format!("WoC request failed: {}", e))?;

    let status = response.status();
    if status.is_success() {
        // New /unspent/all endpoint returns {result: [...], error: ""}
        // Try wrapped format first, fall back to flat array
        let api_utxos: Vec<WhatsOnChainUTXO> = {
            let body = response.text().await
                .map_err(|e| format!("Failed to read WoC response: {}", e))?;
            if let Ok(wrapped) = serde_json::from_str::<WhatsOnChainUnspentAllResponse>(&body) {
                if !wrapped.error.is_empty() {
                    return Err(format!("WoC error: {}", wrapped.error));
                }
                wrapped.result
            } else if let Ok(flat) = serde_json::from_str::<Vec<WhatsOnChainUTXO>>(&body) {
                flat
            } else {
                return Err(format!("Failed to parse WoC response: {}", &body[..body.len().min(200)]));
            }
        };

        let p2pkh_script = generate_p2pkh_script_from_address(address)?;
        // Same skip as the bulk path (B5-T1-P4): an address re-fetched singly because
        // bulk hit its cap must mean what the bulk answer would have meant.
        let utxos: Vec<UTXO> = api_utxos.into_iter().filter(|u| !u.is_spent_in_mempool_tx).map(|u| {
            let confirmed = u.height > 0;
            UTXO {
                txid: u.tx_hash,
                vout: u.tx_pos,
                satoshis: u.value,
                script: p2pkh_script.clone(),
                address_index,
                custom_instructions: None,
                confirmed,
            }
        }).collect();

        let unconfirmed_count = utxos.iter().filter(|u| !u.confirmed).count();
        if unconfirmed_count > 0 {
            log::info!("   ✅ WoC: {} UTXOs ({} confirmed, {} unconfirmed, {} sats)",
                utxos.len(), utxos.len() - unconfirmed_count, unconfirmed_count,
                utxos.iter().map(|u| u.satoshis).sum::<i64>());
        } else {
            log::info!("   ✅ WoC: {} UTXOs ({} sats)", utxos.len(), utxos.iter().map(|u| u.satoshis).sum::<i64>());
        }
        Ok(utxos)
    } else {
        Err(format!("WoC returned status {}", status))
    }
}

/// Fetch UTXOs from GorillaPool ordinals API (fallback)
///
/// API: https://ordinals.gorillapool.io/api/txos/address/{address}/unspent
/// Returns: [{txid, vout, satoshis, owner, spend, ...}]
/// Only includes UTXOs where spend is empty (unspent).
async fn fetch_utxos_gorillapool(client: &reqwest::Client, address: &str, address_index: i32) -> Result<Vec<UTXO>, String> {
    let url = format!("https://ordinals.gorillapool.io/api/txos/address/{}/unspent", address);

    let response = client.get(&url).send().await
        .map_err(|e| format!("GorillaPool request failed: {}", e))?;

    let status = response.status();
    if status.is_success() {
        let api_utxos: Vec<GorillaPoolUTXO> = response.json().await
            .map_err(|e| format!("Failed to parse GorillaPool response: {}", e))?;

        let p2pkh_script = generate_p2pkh_script_from_address(address)?;
        let utxos: Vec<UTXO> = api_utxos.into_iter().map(|u| UTXO {
            txid: u.txid,
            vout: u.vout,
            satoshis: u.satoshis,
            script: p2pkh_script.clone(),
            address_index,
            custom_instructions: None,
            confirmed: true, // GorillaPool only returns confirmed UTXOs
        }).collect();

        log::info!("   ✅ GorillaPool: {} UTXOs ({} sats)", utxos.len(), utxos.iter().map(|u| u.satoshis).sum::<i64>());
        Ok(utxos)
    } else {
        Err(format!("GorillaPool returned status {}", status))
    }
}

/// Check if an address has any transaction history on-chain.
///
/// Uses WhatsOnChain's confirmed history endpoint which returns transactions
/// even if the address currently has zero balance. This distinguishes "used
/// and spent" from "never used" — critical for gap limit logic during recovery.
///
/// API (new): https://api.whatsonchain.com/v1/bsv/main/address/{address}/confirmed/history
/// API (old, deprecated): https://api.whatsonchain.com/v1/bsv/main/address/{address}/history
pub async fn address_has_history(address: &str) -> Result<bool, String> {
    let client = reqwest::Client::builder()
        .timeout(crate::services::CallClass::IndexerAsync.timeout())
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());

    // Try new WoC endpoint first: /address/{addr}/confirmed/history
    // Returns: {"address":"...","result":[{"tx_hash":"...","height":...}, ...],"error":""}
    let new_url = format!("https://api.whatsonchain.com/v1/bsv/main/address/{}/confirmed/history", address);
    match client.get(&new_url).send().await {
        Ok(resp) if resp.status().is_success() => {
            let body = resp.text().await.unwrap_or_default();
            // Parse the wrapped response — history is in the "result" array
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&body) {
                let result = json.get("result").and_then(|v| v.as_array());
                return Ok(result.map(|a| !a.is_empty()).unwrap_or(false));
            }
            // Fallback: check if body has any tx_hash references
            return Ok(body.contains("tx_hash"));
        }
        Ok(resp) if resp.status().as_u16() == 404 => {
            // New endpoint also 404 — try legacy endpoint
            log::debug!("   WoC confirmed/history returned 404 for {}, trying legacy", address);
        }
        Ok(resp) => {
            log::warn!("   ⚠️  WoC confirmed/history returned {} for {}", resp.status(), address);
        }
        Err(e) => {
            log::warn!("   ⚠️  WoC confirmed/history request failed for {}: {}", address, e);
        }
    }

    // Fallback: try legacy endpoint /address/{addr}/history
    let legacy_url = format!("https://api.whatsonchain.com/v1/bsv/main/address/{}/history", address);
    match client.get(&legacy_url).send().await {
        Ok(resp) if resp.status().is_success() => {
            let body = resp.text().await.unwrap_or_default();
            let has_history = body.trim() != "[]" && !body.trim().is_empty();
            Ok(has_history)
        }
        Ok(resp) if resp.status().as_u16() == 404 => {
            // Both endpoints 404 — address genuinely has no history
            Ok(false)
        }
        Ok(resp) => {
            Err(format!("History API returned status {}", resp.status()))
        }
        Err(e) => {
            Err(format!("History API request failed: {}", e))
        }
    }
}

/// Generate P2PKH locking script from a Bitcoin address
///
/// Decodes the address and creates: OP_DUP OP_HASH160 <pubkeyhash> OP_EQUALVERIFY OP_CHECKSIG
fn generate_p2pkh_script_from_address(address: &str) -> Result<String, String> {
    // Decode base58check address (with checksum verification)
    let decoded = bs58::decode(address)
        .with_check(None) // Verify checksum
        .into_vec()
        .map_err(|e| format!("Invalid base58 address: {}", e))?;

    // After checksum removal, we should have: 1 byte version + 20 bytes hash = 21 bytes
    if decoded.len() != 21 {
        return Err(format!("Invalid decoded address length: {} (expected 21)", decoded.len()));
    }

    // Verify it's a mainnet P2PKH address (version byte 0x00)
    if decoded[0] != 0x00 {
        return Err(format!("Not a mainnet P2PKH address (version: 0x{:02x})", decoded[0]));
    }

    // Extract pubkey hash (skip version byte, take next 20 bytes)
    let pubkey_hash = &decoded[1..21];

    // Build P2PKH script: OP_DUP OP_HASH160 <20 bytes> OP_EQUALVERIFY OP_CHECKSIG
    let mut script = Vec::new();
    script.push(0x76); // OP_DUP
    script.push(0xa9); // OP_HASH160
    script.push(0x14); // Push 20 bytes
    script.extend_from_slice(pubkey_hash);
    script.push(0x88); // OP_EQUALVERIFY
    script.push(0xac); // OP_CHECKSIG

    let script_hex = hex::encode(&script);
    log::debug!("   Generated P2PKH script for {}: {}", address, script_hex);
    log::debug!("   Pubkey hash: {}", hex::encode(pubkey_hash));

    Ok(script_hex)
}

/// WhatsOnChain bulk endpoints. **Both are required.**
///
/// ⛔ MEASURED 2026-08-19 — do not "simplify" these into the single
/// `addresses/unspent` endpoint. Despite its name it returns **confirmed only**.
/// Same address, same moment, one unconfirmed output present:
///   addresses/unspent              -> 1 entry  (confirmed only)   ❌
///   addresses/confirmed/unspent    -> 1 entry  (confirmed)        ✔
///   addresses/unconfirmed/unspent  -> 1 entry  (the mempool one)  ✔
/// The single-address `address/{a}/unspent/all` *does* include both, which is why
/// TaskSyncPending's individual tier saw a payment the bulk tier could not.
const WOC_BULK_CONFIRMED: &str =
    "https://api.whatsonchain.com/v1/bsv/main/addresses/confirmed/unspent";
const WOC_BULK_UNCONFIRMED: &str =
    "https://api.whatsonchain.com/v1/bsv/main/addresses/unconfirmed/unspent";

/// One bulk POST with retry/backoff. Returns parsed items, or None if the
/// endpoint could not be read after retries.
async fn fetch_bulk_chunk(
    client: &reqwest::Client,
    url: &str,
    body: &serde_json::Value,
    max_retries: u32,
    initial_delay_ms: u64,
) -> Option<Vec<WhatsOnChainBulkItem>> {
    for attempt in 0..=max_retries {
        let response = match client.post(url).json(body).send().await {
            Ok(resp) => resp,
            Err(e) => {
                if attempt < max_retries {
                    let delay_ms = initial_delay_ms * (1 << attempt);
                    log::warn!("   Bulk request failed (attempt {}/{}): {}. Retrying in {}ms...",
                              attempt + 1, max_retries + 1, e, delay_ms);
                    tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;
                    continue;
                }
                log::warn!("   Bulk request failed after {} attempts: {}", max_retries + 1, e);
                return None;
            }
        };

        let status = response.status();
        if status.is_success() {
            return match response.json::<Vec<WhatsOnChainBulkItem>>().await {
                Ok(items) => Some(items),
                Err(e) => {
                    log::warn!("   Failed to parse bulk response from {}: {}", url, e);
                    None
                }
            };
        } else if status.is_server_error() && attempt < max_retries {
            let delay_ms = initial_delay_ms * (1 << attempt);
            log::warn!("   Bulk server error {} (attempt {}/{}). Retrying in {}ms...",
                      status, attempt + 1, max_retries + 1, delay_ms);
            tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;
            continue;
        } else {
            log::warn!("   Bulk endpoint {} returned status {}", url, status);
            return None;
        }
    }
    None
}

/// WhatsOnChain's bulk confirmed endpoint returns **at most 20 coins per address and
/// does not page** (WoC docs, re-read at the B5-T1-P4 kickoff 2026-10-02). An address
/// that comes back with exactly this many may hold more, so it is re-fetched singly
/// and the single answer replaces the bulk one (`P4-A7`/`A8`).
pub(crate) const WOC_BULK_CONFIRMED_CAP: usize = 20;

/// The pure half of one bulk chunk: turn the confirmed (authoritative) and
/// unconfirmed (best-effort) responses into coins. Returns
/// `(coins, addresses read, addresses to re-fetch singly)`. An address whose
/// confirmed list is at the cap contributes **no** coins here — not from either
/// pass — because the single answer replaces it rather than adding to it.
fn merge_bulk_chunk(
    confirmed: Vec<WhatsOnChainBulkItem>,
    unconfirmed: Option<Vec<WhatsOnChainBulkItem>>,
    addr_to_index: &std::collections::HashMap<&str, i32>,
    addr_to_script: &std::collections::HashMap<&str, String>,
) -> (Vec<UTXO>, Vec<String>, Vec<String>) {
    let mut coins = Vec::new();
    let mut read = Vec::new();
    let mut refetch: Vec<String> = Vec::new();
    let mut mempool_spent_skipped = 0usize;
    let mut unconfirmed_added = 0usize;

    // `false` = confirmed pass (counts toward success), `true` = mempool pass.
    for (is_mempool_pass, batch) in [(false, Some(confirmed)), (true, unconfirmed)] {
        let Some(batch) = batch else { continue };
        for item in batch {
            if !item.error.is_empty() {
                log::warn!("   Bulk API error for {}: {}", item.address, item.error);
                continue;
            }
            if !is_mempool_pass {
                // Count each address ONCE — on the confirmed pass only.
                read.push(item.address.clone());
                if item.utxos().len() >= WOC_BULK_CONFIRMED_CAP {
                    log::info!("   Bulk returned {} coins for {} — re-fetching it singly (bulk caps at {})",
                               item.utxos().len(), item.address, WOC_BULK_CONFIRMED_CAP);
                    refetch.push(item.address.clone());
                }
            }
            if refetch.contains(&item.address) {
                continue;
            }
            let address_index = addr_to_index.get(item.address.as_str()).copied().unwrap_or(0);
            let script = match addr_to_script.get(item.address.as_str()) {
                Some(s) => s.clone(),
                None => continue,
            };
            for u in item.utxos() {
                // Already spent by a mempool tx — WoC still lists it as
                // unspent on the confirmed endpoint. Counting it overstates
                // the balance until the spend confirms.
                if u.is_spent_in_mempool_tx {
                    mempool_spent_skipped += 1;
                    continue;
                }
                if is_mempool_pass {
                    unconfirmed_added += 1;
                }
                coins.push(UTXO {
                    txid: u.tx_hash.clone(),
                    vout: u.tx_pos,
                    satoshis: u.value,
                    script: script.clone(),
                    address_index,
                    custom_instructions: None,
                    // Derived, never assumed: mempool entries omit `height`
                    // and default to 0.
                    confirmed: u.height > 0,
                });
            }
        }
    }

    if unconfirmed_added > 0 || mempool_spent_skipped > 0 {
        log::info!("   Bulk chunk: +{} unconfirmed UTXO(s), {} skipped as spent-in-mempool",
                  unconfirmed_added, mempool_spent_skipped);
    }
    (coins, read, refetch)
}

/// Fetch UTXOs for multiple addresses using WhatsOnChain bulk endpoints.
///
/// Chunks addresses into groups of 20 and, for each chunk, reads **both** the
/// confirmed and the unconfirmed endpoint (see the constants above for why both).
/// The confirmed read is authoritative — if it fails, the chunk falls back to
/// single-address fetching. The unconfirmed read is **best-effort**: losing it
/// costs mempool visibility for one tick, which must not trigger a 20×
/// single-address fallback storm.
///
/// Returns `(utxos, failed)` where `failed` lists the addresses that could **not**
/// be read — their absence from `utxos` means "unknown", never "empty".
async fn fetch_utxos_bulk(addresses: &[crate::json_storage::AddressInfo]) -> Result<(Vec<UTXO>, Vec<String>), String> {
    const MAX_RETRIES: u32 = 3;
    const INITIAL_DELAY_MS: u64 = 1000;

    let client = reqwest::Client::builder()
        .timeout(crate::services::CallClass::IndexerAsync.timeout())
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());
    let mut all_utxos = Vec::new();
    let mut read: std::collections::HashSet<String> = std::collections::HashSet::new();

    // Build address→index lookup
    let addr_to_index: std::collections::HashMap<&str, i32> = addresses.iter()
        .map(|a| (a.address.as_str(), a.index))
        .collect();

    // Pre-generate P2PKH scripts for each address. ⚠️ B5-T1-P4: this is the ASSUMED
    // script; callers observe the real one from the parent before writing a row.
    let mut addr_to_script: std::collections::HashMap<&str, String> = std::collections::HashMap::new();
    for addr in addresses {
        match generate_p2pkh_script_from_address(&addr.address) {
            Ok(script) => { addr_to_script.insert(&addr.address, script); }
            Err(e) => {
                log::warn!("   Failed to generate script for {}: {}", addr.address, e);
            }
        }
    }

    let chunks: Vec<&[crate::json_storage::AddressInfo]> = addresses.chunks(BULK_BATCH_SIZE).collect();

    for (chunk_idx, chunk) in chunks.iter().enumerate() {
        let addr_list: Vec<&str> = chunk.iter().map(|a| a.address.as_str()).collect();

        log::info!("   Bulk UTXO fetch: chunk {}/{} ({} addresses)",
                  chunk_idx + 1, chunks.len(), addr_list.len());

        let body = serde_json::json!({ "addresses": addr_list });

        // Confirmed read is AUTHORITATIVE — its failure drives the fallback.
        let confirmed_items =
            fetch_bulk_chunk(&client, WOC_BULK_CONFIRMED, &body, MAX_RETRIES, INITIAL_DELAY_MS).await;
        let bulk_ok = confirmed_items.is_some();

        // Unconfirmed read is BEST-EFFORT — never let it trigger the fallback.
        let unconfirmed_items = if bulk_ok {
            fetch_bulk_chunk(&client, WOC_BULK_UNCONFIRMED, &body, MAX_RETRIES, INITIAL_DELAY_MS).await
        } else {
            None  // the single-address fallback below already covers both
        };
        if bulk_ok && unconfirmed_items.is_none() {
            log::warn!("   Mempool read unavailable for this chunk — confirmed UTXOs only this tick");
        }

        if let Some(items) = confirmed_items {
            let (mut coins, chunk_read, refetch) =
                merge_bulk_chunk(items, unconfirmed_items, &addr_to_index, &addr_to_script);
            all_utxos.append(&mut coins);
            read.extend(chunk_read);
            // Twenty is not "all": the single answer replaces the bulk one.
            for address in refetch {
                let index = addr_to_index.get(address.as_str()).copied().unwrap_or(0);
                // WoC only (`unspent/all` covers mempool and flags mempool spends);
                // never GorillaPool's confirmed-only fallback (adversarial review MED 3:
                // `task_sweep_reservations` reads presence here as "unspent").
                match fetch_utxos_single_address_with_unconfirmed(&address, index).await {
                    Ok(mut utxos) => all_utxos.append(&mut utxos),
                    Err(e) => {
                        log::warn!("   Single re-fetch failed for {} (bulk hit its cap): {}", address, e);
                        read.remove(&address);
                    }
                }
            }
        }
        // Fallback: single-address fetch for this chunk if bulk failed
        if !bulk_ok {
            log::info!("   Falling back to single-address fetch for {} addresses", chunk.len());
            for (i, addr) in chunk.iter().enumerate() {
                match fetch_utxos_for_address(&addr.address, addr.index).await {
                    Ok(mut utxos) => {
                        read.insert(addr.address.clone()); // This address was successfully checked
                        all_utxos.append(&mut utxos);
                    }
                    Err(e) => log::warn!("   Failed to fetch UTXOs for {}: {}", addr.address, e),
                }
                if i < chunk.len() - 1 {
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                }
            }
        }

        // Small delay between chunks to avoid rate limiting
        if chunk_idx < chunks.len() - 1 {
            tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
        }
    }

    let failed = addresses.iter()
        .filter(|a| !read.contains(&a.address))
        .map(|a| a.address.clone())
        .collect();
    Ok((all_utxos, failed))
}

/// Fetch UTXOs for all addresses in the wallet.
///
/// Uses bulk API (POST /addresses/unspent, 20 addresses/request) for speed.
/// Falls back to single-address fetch if bulk endpoint fails.
///
/// Returns Err if NO addresses could be successfully checked (API is down).
/// Returns Ok with empty vec only if addresses were checked and genuinely have no UTXOs.
/// ⚠️ A PARTIAL failure still returns `Ok` — use [`fetch_all_utxos_detailed`] where an
/// unread address must not read as an empty one.
pub async fn fetch_all_utxos(addresses: &[crate::json_storage::AddressInfo]) -> Result<Vec<UTXO>, String> {
    fetch_all_utxos_detailed(addresses).await.map(|(utxos, _)| utxos)
}

/// [`fetch_all_utxos`], plus the addresses that could not be read (B5-T1-P4).
pub async fn fetch_all_utxos_detailed(addresses: &[crate::json_storage::AddressInfo]) -> Result<(Vec<UTXO>, Vec<String>), String> {
    let (all_utxos, failed) = fetch_utxos_bulk(addresses).await?;

    log::info!("📊 Total UTXOs across all addresses: {} ({} satoshis), {}/{} addresses checked successfully",
        all_utxos.len(),
        all_utxos.iter().map(|u| u.satoshis).sum::<i64>(),
        addresses.len() - failed.len(),
        addresses.len(),
    );

    // If no addresses were successfully checked, the API is likely down.
    // Return Err so callers don't reconcile against empty results.
    if failed.len() == addresses.len() && !addresses.is_empty() {
        return Err("UTXO API unavailable — no addresses could be checked".to_string());
    }

    Ok((all_utxos, failed))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // Network test — hits WhatsOnChain. Run with `cargo test -- --ignored`.
    async fn test_fetch_utxos_nonexistent_address() {
        // Test address with no UTXOs (Satoshi's genesis address)
        let result = fetch_utxos_for_address("1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa", 0).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 0);
    }
}

/// B5-T1-P4 `P4-A7` / `P4-A8`: twenty is not "all".
#[cfg(test)]
mod bulk_cap_tests {
    use super::*;
    use std::collections::HashMap;

    const X: &str = "1XaddrXXXXXXXXXXXXXXXXXXXXXXXXXXXX";
    const Y: &str = "1YaddrYYYYYYYYYYYYYYYYYYYYYYYYYYYY";

    fn item(address: &str, n: usize, height: i64) -> WhatsOnChainBulkItem {
        let result: Vec<serde_json::Value> = (0..n)
            .map(|i| serde_json::json!({"tx_hash": format!("{}{:04}", &address[..6], i), "tx_pos": 0, "value": 1000, "height": height}))
            .collect();
        serde_json::from_value(serde_json::json!({"address": address, "result": result, "error": ""})).unwrap()
    }

    fn maps() -> (HashMap<&'static str, i32>, HashMap<&'static str, String>) {
        (HashMap::from([(X, 1), (Y, 2)]), HashMap::from([(X, "aa".to_string()), (Y, "bb".to_string())]))
    }

    /// The step `fetch_utxos_bulk` runs after the merge: each re-fetched address's
    /// single answer is appended.
    fn with_single(mut coins: Vec<UTXO>, single: Vec<UTXO>) -> Vec<UTXO> {
        coins.extend(single);
        coins
    }

    fn single(n: usize, confirmed: bool) -> Vec<UTXO> {
        (0..n).map(|i| UTXO {
            txid: format!("1Xaddr{:04}", i), vout: 0, satoshis: 1000, script: "aa".into(),
            address_index: 1, custom_instructions: None, confirmed,
        }).collect()
    }

    /// `P4-A7`: bulk returns 20 for X ⇒ X is re-fetched and contributes NO bulk coins,
    /// from either pass, so the single answer (22) is what is used.
    #[test]
    fn p4_a7_exactly_twenty_is_refetched_and_replaced() {
        let (idx, scr) = maps();
        let (coins, read, refetch) = merge_bulk_chunk(
            vec![item(X, 20, 900_000), item(Y, 3, 900_000)],
            Some(vec![item(X, 1, 0)]),
            &idx, &scr);
        assert_eq!(refetch, vec![X.to_string()]);
        assert!(read.contains(&X.to_string()));
        assert_eq!(coins.iter().filter(|u| u.address_index == 1).count(), 0, "no bulk coins for X");
        assert_eq!(coins.iter().filter(|u| u.address_index == 2).count(), 3);

        // (ii) the single answer is the one used — its confirmation status, not bulk's.
        let all = with_single(coins.clone(), single(22, false));
        let xs: Vec<_> = all.iter().filter(|u| u.address_index == 1).collect();
        assert_eq!(xs.len(), 22);
        assert!(xs.iter().all(|u| !u.confirmed), "single's status, never bulk's");
        // (iii) a coin spent between the calls: single says 21 ⇒ 21, never 22.
        let all = with_single(coins, single(21, true));
        assert_eq!(all.iter().filter(|u| u.address_index == 1).count(), 21);
    }

    /// `P4-A8`: an address with 19 coins is NOT re-fetched — bulk stays the path.
    #[test]
    fn p4_a8_nineteen_is_not_refetched() {
        let (idx, scr) = maps();
        let (coins, _, refetch) = merge_bulk_chunk(vec![item(X, 19, 900_000)], None, &idx, &scr);
        assert!(refetch.is_empty());
        assert_eq!(coins.len(), 19);
    }

    /// An address the bulk response reports an error for is not "read".
    #[test]
    fn bulk_item_error_is_not_read() {
        let (idx, scr) = maps();
        let bad: WhatsOnChainBulkItem = serde_json::from_value(
            serde_json::json!({"address": X, "result": [], "error": "rate limited"})).unwrap();
        let (_, read, _) = merge_bulk_chunk(vec![bad, item(Y, 1, 1)], None, &idx, &scr);
        assert_eq!(read, vec![Y.to_string()]);
    }
}

/// B5-T1-P4 live (T2) rows — network, `#[ignore]`d. Run with
/// `cargo test --lib live_ -- --ignored --nocapture`.
#[cfg(test)]
mod live_tests {
    use super::*;

    /// `P4-A1` T2: the real test inscription minted 2026-10-04 to a dev-wallet address
    /// (`964eaf33…:0`, 3,048-byte script — an asset, never spend it). WhatsOnChain's
    /// address lookup does NOT list it (measured: it lists the plain change output of
    /// the same tx, not the inscription), so it reaches the wallet only through the
    /// GorillaPool fallback. That provider, real, reports it with the ASSUMED 25-byte
    /// script; the production `observe_coins` path then reads the real parent and the
    /// stored row carries the true length.
    #[tokio::test]
    #[ignore]
    async fn live_p4_a1_inscription_via_gorillapool_is_stored_with_its_true_length() {
        const ADDR: &str = "1B2QKQrpouDuEKJNmaUwmpBqnfgq2BzEc4";
        const TXID: &str = "964eaf33fd1d6dad03b561d5997ccf376a80d58a5cc1832ab611b65b5cedc83c";
        let client = reqwest::Client::new();

        let woc = fetch_utxos_woc(&client, ADDR, 459).await.expect("WoC answered");
        assert!(!woc.iter().any(|u| u.txid == TXID), "measured premise: WoC does not list the inscription");

        let gp = fetch_utxos_gorillapool(&client, ADDR, 459).await.expect("GorillaPool answered");
        let coin = gp.iter().find(|u| u.txid == TXID && u.vout == 0).expect("GorillaPool lists it").clone();
        assert_eq!(coin.script.len() / 2, 25, "the provider hands us the ASSUMED script");

        let path = std::env::temp_dir().join(format!("hodos_live_a1_{}.db", std::process::id()));
        let db = std::sync::Mutex::new(crate::database::WalletDatabase::new(path).unwrap());
        db.lock().unwrap().connection().execute(
            "INSERT INTO users (userId, identity_key, active_storage, created_at, updated_at) VALUES (1, 'k', 'local', 0, 0)",
            []).unwrap();
        let fetch = |txid: String| {
            let client = client.clone();
            async move {
                let hex = client.get(format!("https://api.whatsonchain.com/v1/bsv/main/tx/{}/hex", txid))
                    .send().await.map_err(|e| e.to_string())?.text().await.map_err(|e| e.to_string())?;
                hex::decode(hex.trim()).map_err(|e| e.to_string())
            }
        };
        let observed = crate::ingest::observe_coins(&db, &[(coin.txid.clone(), coin.vout, coin.satoshis)], fetch).await;
        {
            let d = db.lock().unwrap();
            crate::database::OutputRepository::new(d.connection())
                .upsert_received_utxo_with_confirmed(1, &coin.txid, coin.vout, coin.satoshis, &coin.script, 459, coin.confirmed,
                    crate::ingest::source_for(&observed, &coin.txid, coin.vout))
                .unwrap();
        }
        let d = db.lock().unwrap();
        let (len, inline): (Option<i64>, Option<Vec<u8>>) = d.connection().query_row(
            "SELECT script_length, locking_script FROM outputs WHERE txid = ?1 AND vout = 0", [TXID],
            |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
        println!("stored row: script_length={:?} inline={:?}", len, inline.as_ref().map(|s| s.len()));
        assert_eq!(len, Some(3_048), "the stored row carries the chain's length");
        assert!(inline.is_none(), "above the 1,024 B cap: kept in the raw parent");
    }

    /// `P4-A7` T2: a real 22-output tx to one dev-wallet address (`dc757ff9…`, 2026-10-04,
    /// 22 × 1,000 sats — our own money). Once CONFIRMED: WhatsOnChain's bulk confirmed
    /// endpoint lists only 20 (the limit is real today), and the production
    /// `fetch_all_utxos_detailed` returns all 22.
    #[tokio::test]
    #[ignore]
    async fn live_p4_a7_twenty_two_coins_are_all_returned() {
        const ADDR: &str = "1EPzirbag1sK5rMsmenbWb6EaS1qVjMoZu";
        const TXID: &str = "dc757ff96f960770ffdccdedc1084270f4e84084048f4440772c85c04f343a00";
        let client = reqwest::Client::new();
        let body = serde_json::json!({ "addresses": [ADDR] });
        let items = fetch_bulk_chunk(&client, WOC_BULK_CONFIRMED, &body, 2, 1000).await.expect("bulk answered");
        let bulk: usize = items.iter().map(|i| i.utxos().iter().filter(|u| u.tx_hash == TXID).count()).sum();
        println!("WoC bulk confirmed lists {} of the 22", bulk);
        assert_eq!(bulk, 20, "measured premise: the bulk confirmed endpoint caps at 20 (tx must be confirmed)");

        let info = crate::json_storage::AddressInfo { address: ADDR.into(), index: 460, public_key: String::new(), used: true, balance: 0 };
        let (utxos, failed) = fetch_all_utxos_detailed(&[info]).await.expect("fetch");
        let ours = utxos.iter().filter(|u| u.txid == TXID).count();
        println!("fetch_all_utxos_detailed returns {} of the 22; failed = {:?}", ours, failed);
        assert!(failed.is_empty());
        assert_eq!(ours, 22, "all 22, not the bulk's 20");
    }
}
