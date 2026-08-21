// P0.5 Task A / M2 — SUBJECT: can an ordinary web page reach /processAction?
// Safety: valid-base58, valid-length, mainnet 0x00 version byte, checksum
// deliberately broken, so a bypass dies at address->script conversion.
(function () {
  var body = ({
    outputs: [{ satoshis: 1000000,
                address: '1BvBMSEYstWetqRdfSf1fQ8Y7AE59Xc8aW',
                outputDescription: 'p05 taskA M2' }],
    description: 'p05 taskA M2'
  });
  return window.__hodos_walletCall('processAction', '/processAction', body, 'POST')
    .then(function (r) {
      return JSON.stringify({ href: location.href, ok: true,
                              resp: JSON.stringify(r).slice(0, 500) });
    })
    .catch(function (e) {
      return JSON.stringify({ href: location.href, ok: false,
                              err: String(e).slice(0, 500) });
    });
})()
