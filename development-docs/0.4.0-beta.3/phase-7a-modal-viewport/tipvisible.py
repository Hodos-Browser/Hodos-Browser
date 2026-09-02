import json, subprocess, sys, os, time
RIG = os.path.join("..", "phase-3.5-layout-window-scoping", "p35drive.py")
def ev(js):
    o = subprocess.run([sys.executable, RIG, "eval", "brc100-auth", js],
                       capture_output=True, text=True).stdout.strip()
    return json.loads(json.loads(o.splitlines()[-1])["result"]["value"])
CARD = ("var divs=document.querySelectorAll('div'),bd=null;"
        "for(var i=0;i<divs.length;i++){var cs=getComputedStyle(divs[i]);"
        "if(cs.position==='fixed'&&cs.display==='flex'&&parseInt(cs.width)===window.innerWidth){bd=divs[i];break;}}"
        "var card=bd.firstElementChild;")
# hover ONE icon at a time; only icons whose own rect is inside the card's visible box
PROBE = "(function(idx){" + CARD + r"""
var all=card.querySelectorAll('span'), icons=[];
for(var i=0;i<all.length;i++){var e=all[i];
  if(e.textContent.trim()==='i'&&getComputedStyle(e).borderRadius==='50%') icons.push(e);}
var cr=card.getBoundingClientRect();
if(idx>=icons.length) return JSON.stringify({done:true});
var ic=icons[idx], ir=ic.getBoundingClientRect();
var iconVisible = ir.top>=cr.top && ir.bottom<=cr.bottom;
// clear any previous hover
for(var k=0;k<icons.length;k++)
  icons[k].parentElement.dispatchEvent(new MouseEvent('mouseout',{bubbles:true,relatedTarget:document.body}));
ic.parentElement.dispatchEvent(new MouseEvent('mouseover',{bubbles:true,relatedTarget:document.body}));
var tip=null, sp=ic.parentElement.querySelectorAll('span');
for(var k=0;k<sp.length;k++){var s=getComputedStyle(sp[k]);
  if(s.position==='absolute'&&parseInt(s.width)>=200){tip=sp[k];break;}}
if(!tip) return JSON.stringify({idx:idx,iconVisible:iconVisible,tipOpen:false});
var tr=tip.getBoundingClientRect();
return JSON.stringify({idx:idx,iconVisible:iconVisible,tipOpen:true,
  clipped:(tr.top<cr.top||tr.bottom>cr.bottom),
  tipTop:Math.round(tr.top),tipBot:Math.round(tr.bottom),
  cardTop:Math.round(cr.top),cardBot:Math.round(cr.bottom),
  txt:tip.textContent.replace(/[^\x20-\x7e]/g,'').slice(0,34)});
})(IDX)"""
n, scroll = sys.argv[1], sys.argv[2]
q=subprocess.run([sys.executable,"mkfixture.py",n],capture_output=True,text=True).stdout.strip()
ev('(function(){window.showNotification(%s);return "1"})()'%json.dumps(q)); time.sleep(1.2)
sp=("(function(){"+CARD+"var s=card.querySelectorAll('span');for(var i=0;i<s.length;i++)"
    "{if(/^Show$/.test(s[i].textContent.trim())){s[i].dispatchEvent(new MouseEvent('click',{bubbles:true}));return '\"ok\"';}}return '\"none\"';})()")
ev(sp); time.sleep(0.5)
ev("(function(){"+CARD+"card.scrollTop="+scroll+";return '\"s\"'})()"); time.sleep(0.3)
vis=clip=0
for i in range(30):
    r=ev(PROBE.replace("IDX",str(i)))
    if r.get("done"): break
    if not r.get("iconVisible"): continue
    vis+=1
    if r.get("clipped"):
        clip+=1
        print("  CLIPPED  tip=%s..%s card=%s..%s  %s"%(r["tipTop"],r["tipBot"],r["cardTop"],r["cardBot"],r["txt"]))
print("scrollTop=%s  visible icons hovered=%d  CLIPPED=%d"%(scroll,vis,clip))
