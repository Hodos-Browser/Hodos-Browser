// CONTROL for process_encoded.js — the SAME body on the UNENCODED path.
// Establishes what a correctly-priced call looks like in the same run, so the
// subject's reason string is compared against a live sibling rather than memory.
(function () {
  var body = {
    outputs: [{ satoshis: 5000000,
                address: '1BvBMSEYstWetqRdfSf1fQ8Y7AE59Xc8aW',
                outputDescription: 'p05 panel3 plain' }],
    description: 'p05 panel3 plain'
  };
  window.__hodos_walletCall('processAction', '/processAction', body, 'POST');
  return JSON.stringify({ href: location.href, fired: true });
})()
