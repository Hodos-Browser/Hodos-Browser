#!/usr/bin/env python3
"""
Row 2 of the ticket's evidence: the FIRST post-connect call, over the per-tx cap,
must open the PAYMENT modal — not be delivered to the page as a raw 202.

⛔ This is the row that says WHY "just restore the body" was the wrong fix. That
version would have left the resume on resumeInternalResponse, which treats Rust's
202 ("a prompt is required") as a 2xx and hands it to the page as SUCCESS — a gold
pill for a payment that never happened. Here the 202 must become a modal.

⛔ MONEY: dev balance is 0 and the run ends in DENY, so nothing can be spent. The
payment gate runs in Rust before coin selection, so the modal appears regardless.
"""
import json, sys, time
import p35macprobe as P

SITE = "https://example.com"
OVER_CAP_SATS = 200_000_000   # ~$35 at the dev price; default per-tx cap is $10


def find_target(pred, tries=25, delay=0.6):
    for _ in range(tries):
        for t in P.cdp_targets():
            if pred(t):
                return t
        time.sleep(delay)
    return None


pid = P.preflight()
h = P.Conn(P.headers()[0]["webSocketDebuggerUrl"])
h.send_ipc("tab_create", SITE)
h.close()
t = find_target(lambda t: t.get("type") == "page"
                and t.get("url", "").startswith("https://example.com"))
page = P.Conn(t["webSocketDebuggerUrl"])
time.sleep(2)
assert page.js("typeof window.CWI") == "object", "CWI not injected"

page.js("""
  window.__r = undefined;
  window.CWI.createAction({
    description: 'over-cap probe',
    outputs: [{ satoshis: %d,
                lockingScript: '76a914000000000000000000000000000000000000000088ac',
                outputDescription: 'probe' }]
  }).then(r => window.__r = {ok:true, value:r})
    .catch(e => window.__r = {ok:false, error:String(e && e.message || e)});
  'fired'
""" % OVER_CAP_SATS)
time.sleep(3)


def modal_text():
    m = find_target(lambda t: "/brc100-auth" in t.get("url", ""), tries=10)
    if not m:
        return None, None
    c = P.Conn(m["webSocketDebuggerUrl"])
    return c, (c.js("document.body.innerText.slice(0,240)") or "").replace("\n", " | ")


c, txt = modal_text()
print("modal 1 (expect CONNECT):", (txt or "(none)")[:150])
if c:
    print("  ->", c.js("""(function(){const b=Array.from(document.querySelectorAll('button'))
      .find(x=>/allow|approve|connect/i.test(x.textContent||''));
      if(!b) return 'NO_BUTTON'; b.click(); return 'CLICKED:'+b.textContent;})()"""))
    c.close()

# THE ROW: after the connect is allowed, the over-cap call must raise a PAYMENT prompt.
time.sleep(5)
c2, txt2 = modal_text()
print("modal 2 (expect PAYMENT):", (txt2 or "(none)")[:200])
r = page.js("JSON.stringify(window.__r || null)")
print("page result while modal 2 is up:", (r or "null")[:220])

if c2:
    print("  -> denying:", c2.js("""(function(){const b=Array.from(document.querySelectorAll('button'))
      .find(x=>/deny|reject|cancel|don.t allow/i.test(x.textContent||''));
      if(!b) return 'NO_BUTTON:'+Array.from(document.querySelectorAll('button')).map(x=>x.textContent).join('/');
      b.click(); return 'CLICKED:'+b.textContent;})()"""))
    c2.close()
time.sleep(4)
print("page result final:", (page.js("JSON.stringify(window.__r || null)") or "null")[:220])
page.close()
