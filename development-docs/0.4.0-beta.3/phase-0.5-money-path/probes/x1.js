// P0.5-X1 — a crafted data: frame must be stamped with its ANCESTOR's origin,
// not with the fake authority embedded in its own URL.
//
// ⛔ First attempt measured NOTHING: the frame's inline script began with
// `parent.__x1 = "ran"`, and a data: frame is OPAQUE-ORIGIN, so that throws a
// SecurityError and kills the script before it ever reaches cefMessage. The
// frame looked appended and the row looked green. Liveness must therefore be
// signalled through something that works cross-origin.
//
// The liveness signal here IS the evidence: '__x1_probe_marker' is a message
// name that exists nowhere, so it can only ever be DENIED — and the deny line
// prints the ORIGIN the browser process resolved for that frame, which is the
// exact quantity this row is about. No arm runs, nothing is spent.
(function () {
  var js = 'cefMessage.send("__x1_probe_marker",["{}"]);' +
           'cefMessage.send("send_transaction",[JSON.stringify(' +
           '{toAddress:"1BvBMSEYstWetqRdfSf1fQ8Y7AE59Xc8aW",amount:1000000,sendMax:false})]);' +
           'try{parent.postMessage("x1-ran","*")}catch(e){}';
  var f = document.createElement('iframe');
  window.__x1seen = 'no';
  window.addEventListener('message', function (e) {
    if (e.data === 'x1-ran') window.__x1seen = 'yes';
  });
  f.src = 'data:text/html,' +
          encodeURIComponent('a://127.0.0.1:5137/') +
          '<script>' + js + '<\/script>';
  document.body.appendChild(f);
  return JSON.stringify({href: location.href, appended: true, src: f.src.slice(0, 90)});
})()
