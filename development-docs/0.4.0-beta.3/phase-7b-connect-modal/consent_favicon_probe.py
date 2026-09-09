"""P7b-A1 — does a REAL consent prompt fetch its icon over the network?

⭐ WHY THIS EXISTS AND netwatch.py DOES NOT COVER IT.
`netwatch.py` / `netwatch_domain.py` drive `window.showNotification(...)` with a
hand-built query string. That exercises the React half, but the `favicon=` param
is whatever the harness typed — so it can never test what **C++ actually puts
there**, which is the entire subject of this row. This script triggers a real
Chromium permission prompt, so the param is built by
`HttpRequestInterceptor.cpp :: FaviconParamForDomain` on the live path.

🚨 THE DEFECT IT CAUGHT (2026-09-09, macOS). The param used to be the site's own
*remote* icon URL (`Tab::favicon_url`), which the overlay renders straight into
`<img src>`. For any site whose icon is off-host that is a third-party request at
the instant of the decision — measured on github.com, whose icon lives on
`github.githubassets.com`. Fixed by emitting the STORED BYTES as a `data:` URI
from `FaviconStore`.

WHAT IT ASSERTS, and why each one is needed:
  1. the prompt actually opened (a `brc100-auth` target exists) — a modal that
     never rendered makes zero requests and reads exactly like success;
  2. the modal is really on screen (its text names the site);
  3. ⭐ the rendered `<img>` is a `data:` URI whose decoded byte count equals
     `length(png)` for that host in `favicons.db` — the icon is displayed AND it
     came from disk;
  4. zero non-local requests across a hard reload of the overlay.

⛔ Use the NON-LOCAL count, not a substring match on the leaking host. Under the
old code the overlay's OWN document URL contained the third-party address inside
`?favicon=`, so a substring filter reports 2 for 1 real request. That is a
harness artifact, not a second leak.

NEGATIVE CONTROL (run 2026-09-09, both directions, same machine):
  - fixed build   → `&favicon=data:…`, 2364-byte data URI, **0** non-local
  - reverted line → `&favicon=https://github.githubassets.com/...`, **1**
    non-local request to githubassets.com, 0 data URIs
  - store MISS (example.com, no row) → no param at all, letter tile "E",
    0 broken images, 0 non-local

⚠️ TWO TRAPS that cost time here:
  - an UNANSWERED prompt keeps `PendingPermissionManager` occupied, and the next
    `FireHodosPermissionPrompt` returns false **silently** (it defers to
    Chromium's own UI). `OnShowPermissionPrompt` still logs, so the log looks
    fine while no overlay appears. Restart the browser, or answer the prompt,
    between subjects.
  - the site must be **https** — geolocation is refused on an insecure origin,
    and the refusal is silent from CDP's point of view.

Usage:  consent_favicon_probe.py <https-site> [expected_png_bytes]
        e.g.  consent_favicon_probe.py https://github.com 2364
"""
import json, sys, time
from urllib.request import urlopen
import websocket

PORT = 9322
if PORT == 9222:
    sys.exit("refusing 9222 - that is the installed browser")

site = sys.argv[1] if len(sys.argv) > 1 else "https://github.com"
expect = int(sys.argv[2]) if len(sys.argv) > 2 else None


def targets():
    return json.loads(urlopen("http://127.0.0.1:%d/json/list" % PORT, timeout=5).read())


def one(pred, what):
    hits = [t for t in targets() if pred(t)]
    if not hits:
        sys.exit("no target: %s" % what)
    return hits[0]


def evaluate(ws, expr, mid):
    ws.send(json.dumps({"id": mid, "method": "Runtime.evaluate",
                        "params": {"expression": expr, "returnByValue": True}}))
    ws.settimeout(20)
    while True:
        m = json.loads(ws.recv())
        if m.get("id") == mid:
            return m.get("result", {}).get("result", {}).get("value")


# 1. put a tab on the subject site so OnFaviconURLChange populates the store
tab = one(lambda t: t["url"].startswith("http") and "brc100-auth" not in t["url"], "any tab")
ws = websocket.create_connection(tab["webSocketDebuggerUrl"], timeout=20)
ws.send(json.dumps({"id": 1, "method": "Page.navigate", "params": {"url": site}}))
ws.settimeout(20)
while True:
    if json.loads(ws.recv()).get("id") == 1:
        break
ws.close()
time.sleep(10)   # favicon download is async; the store write lands after load

# 2. trigger a real permission prompt on it
host_pref = site.split("//", 1)[1].rstrip("/")
tab = one(lambda t: host_pref in t["url"], site)
ws = websocket.create_connection(tab["webSocketDebuggerUrl"], timeout=20)
print("trigger:", evaluate(
    ws, "navigator.geolocation.getCurrentPosition(()=>{},()=>{}); 'REQUESTED'", 1))
ws.close()
time.sleep(4)

# 3. ⛔ assertion 1 — the prompt opened at all
ov = [t for t in targets() if "brc100-auth" in t["url"]]
if not ov:
    print("⛔ NO OVERLAY — the prompt never opened, so this run proves NOTHING.")
    print("   Most likely an earlier prompt is still pending. Restart the browser.")
    sys.exit(2)
url = ov[0]["url"]
has = "&favicon=" in url
print("overlay URL length : %d" % len(url))
print("favicon param      : %s" % ("data: URI" if "&favicon=data%3A" in url
                                   else "REMOTE URL <-- LEAK" if has else "absent (store miss)"))

# 4. watch the network across a hard reload, then read the DOM
ws = websocket.create_connection(ov[0]["webSocketDebuggerUrl"], timeout=20)
mid = [1]
def send(method, params=None):
    mid[0] += 1
    ws.send(json.dumps({"id": mid[0], "method": method, "params": params or {}}))
send("Network.enable"); send("Page.enable"); time.sleep(0.4)
send("Page.reload", {"ignoreCache": True})
reqs, loaded, end = [], False, time.time() + 8
ws.settimeout(1.0)
while time.time() < end:
    try:
        m = json.loads(ws.recv())
    except Exception:
        continue
    if m.get("method") == "Network.requestWillBeSent":
        reqs.append(m["params"]["request"]["url"])
    elif m.get("method") == "Page.loadEventFired":
        loaded = True

mid[0] += 1
dom = json.loads(evaluate(ws, """JSON.stringify((()=>{
  const imgs=[...document.querySelectorAll('img')];
  const d=imgs.map(x=>x.src||'').filter(s=>s.startsWith('data:'));
  const b64=(d[0]||'').split(',')[1]||'';
  const pad=b64.endsWith('==')?2:b64.endsWith('=')?1:0;
  return {text:document.body.innerText.slice(0,120),
          dataUris:d.length,
          pngBytes: b64 ? Math.floor(b64.length*3/4)-pad : 0,
          httpImgs: imgs.map(x=>x.src||'').filter(s=>s.startsWith('http')),
          brokenImgs: imgs.filter(x=>x.complete && x.naturalWidth===0).length};})())""", mid[0]))
ws.close()

# ⛔ assertion 2 — the modal is on screen. Zeros from a blank page mean nothing.
mounted = host_pref.split("/")[0] in (dom.get("text") or "")
print("reload load event  : %s" % loaded)
print("modal mounted      : %s   %r" % (mounted, (dom.get("text") or "")[:70]))
print("data URIs / bytes  : %d / %d   broken imgs: %d"
      % (dom["dataUris"], dom["pngBytes"], dom["brokenImgs"]))
if dom["httpImgs"]:
    print("http img srcs      : %s" % [s[:70] for s in dom["httpImgs"]])
if expect is not None:
    print("favicons.db length(png) = %d -> %s"
          % (expect, "MATCH" if dom["pngBytes"] == expect else "MISMATCH"))

ext = [u for u in reqs if not u.startswith("http://127.0.0.1:5137")
       and not u.startswith("data:") and not u.startswith("blob:")]
print("requests: %d total, %d NON-LOCAL  %s" % (len(reqs), len(ext), "<-- LEAK" if ext else ""))
for u in ext:
    print("   ->", u[:110])

if not (loaded and mounted):
    print("⛔ VACUOUS — ignore the counts above.")
    sys.exit(2)
sys.exit(1 if ext else 0)
