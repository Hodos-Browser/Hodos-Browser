# Phase 0 — stray writes in the install root · PHASE CONTRACT

**Workstream:** WS1b(a) · **Ticket:** `../TICKET_stray_log_in_install_root.md` · **Status:** ⬜ NOT STARTED
**Opened:** 2026-08-18 (retrofitted to the harness) · **Platforms:** Windows (3 of 52 writes are also macOS)
**Standard:** `../HARNESS.md`.

---

## 1. Goal

The installed application directory holds no file the browser writes at runtime, and no wallet HTTP
response body — **including the BIP39 recovery phrase** — is ever written to disk outside the
level-gated log directory.

## 2. Done means

- [ ] `{app}` contains **zero** runtime-written files after a full session — measured, not inspected once
- [ ] All 52 relative-path writes gone: `debug_output.log` (44) + `startup_log.txt` (8)
- [ ] No wallet HTTP **response body** reaches any sink; anything kept is routed through `Logger`
- [ ] `{app}\debug.log` stops being recreated across **≥10** launches (ordering-dependent)
- [ ] T0 gate `G1` baseline driven **52 → 0**, and `G5` (response bodies reaching a sink) **15 → 0**
- [ ] A1 decided and implemented; A2's abort recorded in update state and raised above `LOG_WARNING`

⚠️ **Correction carried from verification (2026-08-18).** The ticket's original headline — *"can
silently abort auto-update"* — **did not survive verification**: `MaybeApplyStagedUpdate`
(`cef_browser_shell.cpp:4787`) runs before `CefInitialize` (`:5277`) and only past a
`selfCount == 1` gate (`:4155-4165`) plus a wallet-dead gate (`:4172`), so no Hodos process can hold
the log open during `BuildManifestForTree`. Struck in the ticket, not deleted. **What replaces it is
worse** — see §4 `P0-S1`.

## 3. Invariants preserved

| ID | Invariant | Why this phase could break it |
|---|---|---|
| `R-UPDATE` | Update still applies and still rolls back | Touches `UpdateFs.cpp` (A1) and the apply abort path (A2) |
| `R-INTEXT` | Internal never prompts, external always gates | Not touched — but 19 of the writes are inside `WalletService`'s money-path methods, so the diff sits on that code |
| `R-GOLD` | Gold pill fires | Not touched. Recorded so the boundary run is not skipped |

## 4. Evidence table

| ID | 🟢 GREEN | 🔴 RED — must be *seen* to fail | 🎯 SUBJECT | Tier | Result |
|---|---|---|---|---|---|
| `P0-S1` | ⛔ **Reproduction first.** Create a wallet; the phrase does **not** appear anywhere on disk outside the log dir | *Before* the fix, the same run **must find it** in `{app}\debug_output.log`. If it does not, this row's premise is wrong and §1 changes | Release-shaped build, browser process. `WalletService_mac.cpp` has zero `ofstream` — Windows only | T2 | ⬜ |
| `P0-A1` | Full session → **no** new file in `{app}` | Restore one write → the file reappears | `{app}` = `%LOCALAPPDATA%\HodosBrowser\`, **not** the build dir | T3 | ⬜ |
| `P0-A2` | The same events **do** appear under `%APPDATA%\…\logs\` | Suppress the `Logger` call → they vanish there too | Both halves. **A silent no-op looks identical to a fix** | T3 | ⬜ |
| `P0-A3` | `preflight.ps1` gate `G1` passes at baseline **0** (from 52) | Add one `ofstream("foo.log")` → gate **exits non-zero** | `preflight.ps1 -NegativeControl` proves it, not a code read. Already observed 2026-08-18: `53 > 52` | T0 | ⬜ |
| `P0-A8` | `preflight.ps1` gate `G5` passes at baseline **0** (from 15) — no wallet response body reaches **any** sink | Route one response body through `Logger` instead of `ofstream` → `G5` still catches it | `G1` alone is insufficient: it only sees `ofstream`. `G5` guards the *shape* that puts the phrase on disk | T0 | ⬜ |
| `P0-A4` | `{app}\debug.log` absent across ≥10 launches | Restore `TabManager.cpp:33`'s `LOG(INFO)` → it returns | ⚠️ Ordering-dependent — the message appears 3× in `{app}` and 2× in `logs\`. **One clean launch is not evidence** | T3 | ⬜ |
| `P0-A5` | `[InstallDelete]` clears all three files on upgrade | Plant all three, run an upgrade install → all gone | Already implemented (`hodos-browser.iss:93-97`). **Verify, do not rebuild** | T2 | ⬜ |
| `P0-A6` | A1: a file held open for write no longer aborts the backup | Hold `{app}\x.log` open, run the manifest walk → **pre-fix** it must abort | `BuildManifestForTree` return value, not the log line | T1 | ⬜ |
| `P0-A7` | A2: an abort records reason in update state and is visible | Force an abort with the recording removed → nothing observable | `update-state.json` on disk — reuse `lastFailureBuild`/`lastFailureReason` | T1/T2 | ⬜ |

**Pairing:** `P0-A1` and `P0-A2` are two-sided — "gone from `{app}`" and "still present in `logs\`".
Either alone is satisfied by deleting logging outright, which is the failure mode to avoid.

## 5. Blast radius

- `WalletService.cpp` (19 writes) — inside `makeHttpRequest`, `readResponse`, `createTransaction`,
  `signTransaction`, `broadcastTransaction`, `getBalance`, `getTransactionHistory`, `sendTransaction`.
  These are the **only** diagnostics in some of those functions: re-route, do not simply delete.
- `simple_app.cpp` (17 + 8) — includes `CreateBRC100AuthOverlay*` and `InjectHodosBrowserAPI`.
- `AddressHandler.cpp` (2), `my_overlay_render_handler.cpp/.mm` (3 + 3 — **macOS ships these**).
- `UpdateFs.cpp` — A1 changes what the backup walk tolerates.
- `TabManager.cpp:33` — A3.
- ⚠️ `cef_browser_shell.cpp:4632` falls back to the **relative** literal `"debug.log"` when
  `AppPaths::GetLogDir()` is empty — a latent second source of the same file.

## 6. Out of scope

- The other **43** raw `LOG()` calls across 5 files. Only `TabManager`'s fires pre-init *today*; that
  is a property of call ordering, not a guarantee. Noted as a hazard, not swept.
- `Logger`'s level gate and rotation — **Phase 2**.
- Extending the F8 gate to cover `ofstream` sinks of secret-shaped data — tempting, and deliberately
  left: `G1` removes the sinks entirely, so the F8 widening belongs with Phase 2's logging work.

## 7. Rollback

`git revert` the phase's commits. The changes are deletions plus one exclusion list and two
`UpdateState` fields; nothing is schema-breaking and nothing else depends on them.

---

## Sign-off

- [ ] Every evidence row GREEN **and** its RED observed
- [ ] `scripts/preflight.ps1` run — recorded below
- [ ] `scripts/preflight.ps1 -NegativeControl` — every T0 gate seen to fail
- [ ] `../REGRESSION_SET.md` run in full at the 0 → 0.5 boundary
- [ ] Adversarial review — four questions answered in writing
- [ ] `G1` 52 → 0 and `G5` 15 → 0 in `../HARNESS.md` §9
- [ ] Commits cite row IDs

| Item | Result | Date | By |
|---|---|---|---|
| preflight | | | |
| preflight -NegativeControl | | | |
| regression set (0 → 0.5) | | | |
| adversarial review | | | |
