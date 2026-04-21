/**
 * HTTP Proxy Server to Capture Metanet-Client Requests
 *
 * This proxy intercepts HTTP requests from metanet-client and logs:
 * - Full request URL, method, headers, body
 * - Serialized request format (if BRC-31 authenticated)
 * - Exact bytes being sent
 *
 * Usage:
 * 1. Start this proxy: node capture_metanet_requests.js
 * 2. Configure metanet-client to use this proxy (or set HTTP_PROXY environment variable)
 * 3. Make your certificate request
 * 4. Check the logs for the exact request format
 */

const http = require('http');
const https = require('https');
const { URL } = require('url');
const fs = require('fs');
const path = require('path');

const PROXY_PORT = 8888;
const LOG_FILE = path.join(__dirname, 'metanet_client_requests.log');

// Create log file
const logStream = fs.createWriteStream(LOG_FILE, { flags: 'a' });

function log(message) {
    const timestamp = new Date().toISOString();
    const logMessage = `[${timestamp}] ${message}\n`;
    console.log(logMessage.trim());
    logStream.write(logMessage);
}

function logRequest(req, body) {
    log('\n' + '='.repeat(80));
    log(`📥 INTERCEPTED REQUEST`);
    log('='.repeat(80));
    log(`Method: ${req.method}`);
    log(`URL: ${req.url}`);
    log(`Headers:`);
    Object.entries(req.headers).forEach(([key, value]) => {
        // Mask sensitive auth headers partially
        if (key.toLowerCase().includes('auth') || key.toLowerCase().includes('signature')) {
            const masked = typeof value === 'string' && value.length > 20
                ? value.substring(0, 20) + '...' + value.substring(value.length - 10)
                : '***';
            log(`  ${key}: ${masked}`);
        } else {
            log(`  ${key}: ${value}`);
        }
    });

    if (body && body.length > 0) {
        log(`\nBody (${body.length} bytes):`);

        // Try to parse as JSON
        try {
            const bodyStr = body.toString('utf8');
            const json = JSON.parse(bodyStr);
            log(`  JSON (formatted):`);
            log(JSON.stringify(json, null, 2));

            // Log field order
            log(`  Field order in JSON:`);
            const fields = Object.keys(json);
            fields.forEach((field, i) => {
                log(`    ${i + 1}. "${field}"`);
            });

            // Log exact bytes
            log(`  Body (hex, full): ${body.toString('hex')}`);
            log(`  Body (base64, full): ${body.toString('base64')}`);
        } catch (e) {
            // Not JSON, log as raw
            log(`  Body (hex, first 200 bytes): ${body.toString('hex').substring(0, 400)}`);
            log(`  Body (utf8, first 200 chars): ${body.toString('utf8').substring(0, 200)}`);
        }
    }

    log('='.repeat(80) + '\n');
}

// Create HTTP proxy server
const server = http.createServer((clientReq, clientRes) => {
    const targetUrl = new URL(clientReq.url);

    // Collect request body
    let bodyChunks = [];
    clientReq.on('data', chunk => {
        bodyChunks.push(chunk);
    });

    clientReq.on('end', () => {
        const body = Buffer.concat(bodyChunks);

        // Log the intercepted request
        logRequest(clientReq, body);

        // Determine if HTTPS or HTTP
        const isHttps = targetUrl.protocol === 'https:';
        const httpModule = isHttps ? https : http;

        // Prepare options for forwarding
        const options = {
            hostname: targetUrl.hostname,
            port: targetUrl.port || (isHttps ? 443 : 80),
            path: targetUrl.pathname + targetUrl.search,
            method: clientReq.method,
            headers: { ...clientReq.headers }
        };

        // Remove proxy-related headers
        delete options.headers['proxy-connection'];
        delete options.headers['connection'];
        delete options.headers['host'];

        // Forward the request
        const proxyReq = httpModule.request(options, (proxyRes) => {
            // Log response
            log(`📤 RESPONSE: ${proxyRes.statusCode} ${proxyRes.statusMessage}`);
            log(`Response Headers:`);
            Object.entries(proxyRes.headers).forEach(([key, value]) => {
                if (key.toLowerCase().includes('auth') || key.toLowerCase().includes('signature')) {
                    const masked = typeof value === 'string' && value.length > 20
                        ? value.substring(0, 20) + '...' + value.substring(value.length - 10)
                        : '***';
                    log(`  ${key}: ${masked}`);
                } else {
                    log(`  ${key}: ${value}`);
                }
            });

            // Forward response headers
            clientRes.writeHead(proxyRes.statusCode, proxyRes.headers);

            // Forward response body
            proxyRes.on('data', chunk => {
                clientRes.write(chunk);
            });

            proxyRes.on('end', () => {
                clientRes.end();
            });
        });

        proxyReq.on('error', (err) => {
            log(`❌ Proxy request error: ${err.message}`);
            clientRes.writeHead(502, { 'Content-Type': 'text/plain' });
            clientRes.end(`Proxy error: ${err.message}`);
        });

        // Forward request body
        if (body.length > 0) {
            proxyReq.write(body);
        }
        proxyReq.end();
    });

    clientReq.on('error', (err) => {
        log(`❌ Client request error: ${err.message}`);
        clientRes.writeHead(400, { 'Content-Type': 'text/plain' });
        clientRes.end(`Client error: ${err.message}`);
    });
});

server.listen(PROXY_PORT, () => {
    log(`🚀 HTTP Proxy Server started on port ${PROXY_PORT}`);
    log(`📝 Logging to: ${LOG_FILE}`);
    log(`\nTo use this proxy:`);
    log(`  1. Set HTTP_PROXY=http://localhost:${PROXY_PORT}`);
    log(`  2. Set HTTPS_PROXY=http://localhost:${PROXY_PORT}`);
    log(`  3. Or configure your client to use http://localhost:${PROXY_PORT} as proxy`);
    log(`\nWaiting for requests...\n`);
});

// Handle shutdown gracefully
process.on('SIGINT', () => {
    log('\n\n🛑 Shutting down proxy server...');
    logStream.end();
    server.close(() => {
        log('✅ Proxy server stopped');
        process.exit(0);
    });
});
