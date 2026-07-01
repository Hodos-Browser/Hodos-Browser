# test-apply-forward.ps1 - forward-apply fault-injection rig for the silent updater
# (WINDOWS_AUTOUPDATE_PLAN commit 7). Runs the REAL hodos-update-helper.exe through
# its FORWARD transaction (Phase B): spawn installer -> integrity-gate the new tree
# -> launch the health-probe -> commit OR rollback. Three cases:
#   (1) HAPPY     - installer copies a matching tree, the fake browser writes
#                   apply.json=healthy -> COMMIT (highWater advanced to toBuild).
#   (2) OS-BLOCK  - the installed HodosBrowser.exe is a valid-but-non-runnable file
#                   (simulates SmartScreen/Smart App Control block) -> probe launch
#                   fails -> ROLLBACK to the old build.
#   (3) INTEGRITY - the installed tree does NOT match the signed manifest (a
#                   truncated/tampered file) -> IntegrityGate FAILS -> ROLLBACK.
#
# FULLY ISOLATED temp sandbox (helper --app-dir/--wallet-dir/--update-dir). Never
# touches the real/dev install or wallet. ABORTS if a real wallet is on 31301/31302
# (rollback POSTs /shutdown).
#
# REQUIRES a RIG BUILD of the helper (test seams compiled in):
#   cmake -B cef-native/build -DHODOS_SILENT_AUTOUPDATE=ON -DHODOS_UPDATE_TEST_SEAM=ON
#   cmake --build cef-native/build --config Release --target hodos-update-helper
# Prereqs: openssl + python on PATH.  Usage: pwsh -File scripts/test-apply-forward.ps1

$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path -Parent $PSScriptRoot
$helper   = Join-Path $repoRoot 'cef-native\build\bin\Release\hodos-update-helper.exe'
if (-not (Test-Path $helper)) { Write-Error "helper not built (RIG build): $helper" }
foreach ($tool in @('openssl','python')) {
    if (-not (Get-Command $tool -ErrorAction SilentlyContinue)) { Write-Error "'$tool' not on PATH" }
}
foreach ($port in @(31301,31302,31401,31402)) {
    $live = $false
    try { $c = New-Object Net.Sockets.TcpClient; $c.Connect('127.0.0.1',$port); $live = $c.Connected; $c.Close() } catch {}
    if ($live) { Write-Error "Port $port is listening - close Hodos before running this rig." }
}

$work = Join-Path ([IO.Path]::GetTempPath()) ("hodos-fwd-" + [Guid]::NewGuid().ToString('N').Substring(0,8))
New-Item -ItemType Directory -Force -Path $work | Out-Null

# ---- compile the two tiny fake exes (ignore /VERYSILENT args; no CEF/Inno) --------
$fakeInstaller = Join-Path $work 'fake-installer.exe'
Add-Type -OutputAssembly $fakeInstaller -OutputType ConsoleApplication -TypeDefinition @'
using System; using System.IO;
class P { static int Main(string[] a) {
    string src = Environment.GetEnvironmentVariable("RIG_STAGING");
    string dst = Environment.GetEnvironmentVariable("RIG_APP_DIR");
    if (src == null || dst == null) return 2;
    foreach (string f in Directory.GetFiles(src, "*", SearchOption.AllDirectories)) {
        string rel = f.Substring(src.Length).TrimStart('\\','/');
        string t = Path.Combine(dst, rel);
        Directory.CreateDirectory(Path.GetDirectoryName(t));
        File.Copy(f, t, true);
    }
    return 0;
}}
'@

# The "new build" HodosBrowser.exe: on --post-update-health-probe it flips
# apply.json awaiting-health -> healthy (simulating a healthy new build).
$fakeBrowser = Join-Path $work 'fake-browser.exe'
Add-Type -OutputAssembly $fakeBrowser -OutputType ConsoleApplication -TypeDefinition @'
using System; using System.IO;
class P { static int Main(string[] a) {
    bool probe = false;
    foreach (string s in a) if (s == "--post-update-health-probe") probe = true;
    string ap = Environment.GetEnvironmentVariable("RIG_APPLY_JSON");
    if (probe && ap != null && File.Exists(ap)) {
        string j = File.ReadAllText(ap).Replace("awaiting-health", "healthy");
        string tmp = ap + ".tmp"; File.WriteAllText(tmp, j);
        if (File.Exists(ap)) File.Delete(ap);
        File.Move(tmp, ap);
    }
    return 0;
}}
'@

# ---- throwaway Ed25519 key; publish its raw-32 pub for the manifest seam ----------
$priv = Join-Path $work 'rig_ed.pem'
$pubder = Join-Path $work 'rig_ed_pub.der'
& openssl genpkey -algorithm ed25519 -out $priv 2>$null
& openssl pkey -in $priv -pubout -outform DER -out $pubder 2>$null
$pb = [IO.File]::ReadAllBytes($pubder); $pubB64 = [Convert]::ToBase64String($pb[($pb.Length-32)..($pb.Length-1)])

function Set-File([string]$p,[string]$v) { New-Item -ItemType Directory -Force -Path (Split-Path -Parent $p) | Out-Null; [IO.File]::WriteAllText($p,$v) }
function Read-File([string]$p) { if (Test-Path $p) { [IO.File]::ReadAllText($p) } else { $null } }

$global:allPass = $true
function Check([bool]$c,[string]$m) { if ($c) { Write-Host "  [ok]   $m" -ForegroundColor Green } else { Write-Host "  [FAIL] $m" -ForegroundColor Red; $global:allPass=$false } }

# $mode: 'happy' | 'osblock' | 'integrity'
function Invoke-Case([string]$name,[string]$mode) {
    Write-Host "`n=== CASE: $name ===" -ForegroundColor Cyan
    $sb       = Join-Path $work ([Guid]::NewGuid().ToString('N').Substring(0,8))
    $app      = Join-Path $sb 'app'
    $wallet   = Join-Path $sb 'wallet'
    $update   = Join-Path $sb 'update'
    $pending  = Join-Path $update 'pending'
    $rollback = Join-Path $pending 'rollback'
    $staging  = Join-Path $sb 'staging'      # what the fake installer copies into {app}
    New-Item -ItemType Directory -Force -Path $app,$wallet,$pending,$rollback,(Join-Path $rollback 'wallet'),$staging,(Join-Path $pending 'helper') | Out-Null

    # The OLD build currently in {app} (installer will overwrite) + its rollback backup + wallet snapshot.
    foreach ($f in 'libcef.dll','resources.pak','hodos-wallet.exe','hodos-adblock.exe','hodos-update-helper.exe') {
        Set-File (Join-Path $app $f) 'OLD'; Set-File (Join-Path $rollback $f) 'OLD'
    }
    Set-File (Join-Path $app 'HodosBrowser.exe') 'OLD'
    Set-File (Join-Path $rollback 'HodosBrowser.exe') 'OLD'
    Set-File (Join-Path $wallet 'wallet.db') 'OLD_DB'
    Set-File (Join-Path $rollback 'wallet\wallet.db') 'OLD_DB'

    # The NEW build the installer will lay down. HodosBrowser.exe = the fake browser
    # (happy/integrity) OR a non-runnable file (osblock).
    foreach ($f in 'libcef.dll','resources.pak','hodos-wallet.exe','hodos-adblock.exe','hodos-update-helper.exe') {
        Set-File (Join-Path $staging $f) 'NEW'
    }
    if ($mode -eq 'osblock') { Set-File (Join-Path $staging 'HodosBrowser.exe') 'NOT_A_RUNNABLE_EXE' }
    else                     { Copy-Item $fakeBrowser (Join-Path $staging 'HodosBrowser.exe') }

    # Sign a manifest of the NEW staging with the rig key (build 40101).
    $manifest = Join-Path $pending 'expected-new-manifest.json'
    & python (Join-Path $repoRoot 'scripts\generate-tree-manifest.py') --staging $staging --out $manifest --key $priv --build-number 40101 2>$null
    if ($LASTEXITCODE -ne 0) { Write-Error 'generate-tree-manifest.py failed' }

    # INTEGRITY case: tamper an installed file AFTER signing so the tree won't match.
    if ($mode -eq 'integrity') { Set-File (Join-Path $staging 'libcef.dll') 'TAMPERED_AFTER_SIGNING' }

    # Post-bootstrap apply.json (armed) + global state.
    (@{ schema=1; phase='armed'; fromBuild=40099; toBuild=40101; installerPath=$fakeInstaller;
        rollbackDir=$rollback; rollbackManifestPath=(Join-Path $rollback 'manifest.json');
        expectedNewManifestPath=$manifest; profileId='Default'; toVersion='0.4.1'; signerThumbprint='';
        stagedAt='2026-07-01T00:00:00Z'; failureReason='' } | ConvertTo-Json) | ForEach-Object { Set-File (Join-Path $pending 'apply.json') $_ }
    (@{ schema=1; silent=$true; paused=$false; highWaterBuild=40099; signerThumbprint='';
        lastFailureBuild=0; lastFailureReason=''; rescanAfterRollback=$false } | ConvertTo-Json) | ForEach-Object { Set-File (Join-Path $update 'update-state.json') $_ }

    $env:HODOS_UPDATE_TEST        = '1'
    $env:HODOS_UPDATE_TEST_PUBKEY = $pubB64
    $env:RIG_STAGING              = $staging
    $env:RIG_APP_DIR              = $app
    $env:RIG_APPLY_JSON           = (Join-Path $pending 'apply.json')

    Write-Host "  running: hodos-update-helper (forward apply, mode=$mode)"
    & $helper --app-dir $app --wallet-dir $wallet --update-dir $update --installer $fakeInstaller --to-build 40101 --health-timeout 20 | Out-Null

    $st = (Read-File (Join-Path $update 'update-state.json')) | ConvertFrom-Json
    $ap = (Read-File (Join-Path $pending 'apply.json'))
    if ($mode -eq 'happy') {
        Check ((Read-File (Join-Path $app 'libcef.dll')) -eq 'NEW') 'new build INSTALLED (libcef.dll=NEW)'
        Check ($st.highWaterBuild -eq 40101)                        'COMMIT: highWater advanced to 40101'
        Check ((-not $ap) -or (($ap | ConvertFrom-Json).phase -eq 'healthy')) 'apply.json=healthy (or pending cleaned)'
    } else {
        Check ((Read-File (Join-Path $app 'HodosBrowser.exe')) -eq 'OLD') 'ROLLBACK: HodosBrowser.exe restored to OLD'
        Check ((Read-File (Join-Path $app 'libcef.dll')) -eq 'OLD')       'ROLLBACK: libcef.dll restored to OLD'
        Check ((Read-File (Join-Path $wallet 'wallet.db')) -eq 'OLD_DB')  'ROLLBACK: wallet.db intact (OLD)'
        Check ($st.paused -eq $true)                                     'ROLLBACK: paused=true'
        Check ($st.highWaterBuild -eq 40099)                             'ROLLBACK: highWater NOT advanced (I6)'
    }
    Remove-Item -Recurse -Force $sb -ErrorAction SilentlyContinue
}

try {
    Invoke-Case 'HAPPY (install matches + healthy -> commit)'                 'happy'
    Invoke-Case 'OS-BLOCK (installed exe non-runnable -> rollback)'           'osblock'
    Invoke-Case 'INTEGRITY (installed tree != signed manifest -> rollback)'   'integrity'
}
finally {
    Remove-Item Env:\HODOS_UPDATE_TEST, Env:\HODOS_UPDATE_TEST_PUBKEY, Env:\RIG_STAGING, Env:\RIG_APP_DIR, Env:\RIG_APPLY_JSON -ErrorAction SilentlyContinue
    Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\RunOnce' -Name 'HodosUpdateResume' -ErrorAction SilentlyContinue
    Remove-Item -Recurse -Force $work -ErrorAction SilentlyContinue
}

if ($global:allPass) { Write-Host "`n=== FORWARD-APPLY RIG: ALL ASSERTIONS PASS ===" -ForegroundColor Green; exit 0 }
else                 { Write-Host "`n=== FORWARD-APPLY RIG: FAILURES ABOVE ==="      -ForegroundColor Red;   exit 1 }
