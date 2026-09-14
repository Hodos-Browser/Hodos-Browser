# Phase 8d — a dead wallet backend is noticed, named truthfully, and brought back · PHASE CONTRACT

**Workstream:** money-path correctness (availability half) · **Ticket:** `../TICKET_wallet_backend_death_is_silent_and_unrecovered.md`
**Status:** 🚧 **STAGE 1 (truth) IN PROGRESS** — §8 answered by the owner 2026-09-14: Q1 **both**, Q2 **`WALLET_UNAVAILABLE`**, Q3 **adblock restart-only**; Q4/Q5 stand as stated assumptions.
**Opened:** 2026-09-14 · **Owner:** Matthew Archbold · **Platforms:** 📏 **both** — Windows first, macOS half relayed (`MAC_RELAY_P35_P4_ROUND.md` M11 already told Mac it is theirs too)
**Standard:** `../HARNESS.md`. **Base:** the 8c close-out (`O11`/`O12` commit on `origin/0.4.0`).

> ⚠️ Kickoff method: every claim below is a **code read on today's tree** (symbols, not line numbers),
> except where marked 📏 measured. Nothing has been built.

---

## 0. Plan-vs-tree delta — the ticket's three findings hold, and it under-counts the "no wallet" sites

### 0.1 ✅ `D-1` — F1 holds on both platforms: spawn once, never watched

Windows: `cef_browser_shell.cpp :: LaunchWalletProcess` — `IsPortListening` fast-path (dev mode), else
`CreateProcessA` + `AssignProcessToJobObject`; the handle sits in `g_walletServerProcess` and is only read
again by `StopWalletServer`. macOS: `cef_browser_shell_mac.mm :: SpawnWalletServer` — `posix_spawn` into
`g_wallet_server_pid`, only read again by the SIGTERM at shutdown. Neither has a watcher. The adblock
engine is the same shape on both (`LaunchAdblockProcess` / `SpawnAdblockServer`).

### 0.2 ✅ `D-2` — F2 holds, Windows-only, and the flag has almost no readers

`WaitForWalletHealth` sets `g_walletServerRunning = true` on the **unhealthy** path (*"Process was
launched, just slow to start"*). macOS leaves it `false` and logs *"health check timed out"*.
📏 But `g_walletServerRunning` is read by **one** live decision on Windows — `StopWalletServer`'s early
return — and by nothing on any request path. ⇒ "make the flag honest" is necessary hygiene, but it fixes
**no user-visible behaviour by itself**. The honest state has to reach the three places in `D-4`.

### 0.3 ✅ `D-3` — F3 holds: the `WalletService` daemon API is dead and stays out of scope

`startDaemon` / `stopDaemon` / `isDaemonRunning` / `monitorDaemon` — every call site is inside
`WalletService.cpp` itself (plus the mac stubs). Not touched; ticket §8 stands.

### 0.4 🚨 `D-4` — "no wallet" is manufactured in THREE places, and the worst one is our own wallet panel

The ticket describes the dApp-facing message. The tree has three independent sites that turn a
**transport failure** into **"no wallet"**:

| # | Where | What happens with the wallet dead | Who sees it |
|---|---|---|---|
| (a) | `HttpRequestInterceptor.cpp :: WalletStatusCache::walletExists()` | `fetchWalletStatus()` already classifies **`FetchFailed`** separately from `DoesNotExist` (different TTLs: 2 s vs 30 s) — and then `walletExists()` **collapses both to `false`**. Three callers act on it: the IPC gate (`"No wallet exists. Please create or recover a wallet first."`, code `NO_WALLET`), the BRC-100 gate (same text **plus** the `no_wallet` notification overlay), and the BRC-121 402 path | every dApp |
| (b) | `simple_handler.cpp :: wallet_status_check` | starts from `{exists:false, needsBackup:true}` and only overwrites it if the wallet answers ⇒ `hodosBrowser.wallet.getStatus()` reports **no wallet** on any transport failure | header / App |
| (c) | 🚨 `WalletPanelPage.tsx` mount + `wallet_shown` handlers | `walletFetch('/wallet/status')` → `.catch(() => setWalletStatus('no-wallet'))`; and a body without `exists` (which is what the interceptor's outer timeout returns — `{"error":"Wallet request timeout"}` **as HTTP 200**) takes the same `else` ⇒ **`renderNoWallet()` — the CREATE / RECOVER screen**, and `localStorage.hodos_wallet_exists` is **removed**, so the next open paints "no wallet" before even asking | the user, in our own UI |

(c) is ticket §3's hazard made concrete: with the backend merely dead, the wallet panel itself offers the
user *"recover from your phrase"*. ⭐ The classification the fix needs **already exists** at (a) — it is
thrown away one line later.

### 0.5 📏 `D-5` — restart primitives exist; a "relaunch" is one call on each platform, with one leak to avoid

Windows: `LaunchWalletProcess()` is re-callable (port check → resolve exe → `CreateProcessA` → new job
object), but it **overwrites `g_walletJobObject`** without closing the previous handle — a relaunch path
must close it first. Death detection: `WaitForSingleObject(g_walletServerProcess.hProcess, 0) ==
WAIT_OBJECT_0`. macOS: `waitpid(g_wallet_server_pid, &status, WNOHANG)`, then `SpawnWalletServer()`.
⛔ **Dev mode** (wallet started by `cargo run`, `IsPortListening` fast-path): there is **no handle** —
the supervisor can only *report*, never relaunch. That is correct behaviour, not a gap.

### 0.6 📏 `D-6` — there is a precedent for the poller's shape, and a fresh reason not to use `TID_FILE_*`

`cef_browser_shell.cpp` already runs a detached `std::thread` that polls `QuickHealthCheck()` /
`IsPortListening()` every 500 ms for up to 110 s (the post-update health probe, 6d) — *"touches no CEF"*.
The supervisor is that loop made permanent, at a slower cadence. ⛔ It must **not** be a
`CefPostDelayedTask(TID_FILE_*)` loop: 8c `O12` established that all three file ids are **one shared
thread** in this process, and a 2 s probe timeout there stalls balance/cookie/adblock tasks behind it.

### 0.7 ⭐ `D-7` — prior art (rule 5), one minute each

| Source | What they do | Take |
|---|---|---|
| **Chromium** — network / utility service processes | Crash ⇒ restart **on next use**, with a per-process crash counter; after N crashes in a window the service stays down and the UI degrades | ⭐ the **bounded** part. No hot loop |
| **Brave** — Tor client launcher | Watches the child; relaunches with backoff on unexpected exit; the UI shows *"Tor is unavailable"* rather than pretending | ⭐ closest analogue: a privacy-critical helper whose absence must be **named**, not hidden |
| **Electron** `utilityProcess` | Emits `exit` with a code; the app decides whether to `fork` again | Confirms "the embedder owns the policy" — CEF gives us no supervision for our own children |

We follow Brave's shape: watch, bounded relaunch with backoff, and an explicit *unavailable* state in the
UI. Logged in `../../PRIOR_ART.md` when the code lands.

## 1. Goal

If the wallet backend is not reachable, the user is told **"the wallet service is not running"** — never
"no wallet" — and the browser brings the service back on its own within a bounded number of attempts.

## 2. Done means

- [ ] **The wallet panel never shows create/recover because of a transport failure.** With the backend
      stopped it shows *"Wallet service not running"* with a **Restart** action. `hodos_wallet_exists`
      is not cleared by a transport failure.
- [ ] **dApps are told the truth**: a wallet call with the backend down gets code `WALLET_UNAVAILABLE`
      (not `NO_WALLET`) and the notification overlay says the service is down, not that no wallet exists.
- [ ] `hodosBrowser.wallet.getStatus()` distinguishes *unreachable* from `{exists:false}`.
- [ ] **Supervision**: when the child we launched exits, it is relaunched — bounded retries (3) with
      backoff (2 / 4 / 8 s); after that the state stays *not running* with the manual Restart still live.
      ⛔ Never a hot loop.
- [ ] Windows `g_walletServerRunning` is honest: `false` on the unhealthy startup path (macOS shape),
      `false` again when the child dies.
- [ ] The adblock engine gets the same watcher, **restart-only** (no UI state).
- [ ] Startup first paint is **unchanged** — the supervisor starts after the existing detached health
      threads, and the first probe is not before the existing `WaitForWalletHealth` finishes.
- [ ] macOS half relayed with the `#ifdef` split named.

## 3. Invariants preserved

| ID | Invariant | Why this phase could break it |
|---|---|---|
| `R-INTEXT` | Internal never prompts, external always gates | (a)'s three early-returns sit **inside** the gated paths. Changing what they return must not change *whether* a request is gated or which origin it is attributed to |
| `R-GOLD` | Gold pill on auto-approved payment | Untouched by design — the pill fires on wallet-call success, which this phase never reaches |
| `R-CLOSE` | Overlay close guards | The panel gains a state (`service-down`). It must **allow close** (`wallet_allow_close`) like `no-wallet` does — a stuck-open panel on a dead backend is a new defect |
| P2 (`M5`) | A hung wallet never freezes the UI | The supervisor's probe blocks — it runs on its own `std::thread`, never on `TID_UI` or `TID_FILE_*` (`D-6`) |
| Startup | First paint / wallet launch off the critical path | Measured before and after (P3's `CDP first-paint` instrument) |
| Rule 6 | The instrument is not edited by the change it measures | No gate baselines move in this phase |

## 4. Evidence table — every row GREEN **and** RED, subject named

| Row | Assertion | Subject | RED |
|---|---|---|---|
| `P8d-A1` | Backend stopped (dev wallet by exe path) ⇒ wallet panel shows *service not running* + Restart, **not** create/recover; `hodos_wallet_exists` still set | `WalletPanelPage` state, read via CDP on the wallet overlay | revert (c) ⇒ `renderNoWallet()` |
| `P8d-A2` | Backend stopped ⇒ a dApp wallet call gets `WALLET_UNAVAILABLE`; the notification overlay names the service | the IPC gate's reply body + `BRC100AuthOverlayRoot` type | revert (a) ⇒ `NO_WALLET` + `no_wallet` |
| `P8d-A3` | Backend stopped ⇒ `wallet.getStatus()` says unreachable | the bridge reply | revert (b) ⇒ `{exists:false}` |
| `P8d-A4` | Kill the child we launched (⚠️ **by exe path**) ⇒ relaunched, `/health` back, within ≤ 5 s; wallet panel returns to its live state without a click | `g_walletServerProcess` PID before/after + `/health` | supervisor disabled (rig env `HODOS_NO_SUPERVISE=1`, read once in the browser process) ⇒ never relaunched |
| `P8d-A5` | Exe renamed away ⇒ exactly **3** relaunch attempts (2 / 4 / 8 s), then a stable *not running* state; the Restart button tries once more and fails visibly | log lines + PID | remove the bound ⇒ attempts keep coming |
| `P8d-A6` | Windows: unhealthy startup ⇒ `g_walletServerRunning` **false** (matches macOS) | the flag, via the log line that reports it | old line ⇒ `true` |
| `P8d-A7` | First paint unchanged with the supervisor on | P3's first-paint instrument, 8-run medians | — (a regression IS the red) |
| `P8d-A8` | 🍎 macOS: A1 + A4 | relay | — |

## 5. Blast radius

`cef_browser_shell.cpp` (supervisor thread, F2 line, relaunch helper; Windows) ·
`cef_browser_shell_mac.mm` (mac half — **Mac's lane**) · `HttpRequestInterceptor.cpp` (`WalletStatusCache`
exposes the third state; three `walletExists()` callers) · `simple_handler.cpp` (`wallet_status_check`
reply; a `wallet_restart` IPC for the button) · `WalletPanelPage.tsx` (new state + Restart) ·
`BRC100AuthOverlayRoot.tsx` (notification type `wallet_unavailable`) · `hodosBrowser.d.ts` (status type).
Adblock: the same watcher, `LaunchAdblockProcess` / `SpawnAdblockServer`.

## 6. Out of scope

⛔ The `WalletService` daemon API (`D-3`) — delete-or-adopt is its own decision. ⛔ A hung-but-alive
wallet (P2's territory; the probe sees it as *up*). ⛔ The residual from 8c `O12` (balance / address /
cookie / adblock tasks sharing one file thread) — noted here because it is the same family, but it is a
threading change, not supervision; owner to place it.

## 7. Staging

1. **Truth first** (`D-4` a/b/c + `A1`–`A3`): no supervisor yet. This alone removes the recovery-phrase
   hazard and is the half that protects the user.
2. **Supervision** (`A4`–`A6`): the watcher thread, bounded relaunch, Restart IPC, F2 line.
3. **Adblock** on the same watcher, then the relay note for Mac.

Each stage its own commit, own preflight `-Full`, own relay line.

## 8. Open, and the owner's to answer

| # | Question | My recommendation |
|---|---|---|
| **Q1** | Restart policy: automatic bounded relaunch **and** a manual Restart button, or manual only? | ⭐ **Both.** Auto covers the crash-at-2 a.m. case; the button is the honest fallback when the bound is hit (exe quarantined, port taken) |
| **Q2** | What the dApp sees when the service is down | New code `WALLET_UNAVAILABLE`, text *"Hodos wallet service is not running."* BRC-100 has no code for this; a distinct code is what lets a dApp stop saying "install a wallet" |
| **Q3** | Adblock in scope? | ⭐ Yes, restart-only. Same watcher, ~20 lines, and a user will never diagnose "ads came back" |
| **Q4** | Dev mode (wallet not our child) | Report only, never relaunch. **Assumed**; say so if you disagree |
| **Q5** | The 8c `O12` residual (one shared file thread) | Leave it out of 8d; open a ticket. It is real but no user-visible defect is measured yet |

✅ **Answered 2026-09-14** — Q1 both · Q2 `WALLET_UNAVAILABLE` · Q3 yes, restart-only. Q4 (dev mode: report only) and Q5 (O12 residual → its own ticket) were stated as assumptions and not objected to.
