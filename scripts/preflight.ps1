<#
.SYNOPSIS
  Local T0 (static gates) + T1 (unit tests) runner for the beta.3 sprint harness.

.DESCRIPTION
  The dev fork's Actions minutes are exhausted until ~2026-09-01, so test.yml and ci.yml
  execute nothing. Everything they would have gated runs here instead. See
  development-docs/0.4.0-beta.3/HARNESS.md.

  Static gates RATCHET: each carries a Baseline and fails when violations EXCEED it. That
  lets a gate land before its fix, while a NEW violation still fails at any baseline --
  which is what preserves the negative control.

.PARAMETER NegativeControl
  Prove each static gate can fail. Injects a synthetic violation into a scanned tree, asserts
  the gate reports MORE than its baseline, then removes it. A gate that stays green under
  this is blind, and is reported as FAIL. Runs no unit tests.

.PARAMETER Full
  Also run the frontend type/build gate (tsc -b && vite build). Slow; off by default.

.PARAMETER Only
  Run a subset by id, e.g. -Only G1,G2

.OUTPUTS
  Exit 0 = PASS.  1 = FAIL.  2 = INCOMPLETE (something was skipped; never report as pass).

.EXAMPLE
  pwsh scripts/preflight.ps1
  pwsh scripts/preflight.ps1 -NegativeControl
#>
[CmdletBinding()]
param(
    [switch]$NegativeControl,
    [switch]$Full,
    [string[]]$Only
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$RepoRoot = Split-Path -Parent $PSScriptRoot

# Invoked via -File, "-Only G1,G2" arrives as ONE string, not an array, so every
# -contains test silently misses and every check is skipped. Split defensively.
if ($Only) { $Only = @($Only) -split ',' | ForEach-Object { $_.Trim() } | Where-Object { $_ } }

# --------------------------------------------------------------------------------------
# Static gates (T0)
#
# Baseline: the count of KNOWN violations at the time the gate landed. Lowering it is a
# deliberate commit; raising it requires a written reason in HARNESS.md section 4.
# Probe:    a synthetic violating line, used ONLY by -NegativeControl.
# --------------------------------------------------------------------------------------
$Gates = @(
    [pscustomobject]@{
        Id       = 'G1'
        Name     = 'Bare-filename file sinks (relative path resolves against CWD, i.e. {app})'
        Owner    = 'Phase 0'
        # Lowered 52 -> 0 by Phase 0 (P0-A3). All 52 removed: debug_output.log x44
        # (WalletService 19, simple_app 17, my_overlay_render_handler .cpp/.mm 3+3,
        # AddressHandler 2) + startup_log.txt x8 (simple_app). No residuals.
        Baseline = 0
        Target   = 0
        Paths    = @('cef-native/src', 'cef-native/include')
        Include  = @('*.cpp', '*.mm', '*.h')
        Pattern  = '(ofstream|ifstream|fstream|fopen|freopen)[^;]*"[A-Za-z0-9_.\-]+\.(log|txt|json|db|dat|bin)"'
        Probe    = 'std::ofstream probe("preflight_probe.log", std::ios::app);'
        ProbeExt = '.cpp'
    },
    [pscustomobject]@{
        Id       = 'G2'
        Name     = 'Substring origin checks on the internal frontend port (trust boundary)'
        Owner    = 'Phase 0.5'
        # Measured 2026-08-18 by THIS script. The 5: TabManager.cpp:168,
        # TabManager_mac.mm:191, simple_handler.cpp:7962,
        # simple_render_process_handler.cpp:539 and :541.
        # Target 2 = the two TabManager history-exclusion uses -- sloppy, but not a trust
        # boundary; they go with W7 in beta.4. Phase 0.5 fixes the other three.
        # Lowered 5 -> 2 by Phase 0.5 (P0.5-G3). The three trust-boundary gates now use
        # hodos::IsInternalFrontendUrl (a prefix match). The 2 residuals are named in
        # phase-0.5-money-path/PHASE_CONTRACT.md section 6: TabManager.cpp:176 and
        # TabManager_mac.mm:191, both history-exclusion, not a trust boundary. They go
        # with W7 in beta.4.
        Baseline = 2
        Target   = 2
        Paths    = @('cef-native/src', 'cef-native/include')
        Include  = @('*.cpp', '*.mm', '*.h')
        Pattern  = '(find|rfind)\("(http://)?(127\.0\.0\.1|localhost):5137'
        # A prefix check -- rfind(X, 0) == 0, or find(X) != 0 -- is the CORRECT form and
        # must not be flagged, or the gate teaches the wrong lesson. Only unanchored
        # substring searches are violations.
        Exclude  = '(rfind\([^)]*, *0\)|find\([^)]*\) *!= *0)'
        Probe    = 'bool probe = url.find("127.0.0.1:5137") != std::string::npos;'
        ProbeExt = '.cpp'
    },
    [pscustomobject]@{
        Id       = 'G3'
        Name     = 'F8 secret-log gate (Rust) - secret material into a log/print sink'
        Owner    = 'test.yml (ported)'
        Baseline = 0
        Target   = 0
        Paths    = @('rust-wallet/src', 'adblock-engine/src')
        Include  = @('*.rs')
        Pattern  = '(log::(info|debug|trace|warn)!|println!|eprintln!|print!)\(.*(hex::encode|base64|\.encode)\(&?[A-Za-z0-9_]*(priv|secret|symmetric|mnemonic|seed|wif|xpriv|revelation|hmac_output|hmac_secret)'
        Probe    = 'log::info!("{}", hex::encode(&mnemonic_secret));'
        ProbeExt = '.rs'
    },
    [pscustomobject]@{
        Id       = 'G4'
        Name     = 'F8 secret-log gate (C++) - secret material into a log/print sink'
        Owner    = 'test.yml (ported)'
        Baseline = 0
        Target   = 0
        Paths    = @('cef-native/src', 'cef-native/include')
        Include  = @('*.cpp', '*.mm', '*.h')
        Pattern  = '(std::cout|std::cerr|printf|fprintf|OutputDebugString)[^;]*(\["(mnemonic|privateKey|private_key|seed)"\]|<<[^;]*\b(mnemonic|privateKey)\b)'
        Probe    = 'std::cerr << "probe" << mnemonic << std::endl;'
        ProbeExt = '.cpp'
    },
    [pscustomobject]@{
        Id       = 'G5'
        Name     = 'Full wallet HTTP response bodies reaching a sink (the mnemonic-leak shape)'
        Owner    = 'Phase 0'
        # Measured 2026-08-18 by THIS script. A hand-rolled grep gave 11 -- a narrower
        # pattern. Always baseline with the tool that will do the measuring.
        # Lowered 15 -> 0 by Phase 0 (P0-A8). Response BODIES are gone entirely; the
        # remaining prose hits ("...HTTP response...") were reworded, and txid/error are
        # now extracted into named locals BEFORE the log call rather than reached through
        # `response` inside it. No residuals.
        Baseline = 0
        Target   = 0
        Paths    = @('cef-native/src/core')
        Include  = @('WalletService.cpp', 'WalletService_mac.cpp', '__preflight_probe.cpp')
        Pattern  = '(debugLog *<<|LOG_[A-Z_]+ *\()[^;]*\b(responseBody|response)\b'
        Probe    = 'debugLog << "   Response: " << responseBody << std::endl;'
        ProbeExt = '.cpp'
    }
)

# --------------------------------------------------------------------------------------
# Helpers
# --------------------------------------------------------------------------------------
function Get-GateHits {
    param([Parameter(Mandatory)]$Gate)
    $files = @()
    foreach ($p in $Gate.Paths) {
        $full = Join-Path $RepoRoot $p
        if (-not (Test-Path $full)) { continue }
        $files += Get-ChildItem -Path $full -Recurse -File -Include $Gate.Include -ErrorAction SilentlyContinue
    }
    if ($files.Count -eq 0) { return @() }
    $hits = @($files | Select-String -Pattern $Gate.Pattern -CaseSensitive)
    # Drop whole-line comments: a commented example is documentation, not a violation.
    $hits = @($hits | Where-Object { $_.Line -notmatch '^\s*(//|#)' })
    # Per-gate exclusion for forms that are correct by construction.
    if ($Gate.PSObject.Properties.Name -contains 'Exclude' -and $Gate.Exclude) {
        $hits = @($hits | Where-Object { $_.Line -notmatch $Gate.Exclude })
    }
    return ,$hits
}

$script:Results = New-Object System.Collections.Generic.List[object]

function Add-Result {
    param([string]$Id, [string]$Name, [string]$Status, [string]$Detail)
    $script:Results.Add([pscustomobject]@{ Id = $Id; Name = $Name; Status = $Status; Detail = $Detail })
    $colour = switch ($Status) {
        'PASS'    { 'Green' }
        'FAIL'    { 'Red' }
        'SKIPPED' { 'Yellow' }
        default   { 'Gray' }
    }
    Write-Host ("  [{0,-7}] {1,-4} {2}" -f $Status, $Id, $Name) -ForegroundColor $colour
    if ($Detail) { Write-Host ("            {0}" -f $Detail) -ForegroundColor DarkGray }
}

function Test-Selected {
    param([string]$Id)
    if (-not $Only) { return $true }
    return $Only -contains $Id
}

# --------------------------------------------------------------------------------------
# -NegativeControl : prove every gate can fail
# --------------------------------------------------------------------------------------
if ($NegativeControl) {
    Write-Host ''
    Write-Host 'PREFLIGHT - NEGATIVE CONTROL' -ForegroundColor Cyan
    Write-Host 'Proving each static gate reports MORE than its baseline when a violation is injected.' -ForegroundColor DarkGray
    Write-Host 'A gate that stays green here is blind, and so is anything it was supposed to protect.' -ForegroundColor DarkGray
    Write-Host ''

    foreach ($g in $Gates) {
        if (-not (Test-Selected $g.Id)) { continue }
        $scanDir = Join-Path $RepoRoot $g.Paths[0]
        if (-not (Test-Path $scanDir)) {
            Add-Result $g.Id $g.Name 'SKIPPED' "scan path missing: $($g.Paths[0])"
            continue
        }
        $probeFile = Join-Path $scanDir ('__preflight_probe' + $g.ProbeExt)
        if (Test-Path $probeFile) {
            Add-Result $g.Id $g.Name 'SKIPPED' "probe file already exists, refusing to overwrite: $probeFile"
            continue
        }
        try {
            Set-Content -Path $probeFile -Value $g.Probe -Encoding UTF8
            $n = (Get-GateHits -Gate $g).Count   # function returns a real array via ,$hits
            if ($n -gt $g.Baseline) {
                Add-Result $g.Id $g.Name 'PASS' "detected the injected violation ($n > baseline $($g.Baseline))"
            } else {
                Add-Result $g.Id $g.Name 'FAIL' "GATE IS BLIND: injected a violation and counted $n (baseline $($g.Baseline)). The pattern does not match its own probe."
            }
        } finally {
            if (Test-Path $probeFile) { Remove-Item $probeFile -Force }
        }
    }

    Write-Host ''
    if ($script:Results.Count -eq 0) {
        Write-Host 'NEGATIVE CONTROL: INCOMPLETE - ZERO gates exercised. This is NOT a pass.' -ForegroundColor Yellow
        exit 2
    }
    $ncFailed  = @($script:Results | Where-Object { $_.Status -eq 'FAIL' }).Count
    $ncSkipped = @($script:Results | Where-Object { $_.Status -eq 'SKIPPED' }).Count
    if ($ncFailed -gt 0) {
        Write-Host "NEGATIVE CONTROL: FAIL - $ncFailed gate(s) could not detect their own probe." -ForegroundColor Red
        exit 1
    }
    if ($ncSkipped -gt 0) {
        Write-Host "NEGATIVE CONTROL: INCOMPLETE - $ncSkipped skipped." -ForegroundColor Yellow
        exit 2
    }
    Write-Host 'NEGATIVE CONTROL: PASS - every gate was seen to fail.' -ForegroundColor Green
    exit 0
}

# --------------------------------------------------------------------------------------
# Normal run: T0 then T1
# --------------------------------------------------------------------------------------
Write-Host ''
Write-Host 'PREFLIGHT - T0 static gates' -ForegroundColor Cyan
Write-Host 'CI note: test.yml/ci.yml execute nothing until the Actions quota resets (~2026-09-01).' -ForegroundColor DarkGray
Write-Host ''

foreach ($g in $Gates) {
    if (-not (Test-Selected $g.Id)) { continue }
    $hits = Get-GateHits -Gate $g        # already an array (,$hits) -- do NOT re-wrap in @(), that nests
    $n = $hits.Count
    if ($n -gt $g.Baseline) {
        $sample = $hits | Select-Object -First 5 | ForEach-Object {
            "$($_.Path -replace [regex]::Escape($RepoRoot + [System.IO.Path]::DirectorySeparatorChar), ''):$($_.LineNumber)"
        }
        Add-Result $g.Id $g.Name 'FAIL' "$n violations, over baseline $($g.Baseline) [$($g.Owner), target $($g.Target)]. e.g. $($sample -join '; ')"
    } elseif ($n -lt $g.Baseline) {
        Add-Result $g.Id $g.Name 'PASS' "$n violations, BELOW baseline $($g.Baseline) - lower the baseline in HARNESS.md section 4 [$($g.Owner), target $($g.Target)]"
    } else {
        Add-Result $g.Id $g.Name 'PASS' "$n violations, at baseline [$($g.Owner), target $($g.Target)]"
    }
}

Write-Host ''
Write-Host 'PREFLIGHT - T1 unit tests' -ForegroundColor Cyan
Write-Host ''

function Invoke-CargoTest {
    param([string]$Id, [string]$Manifest, [string]$Label)
    if (-not (Test-Selected $Id)) { return }
    $path = Join-Path $RepoRoot $Manifest
    if (-not (Test-Path $path)) { Add-Result $Id $Label 'SKIPPED' "no manifest at $Manifest"; return }
    if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) { Add-Result $Id $Label 'SKIPPED' 'cargo not on PATH'; return }
    # cargo writes warnings to stderr. With EAP=Stop that becomes a terminating
    # NativeCommandError and the run dies on a *warning* -- so judge by exit code only.
    $prevEap = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    try { & cargo test --manifest-path $path --quiet 2>&1 | Out-String | Write-Verbose }
    finally { $ErrorActionPreference = $prevEap }
    if ($LASTEXITCODE -eq 0) { Add-Result $Id $Label 'PASS' '' }
    else { Add-Result $Id $Label 'FAIL' "cargo test exited $LASTEXITCODE - re-run with -Verbose for output" }
}

Invoke-CargoTest -Id 'T1a' -Manifest 'rust-wallet/Cargo.toml'    -Label 'cargo test - rust-wallet'
Invoke-CargoTest -Id 'T1b' -Manifest 'adblock-engine/Cargo.toml' -Label 'cargo test - adblock-engine'

if (Test-Selected 'T1c') {
    # CMake puts the target wherever RUNTIME_OUTPUT_DIRECTORY says -- measured 2026-08-18 it
    # lands in bin/Release, NOT tests/Release as cef-native/tests/CMakeLists.txt's header
    # comment claims. Probe the real candidates rather than trusting one hard-coded path:
    # a wrong path here reads as "not built" and SKIPS, which is a silent loss of coverage.
    $exe = @(
        'cef-native/build/bin/Release/hodos_tests.exe',
        'cef-native/build/tests/Release/hodos_tests.exe',
        'cef-native/build/bin/hodos_tests.exe',
        'cef-native/build/tests/hodos_tests'
    ) | ForEach-Object { Join-Path $RepoRoot $_ } | Where-Object { Test-Path $_ } | Select-Object -First 1
    if ($exe) {
        $prevEap = $ErrorActionPreference; $ErrorActionPreference = 'Continue'
        try { & $exe 2>&1 | Out-String | Write-Verbose } finally { $ErrorActionPreference = $prevEap }
        if ($LASTEXITCODE -eq 0) { Add-Result 'T1c' 'hodos_tests (C++)' 'PASS' '' }
        else { Add-Result 'T1c' 'hodos_tests (C++)' 'FAIL' "exited $LASTEXITCODE" }
    } else {
        Add-Result 'T1c' 'hodos_tests (C++)' 'SKIPPED' 'not built: cmake -S cef-native -B cef-native/build -DHODOS_BUILD_TESTS=ON; cmake --build cef-native/build --config Release --target hodos_tests'
    }
}

if (Test-Selected 'T1d') {
    if (-not $Full) {
        Add-Result 'T1d' 'frontend build (tsc -b, vite build)' 'SKIPPED' 'not requested - pass -Full'
    } elseif (-not (Get-Command npm -ErrorAction SilentlyContinue)) {
        Add-Result 'T1d' 'frontend build (tsc -b, vite build)' 'SKIPPED' 'npm not on PATH'
    } else {
        Push-Location (Join-Path $RepoRoot 'frontend')
        try {
            $prevEap = $ErrorActionPreference; $ErrorActionPreference = 'Continue'
            try { & npm run build 2>&1 | Out-String | Write-Verbose } finally { $ErrorActionPreference = $prevEap }
            if ($LASTEXITCODE -eq 0) { Add-Result 'T1d' 'frontend build (tsc -b, vite build)' 'PASS' '' }
            else { Add-Result 'T1d' 'frontend build (tsc -b, vite build)' 'FAIL' "npm run build exited $LASTEXITCODE" }
        } finally { Pop-Location }
    }
}

# --------------------------------------------------------------------------------------
# Verdict. A skip is never a pass.
# --------------------------------------------------------------------------------------
$failed  = @($script:Results | Where-Object { $_.Status -eq 'FAIL' })
$skipped = @($script:Results | Where-Object { $_.Status -eq 'SKIPPED' })

Write-Host ''
Write-Host ('-' * 78)
# A run that executed nothing is the single most dangerous "green" this project can
# produce -- it is the shape of all four farbling harnesses. Never call it a pass.
if ($script:Results.Count -eq 0) {
    Write-Host 'PREFLIGHT: INCOMPLETE - ZERO checks executed. This is NOT a pass.' -ForegroundColor Yellow
    Write-Host 'Most likely an -Only filter that matched nothing. Run with no -Only to see the full set.' -ForegroundColor Yellow
    exit 2
}
if ($failed.Count -gt 0) {
    Write-Host "PREFLIGHT: FAIL - $($failed.Count) check(s) failed, $($skipped.Count) skipped." -ForegroundColor Red
    exit 1
}
if ($skipped.Count -gt 0) {
    Write-Host "PREFLIGHT: INCOMPLETE - 0 failed, $($skipped.Count) skipped. This is NOT a pass." -ForegroundColor Yellow
    Write-Host 'Record it as INCOMPLETE in the phase contract sign-off table.' -ForegroundColor Yellow
    exit 2
}
Write-Host 'PREFLIGHT: PASS - all checks ran and passed.' -ForegroundColor Green
Write-Host 'Reminder: a green preflight is T0+T1 only. It says nothing about T2/T3.' -ForegroundColor DarkGray
exit 0
