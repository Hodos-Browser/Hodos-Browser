// Runs in the wallet overlay. Triggers the scan (which falls through to screen
// capture because the active tab has no scannable QR element), then polls for up
// to 8s across the overlay hide/reshow, capturing whichever outcome lands:
//   - a populated send form (recipient/amount)   -> found path
//   - a "not a BSV payment" message               -> A4 unrecognized path
// Also snapshots window.__hodos_qr_probe so a capture-path injection would show.
(function () {
  delete window.__hodos_qr_probe;
  var captured = { recipient: null, amountBsv: null, message: null, injection: '(unset)' };

  function snapshot() {
    var inputs = Array.prototype.slice.call(document.querySelectorAll('input'));
    var rec = inputs.filter(function (i) { return i.placeholder && i.placeholder.indexOf('address') !== -1; });
    var amt = inputs.filter(function (i) { return i.placeholder === '0.00000000'; });
    if (rec.length && rec[0].value) captured.recipient = rec[0].value;
    if (amt.length && amt[0].value) captured.amountBsv = amt[0].value;
    var t = document.body.innerText || '';
    var m = t.match(/Found a QR code[^\n]*/);
    if (m) captured.message = m[0];
    if (window.__hodos_qr_probe !== undefined) captured.injection = String(window.__hodos_qr_probe);
  }

  window.cefMessage && window.cefMessage.send && window.cefMessage.send('qr_scan_request', []);

  return new Promise(function (resolve) {
    var n = 0;
    var iv = setInterval(function () {
      n++;
      snapshot();
      if (n >= 32) { // ~8s at 250ms
        clearInterval(iv);
        captured.href = location.href;
        delete window.__hodos_qr_probe;
        resolve(JSON.stringify(captured));
      }
    }, 250);
  });
})()
