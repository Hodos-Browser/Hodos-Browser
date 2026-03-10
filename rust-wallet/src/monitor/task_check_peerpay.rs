//! TaskCheckPeerPay — Poll MessageBox for incoming BRC-29 PeerPay payments (auto-accept)
//!
//! Correct implementation using:
//! - BRC-103 AuthFetch for authenticated MessageBox API calls
//! - BRC-2 encrypted message decryption
//! - BRC-42 key derivation for payment verification
//! - Persistent tracking in `peerpay_received` table (deduplication)
//! - Auto-accept: derives private key, verifies P2PKH output, stores as spendable
//!
//! Interval: 60 seconds

use actix_web::web;
use log::{info, debug, warn, error};

use crate::AppState;
use crate::messagebox::MessageBoxClient;
use crate::database::{PeerPayRepository, ParentTransactionRepository};

/// PaymentToken — parsed flexibly from the decrypted message body.
/// Different BRC-29 senders may use slightly different formats
/// (e.g., transaction as base64 string OR as byte array).
struct PaymentToken {
    custom_instructions: Option<PaymentInstructions>,
    transaction_bytes: Option<Vec<u8>>,  // Decoded transaction (from string or array)
    amount: Option<i64>,
}

struct PaymentInstructions {
    derivation_prefix: Option<String>,
    derivation_suffix: Option<String>,
}

/// Parse PaymentToken flexibly from JSON value
fn parse_payment_token(data: &[u8]) -> Result<PaymentToken, String> {
    let val: serde_json::Value = serde_json::from_slice(data)
        .map_err(|e| format!("invalid JSON: {}", e))?;

    let obj = val.as_object().ok_or("payment token is not a JSON object")?;

    // Parse customInstructions
    let instructions = obj.get("customInstructions").and_then(|ci| {
        let ci_obj = ci.as_object()?;
        Some(PaymentInstructions {
            derivation_prefix: ci_obj.get("derivationPrefix").and_then(|v| v.as_str().map(|s| s.to_string())),
            derivation_suffix: ci_obj.get("derivationSuffix").and_then(|v| v.as_str().map(|s| s.to_string())),
        })
    });

    // Parse transaction — accept base64 string OR byte array
    let transaction_bytes = if let Some(tx_val) = obj.get("transaction") {
        if let Some(tx_str) = tx_val.as_str() {
            // Base64 string
            Some(base64::Engine::decode(&base64::engine::general_purpose::STANDARD, tx_str)
                .map_err(|e| format!("invalid base64 transaction: {}", e))?)
        } else if let Some(tx_arr) = tx_val.as_array() {
            // Byte array [1, 2, 3, ...]
            let bytes: Result<Vec<u8>, _> = tx_arr.iter().map(|v| {
                v.as_u64().ok_or("non-numeric value in transaction array").map(|n| n as u8)
            }).collect();
            Some(bytes.map_err(|e| format!("invalid transaction array: {}", e))?)
        } else {
            None
        }
    } else {
        None
    };

    // Parse amount — accept number or string
    let amount = obj.get("amount").and_then(|a| {
        a.as_i64().or_else(|| a.as_str().and_then(|s| s.parse::<i64>().ok()))
    });

    Ok(PaymentToken {
        custom_instructions: instructions,
        transaction_bytes,
        amount,
    })
}

/// Run the TaskCheckPeerPay task
pub async fn run(state: &web::Data<AppState>, _client: &reqwest::Client) -> Result<(), String> {
    // Get our master keys
    let (master_privkey, master_pubkey) = {
        let db = state.database.lock().map_err(|e| format!("DB lock: {}", e))?;
        let privkey = match crate::database::get_master_private_key_from_db(&db) {
            Ok(k) => k,
            Err(_) => {
                debug!("TaskCheckPeerPay: no wallet yet, skipping");
                return Ok(());
            }
        };
        let pubkey = match crate::database::get_master_public_key_from_db(&db) {
            Ok(k) => k,
            Err(_) => {
                debug!("TaskCheckPeerPay: can't get public key, skipping");
                return Ok(());
            }
        };
        (privkey, pubkey)
    };

    // Build MessageBox client with our identity
    let mb_client = MessageBoxClient::new(master_privkey.clone(), master_pubkey.clone());

    // List messages from payment_inbox (authenticated + decrypted)
    let messages = match mb_client.list_messages("payment_inbox").await {
        Ok(msgs) => {
            info!("📬 TaskCheckPeerPay: polled payment_inbox — {} message(s)", msgs.len());
            msgs
        }
        Err(e) => {
            warn!("TaskCheckPeerPay: MessageBox API error: {}", e);
            return Ok(()); // Retry next tick
        }
    };

    if messages.is_empty() {
        return Ok(());
    }

    debug!("TaskCheckPeerPay: found {} message(s) in payment_inbox", messages.len());

    let mut processed_count = 0;
    let mut message_ids_to_ack: Vec<String> = Vec::new();

    for msg in &messages {
        // Check if already processed (deduplication)
        {
            let db = state.database.lock().map_err(|e| format!("DB lock: {}", e))?;
            if PeerPayRepository::is_already_processed(db.connection(), &msg.message_id)
                .unwrap_or(false)
            {
                debug!("TaskCheckPeerPay: message {} already processed, skipping", &msg.message_id[..16.min(msg.message_id.len())]);
                message_ids_to_ack.push(msg.message_id.clone());
                continue;
            }
        }

        // Parse the decrypted payment token (flexible: handles string or array for transaction)
        let token: PaymentToken = match parse_payment_token(&msg.body) {
            Ok(t) => t,
            Err(e) => {
                warn!("TaskCheckPeerPay: failed to parse payment token from {}: {}", &msg.message_id[..16.min(msg.message_id.len())], e);
                // Log the raw JSON so we can debug the format
                if let Ok(raw) = String::from_utf8(msg.body.clone()) {
                    warn!("TaskCheckPeerPay: raw token (first 500 chars): {}", &raw[..500.min(raw.len())]);
                }
                // Do NOT acknowledge — retry next tick so the payment isn't lost
                continue;
            }
        };

        let instructions = match token.custom_instructions {
            Some(ref ci) => ci,
            None => {
                warn!("TaskCheckPeerPay: message {} has no customInstructions", &msg.message_id[..16.min(msg.message_id.len())]);
                message_ids_to_ack.push(msg.message_id.clone());
                continue;
            }
        };

        let prefix = match &instructions.derivation_prefix {
            Some(p) => p.clone(),
            None => {
                warn!("TaskCheckPeerPay: missing derivationPrefix");
                message_ids_to_ack.push(msg.message_id.clone());
                continue;
            }
        };

        let suffix = match &instructions.derivation_suffix {
            Some(s) => s.clone(),
            None => {
                warn!("TaskCheckPeerPay: missing derivationSuffix");
                message_ids_to_ack.push(msg.message_id.clone());
                continue;
            }
        };

        let amount = token.amount.unwrap_or(0);
        let tx_bytes = match token.transaction_bytes {
            Some(b) => b,
            None => {
                warn!("TaskCheckPeerPay: missing transaction data");
                // Don't acknowledge — might be a partial message
                continue;
            }
        };

        // Decode sender pubkey
        let sender_pubkey = match hex::decode(&msg.sender) {
            Ok(b) if b.len() == 33 => b,
            _ => {
                warn!("TaskCheckPeerPay: invalid sender key: {}", &msg.sender[..16.min(msg.sender.len())]);
                message_ids_to_ack.push(msg.message_id.clone());
                continue;
            }
        };

        // BRC-29 invoice number: "2-3241645161d8-{prefix} {suffix}"
        let invoice_number = format!("2-3241645161d8-{} {}", prefix, suffix);

        // Derive our child private key using BRC-42 (recipient perspective)
        let child_privkey = match crate::crypto::brc42::derive_child_private_key(
            &master_privkey,
            &sender_pubkey,
            &invoice_number,
        ) {
            Ok(k) => k,
            Err(e) => {
                warn!("TaskCheckPeerPay: BRC-42 derivation failed: {:?}", e);
                message_ids_to_ack.push(msg.message_id.clone());
                continue;
            }
        };

        // Get derived public key
        let secp = secp256k1::Secp256k1::new();
        let child_secret = match secp256k1::SecretKey::from_slice(&child_privkey) {
            Ok(s) => s,
            Err(e) => {
                warn!("TaskCheckPeerPay: invalid derived key: {}", e);
                message_ids_to_ack.push(msg.message_id.clone());
                continue;
            }
        };
        let child_pubkey = secp256k1::PublicKey::from_secret_key(&secp, &child_secret).serialize().to_vec();

        // Parse Atomic BEEF
        let (subject_txid, beef) = match crate::beef::Beef::from_atomic_beef_bytes(&tx_bytes) {
            Ok(result) => result,
            Err(e) => {
                // Try as raw transaction hex in base64
                warn!("TaskCheckPeerPay: not Atomic BEEF, trying raw tx: {}", e);
                message_ids_to_ack.push(msg.message_id.clone());
                continue;
            }
        };

        let main_tx_bytes = match beef.main_transaction() {
            Some(tx) => tx.clone(),
            None => {
                warn!("TaskCheckPeerPay: BEEF has no main transaction");
                message_ids_to_ack.push(msg.message_id.clone());
                continue;
            }
        };

        // Parse transaction to find our output
        let parsed_tx = match crate::beef::ParsedTransaction::from_bytes(&main_tx_bytes) {
            Ok(tx) => tx,
            Err(e) => {
                warn!("TaskCheckPeerPay: failed to parse transaction: {}", e);
                message_ids_to_ack.push(msg.message_id.clone());
                continue;
            }
        };

        // Calculate expected P2PKH script from derived pubkey
        use sha2::{Sha256, Digest};
        use ripemd::Ripemd160;

        let sha_hash = Sha256::digest(&child_pubkey);
        let pubkey_hash = Ripemd160::digest(&sha_hash);

        // Find matching output
        let mut found_output = false;
        let mut found_vout = 0u32;
        let mut found_satoshis = 0i64;

        for (i, output) in parsed_tx.outputs.iter().enumerate() {
            // P2PKH: OP_DUP OP_HASH160 <20 bytes> OP_EQUALVERIFY OP_CHECKSIG
            if output.script.len() == 25
                && output.script[0] == 0x76
                && output.script[1] == 0xa9
                && output.script[2] == 0x14
                && output.script[23] == 0x88
                && output.script[24] == 0xac
                && &output.script[3..23] == pubkey_hash.as_slice()
            {
                found_output = true;
                found_vout = i as u32;
                found_satoshis = output.value;
                break;
            }
        }

        if !found_output {
            warn!("TaskCheckPeerPay: no matching P2PKH output found for derived key");
            message_ids_to_ack.push(msg.message_id.clone());
            continue;
        }

        info!("📬 TaskCheckPeerPay: accepting payment {} sats from {}...",
            found_satoshis, &msg.sender[..16.min(msg.sender.len())]);

        // Store as spendable output in our wallet
        {
            let db = state.database.lock().map_err(|e| format!("DB lock: {}", e))?;

            let custom_instructions = serde_json::json!({
                "type": "brc29_payment",
                "senderIdentityKey": msg.sender,
                "derivationPrefix": prefix,
                "derivationSuffix": suffix
            });

            match crate::handlers::store_derived_utxo(
                &db,
                &subject_txid,
                found_vout,
                found_satoshis,
                &hex::encode(&parsed_tx.outputs[found_vout as usize].script),
                &child_pubkey,
                &custom_instructions,
            ) {
                Ok(_) => {
                    info!("   💾 Stored PeerPay output {}:{} ({} sats)", subject_txid, found_vout, found_satoshis);
                }
                Err(e) => {
                    error!("   ❌ Failed to store PeerPay output: {}", e);
                    // Don't acknowledge — retry next tick
                    continue;
                }
            }

            // Snapshot current BSV/USD price for historical display
            let price_usd_cents = state.price_cache.get_cached()
                .or_else(|| state.price_cache.get_stale())
                .map(|p| (p * 100.0) as i64);

            // Record in peerpay_received for notification tracking
            if let Err(e) = PeerPayRepository::insert_received(
                db.connection(),
                &msg.message_id,
                &msg.sender,
                found_satoshis,
                &prefix,
                &suffix,
                Some(&subject_txid),
                "peerpay",
                price_usd_cents,
            ) {
                error!("   ❌ Failed to record peerpay_received: {}", e);
            }

            // Cache BEEF ancestry data for future BEEF building.
            // The Atomic BEEF contains parent tx raw bytes that we'd otherwise
            // have to fetch from WhatsOnChain API during send — cache them now.
            {
                let parent_tx_repo = ParentTransactionRepository::new(db.connection());

                // Cache all transactions from the BEEF (parents + main tx)
                for tx_raw in &beef.transactions {
                    let hash1 = Sha256::digest(tx_raw);
                    let hash2 = Sha256::digest(&hash1);
                    let tx_txid: Vec<u8> = hash2.into_iter().rev().collect();
                    let tx_txid_hex = hex::encode(&tx_txid);
                    let raw_hex = hex::encode(tx_raw);

                    match parent_tx_repo.upsert(None, &tx_txid_hex, &raw_hex) {
                        Ok(_) => debug!("   💾 Cached BEEF tx {} in parent_transactions", &tx_txid_hex[..16]),
                        Err(e) => warn!("   ⚠️  Failed to cache BEEF tx: {}", e),
                    }
                }
                info!("   📦 Cached {} BEEF transaction(s) for future BEEF building", beef.transactions.len());
            }
        }

        // Invalidate balance cache
        state.balance_cache.invalidate();

        processed_count += 1;
        message_ids_to_ack.push(msg.message_id.clone());
    }

    // Acknowledge all processed messages on the MessageBox server
    if !message_ids_to_ack.is_empty() {
        if let Err(e) = mb_client.acknowledge(&message_ids_to_ack).await {
            warn!("TaskCheckPeerPay: failed to acknowledge messages: {}", e);
            // Non-fatal — messages will be re-fetched and deduplicated next tick
        }
    }

    if processed_count > 0 {
        info!("📬 TaskCheckPeerPay: accepted {} incoming payment(s)", processed_count);
        super::log_monitor_event(
            state,
            "TaskCheckPeerPay:accepted",
            Some(&format!("{} payments", processed_count)),
        );
    }

    Ok(())
}
