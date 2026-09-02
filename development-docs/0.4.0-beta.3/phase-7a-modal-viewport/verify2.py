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

SCROLL_TO_CONNECT = "(function(){" + CARD + r"""
card.scrollTop = card.scrollHeight;
var bs=card.querySelectorAll('button'), c=null;
for(var i=0;i<bs.length;i++) if(/Connect/.test(bs[i].textContent)) c=bs[i];
var r=c.getBoundingClientRect(), vh=window.innerHeight;
var visH=Math.max(0,Math.min(r.bottom,vh)-Math.max(r.top,0));
return JSON.stringify({scrolledTo:card.scrollTop, maxScroll:card.scrollHeight-card.clientHeight,
  connectVisiblePx:Math.round(visH), connectPct:Math.round(100*visH/r.height),
  connectFullyVisible: r.top>=0&&r.bottom<=vh});
})()"""

TIP = "(function(){" + CARD + r"""
card.scrollTop=0;
var all=card.querySelectorAll('span'), icons=[];
for(var i=0;i<all.length;i++){
  var e=all[i];
  if(e.textContent.trim()==='i' && getComputedStyle(e).borderRadius==='50%') icons.push(e);
}
var res=[], cr=card.getBoundingClientRect();
for(var k=0;k<icons.length;k++){
  var wrap=icons[k].parentElement;
  var before=wrap.querySelectorAll('span').length;
  res.push({idx:k, iconTop:Math.round(icons[k].getBoundingClientRect().top),
            distFromCardTop:Math.round(icons[k].getBoundingClientRect().top-cr.top)});
}
return JSON.stringify({cardTop:Math.round(cr.top),cardBottom:Math.round(cr.bottom),
  iconCount:icons.length, icons:res});
})()"""
n=sys.argv[1]
q=subprocess.run([sys.executable,"mkfixture.py",n],capture_output=True,text=True).stdout.strip()
ev('(function(){window.showNotification(%s);return "1"})()'%json.dumps(q)); time.sleep(1.2)
sp=("(function(){"+CARD+"var s=card.querySelectorAll('span');for(var i=0;i<s.length;i++)"
    "{if(/^Show$/.test(s[i].textContent.trim())){s[i].dispatchEvent(new MouseEvent('click',{bubbles:true}));return '\"ok\"';}}return '\"none\"';})()")
ev(sp); time.sleep(0.6)
print("scroll-to-Connect:", json.dumps(ev(SCROLL_TO_CONNECT), indent=2))
print("tooltip icons:", json.dumps(ev(TIP), indent=2))
