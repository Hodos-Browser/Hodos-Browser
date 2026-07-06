# setup-real-apply-test.ps1 - STAGE 2 of the silent-update test plan (real-build test).
# See development-docs/DevOps-CICD/SILENT_UPDATE_TEST_PLAN.md.
#
# Sets up a REAL local silent-update on a THROWAWAY DEV wallet: builds an "N" build
# (0.4.0) and installs it into the dev app dir, builds an "N+1" build (0.4.1), and
# PRE-STAGES a signed N+1 update next to it. Then you launch the installed N build and
# the REAL bootstrap applies it: backs up the old build, snapshots the dev wallet DB,
# runs the (fake, no-Inno) installer, launches the REAL browser as a health probe, and
# COMMITS if healthy. -Break makes the new build fail its health check so you can watch
# the REAL rollback keep the wallet intact.
#
#   Happy path:  pwsh -File scripts/setup-real-apply-test.ps1
#   Rollback:    pwsh -File scripts/setup-real-apply-test.ps1 -Break
#   Re-stage only (skip the ~min-long builds, reuse the installed trees):
#                pwsh -File scripts/setup-real-apply-test.ps1 -SkipBuild
#                pwsh -File scripts/setup-real-apply-test.ps1 -Break -SkipBuild
#
# SAFETY: dev namespace only (HodosBrowserDev). Your real wallet (%APPDATA%\HodosBrowser)
# is never touched. ABORTS if any Hodos backend is listening (31301/31302/31401/31402).
# Prereqs: cmake+vcpkg-configured cef-native/build, cargo, npm, openssl, python.

param(
    [switch]$Break,
    [switch]$SkipBuild,
    [int]$NNum  = 40099,   # "N"   build number (0.4.0)
    [int]$N1Num = 40199    # "N+1" build number (0.4.1) - MUST be > NNum
)

$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path -Parent $PSScriptRoot
function Say([string]$m,[string]$c='Cyan') { Write-Host $m -ForegroundColor $c }

# ---- 0a. Put openssl + python on PATH even if the shell doesn't have them --------
# (Git ships openssl at usr\bin; the py launcher knows where python lives. The manifest
#  signer shells out to openssl too, so it must be on PATH for the whole process.)
function Add-ToPath([string]$dir) {
    if ($dir -and (Test-Path $dir) -and (";$env:PATH;" -notlike "*;$dir;*")) { $env:PATH = "$dir;$env:PATH" }
}
if (-not (Get-Command openssl -ErrorAction SilentlyContinue)) {
    foreach ($c in @("$env:ProgramFiles\Git\usr\bin", "$env:ProgramFiles\Git\mingw64\bin",
                     "${env:ProgramFiles(x86)}\Git\usr\bin")) {
        if (Test-Path (Join-Path $c 'openssl.exe')) { Add-ToPath $c; break }
    }
}
if (-not (Get-Command python -ErrorAction SilentlyContinue)) {
    $pyExe = $null
    try { $pyExe = (& py -c "import sys; print(sys.executable)" 2>$null) } catch {}
    if ($pyExe -and (Test-Path $pyExe)) { Add-ToPath (Split-Path -Parent $pyExe) }
}

# ---- 0. Preflight + safety -------------------------------------------------------
foreach ($tool in @('cmake','cargo','npm','openssl','python')) {
    if (-not (Get-Command $tool -ErrorAction SilentlyContinue)) { Write-Error "'$tool' not on PATH" }
}
foreach ($port in @(31301,31302,31401,31402)) {
    $live = $false
    try { $c = New-Object Net.Sockets.TcpClient; $c.Connect('127.0.0.1',$port); $live = $c.Connected; $c.Close() } catch {}
    if ($live) { Write-Error "Port $port is listening - close Hodos AND any dev wallet/adblock before running this." }
}
if ($N1Num -le $NNum) { Write-Error "N1Num ($N1Num) must be greater than NNum ($NNum)" }

$local     = $env:LOCALAPPDATA
$roaming   = $env:APPDATA
$devApp    = Join-Path $local  'HodosBrowserDev'            # {app} - where N is "installed"
$devWallet = Join-Path $roaming 'HodosBrowserDev\wallet'    # money DB (ROAMING)
$devLog    = Join-Path $roaming 'HodosBrowserDev\logs'      # debug_output.log (bug-#2 fix: out of {app})
$devUpdate = Join-Path $devApp 'update'
$devPending= Join-Path $devUpdate 'pending'
$rig       = Join-Path $local  'Hodos-rig'                  # scratch: keys, fake installer, build trees
$treeN     = Join-Path $rig 'tree-N'
$treeN1    = Join-Path $rig 'tree-N1'
$cefRel    = Join-Path $repoRoot 'cef-native\build\bin\Release'
New-Item -ItemType Directory -Force -Path $rig | Out-Null

Say "=== Stage-2 real-build test setup ($(if($Break){'ROLLBACK leg'}else{'HAPPY leg'})) ===" 'Green'
Say "  dev app dir : $devApp"
Say "  dev wallet  : $devWallet"

# ---- helpers ---------------------------------------------------------------------
function Assemble-Tree([string]$dest) {
    if (Test-Path $dest) { Remove-Item -Recurse -Force $dest }
    New-Item -ItemType Directory -Force -Path $dest,(Join-Path $dest 'locales'),(Join-Path $dest 'frontend') | Out-Null
    Copy-Item (Join-Path $cefRel 'HodosBrowser.exe') $dest
    Copy-Item (Join-Path $repoRoot 'rust-wallet\target\release\hodos-wallet.exe') $dest
    Copy-Item (Join-Path $repoRoot 'adblock-engine\target\release\hodos-adblock.exe') $dest
    Copy-Item (Join-Path $cefRel 'hodos-update-helper.exe') $dest
    Copy-Item (Join-Path $cefRel '*.dll') $dest
    $ws = Join-Path $repoRoot 'external\winsparkle\WinSparkle-0.8.1\x64\Release\WinSparkle.dll'
    if (Test-Path $ws) { Copy-Item $ws $dest }
    Copy-Item (Join-Path $cefRel '*.bin') $dest
    Copy-Item (Join-Path $cefRel '*.dat') $dest
    Copy-Item (Join-Path $cefRel '*.pak') $dest
    $ss = Join-Path $cefRel 'vk_swiftshader_icd.json'; if (Test-Path $ss) { Copy-Item $ss $dest }
    $enus = Join-Path $cefRel 'locales\en-US.pak'
    if (Test-Path $enus) { Copy-Item $enus (Join-Path $dest 'locales\en-US.pak') }
    $fd = Join-Path $repoRoot 'frontend\dist'
    Get-ChildItem -Path $fd -Recurse -File | Where-Object { $_.Extension -ne '.map' } | ForEach-Object {
        $rel = $_.FullName.Substring($fd.Length + 1); $dp = Join-Path $dest "frontend\$rel"
        New-Item -ItemType Directory -Force -Path (Split-Path -Parent $dp) | Out-Null
        Copy-Item $_.FullName $dp
    }
    foreach ($f in @('HodosBrowser.exe','hodos-wallet.exe','hodos-update-helper.exe','libcef.dll','resources.pak','frontend\index.html')) {
        if (-not (Test-Path (Join-Path $dest $f))) { Write-Error "assembled tree missing $f" }
    }
}
function Build-Shell([string]$ver,[int]$num) {
    Say "  building CEF shell $ver (build $num) ..."
    $cfgArgs = @(
        '-S', (Join-Path $repoRoot 'cef-native'),
        '-B', (Join-Path $repoRoot 'cef-native\build'),
        '-DHODOS_SILENT_AUTOUPDATE=ON', '-DHODOS_UPDATE_TEST_SEAM=ON',
        "-DAPP_VERSION=$ver", "-DAPP_BUILD_NUMBER=$num"
    )
    & cmake @cfgArgs | Out-Null
    if ($LASTEXITCODE -ne 0) { Write-Error "cmake configure failed for $ver" }
    & cmake --build (Join-Path $repoRoot 'cef-native\build') --config Release --target HodosBrowserShell hodos-update-helper | Out-Null
    if ($LASTEXITCODE -ne 0) { Write-Error "cmake build failed for $ver" }
}

# ---- 1. Build shared parts + N + N+1 trees ---------------------------------------
if (-not $SkipBuild) {
    Say "[1/5] Building shared backends (rust wallet, adblock, frontend)..."
    & cargo build --release --manifest-path (Join-Path $repoRoot 'rust-wallet\Cargo.toml') | Out-Null
    if ($LASTEXITCODE -ne 0) { Write-Error 'rust-wallet build failed' }
    & cargo build --release --manifest-path (Join-Path $repoRoot 'adblock-engine\Cargo.toml') | Out-Null
    if ($LASTEXITCODE -ne 0) { Write-Error 'adblock build failed' }
    Push-Location (Join-Path $repoRoot 'frontend'); & npm run build | Out-Null; $fe = $LASTEXITCODE; Pop-Location
    if ($fe -ne 0) { Write-Error 'frontend build failed' }

    Say "[2/5] Building + assembling N (0.4.0) and N+1 (0.4.1)..."
    Build-Shell '0.4.0' $NNum;  Assemble-Tree $treeN
    Build-Shell '0.4.1' $N1Num; Assemble-Tree $treeN1
} else {
    Say "[1-2/5] -SkipBuild: reusing existing $treeN / $treeN1"
    if (-not (Test-Path (Join-Path $treeN 'HodosBrowser.exe')))  { Write-Error "no $treeN - run once without -SkipBuild first" }
    if (-not (Test-Path (Join-Path $treeN1 'HodosBrowser.exe'))) { Write-Error "no $treeN1 - run once without -SkipBuild first" }
}

# ---- 3. "Install" N into the dev app dir -----------------------------------------
Say "[3/5] Installing N into $devApp ..."
if (Test-Path $devApp) {
    # Preserve the dev wallet/profile data (those live under %APPDATA%, not here) but
    # clear the old install + any stale update working area.
    Get-ChildItem -Path $devApp -Force | Where-Object { $_.Name -ne 'update' } | Remove-Item -Recurse -Force -ErrorAction SilentlyContinue
}
New-Item -ItemType Directory -Force -Path $devApp | Out-Null
Copy-Item (Join-Path $treeN '*') $devApp -Recurse -Force

# The "new build" the fake installer lays down: N+1 (happy) or the N tree (break -> the
# health probe runs a binary whose baked APP_BUILD_NUMBER != toBuild -> rollback).
$newTree = if ($Break) { $treeN } else { $treeN1 }

# ---- 4. Throwaway key + fake installer + manifest --------------------------------
Say "[4/5] Signing + pre-staging the N+1 update..."
$priv = Join-Path $rig 'rig_ed.pem'; $pubder = Join-Path $rig 'rig_ed_pub.der'
& openssl genpkey -algorithm ed25519 -out $priv 2>$null
& openssl pkey -in $priv -pubout -outform DER -out $pubder 2>$null
$pb = [IO.File]::ReadAllBytes($pubder); $pubB64 = [Convert]::ToBase64String($pb[($pb.Length-32)..($pb.Length-1)])

$fakeInstaller = Join-Path $rig 'hodos-rig-installer.exe'
$fakeSrc = Join-Path $rig 'hodos-rig-installer.cs'
@'
using System; using System.IO;
class P { static int Main(string[] a) {
    string src = Environment.GetEnvironmentVariable("RIG_STAGING");
    string dst = Environment.GetEnvironmentVariable("RIG_APP_DIR");
    if (src == null || dst == null) return 2;
    foreach (string f in Directory.GetFiles(src, "*", SearchOption.AllDirectories)) {
        string rel = f.Substring(src.Length).TrimStart('\\','/');
        string t = Path.Combine(dst, rel);
        Directory.CreateDirectory(Path.GetDirectoryName(t));
        try { File.Copy(f, t, true); } catch {}   // {app}\HodosBrowser.exe may be locked by the exiting probe; best-effort
    }
    return 0;
}}
'@ | Set-Content -Path $fakeSrc -Encoding ascii
# Compile with the .NET Framework C# compiler. Works under BOTH Windows PowerShell 5.1
# AND PowerShell 7 — unlike `Add-Type -OutputAssembly -OutputType ConsoleApplication`,
# which .NET Core (pwsh) does not support.
$csc = Join-Path $env:WINDIR 'Microsoft.NET\Framework64\v4.0.30319\csc.exe'
if (-not (Test-Path $csc)) { Write-Error "csc.exe not found at $csc (need .NET Framework 4.x)" }
if (Test-Path $fakeInstaller) { Remove-Item -Force $fakeInstaller }
& $csc /nologo /target:exe "/out:$fakeInstaller" $fakeSrc | Out-Null
if ($LASTEXITCODE -ne 0 -or -not (Test-Path $fakeInstaller)) { Write-Error 'fake-installer compile (csc) failed' }

# Fresh pending\ (clears any prior run's staged files + rollback backup).
if (Test-Path $devPending) { Remove-Item -Recurse -Force $devPending }
New-Item -ItemType Directory -Force -Path $devPending | Out-Null
Copy-Item $fakeInstaller (Join-Path $devPending 'hodos-rig-installer.exe')
$instSha = (Get-FileHash (Join-Path $devPending 'hodos-rig-installer.exe') -Algorithm SHA256).Hash.ToLower()

# Signed manifest of the NEW tree (buildNumber bound = N1Num, verified by the bootstrap).
& python (Join-Path $repoRoot 'scripts\generate-tree-manifest.py') `
    --staging $newTree --out (Join-Path $devPending 'expected-new-manifest.json') `
    --key $priv --build-number $N1Num | Out-Null
if ($LASTEXITCODE -ne 0) { Write-Error 'generate-tree-manifest.py failed' }

# Marker (update-info.json) - fields per UpdateStager::ParseMarker.
(@{ buildNumber=$N1Num; version='0.4.1'; installerFileName='hodos-rig-installer.exe';
    sha256=$instSha; edVerified=$true; authenticodeVerified=$false; signer='';
    signerThumbprint=''; stagedAt='2026-07-03T00:00:00Z' } | ConvertTo-Json) `
    | Set-Content -Path (Join-Path $devPending 'update-info.json') -Encoding ascii

# Force the DEV environment into SILENT mode (this test proves the silent WRITER + apply
# path; the conservative cross-profile collapse is unit-tested separately). We do NOT hand-
# seed update-state.json silent=true — instead we set the user-facing GLOBAL update setting
# to "silent", which is exactly the input the app's own writer must translate. Writing the
# global updateMode also makes LoadInternal treat it as authoritative and SKIP the one-time
# collapse (which would otherwise pick the most-conservative across any stray dev profiles).
$devGlobalSettings = Join-Path $roaming 'HodosBrowserDev\settings.json'
New-Item -ItemType Directory -Force -Path (Split-Path -Parent $devGlobalSettings) | Out-Null
$gs = @{}
if (Test-Path $devGlobalSettings) {
    try {
        $obj = Get-Content $devGlobalSettings -Raw | ConvertFrom-Json
        if ($obj) { $obj.PSObject.Properties | ForEach-Object { $gs[$_.Name] = $_.Value } }
    } catch {}
}
$gs['updateMode'] = 'silent'
($gs | ConvertTo-Json -Depth 10) | Set-Content -Path $devGlobalSettings -Encoding ascii
Say "  forced dev global updateMode = silent (the scenario under test)"

# Global state: seed silent=FALSE (NOT eligible yet). This deliberately does NOT hand-seed
# silent=true — the whole point of commit #1 is that the app's OWN silent-state writer must
# flip this to true on a normal launch (from the global autoUpdateMode). If we seeded true
# here we'd be masking a missing writer exactly like the pre-#1 rig did. The two-phase launch
# script below runs the app once (the writer flips silent -> true), then relaunches to apply.
# high-water below N1Num, not paused, no prior rejection.
(@{ schema=1; silent=$false; paused=$false; highWaterBuild=$NNum; signerThumbprint='';
    lastFailureBuild=0; lastFailureReason=''; rescanAfterRollback=$false } | ConvertTo-Json) `
    | Set-Content -Path (Join-Path $devUpdate 'update-state.json') -Encoding ascii

# Record wallet-db fingerprint so you can prove it's intact afterwards.
$walletDb = Join-Path $devWallet 'wallet.db'
$haveWallet = Test-Path $walletDb
if ($haveWallet) { (Get-FileHash $walletDb -Algorithm SHA256).Hash.ToLower() | Set-Content (Join-Path $rig 'wallet-db-before.txt') }

# ---- 5. Write the launch + verify scripts ----------------------------------------
$launch = Join-Path $rig 'launch-real-apply-test.ps1'
@"
# TWO-PHASE launch. Phase 1 lets the app's OWN silent-state writer (commit #1) create the
# eligibility (flip update-state.json silent false->true from the global autoUpdateMode) —
# NOT hand-seeded. Phase 2 relaunches so the REAL bootstrap applies the staged N+1. This is
# the proof the writer works: if the app never flips silent, phase 2 will not apply.
`$env:HODOS_DEV = '1'
`$env:HODOS_UPDATE_TEST = '1'
`$env:HODOS_UPDATE_TEST_PUBKEY = '$pubB64'
`$env:RIG_STAGING = '$newTree'
`$env:RIG_APP_DIR = '$devApp'
# --profile=Default skips the profile picker (else it trips the apply's sole-instance
# gate). WorkingDirectory = {app} deliberately MIRRORS the production shortcut (bug #2).
Write-Host 'PHASE 1: prime launch — the app writes update-state.json silent=true itself.' -ForegroundColor Cyan
Write-Host '  (make sure Software updates = Automatic in Settings; it is the default.)' -ForegroundColor DarkGray
Write-Host '  Wait for the window, then CLOSE it to continue.' -ForegroundColor Cyan
Start-Process -FilePath '$devApp\HodosBrowser.exe' -ArgumentList '--profile=Default' -WorkingDirectory '$devApp' -Wait
`$st = Get-Content '$devUpdate\update-state.json' -Raw | ConvertFrom-Json
Write-Host ("  update-state.json silent = {0}  (expect True — written by the app, not the rig)" -f `$st.silent) -ForegroundColor Yellow
if (-not `$st.silent) { Write-Error 'WRITER FAILED: app did not flip silent=true — commit #1 is not working'; exit 1 }
Write-Host 'PHASE 2: relaunch — the real bootstrap now applies the staged N+1.' -ForegroundColor Cyan
Write-Host '  watch $devLog\debug_output.log for "Silent apply:" lines.' -ForegroundColor DarkGray
Start-Process -FilePath '$devApp\HodosBrowser.exe' -ArgumentList '--profile=Default' -WorkingDirectory '$devApp'
"@ | Set-Content -Path $launch -Encoding ascii

$verify = Join-Path $rig 'verify-real-apply-test.ps1'
@"
`$state = Get-Content '$devUpdate\update-state.json' -Raw | ConvertFrom-Json
`$applyP = '$devPending\apply.json'
`$phase = if (Test-Path `$applyP) { (Get-Content `$applyP -Raw | ConvertFrom-Json).phase } else { '(pending cleaned)' }
Write-Host ("highWaterBuild = {0}  (expect {1} on commit, {2} on rollback)" -f `$state.highWaterBuild, $N1Num, $NNum)
Write-Host ("paused         = {0}  (expect False on commit, True on rollback)" -f `$state.paused)
Write-Host ("apply.json     = {0}" -f `$phase)
`$dbNow = '(no wallet)'
if (Test-Path '$walletDb') {
    try { `$dbNow = (Get-FileHash '$walletDb' -Algorithm SHA256).Hash.ToLower() }
    catch { `$dbNow = '(in use - browser still running; close it, then re-run this verify)' }
}
`$dbWas = if (Test-Path '$rig\wallet-db-before.txt') { (Get-Content '$rig\wallet-db-before.txt' -Raw).Trim() } else { '(none)' }
Write-Host ("wallet.db sha  = {0}" -f `$dbNow)
Write-Host ("  before       = {0}" -f `$dbWas)
Write-Host ("  -> wallet.db present + readable = {0}" -f (Test-Path '$walletDb'))
"@ | Set-Content -Path $verify -Encoding ascii

# ---- done ------------------------------------------------------------------------
Say ""
Say "=== READY ($(if($Break){'ROLLBACK leg'}else{'HAPPY leg'})) ===" 'Green'
if (-not $haveWallet) {
    Say "  NOTE: no dev wallet.db found at $walletDb." 'Yellow'
    Say "        The apply/rollback will still run, but the wallet-safety leg is not exercised." 'Yellow'
    Say "        To include it: launch the dev browser normally once + create a THROWAWAY wallet, then re-run." 'Yellow'
}
Say ""
Say "STEP A - launch it:"
Say "    powershell -ExecutionPolicy Bypass -File `"$launch`"" 'White'
Say ""
Say "STEP B - watch $devLog\debug_output.log. Expect the bootstrap to log"
Say "         'Silent apply: eligible ...' then the supervisor to install + health-check."
if ($Break) {
    Say "         (this is the -Break leg: the new build's number won't match -> ROLLBACK)"
}
Say ""
Say "STEP C - after it settles (~30-90s), verify:"
Say "    powershell -ExecutionPolicy Bypass -File `"$verify`"" 'White'
if ($Break) {
    Say "    EXPECT: highWater=$NNum, paused=True, old build restored, wallet.db intact."
} else {
    Say "    EXPECT: highWater=$N1Num, apply.json=healthy (or cleaned), wallet.db intact."
}
Say ""
Say "  Cleanup when done:  Remove-Item -Recurse -Force '$rig','$devApp'"
