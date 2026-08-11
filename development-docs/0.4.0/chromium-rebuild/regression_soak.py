#!/usr/bin/env python3
r"""regression_soak.py — the §7 Thorough regression basket AND the stability soak, in one rig.

They are the same rotation of sites. Pass 1 runs the full per-site assertions and is the
**regression basket**; passes 2..N re-run the rotation and count failures, and that is the
**soak**. Doing it as one process avoids two harnesses fighting over one browser — and
`kill_browser_by_path` kills every dev process, so two concurrent drivers cannot coexist.

## ⚠️ Renderer crashes are detected by PROBING, not by reading the log

There is **no `OnRenderProcessTerminated` handler anywhere in this codebase**, so a renderer
crash writes nothing to `debug_output.log`. A soak that grepped the log for crashes would
report a confident zero forever — the exact false-green shape this project keeps hitting.

Instead every page is probed with JavaScript after it loads. A dead renderer cannot answer,
so a probe that fails to return a value is the crash signal. That is why the probe is
deliberately trivial: it must fail only when the renderer is genuinely unable to respond.

## ⚠️ What "PASS" means here, and what it does not

The roadmap asks for a crash rate **versus the M136 baseline**. We cannot produce that
number: the M136 build is not installed here and we ship **no telemetry**, so there is no
baseline to subtract. What this rig produces is an **absolute** figure — failures per N page
loads on this build. Reporting it as a delta against 136 would be inventing the comparison.
Stated here so nobody quotes it as one.

## Per-site assertions (pass 1)

  * the final URL is on the host we asked for (or a declared, expected redirect)
  * `document.title` is non-empty
  * rendered text exceeds a floor — catches blank pages and interstitials that return 200
  * the page is not a browser error page

Screenshots of every site are written next to the report so the result can be eyeballed;
a page can satisfy all four assertions and still look wrong, and no automated check in this
file is a substitute for a human glancing at it.

## Usage

    python regression_soak.py --exe "...\HodosBrowser.exe" \
        --data-root "%APPDATA%\HodosBrowserDev" --dev --passes 12 --out <dir>
"""

import argparse
import base64
import json
import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from farbling_seed_rotation_check import (  # noqa: E402
    engine_version,
    kill_browser_by_path,
    launch_browser,
    measure,
    resolve_tab,
    snapshot_targets,
    wait_for_cdp,
)
from farbling_cross_profile_check import cdp_port_for  # noqa: E402

try:
    import websocket
except ImportError:
    sys.exit("need websocket-client:  pip install websocket-client")

# CLAUDE.md Testing Standards, Thorough tier. `expect` is the host the tab must actually
# land on — several of these redirect, and asserting the requested host would fail for a
# reason that has nothing to do with the build.
#
# The 4th field is a per-site text floor. It exists for ONE measured reason: google.com's
# homepage renders 147 characters of innerText — deliberately, since it is a logo, a search
# box and a footer — and it did so on all 14 passes of the 2026-08-10 soak, identically.
# The screenshot shows a perfectly rendered, signed-in page.
#
# ⚠️ This is NOT the global floor being lowered to make a row go green. Dropping the floor
# for every site would weaken nine working assertions to accommodate one sparse page. The
# override keeps real discriminating power on that row too: a blank or broken google.com
# renders ~0 characters and still fails against a floor of 100.
BASKET = [
    ("Auth",         "https://x.com/",                  "x.com",           None),
    ("Auth",         "https://www.google.com/",         "google.com",      100),
    ("Auth",         "https://github.com/",             "github.com",      None),
    ("Video/Media",  "https://www.youtube.com/",        "youtube.com",     None),
    ("Video/Media",  "https://www.twitch.tv/",          "twitch.tv",       None),
    ("News",         "https://www.nytimes.com/",        "nytimes.com",     None),
    ("News",         "https://www.reddit.com/",         "reddit.com",      None),
    ("E-commerce",   "https://www.amazon.com/",         "amazon.com",      None),
    ("Productivity", "https://docs.google.com/",        "google.com",      None),
    ("BSV",          "https://whatsonchain.com/",       "whatsonchain.com", None),
]

# Trivial on purpose: it must fail ONLY when the renderer genuinely cannot answer.
PROBE_JS = r"""
(async function () {
  // ⚠️ POLL for rendered content. `measure()` returns as soon as the host appears in the
  // URL, and on an SPA that is the SPLASH SCREEN — x.com and youtube.com both answered with
  // 0 characters of body text on the first run of this harness, and the screenshots showed
  // the X logo on black. Read once and every SPA in the basket is reported as broken.
  // The deadline stays under measure()'s 25 s evaluate window.
  var deadline = Date.now() + 16000;
  var t = '';
  while (Date.now() < deadline) {
    t = document.body ? (document.body.innerText || '') : '';
    if (t.length >= 200 && document.readyState !== 'loading') break;
    await new Promise(function (r) { setTimeout(r, 500); });
  }
  return JSON.stringify({
    href: location.href,
    title: (document.title || '').slice(0, 120),
    textLen: t.length,
    readyState: document.readyState,
    // Chromium's own error pages carry this; a 200 that renders an interstitial will not
    // be caught here and is what the screenshots are for.
    isErrorPage: /ERR_|This site can.t be reached|No internet/i.test(t.slice(0, 400))
  });
})()
"""

MIN_TEXT = 200          # below this a page is blank or an interstitial


def shoot(port, excluded, path):
    """Best-effort screenshot. Never fails the run — it is review material, not a gate."""
    try:
        t = resolve_tab(port, excluded)
        ws = websocket.create_connection(t["webSocketDebuggerUrl"], timeout=25)
        try:
            ws.send(json.dumps({"id": 90, "method": "Page.captureScreenshot",
                                "params": {"format": "png"}}))
            end = time.time() + 20
            while time.time() < end:
                m = json.loads(ws.recv())
                if m.get("id") == 90:
                    data = m.get("result", {}).get("data")
                    if data:
                        with open(path, "wb") as fh:
                            fh.write(base64.b64decode(data))
                    return
        finally:
            ws.close()
    except Exception:                                  # noqa: BLE001
        pass


def boot(args):
    port = cdp_port_for(args.profile, args.dev)
    kill_browser_by_path(args.exe)
    for attempt in range(1, 4):
        if attempt > 1:
            kill_browser_by_path(args.exe)
        launch_browser(args.exe, args.dev, args.profile)
        if wait_for_cdp(port):
            break
    else:
        raise SystemExit("CDP %d never came up" % port)
    return port, snapshot_targets(port, settle=args.settle)


def main():
    for stream in (sys.stdout, sys.stderr):
        try:
            stream.reconfigure(encoding="utf-8", errors="replace")
        except (AttributeError, ValueError):
            pass

    ap = argparse.ArgumentParser()
    ap.add_argument("--exe", required=True)
    ap.add_argument("--data-root", required=True)
    ap.add_argument("--profile", default="Default")
    ap.add_argument("--dev", action="store_true")
    ap.add_argument("--settle", type=float, default=8.0)
    ap.add_argument("--timeout", type=float, default=75.0)
    ap.add_argument("--passes", type=int, default=12,
                    help="pass 1 is the regression basket; the rest are the soak")
    ap.add_argument("--out", default=None, help="directory for screenshots + report")
    args = ap.parse_args()

    out = args.out or os.path.join(os.path.dirname(os.path.abspath(__file__)), "soak_out")
    os.makedirs(out, exist_ok=True)

    port, excluded = boot(args)
    eng = engine_version(port)
    print("engine %s — %d sites x %d passes, output -> %s"
          % (eng, len(BASKET), args.passes, out), flush=True)

    basket = []
    failures = []          # (pass, host, why) across ALL passes
    loads = 0

    for p in range(1, args.passes + 1):
        print("\n===== pass %d/%d =====" % (p, args.passes), flush=True)
        for cat, url, host, floor in BASKET:
            loads += 1
            why = None
            v = None
            try:
                v = measure(port, excluded, url, host, timeout=args.timeout, js=PROBE_JS)
            except (Exception, SystemExit) as exc:      # noqa: BLE001
                # A renderer that cannot answer is the crash signal — see the docstring.
                why = "no probe response (%s)" % str(exc)[:90]

            if v is not None:
                if v["isErrorPage"]:
                    why = "browser error page"
                elif v["textLen"] < (floor or MIN_TEXT):
                    why = ("rendered only %d chars, floor %d (blank/interstitial?)"
                           % (v["textLen"], floor or MIN_TEXT))
                elif not v["title"]:
                    why = "empty document.title"

            status = "ok " if why is None else "FAIL"
            print("  [%s] %-13s %-22s %s %s"
                  % (status, cat, host,
                     ("%5d chars  %s" % (v["textLen"], v["title"][:44])) if v else "",
                     ("<- " + why) if why else ""), flush=True)

            if why:
                failures.append((p, host, why))

            if p == 1:
                basket.append({"category": cat, "host": host, "url": url,
                               "ok": why is None, "why": why,
                               "title": (v or {}).get("title"),
                               "textLen": (v or {}).get("textLen")})
                # ⚠️ Key the filename on the CATEGORY + host, not the host alone: two basket
                # rows resolve to google.com (google.com and docs.google.com), and keying on
                # the host silently overwrote the first with the second — so the review
                # screenshot showed the wrong page for the row that actually failed.
                shoot(port, excluded,
                      os.path.join(out, "%s__%s.png"
                                   % (cat.replace("/", "-"), host.replace(".", "_"))))

    # ---- report ----------------------------------------------------------------------
    print("\n================ RESULTS ================")
    print("\nREGRESSION BASKET (pass 1, Thorough tier):")
    basket_fail = [b for b in basket if not b["ok"]]
    for b in basket:
        print("  %-4s %-13s %-22s %s"
              % ("ok" if b["ok"] else "FAIL", b["category"], b["host"], b["why"] or ""))
    print("  -> %d/%d sites rendered" % (len(basket) - len(basket_fail), len(basket)))
    print("  screenshots in %s — a page can pass every assertion above and still look "
          "wrong; glance at them." % out)

    print("\nSTABILITY SOAK:")
    print("  %d page loads across %d passes, %d failures" % (loads, args.passes, len(failures)))
    if failures:
        seen = {}
        for _p, host, why in failures:
            seen.setdefault(host, []).append(why)
        for host, whys in sorted(seen.items()):
            print("    %-22s %d x  e.g. %s" % (host, len(whys), whys[0][:70]))
    print("  ⚠️ ABSOLUTE figure only. There is no M136 baseline on this machine and we ship")
    print("     no telemetry, so this is NOT a delta against 136 — do not quote it as one.")

    report = {"engine": eng, "passes": args.passes, "loads": loads,
              "basket": basket, "failures": failures}
    with open(os.path.join(out, "report.json"), "w", encoding="utf-8") as fh:
        json.dump(report, fh, indent=2)
    print("\nreport -> %s" % os.path.join(out, "report.json"))

    kill_browser_by_path(args.exe)
    # The soak gates on stability; a basket site being unreachable from this network is not
    # a build defect, so the exit code follows the soak and the basket is reported for the
    # human to judge.
    return 0 if not failures else 1


if __name__ == "__main__":
    sys.exit(main())
