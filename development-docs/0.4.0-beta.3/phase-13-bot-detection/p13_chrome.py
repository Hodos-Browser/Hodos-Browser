#!/usr/bin/env python3
r"""p13_chrome.py -- the Chrome control column for Phase 13 Block B.

Runs the IDENTICAL probe as p13_signals.py (imported, not copied) against stock Chrome on
the same machine, same day, same network.

⛔ Why this does not go through `farbling_seed_rotation_check.measure`: that helper's
chrome-id exclusion exists to disambiguate Hodos's header and ~14 overlays, all of which
report type:"page". Stock Chrome has exactly one page target here, so the ambiguity the
helper defends against does not exist -- and the helper's Hodos-shaped tab resolution
would not apply. The subject is still asserted: the probe returns `href` and this script
refuses any result whose href is not the URL it navigated.
"""
import argparse
import json
import sys
import time
import websocket
from urllib.request import urlopen

from p13_signals import PROBE_B, PROBE_TLS


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--port", type=int, default=9333)
    ap.add_argument("--label", default="chrome")
    ap.add_argument("--url", default="https://example.com/")
    ap.add_argument("--out")
    ap.add_argument("--tls", action="store_true")
    a = ap.parse_args()

    ts = [t for t in json.loads(urlopen("http://127.0.0.1:%d/json" % a.port, timeout=10).read().decode())
          if t.get("type") == "page" and t.get("webSocketDebuggerUrl")]
    if len(ts) != 1:
        print("expected exactly 1 page target, got %d: %s"
              % (len(ts), [t.get("url") for t in ts]))
        return 1
    t = ts[0]
    print("driving the only page target: %s" % t.get("url"))

    # ⛔ suppress_origin, NOT --remote-allow-origins=*. websocket-client sends an Origin
    # header by default and Chromium's devtools_http_handler 403s any Origin-bearing
    # upgrade that is not allow-listed (P9/D3). The fix belongs in the CLIENT: adding a
    # launch flag would make the control column a non-stock Chrome, which is the one thing
    # a positive control may not be.
    ws = websocket.create_connection(t["webSocketDebuggerUrl"], timeout=45,
                                     suppress_origin=True)
    try:
        ws.send(json.dumps({"id": 1, "method": "Page.navigate", "params": {"url": a.url}}))
        time.sleep(5.0)
        ws.send(json.dumps({"id": 2, "method": "Runtime.evaluate",
                            "params": {"expression": (PROBE_TLS if a.tls else PROBE_B), "returnByValue": True,
                                       "awaitPromise": True}}))
        got = None
        end = time.time() + 30
        while time.time() < end:
            m = json.loads(ws.recv())
            if m.get("id") == 2:
                got = m
                break
    finally:
        ws.close()

    if not got or "value" not in got.get("result", {}).get("result", {}):
        print("no value came back: %s" % json.dumps(got)[:400])
        return 1
    sig = json.loads(got["result"]["result"]["value"])
    if sig["href"] != a.url:
        print("SUBJECT MISMATCH: asked for %s, read %s -- refusing" % (a.url, sig["href"]))
        return 1

    print(sig["body"] if a.tls else json.dumps(sig, indent=2))
    if a.out:
        json.dump({"label": a.label, "port": a.port, "kind": "B-signals", "signals": sig},
                  open(a.out, "w"), indent=2)
        print("")
        print("wrote %s" % a.out)
    return 0


if __name__ == "__main__":
    sys.exit(main())
