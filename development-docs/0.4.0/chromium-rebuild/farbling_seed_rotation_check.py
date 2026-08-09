#!/usr/bin/env python3
r"""farbling_seed_rotation_check.py — the ONLY check that catches the constant-seed class.

## Why this exists, and why nothing cheaper will do

The shipped fingerprint bug (`development-docs/TICKET_farbling_constant_seed_shipped.md`)
survived in **every release since the feature was written**. Not because nobody tested it,
but because every check anyone ran was a **same-session** check, and the bug passed all of
them: the farbled value differed from the exempt value, was stable across reads, and was
stable across navigations. It looked perfect. It was a constant.

Brave shipped this same class themselves (#49346). The class is structurally invisible to
any single-session harness, so the durable artifact is not the fix — it is THIS TEST.

What makes it conclusive is the **A → B → A round trip**:

    profileSeed = A   ->  farbled h1   exempt E   large-canvas L
    profileSeed = B   ->  farbled h2   exempt E   large-canvas L      (h2 != h1)
    profileSeed = A   ->  farbled h1   exempt E   large-canvas L      (exact round trip)

Two controls hold still across all three runs — the auth-exempt page and the >=65536px
canvas that is outside the small-canvas farbling gate — so the farbled delta cannot be
render variance or machine noise. And the exact return to h1 proves determinism across
restarts (the login guarantee) in the SAME experiment that proves per-user unlinkability.
One run, both §11 contracts.

## NEGATIVE CONTROL (mandatory — CLAUDE.md -> Testing Standards)

`--negative-control` turns the feature OFF for the farbled domain (per-site Privacy Shield
opt-out, which gates the native path as well as the JS one) and then asserts that this
harness goes **RED**. If the assertions still pass with farbling disabled, the harness is
measuring nothing and exits non-zero saying so.

Run both halves and report both:  "passes, and fails when farbling is disabled."

## ⛔ Subject assertion — three harnesses died here

Hodos's header and ~14 overlays are separate CEF browsers on 127.0.0.1:5137, and CDP
reports every one as `type: "page"`. Driving one produces a fake "intermittent" failure in
correct code, and `location.href` does NOT catch it (the overlay really is at that URL).
So: browser chrome is identified ONCE at startup by CDP target **id** and excluded forever,
and `--log` additionally cross-checks the `role:` the shell recorded for the measured URL.
Never `PUT /json/new` — those targets bypass OnBeforeBrowse and get no key at all.

## Requirements / usage

    pip install websocket-client

    # dev build (HODOS_DEV=1, CDP 9322), with the wallet + vite already running:
    python farbling_seed_rotation_check.py \
        --exe "C:\Users\<you>\Hodos-Browser\cef-native\build\bin\Release\HodosBrowser.exe" \
        --profile-dir "%APPDATA%\HodosBrowserDev\Default" \
        --port 9322 --dev \
        --log "%APPDATA%\HodosBrowserDev\logs\debug_output.log"

    # the negative control (same command + one flag):
    python farbling_seed_rotation_check.py ... --negative-control

⚠️ The browser is killed by **executable path**, never by image name — the owner's INSTALLED
production browser shares the image name `HodosBrowser.exe` and holds CDP 9222.
This script refuses to touch any process outside the directory of `--exe`.

⚠️ This measures the NATIVE canvas path (C3), which is what `profileSeed` feeds. It is
therefore only meaningful against a build whose CEF carries the C2 registry + C3 patches
(fork `dfe5a2343` or later). Against older binaries farbling is absent and this correctly
reports RED — check `CEF_VERSION` before believing a failure.
"""

import argparse
import json
import os
import secrets
import subprocess
import sys
import time
import urllib.error
import urllib.request

try:
    import websocket  # websocket-client
except ImportError:
    sys.exit("need websocket-client:  pip install websocket-client")


# The auth-exempt control page (FingerprintProtection::IsAuthDomain) is a true native
# pass-through, and the farbled page is an ordinary site. Both are real remote origins on
# purpose: 127.0.0.1/localhost is skipped by the farbling registry by design, so a local
# fixture would measure "no farbling" and pass for the wrong reason.
EXEMPT_URL = "https://github.com/"
EXEMPT_HOST = "github.com"
FARBLED_URL = "https://example.com/"
FARBLED_HOST = "example.com"


# Two canvases. The large one is the CONTROL: at 400x200 = 80,000 px it is outside the
# <65536px small-canvas gate, so it is never farbled on any page or any seed. If it ever
# moves, the two measurements are not comparable and no verdict is trustworthy.
MEASURE_JS = r"""
(function () {
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
  function pixHash(c) {
    var d = c.getContext('2d').getImageData(0, 0, c.width, c.height).data;
    var h = 2166136261 >>> 0;
    for (var i = 0; i < d.length; i++) { h ^= d[i]; h = Math.imul(h, 16777619) >>> 0; }
    return ('0000000' + h.toString(16)).slice(-8);
  }
  var small = draw(200, 50);    // 10,000 px  -> inside the gate, farbled
  var large = draw(400, 200);   // 80,000 px  -> outside the gate, NEVER farbled (control)
  return JSON.stringify({
    href: location.href,
    small: pixHash(small),
    large: pixHash(large)
  });
})()
"""


# --------------------------------------------------------------------------------------
# profileSeed handling — read-modify-write, exactly like FarblingPolicy::EnsureProfileSeed
# --------------------------------------------------------------------------------------

def settings_path(profile_dir):
    return os.path.join(profile_dir, "fingerprint_settings.json")


def read_settings(profile_dir):
    path = settings_path(profile_dir)
    try:
        with open(path, "r", encoding="utf-8") as fh:
            doc = json.load(fh)
        return doc if isinstance(doc, dict) else {}
    except (OSError, ValueError):
        return {}


def write_settings(profile_dir, doc):
    """Read-modify-write. This file also holds the user's per-site Privacy Shield toggles;
    FarblingPolicy.cpp goes to some trouble not to clobber them and neither will we."""
    path = settings_path(profile_dir)
    with open(path, "w", encoding="utf-8") as fh:
        json.dump(doc, fh, indent=2)


def set_profile_seed(profile_dir, seed_hex):
    doc = read_settings(profile_dir)
    doc["profileSeed"] = seed_hex
    write_settings(profile_dir, doc)


def set_site_enabled(profile_dir, domain, enabled):
    doc = read_settings(profile_dir)
    sites = doc.get("siteSettings")
    if not isinstance(sites, dict):
        sites = {}
    if enabled:
        sites.pop(domain, None)
    else:
        sites[domain] = {"enabled": False}
    doc["siteSettings"] = sites
    write_settings(profile_dir, doc)


# --------------------------------------------------------------------------------------
# Process control — BY PATH ONLY. Never by image name.
# --------------------------------------------------------------------------------------

def _ps_quote(s):
    """Single-quoted PowerShell literal. Backslashes are NOT escapes in '...'; only the
    quote itself needs doubling. Getting this wrong is not cosmetic -- see below."""
    return "'" + s.replace("'", "''") + "'"


def count_browser_procs(exe_path):
    """How many processes are running out of the directory of exe_path."""
    target_dir = os.path.dirname(os.path.abspath(exe_path))
    if sys.platform != "win32":
        r = subprocess.run(["pgrep", "-fc", os.path.abspath(exe_path)],
                           capture_output=True, text=True, check=False)
        return int(r.stdout.strip() or 0)
    ps = (
        f"$d = {_ps_quote(target_dir)}; "
        "@(Get-CimInstance Win32_Process | Where-Object { $_.ExecutablePath -and "
        "$_.ExecutablePath.StartsWith($d, [System.StringComparison]::OrdinalIgnoreCase) "
        "}).Count"
    )
    r = subprocess.run(["powershell", "-NoProfile", "-Command", ps],
                       capture_output=True, text=True, check=False)
    try:
        return int(r.stdout.strip())
    except ValueError:
        return -1


def kill_browser_by_path(exe_path, verify=True):
    """Kill only processes whose ExecutablePath sits under the directory of exe_path.

    ⛔ The owner's INSTALLED production browser runs ~62 processes from %LOCALAPPDATA%
    under the same image name and holds CDP 9222. A `taskkill /IM HodosBrowser.exe` would
    take it down mid-session, with a real wallet attached. Match on the full path.

    ⛔⛔ AND THE MATCH MUST ACTUALLY MATCH. The first version of this function built the
    pattern with `path.replace("\\", "\\\\")` and interpolated it into a SINGLE-QUOTED
    PowerShell string. PowerShell does not process backslash escapes in '...', so the
    pattern carried literal doubled separators and matched nothing. Nothing was killed;
    the relaunch below was then absorbed by the running instance through the
    SingleInstance named pipe and exited; CDP kept serving the ORIGINAL browser with the
    ORIGINAL seed -- so every phase measured the same process and the harness reported
    identical hashes across "restarts", i.e. it manufactured the exact constant-seed
    signature it exists to detect. `verify` is not optional paranoia; it is the tripwire
    for that whole failure class. Use StartsWith, not -like: `[` and `]` are wildcards.
    """
    target_dir = os.path.dirname(os.path.abspath(exe_path))
    if sys.platform != "win32":
        subprocess.run(["pkill", "-f", os.path.abspath(exe_path)], check=False)
    else:
        ps = (
            f"$d = {_ps_quote(target_dir)}; "
            "Get-CimInstance Win32_Process | Where-Object { $_.ExecutablePath -and "
            "$_.ExecutablePath.StartsWith($d, [System.StringComparison]::OrdinalIgnoreCase) "
            "} | ForEach-Object { Stop-Process -Id $_.ProcessId -Force "
            "-ErrorAction SilentlyContinue }"
        )
        subprocess.run(["powershell", "-NoProfile", "-Command", ps], check=False,
                       stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    time.sleep(6)
    if verify:
        left = count_browser_procs(exe_path)
        if left != 0:
            raise SystemExit(
                f"kill-by-path left {left} process(es) running under {target_dir}. "
                "Every measurement after this would come from the OLD process with the "
                "OLD seed and the harness would fake a constant-seed failure. Aborting.")


def launch_browser(exe_path, dev, profile_id):
    """Launch with an EXPLICIT --profile.

    ⛔ Not cosmetic. With more than one profile and the startup picker enabled, a bare
    launch comes up in picker mode, and picker mode sets remote_debugging_port = 0
    (cef_browser_shell.cpp, g_picker_mode) -- so CDP never binds and every phase looks
    like "the browser failed to start". ProfileManager::ResolveStartup returns
    immediately when argProfile is non-empty, leaving showPicker false, so passing the
    profile both skips the picker and guarantees we measure the profile whose
    fingerprint_settings.json we just edited. Measuring a different profile's seed would
    be a subject error of exactly the kind this harness exists to prevent.
    """
    env = dict(os.environ)
    if dev:
        # The dev safeguard refuses to start a build-directory binary without this, to keep
        # dev off the production database.
        env["HODOS_DEV"] = "1"
    else:
        env.pop("HODOS_DEV", None)
    subprocess.Popen([exe_path, f"--profile={profile_id}"], env=env,
                     cwd=os.path.dirname(os.path.abspath(exe_path)),
                     stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)


def wait_for_cdp(port, timeout=90):
    deadline = time.time() + timeout
    while time.time() < deadline:
        try:
            with urllib.request.urlopen(
                    f"http://127.0.0.1:{port}/json/list", timeout=5) as r:
                json.load(r)
            return True
        except Exception:
            time.sleep(2)
    return False


# --------------------------------------------------------------------------------------
# CDP target selection — id-based, chrome excluded once at startup
# --------------------------------------------------------------------------------------

def engine_version(port):
    """The engine string CDP reports, e.g. 'Chrome/150.0.7871.187'.

    Carried into the attestation token so the release gate can reject a result produced
    against the wrong engine. On M136 the farbling patches do not exist at all, so a
    rotation run there is guaranteed to fail -- but a result pasted from a DIFFERENT
    machine's 150 build would otherwise be indistinguishable from this build's.
    """
    try:
        with urllib.request.urlopen(
                f"http://127.0.0.1:{port}/json/version", timeout=10) as r:
            return json.load(r).get("Browser", "unknown")
    except Exception:
        return "unknown"


def page_targets(port):
    with urllib.request.urlopen(f"http://127.0.0.1:{port}/json/list", timeout=10) as r:
        return [t for t in json.load(r) if t.get("type") == "page"]


def snapshot_targets(port, settle=8.0):
    """Freeze the target landscape BEFORE anything navigates, and return the id set to
    exclude for the rest of the phase.

    Two kinds of thing must be excluded, for the same reason — neither is the tab we
    intend to measure, and driving one silently produces a verdict about the wrong
    document:

    1. Browser chrome. The header ("/") and every overlay ("/tab-list", "/menu",
       "/wallet-panel", ...) are separate CEF browsers on 5137 that CDP reports as
       type:"page". Overlays legitimately receive no farbling key.
    2. Extra restored tabs. Session restore reopens whatever was open at the last
       shutdown, so a phase can start with several real tabs. Leaving them in the
       candidate pool makes selection ambiguous exactly when a cross-site navigation
       swaps the target id, which is when we can least afford to guess.

    We pin ONE tab and exclude everything else, so the pinned tab is the sole candidate
    and an id swap resolves unambiguously to its successor.
    """
    time.sleep(settle)
    chrome, tabs = set(), []
    for t in page_targets(port):
        url = t.get("url", "")
        if "127.0.0.1:5137" in url and "/newtab" not in url:
            chrome.add(t["id"])
        else:
            tabs.append(t)
    if not tabs:
        raise SystemExit("browser came up with no tab target at all")
    # Prefer a fresh NTP; otherwise the first restored tab. Which one does not matter,
    # only that exactly one is in play.
    pinned = next((t for t in tabs if "/newtab" in t.get("url", "")), tabs[0])
    excluded = set(chrome)
    excluded.update(t["id"] for t in tabs if t["id"] != pinned["id"])
    print(f"    excluded {len(chrome)} chrome target(s) + {len(tabs) - 1} extra tab(s); "
          f"driving {pinned.get('url', '?')[:48]}")
    return excluded


def resolve_tab(port, excluded_ids):
    cands = [t for t in page_targets(port) if t["id"] not in excluded_ids]
    if not cands:
        raise SystemExit("no candidate page target -- the pinned tab disappeared")
    if len(cands) == 1:
        return cands[0]
    # Ambiguity is a subject error waiting to happen. Fail loudly rather than guess --
    # guessing here is exactly what produced the fake "intermittent" bug.
    raise SystemExit(
        "ambiguous tab target among %d candidates (%s). A new tab or overlay appeared "
        "mid-phase; do NOT let this script pick one."
        % (len(cands), ", ".join(t.get("url", "?")[:60] for t in cands)))


def measure(port, chrome_ids, url, want_host, timeout=60):
    """Navigate the real tab and read back both canvas hashes."""
    t = resolve_tab(port, chrome_ids)
    ws = websocket.create_connection(t["webSocketDebuggerUrl"], timeout=30)
    try:
        ws.send(json.dumps({"id": 1, "method": "Page.navigate", "params": {"url": url}}))
    finally:
        ws.close()

    deadline = time.time() + timeout
    while time.time() < deadline:
        time.sleep(2.0)
        try:
            t = resolve_tab(port, chrome_ids)
            if want_host not in t.get("url", ""):
                continue
            ws = websocket.create_connection(t["webSocketDebuggerUrl"], timeout=25)
            try:
                ws.send(json.dumps({"id": 2, "method": "Runtime.evaluate",
                                    "params": {"expression": MEASURE_JS,
                                               "returnByValue": True}}))
                got, end = None, time.time() + 25
                while time.time() < end:
                    m = json.loads(ws.recv())
                    if m.get("id") == 2:
                        got = m
                        break
            finally:
                ws.close()
            if not got or "value" not in got.get("result", {}).get("result", {}):
                continue
            val = json.loads(got["result"]["result"]["value"])
            # Value assertion AND subject assertion. href alone is not sufficient (an
            # overlay navigated here would satisfy it) — it is the chrome-id exclusion
            # above that carries the subject guarantee; this is the cheap second gate.
            if want_host not in val["href"]:
                continue
            return val
        except SystemExit:
            raise
        except Exception:
            continue
    raise SystemExit(f"could not measure {url} within {timeout}s")


def check_role_in_log(log_path, host):
    """Cross-check the shell's own record of which browser served the URL.

    `🌐 Resource request: https://example.com/ (role: tablistpanel)` is the tell that a
    harness is driving an overlay. A tab is `role: tab_<n>`.
    """
    if not log_path or not os.path.exists(log_path):
        return None
    try:
        with open(log_path, "r", encoding="utf-8", errors="replace") as fh:
            lines = fh.readlines()[-4000:]
    except OSError:
        return None
    for line in reversed(lines):
        if host in line and "role:" in line:
            role = line.split("role:", 1)[1].split(")", 1)[0].strip()
            return role
    return None


# --------------------------------------------------------------------------------------

def run_phase(label, seed_hex, args):
    print(f"\n--- phase {label}: profileSeed = {seed_hex[:16]}... ----------------------")
    # Kill FIRST, then write. A live browser owns fingerprint_settings.json and can
    # rewrite it on shutdown, so writing into a running profile is a race.
    #
    # The relaunch is retried because a force-killed instance can still be holding the
    # exclusive ProfileLock handle (FILE_FLAG_DELETE_ON_CLOSE) or the single-instance
    # pipe for a moment after its ~19 processes disappear from the process table; a new
    # instance that loses that race exits silently, without ever reaching
    # Logger::Initialize, so there is nothing in debug_output.log to explain it.
    for attempt in range(1, 4):
        kill_browser_by_path(args.exe)
        set_profile_seed(args.profile_dir, seed_hex)
        if args.negative_control:
            # Re-applied every phase, and only while the browser is DOWN. A live
            # browser's SaveSiteSettings() rebuilds "siteSettings" wholesale from its
            # in-memory map, so an entry written behind its back is dropped on the next
            # toggle or shutdown -- which would silently re-enable farbling and turn the
            # negative control green for the wrong reason.
            set_site_enabled(args.profile_dir, FARBLED_HOST, False)
        launch_browser(args.exe, args.dev, args.profile_id)
        if wait_for_cdp(args.port):
            break
        print(f"    launch attempt {attempt} did not bring up CDP {args.port}; retrying")
    else:
        raise SystemExit(
            f"CDP {args.port} never came up after 3 launch attempts. Check that the "
            "wallet and frontend dev server are running, and that nothing else holds "
            "the profile lock.")
    excluded = snapshot_targets(args.port, settle=args.settle)

    ex = measure(args.port, excluded, EXEMPT_URL, EXEMPT_HOST)
    fa = measure(args.port, excluded, FARBLED_URL, FARBLED_HOST)

    role = check_role_in_log(args.log, FARBLED_HOST)
    if role is not None:
        subject_ok = role.startswith("tab_")
        print(f"    SUBJECT: shell served {FARBLED_HOST} to role={role} "
              f"{'OK (a tab)' if subject_ok else 'NOT A TAB -- result is meaningless'}")
        if not subject_ok:
            raise SystemExit("measured the wrong browser; see the CDP trap in this "
                             "file's docstring")

    print(f"    exempt  small={ex['small']}  large={ex['large']}")
    print(f"    farbled small={fa['small']}  large={fa['large']}")
    return {"exempt": ex, "farbled": fa, "engine": engine_version(args.port)}


def main():
    # A Windows console defaults to cp1252 and dies on any non-ASCII we print. The shell
    # log we quote back (role: lines) carries emoji, so this is not hypothetical.
    for stream in (sys.stdout, sys.stderr):
        try:
            stream.reconfigure(encoding="utf-8", errors="replace")
        except (AttributeError, ValueError):
            pass

    ap = argparse.ArgumentParser()
    ap.add_argument("--exe", required=True,
                    help="HodosBrowser.exe to launch; also the kill-by-path root")
    ap.add_argument("--profile-dir", required=True,
                    help="profile dir holding fingerprint_settings.json")
    ap.add_argument("--port", type=int, default=9322,
                    help="CDP port (9322 dev Default, 9222 release Default)")
    ap.add_argument("--dev", action="store_true", help="set HODOS_DEV=1 when launching")
    ap.add_argument("--profile-id", default=None,
                    help="profile to launch (default: basename of --profile-dir). Passed "
                         "as --profile= so the startup picker never appears; picker mode "
                         "disables the CDP port entirely.")
    ap.add_argument("--log", default=None,
                    help="debug_output.log, for the role: subject cross-check")
    ap.add_argument("--settle", type=float, default=10.0,
                    help="seconds to let the browser finish opening its overlays")
    ap.add_argument("--negative-control", action="store_true",
                    help="disable farbling for the farbled domain and assert this "
                         "harness goes RED. Exit 0 only if it does.")
    args = ap.parse_args()

    if not os.path.isfile(args.exe):
        sys.exit(f"--exe not found: {args.exe}")
    if not os.path.isdir(args.profile_dir):
        sys.exit(f"--profile-dir not found: {args.profile_dir}")
    if not args.profile_id:
        args.profile_id = os.path.basename(os.path.normpath(args.profile_dir))
    print(f"launching profile '{args.profile_id}' explicitly (skips the startup picker, "
          f"which would disable the CDP port)")

    original = read_settings(args.profile_dir)
    original_seed = original.get("profileSeed")
    seed_a = original_seed if isinstance(original_seed, str) and len(original_seed) == 64 \
        else secrets.token_hex(32)
    seed_b = secrets.token_hex(32)
    while seed_b == seed_a:
        seed_b = secrets.token_hex(32)

    if args.negative_control:
        print("=" * 78)
        print("NEGATIVE CONTROL: disabling farbling for %s via the per-site Privacy "
              "Shield opt-out." % FARBLED_HOST)
        print("This harness MUST now go red. If it stays green it is testing nothing.")
        print("=" * 78)

    failures = []

    def check(label, ok, detail):
        print("  [%s] %-46s %s" % ("PASS" if ok else "FAIL", label, detail))
        if not ok:
            failures.append(label)

    try:
        a1 = run_phase("A", seed_a, args)
        b = run_phase("B", seed_b, args)
        a2 = run_phase("A'", seed_a, args)

        print("\n" + "=" * 78)
        print("CONTROLS (must hold still — otherwise no verdict below is trustworthy)")
        exempt_stable = a1["exempt"]["small"] == b["exempt"]["small"] == a2["exempt"]["small"]
        check("auth-exempt page identical across all 3 runs", exempt_stable,
              "%s / %s / %s" % (a1["exempt"]["small"], b["exempt"]["small"],
                                a2["exempt"]["small"]))
        large_stable = (a1["farbled"]["large"] == b["farbled"]["large"]
                        == a2["farbled"]["large"] == a1["exempt"]["large"])
        check(">=65536px canvas identical everywhere", large_stable,
              "%s / %s / %s (exempt %s)" % (a1["farbled"]["large"], b["farbled"]["large"],
                                            a2["farbled"]["large"], a1["exempt"]["large"]))

        print("\nFARBLING CONTRACTS")
        check("farbling is active at all (farbled != exempt)",
              a1["farbled"]["small"] != a1["exempt"]["small"],
              "farbled=%s exempt=%s" % (a1["farbled"]["small"], a1["exempt"]["small"]))
        # THE assertion. A constant seed passes everything above and fails only this.
        check("seed A != seed B  (per-user unlinkability)",
              a1["farbled"]["small"] != b["farbled"]["small"],
              "A=%s B=%s" % (a1["farbled"]["small"], b["farbled"]["small"]))
        check("seed A round-trips exactly  (stable across restarts)",
              a1["farbled"]["small"] == a2["farbled"]["small"],
              "A=%s A'=%s" % (a1["farbled"]["small"], a2["farbled"]["small"]))
    finally:
        print("\nrestoring original fingerprint_settings.json ...")
        # Browser down first, or its shutdown write races the restore. verify=False so a
        # stray process cannot raise out of a finally block and mask the real result.
        kill_browser_by_path(args.exe, verify=False)
        doc = read_settings(args.profile_dir)
        if original_seed is None:
            doc.pop("profileSeed", None)
        else:
            doc["profileSeed"] = original_seed
        if args.negative_control:
            sites = doc.get("siteSettings")
            if isinstance(sites, dict):
                orig_sites = original.get("siteSettings")
                if isinstance(orig_sites, dict) and FARBLED_HOST in orig_sites:
                    sites[FARBLED_HOST] = orig_sites[FARBLED_HOST]
                else:
                    sites.pop(FARBLED_HOST, None)
                doc["siteSettings"] = sites
        write_settings(args.profile_dir, doc)

    print("=" * 78)
    if args.negative_control:
        if failures:
            print("NEGATIVE CONTROL PASSED: with farbling disabled the harness went RED "
                  "on -> %s" % ", ".join(sorted(set(failures))))
            print("The green run therefore means something. Report both halves.")
            return 0
        print("NEGATIVE CONTROL FAILED: every assertion still passed with farbling "
              "DISABLED. This harness does not test what it claims — fix it before "
              "trusting any green result from it.")
        return 1

    if failures:
        print("RESULT: %d FAILED -> %s" % (len(failures), ", ".join(sorted(set(failures)))))
        print("If 'seed A != seed B' is the failure, the farbled value does not depend on "
              "the profile seed — that is the constant-seed class. See "
              "development-docs/TICKET_farbling_constant_seed_shipped.md.")
        return 1
    print("RESULT: all seed-rotation contracts hold "
          "(unlinkability + determinism, both controls stable)")
    print()
    print("Paste this single line into promote.yml's `farbling_rotation_token` input.")
    print("The gate re-derives every verdict from it -- it does not trust `verdict=`:")
    print()
    print("FARBLING-ROTATION-v1 engine=%s exempt=%s/%s/%s large=%s/%s/%s "
          "farbled=%s/%s/%s verdict=PASS"
          % (a1["engine"],
             a1["exempt"]["small"], b["exempt"]["small"], a2["exempt"]["small"],
             a1["farbled"]["large"], b["farbled"]["large"], a2["farbled"]["large"],
             a1["farbled"]["small"], b["farbled"]["small"], a2["farbled"]["small"]))
    return 0


if __name__ == "__main__":
    sys.exit(main())
