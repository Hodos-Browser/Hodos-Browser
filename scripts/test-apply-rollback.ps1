# test-apply-rollback.ps1 - fault-injection rig for the silent auto-update ROLLBACK
# (WINDOWS_AUTOUPDATE_PLAN commit 7 - the MUST-TEST core). Exercises the brick-
# critical recovery: a failed apply (apply.json=awaiting-health, no live supervisor)
# is rolled back by `hodos-update-helper.exe --resume`, and we assert the OLD build
# + the OLD money DB are restored intact - the exact thing that prevents a brick.
#
# FULLY ISOLATED: runs the helper against a TEMP sandbox via --app-dir/--wallet-dir/
# --update-dir. It NEVER touches your real or dev Hodos install or wallet. (It does
# POST /shutdown to 31301/31302 as part of rollback cleanup, so it ABORTS if a real
# Hodos wallet is listening - close Hodos before running.)
#
# Cases: (1) graceful death (snapshot = wallet.db only) -> a stale new -wal/-shm must
# be DELETED (V3-3a); (2) hard-kill death (snapshot = wallet.db + -wal) -> both restored.
#
# Prereq: a built helper (cmake --build cef-native/build --config Release --target
# hodos-update-helper). Usage: pwsh -File scripts/test-apply-rollback.ps1

$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path -Parent $PSScriptRoot
$helper   = Join-Path $repoRoot 'cef-native\build\bin\Release\hodos-update-helper.exe'
if (-not (Test-Path $helper)) {
    Write-Error "helper not built: $helper`n  cmake --build cef-native/build --config Release --target hodos-update-helper"
}

# Safety: rollback POSTs /shutdown to the wallet/adblock ports. Refuse to run while a
# real Hodos wallet is up (it would be shut down).
foreach ($port in @(31301, 31302, 31401, 31402)) {
    $live = $false
    try { $c = New-Object Net.Sockets.TcpClient; $c.Connect('127.0.0.1', $port); $live = $c.Connected; $c.Close() } catch {}
    if ($live) { Write-Error "Port $port is listening - a Hodos wallet/adblock is running. Close Hodos before running this rig." }
}

function Set-File([string]$p, [string]$v) {
    New-Item -ItemType Directory -Force -Path (Split-Path -Parent $p) | Out-Null
    [System.IO.File]::WriteAllText($p, $v)
}
function Read-File([string]$p) { if (Test-Path $p) { [System.IO.File]::ReadAllText($p) } else { $null } }

$global:allPass = $true
function Check([bool]$cond, [string]$msg) {
    if ($cond) { Write-Host "  [ok]   $msg" -ForegroundColor Green }
    else       { Write-Host "  [FAIL] $msg" -ForegroundColor Red; $global:allPass = $false }
}

function Invoke-Case([string]$name, [bool]$snapshotHasWal) {
    Write-Host "`n=== CASE: $name ===" -ForegroundColor Cyan
    $sb       = Join-Path ([IO.Path]::GetTempPath()) ("hodos-rollback-" + [Guid]::NewGuid().ToString('N').Substring(0,8))
    $app      = Join-Path $sb 'app'
    $wallet   = Join-Path $sb 'wallet'
    $update   = Join-Path $sb 'update'
    $pending  = Join-Path $update 'pending'
    $rollback = Join-Path $pending 'rollback'
    $rbWallet = Join-Path $rollback 'wallet'
    New-Item -ItemType Directory -Force -Path $app,$wallet,$pending,$rollback,$rbWallet,(Join-Path $pending 'helper') | Out-Null

    # The "new build" a failed apply left in {app}, plus its migrated money DB + dirty -wal/-shm.
    foreach ($f in 'HodosBrowser.exe','libcef.dll','resources.pak','hodos-wallet.exe','hodos-adblock.exe','hodos-update-helper.exe') {
        Set-File (Join-Path $app $f) 'NEW'
    }
    Set-File (Join-Path $wallet 'wallet.db')     'NEW_MIGRATED_DB'
    Set-File (Join-Path $wallet 'wallet.db-wal') 'NEW_DIRTY_WAL'
    Set-File (Join-Path $wallet 'wallet.db-shm') 'NEW_DIRTY_SHM'

    # The rollback backup = the OLD build + the OLD money-DB snapshot.
    foreach ($f in 'HodosBrowser.exe','libcef.dll','resources.pak','hodos-wallet.exe','hodos-adblock.exe','hodos-update-helper.exe') {
        Set-File (Join-Path $rollback $f) 'OLD'
    }
    Set-File (Join-Path $rbWallet 'wallet.db') 'OLD_DB'
    if ($snapshotHasWal) { Set-File (Join-Path $rbWallet 'wallet.db-wal') 'OLD_WAL' }

    # An unconfirmed apply with no live supervisor -> the --resume watchdog rolls back.
    (@{ schema=1; phase='awaiting-health'; fromBuild=40099; toBuild=40101; installerPath='';
        rollbackDir=$rollback; rollbackManifestPath=(Join-Path $rollback 'manifest.json');
        expectedNewManifestPath=''; profileId='Default'; toVersion='0.4.1'; signerThumbprint='';
        stagedAt='2026-06-30T00:00:00Z'; failureReason='' } | ConvertTo-Json) |
        ForEach-Object { Set-File (Join-Path $pending 'apply.json') $_ }
    (@{ schema=1; silent=$true; paused=$false; highWaterBuild=40099; signerThumbprint='';
        lastFailureBuild=0; lastFailureReason=''; rescanAfterRollback=$false } | ConvertTo-Json) |
        ForEach-Object { Set-File (Join-Path $update 'update-state.json') $_ }

    Write-Host "  running: hodos-update-helper --resume (sandbox=$sb)"
    & $helper --resume --app-dir $app --wallet-dir $wallet --update-dir $update | Out-Null

    # Assertions: the OLD build + OLD money DB are restored; stale new -wal/-shm gone.
    Check ((Read-File (Join-Path $app 'HodosBrowser.exe')) -eq 'OLD') 'HodosBrowser.exe restored to OLD'
    Check ((Read-File (Join-Path $app 'libcef.dll'))       -eq 'OLD') 'libcef.dll restored to OLD'
    Check ((Read-File (Join-Path $app 'resources.pak'))    -eq 'OLD') 'resources.pak restored to OLD'
    Check ((Read-File (Join-Path $wallet 'wallet.db'))     -eq 'OLD_DB') 'wallet.db restored to OLD snapshot'
    if ($snapshotHasWal) {
        Check ((Read-File (Join-Path $wallet 'wallet.db-wal')) -eq 'OLD_WAL') 'wallet.db-wal restored to OLD snapshot'
    } else {
        Check (-not (Test-Path (Join-Path $wallet 'wallet.db-wal'))) 'stale new wallet.db-wal DELETED (V3-3a)'
    }
    Check (-not (Test-Path (Join-Path $wallet 'wallet.db-shm'))) 'stale new wallet.db-shm DELETED'

    $st = (Read-File (Join-Path $update 'update-state.json')) | ConvertFrom-Json
    Check ($st.paused -eq $true)               'update-state paused=true (no further silent applies)'
    Check ($st.rescanAfterRollback -eq $true)  'rescanAfterRollback=true (old wallet re-scans on-chain)'
    Check ($st.highWaterBuild -eq 40099)       'highWaterBuild NOT advanced (failed build not the floor, I6)'

    $ap = (Read-File (Join-Path $pending 'apply.json')) | ConvertFrom-Json
    Check ($ap.phase -eq 'rolledback') 'apply.json phase=rolledback'

    Remove-Item -Recurse -Force $sb -ErrorAction SilentlyContinue
}

# Clean any RunOnce the helper armed during the run (it clears it on a completed
# rollback, but be defensive - this is a test artifact, not a real recovery hook).
function Clear-RunOnce {
    Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\RunOnce' `
        -Name 'HodosUpdateResume' -ErrorAction SilentlyContinue
}

try {
    Invoke-Case 'graceful-death (snapshot has no -wal; stale new -wal/-shm must be deleted)' $false
    Invoke-Case 'hard-kill (snapshot has -wal; both restored)'                               $true
}
finally { Clear-RunOnce }

if ($global:allPass) { Write-Host "`n=== ROLLBACK RIG: ALL ASSERTIONS PASS ===" -ForegroundColor Green; exit 0 }
else                 { Write-Host "`n=== ROLLBACK RIG: FAILURES ABOVE ==="      -ForegroundColor Red;   exit 1 }
