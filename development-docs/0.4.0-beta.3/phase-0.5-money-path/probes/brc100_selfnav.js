// The "run first" item — a page navigates ITSELF to the internal brc100-auth UI
// with an attacker-chosen domain. Does the internal (first-party) approval UI
// render + does it expose an Allow control that would write a grant for a domain
// the attacker named? Observe WITHOUT clicking; read the rendered prompt + the
// wallet API it holds (it is at 127.0.0.1:5137 = internal origin).
JSON.stringify({
  href: location.href,
  isInternalOrigin: (location.origin === 'http://127.0.0.1:5137'),
  hasWalletCallBridge: (typeof window.__hodos_walletCall !== 'undefined'),
  hasCefMessage: (typeof window.cefMessage !== 'undefined'),
  bodyText: document.body.innerText.slice(0, 400),
  buttons: Array.prototype.slice.call(document.querySelectorAll('button'))
             .map(function(b){ return (b.innerText||'').trim(); }).filter(Boolean)
})
