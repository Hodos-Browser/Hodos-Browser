/// BRC-62 BEEF (Background Evaluation Extended Format) Parser
///
/// BEEF is a format for packaging Bitcoin transactions with their ancestry
/// and optional SPV proofs for verification without a full blockchain.
///
/// Format:
/// - version (4 bytes: 0x0100beef per BRC-62)
/// - num_bumps (varint) - number of merkle proofs
/// - bumps (array of merkle proofs)
/// - num_txs (varint) - number of transactions
/// - transactions (array of raw transactions, parents first)

use std::io::{Cursor, Read};

/// BEEF V1 version marker (4 bytes: 0x0100beef per BRC-62)
pub const BEEF_V1_MARKER: [u8; 4] = [0x01, 0x00, 0xbe, 0xef];

/// BEEF V2 version marker (4 bytes: 0x0200beef per BRC-96) - DEFAULT
pub const BEEF_V2_MARKER: [u8; 4] = [0x02, 0x00, 0xbe, 0xef];

/// Use BEEF V2 as default (matches TypeScript SDK)
pub const BEEF_VERSION_MARKER: [u8; 4] = BEEF_V2_MARKER;

/// Represents a parsed BEEF structure
#[derive(Debug, Clone)]
pub struct Beef {
    pub version: [u8; 4],  // 4-byte version marker
    pub bumps: Vec<MerkleProof>,
    pub transactions: Vec<Vec<u8>>, // Raw transaction bytes (parents first, main tx last)
    /// Maps transaction index to BUMP index (None if no BUMP)
    pub tx_to_bump: Vec<Option<usize>>,
}

/// Represents a merkle proof (BUMPs - Block Unspent Merkle Proofs)
#[derive(Debug, Clone)]
pub struct MerkleProof {
    pub block_height: u32,
    pub tree_height: u8,
    pub levels: Vec<Vec<Vec<u8>>>, // levels[i] = nodes at level i, each node = [offset][flags][hash?]
}

impl Beef {
    /// Parse BEEF format from hex string
    pub fn from_hex(hex: &str) -> Result<Self, String> {
        let bytes = hex::decode(hex)
            .map_err(|e| format!("Invalid BEEF hex: {}", e))?;
        Self::from_bytes(&bytes)
    }

    /// Parse Atomic BEEF format from base64 string
    ///
    /// Atomic BEEF format:
    /// - 4 bytes: 0x01010101 (magic prefix)
    /// - 32 bytes: Subject TXID (big-endian)
    /// - Variable: Standard BEEF structure
    ///
    /// Returns (subject_txid, Beef)
    pub fn from_atomic_beef_base64(base64_str: &str) -> Result<(String, Self), String> {
        // Decode from base64
        use base64::{Engine as _, engine::general_purpose};
        let bytes = general_purpose::STANDARD.decode(base64_str)
            .map_err(|e| format!("Invalid base64: {}", e))?;

        Self::from_atomic_beef_bytes(&bytes)
    }

    /// Parse Atomic BEEF format from raw bytes
    ///
    /// Returns (subject_txid_hex, Beef)
    pub fn from_atomic_beef_bytes(bytes: &[u8]) -> Result<(String, Self), String> {
        if bytes.len() < 36 {
            return Err(format!("Atomic BEEF too short: {} bytes (need at least 36)", bytes.len()));
        }

        // Check magic prefix (4 bytes: 0x01010101)
        if &bytes[0..4] != &[0x01, 0x01, 0x01, 0x01] {
            return Err(format!("Invalid Atomic BEEF magic prefix: {:02x?}", &bytes[0..4]));
        }

        // Extract subject TXID (32 bytes, big-endian)
        let txid_be = &bytes[4..36];
        let mut txid_le = txid_be.to_vec();
        txid_le.reverse(); // Convert big-endian to little-endian for hex display
        let txid_hex = hex::encode(&txid_le);

        // Parse standard BEEF structure (rest of bytes)
        let beef = Self::from_bytes(&bytes[36..])?;

        Ok((txid_hex, beef))
    }

    /// Parse BEEF format from raw bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        let mut cursor = Cursor::new(bytes);

        // Read 4-byte version marker
        let mut version = [0u8; 4];
        cursor.read_exact(&mut version)
            .map_err(|e| format!("Failed to read BEEF version marker: {}", e))?;

        // Support both V1 and V2
        let is_v2 = if version == BEEF_V2_MARKER {
            true
        } else if version == BEEF_V1_MARKER {
            false
        } else {
            return Err(format!("Unsupported BEEF version: {:02x?}", version));
        };

        // Read number of BUMPs (merkle proofs)
        let num_bumps = read_varint(&mut cursor)?;

        // Read BUMPs
        let mut bumps = Vec::new();
        for _ in 0..num_bumps {
            bumps.push(read_bump(&mut cursor)?);
        }

        // Read number of transactions
        let num_txs = read_varint(&mut cursor)?;

        // Read transactions with their BUMP associations
        let mut transactions = Vec::new();
        let mut tx_to_bump = Vec::new();

        if is_v2 {
            // BEEF V2 format: [format_byte][bump_index?][raw_tx or txid]
            for _ in 0..num_txs {
                let format_byte = read_u8(&mut cursor)?;

                match format_byte {
                    0x00 => {
                        // Raw transaction without BUMP
                        let tx_bytes = read_transaction(&mut cursor)?;
                        transactions.push(tx_bytes);
                        tx_to_bump.push(None);
                    }
                    0x01 => {
                        // Raw transaction with BUMP index
                        let bump_index = read_varint(&mut cursor)? as usize;
                        let tx_bytes = read_transaction(&mut cursor)?;
                        transactions.push(tx_bytes);
                        tx_to_bump.push(Some(bump_index));
                    }
                    0x02 => {
                        // TXID only (32 bytes)
                        let mut txid_bytes = [0u8; 32];
                        cursor.read_exact(&mut txid_bytes)
                            .map_err(|e| format!("Failed to read txid: {}", e))?;
                        // For txid-only, we store it as a special marker
                        // For now, we'll skip txid-only transactions
                        return Err(format!("TXID-only transactions (format 0x02) not yet supported"));
                    }
                    _ => {
                        return Err(format!("Invalid BEEF V2 format byte: 0x{:02x}", format_byte));
                    }
                }
            }
        } else {
            // BEEF V1 format: [raw_tx][bump_flag][bump_index?]
            for _ in 0..num_txs {
                let tx_bytes = read_transaction(&mut cursor)?;
                transactions.push(tx_bytes);

                // Read BUMP association flag
                let has_bump = read_u8(&mut cursor)?;
                if has_bump == 0x01 {
                    // Read BUMP index
                    let bump_index = read_varint(&mut cursor)? as usize;
                    tx_to_bump.push(Some(bump_index));
                } else {
                    tx_to_bump.push(None);
                }
            }
        }

        Ok(Beef {
            version,
            bumps,
            transactions,
            tx_to_bump,
        })
    }

    /// Get the main transaction (last in the array)
    pub fn main_transaction(&self) -> Option<&Vec<u8>> {
        self.transactions.last()
    }

    /// Get parent transactions (all except the last)
    pub fn parent_transactions(&self) -> &[Vec<u8>] {
        if self.transactions.len() > 1 {
            &self.transactions[..self.transactions.len() - 1]
        } else {
            &[]
        }
    }

    /// Check if BEEF has merkle proofs for SPV
    pub fn has_proofs(&self) -> bool {
        !self.bumps.is_empty()
    }

    /// Create a new empty BEEF structure
    pub fn new() -> Self {
        Beef {
            version: BEEF_VERSION_MARKER,
            bumps: Vec::new(),
            transactions: Vec::new(),
            tx_to_bump: Vec::new(),
        }
    }

    /// Add a parent transaction (input UTXO transaction)
    /// Parent transactions must be added before the main transaction
    /// Returns the index of the added transaction
    pub fn add_parent_transaction(&mut self, tx_bytes: Vec<u8>) -> usize {
        // Insert before the main transaction (if any)
        let tx_index = if !self.transactions.is_empty() {
            let main_tx = self.transactions.pop().unwrap();
            let main_bump = self.tx_to_bump.pop();
            let idx = self.transactions.len();
            self.transactions.push(tx_bytes);
            self.tx_to_bump.push(None); // No BUMP initially
            self.transactions.push(main_tx);
            if let Some(bump) = main_bump {
                self.tx_to_bump.push(bump);
            } else {
                self.tx_to_bump.push(None);
            }
            idx
        } else {
            self.transactions.push(tx_bytes);
            self.tx_to_bump.push(None); // No BUMP initially
            self.transactions.len() - 1
        };
        tx_index
    }

    /// Set the main transaction (the signed transaction we're sending)
    pub fn set_main_transaction(&mut self, tx_bytes: Vec<u8>) {
        // Main transaction goes last (unconfirmed, so no BUMP)
        self.transactions.push(tx_bytes);
        self.tx_to_bump.push(None);
    }

    /// Add a Merkle proof (BUMP) from WhatsOnChain merkleproof format
    ///
    /// WhatsOnChain /merkleproof response format:
    /// {
    ///   "block_height": 918980,
    ///   "merkle": ["hash1", "hash2", ...],
    ///   "pos": 4805
    /// }
    ///
    /// # Arguments
    /// * `tx_index` - Index of the transaction this BUMP belongs to
    /// * `proof_json` - WhatsOnChain Merkle proof JSON
    /// Convert WhatsOnChain TSC proof format to proper BUMP format
    /// TSC format: {height: number, index: number, nodes: string[]}
    /// BUMP format requires proper offsets computed from the transaction index
    pub fn add_tsc_merkle_proof(&mut self, txid: &str, tx_index: usize, tsc_json: &serde_json::Value) -> Result<(), String> {
        let block_height = tsc_json["height"]
            .as_u64()
            .ok_or("Missing height in TSC proof")? as u32;

        let tx_index_in_block = tsc_json["index"]
            .as_u64()
            .ok_or("Missing index in TSC proof")?;

        let nodes = tsc_json["nodes"]
            .as_array()
            .ok_or("Missing nodes array in TSC proof")?;

        let _tree_height = nodes.len() as u8;

        // Convert TSC proof to BUMP format
        // This matches the TypeScript SDK's convertProofToMerklePath function
        let merkle_proof = tsc_proof_to_bump(txid, block_height, tx_index_in_block, nodes)?;

        // Add BUMP and update mapping
        let bump_index = self.bumps.len();
        self.bumps.push(merkle_proof);

        // Associate this BUMP with the transaction
        if tx_index < self.tx_to_bump.len() {
            self.tx_to_bump[tx_index] = Some(bump_index);
        } else {
            return Err(format!("Transaction index {} out of bounds (have {} transactions)", tx_index, self.tx_to_bump.len()));
        }

        Ok(())
    }


    /// Serialize BEEF to bytes
    pub fn to_bytes(&self) -> Result<Vec<u8>, String> {
        let mut bytes = Vec::new();

        // Write 4-byte version marker (0x0100beef)
        bytes.extend_from_slice(&self.version);

        // Write number of BUMPs (merkle proofs)
        write_varint(&mut bytes, self.bumps.len() as u64);

        // Write BUMPs (if any)
        for bump in &self.bumps {
            write_bump(&mut bytes, bump)?;
        }

        // Write number of transactions
        write_varint(&mut bytes, self.transactions.len() as u64);
        log::info!("   🔢 Writing {} transactions to BEEF", self.transactions.len());

        // Write transactions using BEEF V2 format
        // V2 format: [format_byte][bump_index][raw_tx] or [format_byte][raw_tx]
        for (i, tx) in self.transactions.iter().enumerate() {
            let tx_start_pos = bytes.len();

            log::info!("      TX {}: {} bytes, starts at BEEF offset {}", i, tx.len(), tx_start_pos);
            log::info!("         First 40 bytes: {}", hex::encode(&tx[..40.min(tx.len())]));

            // BEEF V2: Write format byte and optional BUMP index BEFORE transaction
            if i < self.tx_to_bump.len() {
                if let Some(bump_index) = self.tx_to_bump[i] {
                    // Has BUMP: format byte 0x01, then BUMP index, then transaction
                    bytes.push(0x01); // TX_DATA_FORMAT.RAWTX_AND_BUMP_INDEX
                    write_varint(&mut bytes, bump_index as u64);
                    log::info!("         BEEF V2 format: 0x01 (has BUMP at index {})", bump_index);
                } else {
                    // No BUMP: format byte 0x00, then transaction
                    bytes.push(0x00); // TX_DATA_FORMAT.RAWTX
                    log::info!("         BEEF V2 format: 0x00 (no BUMP)");
                }
            } else {
                // No mapping available, assume no BUMP
                bytes.push(0x00); // TX_DATA_FORMAT.RAWTX
                log::info!("         BEEF V2 format: 0x00 (no mapping)");
            }

            // Write raw transaction bytes (Bitcoin txs are self-describing, no length prefix)
            bytes.extend(tx);
        }

        Ok(bytes)
    }

    /// Serialize BEEF to hex string
    pub fn to_hex(&self) -> Result<String, String> {
        let bytes = self.to_bytes()?;
        Ok(hex::encode(bytes))
    }

    /// Serialize to Atomic BEEF hex string (BRC-95)
    ///
    /// Atomic BEEF format:
    /// - 4 bytes: 0x01010101 (magic prefix)
    /// - 32 bytes: Subject TXID (big-endian)
    /// - Variable: Standard BEEF structure
    ///
    /// # Arguments
    /// * `txid_hex` - Transaction ID as hex string (64 characters)
    ///
    /// # Returns
    /// Hex-encoded Atomic BEEF structure
    pub fn to_atomic_beef_hex(&self, txid_hex: &str) -> Result<String, String> {
        // Decode TXID from hex
        let txid_bytes = hex::decode(txid_hex)
            .map_err(|e| format!("Invalid TXID hex: {}", e))?;

        if txid_bytes.len() != 32 {
            return Err(format!("TXID must be 32 bytes, got {}", txid_bytes.len()));
        }

        let mut atomic_beef = Vec::new();

        // 1. Write Atomic BEEF magic prefix (4 bytes: 0x01010101 per BRC-95)
        atomic_beef.extend(&[0x01, 0x01, 0x01, 0x01]);

        // 2. Write TXID in big-endian format (reverse of little-endian Bitcoin format)
        let mut txid_be = txid_bytes.clone();
        txid_be.reverse();
        atomic_beef.extend(&txid_be);

        // 3. Append standard BEEF structure
        let beef_bytes = self.to_bytes()?;
        atomic_beef.extend(&beef_bytes);

        Ok(hex::encode(atomic_beef))
    }
}

/// Read a single byte
fn read_u8(cursor: &mut Cursor<&[u8]>) -> Result<u8, String> {
    let mut buf = [0u8; 1];
    cursor.read_exact(&mut buf)
        .map_err(|e| format!("Failed to read byte: {}", e))?;
    Ok(buf[0])
}

/// Read a variable-length integer (Bitcoin varint format)
fn read_varint(cursor: &mut Cursor<&[u8]>) -> Result<u64, String> {
    let first = read_u8(cursor)?;

    match first {
        0..=0xfc => Ok(first as u64),
        0xfd => {
            let mut buf = [0u8; 2];
            cursor.read_exact(&mut buf)
                .map_err(|e| format!("Failed to read varint: {}", e))?;
            Ok(u16::from_le_bytes(buf) as u64)
        }
        0xfe => {
            let mut buf = [0u8; 4];
            cursor.read_exact(&mut buf)
                .map_err(|e| format!("Failed to read varint: {}", e))?;
            Ok(u32::from_le_bytes(buf) as u64)
        }
        0xff => {
            let mut buf = [0u8; 8];
            cursor.read_exact(&mut buf)
                .map_err(|e| format!("Failed to read varint: {}", e))?;
            Ok(u64::from_le_bytes(buf))
        }
    }
}

/// Read a merkle proof (BUMP) - matches write_bump format
fn read_bump(cursor: &mut Cursor<&[u8]>) -> Result<MerkleProof, String> {
    // Read block height as varint
    let block_height = read_varint(cursor)? as u32;

    // Read tree height
    let tree_height = read_u8(cursor)?;

    // Read nodes grouped by level
    let mut levels = Vec::with_capacity(tree_height as usize);

    for _ in 0..tree_height {
        let num_nodes_at_level = read_varint(cursor)? as usize;
        let mut nodes_at_level = Vec::with_capacity(num_nodes_at_level);

        for _ in 0..num_nodes_at_level {
            // Read offset
            let offset = read_varint(cursor)?;

            // Read flags
            let flags = read_u8(cursor)?;

            // Build node bytes: [offset][flags][hash?]
            let mut node = Vec::new();
            write_varint(&mut node, offset);
            node.push(flags);

            // If not duplicate, read hash (32 bytes)
            if (flags & 0x01) == 0 {
                let mut hash = vec![0u8; 32];
                cursor.read_exact(&mut hash)
                    .map_err(|e| format!("Failed to read merkle hash: {}", e))?;
                node.extend(hash);
            }

            nodes_at_level.push(node);
        }

        levels.push(nodes_at_level);
    }

    Ok(MerkleProof {
        block_height,
        tree_height,
        levels,
    })
}

/// Read a raw transaction
fn read_transaction(cursor: &mut Cursor<&[u8]>) -> Result<Vec<u8>, String> {
    // BRC-62: Bitcoin transactions are self-describing (NO length prefix!)
    // We need to parse the transaction structure to know its length

    use std::io::Seek;

    let start_pos = cursor.position();

    // Read version (4 bytes)
    let mut version = [0u8; 4];
    cursor.read_exact(&mut version)
        .map_err(|e| format!("Failed to read tx version: {}", e))?;

    // Read input count
    let input_count = read_varint(cursor)?;

    // Read each input
    for _ in 0..input_count {
        // prev txid (32 bytes) + prev vout (4 bytes)
        let mut skip = [0u8; 36];
        cursor.read_exact(&mut skip)
            .map_err(|e| format!("Failed to read input prev: {}", e))?;

        // script length + script bytes
        let script_len = read_varint(cursor)?;
        let mut script = vec![0u8; script_len as usize];
        cursor.read_exact(&mut script)
            .map_err(|e| format!("Failed to read input script: {}", e))?;

        // sequence (4 bytes)
        let mut seq = [0u8; 4];
        cursor.read_exact(&mut seq)
            .map_err(|e| format!("Failed to read sequence: {}", e))?;
    }

    // Read output count
    let output_count = read_varint(cursor)?;

    // Read each output
    for _ in 0..output_count {
        // value (8 bytes)
        let mut value = [0u8; 8];
        cursor.read_exact(&mut value)
            .map_err(|e| format!("Failed to read output value: {}", e))?;

        // script length + script bytes
        let script_len = read_varint(cursor)?;
        let mut script = vec![0u8; script_len as usize];
        cursor.read_exact(&mut script)
            .map_err(|e| format!("Failed to read output script: {}", e))?;
    }

    // Read locktime (4 bytes)
    let mut locktime = [0u8; 4];
    cursor.read_exact(&mut locktime)
        .map_err(|e| format!("Failed to read locktime: {}", e))?;

    let end_pos = cursor.position();
    let tx_len = (end_pos - start_pos) as usize;

    // Go back to start and read the full transaction
    cursor.seek(std::io::SeekFrom::Start(start_pos))
        .map_err(|e| format!("Failed to seek back: {}", e))?;

    let mut tx_bytes = vec![0u8; tx_len];
    cursor.read_exact(&mut tx_bytes)
        .map_err(|e| format!("Failed to read full transaction: {}", e))?;

    Ok(tx_bytes)
}

/// Parse a raw Bitcoin transaction to extract metadata
#[derive(Debug, Clone)]
pub struct ParsedTransaction {
    pub version: u32,
    pub inputs: Vec<ParsedInput>,
    pub outputs: Vec<ParsedOutput>,
    pub lock_time: u32,
}

#[derive(Debug, Clone)]
pub struct ParsedInput {
    pub prev_txid: String,
    pub prev_vout: u32,
    pub script: Vec<u8>,
    pub sequence: u32,
}

#[derive(Debug, Clone)]
pub struct ParsedOutput {
    pub value: i64,
    pub script: Vec<u8>,
}

impl ParsedTransaction {
    /// Parse a raw transaction from bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        let mut cursor = Cursor::new(bytes);

        // Read version
        let mut version_buf = [0u8; 4];
        cursor.read_exact(&mut version_buf)
            .map_err(|e| format!("Failed to read version: {}", e))?;
        let version = u32::from_le_bytes(version_buf);

        // Read inputs
        let input_count = read_varint(&mut cursor)?;
        let mut inputs = Vec::new();
        for _ in 0..input_count {
            // Read previous output (txid + vout)
            let mut prev_txid_bytes = [0u8; 32];
            cursor.read_exact(&mut prev_txid_bytes)
                .map_err(|e| format!("Failed to read prev txid: {}", e))?;

            // Reverse for display (Bitcoin uses little-endian)
            let prev_txid = hex::encode(prev_txid_bytes.iter().rev().copied().collect::<Vec<u8>>());

            let mut prev_vout_buf = [0u8; 4];
            cursor.read_exact(&mut prev_vout_buf)
                .map_err(|e| format!("Failed to read prev vout: {}", e))?;
            let prev_vout = u32::from_le_bytes(prev_vout_buf);

            // Read script
            let script_len = read_varint(&mut cursor)?;
            let mut script = vec![0u8; script_len as usize];
            cursor.read_exact(&mut script)
                .map_err(|e| format!("Failed to read script: {}", e))?;

            // Read sequence
            let mut sequence_buf = [0u8; 4];
            cursor.read_exact(&mut sequence_buf)
                .map_err(|e| format!("Failed to read sequence: {}", e))?;
            let sequence = u32::from_le_bytes(sequence_buf);

            inputs.push(ParsedInput {
                prev_txid,
                prev_vout,
                script,
                sequence,
            });
        }

        // Read outputs
        let output_count = read_varint(&mut cursor)?;
        let mut outputs = Vec::new();
        for _ in 0..output_count {
            // Read value
            let mut value_buf = [0u8; 8];
            cursor.read_exact(&mut value_buf)
                .map_err(|e| format!("Failed to read value: {}", e))?;
            let value = i64::from_le_bytes(value_buf);

            // Read script
            let script_len = read_varint(&mut cursor)?;
            let mut script = vec![0u8; script_len as usize];
            cursor.read_exact(&mut script)
                .map_err(|e| format!("Failed to read script: {}", e))?;

            outputs.push(ParsedOutput {
                value,
                script,
            });
        }

        // Read locktime
        let mut locktime_buf = [0u8; 4];
        cursor.read_exact(&mut locktime_buf)
            .map_err(|e| format!("Failed to read locktime: {}", e))?;
        let lock_time = u32::from_le_bytes(locktime_buf);

        Ok(ParsedTransaction {
            version,
            inputs,
            outputs,
            lock_time,
        })
    }

    /// Parse from hex string
    pub fn from_hex(hex: &str) -> Result<Self, String> {
        let bytes = hex::decode(hex)
            .map_err(|e| format!("Invalid hex: {}", e))?;
        Self::from_bytes(&bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_varint_parsing() {
        // Small value (< 0xfd)
        let bytes = vec![0x12];
        let mut cursor = Cursor::new(bytes.as_slice());
        assert_eq!(read_varint(&mut cursor).unwrap(), 0x12);

        // 2-byte value
        let bytes = vec![0xfd, 0x00, 0x01];
        let mut cursor = Cursor::new(bytes.as_slice());
        assert_eq!(read_varint(&mut cursor).unwrap(), 0x0100);
    }

    #[test]
    fn test_beef_roundtrip() {
        // Use REAL Bitcoin transactions from BRC-62 spec example

        // Parent transaction (from spec)
        let parent_tx_hex = "0100000001cd4e4cac3c7b56920d1e7655e7e260d31f29d9a388d04910f1bbd72304a79029010000006b483045022100e75279a205a547c445719420aa3138bf14743e3f42618e5f86a19bde14bb95f7022064777d34776b05d816daf1699493fcdf2ef5a5ab1ad710d9c97bfb5b8f7cef3641210263e2dee22b1ddc5e11f6fab8bcd2378bdd19580d640501ea956ec0e786f93e76ffffffff013e660000000000001976a9146bfd5c7fbe21529d45803dbcf0c87dd3c71efbc288ac00000000";
        let parent_tx = hex::decode(parent_tx_hex).unwrap();

        // Main transaction (from spec)
        let main_tx_hex = "0100000001ac4e164f5bc16746bb0868404292ac8318bbac3800e4aad13a014da427adce3e000000006a47304402203a61a2e931612b4bda08d541cfb980885173b8dcf64a3471238ae7abcd368d6402204cbf24f04b9aa2256d8901f0ed97866603d2be8324c2bfb7a37bf8fc90edd5b441210263e2dee22b1ddc5e11f6fab8bcd2378bdd19580d640501ea956ec0e786f93e76ffffffff013c660000000000001976a9146bfd5c7fbe21529d45803dbcf0c87dd3c71efbc288ac00000000";
        let main_tx = hex::decode(main_tx_hex).unwrap();

        // Create a test BEEF with 1 parent + 1 main transaction
        let mut beef = Beef::new();

        // Add parent transaction
        let tx_index = beef.add_parent_transaction(parent_tx.clone());

        // Calculate parent TXID for TSC proof
        use sha2::{Sha256, Digest};
        let first_hash = Sha256::digest(&parent_tx);
        let second_hash = Sha256::digest(&first_hash);
        let parent_txid = hex::encode(second_hash.iter().rev().copied().collect::<Vec<u8>>());

        // Add BUMP for parent using TSC format (converted from old WOC format)
        let tsc_proof = serde_json::json!({
            "height": 918980,
            "index": 0,  // Transaction index in block (using 0 for test)
            "nodes": [
                "9b18d77b48fde9b46d54b75d372e30a74cba0114cad4796f8f1d91946866a8bd",
                "45b8d1a256e4de964d2a70408e3ae4265b43544425ea40f370cd76d367575b0e"
            ]
        });
        beef.add_tsc_merkle_proof(&parent_txid, tx_index, &tsc_proof).unwrap();

        // Add main transaction
        beef.set_main_transaction(main_tx.clone());

        // Serialize to bytes
        let beef_bytes = beef.to_bytes().unwrap();

        println!("Generated BEEF: {} bytes", beef_bytes.len());
        println!("Hex: {}", hex::encode(&beef_bytes[..40.min(beef_bytes.len())]));

        // Parse it back
        let parsed_beef = Beef::from_bytes(&beef_bytes).unwrap();

        // Verify structure
        assert_eq!(parsed_beef.bumps.len(), 1, "Should have 1 BUMP");
        assert_eq!(parsed_beef.transactions.len(), 2, "Should have 2 transactions");
        assert_eq!(parsed_beef.transactions[0], parent_tx, "Parent tx should match");
        assert_eq!(parsed_beef.transactions[1], main_tx, "Main tx should match");
        assert_eq!(parsed_beef.bumps[0].block_height, 918980, "Block height should match");

        println!("✅ Round-trip test passed!");
    }

    #[test]
    fn test_beef_bump_associations() {
        let mut beef = Beef::new();

        // Add parent with BUMP
        let parent_tx = vec![0xAA; 10];
        let tx_idx = beef.add_parent_transaction(parent_tx.clone());

        // Calculate parent TXID for TSC proof
        use sha2::{Sha256, Digest};
        let first_hash = Sha256::digest(&parent_tx);
        let second_hash = Sha256::digest(&first_hash);
        let parent_txid = hex::encode(second_hash.iter().rev().copied().collect::<Vec<u8>>());

        // Add BUMP using TSC format
        let tsc_proof = serde_json::json!({
            "height": 100,
            "index": 0,  // Transaction index in block (using 0 for test)
            "nodes": ["9b18d77b48fde9b46d54b75d372e30a74cba0114cad4796f8f1d91946866a8bd"]
        });
        beef.add_tsc_merkle_proof(&parent_txid, tx_idx, &tsc_proof).unwrap();

        // Add main tx (no BUMP)
        let main_tx = vec![0xBB; 10];
        beef.set_main_transaction(main_tx);

        // Verify tx_to_bump mapping
        assert_eq!(beef.tx_to_bump.len(), 2, "Should have 2 tx-to-bump entries");
        assert_eq!(beef.tx_to_bump[0], Some(0), "Parent should map to BUMP 0");
        assert_eq!(beef.tx_to_bump[1], None, "Main tx should have no BUMP");

        println!("✅ BUMP association test passed!");
    }
}

/// Write a variable-length integer (Bitcoin varint format)
fn write_varint(bytes: &mut Vec<u8>, value: u64) {
    if value < 0xfd {
        bytes.push(value as u8);
    } else if value <= 0xffff {
        bytes.push(0xfd);
        bytes.extend(&(value as u16).to_le_bytes());
    } else if value <= 0xffffffff {
        bytes.push(0xfe);
        bytes.extend(&(value as u32).to_le_bytes());
    } else {
        bytes.push(0xff);
        bytes.extend(&value.to_le_bytes());
    }
}

/// Convert WhatsOnChain TSC proof to BUMP format
/// This matches the TypeScript SDK's convertProofToMerklePath function
fn tsc_proof_to_bump(
    txid: &str,
    block_height: u32,
    tx_index: u64,
    nodes: &[serde_json::Value]
) -> Result<MerkleProof, String> {
    // Each level in the BUMP contains nodes with offset+hash+flags
    // We need to compute offsets based on tx_index
    let tree_height = nodes.len() as u8;

    // Organize nodes by level: levels[i] = Vec of nodes at level i
    let mut levels: Vec<Vec<Vec<u8>>> = Vec::with_capacity(tree_height as usize);

    let mut current_index = tx_index;

    for level in 0..tree_height as usize {
        let node_str = nodes[level].as_str()
            .ok_or(format!("Invalid node at level {}", level))?;

        let is_odd = current_index % 2 == 1;
        let sibling_offset = if is_odd { current_index - 1 } else { current_index + 1 };

        let mut nodes_at_level = Vec::new();

        // For level 0, we need to include the TXID as well
        if level == 0 {
            // The TypeScript SDK adds nodes in sorted order by offset
            // If tx is even (left), sibling is odd (right): [txid, sibling]
            // If tx is odd (right), sibling is even (left): [sibling, txid]

            // Build TXID node
            let mut txid_node = Vec::new();
            write_varint(&mut txid_node, current_index);
            txid_node.push(0x02); // txid flag
            let txid_hash = hex::decode(txid)
                .map_err(|e| format!("Invalid txid: {}", e))?;
            let mut reversed_txid = txid_hash;
            reversed_txid.reverse();
            txid_node.extend(&reversed_txid);

            // Build sibling node
            let mut sibling_node = Vec::new();
            write_varint(&mut sibling_node, sibling_offset);
            if node_str == "*" {
                sibling_node.push(0x01); // duplicate flag
            } else {
                sibling_node.push(0x00); // regular hash
                let node_hash = hex::decode(node_str)
                    .map_err(|e| format!("Invalid hash at level {}: {}", level, e))?;
                let mut reversed_hash = node_hash;
                reversed_hash.reverse();
                sibling_node.extend(&reversed_hash);
            }

            // Add in sorted order by offset
            if is_odd {
                nodes_at_level.push(sibling_node); // lower offset first
                nodes_at_level.push(txid_node);
            } else {
                nodes_at_level.push(txid_node); // lower offset first
                nodes_at_level.push(sibling_node);
            }
        } else {
            // For other levels, just add the sibling node
            let mut node = Vec::new();
            write_varint(&mut node, sibling_offset);

            if node_str == "*" {
                node.push(0x01); // duplicate flag
            } else {
                node.push(0x00); // regular hash
                let node_hash = hex::decode(node_str)
                    .map_err(|e| format!("Invalid hash at level {}: {}", level, e))?;
                let mut reversed_hash = node_hash;
                reversed_hash.reverse();
                node.extend(&reversed_hash);
            }
            nodes_at_level.push(node);
        }

        levels.push(nodes_at_level);

        // Move to next level (parent node index)
        current_index >>= 1;
    }

    Ok(MerkleProof {
        block_height,
        tree_height,
        levels,
    })
}

/// Write a merkle proof (BUMP)
fn write_bump(bytes: &mut Vec<u8>, bump: &MerkleProof) -> Result<(), String> {
    // Write block height as varint (NOT fixed 4 bytes!)
    write_varint(bytes, bump.block_height as u64);

    // Write tree height
    bytes.push(bump.tree_height);

    // Write nodes grouped by level (proper BUMP format per BRC-74)
    for level_nodes in &bump.levels {
        // Write number of nodes at this level
        write_varint(bytes, level_nodes.len() as u64);

        // Write each node (pre-formatted as [offset][flags][hash?])
        for node in level_nodes {
            bytes.extend(node);
        }
    }

    Ok(())
}
