//! B5-T2-P1 — token recognisers: what kind of token, if any, a locking script carries.
//!
//! Pure functions over a coin's **real** script (B5-T1-P4 reads it from verified chain
//! bytes). They run inside the classification seam (`classify::classify`) before the
//! money rule, so a token is filed or held and never reaches the balance. 👤 Owner,
//! 2026-10-06: *"only a standard UTXO is money"* — nothing here can make a coin money;
//! a recogniser can only take a coin *away* from the money rule.
//!
//! Specs, pinned in the phase contract §0 (`bsv-blockchain/BRCs` `master`, 2026-10-06):
//! - **BRC-160** `b2cb3149f0` — the `ord` envelope: `OP_FALSE OP_IF "ord" <field> <value> …
//!   OP_0 <body> OP_ENDIF`, before or after the lock. Field tags may be `OP_N` or a 1-byte
//!   push; the last occurrence of a field wins; field 0 (body) ends the list.
//! - **BRC-161** `7c0ba5f220` — BSV-21 JSON (`application/bsv-20`, `"p":"bsv-20"`): value
//!   ops `deploy+mint` / `mint` / `transfer`; `amt` a decimal u64 **string**; `id`
//!   `<64 lowercase hex>_<vout>`. A `tick` field is legacy BSV-20.
//! - **BRC-162** `413dfbd8d4` — Mandala (binary BSV-21): no tag; the layout is the
//!   marker: `<push id | OP_0> <push amount | OP_0> OP_2DROP …`. Recognised and **held
//!   only** (owner, 2026-10-06): no mainnet output exists to test filing against.
//!
//! A script that will not parse is [`Recognised::Unreadable`] — never `None`. Mapping a
//! parse error to "not a token" would hand an unread coin to the money rule (rule 7
//! trip-wire 2: a verdict where an error is owed).

use crate::script::parser::{parse_script_chunks, ScriptChunk};

/// BRC-147 storage basket for 1Sat ordinals.
pub const ONESAT_BASKET: &str = "1sat";
/// BRC-163 storage basket for BSV-21 value outputs. ⛔ Never `1sat` (BRC-163).
pub const BSV21_BASKET: &str = "bsv21";

/// A basket only the wallet's recogniser may file into (normalized name).
pub fn is_asset_basket(name: &str) -> bool {
    name == ONESAT_BASKET || name == BSV21_BASKET
}

const OP_0: u8 = 0x00;
const OP_PUSHDATA4: u8 = 0x4e;
const OP_1NEGATE: u8 = 0x4f;
const OP_1: u8 = 0x51;
const OP_16: u8 = 0x60;
const OP_IF: u8 = 0x63;
const OP_ENDIF: u8 = 0x68;
const OP_2DROP: u8 = 0x6d;

/// A BSV-21 value output, decoded from its inscription (never from a sender's claim).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bsv21 {
    /// Token id. `None` for `deploy+mint`, whose id is the output's own outpoint.
    pub id: Option<String>,
    pub op: String,
    /// Decimal u64 string, canonical (no sign, no leading zero).
    pub amt: String,
    pub sym: Option<String>,
    pub dec: Option<String>,
    /// `txid_vout`, or `_N` (same transaction, expanded when filed).
    pub icon: Option<String>,
}

/// A token shape the wallet recognises but does not file — held, shown, never money.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeldKind {
    /// BRC-162 binary layout.
    Mandala,
    /// BSV-21 `deploy+auth`, `auth` or `burn` — no spendable value (BRC-163 eligibility 2).
    Bsv21NonValue,
    /// Legacy tick-based BSV-20.
    Bsv20Tick,
    /// `application/bsv-20` whose JSON is not canonical BRC-161 (duplicate keys, a numeric
    /// or non-canonical `amt`, a malformed `id`, an unknown `op` …) — contract `P1-X1`.
    Bsv21Malformed,
    /// An `ord` envelope that is not well formed (no body, runs off the end).
    BadEnvelope,
}

impl HeldKind {
    pub fn name(self) -> &'static str {
        match self {
            HeldKind::Mandala => "mandala",
            HeldKind::Bsv21NonValue => "bsv21-non-value",
            HeldKind::Bsv20Tick => "bsv20-tick",
            HeldKind::Bsv21Malformed => "bsv21-malformed",
            HeldKind::BadEnvelope => "bad-envelope",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Recognised {
    /// No token shape: the money rule decides.
    None,
    /// The script does not parse into chunks.
    Unreadable,
    /// A well-formed `ord` envelope whose content type is not `application/bsv-20`.
    /// `content_type` is the media type with parameters removed (BRC-147 `type:`).
    Ordinal { content_type: String },
    /// A BSV-21 value output (`deploy+mint`, `mint`, `transfer`).
    Bsv21Value(Bsv21),
    Held(HeldKind),
}

/// Recognise the token shape of a locking script.
pub fn recognise(script: &[u8]) -> Recognised {
    let chunks = match parse_script_chunks(script) {
        Ok(c) => c,
        Err(_) => return Recognised::Unreadable,
    };
    if is_mandala(&chunks) {
        return Recognised::Held(HeldKind::Mandala);
    }
    let Some(start) = find_envelope(&chunks) else {
        return Recognised::None;
    };
    let Some((content_type, body, _)) = read_envelope(&chunks[start + 3..]) else {
        return Recognised::Held(HeldKind::BadEnvelope);
    };
    let base = media_type(&content_type);
    if base == "application/bsv-20" {
        return bsv20(&body);
    }
    Recognised::Ordinal { content_type: base }
}

fn is_push(c: &ScriptChunk) -> bool {
    c.op <= OP_PUSHDATA4 || c.op == OP_1NEGATE || (OP_1..=OP_16).contains(&c.op)
}

/// BRC-162: `<32|36-byte id | OP_0> <amount push | OP_0> OP_2DROP` at the start.
fn is_mandala(c: &[ScriptChunk]) -> bool {
    if c.len() < 3 || c[2].op != OP_2DROP || !is_push(&c[1]) {
        return false;
    }
    match (&c[0].op, &c[0].data) {
        (&OP_0, None) => true,
        (_, Some(d)) => d.len() == 32 || d.len() == 36,
        _ => false,
    }
}

/// Index of the first `OP_0 OP_IF "ord"`.
fn find_envelope(c: &[ScriptChunk]) -> Option<usize> {
    (0..c.len().saturating_sub(2)).find(|&i| {
        c[i].op == OP_0 && c[i].data.is_none()
            && c[i + 1].op == OP_IF
            && c[i + 2].data.as_deref() == Some(b"ord".as_slice())
    })
}

/// The value bytes of a push chunk (`OP_0` ⇒ empty, `OP_N` ⇒ `[N]`).
fn push_bytes(c: &ScriptChunk) -> Option<Vec<u8>> {
    match (&c.data, c.op) {
        (Some(d), _) => Some(d.clone()),
        (None, OP_0) => Some(Vec::new()),
        (None, op) if (OP_1..=OP_16).contains(&op) => Some(vec![op - OP_1 + 1]),
        _ => None,
    }
}

/// A field tag: `OP_0`/`OP_N`, or a single-byte push of the same number (BRC-160 aliases).
/// Any other push is an unknown field (its value is skipped).
fn field_number(c: &ScriptChunk) -> Option<Option<u8>> {
    if !is_push(c) {
        return None;
    }
    Some(match &c.data {
        Some(d) if d.len() == 1 => Some(d[0]),
        Some(_) => None,
        None => push_bytes(c).map(|b| b.first().copied().unwrap_or(0)),
    })
}

/// Fields after `"ord"` up to `OP_ENDIF`: `(content type, body, index of OP_ENDIF)`, or
/// `None` when malformed.
fn read_envelope(c: &[ScriptChunk]) -> Option<(String, Vec<u8>, usize)> {
    let mut content_type = String::new();
    let mut i = 0;
    loop {
        let tag = field_number(c.get(i)?)?;
        let value = push_bytes(c.get(i + 1)?)?;
        match tag {
            Some(0) => {
                return (c.get(i + 2)?.op == OP_ENDIF).then_some((content_type, value, i + 2));
            }
            Some(1) => content_type = String::from_utf8_lossy(&value).into_owned(),
            _ => {}
        }
        i += 2;
    }
}

/// `bytes` is exactly one well-formed `ord` envelope and nothing else.
fn is_bare_envelope(bytes: &[u8]) -> bool {
    let Ok(c) = parse_script_chunks(bytes) else { return false };
    find_envelope(&c) == Some(0)
        && read_envelope(&c[3..]).map_or(false, |(_, _, end)| 3 + end == c.len() - 1)
}

fn is_p2pkh(s: &[u8]) -> bool {
    s.len() == 25 && s[..3] == [0x76, 0xa9, 0x14] && s[23..] == [0x88, 0xac]
}

/// The 20-byte key hash of a token script whose lock is a plain P2PKH: **exactly** one
/// 25-byte P2PKH and one well-formed `ord` envelope, in either order (BRC-160's
/// `<lock> <envelope>` / `<envelope> <lock>`), nothing else. An envelope is
/// `OP_FALSE OP_IF … OP_ENDIF` — it never executes — so such an output is spendable by
/// that key alone, exactly like the bare P2PKH. Anything looser (a covenant, a second
/// lock, trailing data) is `None`: the wallet does not claim what it cannot spend.
pub fn p2pkh_lock(script: &[u8]) -> Option<&[u8]> {
    if script.len() <= 25 {
        return None;
    }
    let (head, tail) = script.split_at(25);
    if is_p2pkh(head) && is_bare_envelope(tail) {
        return Some(&head[3..23]);
    }
    let (head, tail) = script.split_at(script.len() - 25);
    (is_p2pkh(tail) && is_bare_envelope(head)).then(|| &tail[3..23])
}

/// Media type without parameters, lowercased (BRC-147 `type:`).
fn media_type(ct: &str) -> String {
    ct.split(';').next().unwrap_or("").trim().to_ascii_lowercase()
}

/// A JSON object read with its keys in order, so a duplicate key can be refused — default
/// `serde_json` keeps the last duplicate, an indexer may keep the first (`P1-X1`).
struct Pairs(Vec<(String, serde_json::Value)>);

impl<'de> serde::Deserialize<'de> for Pairs {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> serde::de::Visitor<'de> for V {
            type Value = Pairs;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a JSON object")
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(self, mut m: A) -> Result<Pairs, A::Error> {
                let mut v = Vec::new();
                while let Some(kv) = m.next_entry::<String, serde_json::Value>()? {
                    v.push(kv);
                }
                Ok(Pairs(v))
            }
        }
        d.deserialize_map(V)
    }
}

fn canonical_u64(s: &str) -> bool {
    let digits = !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    digits && (s == "0" || !s.starts_with('0')) && s.parse::<u64>().is_ok()
}

/// `<64 lowercase hex>_<canonical u32 vout>` (BRC-161: the underscore form only).
pub fn is_token_id(s: &str) -> bool {
    match s.split_once('_') {
        Some((txid, vout)) => txid.len() == 64
            && txid.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            && canonical_u64(vout) && vout.parse::<u32>().is_ok(),
        None => false,
    }
}

/// An `application/bsv-20` body, per BRC-161.
fn bsv20(body: &[u8]) -> Recognised {
    let malformed = Recognised::Held(HeldKind::Bsv21Malformed);
    let Ok(Pairs(pairs)) = serde_json::from_slice::<Pairs>(body) else {
        return malformed;
    };
    let mut seen = std::collections::HashSet::new();
    if !pairs.iter().all(|(k, _)| seen.insert(k.as_str())) {
        return malformed;
    }
    let get = |k: &str| pairs.iter().find(|(key, _)| key == k).map(|(_, v)| v);
    let text = |k: &str| get(k).and_then(|v| v.as_str()).map(str::to_string);
    if text("p").as_deref() != Some("bsv-20") {
        return malformed;
    }
    if get("tick").is_some() {
        return Recognised::Held(HeldKind::Bsv20Tick);
    }
    let Some(op) = text("op") else { return malformed };
    match op.as_str() {
        "deploy+auth" | "auth" | "burn" => return Recognised::Held(HeldKind::Bsv21NonValue),
        "deploy+mint" | "mint" | "transfer" => {}
        _ => return malformed,
    }
    // `amt` must be a JSON string holding a canonical u64 — a number, a sign, a leading
    // zero or an overflow is not the token's amount (`P1-X1`).
    let Some(amt) = text("amt").filter(|a| canonical_u64(a)) else { return malformed };
    let id = if op == "deploy+mint" {
        None
    } else {
        match text("id").filter(|i| is_token_id(i)) {
            Some(i) => Some(i),
            None => return malformed,
        }
    };
    // Display fields are deploy-only; a malformed one is dropped, never fatal (BRC-161).
    let deploy = op == "deploy+mint";
    let dec = text("dec").filter(|d| deploy && canonical_u64(d) && d.parse::<u8>().map_or(false, |n| n <= 18));
    let icon = text("icon").filter(|i| deploy && (is_token_id(i)
        || i.strip_prefix('_').map_or(false, |v| canonical_u64(v) && v.parse::<u32>().is_ok())));
    let sym = text("sym").filter(|_| deploy);
    Recognised::Bsv21Value(Bsv21 { id, op, amt, sym, dec, icon })
}

/// Where and how a recognised token is filed (BRC-147 / BRC-163).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Filing {
    Ordinal { content_type: String },
    Bsv21(Bsv21),
}

impl Filing {
    pub fn basket(&self) -> &'static str {
        match self {
            Filing::Ordinal { .. } => ONESAT_BASKET,
            Filing::Bsv21(_) => BSV21_BASKET,
        }
    }

    /// The token id, with `deploy+mint`'s own outpoint filled in.
    fn token_id(b: &Bsv21, txid: &str, vout: i32) -> String {
        b.id.clone().unwrap_or_else(|| format!("{}_{}", txid, vout))
    }

    /// The wallet's own tags for the row.
    ///
    /// Ordinals: `ordinal` and `type:<mime>` — what the script shows. ⚠️ No `origin` tag:
    /// BRC-147 writes it "when origin is known", and an envelope may be a re-inscription
    /// on a later output (BRC-160), so origin is B5-T2-P3's to verify, not ours to claim.
    pub fn tags(&self, txid: &str, vout: i32) -> Vec<String> {
        match self {
            Filing::Ordinal { content_type } => {
                let mut t = vec!["ordinal".to_string()];
                if !content_type.is_empty() {
                    t.push(format!("type:{}", content_type));
                }
                t
            }
            Filing::Bsv21(b) => {
                let mut t = vec![format!("bsv21:{}", Self::token_id(b, txid, vout))];
                if b.op == "deploy+mint" {
                    t.push("bsv21:deploy".to_string());
                }
                t
            }
        }
    }

    /// `customInstructions` the wallet writes when the row has none (a sender's string is
    /// kept verbatim, BRC-37). Ordinals: none — every BRC-147 CI field is a claim we
    /// cannot make from the script alone.
    pub fn custom_instructions(&self, txid: &str, vout: i32) -> Option<String> {
        match self {
            Filing::Ordinal { .. } => None,
            Filing::Bsv21(b) => {
                let mut ci = serde_json::Map::new();
                ci.insert("id".into(), Self::token_id(b, txid, vout).into());
                ci.insert("amt".into(), b.amt.clone().into());
                ci.insert("op".into(), b.op.clone().into());
                if let Some(s) = &b.sym { ci.insert("sym".into(), s.clone().into()); }
                if let Some(d) = &b.dec { ci.insert("dec".into(), d.clone().into()); }
                if let Some(i) = &b.icon {
                    let icon = if i.starts_with('_') { format!("{}{}", txid, i) } else { i.clone() };
                    ci.insert("icon".into(), icon.into());
                }
                Some(serde_json::Value::Object(ci).to_string())
            }
        }
    }
}

/// The seam's token verdict for a script and its satoshi value: filed, held (with the
/// kind), unreadable, or not a token. Eligibility is `satoshis == 1` **and** the shape —
/// never the value alone (BRC-147 / BRC-163 eligibility 1; an envelope on more than one
/// satoshi is outside BRC-160 and is held).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    NotToken,
    File(Filing),
    Held(&'static str),
    Unreadable,
}

pub fn verdict(script: &[u8], satoshis: i64) -> Verdict {
    match recognise(script) {
        Recognised::None => Verdict::NotToken,
        Recognised::Unreadable => Verdict::Unreadable,
        Recognised::Ordinal { content_type } if satoshis == 1 => Verdict::File(Filing::Ordinal { content_type }),
        Recognised::Bsv21Value(b) if satoshis == 1 => Verdict::File(Filing::Bsv21(b)),
        Recognised::Ordinal { .. } | Recognised::Bsv21Value(_) => Verdict::Held("envelope-on-multi-sat"),
        Recognised::Held(k) => Verdict::Held(k.name()),
    }
}

/// A `listOutputs` request under BRC-165 (`p 1sat <scope>`, pinned `5b5d875f23`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum P1satView {
    /// Not a `p 1sat` basket — the ordinary path.
    NotP1sat,
    /// Refused (bare `p 1sat`, extra or unknown scope, missing axis tag).
    Reject(String),
    /// Read storage basket `1sat`. `axis_tag` (`<scope>:<value>`) is a **mandatory** filter
    /// for every scope but `all`; `grant_key` is the view grant to check (`None` for `id`,
    /// which BRC-165 lets through without an inventory grant).
    View { scope: &'static str, axis_tag: Option<String>, grant_key: Option<String> },
}

/// Parse a `listOutputs` basket + tags as a BRC-165 view request.
///
/// ⚠️ Hodos takes exactly **one** axis value per request (BRC-165 allows several): a
/// prompt names one value and a remembered grant covers one, so a request carrying two
/// could be approved for the first and return rows of both.
pub fn p1sat_view(basket: &str, tags: &[String]) -> P1satView {
    let b = basket.trim().to_lowercase();
    // Plain `1sat` (BRC-147's own list example) is routed through the same module as
    // `p 1sat all` — BRC-165 lets a wallet "route plain 1sat through the same module"
    // (adversarial review #5: it used to be refused).
    if b == ONESAT_BASKET {
        return P1satView::View { scope: "all", axis_tag: None, grant_key: Some("p 1sat all".into()) };
    }
    if b == "p 1sat" {
        return P1satView::Reject("basket 'p 1sat' needs a scope: all, collection, app, creator or id (BRC-165)".into());
    }
    let Some(rest) = b.strip_prefix("p 1sat ") else {
        return P1satView::NotP1sat;
    };
    let words: Vec<&str> = rest.split_whitespace().collect();
    let scope = match words.as_slice() {
        ["all"] => "all",
        ["collection"] => "collection",
        ["app"] => "app",
        ["creator"] => "creator",
        ["id"] => "id",
        [one] => return P1satView::Reject(format!("unknown scope '{}' in basket '{}' (BRC-165)", one, b)),
        _ => return P1satView::Reject(format!("basket '{}': exactly one scope token after 'p 1sat' (BRC-165)", b)),
    };
    if scope == "all" {
        return P1satView::View { scope, axis_tag: None, grant_key: Some("p 1sat all".into()) };
    }
    let prefix = format!("{}:", scope);
    let axis: Vec<String> = tags.iter().map(|t| t.trim().to_lowercase())
        .filter(|t| t.len() > prefix.len() && t.starts_with(&prefix)).collect();
    match axis.as_slice() {
        [] => P1satView::Reject(format!("basket 'p 1sat {}' requires a '{}<value>' tag (BRC-165)", scope, prefix)),
        [one] => P1satView::View {
            scope,
            axis_tag: Some(one.clone()),
            grant_key: (scope != "id").then(|| format!("p 1sat {} {}", scope, &one[prefix.len()..])),
        },
        _ => P1satView::Reject(format!("basket 'p 1sat {}': one '{}' value per request", scope, prefix)),
    }
}

#[cfg(test)]
#[path = "tokens_tests.rs"]
pub(crate) mod tests;
