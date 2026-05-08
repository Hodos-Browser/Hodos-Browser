// BRC-121 Simple HTTP 402 Payment — demo server.
//
// Two endpoints exercise different paywall patterns:
//   /paid         — pay every visit (per-request paywall; canonical BRC-121)
//   /paid-session — pay once, set a session cookie, skip 402 for returning visits
//                   (the recommended pattern for content sites — see project notes)
//
// Per BRC-121, first hit is 402 + `x-bsv-sats` + `x-bsv-server`. Hodos's wallet
// builds a BRC-29 BEEF and retries with the 5 retry headers (x-bsv-beef,
// x-bsv-sender, x-bsv-nonce, x-bsv-time, x-bsv-vout). We validate the BEEF,
// optionally set a cookie, return 200 with paid content.
//
// Hodos uses noSend=true on the BRC-121 path; broadcast happens after server
// returns 200 via /wallet/broadcast-nosend. We don't broadcast here.

import express from 'express';
import crypto from 'crypto';
import { PrivateKey, Transaction } from '@bsv/sdk';

const PORT = Number(process.env.PORT || 31402);
const PRICE_SATS = Number(process.env.PRICE_SATS || 100);
// Short window so we can test expiry without waiting around. Real sites
// use 1h–7d depending on price tier.
const SESSION_TTL_SECONDS = Number(process.env.SESSION_TTL_SECONDS || 300);

const serverPriv = process.env.SERVER_WIF
    ? PrivateKey.fromWif(process.env.SERVER_WIF)
    : PrivateKey.fromRandom();
const serverPub = serverPriv.toPublicKey();
const SERVER_PUBKEY_HEX = serverPub.toString();

// Replay guard: once a BEEF is accepted, we don't accept it again.
const acceptedTxids = new Set();

// In-memory session store: token → expiresAtMs.
const sessionStore = new Map();
function newSessionToken() {
    const token = crypto.randomBytes(16).toString('hex');
    sessionStore.set(token, Date.now() + SESSION_TTL_SECONDS * 1000);
    return token;
}
function isSessionValid(token) {
    if (!token) return false;
    const expiresAt = sessionStore.get(token);
    if (!expiresAt) return false;
    if (Date.now() >= expiresAt) {
        sessionStore.delete(token);
        return false;
    }
    return true;
}

const app = express();

// Tiny cookie parser (no dep): pulls a named cookie from the Cookie header.
function parseCookie(req, name) {
    const raw = req.get('cookie') || '';
    for (const part of raw.split(';')) {
        const [k, ...vparts] = part.trim().split('=');
        if (k === name) return vparts.join('=');
    }
    return null;
}

app.get('/health', (_req, res) => {
    res.json({
        ok: true,
        priceSats: PRICE_SATS,
        sessionTtlSeconds: SESSION_TTL_SECONDS,
        serverPubkey: SERVER_PUBKEY_HEX,
        acceptedCount: acceptedTxids.size,
        activeSessions: sessionStore.size,
    });
});

app.get('/', (_req, res) => {
    res.set('content-type', 'text/html; charset=utf-8');
    res.send(`<!doctype html>
<html lang="en"><head><meta charset="utf-8"><title>BRC-121 demo</title></head>
<body style="font-family:system-ui;max-width:680px;margin:40px auto;padding:0 16px;">
  <h1>BRC-121 demo server</h1>
  <p>Server pubkey: <code>${SERVER_PUBKEY_HEX}</code></p>
  <p>Price per visit: <code>${PRICE_SATS}</code> sats</p>
  <p>Session TTL (cookie variant): <code>${SESSION_TTL_SECONDS}</code> seconds</p>
  <ul>
    <li><a href="/paid">Open <code>/paid</code></a> — pay every visit (no cookie)</li>
    <li><a href="/paid-session">Open <code>/paid-session</code></a> — pay once, returning visits served free for ${SESSION_TTL_SECONDS}s via cookie</li>
    <li><a href="/health">Server health</a></li>
  </ul>
</body></html>`);
});

// Validates the BRC-121 retry headers. Canonical wire format (per @bsv/402-pay
// and Hodos) is 5 headers:
//   x-bsv-beef    — base64 atomic BEEF
//   x-bsv-sender  — sender pubkey hex
//   x-bsv-nonce   — base64 nonce (client-chosen)
//   x-bsv-time    — decimal-ms timestamp string
//   x-bsv-vout    — index of the BRC-29 output in the BEEF (usually "0")
// Older / abbreviated form was a single `x-bsv-payment: <BEEF base64>`. We
// accept either so this server works against any BRC-121 client.
//
// Returns { ok: true, txid, sats } on success, { ok: false, status, error } on
// failure, { needPayment: true } if no payment headers present.
function validatePayment(req) {
    let beefB64 = req.get('x-bsv-beef') || req.get('x-bsv-payment');
    if (!beefB64) {
        return { ok: false, needPayment: true };
    }
    try {
        const beefBytes = Buffer.from(beefB64, 'base64');
        let tx;
        try {
            tx = Transaction.fromAtomicBEEF([...beefBytes]);
        } catch {
            tx = Transaction.fromBEEF([...beefBytes]);
        }
        const txid = tx.id('hex');
        if (acceptedTxids.has(txid)) {
            return { ok: false, status: 409, error: 'Replay: BEEF already accepted', txid };
        }
        // x-bsv-vout tells us which output carries the BRC-29 payment. Falls
        // back to 0 for older clients that don't send the header.
        const voutStr = req.get('x-bsv-vout') || '0';
        const vout = Number.parseInt(voutStr, 10);
        if (!Number.isFinite(vout) || vout < 0) {
            return { ok: false, status: 400, error: `Invalid x-bsv-vout '${voutStr}'` };
        }
        const payOut = tx.outputs[vout];
        if (!payOut) {
            return { ok: false, status: 400, error: `BEEF has no output at vout=${vout}` };
        }
        if (typeof payOut.satoshis !== 'number' || payOut.satoshis < PRICE_SATS) {
            return {
                ok: false,
                status: 400,
                error: `Insufficient payment: output ${vout} has ${payOut.satoshis} sats, expected >= ${PRICE_SATS}`,
            };
        }
        acceptedTxids.add(txid);
        return { ok: true, txid, sats: payOut.satoshis, vout };
    } catch (err) {
        return { ok: false, status: 400, error: 'Invalid BEEF: ' + err.message };
    }
}

// Per-request paywall: every visit pays.
app.get('/paid', (req, res) => {
    const result = validatePayment(req);
    if (result.needPayment) {
        console.log(`[402] /paid — challenge sent (${PRICE_SATS} sats → ${SERVER_PUBKEY_HEX.slice(0, 16)}...)`);
        res.set('x-bsv-sats', String(PRICE_SATS));
        res.set('x-bsv-server', SERVER_PUBKEY_HEX);
        res.status(402).send({ error: 'Payment required', priceSats: PRICE_SATS });
        return;
    }
    if (!result.ok) {
        console.error(`[${result.status}] /paid — ${result.error}`);
        res.status(result.status).send({ error: result.error });
        return;
    }
    console.log(`[200] /paid — accepted txid=${result.txid.slice(0, 16)}... (${result.sats} sats at vout ${result.vout})`);
    res.set('content-type', 'text/html; charset=utf-8');
    res.send(`<!doctype html>
<html lang="en"><head><meta charset="utf-8"><title>BRC-121 paid</title></head>
<body style="font-family:system-ui;max-width:680px;margin:40px auto;padding:0 16px;">
  <h1>Payment accepted</h1>
  <p>You paid <strong>${result.sats}</strong> sats. This page is the paid content.</p>
  <p>txid: <code>${result.txid}</code></p>
  <p><a href="/paid">Reload</a> — should re-prompt for payment (per-request paywall, no cookie).</p>
</body></html>`);
});

// Session-cookie paywall: pay once, returning visits served free until cookie expires.
// Used to verify cookie forwarding through Async402ResourceHandler.
app.get('/paid-session', (req, res) => {
    const sessionToken = parseCookie(req, 'paid_session');
    if (isSessionValid(sessionToken)) {
        const remainingSec = Math.floor((sessionStore.get(sessionToken) - Date.now()) / 1000);
        console.log(`[200] /paid-session — VALID COOKIE token=${sessionToken.slice(0, 8)}... (${remainingSec}s remaining) — no 402, no payment`);
        res.set('content-type', 'text/html; charset=utf-8');
        res.send(`<!doctype html>
<html lang="en"><head><meta charset="utf-8"><title>BRC-121 session — cached</title></head>
<body style="font-family:system-ui;max-width:680px;margin:40px auto;padding:0 16px;background:#f0fdf4;">
  <h1>Served from session (no payment)</h1>
  <p>Your session cookie is still valid — <strong>no 402, no payment</strong>.</p>
  <p>Token: <code>${sessionToken.slice(0, 16)}...</code></p>
  <p>Time remaining: <strong>${remainingSec}s</strong></p>
  <p><a href="/paid-session">Reload</a> — also free until expiry.</p>
</body></html>`);
        return;
    }

    // No cookie or expired — fall through to BRC-121 payment.
    const result = validatePayment(req);
    if (result.needPayment) {
        console.log(`[402] /paid-session — challenge sent (${PRICE_SATS} sats → ${SERVER_PUBKEY_HEX.slice(0, 16)}...) [no valid cookie]`);
        res.set('x-bsv-sats', String(PRICE_SATS));
        res.set('x-bsv-server', SERVER_PUBKEY_HEX);
        res.status(402).send({ error: 'Payment required', priceSats: PRICE_SATS });
        return;
    }
    if (!result.ok) {
        console.error(`[${result.status}] /paid-session — ${result.error}`);
        res.status(result.status).send({ error: result.error });
        return;
    }

    // Payment accepted — issue session cookie.
    // HttpOnly so JS can't read it. SameSite=Lax so it travels on top-level
    // navigation. Not Secure (works on plain http://localhost; real deploys
    // add Secure). Path=/paid-session scopes it tightly.
    const token = newSessionToken();
    res.cookie('paid_session', token, {
        maxAge: SESSION_TTL_SECONDS * 1000,  // ms
        path: '/paid-session',
        httpOnly: true,
        sameSite: 'lax',
    });
    console.log(`[200] /paid-session — accepted txid=${result.txid.slice(0, 16)}... (${result.sats} sats) → issued session token=${token.slice(0, 8)}... (TTL ${SESSION_TTL_SECONDS}s)`);
    console.log(`        outgoing Set-Cookie: ${JSON.stringify(res.getHeader('set-cookie'))}`);
    console.log(`        incoming Cookie:     ${JSON.stringify(req.get('cookie') || '<none>')}`);
    res.set('content-type', 'text/html; charset=utf-8');
    res.send(`<!doctype html>
<html lang="en"><head><meta charset="utf-8"><title>BRC-121 session — paid</title></head>
<body style="font-family:system-ui;max-width:680px;margin:40px auto;padding:0 16px;">
  <h1>Payment accepted — session started</h1>
  <p>You paid <strong>${result.sats}</strong> sats. A session cookie was set; reloads are free for the next <strong>${SESSION_TTL_SECONDS}s</strong>.</p>
  <p>txid: <code>${result.txid}</code></p>
  <p>Session token: <code>${token.slice(0, 16)}...</code></p>
  <p><a href="/paid-session">Reload</a> — should serve free from cookie. After ${SESSION_TTL_SECONDS}s, payment will be required again.</p>
</body></html>`);
});

app.listen(PORT, () => {
    console.log(`╭─ BRC-121 demo server`);
    console.log(`│  port:        ${PORT}`);
    console.log(`│  price:       ${PRICE_SATS} sats`);
    console.log(`│  ttl:         ${SESSION_TTL_SECONDS}s (cookie variant)`);
    console.log(`│  server pub:  ${SERVER_PUBKEY_HEX}`);
    console.log(`│  per-request: http://localhost:${PORT}/paid`);
    console.log(`│  per-session: http://localhost:${PORT}/paid-session`);
    console.log(`│  health:      http://localhost:${PORT}/health`);
    console.log(`╰─ Open either path from Hodos to drive the round-trip.`);
});
