"""Minimal CDP driver for the DEV browser on 9322.

Target selection is BY URL and refuses on ambiguity, and every evaluate()
returns location.href alongside the value so the subject is proved, not
asserted. (P0.5 harness rule: 11+ targets all report type:"page"; ten of
them are overlays on :5137.)
"""
import json
import sys
import urllib.request

import websocket

PORT = 9322


def targets():
    with urllib.request.urlopen('http://127.0.0.1:%d/json/list' % PORT,
                                timeout=10) as r:
        return json.load(r)


def pick(url_contains):
    hits = [t for t in targets()
            if t['type'] == 'page' and url_contains in t['url']]
    if len(hits) != 1:
        raise SystemExit('AMBIGUOUS/NONE: %d targets match %r -> %s'
                         % (len(hits), url_contains,
                            [t['url'] for t in hits]))
    return hits[0]


class Session(object):
    def __init__(self, target):
        self.ws = websocket.create_connection(target['webSocketDebuggerUrl'],
                                              timeout=90)
        self.n = 0

    def send(self, method, **params):
        self.n += 1
        self.ws.send(json.dumps({'id': self.n, 'method': method,
                                 'params': params}))
        while True:
            msg = json.loads(self.ws.recv())
            if msg.get('id') == self.n:
                return msg

    def evaluate(self, expr, await_promise=False):
        return self.send('Runtime.evaluate', expression=expr,
                         returnByValue=True, awaitPromise=await_promise,
                         userGesture=True)

    def close(self):
        self.ws.close()


def main():
    cmd = sys.argv[1]
    if cmd == 'list':
        for t in targets():
            print(t['id'], '|', t['type'], '|', t['url'][:100])
        return
    match = sys.argv[2]
    s = Session(pick(match))
    try:
        if cmd == 'nav':
            s.send('Page.enable')
            print(json.dumps(s.send('Page.navigate', url=sys.argv[3])))
        elif cmd == 'eval':
            expr = open(sys.argv[3]).read() if len(sys.argv) > 3 else \
                sys.stdin.read()
            print(json.dumps(s.evaluate(expr), indent=1)[:4000])
        elif cmd == 'evalp':
            expr = open(sys.argv[3]).read()
            print(json.dumps(s.evaluate(expr, await_promise=True),
                             indent=1)[:4000])
    finally:
        s.close()


main()
