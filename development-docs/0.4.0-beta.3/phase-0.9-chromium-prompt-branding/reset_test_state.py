#!/usr/bin/env python3
"""
beta.3 Phase 0.9 — dev test-state control.

WHY THIS EXISTS
---------------
Every failed run on 2026-08-24 failed because of unknown STATE, not broken code:

  * example.com carried a loopback BLOCK stored four days earlier, so no prompt
    appeared — and the wrong conclusion ("Chromium can't prompt") was drawn from it.
  * bitgenius.net was already approved in the wallet, so no connect modal opened and
    the binding under test never engaged.
  * A profile deleted while in use was recreated with the same id and silently
    inherited ~200 MB of the old profile's settings.

Testing against unmeasured state produces wrong conclusions faster than it produces
evidence. Every command here ends by RE-READING what it changed and printing it, and
`verify` exits non-zero if reality does not match what was asked for.

⛔ Stop the dev browser first. Chromium rewrites Preferences on exit and will clobber
   edits made underneath a running process.
⛔ Dev only. Refuses to touch %APPDATA%/HodosBrowser (the installed browser).

USAGE
    python reset_test_state.py show
    python reset_test_state.py clear-loopback <profile-id|ALL>
    python reset_test_state.py clear-wallet-domain <domain|ALL>
    python reset_test_state.py verify [--profile P] [--domain D]
"""

import glob
import json
import os
import shutil
import sqlite3
import sys
import time

def _roots():
    """Resolve the dev/prod data directories for this platform.

    ⛔ MEASURED 2026-08-26 (macOS): this file previously read %APPDATA% unconditionally.
    APPDATA is unset on macOS, so DEV_ROOT collapsed to the bare relative name
    "HodosBrowserDev", `show` died with "dev data dir not found", and the macOS side
    could not run this phase's PREREQUISITE at all — which is why every Phase 0.9 item
    was still unrun there. Worse for the safety guard: PROD_ROOT collapsed to the bare
    "HodosBrowser", so the refuse-to-touch-prod comparison was meaningless on macOS.
    Test-harness only, HARNESS §6.
    """
    if sys.platform == "darwin":
        base = os.path.expanduser("~/Library/Application Support")
    elif os.name == "nt":
        base = os.environ.get("APPDATA", "")
    else:  # linux / other — XDG-ish, kept so the guard still resolves to real paths
        base = os.environ.get("XDG_CONFIG_HOME", os.path.expanduser("~/.config"))
    return os.path.join(base, "HodosBrowserDev"), os.path.join(base, "HodosBrowser")


DEV_ROOT, PROD_ROOT = _roots()
NETWORK_KEYS = ("loopback_network", "local_network", "local_network_access")
SETTING = {1: "ALLOW", 2: "BLOCK", 3: "ASK"}


def guard():
    if not os.path.isdir(DEV_ROOT):
        sys.exit(f"dev data dir not found: {DEV_ROOT}")
    # Never operate on the installed browser's data.
    if os.path.normcase(DEV_ROOT) == os.path.normcase(PROD_ROOT):
        sys.exit("refusing to operate on the installed browser's data directory")


def backup(path):
    dst = f"{path}.bak-{time.strftime('%Y%m%d-%H%M%S')}"
    shutil.copy2(path, dst)
    return dst


def pref_files():
    return sorted(glob.glob(os.path.join(DEV_ROOT, "*", "*", "Preferences")))


def profile_of(pref_path):
    return os.path.relpath(pref_path, DEV_ROOT).split(os.sep)[0]


def read_loopback(pref_path):
    try:
        with open(pref_path, encoding="utf-8") as f:
            d = json.load(f)
    except Exception as e:
        return None, f"unreadable: {e}"
    ex = d.get("profile", {}).get("content_settings", {}).get("exceptions", {})
    rows = []
    for k in NETWORK_KEYS:
        v = ex.get(k)
        if isinstance(v, dict):
            for pat, val in v.items():
                s = val.get("setting") if isinstance(val, dict) else val
                rows.append((k, pat, SETTING.get(s, s)))
    return rows, None


def wallet_db():
    p = os.path.join(DEV_ROOT, "wallet", "wallet.db")
    return p if os.path.isfile(p) else None


def read_wallet_domains():
    db = wallet_db()
    if not db:
        return None
    con = sqlite3.connect(f"file:{db}?mode=ro", uri=True)
    try:
        cols = [c[1] for c in con.execute("PRAGMA table_info(domain_permissions)").fetchall()]
        if not cols:
            return []
        key = "domain" if "domain" in cols else cols[0]
        return [r[0] for r in con.execute(f"SELECT {key} FROM domain_permissions ORDER BY {key}")]
    finally:
        con.close()


def cmd_show():
    print(f"=== dev root: {DEV_ROOT} ===\n")
    print("-- profiles (registry) --")
    reg = os.path.join(DEV_ROOT, "profiles.json")
    ids = []
    if os.path.isfile(reg):
        d = json.load(open(reg, encoding="utf-8"))
        print(f"   currentProfileId={d.get('currentProfileId')} defaultProfileId={d.get('defaultProfileId')}")
        for p in d.get("profiles", []):
            ids.append(p["id"])
            print(f"     {p['id']:<12} {p.get('name','')}")
    print("-- profile dirs on disk --")
    disk = sorted(n for n in os.listdir(DEV_ROOT)
                  if os.path.isdir(os.path.join(DEV_ROOT, n)) and (n == "Default" or n.startswith("Profile_")))
    for n in disk:
        tag = "" if n in ids or n == "Default" else "   <-- ORPHAN (not in registry)"
        print(f"     {n}{tag}")

    print("\n-- loopback / local-network content settings --")
    any_rows = False
    for pf in pref_files():
        rows, err = read_loopback(pf)
        if err:
            print(f"   {profile_of(pf)}: {err}")
            continue
        for k, pat, s in rows:
            any_rows = True
            print(f"   {profile_of(pf):<12} {k:<22} {pat:<50} {s}")
    if not any_rows:
        print("   (none — clean)")

    print("\n-- wallet domain_permissions (GLOBAL: shared by every profile) --")
    doms = read_wallet_domains()
    if doms is None:
        print("   (wallet.db not found)")
    elif not doms:
        print("   (none)")
    else:
        for d in doms:
            print(f"   {d}")


def cmd_clear_loopback(target):
    changed = 0
    for pf in pref_files():
        prof = profile_of(pf)
        if target != "ALL" and prof != target:
            continue
        try:
            with open(pf, encoding="utf-8") as f:
                d = json.load(f)
        except Exception as e:
            print(f"   {prof}: unreadable ({e})")
            continue
        ex = d.get("profile", {}).get("content_settings", {}).get("exceptions", {})
        hit = any(isinstance(ex.get(k), dict) and ex.get(k) for k in NETWORK_KEYS)
        if not hit:
            continue
        print(f"   {prof}: backup -> {os.path.basename(backup(pf))}")
        for k in NETWORK_KEYS:
            if isinstance(ex.get(k), dict):
                ex[k] = {}
        with open(pf, "w", encoding="utf-8") as f:
            json.dump(d, f, separators=(",", ":"))
        changed += 1
    print(f"   cleared in {changed} profile(s)")
    # Re-read rather than trust the write.
    for pf in pref_files():
        rows, _ = read_loopback(pf)
        if rows:
            print(f"   REMAINING {profile_of(pf)}: {rows}")


def cmd_clear_wallet_domain(domain):
    db = wallet_db()
    if not db:
        sys.exit("wallet.db not found")
    print(f"   backup -> {os.path.basename(backup(db))}")
    con = sqlite3.connect(db)
    try:
        # ⛔ Without this the child rows do NOT cascade — a silent no-op that cost
        #    time in P0.8. sqlite3 defaults foreign_keys OFF per connection.
        con.execute("PRAGMA foreign_keys=ON")
        cols = [c[1] for c in con.execute("PRAGMA table_info(domain_permissions)").fetchall()]
        key = "domain" if "domain" in cols else cols[0]
        if domain == "ALL":
            n = con.execute(f"DELETE FROM domain_permissions").rowcount
        else:
            n = con.execute(f"DELETE FROM domain_permissions WHERE {key}=?", (domain,)).rowcount
        con.commit()
        print(f"   deleted {n} row(s)")
    finally:
        con.close()
    left = read_wallet_domains()
    print(f"   remaining domains: {left if left else '(none)'}")


def cmd_verify(profile=None, domain=None):
    ok = True
    for pf in pref_files():
        prof = profile_of(pf)
        if profile and prof != profile:
            continue
        rows, err = read_loopback(pf)
        if err:
            print(f"FAIL {prof}: {err}")
            ok = False
        elif rows:
            print(f"FAIL {prof}: loopback settings still present: {rows}")
            ok = False
        else:
            print(f"ok   {prof}: no loopback settings")
    if domain:
        doms = read_wallet_domains() or []
        if domain in doms:
            print(f"FAIL wallet still approves {domain}")
            ok = False
        else:
            print(f"ok   wallet does not approve {domain}")
    print("VERIFY PASSED" if ok else "VERIFY FAILED")
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    guard()
    args = sys.argv[1:]
    if not args:
        sys.exit(__doc__)
    cmd = args[0]
    if cmd == "show":
        cmd_show()
    elif cmd == "clear-loopback":
        cmd_clear_loopback(args[1] if len(args) > 1 else "ALL")
    elif cmd == "clear-wallet-domain":
        if len(args) < 2:
            sys.exit("need a domain (or ALL)")
        cmd_clear_wallet_domain(args[1])
    elif cmd == "verify":
        prof = args[args.index("--profile") + 1] if "--profile" in args else None
        dom = args[args.index("--domain") + 1] if "--domain" in args else None
        cmd_verify(prof, dom)
    else:
        sys.exit(__doc__)
