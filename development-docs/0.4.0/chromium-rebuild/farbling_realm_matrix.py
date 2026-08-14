#!/usr/bin/env python3
r"""farbling_realm_matrix.py — which §A REALMS carry the top frame's farbling key?

Converts the ❓ rows of FARBLING_DEFINITION_OF_DONE.md §A into ✅ or ⛔, each on its own
evidence. Covers R6 (popup on a real URL), R8 (nested worker), R11 (worklets), R12 (fenced
frame), R13 (sandboxed/opaque-origin iframe), R14 (document.write / javascript: document)
and R15 (bfcache restore).

## The assertion, and why it is the STRONG one

For every realm we do not ask "does it differ from native" — we ask the two-sided question:

    realm value == the top frame's FARBLED value   ⇒  ✅ keyed, and keyed to the TOP FRAME
    realm value == the top frame's NATIVE value    ⇒  ⛔ unkeyed, fails closed to native
    neither                                        ⇒  INVESTIGATE, never silently "pass"

The one-sided version ("differs from native") is the weaker assertion that §3's S3 row was
criticised for: a realm keyed on *its own* origin instead of the top frame's would sail
through it while being exactly the wrong-model outcome the design rejects.

## ⛔ Realms do not all have the same APIs, and comparing across probes is meaningless

A worker and a worklet have no `document`, so the DOM canvas probe every other harness
uses cannot run there. A hash from `document.createElement('canvas')` and a hash from
`new OffscreenCanvas()` are different numbers for the ordinary reason, so comparing one
realm's OffscreenCanvas hash against a DOM-canvas reference would report a difference that
has nothing to do with farbling — the exact ambiguity that made the first worker probe
need its control.

So each realm declares which probe it can run, and is compared ONLY against a reference
taken with **the same probe in the same arm**. Both references are measured every run.

## Capability, not assumption

R12 needs a fenced-frame config that a local page cannot mint. Rather than assume, the
harness reports whether the realm EXISTS in this build (`HTMLFencedFrameElement`), because
"the container is not present" and "the container is present and unkeyed" are different
answers and only one of them is a gap.

## Usage

    python3 farbling_realm_matrix.py --dev \
        --exe ...\cef-native\build\bin\Release\HodosBrowser.exe \
        --data-root %APPDATA%\HodosBrowserDev

    python3 farbling_realm_matrix.py ... --negative-control

Exit 0 = every reachable realm ✅. 1 = at least one ⛔. 2 = run void / something unresolved.
"""

import argparse
import json
import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

try:
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
except Exception:
    pass

try:
    import websocket  # noqa: F401
except ImportError:
    sys.exit("need websocket-client:  pip install websocket-client")

from farbling_seed_rotation_check import (  # noqa: E402
    engine_version,
    kill_browser_by_path,
    launch_browser,
    set_site_enabled,
    wait_for_cdp,
)
from farbling_cross_profile_check import cdp_port_for, profile_dir  # noqa: E402
from farbling_worker_probe import (  # noqa: E402
    optout_state,
    resolve_tab,
    snapshot_targets,
)
from farbling_iframe_check import _rpc  # noqa: E402

HOST = "example.com"
URL = "https://example.com/"
AWAY_URL = "https://example.org/"   # for the bfcache round trip

# (key, §A row, probe kind, expectation) — the expectation is documentation of the prior
# and is never an input to the verdict.
REALMS = [
    ("r6_popup_url",   "R6  popup navigated to a real URL",      "dom", "KEYED"),
    ("r8_nested",      "R8  nested worker (worker -> worker)",   "oc",  "UNKEYED"),
    ("t3_iframe_worker", "T3  worker created INSIDE a subframe", "oc",  "UNKEYED"),
    ("r11_worklet",    "R11 AudioWorklet global scope",          "oc",  "UNKEYED"),
    ("r13_sandbox",    "R13 sandboxed iframe (opaque origin)",   "dom", "KEYED"),
    ("r14_docwrite",   "R14 document.write document",            "dom", "KEYED"),
    ("r14_javascript", "R14 javascript: URL document",           "dom", "KEYED"),
    ("r15_bfcache",    "R15 bfcache-restored page",              "dom", "KEYED"),
]

# ---------------------------------------------------------------------------------------
# OC_PROBE: OffscreenCanvas + navigator only. Runs anywhere with a global scope --
# window, DedicatedWorkerGlobalScope, and (if it has the APIs) a worklet. No document,
# no text: font fallback can legitimately differ between realms and would contaminate a
# comparison meant to isolate farbling.
# ---------------------------------------------------------------------------------------
OC_PROBE = r"""
(async function () {
  var FNV = function (b) {
    var h = 2166136261 >>> 0;
    for (var i = 0; i < b.length; i++) { h ^= (b[i] & 255); h = Math.imul(h, 16777619) >>> 0; }
    return ('0000000' + (h >>> 0).toString(16)).slice(-8);
  };
  var out = { canvas: null, gl: null, blob: null, cores: null, mem: null,
              err: null, has: {} };
  out.has.OffscreenCanvas = (typeof OffscreenCanvas !== 'undefined');
  out.has.navigator = (typeof navigator !== 'undefined');
  try {
    if (out.has.OffscreenCanvas) {
      var c = new OffscreenCanvas(200, 50);
      var x = c.getContext('2d');
      x.fillStyle = '#f60'; x.fillRect(0, 0, 100, 20);
      x.fillStyle = '#069'; x.fillRect(10, 12, 60, 25);
      var g = x.createLinearGradient(0, 0, 200, 50);
      g.addColorStop(0, 'rgba(0,120,255,0.7)');
      g.addColorStop(1, 'rgba(255,0,90,0.35)');
      x.fillStyle = g; x.fillRect(0, 20, 200, 30);
      x.strokeStyle = 'rgba(0,0,0,0.8)'; x.lineWidth = 3;
      x.beginPath(); x.arc(40, 25, 18, 0, Math.PI * 2); x.stroke();
      out.canvas = FNV(x.getImageData(0, 0, 200, 50).data);

      // T6-worker: convertToBlob is reachable HERE too, and a worker is the realm
      // where it cannot fall back to HTMLCanvasElement's hooked encoders -- there is
      // no HTMLCanvasElement in a worker at all.
      try {
        var blob = await c.convertToBlob();
        out.blob = FNV(new Uint8Array(await blob.arrayBuffer()));
      } catch (e) { out.blob = 'ERR:' + String(e && e.message || e); }

      // T10: WebGL readPixels via OffscreenCanvas. The C4 hook reads the
      // ExecutionContext, so this should turn green the moment a worker gets a key --
      // "should" being why it is measured rather than asserted.
      try {
        var gc = new OffscreenCanvas(120, 60);
        var gl = gc.getContext('webgl2') || gc.getContext('webgl');
        if (gl) {
          gl.clearColor(0.25, 0.5, 0.75, 1.0); gl.clear(gl.COLOR_BUFFER_BIT);
          gl.enable(gl.SCISSOR_TEST); gl.scissor(10, 10, 40, 20);
          gl.clearColor(0.9, 0.1, 0.4, 1.0); gl.clear(gl.COLOR_BUFFER_BIT);
          gl.disable(gl.SCISSOR_TEST);
          var px = new Uint8Array(120 * 60 * 4);
          gl.readPixels(0, 0, 120, 60, gl.RGBA, gl.UNSIGNED_BYTE, px);
          out.gl = FNV(px);
        } else { out.gl = 'ERR:nocontext'; }
      } catch (e) { out.gl = 'ERR:' + String(e && e.message || e); }
    }
    if (out.has.navigator) {
      out.cores = (typeof navigator.hardwareConcurrency === 'number')
                    ? navigator.hardwareConcurrency : null;
      out.mem = (typeof navigator.deviceMemory === 'number')
                    ? navigator.deviceMemory : null;
    }
  } catch (e) { out.err = String(e && e.message || e); }
  return out;
})()
"""

# DOM_PROBE: same shape, but through an HTMLCanvasElement, for realms that have a document.
DOM_PROBE = OC_PROBE.replace(
    "var c = new OffscreenCanvas(200, 50);",
    "var c = document.createElement('canvas'); c.width = 200; c.height = 50;")

REALM_JS = r"""
(async function () {
  var OC = %(oc)s;      // OffscreenCanvas probe source, as a string
  var DOM = %(dom)s;    // DOM canvas probe source, as a string
  var out = { href: location.href, realms: {}, caps: {} };

  var put = function (k, v, err) {
    out.realms[k] = { value: v || null, err: err ? String(err && err.message || err) : null };
  };

  // ---- references, both probes, in THIS realm and THIS arm --------------------------
  // Everything below is compared against these and never against the other arm's other
  // probe. Taken first so a later realm that hangs cannot cost us the baseline.
  try { out.refDom = await (0, eval)(DOM); } catch (e) { out.refDomErr = String(e); }
  try { out.refOc = await (0, eval)(OC); } catch (e) { out.refOcErr = String(e); }

  // ---- capabilities ------------------------------------------------------------------
  out.caps.fencedFrame = (typeof HTMLFencedFrameElement !== 'undefined');
  out.caps.audioWorklet = !!(window.AudioContext &&
                             (new AudioContext()).audioWorklet);
  out.caps.paintWorklet = (typeof CSS !== 'undefined' && !!CSS.paintWorklet);

  // ---- R6: popup navigated to a REAL URL ---------------------------------------------
  // Distinct from the about:blank popup already covered by R5: this one commits an
  // http(s) document, so it should take the ordinary main-frame path.
  try {
    var w = window.open('%(url)s?r6=1', 'hodos_r6', 'width=420,height=320');
    if (!w) {
      put('r6_popup_url', null, 'window.open returned null (blocked or intercepted)');
    } else {
      // Wait for the real URL to commit; measuring the intermediate about:blank would
      // silently make this a duplicate of R5 rather than a test of R6.
      var ok = false;
      for (var i = 0; i < 60; i++) {
        try {
          if (w.location && w.location.href.indexOf('r6=1') !== -1
              && w.document && w.document.readyState !== 'loading') { ok = true; break; }
        } catch (e) { /* still cross-origin mid-navigation */ }
        await new Promise(function (r) { setTimeout(r, 250); });
      }
      if (!ok) { put('r6_popup_url', null, 'popup never committed the r6 URL'); }
      else {
        var v = await w.eval(DOM);   // async probe -> MUST await, or we store a Promise
        v.href = w.location.href;      // subject assertion, checked host-side
        put('r6_popup_url', v);
      }
      try { w.close(); } catch (e) {}
    }
  } catch (e) { put('r6_popup_url', null, e); }

  // ---- R8: nested worker (document -> worker A -> worker B) ---------------------------
  try {
    // The probe is async (convertToBlob is promise-valued), so the worker must RESOLVE
    // it before posting. Posting the promise itself would structured-clone-fail and the
    // realm would report UNREACHABLE for a harness bug rather than a farbling result.
    var inner = 'self.onmessage=function(ev){' +
                'Promise.resolve(eval("("+ev.data+")")).then(function(v){' +
                'self.postMessage(v);},function(e){' +
                'self.postMessage({err:String(e&&e.message||e),has:{}});});};';
    var outer = 'self.onmessage=function(ev){' +
      'var iu=URL.createObjectURL(new Blob([ev.data.inner],{type:"text/javascript"}));' +
      'var w2=new Worker(iu);' +          // <-- the NESTED worker; this is the realm
      'w2.onmessage=function(e2){self.postMessage({ok:true,v:e2.data});};' +
      'w2.onerror=function(e2){self.postMessage({ok:false,err:"inner: "+(e2&&e2.message)});};' +
      'w2.postMessage(ev.data.probe);};';
    var ou = URL.createObjectURL(new Blob([outer], { type: 'text/javascript' }));
    var w1 = new Worker(ou);
    var res = await new Promise(function (resolve) {
      var done = false;
      var fin = function (v) { if (!done) { done = true; try { w1.terminate(); } catch (e) {} resolve(v); } };
      setTimeout(function () { fin({ ok: false, err: 'timeout' }); }, 20000);
      w1.onmessage = function (ev) { fin(ev.data); };
      w1.onerror = function (ev) { fin({ ok: false, err: 'outer: ' + (ev && ev.message) }); };
      w1.postMessage({ inner: inner, probe: OC });
    });
    if (res && res.ok) { put('r8_nested', res.v); }
    else { put('r8_nested', null, (res && res.err) || 'no result'); }
  } catch (e) { put('r8_nested', null, e); }

  // ---- R11: AudioWorklet global scope --------------------------------------------------
  // A worklet may simply not HAVE the APIs a fingerprint needs. That is a different answer
  // from "unkeyed", so the probe reports which globals exist rather than only a hash.
  try {
    var wsrc =
      'class P extends AudioWorkletProcessor {' +
      '  constructor(){ super(); var self_ = this;' +
      '  try { Promise.resolve((0,eval)(' + JSON.stringify(OC) + '))' +
      '    .then(function(v){ self_.port.postMessage(v); },' +
      '          function(e){ self_.port.postMessage({err:String(e&&e.message||e),has:{}}); }); }' +
      '  catch(e){ this.port.postMessage({err:String(e&&e.message||e), has:{}}); } }' +
      '  process(){ return false; } }' +
      'registerProcessor("hodos-probe", P);';
    var wu = URL.createObjectURL(new Blob([wsrc], { type: 'text/javascript' }));
    var actx = new AudioContext();
    await actx.audioWorklet.addModule(wu);
    var node = new AudioWorkletNode(actx, 'hodos-probe');
    var wv = await new Promise(function (resolve) {
      var done = false;
      var fin = function (v) { if (!done) { done = true; resolve(v); } };
      setTimeout(function () { fin(null); }, 15000);
      node.port.onmessage = function (ev) { fin(ev.data); };
      node.port.start && node.port.start();
    });
    try { actx.close(); } catch (e) {}
    if (wv) { put('r11_worklet', wv); } else { put('r11_worklet', null, 'no message from worklet'); }
  } catch (e) { put('r11_worklet', null, e); }

  // ---- T3: a worker created INSIDE a same-origin subframe -------------------------------
  // ⭐ The case most likely to be subtly wrong, and the reason it gets its own row.
  //
  // The worker's key comes from whichever ExecutionContext called `new Worker()`. Here
  // that is the IFRAME's window, not the top frame's. Post-P4e the iframe holds the TOP
  // frame's key, so the worker must come out equal to the TOP frame's farbled value. If
  // instead someone ever keys a subframe on its own origin, this row is where it shows up
  // -- and a same-origin iframe would still LOOK right on every other test, because its
  // own origin and the top frame's are identical. It is caught here only because the
  // assertion is "== the top frame's farbled value", not "!= native".
  try {
    var wf2 = document.createElement('iframe');
    document.body.appendChild(wf2);            // about:blank, same origin, scriptable
    var childWin = wf2.contentWindow;
    var iv = await childWin.eval(
      '(' + (function (src) {
        return new Promise(function (resolve) {
          var done = false;
          var fin = function (v) { if (!done) { done = true; resolve(v); } };
          setTimeout(function () { fin({ err: 'timeout', has: {} }); }, 20000);
          try {
            var u = URL.createObjectURL(new Blob([
              'self.onmessage=function(ev){Promise.resolve(eval("("+ev.data+")")).then(' +
              'function(v){self.postMessage(v);},' +
              'function(e){self.postMessage({err:String(e&&e.message||e),has:{}});});};'
            ], { type: 'text/javascript' }));
            var w = new Worker(u);             // <-- created BY THE IFRAME
            w.onmessage = function (ev) { fin(ev.data); };
            w.onerror = function (ev) { fin({ err: 'workererror: ' + (ev && ev.message), has: {} }); };
            w.postMessage(src);
          } catch (e) { fin({ err: String(e && e.message || e), has: {} }); }
        });
      }).toString() + ')(' + JSON.stringify(OC) + ')');
    put('t3_iframe_worker', iv);
    wf2.remove();
  } catch (e) { put('t3_iframe_worker', null, e); }

  // ---- R11b: PaintWorklet global scope -------------------------------------------------
  // ⛔ A PaintWorkletGlobalScope has NO port, NO postMessage and NO fetch, so it cannot
  // hand a value back the way the AudioWorklet probe above does. That is not a reason to
  // skip it -- it is a reason to pick an observable that does escape: `addModule()`
  // REJECTS when the module's top-level code throws. So each capability question is asked
  // as "throw unless the API exists", and the promise's fate is the answer.
  //
  // This matters beyond bookkeeping: a realm that can neither read a fingerprint surface
  // NOR communicate a result is not a bypass, and we should be able to say which of those
  // two it is rather than asserting both.
  out.caps.paintWorkletHas = {};
  var askPaint = async function (name, expr) {
    try {
      var src = 'if (!(' + expr + ')) { throw new Error("absent"); } ' +
                'registerPaint("hodos-' + name + '", class { paint() {} });';
      var u = URL.createObjectURL(new Blob([src], { type: 'text/javascript' }));
      await CSS.paintWorklet.addModule(u);
      out.caps.paintWorkletHas[name] = true;
    } catch (e) {
      out.caps.paintWorkletHas[name] = false;
    }
  };
  try {
    if (typeof CSS !== 'undefined' && CSS.paintWorklet) {
      // TWO control arms, and BOTH are load-bearing:
      //   sanity  -- a module that cannot throw must RESOLVE, else every 'present' is a
      //              false negative;
      //   forced  -- a module that ALWAYS throws must REJECT, else the observable has no
      //              negative direction at all and every 'present' is unfalsifiable.
      // The second arm exists because the first run of this probe reported `document`
      // PRESENT in a PaintWorkletGlobalScope, which cannot be true -- the tell that
      // addModule() was resolving regardless of what the module body did.
      await askPaint('sanity', 'true');
      await askPaint('forced', 'false');
      await askPaint('oc', 'typeof OffscreenCanvas !== "undefined"');
      await askPaint('nav', 'typeof navigator !== "undefined"');
      await askPaint('canvasel', 'typeof document !== "undefined"');
    }
  } catch (e) { out.caps.paintWorkletErr = String(e && e.message || e); }

  // ---- R12: can a fenced frame even be constructed from a page? -----------------------
  // Reported, not assumed. A fenced frame needs a config minted by an API a local page may
  // not be able to call at all; "the container exists" and "I can put content in it" are
  // different facts and only the second makes it measurable here.
  out.caps.fenced = {
    element: (typeof HTMLFencedFrameElement !== 'undefined'),
    config: (typeof FencedFrameConfig !== 'undefined'),
    runAdAuction: !!(navigator.runAdAuction),
    sharedStorage: (typeof window.sharedStorage !== 'undefined'),
    selectURL: !!(window.sharedStorage && window.sharedStorage.selectURL)
  };

  // ---- R13: sandboxed iframe, OPAQUE origin -------------------------------------------
  // `allow-scripts` WITHOUT `allow-same-origin` => opaque origin => the parent cannot
  // script it, so the child must hand its result back over postMessage. If we granted
  // allow-same-origin to make it easier to read, we would no longer be testing R13.
  try {
    var sf = document.createElement('iframe');
    sf.setAttribute('sandbox', 'allow-scripts');
    sf.srcdoc = '<script>window.addEventListener("message",function(ev){' +
                'try { Promise.resolve((0,eval)(ev.data)).then(function(r){' +
                'ev.source.postMessage(JSON.stringify(r),"*");},function(e){' +
                'ev.source.postMessage(JSON.stringify({err:String(e)}),"*");}); }' +
                'catch(e){ ev.source.postMessage(JSON.stringify({err:String(e)}),"*"); }});' +
                'parent.postMessage("ready","*");<\/script>';
    document.body.appendChild(sf);
    var sv = await new Promise(function (resolve) {
      var done = false, ready = false;
      var fin = function (v) { if (!done) { done = true; window.removeEventListener('message', h); resolve(v); } };
      var h = function (ev) {
        if (ev.source !== sf.contentWindow) return;
        if (ev.data === 'ready' && !ready) { ready = true; sf.contentWindow.postMessage(DOM, '*'); return; }
        if (typeof ev.data === 'string' && ev.data !== 'ready') {
          try { fin(JSON.parse(ev.data)); } catch (e) { fin(null); }
        }
      };
      window.addEventListener('message', h);
      setTimeout(function () { fin(null); }, 15000);
    });
    sf.remove();
    if (sv) { put('r13_sandbox', sv); } else { put('r13_sandbox', null, 'no reply from sandboxed frame'); }
  } catch (e) { put('r13_sandbox', null, e); }

  // ---- R14a: document.write ------------------------------------------------------------
  // document.open() replaces the Document but REUSES the LocalDOMWindow, which is where
  // the key lives -- so this is a genuine test of whether the Supplement survives a
  // document swap, not merely another same-origin frame.
  try {
    var wf = document.createElement('iframe');
    document.body.appendChild(wf);
    var d = wf.contentDocument;
    d.open();
    d.write('<!doctype html><html><body><p>hodos r14</p></body></html>');
    d.close();
    var wv2 = await wf.contentWindow.eval(DOM);
    wv2.href = wf.contentWindow.location.href;
    put('r14_docwrite', wv2);
    wf.remove();
  } catch (e) { put('r14_docwrite', null, e); }

  // ---- R14b: javascript: URL -----------------------------------------------------------
  try {
    var jf = document.createElement('iframe');
    jf.src = 'javascript:"<!doctype html><body>hodos r14b</body>"';
    document.body.appendChild(jf);
    await new Promise(function (r) { setTimeout(r, 800); });
    var jv = await jf.contentWindow.eval(DOM);
    jv.href = jf.contentWindow.location.href;
    put('r14_javascript', jv);
    jf.remove();
  } catch (e) { put('r14_javascript', null, e); }

  return JSON.stringify(out);
})()
"""

# R15 runs as its own three-step sequence rather than inside REALM_JS, because it needs
# two real navigations. `window.__hodos_bfcache_marker` is the load-bearing control: if it
# survives the back navigation then the SAME LocalDOMWindow was restored, i.e. bfcache
# genuinely engaged. Without it a plain reload would take the ordinary main-frame path and
# report a trivially green R15 that tested nothing.
BF_MARK_JS = """
(function () { window.__hodos_bfcache_marker = 'hodos-%s'; return 'marked'; })()
"""
BF_READ_JS = r"""
(function () {
  return JSON.stringify({
    href: location.href,
    marker: (typeof window.__hodos_bfcache_marker === 'string')
              ? window.__hodos_bfcache_marker : null
  });
})()
"""


PAINT_CONSOLE_JS = r"""
(async function () {
  // The page's own marker proves the CAPTURE works. Without it, "no worklet message"
  // is indistinguishable from "console is not forwarded from this realm", and absence of
  // evidence would get written down as evidence of absence.
  console.log('HODOSPW page {"realm":"page","oc":' + (typeof OffscreenCanvas !== 'undefined') +
              ',"nav":' + (typeof navigator !== 'undefined') +
              ',"doc":' + (typeof document !== 'undefined') + '}');
  var src =
    'console.log("HODOSPW worklet {\\"realm\\":\\"paintworklet\\",\\"oc\\":" +' +
    ' (typeof OffscreenCanvas !== "undefined") + ",\\"nav\\":" +' +
    ' (typeof navigator !== "undefined") + ",\\"doc\\":" +' +
    ' (typeof document !== "undefined") + "}");' +
    'registerPaint("hodos-pw", class { paint(){} });';
  var u = URL.createObjectURL(new Blob([src], { type: 'text/javascript' }));
  try { await CSS.paintWorklet.addModule(u); } catch (e) { return 'addModule threw: ' + e; }
  // Force the worklet to actually instantiate: Chromium creates the global scope lazily,
  // so a module that is added but never painted may never evaluate at all.
  var d = document.createElement('div');
  d.style.cssText = 'width:60px;height:60px;background-image:paint(hodos-pw)';
  document.body.appendChild(d);
  d.offsetHeight;
  await new Promise(function (r) { setTimeout(r, 2500); });
  d.remove();
  return 'ok';
})()
"""


def probe_paint_worklet(port, excluded, wait=45):
    """Capture PaintWorkletGlobalScope's globals over CDP's console channel.

    ⛔ Why not addModule()'s promise: measured on this build, a module whose body THROWS
    still resolves addModule(), so that observable has no negative direction and every
    answer it gives is unfalsifiable. This channel carries its own control -- the page
    logs the same shape first, so a missing worklet line can be told apart from a console
    that is simply not forwarded out of that realm.
    """
    t = resolve_tab(port, excluded)
    ws = websocket.create_connection(t["webSocketDebuggerUrl"], timeout=30)
    seen = []
    try:
        ws.send(json.dumps({"id": 90, "method": "Runtime.enable"}))
        ws.send(json.dumps({"id": 91, "method": "Log.enable"}))
        ws.send(json.dumps({"id": 92, "method": "Runtime.evaluate",
                            "params": {"expression": PAINT_CONSOLE_JS,
                                       "returnByValue": True, "awaitPromise": True}}))
        end = time.time() + wait
        ws.settimeout(5)
        while time.time() < end:
            try:
                m = json.loads(ws.recv())
            except Exception:
                continue
            texts = []
            if m.get("method") == "Runtime.consoleAPICalled":
                for a in m.get("params", {}).get("args", []):
                    if isinstance(a.get("value"), str):
                        texts.append(a["value"])
            elif m.get("method") == "Log.entryAdded":
                txt = m.get("params", {}).get("entry", {}).get("text")
                if isinstance(txt, str):
                    texts.append(txt)
            for txt in texts:
                if "HODOSPW" in txt:
                    seen.append(txt)
            if any("worklet" in s for s in seen) and any("page" in s for s in seen):
                break
    finally:
        try:
            ws.close()
        except Exception:
            pass

    def pick(tag):
        for s in seen:
            if tag in s:
                try:
                    return json.loads(s[s.index("{"):])
                except Exception:
                    return {"raw": s}
        return None

    return {"page": pick('"realm":"page"'), "worklet": pick('"realm":"paintworklet"'),
            "raw": seen}


def eval_in_tab(port, excluded, expr, wait=90, await_promise=True):
    t = resolve_tab(port, excluded)
    got = _rpc(t["webSocketDebuggerUrl"], "Runtime.evaluate",
               {"expression": expr, "returnByValue": True,
                "awaitPromise": await_promise, "userGesture": True},
               msg_id=2, wait=wait)
    if not got:
        return None
    res = got.get("result", {}).get("result", {})
    return res.get("value")


def navigate(port, excluded, url, want, timeout=90):
    t = resolve_tab(port, excluded)
    _rpc(t["webSocketDebuggerUrl"], "Page.navigate", {"url": url}, msg_id=1)
    deadline = time.time() + timeout
    while time.time() < deadline:
        time.sleep(1.5)
        t = resolve_tab(port, excluded)
        if want in t.get("url", ""):
            time.sleep(1.5)
            return True
    return False


def run_r15(port, excluded, stamp):
    """example.com -> mark -> example.org -> history.back() -> read marker + probe."""
    if not navigate(port, excluded, URL, HOST):
        return None, "could not reach example.com"
    eval_in_tab(port, excluded, BF_MARK_JS % stamp, await_promise=False)
    if not navigate(port, excluded, AWAY_URL, "example.org"):
        return None, "could not navigate away"
    eval_in_tab(port, excluded, "history.back()", await_promise=False)
    deadline = time.time() + 60
    while time.time() < deadline:
        time.sleep(1.5)
        t = resolve_tab(port, excluded)
        if HOST in t.get("url", ""):
            break
    else:
        return None, "never returned to example.com"
    time.sleep(2.0)
    raw = eval_in_tab(port, excluded, BF_READ_JS, await_promise=False)
    if not raw:
        return None, "could not read back"
    info = json.loads(raw)
    if HOST not in (info.get("href") or ""):
        return None, "wrong subject after back: %s" % info.get("href")
    if info.get("marker") != ("hodos-%s" % stamp):
        # NOT a failure of farbling -- a failure to test it. Report it as such.
        return None, ("bfcache did NOT engage (marker %r lost), so this would have "
                      "measured an ordinary reload" % info.get("marker"))
    val = eval_in_tab(port, excluded, DOM_PROBE, await_promise=True)
    return val, None


def boot(args, port, pdir, farbling_on):
    kill_browser_by_path(args.exe)
    set_site_enabled(pdir, HOST, farbling_on)
    for attempt in range(1, 4):
        if attempt > 1:
            kill_browser_by_path(args.exe)
        launch_browser(args.exe, args.dev, args.profile)
        if wait_for_cdp(port):
            break
    else:
        raise SystemExit("CDP %d never came up" % port)
    time.sleep(args.settle)


def arm(args, port, pdir, farbling_on, label):
    boot(args, port, pdir, farbling_on)
    excluded = snapshot_targets(port, args.settle)
    if not navigate(port, excluded, URL, HOST):
        raise SystemExit("could not reach %s" % URL)
    expr = REALM_JS % {"oc": json.dumps(OC_PROBE), "dom": json.dumps(DOM_PROBE),
                       "url": URL}
    raw = eval_in_tab(port, excluded, expr, wait=180)
    if not raw:
        raise SystemExit("realm probe returned nothing")
    data = json.loads(raw)
    data["paintworklet"] = probe_paint_worklet(port, excluded)
    r15val, r15err = run_r15(port, excluded, "a" if farbling_on else "b")
    data["realms"]["r15_bfcache"] = {"value": r15val, "err": r15err}
    data["engine"] = engine_version(port)
    data["optout"] = optout_state(pdir, HOST)
    print("\n  %s: engine=%s  on-disk opt-out for %s=%s"
          % (label, data["engine"], HOST, data["optout"]))
    return data


def ref_for(arm_data, kind):
    return arm_data.get("refDom") if kind == "dom" else arm_data.get("refOc")


def classify(key, kind, farbled, control):
    """Two-sided verdict against SAME-PROBE references from the SAME arms."""
    fr = (farbled["realms"].get(key) or {})
    cr = (control["realms"].get(key) or {})
    fv, cv = fr.get("value"), cr.get("value")
    if fv is None:
        return "UNREACHABLE", fr.get("err") or "no measurement"
    fref, cref = ref_for(farbled, kind), ref_for(control, kind)
    if not fref or not cref or not fref.get("canvas") or not cref.get("canvas"):
        return "VOID", "reference probe missing for this arm"
    farbled_ref, native_ref = fref["canvas"], cref["canvas"]
    if farbled_ref == native_ref:
        return "VOID", "top frame is not farbled in the farbled arm"
    got = fv.get("canvas")
    if got is None:
        # A realm with no canvas API at all is a real answer about that realm, but it is
        # not "unkeyed" -- it is "cannot read this surface here".
        #
        # ⛔ BUT ONLY IF THE PROBE ACTUALLY RAN. "The API is absent" and "my measurement
        # never arrived" both surface as a missing hash, and treating them alike would
        # turn every harness bug into a clean pass. The discriminator is the `has` map:
        #   has.OffscreenCanvas == False  -> the probe ran and reported the API absent
        #   has.OffscreenCanvas missing   -> nothing ran; `value` is a husk
        # This is not hypothetical. Making the probe async broke exactly this: five DOM
        # realms captured a Promise instead of a result and reported NO-SURFACE with an
        # EMPTY has map, which would otherwise have read as "document.write documents
        # have no canvas" -- an absurd conclusion the harness would have stated
        # confidently. It was caught because `None` and `False` print differently.
        has = fv.get("has")
        if not has or "OffscreenCanvas" not in has:
            return "PROBE-FAILED", ("no result reached the harness (has=%r, err=%r) -- "
                                    "this is a measurement failure, NOT a finding about "
                                    "the realm" % (has, fv.get("err")))
        return "NO-SURFACE", ("MEASURED: realm exposes no readable surface "
                              "(OffscreenCanvas=%s navigator=%s)"
                              % (has.get("OffscreenCanvas"), has.get("navigator")))
    if got == farbled_ref:
        extra = ""
        if cv is not None and cv.get("canvas") not in (None, native_ref):
            extra = "  (control arm: %s vs native %s)" % (cv.get("canvas"), native_ref)
        return "KEYED", "%s == the top frame's farbled value%s" % (got, extra)
    if got == native_ref:
        return "UNKEYED", "%s == native; fails closed, no key delivered here" % got
    return "INVESTIGATE", ("%s matches neither farbled (%s) nor native (%s)"
                           % (got, farbled_ref, native_ref))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--exe", required=True)
    ap.add_argument("--data-root", required=True)
    ap.add_argument("--profile", default="Default")
    ap.add_argument("--dev", action="store_true")
    ap.add_argument("--settle", type=float, default=8.0)
    ap.add_argument("--negative-control", action="store_true",
                    help="both arms farbling-OFF; the top-frame reference must then stop "
                         "differing and every realm verdict must become VOID.")
    args = ap.parse_args()

    port = cdp_port_for(args.profile, args.dev)
    pdir = profile_dir(args.data_root, args.profile)
    original = optout_state(pdir, HOST)

    print("== farbling realm matrix (§A) ==")
    if args.negative_control:
        print("  ⚠️ NEGATIVE CONTROL: both arms farbling-OFF; expecting VOID everywhere")

    try:
        farbled = arm(args, port, pdir, not args.negative_control,
                      "arm 1 (farbled)" if not args.negative_control
                      else "arm 1 (NEG-CTL: farbling off)")
        control = arm(args, port, pdir, False, "arm 2 (control/native)")
    finally:
        kill_browser_by_path(args.exe)
        if original is not None:
            set_site_enabled(pdir, HOST, not original)

    print("\n" + "=" * 78)
    if farbled["engine"] != control["engine"]:
        print("  REFUSED — arms on different engines: %s vs %s"
              % (farbled["engine"], control["engine"]))
        return 2
    print("  [PASS] subject: both arms on %s" % farbled["engine"])

    fd, cd = ref_for(farbled, "dom"), ref_for(control, "dom")
    fo, co = ref_for(farbled, "oc"), ref_for(control, "oc")
    print("  references   dom: farbled=%s native=%s   oc: farbled=%s native=%s"
          % (fd and fd.get("canvas"), cd and cd.get("canvas"),
             fo and fo.get("canvas"), co and co.get("canvas")))

    caps = farbled.get("caps") or {}
    print("  capabilities fencedFrame=%s audioWorklet=%s paintWorklet=%s"
          % (caps.get("fencedFrame"), caps.get("audioWorklet"), caps.get("paintWorklet")))

    results = {k: classify(k, kind, farbled, control) for k, _, kind, _ in REALMS}

    if args.negative_control:
        voided = [k for k, (s, _) in results.items() if s == "VOID"]
        ok = len(voided) == len(REALMS)
        print("\n  NEGATIVE CONTROL: %d/%d realms VOID — %s"
              % (len(voided), len(REALMS),
                 "as required" if ok else "⛔ UNEXPECTED (see rows below)"))
        for key, label, _, _ in REALMS:
            print("    %-40s %s" % (label, results[key][0]))
        return 0 if ok else 2

    print("\n  REALMS")
    print("    %-40s %-13s %-9s %s" % ("§A row", "measured", "expected", "detail"))
    print("    " + "-" * 100)
    unkeyed, unresolved = [], []
    for key, label, kind, expect in REALMS:
        state, detail = results[key]
        flag = " " if state == expect else "*"
        print("  %s %-40s %-13s %-9s %s" % (flag, label, state, expect, detail[:44]))
        if state == "UNKEYED":
            unkeyed.append(label)
        elif state not in ("KEYED", "NO-SURFACE"):
            # NO-SURFACE is a terminal ANSWER (measured: nothing readable lives here), so
            # it is not "unresolved". PROBE-FAILED, NOISY and INVESTIGATE still are.
            unresolved.append((label, state, detail))
    print("\n    (* = measurement disagrees with the documented prior)")

    # Per-vector detail for the OffscreenCanvas realms. `canvas` above is the headline,
    # but a worker reaches THREE surfaces and they are hooked in three different patches
    # -- reporting only the 2D one would let a keyed worker hide an unhooked encoder.
    print("\n  WORKER-REACHABLE VECTORS (against the same-arm 'oc' reference)")
    print("    %-40s %-10s %-10s %s" % ("realm", "canvas2d", "webgl", "convertToBlob"))
    print("    " + "-" * 84)
    for key, label, kind, _ in REALMS:
        if kind != "oc":
            continue
        fv = (farbled["realms"].get(key) or {}).get("value")
        if not fv:
            continue
        cells = []
        for field in ("canvas", "gl", "blob"):
            got = fv.get(field)
            fref = (fo or {}).get(field)
            nref = (co or {}).get(field)
            if got is None:
                cells.append("n/a")
            elif isinstance(got, str) and got.startswith("ERR:"):
                cells.append("ERR")
            elif fref == nref:
                cells.append("void")     # reference itself not farbled; nothing to say
            elif got == fref:
                cells.append("KEYED")
            elif got == nref:
                cells.append("native")
            else:
                cells.append("?%s" % str(got)[:6])
        print("    %-40s %-10s %-10s %s" % (label, cells[0], cells[1], cells[2]))

    pw = caps.get("paintWorkletHas") or {}
    pwc = farbled.get("paintworklet") or {}
    print("\n  R11 PaintWorklet global scope")
    print("     addModule-rejection observable: sanity=%s forced-failure=%s -> %s"
          % (pw.get("sanity"), pw.get("forced"),
             "UNUSABLE (a module that always throws still resolved, so it has no "
             "negative direction)" if pw.get("forced") is not False else "usable"))
    if not pwc.get("page"):
        print("     ⛔ NO VERDICT — the console channel's own PAGE control never arrived,")
        print("        so a missing worklet line proves nothing about the worklet.")
    elif not pwc.get("worklet"):
        print("     ⛔ NO VERDICT — page control captured (%s) but no line from the"
              % pwc.get("page"))
        print("        worklet realm. Console may not be forwarded out of it; that is a")
        print("        limit of the instrument, NOT a measurement of the realm.")
    else:
        w = pwc["worklet"]
        print("     [PASS] console channel proven by the page control: %s" % pwc["page"])
        print("     MEASURED in PaintWorkletGlobalScope: OffscreenCanvas=%s navigator=%s "
              "document=%s" % (w.get("oc"), w.get("nav"), w.get("doc")))
        if not w.get("oc") and not w.get("nav"):
            print("     -> no §B surface is reachable in this realm at all.")

    fen = caps.get("fenced") or {}
    print("\n  R12 fenced frame: element=%s config=%s runAdAuction=%s "
          "sharedStorage=%s selectURL=%s"
          % (fen.get("element"), fen.get("config"), fen.get("runAdAuction"),
             fen.get("sharedStorage"), fen.get("selectURL")))
    if fen.get("element"):
        print("     The container EXISTS in this build, so it is a real realm and stays ❓")
        print("     until measured. Note it is NOT scriptable by the embedder by design,")
        print("     so it is a tracker-visibility question, not a page bypass.")

    if unresolved:
        print("\n  ⚠️ NO VERDICT for %d realm(s) — unmeasured, NOT passed:" % len(unresolved))
        for label, state, detail in unresolved:
            print("     %s: %s — %s" % (label, state, detail))
    if unkeyed:
        print("\n  ⛔ %d UNKEYED REALM(S): %s" % (len(unkeyed), ", ".join(unkeyed)))
        return 1
    if unresolved:
        return 2
    print("\n  ✅ every reachable realm carries the top frame's key")
    return 0


if __name__ == "__main__":
    sys.exit(main())
