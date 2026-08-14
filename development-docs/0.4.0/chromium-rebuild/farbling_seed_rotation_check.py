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

## Coverage

All four native vectors, measured in one page visit so they share the same three restarts:

    canvas  getImageData          C3   fork dfe5a2343+
    webgl   readPixels            C4   fork c63654654+
    audio   getChannelData        C5   fork c63654654+
    navigator deviceMemory/cores  C6   fork c63654654+

⚠️ Only meaningful against a build whose CEF actually carries those patches. Against an
older binary farbling is absent and this correctly reports RED — check `CEF_VERSION`
(printed as `engine=`) before believing a failure. Release builds and, until it rebuilds,
macOS are still M136, where every one of these is inert by construction.

⚠️ **The `FARBLING-ROTATION-v1` token deliberately still carries the canvas figures only.**
`promote.yml` parses that exact shape and re-derives its verdict from it; widening the
token is a release-gate change, not a harness change, and is left as an explicit follow-up.
So this script asserts MORE than the promote gate checks — a green gate is not a substitute
for reading this script's own output.

⚠️ There is deliberately **no `A != B` assertion on the navigator values**. See the comment
at that check: their ranges are far too small for it to be anything but flaky.
"""

import argparse
import ctypes
import json
import os
import secrets
import signal
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


# Every value C3/C4/C5/C6 farbles, measured in one page visit so all four ride the same
# three browser restarts.
#
# Each vector carries its own IN-PAGE control wherever the implementation has a size gate,
# because a control that sits outside the gate is never farbled on any page or any seed --
# so if it ever moves, the two measurements are not comparable and no verdict below is
# trustworthy:
#
#   canvas  small 200x50   = 10,000 px  -> inside  the <65536px gate -> farbled
#           large 400x200  = 80,000 px  -> outside the gate          -> CONTROL
#   webgl   small 32x32    =  4,096 B   -> inside  the <262144B gate -> farbled
#           large 256x256  = 262,144 B  -> exactly ON the bound, so outside it -> CONTROL
#
# audio and navigator have no size gate, so their control is the cross-page one: the
# auth-exempt origin, which is a true native pass-through.
MEASURE_JS = r"""
(async function () {
  function fnv(bytes) {
    var h = 2166136261 >>> 0;
    for (var i = 0; i < bytes.length; i++) { h ^= bytes[i]; h = Math.imul(h, 16777619) >>> 0; }
    return ('0000000' + (h >>> 0).toString(16)).slice(-8);
  }
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
    return fnv(c.getContext('2d').getImageData(0, 0, c.width, c.height).data);
  }

  // --- WebGL readPixels (C4) ---------------------------------------------------------
  // A flat clear colour is enough: C4 perturbs the readback buffer, not the rendering, so
  // the scene only has to be deterministic. Anything driver-dependent would add variance
  // that the exempt/large controls would then have to absorb.
  function glHash(size) {
    try {
      var c = document.createElement('canvas');
      c.width = size; c.height = size;
      var gl = c.getContext('webgl') || c.getContext('experimental-webgl');
      if (!gl) { return 'ERR:nocontext'; }
      gl.clearColor(0.25, 0.5, 0.75, 1.0);
      gl.clear(gl.COLOR_BUFFER_BIT);
      var px = new Uint8Array(size * size * 4);
      gl.readPixels(0, 0, size, size, gl.RGBA, gl.UNSIGNED_BYTE, px);
      return fnv(px);
    } catch (e) { return 'ERR:' + e.message; }
  }

  // --- WebAudio (C5) -----------------------------------------------------------------
  // The canonical fingerprint: a tone through a DynamicsCompressor, rendered offline and
  // hashed. Slicing away the head skips the compressor's attack ramp, where tiny timing
  // differences (not farbling) can move samples.
  var audio = 'ERR';
  var audioTwice = 'ERR';
  try {
    var ctx = new OfflineAudioContext(1, 44100, 44100);
    var osc = ctx.createOscillator(); osc.type = 'triangle'; osc.frequency.value = 10000;
    var comp = ctx.createDynamicsCompressor();
    osc.connect(comp); comp.connect(ctx.destination); osc.start(0);
    var buf = await ctx.startRendering();
    audio = fnv(new Uint8Array(new Float32Array(buf.getChannelData(0).slice(4000, 9000)).buffer));
    // Read the SAME buffer a second time. C5 perturbs the buffer's own storage, so a
    // missing once-only guard would compound the factor and change this hash. This is the
    // in-page assertion for that specific defect.
    audioTwice = fnv(new Uint8Array(new Float32Array(buf.getChannelData(0).slice(4000, 9000)).buffer));
  } catch (e) { audio = 'ERR:' + e.message; }

  return JSON.stringify({
    href: location.href,
    small: pixHash(draw(200, 50)),
    large: pixHash(draw(400, 200)),
    glSmall: glHash(32),
    glLarge: glHash(256),
    audio: audio,
    audioTwice: audioTwice,
    deviceMemory: (typeof navigator.deviceMemory === 'number') ? navigator.deviceMemory : null,
    cores: (typeof navigator.hardwareConcurrency === 'number')
             ? navigator.hardwareConcurrency : null
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
    """⚠️ The per-site value is an OBJECT, `{"enabled": false}` — not a bare `false`.

    A bare boolean is silently ignored by the loader and farbling stays ON. The direction
    is safe (it fails towards more protection, not less) but it costs a confusing probe
    run, because the harness then measures a farbled page while believing it disabled the
    feature — i.e. a negative control that quietly isn't one. Anyone hand-editing
    fingerprint_settings.json will reach for the bare boolean first. (Reported by the Mac
    session, 2026-08-10.)
    """
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


def _exe_path_for_pid(pid):
    """The kernel's real executable path for `pid`, or "" if it cannot be determined.

    ⛔⛔ THIS EXISTS BECAUSE argv[0] IS NOT A PATH YOU CAN TRUST, and matching on it
    reproduced the exact failure this module's tripwire was written to catch.

    Measured 2026-08-10: three browser processes launched as `./HodosBrowser.app/Contents/
    MacOS/HodosBrowser` (from a shell sitting in `build/bin`) carry a RELATIVE argv[0].
    A prefix test against the absolute bundle path does not match them, so the scan saw
    only the helpers -- whose argv[0] Chromium does set absolutely. `kill_browser_by_path`
    then killed helpers while three browser processes stayed alive and immediately
    respawned them, so the kill never converged; and `count_browser_procs` would have
    reported 0 the moment only browser processes remained.

    That is the constant-seed signature verbatim: a "successful" kill, a relaunch absorbed
    by the surviving instance, and CDP still serving the ORIGINAL process with the ORIGINAL
    seed. `launch_browser` happens to pass an absolute path, which is why the harness's own
    runs were unaffected -- i.e. the bug was invisible to exactly the code path we exercise
    most, and only a hand-launched browser exposed it.

    `proc_pidpath` (libproc) reports what the kernel actually executed, independent of
    argv[0], the caller's cwd, and any symlink in the launch path.
    """
    global _LIBPROC
    if _LIBPROC is None:
        try:
            _LIBPROC = ctypes.CDLL("/usr/lib/libSystem.B.dylib", use_errno=True)
        except OSError:
            _LIBPROC = False
    if _LIBPROC is False:
        return ""
    buf = ctypes.create_string_buffer(4096)
    n = _LIBPROC.proc_pidpath(ctypes.c_int(pid), buf, ctypes.c_uint32(len(buf)))
    if n <= 0:
        return ""
    return buf.value.decode("utf-8", "replace")


_LIBPROC = None


def _posix_scope_dir(exe_path):
    """The directory that plays the role Windows' `dirname(exe)` plays.

    ⛔ NOT `dirname(exe)` on macOS, and this cost a wrong result before it was caught by
    the counter's own positive control. Windows' exe is `build/bin/Release/HodosBrowser.exe`
    and every child process lives in that same flat directory. macOS's is buried at
    `build/bin/HodosBrowser.app/Contents/MacOS/HodosBrowser`, while the CEF helpers live at
    `build/bin/HodosBrowser.app/Contents/Frameworks/HodosBrowser Helper.app/Contents/MacOS/`.
    So `dirname(exe)` scopes to the ONE main process and misses all five helpers; measured
    here it counted 1 of 6.

    The `.app` bundle root is the correct analogue: it covers browser + renderers +
    utilities, and nothing else. It stays PATH-scoped, so the installed production browser
    in /Applications is still never a target — which is the whole point of kill-by-path.
    """
    p = os.path.abspath(exe_path)
    marker = ".app" + os.sep + "Contents" + os.sep + "MacOS" + os.sep
    i = p.find(marker)
    if i != -1:
        return p[:i + len(".app")]
    return os.path.dirname(p)


def _posix_procs_under(target_dir):
    """[(pid, argv_string)] for every process belonging to the bundle at target_dir.

    Membership is decided by the KERNEL's exec path first, and by argv[0]'s prefix only as
    an additional catch for service children launched through a relative path via the
    bundle (see the second match below). argv[0] never *removes* a match.

    ⛔⛔ This replaced `pgrep -fc <exe>`, which was silently broken on macOS and is the
    reason this docstring is long. THREE separate defects, each individually fatal:

      1. **BSD `pgrep` has no `-c`.** macOS `pgrep` accepts only [-Lfilnoqvx]. The call
         exited rc=2 with an EMPTY stdout and the usage text on stderr, so
         `int(r.stdout.strip() or 0)` evaluated to **0 — always, unconditionally**,
         whether 0 or 600 browsers were running. `count_browser_procs` was a constant
         function. That meant `kill_browser_by_path(verify=True)` ALWAYS believed the
         kill succeeded, disarming the exact tripwire whose docstring below explains it
         is "not optional paranoia". Same false-negative family as `strings` on the
         >4 GB dSYM: the tool read nothing and reported the all-clear.
      2. **It matched the wrong set.** The full exe path matches ONLY the main process.
         Measured on macOS at this pin: 6 processes run under `build/bin` (1 browser,
         3 `--type=renderer`, 2 `--type=utility`) and `pkill -f <exe>` matched **1**.
         Windows matches `ExecutablePath.StartsWith(<dirname of exe>)`, i.e. all of them.
         Matching argv[0]'s prefix against the DIRECTORY restores parity — CEF helpers
         live at `<bin>/HodosBrowser.app/Contents/Frameworks/...`, under the same dir.
      3. **`pgrep -f` takes an extended REGEX, not a literal.** Every `.` in the path is
         a wildcard and `[`/`]` in a user or build directory would be a character class.
         Substring-matching argv[0] in Python removes that class of bug entirely.

    Raises rather than returning 0 if `ps` itself fails — "I could not look" must never be
    reportable as "nothing is running".
    """
    r = subprocess.run(["ps", "-ax", "-ww", "-o", "pid=,args="],
                       capture_output=True, text=True, check=False)
    lines = [l for l in r.stdout.splitlines() if l.strip()]
    if r.returncode != 0 or not lines:
        raise SystemExit(
            "ps failed (rc=%s, %d lines): cannot enumerate processes, so a kill cannot "
            "be verified and an attach cannot be counted. Refusing to report 0.\n%s"
            % (r.returncode, len(lines), r.stderr[:300]))
    prefix = target_dir.rstrip(os.sep) + os.sep
    out = []
    for line in lines:
        pid, _, args = line.strip().partition(" ")
        try:
            pid = int(pid)
        except ValueError:
            continue
        exe = _exe_path_for_pid(pid)
        # Match the KERNEL's executable path, falling back to argv[0] only when libproc
        # cannot answer (e.g. the process died between ps and here).
        probe = exe if exe else args
        if probe.startswith(prefix):
            out.append((pid, args))
            continue
        # ⛔ AND the kernel path alone MISSES the browser's own service children, which
        # is how an orphan survives a kill and then breaks the NEXT boot.
        #
        # The shell launches the wallet and the adblock engine through a relative path
        # that traverses the bundle:
        #
        #   argv[0] .../HodosBrowser.app/Contents/MacOS/../../../../../../rust-wallet/...
        #   kernel  /Users/<u>/Hodos-Browser/rust-wallet/target/release/hodos-wallet
        #
        # so argv[0] sits under the bundle textually while proc_pidpath resolves OUTSIDE
        # it. They are genuinely part of this bundle's process tree -- and they inherit
        # the browser's CDP LISTENING SOCKET. Measured 2026-08-14: after a kill they held
        # 127.0.0.1:9322 with the browser gone, the next launch could not bind it, and the
        # harness died with "CDP <port> never came up" against perfectly good code.
        #
        # This is purely ADDITIVE -- the kernel path stays primary and decides first, so
        # the 2026-08-10 finding (argv[0] is untrustworthy because a RELATIVE argv[0] makes
        # a real process invisible) is not reintroduced: argv[0] here can only ADD matches,
        # never remove one. It also cannot widen scope to the installed production browser,
        # whose argv[0] starts with /Applications and matches no dev-bundle prefix.
        if args.split(" ", 1)[0].startswith(prefix):
            out.append((pid, args))
    return out


def count_browser_procs(exe_path):
    """How many processes are running out of the directory of exe_path."""
    target_dir = os.path.dirname(os.path.abspath(exe_path))
    if sys.platform != "win32":
        return len(_posix_procs_under(_posix_scope_dir(exe_path)))
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
        # SIGKILL every process under the bundle, helpers included. `pkill -f <exe>`
        # reached only the browser process; see _posix_procs_under for why that and the
        # count it was verified with were both wrong.
        #
        # ⛔ ORDER AND REPETITION ARE BOTH LOAD-BEARING, and the `verify` tripwire below
        # caught their absence on the first run of this port: a flat one-pass kill left
        # 2 processes alive. Chromium's browser process RESTARTS a helper that dies, so
        # killing helpers while the browser still lives just spawns replacements. Kill the
        # browser process (no `--type=`) FIRST so nothing is left to respawn anything, then
        # re-enumerate and repeat -- a process that appeared between the enumeration and
        # the kill is invisible to a single pass, and on macOS that race is routine, not
        # theoretical.
        scope = _posix_scope_dir(exe_path)
        for _round in range(5):
            procs = _posix_procs_under(scope)
            if not procs:
                break
            # Browser process first (it is the one with no --type= switch).
            procs.sort(key=lambda pa: ("--type=" in pa[1], pa[0]))
            for pid, _args in procs:
                try:
                    os.kill(pid, signal.SIGKILL)
                except (ProcessLookupError, PermissionError):
                    pass
            time.sleep(1.0)
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
            # Report the scope actually searched. On macOS that is the .app bundle root,
            # NOT dirname(exe) -- printing the latter sent the first triage of this very
            # failure looking in Contents/MacOS, which holds one of the eight processes.
            scope_desc = (target_dir if sys.platform == "win32"
                          else _posix_scope_dir(exe_path))
            raise SystemExit(
                f"kill-by-path left {left} process(es) running under {scope_desc}. "
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

def cef_version(binaries_root=None):
    """The CEF version string, read from the HEADERS the app was built against.

    ⛔ WHY THIS EXISTS, and why engine_version() is not a substitute.

    engine_version() reports the CHROMIUM version over CDP. Two different Hodos engines
    can carry the SAME Chromium version -- P4e (7dd0357) and P4f (9ccef04) are both
    Chromium 150.0.7871.187, differing only in our Blink patches. So engine_version()
    proves two arms are the same build; it CANNOT prove which build that is, and a
    post-fix run against a stale binary would report the pre-fix engine string and look
    entirely correct.

    CEF_VERSION does distinguish them (`+g7dd0357` vs `+g9ccef04`), so a harness that
    needs to assert WHICH engine it measured reads this. It is a build-artifact check,
    not a runtime one -- pair it with an mtime/size check on the libcef the app actually
    loaded if you need the full chain.
    """
    if binaries_root is None:
        here = os.path.dirname(os.path.abspath(__file__))
        binaries_root = os.path.abspath(
            os.path.join(here, "..", "..", "..", "cef-binaries"))
    path = os.path.join(binaries_root, "include", "cef_version.h")
    try:
        with open(path, "r", encoding="utf-8") as fh:
            for line in fh:
                if line.startswith("#define CEF_VERSION "):
                    return line.split('"')[1]
    except OSError:
        pass
    return "unknown"


def engine_version(port):
    """The engine string CDP reports, e.g. 'Chrome/150.0.7871.187'.

    ⚠️ This is the CHROMIUM version and it does NOT identify our engine -- see
    cef_version() above. Do not use it alone to assert you measured a rebuilt binary.

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


def measure(port, chrome_ids, url, want_host, timeout=60, js=None):
    """Navigate the real tab and read back both canvas hashes.

    `js` lets a sibling harness reuse this navigation + subject-assertion path with a
    different probe (see farbling_acceptance_battery.py) instead of re-implementing the
    tab resolution, the chrome-id exclusion and the href re-check — which is where the
    wrong-subject defects in this family have always come from. It must be an expression
    returning a JSON string with an `href` field, and it may be async. Defaults to
    MEASURE_JS, so every existing caller is unaffected."""
    if js is None:
        js = MEASURE_JS
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
                # awaitPromise: MEASURE_JS is async because the WebAudio vector renders an
                # OfflineAudioContext. Without it the value comes back as an unresolved
                # Promise and every measurement silently fails the "value" check below.
                ws.send(json.dumps({"id": 2, "method": "Runtime.evaluate",
                                    "params": {"expression": js,
                                               "returnByValue": True,
                                               "awaitPromise": True}}))
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

    for who, v in (("exempt ", ex), ("farbled", fa)):
        print(f"    {who} canvas={v['small']}/{v['large']}  webgl={v['glSmall']}/{v['glLarge']}"
              f"  audio={v['audio']}  mem={v['deviceMemory']}  cores={v['cores']}")
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

        # ------------------------------------------------------------------------------
        # C4 WebGL / C5 WebAudio / C6 Navigator
        # ------------------------------------------------------------------------------
        print("\nWEBGL readPixels (C4)")
        gl_err = [p for p in (a1, b, a2)
                  for v in (p["exempt"], p["farbled"])
                  if str(v["glSmall"]).startswith("ERR")]
        check("a WebGL context was actually obtained", not gl_err,
              a1["farbled"]["glSmall"] if gl_err else "ok on all runs")
        check(">=262144B readPixels identical everywhere (control)",
              a1["farbled"]["glLarge"] == b["farbled"]["glLarge"]
              == a2["farbled"]["glLarge"] == a1["exempt"]["glLarge"],
              "%s / %s / %s (exempt %s)" % (a1["farbled"]["glLarge"], b["farbled"]["glLarge"],
                                            a2["farbled"]["glLarge"], a1["exempt"]["glLarge"]))
        check("webgl farbled != exempt", a1["farbled"]["glSmall"] != a1["exempt"]["glSmall"],
              "farbled=%s exempt=%s" % (a1["farbled"]["glSmall"], a1["exempt"]["glSmall"]))
        check("webgl seed A != seed B  (unlinkability)",
              a1["farbled"]["glSmall"] != b["farbled"]["glSmall"],
              "A=%s B=%s" % (a1["farbled"]["glSmall"], b["farbled"]["glSmall"]))
        check("webgl seed A round-trips  (determinism)",
              a1["farbled"]["glSmall"] == a2["farbled"]["glSmall"],
              "A=%s A'=%s" % (a1["farbled"]["glSmall"], a2["farbled"]["glSmall"]))

        print("\nWEBAUDIO (C5)")
        check("audio rendered without error",
              not str(a1["farbled"]["audio"]).startswith("ERR"), a1["farbled"]["audio"])
        check("audio farbled != exempt", a1["farbled"]["audio"] != a1["exempt"]["audio"],
              "farbled=%s exempt=%s" % (a1["farbled"]["audio"], a1["exempt"]["audio"]))
        check("audio seed A != seed B  (unlinkability)",
              a1["farbled"]["audio"] != b["farbled"]["audio"],
              "A=%s B=%s" % (a1["farbled"]["audio"], b["farbled"]["audio"]))
        check("audio seed A round-trips  (determinism)",
              a1["farbled"]["audio"] == a2["farbled"]["audio"],
              "A=%s A'=%s" % (a1["farbled"]["audio"], a2["farbled"]["audio"]))
        # Reading the same AudioBuffer twice must give the same bytes. C5 perturbs the
        # buffer's OWN storage with a deterministic factor, so a missing once-only guard
        # multiplies by factor^n and this is the assertion that catches it.
        check("same AudioBuffer read twice is identical  (no compounding)",
              a1["farbled"]["audio"] == a1["farbled"]["audioTwice"],
              "1st=%s 2nd=%s" % (a1["farbled"]["audio"], a1["farbled"]["audioTwice"]))

        print("\nNAVIGATOR (C6)")
        # ⚠️ NO 'A != B' ASSERTION HERE, deliberately. deviceMemory has 4 legal values and
        # hardwareConcurrency has (real-1), so two different seeds collide often -- a 1-in-4
        # chance for deviceMemory alone. An A!=B check on these would be FLAKY, and a flaky
        # assertion in a release gate is worse than no assertion: it trains people to re-run
        # until green. Unlinkability for these low-entropy values is carried by canvas,
        # webgl and audio, which hash over enough bits for the check to be sound.
        mem_f, mem_e = a1["farbled"]["deviceMemory"], a1["exempt"]["deviceMemory"]
        # PRESENCE CHECK -- without this, C6 has no negative control.
        #
        # Measured 2026-08-09 against a binary that genuinely lacked C6: every other
        # navigator assertion below still passed, because this machine's NATIVE values
        # (deviceMemory 32, cores 24) are themselves a legal farbled answer -- 32 is in the
        # allowed set and cores==real satisfies reduce-only. So they are plausibility
        # checks, not evidence the feature ran.
        #
        # A single-value "farbled != native" check would be flaky (deviceMemory collides
        # 1-in-4). Requiring only that the PAIR differs from native for AT LEAST ONE of the
        # two seeds makes a false failure need mem AND cores to both land on their native
        # value for both seed A and seed B: ~(1/4 * 1/23)^2 ~= 1 in 8,500 runs on a 24-core
        # box, and rarer on machines with more cores. Quantified here so a future reader can
        # judge it rather than rediscover it.
        native_pair = (a1["exempt"]["deviceMemory"], a1["exempt"]["cores"])
        check("navigator farbling is active at all (pair != native for some seed)",
              (a1["farbled"]["deviceMemory"], a1["farbled"]["cores"]) != native_pair
              or (b["farbled"]["deviceMemory"], b["farbled"]["cores"]) != native_pair,
              "A=%s B=%s native=%s"
              % ((a1["farbled"]["deviceMemory"], a1["farbled"]["cores"]),
                 (b["farbled"]["deviceMemory"], b["farbled"]["cores"]), native_pair))
        check("deviceMemory in the desktop set {4,8,16,32}", mem_f in (4, 8, 16, 32),
              "farbled=%s (native on this machine=%s)" % (mem_f, mem_e))
        check("deviceMemory deterministic across restarts",
              mem_f == a2["farbled"]["deviceMemory"],
              "A=%s A'=%s" % (mem_f, a2["farbled"]["deviceMemory"]))
        # The exempt page is a native pass-through, so it reports this machine's REAL core
        # count -- which is what makes reduce-only checkable at all from inside the page.
        cores_f, cores_real = a1["farbled"]["cores"], a1["exempt"]["cores"]
        check("hardwareConcurrency REDUCE-ONLY (<= real, >= 2)",
              isinstance(cores_f, int) and isinstance(cores_real, int)
              and 2 <= cores_f <= cores_real,
              "farbled=%s real=%s" % (cores_f, cores_real))
        check("hardwareConcurrency deterministic across restarts",
              cores_f == a2["farbled"]["cores"],
              "A=%s A'=%s" % (cores_f, a2["farbled"]["cores"]))
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
