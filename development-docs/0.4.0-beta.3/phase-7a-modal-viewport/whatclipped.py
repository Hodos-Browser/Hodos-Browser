import json, subprocess, sys, os, time
RIG = os.path.join("..", "phase-3.5-layout-window-scoping", "p35drive.py")
JS = r"""
(function(){
  var divs=document.querySelectorAll('div'), bd=null;
  for(var i=0;i<divs.length;i++){var cs=getComputedStyle(divs[i]);
    if(cs.position==='fixed'&&cs.display==='flex'&&parseInt(cs.width)===window.innerWidth){bd=divs[i];break;}}
  var card=bd.firstElementChild, out=[], vh=window.innerHeight;
  var walk=function(el,d){
    if(d>3) return;
    for(var i=0;i<el.children.length;i++){
      var c=el.children[i], r=c.getBoundingClientRect();
      var txt=(c.textContent||'').trim().replace(/\s+/g,' ').slice(0,52);
      if(txt && r.height>0){
        var vis = r.top>=0 && r.bottom<=vh;
        if(!vis) out.push({top:Math.round(r.top),bot:Math.round(r.bottom),
                           where:(r.top<0?'ABOVE':'BELOW'),txt:txt});
      }
      if(!vis) walk(c,d+1);
    }
  };
  walk(card,0);
  var seen={},uniq=[];
  for(var i=0;i<out.length;i++){var k=out[i].txt; if(!seen[k]){seen[k]=1;uniq.push(out[i]);}}
  return JSON.stringify({viewportH:vh, offscreen:uniq.slice(0,14)});
})()
"""
n = sys.argv[1]
q = subprocess.run([sys.executable,"mkfixture.py",n],capture_output=True,text=True).stdout.strip()
subprocess.run([sys.executable,RIG,"eval","brc100-auth",
                '(function(){window.showNotification(%s);return 1})()'%json.dumps(q)],
               capture_output=True,text=True)
time.sleep(1.2)
o=subprocess.run([sys.executable,RIG,"eval","brc100-auth",JS],capture_output=True,text=True).stdout
line=o.strip().splitlines()[-1]
d=json.loads(json.loads(line)["result"]["value"])
print("N=%s  viewport=%s" % (n, d["viewportH"]))
for r in d["offscreen"]:
    print("  %-5s top=%-6s bot=%-6s  %s" % (r["where"], r["top"], r["bot"], r["txt"]))
