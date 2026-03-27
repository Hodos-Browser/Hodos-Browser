#!/usr/bin/env node
// Proxy server that captures requests to SocialCert and forwards them.
// Run this, then point the metanet-client's certifier URL at localhost:8199.
//
// Usage:
//   node capture_proxy.mjs
//   Then trigger certificate acquisition with certifierUrl = http://localhost:8199
//
// This will log the EXACT headers and body the client sends, then forward to SocialCert.

import http from 'http';
import https from 'https';

const SOCIALCERT_HOST = 'backend.socialcert.net';
const PROXY_PORT = 8199;
let requestCount = 0;

const server = http.createServer((req, res) => {
  requestCount++;
  const reqId = requestCount;

  let body = [];
  req.on('data', chunk => body.push(chunk));
  req.on('end', () => {
    const bodyBuffer = Buffer.concat(body);
    const bodyStr = bodyBuffer.toString('utf8');

    console.log(`\n${'='.repeat(80)}`);
    console.log(`REQUEST #${reqId}: ${req.method} ${req.url}`);
    console.log(`${'='.repeat(80)}`);

    // Log all headers
    console.log('\n--- HEADERS ---');
    for (const [key, value] of Object.entries(req.headers)) {
      if (key.startsWith('x-bsv-auth') || key === 'content-type') {
        console.log(`  ${key}: ${value}`);
      }
    }

    // Log auth headers specifically
    console.log('\n--- AUTH HEADERS (full) ---');
    for (const [key, value] of Object.entries(req.headers)) {
      if (key.startsWith('x-bsv-auth')) {
        console.log(`  ${key}: ${value}`);
      }
    }

    // Log body
    console.log(`\n--- BODY (${bodyBuffer.length} bytes) ---`);
    if (bodyStr.length < 2000) {
      console.log(bodyStr);
    } else {
      console.log(bodyStr.substring(0, 2000) + '...');
    }

    // If it's a JSON body, parse and log structure
    if (req.headers['content-type']?.includes('json') && bodyStr.length > 0) {
      try {
        const json = JSON.parse(bodyStr);
        console.log('\n--- JSON STRUCTURE ---');
        console.log('  Keys:', Object.keys(json));
        if (json.clientNonce) console.log('  clientNonce:', json.clientNonce.substring(0, 30) + '...');
        if (json.type) console.log('  type:', json.type);
        if (json.fields) {
          console.log('  fields keys:', Object.keys(json.fields));
          for (const [k, v] of Object.entries(json.fields)) {
            console.log(`    ${k}: ${v.substring(0, 50)}... (${v.length} chars)`);
          }
        }
        if (json.masterKeyring) {
          console.log('  masterKeyring keys:', Object.keys(json.masterKeyring));
          for (const [k, v] of Object.entries(json.masterKeyring)) {
            console.log(`    ${k}: ${v.substring(0, 50)}... (${v.length} chars)`);
          }
        }
        // Log exact JSON key order
        console.log('  JSON key order (from string):', bodyStr.match(/"[^"]+"\s*:/g)?.slice(0, 10).map(m => m.replace(/[":]/g, '').trim()));
      } catch (e) {
        console.log('  (not valid JSON)');
      }
    }

    // Log body as hex (first 200 bytes)
    console.log(`\n--- BODY HEX (first 200 bytes) ---`);
    console.log(bodyBuffer.slice(0, 200).toString('hex'));

    // Forward to SocialCert
    console.log(`\n--- FORWARDING to https://${SOCIALCERT_HOST}${req.url} ---`);

    const fwdHeaders = { ...req.headers };
    delete fwdHeaders.host;
    fwdHeaders.host = SOCIALCERT_HOST;

    const fwdReq = https.request({
      hostname: SOCIALCERT_HOST,
      port: 443,
      path: req.url,
      method: req.method,
      headers: fwdHeaders
    }, (fwdRes) => {
      let respBody = [];
      fwdRes.on('data', chunk => respBody.push(chunk));
      fwdRes.on('end', () => {
        const respBuffer = Buffer.concat(respBody);
        const respStr = respBuffer.toString('utf8');

        console.log(`\n--- RESPONSE #${reqId}: ${fwdRes.statusCode} ---`);
        console.log('Response headers:');
        for (const [key, value] of Object.entries(fwdRes.headers)) {
          if (key.startsWith('x-bsv-auth') || key === 'content-type') {
            console.log(`  ${key}: ${value}`);
          }
        }
        console.log('Response body:', respStr.substring(0, 500));
        console.log(`${'='.repeat(80)}\n`);

        // Forward response back to client
        res.writeHead(fwdRes.statusCode, fwdRes.headers);
        res.end(respBuffer);
      });
    });

    fwdReq.on('error', (e) => {
      console.error(`Forward error: ${e.message}`);
      res.writeHead(502);
      res.end(JSON.stringify({ error: e.message }));
    });

    fwdReq.end(bodyBuffer);
  });
});

server.listen(PROXY_PORT, () => {
  console.log(`Capture proxy running on http://localhost:${PROXY_PORT}`);
  console.log(`Forwarding to https://${SOCIALCERT_HOST}`);
  console.log(`\nTo test with Rust wallet:`);
  console.log(`  Change certifierUrl to http://localhost:${PROXY_PORT}`);
  console.log(`\nTo test with metanet-client:`);
  console.log(`  Change certifier URL to http://localhost:${PROXY_PORT}`);
  console.log(`\nWaiting for requests...\n`);
});
