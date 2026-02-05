//! Custom AES-GCM implementation matching TypeScript SDK exactly
//!
//! This implements the full AESGCM and AESGCMDecrypt functions from the TypeScript SDK,
//! including proper handling of 32-byte IVs through GHASH.

use aes::Aes256;
use aes::cipher::{BlockEncrypt, KeyInit, generic_array::GenericArray};
use crate::crypto::ghash;

/// Helper: Create a zero block of specified length
fn create_zero_block(length: usize) -> Vec<u8> {
    vec![0u8; length]
}

/// Helper: Convert numeric value to 4-byte big-endian array
/// TypeScript: getBytes(numericValue)
fn get_bytes(value: u32) -> [u8; 4] {
    [
        ((value >> 24) & 0xFF) as u8,
        ((value >> 16) & 0xFF) as u8,
        ((value >> 8) & 0xFF) as u8,
        (value & 0xFF) as u8,
    ]
}

/// Increment least significant 32 bits of a 16-byte block
/// TypeScript: incrementLeastSignificantThirtyTwoBits(block)
/// Increments bytes 12-15 (last 4 bytes), with carry propagation
fn increment_least_significant_32_bits(block: &[u8; 16]) -> [u8; 16] {
    let mut result = *block;

    // Start from byte 15 (last byte) and work backwards to byte 12
    for i in (12..16).rev() {
        let (new_val, overflow) = result[i].overflowing_add(1);
        result[i] = new_val;
        if !overflow {
            break; // No carry, we're done
        }
    }

    result
}

/// Encrypt a single 16-byte block using AES-256
/// TypeScript: AES(counterBlock, key)
fn aes_encrypt_block(block: &[u8; 16], key: &[u8; 32]) -> [u8; 16] {
    let cipher = Aes256::new_from_slice(key).unwrap();
    let mut input_block = GenericArray::from(*block);
    cipher.encrypt_block(&mut input_block);
    input_block.into()
}

/// GCTR (Galois Counter) mode encryption/decryption
/// TypeScript: gctr(input, initialCounterBlock, key)
fn gctr(input: &[u8], initial_counter_block: &[u8; 16], key: &[u8; 32]) -> Vec<u8> {
    if input.is_empty() {
        return Vec::new();
    }

    let mut output = Vec::with_capacity(input.len());
    let mut counter_block = *initial_counter_block;
    let mut pos = 0;
    let n = (input.len() + 15) / 16; // Ceiling division

    for i in 0..n {
        // Encrypt counter block
        let counter = aes_encrypt_block(&counter_block, key);

        // XOR input with encrypted counter
        let chunk_size = (16).min(input.len() - pos);
        for j in 0..chunk_size {
            output.push(input[pos + j] ^ counter[j]);
        }
        pos += chunk_size;

        // Increment counter for next block (except for last iteration)
        if i + 1 < n {
            counter_block = increment_least_significant_32_bits(&counter_block);
        }
    }

    output
}

/// Process IV to get preCounterBlock (matching TypeScript SDK exactly)
fn process_iv_to_pre_counter_block(iv: &[u8], hash_sub_key: &[u8; 16]) -> [u8; 16] {
    if iv.len() == 12 {
        // Standard 12-byte nonce: pad to 16 bytes and add 0x01
        let mut result = [0u8; 16];
        result[..12].copy_from_slice(iv);
        result[12..15].copy_from_slice(&[0u8; 3]);
        result[15] = 0x01;
        log::info!("      IV processing: 12-byte nonce → padded to 16 bytes with 0x01");
        result
    } else {
        // Non-standard IV length: process through GHASH
        log::info!("      IV processing: {}-byte IV → processing through GHASH", iv.len());
        let mut pre_counter_block = iv.to_vec();
        // log::info!("      Step 1: IV (hex): {}", hex::encode(&pre_counter_block));

        // Pad to 16-byte boundary
        let padding = (16 - (iv.len() % 16)) % 16;
        if padding > 0 {
            pre_counter_block.extend_from_slice(&create_zero_block(padding));
            log::info!("      Step 2: Added {} bytes padding", padding);
        } else {
            log::info!("      Step 2: No padding needed (already 16-byte aligned)");
        }

        // Add 8 zero bytes
        pre_counter_block.extend_from_slice(&[0u8; 8]);
        log::info!("      Step 3: Added 8 zero bytes (total length: {})", pre_counter_block.len());

        // Add 4 zero bytes + length encoding (in bits)
        pre_counter_block.extend_from_slice(&[0u8; 4]);
        let length_bits = (iv.len() * 8) as u32;
        let length_bytes = get_bytes(length_bits);
        pre_counter_block.extend_from_slice(&length_bytes);
        log::info!("      Step 4: Added 4 zero bytes + length encoding ({} bits = {:?})", length_bits, length_bytes);
        log::info!("      Step 5: Total GHASH input length: {} bytes", pre_counter_block.len());
        // log::info!("      Step 5: GHASH input (hex, first 64): {}", hex::encode(&pre_counter_block[..pre_counter_block.len().min(64)]));

        // GHASH the whole thing (returns [u8; 16])
        let result = ghash::ghash(&pre_counter_block, hash_sub_key);
        // log::info!("      Step 6: GHASH result (preCounterBlock, hex): {}", hex::encode(&result));
        result
    }
}

/// Custom AESGCM encryption matching TypeScript SDK exactly
/// TypeScript: AESGCM(plainText, additionalAuthenticatedData, initializationVector, key)
pub fn aesgcm_custom(
    plaintext: &[u8],
    additional_data: &[u8],
    iv: &[u8],
    key: &[u8; 32],
) -> Result<(Vec<u8>, Vec<u8>), String> {
    log::info!("   🔐 Custom AESGCM encryption (internal):");
    log::info!("      Plaintext length: {} bytes", plaintext.len());
    // log::info!("      Plaintext (hex): {}", hex::encode(plaintext));
    log::info!("      IV length: {} bytes", iv.len());
    // log::info!("      IV (hex): {}", hex::encode(iv));
    log::info!("      Additional data length: {} bytes", additional_data.len());

    // 1. Generate hash subkey (encrypt zero block)
    let hash_sub_key = ghash::generate_hash_subkey(key);
    // log::info!("      Hash subkey (hex): {}", hex::encode(&hash_sub_key));

    // 2. Process IV to get preCounterBlock
    let pre_counter_block = process_iv_to_pre_counter_block(iv, &hash_sub_key);
    // log::info!("      PreCounterBlock (hex): {}", hex::encode(&pre_counter_block));

    // 3. Increment preCounterBlock for first counter
    let initial_counter = increment_least_significant_32_bits(&pre_counter_block);
    // log::info!("      Initial counter (hex): {}", hex::encode(&initial_counter));

    // 4. Encrypt plaintext using GCTR
    let ciphertext = gctr(plaintext, &initial_counter, key);
    log::info!("      Ciphertext length: {} bytes", ciphertext.len());
    // log::info!("      Ciphertext (hex): {}", hex::encode(&ciphertext));

    // 5. Build plainTag for authentication
    let mut plain_tag = additional_data.to_vec();

    // Pad additional data to 16-byte boundary
    if additional_data.is_empty() {
        plain_tag.extend_from_slice(&[0u8; 16]);
    } else if additional_data.len() % 16 != 0 {
        let padding = 16 - (additional_data.len() % 16);
        plain_tag.extend_from_slice(&create_zero_block(padding));
    }

    // Append ciphertext
    plain_tag.extend_from_slice(&ciphertext);

    // Pad ciphertext to 16-byte boundary
    if ciphertext.is_empty() {
        plain_tag.extend_from_slice(&[0u8; 16]);
    } else if ciphertext.len() % 16 != 0 {
        let padding = 16 - (ciphertext.len() % 16);
        plain_tag.extend_from_slice(&create_zero_block(padding));
    }

    // Append lengths (in bits)
    plain_tag.extend_from_slice(&[0u8; 4]);
    plain_tag.extend_from_slice(&get_bytes((additional_data.len() * 8) as u32));
    plain_tag.extend_from_slice(&[0u8; 4]);
    plain_tag.extend_from_slice(&get_bytes((ciphertext.len() * 8) as u32));

    // 6. Generate authentication tag
    // log::info!("      PlainTag length: {} bytes", plain_tag.len());
    // log::info!("      PlainTag (hex, first 64): {}", hex::encode(&plain_tag[..plain_tag.len().min(64)]));
    let tag_hash = ghash::ghash(&plain_tag, &hash_sub_key);
    // log::info!("      Tag hash (GHASH result, hex): {}", hex::encode(&tag_hash));
    let auth_tag = gctr(&tag_hash, &pre_counter_block, key);
    // log::info!("      Auth tag (hex): {}", hex::encode(&auth_tag));
    // log::info!("      Auth tag length: {} bytes", auth_tag.len());

    Ok((ciphertext, auth_tag))
}

/// Custom AESGCM decryption matching TypeScript SDK exactly
/// TypeScript: AESGCMDecrypt(cipherText, additionalAuthenticatedData, initializationVector, authenticationTag, key)
pub fn aesgcm_decrypt_custom(
    ciphertext: &[u8],
    additional_data: &[u8],
    iv: &[u8],
    auth_tag: &[u8],
    key: &[u8; 32],
) -> Result<Vec<u8>, String> {
    if auth_tag.len() != 16 {
        return Err(format!("Authentication tag must be 16 bytes, got {}", auth_tag.len()));
    }

    // 1. Generate hash subkey
    let hash_sub_key = ghash::generate_hash_subkey(key);

    // 2. Process IV to get preCounterBlock (same as encryption)
    let pre_counter_block = process_iv_to_pre_counter_block(iv, &hash_sub_key);

    // 3. Decrypt ciphertext using GCTR (GCTR is symmetric)
    let initial_counter = increment_least_significant_32_bits(&pre_counter_block);
    let plaintext = gctr(ciphertext, &initial_counter, key);

    // 4. Build compareTag for authentication verification
    let mut compare_tag = additional_data.to_vec();

    // Pad additional data
    if additional_data.is_empty() {
        compare_tag.extend_from_slice(&[0u8; 16]);
    } else if additional_data.len() % 16 != 0 {
        let padding = 16 - (additional_data.len() % 16);
        compare_tag.extend_from_slice(&create_zero_block(padding));
    }

    // Append ciphertext
    compare_tag.extend_from_slice(ciphertext);

    // Pad ciphertext
    if ciphertext.is_empty() {
        compare_tag.extend_from_slice(&[0u8; 16]);
    } else if ciphertext.len() % 16 != 0 {
        let padding = 16 - (ciphertext.len() % 16);
        compare_tag.extend_from_slice(&create_zero_block(padding));
    }

    // Append lengths
    compare_tag.extend_from_slice(&[0u8; 4]);
    compare_tag.extend_from_slice(&get_bytes((additional_data.len() * 8) as u32));
    compare_tag.extend_from_slice(&[0u8; 4]);
    compare_tag.extend_from_slice(&get_bytes((ciphertext.len() * 8) as u32));

    // 5. Generate authentication tag
    let tag_hash = ghash::ghash(&compare_tag, &hash_sub_key);
    let calculated_tag = gctr(&tag_hash, &pre_counter_block, key);

    // 6. Verify tag matches
    if calculated_tag != auth_tag {
        return Err("Authentication tag verification failed".to_string());
    }

    Ok(plaintext)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_increment_lsb_32_bits() {
        let block = [0u8; 16];
        let result = increment_least_significant_32_bits(&block);
        assert_eq!(result[15], 1);
        assert_eq!(result[12..16], [0, 0, 0, 1]);
    }

    #[test]
    fn test_get_bytes() {
        let bytes = get_bytes(0x12345678);
        assert_eq!(bytes, [0x12, 0x34, 0x56, 0x78]);
    }
}
