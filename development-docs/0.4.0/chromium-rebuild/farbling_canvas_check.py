#!/usr/bin/env python3
"""Native-canvas farbling check that drives the REAL TAB, not browser chrome.

## THREE HARNESS DEFECTS THIS FILE EXISTS TO AVOID — all three actually happened

1. **CDP `PUT /json/new` tabs bypass `OnBeforeBrowse`**, so they never receive a
   farbling key and fail against a *correct* implementation. Never create targets.
2. **Measuring the previous page.** Navigate-then-sleep-then-measure can read the old
   document; because a fixed synthetic pattern is drawn, both pages' NATIVE hashes are
   identical, so "farbling didn't run" and "measured the wrong page" look the same.
   Fixed by asserting `location.href` at measurement time.
3. **Measuring the wrong BROWSER.** ⭐ This one produced a fake "intermittent per
   session" bug that cost hours. Hodos's header and ~14 overlays are *separate CEF
   browsers* served from `127.0.0.1:5137`, and CDP reports every one of them as
   `type: "page"`. Picking "the first page target" therefore sometimes selected the
   **tab-list overlay** (`role: tablistpanel`) instead of the tab — and once that
   overlay had been navigated to example.com, `location.href` was example.com, so
   defect-2's assertion passed happily. Overlays legitimately get no farbling key, so
   the result read as "farbling is broken", and which browser got picked varied per
   launch, which read as "intermittent".

   ⛔ **Never identify the tab by "not 127.0.0.1:5137".** After the first navigation an
   overlay no longer matches that either. Identify browser chrome ONCE at startup, by
   target id, and exclude those ids forever.
"""
import json
import sys
import time
import urllib.request

import websocket

PORT = int(sys.argv[1]) if len(sys.argv) > 1 else 9322
REPEATS = int(sys.argv[2]) if len(sys.argv) > 2 else 3

JS = r"""
(() => {
  const c=document.createElement('canvas');c.width=100;c.height=100;
  const x=c.getContext('2d');
  x.fillStyle='#f60';x.fillRect(0,0,100,100);
  x.fillStyle='#0af';x.fillRect(10,10,50,50);
  x.font='16px serif';x.fillStyle='#333';x.fillText('hodos',5,90);
  const d=x.getImageData(0,0,100,100).data;
  let h=0x811c9dc5;for(let i=0;i<d.length;i++){h^=d[i];h=(h*0x01000193)>>>0;}
  return JSON.stringify({href:location.href,hash:h.toString(16).padStart(8,'0')});
})()
"""


def targets():
    with urllib.request.urlopen(f"http://127.0.0.1:{PORT}/json/list", timeout=10) as r:
        return [t for t in json.load(r) if t.get("type") == "page"]


# --- Identify browser chrome ONCE, by id, before anything navigates. -------------
# The tab is the 5137 target serving /newtab; the header ("/") and every overlay
# ("/tab-list", "/menu", "/wallet-panel", "/downloads", "/privacy-shield",
# "/brc100-auth", "/profile-picker", "/site-info", "/bookmarks", ...) are chrome.
chrome_ids, tab_id = set(), None
for t in targets():
    url = t.get("url", "")
    if "127.0.0.1:5137" in url:
        if "/newtab" in url:
            tab_id = t["id"]
        else:
            chrome_ids.add(t["id"])

if tab_id is None:
    raise SystemExit("could not find the tab (no 127.0.0.1:5137/newtab target). "
                     "Restart the browser so it opens a fresh NTP tab.")
print(f"tab target={tab_id[:16]}…   excluded {len(chrome_ids)} chrome target(s) "
      f"(header + overlays)")


def tab_target():
    """The page target that is NOT known browser chrome."""
    cands = [t for t in targets() if t["id"] not in chrome_ids]
    for t in cands:
        if t["id"] == tab_id:
            return t
    # A cross-site navigation can swap the target; take the sole non-chrome one.
    if len(cands) == 1:
        return cands[0]
    for t in cands:
        if "127.0.0.1:5137" not in t.get("url", ""):
            return t
    raise SystemExit(f"ambiguous tab target among {len(cands)} candidates")


def measure(url, want_host):
    t = tab_target()
    ws = websocket.create_connection(t["webSocketDebuggerUrl"], timeout=30)
    ws.send(json.dumps({"id": 1, "method": "Page.navigate", "params": {"url": url}}))
    ws.close()

    deadline = time.time() + 45
    while time.time() < deadline:
        time.sleep(1.5)
        try:
            t = tab_target()
            if want_host not in t.get("url", ""):
                continue
            ws = websocket.create_connection(t["webSocketDebuggerUrl"], timeout=20)
            ws.send(json.dumps({"id": 2, "method": "Runtime.evaluate",
                                "params": {"expression": JS, "returnByValue": True}}))
            got, end = None, time.time() + 20
            while time.time() < end:
                m = json.loads(ws.recv())
                if m.get("id") == 2:
                    got = m
                    break
            ws.close()
            if not got or "result" not in got.get("result", {}):
                continue
            val = json.loads(got["result"]["result"]["value"])
            if want_host not in val["href"]:
                continue
            return val
        except SystemExit:
            raise
        except Exception:
            continue
    raise SystemExit(f"could not measure {url}")


rows = []
for i in range(REPEATS):
    ex = measure("https://github.com/", "github.com")
    fa = measure("https://example.com/", "example.com")
    differ = ex["hash"] != fa["hash"]
    rows.append(differ)
    print(f"  run {i+1}: exempt={ex['hash']}  farbled={fa['hash']}  -> "
          f"{'FARBLED' if differ else 'NOT FARBLED'}")

ok = sum(rows)
print("=" * 74)
print(f"farbled in {ok}/{len(rows)} runs")
sys.exit(0 if ok == len(rows) else 1)
