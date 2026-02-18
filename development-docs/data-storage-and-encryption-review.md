# Hodos Browser — Data Storage & Encryption Review

**Created**: 2026-02-17
**Updated**: 2026-02-17 (DPAPI implemented, whitelist.json deprecated)
**Purpose**: Quick reference of all browser data storage, encryption status, and cross-platform considerations.

---

## 1. Browser Data (C++ CEF Layer)

All browser data lives under `%APPDATA%\HodosBrowser\`:

| Storage | Format | Encryption | Notes |
|---------|--------|------------|-------|
| `Default\` | Chromium profile | None (CEF-managed) | History, cookies, site data, cache |
| `History.db` | SQLite | None | Custom history (HistoryManager singleton) |
| `Bookmarks.db` | SQLite | None | Folder hierarchy, tags (BookmarkManager singleton) |
| `debug.log` | Text | None | Debug output — should be excluded from release builds |
| `identity.json` | JSON | None | User identity data (IdentityHandler) |

**No wallet data in this layer.** Browser data and wallet data are fully isolated.

---

## 2. Wallet Data (Rust Backend)

All wallet data lives under `%APPDATA%\HodosBrowser\wallet\`:

| Storage | Format | Encryption | Status |
|---------|--------|------------|--------|
| `wallet.db` | SQLite (WAL mode) | Mnemonic: AES-256-GCM (PIN) + DPAPI (Windows) | Active — sole source of truth |
| `domainWhitelist.json` | JSON | None | **DEPRECATED** — written by legacy dual-write but never read. Safe to remove. |

### Key Tables (Schema V4)

| Table | Sensitive? | Notes |
|-------|-----------|-------|
| `wallets` | **YES** — contains mnemonic | `mnemonic` = PIN-encrypted, `mnemonic_dpapi` = DPAPI-encrypted (dual encryption) |
| `addresses` | Low — public keys only | HD derivation cache, no private keys |
| `outputs` | Low — UTXOs with derivation info | `derivation_prefix`/`suffix` used for key re-derivation |
| `transactions` | Low — tx records | `raw_tx` BLOB, `status` column |
| `certificates` | Medium — identity certs | BRC-52 selective disclosure |
| `domain_permissions` | Low — trust settings | Per-domain spending limits, rate limits (replaces domainWhitelist.json) |

### Mnemonic Encryption (Dual)

**Column 1: PIN-encrypted** (`wallets.mnemonic`)
- **Algorithm**: AES-256-GCM
- **KDF**: PBKDF2-HMAC-SHA256 (600,000 iterations, 16-byte random salt)
- **Storage format**: hex(nonce_12 || ciphertext || tag_16)
- **PIN salt**: `wallets.pin_salt` = hex(salt_16), NULL for unencrypted legacy wallets
- **Use case**: Portable encryption — works on any machine if user knows PIN

**Column 2: DPAPI-encrypted** (`wallets.mnemonic_dpapi`)
- **Algorithm**: Windows DPAPI (`CryptProtectData` / `CryptUnprotectData`)
- **Storage format**: Raw DPAPI blob (BLOB column)
- **Key management**: OS-managed, tied to current Windows user account login credentials
- **Use case**: Auto-unlock on startup — no user interaction needed

**Private keys**: NEVER stored — derived on-demand from cached mnemonic via BRC-42 or BIP32

### Backup File (`.hodos-wallet`)

- **Encryption**: AES-256-GCM with password-derived key (same KDF, different password)
- **Contains**: All 20+ entities including mnemonic (encrypted by backup password, not PIN)
- **Excluded**: `pin_salt`, `monitor_events`, `derived_key_cache`

---

## 3. Frontend Data (React/TypeScript)

All frontend data is in `localStorage` (origin: `localhost:5137`), shared across all CEF subprocesses:

| Key | Purpose | TTL |
|-----|---------|-----|
| `hodos:wallet:balance` | Balance cache (satoshis + timestamp) | 60s |
| `hodos:wallet:bsvPrice` | BSV/USD price cache | 10min |
| `hodos_wallet_exists` | Cache-first wallet status check | Session |

**No secrets in frontend.** All cryptographic operations happen in Rust.

---

## 4. DPAPI Auto-Unlock (Windows) — Implemented

### How It Works

DPAPI (`CryptProtectData`/`CryptUnprotectData`) encrypts data tied to the current Windows user account. The OS manages the key using the user's login credentials. Decryption succeeds if and only if the same Windows user is logged in. This is the same mechanism Chrome, Firefox, and Edge use for saved passwords.

### Startup Flow

1. **Wallet found** → try `CryptUnprotectData` on `mnemonic_dpapi` column
2. **DPAPI succeeds** → mnemonic cached in memory → wallet ready instantly (no PIN needed)
3. **DPAPI fails** (DB moved to different machine/user) → wallet locked → frontend shows fallback PIN prompt
4. **Legacy wallet** (no PIN, no DPAPI) → plaintext mnemonic cached directly
5. **PIN-protected without DPAPI** (pre-V4 wallet) → locked until PIN entry → DPAPI blob backfilled on unlock

### PIN Is Still Used For

- Initial wallet creation (encrypts mnemonic in DB)
- Wallet recovery and import (encrypts mnemonic in DB)
- Fallback unlock when DPAPI fails (edge case)
- Future: viewing mnemonic in advanced settings, high-risk operations

### Security Properties

| Scenario | Protection |
|----------|-----------|
| Same machine, same user | Auto-unlock via DPAPI (designed behavior) |
| DB file stolen to another machine | DPAPI blob useless; attacker needs 4-digit PIN + 600K PBKDF2 iterations |
| Different Windows user on same machine | DPAPI fails; PIN required |
| Machine physically compromised | Same as any logged-in session — OS-level security applies |

### Implementation Files

| File | Purpose |
|------|---------|
| `rust-wallet/src/crypto/dpapi.rs` | `dpapi_encrypt`/`dpapi_decrypt` FFI wrappers (`windows` crate v0.58) |
| `rust-wallet/src/database/migrations.rs` | V4 migration: `mnemonic_dpapi BLOB` column |
| `rust-wallet/src/database/wallet_repo.rs` | Both `create_wallet()` methods store DPAPI blob alongside PIN encryption |
| `rust-wallet/src/database/connection.rs` | `try_dpapi_unlock()`, `store_dpapi_blob()` methods |
| `rust-wallet/src/main.rs` | Startup: DPAPI → legacy → locked flow |
| `frontend/src/pages/WalletPanelPage.tsx` | Fallback PIN unlock screen (`renderLocked()`) |

---

## 5. In-Memory State (Rust AppState)

| Component | Purpose | Lifetime |
|-----------|---------|----------|
| `database` | SQLite connection (Mutex-wrapped) | App lifetime |
| `balance_cache` | Instant balance reads | App lifetime |
| `fee_rate_cache` | ARC fee policy (1hr TTL) | App lifetime |
| `price_cache` | CryptoCompare + CoinGecko (5min TTL) | App lifetime |
| `auth_sessions` | BRC-103/104 state | Request-scoped |
| `whitelist` | **DEPRECATED** — `DomainWhitelistManager` (JSON file). Kept for legacy endpoint compat only. | App lifetime |
| `cached_mnemonic` | Decrypted mnemonic (WalletDatabase field). Set by DPAPI on startup or PIN unlock. | Session |

### In-Memory State (C++ Singletons)

| Singleton | Purpose | Lifetime |
|-----------|---------|----------|
| `DomainPermissionCache` | DB-backed domain trust levels, queried via WinHTTP to Rust | App lifetime (in-memory cache) |
| `PendingRequestManager` | Per-request map for auth/domain approval flows | App lifetime |
| `WalletStatusCache` | Cached wallet exists/locked status | App lifetime (30s TTL) |

---

## 6. macOS Considerations

When porting to macOS, the following Windows-specific components need platform alternatives:

### DPAPI Replacement: macOS Keychain

| Windows | macOS | Notes |
|---------|-------|-------|
| DPAPI (`CryptProtectData`) | **Keychain Services** (`SecItemAdd`/`SecItemCopyMatching`) | Tied to macOS user login |
| `%APPDATA%\HodosBrowser\` | `~/Library/Application Support/HodosBrowser/` | Standard macOS app data path |
| WinHTTP (interceptor HTTP calls) | `NSURLSession` or `libcurl` | For `DomainPermissionCache`, `WalletStatusCache` |
| `CreateWindowEx` (overlays) | NSWindow / CEF macOS APIs | Overlay window creation |
| Windows Credential Manager | macOS Keychain (alternative to Keychain Services) | For any credential storage |

### macOS Keychain API

```objc
// Store mnemonic in Keychain (equivalent to DPAPI)
SecItemAdd(@{
    kSecClass: kSecClassGenericPassword,
    kSecAttrService: @"com.hodos.browser.wallet",
    kSecAttrAccount: @"mnemonic",
    kSecValueData: mnemonicData,
    kSecAttrAccessible: kSecAttrAccessibleWhenUnlockedThisDeviceOnly
}, NULL);
```

Key difference: macOS Keychain can optionally require Touch ID or password confirmation per-access. For auto-unlock behavior matching Windows DPAPI, use `kSecAttrAccessibleWhenUnlockedThisDeviceOnly` without additional access control.

### Files to Modify for macOS

| File | Change Needed |
|------|--------------|
| `rust-wallet/src/crypto/dpapi.rs` | Add macOS Keychain implementation behind `#[cfg(target_os = "macos")]` |
| `cef-native/src/core/HttpRequestInterceptor.cpp` | Replace WinHTTP with platform-neutral HTTP client |
| `cef-native/cef_browser_shell.cpp` | macOS window management (NSWindow) |
| `cef-native/src/core/BRC100Bridge.cpp` | Replace WinHTTP `makeHttpRequest` |

### Sprint Recommendation

A dedicated "macOS Platform Sprint" should cover:
1. DPAPI → Keychain migration (Rust)
2. WinHTTP → cross-platform HTTP (C++)
3. Window/overlay management (C++)
4. Build system (CMake + Xcode)
5. Code signing and notarization
6. Testing on macOS 13+ (Ventura)

---

## 7. Future Cleanup Items

- [ ] **Remove `domainWhitelist.json` + `DomainWhitelistManager`** — C++ reads from DB via `DomainPermissionCache` now. JSON file is written (legacy dual-write in `add_domain()` handler) but never read. Steps: (1) change C++ `DomainWhitelistTask` to POST to `/domain/permissions` instead of `/domain/whitelist/add`, (2) remove `DomainWhitelistManager` from Rust, (3) remove `whitelist` from `AppState`, (4) delete `domain_whitelist.rs`
- [ ] Remove deprecated `utxos` table (still has ~10 code refs)
- [ ] Remove `identity.json` (migrate to Rust-managed identity)
- [ ] Consolidate debug logging (C++ Logger + Rust env_logger → unified)
- [ ] Audit `Default\` CEF cache for unnecessary data retention
- [ ] Add DB encryption at rest for all tables (SQLCipher or similar) — future consideration

---

**End of Document**
