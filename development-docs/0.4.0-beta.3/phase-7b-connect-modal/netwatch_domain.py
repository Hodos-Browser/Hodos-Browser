"""P7b-A1 (macOS strengthening) — watch the consent modal's network for a domain
whose favicon IS in the local FaviconStore.

Why this exists, and why `netwatch.py` alone was not enough on macOS
-------------------------------------------------------------------
`netwatch.py` opens the modal for `github.com`, which has no row in
`favicons.db`. The modal correctly renders its letter tile, so "0 Google
requests" is consistent with two very different worlds:

  a) the de-Googling worked and the local store served (or correctly declined); or
  b) the favicon path is dead and nothing ever asked anyone for an icon.

macOS was demonstrably in world (b) until 2026-09-08, when `FaviconStore` was
found to have never been initialised here at all (MAC_RELAY_BETA3 round
2026-09-08 §B). So the green had to be re-earned with a subject that forces the
path to run.

This script opens the modal for a domain that HAS a stored icon. Two assertions,
both required:
  1. ⛔ the trigger fired AND the modal rendered the fixture's own text
     (`window.showNotification` existing is not the same as a mounted modal —
     that vacuous green cost this phase three runs on Windows);
  2. ⭐ the rendered <img> is a `data:` URI whose decoded byte count equals the
     `length(png)` of that host's row in `favicons.db`.

With (2) satisfied, "0 requests to google.com/gstatic.com" is a real absence:
the icon was displayed, and it did not come off the network.

⚠️ WHAT THE FIRST macOS RUN ACTUALLY SHOWED (2026-09-09), recorded so the next
reader does not mistake this script for more than it is. Assertion (2) came back
`dataUriCount: 0` for `www.google.com` — NOT because the store was dead, but
because a `showNotification(...)` fixture built here supplies no `favicon=`
param, and the consent modal takes its icon from that param alone
(`BRC100AuthOverlayRoot.tsx:566`, `:1525`), never from `favicon_get`. So the
modal correctly drew its letter tile. ⇒ This script proves the *network* half
(assertion 1 + zero third-party requests). It does NOT prove a store hit on the
consent surface, and it cannot, because that surface does not read the store.
The store-hit control lives on the new tab instead (`netwatch_page.py newtab`
plus a byte-count check against `favicons.db`), which does use `favicon_get`.

⛔ ⭐ UPDATE, same day: the finding this note recorded — that the param C++ builds
was a REMOTE url and the overlay issued a request for it — has been **FIXED**.
`FaviconParamForDomain` now emits the stored bytes as a `data:` URI. ⇒ For this
row use **`consent_favicon_probe.py`**, which triggers a real permission prompt
so the param comes from C++ rather than from the harness. This script remains
useful only for the React half (does the overlay fetch what it is handed).

Usage:  netwatch_domain.py <domain> [expected_png_bytes]
"""
import json, sys, time
from urllib.parse import quote
from urllib.request import urlopen
import websocket

PORT = 9322
if PORT == 9222:
    sys.exit("refusing 9222 - that is the installed browser")

domain = sys.argv[1] if len(sys.argv) > 1 else "www.google.com"
expect = int(sys.argv[2]) if len(sys.argv) > 2 else None

t = None
for x in json.loads(urlopen("http://127.0.0.1:%d/json/list" % PORT, timeout=5).read()):
    if "brc100-auth" in x["url"]:
        t = x
        break
if not t:
    sys.exit("no brc100-auth target — create the overlay first, e.g. send "
             "open_wallet_permissions from the header browser")
print("target:", t["url"])

MANIFEST = {"name": "Netwatch Fixture (%s)" % domain, "description": "d", "iconUrl": "",
            "expiresAt": 0, "version": "1.0",
            "protocols": [{"securityLevel": 1, "name": "p", "keyId": "*",
                           "purpose": "Do a thing"}],
            "baskets": [], "certificates": [],
            "spending": {"perTransactionUsd": 0, "perSessionUsd": 0},
            "counterparties": []}
FIX = ("type=manifest_connect_bundle&domain=" + domain
       + "&manifest=" + quote(json.dumps(MANIFEST), safe=""))

ws = websocket.create_connection(t["webSocketDebuggerUrl"], timeout=20)
mid = [0]
def send(method, params=None):
    mid[0] += 1
    ws.send(json.dumps({"id": mid[0], "method": method, "params": params or {}}))
    return mid[0]

send("Network.enable")
time.sleep(0.4)

eid = send("Runtime.evaluate", {"returnByValue": True, "expression":
    "typeof window.showNotification === 'function' "
    "? (window.showNotification(%s), 'SHOWN') : 'NO_HANDLER'" % json.dumps(FIX)})

trigger, pending = None, []
ws.settimeout(1.0)
deadline = time.time() + 5
while time.time() < deadline and trigger is None:
    try:
        m = json.loads(ws.recv())
    except Exception:
        continue
    if m.get("id") == eid:
        trigger = m.get("result", {}).get("result", {}).get("value")
        if m.get("result", {}).get("exceptionDetails"):
            trigger = "EXCEPTION"
    elif m.get("method") == "Network.requestWillBeSent":
        pending.append(m["params"]["request"]["url"])
if trigger != "SHOWN":
    print("TRIGGER FAILED: %r -- this run proves NOTHING." % trigger)
    ws.close(); sys.exit(2)
print("trigger: SHOWN")

reqs = list(pending)
deadline = time.time() + 4.0
while time.time() < deadline:
    try:
        m = json.loads(ws.recv())
    except Exception:
        continue
    if m.get("method") == "Network.requestWillBeSent":
        reqs.append(m["params"]["request"]["url"])

# ⛔ The subject assertion. A modal that did not mount makes zero requests too.
rid = send("Runtime.evaluate", {"returnByValue": True, "awaitPromise": True, "expression": """(() => {
  const imgs = [...document.querySelectorAll('img')];
  const data = imgs.map(i => i.src || '').filter(s => s.startsWith('data:'));
  const b64  = (data[0] || '').split(',')[1] || '';
  const pad  = b64.endsWith('==') ? 2 : b64.endsWith('=') ? 1 : 0;
  return { text: document.body.innerText.slice(0, 300),
           imgCount: imgs.length,
           dataUriCount: data.length,
           pngBytes: b64 ? Math.floor(b64.length * 3 / 4) - pad : 0,
           httpImgs: imgs.map(i => i.src || '').filter(s => s.startsWith('http')) };
})()"""})
dom = None
ws.settimeout(10)
while dom is None:
    m = json.loads(ws.recv())
    if m.get("id") == rid:
        dom = m.get("result", {}).get("result", {}).get("value")
ws.close()

mounted = "Netwatch Fixture" in (dom.get("text") or "")
print("modal mounted (fixture text present): %s" % mounted)
print("imgs: %d  (data: URIs %d, decoded %d bytes)  http imgs: %s"
      % (dom["imgCount"], dom["dataUriCount"], dom["pngBytes"], dom["httpImgs"] or "none"))
if expect is not None:
    print("store row length(png) = %d  -> %s"
          % (expect, "MATCH" if dom["pngBytes"] == expect else "MISMATCH"))

ext  = [u for u in reqs if not u.startswith("http://127.0.0.1:5137")
        and not u.startswith("data:") and not u.startswith("blob:")]
goog = [u for u in reqs if "google.com" in u or "gstatic.com" in u
        or "duckduckgo.com" in u]
print("total requests observed : %d" % len(reqs))
print("non-local requests      : %d" % len(ext))
for u in ext:
    print("   ->", u[:110])
print("THIRD-PARTY FAVICON     : %d  %s" % (len(goog), "<-- LEAK" if goog else "(none)"))
for u in goog:
    print("   !!", u[:110])
if not mounted:
    print("⛔ VACUOUS — the modal did not render the fixture. Ignore the zeros above.")
    sys.exit(2)
