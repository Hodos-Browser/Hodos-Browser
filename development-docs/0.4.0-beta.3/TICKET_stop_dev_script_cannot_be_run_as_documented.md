# `stop-dev.ps1` fails to start when invoked the way the docs say to invoke it

**Status:** ✅ **FIXED 2026-09-08**, same day, in `scripts/stop-dev.ps1`. Verified by the acceptance list below with the dev wallet AND the installed wallet both running: **19 dev processes stopped, the installed wallet and all 74 installed browser processes survived.** The documented no-argument invocation now works. Matching logic untouched.
**Sprint:** 📌 Unassigned. ⭐ **Small and safety-relevant — worth doing out of band rather than queuing.**
**Surface:** `scripts/stop-dev.ps1` line 28.

---

## Why this is not a cosmetic script bug

This is the tool that exists **because of the 2026-09-01 incident**, where a name-matched
`Stop-Process` on `hodos-wallet.exe` killed the owner's **production** wallet backend. The installed
browser then reported *"no wallet"* to every dApp for ~4 hours while the owner reasonably concluded
first that a site was broken, then that the browser was.

Root `CLAUDE.md` responds to that with a rule and a tool:

> ⭐ **Use the script. Do not hand-write the kill:** `.\scripts\stop-dev.ps1`

⛔ **That exact invocation does not work.** And the pressure that produces a hasty hand-written kill —
a linker holding `HodosBrowser.exe`, `cargo build` failing *"Access is denied"* — is at its highest
precisely when someone runs this and it errors out. A safety tool that will not start is how people
go back to the thing it was built to prevent.

⭐ **The lesson from that incident was "embody a rule in a tool, don't rely on remembering it."**
This is the tool. It has to run.

## The defect

```powershell
[string]$RepoRoot = (Split-Path -Parent $PSScriptRoot)
```

`$PSScriptRoot` is empty at **parameter-binding** time in this invocation, so `Split-Path` throws
before the body runs:

```
Split-Path : Cannot bind argument to parameter 'Path' because it is an empty string.
At C:\Users\archb\Hodos-Browser\scripts\stop-dev.ps1:28 char:45
```

**Measured 2026-09-07**, Windows PowerShell 5.1, invoked as
`powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\stop-dev.ps1`.

## What is NOT wrong — do not "fix" the matching logic

⛔ The path-matching itself is correct and must not be touched. With `-RepoRoot` supplied explicitly
it behaves exactly as designed:

```
Stopped 20 dev process(es); left 77 non-dev process(es) alone.
^ those are the installed build's. Never stop them from here.
```

`-WhatIf` listed only `...\Hodos-Browser\cef-native\build\bin\Release\HodosBrowser.exe`,
`...\rust-wallet\target\release\hodos-wallet.exe` and the dev adblock path, and **spared** every
`C:\Users\archb\AppData\Local\HodosBrowser\*` process. The guard works. Only the startup is broken.

## Workaround until fixed

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\stop-dev.ps1 -RepoRoot "C:\Users\archb\Hodos-Browser"
```

## Fix sketch

Resolve the root **inside the body**, where `$PSScriptRoot` is reliably populated, and fall back to
`$MyInvocation.MyCommand.Path` — leaving the parameter overridable:

```powershell
param([string]$RepoRoot)
if (-not $RepoRoot) {
    $here = if ($PSScriptRoot) { $PSScriptRoot }
            else { Split-Path -Parent $MyInvocation.MyCommand.Path }
    $RepoRoot = Split-Path -Parent $here
}
```

## Acceptance

- [x] `.\scripts\stop-dev.ps1` runs with **no arguments** — verified from the repo root
- [x] `-WhatIf` still names only build-directory paths and still reports the installed build spared
      (`Stopped 0 dev process(es); left 76 non-dev process(es) alone`)
- [x] ⭐ **The property the tool exists for, measured directly** with both wallets running:

      installed wallet  : 1  SURVIVED
      dev wallet        : 0  stopped
      installed browser : 74 SURVIVED
      dev browser       : 0  stopped

- [ ] ⚠️ **Not verified on PowerShell 7.** Fixed and tested on Windows PowerShell 5.1 only. The
      `$MyInvocation` fallback is there for the case where `$PSScriptRoot` is empty; on 7 it should
      simply never be reached. Worth one run if 7 is ever used here.

## 🍎 macOS half — `scripts/stop-dev.sh` ADDED 2026-09-08

`MAC_RELAY_P7C_ROUND.md` M6 flagged that there was **no macOS equivalent**, and that the hazard is not
Windows-specific: `HodosBrowser` / `hodos-wallet` / `hodos-adblock` share their names with the
installed build on macOS too, and the owner runs an installed Hodos on that Mac. `pkill -f hodos-wallet`
is exactly the shape that caused the 2026-09-01 incident.

⛔ **It does not use `pgrep -f`.** `pgrep -f` matches the **argument vector**, and argv[0] is whatever
the launcher passed: a browser started as `./build/bin/HodosBrowser.app/...` has a RELATIVE argv[0] and
is invisible to an absolute-prefix match. 📏 Measured 2026-08-26 — that left two browsers running on one
profile and looked exactly like profile corruption. The script reads **`ps -axo comm=`**, the path the
**kernel** executed, which is absolute however the process was launched.

⚠️ **Two defects found by running it, both fixed before commit** — neither would have shown up in a read:
1. `basename` printed `illegal option -- z` for every login shell, whose `comm` is **`-zsh`** and is
   parsed as a flag. Now `${path##*/}`.
2. The browser spawns the wallet through a relative hop
   (`.../build/bin/HodosBrowser.app/Contents/MacOS/../../../../../../rust-wallet/target/release/hodos-wallet`),
   so a textual repo-root prefix test would also accept a path that starts inside the repo and then
   `..`s **out** of it — the exact case the script exists to refuse. Paths are canonicalised with
   `pwd -P` before the prefix test.

### Acceptance — measured with BOTH builds running

⭐ The negative control is *"the installed wallet survives"*, not *"the script ran"*:

```
                         BEFORE    AFTER
installed browser procs    10   →    10     unchanged
installed wallet            1   →     1     SURVIVED
installed wallet :31301  LISTEN →  LISTEN   still serving
dev processes              10   →     0
dev wallet     :31401    LISTEN →   gone
```

- [x] `./scripts/stop-dev.sh` runs with **no arguments** from the repo root
- [x] `--dry-run` names only build-directory paths and lists every `/Applications/...` process as spared
- [x] ⭐ the installed wallet and all installed browser processes survived a real run

## Related

- Root `CLAUDE.md` § *"Stopping a dev process: match by EXE PATH, never by image name"*
- `phase-7c-quiet-mode/PHASE_CONTRACT.md` §9.2 — where this was first recorded
