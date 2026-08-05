#!/usr/bin/env python3
"""farbling_probe.py — measure Hodos's live bot-signal + farbling surface over CDP.

Written for BOT-1 (2026-08-05) and kept as the standing acceptance harness for the
P4 farbling migration, because every assertion below is one the P4/P6 gate needs and
none of them can be checked by reading source alone.

WHY THIS EXISTS
    BOT-1 removed two JS overrides (navigator.webdriver, navigator.plugins) on the
    grounds that native Chromium already emits the correct values. That is a claim
    about a running binary, so it gets verified against a running binary — every
    release, not once. CreepJS cannot be scripted and only covers the dedicated-worker
    column, so we own this.

USAGE
    Start the dev stack (wallet + frontend + browser, HODOS_DEV=1), then:

        python farbling_probe.py                     # default: pre-P4a expectations
        python farbling_probe.py --expect-native-canvas   # after C3 lands (P4a)
        python farbling_probe.py --port 9222         # against an installed build

    Exit code 0 = all expectations met, 1 = at least one failed. Safe to wire into CI
    once a headed browser is available to the runner.

    Requires: websocket-client (pip install websocket-client).

PORTS
    Dev Default profile = 9322, release Default profile = 9222 (cef_browser_shell.cpp
    adds +100 under HODOS_DEV). Confirm the owning PID before trusting a CDP port —
    an installed build may hold 9222 while you think you are driving the dev build.
"""

import argparse
import json
import sys
import time
import urllib.request

try:
    import websocket  # websocket-client
except ImportError:
    sys.exit("need websocket-client:  pip install websocket-client")

# The spec'd, hard-coded plugin list every modern Chromium reports when a PDF viewer
# is available. Source: whatwg/html#6738, implemented in Blink's DOMPluginArray ctor.
# We build with enable_pdf=true, so this is what we must match EXACTLY. Deviating from
# it by even one name (we used to say "Chrome PDF Plugin", the pre-2021 name) produces
# a plugin list no real Chrome has — a trivially detectable fingerprint.
SPEC_PLUGINS = [
    "PDF Viewer",
    "Chrome PDF Viewer",
    "Chromium PDF Viewer",
    "Microsoft Edge PDF Viewer",
    "WebKit built-in PDF",
]

# An auth-domain site (FingerprintProtection::IsAuthDomain) sees NO farbling, so it is
# our pure-native control. A non-auth site is the farbled case. Checking both is what
# proves a value is right *structurally* rather than right only where farbling runs.
DEFAULT_TARGETS = [
    ("auth-exempt (pure native control)", "https://github.com/", True),
    ("farbled (farbling active)", "https://example.com/", False),
]

PROBE_JS = r"""
(function () {
  var own   = Object.getOwnPropertyDescriptor(navigator, 'webdriver');
  var proto = Object.getOwnPropertyDescriptor(Navigator.prototype, 'webdriver');
  var names = [];
  for (var i = 0; i < navigator.plugins.length; i++) names.push(navigator.plugins[i].name);
  function isNative(fn) { return fn.toString().indexOf('[native code]') !== -1; }
  return JSON.stringify({
    host: location.host,
    webdriver: navigator.webdriver,
    webdriver_own_prop: !!own,
    webdriver_proto_accessor: !!proto,
    plugins: names,
    plugin_filename: navigator.plugins.length ? navigator.plugins[0].filename : null,
    getImageData_native: isNative(CanvasRenderingContext2D.prototype.getImageData),
    toDataURL_native: isNative(HTMLCanvasElement.prototype.toDataURL),
    toBlob_native: isNative(HTMLCanvasElement.prototype.toBlob),
    getChannelData_native: typeof AudioBuffer !== 'undefined'
        ? isNative(AudioBuffer.prototype.getChannelData) : null
  });
})()
"""


def new_tab(cdp, url):
    req = urllib.request.Request(cdp + "/json/new?" + url, method="PUT")
    return json.load(urllib.request.urlopen(req, timeout=10))


def close_tab(cdp, tid):
    try:
        urllib.request.urlopen(cdp + "/json/close/" + tid, timeout=5).read()
    except Exception:
        pass


def evaluate(ws_url, expr, timeout=25):
    ws = websocket.create_connection(ws_url, timeout=timeout)
    try:
        ws.send(json.dumps({"id": 1, "method": "Runtime.enable"}))
        ws.send(json.dumps({
            "id": 2,
            "method": "Runtime.evaluate",
            "params": {"expression": expr, "returnByValue": True, "awaitPromise": False},
        }))
        for _ in range(80):
            msg = json.loads(ws.recv())
            if msg.get("id") == 2:
                res = msg.get("result", {}).get("result", {})
                if "value" not in res:
                    raise RuntimeError(json.dumps(msg)[:400])
                return json.loads(res["value"])
        raise RuntimeError("no Runtime.evaluate reply")
    finally:
        ws.close()


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--port", type=int, default=9322,
                    help="CDP port (9322 dev Default, 9222 release Default)")
    ap.add_argument("--settle", type=float, default=7.0,
                    help="seconds to let each page load before probing")
    ap.add_argument("--expect-native-canvas", action="store_true",
                    help="assert canvas readback is [native code] on FARBLED pages too "
                         "-- i.e. C3 has landed and the JS canvas fragment is gone (P4a)")
    args = ap.parse_args()
    cdp = "http://127.0.0.1:%d" % args.port

    failures = []

    def check(label, ok, detail):
        mark = "PASS" if ok else "FAIL"
        print("    [%s] %-38s %s" % (mark, label, detail))
        if not ok:
            failures.append(label)

    for label, url, is_exempt in DEFAULT_TARGETS:
        print("=" * 78)
        print("%s  ->  %s" % (label, url))
        tab = new_tab(cdp, url)
        time.sleep(args.settle)
        try:
            r = evaluate(tab["webSocketDebuggerUrl"], PROBE_JS)
        finally:
            close_tab(cdp, tab["id"])

        # --- BOT-1 invariants: identical on exempt AND farbled pages, by construction.
        # A per-site farbling opt-out must never change the bot signature.
        check("navigator.webdriver === false", r["webdriver"] is False, repr(r["webdriver"]))
        check("webdriver NOT an own property", r["webdriver_own_prop"] is False,
              "own=%s (True means we tampered)" % r["webdriver_own_prop"])
        check("webdriver IS a prototype accessor", r["webdriver_proto_accessor"] is True,
              "proto=%s (native shape)" % r["webdriver_proto_accessor"])
        check("plugins == spec'd 5", r["plugins"] == SPEC_PLUGINS, str(r["plugins"]))
        check("plugin filename", r["plugin_filename"] == "internal-pdf-viewer",
              str(r["plugin_filename"]))

        # --- Farbling surface. On an exempt page everything must be native ALWAYS.
        # On a farbled page, canvas is native only once C3 has replaced the JS fragment.
        want_native = True if is_exempt else args.expect_native_canvas
        note = "" if is_exempt else ("  (post-C3 expectation)" if args.expect_native_canvas
                                     else "  (pre-P4a: JS farbling still installed)")
        for fn in ("getImageData_native", "toDataURL_native", "toBlob_native"):
            check("%s is [native code]" % fn.replace("_native", ""),
                  r[fn] is want_native, "%s%s" % (r[fn], note))

    print("=" * 78)
    if failures:
        print("RESULT: %d FAILED -> %s" % (len(failures), ", ".join(sorted(set(failures)))))
        return 1
    print("RESULT: all expectations met")
    return 0


if __name__ == "__main__":
    sys.exit(main())
