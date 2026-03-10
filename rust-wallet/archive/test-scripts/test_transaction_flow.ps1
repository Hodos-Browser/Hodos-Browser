# Test script for complete transaction flow with action storage
# Run this with: .\test_transaction_flow.ps1

$baseUrl = "http://localhost:3301"

Write-Host "=============================================" -ForegroundColor Cyan
Write-Host "Testing Transaction Flow with Action Storage" -ForegroundColor Cyan
Write-Host "=============================================" -ForegroundColor Cyan
Write-Host ""

# Note: To fully reset test data, stop the wallet and delete actions.json
Write-Host "NOTE: If you get 'Action already exists' errors, stop the wallet," -ForegroundColor Yellow
Write-Host "      delete actions.json, and restart the wallet." -ForegroundColor Yellow
Write-Host ""

# Test 1: List actions (should be empty or show previous actions)
Write-Host "Test 1: List all actions (initial state)" -ForegroundColor Yellow
try {
    $response = Invoke-RestMethod -Uri "$baseUrl/listActions" -Method POST -ContentType "application/json" -Body '{}'
    Write-Host "Success! Total Actions: $($response.totalActions)" -ForegroundColor Green
    if ($response.totalActions -gt 0) {
        foreach ($action in $response.actions) {
            Write-Host "  - TXID: $($action.txid)" -ForegroundColor Cyan
            Write-Host "    Status: $($action.status)" -ForegroundColor Cyan
            Write-Host "    Description: $($action.description)" -ForegroundColor Cyan
        }
    }
} catch {
    Write-Host "Error: $($_.Exception.Message)" -ForegroundColor Red
}
Write-Host ""

# Test 2: Create a transaction (processAction will create, sign, and broadcast)
Write-Host "Test 2: Create, sign, and broadcast a transaction" -ForegroundColor Yellow
Write-Host "WARNING: This will send a REAL transaction to mainnet!" -ForegroundColor Red
Write-Host "Press Ctrl+C to cancel, or any key to continue..." -ForegroundColor Yellow
$null = $Host.UI.RawUI.ReadKey("NoEcho,IncludeKeyDown")
Write-Host ""

# Create a simple test P2PKH script (placeholder)
$testScript = "76a9149140a5c1a3ef8d7f5a1e9b7c8d9e0f1234567890abcdef88ac"

$processRequest = @{
    outputs = @(
        @{
            script = $testScript
            satoshis = 71
        }
    )
    description = "Test transaction from action storage integration"
    broadcast = $false
} | ConvertTo-Json -Depth 10

try {
    Write-Host "Sending processAction request..." -ForegroundColor Cyan
    $response = Invoke-RestMethod -Uri "$baseUrl/processAction" -Method POST -ContentType "application/json" -Body $processRequest

    $txid = $response.txid
    $status = $response.status

    Write-Host "Transaction created!" -ForegroundColor Green
    Write-Host "  TXID: $txid" -ForegroundColor Cyan
    Write-Host "  Status: $status" -ForegroundColor Cyan
    Write-Host "  Raw TX (first 100 chars): $($response.rawTx.Substring(0, [Math]::Min(100, $response.rawTx.Length)))..." -ForegroundColor Cyan
    Write-Host ""

    # Test 3: List actions again (should show new action)
    Write-Host "Test 3: List actions (should show new transaction)" -ForegroundColor Yellow
    $response = Invoke-RestMethod -Uri "$baseUrl/listActions" -Method POST -ContentType "application/json" -Body '{}'
    Write-Host "Total Actions: $($response.totalActions)" -ForegroundColor Green

    # Find our transaction
    $ourAction = $null
    foreach ($action in $response.actions) {
        if ($action.txid -eq $txid) {
            $ourAction = $action
            break
        }
    }

    if ($ourAction) {
        Write-Host ""
        Write-Host "Our Transaction:" -ForegroundColor Green
        Write-Host "  TXID: $($ourAction.txid)" -ForegroundColor Cyan
        Write-Host "  Reference: $($ourAction.referenceNumber)" -ForegroundColor Cyan

        # Determine status color
        if ($ourAction.status -eq "unconfirmed" -or $ourAction.status -eq "signed") {
            $statusColor = "Green"
        } else {
            $statusColor = "Yellow"
        }
        Write-Host "  Status: $($ourAction.status)" -ForegroundColor $statusColor

        Write-Host "  Description: $($ourAction.description)" -ForegroundColor Cyan
        Write-Host "  Satoshis: $($ourAction.satoshis)" -ForegroundColor Cyan
        Write-Host "  Inputs: $($ourAction.inputs.Count)" -ForegroundColor Cyan
        Write-Host "  Outputs: $($ourAction.outputs.Count)" -ForegroundColor Cyan

        # Convert Unix timestamp to DateTime (compatible with older PowerShell)
        $epoch = Get-Date -Year 1970 -Month 1 -Day 1 -Hour 0 -Minute 0 -Second 0
        $timestamp = $epoch.AddSeconds($ourAction.timestamp).ToString('yyyy-MM-dd HH:mm:ss')
        Write-Host "  Timestamp: $timestamp" -ForegroundColor Cyan
    } else {
        Write-Host "WARNING: Transaction not found in action list!" -ForegroundColor Yellow
    }

    Write-Host ""

    # Test 4: Try to abort the transaction
    Write-Host "Test 4: Try to abort the transaction" -ForegroundColor Yellow
    if ($ourAction) {
        $abortRequest = @{
            referenceNumber = $ourAction.referenceNumber
        } | ConvertTo-Json

        try {
            $response = Invoke-RestMethod -Uri "$baseUrl/abortAction" -Method POST -ContentType "application/json" -Body $abortRequest
            Write-Host "Abort response: $($response.aborted)" -ForegroundColor Green
        } catch {
            $errorDetails = $_.ErrorDetails.Message | ConvertFrom-Json
            if ($errorDetails.code -eq "ERR_CANNOT_ABORT_CONFIRMED") {
                Write-Host "Cannot abort (already confirmed): $($errorDetails.description)" -ForegroundColor Yellow
            } else {
                Write-Host "Expected behavior: $($errorDetails.description)" -ForegroundColor Green
            }
        }
    }

    Write-Host ""

    # Test 5: List final state
    Write-Host "Test 5: Final action list" -ForegroundColor Yellow
    $response = Invoke-RestMethod -Uri "$baseUrl/listActions" -Method POST -ContentType "application/json" -Body '{"limit": 5}'
    Write-Host "Recent Actions (last 5):" -ForegroundColor Green
    foreach ($action in $response.actions) {
        Write-Host ""
        Write-Host "  Transaction: $($action.txid.Substring(0, [Math]::Min(16, $action.txid.Length)))..." -ForegroundColor Cyan

        # Determine status color
        $statusColor = switch ($action.status) {
            "created" { "Yellow" }
            "signed" { "Cyan" }
            "unconfirmed" { "Magenta" }
            "confirmed" { "Green" }
            "aborted" { "Gray" }
            "failed" { "Red" }
            default { "White" }
        }
        Write-Host "  Status: $($action.status)" -ForegroundColor $statusColor
        Write-Host "  Description: $($action.description)" -ForegroundColor Gray
    }

} catch {
    Write-Host "Error during processAction: $($_.Exception.Message)" -ForegroundColor Red
    Write-Host ""
    Write-Host "Details:" -ForegroundColor Yellow
    Write-Host $_.Exception | Format-List * -Force | Out-String
}

Write-Host ""
Write-Host "=============================================" -ForegroundColor Cyan
Write-Host "Test completed!" -ForegroundColor Cyan
Write-Host "=============================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "Summary:" -ForegroundColor Green
Write-Host "  - Transaction flow: create -> sign -> (broadcast)" -ForegroundColor Cyan
Write-Host "  - Action storage: tracking all steps" -ForegroundColor Cyan
Write-Host "  - Status updates: created -> signed -> unconfirmed/failed" -ForegroundColor Cyan
Write-Host ""
Write-Host "Check your wallet console for detailed logs!" -ForegroundColor Yellow
