//! AdblockEngine — wrapper around Brave's `adblock` crate
//!
//! Manages filter list downloading, engine compilation (from the cached lists at every
//! startup), uBO's current scriptlets, and thread-safe request checking via `RwLock<Engine>`.
//!
//! Storage layout:
//! ```
//! %APPDATA%/HodosBrowser/adblock/
//!   lists/
//!     easylist.txt      # Raw downloaded filter list
//!     easyprivacy.txt
//!   meta.json           # Last update times, list metadata
//! ```

use adblock::lists::{FilterSet, ParseOptions};
use adblock::resources::{PermissionMask, Resource, ResourceType};
use adblock::resources::resource_assembler::assemble_scriptlet_resources;
use adblock::request::Request;
use adblock::Engine;
use base64::{Engine as Base64Engine, engine::general_purpose::STANDARD as BASE64};
use log::{info, warn};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::RwLock;
use std::time::{SystemTime, UNIX_EPOCH};

/// Default expiry if filter list doesn't specify `! Expires:` (4 days)
const DEFAULT_EXPIRY_SECS: u64 = 4 * 24 * 3600;

// ============================================================================
// Filter List URLs
// ============================================================================

/// A configured list: local filename and the URLs to try, in order (mirrors).
type ListSource = (&'static str, &'static [&'static str]);

/// Default filter lists — standard (no trusted scriptlet permission)
const FILTER_LISTS: &[ListSource] = &[
    ("easylist.txt", &["https://easylist.to/easylist/easylist.txt"]),
    ("easyprivacy.txt", &["https://easylist.to/easylist/easyprivacy.txt"]),
];

/// Trusted filter lists — loaded with PermissionMask::from_bits(1) to enable
/// trusted scriptlets (trusted-replace-xhr-response, trusted-replace-fetch-response)
///
/// ⛔ uBO's BUILT lists, not the raw `uAssets/master/filters/*.txt` sources (B5-T0-P6): the raw
/// `filters.txt` pulls in 10 sub-lists with `!#include`, which adblock-rust ignores as comments —
/// ~470 KB loaded of ~1.9 MB. The built lists have the includes inlined and real
/// `! Last modified:` dates. Mirrors in the order uBO's own `assets.json` (1.75.0) lists them.
const TRUSTED_FILTER_LISTS: &[ListSource] = &[
    ("ublock-filters.txt", &[
        "https://ublockorigin.github.io/uAssetsCDN/filters/filters.min.txt",
        "https://ublockorigin.pages.dev/filters/filters.min.txt",
        "https://cdn.jsdelivr.net/gh/uBlockOrigin/uAssetsCDN@main/filters/filters.min.txt",
    ]),
    ("ublock-privacy.txt", &[
        "https://ublockorigin.github.io/uAssetsCDN/filters/privacy.min.txt",
        "https://ublockorigin.pages.dev/filters/privacy.min.txt",
        "https://cdn.jsdelivr.net/gh/uBlockOrigin/uAssetsCDN@main/filters/privacy.min.txt",
    ]),
    // Where uBO publishes fast site fixes (YouTube's land here first).
    ("ublock-quick-fixes.txt", &[
        "https://ublockorigin.github.io/uAssetsCDN/filters/quick-fixes.min.txt",
        "https://ublockorigin.pages.dev/filters/quick-fixes.min.txt",
        "https://cdn.jsdelivr.net/gh/uBlockOrigin/uAssetsCDN@main/filters/quick-fixes.min.txt",
    ]),
];

/// After a failed refresh, retry this soon instead of trusting the stale copy for a full expiry.
/// Shorter than the hourly check, so the very next check retries.
pub(crate) const RETRY_AFTER_FAILURE_SECS: u64 = 1800;

/// A list due within this many seconds of an update check is treated as due.
const DUE_SLACK_SECS: u64 = 300;

/// Ceiling of the per-list retry backoff (`retry_after`).
const MAX_RETRY_SECS: u64 = 6 * 3600;

/// Cap on a downloaded list (uBO's built filters are ~2 MB, EasyList ~2 MB).
const MAX_LIST_BYTES: usize = 32 * 1024 * 1024;

/// Every configured list with its URLs and trust level — the one place both compile paths
/// (`build_from_lists`, `compile_from_disk`) and the tests take them from.
fn configured_lists() -> Vec<(&'static str, &'static [&'static str], ParseOptions)> {
    let trusted = ParseOptions { permissions: PermissionMask::from_bits(1), ..ParseOptions::default() };
    FILTER_LISTS.iter().map(|(f, u)| (*f, *u, ParseOptions::default()))
        .chain(TRUSTED_FILTER_LISTS.iter().map(|(f, u)| (*f, *u, trusted)))
        .collect()
}

/// uBlock Origin scriptlets resource file (pinned to 1.48.4 — last version using
/// the old `///`-delimited format that `assemble_scriptlet_resources()` can parse.
/// Post-1.48.x moved to ES module format which is incompatible with adblock-rust 0.10.3.)
const SCRIPTLETS_URL: &str = "https://raw.githubusercontent.com/gorhill/uBlock/1.48.4/assets/resources/scriptlets.js";
const SCRIPTLETS_FILE: &str = "scriptlets.js";

/// Metadata filename
const META_JSON: &str = "meta.json";

/// Configuration version — bump this when filter list URLs, features, or scriptlet
/// loading changes. Forces a fresh download + rebuild on next startup.
const CONFIG_VERSION: u32 = 9; // v1 = easylist+easyprivacy only, v2 = +uBlock+scriptlets, v3 = fixed scriptlets URL, v4 = +hodos-unbreak, v5 = +entity-aware blocking, v6 = $generichide suppresses P1 CSS, v7 = +Cloudflare challenge exceptions, v8 = +uBlock quick-fixes, v9 = uBO built lists + !#if pruning

/// Hodos Browser compatibility exception list (embedded at compile time).
/// Contains #@#+js() blanket exceptions for auth domains where scriptlet injection breaks login.
const HODOS_UNBREAK: &str = include_str!("hodos-unbreak.txt");

/// disconnect.me entity list (embedded at compile time, ~400KB).
/// Maps organizations to their owned domains so same-entity CDN resources
/// (e.g. twimg.com on x.com) are not blocked as trackers.
/// License: CC BY-NC-SA 4.0 (same as Brave's usage).
const ENTITIES_JSON: &str = include_str!("entities.json");

/// Upstream URL for entity list auto-updates (refreshed whenever filter lists are rebuilt)
const ENTITIES_URL: &str =
    "https://raw.githubusercontent.com/disconnectme/disconnect-tracking-protection/master/entities.json";
const ENTITIES_FILE: &str = "entities.json";

// ============================================================================
// Metadata
// ============================================================================

/// Persisted metadata about filter lists
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilterListMeta {
    pub lists: Vec<FilterListInfo>,
    pub engine_built_at: u64,
    #[serde(default)]
    pub config_version: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilterListInfo {
    pub filename: String,
    pub url: String,
    pub downloaded_at: u64,
    pub size_bytes: u64,
    pub rule_count: usize,
    /// Unix timestamp when this list expires (downloaded_at + expiry from list header)
    #[serde(default)]
    pub expires_at: u64,
    /// The list's own `! Version:` header, if it has one
    #[serde(default)]
    pub version: Option<String>,
    /// The list's own `! Last modified:` header, if it has one
    #[serde(default)]
    pub last_modified: Option<String>,
    /// Consecutive failed refreshes (backoff); 0 after a successful download
    #[serde(default)]
    pub failures: u32,
}

impl Default for FilterListMeta {
    fn default() -> Self {
        Self {
            lists: Vec::new(),
            engine_built_at: 0,
            config_version: 0,
        }
    }
}

// ============================================================================
// Engine Status
// ============================================================================

/// Engine lifecycle status (reported via /health)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum EngineStatus {
    Loading,
    Ready,
    Error,
}

/// Full status info (reported via /status)
#[derive(Debug, Clone, Serialize)]
pub struct AdblockStatus {
    pub enabled: bool,
    pub status: EngineStatus,
    pub list_count: usize,
    pub total_rules: usize,
    pub last_update: u64,
    pub update_version: u64,
    pub lists: Vec<FilterListInfo>,
    pub scriptlets: ScriptletStatus,
}

/// Which scriptlet set the engine is running (reported via /status).
#[derive(Debug, Clone, Default, Serialize)]
pub struct ScriptletStatus {
    /// e.g. "uBO 1.75.0", "uBO 1.48.4 (fallback)", or "none"
    pub source: String,
    /// Resources from that source.
    pub count: usize,
    /// Hodos's bundled extras that were added because the source lacks them.
    pub extra: usize,
}

/// A scriptlet set as loaded, kept so a failed refresh can keep the previous one.
#[derive(Clone, Default)]
struct ScriptletSet {
    resources: Vec<Resource>,
    source: String,
    /// Function name -> uBO priority, for scriptlets whose calls must run first.
    priorities: HashMap<String, i64>,
}

impl ScriptletSet {
    fn from_converted(converted: &[crate::ubo_resources::Converted], source: String) -> Self {
        ScriptletSet {
            resources: converted.iter().map(crate::ubo_resources::Converted::to_resource).collect(),
            source,
            priorities: converted.iter().filter(|c| c.priority != 0)
                .filter_map(|c| c.function_name().map(|n| (n.to_string(), c.priority)))
                .collect(),
        }
    }
}

// ============================================================================
// AdblockEngine
// ============================================================================

/// Thread-safe ad & tracker blocking engine
///
/// Wraps `adblock::Engine` with global enable/disable toggle,
/// compilation from the cached lists, and filter list management.
pub struct AdblockEngine {
    engine: RwLock<Option<Engine>>,
    enabled: AtomicBool,
    status: RwLock<EngineStatus>,
    adblock_dir: PathBuf,
    meta: RwLock<FilterListMeta>,
    /// Monotonically increasing version — incremented on each engine rebuild.
    /// C++ uses this to detect when to invalidate its URL cache.
    update_version: AtomicU64,
    /// disconnect.me entity map: same-org domains are treated as first-party
    entity_map: RwLock<EntityMap>,
    /// The scriptlet set currently installed, and its status line.
    scriptlets: RwLock<(ScriptletSet, ScriptletStatus)>,
    /// Serializes `rebuild_engine` and `refresh_scriptlets`: each installs a set and then swaps
    /// or patches the engine; interleaved, the live engine and `scriptlets` could disagree.
    install_lock: tokio::sync::Mutex<()>,
}

// ============================================================================
// Extra Scriptlets (missing from old-format 1.48.4 scriptlets.js)
// ============================================================================

/// Scriptlets that were added to uBlock Origin AFTER the format change to ES
/// modules (post-1.48.x). The `assemble_scriptlet_resources()` parser can only
/// handle the old `///`-delimited format, so these are bundled as embedded JS
/// templates and registered individually via `engine.add_resource()`.
///
/// Each is a self-contained IIFE with `{{1}}`, `{{2}}` etc. template params
/// that the engine substitutes with filter rule arguments.
struct ExtraScriptlet {
    name: &'static str,
    aliases: &'static [&'static str],
    content: &'static str,
    /// true = PermissionMask(1) (trusted), false = PermissionMask(0)
    trusted: bool,
}

const EXTRA_SCRIPTLETS: &[ExtraScriptlet] = &[
    ExtraScriptlet {
        name: "trusted-replace-fetch-response.js",
        aliases: &["trusted-rpfr.js", "trusted-rpfr"],
        content: include_str!("scriptlets/trusted_replace_fetch_response.js"),
        trusted: true,
    },
    ExtraScriptlet {
        name: "trusted-replace-xhr-response.js",
        aliases: &["trusted-rpxr.js", "trusted-rpxr"],
        content: include_str!("scriptlets/trusted_replace_xhr_response.js"),
        trusted: true,
    },
    ExtraScriptlet {
        name: "json-prune-fetch-response.js",
        aliases: &["jpfr.js", "jpfr"],
        content: include_str!("scriptlets/json_prune_fetch_response.js"),
        trusted: false,
    },
    ExtraScriptlet {
        name: "json-prune-xhr-response.js",
        aliases: &["jpxr.js", "jpxr"],
        content: include_str!("scriptlets/json_prune_xhr_response.js"),
        trusted: false,
    },
    ExtraScriptlet {
        name: "trusted-replace-node-text.js",
        aliases: &["trusted-rpnt.js", "trusted-rpnt"],
        content: include_str!("scriptlets/trusted_replace_node_text.js"),
        trusted: true,
    },
    ExtraScriptlet {
        name: "remove-node-text.js",
        aliases: &["rmnt.js", "rmnt"],
        content: include_str!("scriptlets/remove_node_text.js"),
        trusted: false,
    },
];

/// Register the extra bundled scriptlets into the engine (called after
/// `use_resources()` so they supplement the old-format 1.48.4 base set).
fn load_extra_scriptlets(engine: &mut Engine) -> usize {
    let mut count = 0;
    for s in EXTRA_SCRIPTLETS {
        let resource = Resource {
            name: s.name.to_string(),
            aliases: s.aliases.iter().map(|a| a.to_string()).collect(),
            kind: ResourceType::Template,
            content: BASE64.encode(s.content),
            dependencies: vec![],
            permission: if s.trusted {
                PermissionMask::from_bits(1)
            } else {
                PermissionMask::default()
            },
        };
        if engine.add_resource(resource).is_ok() {
            count += 1;
        }
    }
    count
}

// ============================================================================
// Entity Map (disconnect.me same-organization domain grouping)
// ============================================================================

/// Maps domains to entity IDs so same-organization CDN resources are treated
/// as first-party. For example, `twimg.com` and `x.com` both belong to entity
/// "Twitter" — so `pbs.twimg.com` loaded on `x.com` is not blocked as a tracker.
struct EntityMap {
    /// domain → entity_id. Two domains with the same entity_id are same-org.
    domain_to_entity: HashMap<String, u16>,
}

impl EntityMap {
    /// Parse the disconnect.me entities.json format:
    /// `{ "license": "...", "entities": { "OrgName": { "properties": [...], "resources": [...] } } }`
    fn from_json(json_str: &str) -> Self {
        let mut domain_to_entity = HashMap::new();

        let parsed: serde_json::Value = match serde_json::from_str(json_str) {
            Ok(v) => v,
            Err(e) => {
                warn!("Entity map: failed to parse JSON: {}", e);
                return Self { domain_to_entity };
            }
        };

        let entities = match parsed.get("entities").and_then(|e| e.as_object()) {
            Some(e) => e,
            None => {
                warn!("Entity map: missing 'entities' key in JSON");
                return Self { domain_to_entity };
            }
        };

        let mut entity_id: u16 = 0;
        for (_org_name, org_data) in entities {
            let props = org_data.get("properties")
                .and_then(|v| v.as_array())
                .map(|a| a.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>())
                .unwrap_or_default();
            let resources = org_data.get("resources")
                .and_then(|v| v.as_array())
                .map(|a| a.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>())
                .unwrap_or_default();

            // Only assign an entity_id if there are actual domains
            if props.is_empty() && resources.is_empty() {
                continue;
            }

            for domain in props.iter().chain(resources.iter()) {
                let d = domain.to_lowercase();
                domain_to_entity.insert(d, entity_id);
            }

            entity_id = entity_id.saturating_add(1);
        }

        Self { domain_to_entity }
    }

    /// Check if two domains belong to the same organization.
    ///
    /// Uses suffix-walking: for "pbs.twimg.com", checks "pbs.twimg.com",
    /// then "twimg.com" — matches on the entity list's registered domain.
    fn is_same_entity(&self, domain_a: &str, domain_b: &str) -> bool {
        let a_lower = domain_a.to_lowercase();
        let b_lower = domain_b.to_lowercase();

        let id_a = self.lookup_entity(&a_lower);
        let id_b = self.lookup_entity(&b_lower);

        match (id_a, id_b) {
            (Some(a), Some(b)) => a == b,
            _ => false,
        }
    }

    /// Walk the domain suffix chain to find an entity match.
    /// "pbs.twimg.com" → try "pbs.twimg.com", then "twimg.com"
    fn lookup_entity(&self, domain: &str) -> Option<u16> {
        // Try exact match first
        if let Some(&id) = self.domain_to_entity.get(domain) {
            return Some(id);
        }
        // Walk suffixes: strip one label at a time
        let mut remaining = domain;
        while let Some(pos) = remaining.find('.') {
            remaining = &remaining[pos + 1..];
            // Don't look up bare TLDs (e.g. "com")
            if !remaining.contains('.') {
                break;
            }
            if let Some(&id) = self.domain_to_entity.get(remaining) {
                return Some(id);
            }
        }
        None
    }

    fn domain_count(&self) -> usize {
        self.domain_to_entity.len()
    }
}

/// Extract the hostname from a URL string.
/// "https://pbs.twimg.com/media/photo.jpg" → "pbs.twimg.com"
fn extract_domain(url: &str) -> String {
    let after_scheme = url.split("://").nth(1).unwrap_or(url);
    let host = after_scheme.split('/').next().unwrap_or(after_scheme);
    let host = host.split('?').next().unwrap_or(host);
    let host = host.split(':').next().unwrap_or(host); // strip port
    host.to_lowercase()
}

/// Load the entity list from disk (updatable) or fall back to embedded version.
fn load_entity_map(adblock_dir: &Path) -> EntityMap {
    let entity_path = adblock_dir.join(ENTITIES_FILE);
    if entity_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&entity_path) {
            if !content.trim().is_empty() {
                let map = EntityMap::from_json(&content);
                if map.domain_count() > 0 {
                    info!("Entity map: loaded {} domains from disk", map.domain_count());
                    return map;
                }
            }
        }
    }
    // Fall back to embedded version
    let map = EntityMap::from_json(ENTITIES_JSON);
    info!("Entity map: loaded {} domains from embedded list", map.domain_count());
    map
}

impl AdblockEngine {
    /// Create an engine in "loading" state. Call `load()` to initialize.
    pub fn new(adblock_dir: PathBuf) -> Self {
        // Load entity map eagerly (fast — just JSON parsing of embedded data)
        let entity_map = load_entity_map(&adblock_dir);
        Self {
            engine: RwLock::new(None),
            enabled: AtomicBool::new(true),
            status: RwLock::new(EngineStatus::Loading),
            adblock_dir,
            meta: RwLock::new(FilterListMeta::default()),
            update_version: AtomicU64::new(0),
            entity_map: RwLock::new(entity_map),
            scriptlets: RwLock::new(Default::default()),
            install_lock: tokio::sync::Mutex::new(()),
        }
    }

    /// Initialize the engine (async — downloads filter lists if needed).
    ///
    /// Call this after the HTTP server is already listening so /health
    /// can report "loading" while this runs.
    pub async fn load(&self) -> Result<(), String> {
        // Ensure directory structure exists
        let lists_dir = self.adblock_dir.join("lists");
        std::fs::create_dir_all(&lists_dir)
            .map_err(|e| format!("Failed to create adblock directory: {}", e))?;

        // Compile from the lists already on disk (no network, ~0.15 s for ~154k rules).
        // ⛔ Not from a serialized engine.dat: adblock 0.10.3's serialization drops each
        // filter's permission mask, so every trusted-list scriptlet silently stopped resolving
        // after a restart (B5-T0-P6; `test_adblock_serialization_drops_trusted_permission`).
        let meta_path = self.adblock_dir.join(META_JSON);
        let disk_meta: Option<FilterListMeta> = std::fs::read_to_string(&meta_path).ok()
            .and_then(|s| serde_json::from_str(&s).ok());

        let (mut engine, meta) = match disk_meta {
            Some(mut meta) if meta.config_version == CONFIG_VERSION => match compile_from_disk(&self.adblock_dir) {
                Ok((engine, loaded)) => {
                    info!("Ad blocker: compiled engine from {} cached lists", loaded.len());
                    reconcile_meta_with_disk(&mut meta, &loaded);
                    (engine, meta)
                }
                Err(e) => {
                    // Reuse every list still fresh on disk; download only what is missing.
                    warn!("Ad blocker: cannot compile from cached lists ({}), rebuilding...", e);
                    Self::build_from_lists(&self.adblock_dir, Some(&meta), &http_client()).await?
                }
            },
            Some(meta) => {
                info!("Ad blocker: config version changed ({} → {}), rebuilding...",
                    meta.config_version, CONFIG_VERSION);
                Self::build_from_lists(&self.adblock_dir, None, &http_client()).await?
            }
            None => {
                info!("Ad blocker: no list metadata, downloading filter lists...");
                Self::build_from_lists(&self.adblock_dir, None, &http_client()).await?
            }
        };

        // Scriptlets are not part of the compiled lists; install them on every load — from disk only,
        // so startup never waits on GitHub (`refresh_scriptlets` fetches afterwards).
        self.install_scriptlets(&mut engine).await;

        // Store the engine
        {
            let mut eng = self.engine.write().unwrap();
            *eng = Some(engine);
        }
        {
            let mut m = self.meta.write().unwrap();
            *m = meta;
        }
        {
            let mut s = self.status.write().unwrap();
            *s = EngineStatus::Ready;
        }
        self.update_version.store(1, Ordering::Relaxed);

        Ok(())
    }

    /// Check if a network request should be blocked.
    ///
    /// Returns `(blocked, redirect, filter_rule)`.
    pub fn check_request(
        &self,
        url: &str,
        source_url: &str,
        resource_type: &str,
    ) -> (bool, Option<String>, Option<String>) {
        if !self.enabled.load(Ordering::Relaxed) {
            return (false, None, None);
        }

        let engine_guard = self.engine.read().unwrap();
        let engine = match engine_guard.as_ref() {
            Some(eng) => eng,
            None => return (false, None, None), // Engine not loaded yet
        };

        // Build the adblock Request
        let request = match Request::new(url, source_url, resource_type) {
            Ok(req) => req,
            Err(e) => {
                warn!("Ad blocker: invalid request URL '{}': {}", url, e);
                return (false, None, None);
            }
        };

        let result = engine.check_network_request(&request);

        // Entity-aware override: if the engine says "block" but the URL and
        // source belong to the same organization (e.g. twimg.com on x.com),
        // allow the request. Only overrides blocks, not redirects (which are
        // scriptlet-related and should always apply).
        if result.matched && result.redirect.is_none() {
            let url_domain = extract_domain(url);
            let source_domain = extract_domain(source_url);
            if !url_domain.is_empty() && !source_domain.is_empty() {
                let entity_guard = self.entity_map.read().unwrap();
                if entity_guard.is_same_entity(&url_domain, &source_domain) {
                    return (false, None, None);
                }
            }
        }

        (
            result.matched,
            result.redirect.clone(),
            result.filter.clone(),
        )
    }

    /// Get cosmetic filtering resources for a URL.
    ///
    /// Returns (hide_selectors, injected_script, generichide).
    pub fn cosmetic_resources(&self, url: &str) -> (Vec<String>, String, bool) {
        if !self.enabled.load(Ordering::Relaxed) {
            return (Vec::new(), String::new(), false);
        }

        let engine_guard = self.engine.read().unwrap();
        let engine = match engine_guard.as_ref() {
            Some(eng) => eng,
            None => return (Vec::new(), String::new(), false),
        };

        let resources = engine.url_cosmetic_resources(url);

        // When generichide is true (set by @@||domain^$elemhide rules), suppress ALL
        // cosmetic CSS selectors — both hostname-specific and generic. adblock-rust sets
        // the generichide flag but may still return hostname-specific selectors; we clear
        // them here because $elemhide means "no cosmetic CSS filtering at all".
        // Scriptlets (injected_script) are handled separately by #@#+js() exceptions.
        let selectors: Vec<String> = if resources.generichide {
            Vec::new()
        } else {
            resources.hide_selectors.into_iter().collect()
        };

        // Lock order: engine (held above), then scriptlets — as everywhere else.
        let script = order_by_priority(&resources.injected_script, &self.scriptlets.read().unwrap().0.priorities);
        (selectors, wrap_scriptlets(&script), resources.generichide)
    }

    /// Get generic cosmetic selectors matching DOM class names and IDs.
    ///
    /// Two-phase cosmetic filtering:
    /// 1. `cosmetic_resources(url)` returns hostname-specific selectors
    /// 2. This method returns generic selectors matching actual DOM elements
    ///
    /// Internally calls `url_cosmetic_resources(url)` to get the exception set,
    /// then `hidden_class_id_selectors(classes, ids, exceptions)` for matching.
    pub fn hidden_class_id_selectors(
        &self,
        url: &str,
        classes: &[String],
        ids: &[String],
    ) -> Vec<String> {
        if !self.enabled.load(Ordering::Relaxed) {
            return Vec::new();
        }

        let engine_guard = self.engine.read().unwrap();
        let engine = match engine_guard.as_ref() {
            Some(eng) => eng,
            None => return Vec::new(),
        };

        // Get exceptions and generichide flag for this URL
        let resources = engine.url_cosmetic_resources(url);
        if resources.generichide {
            return Vec::new(); // Site suppresses generic cosmetic rules
        }

        engine.hidden_class_id_selectors(classes, ids, &resources.exceptions)
    }

    /// Get whether the engine is globally enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    /// Set global enable/disable
    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Relaxed);
        info!("Ad blocker: {}", if enabled { "enabled" } else { "disabled" });
    }

    /// `load()` failed (e.g. a first start with no network): `Error`, so the update check retries
    /// the load instead of waiting for a `Ready` that will never come.
    pub fn mark_load_failed(&self) {
        *self.status.write().unwrap() = EngineStatus::Error;
    }

    /// Get current lifecycle status
    pub fn get_engine_status(&self) -> EngineStatus {
        *self.status.read().unwrap()
    }

    /// Get full status info
    pub fn get_status(&self) -> AdblockStatus {
        let meta = self.meta.read().unwrap();
        AdblockStatus {
            enabled: self.is_enabled(),
            status: self.get_engine_status(),
            list_count: meta.lists.len(),
            total_rules: meta.lists.iter().map(|l| l.rule_count).sum(),
            last_update: meta.engine_built_at,
            update_version: self.get_update_version(),
            lists: meta.lists.clone(),
            scriptlets: self.scriptlets.read().unwrap().1.clone(),
        }
    }

    /// Rebuild the engine from downloaded filter lists (hot-swap).
    /// Used by the background update task.
    pub async fn rebuild_engine(&self) -> Result<(), String> {
        let _install = self.install_lock.lock().await;
        let previous = self.meta.read().unwrap().clone();
        let (mut engine, meta) = Self::build_from_lists(&self.adblock_dir, Some(&previous), &http_client()).await?;
        // Scriptlets from disk: fetching the pin is refresh_scriptlets()' job, run on the same
        // check — fetching here too would download uBO twice when it is unavailable.
        self.install_scriptlets(&mut engine).await;

        {
            let mut eng = self.engine.write().unwrap();
            *eng = Some(engine);
        }
        {
            let mut m = self.meta.write().unwrap();
            *m = meta;
        }

        // Reload entity map (may have been updated on disk by build_from_lists)
        let new_entity_map = load_entity_map(&self.adblock_dir);
        {
            let mut em = self.entity_map.write().unwrap();
            *em = new_entity_map;
        }

        let ver = self.update_version.fetch_add(1, Ordering::Relaxed) + 1;
        info!("Ad blocker: engine rebuilt successfully (version {})", ver);
        Ok(())
    }

    /// Check if any filter list has expired and needs re-downloading.
    pub fn needs_update(&self) -> bool {
        let meta = self.meta.read().unwrap();
        if meta.lists.is_empty() {
            return false;
        }
        // A configured list that never downloaded is missing from meta: retry it rather than
        // waiting for an unrelated list to expire.
        let missing = configured_lists().iter()
            .any(|(f, _, _)| !meta.lists.iter().any(|l| l.filename == *f));
        let now = now_secs();
        // Due within DUE_SLACK_SECS counts as due: a retry window equal to the check interval
        // (e.g. the 1 h backoff step) must not slip a whole interval on timing drift.
        missing || meta.lists.iter().any(|l| l.expires_at > 0 && now + DUE_SLACK_SECS >= l.expires_at)
    }

    /// Get the current update version (monotonically increasing on each rebuild).
    pub fn get_update_version(&self) -> u64 {
        self.update_version.load(Ordering::Relaxed)
    }

    /// Install the best available scriptlet set into `engine`, then Hodos's extras for any
    /// name the set lacks.
    ///
    /// Preference: uBO's pinned current release (`ubo_resources`), else the old 1.48.4
    /// single-file set. A refresh never installs a smaller set than the one already running
    /// (P6-A6): if the fresh set is smaller, or none is available, the previous one is kept.
    ///
    /// Disk only, so neither `load()` nor a rebuild waits on GitHub; `refresh_scriptlets()` is
    /// the one place that fetches.
    async fn install_scriptlets(&self, engine: &mut Engine) {
        let fresh = obtain_scriptlets(&self.adblock_dir.join("resources"), false).await;
        self.apply_scriptlets(engine, fresh);
    }

    /// Put `fresh` (or the running set, if `fresh` is missing or smaller) into `engine`, then
    /// the extras, and record what was installed. No I/O.
    fn apply_scriptlets(&self, engine: &mut Engine, fresh: Option<ScriptletSet>) {
        let chosen = choose_scriptlets(fresh, &self.scriptlets.read().unwrap().0);
        engine.use_resources(chosen.resources.clone());
        let extra = load_extra_scriptlets(engine);
        info!("Ad blocker: scriptlets: {} ({} resources, {} bundled extras)",
            chosen.source, chosen.resources.len(), extra);
        let status = ScriptletStatus { source: chosen.source.clone(), count: chosen.resources.len(), extra };
        *self.scriptlets.write().unwrap() = (chosen, status);
    }

    /// If the engine is not running uBO's pinned set (first start after an upgrade, a lost
    /// cache, a fetch that failed), fetch it and swap it into the live engine. Called after
    /// `load()` and on every hourly check; a no-op once the pinned set runs.
    pub async fn refresh_scriptlets(&self) {
        let _install = self.install_lock.lock().await;
        let pinned = pinned_scriptlet_source();
        if self.scriptlets.read().unwrap().0.source == pinned {
            return;
        }
        let Some(fresh) = obtain_scriptlets(&self.adblock_dir.join("resources"), true).await else {
            return;
        };
        if fresh.source != pinned && !self.scriptlets.read().unwrap().0.resources.is_empty() {
            return; // the pin is still unavailable; keep what runs
        }
        {
            // Lock order: engine, then scriptlets (apply_scriptlets) — as everywhere else.
            let mut guard = self.engine.write().unwrap();
            match guard.as_mut() {
                Some(engine) => self.apply_scriptlets(engine, Some(fresh)),
                None => return,
            }
        }
        let ver = self.update_version.fetch_add(1, Ordering::Relaxed) + 1;
        info!("Ad blocker: scriptlets refreshed to {} (version {})", pinned, ver);
    }

    // ========================================================================
    // Private helpers
    // ========================================================================

    /// Download filter lists and build engine from scratch
    /// With `previous` (the running meta), a list that has not expired is reused from disk with
    /// its recorded metadata; only expired or missing lists are downloaded. Without it (first
    /// start, config change), every list is downloaded.
    async fn build_from_lists(
        adblock_dir: &Path,
        previous: Option<&FilterListMeta>,
        client: &reqwest::Client,
    ) -> Result<(Engine, FilterListMeta), String> {
        let lists_dir = adblock_dir.join("lists");
        let resources_dir = adblock_dir.join("resources");
        std::fs::create_dir_all(&lists_dir)
            .map_err(|e| format!("Failed to create lists directory: {}", e))?;
        std::fs::create_dir_all(&resources_dir)
            .map_err(|e| format!("Failed to create resources directory: {}", e))?;

        let mut filter_set = FilterSet::new(false); // false = no debug info
        let mut list_infos = Vec::new();
        let now = now_secs();

        // Standard lists (EasyList, EasyPrivacy) with default permissions; uBlock's lists trusted.
        for (filename, urls, opts) in configured_lists() {
            let prev = previous.and_then(|m| m.lists.iter().find(|l| l.filename == filename));
            // A placeholder not yet due keeps its backoff: another list's expiry is no reason
            // to try its dead mirrors again.
            if let Some(p) = prev.filter(|p| p.downloaded_at == 0 && p.expires_at > now + DUE_SLACK_SECS) {
                if !lists_dir.join(filename).exists() {
                    list_infos.push(p.clone());
                    continue;
                }
            }
            match obtain_list(client, &lists_dir, filename, urls, prev, now).await {
                Some((text, info)) => {
                    add_list(&mut filter_set, &text, opts);
                    info!("Ad blocker: loaded {} ({} rules, {} KB)", filename, info.rule_count, info.size_bytes / 1024);
                    list_infos.push(info);
                }
                None => {
                    // Never downloaded, nothing cached: a placeholder (0 rules, downloaded_at 0)
                    // retried with the same backoff as a failed refresh, so an unreachable list
                    // does not force a full rebuild every hour.
                    let failures = prev.map_or(0, |p| p.failures).saturating_add(1);
                    let retry = retry_after(failures);
                    warn!("Ad blocker: {} unavailable; failure {}, retrying in {} s", filename, failures, retry);
                    let mut placeholder = list_info(filename, urls[0], "", 0, now + retry);
                    placeholder.failures = failures;
                    list_infos.push(placeholder);
                }
            }
        }

        if list_infos.iter().all(|l| l.rule_count == 0) {
            return Err("No filter lists available — cannot build engine".to_string());
        }

        // Load Hodos compatibility exceptions (embedded + optional updatable override)
        let unbreak_text = load_unbreak_list(adblock_dir);
        let unbreak_rule_count = unbreak_text.lines().filter(|l| !l.starts_with('!') && !l.trim().is_empty()).count();
        filter_set.add_filter_list(&unbreak_text, ParseOptions::default());
        info!("Ad blocker: loaded hodos-unbreak ({} rules)", unbreak_rule_count);

        // Compile the engine (optimize = true for better performance).
        // Scriptlets are installed by the caller (`install_scriptlets`).
        let engine = Engine::from_filter_set(filter_set, true);

        // Download updated entity list (disconnect.me same-org domain mapping)
        // Non-critical: if download fails, embedded version is used at runtime
        match download_filter_list(client, ENTITIES_URL).await {
            Ok(text) => {
                // Validate JSON before saving
                if serde_json::from_str::<serde_json::Value>(&text).is_ok() {
                    let entity_path = adblock_dir.join(ENTITIES_FILE);
                    if let Err(e) = std::fs::write(&entity_path, &text) {
                        warn!("Ad blocker: failed to save entities.json: {}", e);
                    } else {
                        info!("Ad blocker: updated entities.json ({} KB)", text.len() / 1024);
                    }
                } else {
                    warn!("Ad blocker: downloaded entities.json is not valid JSON, skipping");
                }
            }
            Err(e) => {
                warn!("Ad blocker: failed to download entities.json: {} (using embedded fallback)", e);
            }
        }

        // Save metadata
        let meta = FilterListMeta {
            lists: list_infos,
            engine_built_at: now,
            config_version: CONFIG_VERSION,
        };
        let meta_path = adblock_dir.join(META_JSON);
        if let Err(e) = std::fs::write(
            &meta_path,
            serde_json::to_string_pretty(&meta).unwrap_or_default(),
        ) {
            warn!("Ad blocker: failed to save meta.json: {}", e);
        }

        Ok((engine, meta))
    }
}

// ============================================================================
// Helpers
// ============================================================================

/// Wrap the engine's injected script in a private scope, as uBO's own injector does
/// (`scriptlet-filtering-core.js`, 1.75.0):
///
/// - function-style scriptlets arrive as top-level `function` declarations; unwrapped, they
///   would become page globals (`window.safeSelf`…) — a tell a site can test for;
/// - current scriptlets read `scriptletGlobals` (`safe-self.js`), which the injector must
///   define; without it they throw a ReferenceError inside their try/catch and do nothing.
///
/// Empty stays empty, so the shell still skips injection when there is nothing to inject.
pub(crate) fn wrap_scriptlets(script: &str) -> String {
    if script.is_empty() {
        return String::new();
    }
    format!("(function() {{\nconst scriptletGlobals = {{}};\n{}\n}})();", script)
}

/// Compile the engine from the lists already in `<adblock_dir>/lists` — the same lists, trust
/// levels and unbreak list as `build_from_lists`, with no network. A list missing on disk is
/// skipped (the update check retries it); `Err` only if none is there. Returns the engine and
/// the filenames actually compiled.
fn compile_from_disk(adblock_dir: &Path) -> Result<(Engine, Vec<&'static str>), String> {
    let lists_dir = adblock_dir.join("lists");
    let mut filter_set = FilterSet::new(false);
    let mut loaded = Vec::new();
    for (filename, _, opts) in configured_lists() {
        match std::fs::read_to_string(lists_dir.join(filename)) {
            Ok(text) => {
                add_list(&mut filter_set, &text, opts);
                loaded.push(filename);
            }
            Err(e) => warn!("Ad blocker: {} not cached ({}); compiling without it", filename, e),
        }
    }
    if loaded.is_empty() {
        return Err("no cached lists".into());
    }
    filter_set.add_filter_list(&load_unbreak_list(adblock_dir), ParseOptions::default());
    Ok((Engine::from_filter_set(filter_set, true), loaded))
}

/// After compiling from disk: a list meta still calls fresh but that was NOT on disk (deleted,
/// quarantined, a failed write) is marked expired with 0 rules, so the first update check
/// re-fetches it instead of waiting out its recorded expiry.
fn reconcile_meta_with_disk(meta: &mut FilterListMeta, loaded: &[&str]) {
    // A placeholder (downloaded_at 0) is never on disk: it keeps its own backoff.
    for l in meta.lists.iter_mut().filter(|l| l.downloaded_at > 0 && !loaded.contains(&l.filename.as_str())) {
        l.expires_at = 1;
        l.rule_count = 0;
    }
}

/// Order the calls as uBO does (`scriptlet-filtering-core.js :: sortedCalls`): higher priority
/// first, then by call text. adblock 0.10.3 collects calls in a std `HashMap`, so without this
/// their order — and the nesting of scriptlets that wrap the same API — changes per request.
///
/// adblock-rust's script is the dependency code followed by one block per call, each exactly
/// `"try {\n" + call + "\n} catch ( e ) { }\n"` with the call on one line. Blocks are peeled off
/// the end; anything that does not match that shape stops the parse (old template-style
/// scriptlets span several lines and are left as they are). Ties compare bytes, where uBO uses
/// `localeCompare`: the point is a fixed order.
pub(crate) fn order_by_priority(script: &str, priorities: &HashMap<String, i64>) -> String {
    const HEAD: &str = "try {\n";
    const TAIL: &str = "\n} catch ( e ) { }\n";
    let mut rest = script;
    let mut calls: Vec<&str> = Vec::new();
    while rest.ends_with(TAIL) {
        let body_end = rest.len() - TAIL.len();
        let Some(start) = rest[..body_end].rfind(HEAD) else { break };
        let call = &rest[start + HEAD.len()..body_end];
        if call.contains('\n') {
            break;
        }
        calls.push(call);
        rest = &rest[..start];
    }
    if calls.len() < 2 {
        return script.to_string();
    }
    let prio = |call: &str| call.split('(').next().and_then(|n| priorities.get(n.trim())).copied().unwrap_or(0);
    calls.sort_by(|a, b| prio(b).cmp(&prio(a)).then_with(|| a.cmp(b)));
    let mut out = rest.to_string();
    for call in calls {
        out.push_str(HEAD);
        out.push_str(call);
        out.push_str(TAIL);
    }
    out
}

/// HTTP client for list and resource downloads.
fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
}

/// Pick the set to install: uBO's pinned set always (a newer pin may merge scriptlets and be
/// smaller — counts across versions mean nothing); otherwise the fresh set unless it is
/// missing or smaller than `current`.
fn choose_scriptlets(fresh: Option<ScriptletSet>, current: &ScriptletSet) -> ScriptletSet {
    match fresh {
        Some(f) if f.source == pinned_scriptlet_source() => f,
        // Never replace a running pinned set with an older or fallback one.
        Some(_) if current.source == pinned_scriptlet_source() => current.clone(),
        Some(f) if f.resources.len() >= current.resources.len() => f,
        Some(f) => {
            warn!("Ad blocker: {} has {} scriptlets, fewer than the running {} ({}); keeping the running set",
                f.source, f.resources.len(), current.source, current.resources.len());
            current.clone()
        }
        None if !current.resources.is_empty() => {
            warn!("Ad blocker: no scriptlet set available; keeping the running {}", current.source);
            current.clone()
        }
        None => ScriptletSet { source: "none".into(), ..Default::default() },
    }
}

/// The `ScriptletSet::source` of uBO's pinned current set.
fn pinned_scriptlet_source() -> String {
    format!("uBO {}", crate::ubo_resources::UBO_TAG)
}

/// The best scriptlet set obtainable now: uBO's pinned current release, else the 1.48.4
/// single-file set. `None` if neither is available. With `network: false` only what is
/// already on disk is used (the converted cache, the 1.48.4 file) — nothing is downloaded.
async fn obtain_scriptlets(resources_dir: &Path, network: bool) -> Option<ScriptletSet> {
    if let Err(e) = std::fs::create_dir_all(resources_dir) {
        warn!("Ad blocker: cannot create {}: {}", resources_dir.display(), e);
    }
    let client = http_client();
    let pinned = if network {
        crate::ubo_resources::obtain(resources_dir, &client).await
    } else {
        crate::ubo_resources::cached(resources_dir).ok_or_else(|| "not cached yet".to_string())
    };
    match pinned {
        Ok(converted) => return Some(ScriptletSet::from_converted(&converted, pinned_scriptlet_source())),
        Err(e) => warn!("Ad blocker: uBO {} scriptlets unavailable: {}", crate::ubo_resources::UBO_TAG, e),
    }
    // After a pin bump that could not fetch yet: the last converted set (hash-verified when it
    // was written) beats the 2023 single-file set. refresh_scriptlets() keeps trying the pin.
    if let Some((tag, converted)) = crate::ubo_resources::cached_any(resources_dir) {
        return Some(ScriptletSet::from_converted(&converted, format!("uBO {} (cached; pin {} not fetched yet)", tag, crate::ubo_resources::UBO_TAG)));
    }

    let resources = obtain_legacy(resources_dir, network.then_some((&client, SCRIPTLETS_URL))).await;
    if !resources.is_empty() {
        return Some(ScriptletSet { resources, source: "uBO 1.48.4 (fallback)".into(), priorities: HashMap::new() });
    }
    None
}

/// The 1.48.4 single-file set: used if present and readable; downloaded (when `download` is
/// given) if missing or unreadable. A download that yields no scriptlets is not kept — a portal
/// page saved as scriptlets.js would otherwise disable this fallback for good.
async fn obtain_legacy(resources_dir: &Path, download: Option<(&reqwest::Client, &str)>) -> Vec<Resource> {
    let legacy = resources_dir.join(SCRIPTLETS_FILE);
    // adblock's assembler panics (`expect`) on an unreadable or malformed file; contain it.
    let assemble = |p: &Path| {
        if !p.exists() {
            return Vec::new();
        }
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| assemble_scriptlet_resources(p)))
            .unwrap_or_else(|_| {
                warn!("Ad blocker: {} is unreadable as uBO 1.48.4 scriptlets", p.display());
                Vec::new()
            })
    };
    let mut resources = assemble(&legacy);
    if let (true, Some((client, url))) = (resources.is_empty(), download) {
        match download_filter_list(client, url).await {
            Ok(text) => {
                let tmp = resources_dir.join("scriptlets.js.download");
                if std::fs::write(&tmp, &text).is_ok() {
                    resources = assemble(&tmp);
                    if resources.is_empty() {
                        warn!("Ad blocker: downloaded scriptlets.js has no scriptlets; not kept");
                        let _ = std::fs::remove_file(&tmp);
                    } else if let Err(e) = std::fs::rename(&tmp, &legacy) {
                        warn!("Ad blocker: failed to save scriptlets.js: {}", e);
                    }
                }
            }
            Err(e) => warn!("Ad blocker: failed to download scriptlets.js: {}", e),
        }
    }
    resources
}

/// Parse `! Expires: N days` or `! Expires: N hours` from a filter list header.
/// Returns expiry duration in seconds. Defaults to 4 days if not found.
fn parse_list_expiry(list_text: &str) -> u64 {
    // Only scan the first 50 lines (header section)
    for line in list_text.lines().take(50) {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("! Expires:") {
            let rest = rest.trim();
            // Parse "N days" or "N hours"
            let parts: Vec<&str> = rest.split_whitespace().collect();
            if parts.len() >= 2 {
                if let Ok(n) = parts[0].parse::<u64>() {
                    let unit = parts[1].to_lowercase();
                    if unit.starts_with("day") {
                        return n * 24 * 3600;
                    } else if unit.starts_with("hour") {
                        return n * 3600;
                    }
                }
            }
        }
    }
    DEFAULT_EXPIRY_SECS
}

/// Metadata for a list's text as downloaded (or reused) at `downloaded_at`.
fn list_info(filename: &str, url: &str, text: &str, downloaded_at: u64, expires_at: u64) -> FilterListInfo {
    FilterListInfo {
        filename: filename.to_string(),
        url: url.to_string(),
        downloaded_at,
        size_bytes: text.len() as u64,
        rule_count: text.lines().count(),
        expires_at,
        version: parse_list_header(text, "Version"),
        last_modified: parse_list_header(text, "Last modified"),
        failures: 0,
    }
}

/// Add a list to the filter set after dropping the `!#if` branches that do not apply to Hodos
/// (`preparser`). Every path that compiles a list goes through here.
fn add_list(filter_set: &mut FilterSet, text: &str, opts: ParseOptions) {
    filter_set.add_filter_list(&crate::preparser::prune(text, crate::preparser::HODOS_ENV), opts);
}

/// Reject a download that is not a filter list: a captive portal or error page answers 200
/// with HTML, and writing that over the cached list would switch blocking off. A list starts
/// with `[Adblock …]` or `! Title:`; an HTML page does not. (No size-ratio rule: a list that
/// legitimately shrinks must still replace its cache.)
fn check_plausible(text: &str) -> Result<(), String> {
    if !text.lines().take(20).any(|l| l.starts_with("! Title:") || l.starts_with("[Adblock")) {
        return Err("no filter-list header in the first 20 lines".into());
    }
    Ok(())
}

/// One configured list's text and metadata:
/// - `prev` not expired and the file on disk: reuse both, no download;
/// - else try each URL in turn (mirrors); a plausible answer is saved and used;
/// - else the cached file, retried with backoff (`retry_after`), never a full fresh expiry;
/// - else `None`: the caller records a 0-rule placeholder with the same backoff.
async fn obtain_list(
    client: &reqwest::Client,
    lists_dir: &Path,
    filename: &str,
    urls: &[&str],
    prev: Option<&FilterListInfo>,
    now: u64,
) -> Option<(String, FilterListInfo)> {
    let path = lists_dir.join(filename);
    let cached = std::fs::read_to_string(&path).ok();
    if let (Some(p), Some(text)) = (prev, &cached) {
        // A placeholder (downloaded_at 0) is never "fresh": a file that has appeared since must
        // get real metadata, so try the download and fall back to the file.
        if p.expires_at > now && p.downloaded_at > 0 {
            return Some((text.clone(), p.clone()));
        }
    }
    for url in urls {
        match download_filter_list(client, url).await {
            Ok(text) => match check_plausible(&text) {
                Ok(()) => {
                    // Temp file + rename: an interrupted write must not leave a truncated
                    // list that the next start would load as fresh.
                    let tmp = path.with_extension("txt.download");
                    if let Err(e) = std::fs::write(&tmp, &text).and_then(|_| std::fs::rename(&tmp, &path)) {
                        warn!("Ad blocker: failed to save {}: {}", filename, e);
                        let _ = std::fs::remove_file(&tmp);
                    }
                    let expires_at = now + parse_list_expiry(&text);
                    let info = list_info(filename, url, &text, now, expires_at);
                    return Some((text, info));
                }
                Err(e) => warn!("Ad blocker: rejected {} from {}: {}", filename, url, e),
            },
            Err(e) => warn!("Ad blocker: failed to download {} from {}: {}", filename, url, e),
        }
    }
    let text = cached?;
    let failures = prev.map_or(0, |p| p.failures).saturating_add(1);
    let retry = retry_after(failures);
    warn!("Ad blocker: using cached {}; failure {}, retrying in {} s", filename, failures, retry);
    let url = prev.map(|p| p.url.clone()).unwrap_or_else(|| urls[0].to_string());
    let downloaded_at = prev.map(|p| p.downloaded_at).filter(|d| *d > 0).unwrap_or(now);
    let mut info = list_info(filename, &url, &text, downloaded_at, now + retry);
    info.failures = failures;
    Some((text, info))
}

/// Retry delay after `failures` consecutive failed refreshes: 30 min, 1 h, 2 h, 4 h, then 6 h.
/// The first retry lands within the next hourly check; an unreachable list does not cost a
/// full rebuild every hour.
fn retry_after(failures: u32) -> u64 {
    let doubled = RETRY_AFTER_FAILURE_SECS.saturating_mul(1u64 << failures.saturating_sub(1).min(16));
    doubled.min(MAX_RETRY_SECS)
}

/// Read `! <key>: value` from a filter list header (first 50 lines). Unfilled template
/// placeholders such as uAssets' raw `%timestamp%` are treated as absent.
fn parse_list_header(list_text: &str, key: &str) -> Option<String> {
    let prefix = format!("! {}:", key);
    list_text.lines().take(50).find_map(|line| {
        let v = line.trim().strip_prefix(&prefix)?.trim();
        (!v.is_empty() && !v.contains('%')).then(|| v.to_string())
    })
}

/// Download a filter list from URL, refusing bodies over `MAX_LIST_BYTES` as they arrive.
async fn download_filter_list(client: &reqwest::Client, url: &str) -> Result<String, String> {
    let mut response = client.get(url)
        .send()
        .await
        .map_err(|e| format!("HTTP error: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("HTTP {} from {}", response.status(), url));
    }

    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|e| format!("Failed to read response body: {}", e))? {
        if bytes.len() + chunk.len() > MAX_LIST_BYTES {
            return Err(format!("{} is larger than {} bytes", url, MAX_LIST_BYTES));
        }
        bytes.extend_from_slice(&chunk);
    }
    // Lenient like reqwest's text(): a stray invalid byte must not reject a whole list, and a
    // leading byte-order mark must not hide the `! Title:` header.
    let text = String::from_utf8_lossy(&bytes);
    Ok(text.strip_prefix('\u{feff}').unwrap_or(&text).to_string())
}

/// Load the Hodos compatibility exception list.
/// Checks for an updatable version on disk first; falls back to the embedded version.
fn load_unbreak_list(adblock_dir: &Path) -> String {
    let unbreak_path = adblock_dir.join("hodos-unbreak.txt");
    if unbreak_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&unbreak_path) {
            if !content.trim().is_empty() {
                info!("Ad blocker: using updatable hodos-unbreak.txt from disk");
                return content;
            }
        }
    }
    // Fall back to embedded version
    HODOS_UNBREAK.to_string()
}

/// Current Unix timestamp in seconds
fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn app_dir_name() -> &'static str {
        match std::env::var("HODOS_DEV").as_deref() {
            Ok("1") => "HodosBrowserDev",
            _ => "HodosBrowser",
        }
    }

    /// Helper: build an engine from raw EasyList-style filter rules
    fn engine_from_rules(rules: &str) -> Engine {
        let mut filter_set = FilterSet::new(false);
        filter_set.add_filter_list(rules, ParseOptions::default());
        Engine::from_filter_set(filter_set, true)
    }

    #[test]
    fn test_basic_ad_blocking() {
        let engine = engine_from_rules("||ads.example.com^\n||tracker.net/pixel");

        let req = Request::new("https://ads.example.com/banner.js", "https://news.com", "script").unwrap();
        assert!(engine.check_network_request(&req).matched, "ads.example.com should be blocked");

        let req = Request::new("https://tracker.net/pixel?id=123", "https://news.com", "image").unwrap();
        assert!(engine.check_network_request(&req).matched, "tracker.net/pixel should be blocked");

        let req = Request::new("https://safe.example.com/page.html", "https://news.com", "document").unwrap();
        assert!(!engine.check_network_request(&req).matched, "safe.example.com should NOT be blocked");
    }

    #[test]
    fn test_exception_rules() {
        let rules = "||ads.example.com^\n@@||ads.example.com/allowed.js";
        let engine = engine_from_rules(rules);

        let req = Request::new("https://ads.example.com/banner.js", "https://news.com", "script").unwrap();
        assert!(engine.check_network_request(&req).matched, "banner.js should be blocked");

        let req = Request::new("https://ads.example.com/allowed.js", "https://news.com", "script").unwrap();
        assert!(!engine.check_network_request(&req).matched, "allowed.js should be excepted");
    }

    #[test]
    fn test_resource_type_filtering() {
        let rules = "||cdn.example.com^$script";
        let engine = engine_from_rules(rules);

        let req = Request::new("https://cdn.example.com/app.js", "https://site.com", "script").unwrap();
        assert!(engine.check_network_request(&req).matched, "script type should be blocked");

        let req = Request::new("https://cdn.example.com/logo.png", "https://site.com", "image").unwrap();
        assert!(!engine.check_network_request(&req).matched, "image type should NOT be blocked by script-only rule");
    }

    /// CRITICAL: Verify that `#@#+js()` blanket exception syntax is supported
    /// by adblock-rust 0.10.3. If this test fails, we need a C++ domain-check bypass instead.
    #[test]
    fn test_scriptlet_exception_blanket_js() {
        // Add a scriptlet rule for youtube.com AND a blanket exception for x.com
        let rules = r#"
youtube.com##+js(set, ytInitialPlayerResponse.adPlacements, undefined)
x.com#@#+js()
twitter.com#@#+js()
"#;
        let mut filter_set = FilterSet::new(false);
        filter_set.add_filter_list(rules, ParseOptions::default());
        let engine = Engine::from_filter_set(filter_set, true);

        // x.com should have EMPTY injected_script due to #@#+js() exception
        let xcom_resources = engine.url_cosmetic_resources("https://x.com/home");
        assert!(
            xcom_resources.injected_script.is_empty(),
            "x.com should have no scriptlets due to #@#+js() exception, got: {}",
            &xcom_resources.injected_script[..std::cmp::min(100, xcom_resources.injected_script.len())]
        );

        // twitter.com should also have EMPTY injected_script
        let twitter_resources = engine.url_cosmetic_resources("https://twitter.com/home");
        assert!(
            twitter_resources.injected_script.is_empty(),
            "twitter.com should have no scriptlets due to #@#+js() exception"
        );

        // youtube.com should STILL have scriptlets (no exception for it)
        let yt_resources = engine.url_cosmetic_resources("https://www.youtube.com/watch?v=test");
        // Note: The scriptlet may be empty if resources aren't loaded, but at minimum
        // no exception should suppress it. We check it's not suppressed by an exception.
        // The actual injected_script content depends on whether scriptlet resources are loaded.
        // The key verification is that x.com IS empty.
        let _ = yt_resources; // youtube check is informational
    }

    /// Test that generichide flag suppresses ALL cosmetic CSS selectors (Phase 1 + Phase 2).
    /// This was the root cause of x.com media being hidden — EasyList hostname-specific
    /// CSS selectors were being injected even when generichide was set (from $elemhide/$generichide).
    /// In production, x.com gets generichide=1 from filter lists; our wrapper must clear
    /// hostname-specific selectors when this flag is set.
    #[test]
    fn test_generichide_suppresses_all_css_selectors() {
        // Use $generichide (which adblock-rust 0.10.3 directly supports) to test our wrapper.
        // In production, @@||x.com^$elemhide and EasyList rules both set generichide=true.
        let rules = r#"
x.com##.promoted-tweet
x.com##.ad-container
x.com##div[data-testid="promoted"]
@@||x.com^$generichide
youtube.com##.ad-showing
"#;
        // Our cosmetic_resources() wrapper should return EMPTY selectors when generichide=true
        let adblock = AdblockEngine::new(PathBuf::from("/tmp/test-elemhide"));
        {
            let mut engine_guard = adblock.engine.write().unwrap();
            let mut filter_set = FilterSet::new(false);
            filter_set.add_filter_list(rules, ParseOptions::default());
            *engine_guard = Some(Engine::from_filter_set(filter_set, true));
        }

        let (selectors, _, generichide) = adblock.cosmetic_resources("https://x.com/home");
        assert!(generichide, "generichide should be true for x.com");
        assert!(selectors.is_empty(),
            "x.com should have NO CSS selectors when generichide=true, got: {:?}", selectors);

        // youtube.com has NO generichide — should still get selectors
        let (yt_selectors, _, yt_generichide) = adblock.cosmetic_resources("https://www.youtube.com/");
        assert!(!yt_generichide, "youtube.com should not have generichide");
        assert!(!yt_selectors.is_empty(), "youtube.com should have CSS selectors");
    }

    /// Test that the embedded hodos-unbreak.txt is properly loaded and its rules work
    #[test]
    fn test_hodos_unbreak_list_loaded() {
        let unbreak = HODOS_UNBREAK;
        assert!(!unbreak.is_empty(), "hodos-unbreak.txt should not be empty");
        assert!(unbreak.contains("x.com#@#+js()"), "should contain x.com exception");
        assert!(unbreak.contains("github.com#@#+js()"), "should contain github.com exception");
        assert!(unbreak.contains("accounts.google.com#@#+js()"), "should contain Google auth exception");

        // Build engine with just the unbreak list + a dummy scriptlet rule
        let rules = format!(
            "{}\n{}",
            "example.com##+js(set, test, true)",
            unbreak
        );
        let mut filter_set = FilterSet::new(false);
        filter_set.add_filter_list(&rules, ParseOptions::default());
        let engine = Engine::from_filter_set(filter_set, true);

        // Verify auth domains have empty injected_script
        for domain in &["x.com", "github.com", "accounts.google.com", "login.microsoftonline.com", "discord.com"] {
            let url = format!("https://{}/", domain);
            let resources = engine.url_cosmetic_resources(&url);
            assert!(
                resources.injected_script.is_empty(),
                "{} should have no scriptlets due to hodos-unbreak.txt, got: {}",
                domain,
                &resources.injected_script[..std::cmp::min(80, resources.injected_script.len())]
            );
        }
    }

    #[test]
    fn test_engine_new_starts_in_loading_state() {
        let engine = AdblockEngine::new(PathBuf::from("/tmp/test-adblock"));
        assert_eq!(engine.get_engine_status(), EngineStatus::Loading);
        assert!(engine.is_enabled());
    }

    #[test]
    fn test_enable_disable_toggle() {
        let engine = AdblockEngine::new(PathBuf::from("/tmp/test-adblock"));
        assert!(engine.is_enabled());

        engine.set_enabled(false);
        assert!(!engine.is_enabled());

        engine.set_enabled(true);
        assert!(engine.is_enabled());
    }

    #[test]
    fn test_check_request_returns_false_when_disabled() {
        let engine = AdblockEngine::new(PathBuf::from("/tmp/test-adblock"));
        engine.set_enabled(false);

        let (blocked, _, _) = engine.check_request(
            "https://ads.example.com/banner.js",
            "https://news.com",
            "script",
        );
        assert!(!blocked, "should not block when disabled");
    }

    #[test]
    fn test_check_request_returns_false_when_engine_not_loaded() {
        let engine = AdblockEngine::new(PathBuf::from("/tmp/test-adblock"));
        // Engine is in Loading state — no inner Engine loaded yet

        let (blocked, _, _) = engine.check_request(
            "https://ads.example.com/banner.js",
            "https://news.com",
            "script",
        );
        assert!(!blocked, "should not block when engine not loaded");
    }

    #[test]
    fn test_parse_list_expiry() {
        // EasyList header: "! Expires: 4 days"
        let list = "[Adblock Plus 2.0]\n! Title: EasyList\n! Expires: 4 days\n||ads.example.com^";
        assert_eq!(parse_list_expiry(list), 4 * 24 * 3600);

        // Hours
        let list = "! Expires: 12 hours\n||ads.example.com^";
        assert_eq!(parse_list_expiry(list), 12 * 3600);

        // No Expires header → default (4 days)
        let list = "[Adblock Plus 2.0]\n! Title: Custom\n||ads.example.com^";
        assert_eq!(parse_list_expiry(list), DEFAULT_EXPIRY_SECS);

        // 1 day (singular)
        let list = "! Expires: 1 day\n||ads.example.com^";
        assert_eq!(parse_list_expiry(list), 24 * 3600);
    }

    #[test]
    fn test_needs_update_empty_lists() {
        let engine = AdblockEngine::new(PathBuf::from("/tmp/test-adblock"));
        // No lists loaded → should not need update
        assert!(!engine.needs_update());
    }

    fn list_info(filename: &str, expires_at: u64) -> FilterListInfo {
        FilterListInfo {
            filename: filename.to_string(),
            url: String::new(),
            downloaded_at: 1,
            size_bytes: 1,
            rule_count: 1,
            expires_at,
            version: None,
            last_modified: None,
            failures: 0,
        }
    }

    /// P6-A7: quick-fixes expires every 8 h while the other lists last days. One expired
    /// list must be enough to trigger the update, or quick-fixes waits for the slowest list.
    #[test]
    fn test_needs_update_when_only_quick_fixes_expired() {
        let engine = AdblockEngine::new(PathBuf::from("/tmp/test-adblock"));
        let now = now_secs();
        let fresh = now + 4 * 24 * 3600;
        let mut lists: Vec<FilterListInfo> = FILTER_LISTS.iter().chain(TRUSTED_FILTER_LISTS)
            .map(|(f, _)| list_info(f, fresh))
            .collect();
        *engine.meta.write().unwrap() = FilterListMeta { lists: lists.clone(), engine_built_at: now, config_version: CONFIG_VERSION };
        assert!(!engine.needs_update(), "all lists fresh -> no update");

        lists.iter_mut().find(|l| l.filename == "ublock-quick-fixes.txt")
            .expect("quick-fixes must be a configured list").expires_at = now - 1;
        *engine.meta.write().unwrap() = FilterListMeta { lists, engine_built_at: now, config_version: CONFIG_VERSION };
        assert!(engine.needs_update(), "an expired quick-fixes alone must trigger an update");
    }

    fn set(source: &str, n: usize) -> ScriptletSet {
        let resources = (0..n).map(|i| Resource {
            name: format!("s{}.js", i),
            aliases: vec![],
            kind: ResourceType::Template,
            content: BASE64.encode("(function(){})();"),
            dependencies: vec![],
            permission: PermissionMask::default(),
        }).collect();
        ScriptletSet { resources, source: source.into(), priorities: HashMap::new() }
    }

    /// P6-A5: list versions come from the list's own header; an unfilled placeholder is absent.
    #[test]
    fn test_list_header_versions() {
        let easylist = "[Adblock Plus 2.0]\n! Version: 202610141505\n! Title: EasyList\n! Last modified: 14 Oct 2026 15:05 UTC\n! Expires: 4 days\n||ads.example^";
        assert_eq!(parse_list_header(easylist, "Version").as_deref(), Some("202610141505"));
        assert_eq!(parse_list_header(easylist, "Last modified").as_deref(), Some("14 Oct 2026 15:05 UTC"));
        let uassets_raw = "! Title: uBlock filters – Quick fixes\n! Last modified: %timestamp%\n! Expires: 8 hours";
        assert_eq!(parse_list_header(uassets_raw, "Last modified"), None);
        assert_eq!(parse_list_header(uassets_raw, "Version"), None);
    }

    /// P6-A5: /status reports which scriptlet set is installed, from the install itself.
    #[tokio::test]
    async fn test_status_reports_installed_scriptlet_set() {
        let adblock = AdblockEngine::new(PathBuf::from("/tmp/test-status-scriptlets"));
        assert_eq!(adblock.get_status().scriptlets.source, "", "nothing installed yet");
        let mut engine = engine_from_rules("||ads.example.com^");
        adblock.apply_scriptlets(&mut engine, Some(set("uBO 1.75.0", 151)));
        let s = adblock.get_status().scriptlets;
        // The 6 extras' names are not in this synthetic set, so all six are added.
        assert_eq!((s.source.as_str(), s.count, s.extra), ("uBO 1.75.0", 151, 6));
        // A smaller fresh set leaves the status (and the set) as it was.
        adblock.apply_scriptlets(&mut engine, Some(set("uBO 1.48.4 (fallback)", 90)));
        assert_eq!(adblock.get_status().scriptlets.source, "uBO 1.75.0");
    }

    fn temp_adblock_dir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("hodos-p6-{}-{}", tag, std::process::id()));
        std::fs::create_dir_all(d.join("resources")).unwrap();
        d
    }

    /// Startup installs from disk only: with no cache and no 1.48.4 file nothing is
    /// downloaded (it would need the network), and the source reads "none".
    #[tokio::test]
    async fn test_startup_install_never_downloads() {
        let dir = temp_adblock_dir("startup-none");
        let adblock = AdblockEngine::new(dir.clone());
        let mut engine = engine_from_rules("||ads.example.com^");
        adblock.install_scriptlets(&mut engine).await;
        assert_eq!(adblock.get_status().scriptlets.source, "none");
        assert!(!dir.join("resources").join(SCRIPTLETS_FILE).exists(), "startup must not download the 1.48.4 file");
        // With the pinned set cached, startup uses it.
        crate::ubo_resources::write_test_cache(&dir.join("resources"), crate::ubo_resources::test_converted_set());
        adblock.install_scriptlets(&mut engine).await;
        assert_eq!(adblock.get_status().scriptlets.source, pinned_scriptlet_source());
    }

    /// After load, refresh_scriptlets swaps the pinned set into the LIVE engine and bumps
    /// update_version so the shell drops its cached results.
    #[tokio::test]
    async fn test_refresh_swaps_pinned_set_into_live_engine() {
        let dir = temp_adblock_dir("refresh");
        let adblock = AdblockEngine::new(dir.clone());
        *adblock.engine.write().unwrap() = Some(engine_from_rules("example.com##+js(main-one, x)"));
        assert!(adblock.cosmetic_resources("https://example.com/").1.is_empty(), "no scriptlets before refresh");
        let v0 = adblock.get_update_version();

        crate::ubo_resources::write_test_cache(&dir.join("resources"), crate::ubo_resources::test_converted_set());
        adblock.refresh_scriptlets().await;
        assert_eq!(adblock.get_status().scriptlets.source, pinned_scriptlet_source());
        assert!(adblock.cosmetic_resources("https://example.com/").1.contains("mainOne("), "live engine runs the new set");
        assert_eq!(adblock.get_update_version(), v0 + 1);
        // Already on the pinned set: a second refresh is a no-op.
        adblock.refresh_scriptlets().await;
        assert_eq!(adblock.get_update_version(), v0 + 1);
    }

    /// WHY startup compiles from the lists instead of loading engine.dat (B5-T0-P6):
    /// adblock 0.10.3's serialization drops each filter's permission mask, so a trusted-only
    /// scriptlet resolves on the fresh engine and NOT after serialize -> deserialize.
    /// ⚠️ If this test fails after a crate upgrade, serialization now keeps permissions and a
    /// serialized fast path could be reconsidered — the startup test below still guards it.
    #[test]
    fn test_adblock_serialization_drops_trusted_permission() {
        let trusted = ParseOptions { permissions: PermissionMask::from_bits(1), ..ParseOptions::default() };
        let mut fs = FilterSet::new(false);
        fs.add_filter_list("youtube.com##+js(trusted-probe, a)", trusted);
        let mut fresh = Engine::from_filter_set(fs, true);
        let res = vec![Resource { name: "trusted-probe.js".into(), aliases: vec![],
            kind: ResourceType::Mime(adblock::resources::MimeType::ApplicationJavascript),
            content: BASE64.encode("function trustedProbe(a) {}"), dependencies: vec![], permission: PermissionMask::from_bits(1) }];
        fresh.use_resources(res.clone());
        let url = "https://www.youtube.com/watch?v=x";
        assert!(!fresh.url_cosmetic_resources(url).injected_script.is_empty(), "fresh engine resolves the trusted rule");

        let data = fresh.serialize().unwrap();
        let mut loaded = Engine::default();
        loaded.deserialize(&data).unwrap();
        loaded.use_resources(res);
        assert!(loaded.url_cosmetic_resources(url).injected_script.is_empty(),
            "adblock serialization now keeps the trusted permission — see the doc comment");
    }

    /// The production startup path keeps trusted scriptlets: load() on a dir with every
    /// configured list on disk (a trusted rule in uBO's list) and the pinned set cached —
    /// offline — injects that trusted scriptlet.
    #[tokio::test]
    async fn test_startup_keeps_trusted_scriptlets() {
        let dir = temp_adblock_dir("startup-trusted");
        std::fs::create_dir_all(dir.join("lists")).unwrap();
        let mut lists = Vec::new();
        for (f, _) in FILTER_LISTS.iter().chain(TRUSTED_FILTER_LISTS) {
            let text = if *f == "ublock-filters.txt" { "example.com##+js(trusted-edit, x)\n" } else { "! empty\n" };
            std::fs::write(dir.join("lists").join(f), text).unwrap();
            lists.push(list_info(f, now_secs() + 3600));
        }
        let meta = FilterListMeta { lists, engine_built_at: now_secs(), config_version: CONFIG_VERSION };
        std::fs::write(dir.join(META_JSON), serde_json::to_string(&meta).unwrap()).unwrap();
        crate::ubo_resources::write_test_cache(&dir.join("resources"), crate::ubo_resources::test_converted_set());

        let adblock = AdblockEngine::new(dir);
        adblock.load().await.expect("load from disk, offline");
        let (_, script, _) = adblock.cosmetic_resources("https://example.com/");
        assert!(script.contains("editFn("), "trusted scriptlet missing after startup: {:?}", &script[..script.len().min(120)]);
    }

    /// P6-A6: a refresh never installs fewer scriptlets than are running.
    #[test]
    fn test_choose_scriptlets_never_shrinks() {
        let running = set("uBO 1.75.0", 151);
        assert_eq!(choose_scriptlets(Some(set("uBO 1.75.0", 151)), &running).resources.len(), 151);
        assert_eq!(choose_scriptlets(Some(set("uBO 1.48.4 (fallback)", 90)), &running).source, "uBO 1.75.0");
        assert_eq!(choose_scriptlets(None, &running).source, "uBO 1.75.0");
        // First load: whatever is available, or none.
        let empty = ScriptletSet::default();
        assert_eq!(choose_scriptlets(Some(set("uBO 1.48.4 (fallback)", 90)), &empty).resources.len(), 90);
        assert_eq!(choose_scriptlets(None, &empty).source, "none");
    }

    /// Function-style scriptlets must run in a private scope that defines scriptletGlobals.
    #[test]
    fn test_injected_script_is_wrapped_privately() {
        assert_eq!(wrap_scriptlets(""), "", "nothing to inject stays empty");
        let adblock = AdblockEngine::new(PathBuf::from("/tmp/test-wrap"));
        {
            let mut filter_set = FilterSet::new(false);
            filter_set.add_filter_list("youtube.com##+js(probe)", ParseOptions::default());
            let mut engine = Engine::from_filter_set(filter_set, true);
            engine.use_resources([
                Resource { name: "probe.js".into(), aliases: vec![], kind: ResourceType::Mime(adblock::resources::MimeType::ApplicationJavascript),
                    content: BASE64.encode("function probeFn() { helperFn(); }"), dependencies: vec!["helper.fn".into()], permission: PermissionMask::default() },
                Resource { name: "helper.fn".into(), aliases: vec![], kind: ResourceType::Mime(adblock::resources::MimeType::FnJavascript),
                    content: BASE64.encode("function helperFn() { return scriptletGlobals; }"), dependencies: vec![], permission: PermissionMask::default() },
            ]);
            *adblock.engine.write().unwrap() = Some(engine);
        }
        let (_, script, _) = adblock.cosmetic_resources("https://www.youtube.com/watch?v=x");
        assert!(script.starts_with("(function() {\nconst scriptletGlobals = {};\n"), "got: {}", script);
        assert!(script.ends_with("\n})();"));
        assert!(script.contains("function helperFn()") && script.contains("probeFn()"));
        let (_, none, _) = adblock.cosmetic_resources("https://example.org/");
        assert!(none.is_empty());
    }

    /// Rules (text, from a trusted list?) whose `+js()` produces no script in an engine with
    /// exactly that rule and `resources` — the engine's own answer to "does it resolve".
    fn unresolved_scriptlet_rules(rules: &[(String, bool)], resources: &[Resource]) -> Vec<String> {
        let mut missing = Vec::new();
        for (rule, trusted) in rules {
            let host = rule.split("##").next().unwrap_or("").split(',')
                .find(|d| d.contains("youtube") && !d.starts_with('~'))
                .unwrap_or("www.youtube.com")
                .replace(".*", ".com");
            let opts = ParseOptions {
                permissions: if *trusted { PermissionMask::from_bits(1) } else { PermissionMask::default() },
                ..ParseOptions::default()
            };
            let mut fs = FilterSet::new(false);
            fs.add_filter_list(rule, opts);
            let mut engine = Engine::from_filter_set(fs, true);
            engine.use_resources(resources.to_vec());
            load_extra_scriptlets(&mut engine);
            if engine.url_cosmetic_resources(&format!("https://{}/watch?v=x", host)).injected_script.is_empty() {
                missing.push(rule.clone());
            }
        }
        missing
    }

    /// The instrument for P6-A2 can fail: it names a rule whose scriptlet is absent.
    #[test]
    fn test_unresolved_scriptlet_rules_names_missing() {
        let resources = vec![Resource { name: "present.js".into(), aliases: vec![],
            kind: ResourceType::Mime(adblock::resources::MimeType::ApplicationJavascript),
            content: BASE64.encode("function presentFn(a) {}"), dependencies: vec![], permission: PermissionMask::from_bits(1) }];
        let rules = vec![
            ("youtube.com##+js(present, x)".to_string(), true),
            ("www.youtube.com##+js(absent-scriptlet, x)".to_string(), true),
            // A trusted-only scriptlet from an untrusted list does not resolve either.
            ("youtube.com##+js(present, y)".to_string(), false),
        ];
        assert_eq!(unresolved_scriptlet_rules(&rules, &resources), vec![
            "www.youtube.com##+js(absent-scriptlet, x)".to_string(),
            "youtube.com##+js(present, y)".to_string(),
        ]);
    }

    /// A real engine in a temp dir, built by the production `load()` path (network).
    async fn real_engine(tag: &str) -> (AdblockEngine, PathBuf) {
        let dir = std::env::temp_dir().join(format!("hodos-p6-{}-{}", tag, std::process::id()));
        let adblock = AdblockEngine::new(dir.clone());
        // As main.rs does: load (disk only), then fetch uBO's pinned scriptlets.
        adblock.load().await.expect("load");
        adblock.refresh_scriptlets().await;
        (adblock, dir)
    }

    /// P6-A2: every YouTube `+js()` rule in the loaded lists resolves to a script in the engine,
    /// and every resolved script parses. Network + Node:
    /// `cargo test -- --ignored test_real_youtube_scriptlets_resolve`.
    #[tokio::test]
    #[ignore]
    async fn test_real_youtube_scriptlets_resolve() {
        let (adblock, dir) = real_engine("a2").await;
        let status = adblock.get_status();
        assert!(status.scriptlets.source.starts_with("uBO 1.75"), "running {:?}", status.scriptlets.source);

        let trusted: Vec<&str> = TRUSTED_FILTER_LISTS.iter().map(|(f, _)| *f).collect();
        let mut rules = Vec::new();
        for l in &status.lists {
            let text = std::fs::read_to_string(dir.join("lists").join(&l.filename)).unwrap();
            for line in text.lines() {
                let line = line.trim();
                // A YouTube rule names YouTube in a domain it APPLIES to; `~youtube.com` excludes it.
                let applies_to_youtube = line.split("##").next().unwrap().split(',')
                    .any(|d| !d.starts_with('~') && d.contains("youtube"));
                if !line.starts_with('!') && line.contains("##+js(") && applies_to_youtube {
                    rules.push((line.to_string(), trusted.contains(&l.filename.as_str())));
                }
            }
        }
        assert!(rules.len() > 30, "only {} YouTube scriptlet rules found", rules.len());
        let resources = adblock.scriptlets.read().unwrap().0.resources.clone();
        let missing = unresolved_scriptlet_rules(&rules, &resources);
        assert!(missing.is_empty(), "{} of {} YouTube rules do not resolve:\n{}", missing.len(), rules.len(), missing.join("\n"));

        // Everything YouTube actually receives must at least parse.
        let mut all = String::new();
        for url in ["https://www.youtube.com/watch?v=x", "https://m.youtube.com/", "https://www.youtube.com/"] {
            let (_, script, _) = adblock.cosmetic_resources(url);
            assert!(!script.is_empty(), "{} gets no scriptlets", url);
            all.push_str(&script);
            all.push_str("\n;\n");
        }
        let f = dir.join("youtube-injected.js");
        std::fs::write(&f, &all).unwrap();
        let check = std::process::Command::new("node").arg("--check").arg(&f).output().expect("node");
        assert!(check.status.success(), "injected script does not parse: {}", String::from_utf8_lossy(&check.stderr));
        println!("{} YouTube rules resolve; {} bytes injected parse", rules.len(), all.len());
    }

    /// P6-A4: hodos-unbreak domains get no scriptlets and generichide on the real engine with
    /// quick-fixes and uBO's current scriptlets — and the check has teeth: without the unbreak
    /// list, some of those domains would receive scriptlets. Network:
    /// `cargo test -- --ignored test_real_unbreak_domains_get_no_scriptlets`.
    #[tokio::test]
    #[ignore]
    async fn test_real_unbreak_domains_get_no_scriptlets() {
        let (adblock, dir) = real_engine("a4").await;
        let domains: Vec<String> = HODOS_UNBREAK.lines()
            .filter_map(|l| l.trim().strip_suffix("#@#+js()").map(str::to_string))
            .collect();
        assert!(domains.len() >= 20);
        for d in &domains {
            let (_, script, _) = adblock.cosmetic_resources(&format!("https://{}/", d));
            assert!(script.is_empty(), "{} receives scriptlets despite hodos-unbreak", d);
        }
        for d in ["accounts.google.com", "github.com", "paypal.com", "chase.com", "login.microsoftonline.com"] {
            let (selectors, _, generichide) = adblock.cosmetic_resources(&format!("https://{}/", d));
            assert!(generichide && selectors.is_empty(), "{} lost generichide", d);
        }

        // Teeth: the same lists and scriptlets WITHOUT hodos-unbreak inject into some of them.
        // Same lists, trust and pruning as production (configured_lists + add_list).
        let mut fs = FilterSet::new(false);
        for (f, _, opts) in configured_lists() {
            add_list(&mut fs, &std::fs::read_to_string(dir.join("lists").join(f)).unwrap(), opts);
        }
        let mut bare = Engine::from_filter_set(fs, true);
        bare.use_resources(adblock.scriptlets.read().unwrap().0.resources.clone());
        let exposed: Vec<&String> = domains.iter()
            .filter(|d| !bare.url_cosmetic_resources(&format!("https://{}/", d)).injected_script.is_empty())
            .collect();
        assert!(!exposed.is_empty(), "no unbreak domain would get scriptlets anyway — this test has no teeth");
        println!("{} unbreak domains protected; {} of them would otherwise get scriptlets: {:?}",
            domains.len(), exposed.len(), exposed);
    }

    /// A local HTTP server answering every request with `status` and `body` — a captive
    /// portal, an error page. Returns its base URL.
    async fn serve(status: &'static str, body: &'static str) -> (String, std::sync::Arc<AtomicU64>) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let hits = std::sync::Arc::new(AtomicU64::new(0));
        let counter = hits.clone();
        tokio::spawn(async move {
            loop {
                let Ok((mut sock, _)) = listener.accept().await else { return };
                counter.fetch_add(1, Ordering::SeqCst);
                tokio::spawn(async move {
                    let mut buf = [0u8; 4096];
                    let _ = sock.read(&mut buf).await;
                    let resp = format!("HTTP/1.1 {}\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        status, body.len(), body);
                    let _ = sock.write_all(resp.as_bytes()).await;
                });
            }
        });
        (format!("http://{}", addr), hits)
    }

    const PORTAL: &str = "<!DOCTYPE html><html><body>Sign in to the Wi-Fi</body></html>";

    /// A portal's 200 HTML never replaces a cached list; the cache is used and retried soon.
    #[tokio::test]
    async fn test_portal_page_never_replaces_a_cached_list() {
        let dir = temp_adblock_dir("portal-list");
        std::fs::create_dir_all(dir.join("lists")).unwrap();
        let good = "! Title: EasyList\n! Expires: 4 days\n||ads.example^\n";
        std::fs::write(dir.join("lists").join("easylist.txt"), good).unwrap();
        let (base, hits) = serve("200 OK", PORTAL).await;
        let url = format!("{}/easylist.txt", base);
        let now = now_secs();
        let (text, info) = obtain_list(&http_client(), &dir.join("lists"), "easylist.txt", &[url.as_str()], None, now)
            .await.expect("cached copy");
        assert_eq!(text, good);
        assert_eq!(std::fs::read_to_string(dir.join("lists").join("easylist.txt")).unwrap(), good, "cache not overwritten");
        assert_eq!(info.expires_at, now + RETRY_AFTER_FAILURE_SECS);
        assert!(hits.load(Ordering::SeqCst) > 0, "the portal was never asked: this test proved nothing");
    }

    /// A portal page is never kept as the 1.48.4 fallback file.
    #[tokio::test]
    async fn test_portal_page_is_not_kept_as_legacy_scriptlets() {
        let dir = temp_adblock_dir("portal-legacy");
        let (base, hits) = serve("200 OK", PORTAL).await;
        let url = format!("{}/scriptlets.js", base);
        let resources = obtain_legacy(&dir.join("resources"), Some((&http_client(), url.as_str()))).await;
        assert!(resources.is_empty());
        assert!(!dir.join("resources").join(SCRIPTLETS_FILE).exists(), "portal page must not be saved");
        assert!(!dir.join("resources").join("scriptlets.js.download").exists());
        assert!(hits.load(Ordering::SeqCst) > 0, "the portal was never asked: this test proved nothing");
    }

    /// A list that never downloaded gets a 0-rule placeholder retried on its own cadence, so
    /// needs_update() does not force an hourly rebuild; startup compiles without it.
    #[tokio::test]
    async fn test_unobtainable_list_backs_off_and_startup_compiles_without_it() {
        let dir = temp_adblock_dir("missing-list");
        write_lists(&dir, |f| format!("! Title: {}\n||{}.example^\n", f, f.replace('.', "-")));
        std::fs::remove_file(dir.join("lists").join("ublock-quick-fixes.txt")).unwrap();
        let now = now_secs();
        let (_e, meta) = AdblockEngine::build_from_lists(&dir, None, &offline_client()).await.expect("build from cache");
        let qf = meta.lists.iter().find(|l| l.filename == "ublock-quick-fixes.txt").expect("placeholder");
        assert_eq!((qf.rule_count, qf.downloaded_at, qf.failures), (0, 0, 1));
        assert!(qf.expires_at >= now + RETRY_AFTER_FAILURE_SECS);
        let engine = AdblockEngine::new(PathBuf::from("/tmp/test-adblock"));
        *engine.meta.write().unwrap() = meta.clone();
        assert!(!engine.needs_update(), "a placeholder does not trigger an immediate rebuild");

        // Startup: compiles the four cached lists, no download needed. A rebuild would stamp a
        // new engine_built_at — checked below, so this holds online and offline alike.
        let mut meta = meta;
        meta.engine_built_at = 12345;
        std::fs::write(dir.join(META_JSON), serde_json::to_string(&meta).unwrap()).unwrap();
        let adblock = AdblockEngine::new(dir);
        adblock.load().await.expect("startup compiles without the missing list");
        assert!(adblock.check_request("https://easylist-txt.example/x.js", "https://site.example/", "script").0);
        let qf_after = adblock.get_status().lists.into_iter().find(|l| l.filename == "ublock-quick-fixes.txt").unwrap();
        assert_eq!(qf_after.rule_count, 0, "startup must not have gone to the network for the missing list");
        assert_eq!(adblock.get_status().last_update, 12345, "startup compiled from disk; it did not rebuild");
        assert!(!adblock.needs_update(), "startup must keep the placeholder's backoff, not force a rebuild");
    }

    /// A list meta calls fresh but that is gone from disk is marked for re-fetch at startup.
    #[tokio::test]
    async fn test_startup_refetches_a_list_missing_on_disk() {
        let dir = temp_adblock_dir("gone-from-disk");
        write_lists(&dir, |f| format!("! Title: {}\n||{}.example^\n", f, f.replace('.', "-")));
        let lists = configured_lists().iter().map(|(f, _, _)| list_info(f, now_secs() + 4 * 24 * 3600)).collect();
        let meta = FilterListMeta { lists, engine_built_at: 7, config_version: CONFIG_VERSION };
        std::fs::write(dir.join(META_JSON), serde_json::to_string(&meta).unwrap()).unwrap();
        std::fs::remove_file(dir.join("lists").join("easylist.txt")).unwrap();
        let adblock = AdblockEngine::new(dir);
        adblock.load().await.expect("load");
        assert!(adblock.needs_update(), "EasyList is gone from disk: the first check must re-fetch it");
        let el = adblock.get_status().lists.into_iter().find(|l| l.filename == "easylist.txt").unwrap();
        assert_eq!(el.rule_count, 0, "/status must not report rules that are not loaded");
    }

    /// A list due within the slack counts as due: a retry window equal to the check interval
    /// would otherwise slip a whole interval (/code-review 173 #4).
    #[test]
    fn test_needs_update_counts_a_list_due_within_the_slack() {
        let engine = AdblockEngine::new(PathBuf::from("/tmp/test-adblock"));
        let now = now_secs();
        let lists = configured_lists().iter()
            .map(|(f, _, _)| list_info(f, if *f == "easylist.txt" { now + 60 } else { now + 4 * 24 * 3600 }))
            .collect();
        *engine.meta.write().unwrap() = FilterListMeta { lists, engine_built_at: now, config_version: CONFIG_VERSION };
        assert!(engine.needs_update());
    }

    /// A placeholder not yet due is carried over untouched — its mirrors are not retried
    /// because some other list expired (/code-review 173 #5).
    #[tokio::test]
    async fn test_placeholder_not_due_is_not_retried_by_another_lists_rebuild() {
        let dir = temp_adblock_dir("placeholder-wait");
        write_lists(&dir, |f| format!("! Title: {}\n||{}.example^\n", f, f.replace('.', "-")));
        std::fs::remove_file(dir.join("lists").join("ublock-quick-fixes.txt")).unwrap();
        let now = now_secs();
        let mut lists: Vec<FilterListInfo> = configured_lists().iter().map(|(f, _, _)| {
            let mut i = list_info(f, if *f == "easyprivacy.txt" { now - 1 } else { now + 4 * 24 * 3600 });
            i.downloaded_at = 50;
            i
        }).collect();
        let qf = lists.iter_mut().find(|l| l.filename == "ublock-quick-fixes.txt").unwrap();
        qf.downloaded_at = 0;
        qf.rule_count = 0;
        qf.failures = 2;
        qf.expires_at = now + 3600;
        let prev = FilterListMeta { lists, engine_built_at: now, config_version: CONFIG_VERSION };
        let (_e, meta) = AdblockEngine::build_from_lists(&dir, Some(&prev), &offline_client()).await.unwrap();
        let qf_after = meta.lists.iter().find(|l| l.filename == "ublock-quick-fixes.txt").unwrap();
        assert_eq!((qf_after.failures, qf_after.expires_at), (2, now + 3600), "placeholder must be carried over untouched");
    }

    /// The failure counter saturates (a corrupt meta.json must not wrap the backoff to 0).
    #[tokio::test]
    async fn test_failure_counter_saturates() {
        let dir = temp_adblock_dir("saturate");
        write_lists(&dir, |f| format!("! Title: {}\n", f));
        let now = now_secs();
        let mut prev = list_info("easylist.txt", now - 1);
        prev.failures = u32::MAX;
        let (_t, info) = obtain_list(&offline_client(), &dir.join("lists"), "easylist.txt", &["http://127.0.0.1:9/x"], Some(&prev), now)
            .await.unwrap();
        assert_eq!(info.failures, u32::MAX);
        assert_eq!(info.expires_at, now + MAX_RETRY_SECS);
    }

    #[test]
    fn test_retry_backoff_doubles_to_a_ceiling() {
        assert_eq!(retry_after(1), RETRY_AFTER_FAILURE_SECS);
        assert_eq!(retry_after(2), 2 * RETRY_AFTER_FAILURE_SECS);
        assert_eq!(retry_after(3), 4 * RETRY_AFTER_FAILURE_SECS);
        assert_eq!(retry_after(10), MAX_RETRY_SECS);
        assert_eq!(retry_after(u32::MAX), MAX_RETRY_SECS);
    }

    /// Consecutive failures back off; a placeholder is never reused as if it were a real list.
    #[tokio::test]
    async fn test_failed_refreshes_back_off_and_placeholders_are_not_reused() {
        let dir = temp_adblock_dir("backoff");
        write_lists(&dir, |f| format!("! Title: {}\n||{}.example^\n", f, f.replace('.', "-")));
        let now = now_secs();
        let mut prev = list_info("easylist.txt", now - 1);
        prev.failures = 2;
        prev.downloaded_at = 99;
        let (_t, info) = obtain_list(&offline_client(), &dir.join("lists"), "easylist.txt", &["http://127.0.0.1:9/x"], Some(&prev), now)
            .await.unwrap();
        assert_eq!(info.failures, 3);
        assert_eq!(info.expires_at, now + 4 * RETRY_AFTER_FAILURE_SECS);
        assert_eq!(info.downloaded_at, 99);
        // A placeholder that has not expired is still not "fresh": the file on disk is read and
        // gets real metadata (here via the cache fallback, since the client is offline).
        let mut placeholder = list_info("easylist.txt", now + 3600);
        placeholder.downloaded_at = 0;
        placeholder.rule_count = 0;
        let (_t, info) = obtain_list(&offline_client(), &dir.join("lists"), "easylist.txt", &["http://127.0.0.1:9/x"], Some(&placeholder), now)
            .await.unwrap();
        assert!(info.rule_count > 0 && info.downloaded_at > 0, "placeholder metadata must not stick: {:?}", info);
    }

    /// A corrupt 1.48.4 file must not panic the engine (adblock's assembler uses `expect`).
    #[tokio::test]
    async fn test_corrupt_legacy_file_does_not_panic() {
        let dir = temp_adblock_dir("corrupt-legacy");
        std::fs::create_dir_all(dir.join("resources")).unwrap();
        std::fs::write(dir.join("resources").join(SCRIPTLETS_FILE), b"/// broken\n\xff\xfe not utf-8").unwrap();
        assert!(obtain_legacy(&dir.join("resources"), None).await.is_empty());
    }

    #[test]
    fn test_running_pinned_set_is_not_replaced_by_a_fallback() {
        let running = set(&pinned_scriptlet_source(), 150);
        let older = set("uBO 1.70.0 (cached; pin not fetched yet)", 151);
        assert_eq!(choose_scriptlets(Some(older), &running).source, pinned_scriptlet_source());
    }

    /// A served byte-order mark does not hide the list header. (Lenient decoding of an invalid byte
    /// is not covered: this test server sends &str bodies.)
    #[tokio::test]
    async fn test_download_strips_a_byte_order_mark() {
        let (base, _hits) = serve("200 OK", "\u{feff}! Title: x\n||a^\n").await;
        let text = download_filter_list(&http_client(), &format!("{}/l.txt", base)).await.unwrap();
        assert!(text.starts_with("! Title: x"), "{:?}", &text[..12]);
        assert!(check_plausible(&text).is_ok());
    }

    #[test]
    fn test_choose_prefers_the_pinned_set_even_if_smaller() {
        let running = set("uBO 1.70.0 (cached; pin not fetched yet)", 151);
        let pinned = set(&pinned_scriptlet_source(), 150);
        assert_eq!(choose_scriptlets(Some(pinned), &running).resources.len(), 150);
    }

    #[test]
    fn test_order_by_priority() {
        let deps = "function low(a) {}\nfunction config(a) {}\n";
        let block = |c: &str| format!("try {{\n{}\n}} catch ( e ) {{ }}\n", c);
        let script = format!("{}{}{}{}", deps, block("low(\"3\")"), block("config(\"2\")"), block("low(\"1\")"));
        let prio: HashMap<String, i64> = [("config".to_string(), 100)].into();
        assert_eq!(order_by_priority(&script, &prio),
            format!("{}{}{}{}", deps, block("config(\"2\")"), block("low(\"1\")"), block("low(\"3\")")),
            "priority call first; equal priorities by call text");
        // No priorities: still a fixed order (by call text) — adblock's own order is random.
        assert_eq!(order_by_priority(&script, &HashMap::new()),
            format!("{}{}{}{}", deps, block("config(\"2\")"), block("low(\"1\")"), block("low(\"3\")")));
        // Template-style (multi-line) invocations are left as they are.
        let template = format!("{}try {{\n(function() {{\n}})();\n}} catch ( e ) {{ }}\n", deps);
        assert_eq!(order_by_priority(&template, &prio), template);
    }

    /// End to end: the script the engine hands the shell has the priority call first and the
    /// rest in a fixed order — on every lookup. adblock collects calls in a fresh HashMap per
    /// lookup (random order), so one lookup could pass by chance; 30 cannot (/code-review 171 #6).
    #[test]
    fn test_priority_scriptlet_runs_first_in_injected_script() {
        let mut converted = crate::ubo_resources::test_converted_set();
        converted.iter_mut().find(|c| c.name == "trusted-edit.js").unwrap().priority = 100;
        let adblock = AdblockEngine::new(PathBuf::from("/tmp/test-priority"));
        let trusted = ParseOptions { permissions: PermissionMask::from_bits(1), ..ParseOptions::default() };
        let mut fs = FilterSet::new(false);
        fs.add_filter_list("example.com##+js(main-one, c)\nexample.com##+js(main-one, a)\n\
                            example.com##+js(trusted-edit, z)\nexample.com##+js(main-one, b)", trusted);
        let mut engine = Engine::from_filter_set(fs, true);
        adblock.apply_scriptlets(&mut engine, Some(ScriptletSet::from_converted(&converted, "uBO test".into())));
        *adblock.engine.write().unwrap() = Some(engine);
        for _ in 0..30 {
            let (_, script, _) = adblock.cosmetic_resources("https://example.com/");
            let at = |c: &str| script.find(c).unwrap_or_else(|| panic!("{} missing", c));
            let order = [at("editFn(\"z\")"), at("mainOne(\"a\")"), at("mainOne(\"b\")"), at("mainOne(\"c\")")];
            assert!(order.windows(2).all(|w| w[0] < w[1]), "calls out of order: {:?}", order);
        }
    }

    /// After a pin bump that has not been fetched yet, startup uses the previous pin's
    /// converted set rather than the 2023 single-file set.
    #[tokio::test]
    async fn test_startup_falls_back_to_previous_pin_cache() {
        let dir = temp_adblock_dir("prev-pin");
        crate::ubo_resources::write_test_cache_for_other_pin(&dir.join("resources"), "1.70.0", crate::ubo_resources::test_converted_set());
        let set = obtain_scriptlets(&dir.join("resources"), false).await.expect("a set");
        assert!(set.source.starts_with("uBO 1.70.0 (cached"), "got {:?}", set.source);
        assert_ne!(set.source, pinned_scriptlet_source(), "refresh_scriptlets must still try the pin");
    }

    /// A configured list that never downloaded (absent from meta) triggers an update.
    #[test]
    fn test_needs_update_when_a_configured_list_is_missing() {
        let engine = AdblockEngine::new(PathBuf::from("/tmp/test-adblock"));
        let now = now_secs();
        let lists: Vec<FilterListInfo> = FILTER_LISTS.iter().chain(TRUSTED_FILTER_LISTS)
            .filter(|(f, _)| *f != "ublock-quick-fixes.txt")
            .map(|(f, _)| list_info(f, now + 4 * 24 * 3600))
            .collect();
        *engine.meta.write().unwrap() = FilterListMeta { lists, engine_built_at: now, config_version: CONFIG_VERSION };
        assert!(engine.needs_update(), "quick-fixes never downloaded -> retry now, not when another list expires");
    }

    #[test]
    fn test_check_plausible_rejects_portals_and_collapses() {
        let list = "[Adblock Plus 2.0]\n! Title: EasyList\n||a^\n||b^\n||c^\n||d^\n";
        assert!(check_plausible(list).is_ok());
        assert!(check_plausible("! Title: uBlock filters\n||a^\n").is_ok());
        assert!(check_plausible("<!DOCTYPE html><html>Sign in to Wi-Fi</html>").is_err());
        assert!(check_plausible("\u{feff}  <html>").is_err());
        assert!(check_plausible("||a^\n||b^\n").is_err(), "no header");
        // A list that legitimately shrank a lot is still a list (/code-review 171 #1).
        assert!(check_plausible("! Title: x\n||a^\n").is_ok());
    }

    /// A client whose every request fails fast (nothing listens on port 9).
    fn offline_client() -> reqwest::Client {
        reqwest::Client::builder().proxy(reqwest::Proxy::all("http://127.0.0.1:9").unwrap()).build().unwrap()
    }

    fn write_lists(dir: &Path, text_for: impl Fn(&str) -> String) {
        std::fs::create_dir_all(dir.join("lists")).unwrap();
        for (f, _, _) in configured_lists() {
            std::fs::write(dir.join("lists").join(f), text_for(f)).unwrap();
        }
    }

    /// A rebuild reuses unexpired lists as they are and, when an expired list cannot be
    /// fetched, keeps the cached copy but retries within the hour — never a full fresh expiry.
    #[tokio::test]
    async fn test_rebuild_reuses_fresh_lists_and_retries_failed_ones_soon() {
        let dir = temp_adblock_dir("perlist");
        write_lists(&dir, |f| format!("! Title: {}\n! Expires: 4 days\n||{}.example^\n", f, f.replace('.', "-")));
        let now = now_secs();
        let fresh_until = now + 3 * 24 * 3600;
        let prev = FilterListMeta {
            lists: FILTER_LISTS.iter().chain(TRUSTED_FILTER_LISTS).map(|(f, _)| {
                let mut i = list_info(f, if *f == "ublock-quick-fixes.txt" { now - 10 } else { fresh_until });
                i.downloaded_at = 1234;
                i
            }).collect(),
            engine_built_at: now - 100,
            config_version: CONFIG_VERSION,
        };
        let (_engine, meta) = AdblockEngine::build_from_lists(&dir, Some(&prev), &offline_client()).await.expect("build offline from cache");
        for l in &meta.lists {
            if l.filename == "ublock-quick-fixes.txt" {
                assert!(l.expires_at <= now_secs() + RETRY_AFTER_FAILURE_SECS && l.expires_at > now,
                    "failed refresh retries within the hour, got {}", l.expires_at - now);
                assert_eq!(l.downloaded_at, 1234, "the cached copy is still the old download");
            } else {
                assert_eq!(l.expires_at, fresh_until, "{} was fresh and must be reused untouched", l.filename);
            }
        }
        assert_eq!(meta.lists.len(), 5);

        // First start, nothing cached, offline: no engine rather than an empty one.
        let empty = temp_adblock_dir("perlist-empty");
        assert!(AdblockEngine::build_from_lists(&empty, None, &offline_client()).await.is_err());
    }

    /// Startup compiles only the `!#if` branches that apply to Hodos (env_chromium, ublock).
    #[tokio::test]
    async fn test_startup_prunes_inapplicable_branches() {
        let dir = temp_adblock_dir("startup-prune");
        write_lists(&dir, |f| if f == "ublock-filters.txt" {
            "! Title: t\n!#if env_firefox\n||firefox-only.example^\n!#else\n||not-firefox.example^\n!#endif\n\
             !#if ext_ubol\n||ubol-only.example^\n!#endif\n".to_string()
        } else { "! Title: t\n".to_string() });
        let lists = FILTER_LISTS.iter().chain(TRUSTED_FILTER_LISTS).map(|(f, _)| list_info(f, now_secs() + 3600)).collect();
        let meta = FilterListMeta { lists, engine_built_at: now_secs(), config_version: CONFIG_VERSION };
        std::fs::write(dir.join(META_JSON), serde_json::to_string(&meta).unwrap()).unwrap();
        let adblock = AdblockEngine::new(dir);
        adblock.load().await.expect("load");
        let blocked = |host: &str| adblock.check_request(&format!("https://{}/x.js", host), "https://site.example/", "script").0;
        assert!(blocked("not-firefox.example"), "the applicable branch is loaded");
        assert!(!blocked("firefox-only.example"), "Firefox-only branch must not load");
        assert!(!blocked("ubol-only.example"), "uBO Lite branch must not load");
    }

    /// P6-A1: a fresh build loads quick-fixes as a list, records it in meta.json, and takes
    /// its expiry from the list's own `! Expires:` header (8 hours today).
    /// Network-bound: `cargo test -- --ignored test_build_loads_quick_fixes`.
    #[tokio::test]
    #[ignore]
    async fn test_build_loads_quick_fixes() {
        let dir = std::env::temp_dir().join(format!("hodos-p6-a1-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let (_engine, meta) = AdblockEngine::build_from_lists(&dir, None, &http_client()).await.expect("build");

        // Read what the build persisted, not the value it returned.
        let saved: FilterListMeta = serde_json::from_str(
            &std::fs::read_to_string(dir.join(META_JSON)).unwrap()).unwrap();
        assert_eq!(saved.config_version, CONFIG_VERSION);
        let qf = saved.lists.iter().find(|l| l.filename == "ublock-quick-fixes.txt")
            .unwrap_or_else(|| panic!("quick-fixes missing from meta.json: {:?}",
                saved.lists.iter().map(|l| &l.filename).collect::<Vec<_>>()));
        assert!(qf.rule_count > 0);
        let on_disk = std::fs::read_to_string(dir.join("lists").join("ublock-quick-fixes.txt")).unwrap();
        assert_eq!(qf.expires_at - qf.downloaded_at, parse_list_expiry(&on_disk));
        assert!(qf.expires_at - qf.downloaded_at <= 24 * 3600, "quick-fixes should expire within a day");
        assert_eq!(meta.lists.len(), saved.lists.len());
    }

    #[test]
    fn test_status_defaults() {
        let engine = AdblockEngine::new(PathBuf::from("/tmp/test-adblock"));
        let status = engine.get_status();
        assert!(status.enabled);
        assert_eq!(status.status, EngineStatus::Loading);
        assert_eq!(status.list_count, 0);
        assert_eq!(status.total_rules, 0);
        assert_eq!(status.last_update, 0);
        assert!(status.lists.is_empty());
    }

    /// Test with REAL downloaded EasyList files to verify known ad domains are blocked.
    /// This test only runs if the filter lists have been downloaded.
    #[test]
    fn test_real_easylist_blocks_known_ad_domains() {
        // Try multiple possible locations for the downloaded lists
        let mut possible_dirs = vec![
            PathBuf::from(".").join("adblock").join("lists"),
        ];

        // Platform-specific app data directories
        #[cfg(target_os = "windows")]
        if let Ok(appdata) = std::env::var("APPDATA") {
            possible_dirs.insert(0,
                PathBuf::from(appdata).join(app_dir_name()).join("adblock").join("lists"));
        }

        #[cfg(target_os = "macos")]
        if let Some(home) = std::env::var_os("HOME") {
            possible_dirs.insert(0,
                PathBuf::from(home).join("Library").join("Application Support")
                    .join(app_dir_name()).join("adblock").join("lists"));
        }

        let lists_dir = possible_dirs.iter().find(|d| d.join("easylist.txt").exists());
        let lists_dir = match lists_dir {
            Some(d) => d,
            None => {
                eprintln!("SKIP: No downloaded filter lists found — run the engine once first");
                return;
            }
        };

        let easylist = std::fs::read_to_string(lists_dir.join("easylist.txt")).unwrap();
        let easyprivacy = std::fs::read_to_string(lists_dir.join("easyprivacy.txt")).unwrap();

        let mut filter_set = FilterSet::new(false);
        filter_set.add_filter_list(&easylist, ParseOptions::default());
        filter_set.add_filter_list(&easyprivacy, ParseOptions::default());
        let engine = Engine::from_filter_set(filter_set, true);

        // Known ad/tracker domains that EasyList should definitely block
        let blocked_urls = vec![
            ("https://securepubads.g.doubleclick.net/tag/js/gpt.js", "https://news.com", "script"),
            ("https://pagead2.googlesyndication.com/pagead/js/adsbygoogle.js", "https://news.com", "script"),
            ("https://googleads.g.doubleclick.net/pagead/id", "https://youtube.com", "xmlhttprequest"),
            ("https://static.doubleclick.net/instream/ad_status.js", "https://youtube.com", "script"),
            ("https://ad.doubleclick.net/ddm/trackclk/N123.456", "https://news.com", "image"),
        ];

        for (url, source, rtype) in &blocked_urls {
            let req = Request::new(url, source, rtype).unwrap();
            let result = engine.check_network_request(&req);
            println!("  {} => matched={}, filter={:?}", url, result.matched, result.filter);
            assert!(result.matched, "EasyList should block: {}", url);
        }

        // Known safe URLs that should NOT be blocked
        let allowed_urls = vec![
            ("https://coingeek.com/page.html", "https://coingeek.com", "document"),
            ("https://www.google.com/search?q=test", "https://google.com", "document"),
            ("https://cdn.example.com/app.js", "https://example.com", "script"),
        ];

        for (url, source, rtype) in &allowed_urls {
            let req = Request::new(url, source, rtype).unwrap();
            let result = engine.check_network_request(&req);
            assert!(!result.matched, "Should NOT block: {}", url);
        }
    }

    // ========================================================================
    // Entity Map Tests
    // ========================================================================

    #[test]
    fn test_entity_map_parses_embedded_json() {
        let map = EntityMap::from_json(ENTITIES_JSON);
        assert!(map.domain_count() > 1000, "should have >1000 domains, got {}", map.domain_count());
    }

    #[test]
    fn test_entity_map_same_entity() {
        let map = EntityMap::from_json(ENTITIES_JSON);
        // X Corp: x.com, twitter.com, twimg.com are same entity
        assert!(map.is_same_entity("x.com", "twimg.com"), "x.com and twimg.com should be same entity");
        assert!(map.is_same_entity("twitter.com", "twimg.com"), "twitter.com and twimg.com should be same entity");
        assert!(map.is_same_entity("x.com", "t.co"), "x.com and t.co should be same entity");

        // Google: google.com, gstatic.com, googleapis.com, youtube.com
        assert!(map.is_same_entity("google.com", "gstatic.com"), "google.com and gstatic.com should be same entity");
        assert!(map.is_same_entity("google.com", "googleapis.com"), "google.com and googleapis.com should be same entity");
        assert!(map.is_same_entity("youtube.com", "googlevideo.com"), "youtube.com and googlevideo.com should be same entity");

        // Different entities should NOT match
        assert!(!map.is_same_entity("x.com", "google.com"), "x.com and google.com should be different entities");
        assert!(!map.is_same_entity("facebook.com", "google.com"), "facebook.com and google.com should be different entities");
    }

    #[test]
    fn test_entity_map_subdomain_walking() {
        let map = EntityMap::from_json(ENTITIES_JSON);
        // Subdomains should walk up to the registrable domain
        assert!(map.is_same_entity("pbs.twimg.com", "x.com"), "pbs.twimg.com should match x.com via twimg.com");
        assert!(map.is_same_entity("video.twimg.com", "twitter.com"), "video.twimg.com should match twitter.com");
        assert!(map.is_same_entity("ssl.gstatic.com", "accounts.google.com"), "ssl.gstatic.com should match accounts.google.com");
        assert!(map.is_same_entity("fonts.googleapis.com", "google.com"), "fonts.googleapis.com should match google.com");
    }

    #[test]
    fn test_entity_map_unknown_domains() {
        let map = EntityMap::from_json(ENTITIES_JSON);
        // Completely unknown domains should not match anything
        assert!(!map.is_same_entity("example.com", "example.org"), "unknown domains should not match");
        assert!(!map.is_same_entity("unknown.test", "x.com"), "unknown domain should not match x.com");
    }

    #[test]
    fn test_extract_domain() {
        assert_eq!(extract_domain("https://pbs.twimg.com/media/photo.jpg"), "pbs.twimg.com");
        assert_eq!(extract_domain("https://www.google.com/search?q=test"), "www.google.com");
        assert_eq!(extract_domain("http://example.com:8080/path"), "example.com");
        assert_eq!(extract_domain("https://x.com"), "x.com");
        assert_eq!(extract_domain("data:text/html,test"), "data");  // edge case — data: URLs are pre-filtered by C++
    }

    #[test]
    fn test_entity_map_from_invalid_json() {
        let map = EntityMap::from_json("not json");
        assert_eq!(map.domain_count(), 0, "invalid JSON should produce empty map");

        let map = EntityMap::from_json("{}");
        assert_eq!(map.domain_count(), 0, "missing entities key should produce empty map");
    }

    /// Integration test: entity-aware blocking allows same-entity CDN resources
    /// that would otherwise be blocked by filter rules
    #[test]
    fn test_entity_aware_check_request() {
        // Set up an AdblockEngine with a filter rule that blocks twimg.com
        let engine = AdblockEngine::new(PathBuf::from("/tmp/test-entity-adblock"));

        // Manually inject an adblock engine with a rule blocking twimg.com
        let rules = "||twimg.com^\n||gstatic.com^\n||tracker.example.net^";
        let inner = engine_from_rules(rules);
        {
            let mut eng = engine.engine.write().unwrap();
            *eng = Some(inner);
        }

        // twimg.com on x.com → same entity → should NOT be blocked
        let (blocked, _, _) = engine.check_request(
            "https://pbs.twimg.com/media/photo.jpg",
            "https://x.com/home",
            "image",
        );
        assert!(!blocked, "pbs.twimg.com on x.com should be allowed (same entity)");

        // gstatic.com on google.com → same entity → should NOT be blocked
        let (blocked, _, _) = engine.check_request(
            "https://ssl.gstatic.com/accounts/ui/logo.png",
            "https://accounts.google.com/signin",
            "image",
        );
        assert!(!blocked, "ssl.gstatic.com on accounts.google.com should be allowed (same entity)");

        // tracker.example.net on news.com → different entity → SHOULD be blocked
        let (blocked, _, _) = engine.check_request(
            "https://tracker.example.net/pixel.gif",
            "https://news.com/article",
            "image",
        );
        assert!(blocked, "tracker.example.net on news.com should be blocked (different entity)");

        // twimg.com on news.com → different entity → SHOULD be blocked
        let (blocked, _, _) = engine.check_request(
            "https://pbs.twimg.com/media/embed.jpg",
            "https://news.com/article",
            "image",
        );
        assert!(blocked, "pbs.twimg.com on news.com should be blocked (different entity)");
    }
}
