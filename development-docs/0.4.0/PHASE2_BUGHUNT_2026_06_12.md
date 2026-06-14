# Phase 2 Permission-Surface Bug Hunt + Remaining-Work Map (2026-06-12)

**Source:** 23-agent adversarial workflow (3 kickoff-review + 5 bug-finders + 1 verifier/finding, refute-by-default). Run `wf_ed07d9ee-a61`.
**Scope:** the Phase 2.6 permission surface (Rust gate + pure engine + C++ IPC bridge) through 2.6-E + fix1, plus a readiness map for 2.6-F/G/H.
**Status:** 🟢 B1 + B3 FIXED (2.6-E.fix2, verified live). ⚠️ B2 lockdown ATTEMPTED + REVERTED 2026-06-14 (broke legit direct-fetch dApps) — reachability assessment below was WRONG; B2 re-deferred as a hard design problem. B4 deferred.

## Resolution log
- **⚠️ B2 — LOCKDOWN REVERTED 2026-06-14. The reachability analysis below was WRONG.** I shipped a direct-fetch lockdown (`5de4a62`) assuming external dApps only use the IPC shim and raw fetches are CORS-blocked. **False:** the C++ interceptor serves `Access-Control-Allow-Origin: *` on every response (`GetResponseHeaders` ~1922), so external dApps using the `@bsv/sdk` WalletClient HTTP substrate direct-fetch `localhost:31301` **successfully and commonly** (e.g. `socialcert.net` `createHmac`). The lockdown returned `ERR_DIRECT_FETCH_BLOCKED` and broke cert acquisition. **Co-test caught it within minutes; reverted at `ccf7c6d`.** Corrected understanding: the **Open() / direct-fetch path is LIVE and heavily used**, NOT dead. B2 is real (cross-origin iframe inherits the main-frame origin via `extractDomain`) and reachability is **HIGH**, but a blanket block is not viable — it needs **per-frame origin attribution** (referrer-based, imperfect) or an accepted-risk decision. Re-deferred. (Note: fix2's session cap is unaffected — Open() forwards payment to Rust, so it applies to direct-fetch dApps too. But 2.6-F cert was only IPC-migrated; the Open() cert cascade still runs C++ for direct-fetch — see the "Complete 2.6-F" task.)

- **B1 + B3 — FIXED in 2.6-E.fix2 (2026-06-12).** Wired `record_spending` into the Silent payment branch (`request_gate.rs`) so the cumulative session dollar cap accumulates, and into the X-User-Approved replay branch (cents/browser_id stashed on the approval at mint time via new `mint_pending_payment_approval`) so prompted-then-approved over-cap payments also count. Recorded at gate-Silent time (conservative; consistent with the existing rate/count counters). 102 permission_service + 71 engine tests pass. **Live before/after** (throwaway approved domain, per-session=50¢, unfundable body → zero broadcast): payment #2 (two 30¢ → cumulative 60¢ > 50¢) went from `400`/Silent (bug) to `202 engineReason:session_cap`; log shows `#1 … session_spent_now=30` then `#2 engine Prompt … reason=session_cap`.
- **B2 — DOWNGRADED to Low/Medium after reachability check (2026-06-12).** The cross-origin-iframe escalation is **largely mitigated** but not conclusively closed:
  - **Primary path is SAFE.** The Phase-2.5 shim (`CWIShimScript.h:141`) dispatches via `cefMessage.send('wallet_call')` → `HandleIpcWalletCall` (simple_handler.cpp:1656), which derives `X-Requesting-Domain` from the **calling frame's** origin (`frame->GetURL()`), not the main frame. And the shim is **main-frame-only** (`simple_render_process_handler.cpp:890` skips iframes). So an iframe can't even use the shim, and a sub-frame dApp that does gets its *own* origin. (The shim header comment at CWIShimScript.h:15 "POSTs to localhost:31301" is **stale** — it's IPC now.)
  - **The vulnerable code is the direct-fetch path only** (`extractDomain` @ HttpRequestInterceptor.cpp:6080, used at 4885 → `AsyncWalletResourceHandler`), which attributes by **main frame**. It's reachable only by a *raw* `fetch('http://127.0.0.1:31301/...')` that bypasses the shim — i.e. a malicious iframe.
  - **Browser/CORS layers block the common case.** Wallet CORS (`main.rs:820`) allows only `localhost:5137 / 127.0.0.1[:5137] / localhost` — a cross-origin **JSON** POST fails preflight → never reaches the wallet. Chromium **Private Network Access** (public→localhost) adds another gate.
  - **Residual risk (needs a live test to close):** a CORS **"simple request"** (`Content-Type: text/plain` carrying a JSON body) skips the standard preflight; whether it still reaches `createAction` depends on (a) whether the handler accepts non-JSON content-type and (b) whether CEF 136 enforces PNA for simple requests. **Co-test:** load an approved page embedding a cross-origin iframe that attempts both JSON and `text/plain` raw fetches to `localhost:31301/createAction`; watch `wallet_rCURRENT.log` for whether it reaches the gate and what `X-Requesting-Domain` it's attributed.
  - **Recommended fix (cheap, do regardless):** lock down the direct-fetch path for external origins — the interceptor should reject raw wallet-call fetches from non-allowlisted origins (force all external calls through the per-frame IPC bridge), and/or have `createAction` require a JSON content-type + explicit PNA denial. Aligns with the 2.6-G/H "IPC is the only external entry" direction.
- **B4** — deferred to a later indicator/UX polish.
- Note: `tier5/7/11_*` integration test crates have a pre-existing `PriceCache` compile breakage (from commit `5ed993b`, unrelated to fix2) — worth a separate cleanup.

> Companion to `HelicOps/AUDIT_FIX_TRACKER.md` (the HelicOps audit). These are **new** findings from our own bug
> hunt; IDs are prefixed **B** (bug-hunt) to keep them distinct from HelicOps **F** items.

## Confirmed findings (skeptic agreed, refute-by-default)

| ID | Sev | Where | What's wrong | Fix |
|----|-----|-------|--------------|-----|
| **B1** | 🟠 **High** | `rust-wallet/src/permission_service/state.rs:456` (`record_spending`) — zero production callers; engine read at `crates/hodos_permission_engine/src/matrix_c.rs:268`; gate at `request_gate.rs:714-737` | **Per-session cumulative DOLLAR cap is dead.** `record_spending` (sole writer of `spent_cents`) is never called in production — only in tests + doc-comments. So `session_spent_cents` is **always 0**, and the engine's cumulative check `spent + requested > per_session_limit` degrades to a *second per-tx cap*. An approved domain (per-tx $1 / session $5) can make **unlimited $0.99 auto-approved payments** — the $5 session ceiling never trips. **A 2.6-E regression**: the old C++ `SessionManager` recorded spend; the Rust port wired the rate counter (`increment_payment_rate_counter`, request_gate.rs:732) but not `record_spending`. The 2.6-E smoke tested per-tx + rate (both wired), never the cumulative $ cap. 3 independent agents + 3 skeptics + manual grep confirm. | Call `permission.record_spending(browser_id, domain, cents, now)` after a Silent payment is processed (the doc-comment at request_gate.rs:586 already describes the intended call). Decide record point: at gate-Silent time (simple, but counts not-yet-built txs) vs post-broadcast in the handler (accurate). → **2.6-E.fix2** |
| **B2** | 🟠 **High** *(reachability TBD)* | `cef-native/src/core/HttpRequestInterceptor.cpp:6080-6122` (`extractDomain`), used at 4885/4752 | **Cross-origin iframe origin confusion.** `extractDomain` derives the origin from the **main frame** URL only. A cross-origin iframe (ad slot, third-party embed) issuing a direct `fetch('http://localhost:31301/createAction')` is attributed to the **top-level approved domain** → inherits its caps, identity-key disclosure, and scoped grants. Skeptic confirmed the code behavior but flagged this is the **legacy direct-fetch** path. | **Open question before rating:** is the direct-fetch path still reachable for external pages post-2.5 (page CSP `connect-src`)? Does the primary **IPC-bridge** path share the same main-frame origin determination, or is the shim per-frame? Per-frame origin attribution + a frame-origin check is the likely fix. **Investigate reachability first.** |
| **B3** | 🔵 Low | `crates/hodos_permission_engine/src/matrix_c.rs:249-256`; root `request_gate.rs:629-666` | **Over-cap X-User-Approved replay never advances the session tx count.** Only the Silent branch increments `payment_count_this_session`; the replay (modal-approved) path skips it. A site that deliberately stays over-cap (forcing a prompt each time) can have the user approve a stream that never hits `max_tx_per_session`. Same code area as B1. | Increment the session tx count on the replay path too (fold into the B1 fix). |
| **B4** | 🔵 Low | `cef-native/src/core/HttpRequestInterceptor.cpp:2102-2106` (`isPaymentEndpoint`); 4480-4495/4600-4610 | **Green-dot fires on non-spending `/sendMessage` + zero-fee calls.** Cosmetic false-positive — erodes the signal value of the primary visual payment safeguard over time. Not exploitable, not a missed-fire. | Optionally gate the indicator on an actual non-zero on-chain spend (wallet returns a txid / spent sats). Low priority. |

## Refuted findings (verified NOT bugs — do not re-investigate)
The verify stage killed 9 plausible-but-wrong findings:
- **Page-header forwarding "exploits"** (×3, fix1-replay/session-greendot/cpp-bridge) — the forward loop re-inserts page headers, but the engine-critical headers (`X-Payment-Cents`, `X-Requesting-Domain`) are **C++-injected, not page-supplied**, and deterministic header ordering means injected values win. Not exploitable as claimed.
- **X-Payment-Cents trust** — false premise; it is not page-supplied.
- **X-User-Approved keying on raw header** (handlers.rs:4357) — `engine_authorized_payment` is recomputed correctly; the approval is body-hash-bound + single-use + TTL'd in `consume_and_verify` (state.rs:248-274).
- **Pending approvals not origin-bound** — code-level observation true, exploit conclusion wrong.
- **Protected-basket guardrail relies on caller** — accurate but a hardening suggestion, not a bug.
- **BRC-121 vs engine counter spaces** — refuted (and a known, accepted trade-off per 2.6-E close).
- **pending_approvals RwLock `.expect()` poison** — contingent, structurally can't trigger.

## Design decision — session limits: dollar vs tx-count
**Keep BOTH** (they guard different failure modes):
- **Per-session dollar cap (`per_session_limit_cents`) = PRIMARY.** Bounds actual financial loss. This is B1 (currently dead — must fix).
- **Per-session tx-count cap (`max_tx_per_session`) = SECONDARY backstop.** Cheap; catches runaway loops; and is the **only numeric guard that works when the BSV price feed is unavailable** (can't compute $, can still count). Already wired.
- Dropping either weakens a distinct axis (value vs volume). The wallet already models both in `domain_permissions`; the gap is purely that the $ one isn't recorded.

## Phase 2.6 remaining-work map
| Sub-phase | Scope | Key notes |
|---|---|---|
| **2.6-F** Cert Disclosure (`/proveCertificate` non-sensitive) | **Small (~0.5–1 day)** | Engine branch (`matrix_c.rs:50-62`), `cert_field_permissions` table+repo, and C++ 202-interception (`HttpRequestInterceptor.cpp:3821-3839`) **already exist** (built in 2.6-C). Needs: `dispatch_cert_disclosure` helper + `build_cert_disclosure_context` + rewire `prove_certificate` non-sensitive branch (certificate_handlers.rs ~3245) + delete C++ inline cascade (HttpRequestInterceptor.cpp:3108-3185) + tests. **Decision needed:** add `check_domain_approved` defense-in-depth to `prove_certificate` (currently absent) for parity, or document the exemption. |
| **2.6-G** Domain Trust (catch-all) | **Medium-large (~1.5–2 days)** | `dispatch_domain_trust` + `build_domain_trust_context` + C++ IPC kind-list flip + delete C++ Open()-path inline trust/manifest. Design item: **ManifestFetcher ownership (OQ8)** — porting to Rust adds a timed, no-poison-cached network client. Highest blast radius (catch-all). |
| **2.6-H** Cleanup + ship | **~1 day, BLOCKED on F+G** | Delete C++ engine + 4 inline caches + SessionManager + V23 migration + dual-platform build + 30-45 min thorough smoke. |

**Critical path to Phase 2.6 close: ~3–4 focused sessions.**

### ⚠️ Plan-doc corrections needed before 2.6-H (`PHASE_2_6_ENGINE_TO_RUST.md`)
The agents flagged ~15 stale citations. The big ones:
- **The 5-flag-per-class model was DROPPED** at the 2.6-C kickoff (`flags.rs:1-25`). Only `shadow_log_enabled` survives; C/D/E shipped **flagless** ("land = authoritative"). The doc's LD3 §138-162 and every `engine_rust_*` flag reference in F/G/H Done-when criteria are obsolete.
- **V-migration numbers off by ~2** (doc says V20/V21/V23 in places that don't match `migrations.rs`).
- **"Caches" are inline, not files** (DomainPermissionCache etc. are in-process structures, not separate cache files).
- **SessionManager still live + BRC-121-coupled** — its deletion in 2.6-H is gated on the BRC-121 migration (OQ5).
- **HttpRequestInterceptor.cpp line numbers drifted ~700-900 lines** from the doc's estimates (C/D/E refactors).

## Recommended sequence
1. **B1 → 2.6-E.fix2** (wire `record_spending`; fold in B3). Small, security-relevant, in the just-touched payment path. Live-verify with the gate + logs before & after.
2. **B2 reachability check** — decide priority once we know if the direct-fetch path is live for external pages and whether the IPC bridge shares it.
3. **Plan-doc correction pass**, then **2.6-F → 2.6-G → 2.6-H**.
4. **B4** — fold into a later UX/indicator polish.

**Full workflow output (126 KB):** archived from the run; key file:lines captured above.
