"""Watch any Hodos surface's network across a reload. Usage: netwatch_page.py <url-substr>"""
import json, sys, time
from urllib.request import urlopen
import websocket
PORT = 9322
if PORT == 9222: sys.exit("refusing 9222")
sub = sys.argv[1]
t = [x for x in json.loads(urlopen("http://127.0.0.1:%d/json/list" % PORT, timeout=5).read())
     if sub in x["url"]]
if not t: sys.exit("no target matching %r" % sub)
t = t[0]
print("target:", t["url"])
ws = websocket.create_connection(t["webSocketDebuggerUrl"], timeout=20); i = [0]
def s(m, p=None):
    i[0] += 1; ws.send(json.dumps({"id": i[0], "method": m, "params": p or {}})); return i[0]
s("Network.enable"); s("Page.enable"); time.sleep(0.3)
s("Page.reload", {"ignoreCache": True})
reqs, end = [], time.time() + 7
ws.settimeout(1.0)
loaded = False
while time.time() < end:
    try: m = json.loads(ws.recv())
    except Exception: continue
    if m.get("method") == "Network.requestWillBeSent":
        reqs.append(m["params"]["request"]["url"])
    elif m.get("method") == "Page.loadEventFired":
        loaded = True
ws.close()
if not loaded:
    print("⚠️  no load event seen — treat this run as unproven")
ext = [u for u in reqs if not u.startswith("http://127.0.0.1:5137")
       and not u.startswith("data:") and not u.startswith("blob:")]
third = [u for u in reqs if "google.com" in u or "gstatic.com" in u or "duckduckgo.com" in u]
print("requests: %d total, %d non-local" % (len(reqs), len(ext)))
for u in ext[:10]: print("   ->", u[:100])
print("THIRD-PARTY FAVICON HOSTS: %d %s" % (len(third), "<-- LEAK" if third else "(none)"))
for u in third[:6]: print("   !!", u[:100])
