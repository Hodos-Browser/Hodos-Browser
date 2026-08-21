// POSITIVE CONTROL: send add_domain_permission from the notification overlay
// (role 'notification') — the legitimate sender. The grant SHOULD be written,
// proving the self-nav role gate did not break the real approval flow.
(function(){
  window.cefMessage.send('add_domain_permission', [JSON.stringify({
    domain: 'legit-poscontrol-p3.com',
    identityKeyDisclosureAllowed: false,
    bundledScopeGrant: false
  })]);
  return JSON.stringify({ sent: true, href: location.href });
})()
