// P0.5-C1 — a cross-origin SIMPLE POST must not EXECUTE the handler.
// ⛔ The subject is the SERVER-SIDE EFFECT, not the browser's error. A blocked
// read is not a blocked write: text/plain makes this a "simple" request, so it
// is sent WITHOUT preflight and only actix-cors' block_on_origin_mismatch can
// stop the handler running. Evidence is the absence of "/listActions called".
(function () {
  return fetch('http://127.0.0.1:31401/listActions', {
      method: 'POST', mode: 'cors',
      headers: {'Content-Type': 'text/plain'},
      body: '{"limit":1}'
    })
    .then(function (r) { return r.text().then(function (t) {
      return JSON.stringify({href: location.href, status: r.status, body: t.slice(0,150)});
    }); })
    .catch(function (e) {
      return JSON.stringify({href: location.href, fetchError: String(e).slice(0,150)});
    });
})()
