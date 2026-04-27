#!/usr/bin/env node
// Generate QR code test images for the QR scan feature.
// Run: node scripts/generate-qr-test-images.js

const QRCode = require('qrcode');
const path = require('path');

const outputDir = path.join(__dirname, '..', 'public', 'qr-images');

const testCases = [
  { name: 'bsv-address', content: '1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa' },
  { name: 'bip21-amount', content: 'bitcoin:1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa?amount=0.001&label=Test' },
  { name: 'bip21-noamount', content: 'bitcoin:1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa' },
  { name: 'identity-key', content: '02c0b24547a1b8d0c1702c2d9b0c82c10a55fb23c891ed91d5f8e44b9de2e6d3c1' },
  { name: 'paymail', content: 'user@handcash.io' },
  { name: 'handle', content: '$testhandle' },
  { name: 'website-url', content: 'https://www.google.com' },
  { name: 'segwit', content: 'bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4' },
  { name: 'random-text', content: 'Hello World' },
];

async function generate() {
  for (const tc of testCases) {
    const filePath = path.join(outputDir, `qr-${tc.name}.png`);
    await QRCode.toFile(filePath, tc.content, {
      width: 200,
      margin: 2,
      color: { dark: '#000000', light: '#ffffff' },
    });
    console.log(`Generated: ${filePath}`);
  }
  console.log('Done! All QR test images generated.');
}

generate().catch(err => {
  console.error('Failed:', err);
  process.exit(1);
});
