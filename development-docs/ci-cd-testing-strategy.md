# Hodos Browser — CI/CD, Testing & Distribution Strategy

**Created**: 2026-02-17
**Purpose**: Living document for long-term development quality control. Covers unit testing, integration testing, CI/CD pipelines, version control, and distribution. Will be expanded through sprints as we build toward MVP.

---

## 1. Testing Strategy by Stack Layer

### 1a. Rust Wallet (HIGHEST PRIORITY — handles real money)

**Tool**: `cargo test` (built-in)

Crypto correctness is the single most valuable thing to test. Incorrect signing = lost funds.

| Priority | Module | What to Test | Test Type |
|----------|--------|-------------|-----------|
| **P0** | `crypto/brc42.rs` | Key derivation against ts-sdk vectors | Unit (known vectors) |
| **P0** | `crypto/signing.rs` | HMAC-SHA256, SHA-256 against NIST vectors | Unit (known vectors) |
| **P0** | `crypto/aesgcm_custom.rs` | AES-256-GCM encrypt/decrypt against NIST vectors | Unit (known vectors) |
| **P0** | `handlers.rs` (well_known_auth) | BRC-104 nonce handling (post GHSA-vjpq-xx5g-qvmm fix) | Unit |
| **P1** | `recovery.rs` | BIP-39 mnemonic → seed, BIP-32 HD derivation | Unit (known vectors) |
| **P1** | `beef.rs` / `beef_helpers.rs` | BEEF parse/serialize against ts-sdk hex | Unit (known vectors) |
| **P1** | `transaction/sighash.rs` | ForkID SIGHASH computation | Unit (known vectors) |
| **P1** | `database/migrations.rs` | V1 fresh schema, V2→V3→V4 upgrades | Integration (`:memory:` SQLite) |
| **P2** | `database/*_repo.rs` | CRUD operations, edge cases | Integration (`:memory:` SQLite) |
| **P2** | `handlers.rs` | Request validation, response format | Integration (`actix_web::test`) |
| **P2** | `domain_permission_repo.rs` | Spending limits, rate limit logic | Unit |
| **P3** | `monitor/` tasks | Background task logic (mock DB) | Unit |
| **P3** | `price_cache.rs` | TTL, fallback behavior | Unit (mock HTTP) |

**Handler-level integration tests** (no real server needed):
```rust
#[actix_web::test]
async fn test_wallet_balance_handler() {
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(test_app_state()))
            .route("/wallet/balance", web::get().to(wallet_balance))
    ).await;
    let req = test::TestRequest::get().uri("/wallet/balance").to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
}
```

### 1b. React/TypeScript Frontend (MEDIUM PRIORITY)

**Tool**: Vitest (native Vite integration, faster than Jest)

```bash
npm install -D vitest @testing-library/react @testing-library/jest-dom jsdom
```

| Priority | What | Test Type |
|----------|------|-----------|
| **P1** | Utility functions (price formatting, satoshi/USD conversion) | Unit |
| **P1** | Input validation (send form, PIN, mnemonic) | Unit |
| **P2** | React hooks (`useBalance`, `useBackgroundBalancePoller`) | Unit (mock fetch) |
| **P2** | DomainPermissionForm validation logic | Component |
| **P3** | Notification overlay state transitions | Component |
| **Later** | Full page rendering (WalletPanel, BRC100AuthOverlay) | Component |

**Do NOT test**: CSS styling, CEF-specific bridge behavior (those are integration/E2E).

### 1c. C++ / CEF Shell (LOW PRIORITY for now)

**Tool**: Google Test (industry standard for C++)

CEF testing is notoriously difficult — requires message loop, browser process, render process. Strategy:

| Priority | What | Approach |
|----------|------|----------|
| **P2** | Pure C++ functions (URL parsing, JSON manipulation, domain matching) | Google Test (extract into testable functions) |
| **P3** | `SessionManager` logic (spending tracking, rate limiting) | Google Test (no CEF dependency) |
| **P3** | `DomainPermissionCache` / `BSVPriceCache` | Google Test (mock HTTP) |
| **Later** | Full CEF integration (interceptor, overlay, IPC) | Manual test scripts → custom E2E harness |

**Brave's approach**: Custom test harness built on CEF's test infrastructure. Overkill for us until post-MVP.

### 1d. Integration / E2E Tests (LATER)

- Playwright/Selenium won't work with CEF directly
- **MVP approach**: Manual test checklist for critical flows
- **Post-MVP**: Custom E2E harness using CEF DevTools protocol or IPC to drive interactions
- **Critical flows to automate first**: Wallet creation, send transaction, BRC-100 auth handshake, domain approval

---

## 2. ts-sdk Test Vectors for Rust Wallet

The BSV TypeScript SDK (`bsv-blockchain/ts-sdk`) contains extensive test vectors that can be ported directly to Rust `#[test]` functions. These are deterministic (hardcoded hex values) and require no network access.

### Tier 1 — Port First (core wallet crypto)

#### BRC-42 Private Key Derivation (5 vectors)

**Source**: `src/primitives/__tests/BRC42.private.vectors.ts`

| # | Sender Public Key | Recipient Private Key | Invoice Number | Expected Derived Key |
|---|---|---|---|---|
| 1 | `033f9160df035156f1c48e75eae99914fa1a1546bec19781e8eddb900200bff9d1` | `6a1751169c111b4667a6539ee1be6b7cd9f6e9c8fe011a5f2fe31e03a15e0ede` | `f3WCaUmnN9U=` | `761656715bbfa172f8f9f58f5af95d9d0dfd69014cfdcacc9a245a10ff8893ef` |
| 2 | `027775fa43959548497eb510541ac34b01d5ee9ea768de74244a4a25f7b60fae8d` | `cab2500e206f31bc18a8af9d6f44f0b9a208c32d5cca2b22acfe9d1a213b2f36` | `2Ska++APzEc=` | `09f2b48bd75f4da6429ac70b5dce863d5ed2b350b6f2119af5626914bdb7c276` |
| 3 | `0338d2e0d12ba645578b0955026ee7554889ae4c530bd7a3b6f688233d763e169f` | `7a66d0896f2c4c2c9ac55670c71a9bc1bdbdfb4e8786ee5137cea1d0a05b6f20` | `cN/yQ7+k7pg=` | `7114cd9afd1eade02f76703cc976c241246a2f26f5c4b7a3a0150ecc745da9f0` |
| 4 | `02830212a32a47e68b98d477000bde08cb916f4d44ef49d47ccd4918d9aaabe9c8` | `6e8c3da5f2fb0306a88d6bcd427cbfba0b9c7f4c930c43122a973d620ffa3036` | `m2/QAsmwaA4=` | `f1d6fb05da1225feeddd1cf4100128afe09c3c1aadbffbd5c8bd10d329ef8f40` |
| 5 | `03f20a7e71c4b276753969e8b7e8b67e2dbafc3958d66ecba98dedc60a6615336d` | `e9d174eff5708a0a41b32624f9b9cc97ef08f8931ed188ee58d5390cad2bf68e` | `jgpUIjWFlVQ=` | `c5677c533f17c30f79a40744b18085632b262c0c13d87f3848c385f1389f79a6` |

These directly test `brc42::derive_child_private_key()`.

#### BRC-42 Public Key Derivation (5 vectors)

**Source**: `src/primitives/__tests/BRC42.public.vectors.ts`

| # | Sender Private Key | Recipient Public Key | Invoice Number | Expected Derived Public Key |
|---|---|---|---|---|
| 1 | `583755110a8c059de5cd81b8a04e1be884c46083ade3f779c1e022f6f89da94c` | `02c0c1e1a1f7d247827d1bcf399f0ef2deef7695c322fd91a01a91378f101b6ffc` | `IBioA4D/OaE=` | `03c1bf5baadee39721ae8c9882b3cf324f0bf3b9eb3fc1b8af8089ca7a7c2e669f` |
| 2 | `2c378b43d887d72200639890c11d79e8f22728d032a5733ba3d7be623d1bb118` | `039a9da906ecb8ced5c87971e9c2e7c921e66ad450fd4fc0a7d569fdb5bede8e0f` | `PWYuo9PDKvI=` | `0398cdf4b56a3b2e106224ff3be5253afd5b72de735d647831be51c713c9077848` |
| 3 | `d5a5f70b373ce164998dff7ecd93260d7e80356d3d10abf928fb267f0a6c7be6` | `02745623f4e5de046b6ab59ce837efa1a959a8f28286ce9154a4781ec033b85029` | `X9pnS+bByrM=` | `0273eec9380c1a11c5a905e86c2d036e70cbefd8991d9a0cfca671f5e0bbea4a3c` |
| 4 | `46cd68165fd5d12d2d6519b02feb3f4d9c083109de1bfaa2b5c4836ba717523c` | `031e18bb0bbd3162b886007c55214c3c952bb2ae6c33dd06f57d891a60976003b1` | `+ktmYRHv3uQ=` | `034c5c6bf2e52e8de8b2eb75883090ed7d1db234270907f1b0d1c2de1ddee5005d` |
| 5 | `7c98b8abd7967485cfb7437f9c56dd1e48ceb21a4085b8cdeb2a647f62012db4` | `03c8885f1e1ab4facd0f3272bb7a48b003d2e608e1619fb38b8be69336ab828f37` | `PPfDTTcl1ao=` | `03304b41cfa726096ffd9d8907fe0835f888869eda9653bca34eb7bcab870d3779` |

#### HMAC-SHA256 (5 vectors)

**Source**: `src/primitives/__tests/HMAC.test.ts`

| # | Key (hex) | Message | Expected HMAC (hex) |
|---|---|---|---|
| 1 | `000102030405...3e3f` (64 bytes sequential) | `"Sample message for keylen=blocklen"` (UTF-8) | `8bb9a1db9806f20df7f77b82138c7914d174d59e13dc4d0169c9057b133e1d62` |
| 2 | `000102030405...1e1f` (32 bytes sequential) | `"Sample message for keylen<blocklen"` (UTF-8) | `a28cf43130ee696a98f14a37678b56bcfcbdd9e5cf69717fecf5480f0ebdf790` |
| 3 | `000102030405...6263` (100 bytes sequential) | `"Sample message for keylen=blocklen"` (UTF-8) | `bdccb6c72ddeadb500ae768386cb38cc41c63dbb0878ddb9c7a38a431b78378d` |
| 4 | `000102030405...2e2f30` (49 bytes sequential) | `"Sample message for keylen<blocklen, with truncated tag"` (UTF-8) | `27a8b157839efeac98df070b331d593618ddb985d403c0c786d23b5d132e57c7` |
| 5 | `48f38d0c6a344959cc94502b7b5e8dffb6a5f41795d9066fc9a649557167ee2f` | `1d495eef7761b65dccd0a983d2d7204fea28b5c81f1758046e062eb043755ea1` (hex bytes) | `cf5ad5984f9e43917aa9087380dac46e410ddc8a7731859c84e9d0f31bd43655` |

Vector 5 is most relevant — raw hex bytes for both key and message, matching BRC-42 internal HMAC usage.

#### BRC-3 Signature Compliance (1 vector)

**Source**: `src/wallet/__tests/ProtoWallet.test.ts`

- **Private key**: `6a2991c9de20e38b31d7ea147bf55f5039e4bbc073160f5e0d541d1f17e321b8`
- **Data**: `"BRC-3 Compliance Validated!"` (UTF-8)
- **Protocol**: `[2, 'BRC3 Test']`, KeyID: `'42'`
- **Counterparty**: `0294c479f762f6baa97fbcd4393564c1d7bd8336ebd15928135bbcf575cd1a71a1`
- **Expected DER signature bytes**: `[48, 68, 2, 32, 43, 34, 58, 156, 219, 32, 50, 70, 29, 240, 155, 137, 88, 60, 200, 95, 243, 198, 201, 21, 56, 82, 141, 112, 69, 196, 170, 73, 156, 6, 44, 48, 2, 32, 118, 125, 254, 201, 44, 87, 177, 170, 93, 11, 193, 134, 18, 70, 9, 31, 234, 27, 170, 177, 54, 96, 181, 140, 166, 196, 144, 14, 230, 118, 106, 105]`

Tests full pipeline: BRC-42 derive key + ECDSA sign.

#### BRC-2 HMAC Compliance (1 vector)

**Source**: `src/wallet/__tests/ProtoWallet.test.ts`

- **Private key**: `6a2991c9de20e38b31d7ea147bf55f5039e4bbc073160f5e0d541d1f17e321b8`
- **Data**: `"BRC-2 HMAC Compliance Validated!"` (UTF-8)
- **Protocol**: `[2, 'BRC2 Test']`, KeyID: `'42'`
- **Counterparty**: `0294c479f762f6baa97fbcd4393564c1d7bd8336ebd15928135bbcf575cd1a71a1`
- **Expected HMAC bytes**: `[81, 240, 18, 153, 163, 45, 174, 85, 9, 246, 142, 125, 209, 133, 82, 76, 254, 103, 46, 182, 86, 59, 219, 61, 126, 30, 176, 232, 233, 100, 234, 14]`

Tests full pipeline: BRC-42 derive key + HMAC.

### Tier 2 — Port Soon (key infrastructure)

#### BIP-39 Mnemonic Vectors (24 vectors)

**Source**: `src/compat/__tests/Mnemonic.vectors.ts`

All 24 standard BIP-39 test vectors with passphrase `"TREZOR"`. Key examples:

| Entropy | First Words | Expected Seed (first 32 hex chars) |
|---------|-------------|-------------------------------------|
| `00000000000000000000000000000000` | `abandon abandon abandon...about` | `c55257c360c07c72029aebc1b53c05ed` |
| `ffffffffffffffffffffffffffffffff` | `zoo zoo zoo zoo...wrong` | `ac27495480225222079d7be181583751` |

Tests `recovery.rs` mnemonic-to-seed derivation.

#### BIP-32 HD Wallet Derivation (2 vector sets)

**Source**: `src/compat/__tests/HD.test.ts`

**Set 1** (seed: `000102030405060708090a0b0c0d0e0f`):
- Master xprv: `xprv9s21ZrQH143K3QTDL4LXw2F7HEK3wJUD2nW2nRk4stbPy6cq3jPPqjiChkVvvNKmPGJxWUtg6LnF5kejMRNNU3TGtRBeJgk33yuGBxrMPHi`
- 5 derivation depth levels with expected keys

Tests `recovery::derive_private_key_bip32()`.

#### AES-256-GCM Vectors (NIST standard)

**Source**: `src/primitives/__tests/AESGCM.test.ts`

| Test | Key (hex) | IV (hex) | Plaintext (hex) | Expected Ciphertext | Expected Tag |
|------|-----------|----------|-----------------|--------------------|----|
| GCM-14 (256-bit) | `000...0` (32B) | `000...0` (12B) | `000...0` (16B) | `cea7403d4d606b6e074ec5d3baf39d18` | `d0d1c8a799996bf0265b98b5d48ab919` |
| GCM-15 (256-bit) | `feffe992...` (32B) | `cafebabe...` (12B) | `d9313225...` | `522dc1f0...` | `b094dac5d93471bdec1a502270e3cc6c` |

Tests `crypto/aesgcm_custom.rs`. GCM-15 is most relevant (BRC-2 uses AES-256).

#### Hash Function Vectors

**Source**: `src/primitives/__tests/Hash.test.ts`

| Function | Input | Expected |
|----------|-------|----------|
| SHA-256 | `"abc"` (UTF-8) | `ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad` |
| SHA-256 | `deadbeef` (hex) | `5f78c33274e43fa9de5659265c1d917e25c03722dcb0b8d27db8d5feaa813953` |
| RIPEMD-160 | `"abc"` (UTF-8) | `8eb208f7e05d987a9b044a8e98c6b087f15a0bfc` |

### Tier 3 — Port When Practical (transaction handling)

#### BEEF Serialization Vectors

**Source**: `src/transaction/__tests/Beef.test.ts`

- Valid BEEF hex: `0100beef01fe83e5180002...` (12,000+ hex chars — contains 1 BUMP + 1 transaction)
- Invalid BEEF hex: `0100beff01fe83e5` (wrong version magic)
- Expected TXID: `bd4a39c6dce3bdd982be3c67eb04b83934fd431f8bcb64f9da4413c91c634d07`

Tests `beef.rs` parser.

#### Sighash Vectors (500+ available, port ~20)

**Source**: `src/script/__tests/sighashTestData.ts`

Format: `[raw_tx_hex, script_hex, input_index, hash_type, expected_sighash]`

Includes BSV ForkID SIGHASH and OTDA vectors. Tests `transaction/sighash.rs`.

#### BUMP / Merkle Path Vectors

**Source**: `src/transaction/__tests/bump.valid.vectors.ts`, `MerklePath.test.ts`

- Block height: 813706
- BUMP hex + expected merkle root: `57aab6e6fb1b697174ffb64e062c4728f2ffd33ddcfa02a43b64d8cd29b483b4`

Tests proof verification in `ProvenTxRepository`.

### Tier 4 — Reference Only

| Source | What | Notes |
|--------|------|-------|
| `PBKDF2.vectors.ts` | 13 PBKDF2-SHA512 vectors | BIP-39 seed derivation |
| `DRBG.vectors.ts` | 15 HMAC-DRBG vectors | Internal to ECDSA k-generation |
| `SymmetricKeyCompatibility.test.ts` | Cross-SDK AES-GCM vectors | Go SDK ↔ TS SDK interop |
| `Certificate.test.ts` | Certificate creation/signing | Protocol-level, mostly behavioral |
| `Peer.test.ts` | Mutual auth flow | Dynamic values, few portable vectors |

### How to Use These Vectors

```rust
// Example: rust-wallet/tests/brc42_vectors_test.rs
#[test]
fn test_derive_child_private_key_vector_1() {
    let sender_pubkey = hex::decode(
        "033f9160df035156f1c48e75eae99914fa1a1546bec19781e8eddb900200bff9d1"
    ).unwrap();
    let recipient_privkey = hex::decode(
        "6a1751169c111b4667a6539ee1be6b7cd9f6e9c8fe011a5f2fe31e03a15e0ede"
    ).unwrap();
    let invoice = "f3WCaUmnN9U=";

    let derived = derive_child_private_key(&recipient_privkey, &sender_pubkey, invoice).unwrap();

    assert_eq!(
        hex::encode(&derived),
        "761656715bbfa172f8f9f58f5af95d9d0dfd69014cfdcacc9a245a10ff8893ef"
    );
}
```

---

## 3. CI/CD Pipeline

### Tool: GitHub Actions

Best fit for our multi-language stack: native Windows runners, Rust toolchain support, MSVC pre-installed, free tier (2,000 min/month private repos).

### Pipeline Structure

```
PR opened / push to main
    ├── rust-check (parallel)     ← cargo check + cargo test + cargo clippy + cargo audit
    ├── frontend-check (parallel) ← npm ci + npm run build + npm run lint + npm audit
    └── cef-build (sequential)    ← Only on main merges, not PRs (expensive)
         └── Release workflow      ← Only on version tags (v*)
              └── Build + sign installer → GitHub Release (draft)
```

**Key principle**: Fast checks (Rust, frontend) run in parallel on every PR. Expensive CEF build only on main. Release build only on tags.

### The CEF Binary Problem

CEF binaries are ~350MB and change rarely. Solutions (in order of preference):
1. **`actions/cache`** — cache by CEF version string, 10GB limit is plenty
2. **External storage** (S3/Azure Blob) — download in CI from known URL
3. **Self-hosted runner** — when CI minutes become a cost issue

### Starter Workflow

```yaml
# .github/workflows/ci.yml
name: CI
on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

jobs:
  rust-check:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: actions/cache@v4
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            rust-wallet/target
          key: rust-${{ hashFiles('rust-wallet/Cargo.lock') }}
      - run: cargo check --manifest-path rust-wallet/Cargo.toml
      - run: cargo test --manifest-path rust-wallet/Cargo.toml
      - run: cargo clippy --manifest-path rust-wallet/Cargo.toml -- -D warnings
      - run: cargo audit --manifest-path rust-wallet/Cargo.toml

  frontend-check:
    runs-on: ubuntu-latest  # Frontend doesn't need Windows
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with:
          node-version: 20
          cache: 'npm'
          cache-dependency-path: frontend/package-lock.json
      - run: npm ci
        working-directory: frontend
      - run: npm run build
        working-directory: frontend
```

---

## 4. Version Control Strategy

### Branching: Trunk-Based Development

Used by VS Code, Brave, and most modern teams. Gitflow is overkill for a small team.

```
main (always releasable)
  ├── feature/domain-permissions   (short-lived, 1-3 days)
  ├── fix/nonce-vulnerability      (short-lived)
  └── release/1.0.x               (cut from main at release time)
```

**Rules**:
- `main` is the integration branch. All work merges via PR.
- Feature branches live < 1 week. Longer work uses feature flags.
- Release branches cut from `main` when shipping. Only critical fixes cherry-picked.
- No `develop` branch.
- Tag releases: `v1.0.0`, `v1.0.1`

### Versioning: Semantic Versioning

`MAJOR.MINOR.PATCH` with desktop-app pragmatism:
- **MAJOR**: Breaking changes to user data (wallet DB incompatibility, protocol changes)
- **MINOR**: New features (BRC protocol support, new UI panels)
- **PATCH**: Bug fixes, security patches
- **Build metadata**: `1.2.3+abc1234` (git short hash for internal builds)
- **Pre-release**: `1.0.0-beta.1`, `1.0.0-rc.1`

**Single version source**: Keep version in `Cargo.toml`, derive everywhere else (frontend `package.json`, C++ resource file, installer) via script.

---

## 5. Distribution & Auto-Update

### Phase 1 (MVP): GitHub Releases

- Build NSIS installer (simpler) or WiX MSI (more enterprise)
- Upload signed installer to GitHub Releases on each version tag
- Users download and install manually
- **Zero infrastructure cost** — GitHub handles hosting

### Phase 2 (Post-MVP): Auto-Update with WinSparkle

**WinSparkle** is the best fit for a native C++ CEF app:
- Open-source, mature, lightweight
- C++ API integrates naturally with CEF shell
- Checks an "appcast" XML file for new versions
- Handles download, verification, restart prompting
- Does NOT require Electron

**Appcast file** (hosted on GitHub Pages or S3):
```xml
<?xml version="1.0" encoding="utf-8"?>
<rss version="2.0" xmlns:sparkle="http://www.andymatuschak.org/xml-namespaces/sparkle">
  <channel>
    <item>
      <title>HodosBrowser 1.1.0</title>
      <sparkle:version>1.1.0</sparkle:version>
      <enclosure url="https://github.com/.../HodosBrowser-Setup-1.1.0.exe"
                 sparkle:dsaSignature="..." length="45000000"
                 type="application/octet-stream"/>
    </item>
  </channel>
</rss>
```

### Phase 3 (Scale): Delta Updates + Release Channels

- Full installers are 150-300MB (CEF is huge). Delta updates ship only changed files (~5-20MB).
- **Component updates**: Ship Rust binary + frontend bundle separately from CEF shell (which changes rarely).
- **Release channels**: Canary (opt-in early adopters, 1-week soak) → Stable (all users). Two appcast URLs.

### Code Signing (CRITICAL)

| Platform | Certificate | Cost | Notes |
|----------|------------|------|-------|
| **Windows (OV)** | Authenticode OV cert (DigiCert, Sectigo) | ~$200-400/yr | SmartScreen won't block. CI: store PFX as GitHub secret. |
| **Windows (EV)** | Authenticode EV cert | ~$400-600/yr | Immediately trusted (no reputation buildup). Requires hardware token or cloud signing. |
| **macOS** | Apple Developer cert + notarization | $99/yr | Required for macOS. Do when macOS build starts. |

**Get an OV certificate before first public release.** Without it, Windows SmartScreen will actively warn users away from the installer.

---

## 6. Security Scanning

### Start Now (zero effort)

| Tool | What | How |
|------|------|-----|
| `cargo audit` | Known Rust dependency vulnerabilities | Add to CI: `cargo audit` |
| `npm audit` | Known Node dependency vulnerabilities | Add to CI: `npm audit --audit-level=high` |
| **GitHub Dependabot** | Automated dependency update PRs | Enable in repo settings (free) |
| `cargo clippy` | Rust linting including security patterns | `cargo clippy -- -D warnings` in CI |

### Add Before Release

| Tool | What |
|------|------|
| `cargo-deny` | License checking + advisory DB + duplicate dep detection |
| `cargo-geiger` | Audit `unsafe` code in dependency tree (important — we handle crypto) |
| **GitHub Code Scanning** (CodeQL) | SAST for TypeScript, C++ patterns |

### Add When Scaling

| Tool | What |
|------|------|
| **Semgrep** | Custom SAST rules for Rust, TypeScript, C++ |
| **OWASP ZAP** | Dynamic scanning of Rust HTTP API |

---

## 7. Architectural Consideration: Repo Restructure

**Context**: Brave Browser uses a Rust daemon (`adblock-rust` crate) for ad blocking alongside their C++ browser shell. When we add ad blocking + content filtering (see `data-storage-and-encryption-review.md` Section 11), we should evaluate expanding `rust-wallet/` into a broader Rust workspace.

### Potential Structure

```
hodos-core/                          (renamed from rust-wallet/)
├── Cargo.toml                       (workspace)
├── wallet/                          (existing wallet code)
│   ├── src/
│   └── Cargo.toml
├── content-filter/                  (new — ad block + tracker block)
│   ├── src/
│   └── Cargo.toml
├── privacy/                         (new — fingerprint, WebRTC rules)
│   └── ...
└── server/                          (unified Actix-web server)
    ├── src/main.rs
    └── Cargo.toml
```

**Decision point**: Evaluate during the Browser Security & Data sprint (Section 12 of data-storage doc). Don't restructure prematurely — only if the ad blocker genuinely benefits from workspace sharing with the wallet.

**Open question**: FFI (Rust static lib linked into C++) vs HTTP (localhost API, current pattern) for ad block queries. HTTP is simpler but adds ~1ms per request. FFI is faster but tighter coupling.

---

## 8. Implementation Timeline

### Now (current sprint)
- [x] Fix BRC-104 nonce vulnerability (GHSA-vjpq-xx5g-qvmm)
- [x] Phase 2.3.8 certificate disclosure implemented (untested in production)
- [x] Phase 2.4.1b Rust defense-in-depth domain permission checks
- [x] Phase 2.4.2 Domain permissions management UI (Approved Sites tab)

### Before MVP Release
- [ ] Port Tier 1 ts-sdk test vectors to Rust (BRC-42, HMAC, BRC-3 compliance)
- [ ] Set up GitHub Actions CI (Rust check + test + clippy, frontend build + lint)
- [ ] Enable Dependabot + `cargo audit` + `npm audit`
- [ ] Get code signing certificate (OV minimum)
- [ ] Build NSIS/WiX installer
- [ ] Create release workflow (tag → build → sign → GitHub Release)

### Post-MVP
- [ ] Port Tier 2 vectors (BIP-39, BIP-32, AES-GCM, hash functions)
- [ ] Add Vitest for frontend utilities
- [ ] Integrate WinSparkle for auto-updates
- [ ] Set up canary/stable release channels
- [ ] Add Google Test for C++ pure functions
- [ ] Evaluate repo restructure for ad blocker integration

### When Scaling
- [ ] Port Tier 3 vectors (BEEF, sighash, BUMP)
- [ ] Delta updates (component-based)
- [ ] E2E test harness
- [ ] macOS build pipeline + notarization
- [ ] `cargo-geiger` + Semgrep + CodeQL

---

## 9. UX/UI Sprint Test Plan (Phases 0–2)

This section catalogs the tests we want to build and run for the UX/UI sprint. Organized by phase and stack layer. Tests marked `[auto]` should be automated; tests marked `[manual]` require running the full browser.

---

### 9a. Phase 0 — Startup Flow & Wallet Checks

**What was built**: Rust server auto-launches from C++, `/wallet/status` endpoint, `POST /wallet/create`, frontend localStorage caching, Job Object process cleanup.

#### Rust Tests (`cargo test`)

| ID | Test | Type | Description |
|----|------|------|-------------|
| P0-R1 | `test_health_endpoint` | [auto] Integration | `GET /health` returns 200 with `"ok"` |
| P0-R2 | `test_wallet_status_no_wallet` | [auto] Integration | `GET /wallet/status` returns `{ exists: false }` on fresh DB |
| P0-R3 | `test_wallet_status_with_wallet` | [auto] Integration | After `POST /wallet/create`, `GET /wallet/status` returns `{ exists: true }` |
| P0-R4 | `test_wallet_create_success` | [auto] Integration | `POST /wallet/create` returns 200 with `mnemonic` (12 words), `address`, `walletId` |
| P0-R5 | `test_wallet_create_duplicate` | [auto] Integration | Second `POST /wallet/create` returns 409 Conflict |
| P0-R6 | `test_wallet_create_generates_valid_mnemonic` | [auto] Unit | Returned mnemonic is 12 words, all from BIP-39 wordlist |
| P0-R7 | `test_wallet_create_generates_address` | [auto] Unit | Returned address is valid P2PKH mainnet (starts with `1`) |

#### Manual Tests

| ID | Test | Steps |
|----|------|-------|
| P0-M1 | Server auto-starts | Launch browser → check Rust console appears and shows "listening on 3301" |
| P0-M2 | No-wallet state | Fresh install → open wallet panel → see "No Wallet Found" prompt (not spinner) |
| P0-M3 | Create wallet flow | Click "Create New Wallet" → see 12-word mnemonic → check "I have backed up" → click Continue → wallet panel shows balance |
| P0-M4 | localStorage caching | Close wallet panel → reopen → wallet panel appears instantly (no spinner, no /status fetch) |
| P0-M5 | Process cleanup | Open Task Manager → close browser → verify `hodos-wallet.exe` is no longer running |
| P0-M6 | Dev mode detection | Start `cargo run --release` manually → launch browser → verify browser skips process launch (logs "already running") |

---

### 9b. Phase 1 — Setup & Recovery

**What was built**: 1a: Mnemonic recovery + PIN, 1b: File backup/restore (JSON entity format), 1c: Centbee wallet sweep, Send Max fix.

#### Rust Tests (`cargo test`)

| ID | Test | Type | Description |
|----|------|------|-------------|
| P1-R1 | `test_wallet_recover_valid_mnemonic` | [auto] Integration | `POST /wallet/recover` with valid 12-word mnemonic creates wallet + scans addresses |
| P1-R2 | `test_wallet_recover_invalid_mnemonic` | [auto] Integration | `POST /wallet/recover` with invalid words returns 400 |
| P1-R3 | `test_wallet_recover_with_pin` | [auto] Integration | `POST /wallet/recover` with mnemonic + PIN encrypts mnemonic with PIN-derived key |
| P1-R4 | `test_wallet_export_requires_password` | [auto] Integration | `POST /wallet/export` without password returns 400 |
| P1-R5 | `test_wallet_export_format` | [auto] Integration | `POST /wallet/export` returns JSON with all 13 entity types in camelCase |
| P1-R6 | `test_wallet_import_roundtrip` | [auto] Integration | Export → import → verify all entities match (addresses, outputs, certificates, domain permissions) |
| P1-R7 | `test_wallet_import_wrong_password` | [auto] Integration | `POST /wallet/import` with wrong password returns 401/400 |
| P1-R8 | `test_wallet_import_corrupted_file` | [auto] Integration | `POST /wallet/import` with garbage data returns 400 |
| P1-R9 | `test_centbee_derivation_path` | [auto] Unit | BIP39 mnemonic + PIN passphrase → `m/44'/0/0/0` produces known address (use Centbee test vector) |
| P1-R10 | `test_centbee_change_path` | [auto] Unit | Same mnemonic → `m/44'/0/1/0` produces different known address |
| P1-R11 | `test_send_max_fee_calculation` | [auto] Unit | `estimate_transaction_size()` + `calculate_fee()` with known inputs produces expected fee; send-max leaves correct remainder |
| P1-R12 | `test_address_has_history` | [auto] Unit | Mock WoC `/history` response → function correctly identifies spent vs unspent addresses |

#### Frontend Tests (Vitest)

| ID | Test | Type | Description |
|----|------|------|-------------|
| P1-F1 | `test_mnemonic_word_count_validation` | [auto] Unit | Recovery form rejects <12 and >12 words |
| P1-F2 | `test_pin_length_validation` | [auto] Unit | PIN input rejects <4 digits for Centbee, enforces constraints |
| P1-F3 | `test_send_max_button` | [auto] Component | Send form "Max" button fills amount field with balance minus estimated fee |
| P1-F4 | `test_export_password_match` | [auto] Component | Export form disables download until passwords match and are 8+ chars |

#### Manual Tests

| ID | Test | Steps |
|----|------|-------|
| P1-M1 | Mnemonic recovery | Delete wallet.db → open wallet panel → Recover → enter 12 words → set PIN → verify wallet creates and scans blockchain |
| P1-M2 | Recovery scan progress | During recovery, wallet panel shows syncing banner with address/UTXO counts updating |
| P1-M3 | File export | Wallet overlay → Export Backup → enter password → verify .hodos-wallet file downloads |
| P1-M4 | File import | Delete wallet.db → open wallet panel → Recover → import .hodos-wallet file → enter password → verify wallet restores |
| P1-M5 | Export/import roundtrip | Export → delete wallet.db → import → verify balance, addresses, transaction history match |
| P1-M6 | Centbee recovery | Enter Centbee mnemonic + 4-digit PIN → verify addresses derived at `m/44'/0/0/{i}` → UTXOs swept to Hodos address |
| P1-M7 | Wrong Centbee PIN | Enter correct mnemonic + wrong PIN → addresses found but 0 balance → user sees warning |
| P1-M8 | Send Max | Have some BSV → click Send → click Max → verify amount fills to balance minus fee → send succeeds |

---

### 9c. Phase 2.0 — Price Cache Migration

**What was built**: `price_cache.rs` (CryptoCompare + CoinGecko fallback, 5-min TTL), `/wallet/balance` returns `bsvPrice`, frontend removed 3 external price fetchers.

#### Rust Tests (`cargo test`)

| ID | Test | Type | Description |
|----|------|------|-------------|
| P20-R1 | `test_price_cache_ttl` | [auto] Unit | After `get_price()`, `get_cached()` returns value; after TTL expires, returns `None` |
| P20-R2 | `test_price_cache_fallback` | [auto] Unit | Mock CryptoCompare failure → CoinGecko used → price returned |
| P20-R3 | `test_price_cache_both_fail` | [auto] Unit | Both APIs fail → `get_price()` returns 0.0 (or last cached) |
| P20-R4 | `test_balance_endpoint_includes_price` | [auto] Integration | `GET /wallet/balance` response contains `bsvPrice` field |
| P20-R5 | `test_bsv_price_endpoint` | [auto] Integration | `GET /wallet/bsv-price` returns `{ price: <number> }` |

#### Manual Tests

| ID | Test | Steps |
|----|------|-------|
| P20-M1 | USD display | Open wallet panel → verify USD value shown alongside BSV balance |
| P20-M2 | Price refresh | Wait 5+ minutes → refresh balance → verify price updates (check Rust logs for "fetching price") |

---

### 9d. Phase 2.1 — Domain Permissions DB + Repository

**What was built**: `domain_permissions` + `cert_field_permissions` tables, `DomainPermissionRepository` CRUD, 6 REST endpoints, backup integration.

#### Rust Tests (`cargo test`)

| ID | Test | Type | Description |
|----|------|------|-------------|
| P21-R1 | `test_domain_permission_upsert` | [auto] Integration | Insert new domain → returns ID; upsert same domain → updates, returns same ID |
| P21-R2 | `test_domain_permission_get_by_domain` | [auto] Integration | Insert → `get_by_domain()` returns matching record with correct fields |
| P21-R3 | `test_domain_permission_not_found` | [auto] Integration | `get_by_domain()` for unknown domain returns `Ok(None)` |
| P21-R4 | `test_domain_permission_list_all` | [auto] Integration | Insert 3 domains → `list_all()` returns all 3 sorted by domain |
| P21-R5 | `test_domain_permission_delete` | [auto] Integration | Insert → delete → `get_by_domain()` returns `None` |
| P21-R6 | `test_domain_permission_defaults` | [auto] Unit | `DomainPermission::defaults()` sets per_tx=10, per_session=300, rate=10 |
| P21-R7 | `test_cert_field_approve` | [auto] Integration | `approve_fields()` → `get_approved_fields()` returns them |
| P21-R8 | `test_cert_field_idempotent` | [auto] Integration | Approve same field twice → no error, no duplicate |
| P21-R9 | `test_cert_field_check_approved` | [auto] Integration | Approve 2 of 3 fields → `check_fields_approved()` returns (2 approved, 1 unapproved) |
| P21-R10 | `test_cert_field_revoke` | [auto] Integration | Approve → revoke → `get_approved_fields()` returns empty |
| P21-R11 | `test_domain_permission_cascade_delete` | [auto] Integration | Insert domain + cert fields → delete domain → cert fields also deleted (FK cascade) |
| P21-R12 | `test_get_domain_permission_endpoint` | [auto] Integration | `GET /domain/permissions?domain=X` returns JSON with correct shape |
| P21-R13 | `test_set_domain_permission_endpoint` | [auto] Integration | `POST /domain/permissions` creates/updates → verify with GET |
| P21-R14 | `test_delete_domain_permission_endpoint` | [auto] Integration | `DELETE /domain/permissions?domain=X` removes → GET returns `found: false` |
| P21-R15 | `test_list_domain_permissions_endpoint` | [auto] Integration | `GET /domain/permissions/all` returns `{ permissions: [...] }` |
| P21-R16 | `test_backup_includes_domain_permissions` | [auto] Integration | Export backup → JSON contains `domainPermissions` and `certFieldPermissions` arrays |

---

### 9e. Phase 2.2 — CR-2 Interceptor Refactor

**What was built**: `PendingRequestManager` singleton, `DomainPermissionCache`, `CefRefPtr` parent fix, removed UI-thread hop.

These are C++ changes — mostly manual testing.

#### Manual Tests

| ID | Test | Steps |
|----|------|-------|
| P22-M1 | Concurrent auth requests | Open 2 tabs to different BRC-100 sites → trigger auth on both → verify both get independent notifications (not clobbered) |
| P22-M2 | Request ID tracking | Approve one auth request → verify only that tab proceeds, other remains pending |
| P22-M3 | Handler lifetime | Navigate away from tab during pending auth → verify no crash (CefRefPtr prevents use-after-free) |
| P22-M4 | Domain cache | Approve domain → navigate to same site → verify no re-approval prompt (cache hit) |
| P22-M5 | Domain cache invalidation | Revoke domain in Approved Sites → navigate to site → verify approval prompt reappears |

---

### 9f. Phase 2.3 — Auto-Approve Engine + Notifications

**What was built**: Simplified 2-state trust model, auto-approve in C++ `Open()`, `SessionManager`, `BSVPriceCache` (C++), payment/rate-limit/certificate notifications, notification overlay keep-alive, recovery gap fix.

#### Rust Tests (`cargo test`)

| ID | Test | Type | Description |
|----|------|------|-------------|
| P23-R1 | `test_price_cache_get_cached_public` | [auto] Unit | `price_cache.get_cached()` is accessible and returns `Some(price)` after refresh |

#### Manual Tests

| ID | Test | Steps |
|----|------|-------|
| P23-M1 | Unknown domain → approval | Visit new BRC-100 site → notification overlay appears → click Approve → site proceeds |
| P23-M2 | Unknown domain → deny | Visit new BRC-100 site → click Deny → site gets error response |
| P23-M3 | Approved domain auto-approve | Approve domain with $0.10/tx limit → trigger $0.05 payment → auto-approved (no notification) |
| P23-M4 | Approved domain over limit | Approve domain with $0.10/tx limit → trigger $0.50 payment → payment confirmation notification appears |
| P23-M5 | Payment confirmation approve | Over-limit payment notification → click Approve → payment proceeds |
| P23-M6 | Payment confirmation deny | Over-limit payment notification → click Deny → payment fails, domain stays approved |
| P23-M7 | Modify limits from notification | Over-limit notification → click Modify Limits → change limits → Save → request auto-approved at new limits |
| P23-M8 | Rate limit notification | Approved domain → trigger 11+ requests in 1 minute → rate limit notification appears |
| P23-M9 | Session spending tracking | Approved domain ($3/session) → make multiple small payments → after cumulative $3+ → notification appears |
| P23-M10 | Session reset on tab close | Close tab → reopen site → session spending counter reset to $0 |
| P23-M11 | Notification overlay keep-alive | Trigger notification → dismiss → trigger another → verify instant appearance (no page reload flash) |
| P23-M12 | Keyboard input in notification | Trigger notification with form fields → verify keyboard input works (type in limit fields) |
| P23-M13 | Certificate disclosure | Visit site requesting proveCertificate → notification shows field list → approve → fields shared |
| P23-M14 | Certificate auto-approve | Approve cert fields → same site requests same fields again → auto-approved |
| P23-M15 | Recovery gap scanning | Recover wallet that has spent change addresses → verify scanner finds UTXOs past the gap (doesn't stop early) |

---

### 9g. DPAPI Auto-Unlock

**What was built**: `crypto/dpapi.rs` FFI, `mnemonic_dpapi BLOB` column, dual encryption, startup auto-unlock, DPAPI backfill on PIN unlock.

#### Rust Tests (`cargo test`)

| ID | Test | Type | Description |
|----|------|------|-------------|
| PDP-R1 | `test_dpapi_encrypt_decrypt_roundtrip` | [auto] Unit | `dpapi_encrypt(data)` → `dpapi_decrypt(blob)` returns original data (Windows only) |
| PDP-R2 | `test_dpapi_decrypt_wrong_user` | [auto] Unit | Encrypt on user A → decrypt on user B fails (if testable) |
| PDP-R3 | `test_wallet_status_locked` | [auto] Integration | Wallet exists + no cached mnemonic + no DPAPI blob → `/wallet/status` returns `locked: true` |
| PDP-R4 | `test_wallet_unlock_with_pin` | [auto] Integration | Locked wallet → `POST /wallet/unlock` with correct PIN → wallet unlocked + DPAPI blob stored |
| PDP-R5 | `test_wallet_unlock_wrong_pin` | [auto] Integration | `POST /wallet/unlock` with wrong PIN → 401 |

#### Manual Tests

| ID | Test | Steps |
|----|------|-------|
| PDP-M1 | Auto-unlock on startup | Create wallet with PIN → restart browser → wallet panel shows balance immediately (no PIN prompt) |
| PDP-M2 | DPAPI backfill | Legacy wallet (no DPAPI blob) → unlock with PIN → restart → auto-unlocks (DPAPI blob was stored on first unlock) |

---

### 9h. Phase 2.4 — Defense-in-Depth + Polish

**What was built**: `X-Requesting-Domain` header (C++→Rust), `check_domain_approved()` in Rust, spending limit check in `create_action`, `DomainPermissionsTab` UI, "Manage approved sites" link.

#### Rust Tests (`cargo test`)

| ID | Test | Type | Description |
|----|------|------|-------------|
| P24-R1 | `test_check_domain_approved_no_header` | [auto] Integration | Request without `X-Requesting-Domain` → returns `Ok(None)` (internal request allowed) |
| P24-R2 | `test_check_domain_approved_approved` | [auto] Integration | Insert approved domain → request with header → returns `Ok(Some(perm))` |
| P24-R3 | `test_check_domain_approved_unknown` | [auto] Integration | Request with header for unknown domain → returns `Err(403 Forbidden)` |
| P24-R4 | `test_check_domain_approved_unapproved` | [auto] Integration | Insert domain with trust_level="unknown" → request with header → returns `Err(403)` |
| P24-R5 | `test_create_action_spending_limit` | [auto] Integration | Approved domain with $0.10 limit → `createAction` with $0.50 output → 403 ERR_SPENDING_LIMIT_EXCEEDED |
| P24-R6 | `test_create_action_under_limit` | [auto] Integration | Approved domain with $1.00 limit → `createAction` with $0.50 output → proceeds normally |
| P24-R7 | `test_create_action_internal_no_check` | [auto] Integration | Internal `send_transaction` call (no header) → proceeds without domain check |
| P24-R8 | `test_well_known_auth_blocked` | [auto] Integration | Unapproved domain → `POST /.well-known/auth` with header → 403 |
| P24-R9 | `test_create_hmac_blocked` | [auto] Integration | Unapproved domain → `POST /createHmac` with header → 403 |
| P24-R10 | `test_create_signature_blocked` | [auto] Integration | Unapproved domain → `POST /createSignature` with header → 403 |

#### Frontend Tests (Vitest)

| ID | Test | Type | Description |
|----|------|------|-------------|
| P24-F1 | `test_domain_permissions_tab_empty` | [auto] Component | No permissions → shows "No sites have been approved yet" |
| P24-F2 | `test_domain_permissions_tab_list` | [auto] Component | Mock 3 permissions → table renders 3 rows with correct domain, limits, actions |
| P24-F3 | `test_domain_permissions_edit_modal` | [auto] Component | Click edit → DomainPermissionForm opens pre-filled with current settings |
| P24-F4 | `test_domain_permissions_revoke_confirm` | [auto] Component | Click revoke → confirmation dialog appears with domain name |
| P24-F5 | `test_wallet_overlay_tab_query_param` | [auto] Component | `?tab=4` in URL → Approved Sites tab selected on mount |

#### Manual Tests

| ID | Test | Steps |
|----|------|-------|
| P24-M1 | Approved Sites tab empty | Fresh wallet → wallet overlay → Approved Sites → "No sites" message |
| P24-M2 | Domain appears after approval | Approve a BRC-100 site → Approved Sites tab → domain listed with defaults |
| P24-M3 | Edit limits | Approved Sites → edit icon → change per-tx to $0.50 → Save → table updates |
| P24-M4 | Revoke domain | Approved Sites → delete icon → confirm → domain gone → revisit site → re-prompted |
| P24-M5 | Manage Sites link | Wallet panel → "Manage approved sites" → overlay opens on Approved Sites tab |
| P24-M6 | Rust safety net logs | Approved domain → trigger createAction → Rust logs show "defense-in-depth check passed" |
| P24-M7 | Internal ops unaffected | Send BSV from wallet panel → succeeds (no domain check, no header) |

---

### 9i. Test Infrastructure TODO

Before writing these tests, the following infrastructure is needed:

| Item | Stack | What |
|------|-------|------|
| `test_app_state()` helper | Rust | Creates `AppState` with `:memory:` SQLite, mock caches, user_id=1 |
| Vitest setup | Frontend | `npm install -D vitest @testing-library/react jsdom`, config in `vite.config.ts` |
| Mock fetch | Frontend | Utility to mock `fetch()` calls for Rust API endpoints |
| Test wallet fixture | Rust | Pre-populated `:memory:` DB with wallet, addresses, outputs for handler tests |
| Mock WoC/ARC | Rust | HTTP mock (e.g. `mockito` crate) for blockchain API calls in recovery tests |

**Priority order**: Rust `test_app_state` helper first (enables all P0-R through P24-R tests), then Vitest setup (enables frontend component tests), then mock HTTP for recovery/sync tests.

---

**End of Document**
