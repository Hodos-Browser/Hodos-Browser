#!/usr/bin/env python3
r"""p13_signals.py -- Phase 13 Block A2 + Block B: the instrument-INSENSITIVE signal sheet.

⛔ SUBJECT. This harness does NOT resolve its own tab. It imports `snapshot_targets` +
`measure` from `farbling_seed_rotation_check`, which already carry the chrome-id exclusion
and the href re-check -- the exact machinery that exists because every wrong-subject defect
in this family came from hand-rolling it. Hodos's header and ~14 overlays all report
type:"page" to CDP; a hand-rolled picker drives one of them and reports a confident number
about the wrong browser. (Observed again on the first run of this very file.)

⛔ INSTRUMENT. Everything measured here is independent of whether a debugger is attached:
a UA string, a WebGL renderer string, a plugin list and a `toString()` result do not change
because CDP is open. That is the argument for driving this over CDP. It does NOT extend to
Phase 13's challenge verdicts, which are behaviour-scored and stay human-driven.

Two jobs:
  A2  replay the 0.3.x injected-JS farbling script (extracted verbatim from
      v0.3.0-beta.29:cef-native/include/core/FingerprintScript.h) in the CURRENT engine and
      measure the tells it creates, reading the SAME probe before and after in ONE page
      context so no reload can wipe the injection.
      ⛔ LIMIT: this is the 0.3.x SCRIPT in a 0.4.0 engine, not the 0.3.x BINARY. It
      demonstrates the mechanism; it is not the A/B against the old build and must never
      be reported as one.
  B   dump the signal families (automation / environment / fingerprint / modified-env) so
      Hodos and Chrome can be diffed on the same machine, on the same day.

Usage (browser already running):
    python p13_signals.py --port 9322 --label hodos  --out hodos.json
    python p13_signals.py --port 9322 --replay-03x
    python p13_signals.py --diff hodos.json chrome.json
"""
import argparse
import json
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = r"C:\Users\archb\Hodos-Browser"
sys.path.insert(0, os.path.join(REPO, "development-docs", "0.4.0", "chromium-rebuild"))

from farbling_seed_rotation_check import (  # noqa: E402
    check_role_in_log,
    measure,
    snapshot_targets,
)

# The shell's own record of which browser served the URL. Passing a DIRECTORY makes
# check_role_in_log take the newest debug_output-<pid>.log, which is what the per-PID
# logging since beta.3 Phase 2 requires.
DEV_LOG_DIR = r"C:\Users\archb\AppData\Roaming\HodosBrowserDev\logs"


def assert_tab(host, log_dir=DEV_LOG_DIR):
    """⛔ HARD GATE. `resolve_tab`'s ambiguity check is NOT sufficient on its own.

    Measured 2026-09-21: an overlay (`tab-list`) opened AFTER `snapshot_targets` froze the
    landscape and the pinned tab's id went away, leaving the overlay as the sole candidate.
    `resolve_tab` saw one candidate, did not fire, and the run produced a full, confident
    signal sheet about a 340x480 popup -- including a `screen` of [340,480] that read
    exactly like the degenerate-metrics headless signature it was not.

    The shell's own log is the only instrument that names the role, so it is the gate.
    """
    role = check_role_in_log(log_dir, host)
    if role is None:
        raise SystemExit("SUBJECT UNVERIFIABLE: no 'role:' line for %s in %s" % (host, log_dir))
    if not role.startswith("tab_"):
        raise SystemExit("SUBJECT WRONG: %s was served to role=%s, not a tab. "
                         "Close the overlay and re-run." % (host, role))
    print("    SUBJECT OK: shell served %s to role=%s" % (host, role))
    return role


# --------------------------------------------------------------------- probe

# `measure` requires the expression to return a JSON STRING carrying an `href` field --
# that is how it asserts it read the page it navigated, not a stale one.
PROBE_BODY = r"""
  function glInfo(kind) {
    try {
      var c = document.createElement('canvas');
      var gl = c.getContext(kind) || c.getContext('experimental-' + kind);
      if (!gl) return null;
      var dbg = gl.getExtension('WEBGL_debug_renderer_info');
      return {
        vendor: gl.getParameter(gl.VENDOR),
        renderer: gl.getParameter(gl.RENDERER),
        version: gl.getParameter(gl.VERSION),
        unmaskedVendor: dbg ? gl.getParameter(dbg.UNMASKED_VENDOR_WEBGL) : null,
        unmaskedRenderer: dbg ? gl.getParameter(dbg.UNMASKED_RENDERER_WEBGL) : null,
        maxTexture: gl.getParameter(gl.MAX_TEXTURE_SIZE),
        extCount: (gl.getSupportedExtensions() || []).length
      };
    } catch (e) { return {error: String(e)}; }
  }
  function ts(fn) {
    try { return Function.prototype.toString.call(fn).indexOf('[native code]') >= 0; }
    catch (e) { return 'ERR:' + e; }
  }
  function snap() {
    var plugins = [];
    for (var i = 0; i < navigator.plugins.length; i++) plugins.push(navigator.plugins[i].name);
    return {
      href: location.href,
      /* family A -- automation tells */
      webdriver: navigator.webdriver,
      hasChrome: (typeof window.chrome === 'object' && window.chrome !== null),
      chromeKeys: window.chrome ? Object.keys(window.chrome) : [],
      cdcKeys: Object.getOwnPropertyNames(window).filter(function (k) {
        return /^\$?cdc_|^\$chrome_|_selenium|_Selenium_IDE_Recorder|__webdriver|__driver|__nightmare/.test(k);
      }),
      pluginCount: navigator.plugins.length,
      pluginNames: plugins,
      mimeTypeCount: navigator.mimeTypes.length,
      /* family B -- environment consistency */
      userAgent: navigator.userAgent,
      platform: navigator.platform,
      vendorStr: navigator.vendor,
      language: navigator.language,
      languages: navigator.languages,
      uaDataBrands: (navigator.userAgentData ? navigator.userAgentData.brands : null),
      uaDataPlatform: (navigator.userAgentData ? navigator.userAgentData.platform : null),
      uaDataMobile: (navigator.userAgentData ? navigator.userAgentData.mobile : null),
      timezone: Intl.DateTimeFormat().resolvedOptions().timeZone,
      locale: Intl.DateTimeFormat().resolvedOptions().locale,
      screen: [screen.width, screen.height, screen.availWidth, screen.availHeight, screen.colorDepth],
      windowInner: [innerWidth, innerHeight],
      windowOuter: [outerWidth, outerHeight],
      dpr: devicePixelRatio,
      /* family C -- fingerprint plausibility */
      deviceMemory: navigator.deviceMemory,
      hardwareConcurrency: navigator.hardwareConcurrency,
      maxTouchPoints: navigator.maxTouchPoints,
      pdfViewerEnabled: navigator.pdfViewerEnabled,
      webgl1: glInfo('webgl'),
      webgl2: glInfo('webgl2'),
      /* family G -- modified-environment tells */
      toStringNative: {
        getImageData: ts(CanvasRenderingContext2D.prototype.getImageData),
        toDataURL:    ts(HTMLCanvasElement.prototype.toDataURL),
        toBlob:       ts(HTMLCanvasElement.prototype.toBlob),
        readPixels:   ts(WebGLRenderingContext.prototype.readPixels),
        getChannelData: ts(AudioBuffer.prototype.getChannelData),
        getFloatFrequencyData: ts(AnalyserNode.prototype.getFloatFrequencyData)
      },
      navigatorOwnProps: Object.getOwnPropertyNames(navigator),
      hodosGlobals: Object.getOwnPropertyNames(window).filter(function (k) {
        return /hodos|CWI|cefMessage|babbage|metanet/i.test(k);
      }),
      cookieEnabled: navigator.cookieEnabled
    };
  }
"""

PROBE_B = "(function(){\n" + PROBE_BODY + "\n  return JSON.stringify(snap());\n})()"

# Network / TLS family. tls.peet.ws echoes the ClientHello (JA3/JA4/PeetPrint) AND the
# request header block of the TOP-LEVEL navigation, so one page answers both "does our
# TLS stack look like Chrome's" and "what headers does the engine actually send".
# ⛔ It must be a real navigation, not fetch(): an XHR sends a different Accept and
# Sec-Fetch-* set, which would measure the wrong request.
PROBE_TLS = ("(function(){ return JSON.stringify("
             "{href: location.href, body: document.body ? document.body.innerText : ''}); })()")


def probe_a2():
    """One expression: snapshot, inject the verbatim 0.3.x script, snapshot again.

    Both reads happen in the SAME page context, so nothing between them can reload the
    document and silently wipe the injection -- which is what a two-call version would do,
    because `measure` navigates on every call.
    """
    js = open(os.path.join(HERE, "farble_0.3.x.js"), encoding="utf-8").read()
    js = js.replace("FINGERPRINT_SEED", "123456789")
    return ("(function(){\n" + PROBE_BODY +
            "\n  var before = snap();\n" +
            "  var injectError = null;\n" +
            "  try {\n" + js + "\n  } catch (e) { injectError = String(e); }\n" +
            "  var after = snap();\n" +
            "  return JSON.stringify({href: before.href, before: before,"
            " after: after, injectError: injectError});\n})()")


# ---------------------------------------------------------------------- report

def print_a2(res):
    before, after = res["before"], res["after"]
    print("")
    print("=== A2 -- the 0.3.x injected-JS script replayed in the CURRENT engine ===")
    print("    source : v0.3.0-beta.29 FINGERPRINT_PROTECTION_SCRIPT, verbatim (seed substituted)")
    print("    subject: %s" % before["href"])
    print("    inject : %s" % (res["injectError"] or "clean"))
    print("    LIMIT  : the 0.3.x SCRIPT in a 0.4.0 engine -- NOT a run of the 0.3.x binary.")
    print("")
    print("    %-44s %-11s %-14s" % ("tell", "0.4.0 now", "after 0.3.x JS"))
    tells = 0
    for k in sorted(before["toStringNative"]):
        b, a = before["toStringNative"][k], after["toStringNative"][k]
        flag = ""
        if b != a:
            tells += 1
            flag = "<<< TELL"
        print("    %-44s %-11s %-14s %s"
              % ("toString reports [native code]: " + k, b, a, flag))
    added = sorted(set(after["navigatorOwnProps"]) - set(before["navigatorOwnProps"]))
    if added:
        tells += 1
    print("    %-44s %-11s %-14s %s"
          % ("navigator OWN property count", len(before["navigatorOwnProps"]),
             len(after["navigatorOwnProps"]),
             ("<<< TELL  added=%s" % added) if added else ""))
    print("    %-44s %-11s %-14s" % ("navigator.webdriver", before["webdriver"], after["webdriver"]))
    print("    %-44s %-11s %-14s" % ("navigator.plugins length", before["pluginCount"], after["pluginCount"]))
    print("")
    print("    distinct tells the 0.3.x implementation created: %d" % tells)
    print("    ⇒ NEGATIVE CONTROL for the 0.4.0 column: the same probe, same tab, same run,")
    print("      goes red the moment the old implementation is present. A 0.4.0 'all native'")
    print("      is therefore a measurement and not a tautology.")
    return tells


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--port", type=int, default=9322)
    ap.add_argument("--label", default="hodos")
    ap.add_argument("--url", default="https://example.com/")
    ap.add_argument("--host", default=None, help="host expected in the tab URL (default: from --url)")
    ap.add_argument("--settle", type=float, default=6.0)
    ap.add_argument("--timeout", type=float, default=90.0)
    ap.add_argument("--out")
    ap.add_argument("--replay-03x", action="store_true")
    ap.add_argument("--tls", action="store_true")
    ap.add_argument("--diff", nargs=2)
    a = ap.parse_args()

    for stream in (sys.stdout, sys.stderr):
        try:
            stream.reconfigure(encoding="utf-8", errors="replace")
        except (AttributeError, ValueError):
            pass

    if a.diff:
        x = json.load(open(a.diff[0]))
        y = json.load(open(a.diff[1]))
        xs, ys = x["signals"], y["signals"]
        print("%-3s%-22s | %-46s | %s" % ("", "signal", x["label"], y["label"]))
        print("-" * 132)
        for k in sorted(set(xs) | set(ys)):
            vx, vy = xs.get(k), ys.get(k)
            print("%s%-22s | %-46s | %s"
                  % ("   " if vx == vy else ">> ", k, str(vx)[:46], str(vy)[:58]))
        return 0

    host = a.host or a.url.split("//", 1)[-1].split("/", 1)[0]
    excluded = snapshot_targets(a.port, settle=a.settle)
    js = probe_a2() if a.replay_03x else (PROBE_TLS if a.tls else PROBE_B)
    res = measure(a.port, excluded, a.url, host, timeout=a.timeout, js=js)
    assert_tab(host)

    if a.tls:
        print(res["body"])
        out = {"label": a.label, "kind": "tls", "result": res}
    elif a.replay_03x:
        print_a2(res)
        out = {"label": a.label, "kind": "A2-replay", "result": res}
    else:
        print("")
        print("=== Block B signal sheet -- %s (port %d) ===" % (a.label, a.port))
        print(json.dumps(res, indent=2))
        out = {"label": a.label, "port": a.port, "kind": "B-signals", "signals": res}

    if a.out:
        json.dump(out, open(a.out, "w"), indent=2)
        print("")
        print("wrote %s" % a.out)
    return 0


if __name__ == "__main__":
    sys.exit(main())
