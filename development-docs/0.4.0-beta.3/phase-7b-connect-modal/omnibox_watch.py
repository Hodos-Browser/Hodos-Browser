"""P7b-A2 (omnibox) — watch the omnibox overlay's network while a HUMAN types.

⛔ WHY A HUMAN. Driving this synthetically produced a vacuous green: the overlay
was created by IPC but never shown, so it rendered no rows and no icons, and
"0 requests" was indistinguishable from a pass. The subject has to actually be
drawing suggestions for the absence of a request to mean anything — so this
script records what rendered ALONGSIDE what was requested, and says plainly when
the run proves nothing.

Usage:  py omnibox_watch.py [seconds]      (default 90)
Then:   click the address bar and type a few letters that match your history.
"""
import json, sys, time
from urllib.request import urlopen
import websocket

PORT = 9322
if PORT == 9222:
    sys.exit("refusing 9222 - that is the installed browser")
WINDOW = int(sys.argv[1]) if len(sys.argv) > 1 else 90


def omnibox():
    for t in json.loads(urlopen("http://127.0.0.1:%d/json/list" % PORT, timeout=5).read()):
        if t["url"].endswith("/omnibox"):
            return t
    return None


t = omnibox()
if not t:
    sys.exit("no omnibox target yet — focus the address bar once, then re-run")
print("target:", t["url"])
print("watching %d s — type in the address bar now\n" % WINDOW)

ws = websocket.create_connection(t["webSocketDebuggerUrl"], timeout=20)
mid = [0]


def send(method, params=None):
    mid[0] += 1
    ws.send(json.dumps({"id": mid[0], "method": method, "params": params or {}}))
    return mid[0]


send("Network.enable")
reqs, peak_rows, peak_imgs, samples = [], 0, 0, 0
end = time.time() + WINDOW
ws.settimeout(0.5)
last_poll = 0.0

DOM = ("(function(){var i=document.querySelectorAll('img');var n=0;"
       "for(var k=0;k<i.length;k++){var s=i[k].getAttribute('src')||'';"
       "if(s.indexOf('data:')===0)n++;}"
       "return JSON.stringify({rows:document.querySelectorAll('li').length,"
       "dataUriIcons:n,len:document.body.innerText.length});})()")

while time.time() < end:
    try:
        m = json.loads(ws.recv())
        if m.get("method") == "Network.requestWillBeSent":
            reqs.append(m["params"]["request"]["url"])
    except Exception:
        pass
    if time.time() - last_poll > 0.6:
        last_poll = time.time()
        send("Runtime.evaluate", {"expression": DOM, "returnByValue": True})
        try:
            while True:
                m = json.loads(ws.recv())
                if m.get("method") == "Network.requestWillBeSent":
                    reqs.append(m["params"]["request"]["url"])
                r = m.get("result", {}).get("result", {}).get("value")
                if isinstance(r, str) and r.startswith("{"):
                    d = json.loads(r)
                    peak_rows = max(peak_rows, d["rows"])
                    peak_imgs = max(peak_imgs, d["dataUriIcons"])
                    samples += 1
                    break
        except Exception:
            pass
ws.close()

ext = [u for u in reqs if not u.startswith("http://127.0.0.1:5137") and not u.startswith("data:")]
third = [u for u in reqs if any(h in u for h in ("google.com", "gstatic.com", "duckduckgo.com"))]

print("suggestion rows seen (peak) : %d" % peak_rows)
print("data: URI icons seen (peak) : %d" % peak_imgs)
print("requests observed           : %d total, %d non-local" % (len(reqs), len(ext)))
for u in ext[:10]:
    print("   ->", u[:100])
print("THIRD-PARTY FAVICON HOSTS   : %d %s" % (len(third), "<-- LEAK" if third else "(none)"))

if peak_rows == 0 and peak_imgs == 0:
    print("\n⛔ VACUOUS: the omnibox never rendered a suggestion during this window.")
    print("   'no requests' therefore proves nothing. Re-run and type while it watches.")
    sys.exit(2)
print("\n✅ Non-vacuous: the omnibox WAS drawing suggestions (%d rows, %d local icons)"
      % (peak_rows, peak_imgs))
print("   so the absence of a third-party request is a real result.")
