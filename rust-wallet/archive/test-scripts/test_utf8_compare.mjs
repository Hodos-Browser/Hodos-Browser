#!/usr/bin/env node
// Compare Utils.toUTF8 with our Rust js_to_utf8 behavior
// This tests whether the keyID derivation matches between Rust and JS SDK

import { createRequire } from 'module';
const require = createRequire(import.meta.url);
const sdkPath = '../reference/ts-brc100/node_modules/@bsv/sdk';
const { Utils, Random } = require(sdkPath);

// Reimplement our Rust js_to_utf8 in JavaScript to compare
function rustJsToUtf8(bytes) {
  let result = '';
  let skip = 0;

  for (let i = 0; i < bytes.length; i++) {
    const byte = bytes[i];

    if (skip > 0) {
      skip--;
      continue;
    }

    if (byte <= 0x7f) {
      result += String.fromCodePoint(byte);
    } else if (byte >= 0xc0 && byte <= 0xdf) {
      const byte2 = i + 1 < bytes.length ? bytes[i + 1] : 0;
      skip = 1;
      const codePoint = ((byte & 0x1f) << 6) | (byte2 & 0x3f);
      // Rust: char::from_u32(codePoint).unwrap_or('\uFFFD')
      // In Rust, surrogates (0xD800-0xDFFF) are rejected
      if (codePoint >= 0xD800 && codePoint <= 0xDFFF) {
        result += '\uFFFD';
      } else {
        result += String.fromCodePoint(codePoint);
      }
    } else if (byte >= 0xe0 && byte <= 0xef) {
      const byte2 = i + 1 < bytes.length ? bytes[i + 1] : 0;
      const byte3 = i + 2 < bytes.length ? bytes[i + 2] : 0;
      skip = 2;
      const codePoint = ((byte & 0x0f) << 12) | ((byte2 & 0x3f) << 6) | (byte3 & 0x3f);
      // Rust: char::from_u32(codePoint).unwrap_or('\uFFFD')
      if (codePoint >= 0xD800 && codePoint <= 0xDFFF) {
        result += '\uFFFD';
      } else {
        result += String.fromCodePoint(codePoint);
      }
    } else if (byte >= 0xf0 && byte <= 0xf7) {
      const byte2 = i + 1 < bytes.length ? bytes[i + 1] : 0;
      const byte3 = i + 2 < bytes.length ? bytes[i + 2] : 0;
      const byte4 = i + 3 < bytes.length ? bytes[i + 3] : 0;
      skip = 3;
      const codePoint =
        ((byte & 0x07) << 18) |
        ((byte2 & 0x3f) << 12) |
        ((byte3 & 0x3f) << 6) |
        (byte4 & 0x3f);

      // FIXED Rust behavior: match SDK's broken surrogate pair formula
      if (codePoint >= 0x10000 && codePoint <= 0x10FFFF) {
        // Valid supplementary character
        result += String.fromCodePoint(codePoint);
      } else {
        // Replicate SDK's surrogate pair formula with JS 32-bit arithmetic
        const diff = (codePoint - 0x10000) | 0; // Convert to int32
        const s1 = (0xd800 + (diff >> 10)) & 0xFFFF; // ToUint16
        const s2 = (0xdc00 + (diff & 0x3ff)) & 0xFFFF; // ToUint16
        // Surrogates become U+FFFD, others are normal BMP chars
        if (s1 >= 0xD800 && s1 <= 0xDFFF) {
          result += '\uFFFD';
        } else {
          result += String.fromCharCode(s1);
        }
        if (s2 >= 0xD800 && s2 <= 0xDFFF) {
          result += '\uFFFD';
        } else {
          result += String.fromCharCode(s2);
        }
      }
    }
    // else: 0x80-0xbf (continuation), 0xf8-0xff (invalid) — skip (no output)
  }

  return result;
}

// Now compare for many random 16-byte inputs
console.log("=== Comparing js_to_utf8 (Rust) vs Utils.toUTF8 (SDK) ===\n");

let match = 0, mismatch = 0;
const N = 10000;

for (let trial = 0; trial < N; trial++) {
  const bytes = Array.from(Random(16));

  const sdkResult = Utils.toUTF8(bytes);
  const rustResult = rustJsToUtf8(bytes);

  // Compare the strings
  if (sdkResult === rustResult) {
    match++;
  } else {
    mismatch++;
    if (mismatch <= 10) {
      console.log(`MISMATCH #${mismatch}:`);
      console.log(`  Bytes (hex): ${bytes.map(b => b.toString(16).padStart(2, '0')).join(' ')}`);
      console.log(`  SDK result: ${JSON.stringify(sdkResult)} (${sdkResult.length} chars)`);
      console.log(`  Rust result: ${JSON.stringify(rustResult)} (${rustResult.length} chars)`);
      console.log(`  SDK codepoints: [${[...sdkResult].map(c => 'U+' + c.codePointAt(0).toString(16).toUpperCase().padStart(4, '0')).join(', ')}]`);
      console.log(`  Rust codepoints: [${[...rustResult].map(c => 'U+' + c.codePointAt(0).toString(16).toUpperCase().padStart(4, '0')).join(', ')}]`);

      // Now compare the UTF-8 byte encoding (this is what matters for the invoice number)
      const sdkBytes = Utils.toArray(sdkResult, 'utf8');
      const rustBytes = Utils.toArray(rustResult, 'utf8');
      console.log(`  SDK UTF-8 bytes: [${sdkBytes.map(b => b.toString(16).padStart(2, '0')).join(' ')}]`);
      console.log(`  Rust UTF-8 bytes: [${rustBytes.map(b => b.toString(16).padStart(2, '0')).join(' ')}]`);
      console.log(`  UTF-8 bytes match: ${sdkBytes.length === rustBytes.length && sdkBytes.every((b,i) => b === rustBytes[i]) ? 'YES' : 'NO'}`);

      // Find the specific byte that triggered the mismatch
      for (let i = 0; i < bytes.length; i++) {
        if (bytes[i] >= 0xf0 && bytes[i] <= 0xf7) {
          const b1 = bytes[i], b2 = bytes[i+1] || 0, b3 = bytes[i+2] || 0, b4 = bytes[i+3] || 0;
          const cp = ((b1 & 0x07) << 18) | ((b2 & 0x3f) << 12) | ((b3 & 0x3f) << 6) | (b4 & 0x3f);
          console.log(`  4-byte seq at idx ${i}: bytes=[${b1.toString(16)},${b2.toString(16)},${b3.toString(16)},${b4.toString(16)}] → codepoint=0x${cp.toString(16)} ${cp < 0x10000 ? '(OVERLONG!)' : cp > 0x10FFFF ? '(ABOVE UNICODE!)' : '(valid supplementary)'}`);
        }
      }
      console.log();
    }
  }
}

console.log(`\nResults: ${match} match, ${mismatch} mismatch out of ${N} random 16-byte inputs`);
console.log(`Mismatch rate: ${(mismatch/N*100).toFixed(1)}%`);

// Now test specific edge cases
console.log(`\n=== Edge case tests ===\n`);

// Test 1: 4-byte overlong (0xf0 followed by 0x80-0x8f)
const overlong = [0xf0, 0x80, 0x80, 0x80];
console.log(`Overlong 4-byte [f0 80 80 80]:`);
console.log(`  SDK: ${JSON.stringify(Utils.toUTF8(overlong))}`);
console.log(`  Rust: ${JSON.stringify(rustJsToUtf8(overlong))}`);
console.log(`  SDK codepoints: [${[...Utils.toUTF8(overlong)].map(c => 'U+' + c.codePointAt(0).toString(16).toUpperCase().padStart(4, '0')).join(', ')}]`);
console.log(`  Rust codepoints: [${[...rustJsToUtf8(overlong)].map(c => 'U+' + c.codePointAt(0).toString(16).toUpperCase().padStart(4, '0')).join(', ')}]`);

// Test 2: Valid supplementary character
const valid4 = [0xf0, 0x9f, 0x98, 0x80]; // U+1F600 (😀)
console.log(`\nValid supplementary [f0 9f 98 80] (U+1F600):`);
console.log(`  SDK: ${JSON.stringify(Utils.toUTF8(valid4))}`);
console.log(`  Rust: ${JSON.stringify(rustJsToUtf8(valid4))}`);

// Test 3: Above Unicode range (0xf5+)
const above = [0xf5, 0x80, 0x80, 0x80];
console.log(`\nAbove Unicode [f5 80 80 80]:`);
console.log(`  SDK: ${JSON.stringify(Utils.toUTF8(above))}`);
console.log(`  Rust: ${JSON.stringify(rustJsToUtf8(above))}`);
console.log(`  SDK codepoints: [${[...Utils.toUTF8(above)].map(c => 'U+' + c.codePointAt(0).toString(16).toUpperCase().padStart(4, '0')).join(', ')}]`);
console.log(`  Rust codepoints: [${[...rustJsToUtf8(above)].map(c => 'U+' + c.codePointAt(0).toString(16).toUpperCase().padStart(4, '0')).join(', ')}]`);

// Test 4: 3-byte surrogate (0xed a0-bf xx — encodes U+D800-U+DFFF)
const surr3 = [0xed, 0xa0, 0x80]; // Should encode U+D800
console.log(`\n3-byte surrogate [ed a0 80] (U+D800):`);
console.log(`  SDK: ${JSON.stringify(Utils.toUTF8(surr3))}`);
console.log(`  Rust: ${JSON.stringify(rustJsToUtf8(surr3))}`);
const sdkBytes = Utils.toArray(Utils.toUTF8(surr3), 'utf8');
const rustBytes = Utils.toArray(rustJsToUtf8(surr3), 'utf8');
console.log(`  SDK re-encoded: [${sdkBytes.map(b => b.toString(16).padStart(2, '0')).join(' ')}]`);
console.log(`  Rust re-encoded: [${rustBytes.map(b => b.toString(16).padStart(2, '0')).join(' ')}]`);
