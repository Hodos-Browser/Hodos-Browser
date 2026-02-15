//! Signature Hash (SIGHASH) Calculation for Bitcoin SV
//!
//! BSV uses ForkID SIGHASH algorithm for replay protection after UAHF
//! This is based on the BSV Go SDK's CalcInputPreimage implementation
//!
//! ForkID SIGHASH format:
//! 1. Calculate hashPreviousOuts, hashSequence, hashOutputs
//! 2. Combine in specific format: version + hashes + input details + prev_script + sequence + outputs
//! 3. Double SHA256 hash the result

use super::{Transaction, types::{TransactionError, TransactionResult}};
use sha2::{Sha256, Digest};

/// SIGHASH type constants for BSV
pub const SIGHASH_ALL: u32 = 0x01;
pub const SIGHASH_FORKID: u32 = 0x40;
pub const SIGHASH_ALL_FORKID: u32 = SIGHASH_ALL | SIGHASH_FORKID; // 0x41

/// Calculate BSV ForkID signature hash for transactions
///
/// This implements the BSV ForkID SIGHASH algorithm used after UAHF
/// Based on the BSV Go SDK's CalcInputPreimage function
pub fn calculate_sighash(
    tx: &Transaction,
    input_index: usize,
    prev_script: &[u8],
    _prev_value: i64, // Value of the previous output being spent (8 bytes in preimage)
    sighash_type: u32,
) -> TransactionResult<Vec<u8>> {
    if input_index >= tx.inputs.len() {
        return Err(TransactionError::InvalidFormat(
            format!("Input index {} out of range", input_index)
        ));
    }

    let input = &tx.inputs[input_index];
    let mut buf = Vec::new();

    // Version (4 bytes, little endian)
    let version_bytes = tx.version.to_le_bytes();
    buf.extend_from_slice(&version_bytes);

    // Calculate hashPreviousOuts (if not SIGHASH_ANYONECANPAY)
    if sighash_type & 0x80 == 0 { // No SIGHASH_ANYONECANPAY
        let hash_prevouts = calculate_hash_prevouts(tx)?;
        buf.extend_from_slice(&hash_prevouts);
    } else {
        // SIGHASH_ANYONECANPAY: use zero hash
        buf.extend_from_slice(&[0u8; 32]);
    }

    // Calculate hashSequence (if not SIGHASH_ANYONECANPAY and not SIGHASH_SINGLE/NONE)
    if sighash_type & 0x80 == 0 && (sighash_type & 0x1f) != 0x02 && (sighash_type & 0x1f) != 0x03 {
        let hash_sequence = calculate_hash_sequence(tx)?;
        buf.extend_from_slice(&hash_sequence);
    } else {
        // Use zero hash
        buf.extend_from_slice(&[0u8; 32]);
    }

    // Input details: txid (32 bytes, reversed for wire format) + vout (4 bytes, little endian)
    let txid_bytes = input.prev_out.txid_bytes()?;
    buf.extend_from_slice(&txid_bytes);

    let vout_bytes = input.prev_out.vout.to_le_bytes();
    buf.extend_from_slice(&vout_bytes);

    // Previous script length + script
    let script_len = prev_script.len();
    if script_len <= 0xfc {
        buf.push(script_len as u8);
    } else if script_len <= 0xffff {
        buf.push(0xfd);
        buf.extend_from_slice(&(script_len as u16).to_le_bytes());
    } else {
        buf.push(0xfe);
        buf.extend_from_slice(&(script_len as u32).to_le_bytes());
    }
    buf.extend_from_slice(prev_script);

    // Value of the output spent by this input (8 bytes, little endian)
    let value_bytes = _prev_value.to_le_bytes();
    buf.extend_from_slice(&value_bytes);

    // Sequence (4 bytes, little endian)
    let sequence_bytes = input.sequence.to_le_bytes();
    buf.extend_from_slice(&sequence_bytes);

    // Calculate hashOutputs
    if (sighash_type & 0x1f) != 0x02 && (sighash_type & 0x1f) != 0x03 { // Not SIGHASH_NONE or SIGHASH_SINGLE
        let hash_outputs = calculate_hash_outputs(tx)?;
        buf.extend_from_slice(&hash_outputs);
    } else if (sighash_type & 0x1f) == 0x03 && input_index < tx.outputs.len() { // SIGHASH_SINGLE
        let hash_outputs = calculate_hash_outputs_single(tx, input_index)?;
        buf.extend_from_slice(&hash_outputs);
    } else {
        // Use zero hash
        buf.extend_from_slice(&[0u8; 32]);
    }

    // Locktime (4 bytes, little endian)
    let locktime_bytes = tx.lock_time.to_le_bytes();
    buf.extend_from_slice(&locktime_bytes);

    // SIGHASH type (4 bytes, little endian)
    let sighash_bytes = sighash_type.to_le_bytes();
    buf.extend_from_slice(&sighash_bytes);

    // Debug logging
    log::info!("   📋 BSV ForkID SIGHASH Debug:");
    log::info!("      Input index: {}", input_index);
    log::info!("      SIGHASH type: 0x{:02x}", sighash_type);
    log::info!("      prev_script length: {}", prev_script.len());
    log::info!("      prev_script: {}", hex::encode(prev_script));
    log::info!("      Preimage length: {} bytes", buf.len());
    log::info!("      Preimage: {}", hex::encode(&buf));

    // Double SHA256 hash (SHA256d)
    let hash1 = Sha256::digest(&buf);
    let hash2 = Sha256::digest(&hash1);

    Ok(hash2.to_vec())
}

/// Calculate hash of all previous outputs
fn calculate_hash_prevouts(tx: &Transaction) -> TransactionResult<[u8; 32]> {
    let mut buf = Vec::new();

    for input in &tx.inputs {
        // txid (32 bytes, reversed for wire format) + vout (4 bytes, little endian)
        let txid_bytes = input.prev_out.txid_bytes()?;
        buf.extend_from_slice(&txid_bytes);

        let vout_bytes = input.prev_out.vout.to_le_bytes();
        buf.extend_from_slice(&vout_bytes);
    }

    Ok(Sha256::digest(&Sha256::digest(&buf)).into())
}

/// Calculate hash of all sequence numbers
fn calculate_hash_sequence(tx: &Transaction) -> TransactionResult<[u8; 32]> {
    let mut buf = Vec::new();

    for input in &tx.inputs {
        let sequence_bytes = input.sequence.to_le_bytes();
        buf.extend_from_slice(&sequence_bytes);
    }

    Ok(Sha256::digest(&Sha256::digest(&buf)).into())
}

/// Calculate hash of all outputs
fn calculate_hash_outputs(tx: &Transaction) -> TransactionResult<[u8; 32]> {
    let mut buf = Vec::new();

    for output in &tx.outputs {
        // value (8 bytes, little endian) + script length + script
        let value_bytes = output.value.to_le_bytes();
        buf.extend_from_slice(&value_bytes);

        let script_len = output.script_pubkey.len();
        if script_len <= 0xfc {
            buf.push(script_len as u8);
        } else if script_len <= 0xffff {
            buf.push(0xfd);
            buf.extend_from_slice(&(script_len as u16).to_le_bytes());
        } else {
            buf.push(0xfe);
            buf.extend_from_slice(&(script_len as u32).to_le_bytes());
        }
        buf.extend_from_slice(&output.script_pubkey);
    }

    Ok(Sha256::digest(&Sha256::digest(&buf)).into())
}

/// Calculate hash of outputs for SIGHASH_SINGLE
fn calculate_hash_outputs_single(tx: &Transaction, input_index: usize) -> TransactionResult<[u8; 32]> {
    if input_index >= tx.outputs.len() {
        return Err(TransactionError::InvalidFormat(
            "SIGHASH_SINGLE with input index >= number of outputs".to_string()
        ));
    }

    let output = &tx.outputs[input_index];
    let mut buf = Vec::new();

    // value (8 bytes, little endian) + script length + script
    let value_bytes = output.value.to_le_bytes();
    buf.extend_from_slice(&value_bytes);

    let script_len = output.script_pubkey.len();
    if script_len <= 0xfc {
        buf.push(script_len as u8);
    } else if script_len <= 0xffff {
        buf.push(0xfd);
        buf.extend_from_slice(&(script_len as u16).to_le_bytes());
    } else {
        buf.push(0xfe);
        buf.extend_from_slice(&(script_len as u32).to_le_bytes());
    }
    buf.extend_from_slice(&output.script_pubkey);

    Ok(Sha256::digest(&Sha256::digest(&buf)).into())
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sighash_constants() {
        assert_eq!(SIGHASH_ALL, 0x01);
        assert_eq!(SIGHASH_FORKID, 0x40);
        assert_eq!(SIGHASH_ALL_FORKID, 0x41);
    }
}
