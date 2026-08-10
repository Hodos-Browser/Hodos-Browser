#!/usr/bin/env python3
r"""farbling_acceptance_battery.py — four §7 acceptance rows that all fit in one rig.

Grouped because they share a launch, not because they are conceptually one thing:

  1. **Intra-session consistency** — the same page read twice in one session must give
     IDENTICAL values. A farbling implementation that re-draws from the PRNG per call
     leaks a fresh sample on every read, which is *worse* than not farbling: a site can
     average the noise away and recover the true value.
  2. **Navigator values in a valid set** — `deviceMemory` must be one of the four values
     the web platform actually exposes and `hardwareConcurrency` must not exceed the real
     core count. A farbled value outside the legal set is a fingerprint in itself.
  3. **BOT-1** — `navigator.webdriver === false` and the `window.chrome` stub survived the
     deletion of the injected JS block.
  4. **Q3 T8 — the global toggle**, which gets its own phases because it needs restarts.

## The equality assertions carry their own sensitivity control

"Read twice, values identical" is satisfied trivially if the measurement is broken, if the
page never loaded, or if we are driving the wrong browser. So phase 1 also measures a
SECOND origin and requires it to DIFFER. If two different domains produce the same values,
the equality result above proves nothing and the run is reported RED.

## T8 is cross-validated against a second, independent route to "native"

Turning the global toggle off must not merely *change* the values — it must land on the
true native ones. Phase 2 reaches native by the per-site hard bypass
(`siteSettings[host]={"enabled":false}`) with the global toggle still ON; phase 3 reaches it
by the global toggle. Two different code paths agreeing on the same native values is much
stronger than either alone, and it is the same cross-check that validated T2.

Note what phase 3 also proves incidentally: with the global toggle off, `OnBeforeBrowse`
sends **no** `hodos_farble_key` message at all, so the renderer installs no key. Landing on
native therefore demonstrates the renderer **fails closed** rather than falling back to some
partially-initialised key.

## ⛔ NEGATIVE CONTROL (mandatory — CLAUDE.md Testing Standards)

Rows 1 and 4 carry theirs in-run, as described above: remove the thing under test and the
run goes red on the sensitivity control or on the two-route disagreement.

Rows 2 and 3 are *validators*, not measurements — the only way they can be wrong is by
accepting bad input. `--self-test` feeds them values that must be rejected (deviceMemory=7,
cores above the machine's real count, webdriver=true, a missing window.chrome) and fails if
any is accepted. Run it whenever the validators are touched; it needs no browser.

## Usage

    pip install websocket-client

    python farbling_acceptance_battery.py --self-test        # no browser needed

    python farbling_acceptance_battery.py \
        --exe "C:\...\cef-native\build\bin\Release\HodosBrowser.exe" \
        --data-root "%APPDATA%\HodosBrowserDev" \
        --dev --log "%APPDATA%\HodosBrowserDev\logs\debug_output.log"

⚠️ The browser is killed by executable PATH, never image name.
⚠️ `settings.json` lives at the DATA ROOT, not inside the profile — it is backed up and
restored, including on failure.
"""

import argparse
import json
import os
import shutil
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from farbling_seed_rotation_check import (  # noqa: E402
    check_role_in_log,
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

# Both non-exempt on purpose: this file is about farbled behaviour, and an auth-exempt
# origin would sit at native in every phase and make every assertion pass vacuously.
HOST_A = "example.com"
HOST_B = "iana.org"

# The set C6 is allowed to report. A farbled value outside it is not "more random", it is
# a tell — so this is an equality-to-a-set check, not a range check.
EXPECTED_DEVICE_MEMORY = {4, 8, 16, 32}

FARBLED_FIELDS = ["small", "glSmall", "audio", "deviceMemory", "cores"]

BOT_JS = r"""
(function () {
  var c = window.chrome;
  return JSON.stringify({
    href: location.href,
    webdriver: navigator.webdriver,
    webdriverType: typeof navigator.webdriver,
    hasChrome: (typeof c === 'object' && c !== null),
    chromeKeys: c ? Object.keys(c).slice(0, 12) : [],
    // The injected-JS farbling block is gone; these must report native again or the
    // toString tamper tell is back (Q2 T6 is the dedicated gate, this is the cheap echo).
    getImageDataNative:
      (CanvasRenderingContext2D.prototype.getImageData.toString().indexOf('[native code]') >= 0),
    readPixelsNative:
      (WebGLRenderingContext.prototype.readPixels.toString().indexOf('[native code]') >= 0)
  });
})()
"""


# --------------------------------------------------------------------------------------
# validators — pure, so --self-test can exercise them with no browser
# --------------------------------------------------------------------------------------

def check_navigator(device_memory, cores, real_cores):
    """Return a list of failure strings; empty means valid."""
    bad = []
    if device_memory not in EXPECTED_DEVICE_MEMORY:
        bad.append("deviceMemory=%r not in %s" % (device_memory, sorted(EXPECTED_DEVICE_MEMORY)))
    if not isinstance(cores, int) or cores < 1:
        bad.append("hardwareConcurrency=%r is not a positive integer" % (cores,))
    elif real_cores and cores > real_cores:
        bad.append("hardwareConcurrency=%d exceeds the machine's real core count (%d) — a "
                   "farbled value must never claim MORE hardware than exists"
                   % (cores, real_cores))
    return bad


def check_bot1(probe):
    """Return a list of failure strings; empty means valid."""
    bad = []
    if probe.get("webdriver") is not False:
        bad.append("navigator.webdriver=%r (expected exactly False); automation is "
                   "advertised to every site" % (probe.get("webdriver"),))
    if not probe.get("hasChrome"):
        bad.append("window.chrome is missing — the stub did not survive the injected-JS "
                   "deletion, and its absence is itself a bot signal")
    if not probe.get("getImageDataNative"):
        bad.append("getImageData.toString() does not report [native code]")
    if not probe.get("readPixelsNative"):
        bad.append("readPixels.toString() does not report [native code]")
    return bad


def self_test(real_cores):
    """⛔ The negative control for the two validators above."""
    print("SELF-TEST — every case below MUST be rejected\n")
    cases = [
        ("navigator", lambda: check_navigator(7, 8, real_cores), "deviceMemory=7 (not on the ladder)"),
        ("navigator", lambda: check_navigator(32, real_cores + 1, real_cores),
         "cores one above the real count"),
        ("navigator", lambda: check_navigator(32, 0, real_cores), "cores=0"),
        ("navigator", lambda: check_navigator(None, 8, real_cores), "deviceMemory missing"),
        ("bot1", lambda: check_bot1({"webdriver": True, "hasChrome": True,
                                     "getImageDataNative": True, "readPixelsNative": True}),
         "webdriver=true"),
        ("bot1", lambda: check_bot1({"webdriver": False, "hasChrome": False,
                                     "getImageDataNative": True, "readPixelsNative": True}),
         "window.chrome absent"),
        ("bot1", lambda: check_bot1({"webdriver": False, "hasChrome": True,
                                     "getImageDataNative": False, "readPixelsNative": True}),
         "getImageData not [native code]"),
    ]
    ok = True
    for kind, fn, desc in cases:
        bad = fn()
        status = "rejected OK" if bad else "*** ACCEPTED — VALIDATOR IS BLIND"
        print("  %-9s %-38s %s" % (kind, desc, status))
        if not bad:
            ok = False

    # And a positive control: a legitimate reading must be ACCEPTED, or the validators
    # reject everything and their rejections above mean nothing.
    print()
    pos = [
        ("navigator", check_navigator(32, real_cores, real_cores), "native-looking values"),
        ("navigator", check_navigator(4, 11, real_cores), "a plausible farbled pair"),
        ("bot1", check_bot1({"webdriver": False, "hasChrome": True,
                             "getImageDataNative": True, "readPixelsNative": True}),
         "a clean browser"),
    ]
    for kind, bad, desc in pos:
        status = "accepted OK" if not bad else "*** REJECTED: %s" % "; ".join(bad)
        print("  %-9s %-38s %s" % (kind, desc, status))
        if bad:
            ok = False

    print("\nSELF-TEST %s" % ("PASSED" if ok else "FAILED"))
    return 0 if ok else 1


# --------------------------------------------------------------------------------------

def url_for(host):
    return "https://%s/" % host


def boot(args, bypass_hosts, global_enabled):
    """Kill, write the two settings files, relaunch, return (port, excluded)."""
    pdir = profile_dir(args.data_root, args.profile)
    port = cdp_port_for(args.profile, args.dev)

    kill_browser_by_path(args.exe)

    doc = read_settings(pdir)
    sites = doc.get("siteSettings")
    if not isinstance(sites, dict):
        sites = {}
    for h in (HOST_A, HOST_B):
        sites.pop(h, None)
    for h in bypass_hosts:
        sites[h] = {"enabled": False}      # OBJECT, not a bare boolean
    doc["siteSettings"] = sites
    write_settings(pdir, doc)

    spath = os.path.join(args.data_root, "settings.json")
    with open(spath, "r", encoding="utf-8") as fh:
        sdoc = json.load(fh)
    sdoc.setdefault("privacy", {})["fingerprintProtection"] = global_enabled
    with open(spath, "w", encoding="utf-8") as fh:
        json.dump(sdoc, fh, indent=2)

    for attempt in range(1, 4):
        if attempt > 1:
            kill_browser_by_path(args.exe)
        launch_browser(args.exe, args.dev, args.profile)
        if wait_for_cdp(port):
            break
        print("    launch attempt %d did not bring up CDP %d; retrying" % (attempt, port))
    else:
        raise SystemExit("CDP %d never came up after 3 launch attempts." % port)

    return port, snapshot_targets(port, settle=args.settle)


def read_global_toggle(data_root):
    with open(os.path.join(data_root, "settings.json"), "r", encoding="utf-8") as fh:
        return json.load(fh).get("privacy", {}).get("fingerprintProtection")


def same(a, b):
    return all(a.get(f) == b.get(f) for f in FARBLED_FIELDS)


def diff_fields(a, b):
    return [f for f in FARBLED_FIELDS if a.get(f) != b.get(f)]


def main():
    for stream in (sys.stdout, sys.stderr):
        try:
            stream.reconfigure(encoding="utf-8", errors="replace")
        except (AttributeError, ValueError):
            pass

    ap = argparse.ArgumentParser()
    ap.add_argument("--exe")
    ap.add_argument("--data-root")
    ap.add_argument("--profile", default="Default")
    ap.add_argument("--dev", action="store_true")
    ap.add_argument("--settle", type=float, default=8.0)
    ap.add_argument("--timeout", type=float, default=90.0)
    ap.add_argument("--log", default=None)
    ap.add_argument("--real-cores", type=int, default=None,
                    help="override the detected logical core count")
    ap.add_argument("--self-test", action="store_true",
                    help="exercise the validators against values that must be rejected")
    args = ap.parse_args()

    real_cores = args.real_cores or os.cpu_count() or 0
    if args.self_test:
        return self_test(real_cores)

    if not args.exe or not args.data_root:
        return fail("--exe and --data-root are required unless --self-test is given")
    print("machine logical cores (os.cpu_count): %d" % real_cores)

    spath = os.path.join(args.data_root, "settings.json")
    fpath = settings_path(profile_dir(args.data_root, args.profile))
    backups = {}
    for p in (spath, fpath):
        if os.path.exists(p):
            backups[p] = p + ".battery-backup"
            shutil.copy2(p, backups[p])
    print("backed up %d settings file(s)" % len(backups))

    results = {}
    try:
        # ---- phase 1: global ON, no bypass -------------------------------------------
        print("\n=== phase 1 — global toggle ON, no per-site bypass ==================")
        port, excluded = boot(args, bypass_hosts=[], global_enabled=True)
        eng = engine_version(port)

        a1 = measure(port, excluded, url_for(HOST_A), HOST_A, timeout=args.timeout)
        a2 = measure(port, excluded, url_for(HOST_A), HOST_A, timeout=args.timeout)
        b1 = measure(port, excluded, url_for(HOST_B), HOST_B, timeout=args.timeout)
        bot = measure(port, excluded, url_for(HOST_A), HOST_A, timeout=args.timeout, js=BOT_JS)

        for label, v in (("A read 1", a1), ("A read 2", a2), ("B read 1", b1)):
            print("    %-9s canvas=%s webgl=%s audio=%s mem=%s cores=%s"
                  % (label, v["small"], v["glSmall"], v["audio"],
                     v["deviceMemory"], v["cores"]))
        print("    BOT-1     webdriver=%r (%s)  window.chrome=%s keys=%s"
              % (bot["webdriver"], bot["webdriverType"], bot["hasChrome"],
                 ",".join(bot["chromeKeys"][:6]) or "-"))
        print("    toString  getImageData native=%s  readPixels native=%s"
              % (bot["getImageDataNative"], bot["readPixelsNative"]))

        role = check_role_in_log(args.log, HOST_A) if args.log else None
        if role is not None:
            ok_role = role.startswith("tab_")
            print("    SUBJECT: shell served %s to role=%s %s"
                  % (HOST_A, role, "OK (a tab)" if ok_role else "NOT A TAB -- meaningless"))
            if not ok_role:
                raise SystemExit("measured the wrong browser")

        results["consistency"] = (same(a1, a2), diff_fields(a1, a2))
        results["sensitivity"] = (not same(a1, b1), diff_fields(a1, b1))
        results["navigator"] = check_navigator(a1.get("deviceMemory"), a1.get("cores"),
                                               real_cores)
        results["bot1"] = check_bot1(bot)

        # ---- phase 2: native by per-site hard bypass ---------------------------------
        print("\n=== phase 2 — native via per-site hard bypass (global still ON) =====")
        port, excluded = boot(args, bypass_hosts=[HOST_A], global_enabled=True)
        nat_site = measure(port, excluded, url_for(HOST_A), HOST_A, timeout=args.timeout)
        print("    A native   canvas=%s webgl=%s audio=%s mem=%s cores=%s"
              % (nat_site["small"], nat_site["glSmall"], nat_site["audio"],
                 nat_site["deviceMemory"], nat_site["cores"]))

        # ---- phase 3: native by the GLOBAL toggle (T8) -------------------------------
        print("\n=== phase 3 — global toggle OFF (T8) ===============================")
        port, excluded = boot(args, bypass_hosts=[], global_enabled=False)
        persisted = read_global_toggle(args.data_root)
        nat_glob = measure(port, excluded, url_for(HOST_A), HOST_A, timeout=args.timeout)
        print("    A native   canvas=%s webgl=%s audio=%s mem=%s cores=%s"
              % (nat_glob["small"], nat_glob["glSmall"], nat_glob["audio"],
                 nat_glob["deviceMemory"], nat_glob["cores"]))
        print("    toggle value on disk after restart: %r" % (persisted,))

        results["t8_effective"] = (not same(a1, nat_glob), diff_fields(a1, nat_glob))
        results["t8_is_native"] = (same(nat_glob, nat_site), diff_fields(nat_glob, nat_site))
        results["t8_persists"] = (persisted is False, persisted)
    finally:
        kill_browser_by_path(args.exe)
        for p, b in backups.items():
            shutil.copy2(b, p)
            os.remove(b)
        print("\nrestored %d settings file(s)" % len(backups))

    # ---- verdict ---------------------------------------------------------------------
    print("\n================ RESULTS (engine %s) ================" % eng)
    ok = True

    def row(name, passed, detail=""):
        nonlocal ok
        print("  %-34s %s%s" % (name, "PASS" if passed else "FAIL",
                                ("   " + detail) if detail else ""))
        if not passed:
            ok = False

    sens_ok, sens_moved = results["sensitivity"]
    row("sensitivity control (A vs B differ)", sens_ok,
        "moved: %s" % ",".join(sens_moved) if sens_ok else
        "*** two different origins measured IDENTICALLY — every equality result below is void")

    cons_ok, cons_moved = results["consistency"]
    row("intra-session consistency", cons_ok,
        "" if cons_ok else "*** moved between two reads: %s" % ",".join(cons_moved))

    row("navigator values in valid set", not results["navigator"],
        "; ".join(results["navigator"]))
    row("BOT-1 (webdriver / window.chrome)", not results["bot1"],
        "; ".join(results["bot1"]))

    t8e_ok, t8e_moved = results["t8_effective"]
    row("T8 global toggle takes effect", t8e_ok,
        "moved: %s" % ",".join(t8e_moved) if t8e_ok else "*** toggling it changed nothing")
    t8n_ok, t8n_moved = results["t8_is_native"]
    row("T8 lands on TRUE native (2 routes)", t8n_ok,
        "" if t8n_ok else "*** global-off disagrees with per-site bypass on: %s"
        % ",".join(t8n_moved))
    row("T8 setting persists across restart", results["t8_persists"][0],
        "" if results["t8_persists"][0] else "on-disk value was %r" % (results["t8_persists"][1],))

    if ok:
        print("\nVERDICT: PASS")
        print("BATTERY-v1 engine=%s consistency=ok navigator=(%s,%s) bot1=ok t8=ok"
              % (eng, a1.get("deviceMemory"), a1.get("cores")))
        return 0
    print("\nVERDICT: FAIL")
    return 1


def fail(msg):
    print("ERROR: %s" % msg, file=sys.stderr)
    return 2


if __name__ == "__main__":
    sys.exit(main())
