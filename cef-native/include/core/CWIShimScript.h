#pragma once

// Phase 2 — window.CWI / window.yours / window.panda shim injected via OnContextCreated.
//
// Construction:
//   window.CWI    — canonical 28-method BRC-100 WalletInterface from @bsv/sdk@2.0.13.
//                   Non-writable, non-configurable (matches Brave's window.ethereum pattern).
//   window.yours  — legacy translation layer. Step 1 ships canonical pass-through (same 28
//                   methods as CWI). Step 3 will overlay legacy-yours translations
//                   (sendBsv, signMessage, getAddresses, etc.) per SHIM_TRANSLATION_SPEC.
//                   Writable so competing wallet extensions can override.
//   window.panda  — alias to window.yours. Treechat still targets this name.
//
// Method dispatch:
//   Each method POSTs to http://127.0.0.1:31301/<methodName> with JSON-stringified args.
//   The Hodos CEF HTTP request interceptor (HttpRequestInterceptor::isWalletEndpoint)
//   captures the request on the IO thread, runs it through PermissionEngine::Decide,
//   and either forwards to the Rust wallet (Silent), queues for a notification overlay
//   prompt (Prompt), or returns 403 (Deny). The shim doesn't bypass any gate.
//
// Defensive wrapping:
//   Each method is wrapped in a V8 Proxy with an apply trap so
//   `const fn = window.CWI.createSignature; fn({...})` still binds correctly
//   (defends against "Illegal invocation" — Brave's lesson on detached references).
//
// Multi-provider discovery (bsv:announceProvider — BSV equivalent of EIP-6963):
//   Step 1 ships the listener + initial announcement so dApps that load late still
//   discover us. Step 4 finalizes the announcement payload (icon, rdns).
//
// Injection gating (caller must enforce):
//   - Only external pages (not internal Hodos UI, not overlay browsers).
//   - Only the main frame (skip all iframes — matches Yours Wallet + Brave default).
//   - Secure-context gating (https://, http://localhost) is Step 2; Step 1 allows any
//     external URL for first-pass smoke testing.
//
// Auto-approve posture for shim paths:
//   Step 5 adds a `via_legacy_shim` flag to shim-issued requests that forces Prompt for
//   any path that would otherwise auto-approve, regardless of domain whitelist. Step 1
//   ships the canonical pass-through unchanged — auto-approve behaves as it does for
//   any other BRC-100 caller until Step 5 lands.
//
// References:
//   - development-docs/Sigma-BRC121-Sprint/phase-2-window-cwi-shim/README.md
//   - development-docs/Sigma-BRC121-Sprint/phase-0.2-window-yours-shim-design/SHIM_TRANSLATION_SPEC.md
//   - development-docs/Sigma-BRC121-Sprint/YOURS_CWI_MIGRATION.md (canonical 28-method list)
//   - development-docs/Sigma-BRC121-Sprint/BRAVE_WALLET_REFERENCE.md (Proxy + descriptor patterns)

static const char* CWI_SHIM_SCRIPT = R"JS(
(function() {
    'use strict';
    if (typeof window === 'undefined') return;

    // Re-entry guard. If window.CWI is already injected as non-configurable, skip.
    try {
        var existing = Object.getOwnPropertyDescriptor(window, 'CWI');
        if (existing && !existing.configurable) return;
    } catch (e) {}

    var ENDPOINT_BASE = 'http://127.0.0.1:31301';

    // Canonical 28-method WalletInterface from @bsv/sdk@2.0.13.
    // Order mirrors AUDIT_RESULTS.md groups: identity, crypto, transactions, outputs,
    // certificates, auth, chain info.
    var METHODS = [
        // Identity & keys (3)
        'getPublicKey',
        'revealCounterpartyKeyLinkage',
        'revealSpecificKeyLinkage',
        // Crypto (6)
        'encrypt',
        'decrypt',
        'createHmac',
        'verifyHmac',
        'createSignature',
        'verifySignature',
        // Transactions (5)
        'createAction',
        'signAction',
        'abortAction',
        'listActions',
        'internalizeAction',
        // Outputs (2)
        'listOutputs',
        'relinquishOutput',
        // Certificates (6)
        'acquireCertificate',
        'listCertificates',
        'proveCertificate',
        'relinquishCertificate',
        'discoverByIdentityKey',
        'discoverByAttributes',
        // Auth (2)
        'isAuthenticated',
        'waitForAuthentication',
        // Chain info (4)
        'getHeight',
        'getHeaderForHeight',
        'getNetwork',
        'getVersion'
    ];

    function makeMethod(methodName) {
        var endpoint = ENDPOINT_BASE + '/' + methodName;
        function impl(args, originator) {
            // originator is implicit — backend reads window.location.host from the
            // Origin header on this fetch. Caller-supplied originator is ignored by
            // design; the canonical WalletInterface allows it but auth-gating uses Origin.
            var body = (args == null) ? {} : args;
            var bodyJson;
            try {
                bodyJson = JSON.stringify(body);
            } catch (e) {
                return Promise.reject(new Error('[Hodos][CWI] ' + methodName + ': args not JSON-serializable: ' + e.message));
            }
            return fetch(endpoint, {
                method: 'POST',
                mode: 'cors',
                credentials: 'omit',
                cache: 'no-store',
                headers: { 'Content-Type': 'application/json' },
                body: bodyJson
            }).then(function(r) {
                if (!r.ok) {
                    return r.text().then(function(t) {
                        var err = new Error('[Hodos][CWI] ' + methodName + ' failed: HTTP ' + r.status + (t ? (' ' + t) : ''));
                        err.status = r.status;
                        err.body = t;
                        throw err;
                    });
                }
                return r.text().then(function(t) {
                    if (!t) return null;
                    try { return JSON.parse(t); } catch (e) { return t; }
                });
            });
        }
        // Proxy with apply trap defends against detached references:
        //   const fn = window.CWI.createSignature; fn({...});
        // Without the trap, V8 binds `this` to undefined in strict mode and many wallet
        // implementations throw "Illegal invocation". Our impl doesn't use `this`, but
        // the wrapper future-proofs the shim against impls that might.
        return new Proxy(impl, {
            apply: function(target, thisArg, argumentsList) {
                return Reflect.apply(target, undefined, argumentsList);
            }
        });
    }

    function buildProvider() {
        var obj = Object.create(null);
        for (var i = 0; i < METHODS.length; i++) {
            var m = METHODS[i];
            Object.defineProperty(obj, m, {
                value: makeMethod(m),
                writable: false,
                configurable: false,
                enumerable: true
            });
        }
        return obj;
    }

    var canonicalProvider = buildProvider();
)JS"
// MSVC has a 16380-char hard limit on individual string literals; the JS bundle is
// split across adjacent raw-string literals here, which C++ auto-concatenates at
// compile time. No effect on the injected JS.
R"JS(
    // ====================================================================
    // Phase 2 Step 3 — window.yours legacy translation layer.
    //
    // Builds a legacy provider on top of canonicalProvider that exposes the
    // 16 legacy yours methods (mapped to canonical BRC-100 calls per
    // SHIM_TRANSLATION_SPEC.md) plus the canonical 28 (so BRC-100-aware code
    // targeting window.yours.createSignature still works).
    //
    // Convention: yours-legacy-v1. Published so other shim implementers can interop.
    // ====================================================================

    var LEGACY_ERR = {
        REMOVED: 'YOURS_LEGACY_REMOVED',
        NOT_IMPL: 'NOT_IMPLEMENTED_PRE_PHASE_3',
        DENIED: 'YOURS_LEGACY_USER_DENIED',
        INVALID_ENCODING: 'YOURS_LEGACY_INVALID_ENCODING',
        DATA_TOO_LARGE: 'YOURS_LEGACY_DATA_TOO_LARGE',
        REENTRANT: 'YOURS_LEGACY_REENTRANT',
        MULTI_RECIPIENT: 'YOURS_LEGACY_MULTI_RECIPIENT_UNSUPPORTED',
        DECRYPT_FAILED: 'YOURS_LEGACY_DECRYPT_FAILED',
        ADDRESS_DERIVATION_PENDING: 'YOURS_LEGACY_ADDRESS_DERIVATION_PENDING'
    };

    var YOURS_LEGACY_V1 = {
        SIG_PROTOCOL: [1, 'yours-legacy-message'],
        RECEIVE_PROTOCOL: [2, 'yours-legacy-receive'],
        ORD_RECEIVE_PROTOCOL: [2, 'yours-legacy-ord-receive'],
        ENCRYPT_PROTOCOL: [1, 'yours-legacy-encrypt'],
        KEY_ID: '1',
        COUNTERPARTY_ANYONE: 'anyone',
        COUNTERPARTY_SELF: 'self'
    };

    // R9 mitigation: cap data length below backend's effective max with a margin.
    // 1 MB minus 64-byte margin = generous for messages, rejects pathological inputs.
    var MAX_DATA_BYTES = (1024 * 1024) - 64;

    function typedError(code, methodName, message) {
        var err = new Error('[Hodos][yours] ' + methodName + ': ' + message);
        err.code = code;
        err.methodName = methodName;
        return err;
    }

    function warnDeprecated(methodName) {
        try {
            console.warn('[Hodos] window.yours.' + methodName +
                ' is deprecated. Please migrate to window.CWI.' + methodName +
                ' (or the equivalent BRC-100 method). See SHIM_TRANSLATION_SPEC.');
        } catch (e) {}
    }

    function encodeToBytes(input, encoding, methodName) {
        if (input == null) {
            throw typedError(LEGACY_ERR.INVALID_ENCODING, methodName, 'message is null');
        }
        if (typeof input !== 'string') {
            throw typedError(LEGACY_ERR.INVALID_ENCODING, methodName, 'message must be a string');
        }
        var enc = (encoding == null) ? 'utf8' : String(encoding).toLowerCase();
        var bytes;
        if (enc === 'utf8' || enc === 'utf-8') {
            bytes = Array.from(new TextEncoder().encode(input));
        } else if (enc === 'hex') {
            if (input.length % 2 !== 0) {
                throw typedError(LEGACY_ERR.INVALID_ENCODING, methodName, 'hex string must have even length');
            }
            bytes = [];
            for (var i = 0; i < input.length; i += 2) {
                var v = parseInt(input.substr(i, 2), 16);
                if (isNaN(v)) {
                    throw typedError(LEGACY_ERR.INVALID_ENCODING, methodName, 'invalid hex character at position ' + i);
                }
                bytes.push(v);
            }
        } else if (enc === 'base64') {
            try {
                var bin = atob(input);
                bytes = [];
                for (var j = 0; j < bin.length; j++) bytes.push(bin.charCodeAt(j));
            } catch (e) {
                throw typedError(LEGACY_ERR.INVALID_ENCODING, methodName, 'invalid base64: ' + e.message);
            }
        } else {
            throw typedError(LEGACY_ERR.INVALID_ENCODING, methodName, 'unsupported encoding: ' + enc);
        }
        if (bytes.length > MAX_DATA_BYTES) {
            throw typedError(LEGACY_ERR.DATA_TOO_LARGE, methodName,
                'encoded data ' + bytes.length + ' bytes exceeds cap ' + MAX_DATA_BYTES);
        }
        return bytes;
    }

    function bytesToHex(bytes) {
        if (!bytes) return '';
        if (typeof bytes === 'string') return bytes;
        var s = '';
        for (var i = 0; i < bytes.length; i++) {
            var h = (bytes[i] & 0xff).toString(16);
            if (h.length < 2) h = '0' + h;
            s += h;
        }
        return s;
    }

    function hexToBytes(hex) {
        if (!hex || typeof hex !== 'string') return [];
        var bytes = [];
        for (var i = 0; i < hex.length; i += 2) bytes.push(parseInt(hex.substr(i, 2), 16));
        return bytes;
    }

    function bytesToBase64(bytes) {
        if (!bytes) return '';
        if (typeof bytes === 'string') return bytes;
        var bin = '';
        for (var i = 0; i < bytes.length; i++) bin += String.fromCharCode(bytes[i] & 0xff);
        try { return btoa(bin); } catch (e) { return ''; }
    }

    function bytesToText(bytes) {
        if (!bytes) return '';
        if (typeof bytes === 'string') return bytes;
        try { return new TextDecoder().decode(new Uint8Array(bytes)); }
        catch (e) { return ''; }
    }

    // R9 mitigation: per-frame mutex on legacy methods. One in-flight legacy promise
    // at a time; concurrent calls reject with YOURS_LEGACY_REENTRANT. Local to this
    // injection (each frame gets its own _inFlight via IIFE scope).
    var _inFlight = null;
    function withMutex(methodName, fn) {
        if (_inFlight) {
            return Promise.reject(typedError(LEGACY_ERR.REENTRANT, methodName, 'another legacy call is in flight'));
        }
        var p;
        try {
            p = Promise.resolve(fn());
        } catch (e) {
            return Promise.reject(e);
        }
        _inFlight = p;
        return p.then(
            function(v) { _inFlight = null; return v; },
            function(e) { _inFlight = null; throw e; }
        );
    }

    function makeLegacyMethod(impl) {
        return new Proxy(impl, {
            apply: function(target, thisArg, argumentsList) {
                return Reflect.apply(target, undefined, argumentsList);
            }
        });
    }

    function defineLegacyProp(obj, name, value) {
        Object.defineProperty(obj, name, {
            value: value,
            writable: false,
            configurable: false,
            enumerable: true
        });
    }

    function buildLegacyProvider(canonical) {
        var legacy = Object.create(null);

        // Pass-through: all 28 canonical methods on window.yours too, so BRC-100-aware
        // sites targeting window.yours.createSignature etc. continue to work.
        for (var i = 0; i < METHODS.length; i++) {
            defineLegacyProp(legacy, METHODS[i], canonical[METHODS[i]]);
        }

        // ----- isReady: local boolean -----
        defineLegacyProp(legacy, 'isReady', true);

        // ----- isConnected(): boolean (still routes through canonical auth gate) -----
        defineLegacyProp(legacy, 'isConnected', makeLegacyMethod(function() {
            warnDeprecated('isConnected');
            return canonical.isAuthenticated({}).then(function(r) {
                return !!(r && r.authenticated);
            });
        }));

        // ----- disconnect(): revoke this origin's domain permission -----
        defineLegacyProp(legacy, 'disconnect', makeLegacyMethod(function() {
            warnDeprecated('disconnect');
            var origin = window.location.host;
            return fetch(ENDPOINT_BASE + '/domain/permissions?domain=' + encodeURIComponent(origin), {
                method: 'DELETE',
                mode: 'cors',
                credentials: 'omit'
            }).then(function(r) { return r.ok; });
        }));

        // ----- connect(): open auth flow, return { identityKey, addresses } -----
        // Addresses placeholder until Step 3b address-derivation polish.
        defineLegacyProp(legacy, 'connect', makeLegacyMethod(function() {
            warnDeprecated('connect');
            return withMutex('connect', function() {
                return canonical.waitForAuthentication({}).then(function(authResult) {
                    if (!authResult || authResult.authenticated !== true) {
                        throw typedError(LEGACY_ERR.DENIED, 'connect', 'user denied authentication');
                    }
                    return canonical.getPublicKey({ identityKey: true }).then(function(idResult) {
                        var idKey = idResult && (idResult.publicKey || idResult.identityKey);
                        return {
                            identityKey: idKey || null,
                            // Legacy shape: addresses object with three pubkey-derived addresses.
                            // Step 3 ships placeholders (raw pubkeys + console-warn). Step 3b will
                            // bundle Base58Check encoding so these become real BSV addresses.
                            addresses: {
                                bsvAddress: null,
                                ordAddress: null,
                                identityAddress: null
                            }
                        };
                    });
                });
            });
        }));

        // ----- getAddresses(): placeholder; returns three null slots with deprecation warning -----
        // Spec calls for BRC-42 fresh-address generator with per-origin keyID. The shim has
        // the public keys (canonical.getPublicKey works) but encoding pubkey → P2PKH BSV
        // address needs ripemd160 + Base58Check, which is ~250 lines of JS crypto. Step 3b
        // either bundles that or adds a Rust /wallet/derive-address helper endpoint. For
        // first Step 3 commit, return null slots — Treechat displays the user's identity-key
        // pubkey separately and tolerates missing addresses. Logs a clear deprecation note.
        defineLegacyProp(legacy, 'getAddresses', makeLegacyMethod(function() {
            warnDeprecated('getAddresses');
            try {
                console.warn('[Hodos] window.yours.getAddresses: address derivation arrives in Step 3b. ' +
                    'For now this returns null slots. Use window.CWI.getPublicKey for raw pubkeys.');
            } catch (e) {}
            return Promise.resolve({
                bsvAddress: null,
                ordAddress: null,
                identityAddress: null
            });
        }));

        // ----- getPubKeys(): legacy { bsvPubKey, ordPubKey, identityPubKey } shape -----
        defineLegacyProp(legacy, 'getPubKeys', makeLegacyMethod(function() {
            warnDeprecated('getPubKeys');
            var originKey = 'yours-' + window.location.host;
            return Promise.all([
                canonical.getPublicKey({
                    protocolID: YOURS_LEGACY_V1.RECEIVE_PROTOCOL,
                    keyID: originKey,
                    counterparty: YOURS_LEGACY_V1.COUNTERPARTY_SELF
                }).catch(function() { return null; }),
                canonical.getPublicKey({
                    protocolID: YOURS_LEGACY_V1.ORD_RECEIVE_PROTOCOL,
                    keyID: originKey,
                    counterparty: YOURS_LEGACY_V1.COUNTERPARTY_SELF
                }).catch(function() { return null; }),
                canonical.getPublicKey({ identityKey: true }).catch(function() { return null; })
            ]).then(function(keys) {
                return {
                    bsvPubKey: keys[0] && keys[0].publicKey,
                    ordPubKey: keys[1] && keys[1].publicKey,
                    identityPubKey: keys[2] && (keys[2].publicKey || keys[2].identityKey)
                };
            });
        }));
)JS"
R"JS(
        // ----- getBalance(): listOutputs sum + USD price -----
        defineLegacyProp(legacy, 'getBalance', makeLegacyMethod(function() {
            warnDeprecated('getBalance');
            return Promise.all([
                canonical.listOutputs({ basket: 'default' }),
                fetch(ENDPOINT_BASE + '/wallet/bsv-price', {
                    method: 'GET',
                    mode: 'cors',
                    credentials: 'omit'
                }).then(function(r) { return r.ok ? r.json() : null; }).catch(function() { return null; })
            ]).then(function(results) {
                var outputs = (results[0] && results[0].outputs) || [];
                var sats = 0;
                for (var k = 0; k < outputs.length; k++) {
                    var s = outputs[k].satoshis;
                    if (typeof s === 'number') sats += s;
                }
                var bsv = sats / 1e8;
                var price = results[1];
                var usdInCents = null;
                if (price) {
                    var priceUsd = (typeof price.usd === 'number') ? price.usd :
                                   (typeof price.priceUsd === 'number') ? price.priceUsd :
                                   (typeof price.price === 'number') ? price.price : null;
                    if (priceUsd != null) usdInCents = Math.round(bsv * priceUsd * 100);
                }
                return { bsv: bsv, satoshis: sats, usdInCents: usdInCents };
            });
        }));

        // ----- signMessage({ message, encoding? }): yours-legacy-v1 signature -----
        defineLegacyProp(legacy, 'signMessage', makeLegacyMethod(function(opts) {
            warnDeprecated('signMessage');
            opts = opts || {};
            return withMutex('signMessage', function() {
                var data;
                try {
                    data = encodeToBytes(opts.message, opts.encoding, 'signMessage');
                } catch (e) {
                    return Promise.reject(e);
                }
                return canonical.createSignature({
                    data: data,
                    protocolID: YOURS_LEGACY_V1.SIG_PROTOCOL,
                    keyID: YOURS_LEGACY_V1.KEY_ID,
                    counterparty: YOURS_LEGACY_V1.COUNTERPARTY_ANYONE
                }).then(function(result) {
                    var sig = result && (result.signature || result.sig);
                    return canonical.getPublicKey({
                        protocolID: YOURS_LEGACY_V1.SIG_PROTOCOL,
                        keyID: YOURS_LEGACY_V1.KEY_ID,
                        counterparty: YOURS_LEGACY_V1.COUNTERPARTY_ANYONE,
                        forSelf: true
                    }).then(function(pk) {
                        return {
                            signature: bytesToHex(sig),
                            publicKey: pk && pk.publicKey
                        };
                    });
                });
            });
        }));

        // ----- verifyLegacyMessage(message, signature, identityKey): ecosystem-interop helper -----
        defineLegacyProp(legacy, 'verifyLegacyMessage', makeLegacyMethod(function(message, signature, identityKey) {
            var data;
            try {
                data = encodeToBytes(message, 'utf8', 'verifyLegacyMessage');
            } catch (e) {
                return Promise.reject(e);
            }
            var sigBytes = (typeof signature === 'string') ? hexToBytes(signature) : signature;
            return canonical.verifySignature({
                data: data,
                signature: sigBytes,
                protocolID: YOURS_LEGACY_V1.SIG_PROTOCOL,
                keyID: YOURS_LEGACY_V1.KEY_ID,
                counterparty: YOURS_LEGACY_V1.COUNTERPARTY_ANYONE,
                forSelf: false
            }).then(function(r) { return !!(r && r.valid); });
        }));

        // ----- getSignatures: stub w/ typed error pointing to canonical createAction+signAction -----
        defineLegacyProp(legacy, 'getSignatures', makeLegacyMethod(function() {
            warnDeprecated('getSignatures');
            return Promise.reject(typedError(
                LEGACY_ERR.NOT_IMPL, 'getSignatures',
                'translation not yet implemented. Use window.CWI.createAction({inputs, outputs}) ' +
                'followed by window.CWI.signAction({reference, spends}) — see SHIM_TRANSLATION_SPEC.'
            ));
        }));

        // ----- sendBsv: stub w/ typed error (P2PKH script construction lands in Step 3b) -----
        defineLegacyProp(legacy, 'sendBsv', makeLegacyMethod(function(payments) {
            warnDeprecated('sendBsv');
            return Promise.reject(typedError(
                LEGACY_ERR.NOT_IMPL, 'sendBsv',
                'plain-address translation lands in Step 3b. ' +
                'For now use window.CWI.createAction({outputs: [{satoshis, lockingScript: <P2PKH hex>}]}).'
            ));
        }));

        // ----- broadcast({ rawtx }): try internalizeAction, fall back to /wallet/broadcast -----
        defineLegacyProp(legacy, 'broadcast', makeLegacyMethod(function(opts) {
            warnDeprecated('broadcast');
            opts = opts || {};
            if (!opts.rawtx) {
                return Promise.reject(typedError(LEGACY_ERR.INVALID_ENCODING, 'broadcast', 'missing rawtx'));
            }
            return withMutex('broadcast', function() {
                return canonical.internalizeAction({
                    tx: opts.rawtx,
                    description: 'window.yours.broadcast'
                }).then(function(r) {
                    return { txid: r && r.txid };
                }, function() {
                    return fetch(ENDPOINT_BASE + '/wallet/broadcast', {
                        method: 'POST',
                        mode: 'cors',
                        credentials: 'omit',
                        headers: { 'Content-Type': 'application/json' },
                        body: JSON.stringify({ rawtx: opts.rawtx })
                    }).then(function(r) {
                        return r.json().then(function(j) {
                            if (!r.ok) {
                                throw typedError(LEGACY_ERR.NOT_IMPL, 'broadcast',
                                    'fallback /wallet/broadcast failed: ' + (j && j.error));
                            }
                            return { txid: j && j.txid };
                        });
                    });
                });
            });
        }));

        // ----- encrypt / decrypt: REJECTED — algorithm mismatch with canonical BRC-100 -----
        //
        // R3 finding (David, 2026-05-28, via Agent task): legacy yours-wallet@v4.5.6
        // encrypt/decrypt use ECIES Electrum (@bsv/sdk's ECIES.electrumEncrypt, "BIE1"
        // format: magic + ephemeral pubkey + AES-CBC ciphertext + HMAC). The canonical
        // BRC-100 encrypt uses BRC-2 (BRC-42 ECDH + AES-256-GCM). These are wire-
        // incompatible cryptographic schemes — same primitive name, different bytes.
        //
        // Multi-recipient was real (NOT vestigial): legacy yours mapped over pubKeys[]
        // producing N independent ECIES ciphertexts.
        //
        // Reasoned conclusion: silently routing window.yours.encrypt → canonical
        // window.CWI.encrypt would produce ciphertexts that ARE NOT decryptable by any
        // existing Yours-era ciphertext consumer, and that the wallet itself could not
        // decrypt if a dApp ever stored one and tried to read it back via legacy decrypt.
        // The "translation" would silently produce a different cryptosystem under the
        // same method name — exactly the kind of silent-divergence trap the shim is
        // supposed to prevent (cf. SHIM_TRANSLATION_SPEC §"Design posture").
        //
        // Step 3 ships explicit typed-error rejection. Two real follow-up paths exist:
        //   (A) Add a Rust `encrypt_ecies_electrum` / `decrypt_ecies_electrum` handler
        //       wrapping @bsv/sdk's ECIES.electrumEncrypt (estimate ~200-400 LOC Rust).
        //   (B) Bundle a JS ECIES Electrum implementation in this shim (needs secp256k1
        //       in JS — noble-secp256k1 or equivalent, ~80KB bundle bloat).
        // Decide separately from Phase 2 — not a Step 3 footnote.
        //
        // For new ciphertexts dApps should use canonical window.CWI.encrypt (BRC-2).
        function eciesElectrumNotImplemented(name) {
            return makeLegacyMethod(function() {
                warnDeprecated(name);
                return Promise.reject(typedError(
                    LEGACY_ERR.NOT_IMPL, name,
                    'legacy yours.' + name + ' uses ECIES Electrum (BIE1 format) which is not yet ' +
                    'implemented in the Hodos backend. The canonical window.CWI.' + name +
                    ' uses BRC-2 (AES-GCM) which is wire-incompatible. For NEW data use ' +
                    'window.CWI.' + name + '; legacy ECIES Electrum compat is tracked separately.'
                ));
            });
        }
        defineLegacyProp(legacy, 'encrypt', eciesElectrumNotImplemented('encrypt'));
        defineLegacyProp(legacy, 'decrypt', eciesElectrumNotImplemented('decrypt'));

        // ----- Removed methods: typed REMOVED error -----
        function removed(name, hint) {
            return makeLegacyMethod(function() {
                return Promise.reject(typedError(LEGACY_ERR.REMOVED, name,
                    'removed in BRC-100. ' + hint));
            });
        }
        defineLegacyProp(legacy, 'getExchangeRate', removed('getExchangeRate',
            'Fetch BSV/USD from a public price source (CryptoCompare, CoinGecko).'));
        defineLegacyProp(legacy, 'getSocialProfile', removed('getSocialProfile',
            'Use BRC-100 acquireCertificate + Sigma OAuth provider for identity profile.'));

        // ----- Ordinal methods: typed NOT_IMPLEMENTED error pointing to Phase 3 -----
        function notImpl(name) {
            return makeLegacyMethod(function() {
                return Promise.reject(typedError(LEGACY_ERR.NOT_IMPL, name,
                    'ordinal methods are deferred to Phase 3 (1Sat Ordinals support). ' +
                    'Use window.CWI.createAction with the appropriate 1Sat templates.'));
            });
        }
        defineLegacyProp(legacy, 'inscribe', notImpl('inscribe'));
        defineLegacyProp(legacy, 'transferOrdinal', notImpl('transferOrdinal'));
        defineLegacyProp(legacy, 'purchaseOrdinal', notImpl('purchaseOrdinal'));

        return legacy;
    }

    var legacyProvider = buildLegacyProvider(canonicalProvider);

    // window.CWI — canonical, locked. Brave-style descriptor.
    try {
        Object.defineProperty(window, 'CWI', {
            value: canonicalProvider,
            writable: false,
            configurable: false,
            enumerable: true
        });
    } catch (e) {
        console.warn('[Hodos] Failed to install window.CWI:', e);
        return;
    }

    // window.yours — legacy translation layer (Step 3). Canonical 28 methods + 16 legacy.
    // Writable per spec — competing extensions may override.
    try {
        window.yours = legacyProvider;
    } catch (e) {
        console.warn('[Hodos] Failed to install window.yours:', e);
    }

    // window.panda — alias to window.yours. Treechat still targets this name.
    // Same writable posture.
    try {
        window.panda = window.yours;
    } catch (e) {
        console.warn('[Hodos] Failed to install window.panda:', e);
    }

    // bsv:announceProvider — multi-provider discovery (BSV equivalent of EIP-6963).
    // Step 1 ships the listener + initial announce; Step 4 finalizes icon + rdns + the
    // BRC submission. Sending an early unprompted announce matches EIP-6963 cadence so
    // dApps that load their listener after page-load still receive us.
    try {
        var ANNOUNCEMENT = {
            info: {
                uuid: 'hodos-' + Date.now() + '-' + Math.random().toString(36).slice(2),
                name: 'Hodos',
                icon: 'data:image/svg+xml;base64,', // TODO Step 4: embed Hodos_Gold_Wallet_Icon
                rdns: 'browser.hodos'
            },
            provider: window.CWI
        };
        function announce() {
            try {
                window.dispatchEvent(new CustomEvent('bsv:announceProvider', { detail: ANNOUNCEMENT }));
            } catch (e) {}
        }
        window.addEventListener('bsv:requestProvider', announce);
        announce();
    } catch (e) {
        console.warn('[Hodos] bsv:announceProvider scaffolding failed:', e);
    }

    try {
        console.info('[Hodos] window.CWI / window.yours / window.panda injected (28 BRC-100 methods, Phase 2 Step 1).');
    } catch (e) {}
})();
)JS";
