Write-Host "Step 1: Creating transaction..."
$createBody = @{
    recipientAddress = "1LdE3n5523D4MTTsiJaDAJUnQRdPdbdTeB"
    amount = 1000
    feeRate = 1
} | ConvertTo-Json

try {
    $createResponse = Invoke-RestMethod -Uri "http://localhost:8080/transaction/create" -Method POST -Body $createBody -ContentType "application/json" -TimeoutSec 10
    Write-Host "Transaction created successfully:"
    Write-Host "TxID: $($createResponse.txid)"
    Write-Host "RawTx: $($createResponse.rawTx)"

    Write-Host "`nStep 2: Signing transaction..."
    $signBody = @{
        rawTx = $createResponse.rawTx
    } | ConvertTo-Json

    $signResponse = Invoke-RestMethod -Uri "http://localhost:8080/transaction/sign" -Method POST -Body $signBody -ContentType "application/json" -TimeoutSec 10
    Write-Host "Transaction signed successfully:"
    Write-Host "TxID: $($signResponse.txid)"
    Write-Host "Status: $($signResponse.status)"

    Write-Host "`nStep 3: Broadcasting transaction..."
    $broadcastBody = @{
        signedTx = $signResponse.rawTx
    } | ConvertTo-Json

    $broadcastResponse = Invoke-RestMethod -Uri "http://localhost:8080/transaction/broadcast" -Method POST -Body $broadcastBody -ContentType "application/json" -TimeoutSec 10
    Write-Host "Transaction broadcast successfully:"
    Write-Host "Success: $($broadcastResponse.success)"
    Write-Host "TxID: $($broadcastResponse.txid)"

} catch {
    Write-Host "Error: $($_.Exception.Message)"
    if ($_.Exception.Response) {
        $reader = New-Object System.IO.StreamReader($_.Exception.Response.GetResponseStream())
        $responseBody = $reader.ReadToEnd()
        Write-Host "Response body: $responseBody"
    }
}
