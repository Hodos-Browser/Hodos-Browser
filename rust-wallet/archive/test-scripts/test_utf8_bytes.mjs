#!/usr/bin/env node
// Verify that the FIXED Rust js_to_utf8 produces identical UTF-8 bytes to SDK
import { createRequire } from 'module';
const require = createRequire(import.meta.url);
const sdkPath = '../reference/ts-brc100/node_modules/@bsv/sdk';
const { Utils, Random } = require(sdkPath);

// FIXED Rust simulation
function rustJsToUtf8(bytes) {
  let result = '';
  let skip = 0;
  for (let i = 0; i < bytes.length; i++) {
    const byte = bytes[i];
    if (skip > 0) { skip--; continue; }
    if (byte <= 0x7f) {
      result += String.fromCodePoint(byte);
    } else if (byte >= 0xc0 && byte <= 0xdf) {
      const byte2 = i + 1 < bytes.length ? bytes[i + 1] : 0;
      skip = 1;
      const cp = ((byte & 0x1f) << 6) | (byte2 & 0x3f);
      result += (cp >= 0xD800 && cp <= 0xDFFF) ? '\uFFFD' : String.fromCodePoint(cp);
    } else if (byte >= 0xe0 && byte <= 0xef) {
      const byte2 = i + 1 < bytes.length ? bytes[i + 1] : 0;
      const byte3 = i + 2 < bytes.length ? bytes[i + 2] : 0;
      skip = 2;
      const cp = ((byte & 0x0f) << 12) | ((byte2 & 0x3f) << 6) | (byte3 & 0x3f);
      result += (cp >= 0xD800 && cp <= 0xDFFF) ? '\uFFFD' : String.fromCodePoint(cp);
    } else if (byte >= 0xf0 && byte <= 0xf7) {
      const byte2 = i + 1 < bytes.length ? bytes[i + 1] : 0;
      const byte3 = i + 2 < bytes.length ? bytes[i + 2] : 0;
      const byte4 = i + 3 < bytes.length ? bytes[i + 3] : 0;
      skip = 3;
      const cp = ((byte & 0x07) << 18) | ((byte2 & 0x3f) << 12) | ((byte3 & 0x3f) << 6) | (byte4 & 0x3f);
      if (cp >= 0x10000 && cp <= 0x10FFFF) {
        result += String.fromCodePoint(cp);
      } else {
        const diff = (cp - 0x10000) | 0;
        const s1 = (0xd800 + (diff >> 10)) & 0xFFFF;
        const s2 = (0xdc00 + (diff & 0x3ff)) & 0xFFFF;
        result += (s1 >= 0xD800 && s1 <= 0xDFFF) ? '\uFFFD' : String.fromCharCode(s1);
        result += (s2 >= 0xD800 && s2 <= 0xDFFF) ? '\uFFFD' : String.fromCharCode(s2);
      }
    }
  }
  return result;
}

// Compare UTF-8 BYTES (not strings) for 100k random inputs
let byteMatch = 0, byteMismatch = 0;
const N = 100000;

for (let i = 0; i < N; i++) {
  const bytes = Array.from(Random(16));
  const sdkStr = Utils.toUTF8(bytes);
  const rustStr = rustJsToUtf8(bytes);
  const sdkBytes = Utils.toArray(sdkStr, 'utf8');
  const rustBytes = Utils.toArray(rustStr, 'utf8');

  if (sdkBytes.length === rustBytes.length && sdkBytes.every((b, j) => b === rustBytes[j])) {
    byteMatch++;
  } else {
    byteMismatch++;
    if (byteMismatch <= 3) {
      console.log(`BYTE MISMATCH #${byteMismatch}:`);
      console.log(`  Input: ${bytes.map(b => b.toString(16).padStart(2, '0')).join(' ')}`);
      console.log(`  SDK bytes: [${sdkBytes.map(b => b.toString(16).padStart(2, '0')).join(' ')}]`);
      console.log(`  Rust bytes: [${rustBytes.map(b => b.toString(16).padStart(2, '0')).join(' ')}]`);
    }
  }
}

console.log(`\n100k random 16-byte inputs:`);
console.log(`  UTF-8 byte match: ${byteMatch}`);
console.log(`  UTF-8 byte mismatch: ${byteMismatch}`);
console.log(`  Result: ${byteMismatch === 0 ? '✅ ALL MATCH' : '❌ MISMATCHES FOUND'}`);
