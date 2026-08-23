//! Wallet-manifest fetch + parse. Rust half of a two-layer parser: the C++
//! `ManifestFetcher` (`cef-native/src/core/ManifestFetcher.cpp`) re-parses the
//! same bytes to build the modal, so **the two must move together** — a fix
//! here with a test driven only through here proves nothing (`P0.8-A1`).
//!
//! ## What is parsed (beta.3 Phase 0.8)
//!
//! Three declaration shapes, in precedence order:
//!
//! 1. `metanet.groupPermissions` — **BRC-73**, the canonical interoperable
//!    namespace. *"The current interoperable manifest namespace is
//!    `metanet.groupPermissions`."*
//! 2. `babbage.groupPermissions` — the deprecated legacy namespace. BRC-116
//!    Backwards Compatibility: *"Check for `metanet` first. If present, use it.
//!    If `metanet` is absent, fall back to `babbage`."*
//! 3. top-level `permissions` — **ours**, from `PERMISSION_UX_DESIGN.md` §5.
//!    Zero observed adopters in the 2026-08-22 survey, but it is our own shape
//!    and stays supported.
//!
//! BRC-73 is a *translation layer* into the struct the permission engine
//! already consumes. ⛔ The decision engine is not modified by any of this.
//!
//! ## Validity is deliberately NOT lenient about permissions
//!
//! 🚨 Phase 0.8's core defect: this function used to return `Some` for **any**
//! JSON object, so a plain W3C web-app manifest with no permission namespace at
//! all set `manifest_present = true` in
//! `permission_service::request_gate::domain_trust_gate`, and the user got a
//! `manifest_connect_bundle` modal itemising **nothing** — reading as "allow
//! identity and allow operations without asking" with no guardrails. bitgenius
//! declared four protocols and the modal showed zero.
//!
//! `Some` now means **"at least one recognised permission was declared"**. A
//! document we cannot read falls back to plain `domain_approval`, which is the
//! honest prompt. The fail-loud fallback in
//! `HttpRequestInterceptor.cpp :: tryHandlePendingResponse` already existed —
//! this is what finally lets it fire.
//!
//! ## Hostile input
//!
//! A manifest is attacker-controlled bytes from an untrusted origin. The 64 KB
//! fetch cap is **not** a parse cap: 64 KB of JSON can still declare thousands
//! of entries. Every array is length-capped and every string is truncated
//! (`MAX_ENTRIES` / `MAX_STR` below). Descriptions are rendered in the connect
//! modal, so they are treated as display-only untrusted text — BRC-116 §4.2:
//! *"Wallets MUST treat application-provided description text as untrusted.
//! Wallets MUST compute and display authoritative satoshi amounts from
//! structured numeric fields … rather than relying on free-form description
//! text."*
//!
//! Neither `parse_manifest` nor `fetch_manifest` ever panics — any failure
//! yields `None` (mirrors the C++ `Manifest{valid=false}` contract).

use serde::Serialize;

/// 3-second hard cap — the user is already waiting on connect, but a slow or
/// hostile server must not hang the prompt.
const FETCH_TIMEOUT_MS: u64 = 3000;
/// 64 KB cap. Manifests are tiny; anything larger is rejected without parsing.
const MAX_MANIFEST_BYTES: usize = 64 * 1024;

/// Max entries kept per permission array. A hostile 64 KB manifest can declare
/// thousands of one-line entries; the modal renders every one of them. Anything
/// past this is dropped. 64 is ~16× the largest real manifest observed
/// (bitgenius declares 4 protocols).
pub const MAX_ENTRIES: usize = 64;
/// Max certificate fields kept per certificate entry.
pub const MAX_FIELDS: usize = 64;
/// Max length of a free-text display string (name / description / purpose).
pub const MAX_STR: usize = 512;
/// Max length of an identifier-ish string (protocol name, basket, cert type,
/// counterparty / verifier pubkey hex — a compressed pubkey is 66 chars).
pub const MAX_ID_STR: usize = 256;

/// Which declaration shape a manifest's permissions came from. Display +
/// diagnostics only — never a decision input.
pub const SOURCE_METANET: &str = "metanet";
pub const SOURCE_BABBAGE: &str = "babbage";
pub const SOURCE_HODOS_LEGACY: &str = "hodos-legacy";

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestProtocol {
    pub security_level: i32,
    pub name: String,
    pub key_id: String,
    /// BRC-73 `counterparty`. `'self'`, `'anyone'`, or a compressed pubkey.
    /// Empty = unspecified (Level 1 MAY omit it; a Level 2 entry that omits it
    /// is kept and shown as "any counterparty" rather than silently dropped —
    /// dropping would *understate* what the site asked for, which is the exact
    /// defect this phase closes).
    pub counterparty: String,
    pub purpose: String,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestBasket {
    pub name: String,
    pub access: String,
    pub purpose: String,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestCertificate {
    pub cert_type: String,
    pub fields: Vec<String>,
    /// BRC-73 `verifierPublicKey` — who receives the disclosed fields. BRC-116
    /// §4.4 scopes a certificate permission by type + verifier + fields, so the
    /// user is shown the verifier. Display only: our `cert_field_permissions`
    /// rows are keyed by (domain, certType, field) with no verifier column, and
    /// adding one would be an engine/schema change this phase does not make.
    pub verifier_public_key: String,
    pub purpose: String,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestSpending {
    /// Our legacy shape only (`permissions.spending.perTransactionUsd`).
    /// Same unit and period as `domain_permissions.per_tx_limit_cents`, so it
    /// is the *only* declared figure that may pre-fill a limit field, and then
    /// only behind the user's `default_prefill_from_manifest` opt-in.
    pub per_transaction_usd: i64,
    /// Our legacy shape only. See `per_transaction_usd`.
    pub per_session_usd: i64,
    /// 🚨 BRC-73 `spendingAuthorization.amount` — *"the authorized **monthly**
    /// spend limit in **satoshis**"*. Neither the unit (satoshis vs USD cents)
    /// nor the period (calendar month vs per-transaction / per-session) matches
    /// anything we enforce, and we have no monthly concept at all. It is
    /// **displayed as the site's request and never written** (`R-CAPS`,
    /// `P0.8-A5`). It must never be mapped onto the two fields above.
    pub monthly_satoshis: i64,
    pub purpose: String,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestCounterparty {
    pub cp_type: String,
    pub counterparty: String,
    pub purpose: String,
}

/// A parsed wallet manifest. `Some(_)` means the document parsed as a JSON
/// object **and declared at least one recognised permission** — see the module
/// docs. `raw_json` keeps the original body so the `manifest_connect_bundle`
/// modal payload can carry the exact bytes the dApp served (no re-serialization
/// shape drift), and so the approved snapshot stores what was on screen.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub version: String,
    pub name: String,
    pub description: String,
    pub icon_url: String,
    pub expires_at: i64,
    /// Which shape the permissions were read from: `metanet` / `babbage` /
    /// `hodos-legacy`. The modal names it so a user (and a support log) can see
    /// whether a site is on the current standard.
    pub source_namespace: String,
    /// `groupPermissions.description` — the site's one-line summary of the
    /// whole group. Untrusted display text.
    pub group_description: String,
    pub protocols: Vec<ManifestProtocol>,
    pub baskets: Vec<ManifestBasket>,
    pub certificates: Vec<ManifestCertificate>,
    pub spending: ManifestSpending,
    pub counterparties: Vec<ManifestCounterparty>,
    #[serde(skip)]
    pub raw_json: String,
    /// Which URL served these bytes. Stored with the approved snapshot so
    /// "where did this come from?" is answerable later.
    #[serde(skip)]
    pub source_url: String,
}

impl Manifest {
    /// True iff the document declared at least one permission we recognise.
    /// This is the whole of the Phase 0.8 safety tightening: it is what
    /// separates "a manifest" from "a JSON file that happens to parse".
    pub fn has_declared_permissions(&self) -> bool {
        !self.protocols.is_empty()
            || !self.baskets.is_empty()
            || !self.certificates.is_empty()
            || !self.counterparties.is_empty()
            || self.spending.per_transaction_usd > 0
            || self.spending.per_session_usd > 0
            || self.spending.monthly_satoshis > 0
    }
}

/// The two locations a manifest may be served from, in the order we try them.
///
/// - `/manifest.json` — BRC-73: *"Applications SHOULD serve a W3C web-app
///   manifest at `https://{originator}/manifest.json`."* BRC-73 rides **inside**
///   that document.
/// - `/.well-known/wallet-manifest.json` — ours, from `PERMISSION_UX_DESIGN.md`
///   §5. Kept for the existing fixture dApp and any adopter of our shape.
///
/// Bare hosts default to `https://` (manifests must not be served over
/// plaintext — BRC-116 §9.2 *"Manifest fetches MUST be restricted to HTTPS"*);
/// an explicit scheme is preserved and never downgraded.
///
/// ⚠️ Spec deviation, recorded deliberately: BRC-116 §9.2 also says retrieval
/// *"MUST be constrained to the origin's `manifest.json` resource and MUST
/// reject unrelated paths."* Read strictly, our `.well-known` path is an
/// unrelated path. We keep it because dropping it would break every adopter of
/// our own documented shape; both locations are same-origin, HTTPS, GET-only
/// and capped, so the risk the rule guards against does not apply.
pub fn manifest_urls(origin: &str) -> Vec<String> {
    let Some(base) = manifest_origin_base(origin) else {
        return Vec::new();
    };
    vec![
        format!("{}/manifest.json", base),
        format!("{}/.well-known/wallet-manifest.json", base),
    ]
}

/// Normalize an origin to `scheme://authority` with no path and no trailing
/// slash, or `None` if it is not a usable origin.
///
/// ⛔ `R-PATH`. In production the caller passes the value of
/// `X-Requesting-Domain`, which C++ derives with `hodos::OriginFromUrl`
/// (`PortConfig.h`) and is therefore a bare `host[:port]` — no scheme, no path,
/// no userinfo. This function must not *widen* that: anything carrying a path,
/// query, fragment, userinfo or whitespace is rejected outright rather than
/// concatenated, so a caller that ever hands us `evil.com/a/b` cannot redirect
/// the fetch off the origin's own manifest resource (BRC-116 §9.2: retrieval
/// *"MUST be constrained to the origin's `manifest.json` resource and MUST
/// reject unrelated paths"*).
///
/// Scheme policy follows BRC-116 §9.2 — *"Manifest fetches MUST be restricted
/// to HTTPS (except localhost for development)"*. An explicit `http://` is
/// honoured only for a loopback host; everything else is https, and a bare
/// authority is https.
fn manifest_origin_base(origin: &str) -> Option<String> {
    let trimmed = origin.trim();
    if trimmed.is_empty() {
        return None;
    }

    let (explicit_http, authority) = if let Some(rest) = trimmed.strip_prefix("https://") {
        (false, rest)
    } else if let Some(rest) = trimmed.strip_prefix("http://") {
        (true, rest)
    } else if trimmed.contains("://") {
        // Some other scheme entirely (`file://`, `javascript:`…). Not an origin
        // we will ever fetch from.
        return None;
    } else {
        (false, trimmed)
    };

    // Tolerate exactly one trailing slash on an otherwise clean authority
    // ("https://app.example.com/"), then demand a clean authority.
    let authority = authority.strip_suffix('/').unwrap_or(authority);
    if authority.is_empty() {
        return None;
    }
    if authority
        .chars()
        .any(|c| matches!(c, '/' | '\\' | '?' | '#' | '@') || c.is_whitespace() || c.is_control())
    {
        return None;
    }

    let is_loopback = authority == "localhost"
        || authority.starts_with("localhost:")
        || authority == "127.0.0.1"
        || authority.starts_with("127.0.0.1:")
        || authority.starts_with("[::1]");

    let scheme = if explicit_http && is_loopback { "http" } else { "https" };
    Some(format!("{}://{}", scheme, authority))
}

/// Legacy single-URL helper — our `.well-known` location. Retained because it
/// is the shape the existing tests and `test-fixtures/manifest-dapp/` describe.
pub fn manifest_url(origin: &str) -> Option<String> {
    manifest_origin_base(origin).map(|b| format!("{}/.well-known/wallet-manifest.json", b))
}

// ============================================================================
// Field readers — every one of them bounds what it returns
// ============================================================================

fn clamp_str(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    // `take` on chars, not bytes — truncating mid-codepoint would panic on a
    // byte slice and a hostile manifest is exactly where that would be found.
    s.chars().take(max).collect()
}

fn str_field(obj: &serde_json::Map<String, serde_json::Value>, key: &str, max: usize) -> String {
    obj.get(key)
        .and_then(|v| v.as_str())
        .map(|s| clamp_str(s, max))
        .unwrap_or_default()
}

/// An `iconUrl` is rendered straight into the connect modal's `<img src>`.
/// Accept only an absolute `https://` URL; anything else (a `data:` blob, a
/// `javascript:` string, a relative path, a plaintext `http://` icon) is
/// dropped and the modal falls back to its favicon path.
fn safe_icon_url(raw: &str) -> String {
    let s = clamp_str(raw, MAX_ID_STR);
    if s.starts_with("https://") && s.len() > "https://".len() {
        s
    } else {
        String::new()
    }
}

fn obj<'a>(
    map: &'a serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> Option<&'a serde_json::Map<String, serde_json::Value>> {
    map.get(key).and_then(|v| v.as_object())
}

// ============================================================================
// BRC-73 `groupPermissions` — the standard shape
// ============================================================================

/// Translate one `metanet`/`babbage` `groupPermissions` object into our
/// `Manifest`. Pure; drops malformed entries rather than failing the parse.
fn apply_group_permissions(
    m: &mut Manifest,
    gp: &serde_json::Map<String, serde_json::Value>,
) {
    m.group_description = str_field(gp, "description", MAX_STR);

    // protocolPermissions[] — BRC-73: protocolID is the BRC-43 tuple
    // [securityLevel, protocolName]; `counterparty` is required for a specific
    // Level 2 and MAY be omitted at Level 1; `description` is for the user.
    if let Some(arr) = gp.get("protocolPermissions").and_then(|v| v.as_array()) {
        for p in arr.iter().take(MAX_ENTRIES) {
            let Some(po) = p.as_object() else { continue };
            let mut mp = ManifestProtocol {
                // BRC-73 has no keyID concept — a declaration covers the
                // protocol, so the wildcard our grant rows already understand
                // is the faithful translation.
                key_id: "*".to_string(),
                counterparty: str_field(po, "counterparty", MAX_ID_STR),
                purpose: str_field(po, "description", MAX_STR),
                ..Default::default()
            };
            if let Some(pid) = po.get("protocolID").and_then(|v| v.as_array()) {
                if pid.len() >= 2 {
                    if let Some(lvl) = pid[0].as_i64() {
                        if (0..=2).contains(&lvl) {
                            mp.security_level = lvl as i32;
                        }
                    }
                    if let Some(n) = pid[1].as_str() {
                        mp.name = clamp_str(n, MAX_ID_STR);
                    }
                }
            }
            if !mp.name.is_empty() {
                m.protocols.push(mp);
            }
        }
    }

    // spendingAuthorization — a single object. ⛔ `amount` lands in
    // `monthly_satoshis` and NOWHERE else (R-CAPS / P0.8-A5).
    if let Some(sa) = obj(gp, "spendingAuthorization") {
        let amount = sa.get("amount").and_then(|v| v.as_i64()).unwrap_or(0);
        m.spending.monthly_satoshis = amount.max(0);
        m.spending.purpose = str_field(sa, "description", MAX_STR);
    }

    // basketAccess[] — BRC-73 declares only a basket name. BRC-116 §4.3:
    // basket grants are binary and cover insertion, listing and removal, so
    // `read_write` is the faithful translation; `read` would silently
    // under-grant and re-introduce the per-call prompts the bundle exists to
    // avoid.
    if let Some(arr) = gp.get("basketAccess").and_then(|v| v.as_array()) {
        for b in arr.iter().take(MAX_ENTRIES) {
            let Some(bo) = b.as_object() else { continue };
            let mb = ManifestBasket {
                name: str_field(bo, "basket", MAX_ID_STR),
                access: "read_write".to_string(),
                purpose: str_field(bo, "description", MAX_STR),
            };
            if !mb.name.is_empty() {
                m.baskets.push(mb);
            }
        }
    }

    // certificateAccess[] — type + fields + verifierPublicKey + description.
    if let Some(arr) = gp.get("certificateAccess").and_then(|v| v.as_array()) {
        for c in arr.iter().take(MAX_ENTRIES) {
            let Some(co) = c.as_object() else { continue };
            let mut mc = ManifestCertificate {
                cert_type: str_field(co, "type", MAX_ID_STR),
                verifier_public_key: str_field(co, "verifierPublicKey", MAX_ID_STR),
                purpose: str_field(co, "description", MAX_STR),
                ..Default::default()
            };
            if let Some(fields) = co.get("fields").and_then(|v| v.as_array()) {
                for f in fields.iter().take(MAX_FIELDS) {
                    if let Some(fs) = f.as_str() {
                        mc.fields.push(clamp_str(fs, MAX_ID_STR));
                    }
                }
            }
            if !mc.cert_type.is_empty() {
                m.certificates.push(mc);
            }
        }
    }

    // ⚠️ NOT mapped: BRC-116's `metanet.counterpartyPermissions`. That declares
    // Level-2 *protocol names* for PACT (the concrete counterparty arrives at
    // request time); our `counterparties[]` and
    // `domain_counterparty_permissions` hold specific pubkeys. Folding one into
    // the other would write a grant nobody asked for. PACT is a beta.4 item.
}

// ============================================================================
// Our own top-level `permissions` shape (PERMISSION_UX_DESIGN.md §5)
// ============================================================================

fn apply_hodos_legacy_permissions(
    m: &mut Manifest,
    perms: &serde_json::Map<String, serde_json::Value>,
) {
    if let Some(arr) = perms.get("protocols").and_then(|v| v.as_array()) {
        for p in arr.iter().take(MAX_ENTRIES) {
            let Some(po) = p.as_object() else { continue };
            let mut mp = ManifestProtocol {
                key_id: po
                    .get("keyID")
                    .and_then(|v| v.as_str())
                    .map(|s| clamp_str(s, MAX_ID_STR))
                    .unwrap_or_else(|| "*".to_string()),
                counterparty: str_field(po, "counterparty", MAX_ID_STR),
                purpose: str_field(po, "purpose", MAX_STR),
                ..Default::default()
            };
            // protocolID is [securityLevel, name] per BRC-43. Our shape also
            // allows the flattened `name` + `securityLevel` spelling.
            if let Some(pid) = po.get("protocolID").and_then(|v| v.as_array()) {
                if pid.len() >= 2 {
                    if let Some(lvl) = pid[0].as_i64() {
                        if (0..=2).contains(&lvl) {
                            mp.security_level = lvl as i32;
                        }
                    }
                    if let Some(n) = pid[1].as_str() {
                        mp.name = clamp_str(n, MAX_ID_STR);
                    }
                }
            } else {
                if let Some(lvl) = po.get("securityLevel").and_then(|v| v.as_i64()) {
                    if (0..=2).contains(&lvl) {
                        mp.security_level = lvl as i32;
                    }
                }
                mp.name = str_field(po, "name", MAX_ID_STR);
            }
            if !mp.name.is_empty() {
                m.protocols.push(mp);
            }
        }
    }

    if let Some(arr) = perms.get("baskets").and_then(|v| v.as_array()) {
        for b in arr.iter().take(MAX_ENTRIES) {
            let Some(bo) = b.as_object() else { continue };
            let access = bo
                .get("access")
                .and_then(|v| v.as_str())
                .unwrap_or("read")
                .to_string();
            let mb = ManifestBasket {
                name: str_field(bo, "name", MAX_ID_STR),
                access,
                purpose: str_field(bo, "purpose", MAX_STR),
            };
            if !mb.name.is_empty() && (mb.access == "read" || mb.access == "read_write") {
                m.baskets.push(mb);
            }
        }
    }

    if let Some(arr) = perms.get("certificates").and_then(|v| v.as_array()) {
        for c in arr.iter().take(MAX_ENTRIES) {
            let Some(co) = c.as_object() else { continue };
            let mut mc = ManifestCertificate {
                cert_type: str_field(co, "type", MAX_ID_STR),
                verifier_public_key: str_field(co, "verifierPublicKey", MAX_ID_STR),
                purpose: str_field(co, "purpose", MAX_STR),
                ..Default::default()
            };
            if let Some(fields) = co.get("fields").and_then(|v| v.as_array()) {
                for f in fields.iter().take(MAX_FIELDS) {
                    if let Some(fs) = f.as_str() {
                        mc.fields.push(clamp_str(fs, MAX_ID_STR));
                    }
                }
            }
            if !mc.cert_type.is_empty() {
                m.certificates.push(mc);
            }
        }
    }

    if let Some(s) = obj(perms, "spending") {
        m.spending.per_transaction_usd = s
            .get("perTransactionUsd")
            .and_then(|v| v.as_i64())
            .unwrap_or(0)
            .max(0);
        m.spending.per_session_usd = s
            .get("perSessionUsd")
            .and_then(|v| v.as_i64())
            .unwrap_or(0)
            .max(0);
        m.spending.purpose = str_field(s, "purpose", MAX_STR);
    }

    if let Some(arr) = perms.get("counterparties").and_then(|v| v.as_array()) {
        for cp in arr.iter().take(MAX_ENTRIES) {
            let Some(cpo) = cp.as_object() else { continue };
            let mcp = ManifestCounterparty {
                cp_type: str_field(cpo, "type", MAX_ID_STR),
                counterparty: str_field(cpo, "counterparty", MAX_ID_STR),
                purpose: str_field(cpo, "purpose", MAX_STR),
            };
            if !mcp.cp_type.is_empty() || !mcp.counterparty.is_empty() {
                m.counterparties.push(mcp);
            }
        }
    }
}

// ============================================================================
// Entry point
// ============================================================================

/// Parse a wallet manifest. Mirrors C++ `ManifestFetcher::ParseFromJson` —
/// **keep the two in step**.
///
/// Returns `None` when the JSON is malformed, the top level is not an object,
/// or **no recognised permission was declared**. See the module docs for why
/// that last clause is the whole point of the phase.
pub fn parse_manifest(json: &str) -> Option<Manifest> {
    let value: serde_json::Value = serde_json::from_str(json).ok()?;
    let obj_root = value.as_object()?; // top-level must be an object

    let mut m = Manifest {
        version: str_field(obj_root, "version", MAX_ID_STR),
        // W3C `name`; our shape uses the same key. `short_name` is a fallback.
        name: {
            let n = str_field(obj_root, "name", MAX_STR);
            if n.is_empty() {
                str_field(obj_root, "short_name", MAX_STR)
            } else {
                n
            }
        },
        description: str_field(obj_root, "description", MAX_STR),
        icon_url: safe_icon_url(&str_field(obj_root, "iconUrl", MAX_ID_STR)),
        expires_at: obj_root.get("expiresAt").and_then(|v| v.as_i64()).unwrap_or(0),
        raw_json: json.to_string(),
        ..Default::default()
    };

    // Namespace precedence. BRC-116 Backwards Compatibility: "Check for
    // `metanet` first. If present, use it. If `metanet` is absent, fall back to
    // `babbage`." We condition on `groupPermissions` actually being present,
    // not merely on the namespace key — socialcert.net serves a bare `babbage`
    // object with no `groupPermissions`, and a `metanet: {schemaVersion: 1}`
    // with nothing else should not shadow a populated `babbage`.
    let metanet_gp = obj(obj_root, SOURCE_METANET).and_then(|ns| obj(ns, "groupPermissions"));
    let babbage_gp = obj(obj_root, SOURCE_BABBAGE).and_then(|ns| obj(ns, "groupPermissions"));

    if let Some(gp) = metanet_gp {
        m.source_namespace = SOURCE_METANET.to_string();
        apply_group_permissions(&mut m, gp);
    } else if let Some(gp) = babbage_gp {
        // BRC-116: "Log a deprecation warning when a `babbage` namespace is
        // encountered to encourage migration."
        log::info!(
            "📦 manifest uses the DEPRECATED `babbage.groupPermissions` namespace (BRC-73 canonical is `metanet`)"
        );
        m.source_namespace = SOURCE_BABBAGE.to_string();
        apply_group_permissions(&mut m, gp);
    } else if let Some(perms) = obj(obj_root, "permissions") {
        m.source_namespace = SOURCE_HODOS_LEGACY.to_string();
        apply_hodos_legacy_permissions(&mut m, perms);
    }

    // 🚨 The tightening. A document with no recognised permissions is not a
    // wallet manifest for our purposes, whatever else it contains.
    if !m.has_declared_permissions() {
        return None;
    }

    Some(m)
}

/// Fetch + parse a dApp's wallet manifest, trying both locations in order
/// (`/manifest.json`, then `/.well-known/wallet-manifest.json`). Returns `None`
/// when neither location yields a manifest with declared permissions — 404,
/// timeout, network error, oversized body, non-JSON (an SPA catch-all serving
/// `200 text/html` is the common case, four of ten surveyed sites) and
/// permission-free JSON are all treated identically.
///
/// ⛔ The HTTP status code is never trusted on its own: `200` does not mean
/// "manifest" (`P0.8-A7`). Only a successful parse counts.
pub async fn fetch_manifest(origin: &str) -> Option<Manifest> {
    let urls = manifest_urls(origin);
    if urls.is_empty() {
        return None;
    }
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_millis(FETCH_TIMEOUT_MS))
        .build()
        .ok()?;

    for url in urls {
        let Ok(resp) = client.get(&url).send().await else {
            continue;
        };
        if resp.status() != reqwest::StatusCode::OK {
            continue;
        }
        let Ok(bytes) = resp.bytes().await else { continue };
        if bytes.len() > MAX_MANIFEST_BYTES {
            log::warn!(
                "📦 manifest at {} exceeds the {} byte cap ({} bytes) — rejected without parsing",
                url,
                MAX_MANIFEST_BYTES,
                bytes.len()
            );
            continue;
        }
        let Ok(body) = String::from_utf8(bytes.to_vec()) else {
            continue;
        };
        if let Some(mut m) = parse_manifest(&body) {
            m.source_url = url.clone();
            log::info!(
                "📦 manifest for {} parsed from {} (namespace={}, {} protocols, {} baskets, {} certs, {} counterparties)",
                origin, url, m.source_namespace,
                m.protocols.len(), m.baskets.len(), m.certificates.len(), m.counterparties.len(),
            );
            return Some(m);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── The canonical fixtures. ONE copy, shared with the C++ suite
    // (`cef-native/tests/manifest_fetcher_test.cpp` reads the same files from
    // disk). `include_str!` fails the BUILD if a fixture is deleted, so this
    // can never silently degrade into testing nothing.
    const FX_METANET: &str = include_str!("../../demos/manifest-shapes/brc73-metanet-protocols.json");
    const FX_BABBAGE: &str = include_str!("../../demos/manifest-shapes/brc73-babbage-legacy.json");
    const FX_ALL_CATEGORIES: &str = include_str!("../../demos/manifest-shapes/brc73-all-categories.json");
    const FX_BOTH_NAMESPACES: &str = include_str!("../../demos/manifest-shapes/brc73-both-namespaces.json");
    const FX_HODOS_LEGACY: &str = include_str!("../../demos/manifest-shapes/hodos-legacy-permissions.json");
    const FX_UNRECOGNISED: &str = include_str!("../../demos/manifest-shapes/unrecognised-shape.json");
    const FX_NOT_A_MANIFEST: &str = include_str!("../../demos/manifest-shapes/not-a-manifest.html");
    const FX_BITGENIUS: &str = include_str!("../../demos/manifest-shapes/bitgenius-live-capture.json");

    // ========================================================================
    // URL building
    // ========================================================================

    #[test]
    fn urls_cover_both_locations_standard_first() {
        let urls = manifest_urls("teragun.com");
        assert_eq!(
            urls,
            vec![
                "https://teragun.com/manifest.json".to_string(),
                "https://teragun.com/.well-known/wallet-manifest.json".to_string(),
            ],
            "BRC-73's /manifest.json is tried first; our legacy location second"
        );
    }

    #[test]
    fn urls_preserve_explicit_scheme_and_strip_trailing_slash() {
        let urls = manifest_urls("https://app.example.com/");
        assert_eq!(urls[0], "https://app.example.com/manifest.json");
        assert_eq!(urls[1], "https://app.example.com/.well-known/wallet-manifest.json");
    }

    #[test]
    fn urls_bare_host_defaults_to_https_never_http() {
        for u in manifest_urls("example.com") {
            assert!(u.starts_with("https://"), "manifests must not be fetched over plaintext: {}", u);
        }
    }

    #[test]
    fn urls_empty_origin_is_empty() {
        assert!(manifest_urls("").is_empty());
        assert!(manifest_urls("   ").is_empty());
        assert!(manifest_urls("/").is_empty());
        assert!(manifest_urls("https://").is_empty());
        assert!(manifest_url("").is_none());
    }

    /// `R-PATH` — an origin carrying a path, query, fragment, userinfo or
    /// whitespace is rejected, never concatenated. BRC-116 §9.2: manifest
    /// retrieval must be constrained to the origin's own manifest resource.
    #[test]
    fn urls_reject_anything_that_is_not_a_bare_authority() {
        for bad in [
            "/",
            "evil.com/a/b",
            "https://evil.com/deep/path",
            "evil.com?x=1",
            "evil.com#frag",
            "https://127.0.0.1:5137@evil.com",
            "evil.com\\..\\x",
            "evil .com",
            "evil.com\tx",
            "file:///etc/passwd",
            "javascript://evil.com",
        ] {
            assert!(
                manifest_urls(bad).is_empty(),
                "origin {:?} must not produce a fetch URL, got {:?}",
                bad,
                manifest_urls(bad)
            );
        }
    }

    #[test]
    fn urls_trim_surrounding_whitespace() {
        // Surrounding whitespace is trimmed (safe); interior whitespace is not
        // (rejected above). A header value carrying a stray newline is still a
        // usable authority once trimmed.
        assert_eq!(manifest_urls("  evil.com\n")[0], "https://evil.com/manifest.json");
    }

    /// BRC-116 §9.2 — HTTPS only, except localhost for development.
    #[test]
    fn urls_http_is_upgraded_except_on_loopback() {
        assert!(manifest_urls("http://evil.com")[0].starts_with("https://evil.com/"));
        assert!(manifest_urls("http://localhost:8080")[0].starts_with("http://localhost:8080/"));
        assert!(manifest_urls("http://127.0.0.1:3000")[0].starts_with("http://127.0.0.1:3000/"));
        // A host that merely *starts with* "localhost" is not loopback.
        assert!(manifest_urls("http://localhost.evil.com")[0].starts_with("https://"));
    }

    #[test]
    fn legacy_single_url_helper_unchanged() {
        assert_eq!(
            manifest_url("teragun.com").unwrap(),
            "https://teragun.com/.well-known/wallet-manifest.json"
        );
    }

    // ========================================================================
    // Malformed / non-manifest input
    // ========================================================================

    #[test]
    fn parse_non_object_is_none() {
        assert!(parse_manifest("[1,2,3]").is_none());
        assert!(parse_manifest("\"hello\"").is_none());
        assert!(parse_manifest("42").is_none());
    }

    #[test]
    fn parse_malformed_json_is_none() {
        assert!(parse_manifest("{not json").is_none());
    }

    /// `P0.8-A7` — the SPA catch-all. Four of ten surveyed BSV properties return
    /// exactly this with HTTP 200 for both manifest paths.
    #[test]
    fn a7_html_page_yields_no_manifest() {
        assert!(
            parse_manifest(FX_NOT_A_MANIFEST).is_none(),
            "a 200 text/html SPA page must never be treated as a manifest"
        );
    }

    /// `P0.8-A3` — the defect this phase exists to fix. A valid W3C web-app
    /// manifest with no permission namespace must NOT produce a bundle.
    #[test]
    fn a3_unrecognised_shape_declares_nothing_so_no_manifest() {
        assert!(
            parse_manifest(FX_UNRECOGNISED).is_none(),
            "a permission-free manifest must fall back to domain_approval, \
             not render as a permission-free itemised bundle"
        );
    }

    #[test]
    fn parse_minimal_object_with_no_permissions_is_none() {
        assert!(parse_manifest(r#"{"name":"Teragun","description":"a dApp"}"#).is_none());
        assert!(parse_manifest("{}").is_none());
        // An empty permission namespace is still no permissions.
        assert!(parse_manifest(r#"{"metanet":{"groupPermissions":{}}}"#).is_none());
        assert!(parse_manifest(r#"{"metanet":{"groupPermissions":{"protocolPermissions":[]}}}"#).is_none());
    }

    // ========================================================================
    // BRC-73 — the standard shape
    // ========================================================================

    /// `P0.8-A1` (Rust half) — bitgenius's real 3358-byte manifest.
    #[test]
    fn a1_bitgenius_live_capture_parses_four_protocols() {
        let m = parse_manifest(FX_BITGENIUS).expect("bitgenius manifest must parse");
        assert_eq!(m.protocols.len(), 4, "bitgenius declares four protocols");
        assert_eq!(m.name, "BitGenius");
        assert_eq!(m.source_namespace, SOURCE_METANET);

        assert_eq!(m.protocols[0].security_level, 1);
        assert_eq!(m.protocols[0].name, "identity key retrieval");
        assert_eq!(m.protocols[0].counterparty, "", "Level 1 MAY omit counterparty");
        assert_eq!(
            m.protocols[0].purpose,
            "Use your public identity key as your passwordless BitGenius account."
        );

        assert_eq!(m.protocols[1].security_level, 2);
        assert_eq!(m.protocols[1].name, "server hmac");
        assert_eq!(m.protocols[1].counterparty, "self");

        assert_eq!(m.protocols[3].name, "3241645161d8");
        assert_eq!(
            m.protocols[3].counterparty,
            "0279887cddd8cb44fc34793fa568dfb86badb1e8f1eace5976a6f4cf3f786cc893"
        );

        // Every protocol carries the site's own description — this is what the
        // modal itemises, and showing none of it was the defect.
        for p in &m.protocols {
            assert!(!p.purpose.is_empty(), "protocol {} has no description", p.name);
        }
    }

    #[test]
    fn metanet_fixture_parses_four_protocols_with_wildcard_keyid() {
        let m = parse_manifest(FX_METANET).unwrap();
        assert_eq!(m.protocols.len(), 4);
        assert_eq!(m.source_namespace, SOURCE_METANET);
        assert_eq!(
            m.group_description,
            "Sign in and approve each checkout with your wallet."
        );
        for p in &m.protocols {
            assert_eq!(p.key_id, "*", "BRC-73 has no keyID; wildcard is the translation");
        }
    }

    #[test]
    fn babbage_legacy_namespace_still_parses() {
        let m = parse_manifest(FX_BABBAGE).unwrap();
        assert_eq!(m.protocols.len(), 2);
        assert_eq!(m.source_namespace, SOURCE_BABBAGE);
    }

    /// `P0.8-A8` — precedence. BRC-116: "Check for `metanet` first. If present,
    /// use it."
    #[test]
    fn a8_metanet_wins_over_babbage() {
        let m = parse_manifest(FX_BOTH_NAMESPACES).unwrap();
        assert_eq!(m.source_namespace, SOURCE_METANET);
        assert_eq!(m.protocols.len(), 4, "metanet's four, not babbage's one");
        assert!(
            !m.protocols.iter().any(|p| p.name == "STALE legacy protocol"),
            "the deprecated namespace's content leaked through — precedence is wrong"
        );
    }

    #[test]
    fn babbage_used_when_metanet_has_no_group_permissions() {
        // socialcert.net's real shape: a namespace key with no groupPermissions.
        let json = r#"{
            "name":"X",
            "metanet":{"schemaVersion":1},
            "babbage":{"groupPermissions":{"protocolPermissions":[
                {"protocolID":[1,"identity key retrieval"],"description":"d"}]}}
        }"#;
        let m = parse_manifest(json).unwrap();
        assert_eq!(m.source_namespace, SOURCE_BABBAGE);
        assert_eq!(m.protocols.len(), 1);
    }

    /// Goal 5 — all four BRC-73 categories are data we already carry.
    #[test]
    fn all_four_categories_parse() {
        let m = parse_manifest(FX_ALL_CATEGORIES).unwrap();
        assert_eq!(m.protocols.len(), 4);
        assert_eq!(m.baskets.len(), 2);
        assert_eq!(m.certificates.len(), 1);

        assert_eq!(m.baskets[0].name, "tickets");
        assert_eq!(
            m.baskets[0].access, "read_write",
            "BRC-116 §4.3: basket grants are binary and cover insert/list/remove"
        );

        assert_eq!(m.certificates[0].fields, vec!["firstName", "lastName"]);
        assert_eq!(
            m.certificates[0].verifier_public_key,
            "0279887cddd8cb44fc34793fa568dfb86badb1e8f1eace5976a6f4cf3f786cc893",
            "BRC-116 §4.4 scopes cert access by verifier — the user must see who receives it"
        );
    }

    /// 🚨 `P0.8-A5` / `R-CAPS`, parser half. BRC-73's `amount` is MONTHLY
    /// SATOSHIS. It must land in `monthly_satoshis` and never touch the two
    /// fields that share a unit with our stored caps.
    ///
    /// NEGATIVE CONTROL: map `amount` onto `per_transaction_usd` in
    /// `apply_group_permissions` and this test goes red, and the frontend
    /// pre-fill harness then shows the raised cap reaching the row.
    #[test]
    fn a5_brc73_spending_authorization_never_populates_our_cap_fields() {
        let m = parse_manifest(FX_ALL_CATEGORIES).unwrap();
        assert_eq!(m.spending.monthly_satoshis, 5_000_000);
        assert_eq!(
            m.spending.per_transaction_usd, 0,
            "monthly satoshis must never be read as a per-transaction USD cap"
        );
        assert_eq!(
            m.spending.per_session_usd, 0,
            "monthly satoshis must never be read as a per-session USD cap"
        );
        assert_eq!(
            m.spending.purpose,
            "Up to 5,000,000 satoshis per month for in-app purchases."
        );
    }

    #[test]
    fn brc73_spending_authorization_alone_is_a_declared_permission() {
        let json = r#"{"metanet":{"groupPermissions":{
            "spendingAuthorization":{"amount":10000,"description":"tips"}}}}"#;
        let m = parse_manifest(json).expect("a spending-only manifest still declares something");
        assert_eq!(m.spending.monthly_satoshis, 10000);
        assert!(m.has_declared_permissions());
    }

    #[test]
    fn brc73_drops_protocol_entries_with_no_name() {
        let json = r#"{"metanet":{"groupPermissions":{"protocolPermissions":[
            {"protocolID":[2,"good"],"description":"keep"},
            {"description":"no protocolID -> dropped"},
            {"protocolID":[2,99],"description":"non-string name -> dropped"},
            {"protocolID":["nope"],"description":"too short -> dropped"},
            "not an object"
        ]}}}"#;
        let m = parse_manifest(json).unwrap();
        assert_eq!(m.protocols.len(), 1);
        assert_eq!(m.protocols[0].name, "good");
    }

    #[test]
    fn brc73_out_of_range_security_level_falls_back_to_default() {
        let json = r#"{"metanet":{"groupPermissions":{"protocolPermissions":[
            {"protocolID":[7,"weird"],"description":"d"}]}}}"#;
        let m = parse_manifest(json).unwrap();
        // 0 is the struct default; the point is that 7 is never stored.
        assert!((0..=2).contains(&m.protocols[0].security_level));
    }

    #[test]
    fn brc116_counterparty_permissions_are_not_mapped_to_counterparty_grants() {
        // PACT declares Level-2 protocol NAMES, not pubkeys. Folding them into
        // `counterparties[]` would write a grant nobody asked for.
        let json = r#"{"metanet":{
            "groupPermissions":{"protocolPermissions":[{"protocolID":[1,"p"],"description":"d"}]},
            "counterpartyPermissions":{"protocols":[{"protocolName":"convo","description":"peer"}]}
        }}"#;
        let m = parse_manifest(json).unwrap();
        assert!(m.counterparties.is_empty(), "PACT declarations must not become counterparty grants");
    }

    // ========================================================================
    // Our legacy shape — regression guard
    // ========================================================================

    #[test]
    fn hodos_legacy_shape_still_parses() {
        let m = parse_manifest(FX_HODOS_LEGACY).unwrap();
        assert_eq!(m.source_namespace, SOURCE_HODOS_LEGACY);
        assert_eq!(m.protocols.len(), 1);
        assert_eq!(m.protocols[0].name, "identity key retrieval");
        assert_eq!(m.protocols[0].security_level, 1);
        assert_eq!(m.baskets.len(), 1);
        assert_eq!(m.baskets[0].name, "receipts");
        assert_eq!(m.baskets[0].access, "read", "our shape's explicit default");
        // Our shape's spending IS in our unit and period — it may be shown as a
        // suggestion and, behind the user's opt-in, pre-filled.
        assert_eq!(m.spending.per_transaction_usd, 100);
        assert_eq!(m.spending.per_session_usd, 1000);
        assert_eq!(m.spending.monthly_satoshis, 0);
    }

    #[test]
    fn hodos_legacy_drops_malformed_protocol_and_basket_entries() {
        let json = r#"{
            "permissions": {
                "protocols": [
                    {"protocolID":[2,"1sat ordinal"],"purpose":"mint"},
                    {"purpose":"no name -> dropped"},
                    {"protocolID":[2,99]}
                ],
                "baskets": [
                    {"name":"ord","access":"read_write"},
                    {"name":"bad","access":"delete"},
                    {"access":"read"}
                ]
            }
        }"#;
        let m = parse_manifest(json).unwrap();
        assert_eq!(m.protocols.len(), 1, "only the well-formed protocol survives");
        assert_eq!(m.protocols[0].name, "1sat ordinal");
        assert_eq!(m.protocols[0].key_id, "*", "missing keyID defaults to wildcard");
        assert_eq!(m.baskets.len(), 1, "invalid-access + nameless baskets dropped");
        assert_eq!(m.baskets[0].name, "ord");
    }

    #[test]
    fn hodos_legacy_full_permissions() {
        let json = r#"{
            "name":"App","permissions":{
                "certificates":[{"type":"social","fields":["displayName","avatar"],"purpose":"profile"}],
                "spending":{"perTransactionUsd":5,"perSessionUsd":20},
                "counterparties":[{"counterparty":"02ab","purpose":"pay"}]
            }
        }"#;
        let m = parse_manifest(json).unwrap();
        assert_eq!(m.certificates.len(), 1);
        assert_eq!(m.certificates[0].fields, vec!["displayName", "avatar"]);
        assert_eq!(m.spending.per_transaction_usd, 5);
        assert_eq!(m.counterparties.len(), 1);
        assert_eq!(m.counterparties[0].counterparty, "02ab");
    }

    #[test]
    fn metanet_group_permissions_win_over_our_legacy_top_level_shape() {
        let json = r#"{
            "metanet":{"groupPermissions":{"protocolPermissions":[
                {"protocolID":[1,"standard"],"description":"d"}]}},
            "permissions":{"protocols":[{"protocolID":[1,"legacy"],"purpose":"p"}]}
        }"#;
        let m = parse_manifest(json).unwrap();
        assert_eq!(m.source_namespace, SOURCE_METANET);
        assert_eq!(m.protocols.len(), 1);
        assert_eq!(m.protocols[0].name, "standard");
    }

    // ========================================================================
    // Hostile input
    // ========================================================================

    #[test]
    fn array_lengths_are_capped() {
        let entries: Vec<String> = (0..500)
            .map(|i| format!(r#"{{"protocolID":[1,"p{}"],"description":"d"}}"#, i))
            .collect();
        let json = format!(
            r#"{{"metanet":{{"groupPermissions":{{"protocolPermissions":[{}]}}}}}}"#,
            entries.join(",")
        );
        let m = parse_manifest(&json).unwrap();
        assert_eq!(m.protocols.len(), MAX_ENTRIES, "500 declared entries must not reach the modal");
    }

    #[test]
    fn strings_are_truncated_not_rejected() {
        let long = "A".repeat(5000);
        let json = format!(
            r#"{{"name":"{}","metanet":{{"groupPermissions":{{"protocolPermissions":[
                {{"protocolID":[1,"p"],"description":"{}"}}]}}}}}}"#,
            long, long
        );
        let m = parse_manifest(&json).unwrap();
        assert_eq!(m.name.chars().count(), MAX_STR);
        assert_eq!(m.protocols[0].purpose.chars().count(), MAX_STR);
    }

    #[test]
    fn multibyte_strings_truncate_on_char_boundaries() {
        // Truncating a byte slice mid-codepoint panics; a hostile manifest is
        // exactly where that would be found.
        let long = "é".repeat(5000);
        let json = format!(
            r#"{{"metanet":{{"groupPermissions":{{"protocolPermissions":[
                {{"protocolID":[1,"p"],"description":"{}"}}]}}}}}}"#,
            long
        );
        let m = parse_manifest(&json).unwrap();
        assert_eq!(m.protocols[0].purpose.chars().count(), MAX_STR);
    }

    #[test]
    fn cert_fields_are_capped() {
        let fields: Vec<String> = (0..500).map(|i| format!(r#""f{}""#, i)).collect();
        let json = format!(
            r#"{{"metanet":{{"groupPermissions":{{"certificateAccess":[
                {{"type":"t","fields":[{}],"description":"d"}}]}}}}}}"#,
            fields.join(",")
        );
        let m = parse_manifest(&json).unwrap();
        assert_eq!(m.certificates[0].fields.len(), MAX_FIELDS);
    }

    #[test]
    fn icon_url_accepts_https_only() {
        let mk = |icon: &str| {
            format!(
                r#"{{"iconUrl":"{}","metanet":{{"groupPermissions":{{"protocolPermissions":[
                    {{"protocolID":[1,"p"],"description":"d"}}]}}}}}}"#,
                icon
            )
        };
        assert_eq!(
            parse_manifest(&mk("https://x.example/i.png")).unwrap().icon_url,
            "https://x.example/i.png"
        );
        // The connect modal renders iconUrl into <img src>. Nothing but https.
        for bad in [
            "javascript:alert(1)",
            "data:image/svg+xml;base64,PHN2Zz48L3N2Zz4=",
            "http://x.example/i.png",
            "/relative.png",
            "https://",
        ] {
            assert_eq!(
                parse_manifest(&mk(bad)).unwrap().icon_url,
                "",
                "iconUrl {:?} must be dropped",
                bad
            );
        }
    }

    #[test]
    fn negative_spending_amount_clamps_to_zero() {
        let json = r#"{"metanet":{"groupPermissions":{
            "protocolPermissions":[{"protocolID":[1,"p"],"description":"d"}],
            "spendingAuthorization":{"amount":-999999,"description":"d"}}}}"#;
        let m = parse_manifest(json).unwrap();
        assert_eq!(m.spending.monthly_satoshis, 0);
    }

    /// The shipped demo dApp's manifest must actually parse — otherwise the
    /// demo silently degrades to a plain `domain_approval` prompt and looks
    /// like the feature is broken. `test-fixtures/manifest-dapp/` is what a
    /// human deploys to an HTTPS host to see the itemised modal.
    #[test]
    fn shipped_demo_dapp_manifest_parses() {
        const DEMO: &str = include_str!("../../test-fixtures/manifest-dapp/manifest.json");
        let m = parse_manifest(DEMO).expect("the demo dApp manifest must parse");
        assert_eq!(m.source_namespace, SOURCE_METANET);
        assert_eq!(m.protocols.len(), 3);
        assert_eq!(m.baskets.len(), 2);
        assert_eq!(m.certificates.len(), 1);
        assert_eq!(m.spending.monthly_satoshis, 250_000);
        // The demo exists to show a FULLY itemised modal — every category and
        // every BRC-116 display field the modal can render.
        assert_eq!(m.protocols[1].counterparty, "self");
        assert!(!m.protocols[2].counterparty.is_empty(), "a named Level-2 counterparty");
        assert!(!m.certificates[0].verifier_public_key.is_empty(), "a verifier to display");
        assert!(m.protocols.iter().all(|p| !p.purpose.is_empty()));
    }

    /// The legacy-shape half of the same fixture — the ONLY way to demo the
    /// `R-PROV` "suggested by site" marking, because BRC-73 has no field in our
    /// unit and so can never pre-fill one of our caps.
    #[test]
    fn shipped_demo_dapp_legacy_manifest_still_carries_a_cap_suggestion() {
        const DEMO: &str =
            include_str!("../../test-fixtures/manifest-dapp/.well-known/wallet-manifest.json");
        let m = parse_manifest(DEMO).expect("the legacy demo manifest must parse");
        assert_eq!(m.source_namespace, SOURCE_HODOS_LEGACY);
        assert_eq!(m.spending.per_transaction_usd, 1);
        assert_eq!(m.spending.per_session_usd, 5);
    }

    #[test]
    fn parse_preserves_raw_json_for_the_snapshot() {
        let src = r#"{"metanet":{"groupPermissions":{"protocolPermissions":[{"protocolID":[1,"p"],"description":"d"}]}}}"#;
        let m = parse_manifest(src).unwrap();
        assert_eq!(m.raw_json, src, "the snapshot stores the bytes the site served, verbatim");
    }
}
