# PowerShell script to call acquireCertificate with localhost:3001 test server

# First, get the certifier's public key from the test server
Write-Host "Getting certifier public key from test server..." -ForegroundColor Cyan
try {
    $certifierInfo = Invoke-RestMethod -Uri "http://localhost:3001/certifierPublicKey" -Method GET -UseBasicParsing
    $certifierPublicKey = $certifierInfo.certifier
    Write-Host "✅ Certifier public key: $certifierPublicKey" -ForegroundColor Green
} catch {
    Write-Host "⚠️  Could not get certifier public key from server. Using default..." -ForegroundColor Yellow
    Write-Host "   Make sure the TypeScript SDK server is running on port 3001" -ForegroundColor Yellow
    Write-Host "   Error: $($_.Exception.Message)" -ForegroundColor Red
    # Fallback: use the known test server key
    $certifierPublicKey = "03d902f35f560e0470c63313c7369168d9d7df2d49bf295fd9fb7cb109ccee0494"
}

Write-Host ""

$body = @{
    acquisitionProtocol = 2
    certifierUrl = "http://localhost:3001"
    certifier = $certifierPublicKey
    type = "AGfk/WrT1eBDXpz3mcw386Zww2HmqcIn3uY6x4Af1eo="
    fields = @{
        cool = "true"
    }
} | ConvertTo-Json

Write-Host "Calling /acquireCertificate with test server..." -ForegroundColor Cyan
Write-Host "Body: $body" -ForegroundColor Gray
Write-Host ""

try {
    $response = Invoke-RestMethod -Uri "http://localhost:3301/acquireCertificate" `
        -Method POST `
        -ContentType "application/json" `
        -Body $body

    Write-Host "✅ Success!" -ForegroundColor Green
    Write-Host ($response | ConvertTo-Json -Depth 10)
} catch {
    Write-Host "❌ Error:" -ForegroundColor Red
    Write-Host $_.Exception.Message
    if ($_.ErrorDetails) {
        Write-Host $_.ErrorDetails.Message
    }
}
