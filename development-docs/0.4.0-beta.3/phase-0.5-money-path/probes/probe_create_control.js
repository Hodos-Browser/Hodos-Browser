// P0.5 Task A / M2 — CONTROL: the SAME page, the SAME body, the gated twin.
// Fired without awaiting the promise: an over-cap /createAction opens a modal
// and the promise does not settle until the owner answers it. The evidence is
// the wallet log line, not the return value.
(function () {
  var body = ({
    outputs: [{ satoshis: 1000000,
                address: '1BvBMSEYstWetqRdfSf1fQ8Y7AE59Xc8aW',
                outputDescription: 'p05 taskA M2 control' }],
    description: 'p05 taskA M2 control'
  });
  window.__hodos_walletCall('createAction', '/createAction', body, 'POST')
    .then(function (r) { window.__p05control = 'RESOLVED ' + JSON.stringify(r).slice(0, 300); })
    .catch(function (e) { window.__p05control = 'REJECTED ' + String(e).slice(0, 300); });
  return JSON.stringify({ href: location.href, fired: true });
})()
