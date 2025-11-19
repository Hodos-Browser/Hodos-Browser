$body = @{
    rawTx = "01000000010bf7c613003fc1b11b06edb034d2ea95a084dcf21b777749aaf1729fcedbb8cf0000000000ffffffff02e8030000000000001976a914d744419f640c021972eb2adf7674c26dc4317bf388ac63700000000000001976a9143a298108136e79b2139d6a96cb03825cfa88a01d88ac00000000"
} | ConvertTo-Json

try {
    $response = Invoke-RestMethod -Uri "http://localhost:8080/transaction/sign" -Method POST -Body $body -ContentType "application/json" -TimeoutSec 10
    Write-Host "Success: $($response | ConvertTo-Json)"
} catch {
    Write-Host "Error: $($_.Exception.Message)"
}
