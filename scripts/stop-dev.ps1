<#
.SYNOPSIS
  Stop the DEV browser / wallet / adblock processes — matched by EXECUTABLE PATH, never by name.

.DESCRIPTION
  All three dev processes share their image name with the user's INSTALLED build:
  HodosBrowser.exe, hodos-wallet.exe, hodos-adblock.exe. So this:

      Get-CimInstance Win32_Process -Filter "Name='hodos-wallet.exe'" | Stop-Process -Force

  also kills the installed browser's wallet backend. 🚨 That happened on 2026-09-01 and left the
  owner's production browser reporting "no wallet" to every dApp for ~4 hours — see CLAUDE.md's
  Dev Runbook, and TICKET_wallet_backend_death_is_silent_and_unrecovered.md for the product half.

  This script exists so nobody has to remember the Where-Object clause under build pressure
  (LNK1104 / "Access is denied" both demand the dev processes be stopped first).

  ⛔ It will refuse to stop anything whose path is not under the repo. Read-only until it kills,
  and -WhatIf makes it read-only entirely.

.EXAMPLE
  .\scripts\stop-dev.ps1
  .\scripts\stop-dev.ps1 -WhatIf
#>
[CmdletBinding(SupportsShouldProcess)]
param(
    # Repo root. Everything stopped must live underneath it.
    # ⛔ Resolved in the BODY, not as a parameter default — see below.
    [string]$RepoRoot
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

# 🚨 This used to be a parameter default: `[string]$RepoRoot = (Split-Path -Parent $PSScriptRoot)`.
# `$PSScriptRoot` is empty at PARAMETER-BINDING time under `powershell -File`, so `Split-Path` threw
# before the body ever ran and the documented invocation — `.\scripts\stop-dev.ps1`, with no
# arguments, exactly as CLAUDE.md instructs — died with:
#
#     Split-Path : Cannot bind argument to parameter 'Path' because it is an empty string.
#
# ⛔ Why that mattered more than an ordinary script bug: this script IS the safeguard against the
# 2026-09-01 incident, where a name-matched kill took out the owner's PRODUCTION wallet and left the
# installed browser saying "no wallet" for ~4 hours. The moment someone reaches for it is the moment
# a linker is holding a file and a build is failing — maximum pressure to give up and hand-write
# `Stop-Process -Name hodos-wallet`, which is the exact thing this exists to prevent.
# ⭐ The lesson from that incident was "embody a rule in a tool, don't rely on remembering it."
# A tool that will not start does not embody anything.
#
# Fixed 2026-09-08. The matching logic below was NOT touched — it was measured correct on the same
# day (stopped 20 dev processes, spared all 77 installed-build ones). Only the startup was broken.
if (-not $RepoRoot) {
    $here = if ($PSScriptRoot) { $PSScriptRoot }
            else { Split-Path -Parent $MyInvocation.MyCommand.Path }
    $RepoRoot = Split-Path -Parent $here
}

# name -> the path fragment that identifies the DEV build of it.
# ⚠️ Keep these anchored to build-output directories. A fragment as loose as 'Hodos-Browser'
# would match a production install that happened to sit in a similarly named folder.
$targets = @(
    @{ Name = 'HodosBrowser.exe'; Fragment = 'cef-native\build\bin\Release'      ; Label = 'dev browser' },
    @{ Name = 'hodos-wallet.exe' ; Fragment = 'rust-wallet\target\release'       ; Label = 'dev wallet'  },
    @{ Name = 'hodos-adblock.exe'; Fragment = 'adblock-engine\target\release'    ; Label = 'dev adblock' }
)

$repoFull = (Resolve-Path $RepoRoot).Path
Write-Host "Repo root: $repoFull" -ForegroundColor Cyan
Write-Host ""

$totalStopped = 0
$totalSpared  = 0

foreach ($t in $targets) {
    $all = @(Get-CimInstance Win32_Process -Filter "Name='$($t.Name)'" -ErrorAction SilentlyContinue)
    if ($all.Count -eq 0) { Write-Host ("{0,-13} : not running" -f $t.Label); continue }

    $dev = @($all | Where-Object {
        $_.ExecutablePath -and
        $_.ExecutablePath -like "*$($t.Fragment)*" -and
        # Belt and braces: it must ALSO be inside this repo. Guards against a second checkout.
        $_.ExecutablePath.StartsWith($repoFull, [StringComparison]::OrdinalIgnoreCase)
    })
    # ⛔ Project the ids into a real array FIRST. Under Set-StrictMode, `$dev.ProcessId` on an
    # EMPTY array throws "The property 'ProcessId' cannot be found on this object" — which happens
    # whenever a dev process is already gone but the installed one is still running (the adblock
    # engine dies with the dev browser via the job object, so this is the normal case on the second
    # run, not an edge case). 📏 Hit on 2026-09-01: it aborted the script mid-way, after the browser
    # and wallet had been stopped. It failed SAFE — it throws before killing anything in that arm —
    # but had adblock been first in $targets, nothing would have been stopped at all.
    $devIds = @($dev | ForEach-Object { $_.ProcessId })
    $other  = @($all | Where-Object { $devIds -notcontains $_.ProcessId })

    Write-Host ("{0,-13} : {1} running, {2} dev, {3} spared" -f $t.Label, $all.Count, $dev.Count, $other.Count)

    # Printed, not silent: "I left something alone" is the fact worth seeing. But a browser has
    # ~70 child processes, so show the distinct PATHS rather than every pid — the path is what
    # actually tells you whose build it is.
    $sparedPaths = @($other | ForEach-Object { $_.ExecutablePath } | Sort-Object -Unique)
    foreach ($path in $sparedPaths) {
        $n = @($other | Where-Object { $_.ExecutablePath -eq $path }).Count
        Write-Host ("                spared  {0,3} x  {1}" -f $n, $path) -ForegroundColor DarkGray
    }
    $totalSpared += $other.Count

    foreach ($p in $dev) {
        if ($PSCmdlet.ShouldProcess("pid $($p.ProcessId)  $($p.ExecutablePath)", "Stop-Process")) {
            try {
                Stop-Process -Id $p.ProcessId -Force -ErrorAction Stop
                Write-Host ("                STOPPED pid {0}" -f $p.ProcessId) -ForegroundColor Yellow
                $totalStopped++
            } catch {
                # A child that already died with its parent is not an error worth failing on.
                Write-Host ("                pid {0} already gone" -f $p.ProcessId) -ForegroundColor DarkGray
            }
        }
    }
}

Write-Host ""
Write-Host "Stopped $totalStopped dev process(es); left $totalSpared non-dev process(es) alone." -ForegroundColor Cyan
if ($totalSpared -gt 0) {
    Write-Host "^ those are the installed build's. Never stop them from here." -ForegroundColor Green
}
exit 0
