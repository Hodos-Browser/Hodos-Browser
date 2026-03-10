# PowerShell script to run interoperability tests
# This script runs both TypeScript and Rust tests and compares results

Write-Host "Interoperability Test Suite" -ForegroundColor Cyan
Write-Host "===========================" -ForegroundColor Cyan
Write-Host ""

# Step 1: Run TypeScript test to generate test vectors
Write-Host "Step 1: Generating test vectors with TypeScript SDK..." -ForegroundColor Yellow
$tsTestPath = Join-Path $PSScriptRoot "test_interoperability_ts.js"
if (Test-Path $tsTestPath) {
    node $tsTestPath
    if ($LASTEXITCODE -ne 0) {
        Write-Host "❌ TypeScript test failed!" -ForegroundColor Red
        exit 1
    }
} else {
    Write-Host "⚠️  TypeScript test script not found: $tsTestPath" -ForegroundColor Yellow
    Write-Host "   Skipping TypeScript test generation" -ForegroundColor Yellow
}

Write-Host ""
Write-Host "Step 2: Running Rust interoperability tests..." -ForegroundColor Yellow
Write-Host ""

# Step 2: Run Rust tests
cd $PSScriptRoot
cargo test interoperability_test -- --nocapture

if ($LASTEXITCODE -eq 0) {
    Write-Host ""
    Write-Host "✅ All interoperability tests passed!" -ForegroundColor Green
} else {
    Write-Host ""
    Write-Host "❌ Some interoperability tests failed!" -ForegroundColor Red
    exit 1
}

