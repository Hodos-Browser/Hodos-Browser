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
JS = "(function(){" + CARD + r"""
var all=card.querySelectorAll('span'), icons=[];
for(var i=0;i<all.length;i++){var e=all[i];
  if(e.textContent.trim()==='i'&&getComputedStyle(e).borderRadius==='50%') icons.push(e);}
var cr=card.getBoundingClientRect(), pad=28, out=[];
var clipTop=cr.top+0, clipBottom=cr.bottom;
for(var k=0;k<icons.length;k++){
  var wrap=icons[k].parentElement;
  wrap.dispatchEvent(new MouseEvent('mouseover',{bubbles:true,relatedTarget:document.body}));
}
return JSON.stringify({staged:icons.length});
})()"""
MEASURE = "(function(){" + CARD + r"""
var cr=card.getBoundingClientRect(), out=[];
var tips=card.querySelectorAll('span');
for(var i=0;i<tips.length;i++){
  var s=getComputedStyle(tips[i]);
  if(s.position!=='absolute') continue;
  if(parseInt(s.width)<200) continue;
  var r=tips[i].getBoundingClientRect();
  if(r.height===0) continue;
  out.push({top:Math.round(r.top),bottom:Math.round(r.bottom),
    clippedAbove: r.top < cr.top, clippedBelow: r.bottom > cr.bottom,
    txt:tips[i].textContent.replace(/[^\x20-\x7e]/g,'').slice(0,40)});
}
return JSON.stringify({cardTop:Math.round(cr.top),cardBottom:Math.round(cr.bottom),
  openTooltips:out.length, tips:out});
})()"""
n=sys.argv[1]
q=subprocess.run([sys.executable,"mkfixture.py",n],capture_output=True,text=True).stdout.strip()
ev('(function(){window.showNotification(%s);return "1"})()'%json.dumps(q)); time.sleep(1.2)
sp=("(function(){"+CARD+"var s=card.querySelectorAll('span');for(var i=0;i<s.length;i++)"
    "{if(/^Show$/.test(s[i].textContent.trim())){s[i].dispatchEvent(new MouseEvent('click',{bubbles:true}));return '\"ok\"';}}return '\"none\"';})()")
print("expand:", ev(sp)); time.sleep(0.6)
scroll=sys.argv[2] if len(sys.argv)>2 else "0"
ev("(function(){"+CARD+"card.scrollTop="+scroll+";return '\"s\"'})()"); time.sleep(0.3)
print("staged:", ev(JS))
time.sleep(0.5)
d=ev(MEASURE)
print("card:", d["cardTop"], "->", d["cardBottom"], " open tooltips:", d["openTooltips"])
bad=[t for t in d["tips"] if t["clippedAbove"] or t["clippedBelow"]]
for t in d["tips"]:
    flag = "CLIPPED" if (t["clippedAbove"] or t["clippedBelow"]) else "ok"
    print("  %-8s top=%-6s bot=%-6s %s" % (flag, t["top"], t["bottom"], t["txt"]))
print("CLIPPED COUNT:", len(bad))
