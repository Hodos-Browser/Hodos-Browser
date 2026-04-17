//! TaskConsolidateDust — Automatic dust UTXO consolidation
//!
//! Consolidates UTXOs below a threshold (1000 sats) when 20+ accumulate.
//! Sends all dust to a new self-address in a single transaction.
//! Opt-out via wallet settings (disable_dust_consolidation).
//!
//! Interval: 86400 seconds (24 hours)

use actix_web::web;
use log::{info, warn, error};
use secp256k1::{Secp256k1, SecretKey, Message};

use crate::AppState;
use crate::database::{OutputRepository, derive_key_for_output};
use crate::database::{WalletRepository, AddressRepository, BasketRepository, CommissionRepository, Commission};
use crate::database::{TransactionRepository, ParentTransactionRepository, get_master_private_key_from_db, get_master_public_key_from_db};
use crate::transaction::{Transaction, TxInput, TxOutput, OutPoint, Script};
use crate::transaction::sighash::{calculate_sighash, SIGHASH_ALL_FORKID};
use crate::handlers::{estimate_transaction_size, calculate_fee, address_to_script, broadcast_transaction, HODOS_FEE_ADDRESS, HODOS_SERVICE_FEE_SATS};
use crate::crypto::brc42::derive_child_public_key;
use crate::action_storage::TransactionStatus;

/// Dust threshold: UTXOs at or below this are candidates for consolidation
const DUST_THRESHOLD_SATS: i64 = 1000;

/// Minimum number of dust UTXOs before we consolidate
const MIN_DUST_COUNT: usize = 20;

/// Bitcoin dust limit — outputs below this are rejected by miners
const DUST_LIMIT_SATS: i64 = 546;

/// Result of a consolidation attempt
pub enum ConsolidateResult {
    /// Consolidation completed successfully
    Consolidated { txid: String, input_count: usize, net_sats: i64 },
    /// Skipped (not enough dust, disabled, etc.)
    Skipped(String),
}

/// Run the dust consolidation task (called by monitor and manual endpoint)
pub async fn run(state: &web::Data<AppState>) -> Result<(), String> {
    match run_inner(state).await {
        Ok(ConsolidateResult::Consolidated { .. }) => Ok(()),
        Ok(ConsolidateResult::Skipped(_)) => Ok(()),
        Err(e) => Err(e),
    }
}

/// Inner implementation that returns detailed result
pub async fn run_inner(state: &web::Data<AppState>) -> Result<ConsolidateResult, String> {
    // 1. Check opt-out setting
    let disabled = {
        let db = state.database.lock().map_err(|e| format!("DB lock: {}", e))?;
        let conn = db.connection();
        match conn.query_row(
            "SELECT COALESCE(MAX(CASE WHEN key = 'disable_dust_consolidation' THEN value END), '0') FROM settings",
            [],
            |row| row.get::<_, String>(0),
        ) {
            Ok(val) => val == "1" || val == "true",
            Err(_) => false,
        }
    };

    if disabled {
        info!("   🧹 TaskConsolidateDust: disabled via settings, skipping");
        return Ok(ConsolidateResult::Skipped("Disabled via settings".into()));
    }

    // 2. Read confirmed dust UTXOs from the default basket
    let dust_outputs: Vec<DustUtxo> = {
        let db = state.database.lock().map_err(|e| format!("DB lock: {}", e))?;
        let output_repo = OutputRepository::new(db.connection());

        let confirmed = match output_repo.get_spendable_confirmed_by_user(state.current_user_id) {
            Ok(outputs) => outputs,
            Err(e) => {
                return Err(format!("Failed to get confirmed outputs: {}", e));
            }
        };

        confirmed.iter()
            .filter(|o| o.satoshis <= DUST_THRESHOLD_SATS && o.txid.is_some()
                && o.locking_script.as_ref().map(|s| is_p2pkh_script(s)).unwrap_or(false))
            .map(|o| DustUtxo {
                txid: o.txid.clone().unwrap(),
                vout: o.vout as u32,
                satoshis: o.satoshis,
                locking_script: o.locking_script.clone().unwrap_or_default(),
                derivation_prefix: o.derivation_prefix.clone(),
                derivation_suffix: o.derivation_suffix.clone(),
                sender_identity_key: o.sender_identity_key.clone(),
            })
            .collect()
    };

    if dust_outputs.len() < MIN_DUST_COUNT {
        let msg = format!("{} dust P2PKH UTXOs ≤{} sats (need {} to trigger)", dust_outputs.len(), DUST_THRESHOLD_SATS, MIN_DUST_COUNT);
        info!("   🧹 TaskConsolidateDust: {}", msg);
        return Ok(ConsolidateResult::Skipped(msg));
    }

    let total_dust: i64 = dust_outputs.iter().map(|u| u.satoshis).sum();
    info!("   🧹 TaskConsolidateDust: {} dust UTXOs totaling {} sats — consolidating", dust_outputs.len(), total_dust);

    // 3. Calculate economics
    let fee_rate = state.fee_rate_cache.get_rate().await;
    let input_scripts: Vec<usize> = vec![107; dust_outputs.len()]; // P2PKH unlocking
    let output_scripts: Vec<usize> = vec![25, 25]; // P2PKH self + P2PKH service fee
    let estimated_size = estimate_transaction_size(&input_scripts, &output_scripts);
    let mining_fee = calculate_fee(estimated_size, fee_rate) as i64;

    let net_value = total_dust - mining_fee - HODOS_SERVICE_FEE_SATS;
    if net_value < DUST_LIMIT_SATS {
        let msg = format!("Net value {} sats < dust limit {} after fees (total: {}, mining: {}, service: {})",
            net_value, DUST_LIMIT_SATS, total_dust, mining_fee, HODOS_SERVICE_FEE_SATS);
        info!("   🧹 TaskConsolidateDust: {}", msg);
        return Ok(ConsolidateResult::Skipped(msg));
    }

    // 4. Acquire create_action_lock to prevent concurrent UTXO selection races
    let _create_guard = state.create_action_lock.lock().await;
    let _utxo_guard = state.utxo_selection_lock.lock().await;
    info!("   🔒 Consolidation locks acquired");

    // 5. Re-read dust UTXOs under locks (they may have been spent since initial check)
    let dust_outputs: Vec<DustUtxo> = {
        let db = state.database.lock().map_err(|e| format!("DB lock: {}", e))?;
        let output_repo = OutputRepository::new(db.connection());
        let confirmed = output_repo.get_spendable_confirmed_by_user(state.current_user_id)
            .map_err(|e| format!("Re-read failed: {}", e))?;

        confirmed.iter()
            .filter(|o| o.satoshis <= DUST_THRESHOLD_SATS && o.txid.is_some()
                && o.locking_script.as_ref().map(|s| is_p2pkh_script(s)).unwrap_or(false))
            .map(|o| DustUtxo {
                txid: o.txid.clone().unwrap(),
                vout: o.vout as u32,
                satoshis: o.satoshis,
                locking_script: o.locking_script.clone().unwrap_or_default(),
                derivation_prefix: o.derivation_prefix.clone(),
                derivation_suffix: o.derivation_suffix.clone(),
                sender_identity_key: o.sender_identity_key.clone(),
            })
            .collect()
    };

    if dust_outputs.len() < MIN_DUST_COUNT {
        let msg = format!("Dust count dropped to {} after re-check under lock", dust_outputs.len());
        info!("   🧹 TaskConsolidateDust: {}", msg);
        return Ok(ConsolidateResult::Skipped(msg));
    }

    let total_dust: i64 = dust_outputs.iter().map(|u| u.satoshis).sum();
    let net_value = total_dust - mining_fee - HODOS_SERVICE_FEE_SATS;
    if net_value < DUST_LIMIT_SATS {
        return Ok(ConsolidateResult::Skipped(format!("Net value {} sats below dust limit after re-check", net_value)));
    }

    // 6. Reserve UTXOs (mark as spent with placeholder)
    let placeholder_txid = format!("consolidate-{}", chrono::Utc::now().timestamp_millis());
    {
        let db = state.database.lock().map_err(|e| format!("DB lock: {}", e))?;
        let output_repo = OutputRepository::new(db.connection());
        let utxos_to_reserve: Vec<(String, u32)> = dust_outputs.iter()
            .map(|u| (u.txid.clone(), u.vout))
            .collect();
        let reserved = output_repo.mark_multiple_spent(&utxos_to_reserve, &placeholder_txid)
            .map_err(|e| format!("Failed to reserve UTXOs: {}", e))?;
        info!("   🔒 Reserved {} dust UTXOs", reserved);
        state.balance_cache.invalidate();
    }

    // 7. Generate new change address for the consolidated output
    let (change_address_index, change_script_bytes, change_script_hex) = {
        let db = state.database.lock().map_err(|e| format!("DB lock: {}", e))?;
        let wallet_repo = WalletRepository::new(db.connection());
        let address_repo = AddressRepository::new(db.connection());

        let wallet = wallet_repo.get_primary_wallet()
            .map_err(|e| format!("Wallet: {}", e))?
            .ok_or("No wallet found")?;
        let wallet_id = wallet.id.unwrap();

        let next_index = address_repo.get_max_index(wallet_id)
            .unwrap_or(Some(0))
            .map(|i| i + 1)
            .unwrap_or(0);

        let master_privkey = get_master_private_key_from_db(&db)
            .map_err(|e| format!("Master key: {}", e))?;
        let master_pubkey = get_master_public_key_from_db(&db)
            .map_err(|e| format!("Master pubkey: {}", e))?;

        let invoice = format!("2-receive address-{}", next_index);
        let derived_pubkey = derive_child_public_key(&master_privkey, &master_pubkey, &invoice)
            .map_err(|e| format!("BRC-42: {}", e))?;

        // Create P2PKH script
        use sha2::{Sha256, Digest};
        use ripemd::Ripemd160;
        let sha_hash = Sha256::digest(&derived_pubkey);
        let pubkey_hash = Ripemd160::digest(&sha_hash);
        let script = Script::p2pkh_locking_script(&pubkey_hash)
            .map_err(|e| format!("P2PKH script: {}", e))?;

        // Convert pubkey to address for DB storage
        let mut payload = vec![0x00u8]; // mainnet prefix
        payload.extend_from_slice(&pubkey_hash);
        let sha1 = Sha256::digest(&payload);
        let sha2_hash = Sha256::digest(&sha1);
        let checksum = &sha2_hash[..4];
        payload.extend_from_slice(checksum);
        let change_address = bs58::encode(&payload).into_string();

        // Save address to DB
        let created_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap().as_secs() as i64;

        let address_model = crate::database::Address {
            id: None,
            wallet_id,
            index: next_index,
            address: change_address,
            public_key: hex::encode(&derived_pubkey),
            used: true,
            balance: 0,
            pending_utxo_check: false,
            created_at,
        };

        if address_repo.create(&address_model).is_ok() {
            let _ = wallet_repo.update_current_index(wallet_id, next_index + 1);
        }

        (next_index, script.bytes.clone(), hex::encode(&script.bytes))
    };

    // 8. Build transaction
    let mut tx = Transaction::new();

    // Add dust inputs
    for utxo in &dust_outputs {
        let outpoint = OutPoint::new(utxo.txid.clone(), utxo.vout);
        tx.add_input(TxInput::new(outpoint));
    }

    // Output 0: consolidated self output
    tx.add_output(TxOutput::new(net_value, change_script_bytes.clone()));

    // Output 1: service fee
    let fee_script = address_to_script(HODOS_FEE_ADDRESS)
        .map_err(|e| format!("Fee script: {}", e))?;
    tx.add_output(TxOutput::new(HODOS_SERVICE_FEE_SATS, fee_script));

    // 9. Sign each input
    let secp = Secp256k1::new();
    for (i, utxo) in dust_outputs.iter().enumerate() {
        let private_key_bytes = {
            let db = state.database.lock().map_err(|e| format!("DB lock: {}", e))?;
            derive_key_for_output(
                &db,
                utxo.derivation_prefix.as_deref(),
                utxo.derivation_suffix.as_deref(),
                utxo.sender_identity_key.as_deref(),
            ).map_err(|e| format!("Key derivation input {}: {}", i, e))?
        };

        let sighash = calculate_sighash(&tx, i, &utxo.locking_script, utxo.satoshis, SIGHASH_ALL_FORKID)
            .map_err(|e| format!("Sighash input {}: {}", i, e))?;

        let secret = SecretKey::from_slice(&private_key_bytes)
            .map_err(|e| format!("SecretKey: {}", e))?;
        let message = Message::from_digest_slice(&sighash)
            .map_err(|e| format!("Message: {}", e))?;
        let sig = secp.sign_ecdsa(&message, &secret);
        let mut sig_der = sig.serialize_der().to_vec();
        sig_der.push(SIGHASH_ALL_FORKID as u8);
        let pubkey = secp256k1::PublicKey::from_secret_key(&secp, &secret).serialize();
        let unlocking = Script::p2pkh_unlocking_script(&sig_der, &pubkey);
        tx.inputs[i].set_script(unlocking.bytes);
    }

    // 10. Get txid and serialize
    let txid = tx.txid().map_err(|e| format!("txid: {}", e))?;
    let raw_tx_hex = tx.to_hex().map_err(|e| format!("serialize: {}", e))?;

    info!("   🧹 Consolidation tx: {} ({} inputs → {} sats, fee: {}, service: {})",
        &txid[..16], dust_outputs.len(), net_value, mining_fee, HODOS_SERVICE_FEE_SATS);

    // 11. Cache signed tx for BEEF ancestry
    {
        let db = state.database.lock().map_err(|e| format!("DB lock: {}", e))?;
        let parent_tx_repo = ParentTransactionRepository::new(db.connection());
        let _ = parent_tx_repo.upsert(None, &txid, &raw_tx_hex);
    }

    // 12. Store transaction record + consolidated output in DB
    {
        use crate::action_storage::{StoredAction, ActionStatus, ActionInput, ActionOutput};

        let price_usd_cents = state.price_cache.get_cached()
            .or_else(|| state.price_cache.get_stale())
            .map(|p| (p * 100.0) as i64);

        let action = StoredAction {
            txid: txid.clone(),
            reference_number: format!("consolidate-{}", uuid::Uuid::new_v4()),
            raw_tx: raw_tx_hex.clone(),
            description: Some(format!("Automatic dust consolidation ({} UTXOs)", dust_outputs.len())),
            labels: vec!["dust-consolidation".to_string()],
            status: ActionStatus::Created,
            is_outgoing: true,
            satoshis: net_value,
            timestamp: chrono::Utc::now().timestamp(),
            block_height: None,
            confirmations: 0,
            version: tx.version,
            lock_time: tx.lock_time,
            inputs: dust_outputs.iter().map(|u| ActionInput {
                txid: u.txid.clone(),
                vout: u.vout,
                satoshis: u.satoshis,
                script: None,
            }).collect(),
            outputs: tx.outputs.iter().enumerate().map(|(i, output)| ActionOutput {
                vout: i as u32,
                satoshis: output.value,
                script: Some(hex::encode(&output.script_pubkey)),
                address: None,
            }).collect(),
            price_usd_cents,
        };

        let db = state.database.lock().map_err(|e| format!("DB lock: {}", e))?;
        let tx_repo = TransactionRepository::new(db.connection());

        match tx_repo.add_transaction(&action, state.current_user_id) {
            Ok(transaction_id) => {
                let output_repo = OutputRepository::new(db.connection());
                let basket_repo = BasketRepository::new(db.connection());

                let default_basket_id = basket_repo.find_or_insert("default", state.current_user_id).ok();

                let _ = output_repo.insert_output(
                    state.current_user_id,
                    &txid,
                    0,  // vout 0 = consolidated output
                    net_value,
                    &change_script_hex,
                    default_basket_id,
                    Some("2-receive address"),
                    Some(&change_address_index.to_string()),
                    None,
                    Some("Dust consolidation"),
                    true,  // is_change
                );

                let _ = output_repo.link_outputs_to_transaction(&txid, transaction_id);
                let _ = output_repo.update_spending_description_batch(&placeholder_txid, &txid);

                // Record commission
                let commission_repo = CommissionRepository::new(db.connection());
                let _ = commission_repo.create(&Commission {
                    commission_id: None,
                    user_id: state.current_user_id,
                    transaction_id,
                    satoshis: HODOS_SERVICE_FEE_SATS,
                    key_offset: "hodos-service-fee".to_string(),
                    is_redeemed: false,
                    locking_script: address_to_script(HODOS_FEE_ADDRESS).unwrap(),
                    created_at: 0,
                    updated_at: 0,
                });

                info!("   💾 Consolidation tx stored (id={})", transaction_id);
            }
            Err(e) => {
                warn!("   ⚠️  Failed to store consolidation tx: {}", e);
            }
        }
    }

    state.balance_cache.invalidate();

    // 13. Broadcast (all inputs are confirmed, so raw tx broadcast is sufficient)
    // ARC will accept raw tx and validate against confirmed parents.
    match broadcast_transaction(&raw_tx_hex, Some(&state.database), Some(&txid)).await {
        Ok(msg) => {
            info!("   ✅ Consolidation broadcast successful: {}", msg);

            let db = state.database.lock().map_err(|e| format!("DB lock: {}", e))?;
            let tx_repo = TransactionRepository::new(db.connection());
            let _ = tx_repo.update_broadcast_status(&txid, "broadcast");

            state.request_backup_check_if_significant(net_value);
        }
        Err(e) => {
            error!("   ❌ Consolidation broadcast failed: {}", e);

            // Cleanup: mark tx as failed, delete ghost output, restore inputs
            if let Ok(db) = state.database.lock() {
                let conn = db.connection();
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs() as i64;

                // Mark transaction failed
                let _ = conn.execute(
                    "UPDATE transactions SET status = 'failed', failed_at = ?1 WHERE txid = ?2",
                    rusqlite::params![now, txid],
                );

                // Disable the consolidated output (recoverable by TaskUnFail if tx was actually mined)
                let output_repo = OutputRepository::new(conn);
                let _ = output_repo.disable_by_txid(&txid);

                // Restore dust inputs (un-mark as spent)
                let _ = conn.execute(
                    "UPDATE outputs SET spendable = 1, spent_by = NULL, spending_description = NULL
                     WHERE spending_description = ?1 AND spendable = 0",
                    rusqlite::params![txid],
                );

                // Also try placeholder in case spending_description wasn't updated yet
                let _ = conn.execute(
                    "UPDATE outputs SET spendable = 1, spent_by = NULL, spending_description = NULL
                     WHERE spending_description = ?1 AND spendable = 0",
                    rusqlite::params![placeholder_txid],
                );
            }

            state.balance_cache.invalidate();
            return Err(format!("Broadcast failed: {}", e));
        }
    }

    let input_count = dust_outputs.len();
    Ok(ConsolidateResult::Consolidated { txid, input_count, net_sats: net_value })
}

/// Check if a locking script is standard P2PKH (OP_DUP OP_HASH160 <20 bytes> OP_EQUALVERIFY OP_CHECKSIG)
/// This guards against accidentally trying to spend PushDrop tokens or other non-standard scripts.
fn is_p2pkh_script(script: &[u8]) -> bool {
    script.len() == 25
        && script[0] == 0x76  // OP_DUP
        && script[1] == 0xa9  // OP_HASH160
        && script[2] == 0x14  // Push 20 bytes
        && script[23] == 0x88 // OP_EQUALVERIFY
        && script[24] == 0xac // OP_CHECKSIG
}

/// Internal struct for dust UTXO tracking
struct DustUtxo {
    txid: String,
    vout: u32,
    satoshis: i64,
    locking_script: Vec<u8>,
    derivation_prefix: Option<String>,
    derivation_suffix: Option<String>,
    sender_identity_key: Option<String>,
}
