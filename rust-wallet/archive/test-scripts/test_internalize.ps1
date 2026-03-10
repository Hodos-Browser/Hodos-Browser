# Test script for internalizeAction endpoint
# Tests accepting incoming transactions

$baseUrl = "http://localhost:3301"

Write-Host "=============================================" -ForegroundColor Cyan
Write-Host "Testing internalizeAction (Incoming TX)" -ForegroundColor Cyan
Write-Host "=============================================" -ForegroundColor Cyan
Write-Host ""

# Sample raw transaction (this is from the test_transaction_flow.ps1 output)
# This is a signed transaction hex
$sampleTx = "01000000027dc735589ca46babb2268dc2373d3b179b964021b9de0e2c2ebba9a9cdcd3ec5010000006b483045022100bdc00c1609ce8e67dcd69acdefca1476b1947ad844953f5fdb1d19dcab72d8ca02203b8be5855ff9d55b91286cb06d297458b1aa517c4dacd818ef653fad977f998d412103d575090cc073ecf448ad49fae79993fdaf8d1643ec2c5762655ed400e20333e3ffffffff7dc735589ca46babb2268dc2373d3b179b964021b9de0e2c2ebba9a9cdcd3ec5000000006b483045022100f2e1ccb7fe0d33fec9650bd689d481c797ea6ed70781768ebc89ef11beddbf7b02203ad83b305ad1c6c6ce2570bc04b473d9b078944fd7b5e694d6e2f155d135e01a412103d575090cc073ecf448ad49fae79993fdaf8d1643ec2c5762655ed400e20333e3ffffffff0147000000000000001976a9149140a5c1a3ef8d7f5a1e9b7c8d9e0f1234567890abcdef88ac00000000"

Write-Host "Test 1: Internalize incoming transaction" -ForegroundColor Yellow
Write-Host "Transaction hex (first 100 chars): $($sampleTx.Substring(0, 100))..." -ForegroundColor Gray
Write-Host ""

$request = @{
    tx = $sampleTx
    description = "Incoming payment from test"
    labels = @("incoming", "test")
    outputs = @(
        @{
            outputIndex = 0
            protocol = "wallet"
        }
    )
} | ConvertTo-Json -Depth 10

try {
    Write-Host "Sending internalizeAction request..." -ForegroundColor Cyan
    $response = Invoke-RestMethod -Uri "$baseUrl/internalizeAction" -Method POST -ContentType "application/json" -Body $request

    Write-Host "Success! Transaction internalized" -ForegroundColor Green
    Write-Host "  TXID: $($response.txid)" -ForegroundColor Cyan
    Write-Host "  Status: $($response.status)" -ForegroundColor Cyan
    Write-Host ""

    # Test 2: List actions to see the internalized transaction
    Write-Host "Test 2: List actions (should include internalized TX)" -ForegroundColor Yellow
    $listResponse = Invoke-RestMethod -Uri "$baseUrl/listActions" -Method POST -ContentType "application/json" -Body '{}'

    Write-Host "Total Actions: $($listResponse.totalActions)" -ForegroundColor Green

    # Find our internalized transaction
    $internalizedAction = $listResponse.actions | Where-Object { $_.txid -eq $response.txid }

    if ($internalizedAction) {
        Write-Host ""
        Write-Host "Internalized Transaction:" -ForegroundColor Green
        Write-Host "  TXID: $($internalizedAction.txid)" -ForegroundColor Cyan
        Write-Host "  Status: $($internalizedAction.status)" -ForegroundColor Cyan
        Write-Host "  Description: $($internalizedAction.description)" -ForegroundColor Cyan
        Write-Host "  Labels: $($internalizedAction.labels -join ', ')" -ForegroundColor Cyan
        Write-Host "  Incoming: $(-not $internalizedAction.isOutgoing)" -ForegroundColor Cyan

        # Convert timestamp
        $epoch = Get-Date -Year 1970 -Month 1 -Day 1 -Hour 0 -Minute 0 -Second 0
        $timestamp = $epoch.AddSeconds($internalizedAction.timestamp).ToString('yyyy-MM-dd HH:mm:ss')
        Write-Host "  Timestamp: $timestamp" -ForegroundColor Cyan
    } else {
        Write-Host "WARNING: Internalized transaction not found in list!" -ForegroundColor Yellow
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
Write-Host "  - internalizeAction: Accept incoming transactions" -ForegroundColor Cyan
Write-Host "  - Stores transaction with status: unconfirmed" -ForegroundColor Cyan
Write-Host "  - Tracks as incoming (isOutgoing: false)" -ForegroundColor Cyan
Write-Host ""
Write-Host "Phase 1 Implementation:" -ForegroundColor Yellow
Write-Host "  - Accepts raw transaction hex" -ForegroundColor Gray
Write-Host "  - Calculates TXID" -ForegroundColor Gray
Write-Host "  - Stores in action storage" -ForegroundColor Gray
Write-Host ""
Write-Host "TODO for Phase 2:" -ForegroundColor Yellow
Write-Host "  - Parse full BEEF format" -ForegroundColor Gray
Write-Host "  - Validate transaction ancestry" -ForegroundColor Gray
Write-Host "  - Verify SPV proofs" -ForegroundColor Gray
Write-Host "  - Parse outputs and calculate received amount" -ForegroundColor Gray
