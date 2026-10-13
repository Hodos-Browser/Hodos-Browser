import assert from 'node:assert/strict';
import test from 'node:test';
import { KeyDeriver, P2PKH, PrivateKey, Transaction } from '@bsv/sdk';
import { validatePaymentHeaders } from './payment.js';

const NOW = 1_800_000_000_000;
const serverPrivateKey = PrivateKey.fromRandom();
const senderPrivateKey = PrivateKey.fromRandom();
const sender = senderPrivateKey.toPublicKey();
const nonce = Buffer.from('12345678').toString('base64');
const time = String(NOW);

function paymentHeaders(recipientPrivateKey = serverPrivateKey) {
    const keyID = `${nonce} ${Buffer.from(time).toString('base64')}`;
    const derived = new KeyDeriver(recipientPrivateKey)
        .derivePrivateKey([2, '3241645161d8'], keyID, sender);
    const tx = new Transaction();
    tx.addOutput({ satoshis: 100, lockingScript: new P2PKH().lock(derived.toAddress()) });
    return {
        'x-bsv-beef': Buffer.from(tx.toAtomicBEEF()).toString('base64'),
        'x-bsv-sender': sender.toString(),
        'x-bsv-nonce': nonce,
        'x-bsv-time': time,
        'x-bsv-vout': '0',
    };
}

function validate(headers) {
    return validatePaymentHeaders(headers, {
        acceptedTxids: new Set(),
        now: () => NOW,
        priceSats: 100,
        serverPrivateKey,
    });
}

test('accepts an AtomicBEEF payment locked to the server-derived BRC-29 key', () => {
    assert.equal(validate(paymentHeaders()).ok, true);
});

test('rejects an equal-value output locked to an attacker key', () => {
    const result = validate(paymentHeaders(PrivateKey.fromRandom()));
    assert.equal(result.ok, false);
    assert.match(result.error, /not locked to this server/);
});

test('rejects legacy single-header and incomplete retries', () => {
    const headers = paymentHeaders();
    assert.equal(validate({ 'x-bsv-payment': headers['x-bsv-beef'] }).ok, false);
    delete headers['x-bsv-sender'];
    assert.equal(validate(headers).ok, false);
});

test('rejects stale and non-atomic payment proofs', () => {
    const stale = paymentHeaders();
    stale['x-bsv-time'] = String(NOW - 30_001);
    assert.equal(validate(stale).ok, false);

    const raw = new Transaction();
    raw.addOutput({ satoshis: 100, lockingScript: new P2PKH().lock(serverPrivateKey.toAddress()) });
    const malformed = paymentHeaders();
    malformed['x-bsv-beef'] = Buffer.from(raw.toBEEF()).toString('base64');
    assert.equal(validate(malformed).ok, false);
});
