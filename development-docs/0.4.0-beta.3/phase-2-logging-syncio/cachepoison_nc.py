"""Negative control for the cache-poison test: show the defect shape is REAL.

Writes the cache the way the pre-fix poller did -- setCachedBalance(undefined) -- and reads
it back the way useBalance does. If this shows a confident 0, the GREEN result is not blind.
Restores the good value afterwards.
"""
import json, sys
sys.path.insert(0, r"C:\Users\archb\Hodos-Browser\development-docs\0.4.0-beta.3\phase-1-overlay-input-dpi")
import websocket, cdp

JS = r"""(function(){
  var K='hodos:wallet:balance';
  var good=localStorage.getItem(K);
  // exactly what the pre-fix poller did on a failed fetch
  localStorage.setItem(K, JSON.stringify({balance: undefined, updatedAt: Date.now()}));
  var raw=localStorage.getItem(K);
  var parsed=JSON.parse(raw);
  var asRead=(parsed && parsed.balance) ?? 0;   // useBalance: cachedBal?.balance ?? 0
  localStorage.setItem(K, good);                 // restore
  return JSON.stringify({written:raw, balanceKeyPresent:('balance' in parsed), readsBackAs:asRead});
})()"""

t = cdp.pick("=http://127.0.0.1:5137/")
ws = websocket.create_connection(t["webSocketDebuggerUrl"], timeout=60); ws.settimeout(60)
ws.send(json.dumps({"id": 1, "method": "Runtime.evaluate",
                    "params": {"expression": JS, "returnByValue": True}}))
while True:
    m = json.loads(ws.recv())
    if m.get("id") == 1:
        print(m.get("result", {}).get("result", {}).get("value"))
        break
ws.close()
