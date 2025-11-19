use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use bip39::{Mnemonic, Language};
use bip32::XPrv;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddressInfo {
    pub index: i32,
    pub address: String,
    #[serde(rename = "publicKey")]
    pub public_key: String,
    #[serde(default)]
    pub used: bool,
    #[serde(default)]
    pub balance: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Wallet {
    pub mnemonic: String,
    pub addresses: Vec<AddressInfo>,
    #[serde(rename = "currentIndex")]
    pub current_index: i32,
    #[serde(rename = "backedUp")]
    pub backed_up: bool,
}

pub struct JsonStorage {
    wallet_path: PathBuf,
    wallet: Option<Wallet>,
}

impl JsonStorage {
    pub fn new(wallet_path: PathBuf) -> Result<Self, String> {
        let mut storage = JsonStorage {
            wallet_path,
            wallet: None,
        };
        storage.load()?;
        Ok(storage)
    }

    pub fn load(&mut self) -> Result<(), String> {
        let data = fs::read_to_string(&self.wallet_path)
            .map_err(|e| format!("Failed to read wallet: {}", e))?;

        let wallet: Wallet = serde_json::from_str(&data)
            .map_err(|e| format!("Failed to parse wallet: {}", e))?;

        self.wallet = Some(wallet);
        Ok(())
    }

    pub fn get_wallet(&self) -> Result<&Wallet, String> {
        self.wallet.as_ref().ok_or("No wallet loaded".to_string())
    }

    pub fn get_current_address(&self) -> Result<&AddressInfo, String> {
        let wallet = self.get_wallet()?;
        wallet.addresses.first()
            .ok_or("No addresses in wallet".to_string())
    }

    pub fn get_all_addresses(&self) -> Result<&[AddressInfo], String> {
        let wallet = self.get_wallet()?;
        Ok(&wallet.addresses)
    }

    /// Add a new address to the wallet and save
    pub fn add_address(&mut self, address: AddressInfo) -> Result<(), String> {
        let wallet = self.get_wallet_mut()?;
        wallet.addresses.push(address);
        wallet.current_index += 1;
        self.save()?;
        Ok(())
    }

    /// Get mutable reference to wallet (for modifications)
    fn get_wallet_mut(&mut self) -> Result<&mut Wallet, String> {
        self.wallet.as_mut().ok_or("No wallet loaded".to_string())
    }

    /// Save wallet to file
    pub fn save(&self) -> Result<(), String> {
        let wallet = self.wallet.as_ref().ok_or("No wallet loaded".to_string())?;

        let data = serde_json::to_string_pretty(wallet)
            .map_err(|e| format!("Failed to serialize wallet: {}", e))?;

        fs::write(&self.wallet_path, data)
            .map_err(|e| format!("Failed to write wallet file: {}", e))?;

        Ok(())
    }

    /// Get the master private key (m) from mnemonic
    /// This is the root key before any derivation
    /// Used for BRC-42/BRC-84 key derivation
    pub fn get_master_private_key(&self) -> Result<Vec<u8>, String> {
        let wallet = self.get_wallet()?;

        // Parse mnemonic
        let mnemonic = Mnemonic::parse_in(Language::English, &wallet.mnemonic)
            .map_err(|e| format!("Invalid mnemonic: {}", e))?;

        // Generate seed from mnemonic (no password)
        let seed = mnemonic.to_seed("");

        // Create BIP32 master key from seed
        let master_key = XPrv::new(&seed)
            .map_err(|e| format!("Failed to create master key: {}", e))?;

        // Extract 32-byte master private key
        Ok(master_key.private_key().to_bytes().to_vec())
    }

    /// Get the master public key from the master private key
    /// Returns the compressed 33-byte public key (with prefix byte)
    pub fn get_master_public_key(&self) -> Result<Vec<u8>, String> {
        use secp256k1::{Secp256k1, SecretKey, PublicKey};

        let private_key_bytes = self.get_master_private_key()?;

        let secp = Secp256k1::new();
        let secret_key = SecretKey::from_slice(&private_key_bytes)
            .map_err(|e| format!("Invalid private key: {}", e))?;

        let public_key = PublicKey::from_secret_key(&secp, &secret_key);

        // Return compressed format (33 bytes with prefix)
        Ok(public_key.serialize().to_vec())
    }

    /// Derive private key from mnemonic for a specific address index
    /// Uses BIP39 to convert mnemonic → seed
    /// Uses BIP32 for hierarchical key derivation (matches Go wallet implementation)
    pub fn derive_private_key(&self, index: u32) -> Result<Vec<u8>, String> {
        let wallet = self.get_wallet()?;

        // Parse mnemonic
        let mnemonic = Mnemonic::parse_in(Language::English, &wallet.mnemonic)
            .map_err(|e| format!("Invalid mnemonic: {}", e))?;

        // Generate seed from mnemonic (no password)
        let seed = mnemonic.to_seed("");

        // Create BIP32 master key from seed
        // This matches Go's: bip32.NewMasterKey(seed)
        let master_key = XPrv::new(&seed)
            .map_err(|e| format!("Failed to create master key: {}", e))?;

        // Derive child key at index
        // This matches Go's: masterKey.NewChildKey(uint32(index))
        // Path: m/{index} (simplified for now, not full BIP44)
        let child_key = master_key
            .derive_child(bip32::ChildNumber::new(index, false).unwrap())
            .map_err(|e| format!("Failed to derive child key: {}", e))?;

        // Extract 32-byte private key
        // This matches Go's: derivedKey.Key
        let private_key_bytes = child_key.private_key().to_bytes();

        // DEBUG: Verify public key matches wallet.json
        use secp256k1::{Secp256k1, SecretKey, PublicKey};
        let secp = Secp256k1::new();
        let secret_key = SecretKey::from_slice(&private_key_bytes)
            .map_err(|e| format!("Invalid derived private key: {}", e))?;
        let public_key = PublicKey::from_secret_key(&secp, &secret_key);
        let derived_pubkey_hex = hex::encode(public_key.serialize());

        log::info!("   🔍 DEBUG: Derived pubkey from mnemonic (index {}): {}", index, derived_pubkey_hex);

        Ok(private_key_bytes.to_vec())
    }
}
