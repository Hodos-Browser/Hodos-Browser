# Phase 0.9 — full state, problems found, correct behaviour, and the plan

**Written 2026-08-24** after a day of build-measure-fix cycles that surfaced far more than the
phase was scoped for. Owner called the pause; this document exists so we stop re-deriving the
same facts.

⛔ **Nothing in this document is a plan to write more code until the owner signs off on §5.**

---

## 1. The architectural facts (measured, not assumed)

These constrain every design option below. Each was verified this session.

| # | Fact | Evidence |
|---|---|---|
| F1 | Chromium 150 raises a **Local Network Access** permission for public sites reaching `127.0.0.1`/`localhost`. `kLocalNetworkAccessChecks` is `FEATURE_ENABLED_BY_DEFAULT`, `…Warn=false`. | `services/network/public/cpp/features.cc:237` |
| F2 | It reaches us as `CEF_PERMISSION_TYPE_LOOPBACK_NETWORK` (`1<<27`) via `OnShowPermissionPrompt`. | Log: `mask=0x08000000 mapped=[loopback]`, three separate runs |
| F3 | `127.0.0.1`, `::1`, `localhost` and `*.localhost` are **the same address space**. There is nothing for us to detect. | `net/base/url_util.cc :: HostStringIsLocalhost` |
| F4 | The grant is **blanket per site** — keyed on the requesting origin only, no target port. One ALLOW = every local service. | `content_settings_registry.cc:815`, `TOP_ORIGIN_ONLY_SCOPE` |
| F5 | **Wallet calls never reach the network**, so they never trigger F1 and can never be gated by it. | Log `08:39:34.774 Wallet endpoint detected → /getVersion`, 2 ms *before* the loopback prompt existed |
| F6 | Answering the CEF callback is what writes Chromium's setting. ACCEPT→persistent ALLOW, DENY→persistent BLOCK, DISMISS→nothing. We never write it ourselves. | Measured: an `allow_once` click produced a persistent ALLOW in `Preferences` |
| F7 | CEF has **no "grant once"** result. `ACCEPT/DENY/DISMISS/IGNORE` only. | `cef_types.h :: cef_permission_request_result_t` |
| F8 | Chromium consults **its own** setting before it ever calls us again. Once a decision exists, `OnShowPermissionPrompt` is not called. | Measured: a stored BLOCK on `example.com` produced 0 handler hits |
| F9 | **One wallet DB is shared by every browser profile** (`%APPDATA%/HodosBrowserDev/wallet/wallet.db`). Browser profiles isolate Chromium settings, history, cookies and `site_permissions.db` — they do **not** isolate wallet `domain_permissions`. | 14 approved domains present regardless of profile |

**F5 + F9 together are why the last test looked insane:** `bitgenius.net` was already approved in
the shared wallet DB, so it connected instantly with no modal; the loopback prompt then appeared
for some *other* local request. The user correctly observed the prompt was "asking permission for
something it is already doing" — because the thing it is already doing (wallet access) is **not**
what the prompt governs.

---

## 2. Problem inventory — everything found this session

### 2.1 Defects in the phase's own new code (all now fixed, none verified end-to-end)

| ID | Problem | Status |
|---|---|---|
| P1 | "Allow this time" was a lie — CEF cannot express it, so it persisted (F7). | Fixed: two buttons for network types |
| P2 | Wallet modal painted over the loopback prompt 239–434 ms after it appeared. | Fixed: pre-emption latch + re-show |
| P3 | First collision fix read `g_pendingModalDomain` at close time; the auth handler clears it in the same millisecond, so it never fired. | Fixed: latch set at pre-emption time |
| P4 | 🔴 `domain_approval` carried the local-access grant and rendered **no disclosure**. My notice insert landed in the manifest *Customize* subview. A code comment asserted the opposite. | Fixed + fail-closed ack |
| P5 | 🔴 Deferred prompt bypassed the "don't stomp a live modal" guard — could destroy a payment/cert consent surface and strand an invisible click-eating overlay for up to 300 s. | Fixed: guard + DISMISS |
| P6 | 🟠 A stale binding could be cashed in by any later approval on that host. | Fixed by the ack, not by request bookkeeping |
| P7 | 🟠 Hub toggles for the new types wrote only our SQLite; Chromium's setting governs (F8), so revoke did nothing. | Fixed: write-through via `CefRequestContext::SetContentSetting` |
| P8 | 🟡 Binding matched hardcoded `6`/`7`; those integers were already reassigned once in this project's history. | Fixed: enum |
| P9 | 🟡 Re-show path would re-show a permission already bound to an open modal. | Fixed: `peekParked` skips bound |
| P10 | My own HIGH-3 fix required a `requestId` the connect payloads never send → binding never released → `hasPending()` jammed **every** permission browser-wide. | Fixed: always pop, ack decides |

### 2.2 Pre-existing defects found along the way (NOT caused by 0.9)

| ID | Problem | Scope |
|---|---|---|
| X1 | The padlock **Site permissions** toggles write only our SQLite for location / notifications / clipboard. Same class as P7, still broken for those three. | Ticket |
| X2 | "Allow this time" is equally misleading for location / notifications / clipboard (F7). Camera/mic are genuinely fine (media path persists nothing). | Ticket |
| X3 | Deleting the profile you are running on was permitted. | **Fixed** 2026-08-24 |
| X4 | Profile ids were reused after delete, and `DeleteProfile` never removes files — so a new profile **adopted ~200 MB** of the deleted one's history, cookies and content settings. | **Fixed** 2026-08-24 |
| X5 | `profiles_delete` logged "Profile deleted" unconditionally, even on refusal; React removed the row optimistically. | **Fixed** 2026-08-24 |
| X6 | `--disable-features=Autofill` in `simple_app.cpp` is a **no-op** — no Chromium feature by that name exists. | Ticket |
| X7 | The permission prompt fetches favicons from `google.com/s2/favicons` — a third-party network call from a privacy browser's consent surface. | Ticket |
| X8 | Wallet approvals are global while browser profiles are isolated (F9). Surprising, and it makes profile-based test isolation impossible. | Ticket / design question |

### 2.3 The testing-control problem — the actual root of the tail-chasing

Every failed run today failed for a **state** reason, not a code reason:

- `example.com` had a stored BLOCK from four days earlier → no prompt, wrong conclusion drawn.
- `bitgenius.net` was already wallet-approved (F9) → no connect modal → the binding never engaged.
- A jammed parked permission (P10) → everything fell through to Chromium's stock UI.
- A deleted-then-recreated profile inherited the old profile's settings (X4).

**There is no way to put the system into a known state before a test.** That is the defect that
cost the most time today, and it is fixable in an hour.

---

## 3. Correct behaviour — the specification

Written as "given / when / then" so each line is directly testable.

### 3.1 The two consents are orthogonal (F5)

- **Wallet consent** (our connect modal) governs access to the Hodos wallet.
- **Loopback consent** (Chromium's permission) governs access to *every other* local service.
- Neither implies the other. A site can hold one and not the other.
- ⛔ The prompt copy must never claim that blocking loopback protects the wallet. It does not.

### 3.2 Behaviour matrix

| # | Given | When | Then |
|---|---|---|---|
| B1 | Site not wallet-approved, no loopback decision | Site triggers both | **One** modal: the connect modal, carrying the local-access disclosure. Approve → loopback ALLOW persisted. Decline → nothing persisted (DISMISS). |
| B2 | Site **already** wallet-approved, no loopback decision | Site makes a non-wallet loopback request | Standalone Hodos loopback prompt, two buttons. No connect modal exists to carry it. **This is correct**, and is what looked wrong in testing. |
| B3 | Site has loopback ALLOW already | Any loopback request | No prompt. Chromium never calls us (F8). |
| B4 | Site has loopback BLOCK already | Any loopback request | No prompt, request fails. Chromium never calls us (F8). |
| B5 | A non-connect modal (payment, cert disclosure) owns the overlay | Loopback permission arrives | Never stomp it. Resolve DISMISS; Chromium re-asks later. |
| B6 | Connect modal carries the grant but the disclosure did not render | User approves | **Refuse the grant** (DISMISS). Fail closed. |
| B7 | User revokes in the padlock panel | Sets Block / Reset | Chromium's setting changes, not just ours. Next request is blocked / re-prompts. |
| B8 | User clicks "Don't allow" on the standalone prompt | — | Persistent BLOCK (F6). ⚠️ **Open question — see §5 Q2.** |

### 3.3 Non-goals for 0.9

- Making the loopback permission gate the wallet (that is a security-model change; F5 says it
  currently cannot, and a wallet-only dApp would never raise the prompt to unblock itself).
- Fixing X1/X2 for the existing five permission types.
- Rewording the five existing prompts.

---

## 4. Test controls — build this FIRST

⛔ **No further acceptance testing until this exists.** Today proved that testing against unknown
state produces wrong conclusions faster than it produces evidence.

A `reset-test-state.ps1` that, with the dev browser stopped:

1. Reports current state: profiles on disk vs registry, per-profile `loopback_network` /
   `local_network` entries with values and timestamps, and the wallet's `domain_permissions` rows.
2. `-ClearLoopback <profile>` — removes the loopback/local-network exceptions from that profile's
   `Preferences` (backup first).
3. `-ClearWalletDomain <domain>` — deletes that domain's `domain_permissions` row **and its
   children** (⛔ Python `sqlite3` needs `PRAGMA foreign_keys=ON` or the cascade silently no-ops —
   this bit us in P0.8).
4. `-FreshProfile <name>` — creates a profile with a genuinely unused id and verifies the directory
   did not already exist.
5. `-Verify` — asserts the target state is actually what was asked for, and **fails loudly** if not.

Step 5 is the important one: every wrong conclusion today came from assuming state rather than
asserting it.

---

## 5. Owner decisions — ANSWERED 2026-08-24

| Q | Decision |
|---|---|
| Q1 — already-wallet-approved site gets the loopback prompt alone | **Accept.** Rare, correct in mechanism. Not reworded. "Check whether the wallet already approved it" was considered and deferred — it amounts to auto-allowing blanket local access on the strength of an unrelated earlier decision. |
| Q2 — should a prompt "Don't allow" persist? | **No.** Decisions made in prompts are temporary and get re-prompted. Overt actions in the Site controls panel stay until overtly changed. Now the standard for all approvals. |
| Q3 — clean up profile state? | **Yes**, keep only the dev-wallet profile. Done: 5 profile directories removed (~1.1 GB), registry reduced to `Default`. |
| Q4 — build a test that fails if `LocalAccessNotice` disappears? | **No.** Owner declined. Recorded as an accepted gap; the fail-closed acknowledgement keeps the resulting defect non-exploitable but the invariant is unguarded. |
| Extra — already-approved sites gaining loopback silently | **Don't care.** If a previously approved site shows the modal once after release, fine — no security issue. |
| Extra — is a CWI call a loopback call? | **No**, and neither is a direct wallet fetch. Both are answered by our interceptor before the network, so neither raises the permission. The notice therefore only ever appears when a real loopback permission exists to bind. |

## 6. Outcome

All four acceptance tests passed against **verified** state. Measured results are in
`PHASE_CONTRACT.md` §5; design decisions and their reasoning in §7.

Built during this session, in the order the plan called for:

1. `reset_test_state.py` — the missing test control. `show` / `clear-loopback` /
   `clear-wallet-domain` / `verify`, where `verify` exits non-zero on mismatch.
2. State wiped to one profile and verified clean.
3. Full matrix run: connect-binds-and-grants, decline-stores-nothing, deny-is-temporary,
   allow-reaches-the-server, block-stops-it-server-side, Site-controls-write-through.
4. The fixed 750 ms timer replaced with activity-driven waiting after it was measured racing the
   connect modal (239 / 282 / **830** ms gaps; the last one flashed a prompt for 71 ms).

Tickets filed for everything out of scope: `TICKET_site_permission_dual_store.md`,
`TICKET_prompt_denials_should_not_persist.md`, `TICKET_disable_features_autofill_is_a_noop.md`,
`TICKET_consent_surface_fetches_third_party_favicon.md`,
`TICKET_wallet_global_profiles_isolated.md`. macOS round in `MAC_RELAY_P09_ROUND.md`.

## 7. Still open

- **`P0.9-A5` — DPI cells #4/#6/#9 never run.** Carries to Phase 1 (WS1), which owns the overlay
  input work and the unresolved mouse-offset question.
- **macOS entirely unexercised.** Every mac arm compiles; none has executed.
- **The disclosure invariant is untested** (Q4), by decision.
