"""P0.8 spike: which real sites serve a BRC-73 grouped-permission manifest, and where?

Probes each candidate at both locations:
  /manifest.json                      <- BRC-73's stated location (W3C web-app manifest)
  /.well-known/wallet-manifest.json   <- the path Hodos currently fetches (our own invention)

Reports: HTTP status, whether the body is JSON, and which permission namespaces/categories
are present. Read-only GETs of public files.
"""
import json
import urllib.request
import urllib.error
import ssl

CANDIDATES = [
    "bitgenius.net",
    "now.bsvblockchain.tech",
    "projectbabbage.com",
    "socialcert.net",
    "toolbelt.babbage.systems",
    "coolcert.babbage.systems",
    "peerpay.babbage.systems",
    "messagebox.babbage.systems",
    "1sat.market",
    "metanetapps.com",
]

PATHS = ["/manifest.json", "/.well-known/wallet-manifest.json"]
CATEGORIES = ["protocolPermissions", "spendingAuthorization", "basketAccess", "certificateAccess"]

ctx = ssl.create_default_context()


def fetch(url):
    req = urllib.request.Request(url, headers={"User-Agent": "HodosBrowser-P0.8-spike"})
    try:
        with urllib.request.urlopen(req, timeout=10, context=ctx) as r:
            return r.status, r.read(65536)
    except urllib.error.HTTPError as e:
        return e.code, b""
    except Exception as e:
        return None, str(e).encode()[:60]


def describe(body):
    """Return a short description of the grouped-permission content, if any."""
    try:
        d = json.loads(body)
    except Exception:
        return "not-JSON"
    if not isinstance(d, dict):
        return "not-an-object"
    out = []
    for ns in ("metanet", "babbage"):
        blk = d.get(ns)
        if not isinstance(blk, dict):
            continue
        gp = blk.get("groupPermissions")
        if not isinstance(gp, dict):
            out.append(f"{ns}(no groupPermissions)")
            continue
        parts = []
        for c in CATEGORIES:
            v = gp.get(c)
            if isinstance(v, list):
                parts.append(f"{c}={len(v)}")
            elif isinstance(v, dict):
                parts.append(f"{c}=obj")
        out.append(f"{ns}.groupPermissions[{', '.join(parts) if parts else 'empty'}]")
    # Does it use the shape Hodos currently parses?
    if isinstance(d.get("permissions"), dict):
        out.append("HODOS-legacy top-level 'permissions'")
    if not out:
        keys = ", ".join(list(d.keys())[:6])
        return f"JSON, no perms (keys: {keys})"
    return " | ".join(out)


print(f"{'domain':<28} {'path':<34} {'http':<6} content")
print("-" * 120)
summary = {}
for dom in CANDIDATES:
    for p in PATHS:
        url = f"https://{dom}{p}"
        status, body = fetch(url)
        desc = describe(body) if status == 200 else (body.decode(errors="replace") if status is None else "")
        print(f"{dom:<28} {p:<34} {str(status):<6} {desc}")
        if status == 200 and "groupPermissions[" in desc:
            summary.setdefault(dom, []).append(p)

print("\n--- sites serving a BRC-73 grouped-permission manifest ---")
if not summary:
    print("none")
for dom, paths in summary.items():
    print(f"  {dom}: {', '.join(paths)}")
