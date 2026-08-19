def originFromUrl(u):
    p = u.find("://")
    if p == -1: return ""
    hs = p+3
    ps = u.find('/', hs)
    return u[hs:ps] if ps != -1 else u[hs:]

def matches(origin, host):
    if len(origin) < len(host): return False
    if origin[:len(host)] != host: return False
    return len(origin)==len(host) or origin[len(host)]==':'

def IsInternalOrigin(o):
    if o=="": return True
    return matches(o,"127.0.0.1") or matches(o,"localhost")

cases = [
 "about:blank",
 "about:blank?a://127.0.0.1:5137/",
 "about:blank#a://localhost/",
 "data:text/html,a://127.0.0.1:5137/<script>1</script>",
 "data:text/html;base64,a://localhost:1/",
 "http://127.0.0.1:5137@evil.com/",
 "https://user:pass@127.0.0.1:5137@evil.com/",
 "blob:https://evil.com/uuid",
 "https://evil.com/",
 "http://evil.com/",
 "https://evil.com/?x=a://127.0.0.1/",
 "about:srcdoc",
 "opaque-origin.invalid",
 "hodos://evil.com/",
]
for c in cases:
    o = originFromUrl(c)
    print(f"{c!r:60} -> origin={o!r:28} internal={IsInternalOrigin(o)}")
