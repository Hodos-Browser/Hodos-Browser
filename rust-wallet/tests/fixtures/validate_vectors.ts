/**
 * TypeScript Vector Validator
 * 
 * This script validates that ts_sdk_vectors.json contains correct test data
 * by running each vector against the actual BSV TypeScript SDK.
 * 
 * If this passes and Rust tests fail → bug in Rust code
 * If this fails → bug in the vector data (typo when copying)
 * 
 * Usage:
 *   cd rust-wallet/tests/fixtures
 *   npm install @bsv/sdk
 *   npx ts-node validate_vectors.ts
 * 
 * Or with Bun:
 *   bun validate_vectors.ts
 */

import * as fs from 'fs';

// Type definitions for our vectors
interface BRC42PrivateKeyVector {
  sender_pubkey: string;
  recipient_privkey: string;
  invoice: string;
  expected_derived_key: string;
}

interface BRC42PublicKeyVector {
  sender_privkey: string;
  recipient_pubkey: string;
  invoice: string;
  expected_derived_pubkey: string;
}

interface HMACVector {
  key_hex: string;
  message_utf8?: string;
  message_hex?: string;
  expected_hmac: string;
}

interface Vectors {
  _last_validated: string | null;
  brc42_private_key_derivation: BRC42PrivateKeyVector[];
  brc42_public_key_derivation: BRC42PublicKeyVector[];
  hmac_sha256: HMACVector[];
  brc3_signature_compliance: any;
  brc2_hmac_compliance: any;
}

async function main() {
  console.log('═══════════════════════════════════════════════════════════');
  console.log('  BSV TypeScript SDK Vector Validator');
  console.log('═══════════════════════════════════════════════════════════\n');

  // Load vectors
  const vectorsPath = './ts_sdk_vectors.json';
  if (!fs.existsSync(vectorsPath)) {
    console.error('❌ ts_sdk_vectors.json not found');
    process.exit(1);
  }

  const vectors: Vectors = JSON.parse(fs.readFileSync(vectorsPath, 'utf-8'));
  let passed = 0;
  let failed = 0;

  // Try to import BSV SDK
  let SDK: any;
  try {
    SDK = await import('@bsv/sdk');
    console.log('✓ @bsv/sdk loaded successfully\n');
  } catch (e) {
    console.error('❌ Failed to import @bsv/sdk');
    console.error('   Run: npm install @bsv/sdk');
    console.error('   Then retry: npx ts-node validate_vectors.ts\n');
    process.exit(1);
  }

  // ─── BRC-42 Private Key Derivation ───
  console.log('▶ BRC-42 Private Key Derivation');
  for (let i = 0; i < vectors.brc42_private_key_derivation.length; i++) {
    const v = vectors.brc42_private_key_derivation[i];
    try {
      // Using SDK's BRC-42 implementation
      const senderPubKey = SDK.PublicKey.fromString(v.sender_pubkey);
      const recipientPrivKey = SDK.PrivateKey.fromString(v.recipient_privkey, 'hex');
      
      // Derive child private key
      const derived = recipientPrivKey.deriveChild(senderPubKey, v.invoice);
      const derivedHex = derived.toString();

      if (derivedHex === v.expected_derived_key) {
        console.log(`  ✓ Vector ${i + 1}: PASS`);
        passed++;
      } else {
        console.log(`  ✗ Vector ${i + 1}: FAIL`);
        console.log(`    Expected: ${v.expected_derived_key}`);
        console.log(`    Got:      ${derivedHex}`);
        failed++;
      }
    } catch (e: any) {
      console.log(`  ✗ Vector ${i + 1}: ERROR - ${e.message}`);
      failed++;
    }
  }

  // ─── BRC-42 Public Key Derivation ───
  console.log('\n▶ BRC-42 Public Key Derivation');
  for (let i = 0; i < vectors.brc42_public_key_derivation.length; i++) {
    const v = vectors.brc42_public_key_derivation[i];
    try {
      const senderPrivKey = SDK.PrivateKey.fromString(v.sender_privkey, 'hex');
      const recipientPubKey = SDK.PublicKey.fromString(v.recipient_pubkey);
      
      // Derive child public key
      const derived = recipientPubKey.deriveChild(senderPrivKey, v.invoice);
      const derivedHex = derived.toString();

      if (derivedHex === v.expected_derived_pubkey) {
        console.log(`  ✓ Vector ${i + 1}: PASS`);
        passed++;
      } else {
        console.log(`  ✗ Vector ${i + 1}: FAIL`);
        console.log(`    Expected: ${v.expected_derived_pubkey}`);
        console.log(`    Got:      ${derivedHex}`);
        failed++;
      }
    } catch (e: any) {
      console.log(`  ✗ Vector ${i + 1}: ERROR - ${e.message}`);
      failed++;
    }
  }

  // ─── HMAC-SHA256 ───
  console.log('\n▶ HMAC-SHA256');
  for (let i = 0; i < vectors.hmac_sha256.length; i++) {
    const v = vectors.hmac_sha256[i];
    try {
      const keyBytes = Buffer.from(v.key_hex, 'hex');
      let messageBytes: Buffer;
      
      if (v.message_utf8) {
        messageBytes = Buffer.from(v.message_utf8, 'utf-8');
      } else if (v.message_hex) {
        messageBytes = Buffer.from(v.message_hex, 'hex');
      } else {
        throw new Error('No message provided');
      }

      const hmac = SDK.Hash.sha256hmac(keyBytes, messageBytes);
      const hmacHex = Buffer.from(hmac).toString('hex');

      if (hmacHex === v.expected_hmac) {
        console.log(`  ✓ Vector ${i + 1}: PASS`);
        passed++;
      } else {
        console.log(`  ✗ Vector ${i + 1}: FAIL`);
        console.log(`    Expected: ${v.expected_hmac}`);
        console.log(`    Got:      ${hmacHex}`);
        failed++;
      }
    } catch (e: any) {
      console.log(`  ✗ Vector ${i + 1}: ERROR - ${e.message}`);
      failed++;
    }
  }

  // ─── Summary ───
  console.log('\n═══════════════════════════════════════════════════════════');
  console.log(`  RESULTS: ${passed} passed, ${failed} failed`);
  console.log('═══════════════════════════════════════════════════════════\n');

  if (failed > 0) {
    console.log('⚠️  Some vectors failed. Check the data in ts_sdk_vectors.json');
    console.log('   against the original ts-sdk test files.');
    process.exit(1);
  } else {
    console.log('✅ All vectors validated against @bsv/sdk');
    console.log('   Safe to run Rust tests — any failures are in Rust code.');
    
    // Update last_validated timestamp
    vectors._last_validated = new Date().toISOString();
    fs.writeFileSync(vectorsPath, JSON.stringify(vectors, null, 2));
    console.log(`   Updated _last_validated in ${vectorsPath}`);
  }
}

main().catch(console.error);
