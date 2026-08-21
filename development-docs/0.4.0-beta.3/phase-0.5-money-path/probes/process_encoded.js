// P0.5 panel #3 / Task 2 — SUBJECT: /%70rocessAction from a real external page.
// actix routes the DECODED path, so process_action runs either way; the question
// is whether C++ PRICED it. Pre-fix IsPaymentEndpoint("/%70rocessAction")==false
// => no X-Payment-*, so the prompt read price_unavailable, no gold pill, and the
// per-session dollar cap never advanced.
// DISCRIMINATOR: reason=per_tx_limit (priced) vs reason=price_unavailable (blind).
// Safety: the address is the standard broken-checksum probe address.
(function () {
  var body = {
    outputs: [{ satoshis: 5000000,
                address: '1BvBMSEYstWetqRdfSf1fQ8Y7AE59Xc8aW',
                outputDescription: 'p05 panel3 encoded' }],
    description: 'p05 panel3 encoded'
  };
  window.__hodos_walletCall('processAction', '/%70rocessAction', body, 'POST');
  return JSON.stringify({ href: location.href, fired: true });
})()
