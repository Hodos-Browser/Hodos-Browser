#!/usr/bin/env python3
"""Phase 12 macOS asks — three arrivals, a FRESH video id each (Windows' trap)."""
import sys, time; sys.path.insert(0, '.')
import p12probe as Q, p35macprobe as P

VIDS = ["kXYiU_JCYtU", "YQHsXMglC9A", "RgKAFK5djSk", "hLQl3WQQoQ0", "60ItHLz5WEA"]


def quiet(fn, *a, **k):
    """A navigation kills the pending eval reply; that is not an error."""
    try:
        return fn(*a, **k)
    except Exception as e:
        return "NAV(" + type(e).__name__ + ")"


def report(off, tag):
    print("\n=== " + tag + " ===")
    seen = []
    for ln in Q.log_since(off).splitlines():
        if "P12DIAG" in ln or "💉" in ln:
            pid = ln.split(":")[0].lstrip("[")
            msg = ln.split("] ", 3)[-1] if "] " in ln else ln
            msg = msg.strip()
            seen.append((pid, msg))
            print(f"   pid={pid} {msg[:135]}")
    pids = {p for p, _ in seen}
    print(f"   -> renderer PIDs involved: {sorted(pids)}")
    for key in ("late-arrival inject", "OnContextCreated: injecting",
                "duplicate payload", "Pre-cached"):
        hits = [p for p, m in seen if key in m]
        print(f"   -> {key:<30} {'YES pid=' + hits[0] if hits else 'no'}")
    return seen


def newtab(url, t=60):
    h = P.Conn(P.headers()[0]["webSocketDebuggerUrl"], timeout=t)
    quiet(h.send_ipc, "tab_create", url)
    h.close()


def wait_tab(prefix, tries=40):
    for _ in range(tries):
        for t in P.cdp_targets():
            if t.get("type") == "page" and t.get("url", "").startswith(prefix):
                return t
        time.sleep(0.8)
    return None


P.preflight()

# ---------- 1. CROSS-SITE: github.com -> youtube ----------
newtab("https://github.com")
t = wait_tab("https://github.com")
c = P.Conn(t["webSocketDebuggerUrl"], timeout=60)
time.sleep(10)
off = Q.log_size()
quiet(c.js, f"location.href='https://www.youtube.com/watch?v={VIDS[0]}'")
time.sleep(20)
report(off, f"1. CROSS-SITE  github.com -> youtube/{VIDS[0]}")
quiet(c.close)

# ---------- 2. SAME-PROCESS: youtube -> youtube ----------
t = wait_tab("https://www.youtube.com")
c = P.Conn(t["webSocketDebuggerUrl"], timeout=60)
time.sleep(6)
off = Q.log_size()
quiet(c.js, f"location.href='https://www.youtube.com/watch?v={VIDS[1]}'")
time.sleep(20)
report(off, f"2. SAME-PROCESS  youtube -> youtube/{VIDS[1]}")
quiet(c.close)

# ---------- 3. P12-A3: NEW-TAB arrival via OnBeforePopup ----------
newtab("https://github.com")
t = wait_tab("https://github.com")
c = P.Conn(t["webSocketDebuggerUrl"], timeout=60)
time.sleep(10)
off = Q.log_size()
# window.open with a user gesture -> OnBeforePopup -> CreateNewTabWithUrl
quiet(c.js, f"window.open('https://www.youtube.com/watch?v={VIDS[2]}','_blank')")
time.sleep(22)
report(off, f"3. P12-A3 NEW-TAB (OnBeforePopup) -> youtube/{VIDS[2]}")
quiet(c.close)
