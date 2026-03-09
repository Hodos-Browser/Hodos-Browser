//! AdblockEngine — wrapper around Brave's `adblock` crate
//!
//! Manages filter list downloading, engine compilation, serialization for fast
//! startup, and thread-safe request checking via `RwLock<Engine>`.
//!
//! Storage layout:
//! ```
//! %APPDATA%/HodosBrowser/adblock/
//!   engine.dat          # Serialized compiled engine (binary, fast reload)
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

/// Default filter lists — standard (no trusted scriptlet permission)
const FILTER_LISTS: &[(&str, &str)] = &[
    ("easylist.txt", "https://easylist.to/easylist/easylist.txt"),
    ("easyprivacy.txt", "https://easylist.to/easylist/easyprivacy.txt"),
];

/// Trusted filter lists — loaded with PermissionMask::from_bits(1) to enable
/// trusted scriptlets (trusted-replace-xhr-response, trusted-replace-fetch-response)
const TRUSTED_FILTER_LISTS: &[(&str, &str)] = &[
    ("ublock-filters.txt", "https://raw.githubusercontent.com/uBlockOrigin/uAssets/master/filters/filters.txt"),
    ("ublock-privacy.txt", "https://raw.githubusercontent.com/uBlockOrigin/uAssets/master/filters/privacy.txt"),
];

/// uBlock Origin scriptlets resource file (pinned to 1.48.4 — last version using
/// the old `///`-delimited format that `assemble_scriptlet_resources()` can parse.
/// Post-1.48.x moved to ES module format which is incompatible with adblock-rust 0.10.3.)
const SCRIPTLETS_URL: &str = "https://raw.githubusercontent.com/gorhill/uBlock/1.48.4/assets/resources/scriptlets.js";
const SCRIPTLETS_FILE: &str = "scriptlets.js";

/// Serialized engine filename
const ENGINE_DAT: &str = "engine.dat";

/// Metadata filename
const META_JSON: &str = "meta.json";

/// Configuration version — bump this when filter list URLs, features, or scriptlet
/// loading changes. Forces engine.dat rebuild on next startup.
const CONFIG_VERSION: u32 = 6; // v1 = easylist+easyprivacy only, v2 = +uBlock+scriptlets, v3 = fixed scriptlets URL, v4 = +hodos-unbreak, v5 = +entity-aware blocking, v6 = $generichide suppresses P1 CSS

/// Hodos Browser compatibility exception list (embedded at compile time).
/// Contains #@#+js() blanket exceptions for auth domains where scriptlet injection breaks login.
const HODOS_UNBREAK: &str = include_str!("hodos-unbreak.txt");

/// disconnect.me entity list (embedded at compile time, ~400KB).
/// Maps organizations to their owned domains so same-entity CDN resources
/// (e.g. twimg.com on x.com) are not blocked as trackers.
/// License: CC BY-NC-SA 4.0 (same as Brave's usage).
const ENTITIES_JSON: &str = include_str!("entities.json");

/// Upstream URL for entity list auto-updates (checked every 6 hours alongside filter lists)
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
}

// ============================================================================
// AdblockEngine
// ============================================================================

/// Thread-safe ad & tracker blocking engine
///
/// Wraps `adblock::Engine` with global enable/disable toggle,
/// serialization for fast startup, and filter list management.
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

        // Try loading serialized engine first (fast startup path)
        let engine_path = self.adblock_dir.join(ENGINE_DAT);
        let meta_path = self.adblock_dir.join(META_JSON);

        let (mut engine, meta) = if engine_path.exists() {
            match Self::load_serialized(&engine_path, &meta_path) {
                Ok((eng, meta)) => {
                    // Check if config version matches — rebuild if filter list config changed
                    if meta.config_version != CONFIG_VERSION {
                        info!("Ad blocker: config version changed ({} → {}), rebuilding...",
                            meta.config_version, CONFIG_VERSION);
                        Self::build_from_lists(&self.adblock_dir).await?
                    } else {
                        info!("Ad blocker: loaded serialized engine ({} lists)", meta.lists.len());
                        (eng, meta)
                    }
                }
                Err(e) => {
                    warn!("Ad blocker: failed to load serialized engine: {}", e);
                    warn!("Ad blocker: rebuilding from filter lists...");
                    Self::build_from_lists(&self.adblock_dir).await?
                }
            }
        } else {
            info!("Ad blocker: no serialized engine found, downloading filter lists...");
            Self::build_from_lists(&self.adblock_dir).await?
        };

        // Load scriptlet resources (needed for cosmetic filtering even on deserialized engines)
        let scriptlets_path = self.adblock_dir.join("resources").join(SCRIPTLETS_FILE);
        if scriptlets_path.exists() {
            let resources = assemble_scriptlet_resources(&scriptlets_path);
            let count = resources.len();
            engine.use_resources(resources);
            info!("Ad blocker: loaded {} scriptlet resources from cache", count);
        }

        // Load extra bundled scriptlets (newer ones missing from 1.48.4 old-format file)
        let extra = load_extra_scriptlets(&mut engine);
        if extra > 0 {
            info!("Ad blocker: loaded {} extra bundled scriptlets", extra);
        }

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

        (selectors, resources.injected_script, resources.generichide)
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
        }
    }

    /// Rebuild the engine from downloaded filter lists (hot-swap).
    /// Used by the background update task.
    pub async fn rebuild_engine(&self) -> Result<(), String> {
        let (engine, meta) = Self::build_from_lists(&self.adblock_dir).await?;

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
        let now = now_secs();
        meta.lists.iter().any(|l| l.expires_at > 0 && now >= l.expires_at)
    }

    /// Get the current update version (monotonically increasing on each rebuild).
    pub fn get_update_version(&self) -> u64 {
        self.update_version.load(Ordering::Relaxed)
    }

    // ========================================================================
    // Private helpers
    // ========================================================================

    /// Load engine from serialized binary file
    fn load_serialized(
        engine_path: &Path,
        meta_path: &Path,
    ) -> Result<(Engine, FilterListMeta), String> {
        let data = std::fs::read(engine_path)
            .map_err(|e| format!("Failed to read engine.dat: {}", e))?;

        let mut engine = Engine::default();
        engine.deserialize(&data)
            .map_err(|e| format!("Failed to deserialize engine: {:?}", e))?;

        let meta = if meta_path.exists() {
            let meta_str = std::fs::read_to_string(meta_path)
                .map_err(|e| format!("Failed to read meta.json: {}", e))?;
            serde_json::from_str(&meta_str)
                .unwrap_or_default()
        } else {
            FilterListMeta::default()
        };

        Ok((engine, meta))
    }

    /// Download filter lists and build engine from scratch
    async fn build_from_lists(adblock_dir: &Path) -> Result<(Engine, FilterListMeta), String> {
        let lists_dir = adblock_dir.join("lists");
        let resources_dir = adblock_dir.join("resources");
        std::fs::create_dir_all(&lists_dir)
            .map_err(|e| format!("Failed to create lists directory: {}", e))?;
        std::fs::create_dir_all(&resources_dir)
            .map_err(|e| format!("Failed to create resources directory: {}", e))?;

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        let mut filter_set = FilterSet::new(false); // false = no debug info
        let mut list_infos = Vec::new();
        let now = now_secs();

        // Load standard filter lists (EasyList, EasyPrivacy) — default permissions
        for (filename, url) in FILTER_LISTS {
            if let Some(info) = download_and_add_list(
                &client, &lists_dir, filename, url, now, ParseOptions::default(), &mut filter_set
            ).await {
                list_infos.push(info);
            }
        }

        // Load trusted filter lists (uBlock Origin) — with PermissionMask::from_bits(1)
        let trusted_opts = ParseOptions {
            permissions: PermissionMask::from_bits(1),
            ..ParseOptions::default()
        };
        for (filename, url) in TRUSTED_FILTER_LISTS {
            if let Some(info) = download_and_add_list(
                &client, &lists_dir, filename, url, now, trusted_opts.clone(), &mut filter_set
            ).await {
                list_infos.push(info);
            }
        }

        if list_infos.is_empty() {
            return Err("No filter lists available — cannot build engine".to_string());
        }

        // Load Hodos compatibility exceptions (embedded + optional updatable override)
        let unbreak_text = load_unbreak_list(adblock_dir);
        let unbreak_rule_count = unbreak_text.lines().filter(|l| !l.starts_with('!') && !l.trim().is_empty()).count();
        filter_set.add_filter_list(&unbreak_text, ParseOptions::default());
        info!("Ad blocker: loaded hodos-unbreak ({} rules)", unbreak_rule_count);

        // Compile the engine (optimize = true for better performance)
        let mut engine = Engine::from_filter_set(filter_set, true);

        // Download and load scriptlet resources (for cosmetic filtering + YouTube)
        let scriptlets_path = resources_dir.join(SCRIPTLETS_FILE);
        match download_filter_list(&client, SCRIPTLETS_URL).await {
            Ok(text) => {
                if let Err(e) = std::fs::write(&scriptlets_path, &text) {
                    warn!("Ad blocker: failed to save scriptlets.js: {}", e);
                }
            }
            Err(e) => {
                warn!("Ad blocker: failed to download scriptlets.js: {}", e);
            }
        }
        if scriptlets_path.exists() {
            let resources = assemble_scriptlet_resources(&scriptlets_path);
            let count = resources.len();
            engine.use_resources(resources);
            info!("Ad blocker: loaded {} scriptlet resources", count);
        }

        // Load extra bundled scriptlets (newer ones missing from 1.48.4 old-format file)
        let extra = load_extra_scriptlets(&mut engine);
        if extra > 0 {
            info!("Ad blocker: loaded {} extra bundled scriptlets", extra);
        }

        // Download updated entity list (disconnect.me same-org domain mapping)
        // Non-critical: if download fails, embedded version is used at runtime
        match download_filter_list(&client, ENTITIES_URL).await {
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

        // Serialize for fast startup
        match engine.serialize() {
            Ok(serialized) => {
                let engine_path = adblock_dir.join(ENGINE_DAT);
                if let Err(e) = std::fs::write(&engine_path, &serialized) {
                    warn!("Ad blocker: failed to save engine.dat: {}", e);
                } else {
                    info!("Ad blocker: engine serialized ({} KB)", serialized.len() / 1024);
                }
            }
            Err(e) => {
                warn!("Ad blocker: failed to serialize engine: {:?}", e);
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

/// Download a filter list, add to FilterSet, return metadata on success
async fn download_and_add_list(
    client: &reqwest::Client,
    lists_dir: &Path,
    filename: &str,
    url: &str,
    now: u64,
    parse_options: ParseOptions,
    filter_set: &mut FilterSet,
) -> Option<FilterListInfo> {
    let list_path = lists_dir.join(filename);

    let list_text = match download_filter_list(client, url).await {
        Ok(text) => {
            if let Err(e) = std::fs::write(&list_path, &text) {
                warn!("Ad blocker: failed to save {}: {}", filename, e);
            }
            text
        }
        Err(e) => {
            warn!("Ad blocker: failed to download {}: {}", filename, e);
            match std::fs::read_to_string(&list_path) {
                Ok(text) => {
                    info!("Ad blocker: using cached {} from disk", filename);
                    text
                }
                Err(_) => {
                    warn!("Ad blocker: no cached {} available, skipping", filename);
                    return None;
                }
            }
        }
    };

    let rule_count = list_text.lines().count();
    let size_bytes = list_text.len() as u64;
    let expiry_secs = parse_list_expiry(&list_text);
    let expires_at = now + expiry_secs;

    filter_set.add_filter_list(&list_text, parse_options);

    info!("Ad blocker: loaded {} ({} rules, {} KB)", filename, rule_count, size_bytes / 1024);

    Some(FilterListInfo {
        filename: filename.to_string(),
        url: url.to_string(),
        downloaded_at: now,
        size_bytes,
        rule_count,
        expires_at,
    })
}

/// Download a filter list from URL
async fn download_filter_list(client: &reqwest::Client, url: &str) -> Result<String, String> {
    let response = client.get(url)
        .send()
        .await
        .map_err(|e| format!("HTTP error: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("HTTP {} from {}", response.status(), url));
    }

    response.text()
        .await
        .map_err(|e| format!("Failed to read response body: {}", e))
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
    fn test_engine_serialize_deserialize() {
        let rules = "||ads.example.com^\n||tracker.net^";
        let engine = engine_from_rules(rules);

        let serialized = engine.serialize().expect("serialize should succeed");
        assert!(!serialized.is_empty(), "serialized data should not be empty");

        let mut engine2 = Engine::default();
        engine2.deserialize(&serialized).expect("deserialize should succeed");

        let req = Request::new("https://ads.example.com/banner.js", "https://news.com", "script").unwrap();
        assert!(engine2.check_network_request(&req).matched, "deserialized engine should block ads");
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
                PathBuf::from(appdata).join("HodosBrowser").join("adblock").join("lists"));
        }

        #[cfg(target_os = "macos")]
        if let Some(home) = std::env::var_os("HOME") {
            possible_dirs.insert(0,
                PathBuf::from(home).join("Library").join("Application Support")
                    .join("HodosBrowser").join("adblock").join("lists"));
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

    /// Test with the serialized engine.dat to verify it works after deserialization
    #[test]
    fn test_real_engine_dat_deserialization() {
        let engine_path = {
            #[cfg(target_os = "windows")]
            {
                PathBuf::from(std::env::var("APPDATA").unwrap_or_default())
                    .join("HodosBrowser").join("adblock").join("engine.dat")
            }
            #[cfg(target_os = "macos")]
            {
                PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
                    .join("Library").join("Application Support")
                    .join("HodosBrowser").join("adblock").join("engine.dat")
            }
            #[cfg(not(any(target_os = "windows", target_os = "macos")))]
            {
                PathBuf::from(".").join("adblock").join("engine.dat")
            }
        };

        if !engine_path.exists() {
            eprintln!("SKIP: No engine.dat found — run the engine once first");
            return;
        }

        let data = std::fs::read(&engine_path).unwrap();
        let mut engine = Engine::default();
        engine.deserialize(&data).expect("engine.dat should deserialize");

        // Test a known blocked URL
        let req = Request::new(
            "https://pagead2.googlesyndication.com/pagead/js/adsbygoogle.js",
            "https://news.com",
            "script",
        ).unwrap();
        let result = engine.check_network_request(&req);
        println!("  googlesyndication => matched={}, filter={:?}", result.matched, result.filter);
        assert!(result.matched, "Deserialized engine should block googlesyndication");
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
