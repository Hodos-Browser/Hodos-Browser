"""P2a-A4: a slow but legitimate /transaction/send must still succeed.

The stub delays the send by 8 s -- longer than kWalletDefaultTimeoutMs (5 s), shorter than
kWalletBroadcastTimeoutMs (30 s). If A1 had capped every endpoint with one global constant
this would fail. No money moves: the stub returns a canned txid.
"""
import json, sys, time
sys.path.insert(0, r"C:\Users\archb\Hodos-Browser\development-docs\0.4.0-beta.3\phase-1-overlay-input-dpi")
import websocket, cdp

t = cdp.pick("=http://127.0.0.1:5137/")
ws = websocket.create_connection(t["webSocketDebuggerUrl"], timeout=120); ws.settimeout(120)
start = time.time()
ws.send(json.dumps({"id": 1, "method": "Runtime.evaluate", "params": {
    "expression": "window.hodosBrowser.wallet.sendTransaction({toAddress:'1TestOnlyStubNeverBroadcasts',"
                  "amount:1,note:'P2a-A4'})"
                  ".then(r=>'OK:'+JSON.stringify(r)).catch(e=>'ERR:'+e.message)",
    "awaitPromise": True, "returnByValue": True}}))
while True:
    m = json.loads(ws.recv())
    if m.get("id") == 1:
        v = m.get("result", {}).get("result", {}).get("value")
        print("send elapsed=%.2fs -> %s" % (time.time() - start, str(v)[:120]))
        break
ws.close()
