# Sprint 10: Scriptlet Compatibility System — Implementation Plan

**Created**: 2026-02-25
**Status**: Planning
**Estimated Duration**: 2-3 days
**Dependencies**: Sprint 8 (Ad Blocking) complete, Sprint 9 (Settings) in progress

---

## Problem Statement

Scriptlet injection (Sprint 8e) breaks authentication flows on x.com and likely other sites. Our 6 custom scriptlets + uBlock Origin scriptlets.js hook `fetch()`, `XMLHttpRequest`, and `JSON.parse()` — these hooks interfere with OAuth token exchanges, SSO callbacks, and auth session management.

**Root cause**: Scriptlets inject into **every** page context via `OnContextCreated` (before any page JS runs). On sites like x.com, the `fetch()` proxy from `trusted-replace-fetch-response.js` intercepts auth API calls and can corrupt response bodies or consume response streams. Similarly, `JSON.parse` hooks from `json-prune` can strip auth-related fields from parsed JSON.

**Current workaround**: Users manually disable the privacy shield per-site via the shield icon toggle. This disables ALL ad blocking (network + cosmetic + scriptlets), which is too aggressive.

---

## Research Summary

### How Brave Handles This

Brave uses a **three-layer approach**:

1. **`brave-unbreak.txt`** (~2,500+ rules): A dedicated exception filter list that ships with the browser and auto-updates. Contains `@@` network exception rules and `#@#+js()` scriptlet exception rules for known-broken domains (Facebook OAuth, Google CAPTCHA, Atlassian auth, Adobe auth, etc.).

2. **First-party exemption (Standard mode)**: Network-level blocking skips first-party requests. However, cosmetic filters and scriptlets **still apply** even in Standard mode — this is the exact same breakage vector we have.

3. **Per-site Shields toggle**: Users can set Shields to Standard/Aggressive/Disabled per site. No granular "disable scriptlets only" option in the UI.

**Key insight**: Brave does NOT auto-detect auth flows. Their approach is entirely **reactive** — when users report breakage, the team adds rules to `brave-unbreak.txt`.

### How uBlock Origin Handles This

uBlock Origin supports the `#@#+js()` blanket exception syntax:
- `x.com#@#+js()` — disables ALL scriptlet injection on x.com
- `x.com#@#+js(set, areAdsBlocked, false)` — disables one specific scriptlet

This syntax was added in uBlock Origin v1.22.0 and is supported by adblock-rust since v0.8.2. When a filter list contains `x.com#@#+js()`, the engine's `url_cosmetic_resources("https://x.com/...")` call returns an **empty** `injected_script` field.

### Common Breakage Patterns

| Pattern | Affected Scriptlets | How It Breaks Auth |
|---------|--------------------|--------------------|
| `fetch()` proxy consuming response body | `trusted-replace-fetch-response`, `json-prune-fetch-response` | OAuth token exchange fails — `.text()` on original response consumes the stream |
| XHR `responseText` override | `trusted-replace-xhr-response`, `json-prune-xhr-response` | SSO callbacks get wrong `readyState` or lost `withCredentials` |
| `JSON.parse` hook stripping fields | uBlock `json-prune` | Auth tokens, CSRF tokens, session IDs pruned from parsed objects |
| DOM manipulation | `remove-node-text`, `trusted-replace-node-text` | Hidden CSRF input fields or inline auth config scripts altered |
| Timing conflict | All scriptlets | Scriptlets wrap `fetch` before auth SDK defines its own `fetch` wrapper — layered proxies conflict |

---

## Architecture Decision

### Two-Layer Exception System

**Layer 1: Hodos Compatibility Filter List (adblock-rust native)**

A custom filter list loaded into the adblock engine alongside EasyList/EasyPrivacy. Uses standard `#@#+js()` syntax so the engine handles exceptions natively — no C++ bypass logic needed.

```
! Hodos Browser Scriptlet Compatibility Exceptions
! Updated: 2026-02-25
! Expires: 7 days
! Homepage: https://github.com/ArcBit/Hodos-Browser

! === Authentication Domains ===
! These domains use OAuth/SSO flows that scriptlet injection breaks
x.com#@#+js()
twitter.com#@#+js()
api.twitter.com#@#+js()
abs.twimg.com#@#+js()

! Google Auth (OAuth, FedCM, CAPTCHA)
accounts.google.com#@#+js()
accounts.youtube.com#@#+js()
myaccount.google.com#@#+js()

! Microsoft Auth
login.microsoftonline.com#@#+js()
login.live.com#@#+js()
login.microsoft.com#@#+js()

! GitHub Auth
github.com#@#+js()

! Apple Auth
appleid.apple.com#@#+js()

! Facebook/Meta Auth (OAuth SDK used by many sites)
@@||connect.facebook.net^*/sdk/$script
www.facebook.com#@#+js()

! === Banking / Financial ===
! Banks have strict CSP and auth flows that break with API hooks
*.chase.com#@#+js()
*.bankofamerica.com#@#+js()
*.wellsfargo.com#@#+js()
*.paypal.com#@#+js()

! === E-commerce Auth ===
*.amazon.com#@#+js()
```

**Why this approach**: The `#@#+js()` syntax is supported by adblock-rust 0.10.3 (confirmed since v0.8.2). When the engine processes `url_cosmetic_resources()` for a domain with `#@#+js()`, it returns an empty `injected_script` field. This means:
- No C++ changes needed for basic exception handling
- CSS cosmetic filtering still works (only scriptlets disabled)
- Network-level blocking still works
- The exception list is updatable independently of browser code

**Layer 2: Per-Site Scriptlet Toggle (UI enhancement)**

Extend the existing per-site privacy shield toggle to support three protection levels instead of two:

| Level | Network Blocking | CSS Cosmetic | Scriptlets | Use Case |
|-------|-----------------|-------------|------------|----------|
| **Full** | Yes | Yes | Yes | Default for all sites |
| **Standard** | Yes | Yes | **No** | Sites where scriptlets break auth |
| **Off** | No | No | No | Sites where all blocking breaks |

This gives users a middle ground: they can disable just scriptlets on a site without losing ad blocking entirely.

---

## Implementation Plan

### 10a: Hodos Compatibility Filter List (Day 1, ~4 hours)

**Goal**: Create and load a custom exception filter list that disables scriptlets on known-broken auth domains.

#### Step 1: Create the Exception List File

**File**: `adblock-engine/src/hodos-unbreak.txt` (embedded in binary)

The list uses standard ABP/uBO filter syntax. Include `#@#+js()` blanket exceptions for auth domains, and `@@` network exceptions for essential OAuth SDKs.

```
! Title: Hodos Browser Compatibility Exceptions
! Expires: 7 days
! Last modified: 2026-02-25
! License: MIT

! --- Authentication Domains (scriptlet exceptions) ---
x.com#@#+js()
twitter.com#@#+js()
api.twitter.com#@#+js()
accounts.google.com#@#+js()
accounts.youtube.com#@#+js()
myaccount.google.com#@#+js()
login.microsoftonline.com#@#+js()
login.live.com#@#+js()
login.microsoft.com#@#+js()
github.com#@#+js()
appleid.apple.com#@#+js()
www.facebook.com#@#+js()

! --- OAuth SDK Network Exceptions ---
@@||connect.facebook.net^*/sdk/$script
@@||accounts.google.com^$script,domain=~google.com
@@||apis.google.com/js/api.js$script

! --- Banking / Financial (scriptlet exceptions) ---
*.chase.com#@#+js()
*.bankofamerica.com#@#+js()
*.wellsfargo.com#@#+js()
*.paypal.com#@#+js()
*.stripe.com#@#+js()

! --- E-commerce (scriptlet exceptions) ---
*.amazon.com#@#+js()
```

#### Step 2: Load the Exception List in Engine Init

**File**: `adblock-engine/src/engine.rs`

Modify `build_from_lists()` to load the Hodos unbreak list as an additional filter list with default permissions:

```rust
// After loading EasyList, EasyPrivacy, uBlock filters...
let hodos_unbreak = include_str!("hodos-unbreak.txt");
filter_set.add_filter_list(hodos_unbreak, ParseOptions::default());
```

The `include_str!` macro embeds the file at compile time, so no runtime file I/O is needed. The list is small (~50 rules) so it adds negligible overhead.

**Also load on rebuild/update**: Ensure `rebuild_engine()` (the auto-update path) also includes this list.

#### Step 3: Add Auto-Update Support for Exception List

**File**: `adblock-engine/src/engine.rs`

In addition to the embedded default list, check for an updatable version at `%APPDATA%/HodosBrowser/adblock/hodos-unbreak.txt`. If present, load it instead of the embedded version. This allows updating the exception list without a browser update.

```rust
fn load_unbreak_list() -> String {
    // Try to load from user data directory first (updatable)
    if let Ok(content) = std::fs::read_to_string(&unbreak_path) {
        return content;
    }
    // Fall back to embedded version
    include_str!("hodos-unbreak.txt").to_string()
}
```

#### Step 4: Verify Exception Behavior

Test that `url_cosmetic_resources("https://x.com/")` returns an empty `injected_script` field when the exception list is loaded. Write a unit test:

```rust
#[test]
fn test_scriptlet_exception_xcom() {
    let engine = build_test_engine_with_unbreak();
    let resources = engine.url_cosmetic_resources("https://x.com/");
    assert!(resources.injected_script.is_empty(),
        "x.com should have no scriptlets due to #@#+js() exception");
    // But CSS should still work
    // (hideSelectors may or may not be empty depending on filter rules)
}
```

#### Verification Checklist (10a)

- [ ] Build adblock-engine (`cargo build --release`)
- [ ] Unit test passes: x.com has empty `injected_script`
- [ ] Unit test passes: youtube.com still has non-empty `injected_script`
- [ ] `POST /cosmetic-resources` with `url=https://x.com/` returns empty `injectedScript`
- [ ] `POST /cosmetic-resources` with `url=https://youtube.com/` returns non-empty `injectedScript`

---

### 10b: Per-Site Scriptlet Toggle — Backend (Day 1-2, ~4 hours)

**Goal**: Allow users to toggle scriptlet injection on/off per domain separately from full ad blocking.

#### Step 1: Add `scriptlets_enabled` Column

**File**: `rust-wallet/src/database/migrations.rs`

Add migration V6 (or next available):

```rust
fn migrate_v6(conn: &Connection) -> Result<()> {
    conn.execute(
        "ALTER TABLE domain_permissions ADD COLUMN scriptlets_enabled INTEGER DEFAULT 1",
        [],
    )?;
    Ok(())
}
```

#### Step 2: Add Rust Endpoint

**File**: `rust-wallet/src/handlers.rs`

Add `GET/POST /adblock/scriptlet-toggle?domain=X` endpoints mirroring the existing `/adblock/site-toggle` pattern:

```rust
pub async fn get_scriptlet_toggle(
    query: web::Query<DomainQuery>,
    state: web::Data<AppState>,
) -> impl Responder {
    // Query domain_permissions for scriptlets_enabled
}

pub async fn set_scriptlet_toggle(
    query: web::Query<DomainQuery>,
    body: web::Json<ToggleBody>,
    state: web::Data<AppState>,
) -> impl Responder {
    // Update domain_permissions SET scriptlets_enabled = ?
}
```

#### Step 3: Update C++ DomainPermissionCache

**File**: `cef-native/include/core/AdblockCache.h` (or wherever `DomainPermissionCache` is defined)

Add `scriptlets_enabled` to the cached permission data. When checking whether to fetch/inject scriptlets, check this flag.

#### Step 4: Check Scriptlet Flag Before Injection

**File**: `cef-native/src/handlers/simple_handler.cpp`

In the `OnBeforeBrowse` scriptlet pre-cache logic, before calling `/cosmetic-resources`:

```cpp
// Check if scriptlets are enabled for this domain
std::string domain = ExtractDomain(url);
bool scriptsEnabled = DomainPermissionCache::GetInstance()
    .isScriptletsEnabled(domain);

if (scriptsEnabled) {
    // Existing: fetch cosmetic resources and pre-cache scriptlets
    auto cosmeticData = AdblockCache::GetInstance().getCosmeticResources(url);
    if (!cosmeticData.injectedScript.empty()) {
        // Send preload_cosmetic_script IPC...
    }
} else {
    // Skip scriptlet pre-cache, but still fetch CSS cosmetic data
    auto cosmeticData = AdblockCache::GetInstance().getCosmeticResources(url);
    // Only send CSS, not scriptlets
}
```

Alternatively, since the Hodos unbreak list already handles built-in exceptions via `#@#+js()`, this C++ check is only needed for **user-toggled** exceptions. The two layers work together: built-in list handles known domains, user toggle handles everything else.

#### Step 5: Adblock Engine Support for Per-Request Scriptlet Skip

**File**: `adblock-engine/src/handlers.rs`

Add an optional `skipScriptlets` parameter to `/cosmetic-resources`:

```rust
#[derive(Deserialize)]
pub struct CosmeticRequest {
    pub url: String,
    #[serde(default)]
    pub skip_scriptlets: bool,
}

pub async fn cosmetic_resources(body: web::Json<CosmeticRequest>, ...) -> impl Responder {
    let resources = engine.url_cosmetic_resources(&body.url);
    let injected_script = if body.skip_scriptlets {
        String::new()
    } else {
        resources.injected_script
    };
    // Return with modified injected_script
}
```

#### Verification Checklist (10b)

- [ ] Migration V6 applies cleanly
- [ ] `GET /adblock/scriptlet-toggle?domain=x.com` returns `{"enabled": true}` (default)
- [ ] `POST /adblock/scriptlet-toggle?domain=x.com` with `{"enabled": false}` → scriptlets disabled
- [ ] Subsequent `/cosmetic-resources` with `skip_scriptlets=true` returns empty `injectedScript`
- [ ] Build Rust wallet and adblock engine

---

### 10c: Privacy Shield UI Enhancement (Day 2, ~4 hours)

**Goal**: Update the Privacy Shield panel to show three protection levels and allow granular scriptlet control.

#### Step 1: Update Privacy Shield Panel

**File**: `frontend/src/components/PrivacyShieldPanel.tsx`

Add a third toggle row for scriptlets (below existing ad blocking and cookie blocking toggles):

```
Shield Panel:
┌──────────────────────────────────────┐
│  Privacy Shield for example.com      │
│                                      │
│  ● Ad Blocking          [ON/OFF]     │
│    12 ads blocked                    │
│                                      │
│  ● Scriptlet Injection   [ON/OFF]    │  ← NEW
│    Hooks page JS to strip ad data    │
│    ⚠ Disable if login breaks        │
│                                      │
│  ● Cookie Blocking       [ON/OFF]    │
│    3 cookies blocked                 │
│                                      │
│  [Reset to defaults]                 │
└──────────────────────────────────────┘
```

#### Step 2: Add useScriptlets Hook

**File**: `frontend/src/hooks/useScriptlets.ts` (NEW)

```typescript
export function useScriptlets(domain: string) {
    const [enabled, setEnabled] = useState(true);

    useEffect(() => {
        // Fetch current state
        fetch(`http://127.0.0.1:3301/adblock/scriptlet-toggle?domain=${domain}`)
            .then(r => r.json())
            .then(data => setEnabled(data.enabled));
    }, [domain]);

    const toggle = async () => {
        const newState = !enabled;
        await fetch(`http://127.0.0.1:3301/adblock/scriptlet-toggle?domain=${domain}`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ enabled: newState })
        });
        setEnabled(newState);
        // Notify C++ to invalidate cache and refresh
        window.cefMessage?.send('adblock_scriptlet_toggle', [domain, String(newState)]);
    };

    return { enabled, toggle };
}
```

#### Step 3: Update usePrivacyShield Composition

**File**: `frontend/src/hooks/usePrivacyShield.ts`

Add scriptlet state to the composed hook so PrivacyShieldPanel has all three controls.

#### Step 4: Add IPC Handler for Scriptlet Toggle

**File**: `cef-native/src/handlers/simple_handler.cpp`

Add `adblock_scriptlet_toggle` IPC handler that updates the C++ cache and signals that scriptlet state changed.

#### Verification Checklist (10c)

- [ ] Privacy Shield panel shows three toggles (ads, scriptlets, cookies)
- [ ] Toggling scriptlets off for x.com → disables scriptlet injection on x.com
- [ ] Ad blocking and cosmetic CSS still work when only scriptlets disabled
- [ ] Toggle persists across page refreshes

---

### 10d: Auth Flow Testing & Exception List Tuning (Day 2-3, ~4 hours)

**Goal**: Verify authentication works on critical sites and tune the exception list.

#### Test Matrix

| Site | Auth Flow | Test Steps | Expected |
|------|-----------|------------|----------|
| x.com | OAuth + session cookies | Navigate → Sign In → enter credentials → verify feed loads | Login succeeds with scriptlets off (unbreak list) |
| google.com | Google Sign-In (FedCM/OAuth) | Navigate → Sign In → Google auth flow | Login succeeds |
| github.com | Session cookies + OAuth | Navigate → Sign In → enter credentials | Login succeeds |
| youtube.com | Google auth (shared with google.com) | Navigate → Sign In → verify logged in | Login succeeds, ads still blocked |
| accounts.google.com | OAuth hub | Start from any Google OAuth redirect | Auth flow completes |
| login.microsoftonline.com | Microsoft OAuth | Navigate → Sign In | Auth flow completes |
| amazon.com | Session cookies | Navigate → Sign In → enter credentials | Login succeeds |
| discord.com | Session cookies + OAuth | Navigate → Sign In | Login succeeds |
| reddit.com | OAuth | Navigate → Sign In → choose auth method | Login succeeds |

#### Testing Process

1. Build all three services (wallet, adblock-engine, frontend)
2. Start browser with fresh profile (or clear cookies)
3. For each site in the matrix:
   a. Navigate to site
   b. Verify ads are blocked (check shield count)
   c. Attempt to log in
   d. If login fails: add domain to `hodos-unbreak.txt`, rebuild adblock-engine, re-test
   e. If login succeeds: mark as passing
4. Document any new domains that need exceptions

#### Exception List Maintenance Strategy

- **Version the list**: Include `! Last modified:` header, increment on changes
- **Document each rule**: Comment explaining why the exception exists
- **Test quarterly**: Re-test all excepted domains — some may become compatible as filter lists evolve
- **Auto-update path**: Future sprint can add automatic download from a Hodos CDN endpoint

---

## Files Changed Summary

| File | Changes |
|------|---------|
| **NEW** `adblock-engine/src/hodos-unbreak.txt` | Scriptlet exception filter list |
| `adblock-engine/src/engine.rs` | Load hodos-unbreak.txt in engine build |
| `adblock-engine/src/handlers.rs` | Add `skip_scriptlets` param to `/cosmetic-resources` |
| `rust-wallet/src/database/migrations.rs` | V6 migration: `scriptlets_enabled` column |
| `rust-wallet/src/database/domain_permission_repo.rs` | CRUD for `scriptlets_enabled` |
| `rust-wallet/src/handlers.rs` | `GET/POST /adblock/scriptlet-toggle` endpoints |
| `cef-native/src/handlers/simple_handler.cpp` | Check scriptlet flag before pre-cache, `adblock_scriptlet_toggle` IPC |
| `cef-native/include/core/AdblockCache.h` | Scriptlet flag in domain permission cache |
| `cef-native/src/handlers/simple_render_process_handler.cpp` | No changes needed (engine handles `#@#+js()` natively) |
| **NEW** `frontend/src/hooks/useScriptlets.ts` | React hook for scriptlet toggle |
| `frontend/src/hooks/usePrivacyShield.ts` | Add scriptlet state to composition |
| `frontend/src/components/PrivacyShieldPanel.tsx` | Add scriptlet toggle row |

---

## Cross-Platform Notes

- **adblock-engine changes**: Pure Rust, cross-platform. No platform code needed.
- **Rust wallet changes**: Pure Rust, cross-platform.
- **C++ changes**: All in `simple_handler.cpp` (shared between platforms). `DomainPermissionCache` already uses `#ifdef _WIN32` / `#elif defined(__APPLE__)` for WinHTTP vs stub.
- **Frontend changes**: Pure React/TypeScript, cross-platform.

---

## Risk Mitigation

| Risk | Impact | Mitigation |
|------|--------|------------|
| `#@#+js()` not supported in adblock-rust 0.10.3 | Need C++ bypass instead | Verify with unit test first; fall back to C++ domain check |
| Exception list too broad | Reduces ad blocking effectiveness | Use `#@#+js()` (scriptlets only), not `@@` (all blocking). Keeps network + CSS |
| Exception list too narrow | Auth still breaks on some sites | Testing matrix + easy user toggle as fallback |
| DB migration conflict with Sprint 9 | Migration numbering collision | Coordinate — Sprint 9 uses V5 (adblock_enabled), Sprint 10 uses V6 |
| YouTube ads return when scriptlets disabled | Users see ads on YouTube | YouTube has `#@#+js()` exception **removed** (YouTube needs scriptlets for ad blocking) |

---

## Post-Sprint Tasks

1. Update `development-docs/browser-core/CLAUDE.md` with Sprint 10 completion
2. Update `00-SPRINT-INDEX.md` status
3. Test against full Standard basket (15 min) with focus on auth sites
4. Add new broken domains discovered during testing to `hodos-unbreak.txt`
5. Consider: Should the exception list be downloadable from a CDN for hot-updates?

---

*This document was generated based on research into Brave's `brave-unbreak.txt`, `adblock-resources` repository, uBlock Origin's `#@#+js()` exception system, and analysis of the existing adblock-engine/CEF integration.*
