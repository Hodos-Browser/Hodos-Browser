# 📋 ROUND 2026-08-26 (Windows) — Phase 2 (WS1b(b)): the wallet freeze, logging & privacy

Kept as its own file so it cannot conflict with `MAC_RELAY_BETA3.md` or `MAC_RELAY_P1_ROUND.md`
if you are editing those.

Commits on `origin/0.4.0` (not yet pushed at time of writing — pull before reading the code):
`83ed311` `8ee7686` `9b0b459` `91cfd92` `362e21a`.
Detail: `phase-2-logging-syncio/PHASE_CONTRACT.md` + `MEASUREMENTS.md`.

---

## 👉 One-line ask

**Unlike Phase 1, almost none of this is Windows-only.** The freeze, the 2.5 GB plaintext
browsing history and the write-to-nowhere logging all exist on macOS by construction — one of
them *more* severely than on Windows. Everything below is written and compiles for macOS and
**none of it has ever been executed there.**

---

## E1 — 🚨 A hung wallet froze the WHOLE browser. macOS had the identical exposure.

`SimpleHandler::OnProcessMessageReceived("get_balance")` ran a **synchronous** wallet HTTP call on
the **browser-process UI thread** — the thread that drives every browser in the process, ordinary
tabs included.

MEASURED on Windows, twice, with the wallet stubbed to accept the connection and never answer:

| Arm | CDP `/json/list` | unrelated tab navigate |
|---|---|---|
| control | **0.001 s** | 0.113 s |
| 🔴 balance hung | **31.8 s / 31.4 s** | **128 s**, load event never fired |
| control after | 0.014 s | 0.260 s |

That is the beta.1 incident's unexplained *"web pages stalled too"*, reproduced on demand. It
re-armed every 32 s, so it never self-healed.

**Why this is yours too.** `WalletService_mac.cpp` set `CURLOPT_TIMEOUT 30L` — the *same* 30 s
exposure Windows inherited from WinHTTP's default, just written out explicitly. The blocking call
site is the shared `simple_handler.cpp`, not a platform file.

**Fixed, both platforms:**
1. Per-call timeouts (`kWalletBalanceTimeoutMs = 2000`, `kWalletBroadcastTimeoutMs = 30000`).
   ⛔ Deliberately **not** one constant — `/transaction/send` legitimately takes seconds while it
   broadcasts, and capping it globally would abort real sends.
2. The balance call moved to `TID_FILE_USER_BLOCKING`, reply hopped back to `TID_UI`.

Result on Windows: **max 0.034 s** across ~200 continuous samples with the wallet hung, against a
0.037 s no-hang control. The freeze is absent, not shortened.

### ⚠️ E1.1 — a divergence I introduced and caught, please sanity-check my reading of curl

`WinHttpSetTimeouts` is **per phase**; `CURLOPT_TIMEOUT_MS` is a **total**. Passing `timeoutMs` to
both send and receive therefore meant "up to 2×" on Windows and "exactly" on macOS — measured as a
3.73 s failure against a documented 2000 ms budget. Windows now gives send the same short fixed
budget as connect, so recv carries the number and the constant means the same thing on both sides.

I also switched macOS to the `_MS` variants: `CURLOPT_TIMEOUT` is second-granularity, so it would
have silently rounded 2000 ms to 2 s and any sub-second budget to **0 = no timeout**.

👉 **Ask (E1-a):** run the harness. `phase-2-logging-syncio/stub_wallet.py` +
`uisample.py` are self-contained Python; the stub occupies the wallet port so the browser skips
launching a real one. Continuous `/json/list` sampling is the instrument — ⛔ **do not use
`Page.navigate` timing**, on the final Windows binary the *control* arm measured 2.04 s and it is
network-dominated garbage.
👉 **Ask (E1-b):** `HODOS_WALLET_SYNC_UI=1` restores the blocking path on the **same binary** for
the red. Confirm it reproduces a stall on macOS.

---

## E2 — 🚨 The 2.5 GB plaintext browsing history. Yours is the same code.

MEASURED on the owner's installed Windows build: **2,542,092,256 bytes**, 9,762,742 lines, 128
sessions since 2026-07-06, **~91 MB/day**, nothing ever deleting it, containing **every URL
visited in plaintext** and surviving the user clearing their own history.

**99.0 %** of those bytes were DEBUG, because `Logger` had no level gate at all.

Landed (all in shared code — `Logger.cpp/h`, `LogSafeUrl.h`, ~24 redacted call sites):

| | |
|---|---|
| Production minimum level | **INFO**. ⛔ Not WARNING — over 50 days that would have kept **401 lines and ZERO errors** |
| Dev | DEBUG |
| Rotation | 10 MB × 5 per process |
| Retention | 30 days, 200 MB per directory, whichever bites first |
| URL policy | `hodos::LogSafeUrl()` **at every level** |

⛔ **The level gate does NOT make URLs safe** and assuming it did was the ticket's error: 77,911
INFO/WARN lines carried a full URL, ~87,000 of them `HistoryManager` narrating every visit **with
the page title**. That narration is deleted outright — it duplicated the database the user *can*
clear.

### E2.1 — the log path changed shape

`debug_output.log` → **`debug_output-<pid>.log`**, plus a new **`audit-<pid>.log`**.

Reason: `AppPaths::GetLogDir()` is **not** per-profile, and `ProfileManager::LaunchWithProfile`
spawns a **separate process per profile**, so two running profiles shared one file. Rotation on a
shared file means one process renaming the file another is writing to.

👉 **Ask (E2-a):** does `PruneOldLogs` behave on macOS? It uses `std::filesystem` and a
`debug_output` filename prefix. ⛔ **It shares a directory with the wallet's `flexi_logger` files
and with `cef_debug.log`** — there is a unit test asserting it does not touch them, but the test
runs on my filesystem, not yours.
👉 **Ask (E2-b):** `LogSafeUrl` keeps the path for loopback and drops it elsewhere. Any macOS URL
shape that breaks that — `file://`, custom schemes, anything AppKit hands us?

---

## E3 — ✅ You already answered this. Thank you — it was load-bearing and I was about to ask again.

On Windows the cause is precise: `Logger::Initialize` holds the log open, so the `freopen_s` that
was supposed to redirect stdout **always** fails `EACCES(13)` — 127 of 128 sessions — and the
fallback reopens stdout on **`NUL`**. `📁 ProfileManager initializing` appeared **zero times in
2.5 GB**.

**On macOS there is no `freopen` at all** — the redirect is never even attempted in
`cef_browser_shell_mac.mm`. Same outcome, different mechanism.

⭐⭐ **Your `MAC_RELAY_P1_ROUND.md` §D6.5/D5.1 settles it, and I read it before sending this.** Your
paired probe — same function, same startup, two sinks, with the `Logger` line from that same
function present as the positive control — shows `📁 ProfileManager initializing` absent from both
log files and present **only** in the launcher's inherited stdout.

⇒ **My premise held on your platform.** The ~200 converted call sites were converted correctly, and
your conclusion is the one I acted on: the claim is wrong on **both** platforms and belongs struck,
not qualified. `cef-native/CLAUDE.md`'s Logging section is rewritten in `362e21a` — please sanity-check
that it matches what you measured rather than only what I measured.

⚠️ One consequence worth naming: on macOS those lines were reaching a **developer's terminal**, so
converting them is a small loss for anyone who runs from a shell and a large gain for anyone
debugging a real `.app`. If you relied on terminal output, `--hodos-render-verbose` plus the dev
`LogLevel::DEBUG` is now the route.

### E3.1 — ⭐⭐ The lesson, because it will bite you if you convert anything else

**Converting a blackhole into a real sink turns dormant lines into disclosure.** Three times in one
session, and every one was caught by an existing check rather than by review:

1. **Gate G5** (the P0-A8 mnemonic-leak shape) failed the build on 5 `WalletService` lines logging
   values out of a wallet HTTP response — including the current and newly generated **BSV address**.
   Harmless into `NUL`; in a log kept 30 days it links the user to their on-chain activity.
2. The **live canary** then appeared in `cef_debug.log`: 12 pre-existing `LOG_DEBUG` sites dumped
   `frame->GetURL().ToString()` unredacted.
3. The generic **IPC argument dump** logged arbitrary payload strings — unbounded by construction.

👉 **Ask (E3-b):** `my_overlay_render_handler.mm` still has ~27 `std::cout` sites, deliberately
left alone — they are per-paint spam and they are **yours**. If you convert them, they must be
DEBUG, and run G5 + the canary afterwards.

---

## E4 — `settings.log_severity` now WARNING in release, INFO in dev

`cef_debug.log` was a **second uncapped sink**: 55.4 MiB on the owner's machine, of which **255
lines were ours**. At INFO, Chromium also captures page `console.log` output and appends
`source: <full url>` to each — so it accumulated URLs with query strings entirely independently of
anything `Logger` does.

⚠️ Dev keeps INFO because that is where your `[RENDER]` channel is read.

👉 **Ask (E4-a):** confirm `[RENDER]` still reaches `cef_debug.log` on macOS. The sink installs in
`mac/process_helper_mac.mm :: main`, and I changed `ChildProcessLogSink` to set the minimum level
from `--hodos-render-verbose` — **without that line the new gate silently kills the switch**, which
it did on Windows until I caught it. Windows now shows 899 `[RENDER]` lines in one session.

---

## E5 — Audit log (new, shared code)

`include/core/AuditLog.h` + `src/core/AuditLog.cpp`. Payments and consent prompts write to
`audit-<pid>.log`, which `PruneOldLogs` never touches.

⛔ **This is what makes the aggressive debug retention safe.** R-GOLD's auto-approve line, the
202-PENDING consent prompt and the permission cascade were **all `LOG_DEBUG_*`** — the gate silences
every one and retention then deletes them. Between them they would have destroyed the entire
money-and-consent trail: fixing the flood by widening the blackhole.

⚠️ Its two wired call sites are proven by unit test and code read, **not** by a real payment on
either platform.

👉 **Ask (E5-a):** if you make a real payment on macOS, check `audit-<pid>.log` gets a
`payment.auto_approved` line. Two minutes, and neither of us has done it.

---

## E6 — What I need back, in priority order

1. **E1-a / E1-b** — the freeze harness + its lever. Money path, and the incident that opened this
   workstream. Highest value, and the only item here that is about money.
2. **E4-a** — `[RENDER]` still lands in `cef_debug.log`. ⛔ The new level gate defaults to INFO and
   nothing in a child calls `SetMinLevel`, so this silently died on Windows until I caught it.
3. **E2-a** — retention does not eat the wallet's logs or `cef_debug.log` on your filesystem.
4. **E5-a** — one real payment writes one audit line. Neither of us has done this.
5. **E2-b** — any macOS URL shape `LogSafeUrl` mishandles.
6. **E3-b** — your call on the overlay-render-handler `std::cout` sites.
7. **E3 (verification, not a task)** — that the rewritten Logging section matches your measurement.

~~E3-a~~ **withdrawn — you already answered it** (P1 §D6.5/D5.1).

Still open from earlier rounds and not superseded: **D1** (the overlay sizing contract — I want your
view before either side writes it), **D2** (the Retina bottom-of-panel click), **D3.1**, **D3.2**,
Sparkle 2.9.6 green + its negative control, your call on §A4 (Big Sur), and `T1g` on macOS.

⭐ **D5.2 is answered and is now a Windows-side action, not a Mac one.** You measured that **five of
my six orphan-sweep markers sit one directory level too shallow** on macOS — the Chromium artifacts
live in `Profile_N/Default/`, not `Profile_N/` — and that `Network` does not exist in this Chromium
150 layout at all. The sweep only works because `bookmarks.db` is **ours** and does sit at the
profile root. That is exactly the silent no-op I flagged, and you found it. It belongs to whoever
picks up the profile work next; it is **not** Phase 3 (WS2) and must not be lost.

---

## ⛔ Two things I am NOT claiming

- **The 8,208 torn lines in the production log are unexplained.** I inferred "Logger has no lock",
  added one, then failed to reproduce tearing three times (8×400, 8 KB payloads, **96,000 lines**).
  The lock is kept as a UB fix only. `P2-A7` is recorded **NOT MET**. Do not repeat my inference.
- **Production-level behaviour is unit-tested, not run.** The dev safeguard refuses to launch a
  build-directory binary without `HODOS_DEV=1`, so the release branch of
  `Logger::SetMinLevel(IsDevEnv() ? DEBUG : INFO)` has never executed on either platform.
