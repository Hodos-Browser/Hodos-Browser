// P0.6 T1e — real-source unit harness for the QR scheme classifier (sites #3 and #4).
//
// Exercises the ACTUAL shipped sources, not a re-implementation:
//   #4  frontend/src/utils/bip21.ts          — imported via Node type-stripping
//   #3  cef-native/build_tools/qr-scanner-logic.js — run through its real IIFE in a
//        vm sandbox with a stubbed jsQR (returns the payload) and a minimal DOM
//
// SUBJECT: asserts on the extracted ADDRESS STRING, char-for-char — never on
// "the scanner returned something" (a truncated address would pass that).
//
// Modes:
//   (default)          GREEN — bsv: and bitcoin: both yield the FULL address;
//                      a foreign scheme yields null.
//   --negative-control RED   — reverts the scheme strip to slice(8) (the trap) and
//                      proves the bsv: address truncates and fails the address regex,
//                      i.e. the assertion actually distinguishes full vs truncated.
//
// Exit 0 = all expectations met for the mode; exit 1 = a failure (preflight reads this).
// In --negative-control mode the SUCCESS condition is that the truncation is observed,
// so the harness exits 0 when the trap is correctly caught and 1 if it is NOT — the
// inversion preflight's -NegativeControl expects.

import { readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { dirname, resolve } from 'node:path';
import { tmpdir } from 'node:os';
import vm from 'node:vm';

// Revert the scheme strip to the slice(8) trap (the A6 negative control). Applies
// to both the .ts and .js spellings of "strip by first colon".
function revertStripToSlice8(src) {
  return src
    .replace(/uri\.slice\(uri\.indexOf\(':'\)\s*\+\s*1\)/g, 'uri.slice(8)');
}

const HERE = dirname(fileURLToPath(import.meta.url));
const REPO = resolve(HERE, '../../../..'); // probes -> phase -> 0.4.0-beta.3 -> development-docs -> repo
const NC = process.argv.includes('--negative-control');

const BSV_ADDR = '16cezrim1PR2DGuFivZr8kWUSan1LD6XFZ';
const PAYLOADS = {
  bsv:     `bsv:${BSV_ADDR}?amount=0.11828417&label=PaiyBit%20media`,
  bitcoin: `bitcoin:${BSV_ADDR}?amount=0.001&label=Test`,
  foreign: 'https://example.com/pay',
};

const results = [];
function record(site, name, payload, gotAddress, gotAmount) {
  results.push({ site, name, payload, gotAddress, gotAmount });
}

// ---------------------------------------------------------------------------
// Site #4 — frontend/src/utils/bip21.ts (real import)
// ---------------------------------------------------------------------------
async function runBip21Ts() {
  const realPath = resolve(REPO, 'frontend/src/utils/bip21.ts');
  let importPath = realPath;
  if (NC) {
    // Reintroduce the trap on a COPY of the real source, then import that copy.
    const reverted = revertStripToSlice8(readFileSync(realPath, 'utf8'));
    const tmp = resolve(tmpdir(), 'hodos_bip21_nc.ts');
    writeFileSync(tmp, reverted, 'utf8');
    importPath = tmp;
  }
  const mod = await import(pathToFileURL(importPath).href);
  const parseBIP21 = mod.parseBIP21;
  for (const [name, payload] of Object.entries(PAYLOADS)) {
    const r = parseBIP21(payload);
    record('#4 bip21.ts', name, payload, r ? r.address : null, r ? r.amount : null);
  }
}

// ---------------------------------------------------------------------------
// Site #3 — cef-native/build_tools/qr-scanner-logic.js (real IIFE in a sandbox)
// ---------------------------------------------------------------------------
function makeDomSandbox(payload, capture) {
  const imgEl = { complete: true, naturalWidth: 100, naturalHeight: 100 };
  const canvasStub = () => ({
    width: 0, height: 0,
    getContext: () => ({
      drawImage: () => {},
      getImageData: (x, y, w, h) => ({ data: new Uint8ClampedArray(Math.max(1, w * h * 4)), width: w, height: h }),
    }),
  });
  const document = {
    querySelectorAll: (sel) => (sel === 'img' ? [imgEl] : []),
    createElement: (tag) => (tag === 'canvas' ? canvasStub() : {}),
  };
  const jsQRLib = () => ({ data: payload }); // decode always yields our payload
  const sandbox = {
    document,
    jsQRLib,
    window: { cefMessage: { send: (name, args) => capture(name, args) } },
    Image: function () {}, XMLSerializer: function () {}, Blob: function () {},
    URL: { createObjectURL: () => 'blob:', revokeObjectURL: () => {} },
    Set, Promise, setTimeout, console,
    decodeURIComponent, parseFloat,
  };
  return sandbox;
}

function loadScannerSource() {
  let src = readFileSync(resolve(REPO, 'cef-native/build_tools/qr-scanner-logic.js'), 'utf8');
  if (NC) src = revertStripToSlice8(src); // reintroduce the slice(8) trap for A6
  return src;
}

function runScannerJs(payload) {
  return new Promise((resolveP) => {
    let done = false;
    let timer = null;
    const settle = (v) => { if (done) return; done = true; if (timer) clearTimeout(timer); resolveP(v); };
    const finish = (name, args) => {
      if (name !== 'qr_found' || done) return;
      let arr = [];
      try { arr = JSON.parse(args[0]); } catch { arr = []; }
      settle(arr);
    };
    const sandbox = makeDomSandbox(payload, finish);
    vm.createContext(sandbox);
    try {
      vm.runInContext(loadScannerSource(), sandbox, { timeout: 5000 });
    } catch (e) {
      settle({ __error: String(e) });
    }
    timer = setTimeout(() => settle([]), 3000);
  });
}

async function runScannerAll() {
  for (const [name, payload] of Object.entries(PAYLOADS)) {
    const arr = await runScannerJs(payload);
    if (arr && arr.__error) { record('#3 qr-scanner-logic.js', name, payload, `ERROR:${arr.__error}`, null); continue; }
    const hit = Array.isArray(arr) ? arr.find((r) => r.type === 'bip21' || r.type === 'address') : null;
    record('#3 qr-scanner-logic.js', name, payload, hit ? (hit.address || hit.value) : null, hit ? hit.amount : undefined);
  }
}

// ---------------------------------------------------------------------------
function evaluate() {
  let failures = 0;
  const log = [];
  for (const r of results) {
    let verdict;
    if (NC) {
      // Negative control (A6): with slice(8) reintroduced, bsv: must NOT yield the
      // full address. It manifests two ways, both proving the GREEN assertion catches
      // the trap: #4 (raw parser) returns the truncated string "zrim1PR2…"; #3 (scanner)
      // fails CLOSED to null because that truncated address fails the BSV address regex.
      // Either is a demonstration that the trap breaks the char-for-char GREEN check.
      if (r.name === 'bsv') {
        const trapCaught = r.gotAddress !== BSV_ADDR;
        const how = r.gotAddress === null ? 'fail-closed(null)' : `truncated(${r.gotAddress})`;
        verdict = trapCaught ? `PASS(NC: trap caught — ${how})` : 'FAIL(NC: full address survived slice(8)?!)';
        if (!trapCaught) failures++;
      } else {
        verdict = 'n/a(NC)';
      }
    } else {
      if (r.name === 'foreign') {
        verdict = r.gotAddress === null ? 'PASS' : 'FAIL(foreign accepted)';
        if (r.gotAddress !== null) failures++;
      } else {
        // bsv & bitcoin: must yield the FULL address, char-for-char.
        verdict = r.gotAddress === BSV_ADDR ? 'PASS' : `FAIL(addr=${JSON.stringify(r.gotAddress)})`;
        if (r.gotAddress !== BSV_ADDR) failures++;
      }
    }
    log.push(`  [${verdict}] ${r.site} ${r.name} -> addr=${JSON.stringify(r.gotAddress)} amount=${JSON.stringify(r.gotAmount)}`);
  }
  console.log(`T1e QR scheme classifier — mode=${NC ? 'NEGATIVE-CONTROL' : 'GREEN'}`);
  console.log(log.join('\n'));
  console.log(failures === 0 ? 'T1e: OK' : `T1e: ${failures} FAILURE(S)`);
  return failures === 0 ? 0 : 1;
}

await runBip21Ts();
await runScannerAll();
process.exitCode = evaluate(); // let Node drain handles cleanly instead of process.exit()
