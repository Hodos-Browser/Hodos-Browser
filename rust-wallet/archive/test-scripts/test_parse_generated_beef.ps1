# Test parsing the generated BEEF to verify it's valid

$beefHex = "0100beef01c4050e000e0e9b18d77b48fde9b46d54b75d372e30a74cba0114cad4796f8f1d91946866a8bd45b8d1a256e4de964d2a70408e3ae4265b43544425ea40f370cd76d367575b0e68eb18e8550c7893a27be29285a6777f7f57e89bc4321960920a6506231161b60b374893c8085e0a6afd697ffab4abca776967e3977aeb33615bc28c486e55b569c47a5d59182a27b4927d159f7484ae3a1ada7596ee344abf09f188e7d9e1489810c9ad7e1735ca6ed9d2094d8b091f51ce700eba5a4b58aa49dc609a69f70b3db0a534a5c2f2cc26dc7c03885ef005bb457fdad307c5c4cf72d5e685260f5fd7391615418c93ae8f8366a3a07cef495ede97b18fde63b3bdc9231148cdd257945e7a40075b0bb1d66df21c517491fe94b97c57a6be2908e7261233a0f5cda7843bf06849cccf1f6bbbac17a5aac67797856209481486da815557cc5708e7c311f94a0dcd61a9fbf654c35172a50e29a3fb3deca3577dbbb820e00f646810cde9b32676730aab2cab2e48a45c837028c45cd2ede69ac5c1e9633ffa6ec544c42ff673079b6730aa80457406cea6d136f1916a4d9872e31979c38db66f6cc46dd2016eddc7ed725b45f77aa63bd499243a2edc6cb03f9fc45994721b24104bb002010100000001f9301a6dc915b265aac9ced38cb3608080d0152964f9b2b3fc8c0d34dbde3f8b000000006b48304502210083412d325d26dace28a45df2089963676f8fdcaa1eb6d88498b2b2391161b841022002158963468441718507c1c300e713b1720a68837ce81d90921fbac7f68f1a6f41210243976a3db429791794a6178802b4b58817dbb028e83f7ea82db84c693b0bc18affffffff02e8030000000000001976a91438d0f9c3db1f1f94fa6e6dce0f46da2f80b8f0b588ac23b0b000000000001976a914a249afcdb243ba81c0dd58ef33f6a4b61b49b7a988ac000000000100010000000133461280ba4976e2598c0cd1d30b1ae3739716acea74964e02d677241f60ce7d010000006b48304502210096203e2545d1031db62744a0c9ff2f73e2975ba9bfb373cc6dcfb76f0187b3ce02201cdfc3a4d516e8dfb78a7ec235bba8b076e2fc0330db5e8c7cb07c9741916f764121030fe8c7d8462cd9476b3e4ae26cf54fdf4cecbb83f088143a1c2f924b6a33e369ffffffff0272000000000000001976a9142c3212f361acd04a8273904ed29242536f2f139f88ac299cb000000000001976a914a249afcdb243ba81c0dd58ef33f6a4b61b49b7a988ac0000000000"

Write-Host "Testing BEEF parsing..." -ForegroundColor Cyan
Write-Host "Length: $($beefHex.Length / 2) bytes"
Write-Host ""

# Write to temp file and parse with Rust
[System.IO.File]::WriteAllText("$PSScriptRoot\temp_beef.hex", $beefHex)

Write-Host "Creating Rust test..." -ForegroundColor Yellow

$testCode = @"
use std::fs;

fn main() {
    let hex = fs::read_to_string("temp_beef.hex").unwrap();
    let bytes = hex::decode(hex.trim()).unwrap();

    println!("BEEF bytes: {} bytes", bytes.len());
    println!("First 20 bytes: {:02x?}", &bytes[..20]);
    println!("");

    // Try to parse
    match bitcoin_browser_wallet::beef::Beef::from_bytes(&bytes) {
        Ok(beef) => {
            println!("✅ BEEF parsed successfully!");
            println!("   Transactions: {}", beef.transactions.len());
            println!("   BUMPs: {}", beef.bumps.len());

            for (i, tx) in beef.transactions.iter().enumerate() {
                println!("   TX {}: {} bytes", i, tx.len());
            }
        }
        Err(e) => {
            println!("❌ BEEF parsing FAILED: {}", e);
        }
    }
}
"@

Write-Host $testCode
Write-Host ""
Write-Host "Run this manually to test BEEF parsing" -ForegroundColor Green
