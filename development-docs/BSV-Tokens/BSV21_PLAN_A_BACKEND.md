# BSV-21 Implementation Plan A: Backend/Wallet

## Overview

This plan covers the Rust wallet backend implementation for BSV-21 token support. It can be developed and tested independently of the frontend UI.

**Goal**: Parse inscriptions, query GorillaPool API, create transfer transactions

**Developer**: Backend/Rust focused
**Dependencies**: None (can start immediately)
**Testing**: Unit tests + curl to localhost:3301

---

## Key Design Decisions

### 1. Integration with createAction (Not Separate Endpoint)

BSV-21 transfers will flow through the existing `/createAction` endpoint rather than creating a parallel `/ordinals/transfer` system. This:
- Maintains consistency with BRC-100 architecture
- Reuses existing UTXO selection, signing, and broadcasting logic
- Allows token transfers to be treated as a type of "action"

### 2. Validation Strategy: Trust GorillaPool

For Option 2 (our chosen approach), we:
- **Parse inscriptions locally** (understand what we're building/receiving)
- **Trust GorillaPool for balance validation** (they track the DAG)
- **Validate locally before sending** (check our inputs >= outputs)
- Full local validation would require running our own indexer (future Option 3)

### 3. HD Wallet Key Model

Our wallet uses single HD derivation from master key. Unlike reference implementations that use separate `ordPk` + `paymentPk`:
- We derive keys per-address from master seed
- Token UTXOs and payment UTXOs are both controlled by keys we derive
- No need for separate "ordPk" - we track which key controls each UTXO via address

### 4. Separate Module, Integrated Endpoints

- `rust-wallet/src/ordinals/` - separate module for BSV-21 specific logic
- Token operations flow through `createAction` with additional parameters
- Query endpoints (`/ordinals/tokens/{address}`) remain separate for simplicity

---

## Architecture

```
rust-wallet/src/
├── ordinals/
│   ├── mod.rs              # Module exports
│   ├── envelope.rs         # OP_IF envelope parser (CORRECTED)
│   ├── bsv21.rs            # BSV-21 JSON types and parser
│   ├── api_client.rs       # GorillaPool API client (with caching/retry)
│   ├── transaction.rs      # Transfer inscription builder
│   ├── helpers.rs          # Address/script utility functions
│   ├── error.rs            # OrdinalError, TransferValidationError
│   └── sync.rs             # Token UTXO discovery and sync
├── database/
│   ├── token_utxo_repo.rs  # NEW: Token UTXO repository
│   └── migrations.rs       # Add token_utxos table
├── handlers.rs             # Extend createAction, add query endpoints
└── main.rs                 # Register routes, add GorillaPoolClient to AppState
```

---

## Phase 0: Token UTXO Discovery

**Goal**: Detect and track token UTXOs we own (incoming transfers + existing holdings)

### Why This Phase is Critical

Before we can spend tokens, we need to know we have them. Token UTXO discovery must happen:
1. **On wallet startup**: Sync existing token holdings from GorillaPool
2. **After receiving transactions**: Parse incoming txs for inscriptions
3. **After our own transfers**: Track change outputs

### Tasks

- [ ] Add `sync_token_utxos()` function to fetch from GorillaPool and update DB
- [ ] Add inscription parsing to transaction receive flow
- [ ] Add periodic background sync (optional, configurable)
- [ ] Handle UTXO spent detection

### Implementation

```rust
// rust-wallet/src/ordinals/sync.rs

use super::api_client::{GorillaPoolClient, Bsv20TokenBalance};
use super::envelope::parse_ord_envelope;
use super::bsv21::parse_bsv21;
use crate::database::{UtxoRepository, TokenUtxoRepository};
use rusqlite::Connection;

/// Sync token UTXOs from GorillaPool for all wallet addresses
pub async fn sync_token_utxos(
    conn: &Connection,
    client: &GorillaPoolClient,
) -> Result<SyncResult, OrdinalError> {
    // Get all wallet addresses
    let addresses = AddressRepository::get_all(conn)?;

    let mut tokens_found = 0;
    let mut utxos_added = 0;
    let mut utxos_removed = 0;

    for address in &addresses {
        // Fetch token balances from GorillaPool
        let tokens = match client.get_tokens(&address.address).await {
            Ok(t) => t,
            Err(e) => {
                log::warn!("Failed to fetch tokens for {}: {}", address.address, e);
                continue;
            }
        };

        for token in tokens {
            tokens_found += 1;

            // Cache metadata if not already cached
            if TokenUtxoRepository::get_metadata(conn, &token.id)?.is_none() {
                let metadata = TokenMetadata {
                    token_id: token.id.clone(),
                    symbol: token.sym.clone(),
                    decimals: token.dec,
                    icon_origin: token.icon.clone(),
                    max_supply: None,
                };
                TokenUtxoRepository::cache_metadata(conn, &metadata)?;
            }
        }

        // Fetch UTXOs with inscription data
        let utxos = client.get_utxos_with_inscriptions(&address.address).await?;

        for utxo in utxos {
            // Check if this UTXO contains a BSV-21 inscription
            if let Some(script_hex) = &utxo.script {
                let script_bytes = hex::decode(script_hex).unwrap_or_default();

                if let Some(inscription) = parse_ord_envelope(&script_bytes) {
                    if let Some(bsv21) = parse_bsv21(&inscription) {
                        if bsv21.is_transfer() || bsv21.is_deploy() {
                            // Check if we already have this token UTXO
                            let origin = format!("{}_{}", utxo.txid, utxo.vout);
                            let token_id = bsv21.id.clone().unwrap_or(origin.clone());

                            if !TokenUtxoRepository::exists(conn, &utxo.txid, utxo.vout)? {
                                // First, ensure the base UTXO exists
                                let utxo_id = UtxoRepository::upsert(conn, &utxo)?;

                                // Then add the token metadata
                                TokenUtxoRepository::insert(
                                    conn,
                                    utxo_id,
                                    &token_id,
                                    &bsv21.amt
                                )?;
                                utxos_added += 1;
                            }
                        }
                    }
                }
            }
        }
    }

    // Mark spent UTXOs
    utxos_removed = mark_spent_token_utxos(conn, client).await?;

    Ok(SyncResult {
        tokens_found,
        utxos_added,
        utxos_removed,
    })
}

/// Check if our known token UTXOs are still unspent
async fn mark_spent_token_utxos(
    conn: &Connection,
    client: &GorillaPoolClient,
) -> Result<usize, OrdinalError> {
    let token_utxos = TokenUtxoRepository::get_all_unspent(conn)?;
    let mut removed = 0;

    for token_utxo in token_utxos {
        // Get the base UTXO
        let utxo = UtxoRepository::get_by_id(conn, token_utxo.utxo_id)?;

        // Check if still unspent via API
        let is_spent = client.check_utxo_spent(&utxo.txid, utxo.vout).await?;

        if is_spent {
            UtxoRepository::mark_spent(conn, token_utxo.utxo_id)?;
            // Token UTXO deleted via CASCADE or explicit delete
            removed += 1;
        }
    }

    Ok(removed)
}

/// Parse an incoming transaction for token inscriptions
pub fn process_incoming_transaction(
    conn: &Connection,
    tx: &Transaction,
    our_addresses: &[String],
) -> Result<Vec<TokenUtxo>, OrdinalError> {
    let mut found_tokens = Vec::new();

    for (vout, output) in tx.outputs.iter().enumerate() {
        // Check if this output is to one of our addresses
        let script_hex = hex::encode(&output.script);
        let pubkey_hash = extract_pubkey_hash(&output.script)?;
        let address = pubkey_hash_to_address(&pubkey_hash)?;

        if our_addresses.contains(&address) {
            // Check for inscription
            if let Some(inscription) = parse_ord_envelope(&output.script) {
                if let Some(bsv21) = parse_bsv21(&inscription) {
                    if bsv21.is_transfer() {
                        let token_id = bsv21.id.clone()
                            .ok_or(OrdinalError::MissingTokenId)?;

                        // Insert base UTXO
                        let utxo_id = UtxoRepository::insert(conn, &Utxo {
                            txid: tx.txid.clone(),
                            vout: vout as u32,
                            satoshis: output.satoshis,
                            script: script_hex,
                            spent: false,
                        })?;

                        // Insert token UTXO
                        TokenUtxoRepository::insert(conn, utxo_id, &token_id, &bsv21.amt)?;

                        found_tokens.push(TokenUtxo {
                            id: 0, // Will be set by DB
                            utxo_id,
                            token_id,
                            amount: bsv21.amt,
                        });
                    }
                }
            }
        }
    }

    Ok(found_tokens)
}

#[derive(Debug)]
pub struct SyncResult {
    pub tokens_found: usize,
    pub utxos_added: usize,
    pub utxos_removed: usize,
}
```

### GorillaPool API Extension

```rust
// Add to api_client.rs

impl GorillaPoolClient {
    /// Get UTXOs for an address including inscription data
    pub async fn get_utxos_with_inscriptions(
        &self,
        address: &str
    ) -> Result<Vec<UtxoWithInscription>, ApiError> {
        let url = format!("{}/txos/address/{}/unspent", self.base_url, address);
        let response = self.fetch_with_retry(&url).await?;
        serde_json::from_str(&response)
            .map_err(|e| ApiError::ParseError(e.to_string()))
    }

    /// Check if a specific UTXO has been spent
    pub async fn check_utxo_spent(&self, txid: &str, vout: u32) -> Result<bool, ApiError> {
        let url = format!("{}/txos/{}/{}", self.base_url, txid, vout);
        match self.fetch_with_retry(&url).await {
            Ok(response) => {
                let utxo: UtxoStatus = serde_json::from_str(&response)
                    .map_err(|e| ApiError::ParseError(e.to_string()))?;
                Ok(utxo.spent)
            }
            Err(ApiError::NotFound) => Ok(true), // Not found = spent or never existed
            Err(e) => Err(e),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct UtxoWithInscription {
    pub txid: String,
    pub vout: u32,
    pub satoshis: u64,
    pub script: Option<String>,
    pub height: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct UtxoStatus {
    pub spent: bool,
    pub spend_txid: Option<String>,
}
```

### Startup Integration

```rust
// In main.rs or a startup module

pub async fn on_wallet_startup(
    conn: &Connection,
    gorilla_client: &GorillaPoolClient,
) -> Result<(), OrdinalError> {
    log::info!("Starting token UTXO sync...");

    match sync_token_utxos(conn, gorilla_client).await {
        Ok(result) => {
            log::info!(
                "Token sync complete: {} tokens found, {} UTXOs added, {} removed",
                result.tokens_found, result.utxos_added, result.utxos_removed
            );
        }
        Err(e) => {
            log::warn!("Token sync failed (will retry later): {}", e);
            // Don't fail startup - tokens will sync later
        }
    }

    Ok(())
}
```

---

## Phase 1: Database Schema for Token UTXOs

**Goal**: Track which UTXOs contain BSV-21 tokens

### Tasks

- [ ] Add migration for `token_utxos` table in `src/database/migrations.rs`
- [ ] Create `src/database/token_utxo_repo.rs`
- [ ] Implement CRUD operations for token UTXOs
- [ ] Add token metadata cache table

### Schema

```sql
-- Token UTXO tracking
CREATE TABLE IF NOT EXISTS token_utxos (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    utxo_id INTEGER NOT NULL REFERENCES utxos(id) ON DELETE CASCADE,
    token_id TEXT NOT NULL,           -- Deploy txid_vout (e.g., "abc123...def_0")
    amount TEXT NOT NULL,             -- String for big numbers
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(utxo_id)
);

CREATE INDEX idx_token_utxos_token_id ON token_utxos(token_id);

-- Token metadata cache (symbol, decimals don't change after deploy)
CREATE TABLE IF NOT EXISTS token_metadata (
    token_id TEXT PRIMARY KEY,        -- Deploy txid_vout
    symbol TEXT,
    decimals INTEGER DEFAULT 0,
    icon_origin TEXT,                 -- Icon inscription origin
    max_supply TEXT,
    deploy_height INTEGER,
    cached_at TEXT DEFAULT CURRENT_TIMESTAMP
);
```

### Repository Implementation

```rust
// rust-wallet/src/database/token_utxo_repo.rs

use rusqlite::{params, Connection, Result};

#[derive(Debug, Clone)]
pub struct TokenUtxo {
    pub id: i64,
    pub utxo_id: i64,
    pub token_id: String,
    pub amount: String,
}

#[derive(Debug, Clone)]
pub struct TokenMetadata {
    pub token_id: String,
    pub symbol: Option<String>,
    pub decimals: u8,
    pub icon_origin: Option<String>,
    pub max_supply: Option<String>,
}

pub struct TokenUtxoRepository;

impl TokenUtxoRepository {
    /// Insert a new token UTXO
    pub fn insert(conn: &Connection, utxo_id: i64, token_id: &str, amount: &str) -> Result<i64> {
        conn.execute(
            "INSERT INTO token_utxos (utxo_id, token_id, amount) VALUES (?1, ?2, ?3)",
            params![utxo_id, token_id, amount],
        )?;
        Ok(conn.last_insert_rowid())
    }

    /// Get all token UTXOs for a specific token
    pub fn get_by_token_id(conn: &Connection, token_id: &str) -> Result<Vec<TokenUtxo>> {
        let mut stmt = conn.prepare(
            "SELECT tu.id, tu.utxo_id, tu.token_id, tu.amount
             FROM token_utxos tu
             JOIN utxos u ON tu.utxo_id = u.id
             WHERE tu.token_id = ?1 AND u.spent = 0"
        )?;

        let rows = stmt.query_map(params![token_id], |row| {
            Ok(TokenUtxo {
                id: row.get(0)?,
                utxo_id: row.get(1)?,
                token_id: row.get(2)?,
                amount: row.get(3)?,
            })
        })?;

        rows.collect()
    }

    /// Get all token UTXOs (all tokens) for the wallet
    pub fn get_all_unspent(conn: &Connection) -> Result<Vec<TokenUtxo>> {
        let mut stmt = conn.prepare(
            "SELECT tu.id, tu.utxo_id, tu.token_id, tu.amount
             FROM token_utxos tu
             JOIN utxos u ON tu.utxo_id = u.id
             WHERE u.spent = 0"
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(TokenUtxo {
                id: row.get(0)?,
                utxo_id: row.get(1)?,
                token_id: row.get(2)?,
                amount: row.get(3)?,
            })
        })?;

        rows.collect()
    }

    /// Mark token UTXO as spent (called when parent UTXO is spent)
    pub fn mark_spent(conn: &Connection, utxo_id: i64) -> Result<()> {
        // Token UTXO is automatically handled via CASCADE on utxos table
        // Or we can explicitly delete:
        conn.execute("DELETE FROM token_utxos WHERE utxo_id = ?1", params![utxo_id])?;
        Ok(())
    }

    /// Calculate total balance for a token
    /// NOTE: Uses Rust-side summation to handle big numbers correctly
    /// (SQLite INTEGER is limited to i64, but token amounts can exceed this)
    pub fn get_token_balance(conn: &Connection, token_id: &str) -> Result<String> {
        let utxos = Self::get_by_token_id(conn, token_id)?;

        // Sum in Rust using u128 (or BigUint for extremely large amounts)
        let total: u128 = utxos.iter()
            .filter_map(|u| u.amount.parse::<u128>().ok())
            .sum();

        Ok(total.to_string())
    }

    /// Calculate total balance using BigUint for maximum precision
    /// Use this for tokens with extremely large supplies (> 2^128)
    #[cfg(feature = "bignum")]
    pub fn get_token_balance_bignum(conn: &Connection, token_id: &str) -> Result<String> {
        use num_bigint::BigUint;

        let utxos = Self::get_by_token_id(conn, token_id)?;

        let total: BigUint = utxos.iter()
            .filter_map(|u| BigUint::parse_bytes(u.amount.as_bytes(), 10))
            .sum();

        Ok(total.to_string())
    }
}

// Token metadata cache
impl TokenUtxoRepository {
    pub fn cache_metadata(conn: &Connection, metadata: &TokenMetadata) -> Result<()> {
        conn.execute(
            "INSERT OR REPLACE INTO token_metadata
             (token_id, symbol, decimals, icon_origin, max_supply, cached_at)
             VALUES (?1, ?2, ?3, ?4, ?5, CURRENT_TIMESTAMP)",
            params![
                metadata.token_id,
                metadata.symbol,
                metadata.decimals,
                metadata.icon_origin,
                metadata.max_supply,
            ],
        )?;
        Ok(())
    }

    pub fn get_metadata(conn: &Connection, token_id: &str) -> Result<Option<TokenMetadata>> {
        let mut stmt = conn.prepare(
            "SELECT token_id, symbol, decimals, icon_origin, max_supply
             FROM token_metadata WHERE token_id = ?1"
        )?;

        let result = stmt.query_row(params![token_id], |row| {
            Ok(TokenMetadata {
                token_id: row.get(0)?,
                symbol: row.get(1)?,
                decimals: row.get::<_, i32>(2)? as u8,
                icon_origin: row.get(3)?,
                max_supply: row.get(4)?,
            })
        });

        match result {
            Ok(m) => Ok(Some(m)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }
}
```

---

## Phase 2: GorillaPool API Client (Production-Ready)

**Goal**: Query external indexer with proper error handling, caching, and retry logic

### Tasks

- [ ] Create `rust-wallet/src/ordinals/api_client.rs`
- [ ] Implement `GorillaPoolClient` struct with reqwest
- [ ] Add response caching with TTL
- [ ] Add retry logic with exponential backoff
- [ ] Add rate limiting (if required by GorillaPool)
- [ ] Handle API errors gracefully

### GorillaPool API Endpoints

```
Base URL: https://ordinals.gorillapool.io/api

GET /bsv20/{address}              → List BSV-20/21 tokens for address
GET /bsv20/id/{token_id}          → Token metadata (symbol, supply, etc.)
GET /txos/address/{address}/unspent → UTXOs including inscriptions
GET /inscriptions/origin/{origin}  → Inscription content by origin
```

**Note**: Verify these endpoints and check for:
- Rate limits (add header inspection)
- API key requirements (check docs)
- Response pagination

### Implementation

```rust
// rust-wallet/src/ordinals/api_client.rs

use reqwest::{Client, StatusCode};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::RwLock;
use std::time::{Duration, Instant};

const DEFAULT_BASE_URL: &str = "https://ordinals.gorillapool.io/api";
const CACHE_TTL_SECS: u64 = 60;  // Cache responses for 1 minute
const REQUEST_TIMEOUT_SECS: u64 = 15;
const MAX_RETRIES: u32 = 3;
const RETRY_DELAY_MS: u64 = 500;

#[derive(Debug, Clone, Deserialize)]
pub struct Bsv20TokenBalance {
    pub id: String,
    pub sym: Option<String>,
    pub amt: String,
    pub dec: u8,
    pub icon: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TokenInfo {
    pub id: String,
    pub sym: Option<String>,
    #[serde(rename = "max")]
    pub max_supply: Option<String>,
    pub dec: u8,
    pub icon: Option<String>,
    pub height: Option<u64>,
}

#[derive(Debug)]
pub enum ApiError {
    Network(String),
    RateLimited,
    NotFound,
    ServerError(u16),
    ParseError(String),
    Timeout,
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApiError::Network(e) => write!(f, "Network error: {}", e),
            ApiError::RateLimited => write!(f, "Rate limited by GorillaPool"),
            ApiError::NotFound => write!(f, "Resource not found"),
            ApiError::ServerError(code) => write!(f, "Server error: {}", code),
            ApiError::ParseError(e) => write!(f, "Parse error: {}", e),
            ApiError::Timeout => write!(f, "Request timeout"),
        }
    }
}

struct CacheEntry<T> {
    data: T,
    expires_at: Instant,
}

pub struct GorillaPoolClient {
    base_url: String,
    client: Client,
    // Simple in-memory cache
    token_cache: RwLock<HashMap<String, CacheEntry<Vec<Bsv20TokenBalance>>>>,
    metadata_cache: RwLock<HashMap<String, CacheEntry<TokenInfo>>>,
}

impl GorillaPoolClient {
    pub fn new() -> Self {
        Self::with_base_url(DEFAULT_BASE_URL.to_string())
    }

    pub fn with_base_url(base_url: String) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            base_url,
            client,
            token_cache: RwLock::new(HashMap::new()),
            metadata_cache: RwLock::new(HashMap::new()),
        }
    }

    /// Get BSV-20/21 tokens for an address
    pub async fn get_tokens(&self, address: &str) -> Result<Vec<Bsv20TokenBalance>, ApiError> {
        // Check cache first
        {
            let cache = self.token_cache.read().unwrap();
            if let Some(entry) = cache.get(address) {
                if entry.expires_at > Instant::now() {
                    return Ok(entry.data.clone());
                }
            }
        }

        // Fetch from API with retry
        let url = format!("{}/bsv20/{}", self.base_url, address);
        let response = self.fetch_with_retry(&url).await?;

        let tokens: Vec<Bsv20TokenBalance> = serde_json::from_str(&response)
            .map_err(|e| ApiError::ParseError(e.to_string()))?;

        // Update cache
        {
            let mut cache = self.token_cache.write().unwrap();
            cache.insert(address.to_string(), CacheEntry {
                data: tokens.clone(),
                expires_at: Instant::now() + Duration::from_secs(CACHE_TTL_SECS),
            });
        }

        Ok(tokens)
    }

    /// Get token metadata (cached indefinitely since it doesn't change)
    pub async fn get_token_info(&self, token_id: &str) -> Result<TokenInfo, ApiError> {
        // Check cache (no expiry for metadata)
        {
            let cache = self.metadata_cache.read().unwrap();
            if let Some(entry) = cache.get(token_id) {
                return Ok(entry.data.clone());
            }
        }

        // Fetch from API
        let url = format!("{}/bsv20/id/{}", self.base_url, token_id);
        let response = self.fetch_with_retry(&url).await?;

        let info: TokenInfo = serde_json::from_str(&response)
            .map_err(|e| ApiError::ParseError(e.to_string()))?;

        // Cache indefinitely (metadata doesn't change)
        {
            let mut cache = self.metadata_cache.write().unwrap();
            cache.insert(token_id.to_string(), CacheEntry {
                data: info.clone(),
                expires_at: Instant::now() + Duration::from_secs(86400 * 365), // 1 year
            });
        }

        Ok(info)
    }

    /// Fetch with retry and exponential backoff
    async fn fetch_with_retry(&self, url: &str) -> Result<String, ApiError> {
        let mut last_error = ApiError::Network("Unknown error".to_string());
        let mut delay = RETRY_DELAY_MS;

        for attempt in 0..MAX_RETRIES {
            if attempt > 0 {
                tokio::time::sleep(Duration::from_millis(delay)).await;
                delay *= 2; // Exponential backoff
            }

            match self.client.get(url).send().await {
                Ok(response) => {
                    match response.status() {
                        StatusCode::OK => {
                            return response.text().await
                                .map_err(|e| ApiError::Network(e.to_string()));
                        }
                        StatusCode::NOT_FOUND => {
                            return Err(ApiError::NotFound);
                        }
                        StatusCode::TOO_MANY_REQUESTS => {
                            last_error = ApiError::RateLimited;
                            // Continue to retry
                        }
                        status if status.is_server_error() => {
                            last_error = ApiError::ServerError(status.as_u16());
                            // Continue to retry
                        }
                        status => {
                            return Err(ApiError::ServerError(status.as_u16()));
                        }
                    }
                }
                Err(e) if e.is_timeout() => {
                    last_error = ApiError::Timeout;
                }
                Err(e) => {
                    last_error = ApiError::Network(e.to_string());
                }
            }
        }

        Err(last_error)
    }

    /// Clear all caches (useful for testing or manual refresh)
    pub fn clear_cache(&self) {
        self.token_cache.write().unwrap().clear();
        self.metadata_cache.write().unwrap().clear();
    }

    /// Check if GorillaPool is reachable
    pub async fn health_check(&self) -> bool {
        let url = format!("{}/health", self.base_url);
        self.client.get(&url)
            .timeout(Duration::from_secs(5))
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }
}

impl Default for GorillaPoolClient {
    fn default() -> Self {
        Self::new()
    }
}
```

### Test Cases

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_tokens_caching() {
        let client = GorillaPoolClient::new();

        // First call should hit API
        let result1 = client.get_tokens("1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa").await;

        // Second call should hit cache (would fail if API is slow/down)
        let result2 = client.get_tokens("1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa").await;

        // Both should succeed or both should fail the same way
        assert_eq!(result1.is_ok(), result2.is_ok());
    }

    #[tokio::test]
    async fn test_health_check() {
        let client = GorillaPoolClient::new();
        let healthy = client.health_check().await;
        // This test depends on GorillaPool being up
        println!("GorillaPool healthy: {}", healthy);
    }
}
```

---

## Phase 3: Inscription Envelope Parser (CORRECTED)

**Goal**: Parse OP_FALSE OP_IF envelopes from script bytes

### Envelope Format (Corrected Understanding)

The structure from `reference/js-1sat-ord/src/templates/ordP2pkh.ts:51`:
```javascript
ordAsm = `OP_0 OP_IF ${ordHex} OP_1 ${fileMediaType} OP_0 ${fileHex} OP_ENDIF`;
```

**Byte-level breakdown**:
```
00          OP_0 / OP_FALSE
63          OP_IF
03 6f7264   PUSH 3 bytes "ord" (protocol marker)
51          OP_1 (field 1 marker - content type follows)
XX [data]   PUSH content-type string
00          OP_0 (field 0 marker - content data follows)
XX [data]   PUSH content bytes
68          OP_ENDIF
[P2PKH]     Locking script (DUP HASH160 <20 bytes> EQUALVERIFY CHECKSIG)
```

**Key insight**: `OP_0` (0x00) and `OP_1` (0x51) are **field markers**, not data to be parsed. The data follows each marker as a standard push.

### Tasks

- [ ] Create `rust-wallet/src/ordinals/envelope.rs`
- [ ] Implement `Inscription` struct
- [ ] Implement `parse_ord_envelope(script: &[u8]) -> Option<Inscription>`
- [ ] Handle variable-length push data (OP_PUSHDATA1, OP_PUSHDATA2, OP_PUSHDATA4)
- [ ] **Verify against real inscription scripts from GorillaPool**
- [ ] Write unit tests with real test vectors

### Implementation (Corrected)

```rust
// rust-wallet/src/ordinals/envelope.rs

use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Inscription {
    pub content_type: Option<String>,
    pub content: Vec<u8>,
    pub fields: HashMap<u8, Vec<u8>>,  // Additional fields beyond 0 and 1
}

/// Parse an ordinals inscription envelope from a script.
///
/// Expected format: OP_0 OP_IF "ord" [field markers + data]... OP_ENDIF [locking script]
pub fn parse_ord_envelope(script: &[u8]) -> Option<Inscription> {
    let mut i = 0;

    // Find envelope start: OP_FALSE (0x00) followed by OP_IF (0x63)
    while i + 1 < script.len() {
        if script[i] == 0x00 && script[i + 1] == 0x63 {
            i += 2;
            break;
        }
        i += 1;
    }

    if i >= script.len() {
        return None; // No envelope found
    }

    // Check for "ord" marker: 0x03 (push 3 bytes) followed by "ord"
    if i + 4 > script.len() {
        return None;
    }
    if script[i] != 0x03 || &script[i+1..i+4] != b"ord" {
        return None;
    }
    i += 4;

    // Parse field/value pairs until OP_ENDIF (0x68)
    // Standard format: OP_1 <content-type> OP_0 <content>
    // Field 1 (content-type) MUST come before Field 0 (content)
    let mut fields: HashMap<u8, Vec<u8>> = HashMap::new();
    let mut content_type: Option<String> = None;
    let mut content: Vec<u8> = Vec::new();
    let mut seen_content_type = false;  // Track field ordering

    while i < script.len() && script[i] != 0x68 {
        // Read field marker (OP_0 through OP_16, or OP_1NEGATE)
        let field_num = match script[i] {
            0x00 => {
                // OP_0 = field 0 (content)
                // Content-type (field 1) should come first in standard inscriptions
                if !seen_content_type {
                    log::warn!("Field 0 (content) before field 1 (content-type) - non-standard inscription");
                    // Continue anyway - be lenient in parsing
                }
                0
            }
            0x51 => {
                // OP_1 = field 1 (content-type)
                seen_content_type = true;
                1
            }
            0x52..=0x60 => script[i] - 0x50,  // OP_2 through OP_16 (extensions)
            0x4f => 0xff,                      // OP_1NEGATE = -1 (rare)
            _ => {
                // Not a field marker - likely OP_ENDIF or end of envelope
                break;
            }
        };
        i += 1;

        if i >= script.len() {
            break;
        }

        // Parse the push data following this field marker
        let (data, consumed) = match parse_push_data(&script[i..]) {
            Some(result) => result,
            None => break,
        };
        i += consumed;

        // Store based on field number
        match field_num {
            1 => {
                // Field 1 = content type
                content_type = Some(String::from_utf8_lossy(&data).to_string());
            }
            0 => {
                // Field 0 = content data
                content = data;
            }
            n => {
                // Other fields (extensions)
                fields.insert(n, data);
            }
        }
    }

    // Validate we found OP_ENDIF
    if i >= script.len() || script[i] != 0x68 {
        log::warn!("Inscription envelope missing OP_ENDIF");
        // Continue anyway - return what we parsed
    }

    // Validate we have at least content (field 0)
    if content.is_empty() && content_type.is_none() {
        return None; // Empty inscription is invalid
    }

    Some(Inscription {
        content_type,
        content,
        fields,
    })
}

/// Parse push data from script bytes.
/// Returns (data, bytes_consumed) or None if invalid.
fn parse_push_data(script: &[u8]) -> Option<(Vec<u8>, usize)> {
    if script.is_empty() {
        return None;
    }

    let opcode = script[0];

    match opcode {
        // Direct push: 1-75 bytes
        0x01..=0x4b => {
            let len = opcode as usize;
            if script.len() < 1 + len {
                return None;
            }
            Some((script[1..1+len].to_vec(), 1 + len))
        }

        // OP_PUSHDATA1: next byte is length
        0x4c => {
            if script.len() < 2 {
                return None;
            }
            let len = script[1] as usize;
            if script.len() < 2 + len {
                return None;
            }
            Some((script[2..2+len].to_vec(), 2 + len))
        }

        // OP_PUSHDATA2: next 2 bytes are length (little-endian)
        0x4d => {
            if script.len() < 3 {
                return None;
            }
            let len = u16::from_le_bytes([script[1], script[2]]) as usize;
            if script.len() < 3 + len {
                return None;
            }
            Some((script[3..3+len].to_vec(), 3 + len))
        }

        // OP_PUSHDATA4: next 4 bytes are length (little-endian)
        0x4e => {
            if script.len() < 5 {
                return None;
            }
            let len = u32::from_le_bytes([script[1], script[2], script[3], script[4]]) as usize;
            if script.len() < 5 + len {
                return None;
            }
            Some((script[5..5+len].to_vec(), 5 + len))
        }

        // OP_0 pushes empty byte array
        0x00 => Some((vec![], 1)),

        // Not a push opcode
        _ => None,
    }
}

/// Build an inscription envelope script (for creating transfers)
pub fn build_inscription_envelope(content_type: &str, content: &[u8]) -> Vec<u8> {
    let mut script = Vec::new();

    // Envelope start
    script.push(0x00); // OP_FALSE
    script.push(0x63); // OP_IF

    // "ord" marker
    script.push(0x03); // Push 3 bytes
    script.extend_from_slice(b"ord");

    // Field 1: content type
    script.push(0x51); // OP_1
    push_data(&mut script, content_type.as_bytes());

    // Field 0: content
    script.push(0x00); // OP_0
    push_data(&mut script, content);

    // Envelope end
    script.push(0x68); // OP_ENDIF

    script
}

/// Append push data to script with correct opcode
fn push_data(script: &mut Vec<u8>, data: &[u8]) {
    let len = data.len();
    if len == 0 {
        script.push(0x00); // OP_0
    } else if len <= 0x4b {
        script.push(len as u8);
        script.extend_from_slice(data);
    } else if len <= 0xff {
        script.push(0x4c); // OP_PUSHDATA1
        script.push(len as u8);
        script.extend_from_slice(data);
    } else if len <= 0xffff {
        script.push(0x4d); // OP_PUSHDATA2
        script.extend_from_slice(&(len as u16).to_le_bytes());
        script.extend_from_slice(data);
    } else {
        script.push(0x4e); // OP_PUSHDATA4
        script.extend_from_slice(&(len as u32).to_le_bytes());
        script.extend_from_slice(data);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_inscription() {
        // Build a test inscription
        let content_type = "application/bsv-20";
        let content = r#"{"p":"bsv-20","op":"transfer","id":"abc_0","amt":"100"}"#;

        let envelope = build_inscription_envelope(content_type, content.as_bytes());

        // Parse it back
        let inscription = parse_ord_envelope(&envelope).expect("Should parse");

        assert_eq!(inscription.content_type.as_deref(), Some(content_type));
        assert_eq!(inscription.content, content.as_bytes());
    }

    #[test]
    fn test_build_envelope_format() {
        let envelope = build_inscription_envelope("text/plain", b"Hello");

        // Should start with OP_FALSE OP_IF
        assert_eq!(envelope[0], 0x00);
        assert_eq!(envelope[1], 0x63);

        // Then "ord"
        assert_eq!(envelope[2], 0x03);
        assert_eq!(&envelope[3..6], b"ord");

        // Then OP_1 (field 1)
        assert_eq!(envelope[6], 0x51);
    }

    #[test]
    fn test_no_envelope() {
        // Regular P2PKH script with no inscription
        let p2pkh = hex::decode("76a91489abcdefabbaabbaabbaabbaabbaabbaabbaabba88ac").unwrap();
        assert!(parse_ord_envelope(&p2pkh).is_none());
    }
}
```

---

## Phase 4: BSV-21 JSON Parser

**Goal**: Parse BSV-21 token data from inscription content

(This phase is mostly unchanged, but included for completeness)

### Tasks

- [ ] Create `rust-wallet/src/ordinals/bsv21.rs`
- [ ] Define `Bsv21Token` struct with serde
- [ ] Implement `parse_bsv21(inscription: &Inscription) -> Option<Bsv21Token>`
- [ ] Handle all operation types: `deploy+mint`, `transfer`, `burn`
- [ ] Write unit tests

### Implementation

```rust
// rust-wallet/src/ordinals/bsv21.rs

use serde::{Deserialize, Serialize};
use super::envelope::Inscription;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bsv21Token {
    pub p: String,                    // Protocol: "bsv-20"
    pub op: String,                   // Operation: "deploy+mint", "transfer", "burn"
    #[serde(default)]
    pub id: Option<String>,           // Token ID for transfers (txid_vout)
    pub amt: String,                  // Amount as string
    #[serde(default)]
    pub sym: Option<String>,          // Symbol (deploy only)
    #[serde(default)]
    pub dec: Option<String>,          // Decimals as string (deploy only)
    #[serde(default)]
    pub icon: Option<String>,         // Icon reference (deploy only)
}

impl Bsv21Token {
    pub fn decimals(&self) -> u8 {
        self.dec.as_ref()
            .and_then(|d| d.parse().ok())
            .unwrap_or(0)
    }

    pub fn amount_u128(&self) -> Option<u128> {
        self.amt.parse().ok()
    }

    pub fn is_deploy(&self) -> bool {
        self.op == "deploy+mint"
    }

    pub fn is_transfer(&self) -> bool {
        self.op == "transfer"
    }

    pub fn is_burn(&self) -> bool {
        self.op == "burn"
    }
}

pub fn parse_bsv21(inscription: &Inscription) -> Option<Bsv21Token> {
    match inscription.content_type.as_deref() {
        Some("application/bsv-20") => {}
        _ => return None,
    }

    serde_json::from_slice(&inscription.content).ok()
}

/// Create a transfer inscription JSON
pub fn create_transfer_json(token_id: &str, amount: &str) -> String {
    serde_json::json!({
        "p": "bsv-20",
        "op": "transfer",
        "id": token_id,
        "amt": amount
    }).to_string()
}
```

---

## Phase 5: Integration with createAction

**Goal**: Enable BSV-21 transfers through the existing createAction endpoint

### Design

Instead of a separate `/ordinals/transfer` endpoint, we extend `createAction` to handle token transfers. The request includes:
- Standard createAction fields (description, outputs, etc.)
- Optional `tokenInputs` for BSV-21 transfers

### Extended Request Types

```rust
// In handlers.rs or a new ordinals/handlers.rs

use serde::{Deserialize, Serialize};

/// Token distribution for a transfer
#[derive(Debug, Clone, Deserialize)]
pub struct TokenDistribution {
    pub address: String,
    pub amount: String,           // Raw amount (adjusted for decimals)
    #[serde(default)]
    pub omit_metadata: bool,      // Skip inscription for this output
}

/// Token input mode
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum TokenInputMode {
    #[default]
    Needed,  // Only use what's needed
    All,     // Consume all token UTXOs
}

/// Configuration for splitting token change
#[derive(Debug, Clone, Deserialize)]
pub struct TokenSplitConfig {
    pub outputs: usize,           // Number of change outputs
    pub threshold: Option<String>, // Minimum per output
    #[serde(default)]
    pub omit_metadata: bool,
}

/// Token transfer specification (added to createAction)
#[derive(Debug, Clone, Deserialize)]
pub struct TokenTransferSpec {
    pub token_id: String,
    pub distributions: Vec<TokenDistribution>,
    #[serde(default)]
    pub input_mode: TokenInputMode,
    #[serde(default)]
    pub split_config: Option<TokenSplitConfig>,
    #[serde(default)]
    pub burn: bool,
}

/// Extended CreateActionRequest with optional token transfer
#[derive(Debug, Deserialize)]
pub struct CreateActionRequest {
    // ... existing fields ...
    pub description: String,
    pub outputs: Option<Vec<OutputSpec>>,

    // NEW: Optional token transfer
    #[serde(default)]
    pub token_transfer: Option<TokenTransferSpec>,
}
```

### Transfer Flow in createAction

```rust
// Pseudocode for extended createAction handler

pub async fn create_action(
    req: web::Json<CreateActionRequest>,
    data: web::Data<AppState>,
) -> Result<HttpResponse, Error> {
    let conn = data.db.get_connection()?;

    // Handle token transfer if specified
    if let Some(token_spec) = &req.token_transfer {
        return handle_token_transfer(&conn, &data, token_spec, &req).await;
    }

    // ... existing createAction logic for regular transactions ...
}

async fn handle_token_transfer(
    conn: &Connection,
    data: &AppState,
    token_spec: &TokenTransferSpec,
    req: &CreateActionRequest,
) -> Result<HttpResponse, Error> {
    // 1. Get token UTXOs from database
    let token_utxos = TokenUtxoRepository::get_by_token_id(conn, &token_spec.token_id)?;

    if token_utxos.is_empty() {
        return Err(error::ErrorBadRequest("No token UTXOs found"));
    }

    // 2. Calculate total available
    let total_available: u128 = token_utxos.iter()
        .filter_map(|u| u.amount.parse::<u128>().ok())
        .sum();

    // 3. Calculate total needed
    let total_needed: u128 = token_spec.distributions.iter()
        .filter_map(|d| d.amount.parse::<u128>().ok())
        .sum();

    // 4. Validate
    if total_needed > total_available {
        return Err(error::ErrorBadRequest(format!(
            "Insufficient token balance. Have: {}, Need: {}",
            total_available, total_needed
        )));
    }

    // 5. Select token UTXOs based on input_mode
    let selected_token_utxos = match token_spec.input_mode {
        TokenInputMode::All => token_utxos,
        TokenInputMode::Needed => select_token_utxos(&token_utxos, total_needed),
    };

    // 6. Select payment UTXOs for fees
    let payment_utxos = select_payment_utxos(conn, estimated_fee)?;

    // 7. Build transaction with inscription outputs
    let tx = build_token_transfer_tx(
        &selected_token_utxos,
        &payment_utxos,
        token_spec,
        total_available - total_needed, // change amount
    )?;

    // 8. Sign and broadcast
    let signed_tx = sign_transaction(&conn, &data, tx)?;
    let txid = broadcast_transaction(&data, &signed_tx).await?;

    // 9. Update database
    // - Mark spent UTXOs
    // - Add new token UTXOs for change outputs
    // - Add new token UTXOs for recipient if same wallet

    Ok(HttpResponse::Ok().json(CreateActionResponse {
        txid,
        raw_tx: hex::encode(signed_tx.serialize()),
    }))
}

/// Select minimum token UTXOs needed to cover amount
fn select_token_utxos(utxos: &[TokenUtxo], needed: u128) -> Vec<TokenUtxo> {
    let mut selected = Vec::new();
    let mut total = 0u128;

    // Sort by amount descending (use largest first)
    let mut sorted: Vec<_> = utxos.iter().collect();
    sorted.sort_by(|a, b| {
        let a_amt: u128 = a.amount.parse().unwrap_or(0);
        let b_amt: u128 = b.amount.parse().unwrap_or(0);
        b_amt.cmp(&a_amt)
    });

    for utxo in sorted {
        if total >= needed {
            break;
        }
        let amt: u128 = utxo.amount.parse().unwrap_or(0);
        total += amt;
        selected.push(utxo.clone());
    }

    selected
}
```

### Transaction Builder

```rust
// rust-wallet/src/ordinals/transaction.rs

use super::envelope::build_inscription_envelope;
use super::bsv21::create_transfer_json;

/// Build a complete token transfer transaction
pub fn build_token_transfer_tx(
    token_utxos: &[TokenUtxo],
    payment_utxos: &[Utxo],
    spec: &TokenTransferSpec,
    change_amount: u128,
) -> Result<Transaction, Error> {
    let mut tx = Transaction::new();

    // Add token inputs first
    for token_utxo in token_utxos {
        tx.add_input(/* token UTXO */);
    }

    // Add payment inputs
    for payment_utxo in payment_utxos {
        tx.add_input(/* payment UTXO */);
    }

    // Add recipient outputs with inscriptions
    for dist in &spec.distributions {
        let script = if dist.omit_metadata {
            // Plain P2PKH (for privacy or efficiency)
            build_p2pkh_script(&dist.address)?
        } else {
            // P2PKH with inscription envelope
            let json = create_transfer_json(&spec.token_id, &dist.amount);
            let envelope = build_inscription_envelope("application/bsv-20", json.as_bytes());
            build_inscribed_p2pkh_script(&dist.address, &envelope)?
        };

        tx.add_output(TransactionOutput {
            satoshis: 1, // 1 sat for ordinals
            script,
        });
    }

    // Add token change output(s) if not burning
    if change_amount > 0 && !spec.burn {
        add_token_change_outputs(&mut tx, spec, change_amount)?;
    }

    // Add payment change output
    // ... standard change calculation ...

    Ok(tx)
}

fn build_inscribed_p2pkh_script(address: &str, envelope: &[u8]) -> Result<Vec<u8>, Error> {
    let pubkey_hash = address_to_pubkey_hash(address)?;

    let mut script = envelope.to_vec();

    // Append P2PKH
    script.push(0x76); // OP_DUP
    script.push(0xa9); // OP_HASH160
    script.push(0x14); // Push 20 bytes
    script.extend_from_slice(&pubkey_hash);
    script.push(0x88); // OP_EQUALVERIFY
    script.push(0xac); // OP_CHECKSIG

    Ok(script)
}

fn add_token_change_outputs(
    tx: &mut Transaction,
    spec: &TokenTransferSpec,
    change_amount: u128,
) -> Result<(), Error> {
    let num_outputs = spec.split_config
        .as_ref()
        .map(|c| c.outputs)
        .unwrap_or(1);

    let omit_metadata = spec.split_config
        .as_ref()
        .map(|c| c.omit_metadata)
        .unwrap_or(false);

    let amount_per_output = change_amount / num_outputs as u128;
    let mut remaining = change_amount;

    for i in 0..num_outputs {
        let output_amount = if i == num_outputs - 1 {
            remaining // Last output gets remainder
        } else {
            amount_per_output
        };
        remaining -= output_amount;

        // Build change output to our own address
        let change_address = get_change_address()?;

        let script = if omit_metadata {
            build_p2pkh_script(&change_address)?
        } else {
            let json = create_transfer_json(&spec.token_id, &output_amount.to_string());
            let envelope = build_inscription_envelope("application/bsv-20", json.as_bytes());
            build_inscribed_p2pkh_script(&change_address, &envelope)?
        };

        tx.add_output(TransactionOutput {
            satoshis: 1,
            script,
        });
    }

    Ok(())
}
```

### Transaction Builder Helper Functions

The following helper functions are referenced in the transaction builder and need implementation:

```rust
// rust-wallet/src/ordinals/helpers.rs

use bs58;
use sha2::{Sha256, Digest};
use ripemd::Ripemd160;

/// Convert a Bitcoin address to its pubkey hash (20 bytes)
pub fn address_to_pubkey_hash(address: &str) -> Result<[u8; 20], OrdinalError> {
    // Decode Base58Check
    let decoded = bs58::decode(address)
        .with_check(None)
        .into_vec()
        .map_err(|_| OrdinalError::InvalidAddress(address.to_string()))?;

    // First byte is version, remaining 20 bytes are pubkey hash
    if decoded.len() != 21 {
        return Err(OrdinalError::InvalidAddress(
            format!("Invalid address length: expected 21, got {}", decoded.len())
        ));
    }

    let mut pubkey_hash = [0u8; 20];
    pubkey_hash.copy_from_slice(&decoded[1..21]);
    Ok(pubkey_hash)
}

/// Convert a pubkey hash to a Bitcoin address
pub fn pubkey_hash_to_address(pubkey_hash: &[u8; 20]) -> Result<String, OrdinalError> {
    // Mainnet version byte is 0x00
    let mut versioned = vec![0x00];
    versioned.extend_from_slice(pubkey_hash);

    // Add checksum (first 4 bytes of double SHA256)
    let checksum = &Sha256::digest(&Sha256::digest(&versioned))[..4];
    versioned.extend_from_slice(checksum);

    Ok(bs58::encode(versioned).into_string())
}

/// Extract pubkey hash from a P2PKH script (with or without inscription prefix)
pub fn extract_pubkey_hash(script: &[u8]) -> Result<[u8; 20], OrdinalError> {
    // P2PKH pattern: OP_DUP (76) OP_HASH160 (a9) 14 <20 bytes> OP_EQUALVERIFY (88) OP_CHECKSIG (ac)
    // With inscription prefix: ... 68 (OP_ENDIF) 76 a9 14 <20 bytes> 88 ac

    // Search for the P2PKH pattern
    let p2pkh_pattern = &[0x76, 0xa9, 0x14]; // OP_DUP OP_HASH160 PUSH20

    for i in 0..script.len().saturating_sub(24) {
        if &script[i..i+3] == p2pkh_pattern {
            // Verify the suffix
            if i + 25 <= script.len()
                && script[i + 23] == 0x88  // OP_EQUALVERIFY
                && script[i + 24] == 0xac  // OP_CHECKSIG
            {
                let mut pubkey_hash = [0u8; 20];
                pubkey_hash.copy_from_slice(&script[i+3..i+23]);
                return Ok(pubkey_hash);
            }
        }
    }

    Err(OrdinalError::InvalidScript("No P2PKH pattern found".to_string()))
}

/// Build a standard P2PKH script from an address
pub fn build_p2pkh_script(address: &str) -> Result<Vec<u8>, OrdinalError> {
    let pubkey_hash = address_to_pubkey_hash(address)?;

    let mut script = Vec::with_capacity(25);
    script.push(0x76); // OP_DUP
    script.push(0xa9); // OP_HASH160
    script.push(0x14); // Push 20 bytes
    script.extend_from_slice(&pubkey_hash);
    script.push(0x88); // OP_EQUALVERIFY
    script.push(0xac); // OP_CHECKSIG

    Ok(script)
}

/// Get the wallet's change address (derived from HD key)
/// This should integrate with the existing address derivation system
pub fn get_change_address(conn: &Connection) -> Result<String, OrdinalError> {
    // Get the first derived address, or create a new one
    // This integrates with existing AddressRepository
    use crate::database::AddressRepository;

    let addresses = AddressRepository::get_all(conn)
        .map_err(|e| OrdinalError::Database(e.to_string()))?;

    if let Some(addr) = addresses.first() {
        Ok(addr.address.clone())
    } else {
        // Derive a new address
        // This should call the existing key derivation logic
        Err(OrdinalError::NoAddressAvailable)
    }
}

/// Select payment UTXOs to cover the required satoshis
pub fn select_payment_utxos(
    conn: &Connection,
    required_sats: u64,
) -> Result<Vec<Utxo>, OrdinalError> {
    use crate::database::UtxoRepository;

    let available = UtxoRepository::get_unspent(conn)
        .map_err(|e| OrdinalError::Database(e.to_string()))?;

    // Exclude token UTXOs (they have exactly 1 sat and are in token_utxos table)
    let token_utxo_ids: std::collections::HashSet<i64> =
        TokenUtxoRepository::get_all_unspent(conn)
            .map_err(|e| OrdinalError::Database(e.to_string()))?
            .into_iter()
            .map(|tu| tu.utxo_id)
            .collect();

    let payment_utxos: Vec<_> = available
        .into_iter()
        .filter(|u| !token_utxo_ids.contains(&u.id))
        .collect();

    // Simple coin selection: use largest UTXOs first
    let mut sorted = payment_utxos;
    sorted.sort_by(|a, b| b.satoshis.cmp(&a.satoshis));

    let mut selected = Vec::new();
    let mut total = 0u64;

    for utxo in sorted {
        if total >= required_sats {
            break;
        }
        total += utxo.satoshis;
        selected.push(utxo);
    }

    if total < required_sats {
        return Err(OrdinalError::InsufficientFunds {
            available: total,
            required: required_sats,
        });
    }

    Ok(selected)
}

/// Estimate the fee for a token transfer transaction
pub fn estimate_token_transfer_fee(
    num_token_inputs: usize,
    num_payment_inputs: usize,
    num_outputs: usize,
    sat_per_kb: u64,
) -> u64 {
    // Rough size estimation:
    // - 10 bytes overhead (version, locktime, input/output counts)
    // - Per input: ~148 bytes (outpoint + scriptsig + sequence)
    // - Per P2PKH output: ~34 bytes
    // - Per inscribed output: ~34 + inscription size (~100-200 bytes for BSV-21)

    let input_size = 148 * (num_token_inputs + num_payment_inputs);
    let output_size = 34 * num_outputs + 150 * num_outputs; // Assume inscriptions ~150 bytes
    let tx_size = 10 + input_size + output_size;

    // Fee = size_in_kb * sat_per_kb
    let fee = (tx_size as u64 * sat_per_kb) / 1000;
    fee.max(1) // Minimum 1 sat fee
}
```

---

## Phase 5.5: Error Type Definitions

**Goal**: Define comprehensive error types for ordinals operations

```rust
// rust-wallet/src/ordinals/error.rs

use thiserror::Error;

/// Errors that can occur during ordinals/BSV-21 operations
#[derive(Debug, Error)]
pub enum OrdinalError {
    // === API Errors ===
    #[error("GorillaPool API error: {0}")]
    ApiError(String),

    #[error("GorillaPool rate limited, retry after {retry_after_secs:?} seconds")]
    RateLimited { retry_after_secs: Option<u64> },

    #[error("GorillaPool unavailable")]
    ApiUnavailable,

    // === Validation Errors ===
    #[error("Invalid address: {0}")]
    InvalidAddress(String),

    #[error("Invalid script: {0}")]
    InvalidScript(String),

    #[error("Invalid inscription: {0}")]
    InvalidInscription(String),

    #[error("Invalid BSV-21 JSON: {0}")]
    InvalidBsv21Json(String),

    #[error("Missing token ID in transfer inscription")]
    MissingTokenId,

    #[error("Invalid token amount: {0}")]
    InvalidAmount(String),

    // === Transfer Errors ===
    #[error("Insufficient token balance: have {available}, need {required}")]
    InsufficientTokenBalance { available: String, required: String },

    #[error("Insufficient funds: have {available} sats, need {required} sats")]
    InsufficientFunds { available: u64, required: u64 },

    #[error("No token UTXOs found for token {0}")]
    NoTokenUtxos(String),

    #[error("No payment UTXOs available")]
    NoPaymentUtxos,

    #[error("No address available for change output")]
    NoAddressAvailable,

    // === Database Errors ===
    #[error("Database error: {0}")]
    Database(String),

    // === Transaction Errors ===
    #[error("Transaction signing failed: {0}")]
    SigningFailed(String),

    #[error("Transaction broadcast failed: {0}")]
    BroadcastFailed(String),

    // === Parse Errors ===
    #[error("Script parse error at offset {offset}: {message}")]
    ScriptParseError { offset: usize, message: String },

    #[error("Envelope not found in script")]
    EnvelopeNotFound,

    // === Internal Errors ===
    #[error("Internal error: {0}")]
    Internal(String),
}

impl From<rusqlite::Error> for OrdinalError {
    fn from(err: rusqlite::Error) -> Self {
        OrdinalError::Database(err.to_string())
    }
}

impl From<serde_json::Error> for OrdinalError {
    fn from(err: serde_json::Error) -> Self {
        OrdinalError::InvalidBsv21Json(err.to_string())
    }
}

impl From<hex::FromHexError> for OrdinalError {
    fn from(err: hex::FromHexError) -> Self {
        OrdinalError::InvalidScript(format!("Invalid hex: {}", err))
    }
}

/// Errors specific to transfer validation (used in validate_token_transfer)
#[derive(Debug, Error)]
pub enum TransferValidationError {
    #[error("Insufficient token balance: have {available}, need {requested}")]
    InsufficientBalance { available: String, requested: String },

    #[error("Invalid amount format: {0}")]
    InvalidAmount(String),

    #[error("Zero amount transfers are not allowed")]
    ZeroAmount,

    #[error("Token ID mismatch: expected {expected}, got {actual}")]
    TokenIdMismatch { expected: String, actual: String },

    #[error("No distributions specified")]
    EmptyDistributions,
}
```

### Using Error Types

Update the handler code to use these errors:

```rust
// In handlers.rs

use crate::ordinals::error::{OrdinalError, TransferValidationError};

async fn handle_token_transfer(
    conn: &Connection,
    data: &AppState,
    token_spec: &TokenTransferSpec,
    req: &CreateActionRequest,
) -> Result<HttpResponse, actix_web::Error> {
    // Validation
    let token_utxos = TokenUtxoRepository::get_by_token_id(conn, &token_spec.token_id)
        .map_err(|e| actix_web::error::ErrorInternalServerError(OrdinalError::from(e)))?;

    if token_utxos.is_empty() {
        return Err(actix_web::error::ErrorBadRequest(
            OrdinalError::NoTokenUtxos(token_spec.token_id.clone())
        ));
    }

    validate_token_transfer(token_spec, &token_utxos)
        .map_err(|e| actix_web::error::ErrorBadRequest(e))?;

    // ... rest of implementation
}

fn validate_token_transfer(
    spec: &TokenTransferSpec,
    available_utxos: &[TokenUtxo],
) -> Result<(), TransferValidationError> {
    if spec.distributions.is_empty() {
        return Err(TransferValidationError::EmptyDistributions);
    }

    let total_available: u128 = available_utxos.iter()
        .filter_map(|u| u.amount.parse::<u128>().ok())
        .sum();

    let total_outputs: u128 = spec.distributions.iter()
        .map(|d| {
            d.amount.parse::<u128>()
                .map_err(|_| TransferValidationError::InvalidAmount(d.amount.clone()))
        })
        .collect::<Result<Vec<_>, _>>()?
        .iter()
        .sum();

    if spec.distributions.iter().any(|d| d.amount == "0") {
        return Err(TransferValidationError::ZeroAmount);
    }

    if total_outputs > total_available {
        return Err(TransferValidationError::InsufficientBalance {
            available: total_available.to_string(),
            requested: total_outputs.to_string(),
        });
    }

    Ok(())
}
```

---

## Phase 6: Query Endpoints

**Goal**: Expose token queries (separate from createAction)

### Tasks

- [ ] Add `GET /ordinals/tokens/{address}` - list tokens (via GorillaPool)
- [ ] Add `GET /ordinals/token/{id}` - token info
- [ ] Add `GET /ordinals/balance` - local token balances from database
- [ ] Register routes in main.rs

### Implementation

```rust
// In handlers.rs

/// GET /ordinals/tokens/{address} - Query GorillaPool for token balances
pub async fn get_ordinals_tokens(
    path: web::Path<String>,
    data: web::Data<AppState>,
) -> Result<HttpResponse, actix_web::Error> {
    let address = path.into_inner();

    let tokens = data.gorilla_client
        .get_tokens(&address)
        .await
        .map_err(|e| actix_web::error::ErrorBadGateway(e.to_string()))?;

    Ok(HttpResponse::Ok().json(tokens))
}

/// GET /ordinals/balance - Get local token balances from our database
pub async fn get_local_token_balances(
    data: web::Data<AppState>,
) -> Result<HttpResponse, actix_web::Error> {
    let conn = data.db.get_connection()
        .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?;

    let token_utxos = TokenUtxoRepository::get_all_unspent(&conn)
        .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?;

    // Group by token_id and sum amounts
    let mut balances: HashMap<String, u128> = HashMap::new();
    for utxo in token_utxos {
        let amount: u128 = utxo.amount.parse().unwrap_or(0);
        *balances.entry(utxo.token_id).or_insert(0) += amount;
    }

    // Enrich with metadata
    let mut result = Vec::new();
    for (token_id, balance) in balances {
        let metadata = TokenUtxoRepository::get_metadata(&conn, &token_id)
            .ok()
            .flatten();

        result.push(serde_json::json!({
            "token_id": token_id,
            "balance": balance.to_string(),
            "symbol": metadata.as_ref().and_then(|m| m.symbol.clone()),
            "decimals": metadata.as_ref().map(|m| m.decimals).unwrap_or(0),
        }));
    }

    Ok(HttpResponse::Ok().json(result))
}
```

---

## Error Handling

### GorillaPool Unavailable

```rust
pub async fn get_ordinals_tokens_with_fallback(
    address: &str,
    gorilla_client: &GorillaPoolClient,
    db: &Connection,
) -> Result<TokensResponse, Error> {
    // Try GorillaPool first
    match gorilla_client.get_tokens(address).await {
        Ok(tokens) => {
            // Update local cache
            update_local_token_cache(db, address, &tokens)?;
            Ok(TokensResponse {
                tokens,
                source: "gorillapool",
                cached: false,
            })
        }
        Err(e) => {
            // Fallback to local cache
            log::warn!("GorillaPool unavailable: {}, using cache", e);
            let cached = get_cached_tokens(db, address)?;
            Ok(TokensResponse {
                tokens: cached.tokens,
                source: "cache",
                cached: true,
                cache_age_secs: cached.age_secs,
            })
        }
    }
}
```

### Invalid Transfer Validation

```rust
pub fn validate_token_transfer(
    spec: &TokenTransferSpec,
    available_utxos: &[TokenUtxo],
) -> Result<(), TransferValidationError> {
    let total_available: u128 = available_utxos.iter()
        .filter_map(|u| u.amount.parse::<u128>().ok())
        .sum();

    let total_outputs: u128 = spec.distributions.iter()
        .filter_map(|d| d.amount.parse::<u128>().ok())
        .sum();

    // Check for over-transfer
    if total_outputs > total_available {
        return Err(TransferValidationError::InsufficientBalance {
            available: total_available.to_string(),
            requested: total_outputs.to_string(),
        });
    }

    // Check for under-transfer (warn, don't fail)
    let change = total_available - total_outputs;
    if change > 0 && spec.burn {
        log::info!("Burning {} tokens as requested", change);
    } else if change > 0 && spec.split_config.is_none() {
        log::warn!("Under-transfer: {} tokens will go to single change output", change);
    }

    // Check individual amounts are positive
    for dist in &spec.distributions {
        let amt: u128 = dist.amount.parse()
            .map_err(|_| TransferValidationError::InvalidAmount(dist.amount.clone()))?;
        if amt == 0 {
            return Err(TransferValidationError::ZeroAmount);
        }
    }

    Ok(())
}
```

---

## Dependencies (Cargo.toml)

Add these dependencies to `rust-wallet/Cargo.toml`:

```toml
[dependencies]
# Existing deps...

# BSV-21/Ordinals specific
num-bigint = "0.4"      # BigUint for large token amounts
bs58 = "0.5"            # Base58 encoding/decoding for addresses
ripemd = "0.1"          # RIPEMD160 for pubkey hash (if not already via another dep)
thiserror = "1.0"       # Error derive macro

# These are likely already present:
# reqwest = { version = "0.11", features = ["json"] }
# tokio = { version = "1", features = ["full"] }
# serde = { version = "1", features = ["derive"] }
# serde_json = "1"
# hex = "0.4"
# sha2 = "0.10"
# log = "0.4"

[features]
# Optional: enable BigUint for extremely large token supplies
bignum = []
```

---

## Files to Create/Modify

| File | Action | Description |
|------|--------|-------------|
| `rust-wallet/src/ordinals/mod.rs` | Create | Module exports |
| `rust-wallet/src/ordinals/envelope.rs` | Create | OP_IF parser (corrected) |
| `rust-wallet/src/ordinals/bsv21.rs` | Create | BSV-21 types and parser |
| `rust-wallet/src/ordinals/api_client.rs` | Create | GorillaPool client (with cache/retry) |
| `rust-wallet/src/ordinals/transaction.rs` | Create | Transfer builder |
| `rust-wallet/src/ordinals/helpers.rs` | Create | Address/script helper functions |
| `rust-wallet/src/ordinals/error.rs` | Create | Error type definitions |
| `rust-wallet/src/ordinals/sync.rs` | Create | Token UTXO discovery and sync |
| `rust-wallet/src/database/token_utxo_repo.rs` | Create | Token UTXO repository |
| `rust-wallet/src/database/migrations.rs` | Modify | Add token tables |
| `rust-wallet/src/handlers.rs` | Modify | Extend createAction, add query endpoints |
| `rust-wallet/src/main.rs` | Modify | Register routes, add GorillaPoolClient to AppState, startup sync |
| `rust-wallet/Cargo.toml` | Modify | Add dependencies |

---

## Testing Strategy

### Unit Tests

```bash
cd rust-wallet
cargo test ordinals
```

### Integration Tests

```bash
# Start wallet server
cargo run --release

# Test token query (via GorillaPool)
curl http://localhost:3301/ordinals/tokens/1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa

# Test local balance
curl http://localhost:3301/ordinals/balance

# Test token transfer via createAction
curl -X POST http://localhost:3301/createAction \
  -H "Content-Type: application/json" \
  -d '{
    "description": "Transfer TEST tokens",
    "token_transfer": {
      "token_id": "abc123...def_0",
      "distributions": [
        {"address": "1RecipientAddress...", "amount": "1000000"}
      ]
    }
  }'
```

### Test with Real Data

1. Query GorillaPool for addresses known to have tokens
2. Parse real inscription scripts
3. Create test token on mainnet (costs ~1 sat + fees)

---

## Success Criteria

- [ ] Can query tokens for any address via `/ordinals/tokens/{address}`
- [ ] Can parse inscription envelopes from raw scripts (verified against real data)
- [ ] Token UTXOs tracked in database with correct balances
- [ ] Can create valid BSV-21 transfer transactions via createAction
- [ ] GorillaPool failures fall back to cached data gracefully
- [ ] All unit tests pass
- [ ] Integration tests pass

---

## Open Questions (Resolved)

| Question | Resolution |
|----------|------------|
| Separate endpoint vs createAction? | **createAction** - integrate, don't create parallel system |
| Trust GorillaPool or validate locally? | **Trust GorillaPool** for Option 2; local validation is Option 3 |
| How to track token UTXOs? | **New `token_utxos` table** with foreign key to utxos |
| HD wallet key model? | **Single derivation** - we know which key controls each UTXO |
| Cache strategy? | **1 min TTL for balances, indefinite for metadata** |

## Remaining Open Questions

1. **GorillaPool rate limits**: Need to verify - add logging to detect 429s
   - *Mitigation*: Added retry with exponential backoff in api_client.rs
2. **GorillaPool health check endpoint**: Verify `/health` endpoint exists or find alternative
   - *Fallback*: Use any successful API call as health indicator
3. **Token icons**: How to display? Fetch from GorillaPool or cache locally?
   - *Recommendation*: Cache icon origin in metadata table, fetch on demand in frontend
4. **Test vectors**: Need real inscription script hex for unit tests
   - *Action*: Query GorillaPool for known token addresses, capture real scripts

### Resolved Questions (from earlier review)

| Question | Resolution |
|----------|------------|
| Incoming token detection? | **Phase 0 added** - sync on startup + parse incoming txs |
| Envelope parser field ordering? | **Fixed** - validate OP_1 before OP_0, warn on non-standard |
| Token balance overflow? | **Fixed** - sum in Rust with u128, not SQLite INTEGER |
| Missing helper functions? | **Added** - helpers.rs with address/script utilities |
| Missing error types? | **Added** - error.rs with OrdinalError, TransferValidationError |
| Big number handling? | **Added** - num-bigint dependency, bignum feature flag |

---

**Created**: January 2025
**Updated**: January 2025 (comprehensive review fixes)
**Status**: Planning - Ready for Implementation
**Assignee**: TBD (Backend developer)
