//! Phase 2.6-G — Rust port of the C++ `ManifestFetcher`
//! (`cef-native/src/core/ManifestFetcher.cpp`). Fetches and leniently parses a
//! dApp's `/.well-known/wallet-manifest.json` so the engine can offer the
//! `manifest_connect_bundle` modal (permissions declared up-front) instead of a
//! bare `domain_approval`.
//!
//! OQ8 = A: permission policy + its inputs live in Rust. `parse_manifest` is
//! pure (and unit-tested); `fetch_manifest` is the thin async networking
//! wrapper. Neither ever panics — any failure yields `None` (mirrors the C++
//! `Manifest{valid=false}` contract). Forward-compatible: unknown fields are
//! ignored and malformed entries are dropped rather than failing the whole
//! parse.

use serde::Serialize;

/// 3-second hard cap — the user is already waiting on connect, but a slow or
/// hostile server must not hang the prompt.
const FETCH_TIMEOUT_MS: u64 = 3000;
/// 64 KB cap. Manifests are tiny; anything larger is rejected without parsing.
const MAX_MANIFEST_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestProtocol {
    pub security_level: i32,
    pub name: String,
    pub key_id: String,
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
    pub purpose: String,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestSpending {
    pub per_transaction_usd: i64,
    pub per_session_usd: i64,
    pub purpose: String,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestCounterparty {
    pub cp_type: String,
    pub counterparty: String,
    pub purpose: String,
}

/// A parsed wallet manifest. Presence of `Some(_)` means the document parsed as
/// a JSON object (valid, even if minimal). `raw_json` keeps the original body so
/// the `manifest_connect_bundle` modal payload can carry the exact bytes the
/// dApp served (no re-serialization shape drift).
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub version: String,
    pub name: String,
    pub description: String,
    pub icon_url: String,
    pub expires_at: i64,
    pub protocols: Vec<ManifestProtocol>,
    pub baskets: Vec<ManifestBasket>,
    pub certificates: Vec<ManifestCertificate>,
    pub spending: ManifestSpending,
    pub counterparties: Vec<ManifestCounterparty>,
    #[serde(skip)]
    pub raw_json: String,
}

/// Build the manifest URL. Bare hosts default to `https://` (manifests must not
/// be served over plaintext); an explicit scheme is preserved (never downgraded).
pub fn manifest_url(origin: &str) -> Option<String> {
    if origin.is_empty() {
        return None;
    }
    let mut url = if origin.contains("://") {
        origin.to_string()
    } else {
        format!("https://{}", origin)
    };
    if url.ends_with('/') {
        url.pop();
    }
    url.push_str("/.well-known/wallet-manifest.json");
    Some(url)
}

fn str_field(obj: &serde_json::Map<String, serde_json::Value>, key: &str) -> String {
    obj.get(key)
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

/// Lenient parse — mirrors C++ `ManifestFetcher::ParseFromJson`. Returns `None`
/// when the top level is not a JSON object or the JSON is malformed; otherwise
/// `Some(manifest)` (valid even when minimal). Malformed array entries are
/// dropped, not fatal.
pub fn parse_manifest(json: &str) -> Option<Manifest> {
    let value: serde_json::Value = serde_json::from_str(json).ok()?;
    let obj = value.as_object()?; // top-level must be an object

    let mut m = Manifest {
        version: str_field(obj, "version"),
        name: str_field(obj, "name"),
        description: str_field(obj, "description"),
        icon_url: str_field(obj, "iconUrl"),
        expires_at: obj.get("expiresAt").and_then(|v| v.as_i64()).unwrap_or(0),
        raw_json: json.to_string(),
        ..Default::default()
    };

    if let Some(perms) = obj.get("permissions").and_then(|v| v.as_object()) {
        // protocols[]
        if let Some(arr) = perms.get("protocols").and_then(|v| v.as_array()) {
            for p in arr {
                let Some(po) = p.as_object() else { continue };
                let mut mp = ManifestProtocol {
                    key_id: po
                        .get("keyID")
                        .and_then(|v| v.as_str())
                        .unwrap_or("*")
                        .to_string(),
                    purpose: str_field(po, "purpose"),
                    ..Default::default()
                };
                // protocolID is [securityLevel, name] per BRC-43.
                if let Some(pid) = po.get("protocolID").and_then(|v| v.as_array()) {
                    if pid.len() >= 2 {
                        if let Some(lvl) = pid[0].as_i64() {
                            if (0..=2).contains(&lvl) {
                                mp.security_level = lvl as i32;
                            }
                        }
                        if let Some(n) = pid[1].as_str() {
                            mp.name = n.to_string();
                        }
                    }
                }
                if !mp.name.is_empty() {
                    m.protocols.push(mp);
                }
            }
        }

        // baskets[]
        if let Some(arr) = perms.get("baskets").and_then(|v| v.as_array()) {
            for b in arr {
                let Some(bo) = b.as_object() else { continue };
                let access = bo
                    .get("access")
                    .and_then(|v| v.as_str())
                    .unwrap_or("read")
                    .to_string();
                let mb = ManifestBasket {
                    name: str_field(bo, "name"),
                    access,
                    purpose: str_field(bo, "purpose"),
                };
                if !mb.name.is_empty() && (mb.access == "read" || mb.access == "read_write") {
                    m.baskets.push(mb);
                }
            }
        }

        // certificates[]
        if let Some(arr) = perms.get("certificates").and_then(|v| v.as_array()) {
            for c in arr {
                let Some(co) = c.as_object() else { continue };
                let mut mc = ManifestCertificate {
                    cert_type: str_field(co, "type"),
                    purpose: str_field(co, "purpose"),
                    ..Default::default()
                };
                if let Some(fields) = co.get("fields").and_then(|v| v.as_array()) {
                    for f in fields {
                        if let Some(fs) = f.as_str() {
                            mc.fields.push(fs.to_string());
                        }
                    }
                }
                if !mc.cert_type.is_empty() {
                    m.certificates.push(mc);
                }
            }
        }

        // spending (single object, optional)
        if let Some(s) = perms.get("spending").and_then(|v| v.as_object()) {
            m.spending = ManifestSpending {
                per_transaction_usd: s
                    .get("perTransactionUsd")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0),
                per_session_usd: s.get("perSessionUsd").and_then(|v| v.as_i64()).unwrap_or(0),
                purpose: str_field(s, "purpose"),
            };
        }

        // counterparties[]
        if let Some(arr) = perms.get("counterparties").and_then(|v| v.as_array()) {
            for cp in arr {
                let Some(cpo) = cp.as_object() else { continue };
                let mcp = ManifestCounterparty {
                    cp_type: str_field(cpo, "type"),
                    counterparty: str_field(cpo, "counterparty"),
                    purpose: str_field(cpo, "purpose"),
                };
                if !mcp.cp_type.is_empty() || !mcp.counterparty.is_empty() {
                    m.counterparties.push(mcp);
                }
            }
        }
    }

    Some(m)
}

/// Fetch + parse a dApp's wallet manifest. Returns `None` on any failure (404,
/// timeout, network error, oversized body, non-object JSON) — all treated
/// identically, mirroring the C++ fetcher.
pub async fn fetch_manifest(origin: &str) -> Option<Manifest> {
    let url = manifest_url(origin)?;
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_millis(FETCH_TIMEOUT_MS))
        .build()
        .ok()?;
    let resp = client.get(&url).send().await.ok()?;
    if resp.status() != reqwest::StatusCode::OK {
        return None;
    }
    let bytes = resp.bytes().await.ok()?;
    if bytes.len() > MAX_MANIFEST_BYTES {
        return None;
    }
    let body = String::from_utf8(bytes.to_vec()).ok()?;
    parse_manifest(&body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_defaults_bare_host_to_https() {
        assert_eq!(
            manifest_url("teragun.com").unwrap(),
            "https://teragun.com/.well-known/wallet-manifest.json"
        );
    }

    #[test]
    fn url_preserves_explicit_scheme_and_strips_trailing_slash() {
        assert_eq!(
            manifest_url("https://app.example.com/").unwrap(),
            "https://app.example.com/.well-known/wallet-manifest.json"
        );
    }

    #[test]
    fn url_empty_origin_is_none() {
        assert!(manifest_url("").is_none());
    }

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

    #[test]
    fn parse_minimal_is_valid() {
        let m = parse_manifest(r#"{"name":"Teragun","description":"a dApp"}"#).unwrap();
        assert_eq!(m.name, "Teragun");
        assert_eq!(m.description, "a dApp");
        assert!(m.protocols.is_empty());
    }

    #[test]
    fn parse_drops_malformed_protocol_and_basket_entries() {
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
        assert_eq!(m.protocols[0].security_level, 2);
        assert_eq!(m.protocols[0].key_id, "*", "missing keyID defaults to wildcard");
        assert_eq!(m.baskets.len(), 1, "invalid-access + nameless baskets dropped");
        assert_eq!(m.baskets[0].name, "ord");
    }

    #[test]
    fn parse_full_permissions() {
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
    fn parse_preserves_raw_json() {
        let src = r#"{"name":"X"}"#;
        let m = parse_manifest(src).unwrap();
        assert_eq!(m.raw_json, src);
    }
}
