# Session prompt — beta.3 **Phase 2 (WS1b(b)): logging & synchronous-I/O review**

Written 2026-08-26, after Phase 1 (WS1) landed as `9aae090..02a6f56` on `origin/0.4.0`.

`SPRINT_PLAN.md` §3 defines this as **WS1b(b)** and §4 places it at **Phase 2**, directly after
WS1. WS1b(a) — the 52 stray `{app}` log writes — closed in Phase 0.

⛔ CLAUDE.md's **mandatory phase kickoff workflow applies**: re-read the docs, verify every cited
`file:line` still exists as described, reuse-first audit, risk assessment against the load-bearing
safeguards, confirm the test plan, **hand back a summary before writing any code.** Create
`phase-2-logging-syncio/` and write `PHASE_CONTRACT.md` from `PHASE_CONTRACT_TEMPLATE.md` as part of
the kickoff. ⭐ Per `feedback_risk_list_before_code`, the risk list goes in the contract **first**,
not in a post-mortem.

---

Paste the block below to open the session.

---

Start **beta.3 Phase 2 — WS1b(b), logging and synchronous-I/O review**. ⛔ **Kickoff first: deep
research and adversarial review, and no code until I sign off on the assessment.**

## 0. Read first

1. **Auto-loaded MEMORY.md**, plus the Phase 1 results — `development-docs/0.4.0-beta.3/phase-1-overlay-input-dpi/MEASUREMENTS.md`.
   Phase 1 turned up findings that belong to **this** phase; §2 below.
2. `development-docs/0.4.0-beta.3/SPRINT_PLAN.md` — **§3 WS1b** (both halves; (a) is done) and §4.
3. `development-docs/0.4.0-beta.3/TICKET_production_debug_logging_unbounded.md` — the anchor ticket.
4. `cef-native/CLAUDE.md` → **Logging** section. ⛔ It contains a claim Phase 1 measured to be
   **false**; see §2.2. Treat the whole section as suspect until re-verified.
5. `development-docs/0.4.0-beta.3/HARNESS.md` — tiers, and what can and cannot be a gate.

## 1. What this phase is

Two halves that share a cause: **nobody knows which log lines actually land, or what they cost.**

### (a) The flood — production writes DEBUG forever, with every URL in plaintext

MEASURED 2026-08-26 on this machine:

| | Size | Since |
|---|---|---|
| `%APPDATA%\HodosBrowser\logs\debug_output.log` | **2,422.9 MB** | 2026-07-06 |
| `%APPDATA%\HodosBrowserDev\logs\debug_output.log` | 277.3 MB | 2026-07-03 |

⚠️ **The ticket says 1.58 GB. That was 2026-08-17.** It is now **2.42 GB** — roughly **+93 MB/day**
and still climbing. Re-measure before quoting any figure.

`Logger` has **no level gate and no rotation** — grep for `minLevel|rotat|maxSize` in
`include/core/Logger.h` and `src/core/Logger.cpp` returns **nothing**. So a privacy browser keeps a
plaintext record of **every URL the user has visited**, which **survives them clearing their own
history**, and grows without bound.

### (b) Synchronous I/O on the browser UI thread

The owner's framing. `getBalance` alone does **4 open/write/close cycles per call** plus a
synchronous wallet HTTP request. The beta.1 incident window logged **417 balance calls in 52
minutes** at uniform **2.0 s** gaps — a shape that looks like a timeout, not contention.

### ⛔ And the incident itself is still OPEN

Installed **beta.1** went unresponsive: balances, the advanced wallet, local DB reads **and ordinary
web pages** all stalled together, recovering only after a second restart.

- **Established:** the price was fine throughout (`bsvPrice: 15.145`, fallback chain intact); the
  failure was specifically `/wallet/balance` returning no balance from 15:18:02 onward.
- **Unexplained:** why *web pages* stalled too.
- ⛔ **Must be reproduced, not inferred.** One experiment settles it: **stub `/wallet/balance` to
  hang and see whether page loads stall with it.** Do that early — it is cheap, and it decides
  whether (b) is a performance clean-up or a hang bug.

## 2. ⭐ Phase 1 findings that land in this phase

### 2.1 The subsystem has a **blackhole** as well as a flood

MEASURED 2026-08-26: **`ProfileManager`'s `std::cout` / `std::cerr` reaches no log at all** — not
`debug_output.log`, not `cef_debug.log`. **Zero** occurrences of `📁 ProfileManager initializing`, a
line that runs on every single startup.

Consequence: every profile-delete refusal has been silent — *"cannot delete the last profile"*,
*"…the default profile"*, *"…the profile this window is running on"*. Phase 1 converted **only the
delete path** to `Logger`. The rest of that file, and any other file using `std::cout`, is still
invisible.

⇒ So there are **at least three distinct problems**, and they pull in opposite directions:

| # | Problem | Direction |
|---|---|---|
| 1 | Production writes DEBUG forever, no rotation | far too much |
| 2 | `std::cout` from at least one core file lands nowhere | nothing at all |
| 3 | Renderer-process logging goes to `cef_debug.log`, and was **entirely dead** until 2026-08-09 — which is why a total farbling failure went unreported for the whole life of that feature | wrong place, silently |

⭐ **Framing this as "add a level gate" would miss two thirds of it.** The real question is *which
sink does a given line reach, on which platform, from which process* — and right now nobody can
answer that from the code.

### 2.2 ⛔ A documented claim that measurement contradicts

`cef-native/CLAUDE.md` states, of `std::cout`: *"stdout is redirected to the same file anyway."*
**Measured false on Windows** (§2.1). Either the redirection never happens, or it happens after
`ProfileManager::Initialize` runs, or the CEF 150 bootstrap/DLL model changed it. **Find out which**,
then correct the doc — it is actively steering people toward a sink that does not work.

⚠️ The macOS half is unknown; it is ask **D5.1** in `MAC_RELAY_P1_ROUND.md`.

## 3. Adversarial review — required before any plan is accepted

- Label every finding **claim** vs **measurement**. Phase 1's premise was wrong twice because an
  inference got recorded as a measurement.
- ⛔ **Every proposed check needs its negative control, shown to go RED.** This project has shipped
  harnesses that would have passed with the feature absent.
- ⚠️ **A logging gate is unusually easy to fake.** "The log is smaller" is satisfied by writing
  nothing at all. Any rotation/retention check must also prove the lines that *should* be there
  **still are** — the flood and the blackhole are the same phase, and fixing one by widening the
  other is the obvious failure mode.
- ⚠️ **Assert the right SUBJECT:** which process wrote the line (MAIN / RENDER / BROWSER), and which
  file it landed in. `debug_output.log` and `cef_debug.log` are different sinks with different
  writers.
- For each proposed fix: what would make it wrong? Would it regress something that works today?

## 4. Blast radius — audit explicitly

- ⚠️ **Log lines are the only evidence for several shipped safeguards.** Before deleting or
  down-levelling anything, check it is not the sole record of a security-relevant decision. The
  permission cascade, the BRC-121 paid retry and the profile-delete refusals all narrate themselves
  through the log and nowhere else.
- **`R-GOLD`** — the gold-pill emit path logs; do not let a level gate silence the one line that
  proves a payment was auto-approved.
- **Retention is a product decision, not a technical one.** Deleting a user's log is destroying data
  they may need to diagnose a lost payment. Rotation + a cap is not the same as retention, and the
  cap belongs to the owner, not to whoever writes the code.
- ⛔ `TICKET_stray_log_in_install_root.md` (Phase 0) already established that a log inside `{app}`
  breaks the silent-update backup hash. Any new path must stay outside the install root.

## 5. Deliverable

A written assessment plus `phase-2-logging-syncio/PHASE_CONTRACT.md` that I sign off **before** any
code:

1. which sink each `LOG_*` / `std::cout` call actually reaches, per process and per platform —
   measured, not read;
2. gaps ranked by likelihood × blast radius;
3. proposed changes — level gate, rotation, retention, and the sync-I/O findings, separated;
4. each check with its negative control;
5. the `/wallet/balance` hang experiment, run, with its result;
6. what stays manual, said plainly.

## 6. Machine state

- ⛔ The owner's **installed** browser (`%APPDATA%\HodosBrowser`, ~60 procs) and its wallet on
  **31301** must never be touched. Match by **exe path**, never process name. Its 2.42 GB log is
  **evidence** — do not truncate or delete it without asking.
- Dev stack is **stopped**. Wallet **31401**, Vite **5137**, dev browser from
  `cef-native/build/bin/Release`. ⛔ `cargo build` fails "Access is denied" while the dev wallet
  runs — stop that PID by path.
- ⚠️ The dev browser starts in the **profile picker** when >1 profile exists, and picker mode sets
  `remote_debugging_port = 0`, so **CDP is off entirely**. `--profile=Default` bypasses it.
- `phase-1-overlay-input-dpi/cdp.py` is a reusable CDP helper that refuses port 9222 and errors on
  an ambiguous target rather than guessing.
- ⚠️ Phase 1 left three test fixtures in `%APPDATA%\HodosBrowserDev\` —
  `Profile_7.orphaned-1756000000`, `Profile_8`, `Profile_9.orphaned-1787756666`. Inert; remove when
  convenient.
- ⛔ Never commit `development-docs/X402_INTEGRATION.md` or
  `development-docs/Onchain-Backup-and-Sync/*` — the owner's parallel work. Stash/pop around a rebase.

## 7. Carried from Phase 1, not part of this phase

- **Item 1, the overlay dead strip** — measured for all 8 overlays; our menu matches macOS's 45 px
  exactly. Deliberately deferred to one cross-platform sizing contract; needs the Mac side's input
  (`MAC_RELAY_P1_ROUND.md` §D1).
- **`DPI_RESOLUTION_TEST_MATRIX.md` still has no overlay section** — the last outstanding Phase 1
  kickoff deliverable. ⭐ Now known to be short: fixed-size overlays are **DPI-invariant** (their CSS
  box is a constant by construction), so only the header, the notification overlay and the wallet
  panel actually reflow.
- Tickets filed but not scheduled: `TICKET_chrome_ui_scales_but_its_window_does_not.md`,
  `TICKET_longlived_surfaces_snapshot_state_at_startup.md`,
  `TICKET_deleted_profile_id_reused_over_orphaned_data.md` (residual: `CreateProfile` still does not
  refuse a populated target directory).

## 8. Working style

⭐ Walk me through anything I need to click **step by step**. I am doing the clicking. Tell me what
to do, what I should see, and what a wrong result would mean.
