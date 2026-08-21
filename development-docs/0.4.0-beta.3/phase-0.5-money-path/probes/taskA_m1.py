"""P0.5 Task A / M1 — is the two-phase action lifecycle gated?

CONTROL : POST /createAction   (known-gated, P0.5's own fix)
SUBJECT : POST /processAction  (create+sign+broadcast in one call)

Identical body, identical headers, identical approved domain, same second.
If /processAction is gated the two must agree.

SAFETY: `address` is valid-base58 / valid-length / mainnet version byte 0x00
with a DELIBERATELY BROKEN checksum. Address->script conversion happens
INSIDE create_action_internal, i.e. AFTER the point where the gate would
have fired, so a bypass dies at "Address checksum mismatch" and no
transaction can be built or broadcast.
"""
import json
import urllib.request

WALLET = 'http://127.0.0.1:31401'
DOMAIN = 'teragun.com'          # approved, per_tx_limit_cents = 13
SATS = 1_000_000                # 0.01 BSV
PRICE = 17.555                  # USD/BSV, read from /wallet/price this run
CENTS = int((SATS / 100_000_000.0) * PRICE * 100)   # 17
BAD_ADDR = '1BvBMSEYstWetqTFn5Au4m4GFg7xJaNVN2'[:0] + '1BvBMSEYstWetqRdfSf1fQ8Y7AE59Xc8aW'

BODY = {
    'outputs': [{'satoshis': SATS, 'address': BAD_ADDR,
                 'outputDescription': 'p05 taskA probe'}],
    'description': 'p05 taskA probe',
}

HEADERS = {
    'Content-Type': 'application/json',
    'X-Requesting-Domain': DOMAIN,
    'X-Payment-Satoshis': str(SATS),
    'X-Payment-Cents': str(CENTS),
    'X-Bsv-Price-Available': '1',
    'X-Browser-Id': '1',
}


def post(path):
    data = json.dumps(BODY).encode()
    req = urllib.request.Request(WALLET + path, data=data, headers=HEADERS,
                                 method='POST')
    try:
        with urllib.request.urlopen(req, timeout=60) as r:
            return r.status, r.read().decode()[:600]
    except urllib.error.HTTPError as e:
        return e.code, e.read().decode()[:600]
    except Exception as e:
        return 'ERR', repr(e)


print('cap for %s = 13 cents; this call prices at %d cents (%d sats @ $%s)'
      % (DOMAIN, CENTS, SATS, PRICE))
print('address %s -> checksum deliberately broken' % BAD_ADDR)
print()
for path in ('/createAction', '/processAction'):
    status, body = post(path)
    print('=== %-16s -> HTTP %s' % (path, status))
    print(body)
    print()
