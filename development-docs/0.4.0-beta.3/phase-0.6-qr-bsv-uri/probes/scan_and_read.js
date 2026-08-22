// DOM-path reader: trigger the scan from the wallet overlay, then read the send form.
(function(){
  window.cefMessage && window.cefMessage.send && window.cefMessage.send('qr_scan_request', []);
  return new Promise(function(resolve){
    setTimeout(function(){
      var inputs = Array.prototype.slice.call(document.querySelectorAll('input'));
      resolve(JSON.stringify({
        href: location.href,
        after: {
          hasSendForm: !!document.querySelector('form'),
          inputs: inputs.map(function(i){return {ph:i.placeholder, val:i.value};})
        }
      }));
    }, 3000);
  });
})()
