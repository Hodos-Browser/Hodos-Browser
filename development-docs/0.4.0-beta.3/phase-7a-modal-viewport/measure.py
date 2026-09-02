import json, subprocess, sys, os
RIG = os.path.join("..", "phase-3.5-layout-window-scoping", "p35drive.py")

PROBE = r"""
(function(){
  var q = %s;
  window.showNotification(q);
  return "shown";
})()
"""

GEOM = r"""
(function(){
  var backdrop = null, divs = document.querySelectorAll('div');
  for (var i=0;i<divs.length;i++){
    var cs = getComputedStyle(divs[i]);
    if (cs.position === 'fixed' && cs.display === 'flex' &&
        parseInt(cs.width) === window.innerWidth) { backdrop = divs[i]; break; }
  }
  if (!backdrop) return JSON.stringify({error:'backdrop not found',
      divs: divs.length, bodyLen: document.body.innerHTML.length});
  var card = backdrop.firstElementChild;
  if (!card) return JSON.stringify({error:'card not found under backdrop'});
  var btns = card.querySelectorAll('button');
  var last = btns.length ? btns[btns.length-1] : null;
  var r = card.getBoundingClientRect();
  var b = last ? last.getBoundingClientRect() : null;
  return JSON.stringify({
    viewportH: window.innerHeight,
    cardTop: Math.round(r.top), cardBottom: Math.round(r.bottom),
    cardH: Math.round(r.height),
    cardOverflowsBy: Math.round(r.bottom - window.innerHeight),
    cardTopClipped: Math.round(r.top) < 0,
    buttonCount: btns.length,
    lastButtonText: last ? last.textContent.trim() : null,
    lastButtonBottom: b ? Math.round(b.bottom) : null,
    lastButtonFullyVisible: b ? (b.top >= 0 && b.bottom <= window.innerHeight) : null,
    backdropOverflow: getComputedStyle(backdrop).overflow,
    cardMaxHeight: getComputedStyle(card).maxHeight,
    cardOverflowY: getComputedStyle(card).overflowY
  });
})()
"""

def run(js):
    out = subprocess.run([sys.executable, RIG, "eval", "brc100-auth", js],
                         capture_output=True, text=True)
    return out.stdout.strip(), out.stderr.strip()

n = sys.argv[1] if len(sys.argv) > 1 else "8"
query = subprocess.run([sys.executable, "mkfixture.py", n],
                       capture_output=True, text=True).stdout.strip()
o, e = run(PROBE % json.dumps(query))
print("show:", o.splitlines()[-1] if o else e)
import time; time.sleep(1.2)
o, e = run(GEOM)
line = o.splitlines()[-1] if o else e
try:
    print("N=%s" % n, json.dumps(json.loads(json.loads(line)["result"]["value"]), indent=2))
except Exception:
    print("N=%s raw:" % n, line, e)
