# 🎫 If the wallet backend dies, the browser never notices, never restarts it, and tells the user "no wallet"

**Found:** 2026-09-01, while diagnosing a dApp payment failure on the owner's installed build
**Status:** 🟡 **ASSIGNED — Phase 8, after 8c closes** (owner call 2026-09-12) · **Sprint:** beta.3.
Phase 8 (money-path correctness) is its home because this is the availability half of the same work.
⚠️ 8c batch 2 (`O9`) measured the symptom from a *migrated* call: with the wallet stopped,
`address.generate` **resolved `{}` after 6.2 s on the UI thread** — the "throw a real wallet error"
half of that finding is being fixed inside 8c; the "notice the wallet died and restart it" half is
**this** ticket, and must not be attempted per call site.
📏 **Extra evidence for F2, 2026-09-12:** `WalletService::isConnected()` is also a latch —
`WinHttpConnect` only allocates a handle, it never contacts the wallet, so `connected_` reads **true**
with the wallet dead (measured: an `isConnected()` guard in the address handler never fired). Any
supervision design has to use a real probe (`/wallet/status`) or the child process handle, never this flag
**Filed by:** Phase 4 close-out · **Platforms:** 📏 **both** (see §2.1)

> ⚠️ **Method note.** The mechanism below is **code reading**, stated per claim. The *symptom* was
> observed live on the owner's installed build (Aug 17) for roughly **four hours**. ⛔ **Honest
> provenance: the trigger was mine** — I killed the backend with a name-matched `Stop-Process` (see
> §6). The **defect is independent of how it died**, and §5 lists ways it dies without my help.

---

## 1. What happens

The browser launches `hodos-wallet.exe` **once, at startup**, and then never looks at it again. If
that process goes away, the browser carries on with a stale "it's running" flag, every wallet call
fails at the transport layer, and the user is told — by any dApp that asks — that **there is no
wallet**.

📏 Observed: the owner's installed browser (71 live processes, perfectly responsive) with nothing on
`127.0.0.1:31301`. Every site reporting "no wallet". 👤 The owner's own read, in order: *"it must be
us"* → *"strange, we should not have touched anything"* → *"wait, my installed wallet is broken."*
⇒ the message sent them down two wrong paths before the right one.

## 2. Mechanism — three findings, each verified separately

| # | Claim | Evidence |
|---|---|---|
| **F1** | The wallet is spawned **once at startup** and never re-checked | `cef_browser_shell.cpp` — `CreateProcessA(walletExe…)`, then `AssignProcessToJobObject`. No watcher on `g_walletServerProcess.hProcess` after that |
| **F2** | `g_walletServerRunning` is a **latch, not a state** | Set `true` on launch, and set `true` **again** by `WaitForWalletHealth()` on the *unhealthy* path — comment reads *"Process was launched, just slow to start"*. Nothing ever sets it back to `false` while running |
| **F3** | The one piece of code that **does** detect death is unused, and would not fix it anyway | `WalletService::monitorDaemon()` polls `GetExitCodeProcess` every 5 s — but on exit it only logs a WARNING, sets `daemonRunning_ = false`, and `break`s. **No restart.** And 📏 `startDaemon()` / `isDaemonRunning()` have **zero call sites outside `WalletService.cpp` itself** — the whole daemon-management API is dead code; production uses the `cef_browser_shell.cpp` path in F1 |

⇒ there is no supervision on the live path, and the dormant supervision would not have restarted it.

## 2.1 📏 It is CROSS-PLATFORM, and macOS is very slightly better

⛔ Do not scope this Windows-only. `cef_browser_shell_mac.mm` has the same shape: `SpawnWalletServer()`
once, a startup health-check loop, and then **nothing re-checks**. 📏 `g_walletServerRunning` there is
only ever *set* `true` (2 sites) and *read* (3 sites) — never set back to `false`.

| | Windows | macOS |
|---|---|---|
| Spawn once, never watched | ✅ same defect | ✅ same defect |
| Flag forced `true` when the health check **fails** | 🚨 **yes** — `WaitForWalletHealth()`: *"Process was launched, just slow to start"* | ⭐ **no** — logs *"health check timed out"* and leaves it `false` |

⇒ the core defect is shared; **F2 (the dishonest latch) is Windows-only**. Fix the supervision once,
cross-platform; fix F2 in the Windows arm.

⚠️ **The adblock engine has the identical pattern** (`g_adblockServerRunning`, both platforms). Its
failure is far less alarming — ads stop being blocked — but whatever supervision is built should
cover both backends rather than being written twice.

## 3. Why the message is the worst part

⛔ **"No wallet" is indistinguishable from "your wallet is gone."**

This is a browser that holds real money. A user who sees *no wallet* on a site they were about to pay
has no way to tell apart:

- the backend process died (**recoverable, nothing lost — the actual case**),
- the wallet DB is missing or corrupt,
- they are in the wrong profile,
- they never had a wallet.

🚨 **The dangerous path:** a user who concludes their wallet is gone may go and **restore from their
recovery phrase** to "get it back" — a needlessly risky operation involving the one secret we ask
them to keep offline, performed under stress, to fix a problem whose real remedy is *restart the app*.

⭐ The truthful signal is trivially available: `/health` on `hodos::WalletPort()` either answers or it
does not. `QuickHealthCheck()` **already exists** in `cef_browser_shell.cpp` and is already used at
startup. Nothing needs inventing — it needs calling more than once.

## 4. How exposed are we

| If | Then |
|---|---|
| The backend dies mid-session | ⚠️ Silent. Every wallet call fails; dApps report "no wallet"; balance/send/receive all fail; **no indication the cause is a dead process** |
| The backend never starts (port taken, AV quarantine, missing exe) | Same end state. `WaitForWalletHealth()` logs a WARNING and then sets the flag to `true` anyway (F2) |
| The backend is healthy | No impact — this is invisible in the happy path, which is why it has survived |

⚠️ **Related, already known:** beta.3 Phase 2 found that a wallet outage rendered a confident
**$0.00** balance and poisoned the cache, and that *nothing rendered an error state*. This ticket is
the same family one level down: there, the wallet was **hung**; here, it is **absent**. The Phase 2
fixes addressed the freeze, not the "what do we tell the user" half.

## 5. It reaches users without anyone running a kill command

⛔ Do not dismiss this as self-inflicted. The backend is an ordinary user-space process:

- it crashes (a panic in the Rust wallet ends it — and it is the process doing BEEF assembly, SQLite
  writes and network I/O);
- **antivirus quarantines or blocks it** — the installer adds a firewall rule for it, which is a hint
  at how often it draws attention;
- the user kills it from Task Manager, having seen an unfamiliar `hodos-wallet.exe`;
- Windows ends it under memory pressure;
- a failed or partial auto-update leaves the exe replaced but not running.

📏 In every one of those, today's outcome is identical to what was observed: silent, unrecovered,
misreported.

## 6. ⛔ How this was triggered here — a dev-hygiene defect worth its own fix

The backend was killed by **me**, matching on image name to free a file lock before a build:

```powershell
Get-CimInstance Win32_Process -Filter "Name='hodos-wallet.exe'" | Stop-Process -Force   # ⛔ WRONG
```

**Both builds ship the same image name**, so this takes down the *installed* browser's backend along
with the dev one. ⚠️ The beta.3 session prompt states this rule explicitly — *"Match by exe path,
never by process name"* — but states it **only for `HodosBrowser.exe`**. It was honoured there and not
carried across to the wallet.

The correct form, and the one that should be in the runbook:

```powershell
Get-CimInstance Win32_Process -Filter "Name='hodos-wallet.exe'" |
  Where-Object { $_.ExecutablePath -like '*rust-wallet\target\release*' } |
  Stop-Process -Force
```

⇒ **sub-item: extend the exe-path rule to `hodos-wallet.exe` (and the adblock engine) in
`CLAUDE.md`'s Dev Runbook**, so it is not a rule that lives only in one sprint's session prompt.
⭐ Cheap, and it prevents a whole class of "why is my installed build broken" confusion.

## 7. Proposed fix

**The floor — say the true thing.** Poll `QuickHealthCheck()` on the existing background cadence and
drive one honest state. When the backend is unreachable, wallet-dependent UI says **"Wallet service
not running"** with a *Restart* action — never "no wallet". ⭐ This alone removes the recovery-phrase
hazard in §3, and it is the half that protects the user.

**The system — bring it back.** Relaunch the backend when the health check fails and the process
handle is dead, with bounded retries and backoff (⛔ not an unbounded respawn loop — the exe may be
quarantined or the port taken, and a hot loop there is worse than the outage). `AssignProcessToJobObject`
already ensures a relaunched child dies with the browser.

⚠️ **Do not just wake `WalletService::monitorDaemon()`** — F3 shows it is on a dead path *and* has no
restart. Fixing it in place would put supervision in a class the browser does not use.

⚠️ **Sequencing:** this touches the startup path (`LaunchWalletProcess` / `WaitForWalletHealth`), which
is also where startup-time optimisation work lives. Worth checking that a health poll does not
regress first paint — the wallet launch was deliberately moved off the critical path.

## 8. Not in scope

⛔ The `WalletService` daemon API (`startDaemon` / `stopDaemon` / `isDaemonRunning` / `monitorDaemon`).
📏 It has no callers. It is either **deleted** or **adopted**, and that is its own decision — filing it
here would smuggle a dead-code cleanup into a resilience fix.

## 9. Related

- `0.4.0-beta.3/phase-2-logging-syncio/` — wallet **hang** handling; this is wallet **absence**.
- `CLAUDE.md` → Dev Runbook — where the exe-path rule in §6 belongs.
- `MAC_RELAY_P35_P4_ROUND.md` §M11 — relayed to the macOS side, with §2.1's split.
- 📌 Incidental, from the same session: on **current** code the dApp that prompted this
  (`chaintap.utxoengineer.com`) works end to end — domain-approval prompt → auto-approve →
  `POST /createAction 200`, 4 cents, access token granted. ⇒ **no dApp-compatibility ticket is owed**;
  the failure was entirely this one.
