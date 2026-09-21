#!/usr/bin/env python3
"""
Phase 12 — macOS sanity-check of Windows' 2026-09-21 mitigation (relay round 21a).

THE TWO ASKS:
  1. cross-site arrival (github.com -> a YouTube watch URL) must produce
     `💉 P12: late-arrival inject` in the DESTINATION renderer;
     same-process arrival must still produce `💉 OnContextCreated: injecting`
     followed by `💉 P12: duplicate payload ... dropped`.
  2. `P12-A3` — the NEW-TAB arrival (OnBeforePopup -> CreateNewTabWithUrl),
     unmeasured on both platforms.

⛔ THE TRAP, from Windows, and it is why every trial gets a FRESH video id:
`s_scriptCache` is URL-keyed and ONE-SHOT, so a payload that arrived too late in
trial n sits in the cache and is consumed by trial n+1's context — which looks
exactly like a working early injection. Reusing one URL scored a false 2-of-3
GREEN for a fix that did not work.

⛔ macOS LOG DIVERGENCE, measured here: the `[RENDER]` lines do NOT go to
`cef_debug.log`. `cef_browser_shell_mac.mm` sets `settings.log_file` to the
RELATIVE path "debug.log", so they land in the browser's CWD — for a launch from
`cef-native/` that is `cef-native/debug.log`. It is also NOT truncated per launch
here, so every read below is from a byte offset taken before the trial.
"""
import json, os, subprocess, sys, time
import p35macprobe as P

CEF_LOG = "/Users/matt/Hodos-Browser/cef-native/debug.log"
VIDEOS = ["dQw4w9WgXcQ", "jNQXAC9IVRw", "9bZkp7q19f0", "kJQP7kiw5Fk",
          "3JZ_D3ELwOQ", "L_jWHffIx5E", "fJ9rUzIMcZQ"]


def log_size():
    try:
        return os.path.getsize(CEF_LOG)
    except OSError:
        return 0


def log_since(offset):
    with open(CEF_LOG, "rb") as f:
        f.seek(offset)
        return f.read().decode("utf-8", "replace")


def scriptlet_lines(text):
    """Only the lines this phase is about, with their renderer PID prefix intact.

    ⛔ The PID prefix is load-bearing: `hodos::LogSafeUrl` is ORIGIN-ONLY, so every
    one of these prints bare `https://www.youtube.com` and the URL cannot tell you
    which process, or which navigation, it belongs to.
    """
    out = []
    for ln in text.splitlines():
        if "💉" in ln and ("scriptlet" in ln or "P12" in ln or "injecting" in ln):
            out.append(ln.strip())
    return out


def new_tab(url):
    h = P.Conn(P.headers()[0]["webSocketDebuggerUrl"])
    h.send_ipc("tab_create", url)
    h.close()


def tab_for(prefix, tries=30):
    for _ in range(tries):
        for t in P.cdp_targets():
            if t.get("type") == "page" and t.get("url", "").startswith(prefix):
                return t
        time.sleep(0.7)
    return None


def navigate(conn, url):
    conn.call("Page.navigate", {"url": url})


def trial(label, steps, settle=9):
    """steps: list of (description, callable). Returns the scriptlet lines produced."""
    off = log_size()
    for desc, fn in steps:
        fn()
        time.sleep(settle)
    lines = scriptlet_lines(log_since(off))
    print(f"\n--- {label} ---")
    if not lines:
        print("   (no scriptlet lines at all)")
    for l in lines:
        print("   " + l[:190])
    return lines
