"""Phase 2a experiment stub for 127.0.0.1:31401.

Answers every wallet endpoint quickly EXCEPT:
  * /wallet/balance   -- hangs while  GET /__ctl?hang=1  is in effect (releasable)
  * /transaction/send -- always answers slowly (?slow=SECONDS, default 8)

Responses are verbatim captures from the real dev wallet (2026-08-26).
Threaded, so a hung /wallet/balance blocks only its own connection -- the stub is NOT
the thing under test; the browser's UI thread is.
"""
import json
import os
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import urlparse, parse_qs

HANG = threading.Event()
RELEASE = threading.Event()
HANG_SECS = 90.0

LOG = open(os.path.join(os.path.dirname(os.path.abspath(__file__)),
                        "stub_wallet_access.log"), "a", buffering=1)

CANNED = {
    "/health":           {"status": "ok", "version": "0.1.0-rust", "backend": "rust-wallet"},
    "/getVersion":       {"version": "HodosWallet-Rust v0.0.1",
                          "capabilities": ["getVersion", "getPublicKey", "createSignature",
                                           "isAuthenticated", "createAction", "signAction",
                                           "processAction"],
                          "brc100": True, "timestamp": "2026-08-26T15:52:06.236907400Z"},
    "/wallet/status":    {"exists": True, "locked": False},
    "/wallet/balance":   {"balance": 28332055, "bsvPrice": 16.875},
    "/transaction/send": {"status": "success", "txid": "deadbeef" * 8},
}


class H(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def log_message(self, *a):
        pass

    def _emit(self, obj, code=200):
        body = json.dumps(obj).encode()
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.send_header("Access-Control-Allow-Origin", "*")
        self.end_headers()
        self.wfile.write(body)

    def _handle(self):
        parsed = urlparse(self.path)
        path = parsed.path
        t = time.strftime("%H:%M:%S")

        if path == "/__ctl":
            want = parse_qs(parsed.query).get("hang", ["0"])[0] == "1"
            if want:
                RELEASE.clear()
                HANG.set()
            else:
                HANG.clear()
                RELEASE.set()          # also frees requests already sleeping
            LOG.write("%s CTL hang=%s\n" % (t, want))
            return self._emit({"hang": want})

        # P2a-A4: a send that legitimately takes longer than the DEFAULT budget but less
        # than the broadcast budget. If A1 had capped every endpoint globally this would
        # fail; succeeding proves the generous broadcast budget is real. No money moves --
        # the stub just answers slowly.
        if path == "/transaction/send":
            delay = float(parse_qs(parsed.query).get("slow", [os.environ.get("STUB_SEND_DELAY", "8")])[0])
            LOG.write("%s SLOW-SEND begin %.1fs\n" % (t, delay))
            time.sleep(delay)
            LOG.write("%s SLOW-SEND end\n" % time.strftime("%H:%M:%S"))

        if path == "/wallet/balance" and HANG.is_set():
            LOG.write("%s HANG-BEGIN %s\n" % (t, path))
            RELEASE.wait(HANG_SECS)
            LOG.write("%s HANG-END   %s\n" % (time.strftime("%H:%M:%S"), path))

        LOG.write("%s %s %s\n" % (t, self.command, path))
        self._emit(CANNED.get(path, {}))

    do_GET = do_POST = do_PUT = do_DELETE = _handle

    def do_OPTIONS(self):
        self.send_response(204)
        self.send_header("Access-Control-Allow-Origin", "*")
        self.send_header("Access-Control-Allow-Headers", "*")
        self.send_header("Content-Length", "0")
        self.end_headers()


if __name__ == "__main__":
    ThreadingHTTPServer(("127.0.0.1", 31401), H).serve_forever()
