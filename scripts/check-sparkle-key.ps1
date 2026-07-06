# check-sparkle-key.ps1 - does SPARKLE_EDDSA_PRIVATE_KEY derive the embedded SUPublicEDKey?
#
# PowerShell-native version of the #5 key check (works in Windows PowerShell 5.1 and pwsh 7).
# Run:  powershell -File scripts\check-sparkle-key.ps1    (or  pwsh -File ...)
# Paste the SPARKLE_EDDSA_PRIVATE_KEY *value* when prompted (input is hidden). It never prints
# the private key. You need the value from your password manager or Sparkle's mac keychain -
# GitHub will not show a secret's value. (Or, once these commits are pushed, use the Actions
# tab -> "Check signing key" button, which uses the real secret with no local handling.)

$ErrorActionPreference = 'Stop'
$EXPECTED = 'GVq3mpDl8eelsG0A5wqC5FBYZd3fy7U3we9iZ9+Tq3Q='

# Find openssl (Git for Windows ships it if it is not already on PATH).
$ossl = (Get-Command openssl -ErrorAction SilentlyContinue).Source
if (-not $ossl) {
    foreach ($c in @("$env:ProgramFiles\Git\usr\bin\openssl.exe",
                     "$env:ProgramFiles\Git\mingw64\bin\openssl.exe",
                     "${env:ProgramFiles(x86)}\Git\usr\bin\openssl.exe")) {
        if (Test-Path $c) { $ossl = $c; break }
    }
}
if (-not $ossl) { Write-Error "openssl not found - install Git for Windows, or put openssl on PATH."; return }

$sec = Read-Host -AsSecureString "Paste SPARKLE_EDDSA_PRIVATE_KEY (hidden), then press Enter"
$bstr = [Runtime.InteropServices.Marshal]::SecureStringToBSTR($sec)
try { $plain = [Runtime.InteropServices.Marshal]::PtrToStringBSTR($bstr) }
finally { [Runtime.InteropServices.Marshal]::ZeroFreeBSTR($bstr) }

try { $bytes = [Convert]::FromBase64String($plain.Trim()) }
catch { Write-Error "That value is not valid base64 - paste the raw secret value exactly."; return }
Write-Host ("decoded key: {0} bytes (Sparkle keys are usually 64 = seed+pubkey)" -f $bytes.Length)
if ($bytes.Length -lt 32) { Write-Error "key is only $($bytes.Length) bytes - need at least 32."; return }

# First 32 bytes = the Ed25519 seed (same extraction the release pipeline's openssl path uses).
$seed = New-Object byte[] 32
[Array]::Copy($bytes, 0, $seed, 0, 32)

# PKCS#8 Ed25519 private-key DER = fixed 16-byte prefix + the 32-byte seed.
$prefix = [byte[]](0x30,0x2e,0x02,0x01,0x00,0x30,0x05,0x06,0x03,0x2b,0x65,0x70,0x04,0x22,0x04,0x20)
$der = New-Object byte[] 48
[Array]::Copy($prefix, 0, $der, 0, 16)
[Array]::Copy($seed,   0, $der, 16, 32)
$pem = "-----BEGIN PRIVATE KEY-----`n" + [Convert]::ToBase64String($der) + "`n-----END PRIVATE KEY-----`n"

$pemPath = Join-Path $env:TEMP 'sparkle_check_ed.pem'
$pubPath = Join-Path $env:TEMP 'sparkle_check_pub.der'
Set-Content -Path $pemPath -Value $pem -Encoding ascii -NoNewline

& $ossl pkey -in $pemPath -pubout -outform DER -out $pubPath 2>$null
$rc = $LASTEXITCODE
if ($rc -ne 0 -or -not (Test-Path $pubPath)) {
    Remove-Item $pemPath -Force -ErrorAction SilentlyContinue
    Write-Error "openssl could not read the derived key - the first 32 bytes are not a valid Ed25519 seed (unusual layout). Send me the 'decoded key: N bytes' line."
    return
}
$pubder = [IO.File]::ReadAllBytes($pubPath)
$rawpub = New-Object byte[] 32
[Array]::Copy($pubder, $pubder.Length - 32, $rawpub, 0, 32)
$derived = [Convert]::ToBase64String($rawpub)
Remove-Item $pemPath, $pubPath -Force -ErrorAction SilentlyContinue

Write-Host ""
Write-Host "derived : $derived"
Write-Host "embedded: $EXPECTED"
if ($derived -eq $EXPECTED) {
    Write-Host "`nKEY OK - the openssl path reads it; NO conversion needed. Silent releases will sign the sidecars." -ForegroundColor Green
} else {
    Write-Host "`nNO MATCH - send me this output; I will adjust the pipeline's seed extraction (no need to touch the shared secret)." -ForegroundColor Yellow
}
