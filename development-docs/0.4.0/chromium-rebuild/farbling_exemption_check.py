#!/usr/bin/env python3
r"""farbling_exemption_check.py — Q3 **T2**: native-value equality, the SOLE proof that a
listed auth exemption is actually LIVE.

## Why this exists when `farbling_seed_rotation_check.py` already touches the exempt origin

The rotation harness asserts the exempt origin is **constant across seed rotations**
(`exempt=53225ec8/53225ec8/53225ec8`). That is necessary but **not sufficient**, and the
gap is not hypothetical: an implementation that farbled the exempt origin with a *fixed*
key — a hard-coded key, a zeroed key, a key that failed to install and defaulted — would
be perfectly constant across rotations and would sail through that assertion while the
exemption was, in fact, dead. Constancy proves the value does not follow the seed. It does
not prove the value is **native**.

T2 closes that by comparing against a true-native baseline of the *same origin on the same
machine*, which is the only baseline that cannot be argued with.

## Method — two arms, one launch each

    arm ON      normal settings                      -> M_on
    arm NATIVE  siteSettings[host] = {"enabled":0}   -> M_native

The per-site Privacy Shield override is a **hard bypass**: `OnBeforeBrowse` ORs it into the
same single `enabled` bit that `IsAuthDomain` feeds
(`simple_handler.cpp`, `!IsSiteEnabled(domain)`), so arm NATIVE is native regardless of
what the allowlist says. Then:

    exempt host      M_on == M_native  on every farbled field  => exemption LIVE
    non-exempt host  M_on != M_native  on at least one field   => the measurement is SENSITIVE

⚠️ **The non-exempt row is load-bearing and is not optional.** Without it, "everything is
equal" is exactly what you would observe if farbling were broken, disabled, or if the
harness were measuring the wrong browser — and every exempt host would be reported LIVE.
A run in which the non-exempt control fails to differ is reported RED and no equality
verdict from it may be used.

The two size-gate controls (>=65536px canvas, >=262144B readPixels) sit outside the
farbling gates and must be identical in BOTH arms on EVERY host. If one moves, the arms are
not comparable and the run is void — reported as a failure rather than as noise.

## ⛔ NEGATIVE CONTROL (mandatory — CLAUDE.md Testing Standards)

`--negative-control` asserts the expected-exempt verdict against a host that is **not** on
the allowlist, and requires this harness to go RED for it. That is a faithful simulation of
the feature being off — the identical code path with the exemption absent — and it needs no
rebuild. The exit code is inverted: a correct RED exits 0, and reporting the non-exempt
host as LIVE exits non-zero.

(The stronger control — actually emptying `IsAuthDomain` and rebuilding `cef-native` — is
the exception-list Phase 1 measurement. It is a shell-only rebuild and this same harness
runs against it unchanged: with the list emptied, every host must report NOT LIVE.)

## What this harness does NOT tell you

It proves an exemption is **live**. It says nothing about whether that exemption is
**needed** — that is a breakage question, answerable only by driving the real sign-in flow
(`EXCEPTIONS_DESIGN_REVIEW.md` §5c step 1: fresh profile, no cookies, sign-in and sign-up,
N >= 3 trials on different days, and a PASS is never a licence to delete an entry).

## Usage

    pip install websocket-client

    python farbling_exemption_check.py \
        --exe "C:\...\cef-native\build\bin\Release\HodosBrowser.exe" \
        --data-root "%APPDATA%\HodosBrowserDev" \
        --dev --log "%APPDATA%\HodosBrowserDev\logs\debug_output.log"

    # the other half:
    python farbling_exemption_check.py ... --negative-control

⚠️ The browser is killed by executable PATH, never image name — the installed production
browser shares the image name and holds CDP 9222.
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
from farbling_cross_session_login_check import exempt_hosts, is_exempt  # noqa: E402


# Exempt hosts that are actually navigable as a TOP-LEVEL document. The allowlist also
# carries asset origins (js.hcaptcha.com, www.gstatic.com, newassets.hcaptcha.com,
# cf-turnstile.com, ...) which serve scripts rather than pages; navigating a tab straight at
# a script URL hands it to the download handler instead of rendering a document, so they
# cannot be measured this way and are reported as UNCOVERED rather than silently dropped.
DEFAULT_HOSTS = [
    "github.com",
    "x.com",
    "whatsonchain.com",
    "www.google.com",
    "paypal.com",
    "accounts.google.com",
]

# Not on the allowlist. This is the sensitivity control, and in --negative-control mode it
# is the subject.
CONTROL_HOST = "example.com"

# Fields C3/C4/C5/C6 farble. Equality across the two arms on ALL of these is what "the
# exemption is live" means.
FARBLED_FIELDS = ["small", "glSmall", "audio", "deviceMemory", "cores"]

# Outside the farbling size gates -> must be identical in both arms on every host, or the
# arms are not comparable.
GATE_CONTROLS = ["large", "glLarge"]


def url_for(host):
    return "https://%s/" % host


def run_arm(label, hosts, args, bypass_hosts):
    """One launch; measure every host in turn. `bypass_hosts` get a per-site disable
    written BEFORE the browser starts, which is the hard-bypass (native) arm."""
    pdir = profile_dir(args.data_root, args.profile)
    port = cdp_port_for(args.profile, args.dev)
    print("\n=== arm %s -- CDP %d ==========================================" % (label, port))

    # Kill FIRST: a live browser owns fingerprint_settings.json and rewrites it on
    # shutdown, so writing it while up is a race that silently loses the override.
    kill_browser_by_path(args.exe)

    doc = read_settings(pdir)
    sites = doc.get("siteSettings")
    if not isinstance(sites, dict):
        sites = {}
    for h in hosts:
        sites.pop(h, None)
    for h in bypass_hosts:
        # ⚠️ OBJECT, not a bare boolean — a bare `false` is silently ignored by the loader
        # and farbling stays ON, which would make this "native" arm quietly farbled.
        sites[h] = {"enabled": False}
    doc["siteSettings"] = sites
    write_settings(pdir, doc)
    print("    siteSettings written: %d bypassed, %d normal"
          % (len(bypass_hosts), len(hosts) - len(bypass_hosts)))

    for attempt in range(1, 4):
        if attempt > 1:
            kill_browser_by_path(args.exe)
        launch_browser(args.exe, args.dev, args.profile)
        if wait_for_cdp(port):
            break
        print("    launch attempt %d did not bring up CDP %d; retrying" % (attempt, port))
    else:
        raise SystemExit(
            "CDP %d never came up after 3 launch attempts. Check that the wallet and "
            "frontend dev server are running and that nothing else holds the profile lock."
            % port)

    excluded = snapshot_targets(port, settle=args.settle)

    out = {}
    for h in hosts:
        try:
            # ⚠️ `measure` raises SystemExit on timeout, not Exception — catching only
            # Exception lets one unloadable host abort the whole run after the other
            # measurements have already been paid for.
            v = measure(port, excluded, url_for(h), h, timeout=args.timeout)
        except (Exception, SystemExit) as exc:        # noqa: BLE001
            print("    %-22s UNMEASURED: %s" % (h, exc))
            out[h] = None
            continue
        out[h] = v
        print("    %-22s canvas=%s/%s webgl=%s/%s audio=%s mem=%s cores=%s"
              % (h, v["small"], v["large"], v["glSmall"], v["glLarge"],
                 v["audio"], v["deviceMemory"], v["cores"]))

    # SUBJECT assertion: prove the shell served this to a real tab, not to one of the ~14
    # overlays, which CDP also reports as type:"page".
    role = check_role_in_log(args.log, CONTROL_HOST) if args.log else None
    if role is not None:
        ok = role.startswith("tab_")
        print("    SUBJECT: shell served %s to role=%s %s"
              % (CONTROL_HOST, role, "OK (a tab)" if ok else "NOT A TAB -- meaningless"))
        if not ok:
            raise SystemExit("measured the wrong browser; see the CDP trap in "
                             "farbling_seed_rotation_check.py's docstring")

    return out, engine_version(port)


def compare(host, on, native):
    """Return (verdict, moved_fields, broken_controls)."""
    if on is None or native is None:
        # A host that would not load is NOT evidence either way about its exemption. It is
        # reported as UNMEASURED and listed with the uncovered entries, never counted as a
        # failure (which would be a false red) and never as a pass (a silent cap).
        return "UNMEASURED", [], []
    broken = [f for f in GATE_CONTROLS if on.get(f) != native.get(f)]
    moved = [f for f in FARBLED_FIELDS if on.get(f) != native.get(f)]
    if broken:
        return "VOID", moved, broken
    return ("LIVE" if not moved else "NOT-LIVE"), moved, broken


def main():
    for stream in (sys.stdout, sys.stderr):
        try:
            stream.reconfigure(encoding="utf-8", errors="replace")
        except (AttributeError, ValueError):
            pass

    ap = argparse.ArgumentParser()
    ap.add_argument("--exe", required=True)
    ap.add_argument("--data-root", required=True, help=r"e.g. %%APPDATA%%\HodosBrowserDev")
    ap.add_argument("--profile", default="Default")
    ap.add_argument("--dev", action="store_true")
    ap.add_argument("--settle", type=float, default=8.0)
    ap.add_argument("--timeout", type=float, default=90.0)
    ap.add_argument("--log", default=None)
    ap.add_argument("--hosts", default=None,
                    help="comma-separated exempt hosts to test (default: the navigable subset)")
    ap.add_argument("--negative-control", action="store_true",
                    help="treat the NON-exempt control host as expected-exempt and require "
                         "this harness to go RED for it; exit code inverted")
    args = ap.parse_args()

    allow = exempt_hosts()
    if not allow:
        return fail("could not parse IsAuthDomain out of FingerprintProtection.h — refusing "
                    "to run, because the whole verdict depends on knowing which hosts are "
                    "supposed to be exempt")
    print("IsAuthDomain allowlist parsed: %d entries" % len(allow))

    if args.negative_control:
        subjects = [CONTROL_HOST]
        print("\n*** NEGATIVE CONTROL: %s is NOT on the allowlist and is being tested as if "
              "it were exempt.\n*** A correct run reports it NOT-LIVE and exits 0."
              % CONTROL_HOST)
    else:
        subjects = ([h.strip() for h in args.hosts.split(",") if h.strip()]
                    if args.hosts else list(DEFAULT_HOSTS))
        not_exempt = [h for h in subjects if not is_exempt(h, allow)]
        if not_exempt:
            return fail("these subjects are NOT on the allowlist, so 'exemption live' is not "
                        "a meaningful question for them: %s" % ", ".join(not_exempt))

    # The control always runs alongside the subjects (except in negative-control mode,
    # where it IS the subject).
    hosts = list(dict.fromkeys(subjects + ([] if args.negative_control else [CONTROL_HOST])))

    pdir = profile_dir(args.data_root, args.profile)
    if not os.path.isdir(pdir):
        return fail("profile directory does not exist: %s" % pdir)

    path = settings_path(pdir)
    backup = None
    if os.path.exists(path):
        backup = path + ".exemption-backup"
        shutil.copy2(path, backup)
        print("backed up %s -> %s" % (path, backup))

    try:
        on, eng = run_arm("ON (normal settings)", hosts, args, bypass_hosts=[])
        native, _ = run_arm("NATIVE (per-site hard bypass)", hosts, args, bypass_hosts=hosts)
    finally:
        kill_browser_by_path(args.exe)
        if backup and os.path.exists(backup):
            shutil.copy2(backup, path)
            os.remove(backup)
            print("\nrestored %s" % path)

    print("\n================ RESULTS (engine %s) ================" % eng)
    results = {}
    for h in hosts:
        verdict, moved, broken = compare(h, on.get(h), native.get(h))
        results[h] = (verdict, moved, broken)
        tag = "control" if (h == CONTROL_HOST and not args.negative_control) else "exempt "
        detail = ""
        if broken:
            detail = "  GATE CONTROL MOVED: %s" % ",".join(broken)
        elif moved:
            detail = "  differs on: %s" % ",".join(moved)
        print("  %-8s %-22s %-9s%s" % (tag, h, verdict, detail))

    unmeasured = [h for h in subjects if results[h][0] == "UNMEASURED"]
    covered = {h for h in subjects if results[h][0] != "UNMEASURED"}
    uncovered = sorted((allow - covered) | set(unmeasured))
    print("\nUNCOVERED allowlist entries (%d of %d) — NOT proven by this run:"
          % (len(uncovered), len(allow)))
    print("  " + ", ".join(uncovered))
    if unmeasured:
        print("  (of which %d were ATTEMPTED and would not load: %s — not counted as a "
              "failure, but not proven either)" % (len(unmeasured), ", ".join(unmeasured)))

    ok = True
    if args.negative_control:
        v = results[CONTROL_HOST][0]
        if v == "NOT-LIVE":
            print("\nNEGATIVE CONTROL PASSED: a non-exempt host correctly reports NOT-LIVE, "
                  "so this harness does go red when the exemption is absent.")
            return 0
        print("\n*** NEGATIVE CONTROL FAILED: non-exempt %s reported %s. This harness cannot "
              "tell a live exemption from a dead one; no PASS it has ever produced means "
              "anything." % (CONTROL_HOST, v))
        return 1

    ctrl = results[CONTROL_HOST][0]
    if ctrl != "NOT-LIVE":
        print("\n*** SENSITIVITY CONTROL FAILED: non-exempt %s reported %s — it should have "
              "been farbled and therefore NOT-LIVE. Farbling is off, broken, or this is the "
              "wrong browser. Every equality verdict above is void." % (CONTROL_HOST, ctrl))
        ok = False

    proven = [h for h in subjects if results[h][0] == "LIVE"]
    for h in subjects:
        if results[h][0] not in ("LIVE", "UNMEASURED"):
            print("*** %s: exemption NOT live (%s)" % (h, results[h][0]))
            ok = False
    if not proven:
        print("*** nothing was proven: no subject produced a LIVE verdict.")
        ok = False

    if ok:
        print("\nVERDICT: PASS — %d/%d attempted exemptions proven LIVE by native-value "
              "equality, with a non-exempt control that correctly differs."
              % (len(proven), len(subjects)))
        print("T2-EXEMPTION-v1 engine=%s live=%s control=%s"
              % (eng, "/".join(proven), ctrl))
        return 0
    print("\nVERDICT: FAIL")
    return 1


def fail(msg):
    print("ERROR: %s" % msg, file=sys.stderr)
    return 2


if __name__ == "__main__":
    sys.exit(main())
