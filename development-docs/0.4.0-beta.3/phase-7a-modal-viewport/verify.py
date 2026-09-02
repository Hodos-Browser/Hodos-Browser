import json, subprocess, sys, os, time
RIG = os.path.join("..", "phase-3.5-layout-window-scoping", "p35drive.py")
def ev(js):
    o = subprocess.run([sys.executable, RIG, "eval", "brc100-auth", js],
                       capture_output=True, text=True).stdout.strip()
    return json.loads(json.loads(o.splitlines()[-1])["result"]["value"])

CARD = r"""
var divs=document.querySelectorAll('div'),bd=null;
for(var i=0;i<divs.length;i++){var cs=getComputedStyle(divs[i]);
 if(cs.position==='fixed'&&cs.display==='flex'&&parseInt(cs.width)===window.innerWidth){bd=divs[i];break;}}
var card=bd.firstElementChild;
"""

EXPAND = "(function(){" + CARD + r"""
var sp=card.querySelectorAll('span'),hit=null;
for(var i=0;i<sp.length;i++){ if(/^Show$|^Hide$/.test(sp[i].textContent.trim())){hit=sp[i];break;} }
if(!hit) return JSON.stringify({error:'toggle not found'});
hit.dispatchEvent(new MouseEvent('click',{bubbles:true}));
return JSON.stringify({clicked:hit.textContent.trim()});
})()"""

STATE = "(function(){" + CARD + r"""
var r=card.getBoundingClientRect(), vh=window.innerHeight;
var bs=card.querySelectorAll('button'), last=null;
for(var i=0;i<bs.length;i++){ if(/Connect|Allow|OK|Approve/i.test(bs[i].textContent)) last=bs[i]; }
var lb=last?last.getBoundingClientRect():null;
return JSON.stringify({
  cardH:Math.round(r.height), cardTop:Math.round(r.top), cardBottom:Math.round(r.bottom),
  viewportH:vh, fitsViewport: r.top>=0 && r.bottom<=vh,
  cardScrolls: card.scrollHeight > card.clientHeight+1,
  scrollHeight: card.scrollHeight, clientHeight: card.clientHeight,
  connectVisible: lb ? (lb.top>=0 && lb.bottom<=vh) : null,
  partiesRowCount: (function(){var n=0,d=card.querySelectorAll('div');
     for(var i=0;i<d.length;i++) if(/ — with /.test(d[i].textContent)&&d[i].children.length<4) n++; return n;})()
});
})()"""

TOOLTIPS = "(function(){" + CARD + r"""
var out=[], icons=card.querySelectorAll('span');
var cr=card.getBoundingClientRect();
var n=0;
for(var i=0;i<icons.length;i++){
  var el=icons[i];
  if(el.textContent.trim()!=='i') continue;
  if(getComputedStyle(el).borderRadius!=='50%') continue;
  n++;
  var wrap=el.parentElement;
  wrap.dispatchEvent(new MouseEvent('mouseenter',{bubbles:false}));
}
return JSON.stringify({iconsFound:n});
})()"""
n = sys.argv[1] if len(sys.argv)>1 else "20"
q = subprocess.run([sys.executable,"mkfixture.py",n],capture_output=True,text=True).stdout.strip()
ev('(function(){window.showNotification(%s);return "1"})()' % json.dumps(q))
time.sleep(1.2)
print("N=%s COLLAPSED (default):" % n, json.dumps(ev(STATE), indent=2))
print("expand ->", ev(EXPAND))
time.sleep(0.6)
print("N=%s EXPANDED:" % n, json.dumps(ev(STATE), indent=2))
