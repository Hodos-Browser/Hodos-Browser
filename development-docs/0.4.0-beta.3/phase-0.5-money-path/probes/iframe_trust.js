// TRUST leg — eval INSIDE the framed wallet-panel OOPIF (parented under
// teragun.com). If isInternalPage lacked a frame->IsMain() guard, this
// cross-origin-embedded frame received the privileged surface.
JSON.stringify({
  href: location.href,
  isTopFrame: (window.top === window.self),
  parentIsCrossOrigin: (function(){ try { return window.parent.location.href; } catch(e){ return 'X-ORIGIN PARENT'; } })(),
  hasHodosBrowser: (typeof window.hodosBrowser !== 'undefined'),
  hodosKeys: (typeof window.hodosBrowser !== 'undefined') ? Object.keys(window.hodosBrowser) : [],
  hasCefMessage: (typeof window.cefMessage !== 'undefined'),
  hasWalletCallBridge: (typeof window.__hodos_walletCall !== 'undefined')
})
