#!/usr/bin/env python3
"""b_race_forced_check.py — force the prompt-queue race (TICKET_connect_prompts_arrive_after_approval_and_hang)
on the zanaadu.com tab of the DEV browser. Both platforms: plain CDP on 127.0.0.1:9322.

WHY FORCED: the race needs calls in flight during the ~150 ms approve/close window. Left to chance it
reproduced once in two tries, and a green run where the race never happened proves nothing.

USE: (1) remove zanaadu.com via right-click > Manage Site Permissions (do NOT reload);
     (2) python b_race_forced_check.py --action start   -> connect prompt appears, owner approves;
     (3) a level-1 signature prompt must then APPEAR on screen (answer it either way);
     (4) python b_race_forced_check.py --action read    -> every call answered, 0 errors.
PASS in the browser log: both `🔁 Stale connect prompt ... re-sending` and, if the prompt landed in the
window, `🔁 Re-showing prompt ... hidden by the close`. ⚠️ The second needs the timing; the first is common.

Original notes:

Fires, from the page itself, a stream of wallet calls every 25 ms for `--seconds`:
  - window.CWI.getVersion()                        -> recreates the in-flight connect race (B1)
  - window.CWI.createSignature([1,"hodosbtest"], counterparty self) -> a REAL prompt (level 1 still prompts;
    `self` keeps it ProtocolUse, the same shape as the xanaverse upvote) that
                                                      can land in the approve/close window (B2)
Records every call's outcome on the page so we can see nothing hung.

⛔ Subject: drives the tab whose URL is zanaadu.com on port 9322 (the DEV browser), and prints
the role the shell logged for it. Level 1 so it cannot be silenced by the level-0 fix.
"""
import json, sys, time, argparse
import websocket
from urllib.request import urlopen

ap = argparse.ArgumentParser()
ap.add_argument("--action", choices=["start", "read"], required=True)
ap.add_argument("--seconds", type=float, default=6.0)
a = ap.parse_args()

ts = [t for t in json.loads(urlopen("http://127.0.0.1:9322/json", timeout=10).read().decode())
      if t.get("type") == "page" and "zanaadu.com" in t.get("url", "")]
if len(ts) != 1:
    sys.exit("expected exactly one zanaadu.com tab in the DEV browser, found %d" % len(ts))
ws = websocket.create_connection(ts[0]["webSocketDebuggerUrl"], timeout=30)

if a.action == "start":
    js = """(function(){
      if (!window.CWI) return 'NO CWI';
      window.__bt = {sent:0, ok:0, err:0, errs:[], sig:null, t0:Date.now()};
      var end = Date.now() + %d;
      var sigFired = false;
      var tick = setInterval(function(){
        if (Date.now() > end) { clearInterval(tick); return; }
        window.__bt.sent++;
        window.CWI.getVersion({}).then(function(){ window.__bt.ok++; },
          function(e){ window.__bt.err++; if (window.__bt.errs.length<5) window.__bt.errs.push(String(e&&e.message||e)); });
        // One level-1 signature, fired after ~1 s so it is in flight around the approval.
        if (!sigFired && Date.now() - window.__bt.t0 > 1000) {
          sigFired = true;
          window.__bt.sig = 'pending';
          window.CWI.createSignature({protocolID:[1,'hodosbtest'], keyID:'1', counterparty:'self', data:[1,2,3]})
            .then(function(){ window.__bt.sig = 'ok ' + (Date.now()-window.__bt.t0) + 'ms'; },
                  function(e){ window.__bt.sig = 'err ' + String(e&&e.message||e); });
        }
      }, 25);
      return 'stream started for %d ms';
    })()""" % (int(a.seconds * 1000), int(a.seconds * 1000))
else:
    js = "JSON.stringify(window.__bt||null)"

ws.send(json.dumps({"id": 1, "method": "Runtime.evaluate",
                    "params": {"expression": js, "returnByValue": True}}))
print(json.loads(ws.recv())["result"]["result"].get("value"))
ws.close()
