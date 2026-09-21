#!/usr/bin/env python3
r"""p13_benches.py — run the purpose-built bot/fingerprint test benches against
Hodos, Chrome and Brave, and diff them per signal.

⭐ WHY THIS EXISTS. Phase 13 spent a day trying to provoke *real sites* into challenging us.
👤 Owner: *"I feel like we're just stabbing at the dark."* He was right — eleven of twelve sites
in the human sitting never triggered anything, so eleven greens meant nothing. These pages are
built to hand you a **verdict and a per-signal breakdown** instead, repeatably, without needing
to get unlucky. ⛔ CreepJS and BotD were already cited in `RESEARCH_steps_1_2.md` as *sources*;
nobody thought to run them as a *test*.

⛔ THE CONFOUND, STATED ONCE AND CARRIED ON EVERY ROW. All three browsers are driven over CDP
with a debug port bound. These benches *do* look for automation, so absolute verdicts here are
depressed for all three. ⇒ **Read the DELTA between browsers, never the absolute.** Chrome and
Brave are the controls; a signal where Hodos differs from BOTH is the finding.

⚠️ AND WHAT THESE CANNOT TELL YOU: what a fingerprinting script sees is not the same as whether
DataDome will let you buy a concert ticket. These narrow "which signal is odd" from a guess to a
list. They do not close the category.
"""
import argparse
import json
import os
import sys
import time
import websocket
from urllib.request import urlopen

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
sys.path.insert(0, r"C:\Users\archb\Hodos-Browser\development-docs\0.4.0\chromium-rebuild")

BENCHES = [
    ("creepjs",    "https://abrahamjuliot.github.io/creepjs/",          "abrahamjuliot.github.io", 28),
    ("sannysoft",  "https://bot.sannysoft.com/",                        "bot.sannysoft.com",       14),
    ("pixelscan",  "https://pixelscan.net/",                            "pixelscan.net",           22),
    ("iphey",      "https://iphey.com/",                                "iphey.com",               20),
    ("areyoubot",  "https://deviceandbrowserinfo.com/are_you_a_bot",    "deviceandbrowserinfo.com", 16),
    ("fingerprint","https://fingerprint.com/products/bot-detection/",   "fingerprint.com",         20),
    ("browserleaks","https://browserleaks.com/javascript",              "browserleaks.com",        18),
]

# Pulls the readable verdict out of whatever the page rendered. Also grabs every
# <table> row, because sannysoft (and others) put the per-signal results in one.
EXTRACT = r"""
(function () {
  var txt = (document.body ? document.body.innerText : '') || '';
  var rows = [];
  var trs = document.querySelectorAll('tr');
  for (var i = 0; i < trs.length && rows.length < 120; i++) {
    var cells = trs[i].querySelectorAll('td, th');
    if (cells.length >= 2) {
      var a = (cells[0].innerText || '').trim().replace(/\s+/g, ' ');
      var b = (cells[1].innerText || '').trim().replace(/\s+/g, ' ');
      if (a && a.length < 60 && b.length < 120) rows.push([a, b]);
    }
  }
  var verdict = [];
  var lines = txt.split('\n');
  for (var j = 0; j < lines.length; j++) {
    var L = lines[j].trim();
    if (!L || L.length > 160) continue;
    if (/\b(bot|robot|automat|headless|webdriver|trust|trustworth|consisten|inconsisten|detect|spoof|masked|lies|suspicious|genuine|not a)\b/i.test(L)) {
      verdict.push(L);
    }
  }
  return JSON.stringify({
    href: location.href,
    title: document.title,
    rows: rows,
    verdict: verdict.slice(0, 40),
    textLen: txt.length,
    head: txt.slice(0, 400).replace(/\s+/g, ' ')
  });
})()
"""


class Plain(object):
    """Chrome / Brave: one page target, no overlay ambiguity."""
    def __init__(self, port):
        ts = [t for t in json.loads(urlopen("http://127.0.0.1:%d/json" % port, timeout=10).read().decode())
              if t.get("type") == "page" and t.get("webSocketDebuggerUrl")
              and t.get("url", "").startswith(("http://", "https://"))]
        if not ts:
            raise SystemExit("no http page target on port %d" % port)
        self.ws = websocket.create_connection(ts[0]["webSocketDebuggerUrl"], timeout=120,
                                              suppress_origin=True)
        self.i = 0

    def call(self, m, **p):
        self.i += 1
        self.ws.send(json.dumps({"id": self.i, "method": m, "params": p}))
        while True:
            r = json.loads(self.ws.recv())
            if r.get("id") == self.i:
                return r.get("result", {})

    def read(self, url, host, dwell):
        self.call("Page.enable")
        self.call("Page.navigate", url=url)
        time.sleep(dwell)
        r = self.call("Runtime.evaluate", expression=EXTRACT,
                      returnByValue=True, awaitPromise=True)
        v = r.get("result", {}).get("value")
        return json.loads(v) if v else {"error": "no value"}

    def close(self):
        try:
            self.ws.close()
        except Exception:
            pass


def read_hodos(port, url, host, dwell, excluded):
    """⛔ Hodos goes through the shared, subject-gated path. Its header and ~14 overlays
    all report type:'page'; a hand-rolled picker drives one of them."""
    from farbling_seed_rotation_check import measure
    from p13_signals import assert_tab
    r = measure(port, excluded, url, host, timeout=150, js=EXTRACT)
    assert_tab(host)
    time.sleep(dwell)                       # let the bench finish its own work
    r = measure(port, excluded, url, host, timeout=150, js=EXTRACT)
    assert_tab(host)
    return r


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--hodos-port", type=int, default=9322)
    ap.add_argument("--chrome-port", type=int, default=9333)
    ap.add_argument("--brave-port", type=int, default=9344)
    ap.add_argument("--out", default="benches.json")
    ap.add_argument("--only", default=None, help="comma-separated bench names")
    a = ap.parse_args()
    for st in (sys.stdout, sys.stderr):
        try:
            st.reconfigure(encoding="utf-8", errors="replace")
        except (AttributeError, ValueError):
            pass

    benches = BENCHES
    if a.only:
        want = set(a.only.split(","))
        benches = [b for b in BENCHES if b[0] in want]

    from farbling_seed_rotation_check import snapshot_targets
    results = {}

    for label, port, kind in (("chrome", a.chrome_port, "plain"),
                              ("brave", a.brave_port, "plain"),
                              ("hodos", a.hodos_port, "hodos")):
        print("")
        print("#" * 76)
        print("#  %s  (port %d)" % (label.upper(), port))
        print("#" * 76)
        results[label] = {}
        excluded = snapshot_targets(port, settle=7.0) if kind == "hodos" else None
        conn = Plain(port) if kind == "plain" else None
        try:
            for name, url, host, dwell in benches:
                try:
                    if kind == "plain":
                        r = conn.read(url, host, dwell)
                    else:
                        r = read_hodos(port, url, host, dwell, excluded)
                    results[label][name] = r
                    print("  %-13s %-42s rows=%-4d text=%d"
                          % (name, (r.get("title") or "")[:42], len(r.get("rows", [])),
                             r.get("textLen", 0)))
                except SystemExit as e:
                    print("  %-13s SUBJECT/ERR: %s" % (name, str(e)[:60]))
                    results[label][name] = {"error": str(e)[:120]}
                except Exception as e:
                    print("  %-13s ERROR %s" % (name, str(e)[:60]))
                    results[label][name] = {"error": str(e)[:120]}
        finally:
            if conn:
                conn.close()

    json.dump(results, open(a.out, "w"), indent=2)
    print("")
    print("wrote %s" % a.out)
    return 0


if __name__ == "__main__":
    sys.exit(main())
