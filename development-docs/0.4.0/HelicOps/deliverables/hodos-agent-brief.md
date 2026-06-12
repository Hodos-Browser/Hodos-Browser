# Code Audit — Hodos Browser — Findings Reference

> **This is an informational audit report — not a task list and not an instruction to act.**
> It describes findings only. Do **not** make any edits, commits, or fixes based on this
> document unless the user explicitly asks you to in their prompt. If they do, treat
> everything below as **suggestions** that inform your work — not rules you must follow.
> Apply your own and your team's judgment, tooling, and process.

```yaml
artifact: code-audit-findings-reference
nature: informational   # describes findings; implies no action
verdict: needs_review
files_scanned: 108
findings_total: 3152
priority_findings: 479   # security/correctness/performance, listed below
style_findings: 2673          # maintainability, summarized only
priority_by_severity: {critical: 5, high: 405, medium: 69}
suggested_handling: {human_only: 5, review_suggested: 472, automatable: 2}
```

## About this document

- **Purpose:** a reference list of issues an external code audit surfaced in this codebase, with context and suggested remediation — so the information is on hand *if and when* the user asks you to act on it.
- **No autonomous action:** take no action on these findings unless the user explicitly requests it in their prompt.
- **Suggestions, not directives:** the categories, severities, and "suggested handling" below are the audit's opinion. They are inputs to your judgment, not requirements.
- **Machine-readable:** the same records are in `findings.jsonl` (one JSON object per finding).

## Findings — security, correctness & performance

479 findings across 21 categories. For each: what it is, why it may matter, a suggested fix, then every location. All of this is informational.

### Unhandled error / panic risk (DoS)
*tier: security · suggested handling: worth a human review · 284 finding(s) (high: 284)*

- **Why it may matter:** An unwrap/expect reachable from a request path turns bad input into a crash (DoS).
- **Suggested outcome:** Handler paths return errors instead of panicking on bad input.
- **Suggested fix:** Return Result/? and map failures to a 4xx/5xx; reserve unwrap for proven invariants.
- **Common pitfalls:** Wrapping in catch-all that swallows the error — handle it explicitly.

| severity | location | code |
|---|---|---|
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:10417` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:10441` | `match address_repo.get_all_by_wallet(wallet.id.unwrap()) {` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:10549` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:10615` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:10643` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:10768` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:10818` | `if parsed_beef.is_some() && parsed_beef.as_ref().unwrap().has_proofs() {` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:10882` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:10976` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:11281` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:11287` | `.unwrap()` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:1129` | `let re = Regex::new(r#""keyID"\s*:\s*"[^"]*""#).unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:11318` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:1132` | `let data_re = Regex::new(r#""data"\s*:\s*(\[[^\]]+\])"#).unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:11492` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:11511` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:11582` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:11679` | `.unwrap(); // safe: onchain_markers is not empty` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:11733` | `.unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:11842` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:11893` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:11942` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:11945` | `.map_err(\|e\| format!("Wallet error: {}", e)).unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:11946` | `let wallet = wallet.unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:11947` | `let wallet_id = wallet.id.unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:11957` | `).unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:11964` | `let script = Script::p2pkh_locking_script(&pubkey_hash).unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:11987` | `let secret = SecretKey::from_slice(&backup_privkey).unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:11988` | `let message = Message::from_digest_slice(&sighash).unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:12010` | `let secret = SecretKey::from_slice(&backup_privkey).unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:12011` | `let message = Message::from_digest_slice(&sighash).unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:12031` | `let secret = SecretKey::from_slice(&backup_privkey).unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:12032` | `let message = Message::from_digest_slice(&sighash).unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:12045` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:12070` | `let secret = SecretKey::from_slice(&private_key_bytes).unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:12071` | `let message = Message::from_digest_slice(&sighash).unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:12100` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:12176` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:12199` | `.duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:12200` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:12280` | `.duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:12282` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:12292` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:12741` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:12771` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:1289` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:12911` | `let secret_key = secp256k1::SecretKey::from_slice(&master_privkey).unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:12916` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:12951` | `let mut db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:12967` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:12977` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:13022` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:13052` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:13061` | `let mut status = state.sync_status.write().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:13291` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:13360` | `let status = state.sync_status.read().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:13366` | `let mut status = state.sync_status.write().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:13451` | `let mut db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:13477` | `let mut status = state.sync_status.write().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:13506` | `let mut status = state.sync_status.write().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:13536` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:13552` | `.unwrap()` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:13601` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:13611` | `let mut status = state.sync_status.write().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:13655` | `let status = state.sync_status.read().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:13700` | `let wid = wallet.id.unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:13708` | `let mut status = state.sync_status.write().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:13736` | `let mut status = state.sync_status.write().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:13752` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:13771` | `.unwrap()` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:13825` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:13834` | `let mut status = state.sync_status.write().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:13963` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:14013` | `let mut db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:14102` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:14127` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:14237` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:14342` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:14443` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:14538` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:14544` | `Ok(Some(basket)) => basket.id.unwrap(),` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:14582` | `let output_id = db_output.output_id.unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:14661` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:14810` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:14863` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:14922` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:14995` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:15092` | `let mnemonic = Mnemonic::parse_in(Language::English, &mnemonic_trimmed).unwrap()` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:15104` | `let secret_key = SecretKey::from_slice(&privkey).unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:15118` | `let mut db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:15173` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:15259` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:15402` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:15452` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:15495` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:15532` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:15566` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:15593` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:15666` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:15817` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:15837` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:15948` | `let identity_key_re = regex::Regex::new(r"^(02\|03)[0-9a-fA-F]{64}$").unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:15949` | `let bsv_address_re = regex::Regex::new(r"^[13][a-km-zA-HJ-NP-Z1-9]{25,34}$").unw` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:15950` | `let paymail_re = regex::Regex::new(r"^(\$[a-zA-Z0-9_]+\|[a-zA-Z0-9._%+\-]+@[a-zA` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:16057` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:16073` | `conn.prepare("SELECT '' WHERE 0").unwrap()` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:16086` | `let identity_key_re = regex::Regex::new(r"^(02\|03)[0-9a-fA-F]{64}$").unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:16087` | `let bsv_address_re = regex::Regex::new(r"^[13][a-km-zA-HJ-NP-Z1-9]{25,34}$").unw` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:16120` | `let paymail_re = regex::Regex::new(r"to ([a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:16121` | `let addr_re = regex::Regex::new(r"to ([13][a-km-zA-HJ-NP-Z1-9]{25,34})").unwrap(` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:1619` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:16224` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:16246` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:16292` | `let mut db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:16348` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:16394` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:16498` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:16519` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:16590` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:16665` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:1809` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:1881` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:1922` | `let mut db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:1971` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:2028` | `let mut db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:2040` | `).unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:2042` | `.unwrap()` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:2118` | `let mut db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:2169` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:2295` | `let wid = wallet.id.unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:2332` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:2373` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:2418` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:2609` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:267` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:2766` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:2866` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:289` | `let protocol_id = protocol_id.unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:290` | `let key_id = key_id.unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:2940` | `let secret = SecretKey::from_slice(&anyone_privkey).unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:2982` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:329` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:3413` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:3727` | `.map(\|idx\| parsed_input_beef.as_ref().unwrap().transactions[idx].clone());` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:3888` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:3908` | `match address_repo.get_all_by_wallet(wallet.id.unwrap()) {` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:3927` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:393` | `let mut cache = state.derived_key_cache.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:4001` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:403` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:4042` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:406` | `.unwrap()` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:4120` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:4159` | `let placeholder = reservation_placeholder.as_ref().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:4164` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:4300` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:439` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:4461` | `.expect("HODOS_FEE_ADDRESS constant is invalid");` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:4479` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:4492` | `let wallet_id = wallet.id.unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:4556` | `.unwrap()` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:4723` | `format!(" ({})", output.output_description.as_ref().unwrap())` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:4774` | `let mut pending = PENDING_TRANSACTIONS.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:4801` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:4854` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:4971` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:4994` | `locking_script: address_to_script(HODOS_FEE_ADDRESS).unwrap(),` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:5130` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:5197` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:5240` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:5264` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:5328` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:5349` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:5377` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:5487` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:552` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:5523` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:568` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:5768` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:5791` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:584` | `let master_seckey = SecretKey::from_slice(&master_privkey).expect("Valid private` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:6031` | `.unwrap()` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:6278` | `let pending = PENDING_TRANSACTIONS.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:6382` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:6618` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:6634` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:6657` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:6698` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:6737` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:6797` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:683` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:6870` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:6894` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:6921` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:7065` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:7131` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:7187` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:7344` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:7358` | `let mut pending = PENDING_TRANSACTIONS.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:7375` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:739` | `}).unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:745` | `}).unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:7492` | `let create_body = serde_json::to_vec(&create_req).unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:7499` | `let body_bytes = actix_web::body::to_bytes(create_response.into_body()).await.un` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:7500` | `serde_json::from_slice(&body_bytes).unwrap()` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:7522` | `let sign_body = serde_json::to_vec(&sign_req).unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:7528` | `let body_bytes = actix_web::body::to_bytes(sign_response.into_body()).await.unwr` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:7529` | `serde_json::from_slice(&body_bytes).unwrap()` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:7544` | `let pending = PENDING_TRANSACTIONS.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:7569` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:7590` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:7611` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:809` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:829` | `let re = Regex::new(r#""keyID"\s*:\s*"[^"]*""#).unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:832` | `let data_re = Regex::new(r#""data"\s*:\s*(\[[^\]]+\])"#).unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:8577` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:8596` | `let wallet_id = wallet.id.unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:8673` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:8679` | `.unwrap()` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:8724` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:8747` | `let wallet_id = wallet.id.unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:8784` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:8805` | `let wallet_id = wallet.id.unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:9081` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:9115` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:9233` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:9285` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:9352` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:9388` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:9470` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:9492` | `match repo.get_approved_fields(perm.id.unwrap(), cert_type) {` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:9530` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:9537` | `Ok(Some(p)) => p.id.unwrap(),` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:9602` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:9611` | `let perm_id = perm.id.unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:9704` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:973` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:9795` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:9891` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:9940` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers/certificate_handlers.rs:1057` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers/certificate_handlers.rs:1106` | `let initial_request_json = serde_json::to_string(&initial_request_message).unwra` |
| high | `/tmp/hodos/rust-wallet/src/handlers/certificate_handlers.rs:115` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers/certificate_handlers.rs:1421` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers/certificate_handlers.rs:1479` | `let message = Message::from_digest_slice(&data_hash).unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers/certificate_handlers.rs:1502` | `let js_message = Message::from_digest_slice(&js_data_hash).unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers/certificate_handlers.rs:1589` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers/certificate_handlers.rs:1658` | `for (field_name, field_value) in fields.as_object().unwrap() {` |
| high | `/tmp/hodos/rust-wallet/src/handlers/certificate_handlers.rs:175` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers/certificate_handlers.rs:1984` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers/certificate_handlers.rs:2234` | `let secret_key2 = SecretKey::from_slice(&csr_child_private_key).unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers/certificate_handlers.rs:2235` | `let message2 = Message::from_digest_slice(&csr_hash).unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers/certificate_handlers.rs:2258` | `let child_secret_test = SecretKey::from_slice(&csr_child_private_key).unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers/certificate_handlers.rs:2401` | `cert_response.get("certificate").unwrap().clone()` |
| high | `/tmp/hodos/rust-wallet/src/handlers/certificate_handlers.rs:2564` | `let body_bytes = actix_web::body::to_bytes(response.into_body()).await.unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers/certificate_handlers.rs:2565` | `let nested_response: serde_json::Value = serde_json::from_slice(&body_bytes).unw` |
| high | `/tmp/hodos/rust-wallet/src/handlers/certificate_handlers.rs:298` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers/certificate_handlers.rs:3166` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers/certificate_handlers.rs:3359` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers/certificate_handlers.rs:3510` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers/certificate_handlers.rs:3809` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers/certificate_handlers.rs:3847` | `let anyone_pubkey = hex::decode(ANYONE_PUBKEY_HEX).unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers/certificate_handlers.rs:3863` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers/certificate_handlers.rs:3865` | `let cert_fields = cert_repo_kr.get_certificate_fields(certificate.certificate_id` |
| high | `/tmp/hodos/rust-wallet/src/handlers/certificate_handlers.rs:3951` | `let cert_json_string = serde_json::to_string(&cert_publish_json).unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers/certificate_handlers.rs:3973` | `let secret = SecretKey::from_slice(&master_privkey).unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers/certificate_handlers.rs:4114` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers/certificate_handlers.rs:4160` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers/certificate_handlers.rs:4176` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers/certificate_handlers.rs:4201` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers/certificate_handlers.rs:4270` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers/certificate_handlers.rs:4867` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers/certificate_handlers.rs:4891` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers/certificate_handlers.rs:4895` | `.duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;` |
| high | `/tmp/hodos/rust-wallet/src/handlers/certificate_handlers.rs:4918` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers/certificate_handlers.rs:4923` | `.duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;` |
| high | `/tmp/hodos/rust-wallet/src/handlers/certificate_handlers.rs:4971` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers/certificate_handlers.rs:5020` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers/certificate_handlers.rs:5025` | `.duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;` |
| high | `/tmp/hodos/rust-wallet/src/handlers/certificate_handlers.rs:5094` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers/certificate_handlers.rs:5140` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers/certificate_handlers.rs:803` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers/certificate_handlers.rs:890` | `let db = state.database.lock().unwrap();` |
| high | `/tmp/hodos/rust-wallet/src/handlers/certificate_handlers.rs:942` | `let anyone_pubkey = hex::decode(ANYONE_PUBKEY_HEX).unwrap();` |

### Path traversal (unvalidated file path)
*tier: security · suggested handling: worth a human review · 22 finding(s) (high: 22)*

- **Why it may matter:** Opening a non-literal path without containment lets input escape the intended root.
- **Suggested outcome:** Resolved paths are canonicalized and confined to a fixed root.
- **Suggested fix:** Pin a root, canonicalize the candidate, reject anything escaping the root.
- **Common pitfalls:** Blacklisting '..' — canonicalization is the robust check.

| severity | location | code |
|---|---|---|
| high | `/tmp/hodos/cef-native/src/core/Logger.cpp:16` | `logFile.open(logFilePath, std::ios::app);` |
| high | `/tmp/hodos/frontend/src/components/wallet/TokensTab.tsx:54` | `window.open(url, '_blank');` |
| high | `/tmp/hodos/rust-wallet/src/action_storage.rs:273` | `let data = fs::read_to_string(&self.file_path)` |
| high | `/tmp/hodos/rust-wallet/src/action_storage.rs:288` | `fs::write(&self.file_path, data)` |
| high | `/tmp/hodos/rust-wallet/src/action_storage.rs:431` | `let _ = fs::remove_file(&test_path);` |
| high | `/tmp/hodos/rust-wallet/src/action_storage.rs:438` | `let _ = fs::remove_file(&test_path);` |
| high | `/tmp/hodos/rust-wallet/src/action_storage.rs:445` | `let _ = fs::remove_file(&test_path);` |
| high | `/tmp/hodos/rust-wallet/src/action_storage.rs:475` | `let _ = fs::remove_file(&test_path);` |
| high | `/tmp/hodos/rust-wallet/src/action_storage.rs:482` | `let _ = fs::remove_file(&test_path);` |
| high | `/tmp/hodos/rust-wallet/src/action_storage.rs:512` | `let _ = fs::remove_file(&test_path);` |
| high | `/tmp/hodos/rust-wallet/src/action_storage.rs:519` | `let _ = fs::remove_file(&test_path);` |
| high | `/tmp/hodos/rust-wallet/src/action_storage.rs:572` | `let _ = fs::remove_file(&test_path);` |
| high | `/tmp/hodos/rust-wallet/src/backup.rs:1656` | `fs::copy(source_path, dest_path)` |
| high | `/tmp/hodos/rust-wallet/src/backup.rs:1667` | `fs::copy(&wal_path, &dest_wal)` |
| high | `/tmp/hodos/rust-wallet/src/backup.rs:1678` | `fs::copy(&shm_path, &dest_shm)` |
| high | `/tmp/hodos/rust-wallet/src/backup.rs:1718` | `fs::copy(backup_path, dest_path)` |
| high | `/tmp/hodos/rust-wallet/src/backup.rs:1729` | `fs::copy(&backup_wal, &dest_wal)` |
| high | `/tmp/hodos/rust-wallet/src/backup.rs:1740` | `fs::copy(&backup_shm, &dest_shm)` |
| high | `/tmp/hodos/rust-wallet/src/backup.rs:1928` | `fs::write(dest_path, json)` |
| high | `/tmp/hodos/rust-wallet/src/json_storage.rs:45` | `let data = fs::read_to_string(&self.wallet_path)` |
| high | `/tmp/hodos/rust-wallet/src/json_storage.rs:91` | `fs::write(&self.wallet_path, data)` |
| high | `/tmp/hodos/scripts/generate-appcast.py:97` | `with open(args.output, 'wb') as f:` |

### Secret written to log/output
*tier: security · suggested handling: worth a human review · 20 finding(s) (high: 20)*

- **Why it may matter:** Key/seed/credential material in logs persists secrets wherever logs go.
- **Suggested outcome:** Secret-shaped values never reach a log/stdout sink.
- **Suggested fix:** Redact or remove the value before logging; log a non-reversible identifier if needed.
- **Common pitfalls:** Lowering the log level — the secret is still written.

| severity | location | code |
|---|---|---|
| high | `/tmp/hodos/cef-native/src/core/AddressHandler.cpp:71` | `requires login` |
| high | `/tmp/hodos/cef-native/src/core/WalletService.cpp:440` | `requires login` |
| high | `/tmp/hodos/rust-wallet/src/bin/extract_master_key.rs:66` | `println!("📝 Mnemonic found in database (first 20 chars): {}...\n", &mnemonic_phr` |
| high | `/tmp/hodos/rust-wallet/src/certificate/verifier.rs:276` | `log::info!("      Anyone private key (hex): {}", hex::encode(&anyone_private_key` |
| high | `/tmp/hodos/rust-wallet/src/certificate/verifier.rs:310` | `log::info!("         Shared secret (ECDH result, hex, first 16): {}", hex::encod` |
| high | `/tmp/hodos/rust-wallet/src/certificate/verifier.rs:324` | `log::info!("         HMAC scalar (hex): {}", hex::encode(&hmac_secret.secret_byt` |
| high | `/tmp/hodos/rust-wallet/src/crypto/brc2.rs:111` | `log::info!("      Shared secret (hex, first 16): {}", hex::encode(&shared_secret` |
| high | `/tmp/hodos/rust-wallet/src/crypto/brc2.rs:112` | `log::info!("      Symmetric key (hex, first 16): {}", hex::encode(&symmetric_key` |
| high | `/tmp/hodos/rust-wallet/src/crypto/brc2.rs:150` | `log::info!("      Key (hex, first 16): {}", hex::encode(&symmetric_key[..16]));` |
| high | `/tmp/hodos/rust-wallet/src/crypto/brc2.rs:277` | `log::debug!("   BRC-2 encrypt_certificate_field: derived symmetric key (hex, fir` |
| high | `/tmp/hodos/rust-wallet/src/crypto/brc2.rs:281` | `log::info!("      Derived symmetric key (hex, first 16): {}", hex::encode(&symme` |
| high | `/tmp/hodos/rust-wallet/src/crypto/brc2.rs:75` | `log::info!("      Sender private key (hex, first 16): {}", hex::encode(&sender_p` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:6483` | `log::info!("   Private key (first 8 bytes): {}...", hex::encode(&private_key_byt` |
| high | `/tmp/hodos/rust-wallet/src/handlers/certificate_handlers.rs:1434` | `log::info!("      Our master privkey (first 8 bytes): {}", hex::encode(&our_mast` |
| high | `/tmp/hodos/rust-wallet/src/handlers/certificate_handlers.rs:1699` | `log::info!("      Symmetric key (hex, full 32 bytes): {}", hex::encode(&field_sy` |
| high | `/tmp/hodos/rust-wallet/src/handlers/certificate_handlers.rs:1700` | `log::info!("      Symmetric key (base64): {}", base64::engine::general_purpose::` |
| high | `/tmp/hodos/rust-wallet/src/handlers/certificate_handlers.rs:1733` | `log::info!("      Original symmetric key (32 bytes, hex): {}", hex::encode(&fiel` |
| high | `/tmp/hodos/rust-wallet/src/handlers/certificate_handlers.rs:1736` | `log::info!("      Subject private key (hex, first 16): {}", hex::encode(&subject` |
| high | `/tmp/hodos/rust-wallet/src/handlers/certificate_handlers.rs:2250` | `log::info!("      Our master private key (hex, first 16): {}", hex::encode(&mast` |
| high | `/tmp/hodos/rust-wallet/src/handlers/certificate_handlers.rs:2254` | `log::info!("      Derived child private key (hex, first 16): {}", hex::encode(&c` |

### Weak randomness for cryptography
*tier: security · suggested handling: worth a human review · 18 finding(s) (medium: 18)*

- **Why it may matter:** Non-cryptographic RNGs are predictable and unsafe for key/nonce material.
- **Suggested outcome:** All cryptographic material comes from a CSPRNG.
- **Suggested fix:** Draw from the OS CSPRNG for any security-sensitive randomness.
- **Common pitfalls:** Seeding the weak RNG 'better' — the generator is still non-cryptographic.

| severity | location | code |
|---|---|---|
| medium | `/tmp/hodos/rust-wallet/archive/old-tests/interoperability_test.rs:74` | `rand::thread_rng().fill_bytes(&mut iv);` |
| medium | `/tmp/hodos/rust-wallet/src/authfetch.rs:283` | `let bytes: Vec<u8> = (0..32).map(\|_\| rand::random::<u8>()).collect();` |
| medium | `/tmp/hodos/rust-wallet/src/backup.rs:1279` | `rand::thread_rng().fill_bytes(&mut nonce_bytes);` |
| medium | `/tmp/hodos/rust-wallet/src/backup.rs:2031` | `rand::thread_rng().fill_bytes(&mut nonce_bytes);` |
| medium | `/tmp/hodos/rust-wallet/src/backup.rs:2102` | `rand::thread_rng().fill_bytes(&mut nonce_bytes);` |
| medium | `/tmp/hodos/rust-wallet/src/backup.rs:888` | `rand::thread_rng().fill_bytes(&mut salt);` |
| medium | `/tmp/hodos/rust-wallet/src/backup.rs:894` | `rand::thread_rng().fill_bytes(&mut nonce_bytes);` |
| medium | `/tmp/hodos/rust-wallet/src/crypto/brc2.rs:140` | `rand::thread_rng().fill_bytes(&mut iv_bytes);` |
| medium | `/tmp/hodos/rust-wallet/src/crypto/pin.rs:32` | `rand::thread_rng().fill_bytes(&mut salt);` |
| medium | `/tmp/hodos/rust-wallet/src/crypto/pin.rs:38` | `rand::thread_rng().fill_bytes(&mut nonce_bytes);` |
| medium | `/tmp/hodos/rust-wallet/src/database/wallet_repo.rs:30` | `rand::thread_rng().fill_bytes(&mut entropy);` |
| medium | `/tmp/hodos/rust-wallet/src/handlers.rs:15283` | `let prefix_bytes: Vec<u8> = (0..16).map(\|_\| rand::random::<u8>()).collect();` |
| medium | `/tmp/hodos/rust-wallet/src/handlers.rs:15284` | `let suffix_bytes: Vec<u8> = (0..16).map(\|_\| rand::random::<u8>()).collect();` |
| medium | `/tmp/hodos/rust-wallet/src/handlers.rs:562` | `let our_nonce_bytes: [u8; 32] = rand::random();` |
| medium | `/tmp/hodos/rust-wallet/src/handlers/certificate_handlers.rs:1569` | `rand::thread_rng().fill(&mut csr_request_nonce_bytes);` |
| medium | `/tmp/hodos/rust-wallet/src/handlers/certificate_handlers.rs:1689` | `rand::thread_rng().fill_bytes(&mut field_symmetric_key);` |
| medium | `/tmp/hodos/rust-wallet/src/handlers/certificate_handlers.rs:2019` | `rand::thread_rng().fill(&mut csr_message_nonce_bytes);` |
| medium | `/tmp/hodos/rust-wallet/src/handlers/certificate_handlers.rs:498` | `rand::thread_rng().fill_bytes(&mut first_half);` |

### Missing input neutralization
*tier: security · suggested handling: worth a human review · 16 finding(s) (high: 16)*

- **Why it may matter:** This path reaches a sink without the neutralization applied on sibling paths.
- **Suggested outcome:** Input is neutralized before the sink, consistent with the safe sibling paths.
- **Suggested fix:** Apply the same encode/validate step the comparable safe paths already use.
- **Common pitfalls:** A one-off ad-hoc filter — match the established safe pattern instead.

| severity | location | code |
|---|---|---|
| high | `/tmp/hodos/cef-native/src/handlers/simple_render_process_handler.cpp:1185` | `if (message_name == "brc100_auth_request") {` |
| high | `/tmp/hodos/cef-native/src/handlers/simple_render_process_handler.cpp:1228` | `if (message_name == "identity_status_check_response") {` |
| high | `/tmp/hodos/cef-native/src/handlers/simple_render_process_handler.cpp:1243` | `if (message_name == "create_identity_response") {` |
| high | `/tmp/hodos/cef-native/src/handlers/simple_render_process_handler.cpp:1258` | `if (message_name == "mark_identity_backed_up_response") {` |
| high | `/tmp/hodos/cef-native/src/handlers/simple_render_process_handler.cpp:1273` | `if (message_name == "address_generate_error") {` |
| high | `/tmp/hodos/cef-native/src/handlers/simple_render_process_handler.cpp:1334` | `if (message_name == "create_transaction_error") {` |
| high | `/tmp/hodos/cef-native/src/handlers/simple_render_process_handler.cpp:1348` | `if (message_name == "sign_transaction_response") {` |
| high | `/tmp/hodos/cef-native/src/handlers/simple_render_process_handler.cpp:1366` | `if (message_name == "sign_transaction_error") {` |
| high | `/tmp/hodos/cef-native/src/handlers/simple_render_process_handler.cpp:1382` | `if (message_name == "broadcast_transaction_response") {` |
| high | `/tmp/hodos/cef-native/src/handlers/simple_render_process_handler.cpp:1402` | `if (message_name == "broadcast_transaction_error") {` |
| high | `/tmp/hodos/cef-native/src/handlers/simple_render_process_handler.cpp:1469` | `if (message_name == "send_transaction_error") {` |
| high | `/tmp/hodos/cef-native/src/handlers/simple_render_process_handler.cpp:1505` | `if (message_name == "get_balance_error") {` |
| high | `/tmp/hodos/cef-native/src/handlers/simple_render_process_handler.cpp:1521` | `if (message_name == "get_transaction_history_response") {` |
| high | `/tmp/hodos/cef-native/src/handlers/simple_render_process_handler.cpp:1541` | `if (message_name == "get_transaction_history_error") {` |
| high | `/tmp/hodos/cef-native/src/handlers/simple_render_process_handler.cpp:1793` | `if (message_name == "omnibox_select") {` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:7454` | `pub async fn process_action(` |

### Missing origin check on cross-context message
*tier: security · suggested handling: worth a human review · 13 finding(s) (medium: 13)*

- **Why it may matter:** A cross-context message handler without an origin check trusts any sender.
- **Suggested outcome:** Handlers verify sender origin before acting.
- **Suggested fix:** Check the message origin against an allowlist before processing.

| severity | location | code |
|---|---|---|
| medium | `/tmp/hodos/frontend/src/components/WalletPanel.tsx:127` | `window.addEventListener('message', handleHidden);` |
| medium | `/tmp/hodos/frontend/src/components/WalletPanel.tsx:128` | `window.addEventListener('message', handleShown);` |
| medium | `/tmp/hodos/frontend/src/components/WalletPanel.tsx:429` | `window.addEventListener('message', handler);` |
| medium | `/tmp/hodos/frontend/src/hooks/useDownloads.ts:35` | `window.addEventListener('message', handler);` |
| medium | `/tmp/hodos/frontend/src/hooks/useTabManager.ts:186` | `window.addEventListener('message', handlePaymentIndicator);` |
| medium | `/tmp/hodos/frontend/src/hooks/useTabManager.ts:215` | `window.addEventListener('message', handleTabListResponse);` |
| medium | `/tmp/hodos/frontend/src/pages/MainBrowserView.tsx:244` | `window.addEventListener('message', handlePaymentDismissed);` |
| medium | `/tmp/hodos/frontend/src/pages/MainBrowserView.tsx:293` | `window.addEventListener('message', handleMessage);` |
| medium | `/tmp/hodos/frontend/src/pages/MainBrowserView.tsx:457` | `window.addEventListener('message', handleAutocomplete);` |
| medium | `/tmp/hodos/frontend/src/pages/NewTabPage.tsx:156` | `window.addEventListener('message', handler);` |
| medium | `/tmp/hodos/frontend/src/pages/WalletPanelPage.tsx:104` | `window.addEventListener('message', handler);` |
| medium | `/tmp/hodos/frontend/src/pages/WalletPanelPage.tsx:283` | `window.addEventListener('message', handleHidden);` |
| medium | `/tmp/hodos/frontend/src/pages/WalletPanelPage.tsx:284` | `window.addEventListener('message', handleShown);` |

### Untrusted input reaches code-execution sink (injection)
*tier: security · suggested handling: worth a human review · 12 finding(s) (high: 12)*

- **Why it may matter:** Untrusted input reaches a code/script execution sink unescaped (injection).
- **Suggested outcome:** Input is serialized/encoded before reaching the execution sink.
- **Suggested fix:** Encode via a JSON serializer (or equivalent) before interpolation; prefer a data API over string-built code.
- **Common pitfalls:** HTML/JS-escaping by hand — use a serializer for the data context.

| severity | location | code |
|---|---|---|
| high | `/tmp/hodos/cef-native/src/handlers/simple_render_process_handler.cpp:1167` | `requires login` |
| high | `/tmp/hodos/cef-native/src/handlers/simple_render_process_handler.cpp:1180` | `requires login` |
| high | `/tmp/hodos/cef-native/src/handlers/simple_render_process_handler.cpp:1210` | `requires login` |
| high | `/tmp/hodos/cef-native/src/handlers/simple_render_process_handler.cpp:1223` | `requires login` |
| high | `/tmp/hodos/cef-native/src/handlers/simple_render_process_handler.cpp:1238` | `requires login` |
| high | `/tmp/hodos/cef-native/src/handlers/simple_render_process_handler.cpp:1253` | `requires login` |
| high | `/tmp/hodos/cef-native/src/handlers/simple_render_process_handler.cpp:1268` | `requires login` |
| high | `/tmp/hodos/cef-native/src/handlers/simple_render_process_handler.cpp:1281` | `requires login` |
| high | `/tmp/hodos/cef-native/src/handlers/simple_render_process_handler.cpp:1297` | `requires login` |
| high | `/tmp/hodos/cef-native/src/handlers/simple_render_process_handler.cpp:927` | `requires login` |
| high | `/tmp/hodos/cef-native/src/handlers/simple_render_process_handler.cpp:979` | `requires login` |
| high | `/tmp/hodos/cef-native/src/handlers/simple_render_process_handler.cpp:996` | `requires login` |

### Prototype pollution
*tier: security · suggested handling: worth a human review · 7 finding(s) (high: 7)*

- **Why it may matter:** Unvalidated keys merged into objects can corrupt prototypes.
- **Suggested outcome:** Object keys are validated; prototype keys are rejected.
- **Suggested fix:** Reject __proto__/constructor keys; use null-prototype maps for untrusted data.

| severity | location | code |
|---|---|---|
| high | `/tmp/hodos/frontend/src/components/TabBar.tsx:64` | `tabElsRef.current[index] = el;` |
| high | `/tmp/hodos/frontend/src/components/wallet/TokensTab.tsx:61` | `if (!acc[key]) acc[key] = [];` |
| high | `/tmp/hodos/frontend/src/pages/WalletPanelPage.tsx:27` | `next[index] = digit;` |
| high | `/tmp/hodos/frontend/src/pages/WalletPanelPage.tsx:405` | `newWords[i] = pastedWords[wi].toLowerCase();` |
| high | `/tmp/hodos/frontend/src/pages/WalletPanelPage.tsx:416` | `newWords[index] = value.toLowerCase().replace(/\s/g, '');` |
| high | `/tmp/hodos/frontend/src/pages/WalletPanelPage.tsx:660` | `newWords[i] = pastedWords[wi].toLowerCase();` |
| high | `/tmp/hodos/frontend/src/pages/WalletPanelPage.tsx:671` | `newWords[index] = value.toLowerCase().replace(/\s/g, '');` |

### Insecure deserialization
*tier: security · suggested handling: worth a human review · 7 finding(s) (high: 7)*

- **Why it may matter:** Deserializing untrusted data can instantiate unexpected types.
- **Suggested outcome:** Untrusted input only deserializes through a schema/allowlist.
- **Suggested fix:** Use a safe parser bound to an explicit schema; reject unknown types.
- **Common pitfalls:** Switching parser without validating the data shape.

| severity | location | code |
|---|---|---|
| high | `/tmp/hodos/frontend/src/hooks/useBitcoinBrowser.ts:42` | `const addressData = JSON.parse(event.detail.args[0]);` |
| high | `/tmp/hodos/frontend/src/hooks/useDownloads.ts:26` | `? JSON.parse(event.data.data)` |
| high | `/tmp/hodos/frontend/src/hooks/useHodosBrowser.ts:42` | `const addressData = JSON.parse(event.detail.args[0]);` |
| high | `/tmp/hodos/frontend/src/hooks/useTabManager.ts:143` | `const { cents, domain } = JSON.parse(event.data.data);` |
| high | `/tmp/hodos/frontend/src/hooks/useTabManager.ts:195` | `const response: TabListResponse = JSON.parse(event.data.data);` |
| high | `/tmp/hodos/frontend/src/pages/MainBrowserView.tsx:281` | `? JSON.parse(event.data.data)` |
| high | `/tmp/hodos/frontend/src/pages/NewTabPage.tsx:127` | `? JSON.parse(event.data.data)` |

### Unsafe memory copy / deserialization
*tier: security · suggested handling: worth a human review · 6 finding(s) (high: 6)*

- **Why it may matter:** A copy whose length is not a literal/sizeof can overflow the destination.
- **Suggested outcome:** Copy length is validated against the remaining buffer before the copy.
- **Suggested fix:** Validate length against destination capacity, or use a checked copy.

| severity | location | code |
|---|---|---|
| high | `/tmp/hodos/cef-native/src/core/HttpRequestInterceptor.cpp:714` | `memcpy(data_out, responseData_.c_str() + responseOffset_, bytes_read);` |
| high | `/tmp/hodos/cef-native/src/handlers/my_overlay_render_handler.cpp:208` | `std::memcpy(dib_data_, buffer, width * height * 4);` |
| high | `/tmp/hodos/cef-native/src/handlers/simple_handler.cpp:6097` | `memcpy(data_out, buffer_.data() + write_offset_, to_write);` |
| high | `/tmp/hodos/cef-native/src/handlers/simple_handler.cpp:6843` | `memcpy(GlobalLock(hGlob), text.c_str(), text.size() + 1);` |
| high | `/tmp/hodos/cef-native/third_party/quirc/decode.c:227` | `memcpy(sigma, C, MAX_POLY);` |
| high | `/tmp/hodos/cef-native/third_party/quirc/quirc.c:84` | `(void)memcpy(image, q->image, min);` |

### Unsafe serialization
*tier: security · suggested handling: worth a human review · 4 finding(s) (high: 4)*

- **Why it may matter:** Serialization without bounds/validation can emit malformed or oversized output.
- **Suggested outcome:** Serialized output is bounded and validated.
- **Suggested fix:** Validate and bound the serialized payload.

| severity | location | code |
|---|---|---|
| high | `/tmp/hodos/rust-wallet/src/crypto/dpapi.rs:117` | `let _ = LocalFree(Some(HLOCAL(data_out.pbData as *mut core::ffi::c_void)));` |
| high | `/tmp/hodos/rust-wallet/src/crypto/dpapi.rs:31` | `pbData: plaintext.as_ptr() as *mut u8,` |
| high | `/tmp/hodos/rust-wallet/src/crypto/dpapi.rs:66` | `let _ = LocalFree(Some(HLOCAL(data_out.pbData as *mut core::ffi::c_void)));` |
| high | `/tmp/hodos/rust-wallet/src/crypto/dpapi.rs:83` | `pbData: encrypted.as_ptr() as *mut u8,` |

### Command execution via shell
*tier: security · suggested handling: high stakes — a person should handle this · 3 finding(s) (critical: 3)*

- **Why it may matter:** Spawning a shell from a browser/embedded process is an injection surface.
- **Suggested outcome:** No user-influenced data reaches a shell; process control uses a direct exec API.
- **Suggested fix:** Replace shell-out with a direct process API (posix_spawn/execve) or remove it.
- **Common pitfalls:** Escaping the string and keeping the shell call — the shell itself is the risk.

| severity | location | code |
|---|---|---|
| critical | `/tmp/hodos/cef-native/src/core/ProfileManager.cpp:437` | `int ret = system(cmd.c_str());` |
| critical | `/tmp/hodos/cef-native/src/handlers/simple_handler.cpp:6851` | `FILE* pipe = popen("pbcopy", "w");` |
| critical | `/tmp/hodos/cef-native/src/handlers/simple_handler.cpp:7105` | `FILE* pipe = popen("pbpaste", "r");` |

### Unsafe code block (memory safety)
*tier: security · suggested handling: worth a human review · 2 finding(s) (high: 2)*

- **Why it may matter:** An unsafe block carries manual memory-safety obligations.
- **Suggested outcome:** Each unsafe block is minimal, justified, and locally provable.
- **Suggested fix:** Shrink the unsafe region, document the invariant, prefer a safe abstraction.

| severity | location | code |
|---|---|---|
| high | `/tmp/hodos/rust-wallet/src/crypto/dpapi.rs:39` | `unsafe {` |
| high | `/tmp/hodos/rust-wallet/src/crypto/dpapi.rs:91` | `unsafe {` |

### Hardcoded secret in source
*tier: security · suggested handling: high stakes — a person should handle this · 1 finding(s) (critical: 1)*

- **Why it may matter:** A credential committed to source is exposed to anyone with repo access.
- **Suggested outcome:** No credential literal lives in source; secrets load from runtime config.
- **Suggested fix:** Move the value to environment/secret manager; document the variable; rotate the exposed key.
- **Common pitfalls:** Moving the literal to another committed file — still exposed. Splitting the literal to dodge detection — the literal is the problem.

| severity | location | code |
|---|---|---|
| critical | `/tmp/hodos/rust-wallet/src/handlers.rs:8332` | `let api_key = "mainnet_fa871d12caa95b39076ac0b6b532a410";` |

### SQL injection (string-built query)
*tier: security · suggested handling: high stakes — a person should handle this · 1 finding(s) (critical: 1)*

- **Why it may matter:** Query text built by string interpolation lets input alter query structure.
- **Suggested outcome:** Query structure is fixed; all input arrives as bound parameters.
- **Suggested fix:** Use parameter binding ($1/?N/:name); never format input into SQL text.
- **Common pitfalls:** Quoting/escaping the value manually — bind parameters instead.

| severity | location | code |
|---|---|---|
| critical | `/tmp/hodos/rust-wallet/src/handlers.rs:2048` | `if let Err(e) = conn.execute(&format!("DROP TABLE IF EXISTS \"{}\"", table), [])` |

### Mutable global state
*tier: correctness · suggested handling: worth a human review · 33 finding(s) (medium: 33)*

- **Why it may matter:** Mutable global/static state is shared, hard to reason about, and race-prone.
- **Suggested outcome:** Shared state is scoped or passed explicitly, not global-mutable.
- **Suggested fix:** Make it const, scope it function-local, or thread it through the call graph.

| severity | location | code |
|---|---|---|
| medium | `/tmp/hodos/cef-native/OverlayHelpers_mac.h:35` | `CGFloat overlayHeight, CGFloat headerHeight);` |
| medium | `/tmp/hodos/cef-native/include/core/AdblockCache.h:86` | `class AdblockCache;` |
| medium | `/tmp/hodos/cef-native/include/core/Tab.h:14` | `class SimpleHandler;` |
| medium | `/tmp/hodos/cef-native/include/handlers/simple_app.h:78` | `public CefBrowserProcessHandler,` |
| medium | `/tmp/hodos/cef-native/include/handlers/simple_handler.h:23` | `class TabManager;` |
| medium | `/tmp/hodos/cef-native/include/handlers/simple_handler.h:24` | `class BrowserWindow;` |
| medium | `/tmp/hodos/cef-native/include/handlers/simple_handler.h:27` | `public CefLifeSpanHandler,` |
| medium | `/tmp/hodos/cef-native/include/handlers/simple_handler.h:28` | `public CefDisplayHandler,` |
| medium | `/tmp/hodos/cef-native/include/handlers/simple_handler.h:29` | `public CefLoadHandler,` |
| medium | `/tmp/hodos/cef-native/include/handlers/simple_handler.h:30` | `public CefRequestHandler,` |
| medium | `/tmp/hodos/cef-native/include/handlers/simple_handler.h:31` | `public CefContextMenuHandler,` |
| medium | `/tmp/hodos/cef-native/include/handlers/simple_handler.h:32` | `public CefDialogHandler,` |
| medium | `/tmp/hodos/cef-native/include/handlers/simple_handler.h:33` | `public CefKeyboardHandler,` |
| medium | `/tmp/hodos/cef-native/include/handlers/simple_handler.h:34` | `public CefPermissionHandler,` |
| medium | `/tmp/hodos/cef-native/include/handlers/simple_handler.h:35` | `public CefDownloadHandler,` |
| medium | `/tmp/hodos/cef-native/include/handlers/simple_handler.h:36` | `public CefFindHandler,` |
| medium | `/tmp/hodos/cef-native/third_party/quirc/decode.c:131` | `int shift, const struct galois_field *gf)` |
| medium | `/tmp/hodos/cef-native/third_party/quirc/decode.c:640` | `int bits, int digits)` |
| medium | `/tmp/hodos/cef-native/third_party/quirc/decode.c:697` | `int bits, int digits)` |
| medium | `/tmp/hodos/cef-native/third_party/quirc/identify.c:101` | `quirc_float_t u, quirc_float_t v, struct quirc_point *ret)` |
| medium | `/tmp/hodos/cef-native/third_party/quirc/identify.c:113` | `quirc_float_t *u, quirc_float_t *v)` |
| medium | `/tmp/hodos/cef-native/third_party/quirc/identify.c:133` | `int from, int to,` |
| medium | `/tmp/hodos/cef-native/third_party/quirc/identify.c:134` | `span_func_t func, void *user_data,` |
| medium | `/tmp/hodos/cef-native/third_party/quirc/identify.c:135` | `int *leftp, int *rightp)` |
| medium | `/tmp/hodos/cef-native/third_party/quirc/identify.c:168` | `quirc_pixel_t *row,` |
| medium | `/tmp/hodos/cef-native/third_party/quirc/identify.c:169` | `int from, int to,` |
| medium | `/tmp/hodos/cef-native/third_party/quirc/identify.c:170` | `span_func_t func, void *user_data,` |
| medium | `/tmp/hodos/cef-native/third_party/quirc/identify.c:210` | `int x0, int y0,` |
| medium | `/tmp/hodos/cef-native/third_party/quirc/identify.c:211` | `int from, int to,` |
| medium | `/tmp/hodos/cef-native/third_party/quirc/identify.c:212` | `span_func_t func, void *user_data)` |
| medium | `/tmp/hodos/cef-native/third_party/quirc/identify.c:432` | `int rcode, const struct quirc_point *ref,` |
| medium | `/tmp/hodos/cef-native/third_party/quirc/identify.c:71` | `quirc_float_t w, quirc_float_t h)` |
| medium | `/tmp/hodos/frontend/src/components/HodosButton.tsx:35` | `HodosButton.displayName = 'HodosButton';` |

### Blocking call in async context
*tier: correctness · suggested handling: worth a human review · 16 finding(s) (high: 16)*

- **Why it may matter:** A blocking call inside async code stalls the executor and starves other tasks.
- **Suggested outcome:** Async paths use non-blocking I/O or offload blocking work.
- **Suggested fix:** Use the async I/O API, or run the blocking call on a dedicated thread pool.

| severity | location | code |
|---|---|---|
| high | `/tmp/hodos/adblock-engine/src/engine.rs:402` | `std::fs::create_dir_all(&lists_dir)` |
| high | `/tmp/hodos/adblock-engine/src/engine.rs:684` | `std::fs::create_dir_all(&lists_dir)` |
| high | `/tmp/hodos/adblock-engine/src/engine.rs:686` | `std::fs::create_dir_all(&resources_dir)` |
| high | `/tmp/hodos/adblock-engine/src/engine.rs:737` | `if let Err(e) = std::fs::write(&scriptlets_path, &text) {` |
| high | `/tmp/hodos/adblock-engine/src/engine.rs:765` | `if let Err(e) = std::fs::write(&entity_path, &text) {` |
| high | `/tmp/hodos/adblock-engine/src/engine.rs:783` | `if let Err(e) = std::fs::write(&engine_path, &serialized) {` |
| high | `/tmp/hodos/adblock-engine/src/engine.rs:801` | `if let Err(e) = std::fs::write(` |
| high | `/tmp/hodos/adblock-engine/src/engine.rs:855` | `if let Err(e) = std::fs::write(&list_path, &text) {` |
| high | `/tmp/hodos/adblock-engine/src/engine.rs:862` | `match std::fs::read_to_string(&list_path) {` |
| high | `/tmp/hodos/rust-wallet/src/beef_helpers.rs:172` | `db: &Mutex<WalletDatabase>,` |
| high | `/tmp/hodos/rust-wallet/src/beef_helpers.rs:31` | `db: &Mutex<WalletDatabase>,` |
| high | `/tmp/hodos/rust-wallet/src/cache_helpers.rs:336` | `db: &std::sync::Mutex<WalletDatabase>,` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:11294` | `let size_bytes = std::fs::metadata(dest_path)` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:11321` | `let size_bytes = std::fs::metadata(dest_path)` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:7696` | `db_for_cache: Option<&std::sync::Arc<std::sync::Mutex<crate::database::WalletDat` |
| high | `/tmp/hodos/rust-wallet/src/main.rs:181` | `if let Err(e) = std::fs::create_dir_all(&wallet_dir) {` |

### Inefficient lookup in loop
*tier: performance · suggested handling: worth a human review · 9 finding(s) (high: 9)*

- **Why it may matter:** A linear membership scan inside a loop is quadratic at scale.
- **Suggested outcome:** Membership checks are O(1) where the loop is hot.
- **Suggested fix:** Hoist a set/map for membership instead of scanning in the loop.

| severity | location | code |
|---|---|---|
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:10499` | `if !normalized_tags.contains(&normalized) {` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:12127` | `if !ancestor_txids.contains(extra_txid) {` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:12132` | `if !ancestor_txids.contains(&utxo.txid) {` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:13104` | `if !txids_to_fetch.contains(txid) {` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:13111` | `if !txids_to_fetch.contains(&ptx.txid) {` |
| high | `/tmp/hodos/rust-wallet/src/handlers.rs:3604` | `if !output_tags.contains(&normalized) {` |
| high | `/tmp/hodos/rust-wallet/src/handlers/certificate_handlers.rs:4569` | `if !ancestor_txids.contains(&utxo.txid) {` |
| high | `/tmp/hodos/rust-wallet/src/monitor/task_sync_pending.rs:194` | `if !new_txids.contains(&utxo.txid) {` |
| high | `/tmp/hodos/rust-wallet/src/monitor/task_sync_pending.rs:292` | `if !new_txids.contains(&utxo.txid) {` |

### Repeated expensive work in loop
*tier: performance · suggested handling: often straightforward to automate · 2 finding(s) (medium: 2)*

- **Why it may matter:** Expensive construction repeated each iteration wastes work.
- **Suggested outcome:** Invariant expensive work is hoisted out of the loop.
- **Suggested fix:** Build it once before the loop and reuse.

| severity | location | code |
|---|---|---|
| medium | `/tmp/hodos/rust-wallet/src/handlers.rs:16086` | `let identity_key_re = regex::Regex::new(r"^(02\|03)[0-9a-fA-F]{64}$").unwrap();` |
| medium | `/tmp/hodos/rust-wallet/src/handlers.rs:16087` | `let bsv_address_re = regex::Regex::new(r"^[13][a-km-zA-HJ-NP-Z1-9]{25,34}$").unw` |

### Inefficient scan in loop
*tier: performance · suggested handling: worth a human review · 2 finding(s) (medium: 2)*

- **Why it may matter:** A repeated linear scan inside a loop is quadratic at scale.
- **Suggested outcome:** Repeated lookups use an index, not a per-iteration scan.
- **Suggested fix:** Precompute an index/map outside the loop.

| severity | location | code |
|---|---|---|
| medium | `/tmp/hodos/cef-native/src/core/TabManager.cpp:335` | `if (std::find(tab_order_.begin(), tab_order_.end(), pair.first) == tab_order_.en` |
| medium | `/tmp/hodos/cef-native/src/core/TabManager_mac.mm:325` | `if (std::find(tab_order_.begin(), tab_order_.end(), pair.first) == tab_order_.en` |

### Expensive / unbounded recursion
*tier: performance · suggested handling: worth a human review · 1 finding(s) (medium: 1)*

- **Why it may matter:** Multi-branch recursion can grow exponentially without memoization.
- **Suggested outcome:** Recursion shrinks its argument and memoizes shared subproblems.
- **Suggested fix:** Confirm the base case strictly shrinks; memoize repeated subproblems.

| severity | location | code |
|---|---|---|
| medium | `/tmp/hodos/rust-wallet/src/beef.rs:1266` | `fn find_or_compute_node(proof: &MerkleProof, level: usize, offset: u64) -> Resul` |

## Findings grouped by file

For convenience, in case the user later asks you to work through a particular file.

| file | findings | severities | categories |
|---|---|---|---|
| `/tmp/hodos/rust-wallet/src/handlers.rs` | 258 | critical:2, high:251, medium:5 | Unhandled error / panic risk (DoS) (240), Inefficient lookup in loop (6), Blocking call in async context (3), Weak randomness for cryptography (3), Repeated expensive work in loop (2), Hardcoded secret in source (1), SQL injection (string-built query) (1), Secret written to log/output (1), Missing input neutralization (1) |
| `/tmp/hodos/rust-wallet/src/handlers/certificate_handlers.rs` | 56 | high:52, medium:4 | Unhandled error / panic risk (DoS) (44), Secret written to log/output (7), Weak randomness for cryptography (4), Inefficient lookup in loop (1) |
| `/tmp/hodos/cef-native/src/handlers/simple_render_process_handler.cpp` | 27 | high:27 | Missing input neutralization (15), Untrusted input reaches code-execution sink (injection) (12) |
| `/tmp/hodos/cef-native/third_party/quirc/identify.c` | 13 | medium:13 | Mutable global state (13) |
| `/tmp/hodos/rust-wallet/src/backup.rs` | 12 | high:7, medium:5 | Path traversal (unvalidated file path) (7), Weak randomness for cryptography (5) |
| `/tmp/hodos/cef-native/include/handlers/simple_handler.h` | 12 | medium:12 | Mutable global state (12) |
| `/tmp/hodos/rust-wallet/src/action_storage.rs` | 10 | high:10 | Path traversal (unvalidated file path) (10) |
| `/tmp/hodos/adblock-engine/src/engine.rs` | 9 | high:9 | Blocking call in async context (9) |
| `/tmp/hodos/frontend/src/pages/WalletPanelPage.tsx` | 8 | high:5, medium:3 | Prototype pollution (5), Missing origin check on cross-context message (3) |
| `/tmp/hodos/rust-wallet/src/crypto/brc2.rs` | 7 | high:6, medium:1 | Secret written to log/output (6), Weak randomness for cryptography (1) |
| `/tmp/hodos/rust-wallet/src/crypto/dpapi.rs` | 6 | high:6 | Unsafe serialization (4), Unsafe code block (memory safety) (2) |
| `/tmp/hodos/cef-native/src/handlers/simple_handler.cpp` | 4 | critical:2, high:2 | Command execution via shell (2), Unsafe memory copy / deserialization (2) |
| `/tmp/hodos/frontend/src/hooks/useTabManager.ts` | 4 | high:2, medium:2 | Insecure deserialization (2), Missing origin check on cross-context message (2) |
| `/tmp/hodos/frontend/src/pages/MainBrowserView.tsx` | 4 | high:1, medium:3 | Missing origin check on cross-context message (3), Insecure deserialization (1) |
| `/tmp/hodos/cef-native/third_party/quirc/decode.c` | 4 | high:1, medium:3 | Mutable global state (3), Unsafe memory copy / deserialization (1) |
| `/tmp/hodos/rust-wallet/src/certificate/verifier.rs` | 3 | high:3 | Secret written to log/output (3) |
| `/tmp/hodos/frontend/src/components/WalletPanel.tsx` | 3 | medium:3 | Missing origin check on cross-context message (3) |
| `/tmp/hodos/frontend/src/components/wallet/TokensTab.tsx` | 2 | high:2 | Path traversal (unvalidated file path) (1), Prototype pollution (1) |
| `/tmp/hodos/rust-wallet/src/json_storage.rs` | 2 | high:2 | Path traversal (unvalidated file path) (2) |
| `/tmp/hodos/rust-wallet/src/beef_helpers.rs` | 2 | high:2 | Blocking call in async context (2) |
| `/tmp/hodos/rust-wallet/src/monitor/task_sync_pending.rs` | 2 | high:2 | Inefficient lookup in loop (2) |
| `/tmp/hodos/frontend/src/hooks/useDownloads.ts` | 2 | high:1, medium:1 | Insecure deserialization (1), Missing origin check on cross-context message (1) |
| `/tmp/hodos/frontend/src/pages/NewTabPage.tsx` | 2 | high:1, medium:1 | Insecure deserialization (1), Missing origin check on cross-context message (1) |
| `/tmp/hodos/rust-wallet/src/crypto/pin.rs` | 2 | medium:2 | Weak randomness for cryptography (2) |
| `/tmp/hodos/cef-native/src/core/ProfileManager.cpp` | 1 | critical:1 | Command execution via shell (1) |
| `/tmp/hodos/scripts/generate-appcast.py` | 1 | high:1 | Path traversal (unvalidated file path) (1) |
| `/tmp/hodos/cef-native/src/core/Logger.cpp` | 1 | high:1 | Path traversal (unvalidated file path) (1) |
| `/tmp/hodos/rust-wallet/src/bin/extract_master_key.rs` | 1 | high:1 | Secret written to log/output (1) |
| `/tmp/hodos/rust-wallet/src/cache_helpers.rs` | 1 | high:1 | Blocking call in async context (1) |
| `/tmp/hodos/rust-wallet/src/main.rs` | 1 | high:1 | Blocking call in async context (1) |
| `/tmp/hodos/frontend/src/components/TabBar.tsx` | 1 | high:1 | Prototype pollution (1) |
| `/tmp/hodos/frontend/src/hooks/useBitcoinBrowser.ts` | 1 | high:1 | Insecure deserialization (1) |
| `/tmp/hodos/frontend/src/hooks/useHodosBrowser.ts` | 1 | high:1 | Insecure deserialization (1) |
| `/tmp/hodos/cef-native/src/core/HttpRequestInterceptor.cpp` | 1 | high:1 | Unsafe memory copy / deserialization (1) |
| `/tmp/hodos/cef-native/src/handlers/my_overlay_render_handler.cpp` | 1 | high:1 | Unsafe memory copy / deserialization (1) |
| `/tmp/hodos/cef-native/third_party/quirc/quirc.c` | 1 | high:1 | Unsafe memory copy / deserialization (1) |
| `/tmp/hodos/cef-native/src/core/AddressHandler.cpp` | 1 | high:1 | Secret written to log/output (1) |
| `/tmp/hodos/cef-native/src/core/WalletService.cpp` | 1 | high:1 | Secret written to log/output (1) |
| `/tmp/hodos/cef-native/OverlayHelpers_mac.h` | 1 | medium:1 | Mutable global state (1) |
| `/tmp/hodos/cef-native/include/core/AdblockCache.h` | 1 | medium:1 | Mutable global state (1) |
| `/tmp/hodos/cef-native/include/core/Tab.h` | 1 | medium:1 | Mutable global state (1) |
| `/tmp/hodos/cef-native/include/handlers/simple_app.h` | 1 | medium:1 | Mutable global state (1) |
| `/tmp/hodos/rust-wallet/archive/old-tests/interoperability_test.rs` | 1 | medium:1 | Weak randomness for cryptography (1) |
| `/tmp/hodos/rust-wallet/src/database/wallet_repo.rs` | 1 | medium:1 | Weak randomness for cryptography (1) |
| `/tmp/hodos/rust-wallet/src/authfetch.rs` | 1 | medium:1 | Weak randomness for cryptography (1) |
| `/tmp/hodos/cef-native/src/core/TabManager.cpp` | 1 | medium:1 | Inefficient scan in loop (1) |
| `/tmp/hodos/cef-native/src/core/TabManager_mac.mm` | 1 | medium:1 | Inefficient scan in loop (1) |
| `/tmp/hodos/frontend/src/components/HodosButton.tsx` | 1 | medium:1 | Mutable global state (1) |
| `/tmp/hodos/rust-wallet/src/beef.rs` | 1 | medium:1 | Expensive / unbounded recursion (1) |

## Style / maintainability (summary only)

2673 further findings are style/maintainability (formatting, docs, debug output, complexity). They are not security-relevant and are summarized rather than listed, to keep this reference focused on the higher-signal items.

*Informational code-audit report produced by HelicOps, reflecting the code at scan time. No action is implied or requested by this document. Machine-readable records: `findings.jsonl`.*
