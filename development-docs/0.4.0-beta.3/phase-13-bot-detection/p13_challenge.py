#!/usr/bin/env python3
r"""p13_challenge.py -- the AGENT-RUNNABLE half of the P13 verdict matrix.

⛔ READ THIS BEFORE BELIEVING ANY NUMBER IT PRINTS.

**What it can honestly measure.** reCAPTCHA v3 returns a *numeric score* and renders no UI, so
"were we scored well" is machine-readable and needs no human judgement. Cloudflare Turnstile and
hCaptcha render a widget whose *state* (did it mint a token / did it escalate to a challenge) is
readable from the DOM without solving anything.

**What it CANNOT measure, and why the human rows still exist.**
1. ⛔ **The CDP confound is real here and is NOT waved away.** Unlike the Block B signal sheet -- a
   UA string does not change because a debugger is attached -- these verdicts are *behaviour*-scored.
   Driving over CDP means no mouse, no keystrokes, no dwell, and DataDome's 2026 research describes
   detecting the CDP wire protocol itself. ⇒ **A low score here may be the instrument.**
   ⭐ The mitigation is that BOTH browsers are driven the SAME way, so the Hodos-vs-Chrome **delta**
   is meaningful even when the absolute numbers are depressed. Report the delta, not the absolute.
2. ⛔ **Solving is out of scope and must not be attempted.** If a run reaches an image grid, the
   answer is "escalated to interactive" and the row is done.

**The negative control is mandatory and lives on the v3 row**, because v3 is the one that returns a
number: the same browser relaunched with `--enable-automation` must score LOWER. An all-green matrix
without that cell proves the pages were reachable, not that the test can detect a bot verdict.

Usage:
    python p13_challenge.py --port 9322 --label hodos --kind hodos
    python p13_challenge.py --port 9333 --label chrome --kind plain
"""
import argparse
import json
import os
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
sys.path.insert(0, r"C:\Users\archb\Hodos-Browser\development-docs\0.4.0\chromium-rebuild")

# Pages. ⚠️ Distinct URL per row on purpose -- reusing one URL let a one-shot cache serve
# trial n's failure as trial n+1's success once already this sprint.
PAGES = [
    ("recaptcha-v3", "https://recaptcha-demo.appspot.com/recaptcha-v3-request-scores.php",
     "recaptcha-demo.appspot.com"),
    ("recaptcha-v2", "https://recaptcha-demo.appspot.com/recaptcha-v2-checkbox.php",
     "recaptcha-demo.appspot.com"),
    ("hcaptcha", "https://accounts.hcaptcha.com/demo", "accounts.hcaptcha.com"),
    ("cloudflare-live", "https://whatsonchain.com/", "whatsonchain.com"),
]

# Reads the verdict WITHOUT interacting. Everything here is observation only.
VERDICT_JS = r"""
(function () {
  var txt = (document.body ? document.body.innerText : '') || '';
  var html = (document.documentElement ? document.documentElement.outerHTML : '') || '';
  function has(re) { return re.test(html); }
  var scores = [];
  var m, re = /score["'\s:=]+([01]?\.\d+)/gi;
  while ((m = re.exec(txt)) !== null) scores.push(parseFloat(m[1]));
  return JSON.stringify({
    href: location.href,
    title: document.title,
    /* vendor presence */
    sawRecaptcha:  has(/recaptcha/i),
    sawHcaptcha:   has(/hcaptcha/i),
    sawTurnstile:  has(/turnstile|challenges\.cloudflare\.com/i),
    /* verdict signals -- observation only, nothing is clicked */
    scoresOnPage:  scores,
    challengeWords: (txt.match(/(select all|click each|verify you are human|checking your (browser|site connection)|just a moment|access denied|unusual traffic|are you a robot)/gi) || []),
    rayId: (txt.match(/Ray ID:?\s*([0-9a-f]+)/i) || [null, null])[1],
    /* did a widget mint a token without interaction? */
    gTokenLen: (function () { var e = document.getElementsByName('g-recaptcha-response');
                              return e.length ? (e[0].value || '').length : -1; })(),
    cfTokenLen: (function () { var e = document.getElementsByName('cf-turnstile-response');
                               return e.length ? (e[0].value || '').length : -1; })(),
    hTokenLen: (function () { var e = document.getElementsByName('h-captcha-response');
                              return e.length ? (e[0].value || '').length : -1; })(),
    bodyLen: txt.length,
    excerpt: txt.slice(0, 220).replace(/\s+/g, ' ')
  });
})()
"""


def verdict_of(r):
    """Collapse the observations into one of the four words the matrix allows."""
    if r["challengeWords"]:
        w = " / ".join(sorted(set(x.lower() for x in r["challengeWords"])))
        if any(k in w for k in ("access denied", "unusual traffic")):
            return "BLOCKED (%s)" % w
        return "CHALLENGED (%s)" % w
    if r["scoresOnPage"]:
        return "scored %s" % r["scoresOnPage"]
    for k, label in (("cfTokenLen", "turnstile"), ("gTokenLen", "recaptcha"), ("hTokenLen", "hcaptcha")):
        if r[k] > 20:
            return "PASSED silently (%s token, %d chars)" % (label, r[k])
    if r["bodyLen"] < 40:
        return "EMPTY PAGE -- inconclusive, not a pass"
    return "no challenge seen"


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--port", type=int, required=True)
    ap.add_argument("--label", required=True)
    ap.add_argument("--kind", choices=["hodos", "plain"], required=True,
                    help="hodos = go through the subject-gated shared measure(); "
                         "plain = Chrome/Brave, single page target")
    ap.add_argument("--settle", type=float, default=8.0)
    ap.add_argument("--dwell", type=float, default=9.0,
                    help="seconds to let the widget run before reading")
    ap.add_argument("--out")
    a = ap.parse_args()

    for st in (sys.stdout, sys.stderr):
        try:
            st.reconfigure(encoding="utf-8", errors="replace")
        except (AttributeError, ValueError):
            pass

    rows = []
    if a.kind == "hodos":
        from farbling_seed_rotation_check import measure, snapshot_targets
        from p13_signals import assert_tab
        excluded = snapshot_targets(a.port, settle=a.settle)
        for name, url, host in PAGES:
            try:
                r = measure(a.port, excluded, url, host, timeout=120, js=VERDICT_JS)
                assert_tab(host)
                time.sleep(a.dwell)
                r2 = measure(a.port, excluded, url, host, timeout=120, js=VERDICT_JS)
                rows.append((name, r2))
            except SystemExit as e:
                rows.append((name, {"error": str(e)}))
    else:
        import websocket
        from urllib.request import urlopen
        ts = [t for t in json.loads(urlopen("http://127.0.0.1:%d/json" % a.port, timeout=10).read().decode())
              if t.get("type") == "page" and t.get("webSocketDebuggerUrl")
              and t.get("url", "").startswith(("http://", "https://"))]
        if len(ts) != 1:
            print("expected exactly 1 page target, got %d" % len(ts))
            return 1
        ws = websocket.create_connection(ts[0]["webSocketDebuggerUrl"], timeout=60,
                                         suppress_origin=True)
        try:
            i = [0]

            def call(method, **p):
                i[0] += 1
                ws.send(json.dumps({"id": i[0], "method": method, "params": p}))
                while True:
                    m = json.loads(ws.recv())
                    if m.get("id") == i[0]:
                        return m.get("result", {})

            for name, url, host in PAGES:
                # ⛔ TWO navigations + a dwell, mirroring the hodos path EXACTLY.
                # The first version of this did one navigation and Chrome came back with no
                # reCAPTCHA v3 score at all while Hodos showed 0.9 -- which looked like a
                # finding and was purely the two harness paths driving differently. Both
                # columns must be driven identically or the delta means nothing.
                call("Page.navigate", url=url)
                time.sleep(a.dwell)
                call("Runtime.evaluate", expression=VERDICT_JS,
                     returnByValue=True, awaitPromise=True)
                time.sleep(a.dwell)
                call("Page.navigate", url=url)
                time.sleep(a.dwell)
                res = call("Runtime.evaluate", expression=VERDICT_JS,
                           returnByValue=True, awaitPromise=True)
                v = res.get("result", {}).get("value")
                rows.append((name, json.loads(v) if v else {"error": "no value"}))
        finally:
            ws.close()

    print("")
    print("=== P13 challenge observations -- %s ===" % a.label)
    print("⛔ CDP-driven. Behaviour-scored rows: a bad result may be the instrument.")
    print("   Compare the DELTA against the identically-driven Chrome column, not the absolute.")
    print("")
    for name, r in rows:
        if "error" in r:
            print("  %-16s ERROR %s" % (name, r["error"][:90]))
            continue
        print("  %-16s %s" % (name, verdict_of(r)))
        print("  %-16s   vendors: recaptcha=%s hcaptcha=%s turnstile=%s%s"
              % ("", r["sawRecaptcha"], r["sawHcaptcha"], r["sawTurnstile"],
                 ("  rayId=" + r["rayId"]) if r["rayId"] else ""))
        print("  %-16s   %s" % ("", r["excerpt"][:150]))
    if a.out:
        json.dump({"label": a.label, "rows": rows}, open(a.out, "w"), indent=2)
        print("")
        print("wrote %s" % a.out)
    return 0


if __name__ == "__main__":
    sys.exit(main())
