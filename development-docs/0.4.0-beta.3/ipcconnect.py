#!/usr/bin/env python3
"""
`TICKET_connect_on_the_IPC_transport_still_resends_an_empty_body` — reproduction.

THE RED (owner, Windows, 2026-09-19): on the IPC transport (`window.CWI`, the
provider injected into every https dApp page), approving the CONNECT modal
re-sent the site's call with a 0-byte body:
    page    {"error":"... Invalid JSON: EOF while parsing a value at line 1 column 0"}
    wallet  Raw request body (0 bytes):

⛔ SUBJECT: `window.CWI` is injected on **https main frames only**, so the page has
to be a real https origin — an http:// or internal page would exercise a different
path and prove nothing. example.com is used because it is https and, crucially,
`trustLevel: unknown` in the dev wallet, which is what makes the connect modal fire.

⛔ MONEY: the dev wallet balance is 0, so `createAction` fails at coin selection —
AFTER the body is parsed. That is deliberate: it proves the body arrived without
moving any satoshis. A non-zero body plus "insufficient funds" is the GREEN; a
0-byte body plus "Invalid JSON: EOF" is the RED.
"""
import json, sys, time
import p35macprobe as P

SITE = "https://example.com"


def find_target(pred, tries=25, delay=0.6):
    for _ in range(tries):
        for t in P.cdp_targets():
            if pred(t):
                return t
        time.sleep(delay)
    return None


def main():
    pid = P.preflight()
    hdr = P.headers()[0]
    h = P.Conn(hdr["webSocketDebuggerUrl"])
    print("opening", SITE)
    h.send_ipc("tab_create", SITE)
    h.close()

    t = find_target(lambda t: t.get("type") == "page"
                    and t.get("url", "").startswith("https://example.com"))
    if not t:
        sys.exit("REFUSING: example.com tab never appeared")
    page = P.Conn(t["webSocketDebuggerUrl"])
    time.sleep(2)

    provider = page.js("typeof window.CWI")
    print("window.CWI on the page:", provider)
    if provider != "object":
        sys.exit("REFUSING: the CWI provider is not injected — wrong subject, "
                 "this would measure the HTTP path instead")

    # Fire createAction and park the settled promise on window for later reading.
    # ⭐ Deliberately 100 sats: small, and the wallet has 0 balance anyway.
    page.js("""
      window.__hodosResult = undefined;
      window.CWI.createAction({
        description: 'ipc connect body probe',
        outputs: [{ satoshis: 100,
                    lockingScript: '76a914000000000000000000000000000000000000000088ac',
                    outputDescription: 'probe' }]
      }).then(r => window.__hodosResult = {ok:true, value:r})
        .catch(e => window.__hodosResult = {ok:false, error:String(e && e.message || e)});
      'fired'
    """)
    print("createAction fired; waiting for the connect modal…")
    time.sleep(3)

    # The connect modal is the notification overlay.
    modal = find_target(lambda t: "/brc100-auth" in t.get("url", ""), tries=12)
    if not modal:
        print("⚠️ no connect modal appeared")
    else:
        m = P.Conn(modal["webSocketDebuggerUrl"])
        txt = m.js("document.body.innerText.slice(0,200)")
        print("modal text:", (txt or "").replace("\n", " | ")[:160])
        # ⭐ A React element.click() runs the real onClick — a valid instrument here
        # (it is NOT Input.dispatchMouseEvent, which this project bars).
        clicked = m.js("""(function(){
          const b = Array.from(document.querySelectorAll('button'))
            .find(x => /allow|approve|connect/i.test(x.textContent||''));
          if (!b) return 'NO_BUTTON:' + Array.from(document.querySelectorAll('button'))
            .map(x=>x.textContent).join('/');
          b.click(); return 'CLICKED:' + b.textContent;
        })()""")
        print("modal:", clicked)
        m.close()

    for i in range(20):
        time.sleep(1)
        r = page.js("JSON.stringify(window.__hodosResult || null)")
        if r and r != "null":
            print("\npage result:", r[:400])
            break
    else:
        print("\npage result: (still pending after 20 s)")
    page.close()


if __name__ == "__main__":
    main()
