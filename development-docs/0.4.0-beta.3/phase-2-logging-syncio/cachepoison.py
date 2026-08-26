"""P2a-A6: a wallet outage must not poison the cached balance.

Pre-fix, every failed poll ran setCachedBalance(undefined); JSON.stringify DROPS an
undefined value, so the entry read back as a confident zero.
"""
import json, sys, time, urllib.request
sys.path.insert(0, r"C:\Users\archb\Hodos-Browser\development-docs\0.4.0-beta.3\phase-1-overlay-input-dpi")
import websocket, cdp

def ctl(h):
    urllib.request.urlopen("http://127.0.0.1:31401/__ctl?hang=%d" % (1 if h else 0), timeout=30).read()

def read_cache(label):
    t = cdp.pick("=http://127.0.0.1:5137/")
    ws = websocket.create_connection(t["webSocketDebuggerUrl"], timeout=60); ws.settimeout(60)
    try:
        ws.send(json.dumps({"id": 1, "method": "Runtime.evaluate", "params": {
            "expression": "localStorage.getItem('hodos_balance_cache')"
                          "|| localStorage.getItem('hodos.balance')"
                          "|| JSON.stringify(Object.fromEntries(Object.entries(localStorage)"
                          ".filter(([k])=>/bal/i.test(k))))",
            "returnByValue": True}}))
        while True:
            m = json.loads(ws.recv())
            if m.get("id") == 1:
                v = m.get("result", {}).get("result", {}).get("value")
                print("%-28s %s" % (label, str(v)[:120]))
                return v
    finally:
        ws.close()

ctl(False); time.sleep(2)
before = read_cache("before (wallet healthy)")
ctl(True)
print("   ... wallet hung for 75 s (>2 poll cycles at 30 s) ...")
time.sleep(75)
during = read_cache("during outage")
ctl(False); time.sleep(3)
after = read_cache("after recovery")
print(json.dumps({"poisoned": before != during, "before": str(before)[:80],
                  "during": str(during)[:80]}))
