"""Overlay probe battery — read-only diagnostic of why cert 32 won't clean.

Run from project root: python tmp/overlay_probe.py
All probes are HTTP-only against overlay endpoints. No wallet code changes.
"""
import urllib.request, urllib.error, json, base64, sys, time, socket

IDENTITY_KEY = "020b95583e18ac933d89a131f399890098dc1b3d4a8abcdde3eec4a7b191d2521e"
PUBLISH_TXID = "c8a88544a1b3caccf653f20a1ce466fed392195206500475de621317b07939ef"
SPENDING_TXID = "555916718f6e5e32dafc4a393e224cb4f3f28131c5d3d81b1ddf7d21cadddecf"

HOSTS = [
    "https://overlay-us-1.bsvb.tech",
    "https://overlay-eu-1.bsvb.tech",
    "https://overlay-ap-1.bsvb.tech",
    "https://anvil.sendbsv.com",
]

CERTIFIERS = [
    "03daf815fe38f83da0ad83b5bedc520aa488aef5cbb93a93c67a7fe60406cbffe8",
    "02cf6cdf466951d8dfc9e7c9367511d0007ed6fba35ed42d425cc412fd6cfd4a17",
]

SECTION = "=" * 78
SUB = "-" * 78


def section(s):
    print(f"\n{SECTION}\n{s}\n{SECTION}")


def sub(s):
    print(f"\n{SUB}\n{s}\n{SUB}")


def http(method, url, data=None, headers=None, timeout=20):
    """Returns (status, headers_dict, body_text) or (None, None, error_str)."""
    headers = headers or {}
    req = urllib.request.Request(url, data=data, headers=headers, method=method)
    try:
        with urllib.request.urlopen(req, timeout=timeout) as r:
            body = r.read()
            try:
                body_text = body.decode('utf-8', errors='replace')
            except Exception:
                body_text = repr(body[:200])
            return r.status, dict(r.headers), body_text
    except urllib.error.HTTPError as e:
        body = e.read()
        try:
            body_text = body.decode('utf-8', errors='replace')
        except Exception:
            body_text = repr(body[:200])
        return e.code, dict(e.headers), body_text
    except (urllib.error.URLError, socket.timeout) as e:
        return None, None, str(e)


# ════════════════════════════════════════════════════════════════════════════
# PROBE 1: Discovery — what HTTP surfaces do these overlays expose?
# ════════════════════════════════════════════════════════════════════════════
section("PROBE 1 — Endpoint discovery (GET / and common admin paths)")
discovery_paths = ["/", "/info", "/version", "/api", "/api/v1", "/health", "/topics", "/listTopicManagers", "/listLookupServices"]
for host in HOSTS:
    sub(host)
    for path in discovery_paths:
        url = f"{host}{path}"
        status, hdrs, body = http("GET", url, timeout=10)
        body_preview = (body[:200].replace("\n", " ") if body else "")
        server = (hdrs or {}).get("Server") or (hdrs or {}).get("server") or "?"
        powered = (hdrs or {}).get("X-Powered-By") or (hdrs or {}).get("x-powered-by") or "?"
        print(f"  {path:25s} → {status} [Server={server}, X-Powered-By={powered}] {body_preview[:120]}")

# ════════════════════════════════════════════════════════════════════════════
# PROBE 2: Get the BEEF currently in each overlay's storage for our cert.
#          This is what they admitted; what shape is it?
# ════════════════════════════════════════════════════════════════════════════
section("PROBE 2 — Pull current BEEF from each overlay's /lookup")
lookup_body = json.dumps({
    "service": "ls_identity",
    "query": {"identityKey": IDENTITY_KEY, "certifiers": CERTIFIERS}
}).encode()
beefs_by_host = {}
for host in HOSTS[:3]:  # bsvb hosts only return outputs for this identity
    sub(host)
    status, hdrs, body = http("POST", f"{host}/lookup", data=lookup_body,
                              headers={"Content-Type": "application/json"}, timeout=15)
    print(f"  status={status}")
    if status == 200:
        try:
            data = json.loads(body)
            outputs = data.get("outputs", [])
            print(f"  outputs={len(outputs)}")
            for i, o in enumerate(outputs):
                idx = o.get("outputIndex")
                beef = o.get("beef")
                if isinstance(beef, list):
                    beef_bytes = bytes(beef)
                elif isinstance(beef, str):
                    beef_bytes = base64.b64decode(beef)
                else:
                    beef_bytes = b""
                print(f"    [{i}] outputIndex={idx} beef_size={len(beef_bytes)}")
                beefs_by_host[host] = beef_bytes
        except Exception as e:
            print(f"  parse error: {e}")

# ════════════════════════════════════════════════════════════════════════════
# PROBE 3: Resubmit the lookup-returned BEEF (publish BEEF) back as /submit
#          to that SAME overlay. If dedup fires for publish tx (already
#          applied), we'll see the ambiguous shape.
# ════════════════════════════════════════════════════════════════════════════
section("PROBE 3 — Resubmit publish BEEF (from /lookup) back to each overlay")
for host, beef_bytes in beefs_by_host.items():
    sub(host)
    print(f"  Submitting {len(beef_bytes)} bytes (publish BEEF)")
    # Try the documented submit format: raw body + x-topics header
    status, hdrs, body = http(
        "POST", f"{host}/submit",
        data=beef_bytes,
        headers={
            "Content-Type": "application/octet-stream",
            "x-topics": json.dumps(["tm_identity"])
        },
        timeout=30
    )
    print(f"  status={status}  body={body[:300] if body else '<none>'}")

# ════════════════════════════════════════════════════════════════════════════
# PROBE 4: Submit publish BEEF to a NON-EXISTENT topic — see if engine
#          gives us a distinctive error (proves topics work as expected).
# ════════════════════════════════════════════════════════════════════════════
section("PROBE 4 — Submit to non-existent topic to discover error shape")
if beefs_by_host:
    host = list(beefs_by_host.keys())[0]
    beef_bytes = beefs_by_host[host]
    for topic in ["tm_does_not_exist", "tm_test_only", ""]:
        sub(f"{host} with topic={topic!r}")
        status, hdrs, body = http(
            "POST", f"{host}/submit",
            data=beef_bytes,
            headers={
                "Content-Type": "application/octet-stream",
                "x-topics": json.dumps([topic] if topic else [])
            },
            timeout=20
        )
        print(f"  status={status}  body={body[:400] if body else '<none>'}")

# ════════════════════════════════════════════════════════════════════════════
# PROBE 5: Submit malformed BEEF — see overlay's validation error shape.
#          Different from "dupe" shape, helps disambiguate.
# ════════════════════════════════════════════════════════════════════════════
section("PROBE 5 — Submit malformed BEEF to learn 'validation failure' shape")
for host in HOSTS:
    sub(host)
    bad_beef = b"\x01\x00\xbe\xef" + b"\x00" + b"\x00"  # V1 header, 0 BUMPs, 0 txs
    status, hdrs, body = http(
        "POST", f"{host}/submit",
        data=bad_beef,
        headers={
            "Content-Type": "application/octet-stream",
            "x-topics": json.dumps(["tm_identity"])
        },
        timeout=20
    )
    print(f"  bad BEEF: status={status}  body={body[:400] if body else '<none>'}")

    truly_bad = b"NOT_A_BEEF"
    status, hdrs, body = http(
        "POST", f"{host}/submit",
        data=truly_bad,
        headers={
            "Content-Type": "application/octet-stream",
            "x-topics": json.dumps(["tm_identity"])
        },
        timeout=20
    )
    print(f"  garbage:  status={status}  body={body[:400] if body else '<none>'}")

# ════════════════════════════════════════════════════════════════════════════
# PROBE 6: Submit with NO body — see what overlay says about missing BEEF.
# ════════════════════════════════════════════════════════════════════════════
section("PROBE 6 — Submit with no body / empty body")
for host in HOSTS:
    sub(host)
    for label, body_bytes in [("no-body", b""), ("empty-string", b"")]:
        status, hdrs, body = http(
            "POST", f"{host}/submit",
            data=body_bytes,
            headers={
                "Content-Type": "application/octet-stream",
                "x-topics": json.dumps(["tm_identity"])
            },
            timeout=15
        )
        print(f"  {label}: status={status}  body={body[:300] if body else '<none>'}")
        break

# ════════════════════════════════════════════════════════════════════════════
# PROBE 7: Query lookup for the SPENDING tx specifically — see if overlays
#          have indexed the spending tx as a known applied transaction.
# ════════════════════════════════════════════════════════════════════════════
section("PROBE 7 — Various lookup queries to fingerprint overlay state")
queries = [
    {"name": "by spending txid", "query": {"txid": SPENDING_TXID}},
    {"name": "by publish txid", "query": {"txid": PUBLISH_TXID}},
    {"name": "by identityKey only (no certifiers)", "query": {"identityKey": IDENTITY_KEY}},
    {"name": "by serialNumber", "query": {"serialNumber": "IOhX5O7+FYfVQTWVU5XbUBc8qLHnTLILgbDXBXpckKQ="}},
]
for host in HOSTS[:3]:
    sub(host)
    for q in queries:
        body_json = json.dumps({"service": "ls_identity", "query": q["query"]}).encode()
        status, hdrs, body = http(
            "POST", f"{host}/lookup",
            data=body_json,
            headers={"Content-Type": "application/json"},
            timeout=15
        )
        body_preview = body[:200] if body else "<none>"
        outputs_count = "?"
        try:
            outputs_count = len(json.loads(body).get("outputs", []))
        except Exception:
            pass
        print(f"  {q['name']:42s} → status={status}  outputs={outputs_count}  body={body_preview[:150]}")

print(f"\n{SECTION}\nPROBE BATTERY COMPLETE\n{SECTION}\n")
