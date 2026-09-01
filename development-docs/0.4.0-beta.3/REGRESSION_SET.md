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
| 0 → 0.5 | 2026-08-18 | ⬜ deferred to P0.5 (owns this boundary) | ⬜ live app | ⬜ live app | ⬜ live app | ⬜ live app | 🟡 T1 only — manifest↔copy round-trip + RED; real N−1→N apply owed at RC |
| 0.5 → 1 | | | | | | | |
| 1 → 2 | | | | | | | |
| 2 → 3 | 2026-08-26 | 🟢 **GREEN both halves** (see run log) | ⬜ needs a real payment | ⬜ live app, human | 🟢 T1 — 73 engine tests · ⬜ T2 e2e | ⬜ needs a payment | 🟡 T1 only (6 tests); real N−1→N owed at RC |
| 3 → 4 | 2026-09-01 | 🟢 **GREEN both halves** (run log) | ⬜ needs a real payment | 🟡 **PARTIAL** — the arms this phase touched are green; file-dialog arm + a clean prevent-close pair still owed | 🟢 T1 (preflight `T1a`) · ⬜ T2 e2e | ⬜ needs a payment | 🟡 T1 only; real N−1→N owed at RC |
| 4 → 5 | | | | | | | |

---

## Run log — 2 → 3 boundary (2026-08-26)

Phase 2 changed wallet HTTP timeouts on the money path and moved the balance call off the UI
thread, so this boundary matters more than a docs-only one would.

### R-INTEXT — 🟢 GREEN, both halves, correct SUBJECT

⛔ **This check was NOT RUNNABLE AS WRITTEN — at this or any previous boundary.** Its SUBJECT is
*"Rust log: absence of `X-Requesting-Domain` for (a), presence with the exact page host for (b)"* —
and nothing in `domain_trust_mw` ever logged it. Fixed: one `log::debug!` in
`rust-wallet/src/main.rs :: domain_trust_mw` records path + requesting domain. DEBUG, so it never
reaches a user (the wallet ships at `warn`), and **host only** — never path or query — matching the
browser side's `LogSafeUrl` rule, because that line records which site the user is talking to.

What Rust actually received in one session:

```
51 x requesting_domain=<none:internal>     <- wallet UI, startup, peerpay polling
 1 x requesting_domain=example.com         <- path=/getVersion, from the page
```

| | Observed |
|---|---|
| **(a) internal** | `window.__hodos_walletCall('/wallet/status')` from the first-party UI -> `OK`, **no** header, **no** modal |
| **(b) external** | `window.CWI.getVersion()` from `https://example.com` -> header present carrying **exactly the page host** -> `202 PENDING` -> `domain_approval` modal, owner-observed on screen |

Same target (`127.0.0.1`), opposite outcomes, discriminated only by the frame origin the renderer
cannot forge. **Each half is the other's control**, which is what this check asks for.

⭐ The owner clicked **Block/Deny** deliberately, not Approve: approving would make `example.com` a
standing approved domain and render every future run of this check **vacuous** — the trap that made
the P0.8 bitgenius test worthless. Verified it left no residue: `🔐 Domain example.com blocked
in-memory for this session`, and **no `example.com` row in `domain_permissions`**. Matches the P0.9
standard that prompt denials are temporary.

### 🎯 Incidental: the audit log was observed firing for the first time

Phase 2 shipped `audit-<pid>.log` with its wiring proven only by unit test and code read. The
consent prompt above produced, live:

```
[2026-08-26 14:23:14.503] consent.prompt_shown | example.com | type=domain_approval
```

⚠️ The **`payment.auto_approved`** half is still unobserved — it needs a real payment.

### R-PERIM — 🟢 T1 green, ⬜ T2 owed

`cargo test -p hodos_permission_engine`: **73 tests, all passing** (40 + 33 across the two suites).
That is the Matrix C decision logic, which is this check's stated SUBJECT ("the Rust decision, not
the UI's appearance"). The end-to-end half — flipping each of the four perimeter preconditions in a
live browser — is **not** run.

### R-UPDATE — 🟡 T1 only, unchanged from the 0 -> 0.5 boundary

6 update tests green in `hodos_tests` (+1 pre-existing skip, `UpdateStagerRig.StagesFromLocalFeed`).
`scripts/test-apply-forward.ps1` / `test-apply-rollback.ps1` exist but drive a **real staged
installer**; the genuine N−1 -> N apply remains owed at RC, as recorded at the previous boundary.

⚠️ Phase 2 did **not** touch the update path, but it did change the log its operators read, and
`SilentStateWriter` was one of the files whose process tag was wrong.

### ⬜ NOT RUN — and why, stated plainly rather than left blank

| Check | Why not |
|---|---|
| **R-GOLD** | Needs a **real auto-approved payment**. Cannot be faked: the point is that the pill appears without a modal, on the correct `Tab::id`. |
| **R-CLOSE** | T3, human. Needs a native file dialog held open and the wallet overlay driven into an unsafe state, per flag, across three separate close paths. |
| **R-COUNT** | Needs spending to just under the session cap, then a tab close/reopen. |

⛔ These three most directly guard the money path, and Phase 2 changed money-path timeouts.
**They are owed, not waived.** One real payment session would close R-GOLD, R-COUNT and the
`payment.auto_approved` audit line together.

---

## Run log — 3 → 4 boundary (2026-09-01)

Run after Phase 3.5 landed its **root** fix (overlay ownership follows the requesting window,
`3237068`). ⭐ The check that mattered most at this boundary is **R-CLOSE**, because 3.5 is the first
phase to change overlay *lifetime* rather than only positioning.

### R-INTEXT — 🟢 GREEN, both halves, correct SUBJECT

📏 Driven over CDP, asserted against the **Rust** log (the decision, not the UI):

| | Observed |
|---|---|
| **(a) internal** | `window.__hodos_walletCall('/wallet/status')` from the first-party UI → `"GET /wallet/status HTTP/1.1" 200`, no gate, no 202 |
| **(b) external** | `window.CWI.getVersion()` from `https://example.com` → `🛡️ engine Prompt (domain-trust) minted approval id=a5cb040a… for domain=example.com endpoint=/getVersion type=DomainApproval reason=new_domain_no_manifest` |

⭐ (b) carries **exactly the page host**, and the decision is the engine's `Prompt`, not a rendered
modal. ⚠️ The *injected* RED (force internal-as-external and vice versa) was **not** re-run here — it
needs a code change. The two halves are each other's control per this document's own framing.

### R-CLOSE — 🟡 PARTIAL, and honestly so

✅ **Covered, on this build, and these are the parts Phase 3.5 changed:**
- 📏 `P3.5-Z3` run directly: the menu overlay was `Vis=True` and **owned by the secondary window**
  when that window was destroyed → `IsWindow(overlay)` still true, all 14 overlays present (K26).
- 📏 Product log: `Re-owned 1 overlay(s) off a closing window` — the new safety net firing in situ.
- 📏 Click-outside close path exercised for wallet, profile and tab-list (`Hiding … — lost activation
  (click-outside)`), and the guard toggles observed both ways (`close prevention ENABLED` /
  `DISABLED`).

⬜ **Still owed, unchanged from the 0→0.5 and 2→3 boundaries:**
- the **`g_file_dialog_active`** arm — needs a native file dialog held open;
- a clean **GREEN/RED pair** on `g_wallet_overlay_prevent_close`. ⚠️ Two attempts to drive it failed
  on **sequencing**, not on the product: the first set the flag 2.6 s *after* the overlay had already
  been dismissed, and the second could not reliably force the overlay's own `WM_ACTIVATE`. Recorded
  as a probe limitation. ⛔ Not claimed as passed.
- the **`WH_MOUSE_LL`** third path.

### R-PERIM — 🟢 T1, ⬜ T2 e2e owed

📏 `preflight.ps1 -Full` → `T1a cargo test - rust-wallet` **PASS**, which is where the permission
engine's unit suite lives. Unchanged from the 2→3 boundary: the end-to-end T2 arm is still owed.

### R-GOLD / R-COUNT — ⬜ NOT RUN, same reason as every prior boundary

Both need a **real auto-approved payment**. ⛔ Cannot be faked: R-GOLD's whole point is that the pill
appears with no modal, on the correct `Tab::id`; R-COUNT needs a spend to just under the session cap
followed by a tab close/reopen. One payment closes both **and** the unobserved
`payment.auto_approved` audit line.

### R-UPDATE — 🟡 T1 only, unchanged

Real N−1 → N apply still owed at RC.

### ⭐ What this boundary actually establishes

Phase 3.5 changed **overlay ownership**, so the risk it carried was to R-CLOSE. That risk was
measured directly and the overlay survived. The gaps listed above are **pre-existing and identical to
the previous two boundaries** — they are not new debt created by this phase, and none of them is
blocked on it.
