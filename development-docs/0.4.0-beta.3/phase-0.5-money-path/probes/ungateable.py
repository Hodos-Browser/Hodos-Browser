"""Enumerate the STRUCTURALLY UNGATEABLE class: routes whose handler takes no
`HttpRequest`, so it can never read X-Requesting-Domain and can never call
dispatch_payment / dispatch_privacy_perimeter.

Cross-references the routes actually registered in main.rs, so this is the
reachable surface, not every fn in the file.
"""
import io
import re
import os

ROOT = r'C:\Users\archb\Hodos-Browser\rust-wallet\src'
handlers = io.open(os.path.join(ROOT, 'handlers.rs'), encoding='utf-8',
                   errors='replace').read()
main = io.open(os.path.join(ROOT, 'main.rs'), encoding='utf-8',
               errors='replace').read()

# fn name -> takes HttpRequest?
sigs = {}
for m in re.finditer(r'\npub(?:\(crate\))? async fn (\w+)\s*\(', handlers):
    name = m.group(1)
    # signature = from '(' to the matching ') ->' / ') {'
    tail = handlers[m.end():m.end() + 900]
    body_at = tail.find('\n) ')
    if body_at == -1:
        body_at = tail.find('\n)')
    sig = tail[:body_at if body_at != -1 else 400]
    sigs[name] = ('HttpRequest' in sig, sig.strip().replace('\n', ' '))

# routes registered in main.rs: .route("/x", web::post().to(handlers::fn))
routes = []
for m in re.finditer(r'"(/[^"]*)"\s*,\s*web::(get|post|delete|put)\(\)'
                     r'\s*\.to\(handlers::(\w+)\)', main):
    routes.append((m.group(1), m.group(2).upper(), m.group(3)))
for m in re.finditer(r'web::resource\("(/[^"]*)"\)[\s\S]{0,200}?'
                     r'web::(get|post|delete|put)\(\)\.to\(handlers::(\w+)\)',
                     main):
    routes.append((m.group(1), m.group(2).upper(), m.group(3)))

seen = set()
gated, ungated = [], []
for path, method, fn in routes:
    key = (path, method)
    if key in seen:
        continue
    seen.add(key)
    has, sig = sigs.get(fn, (None, '<not in handlers.rs>'))
    (gated if has else ungated).append((path, method, fn, sig))

print('ROUTES REGISTERED: %d   with HttpRequest: %d   WITHOUT: %d'
      % (len(seen), len(gated), len(ungated)))
print()
print('=== STRUCTURALLY UNGATEABLE (no HttpRequest parameter) ===')
for path, method, fn, sig in sorted(ungated):
    print('%-6s %-42s %s' % (method, path, fn))
