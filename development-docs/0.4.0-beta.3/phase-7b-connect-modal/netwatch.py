"""P7b-A1 — watch the notification overlay's network while a consent modal opens.

SUBJECT: the notification overlay browser (…/brc100-auth), attached BY URL.
Hodos runs ~15 CEF browsers that CDP all reports as type:"page"; this project has
already faked a bug by driving the wrong one.
"""
import json, sys, time
from urllib.request import urlopen
import websocket

PORT = 9322
if PORT == 9222: sys.exit("refusing 9222 - that is the installed browser")

def target(sub):
    for t in json.loads(urlopen("http://127.0.0.1:%d/json/list" % PORT, timeout=5).read()):
        if sub in t["url"]:
            return t
    sys.exit("no target matching %r" % sub)

t = target("brc100-auth")
print("target:", t["url"])
ws = websocket.create_connection(t["webSocketDebuggerUrl"], timeout=20)
mid = [0]
def send(method, params=None):
    mid[0] += 1
    ws.send(json.dumps({"id": mid[0], "method": method, "params": params or {}}))
    return mid[0]

send("Network.enable")
time.sleep(0.4)

FIX = ("type=manifest_connect_bundle&domain=github.com&manifest="
       + __import__("urllib.parse", fromlist=["quote"]).quote(json.dumps({
    "name": "Netwatch Fixture", "description": "d", "iconUrl": "", "expiresAt": 0,
    "version": "1.0", "protocols": [{"securityLevel": 1, "name": "p", "keyId": "*",
    "purpose": "Do a thing"}], "baskets": [], "certificates": [],
    "spending": {"perTransactionUsd": 0, "perSessionUsd": 0}, "counterparties": []}), safe=""))
extra = sys.argv[1] if len(sys.argv) > 1 else ""
# ⛔ ASSERT THE TRIGGER FIRED. Without this, an unbound window.showNotification
# yields "0 requests" — which reads exactly like "no leak". That is the vacuous
# green this project keeps producing; it nearly shipped here too.
eid = send("Runtime.evaluate", {"expression":
     "typeof window.showNotification === 'function' ? (window.showNotification(%s), 'SHOWN') : 'NO_HANDLER'"
     % json.dumps(FIX + extra), "returnByValue": True})
trigger = None
tdl = time.time() + 5
ws.settimeout(1.0)
pending = []
while time.time() < tdl and trigger is None:
    try: m = json.loads(ws.recv())
    except Exception: continue
    if m.get("id") == eid:
        r = m.get("result", {}).get("result", {})
        trigger = r.get("value")
        if m.get("result", {}).get("exceptionDetails"): trigger = "EXCEPTION"
    elif m.get("method") == "Network.requestWillBeSent":
        pending.append(m["params"]["request"]["url"])
if trigger != "SHOWN":
    print("TRIGGER FAILED: %r -- this run proves NOTHING. Reload the overlay." % trigger)
    ws.close(); sys.exit(2)
print("trigger: SHOWN")

reqs, deadline = list(pending), time.time() + 4.0
ws.settimeout(1.0)
while time.time() < deadline:
    try:
        msg = json.loads(ws.recv())
    except Exception:
        continue
    if msg.get("method") == "Network.requestWillBeSent":
        reqs.append(msg["params"]["request"]["url"])
ws.close()

ext = [u for u in reqs if not u.startswith("http://127.0.0.1:5137")
       and not u.startswith("data:") and not u.startswith("blob:")]
goog = [u for u in reqs if "google.com" in u or "gstatic.com" in u]
print("total requests observed : %d" % len(reqs))
print("non-local requests      : %d" % len(ext))
for u in ext: print("   ->", u[:110])
print("GOOGLE REQUESTS         : %d  %s" % (len(goog), "<-- LEAK" if goog else "(none)"))
for u in goog: print("   !!", u[:110])
