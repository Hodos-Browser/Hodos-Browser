# Hodos Browser Windows Release Build Script
# Usage: .\scripts\build-release.ps1 [-Version "0.1.0-alpha.1"] [-SkipBuild] [-NoInstaller]

param(
    [string]$Version = "0.1.0-alpha.1",
    [switch]$SkipBuild,
    [switch]$NoInstaller
)

$ErrorActionPreference = "Stop"
$ProjectRoot = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
$StagingDir = Join-Path $ProjectRoot "staging\HodosBrowser"
$DistDir = Join-Path $ProjectRoot "dist"

Write-Host "============================================" -ForegroundColor Cyan
Write-Host "  Hodos Browser Release Build v$Version" -ForegroundColor Cyan
Write-Host "============================================" -ForegroundColor Cyan
Write-Host ""

# -- Step 0: Validate prerequisites --
function Test-Command($cmd) { $null -ne (Get-Command $cmd -ErrorAction SilentlyContinue) }

Write-Host "[0/8] Validating prerequisites..." -ForegroundColor Yellow
$missing = @()
if (-not (Test-Command "cargo"))  { $missing += "Rust (cargo)" }
if (-not (Test-Command "node"))   { $missing += "Node.js (node)" }
if (-not (Test-Command "npm"))    { $missing += "npm" }
if (-not (Test-Command "cmake"))  { $missing += "CMake" }

if ($missing.Count -gt 0) {
    Write-Host "ERROR: Missing prerequisites: $($missing -join ', ')" -ForegroundColor Red
    exit 1
}
Write-Host "  All prerequisites found." -ForegroundColor Green

if (-not $SkipBuild) {

    # -- Step 1: Build Rust wallet --
    Write-Host ""
    Write-Host "[1/8] Building Rust wallet..." -ForegroundColor Yellow
    Push-Location (Join-Path $ProjectRoot "rust-wallet")
    cargo build --release
    if ($LASTEXITCODE -ne 0) { Pop-Location; Write-Host "ERROR: Wallet build failed" -ForegroundColor Red; exit 1 }
    Pop-Location
    Write-Host "  Wallet built successfully." -ForegroundColor Green

    # -- Step 2: Build adblock engine --
    Write-Host ""
    Write-Host "[2/8] Building adblock engine..." -ForegroundColor Yellow
    Push-Location (Join-Path $ProjectRoot "adblock-engine")
    cargo build --release
    if ($LASTEXITCODE -ne 0) { Pop-Location; Write-Host "ERROR: Adblock build failed" -ForegroundColor Red; exit 1 }
    Pop-Location
    Write-Host "  Adblock engine built successfully." -ForegroundColor Green

    # -- Step 3: Build frontend --
    Write-Host ""
    Write-Host "[3/8] Building frontend..." -ForegroundColor Yellow
    Push-Location (Join-Path $ProjectRoot "frontend")
    # Install dependencies if node_modules is missing (CI/fresh clone)
    if (-not (Test-Path "node_modules")) {
        Write-Host "  Installing npm dependencies..." -ForegroundColor Yellow
        npm ci
        if ($LASTEXITCODE -ne 0) { Pop-Location; Write-Host "ERROR: npm ci failed" -ForegroundColor Red; exit 1 }
    }
    npm run build
    if ($LASTEXITCODE -ne 0) { Pop-Location; Write-Host "ERROR: Frontend build failed" -ForegroundColor Red; exit 1 }
    Pop-Location
    Write-Host "  Frontend built successfully." -ForegroundColor Green

    # -- Step 4: Build CEF shell --
    Write-Host ""
    Write-Host "[4/8] Building CEF shell..." -ForegroundColor Yellow
    Push-Location (Join-Path $ProjectRoot "cef-native")
    cmake --build build --config Release
    if ($LASTEXITCODE -ne 0) { Pop-Location; Write-Host "ERROR: CEF build failed" -ForegroundColor Red; exit 1 }
    Pop-Location
    Write-Host "  CEF shell built successfully." -ForegroundColor Green

} else {
    Write-Host ""
    Write-Host "[1-4/8] Skipping builds (using existing artifacts)..." -ForegroundColor Yellow
}

# -- Step 5: Assemble staging directory --
Write-Host ""
Write-Host "[5/8] Assembling staging directory..." -ForegroundColor Yellow

$CefRelease = Join-Path $ProjectRoot "cef-native\build\bin\Release"

# Clean and create staging dir
if (Test-Path $StagingDir) { Remove-Item -Recurse -Force $StagingDir }
New-Item -ItemType Directory -Force -Path $StagingDir | Out-Null
New-Item -ItemType Directory -Force -Path (Join-Path $StagingDir "locales") | Out-Null
New-Item -ItemType Directory -Force -Path (Join-Path $StagingDir "frontend") | Out-Null

# Copy main executable
Copy-Item (Join-Path $CefRelease "HodosBrowserShell.exe") $StagingDir

# Copy Rust binaries
Copy-Item (Join-Path $ProjectRoot "rust-wallet\target\release\hodos-wallet.exe") $StagingDir
Copy-Item (Join-Path $ProjectRoot "adblock-engine\target\release\hodos-adblock.exe") $StagingDir

# Copy all CEF runtime DLLs (wildcard to prevent breakage on CEF updates)
Copy-Item (Join-Path $CefRelease "*.dll") $StagingDir

# Copy CEF runtime data files
Copy-Item (Join-Path $CefRelease "*.bin") $StagingDir
Copy-Item (Join-Path $CefRelease "*.dat") $StagingDir

# Copy CEF resource paks
Copy-Item (Join-Path $CefRelease "*.pak") $StagingDir

# Copy SwiftShader config
$swiftshaderJson = Join-Path $CefRelease "vk_swiftshader_icd.json"
if (Test-Path $swiftshaderJson) { Copy-Item $swiftshaderJson $StagingDir }

# Copy only en-US locale
$enUsPak = Join-Path $CefRelease "locales\en-US.pak"
if (Test-Path $enUsPak) {
    Copy-Item $enUsPak (Join-Path $StagingDir "locales\en-US.pak")
} else {
    Write-Host "  WARNING: en-US.pak not found at $enUsPak" -ForegroundColor Yellow
}

# Copy frontend dist (excluding source maps)
$frontendDist = Join-Path $ProjectRoot "frontend\dist"
if (Test-Path $frontendDist) {
    Get-ChildItem -Path $frontendDist -Recurse -File | Where-Object { $_.Extension -ne ".map" } | ForEach-Object {
        $relativePath = $_.FullName.Substring($frontendDist.Length + 1)
        $destPath = Join-Path $StagingDir "frontend\$relativePath"
        $destDir = Split-Path -Parent $destPath
        if (-not (Test-Path $destDir)) { New-Item -ItemType Directory -Force -Path $destDir | Out-Null }
        Copy-Item $_.FullName $destPath
    }
} else {
    Write-Host "  WARNING: Frontend dist not found at $frontendDist" -ForegroundColor Yellow
}

Write-Host "  Staging directory assembled." -ForegroundColor Green

# -- Step 6: Verify assembly --
Write-Host ""
Write-Host "[6/8] Verifying staging directory..." -ForegroundColor Yellow

$requiredFiles = @(
    "HodosBrowserShell.exe",
    "hodos-wallet.exe",
    "hodos-adblock.exe",
    "libcef.dll",
    "chrome_elf.dll",
    "resources.pak",
    "chrome_100_percent.pak",
    "chrome_200_percent.pak",
    "icudtl.dat",
    "v8_context_snapshot.bin",
    "locales\en-US.pak",
    "frontend\index.html"
)

$allPresent = $true
foreach ($file in $requiredFiles) {
    $fullPath = Join-Path $StagingDir $file
    if (-not (Test-Path $fullPath)) {
        Write-Host "  MISSING: $file" -ForegroundColor Red
        $allPresent = $false
    }
}

if (-not $allPresent) {
    Write-Host "ERROR: Staging directory is incomplete!" -ForegroundColor Red
    exit 1
}

# Calculate total size
$totalSize = (Get-ChildItem -Path $StagingDir -Recurse -File | Measure-Object -Property Length -Sum).Sum
$totalSizeMB = [math]::Round($totalSize / 1MB, 1)
Write-Host "  All required files present. Total size: ${totalSizeMB} MB" -ForegroundColor Green

# -- Step 7: Create portable zip --
Write-Host ""
Write-Host "[7/8] Creating portable zip..." -ForegroundColor Yellow

if (-not (Test-Path $DistDir)) { New-Item -ItemType Directory -Force -Path $DistDir | Out-Null }

$zipName = "HodosBrowser-$Version-portable.zip"
$zipPath = Join-Path $DistDir $zipName
if (Test-Path $zipPath) { Remove-Item $zipPath }

Compress-Archive -Path "$StagingDir\*" -DestinationPath $zipPath -CompressionLevel Optimal
$zipSizeMB = [math]::Round((Get-Item $zipPath).Length / 1MB, 1)
Write-Host "  Created: $zipName (${zipSizeMB} MB)" -ForegroundColor Green

# -- Step 8: Run Inno Setup (optional) --
if (-not $NoInstaller) {
    Write-Host ""
    Write-Host "[8/8] Building installer..." -ForegroundColor Yellow

    $iscc = $null
    # Check common Inno Setup install locations
    $issLocations = @(
        "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe",
        "$env:ProgramFiles\Inno Setup 6\ISCC.exe",
        "${env:ProgramFiles(x86)}\Inno Setup 5\ISCC.exe"
    )
    foreach ($loc in $issLocations) {
        if (Test-Path $loc) { $iscc = $loc; break }
    }

    if ($iscc) {
        $issFile = Join-Path $ProjectRoot "installer\hodos-browser.iss"
        if (Test-Path $issFile) {
            & $iscc "/DAppVersion=$Version" "/DProjectRoot=$ProjectRoot" "/DStagingDir=$StagingDir" "/DDistDir=$DistDir" $issFile
            if ($LASTEXITCODE -eq 0) {
                Write-Host "  Installer built successfully." -ForegroundColor Green
            } else {
                Write-Host "  WARNING: Installer build failed (exit code $LASTEXITCODE)" -ForegroundColor Yellow
            }
        } else {
            Write-Host "  WARNING: Inno Setup script not found at $issFile" -ForegroundColor Yellow
        }
    } else {
        Write-Host "  Inno Setup not found - skipping installer. Install from: https://jrsoftware.org/isinfo.php" -ForegroundColor Yellow
    }
} else {
    Write-Host ""
    Write-Host "[8/8] Skipping installer (--NoInstaller)." -ForegroundColor Yellow
}

# -- Done --
Write-Host ""
Write-Host "============================================" -ForegroundColor Green
Write-Host "  Build complete!" -ForegroundColor Green
Write-Host "============================================" -ForegroundColor Green
Write-Host ""
Write-Host "  Staging: $StagingDir"
Write-Host "  Portable: $(Join-Path $DistDir $zipName)"
if (-not $NoInstaller -and $iscc) {
    Write-Host "  Installer: $(Join-Path $DistDir "HodosBrowser-$Version-setup.exe")"
}
Write-Host ""
