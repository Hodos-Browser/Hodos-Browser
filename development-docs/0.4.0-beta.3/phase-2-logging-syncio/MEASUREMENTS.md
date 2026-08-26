# Phase 2 — measured results (kickoff)

Everything here is a **measurement** taken 2026-08-26 on the owner's machine. Claims, plans and
rankings live in `PHASE_CONTRACT.md`; this file records only what an instrument reported.

⛔ The installed browser's `%APPDATA%\HodosBrowser\logs\` was **read only**. Nothing was truncated,
rotated or deleted. The installed wallet (31301) and its CDP port (9222) were never addressed.

---

## M0 — The flood, re-measured and counted

`%APPDATA%\HodosBrowser\logs\debug_output.log`, one full pass with `awk` (9.76 M lines).

| | |
|---|---|
| First line | `2026-07-06 14:15:39  Logger initialized for MAIN` |
| Size at 09:44 | **2,542,092,256 bytes = 2,424.3 MiB** |
| Lines | **9,762,742** |
| Sessions (`=== NEW SESSION STARTED ===`) | **128** |
| Average growth | **47.7 MiB/day** over 50.8 days |
| Growth since the ticket (1,700,205,765 B, 2026-08-17) | **≈ 91 MiB/day** |

⚠️ The session prompt says 2,422.9 MB and "+93 MB/day". Both confirmed to within rounding. The
ticket's 1.58 GB is now **1.5× larger**.

### M0.1 — Where the bytes are

| Level | Lines | Bytes | % of file |
|---|---|---|---|
| **DEBUG** | 9,606,433 | 2,516,143,028 | **99.0 %** |
| INFO | 143,537 | 16,186,115 | 0.6 % |
| WARN | 401 | 41,230 | 0.0 % |
| **ERROR** | **0** | 0 | 0.0 % |
| (no level — see M4) | 12,371 | 9,936,000 | 0.4 % |

| Process tag | Lines |
|---|---|
| BROWSER | 9,644,917 |
| MAIN | 104,984 |
| **UNKNOWN** | **470** |
| RENDER | **0** |

| Pattern | Lines | Bytes | % |
|---|---|---|---|
| `🌐 Resource request: <full URL>` | 1,596,204 | 786,957,362 | **31.0 %** |
| `🌐 Method: GET, Connection…` | 1,221,778 | 106,592,861 | 4.2 % |
| `📨 Message received: <ipc>` | 4,185,962 | 440,966,852 | 17.3 % |

⭐ **Zero ERROR lines in 9.76 M lines across 128 sessions.** The dev log, same binary family, has
**364 ERROR** and **1,015 WARN** — so `LOG_ERROR_*` demonstrably can fire. Recorded, not explained.

---

## M1 — 🎯 §2.2 SETTLED: the stdout redirection **never happens**. It goes to `NUL`.

`cef-native/CLAUDE.md` says of `std::cout`: *"stdout is redirected to the same file anyway."*

**Measured false.** `cef_browser_shell.cpp :: RunHodosMain` calls `Logger::Initialize` first, which
holds `debug_output.log` open for append; the `freopen_s` that follows therefore **cannot** get a
second writer and fails `EACCES(13)`. Because a failed `freopen_s` closes the stream first, the code
deliberately reopens both handles on **`NUL`**:

```cpp
if (result1 != 0) freopen_s(&dummy, "NUL", "w", stdout);
if (result2 != 0) freopen_s(&dummy, "NUL", "w", stderr);
LOG_WARNING("freopen failed - stdout: " + ... + ", stderr: " + ...);
```

| Evidence | Value |
|---|---|
| `freopen failed - stdout: 13, stderr: 13` | **127 of 128 sessions** |
| First occurrence | line 5 of the file, `2026-07-06 14:15:39.132` |
| `📁 ProfileManager initializing` (runs every startup) | **0 occurrences in 2.54 GB** |

⭐ So the answer to "either the redirection never happens, or it happens too late, or CEF 150 changed
it" is **the first** — and it has never happened, on any build. The in-code comment already says
*"The redirect above ALWAYS fails with EACCES(13) … true since long before CEF 150."* **The code and
the doc contradict each other and the code is right.** The doc is actively steering authors at a
sink that discards their output.

### M1.1 — What is invisible

Files writing to the NUL'd stdout/stderr, by call-site count:

| File | Sites | Notably silent |
|---|---|---|
| `simple_render_process_handler.cpp` | 77 | (child process — different path, see M3) |
| `src/core/WalletService.cpp` | 44 | wallet create / load / addresses, `❌ Failed to connect to Rust wallet` |
| `src/core/ProfileManager.cpp` | 29 | every profile-delete refusal, `❌ Error saving profiles.json`, `⚠️ RegistryLock … timed out`, `❌ Refusing to launch: invalid profile id` |
| `my_overlay_render_handler.mm` / `.cpp` | 27 / 16 | |
| `simple_handler.cpp` | 26 | |
| `simple_app.cpp` | 22 | |
| `AddressHandler.cpp` | 21 | |
| `HistoryManager.cpp` | 16 | |
| `include/core/AppPaths.h` | 14 | |
| `IdentityHandler.cpp` | 11 | |
| `NavigationHandler.cpp` | 3 | |

`Logger::Log` itself ends with an unconditional `std::cout << logEntry << std::endl` — a second
formatted write per line, into `NUL`, on every log call.

⚠️ **macOS is a CLAIM, not a measurement.** There is no `freopen` anywhere in
`cef_browser_shell_mac.mm` — the redirect is never *attempted* on macOS, so `std::cout` from a
Finder-launched `.app` goes to the process's inherited stdout, i.e. nowhere the user can find. Same
outcome, different mechanism. This is ask **D5.1** in `MAC_RELAY_P1_ROUND.md` and stays open.

---

## M2 — 🚨 NEW: `Logger` has no lock, and the log is measurably torn

`grep -n "mutex\|lock_guard\|atomic"` over `include/core/Logger.h` + `src/core/Logger.cpp` →
**nothing**. `Logger::Log` does an unsynchronised `logFile << logEntry << std::endl` and is called
from the UI thread, the IO thread (resource requests) and CEF's worker threads.

**Measured consequence: 8,208 lines in the file are fragments with no timestamp prefix** —
interleaved writes that tore mid-line:

```
2/badge_count/badge_count.json?supports_ntab_urt=1&include_xchat_count=1 (role: tab_1)
":[{"favicon":"https://abs.twimg.com/favicons/twitter-pip.3.ico", … "url":"https://x.com/home"}]}
timedtext?v=oDyyp0NGmcI&ei=…&expire=1785792827&sparams=…
```

(The remaining ~4,163 non-conforming lines are empty.)

⭐ This matters beyond tidiness: §4 of the session prompt notes the log is the **only** record of
several safeguards. A log with a data race can silently lose or mangle exactly the line you later
need, and the tearing is invisible unless you go looking for it.

---

## M3 — The three sinks, measured

| Writer | Process | Path taken | Lands in | Evidence |
|---|---|---|---|---|
| `LOG_*` | browser | `initialized==true` → `Logger::logFile` | `debug_output.log` | 9.76 M lines |
| `Logger::Log`'s trailing `std::cout` | browser | freopen'd | **`NUL`** | M1 |
| direct `std::cout` / `std::cerr` | browser | same NUL fd | **`NUL`** | M1 |
| `LOG_*_RENDER` etc. | child | `initialized==false` → `Logger::sink` → `InstallChildProcessLogSink` → Chromium `LOG()` | **`cef_debug.log`** | 255 lines, all stamped `ChildProcessLogSink.cpp:61` |
| `std::cout` in a child before the sink exists | child | plain stdout | nowhere | (the pre-2026-08-09 renderer no-op) |

`[RENDER]` appears **0** times in `debug_output.log` and **255** times in `cef_debug.log` — the
documented split is correct, and it is the one part of the Logging section that survives scrutiny.

### M3.1 — `cef_debug.log` is a second uncapped sink, and 99.9 % of it is not ours

| | |
|---|---|
| Size | **58,068,647 bytes = 55.4 MiB** (ticket said 17 MB) |
| Lines | 290,157 |
| Lines that are ours (`[RENDER]`) | **255** |
| Cause | `settings.log_severity = LOGSEVERITY_INFO` in `cef_browser_shell.cpp` |

Top Chromium emitters: `browser_info.cc` (2,377), `dns_config_service_win.cc` (960),
`capture_device_ranking.h` (389), `page_discarding_helper.cc` (117).

---

## M4 — 🚨 NEW: five files log as `[UNKNOWN]` — the SUBJECT column of the log format is broken

`Logger::Log`'s second argument is a `ProcessType` (0 MAIN / 1 RENDER / 2 BROWSER). Five files pass
**10, 11 or 12**, which `GetProcessName` maps to `default: return "UNKNOWN"`:

| File | Code |
|---|---|
| `src/core/SettingsManager.cpp` | 10 |
| `src/core/SilentStateWriter.cpp` | 10 |
| `src/core/ProfileImporter.cpp` | 11 |
| `src/core/PaidContentCache.cpp` | 12 |

470 lines in the production log carry `[UNKNOWN]`. The author's intent was clearly a *subsystem* tag;
the field is a *process* tag. Both the update-mode writer and the paid-content cache — two things you
would very much want attributed — are among them.

---

## M5 — 🎯 E1 SETTLED: a hung `/wallet/balance` **does** stall ordinary web pages

This is the experiment the sprint plan and the session prompt both name as the one that decides
whether half (b) is a performance clean-up or a hang bug. **It is a hang bug.**

### Mechanism under test

`simple_handler.cpp :: OnProcessMessageReceived("get_balance")` runs on the **browser-process UI
thread** and calls `WalletService::getBalance` → `makeHttpRequest` → **synchronous WinHTTP**. The UI
thread drives every browser in the process, tabs included.

⛔ `makeHttpRequest` calls **no `WinHttpSetTimeouts`** — the only `WinHttpSetTimeouts` in the whole
tree is in `update-helper/transaction.cpp`. So WinHTTP's defaults apply (60 s connect, 30 s send,
30 s receive). By contrast `SyncHttpClient.cpp` **does** set all three explicitly — the newer client
already does this correctly.

### Instrument

- `scratchpad/stub_wallet.py` on **127.0.0.1:31401** — replies to every wallet endpoint with a
  verbatim capture from the real dev wallet, except `/wallet/balance`, whose behaviour is switched
  live by `GET /__ctl?hang=0|1`. Threaded, so a hung `/wallet/balance` blocks only its own
  connection. **Instrument control:** with balance hanging, `GET /health` on the same stub still
  answers in **0.057 s** — so any stall observed in the browser is the browser's, not the stub's.
- `scratchpad/e1.py` — fires `window.cefMessage.send('get_balance',[])` on the **header** browser
  (fire-and-forget; waiting for its reply would measure the same block twice), then drives an
  **unrelated tab** browser and times three things.

### 🎯 SUBJECT

Dev build `cef-native/build/bin/Release`, `HODOS_DEV=1`, `--profile=Default`, CDP **9322**, wallet
port **31401** served by the stub. ⛔ The installed browser was never addressed — `cdp.py` refuses
port 9222 outright, and the installed wallet on 31301 was left alone. Because the stub occupies
31401 before launch, `LaunchWalletProcess()` takes its "wallet server already running" early return
and no real wallet is involved.

### Results — same binary, same script, same actions; only the stub's flag differs

**Run 1**

| Arm | `/json/list` | `Page.navigate` ack | `loadEventFired` | `Runtime.evaluate` |
|---|---|---|---|---|
| **CONTROL-BEFORE** (hang=0) | 0.001 s | 0.113 s | 0.020 s | 0.010 s |
| 🔴 **TEST** (balance hung) | **31.784 s** | **128.007 s** | **never** (240 s budget) | **143.993 s** |
| **CONTROL-AFTER** (hang=0) | 0.014 s | 0.260 s | 0.018 s | 0.095 s |

**Run 2 — independent replicate, same binary**

| Arm | `/json/list` | `Page.navigate` ack | `loadEventFired` | `Runtime.evaluate` |
|---|---|---|---|---|
| **CONTROL-BEFORE** (hang=0) | 0.016 s | 0.014 s | 0.018 s | 0.013 s |
| 🔴 **TEST** (balance hung) | **31.383 s** | **128.000 s** | **96.030 s** | 0.044 s |
| **CONTROL-AFTER** (hang=0) | 0.018 s | 0.020 s | 0.013 s | 0.008 s |

⭐ Reproducible: `/json/list` **31.4 / 31.8 s** against a 1–18 ms control, four clean control arms
across two runs on the same binary.

⚠️ **Recorded, not rounded off:** `nav_ack` came out at **128.007 s** and **128.000 s** — two runs
agreeing to 7 ms. That exactness means it is a **timeout constant somewhere in the navigation path**,
not a measure of how long the UI thread was blocked. Treat `/json/list` as the clean UI-thread
liveness number and `nav_ack` / `load` as "the tab was unusable for minutes", not as durations to
quote precisely. In run 2 the page did eventually load, after **96 s**.

- `/json/list` is served by the browser process. **31.8 s vs 1 ms** is the UI thread being blocked,
  measured without involving any page at all — and 31.8 s is WinHTTP's default 30 s receive timeout.
  ⭐ **One hung balance call blocks the whole browser for ~30 s.**
- Navigating an **unrelated tab** to `https://example.com` took **128 s** and never fired its load
  event. That is the incident's unexplained symptom — *"ordinary web pages stalled too"* —
  reproduced on demand.
- ⭐ The control runs **before and after** on the same binary, both clean. The only difference is the
  stub's flag, so "did the binary change or did the behaviour?" cannot be asked.

### Why it does not recover on its own

The stub's access log shows the block re-arming continuously for the whole 3 m 44 s of the TEST arm:

```
10:04:58 HANG-BEGIN → 10:05:24 HANG-END      (then 6 s idle)
10:05:30 HANG-BEGIN → 10:05:56 HANG-END
10:06:02 HANG-BEGIN → 10:06:28 HANG-END
10:06:34 HANG-BEGIN → 10:07:00 HANG-END
10:07:06 HANG-BEGIN → 10:07:32 HANG-END
10:07:38 HANG-BEGIN → 10:08:04 HANG-END
10:08:10 HANG-BEGIN → 10:08:36 HANG-END
```

A **26 s block + 6 s gap, repeating every 32 s** — the browser gives up, the poller re-fires, the UI
thread blocks again. Meanwhile `/wallet/peerpay/status` traffic stops dead and then arrives in a
burst of 13 requests in a single second the moment the hang is lifted: a stalled queue draining.

⚠️ **Recorded, not rounded off:** the stub's own `RELEASE.wait(90.0)` returned after **26 s**, not
90 s, on every iteration, and the stub then failed to write its reply with `ConnectionReset 10054`
(the browser had already hung up). The 10054 is expected and confirms the browser gave up first; the
26 s wait return is **unexplained**. It does not affect the finding — every number in the results
table was taken on the browser, not on the stub — but it is not understood and is not being smoothed
over.

⚠️ **Also unresolved:** `useBackgroundBalancePoller.ts` sets `BALANCE_POLL_MS = 30_000`, which does
**not** match the incident's uniform **2.0 s** gaps. Something else re-arms the balance call more
often than that poller. Not chased here.

---

## M5b — 🎯 Phase 2a POST-FIX: the freeze is gone, and both halves are shown to be load-bearing

Same harness, same subject, dev build rebuilt with (a) per-call timeouts + (b) the balance call
moved to `TID_FILE_USER_BLOCKING`.

### The instrument that decides it

⛔ **`Page.navigate` timing turned out to be a bad instrument** and is not the basis of any claim
here. On the final binary the **control** arm — no hang at all — measured `nav_ack` at **2.040 s**,
and it hit 128 s twice pre-fix. It is dominated by network variance to `example.com`, and a one-shot
probe only sees a block if it happens to coincide with one. ⭐ A control arm that cannot possibly be
blocked showing 2 s is the proof that the number was never measuring the block.

`scratchpad/uisample.py` replaces it: **poll `/json/list` continuously for 25 s and report the max**.
`/json/list` is served by the browser process, so its response time is a direct read of UI-thread
availability, with ~200 samples per window instead of one.

### Continuous UI-thread sampling, ~200 samples per arm

| Binary | control (hang=0) | 🔴 **balance hung** | control after |
|---|---|---|---|
| **Final (a)+(b)** | max 0.037 s | **max 0.034 s** | max 0.031 s |
| Same binary, `HODOS_WALLET_SYNC_UI=1` | max 0.027 s | **max 3.864 s** | max 0.028 s |
| Pre-fix (M5, one-shot) | 0.001 s | **31.8 s / 31.4 s** | 0.014 s |

⭐⭐ **With the fix, a hung wallet is indistinguishable from a healthy one** — 0.034 s against a
0.037 s control, across 197 samples. The freeze is not shortened; it is absent.

### The two fixes decompose cleanly

| Configuration | Worst UI-thread block |
|---|---|
| Pre-fix: sync on UI thread, no timeout | **31.8 s** |
| (a) only — lever restores the sync path, timeout in force | **3.9 s** |
| (a)+(b) — shipped | **0.034 s** |

⛔ **Note the RED honestly:** the same-binary lever reproduces **3.9 s**, not the historical 31.8 s,
because (a) is compiled into that binary too and caps the block. The 31.8 s figure belongs to the
pre-fix binary (M5, two runs). Both are real; neither is the other.

### P2a-A3 / A5 — the answer still arrives (the anti-fake half)

"No freeze" is trivially satisfied by never replying, so `getBalance()` was driven from CDP:

| Wallet state | Result |
|---|---|
| answering | `{"balance":28332055,"bsvPrice":16.875}` in **0.00–0.01 s** |
| **hung** | `{"error":"Failed to fetch total balance"}` in **2.7–5.2 s** — an error, not a spinner |
| answering again | full balance again, immediately |

4 consecutive runs clean.

### M5c — 🎯 P2a-A3 owner-confirmed at the machine, 2026-08-26

⛔ **Checked before asking the owner to click — and the row could not have passed as written.**
Three defects sat behind it, none of them the freeze:

1. **A wallet outage displayed a confident `$0.00`.** `WalletService::getBalance` reports failure as
   a *resolved* IPC response carrying `{"error": ...}` and no `balance`. Nothing on the JS side
   checked for it, so `response.balance` was `undefined` and went straight into state.
2. **It poisoned the cache.** `setCachedBalance(undefined)` → `JSON.stringify` **drops** the key →
   the entry reads back through `cachedBal?.balance ?? 0` as a **real zero**, re-poisoned every 30 s
   by the background poller and still zero on the next launch. Violates the standing
   "caches must not self-poison on failure" rule.
3. **Nothing rendered the error.** `useBalance` has always tracked `error`; `WalletPanel` never
   destructured it.

⭐ Telling a user their balance is zero because a socket did not answer is worse than any spinner.

**Fixes:** the bridge now **rejects** when the payload is not a balance (all three callers already
had `catch` blocks, so they route to their existing error paths and skip their cache writes);
`setCachedBalance` refuses a non-finite value; `WalletPanel` renders a staleness notice.

**Notice contrast measured, not eyeballed: `#7a4b00` on `#fff4e0` = 6.80:1** (needs ≥ 4.5). ⭐ This
project has shipped three controls at a contrast a human reads as absent — 1.07:1, 1.5:1, 2.35:1.

**Owner's observation, three-point:**

| State | Observed |
|---|---|
| healthy | **$4.78**, no notice |
| wallet hung, Refresh clicked | "Refreshing…" briefly → back to "Refresh", **balance still $4.78**, amber notice appears |
| wallet restored, Refresh clicked | notice clears, $4.78 returns |

⭐ Each state is the others' control, so the row is not vacuous: a stuck indicator fails state 1, a
swallowed error fails state 2, a latched error fails state 3.

⚠️ Tooling note: `Invoke-WebRequest` prompts *"Security Warning: Script Execution Risk"* and defaults
to **No**. Pass `-UseBasicParsing` in any future walkthrough.

### P2a-A4 — the generous broadcast budget is real, and load-bearing

The stub answers `/transaction/send` slowly. No money moves — it returns a canned txid.

| Server delay | Budget | Result |
|---|---|---|
| 8 s (> the 5 s default, < the 30 s broadcast budget) | `kWalletBroadcastTimeoutMs` | 🟢 **succeeds in 8.02 s** |
| 35 s (> the broadcast budget) | same | 🔴 **fails at 34.55 s** |

⭐ Green *and* red on the same path: had (a) capped every endpoint with one constant, the 8 s send
would have failed. This is the row that would have caught it.

### ⚠️ Measured, unexplained, not rounded off

A configured **2000 ms** receive budget produces an end-to-end failure at **~3.4–4.0 s**
(`Error 12002 = ERROR_WINHTTP_TIMEOUT`, so the timeout *is* firing, and
`WinHttpSetTimeouts failed` appears **0** times). WinHTTP's internal retry is the likely mechanism;
it has **not** been established. Consequence is bounded and cosmetic — the UI thread is not blocked
either way (0.034 s), so this only affects how fast the UI learns the wallet is down.

⭐ Fixed along the way: the Windows budget was initially passed to **both** the send and receive
phases while macOS's `CURLOPT_TIMEOUT_MS` is a **total**, so the same constant meant different
things per platform. Send now takes the same short fixed budget as connect.

### 🚨 A defect this fix made reachable — found, fixed, and ticketed

Moving the balance call off the UI thread removed the accidental serialisation that had been hiding
a bridge race: `initWindowBridge.ts` resolves through a **single global**
`window.onGetBalanceResponse`, so two overlapping calls clobber each other.

| Run | Before the bridge fix | After |
|---|---|---|
| 1 | error at 4.92 s | clean |
| 2 | 🔴 **`get_balance timed out` at 10.01 s** | clean |
| 3 | error at 4.24 s | clean |
| 4 | — | clean |

**1 in 3 → 0 in 4.** Fixed for `getBalance` by deduping in flight (sound for a read-only query).
⛔ **Not** copied to `sendTransaction` — deduping two sends would collapse two payments into one.
The systemic fix is a per-request id: `TICKET_bridge_single_slot_callbacks_race.md`.

Also fixed while in that file: `get_balance_error` pasted its JSON payload between single quotes
**unescaped**; it now goes through `escapeJsonForJs`, the canonical encoder the rest of the file
already uses.

### Gates

| Check | Result |
|---|---|
| `hodos_tests` | **306 tests, 305 pass, 1 pre-existing skip** (`UpdateStagerRig.StagesFromLocalFeed`) |
| `preflight.ps1 -Full` | **PASS** — T0 (G1–G5, G8) + T1a–T1g |
| `preflight.ps1 -NegativeControl` | **PASS** — every gate seen to fail on an injected violation |
| `tsc --noEmit` (frontend) | clean |

---

## M6 — ⛔ A stale premise in the session prompt and the sprint plan

Both say, in the present tense, that *"`getBalance` alone does 4 open/write/close cycles per call."*

**Already fixed**, by Phase 0 (`ceca3ad`). The current code says so itself:

```cpp
// P0-A1: this is the hottest path in the browser -- 13,643 calls in one dev log,
// 2,551 in a single production session. It opened/wrote/closed a file in {app}
// FOUR times per call. Now one level-gated Logger line …
```

⚠️ …and **that comment is itself wrong on its last clause**: there is no level gate in `Logger`, so
those `LOG_DEBUG_BROWSER` lines are *not* gated and do ship to production. The file-open cycles are
genuinely gone; the synchronous flushed writes replacing them are not.

⭐ Confirmed by the dev log: `makeHttpRequest` is called **12,279** times for `GET /wallet/balance`
and **5** times for everything else combined. `/wallet/balance` is effectively the *only* endpoint on
the fresh-`WalletService`-per-call synchronous path.

---

## M7 — 🚨 A DEBUG-level gate does **not** fix the privacy defect

The ticket's fixes #1 (level gate) and #2 (stop logging URLs) read as if the first largely
accomplishes the second. Measured: it does not.

**77,911 INFO/WARN lines contain a full URL.** The top INFO content by volume:

| Lines | Content |
|---|---|
| 20,778 | `📚 Skipping duplicate visit (debounced): <URL>` |
| 16,006 | `✅ Visit recorded successfully` |
| 16,005 | `📚 Adding visit: <URL>` |
| 14,679 | `📚 URL exists, updating (current visits: N)` |
| 14,671 | `✅ URL updated, new visit count: N` |
| ~21,000 | `📚 Recording history: <URL> [<page title>]` |
| 3,835 | `💉 Injecting scriptlets for <URL>` |

⭐ **~87,000 of the 143,537 INFO lines are `HistoryManager` narrating every visit, with page
titles.** That is a second complete browsing history — one the user cannot clear, duplicating the
history database they *can*.

⇒ Whatever level production defaults to, a **URL-redaction rule applied at every level** is a
separate, independently necessary change.

---

## M8 — What a level gate would actually leave behind

| Production default | Lines kept (over 50.8 days) | Bytes kept | Per day |
|---|---|---|---|
| DEBUG (today) | 9,762,742 | 2,542 MiB | 47.7 MiB |
| **INFO** | 143,938 | 16.2 MiB | **0.32 MiB** |
| **WARNING** | **401** | 40 KiB | **8 lines/day** |

⛔ **This is the trap §3 of the session prompt warns about.** "Production defaults to WARNING" —
the ticket's own wording — would produce a log containing **401 lines in fifty days and zero
errors**. It satisfies "the log is smaller" completely while destroying the log.

### M8.1 — And it would silence the money trail

The lines that are the sole record of a security-relevant decision, with their current levels:

| Evidence | Site | Level |
|---|---|---|
| **R-GOLD** — the one line proving a payment was auto-approved | `HttpRequestInterceptor.cpp :: OnWalletCallSuccess` | **DEBUG** |
| 202 PENDING → modal opened (`requestId=`) | `HttpRequestInterceptor.cpp` | **DEBUG** |
| Domain-permission cache decisions | `HttpRequestInterceptor.cpp` | **DEBUG** |
| Permission JSON handed to the modal | `simple_handler.cpp` | **DEBUG** |
| BRC-121 payment approved — armed replay | `HttpRequestInterceptor.cpp` | INFO |
| Every profile-delete refusal | `ProfileManager.cpp` | *(`std::cout` — already invisible, M1)* |

A production default of WARNING erases **all** of these. A production default of INFO erases the
first four.

---

## M9 — 🚨 A fresh WinHTTP session per IPC call

`SimpleHandler::OnProcessMessageReceived` constructs a stack-local `WalletService` — and therefore a
fresh `WinHttpOpen` + `WinHttpConnect`, torn down in the destructor — **on every message**, at
**16 sites** in `simple_handler.cpp` plus 2 in `AddressHandler.cpp` and 1 in `IdentityHandler.cpp`.
No connection is ever reused. The destructor also calls `stopDaemon()`, which is a no-op only
because `daemonProcess_` was zeroed in the constructor.

⚠️ CLAUDE.md invariant #9 already says *"Never use raw WinHTTP for new singletons — use
`SyncHttpClient`."* `WalletService` predates that rule and was never migrated. **The correct fix
already exists in-tree**, which is the reuse-first answer for half (b).
