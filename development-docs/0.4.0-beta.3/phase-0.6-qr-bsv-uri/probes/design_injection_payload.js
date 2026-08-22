// Phase 0.6 - payload DESIGN aid (not evidence).
// Transcribes, verbatim, the two format strings read from source:
//   1) QRScreenCapture.cpp :: ClassifyAndBuildJson  (BIP21 branch)
//   2) simple_render_process_handler.cpp:920        (qr_screen_capture_result)
// Purpose: confirm the crafted `amount` closes exactly the right brace/paren
// depth BEFORE driving the real binary. The real RED is measured end-to-end.

const BS = String.fromCharCode(92); // backslash, avoids shell-escaping hazards

function JsonEscape(s) { // verbatim: only these 5 cases are escaped
  let out = '';
  for (const c of s) {
    if (c === '"') out += BS + '"';
    else if (c === BS) out += BS + BS;
    else if (c === '\n') out += BS + 'n';
    else if (c === '\r') out += BS + 'r';
    else if (c === '\t') out += BS + 't';
    else out += c;
  }
  return out;
}

function UrlDecode(s) {
  let result = '';
  for (let i = 0; i < s.length; i++) {
    if (s[i] === '%' && i + 2 < s.length) {
      const hex = s.substr(i + 1, 2);
      if (/^[0-9a-fA-F]{2}$/.test(hex)) {
        result += String.fromCharCode(parseInt(hex, 16));
        i += 2;
        continue;
      }
    }
    if (s[i] === '+') { result += ' '; continue; }
    result += s[i];
  }
  return result;
}

// --- ClassifyAndBuildJson, BIP21 branch, verbatim ---
function ClassifyAndBuildJson(text) {
  if (!/^bitcoin:/i.test(text)) return ''; // pre-fix: bitcoin: only
  let address = '', amount = '', label = '';
  const colon = text.indexOf(':');
  const rest = colon !== -1 ? text.substring(colon + 1) : text;
  const q = rest.indexOf('?');
  address = q !== -1 ? rest.substring(0, q) : rest;
  if (q !== -1) {
    for (const pair of rest.substring(q + 1).split('&')) {
      const eq = pair.indexOf('=');
      if (eq === -1) continue;
      const key = pair.substring(0, eq);
      const val = UrlDecode(pair.substring(eq + 1)); // <-- first '=' only
      if (key === 'amount') amount = val;
      else if (key === 'label') label = val;
    }
  }
  let json = '{"type":"bip21","value":"' + JsonEscape(text) + '"';
  if (address) json += ',"address":"' + JsonEscape(address) + '"';
  if (amount) json += ',"amount":' + amount; // <-- RAW, UNQUOTED, UNESCAPED
  if (label) json += ',"label":"' + JsonEscape(label) + '"';
  json += ',"source":"screen"}';
  return json;
}

const PAYLOAD = process.argv[2];
const resultJson = ClassifyAndBuildJson(PAYLOAD);
if (!resultJson) {
  console.log('CLASSIFIER REJECTED (returns "")');
  process.exit(0);
}

// FinishQRScreenCapture wraps it:
const delivered = '{"status":"found","result":' + resultJson + '}';
// simple_render_process_handler.cpp:920, verbatim:
const js = "window.dispatchEvent(new MessageEvent('message',{data:{type:'qr_screen_capture_result',data:" + delivered + "}}));";

console.log('--- payload ---');
console.log(PAYLOAD);
console.log('');
console.log('--- delivered JSON ---');
console.log(delivered);
console.log('');
console.log('--- JS the renderer executes ---');
console.log(js);
console.log('');
console.log('--- is the delivered JSON even valid JSON? ---');
try {
  JSON.parse(delivered);
  console.log('  valid JSON');
} catch (e) {
  console.log('  INVALID JSON: ' + e.message);
}

console.log('');
console.log('--- does that JS parse, and what does it do? ---');
let dispatched = null;
globalThis.window = { dispatchEvent: (e) => { dispatched = e; } };
globalThis.MessageEvent = function (t, o) { this.data = o && o.data; };
try {
  (0, eval)(js);
  console.log('  JS PARSED AND RAN');
  console.log('  window.__hodos_qr_probe = ' + JSON.stringify(globalThis.window.__hodos_qr_probe));
  console.log('  dispatched data      = ' + JSON.stringify(dispatched && dispatched.data));
} catch (e) {
  console.log('  JS THREW: ' + e.constructor.name + ': ' + e.message);
}
