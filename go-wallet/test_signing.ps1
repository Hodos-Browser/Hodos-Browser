$body = @{
    rawTx = "010000000188e68977fab8038af07746e5d687652a44aa15f532509c202749dbad8a4187330100000000ffffffff02e8030000000000001976a91477bff20c60e522dfaa3350c39b030a5d004e839a88acdf390f00000000001976a91462e907b15cbf27d5425399ebf6f0fb50ebb88f1888ac00000000"
    privateKey = "L5EZftvrYaSu6VW4W4M3Z2dN6tjdWiqsGGBqj7CbFJ5JN2xM2nM3"
} | ConvertTo-Json

Write-Host "Testing transaction signing..."
Invoke-RestMethod -Uri "http://localhost:8080/transaction/sign" -Method POST -ContentType "application/json" -Body $body
