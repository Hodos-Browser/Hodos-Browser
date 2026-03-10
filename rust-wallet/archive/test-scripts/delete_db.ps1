# Script to delete the wallet database file (for testing/reset)
# Run this if you get database errors and want to start fresh

$dbPath = "$env:APPDATA\HodosBrowser\wallet\wallet.db"

if (Test-Path $dbPath) {
    Write-Host "Deleting database file: $dbPath"
    Remove-Item $dbPath -Force
    Write-Host "✅ Database file deleted"
} else {
    Write-Host "Database file not found: $dbPath"
}

# Also delete WAL file if it exists
$walPath = "$env:APPDATA\HodosBrowser\wallet\wallet.db-wal"
if (Test-Path $walPath) {
    Write-Host "Deleting WAL file: $walPath"
    Remove-Item $walPath -Force
    Write-Host "✅ WAL file deleted"
}
