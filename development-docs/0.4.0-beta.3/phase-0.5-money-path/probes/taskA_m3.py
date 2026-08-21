"""P0.5 Task A / M3 — the remaining lifecycle questions, measured.

(a) Does `options.noSend` change what the phase-1 gate decides?
(b) Does /listActions disclose the reference numbers signAction accepts?
(c) Does /signAction consult ANYTHING but the reference's existence?
    Measured with a reference that CANNOT exist, so nothing is signed.
"""
import json
import urllib.request

WALLET = 'http://127.0.0.1:31401'
DOMAIN = 'teragun.com'
SATS = 1_000_000
BAD_ADDR = '1BvBMSEYstWetqRdfSf1fQ8Y7AE59Xc8aW'

H = {
    'Content-Type': 'application/json',
    'X-Requesting-Domain': DOMAIN,
    'X-Payment-Satoshis': str(SATS),
    'X-Payment-Cents': '17',
    'X-Bsv-Price-Available': '1',
    'X-Browser-Id': '1',
}


def post(path, body, headers=None):
    req = urllib.request.Request(WALLET + path,
                                 data=json.dumps(body).encode(),
                                 headers=headers if headers is not None else H,
                                 method='POST')
    try:
        with urllib.request.urlopen(req, timeout=60) as r:
            return r.status, r.read().decode()
    except urllib.error.HTTPError as e:
        return e.code, e.read().decode()
    except Exception as e:
        return 'ERR', repr(e)


out = {'outputs': [{'satoshis': SATS, 'address': BAD_ADDR,
                    'outputDescription': 'taskA m3'}],
       'description': 'taskA m3'}

print('--- (a) noSend:true, same over-cap amount, /createAction ---')
b = dict(out)
b['options'] = {'noSend': True}
s, t = post('/createAction', b)
print(s, t[:300])
print()

print('--- (b) /listActions from the same approved dApp origin ---')
s, t = post('/listActions', {'labels': [], 'limit': 5,
                             'includeLabels': False})
print(s, t[:900])
print()

print('--- (c) /signAction with a reference that cannot exist ---')
s, t = post('/signAction',
            {'reference': 'action-00000000-0000-4000-8000-000000000000',
             'options': {'noSend': False}})
print(s, t[:300])
