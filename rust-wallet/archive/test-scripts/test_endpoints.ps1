# Test script for wallet database endpoints
# Run this after starting the wallet with: cargo run

Write-Host "🧪 Testing Wallet Database Endpoints" -ForegroundColor Cyan
Write-Host "====================================" -ForegroundColor Cyan
Write-Host ""

$baseUrl = "http://localhost:3301"

# Test 1: Wallet Status
Write-Host "1️⃣  Testing GET /wallet/status" -ForegroundColor Yellow
try {
    $response = Invoke-RestMethod -Uri "$baseUrl/wallet/status" -Method Get
    Write-Host "   ✅ Response: $($response | ConvertTo-Json)" -ForegroundColor Green
} catch {
    Write-Host "   ❌ Error: $_" -ForegroundColor Red
}
Write-Host ""

# Test 2: Get Public Key
Write-Host "2️⃣  Testing POST /getPublicKey" -ForegroundColor Yellow
try {
    $body = @{} | ConvertTo-Json
    $response = Invoke-RestMethod -Uri "$baseUrl/getPublicKey" -Method Post -Body $body -ContentType "application/json"
    Write-Host "   ✅ Public Key: $($response.publicKey)" -ForegroundColor Green
} catch {
    Write-Host "   ❌ Error: $_" -ForegroundColor Red
}
Write-Host ""

# Test 3: Wallet Balance
Write-Host "3️⃣  Testing GET /wallet/balance" -ForegroundColor Yellow
try {
    $response = Invoke-RestMethod -Uri "$baseUrl/wallet/balance" -Method Get
    Write-Host "   ✅ Balance: $($response.balance) satoshis" -ForegroundColor Green
} catch {
    Write-Host "   ❌ Error: $_" -ForegroundColor Red
}
Write-Host ""

# Test 4: Generate Address
Write-Host "4️⃣  Testing POST /wallet/address/generate" -ForegroundColor Yellow
try {
    $body = @{} | ConvertTo-Json
    $response = Invoke-RestMethod -Uri "$baseUrl/wallet/address/generate" -Method Post -Body $body -ContentType "application/json"
    Write-Host "   ✅ Generated Address: $($response.address)" -ForegroundColor Green
    Write-Host "   ✅ Index: $($response.index)" -ForegroundColor Green
    Write-Host "   ✅ Public Key: $($response.publicKey)" -ForegroundColor Green
} catch {
    Write-Host "   ❌ Error: $_" -ForegroundColor Red
}
Write-Host ""

Write-Host "✅ Testing complete!" -ForegroundColor Cyan
