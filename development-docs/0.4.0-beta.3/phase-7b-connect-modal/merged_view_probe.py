"""P7b-A3/A4/A5/A6 — assert the merged connect view on the real overlay.

SUBJECT: the notification overlay browser (…/brc100-auth), attached BY URL, driven
through window.showNotification — the same injection path C++ uses.

⛔ The protocol fixture is deliberately security level **1**. A level-2 protocol
would have had its identifier printed by the old "Who these are with" footnote, so
a level-2 fixture passes whether or not this change works. That is the vacuous
test this file exists to avoid.
"""
import json, subprocess, sys, os, time, urllib.parse

RIG = os.path.join("..", "phase-3.5-layout-window-scoping", "p35drive.py")


def ev(js):
    out = subprocess.run([sys.executable, RIG, "eval", "brc100-auth", js],
                         capture_output=True, text=True).stdout.strip()
    return json.loads(json.loads(out.splitlines()[-1])["result"]["value"])


MANIFEST = {
    "name": "Merge Fixture", "description": "d", "iconUrl": "", "expiresAt": 0,
    "version": "1.0", "sourceNamespace": "metanet",
    "protocols": [
        # level 1 + a deceptive description: the ticket's attack, minus the
        # level-2 footnote that used to mask it.
        {"securityLevel": 1, "name": "3241645161d8", "keyId": "*",
         "purpose": "Show your profile picture."},
        {"securityLevel": 0, "name": "open thing", "keyId": "*",
         "purpose": "Do something harmless"},
    ],
    "baskets": [{"name": "default", "access": "read_write", "purpose": "Touch the default basket"},
                {"name": "app data", "access": "read", "purpose": "Read its own data"}],
    "certificates": [{"type": "t", "fields": ["email"], "purpose": "Read your email field"}],
    "spending": {"perTransactionUsd": 0, "perSessionUsd": 0, "monthlySatoshis": 10000},
    "counterparties": [],
}
QUERY = ("type=manifest_connect_bundle&domain=merge.example&manifest="
         + urllib.parse.quote(json.dumps(MANIFEST), safe=""))

PROBE = r"""
(function(){
  var body = document.body.innerText;
  var boxes = document.querySelectorAll('input[type=checkbox]');
  var btns = [], b = document.querySelectorAll('button');
  for (var i=0;i<b.length;i++){ var t=(b[i].textContent||'').trim();
    if (t) btns.push(t.replace(/[^\x20-\x7e]/g,'')); }
  var icons = 0, all = document.querySelectorAll('span');
  for (var i=0;i<all.length;i++){
    if (all[i].textContent.trim()==='i' && getComputedStyle(all[i]).borderRadius==='50%') icons++; }
  var ticked = 0, disabled = 0;
  for (var i=0;i<boxes.length;i++){ if(boxes[i].checked) ticked++; if(boxes[i].disabled) disabled++; }
  return JSON.stringify({
    checkboxes: boxes.length, ticked: ticked, disabled: disabled,
    buttons: btns,
    hasCustomizeButton: btns.indexOf('Customize') >= 0,
    protocolIdShown: body.indexOf('3241645161d8') >= 0,
    level0IdShown: body.indexOf('open thing') >= 0,
    deceptivePurposeShown: body.indexOf('Show your profile picture.') >= 0,
    identityAcrossMetanet: body.indexOf('across the Metanet') >= 0,
    quietModeFullLabel: body.indexOf('including ones it did not list above') >= 0,
    quietModeCallout: body.indexOf('Quiet mode is on') >= 0,
    protectedBasketMark: body.indexOf('never auto-granted') >= 0,
    allowWithoutLimits: btns.indexOf('Allow without limits') >= 0,
    counterpartyFootnoteGone: body.indexOf('Who these are with') < 0,
    infoIcons: icons
  });
})()
"""

ev('(function(){window.showNotification(%s); return "1"})()' % json.dumps(QUERY))
time.sleep(1.4)
d = ev(PROBE)
for k in sorted(d):
    print("  %-26s %s" % (k, d[k]))

fails = []
if d["hasCustomizeButton"]:            fails.append("Customize button still present")
if not d["protocolIdShown"]:           fails.append("level-1 protocol id NOT shown")
if not d["identityAcrossMetanet"]:     fails.append("identity label missing 'across the Metanet'")
if not d["quietModeFullLabel"]:        fails.append("quiet-mode label missing the qualifier")
if not d["counterpartyFootnoteGone"]:  fails.append("counterparty footnote still rendered")
if d["checkboxes"] < 5:                fails.append("per-item ticks not on the first screen")
print("\nRESULT:", "FAIL - " + "; ".join(fails) if fails else "PASS")
sys.exit(1 if fails else 0)
