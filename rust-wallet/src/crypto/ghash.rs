//! GHASH implementation matching TypeScript SDK
//!
//! GHASH is the Galois Hash function used in AES-GCM.
//! This implementation matches the TypeScript SDK's behavior exactly.

use aes::Aes256;
use aes::cipher::{BlockEncrypt, KeyInit, generic_array::GenericArray};

/// R constant for GHASH (from TypeScript: R = [0xe1].concat(createZeroBlock(15)))
const R: [u8; 16] = [
    0xe1, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00
];

/// Right shift a 16-byte block (matching TypeScript rightShift function)
fn right_shift(block: &mut [u8; 16]) {
    let mut carry = 0;
    for i in 0..16 {
        let old_carry = carry;
        carry = block[i] & 0x01;
        block[i] = block[i] >> 1;
        if old_carry != 0 {
            block[i] |= 0x80;
        }
    }
}

/// Multiply two 16-byte blocks in GF(2^128) (matching TypeScript multiply function)
fn multiply(block0: &[u8; 16], block1: &[u8; 16]) -> [u8; 16] {
    let mut v = *block1;
    let mut z = [0u8; 16];

    for i in 0..16 {
        for j in (0..8).rev() {
            if (block0[i] & (1 << j)) != 0 {
                // XOR z with v
                for k in 0..16 {
                    z[k] ^= v[k];
                }
            }

            if (v[15] & 1) != 0 {
                right_shift(&mut v);
                // XOR v with R
                for k in 0..16 {
                    v[k] ^= R[k];
                }
            } else {
                right_shift(&mut v);
            }
        }
    }

    z
}

/// GHASH function (matching TypeScript ghash function)
///
/// TypeScript code:
/// ```typescript
/// export function ghash (input: number[], hashSubKey: number[]): number[] {
///   let result = createZeroBlock(16)
///   for (let i = 0; i < input.length; i += 16) {
///     const block = result.slice()
///     for (let j = 0; j < 16; j++) {
///       block[j] ^= input[i + j] ?? 0
///     }
///     result = multiply(block, hashSubKey)
///   }
///   return result
/// }
/// ```
pub fn ghash(input: &[u8], hash_sub_key: &[u8; 16]) -> [u8; 16] {
    let mut result = [0u8; 16];

    // Process input in 16-byte chunks
    for i in (0..input.len()).step_by(16) {
        // Copy current result to block
        let mut block = result;

        // XOR input chunk with block
        for j in 0..16 {
            let input_idx = i + j;
            if input_idx < input.len() {
                block[j] ^= input[input_idx];
            }
            // TypeScript uses ?? 0, which means if input[i + j] is undefined, use 0
            // In Rust, if input_idx >= input.len(), we just don't XOR (already 0)
        }

        // Multiply block with hash subkey
        result = multiply(&block, hash_sub_key);
    }

    result
}

/// Generate hash subkey by encrypting zero block with AES
pub fn generate_hash_subkey(key: &[u8]) -> [u8; 16] {
    let cipher = Aes256::new_from_slice(key).unwrap();
    let zero_block = GenericArray::from([0u8; 16]);
    let mut encrypted = zero_block;
    cipher.encrypt_block(&mut encrypted);
    encrypted.into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ghash_basic() {
        // Test with simple input
        let input = vec![0u8; 16];
        let hash_sub_key = [0u8; 16];
        let result = ghash(&input, &hash_sub_key);
        assert_eq!(result.len(), 16);
    }
}
