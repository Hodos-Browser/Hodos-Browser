// P0.5 panel #3 / Task 1 — SUBJECT: /wallet/pay402 driven from a real external
// page through the wallet_call IPC bridge, i.e. the arm that stamps X-Payment-*
// only for endpoints listed in hodos::IsPaymentEndpoint.
//
// DISCRIMINATOR: the wallet log must read `reason=per_tx_limit`, NOT
// `reason=price_unavailable`. per_tx_limit is only reachable if C++ priced the
// call, which is only possible if the endpoint is in IsPaymentEndpoint — so it
// proves the C++ half landed, exactly as §4q used it for /processAction.
//
// Safety: server_pubkey_hex is format-valid (66 hex, 02 prefix) but its x is
// 0xff..ff, which is > p and therefore NOT on the curve. BRC-42 derivation runs
// BEFORE any UTXO selection, so even a total gate bypass dies at
// "Key derivation failed" and cannot reserve or strand a single satoshi.
(function () {
  var body = {
    server_pubkey_hex:
      '02ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff',
    satoshis: 5000000,
    original_url: 'https://teragun.com/panel3-probe'
  };
  window.__hodos_walletCall('pay402', '/wallet/pay402', body, 'POST');
  return JSON.stringify({ href: location.href, fired: true });
})()
