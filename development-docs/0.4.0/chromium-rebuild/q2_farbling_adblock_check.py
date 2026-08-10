#!/usr/bin/env python3
r"""q2_farbling_adblock_check.py — the statically-decidable half of Q2 §4 (T5, T6, T8).

Three of the eight Q2 rows can be settled without a human watching a video. The other five
(T1 blocked-count, T2 scriptlet/cosmetic injection, T3 YouTube response filter, T4
CreepJS, T7 auth-domain + adblock) need real page loads and are recorded separately.

## T6 — the `[native code]` gate. Q2 calls this "the single most valuable Q2 assertion".

It proves the farbling migration actually landed **below JavaScript**. The injected-JS
implementation could not avoid leaving a tamper tell: a JS-wrapped `toDataURL` reports its
own source from `toString()`, which is a one-line detection any anti-bot script can run.
Native Blink farbling reports `[native code]` because the function genuinely is native.

⚠️ **This assertion is trivially satisfied by deleting the feature**, which is precisely how
one of this project's earlier harnesses passed against a browser with no farbling at all.
`[native code]` is necessary, not sufficient — it is only meaningful **alongside** the
seed-rotation gate proving the values actually move. Do not cite T6 on its own as evidence
that farbling works. It is cited here as evidence that farbling is not *visible*.

**In-page negative control:** the probe wraps a function in JS and asserts the same detector
reports it as NOT native. Without that, "everything is native" is equally consistent with a
detector that always returns true — e.g. if `Function.prototype.toString` were itself
patched, which the probe also checks.

## T8 — no orphaned FP symbols

⚠️ **A naive grep FAILS against correct code here.** The retired symbols all survive as
*tombstone comments* explaining what was deleted and why — which are desirable, since they
are what stops someone re-adding an injected-JS farbling path. So the audit strips comments
before counting, and then has to prove it did not over-strip.

**The positive control is the guard set:** `IsAuthDomain`, `IsSiteEnabled`, `SetSiteEnabled`
and the `fingerprint_*_site_enabled` IPC are **shipped user-facing control** and must still
be present after stripping. Q2 T8 is explicit that this group must NOT go to zero until the
per-site toggle is re-homed (TD-5) — a T8 that went green by deleting the user's Privacy
Shield toggle would be a regression dressed as a cleanup.

## T5 — canvas-touching scriptlet double-wrap

⚠️ **Do not answer this by grepping rule text for "canvas".** The filter lists reference
scriptlets by ALIAS (`aopr`, `acs`, `set`, `nostif`, ...), so no rule contains the word
"canvas" whether or not a canvas scriptlet is in use, and that search returns a confident
zero for the wrong reason.

What determines double-wrap risk is what the scriptlet **implementations** do, because
injection can only ever run code from the available set. So the audit scans the two sets
that can actually be injected — the downloaded `resources/scriptlets.js` and the six
bundled scriptlets — for canvas/WebGL/audio APIs, each with a positive control proving the
file was read.

## Usage

    python q2_farbling_adblock_check.py --repo . --adblock-data "%APPDATA%\HodosBrowserDev\adblock"
    # add the browser rows:
    python q2_farbling_adblock_check.py --repo . --adblock-data ... \
        --exe "...\HodosBrowser.exe" --data-root "%APPDATA%\HodosBrowserDev" --dev
"""

import argparse
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

# ---- T8 -------------------------------------------------------------------------------

# Fully retired by the 2026-08-09 injected-JS deletion. Zero LIVE references expected.
RETIRED = [
    "s_domainSeeds",
    "s_fingerprintDisabledUrls",
    "fingerprint_seed",
    "FINGERPRINT_PROTECTION_SCRIPT",
    "FingerprintScript",
]

# ⛔ Shipped user-facing control. These must NOT go to zero — see the module docstring.
GUARD = [
    "IsAuthDomain",
    "IsSiteEnabled",
    "SetSiteEnabled",
    "fingerprint_get_site_enabled",
    "fingerprint_set_site_enabled",
    "FingerprintProtection",
]

SOURCE_DIRS = [("cef-native", (".cpp", ".h", ".mm", ".hpp")),
               (os.path.join("frontend", "src"), (".ts", ".tsx"))]

BLOCK_COMMENT = re.compile(r"/\*.*?\*/", re.S)
LINE_COMMENT = re.compile(r"//[^\n]*")


def strip_comments(src):
    """Remove block and line comments.

    Deliberately crude: it will also blank a `//` inside a string literal. That direction is
    SAFE for this audit — it can only cause a false "symbol absent", which for the RETIRED
    set is a false pass, but the GUARD set is checked through the identical stripper and
    would collapse first. A stripper aggressive enough to hide a retired symbol cannot leave
    the guard symbols standing."""
    return LINE_COMMENT.sub("", BLOCK_COMMENT.sub("", src))


def scan_symbols(repo):
    live = {s: [] for s in RETIRED + GUARD}
    for rel, exts in SOURCE_DIRS:
        root = os.path.join(repo, rel)
        for dirpath, _dirs, files in os.walk(root):
            for name in files:
                if not name.endswith(exts):
                    continue
                path = os.path.join(dirpath, name)
                try:
                    with open(path, "r", encoding="utf-8", errors="replace") as fh:
                        code = strip_comments(fh.read())
                except OSError:
                    continue
                for sym in live:
                    if sym in code:
                        live[sym].append(os.path.relpath(path, repo))
    return live


def run_t8(repo):
    print("### T8 — orphaned FP symbols (comments stripped)\n")
    live = scan_symbols(repo)
    ok = True

    print("  RETIRED — must have ZERO live references:")
    for sym in RETIRED:
        hits = live[sym]
        print("    %-32s %s" % (sym, "clean" if not hits else
                                "*** %d live: %s" % (len(hits), ", ".join(hits[:3]))))
        if hits:
            ok = False

    print("\n  GUARD (positive control) — must still be PRESENT:")
    for sym in GUARD:
        hits = live[sym]
        print("    %-32s %s" % (sym, "present in %d file(s)" % len(hits) if hits else
                                "*** ABSENT — either the stripper over-stripped (making every "
                                "'clean' above meaningless) or shipped user control was deleted"))
        if not hits:
            ok = False

    print("\n  T8: %s" % ("PASS" if ok else "FAIL"))
    return ok


# ---- T5 -------------------------------------------------------------------------------

CANVAS_API = (r"toDataURL|getImageData|putImageData|getChannelData|copyFromChannel|"
              r"readPixels|OffscreenCanvas|createImageBitmap|AudioContext|"
              r"OfflineAudioContext|WebGLRenderingContext|getContext")


def count_re(path, pattern):
    try:
        with open(path, "r", encoding="utf-8", errors="replace") as fh:
            return len(re.findall(pattern, fh.read()))
    except OSError:
        return None


def run_t5(repo, adblock_data):
    print("\n### T5 — canvas/WebGL/audio-touching scriptlets (double-wrap risk)\n")
    ok = True
    subjects = []

    if adblock_data:
        subjects.append(("downloaded resources/scriptlets.js",
                         os.path.join(adblock_data, "resources", "scriptlets.js"),
                         r"Object|window"))
    bundled_dir = os.path.join(repo, "adblock-engine", "src", "scriptlets")
    if os.path.isdir(bundled_dir):
        for name in sorted(os.listdir(bundled_dir)):
            if name.endswith(".js"):
                subjects.append(("bundled/" + name,
                                 os.path.join(bundled_dir, name),
                                 r"function|=>"))

    for label, path, positive in subjects:
        hits = count_re(path, CANVAS_API)
        pos = count_re(path, positive)
        if hits is None:
            print("    %-40s *** UNREADABLE: %s" % (label, path))
            ok = False
            continue
        if not pos:
            print("    %-40s *** POSITIVE CONTROL FAILED (0 matches for %r) — the file was "
                  "not really scanned, so its 0 canvas hits mean nothing" % (label, positive))
            ok = False
            continue
        print("    %-40s canvas/audio APIs=%d   (positive control=%d)" % (label, hits, pos))
        if hits:
            print("        ^ a scriptlet touching these WILL double-wrap native farbling. "
                  "Q2-1 says accept double-wrap (non-breaking) but verify the site renders.")

    print("\n  ⚠️ Method note: filter lists reference scriptlets by ALIAS, so grepping rule "
          "text for 'canvas' returns 0 whether or not one is in use. Only the implementations "
          "above can actually be injected, so they are the correct subject.")
    print("\n  T5: %s" % ("PASS — no injectable scriptlet touches canvas/WebGL/audio" if ok
                          else "FAIL"))
    return ok


# ---- T6 -------------------------------------------------------------------------------

T6_JS = r"""
(function () {
  function native(fn) {
    try { return Function.prototype.toString.call(fn).indexOf('[native code]') >= 0; }
    catch (e) { return 'ERR:' + e.message; }
  }
  var gl = null;
  try {
    var c = document.createElement('canvas');
    gl = c.getContext('webgl') || c.getContext('experimental-webgl');
  } catch (e) {}

  // ⛔ In-page NEGATIVE CONTROL: a deliberately JS-wrapped function. The same detector
  // MUST report this one as non-native, or it is not discriminating anything.
  var orig = HTMLCanvasElement.prototype.toDataURL;
  var wrapped = function toDataURL() { return orig.apply(this, arguments); };

  return JSON.stringify({
    href: location.href,
    toDataURL:      native(HTMLCanvasElement.prototype.toDataURL),
    getImageData:   native(CanvasRenderingContext2D.prototype.getImageData),
    readPixels:     native(WebGLRenderingContext.prototype.readPixels),
    getParameter:   gl ? native(Object.getPrototypeOf(gl).getParameter ||
                                WebGLRenderingContext.prototype.getParameter) : 'NOGL',
    getChannelData: native(AudioBuffer.prototype.getChannelData),
    // If Function.prototype.toString were itself patched, every answer above would be a
    // lie in the safe-looking direction.
    toStringItself: native(Function.prototype.toString),
    wrappedControl: native(wrapped)
  });
})()
"""


def run_t6(args):
    from farbling_seed_rotation_check import (kill_browser_by_path, launch_browser,
                                              measure, snapshot_targets, wait_for_cdp,
                                              engine_version)
    from farbling_cross_profile_check import cdp_port_for

    print("\n### T6 — [native code] gate (Q2's highest-value assertion)\n")
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

    excluded = snapshot_targets(port, settle=args.settle)
    # A FARBLED origin on purpose: on an auth-exempt origin the patched paths are not even
    # engaged, so [native code] there would say nothing about the farbled case.
    v = measure(port, excluded, "https://example.com/", "example.com",
                timeout=args.timeout, js=T6_JS)
    eng = engine_version(port)
    kill_browser_by_path(args.exe)

    ok = True
    checks = ["toDataURL", "getImageData", "readPixels", "getParameter", "getChannelData",
              "toStringItself"]
    for k in checks:
        val = v.get(k)
        good = (val is True) or (k == "getParameter" and val == "NOGL")
        print("    %-16s %-6s %s" % (k, str(val), "OK" if good else "*** NOT [native code]"))
        if not good:
            ok = False

    ctrl = v.get("wrappedControl")
    good_ctrl = (ctrl is False)
    print("\n    NEGATIVE CONTROL (a JS-wrapped function must read NON-native): %s  %s"
          % (ctrl, "OK" if good_ctrl else
             "*** the detector reports EVERYTHING native — it discriminates nothing"))
    if not good_ctrl:
        ok = False

    print("\n  ⚠️ [native code] is NECESSARY, NOT SUFFICIENT — deleting farbling entirely "
          "would also pass this. Cite it only alongside the seed-rotation gate.")
    print("\n  T6: %s   (engine %s)" % ("PASS" if ok else "FAIL", eng))
    return ok


def main():
    for stream in (sys.stdout, sys.stderr):
        try:
            stream.reconfigure(encoding="utf-8", errors="replace")
        except (AttributeError, ValueError):
            pass

    ap = argparse.ArgumentParser()
    ap.add_argument("--repo", default=".")
    ap.add_argument("--adblock-data", default=None,
                    help=r"e.g. %%APPDATA%%\HodosBrowserDev\adblock")
    ap.add_argument("--exe", default=None, help="omit to skip the browser row (T6)")
    ap.add_argument("--data-root", default=None)
    ap.add_argument("--profile", default="Default")
    ap.add_argument("--dev", action="store_true")
    ap.add_argument("--settle", type=float, default=8.0)
    ap.add_argument("--timeout", type=float, default=90.0)
    args = ap.parse_args()

    results = {"T8": run_t8(args.repo), "T5": run_t5(args.repo, args.adblock_data)}
    if args.exe:
        results["T6"] = run_t6(args)
    else:
        print("\n### T6 skipped (no --exe)")

    print("\n================ Q2 SUMMARY ================")
    for k in sorted(results):
        print("  %s  %s" % (k, "PASS" if results[k] else "FAIL"))
    print("\n  NOT COVERED by this script (need real page loads / a human):")
    print("    T1 adblock blocked-count increments")
    print("    T2 scriptlet + cosmetic injection fires after FP teardown")
    print("    T3 YouTube AdblockResponseFilter adPlacements rename")
    print("    T4 CreepJS worker column == window column  ⛔ KNOWN RED — all workers are")
    print("       unfarbled (P4e deferred); record as an accepted gap, do not chase")
    print("    T7 auth-domain exemption with adblock active")
    return 0 if all(results.values()) else 1


if __name__ == "__main__":
    sys.exit(main())
