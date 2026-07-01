# test-update-feed.ps1 — localhost auto-update test rig (WINDOWS_AUTOUPDATE_PLAN
# commit 4b). Stands up a self-contained Hodos update feed on http://127.0.0.1
# and drives the UpdateStagerRig.StagesFromLocalFeed integration test through the
# FULL path: appcast fetch → installer download → EdDSA verify → stage + marker.
#
# It signs a dummy installer with a THROWAWAY Ed25519 keypair and points the
# updater at that key via the HODOS_UPDATE_TEST_PUBKEY seam (the production
# private key is a CI secret, unavailable locally). EdDSA stays a real hard gate
# — just verified against the rig's test key in test mode.
#
# Prereqs: openssl + python on PATH, and a built hodos_tests.exe
# (cmake --build cef-native/build --config Release --target hodos_tests).
#
# Usage:  pwsh -File scripts/test-update-feed.ps1
#         pwsh -File scripts/test-update-feed.ps1 -Port 38217

param(
    [int]$Port = 38217
)

$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path -Parent $PSScriptRoot
$testExe  = Join-Path $repoRoot 'cef-native\build\bin\Release\hodos_tests.exe'

if (-not (Test-Path $testExe)) {
    Write-Error "hodos_tests.exe not found at $testExe. Build it first:`n  cmake --build cef-native/build --config Release --target hodos_tests"
}
foreach ($tool in @('openssl', 'python')) {
    if (-not (Get-Command $tool -ErrorAction SilentlyContinue)) {
        Write-Error "'$tool' not found on PATH (required by the rig)."
    }
}

$work    = Join-Path ([System.IO.Path]::GetTempPath()) ("hodos-update-rig-" + [System.Guid]::NewGuid().ToString('N').Substring(0,8))
$feedDir = Join-Path $work 'feed'
$pending = Join-Path $work 'pending'
New-Item -ItemType Directory -Force -Path $feedDir, $pending | Out-Null

$server = $null
try {
    # 1) Throwaway Ed25519 keypair; extract the raw 32-byte public key (base64).
    $priv   = Join-Path $work 'test_ed.pem'
    $pubder = Join-Path $work 'test_ed_pub.der'
    & openssl genpkey -algorithm ed25519 -out $priv 2>$null
    & openssl pkey -in $priv -pubout -outform DER -out $pubder 2>$null
    $pubBytes = [System.IO.File]::ReadAllBytes($pubder)
    $rawPub   = $pubBytes[($pubBytes.Length - 32)..($pubBytes.Length - 1)]
    $pubB64   = [Convert]::ToBase64String($rawPub)

    # 2) Dummy "installer" with deterministic content.
    $installer = Join-Path $feedDir 'installer.exe'
    [System.IO.File]::WriteAllBytes($installer, [byte[]](1..4096 | ForEach-Object { $_ % 256 }))
    $size = (Get-Item $installer).Length

    # 3) Sign the installer bytes (Ed25519 is one-shot over raw data → -rawin).
    $sigbin = Join-Path $work 'installer.sig'
    & openssl pkeyutl -sign -inkey $priv -rawin -in $installer -out $sigbin 2>$null
    $sigB64 = [Convert]::ToBase64String([System.IO.File]::ReadAllBytes($sigbin))

    # 4) Build the appcast with the REAL generator (exercises generate-appcast.py
    #    incl. the new <hodosBuildNumber> emission). Dummy DSA sig: the Hodos
    #    updater only verifies edSignature; WinSparkle isn't in this loop.
    $appcast = Join-Path $feedDir 'appcast.xml'
    & python (Join-Path $repoRoot 'scripts\generate-appcast.py') `
        --version '99.9.9' --build-number 99999999 `
        --windows-url "http://127.0.0.1:$Port/installer.exe" --windows-size $size `
        --windows-signature 'DUMMYDSA' --windows-ed-signature $sigB64 `
        --output $appcast
    if ($LASTEXITCODE -ne 0) { Write-Error 'generate-appcast.py failed' }

    # 4b) Sign the WHOLE appcast document → sidecar appcast.xml.ed (commit 4c).
    #     Same throwaway key as the installer; the client verifies this before
    #     parsing. Exercises scripts/sign-appcast.py + the domain-separation prefix.
    & python (Join-Path $repoRoot 'scripts\sign-appcast.py') `
        --in $appcast --key $priv --out "$appcast.ed"
    if ($LASTEXITCODE -ne 0) { Write-Error 'sign-appcast.py failed' }

    # 4c) Generate + sign the expected-new-manifest (commit 6c.3 made staging REQUIRE
    #     it: StagePendingUpdate fetches it from the sibling-of-installer URL, verifies
    #     the sig with the same throwaway key, and binds buildNumber == the appcast
    #     build). The tree content is irrelevant to STAGING (the tree check is an
    #     apply-time gate) - one dummy file is enough.
    $mstaging = Join-Path $work 'mstaging'
    New-Item -ItemType Directory -Force -Path $mstaging | Out-Null
    [System.IO.File]::WriteAllText((Join-Path $mstaging 'HodosBrowser.exe'), 'dummy')
    & python (Join-Path $repoRoot 'scripts\generate-tree-manifest.py') `
        --staging $mstaging --out (Join-Path $feedDir 'expected-new-manifest.json') `
        --key $priv --build-number 99999999
    if ($LASTEXITCODE -ne 0) { Write-Error 'generate-tree-manifest.py failed' }

    # 5) Serve the feed dir on localhost.
    $server = Start-Process python -ArgumentList @('-m','http.server',"$Port",'--bind','127.0.0.1') `
        -WorkingDirectory $feedDir -PassThru -WindowStyle Hidden
    # Wait for the listener.
    $ready = $false
    for ($i = 0; $i -lt 50; $i++) {
        try {
            Invoke-WebRequest "http://127.0.0.1:$Port/appcast.xml" -UseBasicParsing -TimeoutSec 2 | Out-Null
            $ready = $true; break
        } catch { Start-Sleep -Milliseconds 200 }
    }
    if (-not $ready) { Write-Error "test feed server did not come up on port $Port" }

    # 6) Drive the integration test through the live feed.
    $env:HODOS_UPDATE_TEST        = '1'
    $env:HODOS_UPDATE_TEST_PUBKEY = $pubB64
    $env:HODOS_UPDATE_RIG_URL     = "http://127.0.0.1:$Port/appcast.xml"
    $env:HODOS_UPDATE_RIG_PENDING = ($pending -replace '\\','/')

    Write-Host "Running UpdateStagerRig against http://127.0.0.1:$Port/appcast.xml ..."
    & $testExe --gtest_filter='UpdateStagerRig.*'
    $code = $LASTEXITCODE

    if ($code -eq 0) {
        Write-Host "`n=== RIG PASS - staged + marker written ===" -ForegroundColor Green
        if (Test-Path (Join-Path $pending 'update-info.json')) {
            Get-Content (Join-Path $pending 'update-info.json')
        }
    } else {
        Write-Host "`n=== RIG FAIL (exit $code) ===" -ForegroundColor Red
    }
    exit $code
}
finally {
    if ($server -and -not $server.HasExited) { Stop-Process -Id $server.Id -Force -ErrorAction SilentlyContinue }
    Remove-Item Env:\HODOS_UPDATE_TEST, Env:\HODOS_UPDATE_TEST_PUBKEY, Env:\HODOS_UPDATE_RIG_URL, Env:\HODOS_UPDATE_RIG_PENDING -ErrorAction SilentlyContinue
    Remove-Item -Recurse -Force $work -ErrorAction SilentlyContinue
}
