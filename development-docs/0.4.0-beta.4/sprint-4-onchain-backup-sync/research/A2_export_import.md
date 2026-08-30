# A2 — Export/Import Code: What Exists, What's Disabled, What It Would Take to Revive

**Date:** 2026-08-22. **Repo:** `C:/Users/archb/Hodos-Browser` at `6b4a4be` (master).
**Question asked:** Matt believes the export/import code is mostly DISABLED. Is it?

## Answer in three sentences

The **backend is fully live**: `POST /wallet/export` and `POST /wallet/import` are registered
routes with working handlers, hardened with an origin guard in June 2026, and their shared
machinery (`collect_payload`, `import_to_db_with_ids`) is exercised every day by the live on-chain
backup/recovery path. What is disabled is **only the UI**: the Export Backup form and the "Import
from Backup File" button were removed from the JSX in commit `f219da7` (2026-04-02) and replaced
with `hidden for now, backend still supports it` comments — the handler functions and all their
state hooks are still compiled into the frontend, just unreachable. **No BRC-38/39 code exists
anywhere in the repo**; the file format is a custom `.hodos-wallet` JSON envelope
(`format: "hodos-wallet-backup"`, version 1, PBKDF2 + AES-256-GCM).

---

## 1. Every export/import surface found

| # | Surface | Where | Status |
|---|---|---|---|
| 1 | `POST /wallet/export` — encrypt full wallet (incl. mnemonic) into `.hodos-wallet` JSON | handler `wallet_export` `rust-wallet/src/handlers.rs:17211`; route `rust-wallet/src/main.rs:1271` | **LIVE** backend; **UI-hidden** frontend |
| 2 | `POST /wallet/import` — decrypt `.hodos-wallet`, create wallet, import all entities | handler `wallet_import` `handlers.rs:17297`; route `main.rs:1273-1275` | **LIVE** backend; **UI-hidden** frontend |
| 3 | Export UI — password form + blob download `hodos-wallet-backup-<date>.hodos-wallet` | `frontend/src/components/wallet/SettingsTab.tsx:145-189` (`handleExportBackup`) | **Dead-but-compiled**: JSX removed, function suppressed (see §2) |
| 4 | Import UI — file picker, format check, PIN flow, `/wallet/import` POST | `frontend/src/pages/WalletPanelPage.tsx:545-635` (`handleImportFileChange`, `handleStartImport`, `doImportBackup`), form JSX at `:1074-1180` | **Dead-but-compiled**: form JSX exists but `setShowImportForm(true)` is never called anywhere; the button that called it was deleted (see §2) |
| 5 | `POST /wallet/backup` with `format: "file"` (DB file copy) or `"json"` (`export_to_json`, non-sensitive debug export) | handler `wallet_backup` `handlers.rs:12971` (json branch calls `crate::backup::export_to_json` at `:13065`); route `main.rs:1260`; `export_to_json` at `rust-wallet/src/backup.rs:1860-1861` | **LIVE route, NO caller** — grep of frontend/, cef-native/src/, scripts/ finds no invocation (only a comment in `cef-native/src/core/HttpRequestInterceptor.cpp:1847` and binary build artifacts) |
| 6 | On-chain backup/restore (`/wallet/backup/onchain`, `/wallet/recover/onchain`, `/wallet/restore`, `/wallet/recover`) | routes `main.rs:1260-1267`; import at `handlers.rs:15072` | **LIVE end to end** — this is what ships today (`ONCHAIN_BACKUP_SYSTEM.md`); listed here because it shares the file-export machinery (§4) |
| 7 | External-wallet import: "Recover from Centbee" (BIP39 sweep, `/wallet/recover-external`) | handler `wallet_recover_external` `handlers.rs:16027`; UI live in `WalletPanelPage.tsx` | **LIVE** — the only shipping cross-wallet import; it sweeps funds, it does not import state |
| 8 | Mnemonic reveal (Settings) and mnemonic-based recovery | `SettingsTab.tsx`, `WalletPanelPage.tsx:466,497` | **LIVE** — seed-level export/import only, not state |
| 9 | BRC-38 / BRC-39 / `.brc39` / `exportBRC*` / `importBRC*` code | — | **DOES NOT EXIST** in this repo. Grep for `brc38|brc39|brc-38|brc-39` (case-insensitive) hits only two planning docs: `development-docs/0.4.0-beta.4/sprint-4-onchain-backup-sync/README.md` and `SPRINT_KICKOFF_PROMPT.md` |

VERIFIED (all rows): by reading the cited files/lines and grepping the repo, this session.

## 2. The disabling mechanism, quoted

**Export UI** — `SettingsTab.tsx`. Three mechanisms, all in the current file:

1. State reads discarded (line 25-31):
   ```ts
   // Export backup (UI not yet wired)
   const [, setShowExportForm] = useState(false);
   ...
   const [, setExportError] = useState<string | null>(null);
   ```
2. Function kept but voided (line 190):
   ```ts
   void handleExportBackup; // suppress TS6133 until export UI is wired
   ```
3. JSX replaced by a comment (line 379):
   ```tsx
   {/* Export Backup — hidden for now, backend still supports /wallet/export */}
   ```

**Import UI** — `WalletPanelPage.tsx`. One mechanism: the entry-point button was deleted and
replaced by a comment (line 1406):
```tsx
{/* Import from Backup File — hidden for now, backend still supports it */}
```
`showImportForm` (line 157) can only ever be set `false` (lines 254, 645, 1172) — grep for
`setShowImportForm(true)` returns nothing — so the intact import form at `:1074-1180` is
unreachable. VERIFIED.

**Not** feature-flagged, **not** commented-out code, **not** backend-gated. The backend routes
answer if called (an internal caller could hit them today; the June bridge test did exactly that,
§3).

**One real backend restriction (added later, security not disablement):** both handlers 403 any
request carrying a non-empty `X-Requesting-Domain` header (external/website origin), quoting
`handlers.rs:17218-17235`:
```rust
// Security (OQ-4 / bridge migration): export serializes the wallet (encrypted
// mnemonic + every row). It is a user/wallet-internal operation only — no
// external site, even an approved dApp, may trigger it ...
```

## 3. When and why (git history)

| Date | Commit | What happened |
|---|---|---|
| 2026-02-14 | `b1fe160` "phase-1 sub-phase 1 and 2 complete" | Export/import backend + import UI (incl. `setShowImportForm(true)` button) first land |
| 2026-03-09 | `e4e7533` "advanced wallet dashboard update" | SettingsTab created with the full Export Backup form |
| 2026-04-02 | **`f219da7`** "switching to minor response handler cleanup" | **The hide.** Diff removes the whole Export Backup JSX block from SettingsTab and the Import-from-Backup-File `<HodosButton>` from WalletPanelPage, adds both "hidden for now" comments. VERIFIED against `git show f219da7` |
| 2026-04-04 | `9209705` "Fix TS6133 unused variable errors in SettingsTab (export UI not yet wired)" | Adds the `void handleExportBackup` suppression — confirms the hide was deliberate and expected to be temporary |
| 2026-06-24/25 | `e5bb587`, `a95e01e` … `1922aa1` (wallet-UI bridge migration, commits 1-5) | `a95e01e` adds the `X-Requesting-Domain` guard to `wallet_export`/`wallet_import` — fixing a **SEV-1**: per `development-docs/0.4.0/archive/WALLET_UI_BRIDGE_MIGRATION.md` (line 60), "an approved dApp could `__hodos_walletCall('export','/wallet/export',{password},'POST')` and exfiltrate an encrypted backup it can decrypt." `1922aa1` adds 256KB chunked IPC delivery specifically because `/wallet/export` responses are multi-MB |
| 2026-06-25 07:55 | `026f5b4` "test(wallet): TEMP-wire Export Backup UI to exercise bridge Commit 4 (large-export)" | Temporarily un-hides the export UI. Commit body: "The /wallet/export logic (handleExportBackup) already existed in SettingsTab but was disabled … ⚠️ TEMPORARY — revert this commit before running the release pipeline; **the backup/export flow is not finalized**." |
| 2026-06-25 09:27 | `4baa272` | Reverts `026f5b4`. UI hidden again; unchanged since |

**Why it was hidden (2026-04-02):** the commit message ("switching to minor response handler
cleanup") gives **no reason**, and I found no doc dated around 2026-04-02 explaining it. The reason
is **not in the history** — the closest statements are after-the-fact: `026f5b4`'s "the
backup/export flow is not finalized" and the bridge doc's "the backup/export UI button is not
wired yet (**owner deferred**)" (`MACOS_CATCHUP_PLAYBOOK.md:278`). So: deliberately deferred by
the owner as unfinalized; the specific original motivation is undocumented. Plausible contributing
fact (INFERRED, do not treat as established): at hide time the bridge migration hadn't happened,
and two later-discovered problems — the dApp-exfiltration hole (OQ-4/SEV-1) and the multi-MB
response breaking the single-`ExecuteJavaScript` IPC path — mean the hidden UI would have been
unsafe/broken for part of the interim anyway. Both are now fixed in code (`a95e01e`, `1922aa1`).

**Outstanding from the bridge work:** `WALLET_UI_BRIDGE_MIGRATION.md` status line (2026-06-25)
lists as remaining "a **live large-export round-trip** (export → re-import → balances match) since
Commit 4's big-payload path is built+reviewed but **not yet live-tested**". No later record that
this test ran was found (searched "round-trip", "balances match", "large-export" across
development-docs). Whether the owner exercised it in the ~90 minutes the TEMP wire was in place is
not recorded.

## 4. Rot assessment and revival cost

**Almost nothing rotted, because the file path shares its guts with the live on-chain path:**

- `wallet_export` → `collect_payload` (`backup.rs:372`) — the same function the live on-chain
  backup calls via `compress_for_onchain` (`backup.rs:1011`, called from `handlers.rs:13306,
  14087, 15126`). Every schema change since the hide (`domain_permissions` /
  `cert_field_permissions` were already in by Feb `afeef52`; certificate publish-status
  preservation `36fe5db`; deterministic ORDER BY `30c6c89` — all post-hide commits touch this
  shared code) has therefore been carried into the file-export payload automatically. VERIFIED via
  `git log --since=2026-04-02 -- rust-wallet/src/backup.rs` + reading `BackupPayload`
  (`backup.rs:32-60`) and `import_entities` (covers all 20 entity lists incl. the two
  `#[serde(default)]` permission tables, `backup.rs:1564-1591`).
- `wallet_import` → `import_to_db_with_ids` — the same function the live on-chain recovery calls
  (`handlers.rs:15072`). Live-exercised.
- Difference between the two paths: on-chain applies the five strips (`compress_for_onchain`);
  file export does **not** — it is full-fidelity, and additionally carries the **mnemonic
  plaintext inside the encrypted payload** (`collect_payload(conn, &identity_key, &mnemonic)`,
  `handlers.rs:17266`), which the on-chain payload does not (passes `""`).
- The frontend handlers still match the backend contract (checked field-by-field: password ≥8,
  `format === 'hodos-wallet-backup'` check at `WalletPanelPage.tsx:558` matches
  `backup.rs:911/924`; import request shape `{pin, password, backup}` matches
  `WalletImportRequest`, `handlers.rs:17290-17294`).
- The June bridge test (`026f5b4`) proved the export UI still compiled and wired cleanly
  ("tsc+vite pass") with a 39-line diff.

**What still gates revival (not rot — process):**
1. Owner sign-off: flow explicitly "not finalized" / "owner deferred".
2. The unrecorded live large-export round-trip test (bridge §7 matrix) — must run before release.
3. Import only works into an **empty wallet**: `wallet_import` returns 409 "Wallet already exists"
   (`handlers.rs:17389`); it then bulk-deletes auto-created `users`/`output_baskets`/`addresses`
   and re-inserts (`handlers.rs:17411-17417`). No merge mode exists.
4. UX polish: the deleted JSX exists in history (`git show f219da7`/`026f5b4`) but was minimal.

**Scope to revive as-was: S.** Un-hide is literally `git revert 4baa272` plus restoring the
import button (6 lines, visible in `f219da7`'s diff), then the round-trip test. Reasoning: backend
live and security-hardened, payload maintained by the shared on-chain path, TEMP-wire commit is a
working template. The S assumes the existing custom format; it is **not** the scope for BRC-39
(next section).

## 5. Relation to work item 7 (HandCash `.brc39` → Hodos import)

**Head starts (real):**
- The **entire import pipeline after decryption exists and is live-tested**: validate → create
  wallet from mnemonic → ID-remapped entity insert (`import_to_db_with_ids`) → monitor start →
  balance cache seed. A `.brc39` importer only has to produce a `BackupPayload`-shaped (or
  directly DB-shaped) structure; everything downstream is the same code the on-chain recovery
  uses in production.
- The UI shell (file picker → parse → password → PIN flow → result screen) already exists,
  hidden, in `WalletPanelPage.tsx` — reusable for a `.brc39` file with a format sniff.
- The OQ-4 origin guard pattern is established; a new import endpoint must copy it (mirror
  `handlers.rs:17304-17318`).

**Landmines (each VERIFIED against the cited code):**
1. **Our container is not BRC-39.** `.hodos-wallet` = JSON `{format:"hodos-wallet-backup",
   version, salt, data}` with PBKDF2 (`crate::crypto::pin::derive_key_from_pin`) + AES-256-GCM
   (`backup.rs:878-917`). BRC-39 per the item-7 survey is binary magic `WDAT`, Argon2id + AES-GCM.
   Nothing here parses that; the container layer is all-new work.
2. **Our payload is not BRC-38.** 20 entity lists (plus the single `wallet` record) incl. `addresses`,
   `parent_transactions`, `block_headers`, permission tables (`backup.rs:32-60`) — exactly the
   drift work item 0 exists to measure. A2 confirms item 0's premise from the code side.
3. **The mnemonic-matches-identity-key integrity check** (`handlers.rs:17352-17380`): identity
   key is computed as the secp256k1 pubkey of the **BIP32 master node private key itself** (no
   derivation path — `XPrv::new(&seed)` then `master_key.private_key()`), and import hard-fails
   on mismatch. A foreign wallet's document won't carry a Hodos-style `identity_key`, and BRC-38
   §1 excludes root-key material entirely — this check must be bypassed or rethought for foreign
   imports, and how a `.brc39` import gets its seed at all (HandCash encrypts with a
   root-key-derived secret) is an open design question for item 7.
4. **Fresh-wallet-only, destructive import** (§4 point 3): fine for "restore onto a new device",
   wrong shape for "merge a foreign wallet into mine". Item 7's proof can live with
   fresh-wallet-only; a product feature can't.
5. **Multi-MB responses through the UI must use the chunked bridge path** — and that path's live
   round-trip test is still unrecorded (§3). Do item 7's import testing through it and you retire
   that debt at the same time.

**Net:** item 7's Hodos-side import is **M**, not L — decrypt/parse (new) + BRC-38→our-schema
mapping (item 0's table makes this mechanical) riding on a live-tested insert pipeline and an
existing UI shell. Without A2's findings you might have budgeted for building an import pipeline;
it exists.

## 6. Checked / NOT checked

**Checked (read in this session, with line refs):**
- `rust-wallet/src/handlers.rs`: 12971-13090 (`wallet_backup` file/json), 14862-14866, 14971,
  15055-15126, 15993-16060 (Centbee), 17210-17470 (`wallet_export`, `wallet_import` complete)
- `rust-wallet/src/main.rs`: 209-224 (internal-only path comments), 1260-1275 (routes)
- `rust-wallet/src/backup.rs`: 14-80 (structs), 367-372, 870-930 (encrypt/decrypt), 958-964,
  1003-1011 (`compress_for_onchain`), 1336-1379, 1564-1591 (permission-table import),
  1860-1861, 2028-2278 (test names only)
- `frontend/src/components/wallet/SettingsTab.tsx`: 1-60, 130-200, 360-430
- `frontend/src/pages/WalletPanelPage.tsx`: 155-160, 244-300 (via diff), 540-650, 1060-1080,
  1352-1470; grep for every `setShowImportForm` site
- `cef-native/src/core/HttpRequestInterceptor.cpp`: 1820-1880 (chunking)
- `development-docs/0.4.0/archive/WALLET_UI_BRIDGE_MIGRATION.md`: status line + lines 58-158
- `development-docs/MACOS_CATCHUP_PLAYBOOK.md`: line 278
- `development-docs/0.4.0-beta.4/sprint-4-onchain-backup-sync/README.md`: lines 1-100 + work item 7 section
- Git: `git show` full diffs/messages of `f219da7`, `026f5b4`, `4baa272`, `9209705`; stats of
  `e4e7533`; dates of `b1fe160`, `a95e01e`, `afeef52`, `0dca522`; `git log -S` on the disable
  strings; `git log --since=2026-04-02 -- backup.rs`; commit list 2026-06-24..27
- Repo-wide greps: `brc38|brc39` variants, `/wallet/export|import|backup` callers (frontend,
  cef-native/src, scripts, tests), `exportBRC|importBRC|portable|restore_from_file` (no hits
  beyond the above)

**Checked and excluded as out of scope:** `cef-native/src/core/ProfileImporter.cpp` (surfaced by
the `portable`/`import` greps) imports **browser profile data** (history etc.) from other browsers —
it never touches wallet state and is not an export/import surface for this report.

**NOT checked:**
- Whether the export/import handlers actually execute correctly today (no runtime test performed;
  "live" means routed + compiled + shared-code-exercised, and export was last observed working
  2026-06-25 via `026f5b4` — its runtime result was not recorded)
- `crate::crypto::pin::derive_key_from_pin` internals (KDF iteration count not read)
- `wallet_restore` / `wallet_recover` / `wallet_recover_onchain` handler bodies in full (only
  their import call sites)
- The 5 archived branches / `rust-wallet/archive` contents beyond a directory listing; sessions/
- macOS-side bridge code; `cef-native/build/**` binaries (matches noted, not analyzed)
- HandCash/toolbox sources (item 7 survey claims taken as the README states them, not re-verified
  here — out of A2 scope)
