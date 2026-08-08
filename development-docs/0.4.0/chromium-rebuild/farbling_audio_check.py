#!/usr/bin/env python3
"""Measure the LEGACY JS farbling surface (audio + WebGL) exempt-vs-farbled.

This is the vector the fail-closed fix targets. Canvas cannot be used -- its JS
fragment was deleted in C3 and it is now farbled natively, so canvas would show a
difference that says nothing about the JS path.

Drives an EXISTING CEF tab via Page.navigate. Never CDP /json/new: those targets
bypass OnBeforeBrowse, so they would never receive a seed and would report
"fail-closed" no matter what the code did.

BEFORE the fix: farbled != exempt, because of the std::hash(url) constant.
AFTER  the fix: farbled == exempt, i.e. no perturbation at all (fail closed).
"""
import json
import sys
import time
import urllib.request

import websocket

PORT = int(sys.argv[1]) if len(sys.argv) > 1 else 9322
PAGES = [("exempt  (github.com)", "https://github.com/"),
         ("farbled (example.com)", "https://example.com/")]

# Render a tone through a DynamicsCompressor and hash the samples; then hash a WebGL
# readPixels. Both are perturbed by FINGERPRINT_PROTECTION_SCRIPT when it is injected.
JS = r"""
(async () => {
  function h(a){let x=0x811c9dc5;for(let i=0;i<a.length;i++){
    x^=Math.round(a[i]*1e6)|0;x=(x*0x01000193)>>>0;}return x.toString(16).padStart(8,'0');}
  let audio='ERR';
  try{
    const ctx=new OfflineAudioContext(1,44100,44100);
    const osc=ctx.createOscillator();osc.type='triangle';osc.frequency.value=10000;
    const comp=ctx.createDynamicsCompressor();
    osc.connect(comp);comp.connect(ctx.destination);osc.start(0);
    const buf=await ctx.startRendering();
    audio=h(buf.getChannelData(0).slice(4000,9000));
  }catch(e){audio='ERR:'+e.message;}
  let webgl='ERR';
  try{
    const c=document.createElement('canvas');c.width=32;c.height=32;
    const gl=c.getContext('webgl');
    gl.clearColor(0.25,0.5,0.75,1);gl.clear(gl.COLOR_BUFFER_BIT);
    const px=new Uint8Array(32*32*4);gl.readPixels(0,0,32,32,gl.RGBA,gl.UNSIGNED_BYTE,px);
    webgl=h(px);
  }catch(e){webgl='ERR:'+e.message;}
  const audioNative=(''+AudioBuffer.prototype.getChannelData).includes('[native code]');
  return JSON.stringify({audio,webgl,audioNative});
})()
"""


def targets():
    with urllib.request.urlopen(f"http://127.0.0.1:{PORT}/json/list", timeout=10) as r:
        return json.load(r)


def web_tab():
    for t in targets():
        if t.get("type") == "page" and "127.0.0.1:5137" not in t.get("url", ""):
            return t
    for t in targets():
        if t.get("type") == "page":
            return t
    raise SystemExit("no CDP page target")


def measure(url):
    tab = web_tab()
    ws = websocket.create_connection(tab["webSocketDebuggerUrl"], timeout=30)
    ws.send(json.dumps({"id": 1, "method": "Page.navigate", "params": {"url": url}}))
    time.sleep(8)
    ws.close()
    tab = web_tab()
    ws = websocket.create_connection(tab["webSocketDebuggerUrl"], timeout=30)
    ws.send(json.dumps({"id": 2, "method": "Runtime.evaluate",
                        "params": {"expression": JS, "returnByValue": True,
                                   "awaitPromise": True}}))
    deadline = time.time() + 40
    while time.time() < deadline:
        m = json.loads(ws.recv())
        if m.get("id") == 2:
            ws.close()
            return json.loads(m["result"]["result"]["value"])
    raise SystemExit("timeout")


res = {}
for label, url in PAGES:
    v = measure(url)
    res[label] = v
    print(f"{label:24} audio={v['audio']}  webgl={v['webgl']}  "
          f"getChannelData native={v['audioNative']}")

a, b = list(res.values())
print("=" * 78)
same_audio = a["audio"] == b["audio"]
same_webgl = a["webgl"] == b["webgl"]
print(f"audio identical exempt-vs-farbled: {same_audio}")
print(f"webgl identical exempt-vs-farbled: {same_webgl}")
print()
if same_audio and same_webgl:
    print("=> FAIL-CLOSED CONFIRMED: no JS perturbation is applied. The constant is gone.")
else:
    print("=> JS farbling IS still being applied. If the seed is genuinely arriving that is")
    print("   correct behaviour; if not, a constant fallback survives somewhere. Investigate.")
