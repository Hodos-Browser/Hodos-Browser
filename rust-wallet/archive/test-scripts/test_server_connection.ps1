# Quick test to verify the TypeScript SDK server is accessible

Write-Host "Testing connection to TypeScript SDK server..." -ForegroundColor Cyan
Write-Host ""

# Test 1: Check if server is responding
try {
    $response = Invoke-WebRequest -Uri "http://localhost:3001/certifierPublicKey" -Method GET -UseBasicParsing -ErrorAction Stop
    Write-Host "✅ Server is accessible!" -ForegroundColor Green
    Write-Host "   Status: $($response.StatusCode)" -ForegroundColor Gray
    $json = $response.Content | ConvertFrom-Json
    Write-Host "   Certifier Public Key: $($json.certifier)" -ForegroundColor Yellow
    Write-Host ""
} catch {
    Write-Host "❌ Cannot connect to server!" -ForegroundColor Red
    Write-Host "   Error: $($_.Exception.Message)" -ForegroundColor Red
    Write-Host ""
    Write-Host "Make sure the server is running:" -ForegroundColor Yellow
    Write-Host "   node rust-wallet/test_ts_sdk_server.js" -ForegroundColor White
    Write-Host ""
    exit 1
}

# Test 2: Check if port is listening
$port = Get-NetTCPConnection -LocalPort 3001 -ErrorAction SilentlyContinue
if ($port) {
    Write-Host "✅ Port 3001 is listening" -ForegroundColor Green
} else {
    Write-Host "⚠️  Port 3001 is not listening" -ForegroundColor Yellow
}

Write-Host ""
