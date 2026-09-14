# Mac relay — beta.3 Phase 8, first ticket (the 1-satoshi destruction guard)

**Commits:** `383bf4f` (code + tests) · `f15c147` (contract) · `b885875` (`R-DUST`) · `df6d2a6` (D-5 ticket)
**Base:** `b651aa8` (the end of the 7d relay). **Branch:** `0.4.0`.
**Written 2026-09-08.** Predecessor: `MAC_RELAY_P7D_ROUND.md`.

⭐ **The short version: this is Rust-only. There is no macOS port.** No C++, no React, no overlay, no
NSWindow, no platform conditional, no schema change. Five files, all under `rust-wallet/src/`.
`git pull`, `cargo build --release`, `cargo test`. **Your entire job is `M4`.**

⚠️ **Phase 8 is NOT finished.** It is a three-ticket bundle and this closes one. See `M6`.

---

## M1 — What changed

A 1-satoshi output is a token carrier (1Sat Ordinals, OpNS), not spendable value. Spending one into a
larger output **permanently destroys the asset** — BRC-147: *"a general 'pay' or auto-pay grant MUST
NOT authorize spending them."* Four code paths treated such an output as ordinary dust; one of them
runs **automatically every 24 hours**.

One named floor — `TOKEN_RESERVED_SATS` / `is_token_reserved_value` — applied at four sites:

| # | Site | File |
|---|---|---|
| 1 | Daily dust consolidator (the only *automatic* destroyer). Two duplicated candidate filters collapsed into one `is_consolidation_candidate` | `monitor/task_consolidate_dust.rs` |
| 2 | External-wallet sweep + scan. `ExternalScanResult` gains `token_reserved`; `total_balance` now sums the sweepable half only | `recovery.rs` |
| 3 | Coin selection — one filter ahead of the sort covers both the primary and lazy-consolidation passes | `handlers.rs :: select_utxos_greedy` |
| 4 | **`send_max`**, which bypasses the selector entirely. Owner decision: *"1 sats are special tokens. Do not send them with max, only normal UTXOs."* | `handlers.rs :: create_action` |

19 new tests, each with an in-file negative control.

## M2 — ⛔ Plan claims that were false. Do not re-derive them on Mac.

The ticket's line numbers were **all correct** (eleven citations, five files — a first this sprint).
The corrections are about meaning:

1. 🚨 **The severity question is answered: YES, and via a path the ticket did not name.** Not recovery
   — `monitor/task_sync_pending` ingests an incoming 1-sat payment **automatically within 30 s**
   (plus a full sweep on every startup), writing `spendable=1`, `basket_id NULL`, no value filter.
   Now asserted against a seeded DB with the real ingest function, not read from code.
2. ⛔ **"The recovery sweep" is the EXTERNAL-wallet import, not restore-from-seed.**
   `scan_external_wallet` / `build_sweep_transactions` have exactly one caller,
   `handlers.rs :: wallet_recover_external`. `recover_wallet_from_mnemonic` builds **no sweep at all**.
   The ticket's *"fires exactly when a user restores from seed"* is wrong about the trigger.
3. 🚨 **A fourth path the ticket does not name:** `create_action`'s `send_max` does
   `selected_utxos = all_utxos.clone()` and never touches the selector. The ticket's proposed fix
   would have shipped a guard with a one-click hole.
4. ⛔ **`task_consolidate_dust :: is_p2pkh_script` has no teeth** — see `M5`.
5. ⚠️ **The floor could not live where the contract put it.** `handlers` is declared only by the
   **binary** crate; `recovery.rs` compiles into **both** crates, so `crate::handlers::…` does not
   resolve in the library build. It lives in `utxo_fetcher.rs`; `handlers` re-exports it. Worth
   knowing before you add anything shared — this repo has two crate roots with different module sets.

## M3 — Nothing to port. Genuinely.

Checked deliberately, because "Rust-only" has been wrong before:

- No `#ifdef` / `#elif defined(__APPLE__)` added or needed — no platform API is touched.
- No new file, no new module, no `CMakeLists.txt` edit ⇒ **no `cef-native/tests/CMakeLists.txt`
  conflict** this round (the predictable one from the deconfliction protocol).
- `ExternalScanResult` gained a field; its only consumer (`wallet_recover_external`) was updated in
  the same commit. Nothing in `*_mac.mm` reads it.

## M4 — What we need from Mac

| ID | Ask | Why |
|---|---|---|
| `MAC-P8-1` | `cargo test` in `rust-wallet/` after rebasing — expect **458 lib + 526 bin + 16 integration binaries, 0 failed** | 🚨 `cargo build --release` does **not** compile `cfg(test)` code. It passed clean over 8 broken call sites in Phase 7c. A release build is not evidence here. |
| `MAC-P8-2` | `cargo build --release` | Confirms the `utxo_fetcher` ↔ `handlers` re-export resolves on your toolchain too |
| `MAC-P8-3` | Confirm `R-DUST`'s four tests are present and green: `dust_candidate_tests`, `token_reserved_sweep_tests`, `token_reserved_selection_tests`, `token_reserved_exposure_tests` | The invariant is new; a boundary run needs a baseline on both platforms |

⛔ **Nothing else.** Do not port, do not add a mac arm, do not touch these files.

## M5 — ⚠️ Carried finding, filed but NOT fixed: `TICKET_synced_outputs_store_a_fabricated_locking_script.md`

Measured against mainnet, and it will matter to you later:

`utxo_fetcher.rs` **never stores the locking script it saw on chain.** It generates one from the
address at all three fetch sites (neither indexer returns a per-UTXO script), always exactly 25 bytes
in exactly the P2PKH pattern. Measured:

| Kind | Real script | Stored |
|---|---|---|
| Transferred ordinal | bare P2PKH, 25 B | correct, by luck |
| Fresh inscription | P2PKH + ord envelope, **2,596,810 B** | ⛔ 25 B fabrication |
| OrdLock listing | contract, **860 B** | ⛔ 25 B fabrication |

⇒ `is_p2pkh_script()` **cannot return false** for a synced output, so its comment about guarding
against non-standard scripts is untrue for the dominant ingest path (comment corrected in place).

⭐ **Not urgent, and not a live money bug**: all three observed cases are 1-satoshi, which this phase
just excluded from every spend path. It is filed for **beta.4 sprint 1**, whose classifier would read
the fabrication and mark everything `Spendable` without ever erroring. Notes are already in
`0.4.0-beta.4/sprint-1-utxo-safety-guard/README.md` and `REGRESSION_ADDITIONS.md`.

## M6 — ⚠️ Phase 8 is a three-ticket bundle; one is closed

| Ticket | State |
|---|---|
| 🚨 `token_outputs_destroyed_by_dust_paths` | ✅ **closed this round** |
| `placeholder_resolution_failure_broadcasts_anyway` | ⬜ open |
| `bridge_single_slot_callbacks_race` (remainder) | 🚧 **in progress on Windows as Phase 8c** — see M7 |

⛔ Per `SPRINT_PLAN.md` §4.1 these are a **holding pattern, not a queue anyone pulls from** — a ticket
is not work until the owner assigns it. Do not start either one.

## M7 — 🍎 Phase 8c (bridge request ids) touches SHARED C++ and one Mac-lane file

Unlike 8a/8b this is **not Rust-only**. Owed to Mac as a verification, not a port
(`phase-8c-bridge-request-ids/PHASE_CONTRACT.md` `O6`):

| What changed | Where | Mac's job |
|---|---|---|
| Per-request-id promise map (`WalletBridgeV8Handler`, `s_pendingBridgeCalls`, `ResolveBridgeCall` / `RejectBridgeCall`) + 8 migrated methods | `simple_render_process_handler.cpp` (shared) | Build, then from CDP on the header page assert `hodosBrowser.bridge.getStatus.toString()` contains `[native code]` and run 3 concurrent `wallet.getBalance()` — expect 3 correct answers, not 2-of-3 |
| Request-id echo in 8 browser handlers; the `address_generate` handler's byte-identical `#ifdef _WIN32` / `#else` copies **collapsed to one** | `simple_handler.cpp` (shared) | Confirm `address.generate()` still resolves an address on Mac — the `#else` arm you were compiled against is gone, by design |
| **O2 + O5 (2026-09-14):** `send_transaction` runs its wallet call **off the UI thread** (the `get_balance` shape, captureless lambda, id and payload as parameters); the full-transaction-JSON debug line is gone; a rig-only `HODOS_BRIDGE_REPLY_DELAY_MS` seam (read once in the browser process) holds the send reply N ms. The render-side bridge deadline moved from a file-local `30000` to `kBridgeCallTimeoutMs = 45000` in `WalletService.h`, `static_assert`ed above `kWalletBroadcastTimeoutMs + 5000` — they were **equal** before | `simple_handler.cpp` (shared, `send_transaction` arm only) · `simple_render_process_handler.cpp` (shared: one `#include`, one deleted constant) · `WalletService.h` (shared header — `WalletService_mac.cpp` includes it; nothing in the `.cpp` changes) | Build only. If you want a free check: stop the dev wallet, call `hodosBrowser.wallet.sendTransaction({recipient:'x',amount:1})` from CDP with a `/json/list` probe running — the probe must stay fast (Windows RED on the old binary: **2,061 ms**, one sample; GREEN: see `P8c-A9`) |
| ⚠️ **Mac-lane file:** `createWallet`, `loadWallet`, `getAllAddresses`, `getCurrentAddress`, `createTransaction`, `signTransaction`, `broadcastTransaction`, `getTransactionHistory` **deleted** (their only callers were IPC handlers with no JS sender) | `WalletService_mac.cpp` + the shared `WalletService.h` | Build only. Nothing to port; flagged so you do not edit those lines concurrently |

Batch history: stage 1 `getStatus` · stage 2 `sendTransaction` · batch 1 `getBalance`, `get/setBackupModalState` ·
batch 2 (2026-09-12) `address.generate`, `getInfo`, `markBackedUp` + the deletions above · O9 (same day)
`address_generate` off the UI thread + rejecting. 34 legacy slots (cookies, bookmarks) remain; each further
batch will add to this table. ⚠️ Since O8 (below) the surviving bridge natives are **four**: `getStatus`,
`sendTransaction`, `getBalance`, `generateAddress`.

## M8 — 🍎 O8: the backup-overlay chain is deleted on Windows; **its macOS half is YOURS**

Owner call 2026-09-12: the standalone "Wallet Backup Required" modal (`/backup`) was the pre-wallet-overlay
first-run flow. Its only opener sat in a commented-out block in `App.tsx`, and two of the Rust routes it
called (`/wallet/info`, `/wallet/markBackedUp`) do not exist. `WalletPanelPage`'s create flow already shows
the recovery phrase under prevent-close and walks into the PIN step, so nothing is lost.

**Split on purpose** so neither side breaks a build it cannot compile:

| Side | What was removed | State |
|---|---|---|
| **Windows + shared** (done, this round) | `BackupOverlayRoot.tsx` + route; `wallet.getInfo/markBackedUp/get+setBackupModalState` (bridge, types, 4 render arms, 4 browser handlers, the modal-state helpers); `overlay_show_backup`; every `role_ == "backup"` arm in `simple_handler.cpp` **including the `#elif __APPLE__` ones** (`g_backup_overlay_window` externs and assignments); `g_backup_overlay_hwnd`, `BackupOverlayWndProc`, its class registration, the `simple_app.cpp` creator, `BrowserWindow::backup_overlay_hwnd` | ✅ built + smoke-tested on Windows |
| **macOS** (yours) | `CreateBackupOverlayWithSeparateProcess()` in `cef_browser_shell_mac.mm` (decl ~619, body ~3435) and its `g_backup_overlay_window` global + the frame-sync / shutdown blocks that touch it (~2310, ~2368, ~5259); the six `SimpleHandler::GetBackupBrowser()` calls (~1618–1691, ~2370, ~5220); `"backup"` in the role list ~5200 | ⬜ owed |
| **Shared cleanup, after yours lands** | `SimpleHandler::GetBackupBrowser()` + `backup_browser_` (`simple_handler.h/.cpp`), `BrowserWindow::backup_browser` + `backup_overlay_window` and the two role-slot lines in `BrowserWindow.cpp` — kept **only** so your build stays green meanwhile | ⬜ either side, once M8-mac is in |

**Batch 3 (cookies, same day) also touched shared C++:** `src/core/CookieManager.cpp` + `include/core/CookieManager.h`
(every `Handle*` gained a `requestId`; the cookie visitor now answers the empty-jar case from its destructor),
`simple_handler.cpp` (15 cookie / cache / cookie-blocking handlers), `simple_render_process_handler.cpp`
(`Payload::Strs`, 15 natives, 15 arms). Nothing platform-split. Rebuild after your next rebase.

**Batch 4 (bookmarks, 2026-09-13) touched shared C++:** `simple_handler.cpp` (5 bookmark handlers migrated,
9 deleted), `simple_render_process_handler.cpp` (5 natives, 5 arms, 9 arms deleted, and `ResolveBridgeCall`
now converts with `jsonToV8(…, deep = true)`), `src/core/IdentityHandler.cpp` + `include/core/IdentityHandler.h`
(`jsonToV8` gained a `deep` flag; `identity.get` keeps the old default). Nothing platform-split. Rebuild after
your next rebase.

**Batch 5 (adblock + privacy shield, 2026-09-13) touched shared C++:** `simple_handler.cpp` (8 handlers echo the
request id: the six `adblock_*`, `cookie_check_site_allowed`, `fingerprint_get_site_enabled`),
`simple_render_process_handler.cpp` (8 natives, 8 arms). Nothing platform-split. Rebuild after your next rebase.

**Batch 6 (paid cache, 2026-09-13, the last per-call pair) touched shared C++:** `simple_handler.cpp`
(`paid_cache_get_size` / `paid_cache_clear` echo the id), `simple_render_process_handler.cpp` (2 natives, 2 arms).
**34 natives total; stage 3 is complete.** Nothing platform-split. Rebuild after your next rebase — and once
you have, the one 8c row that is yours is a single concurrency check on macOS: 3 concurrent
`hodosBrowser.wallet.getBalance()` from CDP on the header page, expecting 3 correct answers (M7).

⚠️ Per the new root-doc rule: this round touched `simple_handler.cpp`, `simple_render_process_handler.cpp`,
`simple_app.cpp`, `cef_browser_shell.cpp`, `BrowserWindow.h`, `simple_handler.h` — **rebuild after your next
rebase** before anything else. The `#elif __APPLE__` arms I removed only *referenced* your global; your `.mm`
still defines it, so the macOS build should stay green until you remove your half.

## M7 — Instrument discipline (unchanged from 7d; all still cost real time)

1. ⛔ **`cargo build --release` is not `cargo test`.** Release builds skip `cfg(test)` entirely.
2. 🚨 **Vite Fast Refresh preserves React state across an edit** — hard-reload before any React
   measurement. *(No React this round, but it stands.)*
3. ⛔ **A zero can be a suppressed log, not an absence.** Run wallet-log assertions with
   `RUST_LOG=hodos_wallet=debug`.
4. ⛔ **Assert the count moves, not just that a thing is absent.** `R-DUST`'s RED is "the candidate
   set grows to 21", never "the ordinal isn't there" — an absence proves nothing if the fixture never
   reached the filter.
5. ⛔ Exit **143** is SIGTERM (usually your own timeout), not a failure.

## M8 — Owed, and honest about it

Unchanged from the 7d boundary except where noted:

- **Phase 8 adversarial review** — ⬜ owed. The same session wrote the code *and* its tests;
  `HARNESS.md` §6 wants a pass by someone that did not.
- **beta.3 regression set at this boundary** — ⬜ owed.
- **No live-wallet run for Phase 8, by design.** The floor is proven at the predicate/selection layer.
  Also worth knowing: the consolidator needs ~4,550 sats of accumulated dust before it fires at all,
  so a live trigger is slow to arrange and adds no signal.
- **DPI matrix cells #4/#6/#9** — still not run on either platform.
- **Stubbed `R-INTEXT` REDs**, **`R-GOLD` / `R-COUNT`** (no real payment), **`R-PERIM` T2 e2e** —
  carried.
- **Mac's `R4` run from the 7d round** — still owed.
- **7c's adversarial review** and `TICKET_wallet_quiet_detector_blind_to_long_polls.md` — still open.
