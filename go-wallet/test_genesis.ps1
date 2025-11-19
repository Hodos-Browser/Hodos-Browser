$body = @{
    recipientAddress = "1BvBMSEYstWetqTFn5Au4m4GFg7xJaNVN2"
    amount = 1000
    feeRate = 5
    senderAddress = "1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa"
} | ConvertTo-Json

Write-Host "Testing transaction creation with Genesis address as sender..."
Invoke-RestMethod -Uri "http://localhost:8080/transaction/create" -Method POST -ContentType "application/json" -Body $body
