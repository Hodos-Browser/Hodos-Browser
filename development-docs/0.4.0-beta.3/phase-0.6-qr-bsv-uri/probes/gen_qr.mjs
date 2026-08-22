// Generate the Phase 0.6 test QR images as PNG data URLs and emit test pages.
// Uses the qrcode package already in frontend/node_modules (no new dependency).
import QRCode from '../../../../frontend/node_modules/qrcode/lib/index.js';
import { writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, resolve } from 'node:path';

const HERE = dirname(fileURLToPath(import.meta.url));

const BSV_ADDR = '16cezrim1PR2DGuFivZr8kWUSan1LD6XFZ';
const PAYLOADS = {
  // A1/A2 — the real logged PaiyBit payload
  bsv:      `bsv:${BSV_ADDR}?amount=0.11828417&label=PaiyBit%20media`,
  // A3 — bitcoin: must keep working
  bitcoin:  `bitcoin:1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa?amount=0.001&label=Test`,
  // A4 — foreign scheme must be rejected WITH a message naming the scheme
  foreign:  'https://example.com/pay',
  // Injection GREEN (capture path) — attacker amount that WAS arbitrary JS pre-fix
  inj:      `bitcoin:${BSV_ADDR}?amount=(window.__hodos_qr_probe=typeof fetch,0)`,
};

const out = {};
for (const [name, text] of Object.entries(PAYLOADS)) {
  // High error correction, sizeable, quiet zone — so both quirc and jsQR decode reliably.
  const dataUrl = await QRCode.toDataURL(text, { errorCorrectionLevel: 'M', margin: 4, scale: 10 });
  out[name] = { text, dataUrl };
}

// Emit one DOM-scan test page per payload (QR as an <img> the DOM scanner walks),
// plus one CSS-background page per payload (QR NOT a scannable element, forcing the
// screen-capture fallback for the capture-path tests).
function domPage(name) {
  const { text, dataUrl } = out[name];
  return `<!doctype html><html><head><meta charset="utf-8"><title>QR ${name} (DOM)</title></head>
<body style="margin:0;background:#fff;display:flex;flex-direction:column;align-items:center;justify-content:center;height:100vh;font-family:sans-serif">
<div style="font-size:12px;color:#666;margin-bottom:8px">Phase 0.6 DOM test — ${name}</div>
<img id="qr" alt="${name}" src="${dataUrl}" style="width:420px;height:420px">
<div style="font-size:10px;color:#999;margin-top:8px;max-width:600px;word-break:break-all">${text.replace(/</g, '&lt;')}</div>
</body></html>`;
}
function bgPage(name) {
  const { text, dataUrl } = out[name];
  // QR painted as a CSS background-image — invisible to the DOM scanner, so a scan
  // falls through to screen capture. Big and centered for the drag selection.
  return `<!doctype html><html><head><meta charset="utf-8"><title>QR ${name} (capture)</title></head>
<body style="margin:0;height:100vh;background:#fff url('${dataUrl}') center/500px 500px no-repeat">
<div style="position:fixed;top:6px;left:6px;font:11px sans-serif;color:#666">Phase 0.6 CAPTURE test — ${name}</div>
</body></html>`;
}

const manifest = {};
for (const name of Object.keys(PAYLOADS)) {
  const domFile = resolve(HERE, `qrpage_dom_${name}.html`);
  const bgFile = resolve(HERE, `qrpage_capture_${name}.html`);
  writeFileSync(domFile, domPage(name), 'utf8');
  writeFileSync(bgFile, bgPage(name), 'utf8');
  manifest[name] = { text: out[name].text, domFile, bgFile };
}
writeFileSync(resolve(HERE, 'qr_pages_manifest.json'), JSON.stringify(manifest, null, 2), 'utf8');
console.log('Generated test pages:');
for (const [name, m] of Object.entries(manifest)) {
  console.log(`  ${name}: ${m.text}`);
}
