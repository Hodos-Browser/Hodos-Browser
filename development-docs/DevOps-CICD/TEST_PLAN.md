# Hodos Browser — Detailed Test Plan & Catalog

> ## ⚠️ INHERITED / RECONCILED DOCUMENT — READ THIS FIRST
>
> - **Origin:** This file was inherited as `development-docs/UNIT_TESTING.md`, created **2026-03-20**. It was **largely unverified and aspirational** — the prior owner did not trust its claims, and there is **no evidence the tests it describes were ever run as a suite.**
> - **Reconciled against source:** 2026-06-16. Test counts were re-censused directly from the source tree (see §2.0). The old doc's headline "780+ tests" was **inflated/wrong** — the real numbers are in §2.0.
> - **What this doc IS:** the detailed **PLAN / CATALOG** — test vectors to port, example tests, blueprints, and **manual QA checklists**. The vectors, example tests, and checklists here are **PROPOSALS to implement and run — NOT evidence that any test passes.**
> - **"Exists in source" ≠ "passes."** Where a test file is confirmed to exist on disk, that is stated. **Pass-status is NOT verified anywhere in this doc** (no recorded suite run). Treat every "complete / DONE ✅" from the original as **pass-status NOT verified — needs a real run.**
> - **Canonical STRATEGY lives elsewhere:** the authoritative cross-stack testing *strategy* (philosophy, pyramid, CI gating, coverage, secret-log gate, live-e2e harness) is **`DevOps-CICD/TESTING.md`**. This doc is the detailed plan/catalog that the strategy points to.

---

## 1. Testing Philosophy & Conventions

**Moved.** Philosophy, the testing pyramid, and test-location conventions are now owned by **`DevOps-CICD/TESTING.md` (§1–§2).** See TESTING.md for the canonical strategy. This doc carries only the detailed catalog below.

---

## 2. Rust Wallet Tests (HIGHEST PRIORITY)

The wallet handles real money. Crypto correctness is non-negotiable.

### 2.0 Verified Test Census (reconciled 2026-06-16)

> ⚠️ **The original doc claimed "780+ integration tests" / "Already Complete". That figure was FALSE.** It conflated `check!` *assertions* with test *functions*. Below is the census counted directly from source. **Existence is verified; pass-status is NOT** — no recorded run of the full suite exists, so "passes" must be established by an actual `cargo test` / `ctest` / `playwright test` run.

| Stack | What was counted | Verified count | Notes |
|-------|------------------|----------------|-------|
| **Rust — inline** | `#[test]` / `#[tokio::test]` in `rust-wallet/src/**` | **424** | Heaviest in `permission_service/` (~107) and `crypto/`, `services/`, `database/` |
| **Rust — integration** | `#[test]` in `rust-wallet/tests/*.rs` (top-level, excl. `fixtures/node_modules`) | **67** | 14 files; `sdk_interop_test.rs` (9), `tier6/tier7` (10 ea.). The "~535 sighash" etc. in `tests/CLAUDE.md` are `check!` assertions inside single `#[test]` fns, not separate tests |
| **Rust — `check!` assertions** | `check!(` calls in `rust-wallet/tests` | **697** | This is what "780+" was loosely pointing at — assertions, not tests. Still not "780+", and still not verified to pass |
| **Adblock** | `#[test]` in `adblock-engine/` | **23** | All in `adblock-engine/src/engine.rs`. (Original doc listed 9 + a TODO list and never gave the real number.) |
| **C++** | `TEST(...)` / `TEST_F(...)` in `cef-native/tests/*.cpp` | **39** | `manifest_fetcher_test.cpp` (13) + `sensitive_cert_fields_test.cpp` (26). ⚠️ The `permission_engine_test.cpp` (claimed "25 tests") and `session_manager_test.cpp` referenced in §5 / `cef-native/tests/CLAUDE.md` **NO LONGER EXIST** — deleted in Phase 2.6-H when the C++ PermissionEngine/SessionManager were removed. |
| **Frontend — Vitest** | vitest config / dep / `*.test.ts(x)` | **0** | ✅ Verified: `frontend/package.json` has **no** `vitest` / `@testing-library` deps; `npm test` runs **Playwright**. The entire "Vitest" story (§3) is a blueprint, not implemented. |
| **Frontend — Playwright** | `test(` in `frontend/e2e/tests/*.spec.ts` | **54** | ⚠️ Original doc claimed **73**. Real count is 54 across 6 spec files (see §3A). Pass-status not verified. |

**Bottom line:** ~**491** Rust `#[test]` fns + **23** adblock + **39** C++ + **54** Playwright = the real surface. **0** Vitest. None of these have a recorded pass run on file — *they exist in source; that is all that is verified.*

### 2.1 Additional Test Vectors from ts-sdk — TO PORT (unverified)

> ⚠️ **Status: PROPOSED to port.** These vectors are *targets*, not implemented Rust tests. **Before porting, check the existing suite** (`rust-wallet/tests/diagnostic_test.rs` already claims BRC-42 / HMAC / AES-GCM / BIP-39 / BIP-32 / ECDSA / BRC-2 / BRC-3 coverage) to avoid duplication.
>
> **Source-file existence:** ✅ verified present at `rust-wallet/tests/fixtures/node_modules/@bsv/sdk/src/primitives/__tests/BRC42.private.vectors.ts` (the ts-sdk is vendored under the test `fixtures/`). The specific per-file paths in the Appendix were spot-checked for the BRC42 private vectors; the rest are **⚠️ UNVERIFIED individually** (assumed to follow the same `@bsv/sdk/src/...` layout).

These are deterministic (hardcoded hex), require no network.

#### Tier 1 — Port First (Core Wallet Crypto)

**BRC-42 Private Key Derivation (5 vectors)** — Source: `src/primitives/__tests/BRC42.private.vectors.ts`

| # | Sender Public Key | Recipient Private Key | Invoice Number | Expected Derived Key |
|---|---|---|---|---|
| 1 | `033f9160df035156f1c48e75eae99914fa1a1546bec19781e8eddb900200bff9d1` | `6a1751169c111b4667a6539ee1be6b7cd9f6e9c8fe011a5f2fe31e03a15e0ede` | `f3WCaUmnN9U=` | `761656715bbfa172f8f9f58f5af95d9d0dfd69014cfdcacc9a245a10ff8893ef` |
| 2 | `027775fa43959548497eb510541ac34b01d5ee9ea768de74244a4a25f7b60fae8d` | `cab2500e206f31bc18a8af9d6f44f0b9a208c32d5cca2b22acfe9d1a213b2f36` | `2Ska++APzEc=` | `09f2b48bd75f4da6429ac70b5dce863d5ed2b350b6f2119af5626914bdb7c276` |
| 3 | `0338d2e0d12ba645578b0955026ee7554889ae4c530bd7a3b6f688233d763e169f` | `7a66d0896f2c4c2c9ac55670c71a9bc1bdbdfb4e8786ee5137cea1d0a05b6f20` | `cN/yQ7+k7pg=` | `7114cd9afd1eade02f76703cc976c241246a2f26f5c4b7a3a0150ecc745da9f0` |
| 4 | `02830212a32a47e68b98d477000bde08cb916f4d44ef49d47ccd4918d9aaabe9c8` | `6e8c3da5f2fb0306a88d6bcd427cbfba0b9c7f4c930c43122a973d620ffa3036` | `m2/QAsmwaA4=` | `f1d6fb05da1225feeddd1cf4100128afe09c3c1aadbffbd5c8bd10d329ef8f40` |
| 5 | `03f20a7e71c4b276753969e8b7e8b67e2dbafc3958d66ecba98dedc60a6615336d` | `e9d174eff5708a0a41b32624f9b9cc97ef08f8931ed188ee58d5390cad2bf68e` | `jgpUIjWFlVQ=` | `c5677c533f17c30f79a40744b18085632b262c0c13d87f3848c385f1389f79a6` |

**BRC-42 Public Key Derivation (5 vectors)** — Source: `src/primitives/__tests/BRC42.public.vectors.ts`

| # | Sender Private Key | Recipient Public Key | Invoice Number | Expected Derived Public Key |
|---|---|---|---|---|
| 1 | `583755110a8c059de5cd81b8a04e1be884c46083ade3f779c1e022f6f89da94c` | `02c0c1e1a1f7d247827d1bcf399f0ef2deef7695c322fd91a01a91378f101b6ffc` | `IBioA4D/OaE=` | `03c1bf5baadee39721ae8c9882b3cf324f0bf3b9eb3fc1b8af8089ca7a7c2e669f` |
| 2 | `2c378b43d887d72200639890c11d79e8f22728d032a5733ba3d7be623d1bb118` | `039a9da906ecb8ced5c87971e9c2e7c921e66ad450fd4fc0a7d569fdb5bede8e0f` | `PWYuo9PDKvI=` | `0398cdf4b56a3b2e106224ff3be5253afd5b72de735d647831be51c713c9077848` |
| 3 | `d5a5f70b373ce164998dff7ecd93260d7e80356d3d10abf928fb267f0a6c7be6` | `02745623f4e5de046b6ab59ce837efa1a959a8f28286ce9154a4781ec033b85029` | `X9pnS+bByrM=` | `0273eec9380c1a11c5a905e86c2d036e70cbefd8991d9a0cfca671f5e0bbea4a3c` |
| 4 | `46cd68165fd5d12d2d6519b02feb3f4d9c083109de1bfaa2b5c4836ba717523c` | `031e18bb0bbd3162b886007c55214c3c952bb2ae6c33dd06f57d891a60976003b1` | `+ktmYRHv3uQ=` | `034c5c6bf2e52e8de8b2eb75883090ed7d1db234270907f1b0d1c2de1ddee5005d` |
| 5 | `7c98b8abd7967485cfb7437f9c56dd1e48ceb21a4085b8cdeb2a647f62012db4` | `03c8885f1e1ab4facd0f3272bb7a48b003d2e608e1619fb38b8be69336ab828f37` | `PPfDTTcl1ao=` | `03304b41cfa726096ffd9d8907fe0835f888869eda9653bca34eb7bcab870d3779` |

**HMAC-SHA256 (5 vectors)** — Source: `src/primitives/__tests/HMAC.test.ts`

| # | Key (hex) | Message | Expected HMAC (hex) |
|---|---|---|---|
| 1 | `000102030405...3e3f` (64 bytes) | `"Sample message for keylen=blocklen"` | `8bb9a1db9806f20df7f77b82138c7914d174d59e13dc4d0169c9057b133e1d62` |
| 2 | `000102030405...1e1f` (32 bytes) | `"Sample message for keylen<blocklen"` | `a28cf43130ee696a98f14a37678b56bcfcbdd9e5cf69717fecf5480f0ebdf790` |
| 3 | `000102030405...6263` (100 bytes) | `"Sample message for keylen=blocklen"` | `bdccb6c72ddeadb500ae768386cb38cc41c63dbb0878ddb9c7a38a431b78378d` |
| 4 | `000102030405...2e2f30` (49 bytes) | `"Sample message for keylen<blocklen, with truncated tag"` | `27a8b157839efeac98df070b331d593618ddb985d403c0c786d23b5d132e57c7` |
| 5 | `48f38d0c6a344959cc94502b7b5e8dffb6a5f41795d9066fc9a649557167ee2f` | `1d495eef7761b65dccd0a983d2d7204fea28b5c81f1758046e062eb043755ea1` (hex) | `cf5ad5984f9e43917aa9087380dac46e410ddc8a7731859c84e9d0f31bd43655` |

Vector 5 is most relevant — raw hex bytes matching BRC-42 internal HMAC usage.

**BRC-3 Signature Compliance (1 vector)** — Source: `src/wallet/__tests/ProtoWallet.test.ts`
- Private key: `6a2991c9de20e38b31d7ea147bf55f5039e4bbc073160f5e0d541d1f17e321b8`
- Data: `"BRC-3 Compliance Validated!"` (UTF-8); Protocol: `[2, 'BRC3 Test']`, KeyID: `'42'`
- Counterparty: `0294c479f762f6baa97fbcd4393564c1d7bd8336ebd15928135bbcf575cd1a71a1`
- Expected DER signature: `[48, 68, 2, 32, 43, 34, 58, 156, ...]` (full bytes in ts-sdk). Tests full pipeline: BRC-42 derive + ECDSA sign.

**BRC-2 HMAC Compliance (1 vector)**
- Private key: `6a2991c9de20e38b31d7ea147bf55f5039e4bbc073160f5e0d541d1f17e321b8`
- Data: `"BRC-2 HMAC Compliance Validated!"` (UTF-8); Protocol: `[2, 'BRC2 Test']`, KeyID: `'42'`
- Counterparty: `0294c479f762f6baa97fbcd4393564c1d7bd8336ebd15928135bbcf575cd1a71a1`
- Expected HMAC: `[81, 240, 18, 153, 163, 45, 174, 85, ...]`

#### Tier 2 — Port Soon (Key Infrastructure)

**BIP-39 Mnemonic Vectors (24 vectors)** — Source: `src/compat/__tests/Mnemonic.vectors.ts` (passphrase `"TREZOR"`)

| Entropy | First Words | Expected Seed (first 32 hex) |
|---------|-------------|------------------------------|
| `00000000000000000000000000000000` | `abandon abandon abandon...about` | `c55257c360c07c72029aebc1b53c05ed` |
| `ffffffffffffffffffffffffffffffff` | `zoo zoo zoo zoo...wrong` | `ac27495480225222079d7be181583751` |

Tests `recovery.rs` mnemonic-to-seed.

**BIP-32 HD Wallet Derivation** — Source: `src/compat/__tests/HD.test.ts`
- Seed: `000102030405060708090a0b0c0d0e0f`
- Master xprv: `xprv9s21ZrQH143K3QTDL4LXw2F7HEK3wJUD2nW2nRk4stbPy6cq3jPPqjiChkVvvNKmPGJxWUtg6LnF5kejMRNNU3TGtRBeJgk33yuGBxrMPHi`
- Tests `recovery::derive_private_key_bip32()`.

**AES-256-GCM Vectors (NIST)** — Source: `src/primitives/__tests/AESGCM.test.ts`

| Test | Key | IV | Plaintext | Expected Ciphertext | Expected Tag |
|------|-----|----|-----------|--------------------|--------------|
| GCM-15 | `feffe992...` | `cafebabe...` | `d9313225...` | `522dc1f0...` | `b094dac5d93471bdec1a502270e3cc6c` |

Tests `crypto/aesgcm_custom.rs`.

#### Tier 3 — Port When Practical
- **BEEF Serialization** — `src/transaction/__tests/Beef.test.ts`
- **Sighash Vectors** — `src/script/__tests/sighashTestData.ts` (500+ vectors, port ~20)
- **BUMP / Merkle Path** — `src/transaction/__tests/bump.valid.vectors.ts`

### 2.2 Example Rust Test (template — verify against current `crypto::brc42` API before use)

> ⚠️ The `derive_child_private_key` signature below is from the 2026-03 doc and is **UNVERIFIED** against current `rust-wallet/src/crypto/brc42.rs`. Confirm the function name/arity before copying.

```rust
// rust-wallet/tests/brc42_vectors_test.rs
use rust_wallet::crypto::brc42::derive_child_private_key;

#[test]
fn test_derive_child_private_key_vector_1() {
    let sender_pubkey = hex::decode(
        "033f9160df035156f1c48e75eae99914fa1a1546bec19781e8eddb900200bff9d1").unwrap();
    let recipient_privkey = hex::decode(
        "6a1751169c111b4667a6539ee1be6b7cd9f6e9c8fe011a5f2fe31e03a15e0ede").unwrap();
    let invoice = "f3WCaUmnN9U=";
    let derived = derive_child_private_key(&recipient_privkey, &sender_pubkey, invoice)
        .expect("derivation should succeed");
    assert_eq!(hex::encode(&derived),
        "761656715bbfa172f8f9f58f5af95d9d0dfd69014cfdcacc9a245a10ff8893ef");
}
```

### 2.3 Test Priority by Module (proposed)

| Priority | Module | What to Test |
|----------|--------|--------------|
| **P0** | `crypto/brc42.rs` | Key derivation against ts-sdk vectors |
| **P0** | `crypto/signing.rs` | HMAC-SHA256, SHA-256 against NIST |
| **P0** | `crypto/aesgcm_custom.rs` | AES-256-GCM encrypt/decrypt |
| **P0** | `handlers.rs` (well_known_auth) | BRC-104 nonce handling |
| **P1** | `recovery.rs` | BIP-39, BIP-32 derivation |
| **P1** | `beef.rs` | BEEF parse/serialize |
| **P1** | `transaction/sighash.rs` | ForkID SIGHASH |
| **P1** | `database/migrations.rs` | V1→…→VN upgrades |
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

## 3. Frontend Tests (Vitest) — BLUEPRINT, NOT IMPLEMENTED

> ⚠️ **There are ZERO Vitest tests today.** ✅ Verified: `frontend/package.json` has no `vitest` or `@testing-library` dependency, and `npm test` runs Playwright (§3A). Everything in this section is a **blueprint to build**, not a description of existing tests. PIPE-A7 / TESTING.md §12 has the open decision to build this thin layer.

### 3.1 Setup (proposed)

```bash
npm install -D vitest @testing-library/react @testing-library/jest-dom jsdom
```
```typescript
// vite.config.ts
export default defineConfig({
  test: { environment: 'jsdom', setupFiles: ['./tests/setup.ts'] },
});
```

### 3.2 Test Priority (proposed)

| Priority | What | Test Type |
|----------|------|-----------|
| **P1** | Utility functions (price formatting, satoshi/USD) | Unit |
| **P1** | Input validation (send form, PIN, mnemonic) | Unit |
| **P2** | React hooks (`useBalance`, `useBackgroundBalancePoller`) | Unit (mock fetch) |
| **P2** | `DomainPermissionForm` validation | Component |
| **P3** | Notification overlay state | Component |
| **Skip** | Full page rendering | Manual tests |

### 3.3 Example Tests (templates — modules below are unverified)

```typescript
// src/utils/__tests__/formatters.test.ts
import { describe, it, expect } from 'vitest';
import { formatSatoshis, formatUSD } from '../formatters';

describe('formatSatoshis', () => {
  it('formats whole BSV', () => { expect(formatSatoshis(100_000_000)).toBe('1.00000000'); });
  it('formats fractional', () => { expect(formatSatoshis(123_456_789)).toBe('1.23456789'); });
  it('handles zero', () => { expect(formatSatoshis(0)).toBe('0.00000000'); });
});
describe('formatUSD', () => {
  it('formats with 2 decimals', () => { expect(formatUSD(1234.567)).toBe('$1,234.57'); });
});
```
```typescript
// src/components/__tests__/SendForm.test.tsx
import { render, screen, fireEvent } from '@testing-library/react';
import { SendForm } from '../SendForm';

describe('SendForm', () => {
  it('validates address format', () => {
    render(<SendForm balance={100000} />);
    fireEvent.change(screen.getByLabelText('Address'), { target: { value: 'invalid' } });
    expect(screen.getByText('Invalid BSV address')).toBeInTheDocument();
  });
  it('Max button fills balance minus fee', () => {
    render(<SendForm balance={100000} estimatedFee={226} />);
    fireEvent.click(screen.getByText('Max'));
    expect(screen.getByLabelText('Amount')).toHaveValue('99774');
  });
});
```

---

## 3A. Playwright E2E Smoke Tests (exists in source — pass-status NOT verified)

> **Added:** 2026-03-27 via PR #82 by John Calhoun.
> **Status (reconciled 2026-06-16):** ✅ **6 spec files exist** in `frontend/e2e/tests/`; `@playwright/test` is a dev dependency; `npm test` runs them. ⚠️ The original "73 tests passing on macOS" is **TWO claims, both unverified here**: the real `test(` count is **54**, and **no passing run is on record** (pass-status needs a real `playwright test` run, Windows + macOS).

### What These Are

Playwright launches real Chromium and tests our React frontend routes/components/UI structure — **smoke tests** (elements exist & render), not full transaction flows. Because Playwright runs in plain Chromium (not the CEF shell), a **CEF bridge mock** (`frontend/e2e/helpers/bridge-mock.ts`) stubs `window.hodosBrowser` so components render.

### Test Coverage (file counts verified; "passing" not verified)

| Spec File | What It Tests | `test(` count (verified) |
|-----------|---------------|--------------------------|
| `smoke.spec.ts` | Routes load without JS errors (data-driven loop over a route list) | 1 |
| `wallet-panel.spec.ts` | Balance area, send/receive buttons, transaction form | 8 |
| `wallet-dashboard.spec.ts` | 4-quadrant layout, sidebar tabs, tab switching | 10 |
| `wallet-activity.spec.ts` | Filter buttons, empty state, transaction list | 4 |
| `settings-page.spec.ts` | Settings sections render, privacy toggles | 11 |
| `cross-cutting.spec.ts` | Menu items, downloads, privacy shield, new tab | 20 |
| **Total** | | **54** |

> Note: the original doc's per-file numbers (20/9/10/4/10/20 = 73) do not match source. `smoke.spec.ts` uses a single `test(` over a route array rather than one `test(` per route, which partly explains the gap.

### How to Run

```bash
cd frontend
npm test           # run all Playwright tests headless
npm run test:ui    # interactive UI mode
```
Requires the frontend dev server on `:5137` (`npm run dev`).

### What These Do NOT Test
- Real BSV transactions (needs funded wallet + Rust backend)
- BRC-100 auth flows (needs CEF + live BRC-100 site)
- CEF overlay lifecycle (open/close/focus)
- Settings persistence (needs Rust backend)
- Certificate operations

### Future: Integrating with Unit Tests
1. Keep Playwright for route/UI smoke regressions.
2. Add Vitest unit tests (§3) for pure logic.
3. Expand Playwright against the real Rust backend for integration flows (requires starting Rust in CI).
4. CI: run `npm test` (Playwright) alongside Vitest in the `frontend-test` job. The bridge mock doubles as documentation of React↔CEF IPC contracts — keep it current.

---

## 4. Adblock Engine Tests (exist in source — pass-status NOT verified)

> ✅ **Verified:** 23 `#[test]` functions in `adblock-engine/src/engine.rs`. ⚠️ The original doc's "DONE ✅" per-test markers are **pass-status NOT verified** — no recorded run.

### 4.1 Existing Tests (exist in `engine.rs` — needs a real run to confirm green)

| Test | Status |
|------|--------|
| `test_basic_ad_blocking` | exists; pass-status unverified |
| `test_exception_rules` | exists; pass-status unverified |
| `test_resource_type_filtering` | exists; pass-status unverified |
| `test_engine_new_starts_in_loading_state` | exists; pass-status unverified |
| `test_enable_disable_toggle` | exists; pass-status unverified |
| `test_check_request_returns_false_when_disabled` | exists; pass-status unverified |
| `test_check_request_returns_false_when_engine_not_loaded` | exists; pass-status unverified |
| `test_engine_serialize_deserialize` | exists; pass-status unverified |
| `test_status_defaults` | exists; pass-status unverified |

> The 9 above are the named subset from the original doc; the file actually contains **23** `#[test]` fns. Enumerate the full set on the next pass.

### 4.2 Additional Coverage (Post-MVP, proposed)
- [ ] Entity-aware blocking (disconnect.me entity map)
- [ ] Cosmetic filter generation
- [ ] `$generichide` exception handling
- [ ] Filter list parsing edge cases

---

## 5. C++ Tests (exist in source — pass-status NOT verified)

> ✅ **Verified:** 2 GoogleTest files in `cef-native/tests/` totalling **39** `TEST`/`TEST_F` cases: `manifest_fetcher_test.cpp` (13) + `sensitive_cert_fields_test.cpp` (26). Built via CMake FetchContent GoogleTest 1.14.
>
> ⚠️ **STALE in the original doc:** the example below was `session_manager_test.cpp`, and `cef-native/tests/CLAUDE.md` references `permission_engine_test.cpp` ("25 tests"). **Both of those files NO LONGER EXIST** — the C++ `SessionManager` and `PermissionEngine` were deleted in **Phase 2.6-H** (commits `1d7de47`, `f02cf91`) when that logic moved to Rust. The SessionManager example is kept below only as a GoogleTest *style* reference, **not** as a description of any current test.

### 5.1 When to Add
Add C++ tests when pure C++ logic is extractable from CEF (URL parsing, JSON manipulation, manifest parsing, cert-field classification — the latter two are what the two existing files cover).

### 5.2 Framework
**Google Test** (CMake FetchContent, pinned `GIT_TAG` — see TESTING.md §11 for the hermetic-CI note).

```cpp
// ⚠️ STYLE REFERENCE ONLY — this SessionManager was deleted in Phase 2.6-H. Not a live test.
#include <gtest/gtest.h>
#include "SessionManager.h"

TEST(SessionManager, TracksSpendingPerSession) {
    SessionManager mgr;
    mgr.RecordSpend("example.com", 1000);
    mgr.RecordSpend("example.com", 500);
    EXPECT_EQ(mgr.GetSessionSpending("example.com"), 1500);
}
```

---

## 6. Manual Test Checklists

> These are **manual QA procedures** — inherently "to run", not automated and not verified. Keep them; run them by hand during Standard/Thorough verification (TESTING.md §10 / root CLAUDE.md Testing Standards).

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

**Moved.** CI/CD integration strategy is owned by **`DevOps-CICD/TESTING.md` §4** and **`DevOps-CICD/BUILD_AND_RELEASE.md` §5**. The drafts below are retained only as starting material.

> ⚠️ **REFERENCE DRAFT for PIPE-CI — NOT LIVE.** There is **no `ci.yml` today** (confirmed: `release.yml` runs no tests; see `0.4.0/SPRINT_0_4_0_MASTER_PLAN.md` §3, PIPE-CI / PIPE-TESTGATE). The canonical workflow *shape* lives in TESTING.md §4 / BUILD_AND_RELEASE.md §5. The "PR cannot merge if tests fail" gate described here **does not exist** — it is a proposal.

### 7.1 Test Jobs (reference draft — not live)

```yaml
# ⚠️ REFERENCE DRAFT — .github/workflows/ci.yml does NOT exist. Canonical shape: TESTING.md §4.
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

### 7.2 Coverage Reporting (reference draft — not live, Future)

```yaml
  rust-coverage:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo install cargo-tarpaulin
      - run: cargo tarpaulin --manifest-path rust-wallet/Cargo.toml --out Html
      - uses: actions/upload-artifact@v4
        with: { name: coverage-report, path: tarpaulin-report.html }
```

### 7.3 Security Scanning (proposed)

| Tool | What | When |
|------|------|------|
| `cargo audit` | Rust CVEs | Every CI run |
| `npm audit` | npm CVEs | Every CI run |
| Dependabot | Automated update PRs | Weekly |
| `cargo-deny` | License + advisory | Pre-release |
| `cargo-geiger` | Unsafe code audit | Pre-release |

***Open decision (see TESTING.md §12):*** minimum coverage threshold for crypto (P0) modules? Recommendation in the original doc: 90%+ for `src/crypto/` as a CI gate.

---

## 8. Local Test Runner

> ⚠️ **REFERENCE DRAFT — `scripts/test-all.ps1` is a sketch, not committed.** Note `npm test -- --run` assumes Vitest; with the current **Playwright** `npm test`, drop `--run`.

```powershell
# scripts/test-all.ps1 (reference sketch)
param([switch]$Coverage)
Write-Host "=== RUST WALLET ===" -ForegroundColor Cyan
cargo test --manifest-path rust-wallet/Cargo.toml
Write-Host "`n=== ADBLOCK ENGINE ===" -ForegroundColor Cyan
cargo test --manifest-path adblock-engine/Cargo.toml
Write-Host "`n=== FRONTEND ===" -ForegroundColor Cyan
Push-Location frontend; npm test; Pop-Location
Write-Host "`n=== DONE ===" -ForegroundColor Green
```

---

## Appendix: ts-sdk Vector Source Files

> ✅ Vendored under `rust-wallet/tests/fixtures/node_modules/@bsv/sdk/src/...`. The BRC42 private-vectors path is verified present; the remaining paths are **⚠️ assumed (UNVERIFIED individually)** to follow the same layout.

| File | Vectors |
|------|---------|
| `src/primitives/__tests/BRC42.private.vectors.ts` | 5 private key derivation ✅ exists |
| `src/primitives/__tests/BRC42.public.vectors.ts` | 5 public key derivation ⚠️ |
| `src/primitives/__tests/HMAC.test.ts` | 5 HMAC-SHA256 ⚠️ |
| `src/primitives/__tests/AESGCM.test.ts` | NIST AES-GCM ⚠️ |
| `src/compat/__tests/Mnemonic.vectors.ts` | 24 BIP-39 ⚠️ |
| `src/compat/__tests/HD.test.ts` | BIP-32 derivation ⚠️ |
| `src/transaction/__tests/Beef.test.ts` | BEEF serialization ⚠️ |
| `src/script/__tests/sighashTestData.ts` | 500+ sighash ⚠️ |
| `src/wallet/__tests/ProtoWallet.test.ts` | BRC-2, BRC-3 compliance ⚠️ |

---

## See Also
- **`DevOps-CICD/TESTING.md`** — canonical cross-stack testing STRATEGY (philosophy, pyramid, CI gating, coverage, secret-log gate, live-e2e harness). This catalog implements that strategy.
- **`DevOps-CICD/BUILD_AND_RELEASE.md` §5** — build/release pipeline that the tests plug into.
- **`rust-wallet/tests/CLAUDE.md`** — Rust integration-test inventory (note: its "780+" is `check!` assertions, not test functions).
- **`cef-native/tests/CLAUDE.md`** — C++ test inventory (note: now stale re: deleted `permission_engine_test.cpp`).
- **`0.4.0/SPRINT_0_4_0_MASTER_PLAN.md` §3** — PIPE-CI / PIPE-TESTGATE (the missing CI work).
