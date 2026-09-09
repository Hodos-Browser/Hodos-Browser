#!/usr/bin/env python3
"""
beta.3 Phase 5 (loopback routing & trust boundary) — macOS arm, rows R1/R2/M4.

🎯 SUBJECT: OUR RUST WALLET'S LOG, not the page and not the C++ log.
The line that decides every row is

    R-INTEXT trust: path=<path> requesting_domain=<the page's host>

⛔ The C++ "Scheme downgrade" line proves nothing — it prints whether or not the
rewrite took effect (ADVERSARIAL_PANEL_2 #29). The page cannot tell which wallet
answered, so the page's own view of a response is NOT the evidence either.

⛔ WHY ONE PROBE PER RUN. A first attempt fired all four fetches inside one
Runtime.evaluate. Two `/getVersion requesting_domain=example.com` lines came back
45 s apart and the evaluate never returned, so the second line could not be
attributed to the https/2121 probe rather than a retry of the http/3321 one.
Each probe now runs alone, inside its own before/after window on the wallet log,
and every fetch carries an AbortController deadline so a hang is reported as a
hang instead of taking the whole run down with it.

⭐ NEGATIVE CONTROL (free, and it beats stopping another wallet): probe
`http://127.0.0.1:3322/getNetwork` — one digit off 3321 and deliberately not in
our gate. It must produce ZERO new Rust lines and a TypeError on the page. That
is what proves the gate is what causes interception.

USAGE
    python p5probe_mac.py            # all probes, in order
"""

import json
import os
import subprocess
import sys
import time

import websocket

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)),
                                "..", "phase-1-overlay-input-dpi"))
import cdp  # noqa: E402

PAGE = "https://example.com/"
WALLET_LOG = os.path.expanduser(
    "~/Library/Application Support/HodosBrowserDev/logs/wallet_rCURRENT.log")

PROBES = [
    ("R1  http  loopback 3321",
     "http://127.0.0.1:3321/getVersion",
     "must reach OUR wallet with requesting_domain=example.com"),
    ("R2  https loopback 2121",
     "https://127.0.0.1:2121/getVersion",
     "the TLS question: does a CefResourceHandler take over PRE-TLS on macOS"),
    ("M4  https example.com?x=",
     "https://example.com/getNetwork?x=127.0.0.1:3321",
     "must be example.com's own 404, NOT our wallet answering"),
    ("NEG http  loopback 3322",
     "http://127.0.0.1:3322/getNetwork",
     "one digit off, not gated: expect TypeError and ZERO new Rust lines"),
]

FETCH_JS = """(async function(){
  const ctl = new AbortController();
  const to = setTimeout(function(){ ctl.abort(); }, 8000);
  try{
    const r = await fetch(%s, {signal: ctl.signal});
    const t = await r.text();
    clearTimeout(to);
    return JSON.stringify({status: r.status, len: t.length,
                           head: t.slice(0,120).replace(/\\s+/g,' ')});
  }catch(e){
    clearTimeout(to);
    return JSON.stringify({error: e.constructor.name + ': ' + e.message});
  }
})()"""


def log_len():
    try:
        with open(WALLET_LOG, "rb") as f:
            return sum(1 for _ in f)
    except OSError:
        return 0


def new_lines(since):
    with open(WALLET_LOG, "r", errors="replace") as f:
        return [l.rstrip("\n") for l in f.readlines()[since:]]


def run(url):
    t = cdp.pick(PAGE)
    ws = websocket.create_connection(t["webSocketDebuggerUrl"], timeout=30)
    try:
        ws.send(json.dumps({"id": 1, "method": "Runtime.evaluate", "params": {
            "expression": FETCH_JS % json.dumps(url),
            "returnByValue": True, "awaitPromise": True, "userGesture": True}}))
        while True:
            m = json.loads(ws.recv())
            if m.get("id") == 1:
                break
    except websocket.WebSocketTimeoutException:
        return t["url"], {"error": "CDP TIMEOUT — the evaluate never returned"}
    finally:
        ws.close()
    r = m.get("result", {})
    if "exceptionDetails" in r:
        return t["url"], {"error": "threw: %s" % json.dumps(r["exceptionDetails"])[:200]}
    return t["url"], json.loads(r.get("result", {}).get("value") or "{}")


def main():
    if not os.path.exists(WALLET_LOG):
        sys.exit("VACUOUS: wallet log not found at %s" % WALLET_LOG)

    # ⛔ Positive-control the sink FIRST. R-INTEXT logs at debug and the wallet
    # defaults to info, so a zero here can be a suppressed log rather than an
    # absence (7d M6.3). If no R-INTEXT line exists at all, nothing below means
    # anything.
    with open(WALLET_LOG, "r", errors="replace") as f:
        body = f.read()
    n = body.count("R-INTEXT")
    print("=== sink positive control: %d existing R-INTEXT lines" % n)
    if n == 0:
        sys.exit("VACUOUS: no R-INTEXT lines in the wallet log at all. Restart "
                 "the wallet with RUST_LOG=hodos_wallet=debug before trusting "
                 "any zero below.")
    print()

    for label, url, why in PROBES:
        before = log_len()
        page, res = run(url)
        time.sleep(2.0)   # let the wallet flush
        lines = [l for l in new_lines(before) if "R-INTEXT" in l]
        ext = [l for l in lines if "requesting_domain=" in l
               and "<none:internal>" not in l]

        print("%s" % label)
        print("  url      : %s" % url)
        print("  why      : %s" % why)
        print("  page saw : %s" % json.dumps(res))
        print("  new R-INTEXT lines: %d total, %d external-domain" % (len(lines), len(ext)))
        for l in ext:
            print("    %s" % l.split("] ", 1)[-1])
        print()


if __name__ == "__main__":
    main()
