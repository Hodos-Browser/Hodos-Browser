#!/usr/bin/env python3
r"""farbling_perf_check.py — the §7 "canvas/WebGL performance-regression gate".

Farbling perturbs the readback buffer, so `getImageData` and `readPixels` do measurably
more work. The question is whether the overhead is small enough that a real page does not
notice it.

## The baseline is the same machine in the same session, not a remembered number

Comparing against a figure recorded on another day or another box measures the box. Both
arms run here, back to back: arm ON is normal, arm NATIVE uses the per-site hard bypass.
The only difference between them is whether the perturbation runs.

## The null-effect control is free, and it is what makes the numbers trustworthy

Both farbled vectors have a SIZE GATE, and operations above it are never perturbed:

    canvas  200x50   = 10,000 px  -> inside the <65536px gate   -> farbled
            400x200  = 80,000 px  -> outside                    -> NOT farbled  (control)
    webgl   32x32    =  4,096 B   -> inside the <262144B gate   -> farbled
            256x256  = 262,144 B  -> on the bound, so outside    -> NOT farbled  (control)

So the large operations must show a ratio of about **1.0** between the two arms. That is a
direct measurement of this rig's timing noise, on the same APIs, in the same run. If the
control ratio is not near 1.0, the machine is too noisy for the small-operation numbers to
mean anything and the run says so instead of reporting a number.

⚠️ **A ratio is not automatically a regression.** The farbled path does strictly more work
by design; the gate is about magnitude. The threshold is deliberately a CLI argument with a
stated default rather than a constant buried in the file, because it is a product judgement
about acceptable overhead, not a measurement.

## Usage

    python farbling_perf_check.py --exe "...\HodosBrowser.exe" \
        --data-root "%APPDATA%\HodosBrowserDev" --dev [--max-ratio 3.0]
"""

import argparse
import os
import shutil
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from farbling_seed_rotation_check import (  # noqa: E402
    engine_version,
    kill_browser_by_path,
    launch_browser,
    measure,
    read_settings,
    settings_path,
    snapshot_targets,
    wait_for_cdp,
    write_settings,
)
from farbling_cross_profile_check import cdp_port_for, profile_dir  # noqa: E402

HOST = "example.com"

PERF_JS = r"""
(function () {
  function timeIt(fn, iters) {
    fn(); fn();                       // warm up: first call pays for allocation + JIT
    var t0 = performance.now();
    var sink = 0;
    for (var i = 0; i < iters; i++) { sink += fn(); }
    var ms = (performance.now() - t0) / iters;
    return {ms: ms, sink: sink};      // sink is returned so nothing is optimised away
  }

  function mkCanvas(w, h) {
    var c = document.createElement('canvas');
    c.width = w; c.height = h;
    var x = c.getContext('2d');
    x.fillStyle = '#f60'; x.fillRect(0, 0, w, h);
    x.fillStyle = '#069'; x.fillText('perf', 2, 10);
    return x;
  }
  var small2d = mkCanvas(200, 50);
  var large2d = mkCanvas(400, 200);

  function mkGl(size) {
    var c = document.createElement('canvas');
    c.width = size; c.height = size;
    var gl = c.getContext('webgl') || c.getContext('experimental-webgl');
    if (!gl) return null;
    gl.clearColor(0.25, 0.5, 0.75, 1.0);
    gl.clear(gl.COLOR_BUFFER_BIT);
    return {gl: gl, buf: new Uint8Array(size * size * 4), size: size};
  }
  var smallGl = mkGl(32);
  var largeGl = mkGl(256);

  function glRead(g) {
    if (!g) return function () { return 0; };
    return function () {
      g.gl.readPixels(0, 0, g.size, g.size, g.gl.RGBA, g.gl.UNSIGNED_BYTE, g.buf);
      return g.buf[0];
    };
  }

  var r = {
    href: location.href,
    canvasSmall: timeIt(function () { return small2d.getImageData(0, 0, 200, 50).data[0]; }, 200).ms,
    canvasLarge: timeIt(function () { return large2d.getImageData(0, 0, 400, 200).data[0]; }, 60).ms,
    glSmall: timeIt(glRead(smallGl), 200).ms,
    glLarge: timeIt(glRead(largeGl), 60).ms,
    haveGl: !!smallGl
  };
  return JSON.stringify(r);
})()
"""

FARBLED_OPS = [("canvasSmall", "getImageData 200x50   (farbled)"),
               ("glSmall", "readPixels   32x32    (farbled)")]
CONTROL_OPS = [("canvasLarge", "getImageData 400x200  (control, above gate)"),
               ("glLarge", "readPixels   256x256  (control, above gate)")]


def run_arm(label, args, bypass):
    pdir = profile_dir(args.data_root, args.profile)
    port = cdp_port_for(args.profile, args.dev)
    kill_browser_by_path(args.exe)

    doc = read_settings(pdir)
    sites = doc.get("siteSettings")
    if not isinstance(sites, dict):
        sites = {}
    sites.pop(HOST, None)
    if bypass:
        sites[HOST] = {"enabled": False}
    doc["siteSettings"] = sites
    write_settings(pdir, doc)

    for attempt in range(1, 4):
        if attempt > 1:
            kill_browser_by_path(args.exe)
        launch_browser(args.exe, args.dev, args.profile)
        if wait_for_cdp(port):
            break
    else:
        raise SystemExit("CDP %d never came up" % port)

    excluded = snapshot_targets(port, settle=args.settle)
    best = None
    for i in range(args.repeats):
        v = measure(port, excluded, "https://%s/" % HOST, HOST,
                    timeout=args.timeout, js=PERF_JS)
        # Take the MINIMUM across repeats, not the mean: timing noise on a desktop is
        # one-sided (other work only ever makes a run slower), so the minimum is the
        # cleanest estimate of the true cost and the mean mostly measures the background.
        if best is None:
            best = v
        else:
            for k, _ in FARBLED_OPS + CONTROL_OPS:
                best[k] = min(best[k], v[k])
    print("    %-22s canvasSmall=%.4fms glSmall=%.4fms canvasLarge=%.4fms glLarge=%.4fms"
          % (label, best["canvasSmall"], best["glSmall"],
             best["canvasLarge"], best["glLarge"]))
    return best, engine_version(port)


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
    ap.add_argument("--timeout", type=float, default=90.0)
    ap.add_argument("--repeats", type=int, default=3,
                    help="page measurements per arm; the minimum is taken")
    ap.add_argument("--max-ratio", type=float, default=3.0,
                    help="fail if a farbled op costs more than this multiple of native "
                         "(default 3.0 — a product judgement, not a measurement)")
    ap.add_argument("--control-tolerance", type=float, default=0.35,
                    help="how far the above-gate control ratio may sit from 1.0 before the "
                         "rig is declared too noisy to trust (default 0.35)")
    args = ap.parse_args()

    pdir = profile_dir(args.data_root, args.profile)
    path = settings_path(pdir)
    backup = None
    if os.path.exists(path):
        backup = path + ".perf-backup"
        shutil.copy2(path, backup)

    try:
        print("=== timing both arms on the same machine, back to back ===============")
        native, eng = run_arm("NATIVE (bypassed)", args, bypass=True)
        farbled, _ = run_arm("FARBLED", args, bypass=False)
    finally:
        kill_browser_by_path(args.exe)
        if backup and os.path.exists(backup):
            shutil.copy2(backup, path)
            os.remove(backup)
            print("\nrestored %s" % path)

    if not farbled.get("haveGl"):
        print("\n*** no WebGL context — the WebGL rows below are not measurements.")

    print("\n================ RESULTS (engine %s) ================" % eng)
    print("  %-42s %10s %10s %8s" % ("operation", "native", "farbled", "ratio"))
    ok = True

    print("\n  above the size gate — NOT farbled, so these measure rig noise:")
    control_bad = False
    for key, label in CONTROL_OPS:
        n, f = native[key], farbled[key]
        ratio = (f / n) if n else float("inf")
        off = abs(ratio - 1.0)
        flag = "OK" if off <= args.control_tolerance else "*** NOISY"
        if off > args.control_tolerance:
            control_bad = True
        print("  %-42s %9.4fms %9.4fms %7.2fx %s" % (label, n, f, ratio, flag))

    if control_bad:
        print("\n  *** The above-gate controls should sit near 1.0x — they are not farbled in")
        print("      either arm. They do not, so this machine's timing noise is larger than")
        print("      the effect being measured and the numbers below cannot be trusted.")
        print("      Re-run on a quiet machine, or raise --repeats, before reading a verdict.")
        ok = False

    print("\n  inside the size gate — farbled, so these carry the real overhead:")
    for key, label in FARBLED_OPS:
        n, f = native[key], farbled[key]
        ratio = (f / n) if n else float("inf")
        flag = "OK" if ratio <= args.max_ratio else "*** OVER BUDGET"
        if ratio > args.max_ratio:
            ok = False
        print("  %-42s %9.4fms %9.4fms %7.2fx %s" % (label, n, f, ratio, flag))

    print("\n  budget: %.2fx (--max-ratio). This is a product judgement about acceptable"
          % args.max_ratio)
    print("  overhead, not something the measurement decides — change it deliberately.")
    print("\nVERDICT: %s" % ("PASS" if ok else "FAIL"))
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
