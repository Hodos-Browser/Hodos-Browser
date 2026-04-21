# Adblock Engine Source
> Standalone ad & tracker blocking HTTP service built on Brave's `adblock-rust` crate

## Overview

This is a standalone Actix-web HTTP service (port 31302) that wraps Brave's `adblock` crate to provide network request blocking, cosmetic CSS selector filtering, and scriptlet injection for the Hodos Browser. The CEF C++ layer starts this process via `CreateProcessA` + Job Object (auto-kill on browser exit) and communicates via HTTP. The C++ side caches results in `AdblockCache` — see `cef-native/include/core/AdblockCache.h`.

## Files

| File | Lines | Purpose |
|------|-------|---------|
| `main.rs` | 154 | Entry point: Actix-web server setup, two-phase startup (HTTP first, engine async), background auto-update task (6-hour interval), cross-platform storage directory resolution |
| `engine.rs` | 1414 | `AdblockEngine` struct: filter list download/caching, engine compile/serialize/deserialize, `RwLock<Engine>` thread-safe request checking, entity-aware same-org override, cosmetic resource extraction, comprehensive test suite |
| `handlers.rs` | 189 | HTTP endpoint handlers: `/health`, `/check`, `/status`, `/toggle`, `/shutdown`, `/cosmetic-resources`, `/cosmetic-hidden-ids` |
| `hodos-unbreak.txt` | 118 | Embedded compatibility exception list: `#@#+js()` blanket scriptlet exceptions + `$generichide` cosmetic CSS exceptions for auth/banking/e-commerce domains |
| `entities.json` | ~400KB | Embedded disconnect.me entity list mapping organizations to their owned domains (CC BY-NC-SA 4.0) |
| `scriptlets/` | 6 files | Bundled JS scriptlet templates for uBlock Origin rules added after the 1.48.x format change |

## Architecture

### Two-Phase Startup
1. HTTP server starts immediately — `/health` returns `"loading"`
2. Engine loads async in background (deserialize `engine.dat` or download filter lists)
3. Once ready, `/health` returns `"ready"` — C++ polls this during startup

### Filter Lists
Four filter lists, split by trust level:

| List | Trust | Source |
|------|-------|--------|
| `easylist.txt` | Standard (`PermissionMask::default()`) | easylist.to |
| `easyprivacy.txt` | Standard | easylist.to |
| `ublock-filters.txt` | Trusted (`PermissionMask::from_bits(1)`) | uBlockOrigin/uAssets |
| `ublock-privacy.txt` | Trusted | uBlockOrigin/uAssets |

Plus `hodos-unbreak.txt` (embedded, standard permissions) and scriptlet resources from uBlock Origin 1.48.4 (pinned to old `///`-delimited format compatible with `assemble_scriptlet_resources()`).

### Storage Layout
```
~/Library/Application Support/HodosBrowser/adblock/   (macOS)
%APPDATA%/HodosBrowser/adblock/                       (Windows)
  engine.dat          # Serialized compiled engine (binary, fast reload)
  meta.json           # List metadata, config_version, timestamps
  hodos-unbreak.txt   # Updatable override (falls back to embedded)
  entities.json       # Updatable disconnect.me entity list
  lists/
    easylist.txt
    easyprivacy.txt
    ublock-filters.txt
    ublock-privacy.txt
  resources/
    scriptlets.js     # uBlock Origin scriptlet resource file
```

### Config Version
`CONFIG_VERSION` (currently 6) forces `engine.dat` rebuild when filter list URLs, features, or scriptlet loading logic changes. Bump this constant when modifying filter list configuration.

## Key Types

### `AdblockEngine` (`engine.rs`)
Thread-safe wrapper around `adblock::Engine`. Core fields:
- `engine: RwLock<Option<Engine>>` — the compiled filter engine (None while loading)
- `enabled: AtomicBool` — global on/off toggle
- `status: RwLock<EngineStatus>` — lifecycle: `Loading` | `Ready` | `Error`
- `update_version: AtomicU64` — monotonically increasing; C++ uses this to invalidate its URL cache
- `entity_map: RwLock<EntityMap>` — disconnect.me same-org domain grouping

Key methods:
- `new(adblock_dir)` — creates engine in Loading state
- `load()` — async initialization: deserialize or download+build
- `check_request(url, source_url, resource_type)` → `(blocked, redirect, filter)` — with entity-aware override
- `cosmetic_resources(url)` → `(selectors, injected_script, generichide)` — Phase 1 cosmetic filtering
- `hidden_class_id_selectors(url, classes, ids)` → selectors — Phase 2 cosmetic filtering
- `rebuild_engine()` — hot-swap engine with freshly downloaded lists + reload entity map from disk (background update)
- `needs_update()` — checks if any list has expired based on `! Expires:` header

### `EntityMap` (`engine.rs`)
Maps domains to organization IDs using disconnect.me's `entities.json`. Enables same-entity CDN allowance (e.g., `twimg.com` on `x.com` is not blocked as a tracker). Uses suffix-walking: `pbs.twimg.com` → tries `pbs.twimg.com`, then `twimg.com`.

### `EngineStatus` (`engine.rs`)
```rust
enum EngineStatus { Loading, Ready, Error }
```

### `FilterListMeta` / `FilterListInfo` (`engine.rs`)
Persisted metadata in `meta.json`: list filenames, URLs, download timestamps, rule counts, expiry timestamps, and `config_version`.

## HTTP Endpoints

| Method | Path | Request Body | Response | Purpose |
|--------|------|-------------|----------|---------|
| GET | `/health` | — | `{ "status": "ready"\|"loading" }` | Lifecycle check (C++ polls at startup) |
| POST | `/check` | `{ "url", "sourceUrl", "resourceType" }` | `{ "blocked", "redirect", "filter", "version" }` | Check if URL should be blocked |
| GET | `/status` | — | `{ "enabled", "status", "listCount", "totalRules", "lastUpdate", "lists": [...] }` | Full engine status |
| POST | `/toggle` | `{ "enabled": bool }` | `{ "enabled": bool }` | Enable/disable blocking |
| POST | `/shutdown` | — | `{ "status": "shutting_down" }` | Graceful exit (spawns delayed `exit(0)`) |
| POST | `/cosmetic-resources` | `{ "url", "skipScriptlets"? }` | `{ "hideSelectors", "injectedScript", "generichide" }` | Phase 1 cosmetic selectors + scriptlets |
| POST | `/cosmetic-hidden-ids` | `{ "url", "classes", "ids" }` | `{ "selectors" }` | Phase 2 generic selectors matching DOM elements |

## Extra Bundled Scriptlets

Six scriptlets are embedded as JS templates in `scriptlets/` because they were added to uBlock Origin after the format changed from the old `///`-delimited format to ES modules (post-1.48.x). The `assemble_scriptlet_resources()` parser can only handle the old format.

| Scriptlet | Trust | Purpose |
|-----------|-------|---------|
| `trusted-replace-fetch-response.js` | Trusted | Replace patterns in fetch() response bodies |
| `trusted-replace-xhr-response.js` | Trusted | Replace patterns in XHR response bodies |
| `json-prune-fetch-response.js` | Standard | Remove keys from JSON fetch() responses |
| `json-prune-xhr-response.js` | Standard | Remove keys from JSON XHR responses |
| `trusted-replace-node-text.js` | Trusted | Replace text content in DOM nodes |
| `remove-node-text.js` | Standard | Remove matching text from DOM nodes |

## Entity-Aware Blocking

When the engine says "block" a network request, `check_request()` checks whether the URL domain and source domain belong to the same organization using the disconnect.me entity list. If they do (e.g., `pbs.twimg.com` loaded on `x.com` — both X Corp), the block is overridden to allow. This only applies to network blocks, not redirects (which are scriptlet-related).

## Hodos Unbreak List

`hodos-unbreak.txt` is a compile-time embedded exception list that protects auth/banking domains from breakage. Each domain gets two layers:
1. `#@#+js()` — blanket disable all scriptlet injection (fetch/XHR proxy breaks OAuth)
2. `@@||domain^$generichide` — disable cosmetic CSS hiding (EasyList hides login forms)

Protected categories: Twitter/X, Google Auth, Microsoft Auth, GitHub, Apple, Facebook/Meta, Discord, Reddit, banking (Chase, BofA, Wells Fargo, PayPal, Stripe), e-commerce (Amazon).

An updatable version can be placed at `<adblock_dir>/hodos-unbreak.txt` to override the embedded one without recompiling.

## Background Auto-Update

A tokio task runs every 6 hours (after an initial 60-second delay). It checks `needs_update()` (any list past its `expires_at` timestamp), then calls `rebuild_engine()` to download fresh lists, recompile, and hot-swap the engine. The `update_version` counter increments so C++ can invalidate its `AdblockCache`.

## Testing

The test suite in `engine.rs` covers:
- Basic ad blocking, exception rules, resource type filtering
- `#@#+js()` blanket scriptlet exceptions
- `$generichide` suppressing all cosmetic CSS selectors
- Hodos unbreak list loading and auth domain protection
- Engine lifecycle (loading state, enable/disable, disabled-returns-false)
- Serialization/deserialization round-trip
- `! Expires:` header parsing (days, hours, default)
- Entity map parsing, same-entity detection, subdomain walking, invalid JSON
- Entity-aware blocking integration (same-entity allowed, cross-entity blocked)
- Real EasyList integration tests (skip if lists not downloaded)

Run tests: `cargo test` from `adblock-engine/`

## Related

- `/CLAUDE.md` — root project context, architecture overview
- `cef-native/include/core/AdblockCache.h` — C++ side: sync HTTP to port 31302, URL result cache, per-browser blocked counts, `AdblockResponseFilter` for YouTube
- `cef-native/src/handlers/simple_handler.cpp` — scriptlet pre-cache IPC, cosmetic CSS/scriptlet injection, scriptlet toggle per-domain
- `cef-native/src/handlers/simple_render_process_handler.cpp` — V8 scriptlet injection in `OnContextCreated`, scriptlet pre-cache via `s_scriptCache`
- `adblock-engine/src/scriptlets/CLAUDE.md` — bundled scriptlet documentation
