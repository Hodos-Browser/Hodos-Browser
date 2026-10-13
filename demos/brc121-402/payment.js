import { KeyDeriver, P2PKH, PublicKey, Transaction } from '@bsv/sdk';

const BRC29_PROTOCOL_ID = [2, '3241645161d8'];
const DEFAULT_PAYMENT_WINDOW_MS = 30_000;

function header(headers, name) {
    const value = headers[name];
    return Array.isArray(value) ? value[0] : value;
}

function canonicalBase64(value) {
    if (typeof value !== 'string' || value.length === 0 || value.length > 4096) return null;
    if (!/^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/.test(value)) return null;
    const decoded = Buffer.from(value, 'base64');
    return decoded.toString('base64') === value ? decoded : null;
}

/** Validate a canonical BRC-121 retry and bind its payment to this server. */
export function validatePaymentHeaders(headers, options) {
    const beefB64 = header(headers, 'x-bsv-beef');
    const sender = header(headers, 'x-bsv-sender');
    const nonce = header(headers, 'x-bsv-nonce');
    const time = header(headers, 'x-bsv-time');
    const voutText = header(headers, 'x-bsv-vout');
    if (!beefB64 && !sender && !nonce && !time && !voutText) return { ok: false, needPayment: true };
    if (!beefB64 || !sender || !nonce || !time || !voutText) {
        return { ok: false, status: 400, error: 'Incomplete BRC-121 payment headers' };
    }

    try {
        const beef = canonicalBase64(beefB64);
        const nonceBytes = canonicalBase64(nonce);
        if (!beef) return { ok: false, status: 400, error: 'Invalid x-bsv-beef encoding' };
        if (!nonceBytes || nonceBytes.length < 8 || nonceBytes.length > 64) {
            return { ok: false, status: 400, error: 'Invalid x-bsv-nonce encoding' };
        }

        const senderKey = PublicKey.fromString(sender);
        if (senderKey.toString() !== sender.toLowerCase()) {
            return { ok: false, status: 400, error: 'x-bsv-sender must be a canonical compressed public key' };
        }

        if (!/^(0|[1-9][0-9]*)$/.test(voutText)) {
            return { ok: false, status: 400, error: `Invalid x-bsv-vout '${voutText}'` };
        }
        const vout = Number(voutText);
        if (!Number.isSafeInteger(vout)) return { ok: false, status: 400, error: 'x-bsv-vout is too large' };

        if (!/^[0-9]+$/.test(time)) return { ok: false, status: 400, error: 'Invalid x-bsv-time' };
        const timestamp = Number(time);
        const now = options.now?.() ?? Date.now();
        const paymentWindowMs = options.paymentWindowMs ?? DEFAULT_PAYMENT_WINDOW_MS;
        if (!Number.isSafeInteger(timestamp) || Math.abs(now - timestamp) > paymentWindowMs) {
            return { ok: false, status: 400, error: 'Expired x-bsv-time' };
        }

        const tx = Transaction.fromAtomicBEEF([...beef]);
        const txid = tx.id('hex');
        if (options.acceptedTxids.has(txid)) {
            return { ok: false, status: 409, error: 'Replay: BEEF already accepted', txid };
        }

        const payOut = tx.outputs[vout];
        if (!payOut) return { ok: false, status: 400, error: `BEEF has no output at vout=${vout}` };
        if (!Number.isSafeInteger(payOut.satoshis) || payOut.satoshis < options.priceSats) {
            return { ok: false, status: 400, error: `Insufficient payment at vout=${vout}` };
        }

        const keyID = `${nonce} ${Buffer.from(time, 'utf8').toString('base64')}`;
        const receiverKey = new KeyDeriver(options.serverPrivateKey)
            .derivePrivateKey(BRC29_PROTOCOL_ID, keyID, senderKey);
        const expectedScript = new P2PKH().lock(receiverKey.toAddress()).toHex();
        if (payOut.lockingScript.toHex() !== expectedScript) {
            return { ok: false, status: 400, error: 'Payment output is not locked to this server' };
        }

        options.acceptedTxids.add(txid);
        return { ok: true, txid, sats: payOut.satoshis, vout };
    } catch (error) {
        return { ok: false, status: 400, error: `Invalid AtomicBEEF: ${error instanceof Error ? error.message : String(error)}` };
    }
}
