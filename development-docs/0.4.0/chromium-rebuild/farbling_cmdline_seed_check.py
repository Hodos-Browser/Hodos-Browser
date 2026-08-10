#!/usr/bin/env python3
r"""farbling_cmdline_seed_check.py — the §7 "no persistent seed on any renderer command
line" acceptance row (C2 threat model).

The persistent per-profile seed is the one long-lived secret in the farbling design: it is
what makes a fingerprint stable across restarts, so anything that can read it can link a
user across every profile-lifetime. C2's design says the master seed **never leaves the
browser process** — the renderer is handed only a per-origin `domain_key`, over mojo, at
navigation commit. A command line is world-readable to any process on the box, so a seed
(or even a domain key) appearing there would defeat the whole delivery design.

This checks the live process table, not the source. Reading the code tells you what the
current code intends; it does not tell you what this binary, with these switches, on this
machine, actually launched. A switch appended by CEF, by `OnBeforeChildProcessLaunch`, or
by a future field-trial config would not show up in any grep of our own source.

## ⛔ The positive control is the whole point

An absence is only evidence if the instrument can detect a presence. `Win32_Process.
CommandLine` returns **empty** for processes the caller cannot open — so a script run with
the wrong privileges sees empty strings everywhere and reports a triumphant "no seed found
on any renderer", having read nothing at all. That is the same false-negative shape as
`strings(1)` on a >4 GB symbol file (relay 2026-08-09c §2c).

So before believing any absence this script asserts, on the same data:

  * at least one child process command line contains `--type=renderer`
  * at least one command line contains `--profile=`   (a switch we know we pass)
  * the scan actually saw N > 1 processes under the build directory

If any of those fail the run exits non-zero with "BLIND", and no absence claim is made.

## What is searched for

  * the profile seed hex, both cases, and its base64 form
  * the derived `domain_key = HMAC-SHA256(profile_seed, registrable_domain)` for each
    origin visited, hex — a leak of the per-origin key is less bad than the master seed
    but still not supposed to be there
  * a catch-all regex for any 32- or 64-char hex run, so an unexpected encoding of some
    other long-lived secret is still caught

## Usage — Windows and macOS (ported 2026-08-10)

    # Windows (Win32_Process)
    python farbling_cmdline_seed_check.py \
        --exe "C:\...\cef-native\build\bin\Release\HodosBrowser.exe" \
        --data-root "%APPDATA%\HodosBrowserDev" --profile Default --dev

    # macOS (ps -ww -o pid=,args=)
    python3 farbling_cmdline_seed_check.py \
        --exe "$HOME/Hodos-Browser/cef-native/build/bin/HodosBrowser.app/Contents/MacOS/HodosBrowser" \
        --data-root "$HOME/Library/Application Support/HodosBrowserDev" \
        --profile Default --dev

Add `--attach` to inspect a browser that is already running (it must have visited a
farbled page, or no key has been derived yet).

⛔ On macOS export `HODOS_MAC_DEV_FLAGS=1` yourself; `--dev` sets only `HODOS_DEV`.

`--self-test` runs the planted-leak proof on either platform and touches no browser.
"""

import argparse
import base64
import hashlib
import hmac
import json
import os
import re
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from farbling_seed_rotation_check import (  # noqa: E402
    EXEMPT_HOST,
    EXEMPT_URL,
    FARBLED_HOST,
    FARBLED_URL,
    _posix_procs_under,
    _posix_scope_dir,
    kill_browser_by_path,
    launch_browser,
    measure,
    read_settings,
    snapshot_targets,
    wait_for_cdp,
)
from farbling_cross_profile_check import cdp_port_for  # noqa: E402

# The catch-all for an UNKNOWN long-lived secret: a switch whose entire value is a long
# hex string. It is deliberately anchored to the whole value rather than run over the raw
# command line, because a free-floating "32+ hex characters" regex matches inside ordinary
# Chromium base64 blobs -- `--gpu-preferences=UAAAAAAAAADgAAAEAAAA...` contains a 32-char
# run of A and B, which are hex digits. That false positive is not a harmless annoyance:
# a catch-all that cries wolf on every GPU process is a catch-all that gets deleted.
#
# Known limit, stated rather than papered over: this cannot see a secret *embedded* in a
# larger encoded blob. Our own known secrets are searched for as substrings below, which
# covers the realistic leak; the whole-value rule only backstops an unrecognised one.
WHOLE_HEX_VALUE = re.compile(r"^[0-9a-fA-F]{32,}$")


def process_table(exe_path):
    """Every process running out of the build directory, with its full command line.

    ## macOS arm (`ps -ww -o pid=,args=`), and the three ways it could lie

    Ported 2026-08-10. `ps -ww` is the documented equivalent of `Win32_Process.CommandLine`,
    but "equivalent" had to be *measured*, because every failure mode here is a silent
    false PASS -- this script's output is an ABSENCE claim, and an instrument that reads
    nothing reports the same clean absence as a browser that leaks nothing.

    1. **Truncation.** If `ps` capped argv, a seed appended near the end would be invisible
       and the run would go green. Measured on this box: `ps -ww` returned the complete
       argv, tail sentinel intact, at 1 KB / 5 KB / 20 KB / 60 KB / 120 KB / **250 KB**.
       The longest real Hodos command line here is a renderer at **1420 chars**, i.e. ~178x
       margin. `assert_not_truncated()` below re-checks that margin at runtime rather than
       trusting this paragraph.
    2. **argv hidden for other users' processes.** macOS returns only the executable path,
       with no arguments, for processes the caller does not own -- the exact analogue of
       `Win32_Process.CommandLine` returning empty without the right privileges. Caught by
       the existing `--type=renderer` / `--profile=` positive controls, which measurement
       confirms are both present on a real macOS renderer line.
    3. **Self-match.** `_posix_procs_under` matches argv[0]'s PREFIX, not a substring of the
       whole line, so this scanner (and the shell that launched it) cannot match itself even
       though its own command line contains the build path. That trap has already fired once
       in this project -- `pgrep -f "automate-git.py"` matched the watcher and reported a
       finished build as still running.

    ⛔ The one thing NOT ported is the Windows `ExecutablePath` semantics: on macOS the scope
    is the `.app` bundle root, not `dirname(exe)`. See `_posix_scope_dir` -- `dirname(exe)`
    is `Contents/MacOS`, which contains the browser process and NONE of the five helpers,
    so it would have scanned 1 process instead of 8 and still called it a pass.
    """
    if sys.platform != "win32":
        return [{"pid": pid, "cmd": cmd}
                for pid, cmd in _posix_procs_under(_posix_scope_dir(exe_path))]
    target_dir = os.path.dirname(os.path.abspath(exe_path))
    ps = (
        "$d = '%s'; "
        "Get-CimInstance Win32_Process | Where-Object { $_.ExecutablePath -and "
        "$_.ExecutablePath.StartsWith($d, [System.StringComparison]::OrdinalIgnoreCase) "
        "} | Select-Object ProcessId, CommandLine | ConvertTo-Json -Compress -Depth 3"
        % target_dir.replace("'", "''")
    )
    r = subprocess.run(["powershell", "-NoProfile", "-Command", ps],
                       capture_output=True, text=True, check=False)
    raw = (r.stdout or "").strip()
    if not raw:
        return []
    doc = json.loads(raw)
    if isinstance(doc, dict):
        doc = [doc]
    return [{"pid": d.get("ProcessId"), "cmd": d.get("CommandLine") or ""} for d in doc]


def scan(procs, needles):
    """Return [(pid, what)] for every secret visible on a command line."""
    hits = []
    for p in procs:
        for label, needle in needles.items():
            if needle and needle in p["cmd"]:
                hits.append((p["pid"], label))
        for token in p["cmd"].split():
            value = token.split("=", 1)[1] if token.startswith("--") and "=" in token \
                else token
            if WHOLE_HEX_VALUE.match(value):
                hits.append((p["pid"], "switch value is a %d-char hex string: %s"
                             % (len(value), token[:60])))
    return hits


def self_test(seed_hex):
    """Prove `scan` still detects a planted leak after the false-positive fix.

    The whole-value hex rule above was tightened in response to a false positive on
    `--gpu-preferences`. Tightening a detector without re-proving it can still detect is
    how a check quietly becomes decorative -- so this plants the real secrets into a
    synthetic process table and asserts all three are caught, while the genuine
    `--gpu-preferences` blob that caused the false positive is NOT.
    """
    key = domain_key_hex(seed_hex, FARBLED_HOST)
    gpu_pref = ("--gpu-preferences=UAAAAAAAAADgAAAEAAAAAAAAAAAAAGAAOAAAAAAAAAABAAAAAAA"
                "AAAAAAAAAAAAAAgAAAAAAAAAAAAAAAAAAABgAAAAAAAAAGAAAAAAAAAAIAAAAAAAAAAg"
                "AAAAAAAAACAAAAAAAAAA=")
    fake = [
        {"pid": 1, "cmd": "HodosBrowser.exe --type=renderer --hodos-seed=" + seed_hex},
        {"pid": 2, "cmd": "HodosBrowser.exe --type=renderer --blob=xx" + key + "yy"},
        {"pid": 3, "cmd": "HodosBrowser.exe --type=gpu-process " + gpu_pref},
    ]
    needles = {
        "profile seed (lower hex)": seed_hex.lower(),
        "domain_key(%s)" % FARBLED_HOST: key,
    }
    hits = scan(fake, needles)
    by_pid = {}
    for pid, what in hits:
        by_pid.setdefault(pid, []).append(what)

    ok = True
    for pid, why in ((1, "planted profile seed as a switch value"),
                     (2, "planted domain key embedded in a blob")):
        got = bool(by_pid.get(pid))
        print("  %s detector catches %-42s %s"
              % ("PASS" if got else "FAIL", why, by_pid.get(pid, "MISSED")))
        ok = ok and got
    clean = not by_pid.get(3)
    print("  %s detector ignores %-43s %s"
          % ("PASS" if clean else "FAIL", "the real --gpu-preferences blob",
             "no hit" if clean else by_pid[3]))
    return 0 if (ok and clean) else 1


def assert_not_truncated():
    """Prove, at runtime, that this platform's process table returns a COMPLETE argv.

    A truncating `ps` is the one macOS failure mode the `--type=renderer` / `--profile=`
    positive controls cannot catch: both of those switches sit near the FRONT of the line,
    so they would still be visible on an argv that had been cut off well before its end --
    and a seed appended after the cut would be silently unfindable. The controls would say
    "instrument working", the scan would say "no seed", and both would be wrong.

    So: launch a throwaway process whose argv is far longer than any real browser command
    line and which ends in a known sentinel, then require the sentinel back. This measures
    the actual limit on the actual box instead of trusting a number in a comment.

    Returns (ok, detail). Never raises -- a control that cannot run must degrade to BLIND,
    not to an exception that a caller might catch and ignore.
    """
    if sys.platform == "win32":
        return True, "not applicable (Win32_Process returns the full CommandLine)"
    pad = 64000                       # ~45x the longest real Hodos command line measured
    sentinel = "ZcmdlineProbeZ"
    arg = "--pad=" + ("A" * pad) + sentinel
    proc = None
    try:
        proc = subprocess.Popen([sys.executable, "-c", "import time; time.sleep(20)", arg],
                                stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        deadline = time.time() + 10
        seen = ""
        while time.time() < deadline:
            time.sleep(0.4)
            r = subprocess.run(["ps", "-ww", "-o", "args=", "-p", str(proc.pid)],
                               capture_output=True, text=True, check=False)
            seen = r.stdout.strip()
            if seen:
                break
        if not seen:
            return False, "the probe process never appeared in ps"
        if not seen.endswith(sentinel):
            return False, ("ps TRUNCATED a %d-char argv to %d chars -- a secret past the "
                           "cut would be invisible and this scan would report a false "
                           "'no seed found'" % (len(arg), len(seen)))
        return True, ("ps returned a %d-char argv intact (sentinel recovered)" % len(seen))
    except OSError as e:
        return False, "could not run the truncation probe: %s" % e
    finally:
        if proc is not None:
            try:
                proc.kill()
                proc.wait(timeout=5)
            except Exception:
                pass


def domain_key_hex(seed_hex, registrable_domain):
    """Mirrors FarblingPolicy.cpp :: ComputeDomainKey --
    HMAC-SHA256(key = profile seed BYTES, msg = registrable domain)."""
    return hmac.new(bytes.fromhex(seed_hex),
                    registrable_domain.encode("utf-8"),
                    hashlib.sha256).hexdigest()


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
    ap.add_argument("--attach", action="store_true")
    ap.add_argument("--self-test", action="store_true",
                    help="prove the detector still catches a planted leak, then exit")
    args = ap.parse_args()

    pdir = os.path.join(args.data_root, args.profile)

    if args.self_test:
        seed = read_settings(pdir).get("profileSeed")
        if not seed:
            return fail("no profileSeed in %s to plant" % pdir)
        print("== detector self-test (planted leaks, synthetic process table) ==")
        return self_test(seed)

    port = cdp_port_for(args.profile, args.dev)

    if not args.attach:
        for attempt in range(1, 4):
            kill_browser_by_path(args.exe)
            launch_browser(args.exe, args.dev, args.profile)
            if wait_for_cdp(port):
                break
            print("    launch attempt %d did not bring up CDP %d; retrying" % (attempt, port))
        else:
            return fail("CDP %d never came up" % port)
        # Visit both a farbled and an exempt origin, so the browser has actually derived
        # and delivered a domain key. Checking a browser that has only ever shown the NTP
        # would prove nothing: there would be no key in existence to leak.
        excluded = snapshot_targets(port, settle=args.settle)
        measure(port, excluded, EXEMPT_URL, EXEMPT_HOST)
        measure(port, excluded, FARBLED_URL, FARBLED_HOST)
        print("    visited %s and %s -- a domain key now exists to look for"
              % (EXEMPT_HOST, FARBLED_HOST))

    seed = read_settings(pdir).get("profileSeed")
    if not seed:
        return fail("no profileSeed in %s -- nothing to look for, so this proves nothing"
                    % pdir)

    needles = {
        "profile seed (lower hex)": seed.lower(),
        "profile seed (upper hex)": seed.upper(),
        "profile seed (base64)": base64.b64encode(bytes.fromhex(seed)).decode(),
    }
    for host in (FARBLED_HOST, EXEMPT_HOST):
        needles["domain_key(%s)" % host] = domain_key_hex(seed, host)

    procs = process_table(args.exe)
    print("\n== scanned %d process(es) under the build directory ==" % len(procs))

    # ---- positive control: prove the instrument can see anything at all ---------------
    with_cmd = [p for p in procs if p["cmd"].strip()]
    saw_renderer = any("--type=renderer" in p["cmd"] for p in procs)
    saw_profile = any("--profile=" in p["cmd"] for p in procs)
    longest = max((len(p["cmd"]) for p in procs), default=0)
    intact, trunc_detail = assert_not_truncated()
    print("   command lines readable : %d/%d" % (len(with_cmd), len(procs)))
    print("   saw --type=renderer    : %s" % saw_renderer)
    print("   saw --profile=         : %s" % saw_profile)
    print("   longest command line   : %d chars" % longest)
    print("   argv not truncated     : %s (%s)" % (intact, trunc_detail))
    if len(procs) < 2 or not with_cmd or not saw_renderer or not saw_profile:
        return fail("BLIND -- the scan could not read real child command lines, so its "
                    "'no seed found' would be an artefact of reading nothing. Re-run "
                    "with the browser up, from an account that can open the child "
                    "processes.")
    if not intact:
        return fail("BLIND -- %s. A seed past the truncation point would be unfindable, "
                    "so no absence claim is made." % trunc_detail)
    print("   positive control OK: this scan can read COMPLETE renderer command lines.")

    # ---- the actual search -----------------------------------------------------------
    hits = scan(procs, needles)

    print()
    if hits:
        print("RESULT: FAIL -- a persistent secret is visible on a command line:")
        for pid, what in hits:
            print("   pid %s: %s" % (pid, what))
        return 1

    print("RESULT: PASS -- profile seed (hex/base64), both derived domain keys, and any")
    print("32+ char hex run are absent from all %d command lines, and the positive"
          % len(procs))
    print("control confirms this scan does read renderer command lines.")
    print("seed searched for: %s...  domain_key(%s): %s..."
          % (seed[:16], FARBLED_HOST, needles["domain_key(%s)" % FARBLED_HOST][:16]))
    return 0


def fail(msg):
    print("RESULT: FAIL -- %s" % msg)
    return 1


if __name__ == "__main__":
    sys.exit(main())
