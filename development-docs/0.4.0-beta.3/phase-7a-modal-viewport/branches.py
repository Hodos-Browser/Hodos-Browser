import json, subprocess, sys, os, time, urllib.parse
RIG = os.path.join("..", "phase-3.5-layout-window-scoping", "p35drive.py")
def ev(js):
    o = subprocess.run([sys.executable, RIG, "eval", "brc100-auth", js],
                       capture_output=True, text=True).stdout.strip()
    return json.loads(json.loads(o.splitlines()[-1])["result"]["value"])
CARD = ("var divs=document.querySelectorAll('div'),bd=null;"
        "for(var i=0;i<divs.length;i++){var cs=getComputedStyle(divs[i]);"
        "if(cs.position==='fixed'&&cs.display==='flex'&&parseInt(cs.width)===window.innerWidth){bd=divs[i];break;}}"
        "var card=bd?bd.firstElementChild:null;")
STATE = "(function(){" + CARD + r"""
if(!card) return JSON.stringify({error:'no card'});
var r=card.getBoundingClientRect(), vh=window.innerHeight, cs=getComputedStyle(card);
var bs=card.querySelectorAll('button'), worst=null;
for(var i=0;i<bs.length;i++){var br=bs[i].getBoundingClientRect();
  if(!bs[i].textContent.trim()) continue;
  var v=Math.max(0,Math.min(br.bottom,vh)-Math.max(br.top,0));
  if(worst===null||v<worst.v) worst={v:Math.round(v),h:Math.round(br.height),
    t:bs[i].textContent.replace(/[^\x20-\x7e]/g,'').slice(0,14)};}
return JSON.stringify({cardH:Math.round(r.height), fits:(r.top>=0&&r.bottom<=vh),
  maxH:cs.maxHeight, ovf:cs.overflowY, scrolls:card.scrollHeight>card.clientHeight+1,
  worstButton:worst});
})()"""
FIELDS = ",".join("field_number_%02d" % i for i in range(40))
CASES = [
 ("domain_approval",        "type=domain_approval&domain=greedy.example"),
 ("payment_confirmation",   "type=payment_confirmation&domain=greedy.example&satoshis=500000&cents=250&perTxLimit=100&perSessionLimit=1000&sessionSpent=0&exceededLimit=perTx"),
 ("certificate_disclosure", "type=certificate_disclosure&domain=greedy.example&fields=" + urllib.parse.quote(FIELDS) + "&certType=identity&certifier=" + "02"+"ab"*32),
 ("rate_limit_exceeded",    "type=rate_limit_exceeded&domain=greedy.example&rateLimit=30&maxTxPerSession=100"),
 ("identity_key_reveal",    "type=identity_key_reveal&domain=greedy.example"),
]
for name, q in CASES:
    ev('(function(){window.showNotification(%s);return "1"})()' % json.dumps(q)); time.sleep(1.0)
    d = ev(STATE)
    wb = d.get("worstButton") or {}
    print("%-24s cardH=%-5s fits=%-5s scrolls=%-5s maxH=%-18s leastVisibleBtn=%s (%s/%spx)"
          % (name, d.get("cardH"), d.get("fits"), d.get("scrolls"), d.get("maxH"),
             wb.get("t"), wb.get("v"), wb.get("h")))
