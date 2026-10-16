//! uBlock Origin's current scriptlets, fetched at runtime from a pinned release and converted
//! into adblock-rust `Resource`s (B5-T0-P6).
//!
//! Why: adblock-rust (0.10.3 and still 0.13.3) only parses uBO's pre-2023 single-file
//! resource format, so we were stuck on uBO 1.48.4 and today's YouTube rules name scriptlets
//! that version does not have. adblock-rust *does* run modern function-style scriptlets —
//! the gap is getting them into `Resource` form. Brave converts in Node at packaging time;
//! we convert in Rust at runtime (`js_scan`), from the same source files.
//!
//! Licence: uBO is GPL-3.0. The sources are downloaded at runtime and never embedded in the
//! binary, as with the 1.48.4 file before them.
//!
//! Supply chain: the release is pinned by tag AND by a SHA-256 over every fetched file. A moved
//! tag, a tampered file or a changed import graph fails the hash and the caller falls back.

use crate::js_scan::{self, Import, Module, Value};
use adblock::resources::{MimeType, PermissionMask, Resource, ResourceType};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;

/// uBO release the scriptlets come from. Bumping it means bumping `UBO_SOURCES_SHA256` too;
/// the network test `test_ubo_layout_pinned_release` prints the new value.
pub const UBO_TAG: &str = "1.75.0";

/// SHA-256 over the files `fetch_sources` returns for `UBO_TAG` (see `sources_sha256`).
pub const UBO_SOURCES_SHA256: &str = "3985304b9a2060f91d294944ac400ab576501721788c830609ed94a0261f77bb";

/// The module that imports every other scriptlet module (uBO `src/js/resources/scriptlets.js`).
pub const ENTRY: &str = "resources/scriptlets.js";

/// Defensive caps on what a fetch may pull in (1.75.0: 35 files, ~350 KB).
const MAX_FILES: usize = 100;
const MAX_TOTAL_BYTES: usize = 4 * 1024 * 1024;

/// Cache of the converted set, so a rebuild does not re-download a pinned release.
pub const CACHE_FILE: &str = "ubo-scriptlets.json";

/// Version of the conversion itself (`js_scan` + `convert` + `to_resource`). Bump it when a
/// change would convert the same sources differently, so cached sets are rebuilt.
const CONVERTER_VERSION: u32 = 2; // 2: priority kept

/// Overall deadline for fetching a release (35 files today), so a slow or blackholed host
/// cannot hold a rebuild for 35 x the per-request timeout.
const FETCH_DEADLINE_SECS: u64 = 90;

/// Metadata keys uBO uses today. Anything else is a layout change and fails conversion.
const KNOWN_KEYS: &[&str] = &["name", "aliases", "fn", "dependencies", "requiresTrust", "world", "priority"];

fn base_url(tag: &str) -> String {
    format!("https://raw.githubusercontent.com/gorhill/uBlock/{}/src/js/", tag)
}

/// Resolve an import specifier relative to the importing module, as a path under `src/js/`.
/// Only relative specifiers that stay inside `src/js/` are allowed.
pub fn resolve_spec(from: &str, spec: &str) -> Result<String, String> {
    // `%` (percent-encoded `..`), `?`, `#` and `\` would let the URL parser see a path we
    // did not resolve here.
    if !(spec.starts_with("./") || spec.starts_with("../")) || !spec.ends_with(".js")
        || spec.contains(['%', '?', '#', '\\'])
    {
        return Err(format!("unsupported import specifier {:?} in {}", spec, from));
    }
    let mut parts: Vec<&str> = from.split('/').collect();
    parts.pop(); // the importing file's name
    for seg in spec.split('/') {
        match seg {
            "." => {}
            ".." => {
                if parts.pop().is_none() {
                    return Err(format!("import {:?} in {} leaves src/js/", spec, from));
                }
            }
            "" => return Err(format!("empty path segment in {:?}", spec)),
            s => parts.push(s),
        }
    }
    Ok(parts.join("/"))
}

fn import_spec(i: &Import) -> &str {
    match i {
        Import::SideEffect { spec } | Import::Named { spec, .. } => spec,
    }
}

/// Download `ENTRY` and everything it imports, transitively. Keys are paths under `src/js/`.
pub async fn fetch_sources(client: &reqwest::Client, tag: &str) -> Result<BTreeMap<String, String>, String> {
    let base = base_url(tag);
    let mut sources = BTreeMap::new();
    let mut queue = vec![ENTRY.to_string()];
    let mut total = 0usize;
    while let Some(path) = queue.pop() {
        if sources.contains_key(&path) {
            continue;
        }
        if sources.len() >= MAX_FILES {
            return Err(format!("more than {} files imported", MAX_FILES));
        }
        let url = format!("{}{}", base, path);
        let resp = client.get(&url).send().await.map_err(|e| format!("GET {}: {}", url, e))?;
        if !resp.status().is_success() {
            return Err(format!("HTTP {} from {}", resp.status(), url));
        }
        // Read with the cap applied as bytes arrive, not after the whole body is in memory.
        let mut resp = resp;
        let mut bytes = Vec::new();
        while let Some(chunk) = resp.chunk().await.map_err(|e| format!("reading {}: {}", url, e))? {
            total += chunk.len();
            if total > MAX_TOTAL_BYTES {
                return Err(format!("sources exceed {} bytes", MAX_TOTAL_BYTES));
            }
            bytes.extend_from_slice(&chunk);
        }
        let text = String::from_utf8(bytes).map_err(|_| format!("{} is not UTF-8", url))?;
        let module = js_scan::scan_module(&text).map_err(|e| format!("{}: {}", path, e))?;
        for imp in &module.imports {
            queue.push(resolve_spec(&path, import_spec(imp))?);
        }
        sources.insert(path, text);
    }
    Ok(sources)
}

/// SHA-256 over (path, length, bytes) of every file, in path order. Lowercase hex.
pub fn sources_sha256(sources: &BTreeMap<String, String>) -> String {
    let mut h = Sha256::new();
    for (path, text) in sources {
        h.update(path.as_bytes());
        h.update([0u8]);
        h.update((text.len() as u64).to_le_bytes());
        h.update(text.as_bytes());
    }
    h.finalize().iter().map(|b| format!("{:02x}", b)).collect()
}

/// One scriptlet as converted, before encoding into a `Resource` (kept for tests and
/// diagnostics: `code` is the exact JavaScript text).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Converted {
    pub name: String,
    pub aliases: Vec<String>,
    pub code: String,
    pub dependencies: Vec<String>,
    pub requires_trust: bool,
    /// uBO's execution world; adblock-rust has no isolated world, so `ISOLATED` scriptlets run
    /// in the main world like everything else (as the 1.48.4 set always did).
    pub world: String,
    /// uBO runs higher-priority calls first (`scriptlet-filtering-core.js :: sortedCalls`);
    /// e.g. `proxy-apply-config.js` (100) configures `proxyApplyFn` before anything installs
    /// a proxy. adblock-rust runs calls in filter order, so the engine reorders (P6).
    #[serde(default)]
    pub priority: i64,
}

impl Converted {
    pub fn to_resource(&self) -> Resource {
        Resource {
            name: self.name.clone(),
            aliases: self.aliases.clone(),
            kind: ResourceType::Mime(if self.name.ends_with(".fn") {
                MimeType::FnJavascript
            } else {
                MimeType::ApplicationJavascript
            }),
            content: BASE64.encode(&self.code),
            dependencies: self.dependencies.clone(),
            permission: if self.needs_trust() { PermissionMask::from_bits(1) } else { PermissionMask::default() },
        }
    }

    /// Trusted lists only: uBO's `requiresTrust`, OR any name/alias starting `trusted-`.
    /// uBO 1.75.0 registers `trusted-prevent-xhr.js` (arbitrary XHR responses) without
    /// `requiresTrust`; going by the flag alone would let EasyList-level rules forge responses.
    /// `requires_trust` itself stays uBO's own flag, so the Node differential test compares like
    /// with like; the policy is applied here.
    pub fn needs_trust(&self) -> bool {
        self.requires_trust
            || std::iter::once(&self.name).chain(&self.aliases).any(|n| n.starts_with("trusted-"))
    }

    /// The function name an invocation of this scriptlet calls (`function NAME(`).
    pub fn function_name(&self) -> Option<&str> {
        let rest = self.code.strip_prefix("function")?.trim_start();
        let end = rest.find(|c: char| c == '(' || c.is_whitespace())?;
        Some(&rest[..end])
    }
}

/// Modules in ES evaluation order: depth-first, imports in source order, post-order.
fn evaluation_order(modules: &HashMap<String, Module>) -> Result<Vec<String>, String> {
    fn visit(path: &str, modules: &HashMap<String, Module>, seen: &mut HashSet<String>, out: &mut Vec<String>) -> Result<(), String> {
        if !seen.insert(path.to_string()) {
            return Ok(());
        }
        let m = modules.get(path).ok_or_else(|| format!("module {} was not fetched", path))?;
        for imp in &m.imports {
            visit(&resolve_spec(path, import_spec(imp))?, modules, seen, out)?;
        }
        out.push(path.to_string());
        Ok(())
    }
    let mut out = Vec::new();
    visit(ENTRY, modules, &mut HashSet::new(), &mut out)?;
    Ok(out)
}

/// Follow a module-scope identifier to the declaration it names: (module path, decl name).
fn resolve_ident(modules: &HashMap<String, Module>, path: &str, name: &str, depth: usize) -> Result<(String, String), String> {
    if depth > 20 {
        return Err(format!("identifier {:?} in {}: alias/import chain too long", name, path));
    }
    let m = modules.get(path).ok_or_else(|| format!("module {} was not fetched", path))?;
    if m.decls.contains_key(name) {
        return Ok((path.to_string(), name.to_string()));
    }
    if let Some(target) = m.aliases.get(name) {
        return resolve_ident(modules, path, target, depth + 1);
    }
    for imp in &m.imports {
        if let Import::Named { spec, names } = imp {
            if let Some((_, exported)) = names.iter().find(|(local, _)| local == name) {
                return resolve_ident(modules, &resolve_spec(path, spec)?, exported, depth + 1);
            }
        }
    }
    Err(format!("identifier {:?} in {} does not name a top-level function or class", name, path))
}

fn str_list(v: &Value, what: &str) -> Result<Vec<String>, String> {
    match v {
        Value::Array(items) => items.iter().map(|i| match i {
            Value::Str(s) => Ok(s.clone()),
            other => Err(format!("{}: expected strings, found {:?}", what, other)),
        }).collect(),
        other => Err(format!("{}: expected an array, found {:?}", what, other)),
    }
}

/// Convert fetched sources into scriptlets, in uBO's registration order.
pub fn convert(sources: &BTreeMap<String, String>) -> Result<Vec<Converted>, String> {
    let mut modules = HashMap::new();
    for (path, text) in sources {
        modules.insert(path.clone(), js_scan::scan_module(text).map_err(|e| format!("{}: {}", path, e))?);
    }

    // (module, decl) -> registered name. uBO's registerScriptlet turns a function-valued
    // dependency into `fn.details.name` at call time, so the dependency must already be
    // registered — evaluation order reproduces that.
    let mut registered: HashMap<(String, String), String> = HashMap::new();
    let mut out: Vec<Converted> = Vec::new();
    let mut names: HashSet<String> = HashSet::new();
    let mut decl_module: HashMap<String, String> = HashMap::new();

    for path in evaluation_order(&modules)? {
        for (fn_ident, fields) in &modules[&path].registrations {
            let ctx = format!("{} registration of {}", path, fn_ident);
            let get = |k: &str| fields.iter().find(|(key, _)| key == k).map(|(_, v)| v);
            for (key, _) in fields {
                if !KNOWN_KEYS.contains(&key.as_str()) {
                    return Err(format!("{}: unknown key {:?}", ctx, key));
                }
            }
            let name = match get("name") {
                Some(Value::Str(s)) if !s.is_empty() => s.clone(),
                _ => return Err(format!("{}: missing string `name`", ctx)),
            };
            let aliases = match get("aliases") {
                Some(v) => str_list(v, &format!("{} aliases", ctx))?,
                None => Vec::new(),
            };
            let mut dependencies = Vec::new();
            if let Some(v) = get("dependencies") {
                let items = match v {
                    Value::Array(items) => items,
                    other => return Err(format!("{}: dependencies is {:?}", ctx, other)),
                };
                for d in items {
                    dependencies.push(match d {
                        Value::Str(s) => s.clone(),
                        Value::Ident(id) => {
                            let key = resolve_ident(&modules, &path, id, 0).map_err(|e| format!("{}: {}", ctx, e))?;
                            registered.get(&key).cloned().ok_or_else(|| {
                                format!("{}: dependency {} is not registered before use", ctx, id)
                            })?
                        }
                        other => return Err(format!("{}: dependency {:?}", ctx, other)),
                    });
                }
            }
            let requires_trust = match get("requiresTrust") {
                None => false,
                Some(Value::Bool(b)) => *b,
                Some(other) => return Err(format!("{}: requiresTrust is {:?}", ctx, other)),
            };
            let world = match get("world") {
                None => "MAIN".to_string(),
                Some(Value::Str(s)) if s == "MAIN" || s == "ISOLATED" => s.clone(),
                Some(other) => return Err(format!("{}: world is {:?}", ctx, other)),
            };
            let priority = match get("priority") {
                None => 0,
                Some(Value::Num(n)) => n.parse::<i64>().map_err(|_| format!("{}: priority {:?} is not an integer", ctx, n))?,
                Some(other) => return Err(format!("{}: priority is {:?}", ctx, other)),
            };

            let key = resolve_ident(&modules, &path, fn_ident, 0).map_err(|e| format!("{}: {}", ctx, e))?;
            let code = modules[&key.0].decls[&key.1].clone();
            if !name.ends_with(".fn") {
                // adblock-rust invokes a scriptlet function-style only when its text starts
                // `function NAME(`; anything else would be treated as an old template.
                let head = code.split('(').next().unwrap_or("");
                if head.trim() != format!("function {}", key.1) {
                    return Err(format!("{}: injectable scriptlet is not a plain function declaration", ctx));
                }
            }
            for n in std::iter::once(&name).chain(aliases.iter()) {
                if !names.insert(n.clone()) {
                    return Err(format!("{}: name {:?} registered twice", ctx, n));
                }
            }
            // adblock-rust concatenates every dependency into one script: two modules' top-level
            // declarations with the same name would silently shadow each other (or, for a class,
            // be a SyntaxError). ES modules allow it; the injected script does not.
            if let Some(other) = decl_module.insert(key.1.clone(), key.0.clone()) {
                if other != key.0 {
                    return Err(format!("{}: `{}` is declared in both {} and {}", ctx, key.1, other, key.0));
                }
            }
            if registered.insert(key, name.clone()).is_some() {
                return Err(format!("{}: the same function is registered twice", ctx));
            }
            out.push(Converted { name, aliases, code, dependencies, requires_trust, world, priority });
        }
    }

    // Every dependency must name something in the set, or the engine silently drops the rule.
    for c in &out {
        for d in &c.dependencies {
            if !names.contains(d) {
                return Err(format!("{} depends on {:?}, which is not registered", c.name, d));
            }
        }
    }
    if out.is_empty() {
        return Err("no scriptlets registered".into());
    }
    Ok(out)
}

#[derive(Serialize, Deserialize)]
struct Cache {
    tag: String,
    sources_sha256: String,
    /// Absent in a cache written before the field existed: reads as 0, a mismatch.
    #[serde(default)]
    converter_version: u32,
    scriptlets: Vec<Converted>,
}

/// Whatever converted set is cached, for ANY pin or converter, with its tag. A fallback only:
/// after a pin bump whose fetch fails (offline first start), the last hash-verified modern set
/// beats the 2023 single-file set. Never touches the network.
pub fn cached_any(resources_dir: &Path) -> Option<(String, Vec<Converted>)> {
    let text = std::fs::read_to_string(resources_dir.join(CACHE_FILE)).ok()?;
    let c = serde_json::from_str::<Cache>(&text).ok()?;
    (!c.scriptlets.is_empty()).then_some((c.tag, c.scriptlets))
}

/// The cached converted set, if it was made for the current pin by the current converter.
/// Never touches the network.
pub fn cached(resources_dir: &Path) -> Option<Vec<Converted>> {
    let text = std::fs::read_to_string(resources_dir.join(CACHE_FILE)).ok()?;
    match serde_json::from_str::<Cache>(&text) {
        Ok(c) if c.tag == UBO_TAG && c.sources_sha256 == UBO_SOURCES_SHA256
            && c.converter_version == CONVERTER_VERSION && !c.scriptlets.is_empty() => Some(c.scriptlets),
        Ok(_) => {
            log::info!("Ad blocker: cached uBO scriptlets are for another pin or converter");
            None
        }
        Err(e) => {
            log::warn!("Ad blocker: unreadable {}: {}", CACHE_FILE, e);
            None
        }
    }
}

/// The converted set for `UBO_TAG`: from the cache when it matches the pin, else fetched,
/// hash-checked, converted and cached. An `Err` means the caller must keep what it has.
pub async fn obtain(resources_dir: &Path, client: &reqwest::Client) -> Result<Vec<Converted>, String> {
    if let Some(set) = cached(resources_dir) {
        return Ok(set);
    }
    let cache_path = resources_dir.join(CACHE_FILE);

    let deadline = std::time::Duration::from_secs(FETCH_DEADLINE_SECS);
    let sources = tokio::time::timeout(deadline, fetch_sources(client, UBO_TAG)).await
        .map_err(|_| format!("fetching uBO {} took longer than {} s", UBO_TAG, FETCH_DEADLINE_SECS))??;
    let hash = sources_sha256(&sources);
    if hash != UBO_SOURCES_SHA256 {
        return Err(format!("uBO {} sources hash {} does not match the pinned {}", UBO_TAG, hash, UBO_SOURCES_SHA256));
    }
    let scriptlets = convert(&sources)?;
    let cache = Cache { tag: UBO_TAG.into(), sources_sha256: hash, converter_version: CONVERTER_VERSION, scriptlets };
    match serde_json::to_string(&cache) {
        Ok(json) => {
            if let Err(e) = std::fs::write(&cache_path, json) {
                log::warn!("Ad blocker: failed to save {}: {}", CACHE_FILE, e);
            }
        }
        Err(e) => log::warn!("Ad blocker: failed to encode {}: {}", CACHE_FILE, e),
    }
    Ok(cache.scriptlets)
}

/// Test seam: write a cache for the current pin and converter, so engine tests can run the
/// real install/refresh paths offline.
#[cfg(test)]
pub(crate) fn write_test_cache(resources_dir: &Path, scriptlets: Vec<Converted>) {
    std::fs::create_dir_all(resources_dir).unwrap();
    let c = Cache { tag: UBO_TAG.into(), sources_sha256: UBO_SOURCES_SHA256.into(), converter_version: CONVERTER_VERSION, scriptlets };
    std::fs::write(resources_dir.join(CACHE_FILE), serde_json::to_string(&c).unwrap()).unwrap();
}

/// Test seam: a cache as an OLDER release (another tag, hash and converter) would have left it.
#[cfg(test)]
pub(crate) fn write_test_cache_for_other_pin(resources_dir: &Path, tag: &str, scriptlets: Vec<Converted>) {
    std::fs::create_dir_all(resources_dir).unwrap();
    let c = Cache { tag: tag.into(), sources_sha256: "0".repeat(64), converter_version: 1, scriptlets };
    std::fs::write(resources_dir.join(CACHE_FILE), serde_json::to_string(&c).unwrap()).unwrap();
}

/// Test seam: a small converted set (the miniature layout from the tests below).
#[cfg(test)]
pub(crate) fn test_converted_set() -> Vec<Converted> {
    convert(&tests::mini()).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn src(files: &[(&str, &str)]) -> BTreeMap<String, String> {
        files.iter().map(|(p, t)| (p.to_string(), t.to_string())).collect()
    }

    /// A miniature of uBO's layout, written for this test (not uBO code): the entry imports
    /// modules, `registerScriptlet` with a function-valued dependency, `builtinScriptlets.push`
    /// with string dependencies, an import alias through `shared.js`, and a class from outside
    /// `resources/`.
    pub(crate) fn mini() -> BTreeMap<String, String> {
        src(&[
            (ENTRY, "import './edit.js';\nimport { helperFn } from './utils.js';\n\
                     import { registeredScriptlets } from './base.js';\n\
                     export const builtinScriptlets = registeredScriptlets;\n\
                     function mainOne(a) { helperFn(a); }\n\
                     builtinScriptlets.push({ name: 'main-one.js', aliases: [ 'm1.js' ], fn: mainOne, dependencies: [ 'helper.fn' ] });"),
            ("resources/base.js", "export const registeredScriptlets = [];\n\
                     export const registerScriptlet = (fn, details) => { registeredScriptlets.push(details); };"),
            ("resources/utils.js", "import { registerScriptlet } from './base.js';\n\
                     export function helperFn(x) { return `${x}}`; }\n\
                     registerScriptlet(helperFn, { name: 'helper.fn' });"),
            ("resources/shared.js", "import { Q as __Q__ } from '../q.js';\nimport { registerScriptlet } from './base.js';\n\
                     export const Q = __Q__;\nregisterScriptlet(Q, { name: 'q.fn' });"),
            ("q.js", "export class Q { run() { return /}/.test('}'); } }"),
            ("resources/edit.js", "import { registerScriptlet } from './base.js';\nimport { Q } from './shared.js';\n\
                     import { helperFn } from './utils.js';\n\
                     function editFn(p) { return new Q().run(p); }\n\
                     registerScriptlet(editFn, { name: 'trusted-edit.js', requiresTrust: true, world: 'ISOLATED', dependencies: [ Q, helperFn ] });"),
        ])
    }

    #[test]
    fn converts_the_miniature_layout() {
        let out = convert(&mini()).unwrap();
        let by: HashMap<_, _> = out.iter().map(|c| (c.name.as_str(), c)).collect();
        assert_eq!(by.len(), 4);
        assert_eq!(by["q.fn"].code, "class Q { run() { return /}/.test('}'); } }");
        assert_eq!(by["helper.fn"].code, "function helperFn(x) { return `${x}}`; }");
        let edit = by["trusted-edit.js"];
        assert_eq!(edit.dependencies, vec!["q.fn", "helper.fn"]);
        assert!(edit.requires_trust);
        assert_eq!(edit.world, "ISOLATED");
        assert_eq!(by["main-one.js"].aliases, vec!["m1.js"]);
        // Registration order follows ES evaluation order: imports before the entry's own code.
        assert_eq!(out.last().unwrap().name, "main-one.js");
    }

    #[test]
    fn resources_have_the_engine_shape() {
        let out = convert(&mini()).unwrap();
        let r: Vec<Resource> = out.iter().map(Converted::to_resource).collect();
        let edit = r.iter().find(|r| r.name == "trusted-edit.js").unwrap();
        assert_eq!(edit.kind, ResourceType::Mime(MimeType::ApplicationJavascript));
        assert_eq!(format!("{:?}", edit.permission), format!("{:?}", PermissionMask::from_bits(1)));
        let helper = r.iter().find(|r| r.name == "helper.fn").unwrap();
        assert_eq!(helper.kind, ResourceType::Mime(MimeType::FnJavascript));
        assert_eq!(BASE64.decode(&helper.content).unwrap(), b"function helperFn(x) { return `${x}}`; }");
    }

    #[test]
    fn layout_changes_fail_instead_of_shrinking() {
        let mutate = |path: &str, from: &str, to: &str| {
            let mut s = mini();
            let t = s.get_mut(path).unwrap();
            assert!(t.contains(from), "fixture lacks {:?}", from);
            *t = t.replace(from, to);
            convert(&s)
        };
        // An unknown metadata key.
        assert!(mutate("resources/utils.js", "{ name: 'helper.fn' }", "{ name: 'helper.fn', runAt: 1 }").is_err());
        // A dependency on something never registered.
        assert!(mutate(ENTRY, "dependencies: [ 'helper.fn' ]", "dependencies: [ 'gone.fn' ]").is_err());
        // A function-valued dependency registered only later.
        assert!(mutate("resources/edit.js", "dependencies: [ Q, helperFn ]", "dependencies: [ Q, editFn ]").is_err());
        // An identifier that names nothing.
        assert!(mutate("resources/utils.js", "registerScriptlet(helperFn", "registerScriptlet(missingFn").is_err());
        // Duplicate name.
        assert!(mutate(ENTRY, "name: 'main-one.js'", "name: 'trusted-edit.js'").is_err());
        // An injectable scriptlet that is not a plain `function NAME(` declaration.
        assert!(mutate(ENTRY, "function mainOne(a)", "async function mainOne(a)").is_err());
        // A missing module.
        let mut s = mini();
        s.remove("q.js");
        assert!(convert(&s).is_err());
    }

    #[test]
    fn spec_resolution_stays_inside_src_js() {
        assert_eq!(resolve_spec("resources/a.js", "./b.js").unwrap(), "resources/b.js");
        assert_eq!(resolve_spec("resources/a.js", "../jsonpath.js").unwrap(), "jsonpath.js");
        assert!(resolve_spec("resources/a.js", "../../x.js").is_err());
        assert!(resolve_spec("resources/a.js", "https://evil.example/x.js").is_err());
        assert!(resolve_spec("resources/a.js", "/abs.js").is_err());
        assert!(resolve_spec("resources/a.js", "./b.mjs").is_err());
        assert!(resolve_spec("resources/a.js", "./%2e%2e/%2e%2e/x.js").is_err());
        assert!(resolve_spec("resources/a.js", "./b.js?x=.js").is_err());
        assert!(resolve_spec("resources/a.js", "./b.js#.js").is_err());
        assert!(resolve_spec("resources/a.js", r"./a\..\..\..\x.js").is_err());
    }

    /// The layout check (owner, 2026-10-13): the pinned release still fetches, matches its
    /// hash and converts. Prints the hash for a pin bump. Network-bound:
    /// `cargo test -- --ignored test_ubo_layout_pinned_release`.
    #[tokio::test]
    #[ignore]
    async fn test_ubo_layout_pinned_release() {
        let client = reqwest::Client::new();
        let sources = fetch_sources(&client, UBO_TAG).await.expect("fetch");
        let hash = sources_sha256(&sources);
        println!("uBO {}: {} files, {} bytes, sources_sha256 = {}", UBO_TAG, sources.len(),
            sources.values().map(|s| s.len()).sum::<usize>(), hash);
        let out = convert(&sources).expect("convert");
        let main = out.iter().filter(|c| !c.name.ends_with(".fn")).count();
        println!("converted {} scriptlets ({} injectable, {} helpers)", out.len(), main, out.len() - main);
        assert_eq!(hash, UBO_SOURCES_SHA256, "pinned hash is stale for {}", UBO_TAG);
    }

    /// Differential test: our conversion equals what uBO's own module code produces when Node
    /// loads it. The independent answer is the JavaScript engine, not our reading of uBO.
    /// Network + Node: `cargo test -- --ignored test_ubo_conversion_matches_node`.
    #[tokio::test]
    #[ignore]
    async fn test_ubo_conversion_matches_node() {
        let client = reqwest::Client::new();
        let sources = fetch_sources(&client, UBO_TAG).await.expect("fetch");
        let ours = convert(&sources).expect("convert");

        let dir = std::env::temp_dir().join(format!("hodos-p6-ubo-{}-{}", UBO_TAG, std::process::id()));
        for (rel, text) in &sources {
            let p = dir.join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(&p, text).unwrap();
        }
        // uBO's .js files are ES modules.
        std::fs::write(dir.join("package.json"), r#"{ "type": "module" }"#).unwrap();
        let script = dir.join("ubo-reference-dump.mjs");
        std::fs::write(&script, include_str!("../scripts/ubo-reference-dump.mjs")).unwrap();

        let run = std::process::Command::new("node").arg(&script).arg(&dir).output().expect("node must be installed");
        assert!(run.status.success(), "node failed: {}", String::from_utf8_lossy(&run.stderr));
        let reference: Vec<Converted> = serde_json::from_slice(&run.stdout).expect("node output");

        assert_eq!(ours.len(), reference.len(), "scriptlet count differs");
        let mut diffs = Vec::new();
        for (o, r) in ours.iter().zip(&reference) {
            if o != r {
                let field = if o.name != r.name { "name (order)" } else if o.code != r.code { "code" }
                    else if o.dependencies != r.dependencies { "dependencies" } else if o.aliases != r.aliases { "aliases" }
                    else { "trust/world" };
                diffs.push(format!("{} vs {}: {}", o.name, r.name, field));
            }
        }
        assert!(diffs.is_empty(), "{} of {} differ from Node:\n{}", diffs.len(), ours.len(), diffs.join("\n"));
        println!("{} scriptlets identical to Node's reference", ours.len());
    }

    /// A client whose every request fails fast (nothing listens on port 9).
    fn offline_client() -> reqwest::Client {
        reqwest::Client::builder().proxy(reqwest::Proxy::all("http://127.0.0.1:9").unwrap()).build().unwrap()
    }

    fn temp_dir(tag: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("hodos-p6-obtain-{}-{}", tag, std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn write_cache(dir: &Path, tag: &str, sha: &str, scriptlets: Vec<Converted>) {
        write_cache_v(dir, tag, sha, CONVERTER_VERSION, scriptlets)
    }

    fn write_cache_v(dir: &Path, tag: &str, sha: &str, converter_version: u32, scriptlets: Vec<Converted>) {
        let c = Cache { tag: tag.into(), sources_sha256: sha.into(), converter_version, scriptlets };
        std::fs::write(dir.join(CACHE_FILE), serde_json::to_string(&c).unwrap()).unwrap();
    }

    /// A cache for the current pin is used without touching the network.
    #[tokio::test]
    async fn obtain_uses_a_matching_cache_offline() {
        let dir = temp_dir("hit");
        let set = convert(&mini()).unwrap();
        write_cache(&dir, UBO_TAG, UBO_SOURCES_SHA256, set.clone());
        assert_eq!(obtain(&dir, &offline_client()).await.unwrap(), set);
    }

    /// A cache for another pin (tag or hash) is not used; with no network that is an `Err`,
    /// which the engine answers by keeping what it runs.
    #[tokio::test]
    async fn obtain_refuses_a_cache_for_another_pin() {
        let set = convert(&mini()).unwrap();
        let dir = temp_dir("tag");
        write_cache(&dir, "1.74.0", UBO_SOURCES_SHA256, set.clone());
        assert!(obtain(&dir, &offline_client()).await.is_err());
        let dir = temp_dir("sha");
        write_cache(&dir, UBO_TAG, &"0".repeat(64), set);
        assert!(obtain(&dir, &offline_client()).await.is_err());
        let dir = temp_dir("none");
        assert!(obtain(&dir, &offline_client()).await.is_err());
        // A set converted by an older converter is not used either.
        let dir = temp_dir("conv");
        write_cache_v(&dir, UBO_TAG, UBO_SOURCES_SHA256, CONVERTER_VERSION - 1, convert(&mini()).unwrap());
        assert!(cached(&dir).is_none());
        assert!(obtain(&dir, &offline_client()).await.is_err());
    }

    /// Early warning for a pin bump (owner, 2026-10-13: "check uBO's file layout on every
    /// build"): uBO's LATEST release must still fetch and convert. Users stay on the pinned
    /// release; this tells us before we bump. Network; warning-only in CI:
    /// `cargo test -- --ignored test_latest_ubo_release_converts`.
    #[tokio::test]
    #[ignore]
    async fn test_latest_ubo_release_converts() {
        let client = reqwest::Client::builder().user_agent("hodos-adblock-layout-check").build().unwrap();
        let latest: serde_json::Value = client.get("https://api.github.com/repos/gorhill/uBlock/releases/latest")
            .send().await.expect("GitHub API").json().await.expect("release JSON");
        let tag = latest["tag_name"].as_str().expect("tag_name").to_string();
        let sources = fetch_sources(&client, &tag).await.unwrap_or_else(|e| panic!("uBO {} fetch: {}", tag, e));
        let out = convert(&sources).unwrap_or_else(|e| panic!("uBO {} no longer converts: {}", tag, e));
        println!("latest uBO {}: {} files, {} scriptlets{}", tag, sources.len(), out.len(),
            if tag == UBO_TAG { " (= our pin)" } else { " — newer than our pin; bump UBO_TAG when ready" });
    }

    #[test]
    fn trusted_prefix_requires_trust_even_without_the_flag() {
        let mut c = convert(&mini()).unwrap().into_iter().find(|c| c.name == "main-one.js").unwrap();
        assert!(!c.needs_trust());
        c.name = "trusted-prevent-xhr.js".into();
        assert!(!c.requires_trust, "uBO's own flag is kept as uBO wrote it");
        assert!(c.needs_trust());
        assert_eq!(format!("{:?}", c.to_resource().permission), format!("{:?}", PermissionMask::from_bits(1)));
        c.name = "plain.js".into();
        c.aliases = vec!["trusted-alias.js".into()];
        assert!(c.needs_trust(), "a trusted- alias counts too");
    }

    #[test]
    fn priority_is_kept_and_names_are_found() {
        let mut s = mini();
        let e = s.get_mut("resources/edit.js").unwrap();
        *e = e.replace("requiresTrust: true,", "requiresTrust: true, priority: 100,");
        let out = convert(&s).unwrap();
        let edit = out.iter().find(|c| c.name == "trusted-edit.js").unwrap();
        assert_eq!(edit.priority, 100);
        assert_eq!(edit.function_name(), Some("editFn"));
        assert_eq!(out.iter().find(|c| c.name == "q.fn").unwrap().function_name(), None, "a class has no function name");
    }

    #[test]
    fn a_name_declared_in_two_modules_is_rejected() {
        let mut s = mini();
        let e = s.get_mut("resources/edit.js").unwrap();
        // edit.js declares its own helperFn and registers it under a second name.
        *e = format!("{}\nfunction helperFn(y) {{ return y; }}\nregisterScriptlet(helperFn, {{ name: 'helper2.fn' }});", e)
            .replace("import { helperFn } from './utils.js';\n", "");
        *e = e.replace("dependencies: [ Q, helperFn ]", "dependencies: [ Q ]");
        let err = convert(&s).unwrap_err();
        assert!(err.contains("helperFn") && err.contains("declared in both"), "{}", err);
    }

    #[test]
    fn cached_any_returns_another_pins_set() {
        let dir = temp_dir("any");
        write_cache_v(&dir, "1.70.0", &"0".repeat(64), 1, convert(&mini()).unwrap());
        assert!(cached(&dir).is_none());
        let (tag, set) = cached_any(&dir).unwrap();
        assert_eq!((tag.as_str(), set.len()), ("1.70.0", 4));
    }

    #[test]
    fn hash_covers_paths_and_contents() {
        let a = sources_sha256(&src(&[("a.js", "x"), ("b.js", "y")]));
        assert_ne!(a, sources_sha256(&src(&[("a.js", "x"), ("b.js", "z")])));
        assert_ne!(a, sources_sha256(&src(&[("a.js", "x"), ("c.js", "y")])));
        // Length-prefixing: moving bytes between files changes the hash.
        assert_ne!(sources_sha256(&src(&[("a.js", "xy"), ("b.js", "")])), sources_sha256(&src(&[("a.js", "x"), ("b.js", "y")])));
        assert_eq!(a.len(), 64);
    }
}
