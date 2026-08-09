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

    Exit code 0 = all expectations met, 1 = at least one failed.

    ⚠️ NOT wireable into GitHub-hosted CI today, and the reason is not the runner. The
    `cef-binaries` release asset the build job pulls does not exist on the development
    repo at all, and the org repo's `cef-binaries-windows-150.zip` predates the C2/C3
    farbling patches — so a hosted job would compile against binaries with no farbling
    and go red against correct code. The gate therefore lives on the build host; see
    development-docs/DevOps-CICD/FARBLING_RELEASE_GATE.md.

    Requires: websocket-client (pip install websocket-client).

PORTS
    Dev Default profile = 9322, release Default profile = 9222 (cef_browser_shell.cpp
    adds +100 under HODOS_DEV). Confirm the owning PID before trusting a CDP port —
    an installed build may hold 9222 while you think you are driving the dev build.

    ⛔⛔ TARGET SELECTION IS ID-BASED, AND MUST STAY THAT WAY.
    Hodos's header and ~14 overlays are separate CEF browsers on 127.0.0.1:5137, and CDP
    reports every one as type:"page". On 2026-08-08 a URL-heuristic version of this harness
    made a WORKING implementation look like an intermittent per-session bug for hours (it was
    driving role: tablistpanel, which legitimately gets no farbling key). Asserting
    location.href does NOT catch it, because the overlay really is at the URL you navigated
    it to, and "the first target that is not 127.0.0.1:5137" does not catch it either —
    after the first navigation an overlay no longer matches that.

    ⇒ Browser chrome is therefore identified ONCE at startup, by CDP target id, and excluded
      for the rest of the run (`ChromeExcluder` below). Ported 2026-08-09; the behavioural
      comparison is no longer advisory. Same selection as farbling_canvas_check.py, whose
      header documents all three harness defects found in this project.

    ⚠️ A GREEN RESULT HERE IS STILL NOT SUFFICIENT: the shipped constant-seed bug would pass
      every assertion in this file. Per-user unlinkability requires rotating profileSeed in
      <profile>/fingerprint_settings.json across restarts -- run
      **farbling_seed_rotation_check.py**, which automates that and carries a negative
      control.

    ⚠️ OBSERVED 2026-08-07, not hypothetical: on the dev machine the developer's own
    INSTALLED browser was running and holding 9222, while ~8 HodosBrowser.exe
    processes ran from %LOCALAPPDATA%\HodosBrowser. Probing 9222 there would have
    driven the real browser, with a real wallet attached, and reported its results as
    the dev build's. Always check first:

        netstat -ano | grep -E ":9222|:9322" | grep LISTENING
        powershell -NoProfile -Command "Get-CimInstance Win32_Process \
            -Filter \"ProcessId=<pid>\" | Select ExecutablePath"

    The dev build's path contains `build\bin\Release`. Anything under %LOCALAPPDATA%
    is the installed product — do not drive it, and never kill by image name, since
    both share the image name HodosBrowser.exe.
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

  // --- Behavioural canvas probe (C3) --------------------------------------
  // Nativeness alone proves only that the JS override is gone -- it would pass
  // just as happily if we deleted the fragment and implemented no native
  // farbling at all. These read actual pixels.
  //
  // TWO canvases, and the large one is the control that makes the comparison
  // sound: it is deliberately OUTSIDE the <65536px small-canvas gate, so it is
  // never farbled on any page. If the large hashes match across two pages while
  // the small hashes differ, that difference IS farbling and not some incidental
  // rendering difference between the two pages.
  function draw(w, h) {
    var c = document.createElement('canvas');
    c.width = w; c.height = h;
    var x = c.getContext('2d');
    x.textBaseline = 'top';
    x.font = '14px "Arial"';
    x.fillStyle = '#f60'; x.fillRect(0, 0, 100, 20);
    x.fillStyle = '#069'; x.fillText('Hodos farbling probe', 2, 2);
    x.strokeStyle = 'rgba(0,120,255,0.7)';
    x.beginPath(); x.arc(40, 25, 18, 0, Math.PI * 2); x.stroke();
    return c;
  }
  function fnv(s) {
    var h = 2166136261 >>> 0;
    for (var i = 0; i < s.length; i++) { h ^= s.charCodeAt(i); h = Math.imul(h, 16777619) >>> 0; }
    return ('0000000' + h.toString(16)).slice(-8);
  }
  function pixHash(c) {
    var d = c.getContext('2d').getImageData(0, 0, c.width, c.height).data;
    var h = 2166136261 >>> 0;
    for (var i = 0; i < d.length; i++) { h ^= d[i]; h = Math.imul(h, 16777619) >>> 0; }
    return ('0000000' + h.toString(16)).slice(-8);
  }
  var small = draw(200, 50);     // 10,000 px -> inside the gate, farbled
  var large = draw(400, 200);    // 80,000 px -> outside the gate, never farbled

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
        ? isNative(AudioBuffer.prototype.getChannelData) : null,

    // Behavioural. `_repeat` re-reads the SAME canvas: farbling is deterministic,
    // so it must be byte-identical. An unstable fingerprint is itself a
    // fingerprint, and it is also what a mutate-the-canvas implementation would
    // produce (each read re-flipping the previous read's bits).
    small_daturl_hash: fnv(small.toDataURL()),
    small_daturl_hash_repeat: fnv(small.toDataURL()),
    small_pixel_hash: pixHash(small),
    small_pixel_hash_repeat: pixHash(small),
    large_daturl_hash: fnv(large.toDataURL()),
    large_pixel_hash: pixHash(large)
  });
})()
"""


def _pages(cdp):
    targets = json.load(urllib.request.urlopen(cdp + "/json/list", timeout=10))
    return [t for t in targets if t.get("type") == "page"]


class ChromeExcluder:
    """Resolves the real tab, by id, for the whole run.

    ⛔ DO NOT go back to `PUT /json/new?<url>`. A target created that way does NOT
    go through CEF's SimpleHandler::OnBeforeBrowse, so the browser process never
    sends it a farbling key (nor a fingerprint seed) -- meaning every farbling
    assertion below fails no matter how correct the implementation is. That
    defect cost a full debugging cycle on 2026-08-07: the probe was reporting
    "farbling is off" against a build where the browser was demonstrably sending
    the key, because the tabs the probe made were invisible to CEF.

    ⛔ AND DO NOT go back to picking by URL. The header ("/") and every overlay
    ("/tab-list", "/menu", "/wallet-panel", "/downloads", "/privacy-shield", ...)
    are separate CEF browsers on 5137 that CDP reports as type:"page". Both
    "first page target" and "first target that is not 5137" can land on one --
    the second because an overlay stops matching 5137 the moment you navigate it
    somewhere else. So chrome is snapshotted ONCE, before anything navigates, and
    excluded by id from then on.

    Driving a tab CEF made itself, via Page.navigate, is the only harness that
    exercises the real navigation path.
    """

    def __init__(self, cdp):
        self.cdp = cdp
        self.chrome_ids = set()
        for t in _pages(cdp):
            url = t.get("url", "")
            if "127.0.0.1:5137" in url and "/newtab" not in url:
                self.chrome_ids.add(t["id"])
        print("target selection: excluded %d chrome target(s) by id "
              "(header + overlays)" % len(self.chrome_ids))

    def tab(self):
        cands = [t for t in _pages(self.cdp) if t["id"] not in self.chrome_ids]
        if not cands:
            raise RuntimeError("no non-chrome page target -- is the browser running "
                               "with an open tab?")
        if len(cands) == 1:
            return cands[0]
        for t in cands:
            if "/newtab" in t.get("url", ""):
                return t
        # Ambiguity is a subject error waiting to happen; guessing here is precisely
        # what produced the fake "intermittent" bug. Fail loudly instead.
        raise RuntimeError(
            "ambiguous tab target among %d candidates (%s) -- close extra tabs or "
            "disable session restore; do NOT let this script pick one."
            % (len(cands), ", ".join(t.get("url", "?")[:60] for t in cands)))


def navigate(ws_url, url, settle):
    """Navigate an existing CEF tab and wait for it to settle."""
    ws = websocket.create_connection(ws_url, timeout=30)
    try:
        ws.send(json.dumps({"id": 1, "method": "Page.enable"}))
        ws.send(json.dumps({"id": 2, "method": "Page.navigate",
                            "params": {"url": url}}))
        time.sleep(settle)
    finally:
        ws.close()


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

    # Snapshot browser chrome BEFORE the first navigation. After that, an overlay no
    # longer looks like chrome by URL, so this has to happen exactly here.
    excluder = ChromeExcluder(cdp)

    failures = []

    def check(label, ok, detail):
        mark = "PASS" if ok else "FAIL"
        print("    [%s] %-38s %s" % (mark, label, detail))
        if not ok:
            failures.append(label)

    seen = {}

    for label, url, is_exempt in DEFAULT_TARGETS:
        print("=" * 78)
        print("%s  ->  %s" % (label, url))
        tab = excluder.tab()
        navigate(tab["webSocketDebuggerUrl"], url, args.settle)
        # Re-resolve: a cross-site navigation swaps renderer process AND target,
        # so the socket we navigated with may no longer address the new document.
        tab = excluder.tab()
        r = evaluate(tab["webSocketDebuggerUrl"], PROBE_JS)
        # Value assertion. The SUBJECT guarantee comes from the id-based exclusion
        # above, not from this -- an overlay navigated here would satisfy it too.
        if r.get("host") not in url:
            check("landed on the requested host", False,
                  "probed %r, expected %r -- results below are for the WRONG document"
                  % (r.get("host"), url))
        seen["exempt" if is_exempt else "farbled"] = r

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

        # --- Intra-session consistency. Always required, on every page: reading
        # the same canvas twice must give byte-identical results. This is the
        # assertion that fails if farbling ever mutates the canvas instead of the
        # readback, because the second read would re-flip the first read's bits.
        check("toDataURL stable across reads",
              r["small_daturl_hash"] == r["small_daturl_hash_repeat"],
              "%s vs %s" % (r["small_daturl_hash"], r["small_daturl_hash_repeat"]))
        check("getImageData stable across reads",
              r["small_pixel_hash"] == r["small_pixel_hash_repeat"],
              "%s vs %s" % (r["small_pixel_hash"], r["small_pixel_hash_repeat"]))

    # --- Cross-page behavioural proof (only meaningful once C3 has landed) -----
    # This is what actually proves native farbling runs AND that C2's per-origin
    # key reached the renderer. Nativeness above cannot show either.
    print("=" * 78)
    if args.expect_native_canvas and "exempt" in seen and "farbled" in seen:
        e, f = seen["exempt"], seen["farbled"]
        print("cross-page behavioural comparison (exempt vs farbled)")

        # CONTROL FIRST. The large canvas is outside the small-canvas gate, so it
        # is unfarbled on both pages. If it does NOT match, the two pages are not
        # rendering comparably and no verdict below can be trusted -- so this is
        # reported as its own failure rather than silently weakening the next one.
        control_ok = e["large_pixel_hash"] == f["large_pixel_hash"]
        check("CONTROL: >=65536px canvas identical on both pages", control_ok,
              "%s vs %s%s" % (e["large_pixel_hash"], f["large_pixel_hash"],
                              "" if control_ok else "  <- comparison INVALID, ignore verdicts below"))

        check("farbled page differs from exempt (getImageData)",
              e["small_pixel_hash"] != f["small_pixel_hash"],
              "exempt=%s farbled=%s" % (e["small_pixel_hash"], f["small_pixel_hash"]))
        check("farbled page differs from exempt (toDataURL)",
              e["small_daturl_hash"] != f["small_daturl_hash"],
              "exempt=%s farbled=%s" % (e["small_daturl_hash"], f["small_daturl_hash"]))
    else:
        print("cross-page behavioural comparison: SKIPPED (pre-C3; pass --expect-native-canvas)")

    print("=" * 78)
    if failures:
        print("RESULT: %d FAILED -> %s" % (len(failures), ", ".join(sorted(set(failures)))))
        return 1
    print("RESULT: all expectations met")
    return 0


if __name__ == "__main__":
    sys.exit(main())
