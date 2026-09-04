import json, sys, time
from urllib.request import urlopen
import websocket
t=[x for x in json.loads(urlopen("http://127.0.0.1:9322/json/list",timeout=5).read()) if "brc100-auth" in x["url"]][0]
ws=websocket.create_connection(t["webSocketDebuggerUrl"],timeout=20); i=[0]
def s(m,p=None):
    i[0]+=1; ws.send(json.dumps({"id":i[0],"method":m,"params":p or {}})); return i[0]
s("Runtime.enable"); s("Log.enable"); s("Page.enable"); time.sleep(0.3)
s("Page.reload",{"ignoreCache":True})
end=time.time()+8; ws.settimeout(1.0); seen=[]
while time.time()<end:
    try: m=json.loads(ws.recv())
    except Exception: continue
    if m.get("method")=="Runtime.exceptionThrown":
        d=m["params"]["exceptionDetails"]
        seen.append("EXC: "+(d.get("exception",{}).get("description") or d.get("text",""))[:300])
    elif m.get("method")=="Log.entryAdded":
        e=m["params"]["entry"]
        if e.get("level") in ("error","warning"): seen.append("%s: %s"%(e["level"].upper(),e["text"][:250]))
    elif m.get("method")=="Runtime.consoleAPICalled" and m["params"]["type"] in ("error","warning"):
        seen.append("CONSOLE: "+" ".join(str(a.get("value"))[:200] for a in m["params"]["args"]))
ws.close()
print("\n".join(seen[:14]) if seen else "(no errors captured)")
