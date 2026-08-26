"""P2a-A5 (anti-fake): the balance answer must still ARRIVE.

"No freeze" is trivially satisfied by never replying, so A2 alone is not enough.
"""
import json, sys, time, urllib.request
sys.path.insert(0, r"C:\Users\archb\Hodos-Browser\development-docs\0.4.0-beta.3\phase-1-overlay-input-dpi")
import websocket, cdp

def ctl(h):
    urllib.request.urlopen("http://127.0.0.1:31401/__ctl?hang=%d" % (1 if h else 0), timeout=30).read()

def call(label):
    t = cdp.pick("=http://127.0.0.1:5137/")
    ws = websocket.create_connection(t["webSocketDebuggerUrl"], timeout=60); ws.settimeout(60)
    try:
        start = time.time()
        ws.send(json.dumps({"id": 1, "method": "Runtime.evaluate", "params": {
            "expression": "window.hodosBrowser.wallet.getBalance()"
                          ".then(r=>'OK:'+JSON.stringify(r)).catch(e=>'ERR:'+e.message)",
            "awaitPromise": True, "returnByValue": True}}))
        while True:
            m = json.loads(ws.recv())
            if m.get("id") == 1:
                v = m.get("result", {}).get("result", {}).get("value")
                print("%-26s %6.2fs  %s" % (label, time.time() - start, str(v)[:90]))
                return v
    finally:
        ws.close()

if __name__ == "__main__":
    ctl(False); time.sleep(0.5)
    a = call("wallet answering")
    ctl(True); time.sleep(0.5)
    b = call("wallet hung")
    ctl(False); time.sleep(1.5)
    c = call("wallet answering again")
    print(json.dumps({"answered": str(a)[:60], "hung": str(b)[:60], "recovered": str(c)[:60]}))
