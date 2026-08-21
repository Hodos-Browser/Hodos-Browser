// P0.5-G1 SUBJECT CONTROL — was IsFrontendAvailable() actually TRUE in this build?
// A green G1 is vacuous if the local file handler never engaged. `__g1_marker.txt`
// exists ONLY in {exe}\frontend\. Vite (outside the browser) answers the same URL
// with its dev-server index.html, so the marker can only come from disk.
(function () {
  return fetch('http://127.0.0.1:5137/__g1_marker.txt', {cache: 'no-store'})
    .then(function (r) { return r.text().then(function (t) {
      return JSON.stringify({
        href: location.href, status: r.status,
        servedFromDisk: t.indexOf('G1-SUBJECT-MARKER-a7f3c2') !== -1,
        body: t.slice(0, 120)
      });
    }); })
    .catch(function (e) {
      return JSON.stringify({href: location.href, err: String(e).slice(0, 200)});
    });
})()
