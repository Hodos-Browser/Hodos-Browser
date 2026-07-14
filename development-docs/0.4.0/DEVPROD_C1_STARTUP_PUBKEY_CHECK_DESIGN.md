# C1 — Startup master-pubkey check (design v2)

**Status (2026-07-14): SHELVED — NOT to be built.** Owner decided against the key-*check*: it's insurance, not a fix; the source bug (shared macOS Keychain slot) is fixed at the source (`a85985f`) and benefit to non-dev users is ~nil, not worth a high-risk money/signing-path change. We *prevent* the wrong storage spot (the **Mode-B env safeguard**, §5 — implemented) rather than *detect* a wrong key. This doc is retained for the record and in case we ever want a low-risk log-only tripwire. Everything below is the (unbuilt) design.

---

**Original status:** DESIGN ONLY. No code. Touches the signing/startup path → owner sign-off + adversarial review required before any implementation (CLAUDE.md inv #8, #2, #3).

**v2 (2026-07-14):** revised after a 5-lens adversarial review (`wf_01d524e9-807`). Review verdict: core mechanism **sound and non-circular** (validated against real code); five design-changing corrections folded in below — the largest being **gate at the DB chokepoint, not the HTTP handlers** (Option A as originally written *leaked* through the Monitor's autonomous signing).

**Goal:** make wallet-key contamination *impossible to run with*, environment-independent — a future Keychain/DPAPI/namespace mistake is caught in seconds at startup instead of after signing money with the wrong key. Durable fix for the `a85985f` NULLFAIL class.

---

## 1. What it catches (validated) — Mode A yes, Mode B no

Two contamination modes need **different** defenses. Both are required to close C1.

- **Mode A — wrong-key contamination (the `a85985f` bug) — CAUGHT.** Genuine prod DB (`users.identity_key` = P) but the auto-unlocked secret was clobbered (dev wallet overwrote the shared Keychain item) → cached mnemonic derives Q ≠ P → **mismatch → refuse.** Review confirmed against real code that `get_master_public_key_from_db` (`helpers.rs:35,47-50`) derives strictly from the **cached mnemonic** (via `get_cached_mnemonic`), while `users.identity_key` was persisted from the mnemonic at create time (`connection.rs:299-303/477-481`) and is **never updated afterward** (no `UPDATE users SET identity_key` exists) — so the comparison is genuinely non-tautological.
- **Mode B — namespace flip — NOT caught by the pubkey check.** A stray `HODOS_DEV=1` opens the dev DB *and* the dev secret; they're self-consistent (derived == stored within the dev namespace) → passes. Closed by the **inverted env safeguard** (§5), not this check.

**Encoding false-positive risk (design's old "#1 risk") — DISPROVEN.** Both sides are lowercase hex of the compressed 33-byte secp256k1 key, produced by identical code; `wallet_import` (`handlers.rs:16932`) already compares by exact equality and has shipped. `eq_ignore_ascii_case` is harmless belt-and-suspenders. BIP39 passphrase is empty on every identity-deriving path (no hidden divergence).

---

## 2. The failure mode: gate at the DB chokepoint (REVISED — was the critical review finding)

**Do NOT gate at the top of named HTTP handlers.** The Monitor signs and broadcasts **autonomously**, never passing through an HTTP handler — `task_consolidate_dust.rs:191/263/390` signs UTXOs and broadcasts a real tx; `task_check_peerpay.rs:83-103,205` signs BRC-103 auth; `task_backup.rs:71` auto-fires the on-chain backup (**the exact `a85985f` path**); cert publish/unpublish sign via their own builders. A handler-layer flag leaks all of these. Hand-enumerating ~50 private-key call sites is also error-prone (the original draft even named the wrong backup handler — `wallet_backup` is a file export that signs nothing; the money path is `wallet_backup_onchain`/`do_onchain_backup`).

**Gate at the single true chokepoint:** every private-key path funnels through `WalletDatabase::get_cached_mnemonic()` (`connection.rs:238`) → `get_master_private_key_from_db` (`helpers.rs:15`), `derive_private_key_bip32` (`recovery.rs:49`), all three Monitor tasks, all cert handlers, `reveal_mnemonic`, `wallet_export`, `do_onchain_backup`. Set an `identity_contaminated` flag on `WalletDatabase`; `get_cached_mnemonic()` returns an error (mirroring the existing locked-wallet error) when it's set. **Complete-by-construction** — no enumeration, covers background tasks automatically.

**Reentrancy fix (review medium):** the verify routine itself needs the secret (`get_master_public_key_from_db` → `get_cached_mnemonic`). Provide a `get_cached_mnemonic_unchecked()` (or a pure `verify_identity(mnemonic, stored_pubkey_hex)` fn that takes the plaintext) that **bypasses** the guard, and have `verify_unlocked_identity()` use it — so the check can re-evaluate/clear after a legitimate recovery even while the flag is set.

**Failure semantics = Option A, now strictly dominant.** Because the gate is at the chokepoint, refusing there is behaviorally identical to a locked wallet — no secret leaves the process (Option B's only advantage) **and** no brick (server stays up, `/wallet/status` reads the *flag*, not the secret, to render a "key mismatch — do not use" banner; recovery surface intact). `process::exit` is reserved as belt-and-suspenders after the flag is set, never the primary mechanism. **Never** hard-exit or refuse on an *inconclusive* condition (see §3).

## 3. Resolving identity + the never-refuse-on-inconclusive invariant

**Resolve via `UserRepository::get_default()`, not `current_user_id`.** At the startup insertion point (§4) `current_user_id`/`AppState` don't exist yet (bound at `main.rs:606`, AppState `~:660`); `get_default()` (userId ASC LIMIT 1, `user_repo.rs:105-111`) is exactly what `main.rs:576` uses to seed the active user. Drop the "ID 1" assumption — `userId` is AUTOINCREMENT, the default is the lowest surviving id, not literally 1.

```
let derived_hex = hex::encode(get_master_public_key_from_db_unchecked(db)?);   // compressed 33-byte, lowercase
match UserRepository::new(db.connection()).get_default()?.and_then(|u| u.identity_key) {
    Some(stored) if stored.eq_ignore_ascii_case(&derived_hex) => Match,          // healthy
    Some(stored)                                              => Mismatch,       // Mode A → set flag, refuse
    None                                                      => Inconclusive,   // NEVER refuse (see below)
}
// derivation error / locked wallet → Inconclusive, never refuse
```

**HARD INVARIANT (review high):** `None` / derivation-error / locked-wallet ⇒ **Inconclusive**, never Mismatch. A pre-V17 or externally-manipulated wallet has an **empty `users` table** (`UserRepository::create` is only ever called from the two create paths — no migration backfill exists; `CREATE TABLE IF NOT EXISTS users` inserts no row). On Inconclusive: **warn + continue**, and backfill the derived key as the stored identity **only if the wallet has zero tx/output history** (genuinely new); on a wallet *with* history, warn-and-continue but do **not** silently backfill (that would launder a contaminated key into the anchor). This invariant is why Option A is mandated over B — B would turn every inconclusive/transient state into a permanent brick.

## 4. Where it runs (REVISED ordering)

**Startup — run FIRST inside `if db.is_unlocked()` (`main.rs:368`), before the two backfills** (`store_dpapi_blob` `:373` and `ensure_master_address_exists` `:378`). Both derive from and *persist* the cached mnemonic; on a contaminated boot `store_dpapi_blob` would make the wrong secret sticky in Keychain (when `!has_dpapi`) and `ensure_master_address_exists` writes a contaminated address row — **before** any refusal. Running the check first and short-circuiting both backfills on Mismatch prevents persisting contaminated derived material. The still-owned `&db` (pre-`Arc<Mutex>` wrap) is in scope here; resolve identity via `get_default()` (§3).

**Other unlock transitions:**
- `POST /wallet/unlock` (PIN) — after the mnemonic is cached, at the handler tail.
- `wallet_recover` / `wallet_recover_external` / `wallet_import` — at the **handler tail, after the final `users.identity_key` is persisted** (review medium: `wallet_import` DELETEs the derived user row and re-inserts the backup's `users` rows at `handlers.rs:16972-16975` → the final identity comes from the backup blob, so the check must run *after* that, not inside the shared `create_wallet_from_existing_mnemonic` helper). **Never** hook the check into `cache_mnemonic()`/mid-helper — the cache precedes the identity INSERT and a premature FirstRun-backfill would create a duplicate `users` row (plain INSERT, no upsert).
- `wallet_restore` is a file swap requiring a server restart → covered by the startup check. All in-handler create/recover paths hard-refuse if a wallet already exists (409), so recovery always runs against a **fresh DB** — there is no "old identity" to false-positive against (reframes the old §6.4).

## 5. Companion: inverted env safeguard (closes Mode B) — REVISED

The original "detect prod install locations" approach is **flawed** (review): a portable/ZIP build extracts anywhere (Desktop, USB, temp) — matching neither dev-build paths nor enumerated prod locations — so `HODOS_DEV=1` on a portable prod binary bypasses it. Also self-referential: `GetAppInstallDir()`/`app_dir_name()` derive their path *from* `HODOS_DEV`, so under the very flip you're catching they resolve to the Dev dir.

**Invert the rule using the discriminator that already exists:** `HODOS_DEV=1` is only legitimate from a recognized **dev-build path**. In BOTH layers (`main.rs:180-183` Rust `enforce_dev_safeguard`, `AppPaths.h:141-162` C++ `EnforceDevSafeguard`) add the reciprocal:

```
if HODOS_DEV == 1 && !is_dev_build_path(exe_path) { /* prod binary with a stray dev flag */ }
```

This closes `%LOCALAPPDATA%\HodosBrowser`, the `.app` bundle, **and** portable-anywhere in one rule without knowing where prod lives. If any literal path compare is kept, compare against the hardcoded string `"HodosBrowser"`, never `GetAppDirName()` (circular).

**Split-brain fix (review medium):** the C++ shell and Rust wallet read `HODOS_DEV` independently and the shell spawns the wallet child. Make the decision **identical and symmetric** in both layers, and have the shell **scrub `HODOS_DEV` from the environment before spawning the wallet child** so the child can't inherit a stray flag. Owner decision Q2: on a prod-binary-with-`HODOS_DEV`, **refuse to start** vs **force-prod-namespace + loud warning** — pick one and apply it the same in both layers.

## 6. Tests

- Unit: derived==stored → Match; derived!=stored → Mismatch (flag set, `get_cached_mnemonic` errors); stored None + zero history → FirstRun-backfill; stored None + has history → Inconclusive-warn (no backfill, no refuse); derivation error/locked → Inconclusive.
- **Regression (the real bug):** DB with `users.identity_key = P`, cache a mnemonic deriving Q≠P → assert flag set AND that a Monitor-path sign (e.g. `do_onchain_backup` / `task_consolidate_dust`) refuses, not just HTTP handlers.
- **Encoding:** correct key, different case → Match; assert stored length == 66 hex chars (catches a future compressed→uncompressed/DER format change that would silently brick).
- Reentrancy: after flag set, `verify_unlocked_identity` can still read the secret via the unchecked path and clear the flag on a legitimate recovery.
- Restore/import: restore a backup → post-restore check Matches (derived-from-restored-mnemonic == restored identity_key); no duplicate `users` row.
- Mode B: prod-location (and portable-path) binary + `HODOS_DEV=1` → safeguard refuses/forces-prod in BOTH layers; wallet child spawned with `HODOS_DEV` scrubbed.

## 7. Open questions for the owner (before code)

- **Q1.** Confirm the chokepoint gate (`get_cached_mnemonic` returns error when contaminated) + the never-refuse-on-inconclusive invariant — i.e. Option A relocated to the chokepoint. (Review: this strictly dominates the hard-exit Option B.)
- **Q2.** Mode-B on a prod/portable binary with a stray `HODOS_DEV`: **refuse to start**, or **force-prod-namespace + loud warning**? (Applied symmetrically in both layers, wallet child spawned with `HODOS_DEV` scrubbed.)
- **Q3.** FirstRun backfill policy: backfill the derived identity **only** when the wallet has zero tx/output history; warn-and-continue (no backfill) otherwise — acceptable?
- **Q4.** Out of scope for now, note for later: bind the DPAPI/Keychain secret to *this* DB via per-DB entropy/tag (`dpapi.rs`) so a swapped secret fails to decrypt rather than decrypting to the wrong key. Deeper change; defer.
