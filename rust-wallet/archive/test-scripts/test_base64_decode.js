// Test TypeScript SDK's base64ToArray behavior
const base64Chars = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/';

const base64ToArray = (msg) => {
  const result = [];
  let currentBit = 0;
  let currentByte = 0;

  for (const char of msg.replace(/=+$/, '')) {
    const idx = base64Chars.indexOf(char);
    // In JavaScript, -1 in bitwise ops becomes 0xFFFFFFFF
    currentBit = (currentBit << 6) | (idx === -1 ? 0xFFFFFFFF : idx);
    currentByte += 6;

    if (currentByte >= 8) {
      currentByte -= 8;
      result.push((currentBit >> currentByte) & 0xff);
      currentBit &= (1 << currentByte) - 1;
    }
  }

  return result;
};

const nonce1 = 'Mg84EukgKCBbq+/sc7xYjgE/Ew+o1kKZDqzvlgOlcag=';
const nonce2 = 'H1s6scwPQ+OMtDPhCfFFRvJMVGnUBYNi0aGPsUpcVpIRgGl4xh7j5JAXX5s7hPHd';
const concat = nonce1 + nonce2;

console.log('Nonce1:', nonce1);
console.log('Nonce2:', nonce2);
console.log('Concatenated:', concat);
console.log('After replace(/=+$/, ""):', concat.replace(/=+$/, ''));
console.log('');

// Proper decode (decode each separately, then concatenate)
const buf1 = Buffer.from(nonce1, 'base64');
const buf2 = Buffer.from(nonce2, 'base64');
const proper = Buffer.concat([buf1, buf2]);
console.log('Proper decode length:', proper.length);
console.log('Proper decode (hex):', proper.toString('hex'));
console.log('');

// TypeScript SDK decode (concatenate strings, then decode)
const tsDecoded = base64ToArray(concat);
console.log('TypeScript SDK decode length:', tsDecoded.length);
console.log('TypeScript SDK decode (hex):', tsDecoded.map(b => b.toString(16).padStart(2, '0')).join(''));
console.log('');

// Check where the difference is
console.log('Difference at byte:', tsDecoded.length - proper.length);
if (tsDecoded.length > proper.length) {
  console.log('Extra bytes in TS decode:', tsDecoded.slice(proper.length).map(b => b.toString(16).padStart(2, '0')).join(''));
}
