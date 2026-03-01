<#
.SYNOPSIS
    Unified test runner for Hodos Browser (all stacks)

.DESCRIPTION
    Runs tests across Rust wallet, adblock engine, and frontend.
    Use -NightlyReport for overnight runs with saved logs.

.PARAMETER Coverage
    Generate coverage reports (requires cargo-tarpaulin and vitest coverage)

.PARAMETER Verbose
    Show full test output (cargo test --nocapture)

.PARAMETER NightlyReport
    Save detailed logs to test-reports/YYYY-MM-DD/ for morning review

.PARAMETER Filter
    Filter tests by name (Rust only, passed to cargo test)

.PARAMETER SkipFrontend
    Skip frontend tests (useful during Rust-focused work)

.EXAMPLE
    ./scripts/test-all.ps1
    ./scripts/test-all.ps1 -Verbose
    ./scripts/test-all.ps1 -NightlyReport
    ./scripts/test-all.ps1 -Filter "brc42"
#>

param(
    [switch]$Coverage,
    [switch]$Verbose,
    [switch]$NightlyReport,
    [string]$Filter = "",
    [switch]$SkipFrontend
)

$ErrorActionPreference = "Continue"  # Don't stop on first failure
$results = @()
$startTime = Get-Date

Write-Host "═══════════════════════════════════════════════════════════" -ForegroundColor Cyan
Write-Host "  HODOS BROWSER — FULL TEST SUITE" -ForegroundColor Cyan
Write-Host "  Started: $startTime" -ForegroundColor Cyan
Write-Host "═══════════════════════════════════════════════════════════" -ForegroundColor Cyan

# ─── Rust Wallet Tests ───
Write-Host "`n▶ RUST WALLET TESTS" -ForegroundColor Yellow
Push-Location rust-wallet

$cargoArgs = @("test")
if ($Verbose) { $cargoArgs += @("--", "--nocapture") }
if ($Filter -and -not $Verbose) { $cargoArgs += @("--", $Filter) }
if ($Filter -and $Verbose) { $cargoArgs += @($Filter) }

$rustStart = Get-Date
try {
    if ($Coverage) {
        $rustOutput = cargo tarpaulin --out Html --output-dir ../test-reports/rust-wallet 2>&1 | Tee-Object -Variable rustLog
    } else {
        $rustOutput = & cargo $cargoArgs 2>&1 | Tee-Object -Variable rustLog
    }
    $rustExit = $LASTEXITCODE
} catch {
    $rustExit = 1
    $rustLog = $_.Exception.Message
}
$rustDuration = (Get-Date) - $rustStart

$results += @{
    Stack = "rust-wallet"
    Exit = $rustExit
    Duration = $rustDuration
    Log = ($rustLog -join "`n")
}
Pop-Location

# ─── Adblock Engine Tests ───
Write-Host "`n▶ ADBLOCK ENGINE TESTS" -ForegroundColor Yellow
Push-Location adblock-engine

$adblockStart = Get-Date
try {
    $adblockOutput = cargo test 2>&1 | Tee-Object -Variable adblockLog
    $adblockExit = $LASTEXITCODE
} catch {
    $adblockExit = 1
    $adblockLog = $_.Exception.Message
}
$adblockDuration = (Get-Date) - $adblockStart

$results += @{
    Stack = "adblock-engine"
    Exit = $adblockExit
    Duration = $adblockDuration
    Log = ($adblockLog -join "`n")
}
Pop-Location

# ─── Frontend Tests ───
if (-not $SkipFrontend) {
    Write-Host "`n▶ FRONTEND TESTS" -ForegroundColor Yellow
    Push-Location frontend

    $frontendStart = Get-Date
    try {
        if ($Coverage) {
            $frontendOutput = npm test -- --run --coverage 2>&1 | Tee-Object -Variable frontendLog
        } else {
            $frontendOutput = npm test -- --run 2>&1 | Tee-Object -Variable frontendLog
        }
        $frontendExit = $LASTEXITCODE
    } catch {
        $frontendExit = 1
        $frontendLog = $_.Exception.Message
    }
    $frontendDuration = (Get-Date) - $frontendStart

    $results += @{
        Stack = "frontend"
        Exit = $frontendExit
        Duration = $frontendDuration
        Log = ($frontendLog -join "`n")
    }
    Pop-Location
} else {
    Write-Host "`n▶ FRONTEND TESTS (skipped)" -ForegroundColor DarkGray
}

# ─── Summary ───
$endTime = Get-Date
$totalDuration = $endTime - $startTime

Write-Host "`n═══════════════════════════════════════════════════════════" -ForegroundColor Cyan
Write-Host "  TEST SUMMARY" -ForegroundColor Cyan
Write-Host "═══════════════════════════════════════════════════════════" -ForegroundColor Cyan

foreach ($r in $results) {
    $status = if ($r.Exit -eq 0) { "✓ PASS" } else { "✗ FAIL" }
    $color = if ($r.Exit -eq 0) { "Green" } else { "Red" }
    Write-Host "  $($r.Stack.PadRight(20)) $status  ($([math]::Round($r.Duration.TotalSeconds, 1))s)" -ForegroundColor $color
}

Write-Host "`n  Total time: $([math]::Round($totalDuration.TotalMinutes, 2)) minutes"
Write-Host "  Finished: $endTime"

# ─── Nightly Report ───
if ($NightlyReport) {
    $reportDir = "test-reports/$(Get-Date -Format 'yyyy-MM-dd')"
    New-Item -ItemType Directory -Force -Path $reportDir | Out-Null
    
    $summaryData = @{
        timestamp = $endTime.ToString("o")
        duration_seconds = $totalDuration.TotalSeconds
        results = @()
    }
    
    foreach ($r in $results) {
        $summaryData.results += @{
            stack = $r.Stack
            passed = ($r.Exit -eq 0)
            exit_code = $r.Exit
            duration_seconds = $r.Duration.TotalSeconds
        }
        
        # Save individual log
        $r.Log | Out-File "$reportDir/$($r.Stack).log" -Encoding utf8
    }
    
    $summaryData | ConvertTo-Json -Depth 3 | Out-File "$reportDir/summary.json" -Encoding utf8
    
    Write-Host "`n  📁 Report saved to: $reportDir" -ForegroundColor Cyan
    Write-Host "     - summary.json (pass/fail + timing)"
    Write-Host "     - rust-wallet.log"
    Write-Host "     - adblock-engine.log"
    if (-not $SkipFrontend) {
        Write-Host "     - frontend.log"
    }
}

# ─── Exit Code ───
$failCount = ($results | Where-Object { $_.Exit -ne 0 }).Count
if ($failCount -gt 0) {
    Write-Host "`n  ⚠️  $failCount stack(s) failed" -ForegroundColor Red
} else {
    Write-Host "`n  ✅ All tests passed!" -ForegroundColor Green
}

exit $failCount
