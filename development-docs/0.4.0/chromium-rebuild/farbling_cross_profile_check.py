#!/usr/bin/env python3
r"""farbling_cross_profile_check.py — the §7 "cross-profile difference" acceptance row.

Two profiles of the SAME browser, same site, same machine, same session of the same
binary -> different farbled values. That is the per-profile-seed contract: two people
sharing one install must not share one fingerprint.

## What makes this different from the seed-rotation check

`farbling_seed_rotation_check.py` writes a seed and proves the values follow it. This one
writes NOTHING in its normal mode: each profile's seed is whatever `EnsureProfileSeed`
generated for it from the CSPRNG, and the question is whether two independently seeded
profiles actually diverge. A harness that sets both seeds itself would be re-testing seed
rotation with extra steps.

## Controls (in-page, per profile)

Same as the rotation harness, and for the same reason — a bare "A differs from B" cannot
tell farbling apart from render variance between two launches:

  * the auth-exempt origin (github.com) is a native pass-through -> must be IDENTICAL
    across the two profiles
  * the >=65536px canvas and the >=262144B readPixels sit outside the farbling size gates
    -> must be IDENTICAL across the two profiles

If a control moves, the two profiles are not comparable and no difference below means
anything, so a moving control fails the run outright rather than being reported as noise.

## ⛔ NEGATIVE CONTROL (mandatory — CLAUDE.md Testing Standards)

`--negative-control` copies profile A's seed into profile B, removing the one thing that
is supposed to make them differ, and then asserts this harness goes RED. If the profiles
still measure differently with identical seeds, the difference was never coming from the
seed and this test proves nothing. Profile B's original settings file is backed up and
restored afterwards.

Report both halves: "differs across profiles, and stops differing when the seeds match."

## ⛔ There is deliberately no assertion on navigator (deviceMemory / cores)

Their legal ranges are tiny — `deviceMemory` has four values — so "A != B" would flake
roughly one run in four. A flaky release gate is worse than no gate. The values are
printed for the record and excluded from the verdict, exactly as in the rotation harness.

## Usage

    pip install websocket-client

    python farbling_cross_profile_check.py \
        --exe "C:\...\cef-native\build\bin\Release\HodosBrowser.exe" \
        --data-root "%APPDATA%\HodosBrowserDev" \
        --profile-a Default --profile-b Profile_1 \
        --dev --log "%APPDATA%\HodosBrowserDev\logs\debug_output.log"

There is deliberately no `--port`: the shell derives the CDP port from the profile id, so
the two profiles listen on two different ports (see `cdp_port_for`).

    # then, the other half:
    python farbling_cross_profile_check.py ... --negative-control

⚠️ Profile B must already exist in `profiles.json`; this script does not create profiles.
⚠️ The browser is killed by executable PATH, never image name (the installed production
browser shares the image name and holds CDP 9222).
"""

import argparse
import json
import os
import shutil
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from farbling_seed_rotation_check import (  # noqa: E402
    EXEMPT_HOST,
    EXEMPT_URL,
    FARBLED_HOST,
    FARBLED_URL,
    check_role_in_log,
    engine_version,
    kill_browser_by_path,
    launch_browser,
    measure,
    read_settings,
    settings_path,
    snapshot_targets,
    wait_for_cdp,
)


def profile_dir(data_root, profile_id):
    return os.path.join(data_root, profile_id)


def cdp_port_for(profile_id, dev):
    """⚠️ The CDP port is DERIVED FROM THE PROFILE ID -- it is not one fixed port.

    Mirrors `cef_browser_shell.cpp` (the `g_picker_mode` / `profileId == "Default"` block):

        Default      -> 9222
        Profile_<N>  -> 9222 + N
        then, if HODOS_DEV=1, +100

    So profile `Profile_1` in a dev build listens on **9323**, not 9322. Point a harness at
    the wrong one and the second profile looks like "the browser failed to start" three
    times in a row -- which is exactly how this function came to exist. Keep it in step
    with the shell if that block ever changes.
    """
    if profile_id == "Default":
        port = 9222
    else:
        offset = 0
        if "_" in profile_id:
            try:
                offset = int(profile_id.split("_", 1)[1])
            except ValueError:
                offset = 0
        port = 9222 + offset
    return port + (100 if dev else 0)


def run_profile(label, profile_id, args):
    """Launch one profile, measure the exempt and farbled pages, return both."""
    pdir = profile_dir(args.data_root, profile_id)
    port = cdp_port_for(profile_id, args.dev)
    print("\n--- profile %s (%s) -- CDP %d --------------------------------"
          % (label, profile_id, port))

    for attempt in range(1, 4):
        # Kill FIRST. A live browser owns fingerprint_settings.json and rewrites it on
        # shutdown, so reading (or writing) it while up is a race.
        kill_browser_by_path(args.exe)
        launch_browser(args.exe, args.dev, profile_id)
        if wait_for_cdp(port):
            break
        print("    launch attempt %d did not bring up CDP %d; retrying" % (attempt, port))
    else:
        raise SystemExit(
            "CDP %d never came up after 3 launch attempts. Check that the wallet and "
            "frontend dev server are running, that nothing else holds the profile lock, "
            "and that this is the port the shell derives for profile %r."
            % (port, profile_id))

    excluded = snapshot_targets(port, settle=args.settle)
    ex = measure(port, excluded, EXEMPT_URL, EXEMPT_HOST)
    fa = measure(port, excluded, FARBLED_URL, FARBLED_HOST)

    role = check_role_in_log(args.log, FARBLED_HOST)
    if role is not None:
        ok = role.startswith("tab_")
        print("    SUBJECT: shell served %s to role=%s %s"
              % (FARBLED_HOST, role, "OK (a tab)" if ok else "NOT A TAB -- meaningless"))
        if not ok:
            raise SystemExit("measured the wrong browser; see the CDP trap in "
                             "farbling_seed_rotation_check.py's docstring")

    for who, v in (("exempt ", ex), ("farbled", fa)):
        print("    %s canvas=%s/%s  webgl=%s/%s  audio=%s  mem=%s  cores=%s"
              % (who, v["small"], v["large"], v["glSmall"], v["glLarge"],
                 v["audio"], v["deviceMemory"], v["cores"]))

    # Read the seed only after the browser has been up, so a first-run profile has had
    # EnsureProfileSeed generate one.
    seed = read_settings(pdir).get("profileSeed")
    print("    profileSeed on disk: %s" % (seed[:16] + "..." if seed else "MISSING"))
    return {"exempt": ex, "farbled": fa, "seed": seed,
            "engine": engine_version(port)}


def main():
    for stream in (sys.stdout, sys.stderr):
        try:
            stream.reconfigure(encoding="utf-8", errors="replace")
        except (AttributeError, ValueError):
            pass

    ap = argparse.ArgumentParser()
    ap.add_argument("--exe", required=True)
    ap.add_argument("--data-root", required=True,
                    help=r"e.g. %APPDATA%\HodosBrowserDev")
    ap.add_argument("--profile-a", default="Default")
    ap.add_argument("--profile-b", default="Profile_1")
    # No --port: the shell derives it from the profile id (see cdp_port_for).
    ap.add_argument("--dev", action="store_true")
    ap.add_argument("--settle", type=float, default=8.0)
    ap.add_argument("--log", default=None)
    ap.add_argument("--negative-control", action="store_true",
                    help="give B the same seed as A and assert this harness goes RED")
    args = ap.parse_args()

    dir_b = profile_dir(args.data_root, args.profile_b)
    if not os.path.isdir(dir_b):
        return fail("profile B directory does not exist: %s (create the profile in the "
                    "picker first; this script does not create profiles)" % dir_b)

    backup = None
    try:
        a = run_profile("A", args.profile_a, args)

        if args.negative_control:
            # Remove the ONE thing that is supposed to make B differ from A.
            path_b = settings_path(dir_b)
            if os.path.exists(path_b):
                backup = path_b + ".xprofile-backup"
                shutil.copy2(path_b, backup)
            doc = read_settings(dir_b)
            doc["profileSeed"] = a["seed"]
            with open(path_b, "w", encoding="utf-8") as fh:
                json.dump(doc, fh, indent=2)
            print("\n[negative control] profile B seeded with profile A's seed "
                  "%s..." % a["seed"][:16])

        b = run_profile("B", args.profile_b, args)
    finally:
        if backup:
            shutil.move(backup, settings_path(dir_b))
            print("[negative control] restored profile B's original settings file")

    print("\n== verdict ==")
    failures = []
    differs = []

    def check(label, ok, detail):
        print("  %s %-40s %s" % ("PASS" if ok else "FAIL", label, detail))
        if not ok:
            failures.append(label)

    # --- setup sanity -------------------------------------------------------------
    if not a["seed"] or not b["seed"]:
        return fail("a profile has no profileSeed on disk -- EnsureProfileSeed did not run")
    seeds_same = a["seed"] == b["seed"]
    if args.negative_control:
        check("seeds are identical (control setup)", seeds_same,
              "%s... vs %s..." % (a["seed"][:12], b["seed"][:12]))
    else:
        check("profiles have independent seeds", not seeds_same,
              "%s... vs %s..." % (a["seed"][:12], b["seed"][:12]))

    # --- controls: these must NOT move, in either mode ------------------------------
    for label, key, where in (("exempt canvas", "small", "exempt"),
                              ("exempt webgl", "glSmall", "exempt"),
                              ("exempt audio", "audio", "exempt"),
                              ("large canvas (outside gate)", "large", "farbled"),
                              ("large readPixels (outside gate)", "glLarge", "farbled")):
        same = a[where][key] == b[where][key]
        check("CONTROL %s holds still" % label, same,
              "%s vs %s" % (a[where][key], b[where][key]))

    # --- the actual contract --------------------------------------------------------
    for label, key in (("canvas getImageData", "small"),
                       ("webgl readPixels", "glSmall"),
                       ("webaudio getChannelData", "audio")):
        diff = a["farbled"][key] != b["farbled"][key]
        differs.append(diff)
        if args.negative_control:
            check("%s is now IDENTICAL (control)" % label, not diff,
                  "%s vs %s" % (a["farbled"][key], b["farbled"][key]))
        else:
            check("%s differs across profiles" % label, diff,
                  "%s vs %s" % (a["farbled"][key], b["farbled"][key]))

    print("  ---- navigator, recorded but deliberately NOT asserted "
          "(4 legal deviceMemory values would make A!=B flaky) ----")
    print("       A mem=%s cores=%s   B mem=%s cores=%s"
          % (a["farbled"]["deviceMemory"], a["farbled"]["cores"],
             b["farbled"]["deviceMemory"], b["farbled"]["cores"]))

    print()
    if failures:
        if args.negative_control:
            print("RESULT: NEGATIVE CONTROL FAILED -- with both profiles on the SAME seed "
                  "the values still differ. Whatever this harness is measuring, it is not "
                  "the per-profile seed, so the green run proves nothing.")
        else:
            print("RESULT: FAIL -- %d row(s): %s" % (len(failures), ", ".join(failures)))
        return 1

    if args.negative_control:
        print("RESULT: negative control OK -- identical seeds collapse every farbled "
              "value to identical, i.e. this harness does go RED when the per-profile "
              "seed stops being per-profile.")
    else:
        print("RESULT: PASS -- two independently seeded profiles produce different "
              "canvas, WebGL and WebAudio fingerprints on the same site, while both "
              "the auth-exempt origin and the outside-the-gate controls hold still.")
        print("engine=%s  A=%s  B=%s"
              % (a["engine"], a["farbled"]["small"], b["farbled"]["small"]))
    return 0


def fail(msg):
    print("RESULT: FAIL -- %s" % msg)
    return 1


if __name__ == "__main__":
    sys.exit(main())
