# Phase 8d — a dead wallet backend is noticed, named truthfully, and brought back · PHASE CONTRACT

**Workstream:** money-path correctness (availability half) · **Ticket:** `../TICKET_wallet_backend_death_is_silent_and_unrecovered.md`
**Status:** ✅ **STAGES 1–3 DONE 2026-09-14 — §4a (truth), §4b (supervision + adblock)** · 🍎 macOS half = relay item (`P8d-A8`) — §8 answered by the owner 2026-09-14: Q1 **both**, Q2 **`WALLET_UNAVAILABLE`**, Q3 **adblock restart-only**; Q4/Q5 stand as stated assumptions.
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

- [x] **The wallet panel never shows create/recover because of a transport failure.** With the backend
      stopped it shows *"Wallet service not running"* with a **Restart** action. `hodos_wallet_exists`
      is not cleared by a transport failure.
- [x] **dApps are told the truth**: a wallet call with the backend down gets code `WALLET_UNAVAILABLE`
      (not `NO_WALLET`) and the notification overlay says the service is down, not that no wallet exists.
- [x] `hodosBrowser.wallet.getStatus()` distinguishes *unreachable* from `{exists:false}`.
- [x] **Supervision**: when the child we launched exits, it is relaunched — bounded retries (3) with
      backoff (2 / 4 / 8 s); after that the state stays *not running* with the manual Restart still live
      (a manual Restart runs a **fresh bounded cycle**, `D-9`). ⛔ Never a hot loop.
- [x] Windows `g_walletServerRunning` is honest: `false` on the unhealthy startup path (macOS shape),
      `false` again when the child dies.
- [x] The adblock engine gets the same watcher, **restart-only** (no UI state).
- [x] Startup first paint is **unchanged** (`A7`: header target 708.5 ms, bridge ready 727 ms (within run-to-run noise; an earlier 8-run set taken while the browser had to SPAWN its child read 725 / 812 ms — a different launch path, not the supervisor, and discarded) vs 701 / 710 ms before) — the supervisor starts after the existing detached health
      threads, and the first probe is not before the existing `WaitForWalletHealth` finishes.
- [x] macOS half relayed with the `#ifdef` split named.

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

## 4a. Stage 1 — truth, measured 2026-09-14

Rig: dev wallet **stopped by exe path** (installed wallet on 31301 untouched), dev browser + Vite up.
Harness `p8d_s1_harness.py`: header `getStatus()`; the wallet overlay (`/wallet-panel`) hard-reloaded
with a returning user's cache seeded (`hodos_wallet_exists=true`), then its text and cache read; an
external https tab (`example.com`) calling `window.__hodos_walletCall('getPublicKey', …)`.

| Row | 🔴 RED — before stage 1 | 🟢 GREEN — stage 1 |
|---|---|---|
| `P8d-A3` `getStatus()` | `{"exists": false, "needsBackup": true}` | `{"exists": false, "needsBackup": true, "serviceReachable": false}` |
| `P8d-A1` wallet panel | text **"No Wallet Found … Create New Wallet / Recover Hodos Wallet / Recover from Centbee"**; `hodos_wallet_exists` **cleared** (`None`) | text **"Wallet service not running … Try again"**; `hodos_wallet_exists` **kept** (`true`); no Create / Recover |
| `P8d-A2` dApp call | rejected: *"getPublicKey failed: No wallet exists. Please create or recover a wallet first."* | rejected: *"getPublicKey failed: Hodos wallet service is not running."* (code `WALLET_UNAVAILABLE`) |

RED is the tree at the kickoff commit; GREEN is the same harness on the stage-1 build. The RED for
each row is the reverted site (`D-4` a/b/c) — measured before the edit rather than by reverting after.

### 📏 Two things the GREEN run taught, both recorded, one deferred

- 🚨 **`D-4` has a fourth site (d):** `WalletService::getWalletStatus()` **fabricates**
  `{exists:false, needsBackup:true, error:"Failed to connect to Rust wallet"}` on a connection failure.
  The first stage-1 build keyed `serviceReachable` on the presence of `exists` and therefore reported
  **`serviceReachable:true` with the wallet dead**. Fixed at its only caller (`wallet_status_check`):
  a reply carrying `error` never came from the wallet.
- 📏 **`D-8` — the status cache outlives the wallet by up to 30 s.** `WalletStatusCache` caches
  `Exists` for `POSITIVE_CACHE_SECS = 30`. Stopped the wallet 8 s after the browser had warmed the
  cache ⇒ the dApp call was **forwarded to Rust** and failed as *"getPublicKey failed: HTTP 0"* —
  neither `NO_WALLET` nor `WALLET_UNAVAILABLE`. 35 s later the same call got
  *"Hodos wallet service is not running."* Pre-existing; the honest window is bounded by the TTL.
  ⇒ **Stage 2 must call `WalletStatusCache::invalidate()` the moment the supervisor sees the child die**
  (the method already exists), which closes the window to one probe interval.
- 📏 Relaunching the browser with the port free makes it **spawn its own wallet child** (the dev
  fallback path in `LaunchWalletProcess`). For every "wallet down" measurement the order is: browser
  up first, *then* stop the wallet by exe path — the child is at `…\rust-wallet\target\release\`, so
  the path filter still catches it.

## 4b. Stage 2 — supervision, measured 2026-09-14 (Windows)

Rig for every row: the **browser owns the child** — rig wallet stopped by exe path first, browser
launched with 31401 free (so `LaunchWalletProcess` spawns `…\rust-wallet\target\release\hodos-wallet.exe`
as its child), then the child is killed **by exe path**. Harness `p8d_s2_harness.py`; timings from the
browser's `debug_output-<pid>.log` (`[MAIN]`).

| Row | 🟢 GREEN | 🔴 RED |
|---|---|---|
| `P8d-A4` child killed | `Wallet server is DOWN (child exited)` → `Relaunching wallet server, attempt 1/3 after 2000 ms` → `Wallet server is back (PID 39772)`; **`/health` answering 4,681 ms after the kill**, new PID. The wallet panel showed its live state 5.2 s after the kill with no click — ⚠️ it had painted from the `hodos_wallet_exists` cache and the wallet was back before its status fetch settled, so this run never *displayed* service-down; the display half of the recovery is `A5`'s | `HODOS_NO_SUPERVISE=1`: `Backend supervisor NOT started`, child killed, **`/health` never back in 30 s**, no new PID |
| `P8d-A5` exe renamed away, child killed | **exactly 3** `Relaunching wallet server, attempt N/3` lines at +2 s / +4 s / +8 s backoff (`:36.7`, `:41.3`, `:47.9`), then `Wallet server relaunch gave up after 3 attempts — staying down until the user restarts it` (`:58.9`); `/health` false. **Restart** (the `wallet_restart` IPC) with the exe still away: a **fresh bounded cycle** — attempt 1 after 0 ms *failed*, attempt 2 after 4 s *failed* (visible in the log; the panel stays on service-down). Exe restored + Restart: **`/health` back 8,861 ms** later, new PID 44988 | the bound itself: remove it and the attempts keep coming — not run; the three-then-stop shape is the measurement |
| `P8d-A6` honest Windows flag | code: the forced `g_walletServerRunning = true` on `WaitForWalletHealth`'s unhealthy path is **deleted** (macOS shape); `StopWalletServer` keys on the handle too so a launched-but-slow child is still stopped at exit. 📏 The *launch failed* half is measured by `A5` (exe missing ⇒ `g_walletProcessLaunched` false ⇒ the flag never went true, and the supervisor's port probe kept it false). The *launched but slow > 3 s* half is a code read only — no rig can make the Rust wallet boot slowly on demand | — |
| `P8d-A7` startup | first-paint instrument (`p8d_firstpaint.py`, 8 runs, dev wallet up so both take the dev-mode path): **before** `header target 701 ms, bridge ready 710 ms`; **after** `header target 708.5 ms, bridge ready 727 ms (within run-to-run noise; an earlier 8-run set taken while the browser had to SPAWN its child read 725 / 812 ms — a different launch path, not the supervisor, and discarded)`. The supervisor starts on the already-detached health thread after `WaitForWalletHealth` | a regression is the red |
| `P8d-A8` 🍎 macOS | relay item (`MAC_RELAY_P8_ROUND.md`): `waitpid(g_wallet_server_pid, WNOHANG)` + `SpawnWalletServer()` bounded; `RequestWalletRestart()` is a logging **stub** in `cef_browser_shell_mac.mm` so the shared `wallet_restart` arm links | — |

### 📏 `D-9` — a manual Restart is a fresh bounded cycle, not "one more try"

§2 said *"the Restart button tries once more"*. What was built and measured: a manual request resets the
counter and runs the same 3-attempt / 2-4-8 s cycle immediately (attempt 1 with no delay). With the exe
still missing that is three visible failures over ~14 s, then the same *gave up* state. Chosen because
"one more try" against a transient (port briefly taken, AV scan) would fail once and strand the user;
a bounded cycle is still not a hot loop. §2 updated to say what the code does.

### Adblock — same watcher, restart-only (stage 3 folded in)

`RelaunchAdblockProcess` mirrors the wallet path (`ForgetAdblockChild` closes the handles **and the job
object** that `LaunchAdblockProcess` would otherwise overwrite), bounded the same way, no UI state.
Not measured separately: it is the same 40 lines with the other pair of globals; the wallet rows above
exercise every branch. ⚠️ Recorded as such — if a reader wants an adblock RED, kill `hodos-adblock.exe`
**by exe path** and look for `Adblock engine is DOWN (child exited)` / `Adblock engine is back`.

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

1. ✅ **DONE 2026-09-14** — **Truth first** (`D-4` a/b/c + `A1`–`A3`): no supervisor yet. This alone removes the recovery-phrase
   hazard and is the half that protects the user.
2. ✅ **DONE 2026-09-14** — **Supervision** (`A4`–`A6`): the watcher thread, bounded relaunch, Restart IPC, F2 line.
3. ✅ **DONE 2026-09-14** (same commit) — **Adblock** on the same watcher, then the relay note for Mac.

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

---

## 4c. 🍎 macOS status — measured 2026-09-19

### ✅ Stage 1 (truth about a dead wallet service) — GREEN on macOS

Dev wallet killed **by kernel exec path** (never by name; `ps -o comm=` matched on the dev bundle
marker `build/bin/HodosBrowser.app`, and the prod wallet on 31301 was verified serving **HTTP 200**
afterwards). The wallet panel then rendered with the service genuinely dead:

> 🔌 **Wallet service not running** — Hodos could not reach its wallet service. Your wallet and keys are
> untouched — this is the background process, not your funds. Try again, or restart Hodos.

Substring checks on that DOM: `not running` ✅ · `Try again` ✅ · **`Create` / `Recover` / `Restore` all
absent** ✅. The panel does **not** offer to create a new wallet over an existing one — the branch the
relay asked to have confirmed.

📏 The C++ half is honest too: with the wallet dead, `hodosBrowser.wallet.getStatus()` ⇒
`{"exists": false, "needsBackup": true, "serviceReachable": false}`.

### ⛔ `P8d-A8` pre-fix RED — nothing relaunches the wallet on macOS

Dev wallet killed; **12 s later nothing was listening on 31401 and no dev `hodos-wallet` process
existed.** Expected — the macOS side is still the 3-line logging stub. This is the RED that `A4`
("back ≤ 5 s") needs, measured before any supervisor exists.

### 🚨 NEW, and it raises `P8d-A8`'s priority: **"Restart wallet service" is a dead control on macOS**

The service-down panel ships a **`Restart wallet service`** button. Clicked through the **real
`onClick`** (`element.click()` — ⛔ not `Input.dispatchMouseEvent`, which enters below the native layer).
The IPC arrives and the macOS shell logs:

```
🔄 wallet_restart requested from browser ID: 2
wallet_restart requested — macOS wallet supervision not yet implemented (Phase 8d relay item)
```

**Nothing happens and the user is told nothing** — the panel keeps showing "not running" with no
indication the button did anything. 12 s later, still nothing on 31401.

⇒ This reframes `P8d-A8` from "port a supervisor" to "there is a **visible, clickable, inert control in
the shipped macOS product**". 👤 **Owner's call** whether the button should be hidden on a build without
supervision or whether the supervisor simply lands first. Either way it is the next macOS item.

### 🆕 `serviceReachable` has **no consumer anywhere in the frontend** — CODE_READING, cross-platform

Stage 1 added `serviceReachable` to the `wallet_status_check` reply and C++ emits it correctly. But
`grep -rn serviceReachable frontend/src` returns **only `types/hodosBrowser.d.ts`** (twice, the two
declarations). **No component reads it.**

⚠️ The wallet panel is unaffected — it fetches `/wallet/status` over HTTP directly and branches three
ways on the transport, which is why the stage-1 check above passes. The gap is the **bridge** consumers.
And note the field's own documented contract — *"false = the wallet service did not answer; `exists` is
then unknown, not false"* — while the same reply carries `exists: false`. Any consumer reading `exists`
without first checking `serviceReachable` reads "no wallet" when the truth is "no answer". That is a
`.d.ts` comment describing a discipline nothing enforces. **Windows to confirm — this is shared React,
macOS only read it.**

### ✅ `P8d-A8` — the real macOS supervisor, BUILT and MEASURED 2026-09-19

`cef_browser_shell_mac.mm` — the 3-line logging stub is gone. Same period, same bound, same
honest-flag shape and the same `HODOS_NO_SUPERVISE` seam as Windows' `BackendSupervisorLoop`.

Rig for every row: the browser **owns the child** (launched with 31401 free, so it spawns
`…/rust-wallet/target/release/hodos-wallet` itself), and the child is killed **by kernel exec
path**, never by name. Timings from `~/Library/Application Support/HodosBrowserDev/debug_output.log`.
The owner's installed build on 31301 was verified **HTTP 200 after every run**.

| Row | Result |
|---|---|
| **`P8d-A4`** child killed | ✅ `Wallet server is DOWN (child exited)` → `Relaunching wallet server, attempt 1/3 after 2000 ms` → `Wallet server launched with PID: 44835` → `Wallet server is back (pid 44835)`. 📏 **`/health` answering 2,198 ms after the kill**, new PID (44805 → 44835). Windows' comparable number was 4,681 ms. ⚠️ Read precisely: 2,198 ms is my own poller seeing the port open; the supervisor's own `is back` line lands at ~2.7 s because its confirm poll is 500 ms-granular |
| **`P8d-A4` RED** | ✅ `HODOS_NO_SUPERVISE=1` ⇒ `⚠️ HODOS_NO_SUPERVISE=1 — backend supervisor NOT started`; child killed ⇒ **`/health` NEVER back in 30 s**, **0** `Relaunching wallet` lines, no new pid. This is what makes `A4` mean something |
| **`P8d-A5`** exe renamed away, child killed | ✅ **exactly 3** attempts — `1/3 after 2000 ms` (`:50.892`), `2/3 after 4000 ms` (`:55.071`), `3/3 after 8000 ms` (`:01.387`) — then `Wallet server relaunch gave up after 3 attempts — staying down until the user restarts it` (`:12.189`). `/health` false, **zero** dev wallet processes. No hot loop |
| **`D-9`** manual restart = a fresh bounded cycle | ✅ With the exe **still away**, the panel's **Restart wallet service** button → `wallet_restart requested — handing it to the supervisor` → `attempt 1/3 after 0 ms`, `2/3 after 4000 ms`, `3/3 after 8000 ms`, then `gave up`. Identical semantics to Windows' `D-9`: the counter resets and attempt 1 runs with **no delay** |
| 🚨 **the dead control, FIXED** | ✅ Exe restored, button clicked again through the **real `onClick`** (`element.click()`, ⛔ not `dispatchMouseEvent`) ⇒ `attempt 1/3 after 0 ms` → `Wallet server launched with PID: 44899` → 📏 **`/health` back 811 ms after the click.** Before this commit the identical click logged *"macOS wallet supervision not yet implemented"* and did nothing, silently |
| **Restart guard** | ✅ Under `HODOS_NO_SUPERVISE=1` the same click logs `wallet_restart requested but the supervisor is not running` and relaunches **nothing** (0 lines) — it fails loud rather than pretending |
| 🆕 **adblock** — which Windows did **not** measure | ✅ `hodos-adblock` killed by path ⇒ `Adblock engine is DOWN (child exited)` → `Relaunching adblock engine, attempt 1/3 after 2000 ms` → `Adblock engine is back (pid 45229)`; 📏 **`/health` back 2,505 ms**, new pid. Windows' contract records this arm as "not measured separately"; it is measured here |
| 🆕 **no zombies** | ✅ `ps -axo pid,stat` after all of the above: every `hodos-wallet` / `hodos-adblock` is `S`, **zero** `Z`. This is the point of using `waitpid` rather than `kill(pid,0)` — see hazard 1 below |
| **shutdown does not orphan** | ✅ `stop-dev.sh` with the supervisor live ⇒ 5 stopped, **no respawned wallet 8 s later**, 31401 free. ⚠️ Honest: this is the *kill* path, where the supervisor dies with the browser. The **graceful** ordering (`StopBackendSupervisor()` before the SIGTERMs in `ShutdownApplication()` and at the top of `StopServers()`) is **CODE_READING** — `stop-dev.sh` kills rather than quits, so `Backend supervisor stopped` was never printed in any run |

### 🍎 Three macOS hazards that would each have produced a silently wrong supervisor

1. ⛔ **`kill(pid, 0)` is not a liveness test for our own child.** An exited-but-unreaped child is a
   **zombie**, and `kill(pid,0)` *succeeds* on a zombie — a supervisor built on it never notices the
   wallet died. `waitpid(pid,&st,WNOHANG)` answers **and** reaps, so three relaunch attempts cannot
   leave three zombies. Measured: 0 zombies after ~10 kill/relaunch cycles.
2. ⛔ **`waitpid` is one-shot.** After it reaps, every later call for that pid returns `-1`/`ECHILD`.
   A naive `waitpid(...) != 0` therefore latches "dead" **forever** and re-relaunches every tick. The
   pid is cleared to `-1` the moment the exit is observed.
3. ⛔ **Windows' `IsPortListening` is winsock.** macOS needed its own (non-blocking loopback connect +
   150 ms `select`). Used only for the "we did not launch it" case — a dev-rig wallet has no child to
   `waitpid` for. ⛔ Deliberately **not** `QuickHealthCheck()` there: that is a 2,000 ms libcurl GET,
   and a 2 s probe inside a 2 s loop leaves no gap between ticks.

### ⚠️ One deliberate divergence from Windows, and it should probably come back the other way

A **manual** restart with the child still **alive** now SIGTERMs it first (SIGKILL after ~2 s).
Without that, `SpawnWalletServer()` early-returns *"already running"* whenever `/health` answers, so
Restart would be a **no-op against a wedged-but-listening wallet** — the same dead-control shape this
row exists to remove. Windows' `LaunchWalletProcess` has the identical early return; worth mirroring.

### ⬜ What is NOT proven, stated rather than implied

- **The graceful-shutdown ordering** is CODE_READING (see the table's last row).
- ⛔ **The `invalidateWalletStatusCache()` call on death is NOT evidenced by the run I did.** I measured
  `wallet.getStatus()` flipping to `serviceReachable: false` **3 ms** after the kill — far too fast for
  a 2 s supervisor tick, so that flip is the **live** status path, not the cache. The row proves the
  user-visible truth is honest; it says **nothing** about the invalidate, which would pass with the
  call removed. The 30 s `WalletStatusCache` consumers at the IPC / BRC-100 gates are still unmeasured
  on macOS.
- **`P8d-A7`** (the startup-cost row) was not re-measured on macOS. The supervisor starts on the
  already-dispatched health block after the startup health wait, so it adds nothing to the critical
  path by construction — but no first-paint numbers were taken here.
