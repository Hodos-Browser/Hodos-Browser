// P0.5 Task C — the C++ trust-boundary rows, re-run from ONE external page.
// X2, X3, X1, E1, G2. Returns location.href with every value: 11 CDP targets all
// report type:"page" and ten are overlays, so the subject must be proved.
//
// Safety: send_transaction's body is the valid-prefix / invalid-checksum probe
// address. If the C2 gate ever regressed, the call dies at address decode.
(function () {
  var out = { href: location.href, origin: location.origin };

  // G2 — an external page must NOT be handed the identity surface, with or
  // without the :5137 substring in its own URL.
  out.G2_identity = (window.hodosBrowser && typeof window.hodosBrowser.identity);
  out.G2_hodosBrowser = typeof window.hodosBrowser;

  // X2 — the arm that spent unprompted before P0.5.
  try {
    window.cefMessage.send('send_transaction', [JSON.stringify({
      toAddress: '1BvBMSEYstWetqRdfSf1fQ8Y7AE59Xc8aW',
      amount: 1000000, sendMax: false })]);
    out.X2_sent = true;
  } catch (e) { out.X2_sent = 'threw: ' + e; }

  // X3 — the default-deny allowlist, not any one arm.
  try {
    window.cefMessage.send('get_balance', ['{}']);
    out.X3_sent = true;
  } catch (e) { out.X3_sent = 'threw: ' + e; }

  // E1 — about:blank child must inherit this origin, not manufacture an empty one.
  try {
    var f = document.createElement('iframe');
    document.body.appendChild(f);
    f.contentWindow.cefMessage.send('send_transaction', [JSON.stringify({
      toAddress: '1BvBMSEYstWetqRdfSf1fQ8Y7AE59Xc8aW',
      amount: 1000000, sendMax: false })]);
    out.E1_sent = true;
  } catch (e) { out.E1_sent = 'threw: ' + String(e).slice(0, 120); }

  // X1 — a crafted data: frame that scripts ITSELF (a data: frame is
  // opaque-origin, so it must run its own inline script) and supplies a
  // non-empty attacker origin at step 1 of the old derivation.
  try {
    var g = document.createElement('iframe');
    g.src = 'data:text/html,a://127.0.0.1:5137/<script>' +
            'parent.__x1 = "ran";' +
            'cefMessage.send("send_transaction", [JSON.stringify(' +
            '{toAddress:"1BvBMSEYstWetqRdfSf1fQ8Y7AE59Xc8aW",amount:1000000,sendMax:false})]);' +
            '<\/script>';
    document.body.appendChild(g);
    out.X1_frame = 'appended';
  } catch (e) { out.X1_frame = 'threw: ' + String(e).slice(0, 120); }

  return JSON.stringify(out);
})()
