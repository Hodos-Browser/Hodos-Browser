# Sprint 8: Ad & Tracker Blocking — Research Findings

**Date**: 2026-02-22 (Updated: 2026-02-23)

---

## Architecture Decision: Separate Microservice (`adblock-engine/`)

**Chosen approach**: Standalone Rust HTTP service at repo root (`adblock-engine/`), running on **port 3302**. C++ starts it alongside the wallet server and calls it via sync WinHTTP with a C++-side URL cache.

**Why separate from `rust-wallet`**:
1. **Separation of concerns** — the wallet handles money/crypto, adblock handles content filtering. Completely unrelated.
2. **Attack surface** — adding a large dependency tree to the security-critical wallet process increases risk.
3. **Independence** — ad blocking works even if the wallet is locked/down, and vice versa.
4. **Startup** — downloading filter lists on first run shouldn't delay wallet availability.
5. **Crash isolation** — engine crash doesn't crash the browser or wallet.

**Why not FFI (static library linked into C++)**: FFI requires a separate crate, cbindgen, CMake linking, system library deps (`ws2_32 userenv bcrypt ntdll`), panic-catching on every call, and `#[repr(C)]` structs. HTTP microservice uses proven patterns already in the codebase.

**Latency trade-off**: Engine check is ~5.7μs in-process, but ~1-2ms via localhost HTTP. Mitigated by aggressive C++-side URL cache — after initial checks, cache hit rate is very high. A typical page loads 50-200 resources, but many share the same ad domains. The 1-2ms per unique URL is imperceptible (page loads take seconds).

---

## The `adblock` Crate

- **Crate**: `adblock` on crates.io (NOT `adblock-rust`)
- **Version**: `=0.10.3` (pinned — v0.10.4+ requires unstable `unsigned_is_multiple_of`, needs Rust 1.87+; we're on stable 1.85.1)
- **Also pin**: `rmp = "=0.8.14"` (rmp-serde 0.15 compat required by adblock 0.10.x)
- **Repo**: `brave/adblock-rust` on GitHub
- **License**: MPL-2.0
- **Performance**: ~5.7μs per `check_network_request()` (242,945 requests across 500 sites benchmark)
- **Memory**: ~50-80MB process footprint

### Core API

```rust
use adblock::lists::{FilterSet, ParseOptions};
use adblock::request::Request;
use adblock::Engine;

// 1. Build filter set from text lists
let mut filter_set = FilterSet::new(false); // false = no debug info
filter_set.add_filter_list(&easylist_text, ParseOptions::default());
filter_set.add_filter_list(&easyprivacy_text, ParseOptions::default());

// 2. Compile engine
let engine = Engine::from_filter_set(filter_set, true); // true = optimize

// 3. Check a request
let request = Request::new(
    "https://ads.example.com/banner.js",   // URL being requested
    "https://news.example.com/article",     // page URL (source)
    "script",                               // resource type
).unwrap();
let result = engine.check_network_request(&request);
if result.matched { /* block */ }

// 4. Serialize for fast startup (v0.10.3 uses .serialize(), not .serialize_raw())
let bytes: Vec<u8> = engine.serialize().unwrap();
std::fs::write("engine.dat", &bytes).unwrap();

// 5. Deserialize (fast reload)
let data = std::fs::read("engine.dat").unwrap();
let mut engine = Engine::default();
engine.deserialize(&data).expect("deserialization failed");
```

### BlockerResult Fields
```rust
pub struct BlockerResult {
    pub matched: bool,              // true = should block
    pub important: bool,            // $important rule
    pub redirect: Option<String>,   // redirect content ($redirect rules)
    pub rewritten_url: Option<String>, // URL after $removeparam
    pub exception: Option<String>,  // matched exception rule (@@...)
    pub filter: Option<String>,     // the blocking rule that matched
}
```

### Resource Types (17 variants)
`beacon`, `csp`, `document`, `dtd`, `fetch`, `font`, `image`, `media`, `object`, `other`, `ping`, `script`, `stylesheet`, `subdocument`, `websocket`, `xlst`, `xmlhttprequest`

### Cargo.toml Configuration
```toml
[dependencies]
# Pinned to 0.10.3: last version compatible with stable Rust 1.85 (0.10.4+ needs unstable unsigned_is_multiple_of)
# default-features = false to disable "unsync-regex-caching" — enables Send+Sync for RwLock<Engine>
adblock = { version = "=0.10.3", default-features = false, features = [
    "embedded-domain-resolver",
    "full-regex-handling",
] }
rmp = "=0.8.14"  # Pin for rmp-serde 0.15 compat (required by adblock 0.10.x)
```

### Key Notes
- In v0.10.3, the feature that removes `Send+Sync` is `unsync-regex-caching` (NOT `single-thread` which is the name in newer versions). `default-features = false` disables it.
- Serialization format has NO cross-version compatibility guarantee — always keep source lists to rebuild
- Cosmetic filtering (CSS element hiding) supported but complex — **skip for MVP**
- `FilterFormat::Hosts` supported for hosts-file format lists

---

## Brave's Default Filter Lists

| List | URL | Expires |
|------|-----|---------|
| EasyList | `https://easylist.to/easylist/easylist.txt` | 4 days |
| EasyPrivacy | `https://easylist.to/easylist/easyprivacy.txt` | 4 days |
| uBlock Origin Filters | `https://raw.githubusercontent.com/uBlockOrigin/uAssets/master/filters/filters.txt` | varies |
| uBlock Origin Privacy | `https://raw.githubusercontent.com/uBlockOrigin/uAssets/master/filters/privacy.txt` | varies |
| URLhaus Malware | `https://malware-filter.gitlab.io/malware-filter/urlhaus-filter-agh-online.txt` | varies |

**Brave also ships**: `brave-unbreak.txt` (exception rules), `coin-miners.txt` (crypto mining). We may need our own unbreak list eventually.

### Standard vs Aggressive Mode
- **Standard** (default, what we'll use): Only blocks **third-party** requests matching filter lists. First-party requests pass through. This is the critical breakage-prevention decision.
- **Aggressive**: Blocks first-party AND third-party. Higher breakage risk.

---

## Service Architecture

### Process Model

```
C++ CEF Shell
    ├── Rust Wallet Backend (localhost:3301) — existing
    └── Adblock Engine (localhost:3302) — NEW, separate process
```

C++ starts both processes via `CreateProcessA` + Job Object (auto-kill on browser exit). The adblock engine is **non-critical** — if it fails to start, browsing works without ad blocking.

### Startup Flow (Two-Phase)

```
1. C++ calls StartAdblockServer()
   - CreateProcessA → hodos-adblock.exe
   - Job Object (KILL_ON_JOB_CLOSE) for auto-cleanup
   - Health poll: 6 attempts × 500ms = 3s max (shorter than wallet's 5s — non-critical)

2. Adblock engine main():
   a. Start HTTP server immediately
   b. GET /health returns {"status": "loading"}
   c. Check for serialized engine.dat
      - If exists → deserialize (~100ms) → "ready"
      - If not → download EasyList + EasyPrivacy (3-10s) → compile → serialize → "ready"
   d. GET /health now returns {"status": "ready"}

3. C++ sets g_adblockServerRunning = true if process launched
   - Even if engine still loading — pages load without blocking until ready
   - First-run: ads slip through for ~5-10s while lists download. Acceptable.
```

### C++ Integration Point

Hook in `GetResourceRequestHandler()` in `simple_handler.cpp`, BEFORE wallet interception:

```cpp
// --- ADBLOCK CHECK (before wallet interception) ---
if (g_adblockServerRunning && !isLocalUrl(url)) {
    std::string sourceUrl = frame ? frame->GetURL().ToString() : "";
    std::string resourceType = CefResourceTypeToAdblock(request->GetResourceType());

    auto result = AdblockCache::GetInstance().check(url, sourceUrl, resourceType);
    if (result && result->blocked) {
        disable_default_handling = true;
        return new BlockedResourceHandler();  // Returns 0-byte response
    }
}
// --- END ADBLOCK CHECK ---
// Existing wallet interception logic continues...
```

### C++ AdblockCache Singleton

Sync WinHTTP calls to `POST localhost:3302/check` with an in-memory result cache:

```cpp
class AdblockCache {
    struct CacheKey { std::string url; std::string sourceDomain; };
    struct Result { bool blocked; std::string filter; };

    std::shared_mutex mutex_;
    std::unordered_map<size_t, Result> cache_;  // hash(url+sourceDomain) → result

    // Cache invalidation:
    // - clearForBrowser(browserId) on main frame navigation
    // - clearAll() on filter list update or per-site toggle change
};
```

Same pattern as `DomainPermissionCache`, `BSVPriceCache`, `WalletStatusCache` — all sync WinHTTP singletons.

### Endpoints (Adblock Engine, port 3302)

| Method | Path | Purpose |
|--------|------|---------|
| GET | `/health` | `{"status": "ready"}` or `{"status": "loading"}` |
| POST | `/check` | `{url, sourceUrl, resourceType}` → `{blocked, filter, redirect}` |
| GET | `/status` | `{enabled, listCount, totalRules, lastUpdate, lists}` |
| POST | `/toggle` | `{enabled: bool}` → `{enabled}` |

---

## Per-Site Toggle Design

**Brave's approach**: Lion icon → popup with "Shields UP/DOWN" + blocked count. Advanced view has granular dropdowns.

**Our approach**: Leverage existing `domain_permissions` infrastructure:
- Add `adblock_enabled` boolean column (default true) to `domain_permissions` table
- C++ checks `DomainPermissionCache` before calling adblock engine
- Shield icon in header bar with blocked count badge
- Click → popup: "Ad blocking ON/OFF for [domain]" + count

**Comparison to existing cookie blocking pattern**:
- `CookieBlockManager`: singleton + SQLite + in-memory cache + `shared_mutex`
- Hook: `GetCookieAccessFilter()` in `HttpRequestInterceptor.cpp`
- Per-domain: `blocked_domains` table with `is_wildcard` and `source`
- Default list: `DefaultTrackerList.h` (24 tracker domains)
- Frontend: `useCookieBlocking.ts` + `CookiesPanel.tsx`
- Ad blocking should follow this same pattern

---

## Implementation Phases

### Phase 8a: Standalone Rust Engine (~1-2 days)
- Create `adblock-engine/` at repo root with `Cargo.toml`
- `src/main.rs`: Actix-web server on port 3302, 2 workers
- `src/engine.rs`: `AdblockEngine` struct — `RwLock<Engine>`, init from serialized or download, check_request(), serialize/deserialize, global toggle
- `src/handlers.rs`: HTTP endpoint handlers (`/health`, `/check`, `/status`, `/toggle`)
- Download EasyList + EasyPrivacy on first run → `%APPDATA%/HodosBrowser/adblock/lists/`
- Serialize compiled engine to `engine.dat` for fast startup
- Two-phase startup: server starts immediately with "loading", engine loads async

### Phase 8b: C++ Integration (~1 day)
- `StartAdblockServer()` / `StopAdblockServer()` in `cef_browser_shell.cpp` (mirror wallet pattern)
- `AdblockCache` singleton: sync WinHTTP to `/check`, in-memory URL cache
- Hook in `GetResourceRequestHandler()` BEFORE wallet interception
- `BlockedResourceHandler`: returns `RV_CANCEL` or 0-byte response for blocked URLs
- `CefResourceTypeToAdblock()` mapping function
- Cache invalidation on main frame navigation (per browser ID)
- macOS cross-platform stubs (`#ifdef _WIN32` / `#elif defined(__APPLE__)`)

### Phase 8c: Per-Site Toggle + UI (~1-2 days)
- `adblock_enabled` column in `domain_permissions` (migration V5)
- Rust wallet endpoint: `GET/POST /adblock/site-toggle?domain=X` (queries/updates domain_permissions)
- C++ `DomainPermissionCache`: add adblock status to cached data
- Shield icon in header React (MUI): blocked count badge
- Click → small dropdown/popup: domain name, ON/OFF toggle, blocked count, "Blocking may break this site" hint
- IPC: `adblock_site_toggle` message

### Phase 8d: Filter List Auto-Update (~0.5 day)
- Background tokio task in adblock-engine (every 6 hours)
- Check `meta.json` timestamps, respect `Expires` headers
- Download updated lists, recompile engine, swap under write lock, re-serialize
- C++ cache invalidation via version counter on `/status`

### Phase 8e: Cosmetic Filtering + Scriptlet Injection — YouTube Ad Blocking (~2-3 days)

**Why**: Network-level blocking (8a-8d) catches third-party ad requests but cannot block YouTube ads. YouTube embeds ad metadata inside first-party JSON API responses (`ytInitialPlayerResponse.adPlacements`, `playerResponse.playerAds`). Blocking these requires **scriptlet injection** — JavaScript that hooks `JSON.parse()`, `fetch()`, and `XMLHttpRequest` to strip ad fields before the player sees them.

**This is how Brave blocks YouTube ads.** The entire mechanism is open source (MPL-2.0 + GPL-3.0). We already use the same `adblock-rust` engine which has full scriptlet support via `engine.use_resources()` and `engine.url_cosmetic_resources()` — we just aren't calling those APIs yet.

#### Research Findings (2026-02-24)

##### `resource-assembler` confirmed compatible with v0.10.3
- Feature exists in v0.10.3 (one of 10 features on docs.rs). NOT a default feature — must be explicitly enabled.
- `assemble_scriptlet_resources(&Path) -> Vec<Resource>` parses uBlock's `scriptlets.js` format.
- The feature does direct filesystem I/O — described as "not necessary for in-browser use" by Brave (they compile in-process). Fine for our separate microservice architecture.

##### Key API surface (v0.10.3)
```rust
// Load scriptlet resources into engine
engine.use_resources(resources: impl IntoIterator<Item = Resource>);
engine.add_resource(resource: Resource) -> Result<(), AddResourceError>;

// Query cosmetic resources for a URL
engine.url_cosmetic_resources(url: &str) -> UrlSpecificResources;

// UrlSpecificResources struct
pub struct UrlSpecificResources {
    pub hide_selectors: HashSet<String>,      // CSS selectors → display:none!important
    pub procedural_actions: HashSet<String>,   // JSON-encoded procedural filters
    pub exceptions: HashSet<String>,           // Class/id selectors exempt from generic rules
    pub injected_script: String,               // Fully-assembled JS for scriptlet injection
    pub generichide: bool,                     // $generichide exception applies
}

// Resource struct (for manual construction if needed)
pub struct Resource {
    pub name: String,
    pub aliases: Vec<String>,
    pub kind: ResourceType,        // Template (has {{1}}) or Mime(MimeType)
    pub content: String,           // Base64-encoded resource data
    pub dependencies: Vec<String>,
    pub permission: PermissionMask,
}
```

##### PermissionMask for trusted scriptlets
```rust
// Bit 0 (value 1) = "trusted" — enables trusted-replace-xhr-response etc.
// Bit 1 (value 2) = Brave-specific scriptlets
let trusted_options = ParseOptions {
    permissions: PermissionMask::from_bits(1),
    ..ParseOptions::default()
};
filter_set.add_filter_list(&ublock_filters_text, trusted_options);

// EasyList/EasyPrivacy use default (no trusted scriptlets needed)
filter_set.add_filter_list(&easylist_text, ParseOptions::default());
```

##### Brave's injection pipeline (source: brave-core cosmetic_filters_js_handler.cc)
1. **Trigger**: `DidCreateScriptContext` (equivalent to our `OnContextCreated`)
2. **Acquisition**: Async Mojo IPC to browser process → engine → back to renderer
3. **Injection**: `ExecuteScriptInIsolatedWorld()` (isolated V8 world, still hooks page APIs)
4. **CSS**: `CSSRulesRoutine()` injects raw CSS or JSON-based selector system
5. **Post-load**: MutationObserver watches for new DOM elements, applies additional generic selectors

##### Critical timing bug Brave had (issue #18301)
Brave's async injection let YouTube scripts run before scriptlets could hook APIs. Fix (PR #10214, v1.31.x) made injection synchronous, blocking page load until scriptlets were available. **This is the #1 risk for our implementation.**

##### Communication pattern for our architecture
**Recommended: Browser process pre-cache + IPC to renderer**
```
1. Browser process: OnLoadStart(frame)
   → POST localhost:3302/cosmetic-resources with frame URL
   → Cache result keyed by (browserId, frameId)
   → SendProcessMessage(PID_RENDERER, "inject_cosmetic", {script, css})

2. Renderer process: OnProcessMessageReceived()
   → frame->ExecuteJavaScript(injectedScript)
   → frame->ExecuteJavaScript(cssInjectionCode)
```

**Timing risk**: `OnLoadStart` fires after navigation commit but before page content renders. Small window where early inline `<script>` tags could execute before IPC arrives. For YouTube this is likely acceptable — critical scripts (`ytInitialPlayerResponse`) are in later-loading JS bundles, not inline in initial HTML.

**NOT recommended**: Direct HTTP from renderer process (sandboxed, blocks V8 thread), or sync IPC (CEF IPC is inherently async).

##### YouTube scriptlet compatibility

| Scriptlet | Type | v0.10.3 | Permission | Purpose |
|-----------|------|---------|------------|---------|
| `set-constant` (`set`) | Standard | YES | default | Null out `ytInitialPlayerResponse.adPlacements` etc. |
| `json-prune` | Standard | YES | default | Strip ad fields from `JSON.parse()` results |
| `abort-on-property-read` | Standard | YES | default | Block reads of ad-related properties |
| `trusted-replace-xhr-response` | **Trusted** | YES | `from_bits(1)` | Modify XHR responses to strip ad data |
| `trusted-replace-fetch-response` | **Trusted** | YES | `from_bits(1)` | Modify fetch responses to strip ad data |

**Without trusted scriptlets**: `set-constant` + `json-prune` catch ~60-70% of YouTube ads (initial page data). **With trusted scriptlets**: catches remaining XHR/fetch-delivered ads (~95%+). **Recommendation**: Enable trusted scriptlets for uBlock Origin filter lists.

##### Performance (from Brave's testing)
- Blanket CSS injection: ~4% CPU overhead
- uBlock Origin-style approach: ~17% CPU overhead
- MutationObserver approach: ~22% CPU overhead
- Starting with Phase 1 (CSS only) gives lowest overhead

---

#### Implementation: Sub-Phase 8e-1 — CSS Cosmetic Filtering (Rust + C++, ~1 day)

Simpler, no timing issues, immediate visible value (hides ad banners, sidebar ads, overlay elements).

**Rust (adblock-engine)**:
- Enable `resource-assembler` feature in Cargo.toml
- Download `scriptlets.js` + uBlock Origin filter lists during `build_from_lists()`
- Call `assemble_scriptlet_resources()` → `engine.use_resources(resources)` after engine build
- Load uBlock filters with `PermissionMask::from_bits(1)` (trusted)
- New endpoint: `POST /cosmetic-resources` → calls `engine.url_cosmetic_resources(url)` → returns JSON

**C++ (browser process)**:
- `OnLoadStart()` in `simple_handler.cpp`: POST to `/cosmetic-resources`, cache result
- Send `inject_cosmetic` IPC to renderer with `hideSelectors` as CSS string

**C++ (renderer process)**:
- `OnProcessMessageReceived()` in `simple_render_process_handler.cpp`
- Inject CSS: `document.head.insertAdjacentHTML('beforeend', '<style>SELECTORS { display:none!important }</style>')`

#### Implementation: Sub-Phase 8e-2 — Scriptlet Injection (C++, ~1-2 days)

Complex, timing-critical, needed for YouTube video ads.

**Rust**: Already done in 8e-1 — `injected_script` field is populated by `url_cosmetic_resources()` once resources are loaded.

**C++ (browser process)**:
- Extend `inject_cosmetic` IPC to include `injectedScript` string alongside CSS
- Pre-cache cosmetic data per frame URL on navigation

**C++ (renderer process)**:
- `OnProcessMessageReceived()`: if `injectedScript` non-empty, `frame->ExecuteJavaScript(script, url, 0)`
- Runs in page's main V8 context (NOT isolated world — this is correct for us, scriptlets need to hook `JSON.parse()`, `fetch()`, `XMLHttpRequest` on the page's window object)

**Open question**: Is the `OnLoadStart → IPC → renderer injection` fast enough for YouTube? If not, alternative approach: browser process sends cosmetic IPC proactively when navigation starts (before load), and renderer caches it until `OnContextCreated()` fires.

#### Key YouTube Scriptlet Rules (from uBlock Origin Filters)
```
youtube.com##+js(set, ytInitialPlayerResponse.adPlacements, undefined)
youtube.com##+js(set, ytInitialPlayerResponse.adSlots, undefined)
youtube.com##+js(set, ytInitialPlayerResponse.playerAds, undefined)
youtube.com##+js(trusted-replace-xhr-response, /"adPlacements.*?"\}/, , /player/)
youtube.com##+js(trusted-replace-fetch-response, '"adPlacements"', '"no_ads"', player?)
youtube.com##+js(json-prune, playerResponse.adPlacements playerResponse.playerAds)
```

#### Important Notes
- **Arms race**: YouTube changes ad delivery frequently. Auto-updating filter lists (8d) is essential.
- **Trusted scriptlets**: `trusted-replace-xhr-response` and `trusted-replace-fetch-response` require `PermissionMask::from_bits(1)` on the uBlock filters list. Without this, only `set-constant` and `json-prune` work (partial YouTube blocking).
- **Timing is critical**: If scriptlets inject after YouTube's player code runs, they miss the initial API calls. Brave's fix: synchronous injection. Our approach: pre-cache + IPC (test whether latency is acceptable).
- **`resource-assembler` feature**: Confirmed compatible with v0.10.3 (verified 2026-02-24).

---

## Project Structure

```
adblock-engine/                    # Separate Rust project at repo root
  Cargo.toml
  src/
    main.rs                        # Actix-web server on port 3302, two-phase startup
    engine.rs                      # AdblockEngine: RwLock<Engine>, init, serialize, check
    handlers.rs                    # HTTP endpoint handlers
```

---

## CEF Resource Type Mapping

```cpp
// Map CEF resource type to adblock-rust string
static const char* CefResourceTypeToAdblock(cef_resource_type_t type) {
    switch (type) {
        case RT_MAIN_FRAME:    return "document";
        case RT_SUB_FRAME:     return "subdocument";
        case RT_STYLESHEET:    return "stylesheet";
        case RT_SCRIPT:        return "script";
        case RT_IMAGE:         return "image";
        case RT_FONT_RESOURCE: return "font";
        case RT_SUB_RESOURCE:  return "other";
        case RT_OBJECT:        return "object";
        case RT_MEDIA:         return "media";
        case RT_WORKER:        return "other";
        case RT_SHARED_WORKER: return "other";
        case RT_PREFETCH:      return "other";
        case RT_FAVICON:       return "image";
        case RT_XHR:           return "xmlhttprequest";
        case RT_PING:          return "ping";
        case RT_SERVICE_WORKER: return "other";
        case RT_CSP_REPORT:    return "csp";
        case RT_PLUGIN_RESOURCE: return "object";
        default:               return "other";
    }
}
```

---

## Filter List Storage Layout

```
%APPDATA%/HodosBrowser/
  adblock/
    engine.dat          # Serialized compiled engine (binary, fast reload)
    meta.json           # Last update times, expiry timestamps, list metadata
    lists/
      easylist.txt      # Raw downloaded filter list
      easyprivacy.txt
      ublock-filters.txt    # uBlock Origin Filters (loaded with trusted permissions)
      ublock-privacy.txt    # uBlock Origin Privacy
    resources/
      scriptlets.js     # uBlock Origin scriptlets (parsed by resource-assembler)
```
