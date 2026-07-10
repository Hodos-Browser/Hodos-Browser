# Wallet-UI → C++ Bridge Migration (Full Bridge, Reuse-Based)

> **Status:** **CORE DELIVERABLE LANDED + BUILT (2026-06-25; not pushed, not live-tested).** Reuse-based pivot (red-team `wf_d67e8bd5-37e`). Commit 1 `a95e01e` (Rust port gate + self-call + export/import guard) ✅ · Commit 2 `4ba4fe7` (C++ `PortConfig.h` + ~41 sites) ✅ · Commit 3 `3b3e063` (internal-origin header omission + bridge on first-party) ✅ · Commit 5 `6331ef1` (frontend 77-site → `walletFetch`, tsc+vite) ✅ · Commit 4 `1922aa1` (size-gated chunked delivery for large responses; adversarially reviewed by 3 agents, fail-closed integrity + sliding timeout folded in; builds clean) ✅. Dev(31401)+installed(31301) can now run simultaneously. **OQ-1: ships BEFORE `0.3.0-beta.16`.** Remaining: **2b** fail-closed `is_dev()` (DEFERRED — changes data-dir isolation semantics, wants owner sign-off) · **6** mac compile-verify (needs a Mac; mac literals already edited) · **7** docs (broader CLAUDE.md invariant refresh) · then the full §7 test matrix — incl. a **live large-export round-trip** (export → re-import → balances match) since Commit 4's big-payload path is built+reviewed but not yet live-tested, and the backup/export UI button is not wired yet — BEFORE the release pipeline.
> **Driver:** Dev/prod port deconfliction (run installed + dev browser simultaneously) surfaced that the first-party wallet UI calls Rust directly. Owner chose the best-practice end-state: **C++ owns the wallet port for ALL traffic; the frontend makes zero direct HTTP calls.**
> **Branch:** `0.4.0` (→ origin/staging → origin/main per DevOps-CICD).

---

## 0. ⚠️ The pivot (read first)

The first plan proposed a **new** `wallet_proxy_request` IPC + a **role-based** first-party gate. The red-team's second pass caught that this **reinvents existing, shipped infrastructure** (CLAUDE.md reuse-first violation). The reality, verified line-by-line:

- **A generic wallet IPC bridge already exists** — `window.__hodos_walletCall(method, endpoint, body, httpMethod)` (`CWIShimScript.h:109`) → `cefMessage.send('wallet_call', …)` (`simple_handler.cpp:1886`) → `HandleIpcWalletCall` (`HttpRequestInterceptor.cpp:2099`) → response via `wallet_response` IPC → `window.__hodos_walletResponse` (`CWIShimScript.h:74`). It has a `pending` promise-correlation table and a **50MB request ceiling** already.
- **It gates internal-vs-external by frame ORIGIN, not role** — origin parsed from `frame->GetURL()` (`simple_handler.cpp:1903-1914`), tested by `IsInternalOrigin()` (`HttpRequestInterceptor.cpp:1012`). Origin is the browser-process's truth; **JS cannot forge it**. So a website calling `wallet_call` carries its real domain → external → full gate. **The role-based "escalation keystone" was solving an already-solved problem.**

**Therefore the migration is REUSE, not new-build:** route the 77 first-party fetches through the existing `__hodos_walletCall`, make that bridge available on internal pages, and apply one small correctness fix (below). The big "new proxy" commit evaporates.

---

## 1. Goal

Route **every** first-party wallet-UI request through the existing C++ `wallet_call` bridge so that: (1) C++ is the single choke point; (2) C++ owns the port — frontend never knows it → dev (`HODOS_DEV=1`, wallet `31401`/adblock `31402`) and prod (`31301`/`31302`) run simultaneously; (3) nothing breaks — behavior byte-for-byte preserved for every gate and privacy-perimeter safeguard.

Non-goal: changing crypto/signing/schema or the website/CWI-shim provider. Only the **transport** of first-party calls changes.

---

## 2. The trust contract (load-bearing) + the one real fix

Rust's internal-vs-external decision keys off **presence of a non-empty `X-Requesting-Domain` header**:

| Where | Rule |
|---|---|
| `main.rs:62-74` (`domain_trust_mw`, wraps every route) | absent/empty → internal, no gate. present → `domain_trust_gate` (403/202). |
| `handlers.rs:621-661` (`check_domain_approved`) | `None`/empty → internal; present-unapproved → 403. |
| `handlers.rs:12516-12527` (`wallet_backup`), `:14545-14552` (`wallet_restore`) | **reject (403) ANY non-empty `X-Requesting-Domain`** — local-only, unconditional. |
| `handlers.rs` `get_public_key` identity branch | privacy-perimeter prompt only when header present. |

**Boundary encoding:** first-party (`127.0.0.1:5137`) sends no header; C++ injects it only for website traffic. `first-party = no header. website = real domain header.`

### THE FIX (this is "Rule 1", now a few lines): `runIpcCallDirect` must not stamp the header for internal origins

`HandleIpcWalletCall` routes internal origins to `runIpcCallDirect` (`:2111-2115`). But `runIpcCallDirect` stamps the header for **any non-empty origin** (`HttpRequestInterceptor.cpp:1868`):
```cpp
if (!origin.empty()) headers["X-Requesting-Domain"] = origin;
```
A first-party frame's origin is `127.0.0.1:5137` (non-empty) → header stamped → `wallet_backup` 403s, identity-key prompts, gates fire. **Fix:** omit the header when `IsInternalOrigin(origin)` is true (internal = wallet-internal = no domain). One conditional. This is the entire trust-boundary change; the escalation door never existed because origin (not message/role) decides, and a website's origin is external.

> **Pre-existing iframe caveat (NOT introduced here, do not regress):** `IsInternalOrigin` returns true for a `127.0.0.1:5137` frame. The shim is gated to main-frame + https external pages (`simple_render_process_handler.cpp:887-894`), and the frontend should keep `frame-ancestors`/X-Frame-Options so a remote page can't iframe our UI and inherit internal trust. Tracked, not in scope.

---

## 3. Current reality (verified 2026-06-24, file:line)

- **Frontend:** ~77 inline `fetch('http://127.0.0.1:31301/...')` across ~14 source files (+4 CLAUDE.md docs, out of scope). No shared base constant. `App.tsx:130-135` documents direct-to-Rust.
- **Existing bridge (REUSE):** `__hodos_walletCall`/`__hodos_walletResponse` (`CWIShimScript.h:74,109`), `wallet_call` dispatch (`simple_handler.cpp:1886-1938`), `HandleIpcWalletCall` (`HttpRequestInterceptor.cpp:2099`), `IsInternalOrigin` (`:1012`), `runIpcCallDirect` (`:1861`, off-UI-thread `TID_FILE_USER_BLOCKING`→`CefPostTask(TID_UI)`; frame-guarded `sendWalletResponseIpc:1812`), `wallet_response`→`__hodos_walletResponse` render delivery (`simple_render_process_handler.cpp:1064-1085`).
- **Injection gap:** `CWI_SHIM_SCRIPT` (which *contains* the `__hodos_walletCall` IIFE) is injected **only for external https main frames** (`simple_render_process_handler.cpp:886`). **First-party/overlay pages do NOT have `__hodos_walletCall` yet** → must extract the transport IIFE and inject it for internal+overlay pages (provider object stays external-only).
- **Render wall:** `wallet_response` delivery is a single `ExecuteJavaScript` of the full escaped payload (`:1077-1084`) — existing code, shared with the website path. Large first-party responses (`/wallet/export`, `/wallet/recover*`) hit this.
- **Rust monitor self-call (SEV-1):** `monitor/task_backup.rs:71` hardcodes `http://127.0.0.1:31301/wallet/backup/onchain` (the only Rust self-call; grep-confirmed). Dev monitor would hit the prod wallet.
- **OQ-4 → SEV-1 (exfiltration):** `wallet_export` (`handlers.rs:16388`) and `wallet_import` (`:16454`) take `(state, body)` with **no `http_req`** → **no `X-Requesting-Domain` guard**, unlike `wallet_backup`/`wallet_restore`. `__hodos_walletCall` is reachable by approved websites → an approved dApp could `__hodos_walletCall('export','/wallet/export',{password},'POST')` and exfiltrate an encrypted backup it can decrypt. **Pre-existing; fix in commit 1.** (Broader observation: external origins can call arbitrary endpoints via the raw bridge — the per-endpoint header-guard is the mitigation; a future endpoint allowlist for external origins is worth considering. Flag for owner.)
- **C++ own-calls + recognition sites (port):** ~35 × `31301` + ~6 × `31302`. Host-form split: `simple_handler.cpp:7650` (`127.0.0.1:31301` bypass) vs `:7751` (`localhost:31301` install gate); legacy `:7752-7754` are `localhost:3321/2121/8080` (NOT wallet/adblock — out of scope). Other literal sites: `runIpcCallDirect:1870`, `redirectPort:3667-3668`, `isSocketIOConnection:5023`, `.well-known/auth:3693-3700`, auto-launch/health/shutdown `cef_browser_shell.cpp:3301/3405/3510` (+ mac `_mac.mm:4972/5113`), `AdblockCache.h`, `WalletService.cpp`/`_mac`, `BRC100Bridge.cpp`, `IdentityHandler.cpp`.
- **HODOS_DEV not fail-closed:** installed binary with `HODOS_DEV=1` env not positively refused (`AppPaths.h` — verify anchor before relying).

---

## 4. Architecture (reuse-based)

```
React:  walletFetch('/wallet/balance', { method, body, signal })          ← fetch-like shim (1 new file)
          → window.hodosBrowser.wallet.request(method, endpoint, body, httpMethod)
            → window.__hodos_walletCall(...)   ← EXISTING bridge (CWIShimScript transport IIFE)
              → cefMessage.send('wallet_call', [reqId, method, endpoint, body, httpMethod])   ← EXISTING
   C++ (EXISTING path):
      HandleIpcWalletCall(origin = frame->GetURL() host[:port]):
        IsInternalOrigin(origin)?  → runIpcCallDirect  → [FIX: omit X-Requesting-Domain] → SyncHttpClient to WalletPort()
        else (website)             → wallet-exists → domain trust → Rust domain_trust_gate (unchanged)
      → wallet_response IPC → __hodos_walletResponse → resolve/reject the pending promise
```

**Frontend gets the bridge:** `__hodos_walletCall` is the transport. The shim is injected external-only today, so extract its transport IIFE into a standalone script and inject it on internal+overlay contexts (the `window.CWI/yours/panda` provider object stays external-only — first-party UI doesn't need a dApp provider).

**Response shim:** `walletFetch(path, init)` returns a `Response`-like object (`ok`, `status`, `json()`, `text()`) so the 77 swaps are mechanical; it must honor `init.signal` (reject pending promise with `name='AbortError'` and drop the late `wallet_response`) for the debounced `AbortController` sites (`components/TransactionForm.tsx:116-132`).

**Large payloads:** the existing 50MB *request* ceiling stands. For large *responses* (`/wallet/export`, `/wallet/recover*`), the single-`ExecuteJavaScript` render wall (`:1077-1084`) must be bounded — deliver in ≤256KB slices that JS concatenates (or stash on a `CefV8Value`) so no single compile ≈ payload length. This modifies the **existing** `wallet_response` path (blast radius: also affects website large responses — currently they'd block too).

---

## 5. Commit breakdown & size (collapsed from 10 → 7)

| # | Commit | Layer | Notes | Risk |
|---|---|---|---|---|
| 1 | Rust port gate + self-call + **OQ-4 guard** | Rust | `wallet_port()`/`wallet_self_port()`/`adblock_port()` gated on `HODOS_DEV`; `main.rs` bind+logs; `adblock main.rs` bind; `task_backup.rs:71`; **add `http_req` + `X-Requesting-Domain` 403 guard to `wallet_export`+`wallet_import`** (mirror `wallet_restore`). Grep-gate: no `31301/31302` literal left in `rust-wallet/src`. | **med** (money-path) |
| 2 | C++ port: `IsWalletHostPort()` matcher (both host forms) + `WalletPort()`/`AdblockPort()`; migrate ALL ~41 sites incl. `runIpcCallDirect:1870`, `redirectPort:3667-3668`, `7650`, `7751` (match both forms), `5023`, `.well-known/auth`, auto-launch `3301/3405/3510` (+mac). Grep-gated. | C++ | cross-platform | **high** (routing) |
| 2b | Fail-closed `HODOS_DEV` = shared `is_dev()` (`HODOS_DEV=1` AND build-dir) consumed by both data-dir + port selection | Rust+C++ | release safety | med |
| 3 | **Trust fix** (was the big proxy): `runIpcCallDirect` omit `X-Requesting-Domain` when `IsInternalOrigin(origin)` **+ extract `__hodos_walletCall` transport IIFE → inject on internal+overlay pages** | C++ | ~30 lines + injection split | **high** (trust boundary) |
| 4 | Bounded large-response delivery on the existing `wallet_response` path (≤256KB slices / V8Value) for export/recover | C++ + JS | modifies live Phase-2.5 code | **high** (data integrity) |
| 5 | Frontend: `wallet.request` → `__hodos_walletCall` + `walletApi.ts` Response shim (incl. `signal`/abort); migrate 77 sites (file-by-file, `tsc`-gated); confirm no `31301` literal remains | Frontend | mechanical + abort handling | low-but-broad |
| 6 | macOS parity: injection split + port matcher + any mac-only literals (`_mac.mm`) | C++ (mac) | mirror of 2-4 | med |
| 7 | Docs: flip both frontend CLAUDE.md "never call Rust directly" invariants to true; root CLAUDE.md Key Files + port table; this doc → DONE | docs | invariant #11/#12 | low |

**Honest size:** ~7 commits, multi-session. The collapse vs the first plan: the "new generic proxy" commit became the few-line `runIpcCallDirect` fix + an injection split (commit 3); the JS bridge + promise correlation + 50MB cap already exist (no commit 4/5 rebuild). Genuinely hard commits remain **3 (trust boundary)** and **4 (large-response delivery)**; the rest is plumbing/mechanical. New code is now dominated by the 77-site frontend swap + the port matcher.

---

## 6. Risk register

| Risk | Sev | Mitigation |
|---|---|---|
| `runIpcCallDirect` stamps header on first-party → backup 403 / identity prompt / gates | SEV-1 | Commit 3 omit-when-internal; §7 re-verifies backup + identity before/after |
| Rust monitor self-call hits prod wallet in dev | SEV-1 | Commit 1 `wallet_self_port()` + grep-gate + dev-targets-31401 test |
| Approved dApp exfiltrates wallet via `__hodos_walletCall('export'/'import')` | SEV-1 | Commit 1 OQ-4 guard (mirror `wallet_restore`); §7 website-export attack test |
| Large export response funneled through one full-payload `ExecuteJavaScript` → block/OOM | SEV-2 | Commit 4 bounded ≤256KB slice delivery |
| Host-form split (`localhost:` vs `127.0.0.1:`) → bare port swap drops interception | SEV-2 | Commit 2 `IsWalletHostPort` both forms; grep-gate |
| Website reaches `wallet_call` and is mis-trusted | (closed) | Origin-based gate already handles it (origin un-forgeable); §7 attack test confirms |
| Debounced AbortController site resolves a superseded promise | SEV-3 | Shim honors `signal`, rejects `AbortError`, drops late `wallet_response` |
| Dev browser auto-launches prod-port wallet | SEV-3 | Commit 2 auto-launch literals → `WalletPort()`; `EnforceDevSafeguard` blocks DB corruption |
| Installed build + `HODOS_DEV=1` env binds dev port | SEV-3 | Commit 2b fail-closed `is_dev()` |
| Startup-latency regression (~2s freeze) | SEV-2 | Fire-and-forget IPC (existing pattern); measure `headerPainted` ~200-400ms |
| iframe origin-confusion (pre-existing) | note | keep `frame-ancestors`; not in scope, don't regress |
| macOS divergence | SEV-2 | Commit 6 parity + compile/smoke |

---

## 7. Test matrix (must pass before merge)

**Unit:** Rust `wallet_port`/`adblock_port`/`wallet_self_port` = 31401/31402 iff dev; C++ `IsWalletHostPort` recognizes both host forms in dev (31401) + prod (31301).

**Trust-boundary (byte-identical before/after):**
- `wallet_backup`/`wallet_restore` from first-party UI → succeed; from website → still 403.
- **Attack:** website tab calling `__hodos_walletCall('export','/wallet/export',{password},'POST')` → **rejected** (commit 1 guard); and `…'/wallet/backup'` → rejected; confirm Rust sees the website domain header (external), never header-absence.
- `getPublicKey({identityKey:true})` first-party → no prompt; website → prompts.
- First-party gated endpoint → passes ungated; unapproved website → 403/202.
- Rust logs: **zero** `X-Requesting-Domain` on first-party `wallet_call`s.

**Port completeness:** grep `rust-wallet/src`, `cef-native`, `frontend/src` → no `31301/31302` literal outside helper/matcher; dev wallet on 31401 → browser skips auto-launch; installed-path + `HODOS_DEV=1` → must NOT bind 31401/use `HodosBrowserDev`.

**Payload/integrity:** `/wallet/export` (multi-MB) round-trips; re-import + diff DB equality; no single `ExecuteJavaScript` ≈ payload length (measure render RSS); `/wallet/recover*` large sets intact.

**Threading/perf:** UI responsive during a throttled wallet call; `headerPainted` ~200-400ms.

**Simultaneous run (the goal):** installed (31301) + dev (`HODOS_DEV=1`, 31401) at once; ops hit own backend; dev TaskBackup targets 31401; dev DB `HodosBrowserDev`, prod `HodosBrowser`.

**Smoke (CLAUDE.md Standard):** wallet create/load, balance, send, approved-sites, settings, peerpay; main view + each overlay; Windows + macOS.

---

## 8. Open questions

- **OQ-1 (DECIDED):** full bridge ships **before** `0.3.0-beta.16`.
- **OQ-2 (RESOLVED):** large-response delivery = bounded ≤256KB slices; streaming-to-disk out of scope; cap is the mitigation.
- **OQ-3:** re-base existing typed `wallet.*` methods (`initWindowBridge.ts`) onto `__hodos_walletCall`? **Defer** (limit blast radius).
- **OQ-4 (RESOLVED → commit 1):** `wallet_export`/`wallet_import` get the `wallet_restore`-style hard-wall. ⚠️ **Security change to money-layer Rust made while owner away — flagged for review.** Broader: consider an external-origin endpoint allowlist (future).

---

## 9. Provenance

3-agent review + workflow `wf_d67e8bd5-37e` (runs 1+2; verdict GO-WITH-EDITS then the reuse pivot). Per-finding verify agents were repeatedly rate-limited (transient server limit); the referee verified anchors by direct code-reading, and **all load-bearing anchors in §0/§2/§3 were personally re-verified** against current code before this rewrite (wallet_call dispatch, IsInternalOrigin, runIpcCallDirect stamp+port, injection external-only gate, export/import missing guard, restore guard present, task_backup self-call). Further red-team runs are blocked by the active rate limit and have diminishing returns post-pivot.
