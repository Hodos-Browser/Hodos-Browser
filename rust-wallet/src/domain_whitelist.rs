use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DomainWhitelistEntry {
    pub domain: String,
    pub added_at: i64,      // Unix timestamp (seconds since epoch)
    pub last_used: i64,     // Unix timestamp (seconds since epoch)
    pub request_count: u32,
    pub is_permanent: bool,
}

pub struct DomainWhitelistManager {
    whitelist: Arc<Mutex<HashMap<String, DomainWhitelistEntry>>>,
    file_path: PathBuf,
}

impl DomainWhitelistManager {
    pub fn new() -> Self {
        let file_path = Self::get_whitelist_path();

        let manager = Self {
            whitelist: Arc::new(Mutex::new(HashMap::new())),
            file_path,
        };

        // Load existing whitelist
        if let Err(e) = manager.load_whitelist() {
            log::warn!("Failed to load whitelist: {}", e);
        }

        manager
    }

    fn get_whitelist_path() -> PathBuf {
        let home_dir = dirs::home_dir().expect("Could not determine home directory");
        home_dir
            .join("AppData")
            .join("Roaming")
            .join("HodosBrowser")
            .join("wallet")
            .join("domainWhitelist.json")
    }

    fn load_whitelist(&self) -> Result<(), Box<dyn std::error::Error>> {
        // Create directory if it doesn't exist
        if let Some(parent) = self.file_path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Check if file exists
        if !self.file_path.exists() {
            log::info!("📂 Whitelist file does not exist yet: {:?}", self.file_path);
            return Ok(());
        }

        // Read file
        let data = fs::read_to_string(&self.file_path)?;
        log::info!("📂 Loading whitelist from: {:?}", self.file_path);

        // Parse JSON
        let entries: Vec<DomainWhitelistEntry> = serde_json::from_str(&data)?;
        log::info!("📂 Loaded {} existing domains from whitelist", entries.len());

        // Convert to map
        let mut whitelist = self.whitelist.lock().unwrap();
        for entry in entries {
            log::info!("   - {}", entry.domain);
            whitelist.insert(entry.domain.clone(), entry);
        }

        Ok(())
    }

    fn save_whitelist(&self) -> Result<(), Box<dyn std::error::Error>> {
        let whitelist = self.whitelist.lock().unwrap();

        // Convert map to vec
        let entries: Vec<DomainWhitelistEntry> = whitelist.values().cloned().collect();
        log::info!("💾 Saving {} domains to whitelist file: {:?}", entries.len(), self.file_path);
        for entry in &entries {
            log::info!("   - {}", entry.domain);
        }

        // Marshal to JSON
        let data = serde_json::to_string_pretty(&entries)?;

        // Write to file
        fs::write(&self.file_path, data)?;
        log::info!("✅ Whitelist saved successfully");

        Ok(())
    }

    pub fn is_domain_whitelisted(&self, domain: &str) -> bool {
        let whitelist = self.whitelist.lock().unwrap();

        if let Some(entry) = whitelist.get(domain) {
            // Check if it's a one-time entry that has been used
            if !entry.is_permanent && entry.request_count > 0 {
                return false;
            }
            return true;
        }

        false
    }

    pub fn add_to_whitelist(&self, domain: String, is_permanent: bool) -> Result<(), Box<dyn std::error::Error>> {
        let mut whitelist = self.whitelist.lock().unwrap();

        log::info!("📝 Adding domain to whitelist: {} (permanent: {})", domain, is_permanent);
        log::info!("📝 Current whitelist size before add: {}", whitelist.len());

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let entry = DomainWhitelistEntry {
            domain: domain.clone(),
            added_at: now,
            last_used: now,
            request_count: 0,
            is_permanent,
        };

        whitelist.insert(domain.clone(), entry);
        log::info!("📝 Current whitelist size after add: {}", whitelist.len());
        drop(whitelist); // Release lock before saving

        self.save_whitelist()?;

        log::info!("✅ Added domain to whitelist: {} (permanent: {})", domain, is_permanent);

        Ok(())
    }

    pub fn record_request(&self, domain: &str) -> Result<(), Box<dyn std::error::Error>> {
        let mut whitelist = self.whitelist.lock().unwrap();

        if let Some(entry) = whitelist.get_mut(domain) {
            entry.last_used = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64;
            entry.request_count += 1;
            drop(whitelist); // Release lock before saving

            self.save_whitelist()?;
            Ok(())
        } else {
            Err(format!("domain not in whitelist: {}", domain).into())
        }
    }

    pub fn remove_from_whitelist(&self, domain: &str) -> Result<(), Box<dyn std::error::Error>> {
        let mut whitelist = self.whitelist.lock().unwrap();
        whitelist.remove(domain);
        drop(whitelist); // Release lock before saving

        self.save_whitelist()?;

        log::info!("✅ Removed domain from whitelist: {}", domain);

        Ok(())
    }

    pub fn get_whitelist(&self) -> HashMap<String, DomainWhitelistEntry> {
        let whitelist = self.whitelist.lock().unwrap();
        whitelist.clone()
    }
}
