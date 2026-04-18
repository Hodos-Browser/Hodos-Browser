//! Utility to extract the master private key from the wallet database
//!
//! Usage: cargo run --bin extract_master_key

use rusqlite::Connection;
use bip39::{Mnemonic, Language};
use bip32::XPrv;

fn app_dir_name() -> &'static str {
    match std::env::var("HODOS_DEV").as_deref() {
        Ok("1") => "HodosBrowserDev",
        _ => "HodosBrowser",
    }
}

fn main() {
    // Initialize logging
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();

    println!("🔐 Extracting master private key from wallet database...\n");

    // Get database path - cross-platform (same logic as main.rs)
    let wallet_dir = dirs::data_dir()
        .unwrap_or_else(|| {
            match std::env::var("APPDATA") {
                Ok(appdata) => std::path::PathBuf::from(appdata),
                Err(_) => {
                    eprintln!("⚠️  Could not determine data directory, using current directory");
                    std::path::PathBuf::from(".")
                }
            }
        })
        .join(app_dir_name())
        .join("wallet");

    let db_path = wallet_dir.join("wallet.db");
    let db_path_str = db_path.to_string_lossy().to_string();

    println!("📁 Database path: {}", db_path_str);

    // Open database connection
    let conn = match Connection::open(&db_path_str) {
        Ok(conn) => {
            println!("✅ Database opened successfully\n");
            conn
        },
        Err(e) => {
            eprintln!("❌ Failed to open database: {}", e);
            std::process::exit(1);
        }
    };

    // Get mnemonic from database
    let mnemonic_phrase: String = match conn.query_row(
        "SELECT mnemonic FROM wallets ORDER BY id LIMIT 1",
        [],
        |row| row.get(0)
    ) {
        Ok(mnemonic) => mnemonic,
        Err(e) => {
            eprintln!("❌ Failed to get mnemonic from database: {}", e);
            std::process::exit(1);
        }
    };

    println!("📝 Mnemonic found in database (first 20 chars): {}...\n", &mnemonic_phrase[..std::cmp::min(20, mnemonic_phrase.len())]);

    // Parse mnemonic
    let mnemonic = match Mnemonic::parse_in(Language::English, &mnemonic_phrase) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("❌ Failed to parse mnemonic: {}", e);
            std::process::exit(1);
        }
    };

    // Generate seed from mnemonic (no password)
    let seed = mnemonic.to_seed("");

    // Create BIP32 master key from seed
    let master_key = match XPrv::new(&seed) {
        Ok(key) => key,
        Err(e) => {
            eprintln!("❌ Failed to create master key: {}", e);
            std::process::exit(1);
        }
    };

    // Extract 32-byte master private key
    let key_bytes = master_key.private_key().to_bytes().to_vec();
    let key_hex = hex::encode(&key_bytes);

    println!("✅ Master private key extracted successfully!\n");
    println!("📋 Master Private Key (32 bytes):");
    println!("   Hex: {}", key_hex);
    println!("   Length: {} bytes\n", key_bytes.len());

    println!("📝 Copy this hex value to test_csr_comparison.ts:");
    println!("   const subjectPrivateKeyHex = '{}';", key_hex);
    println!();
}
