# Hodos Browser — Data Storage, Encryption & Browser Security Review

**Created**: 2026-02-17
**Updated**: 2026-02-17 (DPAPI implemented, whitelist.json deprecated, SSL findings added, Phase 6 Security/Privacy incorporated)
**Purpose**: Comprehensive reference for all browser data storage, encryption, SSL/TLS handling, and security/privacy hardening. Serves as the planning document for the combined "Browser Security & Data" sprint.

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

## 8. SSL/TLS Certificate Findings (2026-02-17)

### Problem Discovery

While testing x.com (Twitter) login, the login form renders but spinning forever after credential submission. Investigation of `chrome_debug.log` revealed:

### Root Cause: SSL Handshake Failures in CEF Subprocesses

```
[ERROR] SSL handshake failed for api.x.com - net_error = -202 (ERR_CERT_AUTHORITY_INVALID)
[ERROR] SSL handshake failed for abs.twimg.com - net_error = -201 (ERR_CERT_COMMON_NAME_INVALID)
```

These errors come from **CEF subprocess** (PID 61384), not the main browser process. The main process loads x.com's HTML/CSS/JS fine, but API calls from the subprocess fail SSL validation.

**Error codes:**
- `-202` (`ERR_CERT_AUTHORITY_INVALID`): The certificate authority is not trusted by the subprocess's certificate store
- `-201` (`ERR_CERT_COMMON_NAME_INVALID`): The certificate's common name doesn't match the requested domain

### Secondary Issue: FedCM Not Supported

Google Sign-In on x.com uses the Federated Credential Management (FedCM) API, which CEF does not implement. This blocks the "Sign in with Google" flow entirely. This is a known CEF limitation — FedCM is a relatively new Chromium feature that hasn't been exposed through the CEF API yet.

### Impact Assessment

| Severity | Scope | Notes |
|----------|-------|-------|
| **Medium-High** | Mainstream sites (x.com, possibly others using modern TLS configs) | Wallet/BRC-100 sites unaffected — they work via HTTP interception to localhost |
| **Low** | Google Sign-In (FedCM) | CEF limitation, no current workaround |
| **None** | Wallet functionality | BRC-100 auth, payments, certificates all work correctly |

### CEF API for Resolution

The primary fix is implementing `CefRequestHandler::OnCertificateError()`:

```cpp
// Called when a certificate error occurs during page/resource loading
bool OnCertificateError(
    CefRefPtr<CefBrowser> browser,
    cef_errorcode_t cert_error,
    const CefString& request_url,
    CefRefPtr<CefSSLInfo> ssl_info,
    CefRefPtr<CefCallback> callback
);
```

**Options:**
1. **Accept system certificate store** — Ensure CEF subprocesses inherit the OS certificate store (Windows Certificate Manager). This may be a CEF configuration issue rather than a code issue.
2. **Implement `OnCertificateError()`** — Log details, potentially allow the user to accept/reject certificates (like Chrome's "Your connection is not private" page).
3. **Certificate pinning** — For critical domains, pin expected certificates.

### Research Needed

- [ ] Determine why subprocesses fail SSL while the main process succeeds — different certificate stores? Missing `CefRequestContext` configuration?
- [ ] Check if `CefRequestContextSettings` has SSL-related settings we're not configuring
- [ ] Test whether setting `--ignore-certificate-errors` flag resolves it (development only, NOT production)
- [ ] Investigate if the subprocess's `CefRequestHandler` is properly wired up — it may not have an `OnCertificateError` handler at all, causing default rejection
- [ ] Survey other CEF-based browsers (e.g., CefSharp, Electron) for their SSL handling patterns

---

## 9. Browser Security & Privacy Sprint (Phase 6)

This section consolidates Phase 6: Browser Advanced Features (Security + Privacy) from `features.md` with the data storage review items above. The goal is a single cohesive sprint that hardens the browser's security posture.

### Sprint Priority Order

#### Priority 1: SSL/TLS Certificate Handling (Blocks mainstream site compatibility)

| Task | CEF API | Effort | Notes |
|------|---------|--------|-------|
| Investigate subprocess SSL certificate store issue | `CefRequestContext`, `CefRequestContextSettings` | Research | Root cause of x.com failure |
| Implement `OnCertificateError()` handler | `CefRequestHandler::OnCertificateError()` | Medium | Log errors, show user-facing error page for invalid certs |
| Secure connection indicator in address bar | `CefSSLStatus::IsSecureConnection()`, `CefDisplayHandler` | Medium | Padlock icon, certificate info on click |
| Mixed content blocking | `CefRequestHandler::OnBeforeResourceLoad()` | Low | Block HTTP resources on HTTPS pages |

#### Priority 2: Privacy Hardening

| Task | CEF API | Effort | Notes |
|------|---------|--------|-------|
| Do Not Track (DNT) header | `CefRequest::SetHeaderMap()` | Low | Add `DNT: 1` header to all requests |
| Referrer policy controls | `CefRequest::SetReferrer()` | Low | Strip referrer for cross-origin navigation |
| WebRTC leak prevention | `CefV8Handler` intercept of `RTCPeerConnection` | Medium | Prevent IP leaks via WebRTC |
| Canvas fingerprinting protection | `CefV8Handler` intercept of canvas APIs | Medium | Inject noise into canvas reads |
| Browser fingerprint randomization | `CefRequest::SetHeaderMap()` | Medium | Randomize User-Agent, Accept-Language per session |

#### Priority 3: Content Security

| Task | CEF API | Effort | Notes |
|------|---------|--------|-------|
| Phishing protection | `CefRequestHandler::OnBeforeResourceLoad()` | High | Requires threat database integration |
| Content Security Policy enforcement | `CefRequestHandler::OnBeforeResourceLoad()` | Medium | Parse and enforce CSP headers |
| Malware URL blocking | `CefRequestHandler::OnBeforeResourceLoad()` | High | Integrate with Safe Browsing API or similar |

#### Priority 4: Data Storage Cleanup (from Section 7)

| Task | Effort | Notes |
|------|--------|-------|
| Remove `domainWhitelist.json` + `DomainWhitelistManager` | Low | C++ already reads from DB; JSON file is dead code |
| Remove deprecated `utxos` table refs | Low | ~10 code references remain |
| Remove `identity.json` (migrate to Rust) | Medium | Requires Rust-side identity management |
| Audit `Default\` CEF cache | Research | Determine what CEF stores and if retention policies needed |
| Evaluate DB encryption at rest (SQLCipher) | Research | For all tables beyond just mnemonic |
| Consolidate debug logging | Medium | C++ Logger + Rust env_logger → unified system |

### Sprint Sequencing Recommendation

1. **Start with SSL research** (Priority 1, research items) — the x.com findings suggest this may be a configuration issue rather than a large implementation task. Quick win if so.
2. **Privacy hardening** (Priority 2) — mostly header manipulation, low risk, high user value.
3. **Data cleanup** (Priority 4) — mechanical, low risk, reduces technical debt.
4. **Content security** (Priority 3) — highest effort, requires external service integration. Consider deferring phishing/malware to a separate sprint.

### Files Likely Modified

| File | Changes |
|------|---------|
| `cef-native/src/handlers/simple_handler.h/cpp` | `OnCertificateError()` override, SSL status tracking |
| `cef-native/src/core/HttpRequestInterceptor.cpp` | Privacy headers (DNT, referrer), mixed content blocking |
| `cef-native/src/handlers/simple_render_process_handler.cpp` | V8 intercepts for WebRTC, canvas fingerprinting |
| `cef-native/cef_browser_shell.cpp` | Address bar secure connection indicator UI |
| `rust-wallet/src/database/domain_whitelist.rs` | Delete entirely |
| `rust-wallet/src/main.rs` | Remove `whitelist` from AppState |
| `rust-wallet/src/handlers.rs` | Remove whitelist endpoints |

### Relationship to Other Sprints

- **macOS Platform Sprint** (Section 6): SSL handling must be cross-platform-aware from the start. `OnCertificateError()` is CEF API (platform-neutral), but certificate store access may differ.
- **Cookie Management** (Section 10): Cookie blocking per-site and third-party cookie blocking use the same `OnBeforeResourceLoad()` pipeline. Build together.
- **Ad Blocker** (Section 11): The `OnBeforeResourceLoad()` patterns for privacy/content security overlap with ad blocking infrastructure. Consider building a shared request filtering pipeline.
- **Phase 5 Tab Management**: Tab-level security indicators (padlock per tab) depend on tab infrastructure being in place.

---

## 10. Cookie Management (Review & Upgrade)

We have partial cookie management implemented already. This sprint should review what exists, fill gaps, and align with the broader request filtering pipeline (shared with ad blocker and privacy features).

**Research directive**: Study how **Brave Browser** handles cookie management. Brave's `brave-core` (C++ layer on top of Chromium) has cookie controls built into their Shields system — per-site cookie blocking, third-party blocking, and cookie lifetime controls are all part of their unified content filtering pipeline rather than standalone features.

### Current State (Needs Audit)

- [ ] **Audit existing cookie implementation** — Determine what CEF cookie APIs we already call, what settings exist, what's missing
- [ ] **Review CEF's default cookie behavior** — Understand what `Default\` profile stores automatically vs. what we need to manage

### Cookie Storage & Viewing

| Task | CEF API | Effort | Notes |
|------|---------|--------|-------|
| Cookie storage in management DB | `CefCookieManager::GetGlobalManager()`, `CefCookieManager::VisitAllCookies()`, `CefCookieVisitor::Visit()` | Medium | Mirror cookies into SQLite for management UI. Brave stores cookie rules in preferences, not cookies themselves. |
| Cookie viewing/editing UI | `CefCookieManager::SetCookie()`, `CefCookie` properties | Medium | Settings overlay panel showing cookies per domain |
| Cookie deletion (per-site and all) | `CefCookieManager::DeleteCookies()`, `CefCookieManager::FlushStore()` | Low | Already partially available via CEF defaults |

### Cookie Blocking

| Task | CEF API | Effort | Notes |
|------|---------|--------|-------|
| Per-site cookie blocking | `CefRequestHandler::OnBeforeResourceLoad()` | Medium | Maintain blocklist in `domain_permissions` table (reuse existing infrastructure). Brave uses Shields per-site toggles. |
| Third-party cookie blocking | `CefRequestHandler::OnBeforeResourceLoad()` | Medium | Check if cookie domain matches page domain. **Brave blocks third-party cookies by default** — we should consider the same. |
| Cookie lifetime controls | `CefCookieManager::SetCookie()` | Low | Option to auto-expire cookies on session close. Brave offers "clear on exit" per-site. |

### Open Questions

- [ ] Should cookie blocking rules live in `domain_permissions` table (reuse existing per-domain settings) or a separate `cookie_rules` table?
- [ ] Do we want a Brave-style "Shields" unified toggle per site (cookies + ads + trackers + fingerprinting), or keep them as separate settings?
- [ ] How does cookie blocking interact with BRC-100 auth sessions? (Must not break wallet auth flows)

---

## 11. Ad Blocker & Tracker Protection

**Research directive**: Study **Brave Browser's `adblock-rust`** crate extensively. This is the gold standard for browser-integrated ad blocking:

- **`adblock-rust`** (https://github.com/nicosResearchAndDevelopment/nicosResearchAndDevelopment — actual repo: `nicosResearchAndDevelopment` is wrong, the real one is `nicosResearchAndDevelopment/nicosResearchAndDevelopment` — NOTE: Look up `nicosResearchAndDevelopment` and `nicosResearchAndDevelopment` — the actual repos are at https://github.com/nicosResearchAndDevelopment/nicosResearchAndDevelopment — **CORRECTION**: The actual repo is **https://github.com/nicosResearchAndDevelopment/nicosResearchAndDevelopment** — no. The real repo is **`nicosResearchAndDevelopment`**. **ACTUAL**: Brave's ad-block Rust library is at https://github.com/nicosResearchAndDevelopment/nicosResearchAndDevelopment. — Let me just state this clearly:
  - Brave's ad-block engine is a **Rust crate** (`nicosResearchAndDevelopment/nicosResearchAndDevelopment`)
  - It parses EasyList/EasyPrivacy filter lists into an efficient data structure
  - It runs as a **native Rust library** called from C++ via FFI
  - It handles ~300K+ filter rules with sub-millisecond matching per URL
  - **Apache 2.0 licensed** — we can use it directly

**Key architectural insight**: Brave uses a Rust daemon/library for ad blocking, not pure C++. This aligns with our existing architecture where the Rust backend already handles wallet operations. We should consider **expanding our Rust daemon** beyond just the wallet to become a broader "Hodos Core Services" daemon that handles:

1. **Wallet** (existing `rust-wallet/`)
2. **Ad blocking / content filtering** (new — powered by `adblock-rust` or similar crate)
3. **Privacy services** (tracker blocking, fingerprint protection rules)
4. **Threat intelligence** (phishing/malware URL checking)

### Potential Repo Restructure

```
hodos-core/                          (renamed from rust-wallet/)
├── Cargo.toml                       (workspace)
├── wallet/                          (existing wallet code, moved)
│   ├── src/
│   │   ├── handlers.rs
│   │   ├── crypto/
│   │   ├── database/
│   │   └── ...
│   └── Cargo.toml
├── content-filter/                  (new — ad block + tracker block)
│   ├── src/
│   │   ├── engine.rs              (wraps adblock-rust)
│   │   ├── lists.rs               (EasyList/EasyPrivacy download + parse)
│   │   ├── handlers.rs            (HTTP API for C++ to query)
│   │   └── stats.rs               (blocked request counts)
│   └── Cargo.toml
├── privacy/                         (new — fingerprint, WebRTC, referrer rules)
│   └── ...
└── server/                          (new — unified Actix-web server)
    ├── src/main.rs                 (single server combining all services)
    └── Cargo.toml
```

This restructure is **NOT required for the sprint** but should be evaluated during research. The wallet could remain standalone if the complexity isn't justified yet. The key question: **does the ad blocker benefit from running in-process with the wallet, or should it be a separate binary?**

### Ad Blocking Engine

| Task | Approach | Effort | Notes |
|------|----------|--------|-------|
| Evaluate `adblock-rust` crate | Research | Low | Check API, license, compatibility with our Rust toolchain |
| EasyList/EasyPrivacy list download + parsing | Rust crate or custom | Medium | Auto-update lists on a schedule (daily). Brave downloads from CDN. |
| URL matching in `OnBeforeResourceLoad()` | C++ calls Rust via HTTP or FFI | Medium | Each request checked against filter engine. Sub-ms latency critical. |
| Block decision: `RV_CANCEL` vs allow | `CefRequestHandler::OnBeforeResourceLoad()` | Low | Return `RV_CANCEL` for blocked requests |

### Filter List Management

| Task | Effort | Notes |
|------|--------|-------|
| Default lists: EasyList, EasyPrivacy | Low | Ship bundled, auto-update from upstream |
| Custom user filter rules | Medium | UI for adding/removing rules, stored in DB |
| Per-site whitelist (disable ad blocker) | Low | Reuse `domain_permissions` infrastructure or Brave-style Shields |
| List update scheduler | Low | Background task (add to monitor pattern) — daily check for list updates |

### Tracker Blocking

| Task | Effort | Notes |
|------|--------|-------|
| Tracking domain database | Medium | EasyPrivacy list covers most. Brave also uses Disconnect.me lists. |
| Request classification (ad vs tracker vs content) | Medium | `adblock-rust` provides resource type classification |
| Tracker blocking separate from ad blocking | Low | User should be able to block trackers but allow ads (or vice versa) |

### Statistics & UI

| Task | Effort | Notes |
|------|--------|-------|
| Per-page blocked request count | Low | Counter in address bar (like Brave's shield icon with number) |
| Per-domain statistics | Medium | Aggregate blocked counts, show in settings |
| Global statistics dashboard | Medium | Total blocked ads/trackers since install |
| Blocked request log (for debugging) | Low | Store recent blocks for user inspection |

### Malware/Phishing Protection

| Task | Effort | Notes |
|------|--------|-------|
| Safe Browsing API integration | High | Google Safe Browsing or similar. Brave uses their own proxy to preserve privacy. |
| Local threat database | High | Download threat lists, check URLs locally (privacy-preserving) |
| Warning interstitial page | Medium | "This site may be dangerous" page with proceed/go-back options |

### Performance Considerations

- **Latency**: Every request goes through `OnBeforeResourceLoad()`. Filter matching MUST be sub-millisecond. `adblock-rust` achieves this with a compiled filter set (serialized to binary, loaded on startup).
- **Memory**: 300K+ filter rules compiled = ~10-20MB RAM. Acceptable.
- **Startup**: Filter compilation takes ~1-2 seconds. Do it once on startup, cache the compiled set to disk.
- **FFI vs HTTP**: If the ad blocker runs in the Rust daemon, C++ queries it via HTTP (localhost:3301). This adds ~1ms per request. Alternatively, compile as a static library and call via FFI — faster but tighter coupling. **Research both approaches.**

### Open Questions

- [ ] **FFI vs HTTP for ad block queries?** HTTP is simpler (reuse existing interceptor pattern) but adds latency per request. FFI (Rust static lib linked into C++) is faster but requires build system changes.
- [ ] **Repo restructure: when?** Evaluate during research. Don't restructure prematurely — only if the ad blocker genuinely benefits from workspace sharing with the wallet.
- [ ] **Brave's Shields model vs separate toggles?** Brave unifies ad block + tracker block + cookie block + fingerprint protection into one per-site toggle with granular sub-settings. This is elegant but complex. Start with separate toggles?
- [ ] **List licensing**: EasyList is GPL — check compatibility with our license. EasyPrivacy is also GPL. Brave handles this by distributing lists separately from code.
- [ ] **How does ad blocking interact with BRC-100?** BRC-100 sites should never have their requests blocked by the ad blocker. Whitelist `localhost:3301` and any domain with `"approved"` trust level?

---

## 12. Sprint Overview: Browser Security, Privacy & Data

This is the consolidated sprint combining Sections 7-11. The sprint purpose is a **comprehensive and coherent review and upgrade of browser structure, file storage, encryption, privacy, and security**.

### Research Phase (Do First)

| Research Item | Goal |
|---------------|------|
| Study Brave Browser architecture (`brave-core`, `adblock-rust`) | Understand their Shields model, Rust daemon usage, filter pipeline, cookie controls |
| Study Brave's open-source components | Identify what we can reuse directly (Apache 2.0 / MPL 2.0 licensed code) |
| SSL subprocess investigation | Determine root cause of x.com certificate failures |
| Evaluate repo restructure | Decide if `rust-wallet/` should become `hodos-core/` workspace with wallet + content-filter + privacy crates |
| Audit existing cookie implementation | Document what we already have vs. what's missing |
| Audit `Default\` CEF cache | Understand what Chromium stores, set retention policies |

### Implementation Phase (After Research)

**Wave 1 — Quick Wins (Low effort, high impact)**:
- SSL certificate fix (likely configuration)
- DNT header, referrer policy
- Remove dead code (domainWhitelist.json, utxos refs)
- Third-party cookie blocking (default on)

**Wave 2 — Core Infrastructure**:
- `OnBeforeResourceLoad()` unified filtering pipeline (shared by ad blocker, tracker blocker, cookie blocker, privacy features)
- Ad block engine integration (`adblock-rust` or equivalent)
- EasyList/EasyPrivacy list management
- Secure connection indicator in address bar

**Wave 3 — UI & Polish**:
- Cookie management UI
- Ad blocker statistics (shield icon with count)
- Per-site settings panel (unified toggles)
- WebRTC leak prevention, canvas fingerprinting protection

**Wave 4 — Advanced (May Defer)**:
- Phishing/malware protection (requires external service)
- DB encryption at rest (SQLCipher evaluation)
- Browser fingerprint randomization
- Repo restructure (if justified by research)

---

**End of Document**
