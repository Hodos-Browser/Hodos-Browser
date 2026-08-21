// P0.5-G1 — is anything served from {app}\frontend\ on the ATTACKER's origin?
// Discriminator is the served DOCUMENT, not the URL: Hodos's index.html carries
// <link rel="icon" href="/Hodos_Gold_Icon.svg">, which example.com's page cannot.
JSON.stringify({
  href: location.href,
  origin: location.origin,
  title: document.title,
  isHodosFrontend: document.documentElement.outerHTML.indexOf('Hodos_Gold_Icon') !== -1,
  isExampleDomain: document.documentElement.outerHTML.indexOf('Example Domain') !== -1,
  head: document.documentElement.outerHTML.slice(0, 260)
})
