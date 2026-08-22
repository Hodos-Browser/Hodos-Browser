// A4 (React half) — dispatch a synthetic `unrecognized` screen-capture result into
// the REAL wallet overlay and read the user-visible message. This exercises the
// exact React arm added in WalletPanel.tsx (status==='unrecognized' -> scanMessage),
// through the same MessageEvent path + origin gate the C++ renderer uses. The C++
// half (scheme extraction + delivery) is covered by the SchemeForMessage unit test.
(function () {
  // Same shape the renderer builds: dispatchEvent(MessageEvent('message',{data:{type,data}}))
  window.dispatchEvent(new MessageEvent('message', {
    data: { type: 'qr_screen_capture_result', data: { status: 'unrecognized', scheme: 'https' } }
  }));
  return new Promise(function (resolve) {
    setTimeout(function () {
      var t = document.body.innerText || '';
      var m = t.match(/Found a QR code[^\n]*/);
      resolve(JSON.stringify({
        href: location.href,
        message_rendered: m ? m[0] : null,
        // negative-control shape: a not_found must NOT produce the unrecognized message
      }));
    }, 500);
  });
})()
