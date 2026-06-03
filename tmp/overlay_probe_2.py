"""Round 2 probes — based on round 1 findings.

Key discoveries from round 1:
- overlay-eu-1, overlay-ap-1 are overlay-express-examples v2.1.6 (Express stack)
- overlay-us-1 is currently 503ing everything (offline / broken)
- anvil.sendbsv is a different (Go nginx) stack, not overlay-express
- /version and /listTopicManagers work and tell us version info
- Malformed BEEF returns HTTP 400 with clear error
- Ambiguous 200 STEAK is specifically the DEDUP signature (proven via PROBE 3)
"""
import urllib.request, urllib.error, json, base64, socket

IDENTITY_KEY = "020b95583e18ac933d89a131f399890098dc1b3d4a8abcdde3eec4a7b191d2521e"
PUBLISH_TXID = "c8a88544a1b3caccf653f20a1ce466fed392195206500475de621317b07939ef"

# us-1 is currently 503, skip it for now
HOSTS = ["https://overlay-eu-1.bsvb.tech", "https://overlay-ap-1.bsvb.tech"]

def http(method, url, data=None, headers=None, timeout=20):
    headers = headers or {}
    req = urllib.request.Request(url, data=data, headers=headers, method=method)
    try:
        with urllib.request.urlopen(req, timeout=timeout) as r:
            body = r.read().decode('utf-8', errors='replace')
            return r.status, dict(r.headers), body
    except urllib.error.HTTPError as e:
        return e.code, dict(e.headers), e.read().decode('utf-8', errors='replace')
    except (urllib.error.URLError, socket.timeout) as e:
        return None, None, str(e)

print("="*78)
print("PROBE 8 — Get full /listTopicManagers response from eu-1")
print("="*78)
status, hdrs, body = http("GET", "https://overlay-eu-1.bsvb.tech/listTopicManagers")
try:
    data = json.loads(body)
    print(f"\nTopic managers on eu-1: {len(data)}")
    tm_identity_present = False
    for name, meta in data.items():
        marker = "  ★" if name == "tm_identity" else "   "
        if name == "tm_identity":
            tm_identity_present = True
        desc = (meta.get("shortDescription") or "")[:50]
        ver = meta.get("version", "?")
        print(f"{marker} {name:30s} v{ver:8s}  {desc}")
    print(f"\ntm_identity registered: {tm_identity_present}")
except Exception as e:
    print(f"parse error: {e}\nbody: {body[:500]}")

print("\n" + "="*78)
print("PROBE 9 — Probe admin / debug endpoints overlay-express might expose")
print("="*78)
admin_paths = [
    "/lookup/ls_identity",            # lookup service direct
    "/admin",
    "/admin/storage",
    "/admin/applied-transactions",
    "/admin/cleanup",
    "/.well-known",
    "/.well-known/overlay",
    "/metrics",
    "/api/openapi.json",
    "/swagger",
    "/swagger.json",
    "/listAppliedTransactions",
    "/getAdmittedOutputs",
    "/storage",
    "/storage/utxos",
]
for host in HOSTS:
    print(f"\n--- {host} ---")
    for path in admin_paths:
        status, hdrs, body = http("GET", f"{host}{path}", timeout=8)
        if status and status != 404:
            body_short = body[:150].replace("\n", " ") if body else ""
            print(f"  {path:40s} → {status}  {body_short}")

print("\n" + "="*78)
print("PROBE 10 — Check /health for state hints (counts of stored UTXOs etc.)")
print("="*78)
for host in HOSTS:
    print(f"\n--- {host} ---")
    status, hdrs, body = http("GET", f"{host}/health", timeout=10)
    print(f"status={status}")
    print(f"body:\n{body[:2000]}")

print("\n" + "="*78)
print("PROBE 11 — Check overlay-us-1 in case it recovered")
print("="*78)
for path in ["/health", "/version", "/"]:
    status, hdrs, body = http("GET", f"https://overlay-us-1.bsvb.tech{path}", timeout=10)
    print(f"{path}: status={status} body={body[:100]}")

print("\n" + "="*78)
print("PROBE 12 — Re-submit publish BEEF to ap-1 (which has dedup hit + no UTXO).")
print("           Look at returned STEAK shape vs eu-1's (which has dedup hit + UTXO present).")
print("="*78)
# Get the BEEF from eu-1 first (us-1 is down, ap-1 returns 0 outputs)
lookup_body = json.dumps({
    "service": "ls_identity",
    "query": {"identityKey": IDENTITY_KEY, "certifiers": [
        "02cf6cdf466951d8dfc9e7c9367511d0007ed6fba35ed42d425cc412fd6cfd4a17"
    ]}
}).encode()
status, hdrs, body = http("POST", "https://overlay-eu-1.bsvb.tech/lookup",
                          data=lookup_body, headers={"Content-Type": "application/json"})
beef_bytes = None
if status == 200:
    data = json.loads(body)
    outputs = data.get("outputs", [])
    if outputs:
        beef_raw = outputs[0].get("beef")
        beef_bytes = bytes(beef_raw) if isinstance(beef_raw, list) else base64.b64decode(beef_raw)
        print(f"Got publish BEEF: {len(beef_bytes)} bytes")

if beef_bytes:
    for host in HOSTS:
        print(f"\n--- {host} ---")
        for label, hdrs_extra in [
            ("standard", {"Content-Type": "application/octet-stream", "x-topics": json.dumps(["tm_identity"])}),
        ]:
            status, response_hdrs, body = http(
                "POST", f"{host}/submit",
                data=beef_bytes,
                headers=hdrs_extra,
                timeout=30
            )
            print(f"  {label}: status={status}  body={body[:300]}")
            # Look for any helpful headers
            for h in ["x-error", "x-request-id", "x-trace-id", "x-debug", "server", "x-overlay"]:
                v = response_hdrs.get(h) if response_hdrs else None
                if v:
                    print(f"    header[{h}] = {v}")

print("\n" + "="*78)
print("PROBE 13 — Submit the publish BEEF with WRONG topic to confirm topic routing.")
print("           tm_protomap is a known topic; should reject our identity BEEF cleanly.")
print("="*78)
if beef_bytes:
    for host in HOSTS:
        print(f"\n--- {host} ---")
        for topic in ["tm_protomap", "tm_identity", "tm_certmap"]:
            status, hdrs, body = http(
                "POST", f"{host}/submit",
                data=beef_bytes,
                headers={
                    "Content-Type": "application/octet-stream",
                    "x-topics": json.dumps([topic])
                },
                timeout=20
            )
            print(f"  topic={topic:15s} status={status}  body={body[:200]}")

print("\n" + "="*78)
print("PROBE COMPLETE")
print("="*78)
