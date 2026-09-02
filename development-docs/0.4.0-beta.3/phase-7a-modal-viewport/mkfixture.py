import json, urllib.parse, sys
n = int(sys.argv[1]) if len(sys.argv) > 1 else 8
CP = "02" + "ab" * 32
m = {
  "name": "Greedy Test dApp", "description": "fixture", "iconUrl": "",
  "expiresAt": 0, "version": "1.0", "sourceNamespace": "metanet",
  "protocols": [
    {"securityLevel": 2, "name": "proto-%02d-3241645161d8" % i, "keyId": "*",
     "purpose": "Use protocol number %d for something that needs a sentence" % i,
     "counterparty": CP}
    for i in range(n)],
  "baskets": [{"name": "basket-%02d" % i, "access": "read_write",
               "purpose": "Manage basket %d" % i} for i in range(max(1, n // 2))],
  "certificates": [{"type": "t", "fields": ["email", "name"],
                    "purpose": "Read certificate fields %d" % i,
                    "verifierPublicKey": CP} for i in range(max(1, n // 4))],
  "spending": {"perTransactionUsd": 0, "perSessionUsd": 0, "monthlySatoshis": 10000},
  "counterparties": [{"type": "specific", "counterparty": CP,
                      "purpose": "Talk to peer %d" % i} for i in range(max(1, n // 4))],
}
print("type=manifest_connect_bundle&domain=greedy.example&manifest="
      + urllib.parse.quote(json.dumps(m), safe=""))
