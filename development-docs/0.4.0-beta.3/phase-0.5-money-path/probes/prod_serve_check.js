// Subject control (G1-style): fetch a marker that exists ONLY in the staged
// {exe}/frontend/ dir. If the disk-serve (LocalFileResourceRequestHandler) path
// is live, this returns the marker; if Vite is answering 5137, it returns the
// dev index.html instead. Proves WHICH serve path is engaged before we rely on it.
fetch('http://127.0.0.1:5137/__p3_marker.txt')
  .then(function(r){ return r.text(); })
  .then(function(t){ return JSON.stringify({ marker: t.slice(0,40) }); })
  .catch(function(e){ return JSON.stringify({ err: String(e) }); })
