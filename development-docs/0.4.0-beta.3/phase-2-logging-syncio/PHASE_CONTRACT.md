# Phase 2 — logging & synchronous I/O · PHASE CONTRACT

**Workstream:** WS1b(b) · **Ticket:** `TICKET_production_debug_logging_unbounded.md` · **Status:** ✅ **Phase 2a COMPLETE** — all rows GREEN with REDs observed, `P2a-A3` owner-confirmed at the machine 2026-08-26 · Phase 2b not started
**Opened:** 2026-08-26 · **Owner:** Matthew · **Platforms:** Windows (macOS: relay asks D5.1 + D5.2)
**Standard:** `../HARNESS.md`. Measurements: `MEASUREMENTS.md` — every number below is cited from there.

---

## 0. What the kickoff changed about the phase as written

| # | Session prompt / ticket says | Kickoff found | Where |
|---|---|---|---|
| 1 | "`getBalance` does 4 open/write/close cycles per call" (present tense) | **Already fixed in Phase 0** (`ceca3ad`). Stale premise. | M6 |
| 2 | `cef-native/CLAUDE.md`: "stdout is redirected to the same file anyway" | **False, and never was true.** The `freopen_s` fails `EACCES(13)` and the fallback reopens on **`NUL`**. 127/128 sessions. | M1 |
| 3 | Ticket fix #1: "production defaulting to WARNING" | Would leave **401 lines in 50 days and zero ERRORs**. Satisfies "smaller" by destroying the log. | M8 |
| 4 | Ticket implies a level gate largely handles the URL problem | It does not. **77,911 INFO/WARN lines carry a full URL**, ~87k of them `HistoryManager` narrating every visit **with page titles**. | M7 |
| 5 | Incident: "unexplained why web pages stalled too" | **Explained and reproduced.** Unrelated tab: 128 s to navigate, load event never fired. | M5 |
| 6 | (not mentioned anywhere) | `Logger` has **no lock**; **8,208 torn line fragments** measured in the production log. | M2 |
| 7 | (not mentioned anywhere) | Five files log as `[UNKNOWN]` — process codes 10/11/12. | M4 |
| 8 | (not mentioned anywhere) | `cef_debug.log` is a **second uncapped sink**, 55.4 MiB, of which **255 lines are ours**. | M3.1 |

⭐ **The headline: half (b) is a hang bug, not a performance clean-up.** One hung `/wallet/balance`
freezes the entire browser process for ~30 s and re-arms every 32 s. That reorders this phase.

---

## 1. Goal

A user's browser does not freeze when the wallet stops answering, and Hodos stops keeping an
uncapped plaintext record of every URL they have ever visited.

## 2. Done means

- [x] ✅ **2a DONE.** With `/wallet/balance` hung, the browser UI thread is never blocked:
      max **0.034 s** across ~200 continuous samples, against a 0.037 s no-hang control (M5b).
      ⛔ The original wording of this line ("an unrelated tab navigates in < 2 s") was **dropped as
      unmeasurable** — `Page.navigate` timing is network-dominated and the *control* arm itself
      measured 2.040 s. A criterion its own control cannot pass is not a criterion.
- [ ] A production session's `debug_output.log` contains **no full URL** — origin at most — at any level.
- [ ] `debug_output.log` and `cef_debug.log` are both **bounded**: total on disk ≤ the agreed cap,
      enforced across restarts.
- [ ] The production log still contains a **named list of lines that must be there** — startup,
      profile resolution, and the R-GOLD auto-approve record — after the gate lands. (Anti-fake: see §4 `P2-A4`.)
- [ ] `📁 ProfileManager initializing` appears in `debug_output.log` (it appears **0** times in
      2.54 GB today, M1).
- [ ] No torn lines: a concurrency test drives `Logger::Log` from N threads and every line is intact.
- [ ] `cef-native/CLAUDE.md`'s Logging section says what the code does.
- [ ] Existing installs' 2.4 GB is dealt with per the **owner's** retention decision (§8 Q1).

## 3. Invariants preserved

| ID | Invariant | Why this phase could break it |
|---|---|---|
| **R-GOLD** | Auto-approved payment shows the gold pill | Its **only log record** is `LOG_DEBUG_HTTP` in `OnWalletCallSuccess` (M8.1). A level gate silences it. The pill itself is IPC, not logging — but the *evidence* dies, and `OnWalletCallSuccess` is also the function being read while we touch the interceptor's logging. |
| **R-INTEXT** | Internal never prompts, external always gates | The permission cascade narrates itself at DEBUG and nowhere else. Down-levelling it removes the only way to audit a wrong decision after the fact. |
| **R-PERIM** | The four privacy-perimeter gates | `🛡️ 202 PENDING intercepted — modal opened (requestId=…)` is DEBUG. Same argument. ⛔ Also: the modal-resume `requestId` invariant means anything touching interceptor code risks the page hanging. |
| **R-UPDATE** | A staged update still applies | `SilentStateWriter` is one of the `[UNKNOWN]`-tagged files (M4) and writes update state. Any log path change near it must not change *when* it writes. ⛔ And `TICKET_stray_log_in_install_root` established that a log inside `{app}` breaks the backup hash — **rotation must not create a single file outside `AppPaths::GetLogDir()`**. |
| **R-COUNT** | Per-session counters reset on tab close | Untouched, but listed because `simple_handler.cpp` IPC dispatch is edited for the threading fix. |
| **R-CLOSE** | Overlay close guards | Untouched. Listed because `simple_app.cpp` has 22 `std::cout` sites that a mechanical conversion would touch. |

## 4. Evidence table

⛔ No empty RED or SUBJECT cells. A green result is reported with its red half or not at all.

| ID | 🟢 GREEN — must be true | 🔴 RED — must be *seen* to fail, and how | 🎯 SUBJECT | Tier | Result |
|---|---|---|---|---|---|
| ~~`P2-A1`~~ | **MOVED to `P2a-A1`/`P2a-A2` (§8.1) and GREEN.** Kept as a stub so row ids do not shift | **Already observed RED, pre-fix: 31.8 s / 128 s / no load event** (M5). Post-fix, re-arm the old path on the **same binary** via `HODOS_WALLET_SYNC_UI=1` and reproduce it | Dev build `build/bin/Release`, `HODOS_DEV=1`, `--profile=Default`, **CDP 9322**, wallet 31401 = `stub_wallet.py`. ⛔ Installed browser (9222 / 31301) never addressed — `cdp.py` refuses 9222 | T2 | ✅ moved |
| ~~`P2-A2`~~ | **MOVED to `P2a-A3` (§8.1). GREEN at T2 and T3 — owner-confirmed at the machine 2026-08-26.** | Set the timeout absurdly high → the spinner returns. Proves the timeout is what ends it, not chance | The **wallet overlay** browser, not the header. `dpr`-independent | T3 (human) | ✅ moved |
| `P2-A3` | Release-config `Logger` suppresses `LOG_DEBUG_*` **without formatting the string** | Unit test: a stub sink counts invocations *and* a formatting counter. Flip the level to DEBUG in the **same binary** → both counters rise. If only the write is skipped and the format still runs, the row is RED | `hodos_tests` (CEF-free target that already links `Logger.cpp`), release config | T1 | ⬜ |
| `P2-A4` | ⭐ **Anti-fake row.** After the gate, a real 10-minute session's `debug_output.log` still contains **every** line on the required list: `Logger initialized for MAIN`, `Profile resolved:`, `SitePermissionStore initialized`, the R-GOLD auto-approve line, and `📁 ProfileManager initializing` | Delete any one line's `LOG_` call → the check fails naming that line. ⛔ Without this row, "the log shrank by 99 %" is satisfied by writing nothing | `debug_output.log` **only** — not `cef_debug.log`. Assert the `[MAIN]`/`[BROWSER]` tag on each | T2 | ⬜ |
| `P2-A5` | No full URL in a production-config session log at any level: every URL is origin-only | Point a session at a URL with a query string (`?token=abc`), grep the log for `token=abc` → must be absent. **RED control:** revert the redaction on the same binary → it appears | Production config (`HODOS_DEV` unset is not runnable here — use the release **level default** with the dev data dir). State which | T2 | ⬜ |
| `P2-A6` | Rotation caps total bytes across restarts, **and content survives the boundary** | Write past the cap → oldest file gone, total ≤ cap. **Two-sided:** a line written 1 KB before a rotation must still be findable in the rotated set. A "rotation" that truncates passes the first half and fails the second | `AppPaths::GetLogDir()`. ⛔ Assert **nothing** was created inside `{app}` (`TICKET_stray_log_in_install_root`) | T1 + T2 | ⬜ |
| `P2-A7` | `Logger::Log` from N threads produces N intact lines, none torn | Remove the lock in the same test binary → torn lines reappear. Baseline for "torn" is the measured shape: a line with no `[` timestamp prefix (8,208 exist today, M2) | `hodos_tests` | T1 | ⬜ |
| `P2-A8` | `📁 ProfileManager initializing` present in `debug_output.log`; **0** `[UNKNOWN]` process tags | **Pre-fix RED already measured**: 0 occurrences and 470 `[UNKNOWN]` (M1, M4). Same-binary control: revert one converted call site → it vanishes again | `debug_output.log`, `[MAIN]` tag | T2 | ⬜ |
| `P2-A9` | `cef_debug.log` bounded and at WARNING in release | Set `log_severity` back to INFO → it resumes growing at the measured rate | `cef_debug.log`, release config. ⚠️ Do **not** silence `[RENDER]` — assert our 255-line channel still works | T2 | ⬜ |
| `P2-G9` | T0 gate: no new `std::cout`/`std::cerr`/`printf` in `cef-native/src|include` outside `Logger.cpp`. **Ratcheted**, baseline = today's count | `preflight.ps1 -NegativeControl -Only G9` injects one → gate fires | Windows tree only. ⚠️ Scope `.mm` deliberately or state why not (the G8 lesson: a wrong-scoped baseline hides the line that matters) | T0 | ⬜ |
| `P2-G10` | T0 gate: `Logger::Log` contains both a level comparison and a lock acquisition | Delete either → gate fires | `src/core/Logger.cpp` | T0 | ⬜ |

**Two-sided pairing.** `P2-A3`/`P2-A4` are each other's control and are the heart of this phase:
A3 says *the noise is gone*, A4 says *the signal is still there*. **A fix that satisfies one by
breaking the other is the failure mode**, and neither row can see it alone. Likewise `P2-A5`
(no URLs) is paired with `P2-A4` (the required lines survive) — redaction must not eat the evidence.

## 5. Blast radius — written before the code, not after

⭐ Per `feedback_risk_list_before_code`: 0.9's branding was an hour of work inside a day-long phase
because ~60 % was shared-overlay coupling nobody wrote down. This list is the attempt not to repeat that.

**Files a level gate touches indirectly:** every `.cpp` that logs — **~90 local macro families**
across the tree (CLAUDE.md lists 22 suffixes). The gate itself is one function, but the *decision*
about which lines are promoted touches the interceptor, the permission path and the profile manager.

| Risk | Why it is real here | Mitigation in the plan |
|---|---|---|
| **Silencing the audit trail** | R-GOLD, 202-PENDING and the permission cascade are all DEBUG (M8.1) | `P2-A4` names them explicitly; promotion happens **in the same commit** as the gate |
| **Two writers on one file** | The `freopen` fallback exists *because* `Logger` already holds the file. "Making the redirect work" would create the exact double-writer that tore 8,208 lines | ⛔ Do **not** fix the blackhole by making `freopen` succeed. Convert call sites to `Logger` instead |
| **Rotation racing the silent updater** | `TICKET_stray_log_in_install_root` — a file inside `{app}` broke the backup hash. A rotating writer creates *and deletes* files | All paths via `AppPaths::GetLogDir()`; `P2-A6` asserts `{app}` stays clean |
| **Moving the wallet call off the UI thread** | CLAUDE.md invariant #8: CEF threading is fragile. The renderer already handles `get_balance_response`/`_error` asynchronously and `initWindowBridge.ts` already has a timeout — so the **contract is already async**; only the C++ side blocks | ⛔ **Owner decision Q4.** Nothing lands here without it |
| **`WalletService` destructor calls `stopDaemon()`** | Safe today only because `daemonProcess_` is zeroed in the ctor. Reusing a long-lived `WalletService` changes that assumption | Audit `stopDaemon` before any lifetime change |
| **`[UNKNOWN]` renumbering** | `SilentStateWriter` is one of them and writes update state (R-UPDATE) | Change the *tag*, never the write timing |
| **The 2.42 GB file is evidence** | It is the only record of the beta.1 incident window | ⛔ Not truncated or deleted by us. Owner decides (Q1) |
| **macOS** | The redirect is never attempted there (M1); the sink install is in `process_helper_mac.mm`; rotation must work on a sandboxed `.app` | Relay asks D5.1/D5.2; no macOS claims made from Windows |

## 6. Out of scope

- **The `[RENDER]` → `cef_debug.log` split.** It is the one documented thing that measured correct (M3).
- **`SyncHttpClient`'s own call sites** (31 in the interceptor). They already set timeouts. Only
  `WalletService` is migrated.
- **The `2.0 s` balance cadence.** `BALANCE_POLL_MS = 30_000` does not explain the incident's uniform
  2.0 s gaps (M5). Recorded as unexplained; chasing it is not needed to fix the hang.
- **The zero-ERROR-lines question** (M0.1). Noted, not chased.
- **Structured/JSON logging.** Tempting while rewriting the formatter. Not this phase.
- **A telemetry or crash-report channel.** Explicitly against the product thesis.
- Phase 1 leftovers: the overlay dead strip, the DPI matrix overlay section, the three unscheduled tickets.

## 7. Rollback

One commit reverts. The level gate, the lock and the URL redaction are additive edits inside
`Logger.cpp` / `Logger.h` plus a bounded list of call sites; rotation is a new file behind a flag.
`git revert` restores today's behaviour, which is "log everything forever" — no data migration to undo.
⚠️ The **exception** is the one-time cleanup of existing installs (§8 Q1): deleting a user's log is
not revertible, which is why it is the owner's call and lands last, separately.

---

## 8. Owner decisions — ✅ ALL SETTLED 2026-08-26

| # | Question | ✅ Decision |
|---|---|---|
| **Q1** | Retention | **Delete, don't keep.** Rotate at **10 MB, keep 5**, *and* delete anything older than **30 days** — whichever comes first. The existing 2.4 GB is **deleted once on upgrade**. Owner: *"What is the point of keeping these old logs forever? They are just logs."* ⚠️ Deferred one build: the owner's current 2.42 GB file is the before/after baseline for 2a and is deleted only after 2a lands. |
| **Q2** | Production default level | **INFO.** ⛔ The ticket's "defaulting to WARNING" is **rejected** — M8 measured that it leaves 401 lines in 50 days and zero ERRORs. |
| **Q3** | Separate always-on audit log | **Yes, small.** Payments, permission grants **and denials**, profile deletions. Kept longer than the debug log and never rotated away by log noise. This is what makes Q1's aggressive deletion safe. |
| **Q4** | Move `get_balance` off the UI thread | **Yes — both (a) and (b), in that order.** (a) a per-call timeout, small and independently verifiable; (b) the async move that actually removes the freeze. |
| **Q5** | History log lines | **Delete them.** |
| **Q6** | Split the phase | **Yes. 2a = the freeze** (this contract's `P2a-*` rows), **2b = logging** (`P2-*`). 2a ships first. |

### 8.1 Phase 2a — the freeze · evidence rows

⚠️ **macOS carries the identical defect.** `WalletService_mac.cpp` already sets
`CURLOPT_TIMEOUT 30L` — the *same* 30 s exposure as WinHTTP's default, just explicitly. Both
platforms need both fixes; (a) is not a Windows-only change.

⚠️ **One global short timeout would be wrong.** `makeHttpRequest` also carries `/transaction/send`,
which legitimately takes seconds while it broadcasts. (a) is a **per-call** timeout — short for
balance, generous for sends — not one constant.

| ID | 🟢 GREEN | 🔴 RED — must be *seen* to fail | 🎯 SUBJECT | Tier | Result |
|---|---|---|---|---|---|
| `P2a-A1` | **(a)** With the stub hanging, the UI-thread block per balance call is capped by the timeout, not 30 s | 🔴 **SEEN**: pre-fix 31.8 s / 31.4 s (M5); with the lever restoring the sync path on the SAME binary, 3.9 s | Dev build, CDP 9322, wallet 31401 = `stub_wallet.py`; installed browser never addressed (`cdp.py` refuses 9222) | T2 | 🟢 **GREEN** |
| `P2a-A2` | **(b)** With the stub hanging, the UI thread is never blocked | 🔴 **SEEN**: `HODOS_WALLET_SYNC_UI=1` on the same binary → max 3.864 s vs 0.027 s control | ~200 continuous `/json/list` samples per arm. ⛔ `Page.navigate` timing was REJECTED as the instrument — the control arm itself measured 2.040 s (M5b) | T2 | 🟢 **GREEN** — max **0.034 s** hung vs 0.037 s control |
| `P2a-A3` | A hung wallet leaves the **last known balance** on screen with a visible staleness notice — never a spinner, never $0.00 | 🔴 **SEEN, three-point, owner-observed 2026-08-26**: healthy = no notice; hung = notice appears; recovered = notice clears. Each state is the others' control. Pre-fix the same run showed a confident **$0.00** | **Owner at the machine**, dev browser + stub. Value asserted ($4.78 held), not just absence of a hang | T2 + **T3** | 🟢 **GREEN — owner-confirmed** |
| `P2a-A6` | A wallet outage does not poison the cached balance | 🔴 **SEEN**: writing the cache the pre-fix way gives `{"balanceKeyPresent":false,"readsBackAs":0}` — `JSON.stringify` DROPS an undefined value, so it reads back as a confident zero | `localStorage` in the header browser, across a **75 s** outage (> 2 poll cycles) | T2 | 🟢 **GREEN** — `28332055` unchanged, `poisoned: false` |
| `P2a-A4` | `/transaction/send` still completes when it legitimately takes > the default budget | 🔴 **SEEN**: server delayed 35 s (> the 30 s broadcast budget) → send **fails at 34.55 s** | Stub `/transaction/send`, canned txid — no money moves | T2 | 🟢 **GREEN** — 8 s send succeeds in 8.02 s |
| `P2a-A5` | Balance replies still arrive and carry the real value | 🔴 A reply that never arrives shows as the JS 10 s timeout — **observed 1 run in 3** before the bridge race was fixed | Value asserted, not just absence of hang: `{"balance":28332055,...}` | T2 | 🟢 **GREEN** — 4/4 clean |

**Two-sided pairing:** `P2a-A2` (no freeze) and `P2a-A5` (the answer still arrives) are each other's
control. Making the browser fast by never replying passes A2 alone.

---

## Sign-off

- [ ] Every evidence row GREEN **and** its RED observed
- [ ] `scripts/preflight.ps1` run — result + date recorded below
- [ ] `scripts/preflight.ps1 -NegativeControl` run — every T0 gate seen to fail
- [ ] `../REGRESSION_SET.md` run in full at this boundary — result recorded
- [ ] Adversarial review complete, four questions answered in writing
- [ ] Any baseline lowered in `../HARNESS.md` §4, residuals listed with reasons
- [ ] Commit messages cite the row IDs they satisfy

| Item | Result | Date | By |
|---|---|---|---|
| preflight | | | |
| preflight -NegativeControl | | | |
| regression set | | | |
| adversarial review | | | |
