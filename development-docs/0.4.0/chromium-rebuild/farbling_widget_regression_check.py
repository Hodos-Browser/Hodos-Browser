#!/usr/bin/env python3
r"""farbling_widget_regression_check.py — do third-party widgets still WORK now that
subframes are farbled?

## The gap this closes

`PLAN_P4e_iframe_farbling.md` §0 named this surface and asked for it in the regression
basket: "third-party widgets in iframes on **non-exempt** sites (Stripe, reCAPTCHA,
Turnstile, 3-D Secure) are native today and become farbled. Put them in the regression
basket explicitly." It was never added.

⛔ **And `regression_soak.py` cannot cover it, for a reason worth stating.** Measured
2026-08-15: **6 of its 10 basket sites are on the `IsAuthDomain` allowlist** (x.com,
github.com, amazon.com, whatsonchain.com, google.com, docs.google.com), so they run with
farbling **OFF**. Its 10/10 pass is largely a test of a build with the feature disabled --
the exact "would this pass if the feature were absent?" failure this project keeps hitting,
in a new place. None of the four farbled sites embeds a captcha or payment widget.

## What this asserts, and the control that makes it mean something

For each target: a **non-exempt top frame** that embeds a **third-party widget iframe**.

  1. CONTROL -- the top frame must actually be farbled (its canvas hash != the native hash
     measured on the same build). Without this, a green result could simply be a page where
     farbling never ran, which is what would happen if someone later added the host to the
     allowlist. The control is what stops this becoming another vacuous suite.
  2. The widget must reach a WORKING state -- not merely "an iframe exists". Turnstile must
     produce a non-empty response token; Stripe must render its payment fields.

⚠️ **What this cannot prove.** A captcha that solves for an automated visitor today may
still score a real user differently, and these vendors do not publish their signals. A pass
means "the widget completes its own success path on a farbled page", NOT "our fingerprint
is indistinguishable to Cloudflare's risk engine". Do not upgrade the claim.

## Usage

    python3 farbling_widget_regression_check.py --dev \
        --exe ...\cef-native\build\bin\Release\HodosBrowser.exe \
        --data-root %APPDATA%\HodosBrowserDev

Exit 0 = every reachable target's widget worked, with its farbling control green.
Exit 1 = a widget failed on a page proven to be farbled -> a real regression.
Exit 2 = nothing conclusive (all targets unreachable, or controls failed).
"""

import argparse
import json
import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

try:
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
except Exception:
    pass

try:
    import websocket  # noqa: F401
except ImportError:
    sys.exit("need websocket-client:  pip install websocket-client")

from farbling_seed_rotation_check import (  # noqa: E402
    cef_version,
    engine_version,
    kill_browser_by_path,
    launch_browser,
    set_site_enabled,
    wait_for_cdp,
)
from farbling_cross_profile_check import cdp_port_for, profile_dir  # noqa: E402
from farbling_worker_probe import resolve_tab, snapshot_targets  # noqa: E402
from farbling_iframe_check import _rpc  # noqa: E402

# Each target is a NON-EXEMPT top frame embedding a third-party widget. Hosts are checked
# against the live allowlist at runtime -- if one is ever added to IsAuthDomain this suite
# would silently stop testing anything, so it refuses instead.
#
# ⚠️ `host` MUST be the FULL HOSTNAME, not the registrable domain. MEASURED 2026-08-15:
# the per-site Privacy Shield opt-out is looked up by hostname -- `simple_handler.cpp`
# slices it straight out of the nav URL and passes it to IsSiteEnabled -- whereas the
# farbling KEY is HMAC(seed, registrable_domain). Two different keys off the same URL.
# Getting this wrong is silent: the opt-out never matches, BOTH arms come out farbled,
# their canvases are identical, and the control reports "top frame not farbled" rather
# than anything that points at the real cause. It cost two runs here.
TARGETS = [
    {
        "name": "Cloudflare Turnstile",
        "url": "https://demo.turnstile.workers.dev/",
        "host": "demo.turnstile.workers.dev",
        # Turnstile writes its token into a hidden input once the challenge passes.
        "success_js": """
            (function () {
              var el = document.querySelector('[name="cf-turnstile-response"]');
              var tok = el ? (el.value || '') : '';
              var frames = document.querySelectorAll('iframe').length;
              return JSON.stringify({ok: tok.length > 20, detail: 'token len ' + tok.length,
                                     frames: frames});
            })()
        """,
    },
    {
        "name": "Stripe checkout (payment element)",
        "url": "https://checkout.stripe.dev/",
        "host": "checkout.stripe.dev",
        # Stripe renders its fields inside js.stripe.com iframes. "A payment iframe exists
        # and the page is past its skeleton" is the strongest thing observable without a
        # real card.
        "success_js": """
            (function () {
              var fr = Array.prototype.slice.call(document.querySelectorAll('iframe'));
              var stripe = fr.filter(function (f) {
                return (f.src || '').indexOf('stripe.com') !== -1;
              });
              return JSON.stringify({ok: stripe.length > 0,
                                     detail: stripe.length + ' stripe iframe(s) of '
                                             + fr.length,
                                     frames: fr.length});
            })()
        """,
    },
]

# Same geometry-only canvas as the other harnesses, so the control's native value is
# comparable to what they measure.
CANVAS_JS = r"""
(function () {
  try {
    var FNV = function (b) {
      var h = 2166136261 >>> 0;
      for (var i = 0; i < b.length; i++) { h ^= (b[i] & 255); h = Math.imul(h, 16777619) >>> 0; }
      return ('0000000' + (h >>> 0).toString(16)).slice(-8);
    };
    var c = document.createElement('canvas'); c.width = 200; c.height = 50;
    var x = c.getContext('2d');
    x.fillStyle = '#f60'; x.fillRect(0, 0, 100, 20);
    x.fillStyle = '#069'; x.fillRect(10, 12, 60, 25);
    var g = x.createLinearGradient(0, 0, 200, 50);
    g.addColorStop(0, 'rgba(0,120,255,0.7)');
    g.addColorStop(1, 'rgba(255,0,90,0.35)');
    x.fillStyle = g; x.fillRect(0, 20, 200, 30);
    x.strokeStyle = 'rgba(0,0,0,0.8)'; x.lineWidth = 3;
    x.beginPath(); x.arc(40, 25, 18, 0, Math.PI * 2); x.stroke();
    return JSON.stringify({href: location.href,
                           canvas: FNV(x.getImageData(0, 0, 200, 50).data)});
  } catch (e) { return JSON.stringify({href: location.href, canvas: null,
                                       err: String(e && e.message || e)}); }
})()
"""

ALLOWLIST_HEADER = os.path.join(
    os.path.dirname(os.path.abspath(__file__)),
    "..", "..", "..", "cef-native", "include", "core", "FingerprintProtection.h")


def allowlist_hosts():
    """Parse IsAuthDomain's host literals. Used only to REFUSE a target that has become
    exempt -- never to decide a verdict."""
    hosts = set()
    try:
        with open(os.path.abspath(ALLOWLIST_HEADER), "r", encoding="utf-8") as fh:
            body = fh.read()
    except OSError:
        return hosts
    start = body.find("IsAuthDomain")
    if start == -1:
        return hosts
    chunk = body[start:start + 6000]
    import re
    for m in re.finditer(r'"([a-z0-9][a-z0-9.\-]*\.[a-z]{2,})"', chunk):
        hosts.add(m.group(1))
    return hosts


def evaluate(port, excluded, expr, wait=45):
    tab = resolve_tab(port, excluded)
    got = _rpc(tab["webSocketDebuggerUrl"], "Runtime.evaluate",
               {"expression": expr, "returnByValue": True, "awaitPromise": True},
               msg_id=7, wait=wait)
    if not got:
        return None
    res = got.get("result", {}).get("result", {})
    return res.get("value")


def goto(port, excluded, url, want_host, timeout=90):
    tab = resolve_tab(port, excluded)
    _rpc(tab["webSocketDebuggerUrl"], "Page.navigate", {"url": url}, msg_id=1)
    deadline = time.time() + timeout
    while time.time() < deadline:
        time.sleep(2.0)
        tab = resolve_tab(port, excluded)
        if want_host in tab.get("url", ""):
            return True
    return False


def boot(args, port, pdir, host, farbling_on):
    kill_browser_by_path(args.exe)
    set_site_enabled(pdir, host, farbling_on)
    for attempt in range(1, 4):
        if attempt > 1:
            kill_browser_by_path(args.exe)
        launch_browser(args.exe, args.dev, args.profile)
        if wait_for_cdp(port):
            break
    else:
        raise SystemExit("CDP %d never came up" % port)
    time.sleep(args.settle)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--exe", required=True)
    ap.add_argument("--data-root", required=True)
    ap.add_argument("--profile", default="Default")
    ap.add_argument("--dev", action="store_true")
    ap.add_argument("--settle", type=float, default=8.0)
    ap.add_argument("--widget-wait", type=float, default=12.0,
                    help="seconds to let the widget settle before asserting")
    args = ap.parse_args()

    port = cdp_port_for(args.profile, args.dev)
    pdir = profile_dir(args.data_root, args.profile)
    exempt = allowlist_hosts()

    print("== third-party widget regression (farbled top frames) ==")
    print("   engine %s" % cef_version())

    results = []
    try:
        for t in TARGETS:
            if t["host"] in exempt:
                print("\n  ⛔ REFUSED %s — %s is now on the IsAuthDomain allowlist, so its "
                      "top frame is NOT farbled and this target tests nothing."
                      % (t["name"], t["host"]))
                results.append((t["name"], "REFUSED", "target became exempt"))
                continue

            # --- farbled arm ------------------------------------------------------------
            oh = t.get("optout_host", t["host"])
            boot(args, port, pdir, oh, farbling_on=True)
            excluded = snapshot_targets(port, args.settle)
            if not goto(port, excluded, t["url"], t["host"]):
                print("\n  ⚠️ UNREACHABLE %s (%s did not load)" % (t["name"], t["host"]))
                results.append((t["name"], "UNREACHABLE", "page did not load"))
                continue
            time.sleep(args.widget_wait)
            farb_raw = evaluate(port, excluded, CANVAS_JS)
            widget_raw = evaluate(port, excluded, t["success_js"])

            # --- native arm, same page, farbling off -----------------------------------
            boot(args, port, pdir, oh, farbling_on=False)
            excluded = snapshot_targets(port, args.settle)
            goto(port, excluded, t["url"], t["host"])
            time.sleep(args.widget_wait)
            nat_raw = evaluate(port, excluded, CANVAS_JS)
            # ⛔ THE DECISIVE CONTROL, and its absence made the first run report a Stripe
            # "REGRESSION" on a page that renders zero iframes in BOTH arms. Measuring the
            # widget only where farbling is ON cannot distinguish "farbling broke it" from
            # "this target never worked / the page changed / the demo moved". A regression
            # is works-natively AND fails-farbled. Nothing else is.
            nat_widget_raw = evaluate(port, excluded, t["success_js"])

            farb = json.loads(farb_raw) if farb_raw else {}
            nat = json.loads(nat_raw) if nat_raw else {}
            widget = json.loads(widget_raw) if widget_raw else {}
            nat_widget = json.loads(nat_widget_raw) if nat_widget_raw else {}

            print("\n  %s  (%s)" % (t["name"], t["host"]))
            print("    canvas farbled=%s  native=%s" % (farb.get("canvas"),
                                                        nat.get("canvas")))
            # THE CONTROL. A widget that works on an unfarbled page proves nothing.
            if not farb.get("canvas") or not nat.get("canvas"):
                print("    ⚠️ NO VERDICT — could not read the canvas control")
                results.append((t["name"], "NO-VERDICT", "control unreadable"))
                continue
            if farb["canvas"] == nat["canvas"]:
                print("    ⚠️ NO VERDICT — the top frame is NOT farbled, so a working")
                print("       widget here says nothing about the change under test.")
                results.append((t["name"], "NO-VERDICT", "top frame not farbled"))
                continue
            print("    [PASS] control: top frame IS farbled")

            ok = bool(widget.get("ok"))
            nat_ok = bool(nat_widget.get("ok"))
            print("    widget farbled: %-9s (%s)"
                  % ("WORKS" if ok else "FAILED", widget.get("detail")))
            print("    widget native:  %-9s (%s)"
                  % ("WORKS" if nat_ok else "FAILED", nat_widget.get("detail")))
            if not nat_ok:
                print("    ⚠️ NO VERDICT — the widget does not work with farbling OFF")
                print("       either, so this target is broken/moved and says nothing")
                print("       about the change under test.")
                results.append((t["name"], "NO-VERDICT",
                                "fails natively too: %s" % nat_widget.get("detail")))
            elif ok:
                results.append((t["name"], "OK", widget.get("detail")))
            else:
                results.append((t["name"], "REGRESSION",
                                "works natively (%s) but fails farbled (%s)"
                                % (nat_widget.get("detail"), widget.get("detail"))))
    finally:
        kill_browser_by_path(args.exe)
        for t in TARGETS:
            set_site_enabled(pdir, t.get("optout_host", t["host"]), True)

    print("\n" + "=" * 70)
    for name, state, detail in results:
        print("  %-34s %-12s %s" % (name, state, detail))

    regressions = [r for r in results if r[1] == "REGRESSION"]
    conclusive = [r for r in results if r[1] in ("OK", "REGRESSION")]
    if regressions:
        print("\n  ⛔ %d widget regression(s) on farbled pages" % len(regressions))
        return 1
    if not conclusive:
        print("\n  ⚠️ NOTHING CONCLUSIVE — no target produced a controlled result.")
        print("     This is NOT a pass; the surface remains untested.")
        return 2
    print("\n  ✅ %d/%d controlled target(s): widget works on a farbled page"
          % (len(conclusive), len(conclusive)))
    print("     ⚠️ Scope: the widget completes its own success path. It does NOT mean our")
    print("        fingerprint is indistinguishable to the vendor's risk engine.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
