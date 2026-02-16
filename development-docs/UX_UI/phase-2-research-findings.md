# Phase 2 Research Findings: Domain Permissions & User Notifications

**Date:** 2026-02-15
**Status:** Design Decisions Made -- Implementation In Progress

---

## Executive Summary

Phase 2 adds a permission/guard rail system controlling what happens when websites make BRC-100 requests (payments, certificates, identity) to the Hodos wallet. This document compiles research from 5 parallel investigations covering BSV specs, industry standards, current codebase analysis, and database planning.

**Key takeaways:**
1. BSV specs define security levels (0/1/2) but leave spending limits and UI to the wallet implementer
2. Industry consensus: 4-tier trust model, time-limited sessions, rate limiting, separate connection from authorization
3. CR-2 (Interceptor Architecture) is a hard prerequisite -- current single-global request handler can't support concurrent notifications
4. Domain permissions should be added directly to the V1 migration (single wallet, recoverable)

---

## 1. BSV SDK Permission Model

### BRC-43 Security Levels (0, 1, 2)

| Level | Permission Semantics | Prompt Strategy | Cache |
|-------|---------------------|-----------------|-------|
| **0** | Open access -- no permission needed | Never prompt | N/A |
| **1** | Protocol-wide -- grant once per protocol per app | Prompt once, remember | Indefinite or with expiry |
| **2** | Counterparty-specific -- grant per (protocol, counterparty) | Prompt for each new peer | Per counterparty |

**Invoice number format:** `<securityLevel>-<protocolID>-<keyID>`

**Reserved namespaces:** `admin*` (BRC-44, wallet internal use), `p ` prefix (BRC-98, future permissioned schemes) -- must reject apps requesting these.

### BRC-73 Group Permissions (App Manifest)

Apps can declare all permission needs upfront via `manifest.json`:

| Category | What It Controls |
|----------|-----------------|
| **protocolPermissions** | Key derivation for signing/encryption/HMAC |
| **spendingAuthorization** | Total spending budget (amount in sats + duration in seconds) |
| **basketAccess** | Insert/spend from UTXO baskets |
| **certificateAccess** | Reveal specific certificate fields to specific verifiers |

**Flow:** App calls `waitForAuthentication` -> wallet fetches manifest -> shows grouped dialog -> user accepts/denies per category -> grants cached with spending duration as expiry.

**Important:** Privileged operations CANNOT be in group permissions -- always require one-off requests with `privilegedReason`.

### BRC-52 Certificates

- Certificate types are **open-ended** -- any certifier can define any type via a 256-bit type ID
- Risk differentiation is by: type ID, certifier identity, fields requested, selective disclosure
- **CertMap** npm package resolves type IDs to human-readable metadata (name, icon, description)
- Wallet must show: certificate type name, certifier identity, specific fields being requested
- `certifierUrl` implements a 2-step protocol: `/initialRequest` -> `/signCertificate` (BRC-53)

### BRC-105 HTTP Payments

- Server returns HTTP 402 with `x-bsv-payment-satoshis-required` header
- Client wallet constructs transaction and resends with `x-bsv-payment` header
- **No auto-approve thresholds specified** -- wallet-implementation-specific

### Wallet-Toolbox Reference Implementation

The `WalletPermissionsManager` wraps the wallet and intercepts all calls:
- **Spending authorization:** Per-app monthly budget from manifest, tracked via `querySpentSince()`
- **Auto-approve conditions:** Admin originator (unrestricted), config flags, 5-minute permission cache TTL, 15-second recent-grant grace period, security level 0 (always allowed)
- **On-chain permission tokens:** PushDrop-based tokens in admin baskets that persist across sessions

**Key insight:** `createAction` itself does NOT check permissions. Permission enforcement is entirely in the wrapper layer that intercepts before the call reaches the wallet. Same pattern applies to Hodos -- enforcement at the C++ interceptor / Rust handler level.

---

## 2. Industry Standards

### Common Trust Tier Model

Every major wallet implements some version of this:

| Tier | Behavior | Used By |
|------|----------|---------|
| **Blocked** | All requests rejected silently | MetaMask, Brave, Phantom, Trust |
| **Unknown** (default) | Every request prompts | Universal |
| **Connected** | Can see addresses; transactions still prompt | MetaMask, Brave, Phantom |
| **Trusted** | Auto-approve below threshold; prompt above | Phantom (auto-confirm), Rabby |

**No reputable wallet has "fully trusted"** -- even Phantom's auto-confirm has 2-hour expiry, rate limits, and curated app lists.

### Phantom Auto-Confirm (Best Reference)

- User enables per-app, per-tab
- **2-hour time limit** -- auto-deactivates
- **10 tx/min rate limit**
- **Tab-scoped** -- only the specific tab where enabled
- **Background simulation** -- all auto-confirmed txs simulated; suspicious activity stops auto-confirm
- **Curated allowlist** -- only vetted apps (Magic Eden, Jupiter, Tensor, etc.)
- **Default off** -- explicit opt-in

### Coinbase Spend Permissions

- On-chain ERC-712 signatures: spender, token, amount, time period
- Session keys with time-bounded permissions
- Sub Accounts for hierarchical ownership
- Agentic wallets: session caps, per-tx limits, KYT screening

### MetaMask Model

- **EIP-2255:** `wallet_requestPermissions` / `wallet_getPermissions` / `wallet_revokePermissions`
- Connection does NOT authorize spending -- separate approval for every transaction
- **Spending caps** for token approvals (user-customizable)
- **Blockaid integration:** Pre-sign transaction simulation, risk scoring, phishing detection
- **ERC-7715:** Session keys with scoped, pre-approved permissions

### Spending Limit Patterns

| Type | Description | Who |
|------|-------------|-----|
| Per-transaction cap | Max per single tx | MetaMask, Coinbase |
| Per-session cap | Max total during session | Coinbase, Phantom (via rate limit) |
| Per-day cap | Max total in 24 hours | Coinbase, exchange wallets |
| Per-domain cap | Max a specific site can spend | Rabby |
| Auto-approve threshold | Below X, no popup | Phantom (curated apps only) |

### Anti-Patterns to Avoid

1. **Infinite/unlimited approvals by default** -- $2.7B lost to approval phishing
2. **Blind signing** -- showing hex instead of human-readable details
3. **Single permission for everything** -- permissions must be granular and scoped
4. **No native approval management** -- users shouldn't need third-party tools
5. **Permissions surviving wallet reset** -- reset must clear all domain permissions
6. **Approval fatigue** -- constant prompts cause users to blindly click approve

### Approval Fatigue Solutions

- Reduce prompt frequency through smart defaults and auto-approve for verified low-risk operations
- Make dangerous prompts **visually different** from routine ones (different color, layout, warning icons)
- Show **outcome-focused language** ("You will send 1,000 sats") not action-focused ("Sign this transaction?")
- **Disable confirm button briefly** (1-2s) for high-risk operations
- Batch approvals where possible

---

## 3. Current Codebase Analysis

### HTTP Interceptor Flow (C++)

```
Web page makes request
  -> HttpRequestInterceptor::GetResourceHandler() (IO thread)
    -> isWalletEndpoint(url)?
    -> extractDomain() from main frame or referrer
    -> Create AsyncWalletResourceHandler
      -> Open() checks DomainVerifier::isDomainWhitelisted()
        -> NOT whitelisted: trigger approval modal (60s timeout)
        -> Whitelisted: forward to localhost:3301
  -> Response delivered back to web page
```

### Current Problems (Must Fix for Phase 2)

| Issue | Impact | Fix |
|-------|--------|-----|
| **Single global `g_pendingAuthRequest`** | Concurrent requests clobber each other | CR-2.2: Per-request map |
| **File I/O on every request** | Whitelist read from disk on IO thread | CR-2.4: In-memory cache |
| **No thread synchronization** | Data race on globals from IO + UI threads | CR-2.3: Mutex |
| **Modal deduplication buggy** | Second request from same domain silently dropped | CR-2.2 fixes |
| **POST /domain/whitelist/add missing** | Endpoint doesn't exist in Rust handlers | Phase 2 implements |
| **Binary permission model** | Whitelisted or not -- no scopes/limits | Phase 2 replaces |
| **Well-known auth bypasses checks** | Localhost auth redirected before domain verification | Phase 2 fixes |
| **Wallet HTTP calls on UI thread** | Slow wallet freezes entire browser | CR-2.1: Move to IO thread |

### CR-2 Prerequisites (4-5 day effort)

| Item | Description | Effort |
|------|-------------|--------|
| CR-2.1 | Move wallet HTTP calls off UI thread | 2-3 days (7 call sites) |
| CR-2.2 | Replace `g_pendingAuthRequest` with per-request map | 4-6 hrs |
| CR-2.3 | Add mutex on global state | 1-2 hrs |
| CR-2.4 | Cache whitelist in memory | 4-6 hrs |
| CR-2.5 | Fix thread race in request completion | 3-4 hrs |
| CR-2.6 | Fix raw pointer in AsyncHTTPClient | 1 hr |

### Rust Backend -- No Permission Checks

- `well_known_auth()` accepts BRC-104 auth from ANY caller -- no domain checking
- `create_action()` has no permission scope validation
- Defense-in-depth: should add domain permission check in Rust handlers too

### Existing Domain Whitelist (Rust)

`domain_whitelist.rs` has `DomainWhitelistManager` with:
- In-memory `Arc<Mutex<HashMap>>` + JSON file persistence
- `DomainWhitelistEntry`: domain, added_at, last_used, request_count, is_permanent
- One-time entry logic: returns false if `!is_permanent && request_count > 0`
- **No spending limits, no certificate levels, no per-operation scopes**

---

## 4. Existing Phase 2 Design Decisions (from docs)

From `phase-2-user-notifications.md` (2026-02-11):

### Escalation Levels (Already Decided)

| Level | Interruption | When |
|-------|-------------|------|
| **Quiet** | None | Routine from trusted sites, public key, low-risk |
| **Notification** | Minimal (dismissible) | First-time site, routine from new site |
| **Modal** | Requires action | Certificates/PII, payments, identity, high-risk |

### MVP Auto-Approve Rules (Already Decided)

- Trusted domains auto-approve below configurable sat threshold (per-tx and per-day)
- Certificate read operations auto-approved for trusted domains
- Everything else prompts
- Auto-approve must NEVER apply to: master identity key, certificate issuance, transactions above threshold, unknown domains
- Auto-approve settings require PIN to modify

### No-Wallet HTTP Intercept (Already Decided)

If an HTTP request to the wallet is intercepted but no wallet exists, trigger create/recover modal automatically.

---

## 5. Database Schema Plan

### Add to V1 Migration (Not V2/V3)

Since we have one wallet and can recover it, add `domain_permissions` directly to `create_schema_v1()` in `migrations.rs`.

### Proposed Table

```sql
CREATE TABLE IF NOT EXISTS domain_permissions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL,
    domain TEXT NOT NULL,
    trust_level TEXT NOT NULL DEFAULT 'unknown',
    -- Spending limits (satoshis)
    per_tx_limit INTEGER NOT NULL DEFAULT 1000,
    per_day_limit INTEGER NOT NULL DEFAULT 10000,
    daily_spent INTEGER NOT NULL DEFAULT 0,
    daily_reset_at INTEGER NOT NULL DEFAULT 0,
    -- Session
    session_expiry INTEGER,
    -- Certificate auto-approve
    cert_auto_approve INTEGER NOT NULL DEFAULT 0,
    -- Metadata
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    decided_at INTEGER,
    FOREIGN KEY (user_id) REFERENCES users(userId),
    UNIQUE(user_id, domain)
);

CREATE INDEX IF NOT EXISTS idx_domain_permissions_domain
    ON domain_permissions(domain);
```

### Trust Levels

| Value | Meaning |
|-------|---------|
| `blocked` | All requests rejected silently |
| `unknown` | Every request prompts (default for new domains) |
| `connected` | Can request public key without prompt; everything else prompts |
| `trusted` | Auto-approve below per_tx_limit; prompt above |

### Backup/Export Changes

Need to add to `backup.rs`:
- New `BackupDomainPermission` serde struct
- Add `domain_permissions` field to `BackupPayload`
- Add SELECT query in `collect_payload()`
- Add INSERT in `import_to_db()` (after users, before settings -- FK dependency)
- Include in log stats

### New Repository

Create `rust-wallet/src/database/domain_permission_repo.rs` with:
- `get_by_domain(user_id, domain)` -> `Option<DomainPermission>`
- `upsert(user_id, domain, trust_level, limits...)` -> id
- `update_daily_spent(id, amount)` -> track cumulative spending
- `reset_daily_if_expired(id)` -> auto-reset at midnight
- `list_all(user_id)` -> `Vec<DomainPermission>`
- `delete(id)` -> remove domain

---

## 6. Open Design Decisions

### Decision 1: Manifest-Based vs Per-Operation Permissions

**Option A (MVP):** Per-operation prompts -- prompt when each request type is first seen from a domain. Simpler to build, doesn't require fetching manifests.

**Option B (SDK-aligned):** Fetch `manifest.json` from the app on first BRC-100 request, show grouped permission dialog upfront. More work but matches wallet-toolbox pattern.

**Recommendation:** Start with **Option A** for MVP. Add manifest support in Phase 4 when building the full wallet settings UI.

### Decision 2: Where to Enforce Permissions

**C++ layer only:** Current pattern -- interceptor gates requests before they reach Rust. Fast, but single layer.

**C++ + Rust (defense-in-depth):** Both layers check. C++ handles UI/prompting, Rust validates permissions as a second check. More robust against bypass.

**Recommendation:** Both layers. C++ handles prompting/caching. Rust validates that the domain has the required permission before executing.

### Decision 3: Spending Limit Defaults

~~Based on satoshi amounts.~~ **UPDATED 2026-02-15:** Spending limits are now **USD-based** (see Section 7, Decision 3). The backend fetches the exchange rate and converts at evaluation time. Defaults:

| Limit | Proposed Default | Rationale |
|-------|-----------------|-----------|
| Per-tx auto-approve | $0.05 USD | Covers routine BRC-105 micropayments |
| Per-day auto-approve | $0.50 USD | Rolling 24h cumulative cap |
| Rate limit | 10 requests/min | Matches Phantom, prevents drain |

### Decision 4: Session Expiry for "Trusted" Domains

~~**Recommendation:** Option C -- default 24-hour trusted session.~~

**UPDATED 2026-02-15:** Trust is **permanent by default** once the user grants it. No auto-expiry. Users can manually revoke trust per-domain in settings. Rate limiting (10 tx/min) prevents abuse. This avoids the annoyance of re-approving trusted apps.

### Decision 5: Advanced Settings in First-Visit Notification

User requested an "Advanced Settings" button in the initial notification allowing per-domain customization on first visit.

**Proposed flow:**
1. First BRC-100 request from new domain -> notification appears
2. Quick options: "Allow" (Connected) / "Block" / "Dismiss"
3. "Advanced Settings" expands to show: trust level selector, spending limits, certificate auto-approve toggle
4. Settings saved to `domain_permissions` table

### Decision 6: CR-2 Timing

**Option A:** Complete CR-2 fully before starting Phase 2 code (sequential).

**Option B:** Do CR-2 and Phase 2 together -- build the per-request map as part of the new notification system.

**Recommendation:** **Option B** -- interleave them. CR-2.2 (per-request map) naturally emerges when building the notification flow. CR-2.1 (off-thread wallet calls) should be done first as a standalone improvement.

---

## 7. Design Decisions (2026-02-15)

### Decision 1: BRC-43 Security Levels → Deferred
**Not implementing for MVP.** BRC-43 security levels (0/1/2) add complexity and are not critical for the initial permission system. Defer to Phase 4 (full wallet settings) when we add BRC-73 manifest support.

### Decision 2: No 2-Hour Auto-Confirm
**Rejected.** Phantom's 2-hour expiry was considered but deemed annoying for users. Trust stays indefinitely once granted by the user. Users can manually revoke trust per-domain in settings. Rate limiting (10 tx/min) provides protection against runaway apps.

### Decision 3: USD-Based Spending Limits
**Spending limits defined in USD, not satoshis.** BSV's price volatility and micropayment use cases make fixed satoshi thresholds impractical — what's pennies today could be dollars tomorrow. Backend needs the exchange rate to evaluate limits.

| Limit | Default | User-Adjustable |
|-------|---------|----------------|
| Per-tx auto-approve | $0.05 USD | Yes, up to $5.00 |
| Per-day auto-approve | $0.50 USD | Yes, up to $50.00 |
| Rate limit | 10 tx/min | Yes, per-domain |

### Decision 4: Exchange Rate Moved to Backend
**All BSV/USD price fetching centralized in Rust backend** via a new `price_cache.rs` module. Frontend removes all CryptoCompare/CoinGecko calls and gets price from `/wallet/balance` response. This:
- Eliminates 3 redundant price fetchers in frontend
- Enables Rust handlers to evaluate USD-based spending limits
- Single source of truth for exchange rate across all components

### Decision 5: Permission Check Architecture
- **C++ interceptor**: Checks domain trust level (string lookup, fast path)
- **Rust handler**: Checks spending amounts (requires parsing createAction body + exchange rate)
- Defense-in-depth: both layers check, but C++ handles UI/prompting

### Decision 6: CR-2 Interleaved with Phase 2
CR-2.2 (per-request map) built as part of the notification system. CR-2.1 (off-thread wallet calls) done first as standalone.

---

## 8. Implementation Sub-Phases

### Sub-Phase 2.0: Price Cache Migration (Current)
Move BSV/USD exchange rate from frontend to backend.

| Step | Description | Files |
|------|-------------|-------|
| 2.0.1 | Create `price_cache.rs` (CryptoCompare + CoinGecko fallback, 5-min TTL) | `rust-wallet/src/price_cache.rs` |
| 2.0.2 | Add `PriceCache` to `AppState` | `rust-wallet/src/main.rs` |
| 2.0.3 | Include `bsv_usd_price` in `/wallet/balance` response | `rust-wallet/src/handlers.rs` |
| 2.0.4 | Remove frontend price fetching (3 locations + localStorage) | `frontend/src/hooks/useBalance.ts`, `useBackgroundBalancePoller.ts`, `TransactionForm.tsx`, `balanceCache.ts` |
| 2.0.5 | Frontend reads price from balance response | Same frontend files |

### Sub-Phase 2.1: Domain Permissions DB + Repository
| Step | Description |
|------|-------------|
| 2.1.1 | Add `domain_permissions` table to V1 migration |
| 2.1.2 | Create `DomainPermissionRepository` with CRUD ops |
| 2.1.3 | Add REST endpoints: `GET /domain/permissions/check`, `POST /domain/permissions/set` |
| 2.1.4 | Update backup.rs to include domain_permissions in export/import |

### Sub-Phase 2.2: CR-2 Interceptor Refactor
| Step | Description |
|------|-------------|
| 2.2.1 | CR-2.1: Move wallet HTTP calls off UI thread |
| 2.2.2 | CR-2.2 + CR-2.3: Per-request map with mutex (replaces single global) |
| 2.2.3 | CR-2.4: Move whitelist from JSON file to DB-backed |

### Sub-Phase 2.3: Notification UI + Permission Flow
| Step | Description |
|------|-------------|
| 2.3.1 | Build `UserNotificationModal` component (payment, certificate, domain variants) |
| 2.3.2 | Wire C++ → Frontend → C++ message flow |
| 2.3.3 | Implement auto-approve engine (trust level + USD spending limits) |
| 2.3.4 | Rate limiting (10 req/min per domain) |
| 2.3.5 | "Advanced Settings" in first-visit notification |
| 2.3.6 | "Block This Site" functionality |

### Sub-Phase 2.4: Defense-in-Depth + Polish
| Step | Description |
|------|-------------|
| 2.4.1 | Rust-side permission checks in `well_known_auth` and `create_action` |
| 2.4.2 | Session expiry logic (permanent by default, user can set expiry) |
| 2.4.3 | Daily spending reset |
| 2.4.4 | Integration testing (concurrent tabs, timeouts, edge cases) |
| 2.4.5 | Documentation updates |

---

## Sources

### BSV Specs
- [BRC-43: Security Levels](https://github.com/bitcoin-sv/BRCs/blob/master/key-derivation/0043.md)
- [BRC-44: Admin Protocol Namespaces](https://bsv.brc.dev/key-derivation/0044)
- [BRC-52: Identity Certificates](https://bsv.brc.dev/peer-to-peer/0052)
- [BRC-53: Certificate Creation/Revelation](https://github.com/bitcoin-sv/BRCs/blob/master/wallet/0053.md)
- [BRC-73: Group Permissions](https://bsv.brc.dev/wallet/0073)
- [BRC-100: Wallet Interface](https://bsv.brc.dev/wallet/0100)
- [BRC-103: Mutual Authentication](https://hub.bsvblockchain.org/brc/peer-to-peer/0103)
- [BRC-104: HTTP Transport](https://bsv.brc.dev/peer-to-peer/0104)
- [BRC-105: HTTP Monetization](https://hub.bsvblockchain.org/brc/payments/0105)

### Industry
- [EIP-2255: Wallet Permissions](https://eips.ethereum.org/EIPS/eip-2255)
- [ERC-7715: Grant Permissions](https://eips.ethereum.org/EIPS/eip-7715)
- [CAIP-25: Session Authorization](https://chainagnostic.org/CAIPs/caip-25)
- [Phantom Auto-Confirm](https://phantom.com/learn/blog/auto-confirm)
- [Coinbase Spend Permissions](https://docs.cdp.coinbase.com/server-wallets/v2/evm-features/spend-permissions)
- [MetaMask Blockaid Security](https://metamask.io/news/metamask-security-alerts-by-blockaid-the-new-normal-for-a-safer-transaction)

### Internal
- [phase-2-user-notifications.md](./phase-2-user-notifications.md)
- [HTTP_INTERCEPTOR_FLOW_GUIDE.md](./HTTP_INTERCEPTOR_FLOW_GUIDE.md)
- [CEF_REFINEMENT_TRACKER.md](../CEF_REFINEMENT_TRACKER.md)
- `rust-wallet/src/domain_whitelist.rs`
- `cef-native/src/core/HttpRequestInterceptor.cpp`

---

**End of Document**
