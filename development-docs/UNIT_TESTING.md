# Hodos Browser — Testing, CI/CD & Quality Strategy

**Created:** 2026-03-20
**Priority:** 🟡 Post-MVP (testing expansion) / 🔴 MVP (CI/CD pipeline — see BUILD_AND_RELEASE.md §5)
**Purpose:** Comprehensive testing strategy for long-term product quality and CI/CD integration

> **Relationship to BUILD_AND_RELEASE.md:** The CI/CD pipeline defined in BUILD_AND_RELEASE.md §5 runs the tests defined here. The `ci.yml` workflow runs on every PR. The `release.yml` workflow gates installer builds on passing tests. Security scanning (`cargo audit`, `npm audit`) runs in both workflows.

---

## 1. Testing Philosophy

### 1.1 What We Test (and Don't)

| Priority | What | Why |
|----------|------|-----|
| **P0** | Crypto correctness | Real money at stake. Incorrect signing = lost funds. |
| **P1** | Database migrations | Data integrity across upgrades |
| **P1** | API handlers | Contract stability |
| **P2** | Frontend utilities | Price formatting, validation |
| **P2** | React hooks | State management |
| **P3** | Components | UI rendering |
| **Skip** | CSS styling | Not worth automating |
| **Skip** | Full CEF integration | Manual test checklists |

### 1.2 Testing Pyramid

```
         ┌─────────┐
         │   E2E   │  ← Few (manual checklists for MVP)
        ┌┴─────────┴┐
        │Integration│  ← Some (handler + DB tests)
       ┌┴───────────┴┐
       │    Unit     │  ← Most (crypto, utils, pure functions)
       └─────────────┘
```

### 1.3 Test Location Conventions

| Stack | Location | Framework |
|-------|----------|-----------|
| Rust (unit) | `src/**/*_tests.rs` or inline `#[cfg(test)]` | `cargo test` |
| Rust (integration) | `tests/*.rs` | `cargo test` |
| Frontend | `src/**/__tests__/*.test.tsx` | Vitest |
| C++ | `tests/*.cpp` | Google Test (future) |

---

## 2. Rust Wallet Tests (HIGHEST PRIORITY)

The wallet handles real money. Crypto correctness is non-negotiable.

### 2.0 Existing Test Suite (Already Complete)

> **We already have 780+ integration tests** in `rust-wallet/tests/` across 12 tier files. These cover BRC-42/43 key derivation, HMAC-SHA256, AES-256-GCM (NIST vectors), BIP-39 (24 TREZOR vectors), BIP-32, ECDSA, BEEF serialization, sighash (500+ bitcoin-sv vectors), certificates, and more. See `rust-wallet/tests/CLAUDE.md` for the full inventory.
>
> The vectors below from `ts-sdk` are **additional cross-validation** targets. Many overlap with what's already tested. Before porting, check the existing suite to avoid duplicating work.

### 2.1 Additional Test Vectors from ts-sdk

Port test vectors from `bsv-blockchain/ts-sdk` to Rust `#[test]` functions. These are deterministic (hardcoded hex values), require no network access.

#### Tier 1 — Port First (Core Wallet Crypto)

**BRC-42 Private Key Derivation (5 vectors)**

Source: `src/primitives/__tests/BRC42.private.vectors.ts`

| # | Sender Public Key | Recipient Private Key | Invoice Number | Expected Derived Key |
|---|---|---|---|---|
| 1 | `033f9160df035156f1c48e75eae99914fa1a1546bec19781e8eddb900200bff9d1` | `6a1751169c111b4667a6539ee1be6b7cd9f6e9c8fe011a5f2fe31e03a15e0ede` | `f3WCaUmnN9U=` | `761656715bbfa172f8f9f58f5af95d9d0dfd69014cfdcacc9a245a10ff8893ef` |
| 2 | `027775fa43959548497eb510541ac34b01d5ee9ea768de74244a4a25f7b60fae8d` | `cab2500e206f31bc18a8af9d6f44f0b9a208c32d5cca2b22acfe9d1a213b2f36` | `2Ska++APzEc=` | `09f2b48bd75f4da6429ac70b5dce863d5ed2b350b6f2119af5626914bdb7c276` |
| 3 | `0338d2e0d12ba645578b0955026ee7554889ae4c530bd7a3b6f688233d763e169f` | `7a66d0896f2c4c2c9ac55670c71a9bc1bdbdfb4e8786ee5137cea1d0a05b6f20` | `cN/yQ7+k7pg=` | `7114cd9afd1eade02f76703cc976c241246a2f26f5c4b7a3a0150ecc745da9f0` |
| 4 | `02830212a32a47e68b98d477000bde08cb916f4d44ef49d47ccd4918d9aaabe9c8` | `6e8c3da5f2fb0306a88d6bcd427cbfba0b9c7f4c930c43122a973d620ffa3036` | `m2/QAsmwaA4=` | `f1d6fb05da1225feeddd1cf4100128afe09c3c1aadbffbd5c8bd10d329ef8f40` |
| 5 | `03f20a7e71c4b276753969e8b7e8b67e2dbafc3958d66ecba98dedc60a6615336d` | `e9d174eff5708a0a41b32624f9b9cc97ef08f8931ed188ee58d5390cad2bf68e` | `jgpUIjWFlVQ=` | `c5677c533f17c30f79a40744b18085632b262c0c13d87f3848c385f1389f79a6` |

**BRC-42 Public Key Derivation (5 vectors)**

Source: `src/primitives/__tests/BRC42.public.vectors.ts`

| # | Sender Private Key | Recipient Public Key | Invoice Number | Expected Derived Public Key |
|---|---|---|---|---|
| 1 | `583755110a8c059de5cd81b8a04e1be884c46083ade3f779c1e022f6f89da94c` | `02c0c1e1a1f7d247827d1bcf399f0ef2deef7695c322fd91a01a91378f101b6ffc` | `IBioA4D/OaE=` | `03c1bf5baadee39721ae8c9882b3cf324f0bf3b9eb3fc1b8af8089ca7a7c2e669f` |
| 2 | `2c378b43d887d72200639890c11d79e8f22728d032a5733ba3d7be623d1bb118` | `039a9da906ecb8ced5c87971e9c2e7c921e66ad450fd4fc0a7d569fdb5bede8e0f` | `PWYuo9PDKvI=` | `0398cdf4b56a3b2e106224ff3be5253afd5b72de735d647831be51c713c9077848` |
| 3 | `d5a5f70b373ce164998dff7ecd93260d7e80356d3d10abf928fb267f0a6c7be6` | `02745623f4e5de046b6ab59ce837efa1a959a8f28286ce9154a4781ec033b85029` | `X9pnS+bByrM=` | `0273eec9380c1a11c5a905e86c2d036e70cbefd8991d9a0cfca671f5e0bbea4a3c` |
| 4 | `46cd68165fd5d12d2d6519b02feb3f4d9c083109de1bfaa2b5c4836ba717523c` | `031e18bb0bbd3162b886007c55214c3c952bb2ae6c33dd06f57d891a60976003b1` | `+ktmYRHv3uQ=` | `034c5c6bf2e52e8de8b2eb75883090ed7d1db234270907f1b0d1c2de1ddee5005d` |
| 5 | `7c98b8abd7967485cfb7437f9c56dd1e48ceb21a4085b8cdeb2a647f62012db4` | `03c8885f1e1ab4facd0f3272bb7a48b003d2e608e1619fb38b8be69336ab828f37` | `PPfDTTcl1ao=` | `03304b41cfa726096ffd9d8907fe0835f888869eda9653bca34eb7bcab870d3779` |

**HMAC-SHA256 (5 vectors)**

Source: `src/primitives/__tests/HMAC.test.ts`

| # | Key (hex) | Message | Expected HMAC (hex) |
|---|---|---|---|
| 1 | `000102030405...3e3f` (64 bytes) | `"Sample message for keylen=blocklen"` | `8bb9a1db9806f20df7f77b82138c7914d174d59e13dc4d0169c9057b133e1d62` |
| 2 | `000102030405...1e1f` (32 bytes) | `"Sample message for keylen<blocklen"` | `a28cf43130ee696a98f14a37678b56bcfcbdd9e5cf69717fecf5480f0ebdf790` |
| 3 | `000102030405...6263` (100 bytes) | `"Sample message for keylen=blocklen"` | `bdccb6c72ddeadb500ae768386cb38cc41c63dbb0878ddb9c7a38a431b78378d` |
| 4 | `000102030405...2e2f30` (49 bytes) | `"Sample message for keylen<blocklen, with truncated tag"` | `27a8b157839efeac98df070b331d593618ddb985d403c0c786d23b5d132e57c7` |
| 5 | `48f38d0c6a344959cc94502b7b5e8dffb6a5f41795d9066fc9a649557167ee2f` | `1d495eef7761b65dccd0a983d2d7204fea28b5c81f1758046e062eb043755ea1` (hex) | `cf5ad5984f9e43917aa9087380dac46e410ddc8a7731859c84e9d0f31bd43655` |

Vector 5 is most relevant — raw hex bytes matching BRC-42 internal HMAC usage.

**BRC-3 Signature Compliance (1 vector)**

Source: `src/wallet/__tests/ProtoWallet.test.ts`

- Private key: `6a2991c9de20e38b31d7ea147bf55f5039e4bbc073160f5e0d541d1f17e321b8`
- Data: `"BRC-3 Compliance Validated!"` (UTF-8)
- Protocol: `[2, 'BRC3 Test']`, KeyID: `'42'`
- Counterparty: `0294c479f762f6baa97fbcd4393564c1d7bd8336ebd15928135bbcf575cd1a71a1`
- Expected DER signature: `[48, 68, 2, 32, 43, 34, 58, 156, ...]` (full bytes in ts-sdk)

Tests full pipeline: BRC-42 derive key + ECDSA sign.

**BRC-2 HMAC Compliance (1 vector)**

- Private key: `6a2991c9de20e38b31d7ea147bf55f5039e4bbc073160f5e0d541d1f17e321b8`
- Data: `"BRC-2 HMAC Compliance Validated!"` (UTF-8)
- Protocol: `[2, 'BRC2 Test']`, KeyID: `'42'`
- Counterparty: `0294c479f762f6baa97fbcd4393564c1d7bd8336ebd15928135bbcf575cd1a71a1`
- Expected HMAC: `[81, 240, 18, 153, 163, 45, 174, 85, ...]`

#### Tier 2 — Port Soon (Key Infrastructure)

**BIP-39 Mnemonic Vectors (24 vectors)**

Source: `src/compat/__tests/Mnemonic.vectors.ts`

All 24 standard BIP-39 vectors with passphrase `"TREZOR"`:

| Entropy | First Words | Expected Seed (first 32 hex) |
|---------|-------------|------------------------------|
| `00000000000000000000000000000000` | `abandon abandon abandon...about` | `c55257c360c07c72029aebc1b53c05ed` |
| `ffffffffffffffffffffffffffffffff` | `zoo zoo zoo zoo...wrong` | `ac27495480225222079d7be181583751` |

Tests `recovery.rs` mnemonic-to-seed derivation.

**BIP-32 HD Wallet Derivation**

Source: `src/compat/__tests/HD.test.ts`

Seed: `000102030405060708090a0b0c0d0e0f`
Master xprv: `xprv9s21ZrQH143K3QTDL4LXw2F7HEK3wJUD2nW2nRk4stbPy6cq3jPPqjiChkVvvNKmPGJxWUtg6LnF5kejMRNNU3TGtRBeJgk33yuGBxrMPHi`

Tests `recovery::derive_private_key_bip32()`.

**AES-256-GCM Vectors (NIST)**

Source: `src/primitives/__tests/AESGCM.test.ts`

| Test | Key | IV | Plaintext | Expected Ciphertext | Expected Tag |
|------|-----|----|-----------|--------------------|--------------|
| GCM-15 | `feffe992...` | `cafebabe...` | `d9313225...` | `522dc1f0...` | `b094dac5d93471bdec1a502270e3cc6c` |

Tests `crypto/aesgcm_custom.rs`.

#### Tier 3 — Port When Practical

- **BEEF Serialization** — `src/transaction/__tests/Beef.test.ts`
- **Sighash Vectors** — `src/script/__tests/sighashTestData.ts` (500+ vectors, port ~20)
- **BUMP / Merkle Path** — `src/transaction/__tests/bump.valid.vectors.ts`

### 2.2 Example Rust Test

```rust
// rust-wallet/tests/brc42_vectors_test.rs

use rust_wallet::crypto::brc42::derive_child_private_key;

#[test]
fn test_derive_child_private_key_vector_1() {
    let sender_pubkey = hex::decode(
        "033f9160df035156f1c48e75eae99914fa1a1546bec19781e8eddb900200bff9d1"
    ).unwrap();
    let recipient_privkey = hex::decode(
        "6a1751169c111b4667a6539ee1be6b7cd9f6e9c8fe011a5f2fe31e03a15e0ede"
    ).unwrap();
    let invoice = "f3WCaUmnN9U=";

    let derived = derive_child_private_key(&recipient_privkey, &sender_pubkey, invoice)
        .expect("derivation should succeed");

    assert_eq!(
        hex::encode(&derived),
        "761656715bbfa172f8f9f58f5af95d9d0dfd69014cfdcacc9a245a10ff8893ef"
    );
}
```

### 2.3 Test Priority by Module

| Priority | Module | What to Test |
|----------|--------|--------------|
| **P0** | `crypto/brc42.rs` | Key derivation against ts-sdk vectors |
| **P0** | `crypto/signing.rs` | HMAC-SHA256, SHA-256 against NIST |
| **P0** | `crypto/aesgcm_custom.rs` | AES-256-GCM encrypt/decrypt |
| **P0** | `handlers.rs` (well_known_auth) | BRC-104 nonce handling |
| **P1** | `recovery.rs` | BIP-39, BIP-32 derivation |
| **P1** | `beef.rs` | BEEF parse/serialize |
| **P1** | `transaction/sighash.rs` | ForkID SIGHASH |
| **P1** | `database/migrations.rs` | V1→V2→V3→V4 upgrades |
| **P2** | `database/*_repo.rs` | CRUD operations |
| **P2** | `handlers.rs` | Request validation, responses |
| **P3** | `monitor/` | Background task logic |
| **P3** | `price_cache.rs` | TTL, fallback |

### 2.4 Test Infrastructure Needed

| Item | What |
|------|------|
| `test_app_state()` helper | Creates `AppState` with `:memory:` SQLite, mock caches |
| Test wallet fixture | Pre-populated DB with wallet, addresses, outputs |
| Mock HTTP | `mockito` crate for WoC/ARC calls |

---

## 3. Frontend Tests (Vitest)

### 3.1 Setup

```bash
npm install -D vitest @testing-library/react @testing-library/jest-dom jsdom
```

```typescript
// vite.config.ts
export default defineConfig({
  test: {
    environment: 'jsdom',
    setupFiles: ['./tests/setup.ts'],
  },
});
```

### 3.2 Test Priority

| Priority | What | Test Type |
|----------|------|-----------|
| **P1** | Utility functions (price formatting, satoshi/USD) | Unit |
| **P1** | Input validation (send form, PIN, mnemonic) | Unit |
| **P2** | React hooks (`useBalance`, `useBackgroundBalancePoller`) | Unit (mock fetch) |
| **P2** | `DomainPermissionForm` validation | Component |
| **P3** | Notification overlay state | Component |
| **Skip** | Full page rendering | Manual tests |
| **Skip** | CSS styling | Not worth it |

### 3.3 Example Tests

```typescript
// src/utils/__tests__/formatters.test.ts
import { describe, it, expect } from 'vitest';
import { formatSatoshis, formatUSD } from '../formatters';

describe('formatSatoshis', () => {
  it('formats whole BSV', () => {
    expect(formatSatoshis(100_000_000)).toBe('1.00000000');
  });
  
  it('formats fractional', () => {
    expect(formatSatoshis(123_456_789)).toBe('1.23456789');
  });
  
  it('handles zero', () => {
    expect(formatSatoshis(0)).toBe('0.00000000');
  });
});

describe('formatUSD', () => {
  it('formats with 2 decimals', () => {
    expect(formatUSD(1234.567)).toBe('$1,234.57');
  });
});
```

```typescript
// src/components/__tests__/SendForm.test.tsx
import { render, screen, fireEvent } from '@testing-library/react';
import { SendForm } from '../SendForm';

describe('SendForm', () => {
  it('validates address format', () => {
    render(<SendForm balance={100000} />);
    
    const addressInput = screen.getByLabelText('Address');
    fireEvent.change(addressInput, { target: { value: 'invalid' } });
    
    expect(screen.getByText('Invalid BSV address')).toBeInTheDocument();
  });
  
  it('Max button fills balance minus fee', () => {
    render(<SendForm balance={100000} estimatedFee={226} />);
    
    fireEvent.click(screen.getByText('Max'));
    
    const amountInput = screen.getByLabelText('Amount');
    expect(amountInput).toHaveValue('99774'); // 100000 - 226
  });
});
```

---

## 4. Adblock Engine Tests

### 4.1 Existing Tests (DONE ✅)

| Test | Status |
|------|--------|
| `test_basic_ad_blocking` | ✅ |
| `test_exception_rules` | ✅ |
| `test_resource_type_filtering` | ✅ |
| `test_engine_new_starts_in_loading_state` | ✅ |
| `test_enable_disable_toggle` | ✅ |
| `test_check_request_returns_false_when_disabled` | ✅ |
| `test_check_request_returns_false_when_engine_not_loaded` | ✅ |
| `test_engine_serialize_deserialize` | ✅ |
| `test_status_defaults` | ✅ |

### 4.2 Additional Coverage (Post-MVP)

- [ ] Entity-aware blocking (disconnect.me entity map)
- [ ] Cosmetic filter generation
- [ ] `$generichide` exception handling
- [ ] Filter list parsing edge cases

---

## 5. C++ Tests (Future)

### 5.1 When to Add

C++ testing is complex with CEF. Add when:
- Pure C++ functions extracted (URL parsing, JSON manipulation)
- SessionManager logic testable without CEF
- DomainPermissionCache testable with mock HTTP

### 5.2 Framework

**Google Test** — industry standard for C++

```cpp
// tests/session_manager_test.cpp
#include <gtest/gtest.h>
#include "SessionManager.h"

TEST(SessionManager, TracksSpendingPerSession) {
    SessionManager mgr;
    mgr.RecordSpend("example.com", 1000); // 1000 satoshis
    mgr.RecordSpend("example.com", 500);
    
    EXPECT_EQ(mgr.GetSessionSpending("example.com"), 1500);
}

TEST(SessionManager, ResetsOnTabClose) {
    SessionManager mgr;
    mgr.RecordSpend("example.com", 1000);
    mgr.ResetSession("example.com");
    
    EXPECT_EQ(mgr.GetSessionSpending("example.com"), 0);
}
```

---

## 6. Manual Test Checklists

For MVP, these replace automated E2E tests.

### 6.1 Wallet Creation & Recovery

- [ ] Fresh install → "No Wallet Found" prompt
- [ ] Create wallet → 12-word mnemonic displayed
- [ ] Mnemonic recovery → wallet restores, addresses scan
- [ ] File export → .hodos-wallet file downloads
- [ ] File import → wallet restores with all data
- [ ] Centbee recovery → m/44'/0/0/x derivation correct

### 6.2 Transactions

- [ ] Send BSV → transaction broadcasts, balance updates
- [ ] Send Max → correct amount (balance - fee)
- [ ] Receive → address displayed, QR code works

### 6.3 BRC-100 Auth

- [ ] Unknown domain → approval notification
- [ ] Approve → domain added, site proceeds
- [ ] Deny → site gets error
- [ ] Over-limit payment → confirmation notification
- [ ] Rate limit → notification after 10+ requests/minute

### 6.4 Domain Permissions

- [ ] Approved Sites tab shows all approved domains
- [ ] Edit limits → changes saved
- [ ] Revoke → domain removed, re-prompted on visit

### 6.5 Ad Blocking

- [ ] Ad-heavy site → ads blocked (check debug log)
- [ ] Toggle disabled → ads load
- [ ] Entity-aware → same-org CDNs not blocked

---

## 7. CI/CD Pipeline Integration

> **The authoritative CI/CD workflow files are defined in BUILD_AND_RELEASE.md §5.** This section describes how testing plugs into that pipeline.

### 7.0 Integration with Build & Release

| Pipeline Stage | Tests Run | Gate Behavior |
|----------------|-----------|---------------|
| **PR / push to main** (`ci.yml`) | `cargo test` (wallet + adblock), `npm run build`, `npm run lint`, `cargo audit`, `npm audit` | PR cannot merge if any fail |
| **Release** (`release.yml`) | Full `cargo test` suite | Installer builds blocked until tests pass |
| **Post-MVP** | + Vitest frontend tests, + coverage thresholds, + `cargo-deny` license check | Progressive tightening |

***Needs a decision:*** Should we enforce a minimum code coverage threshold for crypto modules (P0)? Recommendation: 90%+ for `src/crypto/` as a CI gate, no threshold for other modules initially.

### 7.1 Test Jobs

```yaml
# .github/workflows/ci.yml
jobs:
  rust-test:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo test --manifest-path rust-wallet/Cargo.toml
      - run: cargo test --manifest-path adblock-engine/Cargo.toml

  frontend-test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
      - run: npm ci
        working-directory: frontend
      - run: npm test -- --run
        working-directory: frontend
```

### 7.2 Coverage Reporting (Future)

```yaml
  rust-coverage:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo install cargo-tarpaulin
      - run: cargo tarpaulin --manifest-path rust-wallet/Cargo.toml --out Html
      - uses: actions/upload-artifact@v4
        with:
          name: coverage-report
          path: tarpaulin-report.html
```

### 7.3 Security Scanning

| Tool | What | When |
|------|------|------|
| `cargo audit` | Rust CVEs | Every CI run |
| `npm audit` | npm CVEs | Every CI run |
| Dependabot | Automated update PRs | Weekly |
| `cargo-deny` | License + advisory | Pre-release |
| `cargo-geiger` | Unsafe code audit | Pre-release |

---

## 8. Local Test Runner

```powershell
# scripts/test-all.ps1
param([switch]$Coverage)

Write-Host "=== RUST WALLET ===" -ForegroundColor Cyan
cargo test --manifest-path rust-wallet/Cargo.toml

Write-Host "`n=== ADBLOCK ENGINE ===" -ForegroundColor Cyan
cargo test --manifest-path adblock-engine/Cargo.toml

Write-Host "`n=== FRONTEND ===" -ForegroundColor Cyan
Push-Location frontend
npm test -- --run
Pop-Location

Write-Host "`n=== DONE ===" -ForegroundColor Green
```

---

## Appendix: ts-sdk Vector Source Files

For complete test vectors, reference these files in `bsv-blockchain/ts-sdk`:

| File | Vectors |
|------|---------|
| `src/primitives/__tests/BRC42.private.vectors.ts` | 5 private key derivation |
| `src/primitives/__tests/BRC42.public.vectors.ts` | 5 public key derivation |
| `src/primitives/__tests/HMAC.test.ts` | 5 HMAC-SHA256 |
| `src/primitives/__tests/AESGCM.test.ts` | NIST AES-GCM |
| `src/compat/__tests/Mnemonic.vectors.ts` | 24 BIP-39 |
| `src/compat/__tests/HD.test.ts` | BIP-32 derivation |
| `src/transaction/__tests/Beef.test.ts` | BEEF serialization |
| `src/script/__tests/sighashTestData.ts` | 500+ sighash |
| `src/wallet/__tests/ProtoWallet.test.ts` | BRC-2, BRC-3 compliance |
