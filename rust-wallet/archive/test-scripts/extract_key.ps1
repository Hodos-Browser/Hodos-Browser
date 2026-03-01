# PowerShell script to extract master private key from wallet database
# This extracts the key needed for the TypeScript CSR comparison test

Write-Host "Extracting master private key from wallet database..." -ForegroundColor Cyan
Write-Host ""

Set-Location $PSScriptRoot

# Run the extraction utility
cargo run --bin extract_master_key

$exitCode = $LASTEXITCODE
if ($exitCode -eq 0) {
    Write-Host ""
    Write-Host "Key extraction complete!" -ForegroundColor Green
    Write-Host ""
    Write-Host "Next steps:" -ForegroundColor Cyan
    Write-Host "   1. Copy the hex value shown above" -ForegroundColor White
    Write-Host "   2. Paste it into test_csr_comparison.ts as subjectPrivateKeyHex" -ForegroundColor White
    Write-Host "   3. Run the TypeScript comparison test" -ForegroundColor White
} else {
    Write-Host ""
    Write-Host "Key extraction failed with exit code: $exitCode" -ForegroundColor Red
}
