// SERVE+TRUST legs — from a cross-origin page (teragun.com), frame the internal
// wallet-panel and observe whether it is served (no X-Frame-Options) and whether
// the parent can even touch it. A real cross-origin document that LOADED throws
// SecurityError on contentWindow.document — that throw is the positive signal
// that a document committed in the frame (a blocked frame stays about:blank,
// same-origin-accessible).
(function () {
  return new Promise(function (resolve) {
    var f = document.createElement('iframe');
    f.id = 'p3f';
    f.src = 'http://127.0.0.1:5137/wallet-panel';
    var loaded = false;
    f.onload = function () { loaded = true; };
    document.body.appendChild(f);
    setTimeout(function () {
      var crossOrigin = null, sameOriginBlank = null;
      try {
        var d = f.contentWindow.document;      // throws if a real x-origin doc loaded
        sameOriginBlank = (f.contentWindow.location.href === 'about:blank'
                           || d.body === null || d.body.innerHTML === '');
        crossOrigin = false;
      } catch (e) {
        crossOrigin = true;                    // SecurityError => a cross-origin doc committed
      }
      resolve(JSON.stringify({
        parent: location.href,
        onloadFired: loaded,
        frameHrefFromParent: (function(){ try { return f.contentWindow.location.href; } catch(e){ return 'BLOCKED(x-origin)'; } })(),
        crossOriginDocCommitted: crossOrigin,
        sameOriginBlank: sameOriginBlank
      }));
    }, 2500);
  });
})()
