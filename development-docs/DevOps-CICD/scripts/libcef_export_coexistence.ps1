# libcef_export_coexistence.ps1 — the symbol-coexistence re-measure from
# TICKET_dependency_freshness_review.md, made repeatable. Run after every engine bump.
#
# WHAT IT ANSWERS. Our vcpkg OpenSSL / sqlite3 are statically linked into HodosBrowser.dll and the
# Rust binaries; Chromium's BoringSSL / SQLite are statically linked INSIDE libcef.dll. They live in
# one process, so the question is not version-matching but whether libcef.dll EXPORTS any crypto /
# sqlite symbol a later loader could bind to instead of ours. This prints the numbers rather than
# letting "no overlap" stay an assumption. 2026-08-17 (P4f): 247 exports, 240 cef_*, 1 crypto/sqlite
# (sqlite3_dbdata_init).
#
# Usage:  pwsh development-docs/DevOps-CICD/scripts/libcef_export_coexistence.ps1 [-Dll <path>]
#         Default -Dll is the staged distribution's cef-binaries/Release/libcef.dll.
# Negative control: point -Dll at cef-native/build/bin/Release/HodosBrowser.dll — a DLL that does
# statically link OUR OpenSSL/SQLite and exports a different shape — and confirm the numbers change.
# A script whose output never moves is not measuring the file named.

param(
    [string]$Dll = (Join-Path (Split-Path -Parent (Split-Path -Parent (Split-Path -Parent $PSScriptRoot))) 'cef-binaries\Release\libcef.dll')
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

if (-not (Test-Path $Dll)) { Write-Error "not found: $Dll"; exit 2 }

# dumpbin ships with the MSVC toolset; find the newest one under either VS root.
$dumpbin = Get-ChildItem -Path @('C:\Program Files\Microsoft Visual Studio', 'C:\Program Files (x86)\Microsoft Visual Studio') `
    -Recurse -Filter dumpbin.exe -ErrorAction SilentlyContinue |
    Where-Object { $_.FullName -like '*\Hostx64\x64\dumpbin.exe' } |
    Sort-Object FullName -Descending | Select-Object -First 1
if (-not $dumpbin) { Write-Error 'dumpbin.exe not found under either Visual Studio root'; exit 2 }

$item = Get-Item $Dll
$md5  = (Get-FileHash -Algorithm MD5 $Dll).Hash.ToLower()
$raw  = & $dumpbin.FullName /EXPORTS $Dll 2>&1
if ($LASTEXITCODE -ne 0) { Write-Error "dumpbin exited $LASTEXITCODE"; exit 2 }

# Export table rows look like:  "        1    0 00012345 cef_add_cross_origin_whitelist_entry"
$names = @()
$inTable = $false
foreach ($line in $raw) {
    if ($line -match '^\s+ordinal\s+hint\s+RVA\s+name') { $inTable = $true; continue }
    if ($inTable -and $line -match '^\s+Summary') { break }
    if ($inTable -and $line -match '^\s+\d+\s+[0-9A-Fa-f]+\s+[0-9A-Fa-f]{8}\s+(\S+)') { $names += $Matches[1] }
}

$cef    = @($names | Where-Object { $_ -like 'cef_*' })
$crypto = @($names | Where-Object { $_ -match '^(sqlite3_|SSL_|CRYPTO_|EVP_|BIO_|OPENSSL_|BN_|EC_|ECDSA_|RSA_|X509_|HMAC_|SHA[0-9]*_|AES_|RAND_)' })

"file                   : $($item.FullName)"
"size / md5             : $($item.Length) / $md5"
"dumpbin                : $($dumpbin.FullName)"
"total exported symbols : $($names.Count)"
"cef_* exports          : $($cef.Count)"
"crypto/sqlite exports  : $($crypto.Count)" + $(if ($crypto.Count -gt 0) { "   -> " + ($crypto -join ', ') } else { '' })
if ($names.Count -eq 0) { Write-Error 'parsed zero exports — the dumpbin output shape changed; do not trust this run'; exit 2 }
