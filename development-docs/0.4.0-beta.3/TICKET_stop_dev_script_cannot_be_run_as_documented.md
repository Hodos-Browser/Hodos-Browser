# `stop-dev.ps1` fails to start when invoked the way the docs say to invoke it

**Status:** 🔴 OPEN — filed 2026-09-08, measured during Phase 7c.
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

- [ ] `.\scripts\stop-dev.ps1` runs with **no arguments**, from the repo root and from `scripts\`
- [ ] `-WhatIf` still names only build-directory paths and still reports the installed build spared
- [ ] ⛔ **Negative control:** with a dev wallet **and** the installed wallet both running, confirm the
      script stops the dev one and the installed one **survives** — the actual property the tool
      exists for. A green that only proves "the script started" proves nothing.
- [ ] Verified on both PowerShell 5.1 and 7 if both are in use

## Related

- Root `CLAUDE.md` § *"Stopping a dev process: match by EXE PATH, never by image name"*
- `phase-7c-quiet-mode/PHASE_CONTRACT.md` §9.2 — where this was first recorded
