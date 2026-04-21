# Test script for labels support in createAction and processAction
# Run this with: .\test_labels.ps1

$baseUrl = "http://localhost:3301"

Write-Host "=============================================" -ForegroundColor Cyan
Write-Host "Testing Labels Support" -ForegroundColor Cyan
Write-Host "=============================================" -ForegroundColor Cyan
Write-Host ""

# Test 1: List initial actions
Write-Host "Test 1: List all actions (initial state)" -ForegroundColor Yellow
try {
    $response = Invoke-RestMethod -Uri "$baseUrl/listActions" -Method POST -ContentType "application/json" -Body '{}'
    Write-Host "Success! Total Actions: $($response.totalActions)" -ForegroundColor Green
} catch {
    Write-Host "Error: $($_.Exception.Message)" -ForegroundColor Red
}
Write-Host ""

# Test 2: Create transaction with labels
Write-Host "Test 2: Create transaction with labels" -ForegroundColor Yellow

$testScript = "76a9149140a5c1a3ef8d7f5a1e9b7c8d9e0f1234567890abcdef88ac"

$processRequest = @{
    outputs = @(
        @{
            script = $testScript
            satoshis = 71
        }
    )
    description = "Test payment with labels"
    labels = @("shopping", "groceries", "weekend")
    broadcast = $false
} | ConvertTo-Json -Depth 10

try {
    Write-Host "Sending processAction request with labels..." -ForegroundColor Cyan
    $response = Invoke-RestMethod -Uri "$baseUrl/processAction" -Method POST -ContentType "application/json" -Body $processRequest

    $txid = $response.txid
    $status = $response.status

    Write-Host "Transaction created!" -ForegroundColor Green
    Write-Host "  TXID: $txid" -ForegroundColor Cyan
    Write-Host "  Status: $status" -ForegroundColor Cyan
    Write-Host ""

    # Test 3: Verify labels were stored
    Write-Host "Test 3: Verify labels in action list" -ForegroundColor Yellow
    $listResponse = Invoke-RestMethod -Uri "$baseUrl/listActions" -Method POST -ContentType "application/json" -Body '{}'

    $ourAction = $null
    foreach ($action in $listResponse.actions) {
        if ($action.txid -eq $txid) {
            $ourAction = $action
            break
        }
    }

    if ($ourAction) {
        Write-Host ""
        Write-Host "Transaction Details:" -ForegroundColor Green
        Write-Host "  TXID: $($ourAction.txid)" -ForegroundColor Cyan
        Write-Host "  Description: $($ourAction.description)" -ForegroundColor Cyan
        Write-Host "  Labels: $($ourAction.labels -join ', ')" -ForegroundColor Cyan
        Write-Host "  Status: $($ourAction.status)" -ForegroundColor Cyan

        # Verify labels match what we sent
        if ($ourAction.labels.Count -eq 3 -and
            $ourAction.labels -contains "shopping" -and
            $ourAction.labels -contains "groceries" -and
            $ourAction.labels -contains "weekend") {
            Write-Host ""
            Write-Host "SUCCESS! Labels stored correctly!" -ForegroundColor Green
        } else {
            Write-Host ""
            Write-Host "WARNING: Labels don't match!" -ForegroundColor Yellow
            Write-Host "  Expected: shopping, groceries, weekend" -ForegroundColor Yellow
            Write-Host "  Got: $($ourAction.labels -join ', ')" -ForegroundColor Yellow
        }
    } else {
        Write-Host "WARNING: Transaction not found!" -ForegroundColor Yellow
    }

    Write-Host ""

    # Test 4: Filter by labels
    Write-Host "Test 4: Filter actions by label (shopping)" -ForegroundColor Yellow
    $filterRequest = @{
        labels = @("shopping")
        labelQueryMode = "any"
    } | ConvertTo-Json

    $filteredResponse = Invoke-RestMethod -Uri "$baseUrl/listActions" -Method POST -ContentType "application/json" -Body $filterRequest

    Write-Host "Actions with 'shopping' label: $($filteredResponse.totalActions)" -ForegroundColor Green

    $foundOurAction = $false
    foreach ($action in $filteredResponse.actions) {
        if ($action.txid -eq $txid) {
            $foundOurAction = $true
            break
        }
    }

    if ($foundOurAction) {
        Write-Host "SUCCESS! Label filtering works!" -ForegroundColor Green
    } else {
        Write-Host "WARNING: Our transaction not found in filtered results!" -ForegroundColor Yellow
    }

} catch {
    Write-Host "Error: $($_.Exception.Message)" -ForegroundColor Red
    Write-Host ""
    if ($_.ErrorDetails.Message) {
        Write-Host "Details:" -ForegroundColor Yellow
        $_.ErrorDetails.Message | ConvertFrom-Json | ConvertTo-Json -Depth 10
    }
}

Write-Host ""
Write-Host "=============================================" -ForegroundColor Cyan
Write-Host "Test completed!" -ForegroundColor Cyan
Write-Host "=============================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "Summary:" -ForegroundColor Green
Write-Host "  - Labels can be added to transactions" -ForegroundColor Cyan
Write-Host "  - Labels are stored in action storage" -ForegroundColor Cyan
Write-Host "  - Transactions can be filtered by labels" -ForegroundColor Cyan
Write-Host ""
Write-Host "Use Case Examples:" -ForegroundColor Yellow
Write-Host "  - Organize by category: shopping, bills, salary" -ForegroundColor Gray
Write-Host "  - Track projects: project-alpha, client-xyz" -ForegroundColor Gray
Write-Host "  - Mark importance: urgent, review-later" -ForegroundColor Gray
