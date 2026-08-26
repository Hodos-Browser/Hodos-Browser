#!/usr/bin/env python3
"""
beta.3 Phase 1 — CDP helper for the OSR overlays.

WHY IT EXISTS
-------------
The header and ~14 overlays are separate CEF browsers that CDP reports ALL as
`type:"page"`. Driving the wrong one faked a bug in this project before (the farbling
"intermittent per-session bug" that was really a harness pointed at an overlay). So
this tool never selects a target by index or by type — every command names the target
by URL substring, and every result prints the URL it actually talked to.

⛔ DEV ONLY. Defaults to port 9322 (dev = 9222 + 100). The installed browser is on
   9222 and must never be touched.

USAGE
    python cdp.py list
    python cdp.py eval  <url-substring> "<javascript>"
    python cdp.py probe <url-substring>      # geometry of that overlay's document
"""

import json
import sys

import websocket  # websocket-client

try:
    from urllib.request import urlopen
except ImportError:  # pragma: no cover
    from urllib2 import urlopen  # type: ignore

DEV_PORT = 9322
PROD_PORT = 9222


def targets(port=DEV_PORT):
    if port == PROD_PORT:
        sys.exit("refusing to attach to 9222 — that is the installed browser")
    return json.loads(urlopen("http://127.0.0.1:%d/json/list" % port, timeout=5).read())


def pick(url_substr, port=DEV_PORT):
    """Resolve exactly one target by URL substring. Ambiguity is an error, not a guess.

    A leading '=' means exact-URL match, which is how you address the header browser —
    its URL is a prefix of every other overlay's.
    """
    all_t = targets(port)
    if url_substr.startswith("="):
        hits = [t for t in all_t if t.get("url", "") == url_substr[1:]]
    else:
        hits = [t for t in all_t if url_substr in t.get("url", "")]
    if not hits:
        sys.exit("no target whose url contains %r" % url_substr)
    if len(hits) > 1:
        sys.exit(
            "ambiguous: %d targets match %r -> %s"
            % (len(hits), url_substr, [t["url"] for t in hits])
        )
    return hits[0]


def evaluate(url_substr, expression, port=DEV_PORT):
    t = pick(url_substr, port)
    ws = websocket.create_connection(t["webSocketDebuggerUrl"], timeout=10)
    try:
        ws.send(json.dumps({
            "id": 1,
            "method": "Runtime.evaluate",
            "params": {"expression": expression, "returnByValue": True,
                       "awaitPromise": True, "userGesture": True},
        }))
        while True:
            msg = json.loads(ws.recv())
            if msg.get("id") == 1:
                return t, msg
    finally:
        ws.close()


# Geometry of the rendered document, for the window-vs-content delta (P1-A5) and for
# reading devicePixelRatio, which is what CEF actually applied from GetScreenInfo.
PROBE_JS = """(function(){
  var d=document.documentElement, b=document.body;
  var r=d.getBoundingClientRect();
  var br=b?b.getBoundingClientRect():{width:0,height:0};
  return JSON.stringify({
    dpr: window.devicePixelRatio,
    inner: [window.innerWidth, window.innerHeight],
    outer: [window.outerWidth, window.outerHeight],
    docRect: [Math.round(r.width), Math.round(r.height)],
    bodyRect: [Math.round(br.width), Math.round(br.height)],
    scrollH: [d.scrollWidth, d.scrollHeight],
    bodyScrollH: b?b.scrollHeight:0,
    url: location.pathname
  });
})()"""


def main():
    if len(sys.argv) < 2:
        sys.exit(__doc__)
    cmd = sys.argv[1]

    if cmd == "list":
        for t in targets():
            print("%-9s %s" % (t.get("type"), t.get("url")))
        return

    if cmd == "eval":
        t, msg = evaluate(sys.argv[2], sys.argv[3])
        print("target: %s" % t["url"])
        res = msg.get("result", {}).get("result", {})
        if "exceptionDetails" in msg.get("result", {}):
            print("EXCEPTION: %s" % json.dumps(msg["result"]["exceptionDetails"])[:400])
        print("value : %s" % res.get("value"))
        return

    if cmd == "probe":
        t, msg = evaluate(sys.argv[2], PROBE_JS)
        print("target: %s" % t["url"])
        val = msg.get("result", {}).get("result", {}).get("value")
        try:
            print(json.dumps(json.loads(val), indent=2))
        except Exception:
            print(val)
        return

    sys.exit("unknown command %r" % cmd)


if __name__ == "__main__":
    main()
