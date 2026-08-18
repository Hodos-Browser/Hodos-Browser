# beta.3 standing regression set

Run **in full at every phase boundary**, not only in the phase that owns the code. This is what
"not breaking anything else" means concretely.

> Each check carries its own RED. A check that has never been seen to fail is not a check — see
> `HARNESS.md` §2. Where a RED is destructive, use a scratch profile, never the production one.

---

## R-INTEXT — internal never prompts, external always gates ⭐

**The load-bearing one this sprint.** Phases 0.5 and 5 both touch this boundary, and Phase 5 rewrites
the predicate every request passes through.

**The mechanism, so the check tests the right thing:** the discriminator is **not** the target — both
the wallet UI and a dApp address `127.0.0.1`. It is the **caller's frame origin**, re-derived in the
browser process at `simple_handler.cpp :: OnProcessMessageReceived` (`wallet_call` arm) from
`frame->GetURL()`, which the renderer cannot forge. Internal origin ⇒ **no** `X-Requesting-Domain` ⇒
`domain_trust_mw` (`rust-wallet/src/main.rs:65-75`) passes through ungated. External ⇒ header present
⇒ permission engine.

| | |
|---|---|
| **GREEN a** | A send initiated by the user in the wallet UI completes with **no modal** |
| **GREEN b** | The same operation from an external page, over cap, produces a **202 + modal** |
| **RED** | Each half is the other's control. Force internal-as-external (stub the origin derivation to always emit the header) → (a) must start prompting. Force external-as-internal (suppress the header) → (b) must go silent. **Both must be observed.** |
| **SUBJECT** | Rust log: **absence** of `X-Requesting-Domain` for (a), **presence with the exact page host** for (b). Reading the C++ side alone proves nothing — the assertion is what Rust received. |
| **Tier** | T2 |

⚠️ `IsInternalOrigin("")` returns **`true`** (`HttpRequestInterceptor.cpp:1016`). A frame URL with no
`://` therefore collapses to internal. Any change near origin derivation must re-check this, and it
is the one path by which external can silently become internal.

## R-GOLD — the gold pill payment indicator

| | |
|---|---|
| **GREEN** | An auto-approved payment shows the **gold pill** on the originating tab |
| **RED** | Stub `OnWalletCallSuccess`'s emit → no pill. Confirm both createAction silent-approve **and** the BRC-121 paid-retry (`firePaymentSuccessIpc()`) paths |
| **SUBJECT** | Correct **tab**: `Tab::id` ≠ `CefBrowser::GetIdentifier()` — translate via `TabManager::GetTabIdForBrowserIdentifier`. A pill on the wrong tab is a failure |
| **Tier** | T2/T3 |

It is a **gold pill**, never a "green dot". It is the user's primary visual safeguard against silent
payment abuse and must survive every refactor.

## R-CLOSE — overlay close guards

| | |
|---|---|
| **GREEN a** | Native file dialog open → no overlay closes (`g_file_dialog_active`) |
| **GREEN b** | Wallet overlay in an unsafe state (mnemonic shown, PIN entry) survives focus loss (`g_wallet_overlay_prevent_close`) |
| **RED** | Clear each flag → the overlay closes. Observe per flag, not once for both |
| **SUBJECT** | Both close paths: the overlay's own `WM_ACTIVATE` **and** the main WndProc's `WM_ACTIVATEAPP` — and, for dropdown overlays, the `WH_MOUSE_LL` hook, which is a **third** path |
| **Tier** | T3 |

⚠️ Known gap feeding Phase 1: **no** `*MouseHookProc` consults `g_file_dialog_active`. The guard is
honoured on the WndProc paths only.

## R-PERIM — the four privacy-perimeter gates

| | |
|---|---|
| **GREEN** | identity-key reveal · key-linkage reveal · sensitive cert fields · over-cap spend each behave per `matrix_c.rs` |
| **RED** | Per gate, flip its precondition and observe the opposite outcome. Sensitive cert fields must prompt **unconditionally** — if that one ever goes silent, it is a defect, not a setting |
| **SUBJECT** | The Rust decision (`PermissionDecision` kind + reason), not the UI's appearance. A modal that renders is not proof the engine decided to prompt |
| **Tier** | T1 (engine) + T2 (end-to-end) |

## R-COUNT — per-session counters

| | |
|---|---|
| **GREEN** | Per-session spend counters reset on tab close — **by design** |
| **RED** | Spend to just under the session cap, close the tab, reopen: the counter must be reset. If it persists, `POST /wallet/session/close` did not fire |
| **SUBJECT** | `PermissionService.session_counters` (`permission_service/state.rs`), not a UI total |
| **Tier** | T2 |

## R-UPDATE — the update path still applies

| | |
|---|---|
| **GREEN** | A staged update applies N−1 → N and the browser relaunches healthy |
| **RED** | Corrupt the staged installer after signing → the apply must **refuse and roll back**, not proceed |
| **SUBJECT** | The **real** N−1 → N transition, not a synthetic pair. `scripts/test-apply-forward.ps1` / `test-apply-rollback.ps1` drive our helper — they do **not** cover Sparkle or WinSparkle |
| **Tier** | T2 |

Auto-update must never force a reinstall and must never brick an install.

---

## Boundary run record

| Phase boundary | Date | R-INTEXT | R-GOLD | R-CLOSE | R-PERIM | R-COUNT | R-UPDATE |
|---|---|---|---|---|---|---|---|
| 0 → 0.5 | 2026-08-18 | ⬜ deferred to P0.5 (its own subject) | ⬜ needs live app | ⬜ needs live app | ⬜ needs live app | 🟡 T1 covered (manifest↔copy round-trip + RED); real N−1→N apply owed at RC | |
| 0.5 → 1 | | | | | | | |
| 1 → 2 | | | | | | | |
| 2 → 3 | | | | | | | |
| 3 → 4 | | | | | | | |
| 4 → 5 | | | | | | | |
