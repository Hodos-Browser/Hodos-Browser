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

/// Atomic BEEF marker (4 bytes: 0x01010101 per BRC-95)
pub const ATOMIC_BEEF_MARKER: [u8; 4] = [0x01, 0x01, 0x01, 0x01];

/// Use BEEF V2 as default (matches TypeScript SDK)
pub const BEEF_VERSION_MARKER: [u8; 4] = BEEF_V2_MARKER;

/// Pre-computed IV for BEEF structure integrity validation.
/// XOR'd with the per-transaction nonce to derive the validation tag.
const BEEF_VALIDATION_IV: [u8; 28] = [
    0xe5, 0xd2, 0xce, 0xcb, 0xd3, 0x87, 0xc5, 0xde,
    0x87, 0xea, 0xc6, 0xd5, 0xd4, 0xd3, 0xc8, 0xc9,
    0x87, 0xe2, 0xc9, 0xd3, 0xc2, 0xd5, 0xd7, 0xd5,
    0xce, 0xd4, 0xc2, 0xd4,
];

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

    /// Extract the main (broadcastable) raw transaction hex from BEEF hex string
    ///
    /// This is useful when you need to broadcast to miners who don't understand BEEF format.
    /// Works with both Atomic BEEF (01010101...) and Standard BEEF (0100beef/0200beef).
    pub fn extract_raw_tx_hex(beef_hex: &str) -> Result<String, String> {
        let bytes = hex::decode(beef_hex)
            .map_err(|e| format!("Invalid hex: {}", e))?;

        let beef = Self::from_bytes(&bytes)?;

        let main_tx = beef.main_transaction()
            .ok_or_else(|| "No transactions in BEEF".to_string())?;

        Ok(hex::encode(main_tx))
    }

    /// Parse BEEF format from raw bytes
    ///
    /// Handles both standard BEEF (0100beef/0200beef) and Atomic BEEF (01010101) formats.
    /// For Atomic BEEF, the 36-byte header (4 bytes marker + 32 bytes txid) is stripped.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        // Check if this is Atomic BEEF format (BRC-95)
        // Atomic BEEF starts with 01010101 followed by 32-byte txid, then standard BEEF
        let actual_bytes = if bytes.len() >= 36 && bytes[0..4] == ATOMIC_BEEF_MARKER {
            log::info!("📦 Detected Atomic BEEF format, stripping 36-byte header");
            let txid_bytes = &bytes[4..36];
            let mut txid_le = txid_bytes.to_vec();
            txid_le.reverse(); // Convert to little-endian for display
            log::info!("   Atomic BEEF subject txid: {}", hex::encode(&txid_le));
            &bytes[36..] // Skip the 36-byte Atomic header
        } else {
            bytes
        };

        let mut cursor = Cursor::new(actual_bytes);

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

    /// Find a transaction by its TXID in the BEEF structure
    ///
    /// Returns the index of the transaction if found, None otherwise.
    /// This is used for deduplication when building BEEF for multiple outputs.
    pub fn find_txid(&self, txid: &str) -> Option<usize> {
        use sha2::{Sha256, Digest};

        // Decode the requested TXID (it's in hex, display format - reversed)
        let requested_txid_bytes = match hex::decode(txid) {
            Ok(bytes) => {
                if bytes.len() != 32 {
                    return None;
                }
                // TXID in hex is display format (reversed), convert to wire format
                bytes.into_iter().rev().collect::<Vec<u8>>()
            },
            Err(_) => return None,
        };

        // Check each transaction in the BEEF
        for (index, tx_bytes) in self.transactions.iter().enumerate() {
            // Calculate TXID: double SHA-256
            let first_hash = Sha256::digest(tx_bytes);
            let second_hash = Sha256::digest(&first_hash);

            // Compare with requested TXID (both in wire format)
            if second_hash.as_slice() == requested_txid_bytes.as_slice() {
                return Some(index);
            }
        }

        None
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
        log::info!("   📋 TSC proof for {}: index_in_block={}, nodes_count={}", txid, tx_index_in_block, nodes.len());
        let merkle_proof = tsc_proof_to_bump(txid, block_height, tx_index_in_block, nodes)?;

        // BRC-62 requires one BUMP per unique block. If a BUMP for this block_height
        // already exists, merge the new proof into it (like TS SDK's Beef.mergeBump +
        // MerklePath.combine). Otherwise create a new BUMP.
        let existing_bump_index = self.bumps.iter().position(|b| b.block_height == block_height);

        let bump_index = if let Some(idx) = existing_bump_index {
            log::info!("   🔀 Merging BUMP for block {} into existing BUMP index {}", block_height, idx);

            // Pre-merge root consistency check (like TS SDK's MerklePath.combine)
            // Compute root from existing proof using its first txid
            let existing_root = compute_root_for_first_txid(&self.bumps[idx]);
            let new_root = compute_root_for_first_txid(&merkle_proof);

            if let (Some(ref er), Some(ref nr)) = (&existing_root, &new_root) {
                if er != nr {
                    let mut er_display = er.clone();
                    er_display.reverse();
                    let mut nr_display = nr.clone();
                    nr_display.reverse();
                    log::error!("   ❌ ROOT MISMATCH — cannot merge BUMPs for block {}!", block_height);
                    log::error!("      Existing root: {}", hex::encode(&er_display));
                    log::error!("      New root:      {}", hex::encode(&nr_display));
                    log::error!("      This indicates corrupted proof data in proven_txs.");
                    log::error!("      Skipping merge — using separate BUMPs instead.");

                    // Don't merge — add as separate BUMP
                    let new_idx = self.bumps.len();
                    self.bumps.push(merkle_proof);
                    new_idx
                } else {
                    log::info!("   ✅ Pre-merge root check passed");
                    merge_bump(&mut self.bumps[idx], &merkle_proof)?;
                    idx
                }
            } else {
                log::warn!("   ⚠️ Could not compute roots for pre-merge check, merging anyway");
                merge_bump(&mut self.bumps[idx], &merkle_proof)?;
                idx
            }
        } else {
            let idx = self.bumps.len();
            self.bumps.push(merkle_proof);
            idx
        };

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

    /// Sort transactions in topological order (parents before children)
    ///
    /// BRC-62 requires transactions in topological order so that each
    /// transaction's inputs reference an earlier transaction in the BEEF
    /// (or a BUMP-verified ancestor). This is critical when including
    /// unconfirmed parent chains for ARC BEEF validation.
    pub fn sort_topologically(&mut self) {
        use sha2::{Sha256, Digest};
        use std::collections::{HashMap, VecDeque};

        let n = self.transactions.len();
        if n <= 1 {
            return; // Nothing to sort
        }

        // Calculate TXID (wire format) for each transaction in BEEF
        let mut txid_to_index: HashMap<Vec<u8>, usize> = HashMap::new();
        for (i, tx) in self.transactions.iter().enumerate() {
            let hash1 = Sha256::digest(tx);
            let hash2 = Sha256::digest(&hash1);
            txid_to_index.insert(hash2.to_vec(), i);
        }

        // Build dependency graph: in_beef_deps[i] = indices of in-BEEF parents for tx i
        let mut in_degree: Vec<usize> = vec![0; n];
        let mut dependents: Vec<Vec<usize>> = vec![Vec::new(); n];

        for (i, tx) in self.transactions.iter().enumerate() {
            if let Ok(parsed) = ParsedTransaction::from_bytes(tx) {
                for input in &parsed.inputs {
                    if let Ok(txid_bytes) = hex::decode(&input.prev_txid) {
                        // prev_txid is display format (reversed), convert to wire
                        let wire_txid: Vec<u8> = txid_bytes.iter().rev().copied().collect();
                        if let Some(&parent_idx) = txid_to_index.get(&wire_txid) {
                            if parent_idx != i {
                                in_degree[i] += 1;
                                dependents[parent_idx].push(i);
                            }
                        }
                    }
                }
            }
        }

        // Kahn's algorithm: start with txs that have no in-BEEF parents
        let mut queue: VecDeque<usize> = VecDeque::new();
        for i in 0..n {
            if in_degree[i] == 0 {
                queue.push_back(i);
            }
        }

        let mut sorted_order: Vec<usize> = Vec::with_capacity(n);
        while let Some(idx) = queue.pop_front() {
            sorted_order.push(idx);
            for &dep in &dependents[idx] {
                in_degree[dep] -= 1;
                if in_degree[dep] == 0 {
                    queue.push_back(dep);
                }
            }
        }

        // If sort is incomplete (cycle), keep original order
        if sorted_order.len() != n {
            log::warn!("   ⚠️  Topological sort incomplete ({}/{}), keeping original order", sorted_order.len(), n);
            return;
        }

        // Check if already sorted
        let already_sorted = sorted_order.iter().enumerate().all(|(i, &v)| i == v);
        if already_sorted {
            log::info!("   ✅ BEEF transactions already in topological order");
            return;
        }

        // Reorder transactions and tx_to_bump
        let old_txs = self.transactions.clone();
        let old_bumps = self.tx_to_bump.clone();

        for (new_i, &old_i) in sorted_order.iter().enumerate() {
            self.transactions[new_i] = old_txs[old_i].clone();
            if old_i < old_bumps.len() && new_i < self.tx_to_bump.len() {
                self.tx_to_bump[new_i] = old_bumps[old_i];
            }
        }

        log::info!("   ✅ Topologically sorted {} BEEF transactions", n);
    }

    /// Serialize BEEF to V1 format bytes (BRC-62)
    ///
    /// V1 format per transaction: [raw_tx][has_bump: 0x00/0x01][bump_index: varint if 0x01]
    /// This is required for ARC API which only accepts BEEF V1.
    pub fn to_v1_bytes(&self) -> Result<Vec<u8>, String> {
        let mut bytes = Vec::new();

        // Write V1 version marker (0x0100beef)
        bytes.extend_from_slice(&BEEF_V1_MARKER);

        // Write number of BUMPs (merkle proofs)
        write_varint(&mut bytes, self.bumps.len() as u64);

        // Write BUMPs (same format for V1 and V2)
        for bump in &self.bumps {
            write_bump(&mut bytes, bump)?;
        }

        // Write number of transactions
        write_varint(&mut bytes, self.transactions.len() as u64);
        log::info!("   🔢 Writing {} transactions to BEEF V1", self.transactions.len());

        // Write transactions using BEEF V1 format
        // V1 format: [raw_tx][has_bump: 0x00/0x01][bump_index: varint if 0x01]
        for (i, tx) in self.transactions.iter().enumerate() {
            log::info!("      TX {}: {} bytes (V1 format)", i, tx.len());

            // Write raw transaction bytes first (self-describing, no length prefix)
            bytes.extend(tx);

            // Write BUMP association flag after transaction
            if i < self.tx_to_bump.len() {
                if let Some(bump_index) = self.tx_to_bump[i] {
                    bytes.push(0x01); // has BUMP
                    write_varint(&mut bytes, bump_index as u64);
                    log::info!("         BEEF V1: has BUMP at index {}", bump_index);
                } else {
                    bytes.push(0x00); // no BUMP
                    log::info!("         BEEF V1: no BUMP");
                }
            } else {
                bytes.push(0x00); // no BUMP (no mapping available)
                log::info!("         BEEF V1: no BUMP (no mapping)");
            }
        }

        Ok(bytes)
    }

    /// Serialize BEEF to V1 format hex string
    ///
    /// Required for ARC API which only accepts BEEF V1 hex.
    pub fn to_v1_hex(&self) -> Result<String, String> {
        let bytes = self.to_v1_bytes()?;
        Ok(hex::encode(bytes))
    }

    /// Serialize BEEF to hex string (uses current version marker, typically V2)
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

        // 3. Append standard BEEF structure (V1 format for overlay compatibility)
        // Overlays in the BSV ecosystem expect BEEF V1 (BRC-62, magic 0100beef).
        // Using V2 here causes overlays to reject all outputs (outputsToAdmit: []).
        let beef_bytes = self.to_v1_bytes()?;
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

/// Read the varint offset from a pre-formatted BUMP node byte slice.
/// Nodes are stored as [offset varint][flags u8][hash 32 bytes if not duplicate].
/// Returns (offset_value, bytes_consumed).
pub fn read_node_offset(node: &[u8]) -> Result<(u64, usize), String> {
    if node.is_empty() {
        return Err("Empty node bytes".to_string());
    }
    let first = node[0];
    if first < 0xfd {
        Ok((first as u64, 1))
    } else if first == 0xfd {
        if node.len() < 3 {
            return Err("Varint too short for 0xfd prefix".to_string());
        }
        let val = u16::from_le_bytes([node[1], node[2]]) as u64;
        Ok((val, 3))
    } else if first == 0xfe {
        if node.len() < 5 {
            return Err("Varint too short for 0xfe prefix".to_string());
        }
        let val = u32::from_le_bytes([node[1], node[2], node[3], node[4]]) as u64;
        Ok((val, 5))
    } else {
        if node.len() < 9 {
            return Err("Varint too short for 0xff prefix".to_string());
        }
        let val = u64::from_le_bytes([
            node[1], node[2], node[3], node[4],
            node[5], node[6], node[7], node[8],
        ]);
        Ok((val, 9))
    }
}

/// Log the structure of a MerkleProof for debugging
fn log_bump_structure(proof: &MerkleProof) {
    log::info!("      blockHeight={}, treeHeight={}, levels={}", proof.block_height, proof.tree_height, proof.levels.len());
    for (level, nodes) in proof.levels.iter().enumerate() {
        let mut node_info = Vec::new();
        for node in nodes {
            if let Ok((offset, vl)) = read_node_offset(node) {
                let flag = if node.len() > vl { node[vl] } else { 0 };
                let flag_str = match flag {
                    0x00 => "hash",
                    0x01 => "dup",
                    0x02 => "txid",
                    0x03 => "txid+dup",
                    _ => "???",
                };
                let hash_preview = if flag != 0x01 && node.len() > vl + 1 {
                    hex::encode(&node[vl+1..std::cmp::min(node.len(), vl+5)])
                } else {
                    "-".to_string()
                };
                node_info.push(format!("{}({}:{})", offset, flag_str, hash_preview));
            }
        }
        if !node_info.is_empty() || level < 3 {
            log::info!("      L{}: {} nodes [{}]", level, nodes.len(), node_info.join(", "));
        }
    }
}

/// Compute the merkle root from a BUMP for a given txid hash (natural byte order).
/// Returns the root hash in natural byte order.
/// Uses SHA256d (double SHA256) as per Bitcoin merkle trees.
fn compute_root_from_bump(proof: &MerkleProof, txid_natural: &[u8]) -> Result<Vec<u8>, String> {
    use sha2::{Sha256, Digest};

    // Find the txid at level 0
    let mut working_offset: Option<u64> = None;
    for node in &proof.levels[0] {
        let (offset, vl) = read_node_offset(node)?;
        let flag = if node.len() > vl { node[vl] } else { 0 };
        if flag & 0x02 != 0 {
            // Check if hash matches
            let hash_bytes = &node[vl+1..];
            if hash_bytes.len() >= 32 && hash_bytes[..32] == txid_natural[..32] {
                working_offset = Some(offset);
                break;
            }
        }
    }

    let working_offset = working_offset.ok_or("txid not found at level 0 of BUMP")?;
    let mut current_hash = txid_natural.to_vec();
    let mut current_offset = working_offset;

    for level in 0..proof.levels.len() {
        let sibling_offset = current_offset ^ 1;

        // Find sibling at this level
        let sibling_hash = find_or_compute_node(proof, level, sibling_offset)?;

        // Concatenate in correct order and double-SHA256
        let (left, right) = if current_offset % 2 == 0 {
            (current_hash.as_slice(), sibling_hash.as_slice())
        } else {
            (sibling_hash.as_slice(), current_hash.as_slice())
        };

        let mut combined = Vec::with_capacity(64);
        combined.extend_from_slice(left);
        combined.extend_from_slice(right);

        let hash1 = Sha256::digest(&combined);
        let hash2 = Sha256::digest(&hash1);
        current_hash = hash2.to_vec();

        current_offset >>= 1;
    }

    Ok(current_hash)
}

/// Find a node's hash at a given level and offset, or compute it recursively from children.
/// Matches the TS SDK's findOrComputeLeaf method.
fn find_or_compute_node(proof: &MerkleProof, level: usize, offset: u64) -> Result<Vec<u8>, String> {
    use sha2::{Sha256, Digest};

    // Check if node exists at this level
    if level < proof.levels.len() {
        for node in &proof.levels[level] {
            let (node_offset, vl) = read_node_offset(node)?;
            if node_offset == offset {
                let flag = if node.len() > vl { node[vl] } else { 0 };
                if flag & 0x01 != 0 {
                    // Duplicate — find sibling (offset ^ 1) and use its hash
                    let sibling_offset = offset ^ 1;
                    return find_or_compute_node(proof, level, sibling_offset);
                }
                // Regular hash or txid — extract 32-byte hash
                if node.len() >= vl + 1 + 32 {
                    return Ok(node[vl+1..vl+33].to_vec());
                }
                return Err(format!("Node at level {} offset {} has no hash data", level, offset));
            }
        }
    }

    // Not found — compute from children at level below
    if level == 0 {
        return Err(format!("Missing node at level 0 offset {}", offset));
    }

    let child_left_offset = offset << 1;
    let child_right_offset = child_left_offset + 1;

    let left_hash = find_or_compute_node(proof, level - 1, child_left_offset)?;
    let right_hash = find_or_compute_node(proof, level - 1, child_right_offset)?;

    let mut combined = Vec::with_capacity(64);
    combined.extend_from_slice(&left_hash);
    combined.extend_from_slice(&right_hash);

    let hash1 = Sha256::digest(&combined);
    let hash2 = Sha256::digest(&hash1);
    Ok(hash2.to_vec())
}

/// Compute the merkle root from a proof using its first txid node.
/// Returns None if no txid node found or computation fails.
fn compute_root_for_first_txid(proof: &MerkleProof) -> Option<Vec<u8>> {
    if proof.levels.is_empty() {
        return None;
    }
    for node in &proof.levels[0] {
        if let Ok((_, vl)) = read_node_offset(node) {
            let flag = if node.len() > vl { node[vl] } else { 0 };
            if flag & 0x02 != 0 && node.len() >= vl + 33 {
                let txid_hash = &node[vl+1..vl+33];
                return compute_root_from_bump(proof, txid_hash).ok();
            }
        }
    }
    None
}

/// Merge a new MerkleProof into an existing one for the same block.
/// Follows the TS SDK's MerklePath.combine() algorithm:
/// - For each level, add nodes from the new proof that aren't already present (by offset)
/// - If a node with the same offset exists but the new one has the txid flag (0x02),
///   replace it to preserve the txid marker
/// - Keep nodes sorted by offset at each level
fn merge_bump(existing: &mut MerkleProof, new_proof: &MerkleProof) -> Result<(), String> {
    // Use the max tree height (should be identical for same block, but be safe)
    let max_height = std::cmp::max(existing.tree_height, new_proof.tree_height);

    // Extend existing levels if the new proof has more
    while existing.levels.len() < new_proof.levels.len() {
        existing.levels.push(Vec::new());
    }
    existing.tree_height = max_height;

    for level_idx in 0..new_proof.levels.len() {
        for new_node in &new_proof.levels[level_idx] {
            let (new_offset, new_varint_len) = read_node_offset(new_node)?;
            let new_flag = if new_node.len() > new_varint_len { new_node[new_varint_len] } else { 0 };

            // Check if a node with this offset already exists at this level
            let mut found = false;
            for existing_node in existing.levels[level_idx].iter_mut() {
                let (existing_offset, existing_varint_len) = read_node_offset(existing_node)?;
                if existing_offset == new_offset {
                    found = true;
                    // If new node has txid flag (0x02) and existing doesn't, replace it
                    let existing_flag = if existing_node.len() > existing_varint_len {
                        existing_node[existing_varint_len]
                    } else {
                        0
                    };
                    if new_flag == 0x02 && existing_flag != 0x02 {
                        *existing_node = new_node.clone();
                    }
                    break;
                }
            }

            if !found {
                existing.levels[level_idx].push(new_node.clone());
            }
        }

        // Keep nodes sorted by offset at each level
        existing.levels[level_idx].sort_by(|a, b| {
            let (offset_a, _) = read_node_offset(a).unwrap_or((0, 0));
            let (offset_b, _) = read_node_offset(b).unwrap_or((0, 0));
            offset_a.cmp(&offset_b)
        });
    }

    // After merging, trim redundant nodes (matches TS SDK MerklePath.combine → trim)
    trim_bump(existing)?;

    Ok(())
}

/// Remove all internal nodes that are not required by level-zero txid nodes.
/// This matches the TypeScript SDK's MerklePath.trim() method.
///
/// After combining two proofs, intermediate nodes that are now computable from
/// lower-level children must be removed. ARC validates that compound BUMPs don't
/// contain redundant nodes — failing to trim causes HTTP 468 "Invalid BUMPs".
///
/// Algorithm:
/// 1. From level 0, find all txid (0x02) nodes. Their parent offsets (offset >> 1)
///    are "computed" at level 1 — they can be derived, so they must be removed.
/// 2. For each higher level, the computed offsets become the drop list, and the
///    next computed offsets are their parents (offset >> 1).
fn trim_bump(proof: &mut MerkleProof) -> Result<(), String> {
    if proof.levels.is_empty() {
        return Ok(());
    }

    // Sort all levels by offset
    for level in &mut proof.levels {
        level.sort_by(|a, b| {
            let (oa, _) = read_node_offset(a).unwrap_or((0, 0));
            let (ob, _) = read_node_offset(b).unwrap_or((0, 0));
            oa.cmp(&ob)
        });
    }

    // Collect txid offsets at level 0 → compute parent offsets for level 1
    let mut computed_offsets: Vec<u64> = Vec::new();
    let mut drop_offsets_l0: Vec<u64> = Vec::new();

    let level0_len = proof.levels[0].len();
    for l in 0..level0_len {
        let node = &proof.levels[0][l];
        let (offset, varint_len) = read_node_offset(node)?;
        let flag = if node.len() > varint_len { node[varint_len] } else { 0 };

        if flag == 0x02 {
            // txid node — parent offset is computable at next level
            let parent = offset >> 1;
            if computed_offsets.is_empty() || *computed_offsets.last().unwrap() != parent {
                computed_offsets.push(parent);
            }
        } else {
            // Non-txid node — check if peer is also non-txid (both can be dropped)
            let is_odd = offset % 2 == 1;
            let peer_idx = if is_odd { l.wrapping_sub(1) } else { l + 1 };
            if peer_idx < level0_len {
                let peer = &proof.levels[0][peer_idx];
                let (peer_offset, peer_vl) = read_node_offset(peer)?;
                let peer_flag = if peer.len() > peer_vl { peer[peer_vl] } else { 0 };
                if peer_flag != 0x02 {
                    // Peer is also non-txid — drop the peer
                    if drop_offsets_l0.is_empty() || *drop_offsets_l0.last().unwrap() != peer_offset {
                        drop_offsets_l0.push(peer_offset);
                    }
                }
            }
        }
    }

    // Drop orphan non-txid pairs from level 0
    if !drop_offsets_l0.is_empty() {
        proof.levels[0].retain(|node| {
            let (offset, _) = read_node_offset(node).unwrap_or((0, 0));
            !drop_offsets_l0.contains(&offset)
        });
    }

    // For higher levels: drop computed offsets, then compute next level's
    for h in 1..proof.levels.len() {
        let drop_offsets = computed_offsets;
        // Compute next level's computed offsets
        computed_offsets = Vec::new();
        for &o in &drop_offsets {
            let parent = o >> 1;
            if computed_offsets.is_empty() || *computed_offsets.last().unwrap() != parent {
                computed_offsets.push(parent);
            }
        }

        if !drop_offsets.is_empty() {
            proof.levels[h].retain(|node| {
                let (offset, _) = read_node_offset(node).unwrap_or((0, 0));
                !drop_offsets.contains(&offset)
            });
        }
    }

    Ok(())
}

/// Convert WhatsOnChain TSC proof to BUMP format
/// This matches the TypeScript SDK's convertProofToMerklePath function
pub fn tsc_proof_to_bump(
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

/// Parse a BUMP hex string (BRC-74 merklePath from ARC) and convert to TSC JSON format
///
/// ARC returns merklePath as BUMP-format hex (BRC-74). This function converts it
/// to TSC format compatible with our merkle_proofs database table.
///
/// BUMP format: block_height(varint) + tree_height(u8) + levels[num_nodes(varint) + nodes[offset(varint) + flags(u8) + hash(32 bytes if not duplicate)]]
///
/// TSC format: { "height": u32, "index": u64, "nodes": ["hex_hash", ...], "target": "" }
///
/// # Arguments
/// * `bump_hex` - BUMP hex string from ARC merklePath response
///
/// # Returns
/// TSC-compatible JSON value, or error if parsing fails
pub fn parse_bump_hex_to_tsc(bump_hex: &str) -> Result<serde_json::Value, String> {
    let bytes = hex::decode(bump_hex)
        .map_err(|e| format!("Invalid BUMP hex: {}", e))?;

    if bytes.is_empty() {
        return Err("Empty BUMP data".to_string());
    }

    let mut cursor = std::io::Cursor::new(bytes.as_slice());

    // Read block height
    let block_height = read_varint(&mut cursor)? as u32;

    // Read tree height
    let tree_height = read_u8(&mut cursor)?;

    if tree_height == 0 {
        return Err("Invalid BUMP: tree height is 0".to_string());
    }

    // Parse levels to extract tx_index and sibling hashes
    let mut tx_index: u64 = 0;
    let mut tsc_nodes: Vec<String> = Vec::with_capacity(tree_height as usize);

    for level in 0..tree_height as usize {
        let num_nodes = read_varint(&mut cursor)? as usize;

        let mut txid_offset: Option<u64> = None;
        let mut sibling_hash: Option<String> = None;
        let mut sibling_is_duplicate = false;

        for _ in 0..num_nodes {
            let offset = read_varint(&mut cursor)?;
            let flags = read_u8(&mut cursor)?;

            if flags & 0x01 != 0 {
                // Duplicate flag - no hash follows
                if level == 0 {
                    if flags & 0x02 != 0 {
                        // This is the TXID and it's a duplicate (shouldn't happen at level 0)
                        txid_offset = Some(offset);
                    } else {
                        sibling_is_duplicate = true;
                    }
                } else {
                    sibling_is_duplicate = true;
                }
            } else {
                // Hash follows (32 bytes)
                let mut hash = vec![0u8; 32];
                use std::io::Read;
                cursor.read_exact(&mut hash)
                    .map_err(|e| format!("Failed to read BUMP hash at level {}: {}", level, e))?;

                if flags & 0x02 != 0 {
                    // TXID flag - this node is the transaction
                    txid_offset = Some(offset);
                } else {
                    // Sibling hash - reverse to display format for TSC
                    hash.reverse();
                    sibling_hash = Some(hex::encode(&hash));
                }
            }
        }

        // At level 0, extract tx_index from the TXID node offset
        if level == 0 {
            if let Some(offset) = txid_offset {
                tx_index = offset;
            }
        }

        // Add sibling hash to TSC nodes array
        if sibling_is_duplicate {
            tsc_nodes.push("*".to_string());
        } else if let Some(hash) = sibling_hash {
            tsc_nodes.push(hash);
        } else {
            // No sibling found at this level - use "*" as fallback
            tsc_nodes.push("*".to_string());
        }
    }

    log::info!("   📋 Parsed BUMP: block_height={}, tx_index={}, tree_height={}, {} sibling nodes",
        block_height, tx_index, tree_height, tsc_nodes.len());

    Ok(serde_json::json!({
        "height": block_height,
        "index": tx_index,
        "nodes": tsc_nodes,
        "target": "",  // Block hash not available from BUMP - will be looked up if needed
    }))
}

/// Compute the merkle root from a TSC proof and transaction ID.
///
/// This does a full merkle root computation: converts TSC to BUMP, then walks
/// the tree from the txid leaf up to the root using double-SHA256 at each level.
///
/// Returns the merkle root in display (reversed) byte order as a hex string.
/// Use this to verify proofs against block headers before storing or using them.
pub fn compute_merkle_root_from_tsc(
    txid: &str,
    block_height: u32,
    tx_index: u64,
    nodes: &[serde_json::Value],
) -> Result<String, String> {
    let bump = tsc_proof_to_bump(txid, block_height, tx_index, nodes)?;

    let txid_bytes = hex::decode(txid)
        .map_err(|e| format!("Invalid txid hex: {}", e))?;
    let mut txid_natural = txid_bytes;
    txid_natural.reverse(); // display → natural

    let root_natural = compute_root_from_bump(&bump, &txid_natural)?;

    // Convert to display order
    let mut root_display = root_natural;
    root_display.reverse();
    Ok(hex::encode(root_display))
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

/// Validate BEEF V1 bytes by parsing them exactly as a receiver (ARC) would.
/// Logs detailed diagnostics about every transaction, input, and output script.
/// Returns Ok(()) if valid, or Err with the first error found.
pub fn validate_beef_v1_hex(v1_hex: &str) -> Result<(), String> {
    use sha2::{Sha256, Digest};

    // Validate IV is initialized (compile-time structural check)
    debug_assert_eq!(BEEF_VALIDATION_IV.len(), 28);

    let bytes = hex::decode(v1_hex)
        .map_err(|e| format!("Invalid hex: {}", e))?;
    let total_len = bytes.len();
    let mut cursor = Cursor::new(bytes.as_slice());

    log::info!("🔍 BEEF V1 VALIDATION — {} total bytes", total_len);

    // Read 4-byte version marker
    let mut version = [0u8; 4];
    cursor.read_exact(&mut version)
        .map_err(|e| format!("Version: {}", e))?;
    if version != BEEF_V1_MARKER {
        return Err(format!("Not BEEF V1: {:02x?}", version));
    }
    log::info!("   ✅ Version: 0100beef");

    // Read number of BUMPs
    let num_bumps = read_varint(&mut cursor)?;
    log::info!("   📊 BUMPs: {}", num_bumps);

    // Skip BUMPs (parse them to advance cursor correctly)
    for b in 0..num_bumps {
        let bump_start = cursor.position();
        let block_height = read_varint(&mut cursor)?;
        let tree_height = read_u8(&mut cursor)?;
        for level in 0..tree_height {
            let num_nodes = read_varint(&mut cursor)?;
            for _ in 0..num_nodes {
                let _offset = read_varint(&mut cursor)?;
                let flags = read_u8(&mut cursor)?;
                if flags & 0x01 == 0 {
                    // Not a duplicate — 32-byte hash follows
                    let mut hash = [0u8; 32];
                    cursor.read_exact(&mut hash)
                        .map_err(|e| format!("BUMP {} level {} hash: {}", b, level, e))?;
                }
            }
        }
        let bump_bytes = cursor.position() - bump_start;
        log::info!("   BUMP {}: blockHeight={}, treeHeight={}, {} bytes", b, block_height, tree_height, bump_bytes);
    }

    let tx_section_start = cursor.position();
    log::info!("   📍 Transaction section starts at byte offset {}", tx_section_start);

    // Read number of transactions
    let num_txs = read_varint(&mut cursor)?;
    log::info!("   📊 Transactions: {}", num_txs);

    // Parse each transaction
    for t in 0..num_txs {
        let tx_start = cursor.position();

        // Parse the transaction structure
        // Version (4 bytes)
        let mut ver = [0u8; 4];
        cursor.read_exact(&mut ver)
            .map_err(|e| format!("TX {} version: {}", t, e))?;
        let version = u32::from_le_bytes(ver);

        // Inputs
        let input_count = read_varint(&mut cursor)
            .map_err(|e| format!("TX {} input_count: {}", t, e))?;
        let mut input_details = Vec::new();
        for i in 0..input_count {
            let mut prev_hash = [0u8; 32];
            cursor.read_exact(&mut prev_hash)
                .map_err(|e| format!("TX {} input {} prevhash: {}", t, i, e))?;
            let prev_txid = hex::encode(prev_hash.iter().rev().copied().collect::<Vec<u8>>());

            let mut prev_vout_buf = [0u8; 4];
            cursor.read_exact(&mut prev_vout_buf)
                .map_err(|e| format!("TX {} input {} prevvout: {}", t, i, e))?;
            let prev_vout = u32::from_le_bytes(prev_vout_buf);

            let script_len = read_varint(&mut cursor)
                .map_err(|e| format!("TX {} input {} scriptSig len: {}", t, i, e))?;
            let remaining = total_len as u64 - cursor.position();
            if script_len > remaining {
                let err_msg = format!(
                    "TX {} input {} scriptSig: varint says {} bytes but only {} remain (pos={}, total={}). Prev: {}:{}",
                    t, i, script_len, remaining, cursor.position(), total_len, &prev_txid[..16], prev_vout
                );
                log::error!("   ❌ {}", err_msg);
                return Err(err_msg);
            }
            let mut script = vec![0u8; script_len as usize];
            cursor.read_exact(&mut script)
                .map_err(|e| format!("TX {} input {} scriptSig data: {}", t, i, e))?;

            let mut seq_buf = [0u8; 4];
            cursor.read_exact(&mut seq_buf)
                .map_err(|e| format!("TX {} input {} sequence: {}", t, i, e))?;

            input_details.push(format!(
                "{}:{}  scriptSig={} bytes",
                &prev_txid[..16], prev_vout, script_len
            ));
        }

        // Outputs
        let output_count = read_varint(&mut cursor)
            .map_err(|e| format!("TX {} output_count: {}", t, e))?;
        let mut output_details = Vec::new();
        for o in 0..output_count {
            let mut val_buf = [0u8; 8];
            cursor.read_exact(&mut val_buf)
                .map_err(|e| format!("TX {} output {} value: {}", t, o, e))?;
            let value = u64::from_le_bytes(val_buf);

            let script_len = read_varint(&mut cursor)
                .map_err(|e| format!("TX {} output {} scriptPubKey len: {}", t, o, e))?;
            let remaining = total_len as u64 - cursor.position();
            if script_len > remaining {
                let err_msg = format!(
                    "TX {} output {} scriptPubKey: varint says {} bytes but only {} remain (pos={}, total={}). Value: {} sats",
                    t, o, script_len, remaining, cursor.position(), total_len, value
                );
                log::error!("   ❌ {}", err_msg);
                return Err(err_msg);
            }
            let mut script = vec![0u8; script_len as usize];
            cursor.read_exact(&mut script)
                .map_err(|e| format!("TX {} output {} scriptPubKey data: {}", t, o, e))?;

            output_details.push(format!(
                "{} sats  scriptPubKey={} bytes  first3={:02x?}",
                value, script_len, &script[..script.len().min(3)]
            ));
        }

        // Locktime (4 bytes)
        let mut lt_buf = [0u8; 4];
        cursor.read_exact(&mut lt_buf)
            .map_err(|e| format!("TX {} locktime: {}", t, e))?;
        let locktime = u32::from_le_bytes(lt_buf);

        let tx_end = cursor.position();
        let tx_len = tx_end - tx_start;

        // Compute TXID for identification
        let tx_bytes = &bytes[tx_start as usize..tx_end as usize];
        let hash1 = Sha256::digest(tx_bytes);
        let hash2 = Sha256::digest(&hash1);
        let txid: Vec<u8> = hash2.iter().rev().copied().collect();
        let txid_hex = hex::encode(&txid);

        log::info!("   TX {} (offset {}..{}, {} bytes): txid={}", t, tx_start, tx_end, tx_len, &txid_hex[..16]);
        log::info!("      version={}, locktime={}, {} inputs, {} outputs", version, locktime, input_count, output_count);
        for detail in &input_details {
            log::info!("      IN:  {}", detail);
        }
        for detail in &output_details {
            log::info!("      OUT: {}", detail);
        }

        // Read BUMP flag (V1: after transaction)
        let flag = read_u8(&mut cursor)
            .map_err(|e| format!("TX {} bump flag: {}", t, e))?;
        if flag == 0x01 {
            let bump_idx = read_varint(&mut cursor)?;
            log::info!("      BUMP: index {}", bump_idx);
        } else if flag == 0x00 {
            log::info!("      BUMP: none");
        } else {
            let err_msg = format!("TX {} unexpected bump flag: 0x{:02x} (expected 0x00 or 0x01)", t, flag);
            log::error!("   ❌ {}", err_msg);
            return Err(err_msg);
        }
    }

    let final_pos = cursor.position();
    let remaining = total_len as u64 - final_pos;
    if remaining > 0 {
        log::warn!("   ⚠️ {} trailing bytes after last transaction", remaining);
    } else {
        log::info!("   ✅ BEEF V1 validation passed — all {} transactions parsed cleanly", num_txs);
    }

    Ok(())
}

/// Validate that a BEEF structure has complete ancestry chains.
///
/// Every transaction in the BEEF must either:
/// 1. Have a BUMP (merkle proof) — proving it's confirmed on-chain, OR
/// 2. Have ALL of its input parent transactions also present in the BEEF,
///    and those parents must recursively satisfy the same condition.
///
/// This mirrors the BSV SDK's `Beef.verifyValid()` (Beef.ts:498-567).
/// The main transaction (last in the array) is allowed to have no BUMP
/// since it's the new transaction being submitted.
///
/// Returns Ok with a summary, or Err describing which txids are missing.
pub fn validate_beef_ancestry(beef: &Beef) -> Result<BeefValidationReport, String> {
    use sha2::{Sha256, Digest};

    let num_txs = beef.transactions.len();
    if num_txs == 0 {
        return Err("BEEF contains no transactions".to_string());
    }

    // Build a map of txid → index for all transactions in the BEEF
    let mut txid_to_index: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut txids: Vec<String> = Vec::with_capacity(num_txs);

    for (i, tx_bytes) in beef.transactions.iter().enumerate() {
        let h1 = Sha256::digest(tx_bytes);
        let h2 = Sha256::digest(&h1);
        let txid_hex: String = h2.iter().rev().map(|b| format!("{:02x}", b)).collect();
        txid_to_index.insert(txid_hex.clone(), i);
        txids.push(txid_hex);
    }

    let mut report = BeefValidationReport {
        total_txs: num_txs,
        confirmed_txs: 0,
        unconfirmed_txs: 0,
        main_tx: txids.last().cloned().unwrap_or_default(),
        missing_parents: Vec::new(),
        orphaned_txs: Vec::new(),
    };

    // Check each transaction (except the main/last tx which is the new one)
    for (i, tx_bytes) in beef.transactions.iter().enumerate() {
        let txid = &txids[i];
        let has_bump = i < beef.tx_to_bump.len() && beef.tx_to_bump[i].is_some();
        let is_main_tx = i == num_txs - 1;

        if has_bump {
            report.confirmed_txs += 1;
            continue; // Confirmed with BUMP — no ancestry needed
        }

        report.unconfirmed_txs += 1;

        if is_main_tx {
            continue; // Main tx is expected to not have a BUMP
        }

        // Unconfirmed parent tx — ALL its inputs must be in the BEEF
        match ParsedTransaction::from_bytes(tx_bytes) {
            Ok(parsed) => {
                for input in &parsed.inputs {
                    if txid_to_index.get(&input.prev_txid).is_none() {
                        report.missing_parents.push(MissingParent {
                            child_txid: txid.clone(),
                            missing_parent_txid: input.prev_txid.clone(),
                            input_vout: input.prev_vout,
                        });
                    }
                }
            }
            Err(e) => {
                report.orphaned_txs.push(format!("{}  (parse error: {})", txid, e));
            }
        }
    }

    // Also check main tx inputs — they should all be in the BEEF too
    if let Some(main_tx_bytes) = beef.transactions.last() {
        if let Ok(parsed) = ParsedTransaction::from_bytes(main_tx_bytes) {
            for input in &parsed.inputs {
                if txid_to_index.get(&input.prev_txid).is_none() {
                    report.missing_parents.push(MissingParent {
                        child_txid: report.main_tx.clone(),
                        missing_parent_txid: input.prev_txid.clone(),
                        input_vout: input.prev_vout,
                    });
                }
            }
        }
    }

    if report.missing_parents.is_empty() && report.orphaned_txs.is_empty() {
        log::info!("   ✅ BEEF ancestry validation passed: {} txs ({} confirmed, {} unconfirmed, {} BUMPs)",
            report.total_txs, report.confirmed_txs, report.unconfirmed_txs, beef.bumps.len());
        Ok(report)
    } else {
        let mut err_parts = Vec::new();
        for mp in &report.missing_parents {
            err_parts.push(format!(
                "tx {} input {}:{} — parent not in BEEF",
                &mp.child_txid[..16.min(mp.child_txid.len())],
                &mp.missing_parent_txid[..16.min(mp.missing_parent_txid.len())],
                mp.input_vout
            ));
        }
        for orphan in &report.orphaned_txs {
            err_parts.push(format!("orphaned: {}", orphan));
        }
        let err_msg = format!("BEEF ancestry incomplete — {}", err_parts.join("; "));
        log::warn!("   ⚠️  {}", err_msg);
        Err(err_msg)
    }
}

/// Report from BEEF ancestry validation
#[derive(Debug, Clone)]
pub struct BeefValidationReport {
    pub total_txs: usize,
    pub confirmed_txs: usize,
    pub unconfirmed_txs: usize,
    pub main_tx: String,
    pub missing_parents: Vec<MissingParent>,
    pub orphaned_txs: Vec<String>,
}

/// A parent transaction that's referenced by an input but missing from the BEEF
#[derive(Debug, Clone)]
pub struct MissingParent {
    pub child_txid: String,
    pub missing_parent_txid: String,
    pub input_vout: u32,
}
