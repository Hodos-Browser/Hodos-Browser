#!/usr/bin/env python3
"""
beta.3 Phase 3.5 — CDP driver for the two-window Z-order rows.

WHY IT EXISTS
-------------
Every row in this phase is T3 ("a human with two windows"), and the reason is that
the actions are clicks. But clicking is also what BREAKS the measurement: overlays
hide on focus loss, and touching any other window to run a probe IS focus loss
(MEASUREMENTS.md K8.3). Driving the action from outside the desktop input queue lets
winprobe.ps1 watch continuously while the action happens.

⛔ SUBJECT DISCIPLINE. The header and ~14 overlays are separate CEF browsers that CDP
   reports ALL as type:"page" (this project has already faked a bug by driving the
   wrong one). Targets are named by URL, never by index, and every command prints the
   URL it actually talked to. Dev port 9322 only; 9222 is the installed browser.

⚠️ FIDELITY. `send` reproduces what a React onClick does — it posts the same
   cefMessage IPC from the same browser, so C++ GetOwnerWindow() resolves the same
   window. It does NOT reproduce the activation a real click also causes; use
   `raise` first, or read the Z column from a sample taken before the action.

USAGE
    python p35drive.py list
    python p35drive.py key   <url-substr> <VK> [--ctrl] [--shift]
    python p35drive.py send  <url-substr> <ipc_name> [json_args]
    python p35drive.py eval  <url-substr> "<javascript>"
"""

import argparse
import json
import sys
from urllib.request import urlopen

import websocket

DEV_PORT = 9322


def targets(port=DEV_PORT):
    if port == 9222:
        sys.exit("refusing to attach to 9222 - that is the installed browser")
    return json.loads(urlopen("http://127.0.0.1:%d/json/list" % port, timeout=5).read())


def pick(url_substr, port=DEV_PORT):
    """Exactly one target. A leading '=' means exact URL match (how you address a
    header browser -- its URL is a prefix of every overlay's); a leading '#' means
    match by target id, which is the ONLY way to tell window A's header from window
    B's once a second window exists -- their URLs are identical."""
    all_t = targets(port)
    if url_substr.startswith("#"):
        hits = [t for t in all_t if t.get("id", "") == url_substr[1:]]
    elif url_substr.startswith("="):
        hits = [t for t in all_t if t.get("url", "") == url_substr[1:]]
    else:
        hits = [t for t in all_t if url_substr in t.get("url", "")]
    if not hits:
        sys.exit("no target whose url contains %r" % url_substr)
    if len(hits) > 1:
        sys.exit("ambiguous: %d targets match %r -> %s"
                 % (len(hits), url_substr, [t["url"] for t in hits]))
    return hits[0]


def call(target, method, params, port=DEV_PORT):
    ws = websocket.create_connection(target["webSocketDebuggerUrl"], timeout=10)
    try:
        ws.send(json.dumps({"id": 1, "method": method, "params": params}))
        while True:
            msg = json.loads(ws.recv())
            if msg.get("id") == 1:
                return msg
    finally:
        ws.close()


def main():
    ap = argparse.ArgumentParser()
    sub = ap.add_subparsers(dest="cmd", required=True)
    sub.add_parser("list")
    k = sub.add_parser("key")
    k.add_argument("url"); k.add_argument("vk", type=int)
    k.add_argument("--ctrl", action="store_true"); k.add_argument("--shift", action="store_true")
    s = sub.add_parser("send")
    s.add_argument("url"); s.add_argument("name"); s.add_argument("args", nargs="?", default="[]")
    e = sub.add_parser("eval")
    e.add_argument("url"); e.add_argument("js")
    a = ap.parse_args()

    if a.cmd == "list":
        for t in targets():
            print("%-6s %-34s %s" % (t["type"], t["id"], t["url"]))
        return

    t = pick(a.url)
    print("target: %s" % t["url"])

    if a.cmd == "key":
        mods = (2 if a.ctrl else 0) | (8 if a.shift else 0)
        for typ in ("rawKeyDown", "keyUp"):
            r = call(t, "Input.dispatchKeyEvent", {
                "type": typ, "modifiers": mods,
                "windowsVirtualKeyCode": a.vk, "nativeVirtualKeyCode": a.vk,
            })
            print("%s -> %s" % (typ, json.dumps(r.get("result", r.get("error")))))
    elif a.cmd == "send":
        js = "window.cefMessage.send(%s, %s)" % (json.dumps(a.name), a.args)
        r = call(t, "Runtime.evaluate", {"expression": js, "returnByValue": True})
        print(json.dumps(r.get("result", r.get("error"))))
    elif a.cmd == "eval":
        r = call(t, "Runtime.evaluate", {"expression": a.js, "returnByValue": True})
        print(json.dumps(r.get("result", r.get("error"))))


if __name__ == "__main__":
    main()
