# PushDrop Cross-Implementation Validation Test
#
# This script uses the TypeScript SDK to generate PushDrop scripts,
# then tests our Rust implementation by decoding them.
# This avoids circular validation by using the reference implementation.

Write-Host "🧪 PushDrop Cross-Implementation Validation Test" -ForegroundColor Cyan
Write-Host "================================================" -ForegroundColor Cyan
Write-Host ""

# Check if Node.js is available
$nodeVersion = node --version 2>$null
if (-not $nodeVersion) {
    Write-Host "❌ Node.js not found. Please install Node.js to run cross-validation tests." -ForegroundColor Red
    Write-Host "   These tests use the TypeScript SDK (@bsv/sdk) as the reference implementation." -ForegroundColor Yellow
    exit 1
}

Write-Host "✅ Node.js found: $nodeVersion" -ForegroundColor Green
Write-Host ""

# Create temporary test script
$testScript = @"
const { PushDrop, PrivateKey, Utils, PublicKey } = require('@bsv/sdk');

// Simple mock wallet for testing
class MockWallet {
    constructor(privateKey) {
        this.privateKey = privateKey;
        this.publicKey = PublicKey.fromPrivateKey(privateKey);
    }

    async getPublicKey({ protocolID, keyID, counterparty }) {
        // For testing, we'll use the same public key regardless of parameters
        return { publicKey: this.publicKey.toString() };
    }

    async createSignature({ data, protocolID, keyID, counterparty }) {
        // For testing, we don't need actual signatures
        // Just return a dummy signature
        return { signature: new Array(64).fill(0) };
    }
}

async function generateTestVectors() {
    const privateKey = PrivateKey.fromRandom();
    const wallet = new MockWallet(privateKey);
    const pushDrop = new PushDrop(wallet);

    const testCases = [
        {
            name: 'Empty fields',
            fields: [],
            protocolID: [0, 'tests'],
            keyID: 'test-key',
            counterparty: 'self'
        },
        {
            name: 'Single field - small',
            fields: [[1, 2, 3]],
            protocolID: [0, 'tests'],
            keyID: 'test-key',
            counterparty: 'self'
        },
        {
            name: 'Single field - text',
            fields: [Utils.toArray('hello world', 'utf8')],
            protocolID: [0, 'tests'],
            keyID: 'test-key',
            counterparty: 'self'
        },
        {
            name: 'Multiple fields',
            fields: [
                Utils.toArray('field1', 'utf8'),
                Utils.toArray('field2', 'utf8'),
                [0xde, 0xad, 0xbe, 0xef]
            ],
            protocolID: [0, 'tests'],
            keyID: 'test-key',
            counterparty: 'self'
        },
        {
            name: 'Special opcodes - OP_0',
            fields: [[0]],
            protocolID: [0, 'tests'],
            keyID: 'test-key',
            counterparty: 'self'
        },
        {
            name: 'Special opcodes - OP_1 through OP_16',
            fields: [[1], [2], [3], [16]],
            protocolID: [0, 'tests'],
            keyID: 'test-key',
            counterparty: 'self'
        },
        {
            name: 'Large field - OP_PUSHDATA1',
            fields: [new Array(200).fill(0x42)],
            protocolID: [0, 'tests'],
            keyID: 'test-key',
            counterparty: 'self'
        },
        {
            name: 'Large field - OP_PUSHDATA2',
            fields: [new Array(500).fill(0x42)],
            protocolID: [0, 'tests'],
            keyID: 'test-key',
            counterparty: 'self'
        }
    ];

    const results = [];

    for (const testCase of testCases) {
        try {
            // Lock without signature for cleaner test vectors
            const lockingScript = await pushDrop.lock(
                testCase.fields,
                testCase.protocolID,
                testCase.keyID,
                testCase.counterparty,
                false,  // forSelf
                false   // includeSignature = false (don't include signature in fields)
            );

            // Get public key
            const { publicKey } = await wallet.getPublicKey({
                protocolID: testCase.protocolID,
                keyID: testCase.keyID,
                counterparty: testCase.counterparty
            });

            // Decode to verify
            const decoded = PushDrop.decode(lockingScript);

            results.push({
                name: testCase.name,
                scriptHex: lockingScript.toHex(),
                publicKeyHex: publicKey,
                fields: testCase.fields.map(f => Array.from(f)),
                decodedFields: decoded.fields.map(f => Array.from(f)),
                decodedPublicKey: decoded.lockingPublicKey.toString()
            });
        } catch (error) {
            results.push({
                name: testCase.name,
                error: error.message
            });
        }
    }

    console.log(JSON.stringify(results, null, 2));
}

generateTestVectors().catch(console.error);
"@

# Write test script to temp file
$tempScript = [System.IO.Path]::GetTempFileName() + ".js"
$testScript | Out-File -FilePath $tempScript -Encoding UTF8

Write-Host "📝 Generating test vectors with TypeScript SDK..." -ForegroundColor Yellow
Write-Host ""

# Change to reference directory where @bsv/sdk is installed
$originalDir = Get-Location
Set-Location "$PSScriptRoot\..\reference\ts-brc100"

try {
    # Run Node.js script to generate test vectors
    $testVectorsJson = node $tempScript 2>&1

    if ($LASTEXITCODE -ne 0) {
        Write-Host "❌ Failed to generate test vectors:" -ForegroundColor Red
        Write-Host $testVectorsJson -ForegroundColor Red
        exit 1
    }

    # Parse JSON results
    $testVectors = $testVectorsJson | ConvertFrom-Json

    Write-Host "✅ Generated $($testVectors.Count) test vectors" -ForegroundColor Green
    Write-Host ""

    # Display test vectors
    Write-Host "📋 Test Vectors:" -ForegroundColor Cyan
    Write-Host "===============" -ForegroundColor Cyan
    Write-Host ""

    foreach ($vector in $testVectors) {
        if ($vector.error) {
            Write-Host "❌ $($vector.name): $($vector.error)" -ForegroundColor Red
        } else {
            Write-Host "✅ $($vector.name)" -ForegroundColor Green
            Write-Host "   Script (hex): $($vector.scriptHex)" -ForegroundColor Gray
            Write-Host "   Public Key: $($vector.publicKeyHex)" -ForegroundColor Gray
            Write-Host "   Fields: $($vector.fields.Count)" -ForegroundColor Gray
            Write-Host ""
        }
    }

    # Save test vectors to file for Rust tests
    $testVectorsFile = "$PSScriptRoot\pushdrop_test_vectors.json"
    $testVectorsJson | Out-File -FilePath $testVectorsFile -Encoding UTF8
    Write-Host "💾 Test vectors saved to: $testVectorsFile" -ForegroundColor Green
    Write-Host ""
    Write-Host "📝 Next step: Create Rust tests that decode these scripts" -ForegroundColor Yellow
    Write-Host "   and verify fields match the expected values." -ForegroundColor Yellow

} finally {
    Set-Location $originalDir
    Remove-Item $tempScript -ErrorAction SilentlyContinue
}

Write-Host ""
Write-Host "✅ Cross-validation test vector generation complete!" -ForegroundColor Green
