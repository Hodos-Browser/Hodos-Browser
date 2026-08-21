"""P0.5 Task C — the Rust-side half of the evidence table, re-run whole.

⛔ The WHOLE table, not the failing rows. That discipline is the only reason the
fourth :5137 gate was ever found.

SAFETY
  * Every fund-mover uses the valid-prefix / invalid-checksum probe address, so
    anything that slips a gate dies at build. Address validation runs BEFORE the
    gate on /transaction/send, so a format-invalid address would measure nothing.
  * teragun.com is APPROVED with per_tx_limit_cents = 13, so an over-cap call is
    unmistakable. An UNAPPROVED domain makes domain_trust_mw answer first and the
    row goes green with the feature disabled.
  * peerpay: 🚨 peerpay_send broadcasts to ANY well-formed identity key with no
    reachability check. The amount here is deliberately OVER CAP so the engine
    returns 202 and the handler never runs. NEVER approve one of these prompts.
"""
import json
import urllib.request

WALLET = 'http://127.0.0.1:31401'
DOMAIN = 'teragun.com'
SATS = 1_000_000                 # 0.01 BSV
BAD_ADDR = '1BvBMSEYstWetqRdfSf1fQ8Y7AE59Xc8aW'

price = json.load(urllib.request.urlopen(WALLET + '/wallet/bsv-price', timeout=15))
PRICE = price['priceUsd']
CENTS = int((SATS / 100_000_000.0) * PRICE * 100)
print('price $%s/BSV; %d sats prices at %d cents; teragun cap = 13 cents'
      % (PRICE, SATS, CENTS))
assert CENTS > 13, 'probe is NOT over cap — the rows below would not discriminate'


def call(path, body, domain=DOMAIN, pay=True, method='POST'):
    h = {'Content-Type': 'application/json'}
    if domain:
        h['X-Requesting-Domain'] = domain
    if pay:
        h.update({'X-Payment-Satoshis': str(SATS), 'X-Payment-Cents': str(CENTS),
                  'X-Bsv-Price-Available': '1', 'X-Browser-Id': '1'})
    req = urllib.request.Request(WALLET + path, data=json.dumps(body).encode(),
                                 headers=h, method=method)
    try:
        with urllib.request.urlopen(req, timeout=90) as r:
            return r.status, r.read().decode()
    except urllib.error.HTTPError as e:
        return e.code, e.read().decode()
    except Exception as e:
        return 'ERR', repr(e)


def row(rid, claim, path, body, expect, **kw):
    status, text = call(path, body, **kw)
    try:
        j = json.loads(text)
        summary = j.get('engineReason') or j.get('error') or j.get('status') or text[:90]
    except Exception:
        summary = text[:90]
    ok = expect(status, text)
    print('%-10s %-6s %-46s HTTP %-4s  %s'
          % (rid, 'PASS' if ok else '**FAIL**', claim, status, str(summary)[:80]))
    return ok


OUT = {'outputs': [{'satoshis': SATS, 'address': BAD_ADDR}],
       'description': 'P0.5 table re-run'}


def pending(s, t):
    return s == 202 and 'payment_confirmation' in t


def notpay(s, t):
    return s == 400 and 'checksum' in t


results = []
print()
print('--- gated fund-movers, APPROVED domain, over cap -> 202 payment_confirmation ---')
results.append(row('R2', '/transaction/send external over-cap', '/transaction/send',
                   {'toAddress': BAD_ADDR, 'amount': SATS}, pending))
results.append(row('X6', '/processAction external over-cap', '/processAction',
                   OUT, pending))
results.append(row('-', '/createAction external over-cap', '/createAction',
                   OUT, pending))
results.append(row('X5a', '/wallet/peerpay/send external', '/wallet/peerpay/send',
                   {'recipient_identity_key': '02' + '11' * 32,
                    'amount_satoshis': SATS}, pending))
results.append(row('X5b', '/wallet/paymail/send external', '/wallet/paymail/send',
                   {'paymail': 'nobody@example.invalid',
                    'amount_satoshis': SATS}, pending))

print()
print('--- R3: external sendMax -> FORCED prompt (not a cap evaluation) ---')
results.append(row('R3', '/transaction/send sendMax external', '/transaction/send',
                   {'toAddress': BAD_ADDR, 'sendMax': True}, pending, pay=False))

print()
print('--- R1 half: header-free INTERNAL caller must be UNCHANGED (no gate) ---')
results.append(row('R1*', '/transaction/send internal', '/transaction/send',
                   {'toAddress': BAD_ADDR, 'amount': SATS}, notpay,
                   domain=None, pay=False))
results.append(row('R1*', '/processAction internal', '/processAction', OUT,
                   notpay, domain=None, pay=False))

print()
print('--- §4k / panel #2 1.3+1.4: permission surface is first-party only ---')
results.append(row('1.3', 'POST /domain/%70ermissions (encoded)',
                   '/domain/%70ermissions', {'domain': 'evil.example',
                                             'perTxLimitCents': 999999},
                   lambda s, t: s == 403 and 'first_party_only' in t, pay=False))
results.append(row('1.4', 'POST /wallet/session/close', '/wallet/session/close', {},
                   lambda s, t: s == 403, pay=False))
results.append(row('T2.2', 'POST /wallet/reveal-mnemonic', '/wallet/reveal-mnemonic',
                   {'pin': '000000'}, lambda s, t: s == 403, pay=False))
results.append(row('T2.3', 'POST /wallet/settings', '/wallet/settings',
                   {'defaultPerTxLimitCents': 999999},
                   lambda s, t: s == 403, pay=False))

print()
print('--- unapproved origin: domain_trust_mw must answer FIRST (the false-green trap) ---')
results.append(row('ctl', 'unapproved -> domain_approval', '/processAction', OUT,
                   lambda s, t: s == 202 and 'domain_approval' in t,
                   domain='never-approved-probe.invalid'))

print()
print('%d/%d rows PASS' % (sum(1 for r in results if r), len(results)))
