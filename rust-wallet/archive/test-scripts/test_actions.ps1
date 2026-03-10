# Test script for action storage endpoints
# Run this with: .\test_actions.ps1

$baseUrl = "http://localhost:3301"

Write-Host "========================================" -ForegroundColor Cyan
Write-Host "Testing Action Storage System" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

# Test 1: List actions (should be empty initially)
Write-Host "Test 1: List all actions (empty initially)" -ForegroundColor Yellow
$response = Invoke-RestMethod -Uri "$baseUrl/listActions" -Method POST -ContentType "application/json" -Body '{}'
Write-Host "Total Actions: $($response.totalActions)" -ForegroundColor Green
Write-Host "Actions: $($response.actions | ConvertTo-Json -Depth 5)" -ForegroundColor Green
Write-Host ""

# Test 2: Manually add a test action (using direct file manipulation)
Write-Host "Test 2: Adding a test action manually..." -ForegroundColor Yellow
$actionsPath = "$env:APPDATA\HodosBrowser\wallet\actions.json"
Write-Host "Actions file: $actionsPath" -ForegroundColor Cyan

# Create test action
$testAction = @{
    "test_txid_12345" = @{
        txid = "test_txid_12345"
        referenceNumber = "test_ref_123"
        rawTx = "0100000001..."
        description = "Test transaction"
        labels = @("test", "shopping")
        status = "created"
        isOutgoing = $true
        satoshis = 50000
        timestamp = [DateTimeOffset]::UtcNow.ToUnixTimeSeconds()
        blockHeight = $null
        confirmations = 0
        version = 1
        lockTime = 0
        inputs = @()
        outputs = @()
    }
}

# Use UTF8 without BOM to avoid JSON parsing issues
$json = $testAction | ConvertTo-Json -Depth 10
[System.IO.File]::WriteAllText($actionsPath, $json, [System.Text.UTF8Encoding]::new($false))
Write-Host "✅ Test action created in actions.json" -ForegroundColor Green
Write-Host ""

# Restart is needed for wallet to reload actions
Write-Host "⚠️  NOTE: Restart the Rust wallet to reload actions.json" -ForegroundColor Yellow
Write-Host "Press any key after restarting the wallet..."
$null = $Host.UI.RawUI.ReadKey("NoEcho,IncludeKeyDown")
Write-Host ""

# Test 3: List actions again (should show 1 action)
Write-Host "Test 3: List all actions (should show 1)" -ForegroundColor Yellow
$response = Invoke-RestMethod -Uri "$baseUrl/listActions" -Method POST -ContentType "application/json" -Body '{}'
Write-Host "Total Actions: $($response.totalActions)" -ForegroundColor Green
Write-Host "Actions:" -ForegroundColor Green
$response.actions | ForEach-Object {
    Write-Host "  - TXID: $($_.txid)" -ForegroundColor Cyan
    Write-Host "    Reference: $($_.referenceNumber)" -ForegroundColor Cyan
    Write-Host "    Status: $($_.status)" -ForegroundColor Cyan
    Write-Host "    Description: $($_.description)" -ForegroundColor Cyan
    Write-Host "    Labels: $($_.labels -join ', ')" -ForegroundColor Cyan
}
Write-Host ""

# Test 4: Abort action
Write-Host "Test 4: Abort the test action" -ForegroundColor Yellow
$abortRequest = @{
    referenceNumber = "test_ref_123"
} | ConvertTo-Json

try {
    $response = Invoke-RestMethod -Uri "$baseUrl/abortAction" -Method POST -ContentType "application/json" -Body $abortRequest
    Write-Host "✅ Aborted: $($response.aborted)" -ForegroundColor Green
} catch {
    Write-Host "❌ Error: $($_.Exception.Message)" -ForegroundColor Red
}
Write-Host ""

# Test 5: List actions again (status should be 'aborted')
Write-Host "Test 5: List actions (status should be aborted)" -ForegroundColor Yellow
$response = Invoke-RestMethod -Uri "$baseUrl/listActions" -Method POST -ContentType "application/json" -Body '{}'
$response.actions | ForEach-Object {
    Write-Host "  - TXID: $($_.txid)" -ForegroundColor Cyan
    Write-Host "    Status: $($_.status)" -ForegroundColor $(if ($_.status -eq "aborted") { "Green" } else { "Yellow" })
}
Write-Host ""

# Test 6: Try to abort again (should say already aborted)
Write-Host "Test 6: Try to abort again (should say already aborted)" -ForegroundColor Yellow
try {
    $response = Invoke-RestMethod -Uri "$baseUrl/abortAction" -Method POST -ContentType "application/json" -Body $abortRequest
    Write-Host "✅ Response: $($response.aborted)" -ForegroundColor Green
} catch {
    Write-Host "❌ Error: $($_.Exception.Message)" -ForegroundColor Red
}
Write-Host ""

# Test 7: Test label filtering
Write-Host "Test 7: Filter by label 'shopping'" -ForegroundColor Yellow
$filterRequest = @{
    labels = @("shopping")
    labelQueryMode = "any"
} | ConvertTo-Json

$response = Invoke-RestMethod -Uri "$baseUrl/listActions" -Method POST -ContentType "application/json" -Body $filterRequest
Write-Host "Total Actions with 'shopping' label: $($response.totalActions)" -ForegroundColor Green
Write-Host ""

# Test 8: Try to abort non-existent action
Write-Host "Test 8: Try to abort non-existent action" -ForegroundColor Yellow
$badRequest = @{
    referenceNumber = "nonexistent_ref"
} | ConvertTo-Json

try {
    $response = Invoke-RestMethod -Uri "$baseUrl/abortAction" -Method POST -ContentType "application/json" -Body $badRequest
    Write-Host "Unexpected success!" -ForegroundColor Red
} catch {
    $errorDetails = $_.ErrorDetails.Message | ConvertFrom-Json
    Write-Host "✅ Expected error: $($errorDetails.code)" -ForegroundColor Green
    Write-Host "   Description: $($errorDetails.description)" -ForegroundColor Cyan
}
Write-Host ""

Write-Host "========================================" -ForegroundColor Cyan
Write-Host "All tests completed!" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
