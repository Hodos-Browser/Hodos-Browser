"""Phase 2 / E1 — does a hung /wallet/balance stall unrelated web pages?

MECHANISM: SimpleHandler::OnProcessMessageReceived("get_balance") runs on the browser
process UI THREAD and calls WalletService::getBalance -> makeHttpRequest -> synchronous
WinHTTP with NO WinHttpSetTimeouts (so WinHTTP defaults apply). The UI thread drives every
browser in the process, tabs included.

SUBJECT: dev build cef-native/build/bin/Release, HODOS_DEV=1, --profile=Default, CDP 9322,
wallet 31401 = stub_wallet.py. cdp.py refuses 9222, so the installed browser cannot be hit.
NEGATIVE CONTROL: same binary, same script, same actions, stub hang=0.
"""
import json, sys, time, urllib.request
sys.path.insert(0, r"C:\Users\archb\Hodos-Browser\development-docs\0.4.0-beta.3\phase-1-overlay-input-dpi")
import websocket, cdp

HEADER = "=http://127.0.0.1:5137/"
T = 300


def ctl(h):
    urllib.request.urlopen("http://127.0.0.1:31401/__ctl?hang=%d" % (1 if h else 0), timeout=30).read()


def conn(sub):
    t = cdp.pick(sub)
    w = websocket.create_connection(t["webSocketDebuggerUrl"], timeout=T)
    w.settimeout(T)
    return t, w


def send(w, i, m, p=None):
    w.send(json.dumps({"id": i, "method": m, "params": p or {}}))


def wait_id(w, i):
    while True:
        m = json.loads(w.recv())
        if m.get("id") == i:
            return m


def wait_ev(w, name, budget):
    end = time.time() + budget
    while time.time() < end:
        w.settimeout(max(1, end - time.time()))
        try:
            m = json.loads(w.recv())
        except Exception:
            return None
        if m.get("method") == name:
            return m
    return None


TAB_WS = {"url": None}


def arm(label, hang, tab_sub, nav_url):
    ctl(hang)
    time.sleep(0.5)
    _, hdr = conn(HEADER)
    if TAB_WS["url"] is None:
        t = cdp.pick(tab_sub); TAB_WS["url"] = t["webSocketDebuggerUrl"]
    tab = websocket.create_connection(TAB_WS["url"], timeout=T); tab.settimeout(T)
    try:
        send(tab, 1, "Page.enable"); wait_id(tab, 1)
        # Fire the balance IPC and do NOT wait for its reply -- the reply itself has to
        # cross the browser process, so waiting would measure the same block twice.
        send(hdr, 10, "Runtime.evaluate",
             {"expression": "window.cefMessage.send('get_balance',[]);1", "returnByValue": True})
        time.sleep(0.5)

        # UI-thread liveness, independent of any page: CDP's /json/list is served by the
        # browser process. 1.3 ms when idle.
        t = time.time()
        try:
            urllib.request.urlopen("http://127.0.0.1:9322/json/list", timeout=240).read()
            jl = time.time() - t
        except Exception:
            jl = float("nan")

        t = time.time(); send(tab, 20, "Page.navigate", {"url": nav_url}); wait_id(tab, 20)
        nav = time.time() - t
        t = time.time(); ev = wait_ev(tab, "Page.loadEventFired", 240)
        load = (time.time() - t) if ev else float("nan")
        t = time.time(); send(tab, 30, "Runtime.evaluate",
                              {"expression": "location.href", "returnByValue": True})
        r = wait_id(tab, 30); ev2 = time.time() - t
        href = r.get("result", {}).get("result", {}).get("value")
        print("%-24s hang=%-5s json_list=%7.3fs  nav_ack=%7.3fs  load=%7.3fs  eval=%7.3fs  -> %s"
              % (label, hang, jl, nav, load, ev2, str(href)[:40]))
        return dict(arm=label, hang=hang, json_list=round(jl, 3), nav_ack=round(nav, 3),
                    load=round(load, 3), eval=round(ev2, 3), href=href)
    finally:
        try: hdr.close()
        except Exception: pass
        try: tab.close()
        except Exception: pass
        ctl(False)
        time.sleep(1.0)


if __name__ == "__main__":
    tab_sub = sys.argv[1] if len(sys.argv) > 1 else "/newtab"
    nav = sys.argv[2] if len(sys.argv) > 2 else "https://example.com/"
    out = [arm("CONTROL-BEFORE", False, tab_sub, nav + "?n=1")]
    time.sleep(2)
    out.append(arm("TEST (balance hung)", True, tab_sub, nav + "?n=2"))
    time.sleep(2)
    out.append(arm("CONTROL-AFTER", False, tab_sub, nav + "?n=3"))
    print(json.dumps(out, indent=1))
