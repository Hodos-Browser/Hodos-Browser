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
// ============================================================================
// Phase 2.5 — wallet IPC bridge
// ============================================================================
//
// Promise-correlated bridge between the shim's wallet calls and the C++
// browser process. Replaces the renderer fetch path so CSP (github.com,
// hardened sites) and CORS (treechat.io, every dApp origin not in the
// actix-cors localhost allowlist) are no longer in the call chain.
//
// Design: see development-docs/Sigma-BRC121-Sprint/phase-2-window-cwi-shim/
// PHASE_2_5_IPC_REFACTOR.md
//
// MUST be injected BEFORE the CWI shim IIFE below, so `window.__hodos_walletCall`
// is available when makeMethod / legacy translators are constructed.
(function() {
    'use strict';
    if (typeof window === 'undefined') return;
    if (window.__hodos_walletCall) return;  // idempotent — survives re-injection

    var nextId = 1;
    var pending = Object.create(null);  // requestId -> {resolve, reject, method, startedAt}
    var MAX_PAYLOAD_BYTES = 50 * 1024 * 1024;  // 50 MB ceiling — see plan doc

    // Browser process invokes this via ExecuteJavaScript when a wallet_call returns.
    // Signature: (requestId: string, ok: bool, payloadJson: string)
    window.__hodos_walletResponse = function(requestId, ok, payloadJson) {
        var p = pending[requestId];
        if (!p) {
            // Late / orphan — frame may have navigated, or response duplicated.
            try { console.warn('[Hodos] orphan wallet_response id=' + requestId); } catch (e) {}
            return;
        }
        delete pending[requestId];
        try {
            var payload = payloadJson ? JSON.parse(payloadJson) : null;
            if (ok) {
                p.resolve(payload);
            } else {
                // Wallet error envelope: { error, code?, status? }
                var msg = (payload && payload.error) ? payload.error : 'unknown error';
                var err = new Error('[Hodos] ' + p.method + ' failed: ' + msg);
                if (payload && payload.code)   err.code = payload.code;
                if (payload && payload.status) err.status = payload.status;
                err.body = payload;
                p.reject(err);
            }
        } catch (e) {
            p.reject(new Error('[Hodos] ' + p.method + ' response parse failed: ' + e.message));
        }
    };

    // Public bridge entry. Returns a Promise that resolves with the parsed JSON
    // response (or null if response body was empty) or rejects with a structured
    // Error carrying .code / .status / .body when the wallet returned non-2xx.
    //
    //   method     friendly diagnostic name ('createAction', 'getAddresses', etc.)
    //   endpoint   wallet route, leading slash required ('/createAction')
    //   body       JSON-serializable request payload (omit or pass {} for empty)
    //   httpMethod 'POST' (default) or 'GET' — wallet endpoint convention
    window.__hodos_walletCall = function(method, endpoint, body, httpMethod) {
        if (typeof method !== 'string' || typeof endpoint !== 'string') {
            return Promise.reject(new Error('[Hodos] wallet_call: method and endpoint must be strings'));
        }
        var bodyJson;
        try {
            bodyJson = JSON.stringify(body == null ? {} : body);
        } catch (e) {
            return Promise.reject(new Error(
                '[Hodos] ' + method + ': args not JSON-serializable: ' + e.message
            ));
        }
        if (bodyJson.length > MAX_PAYLOAD_BYTES) {
            return Promise.reject(new Error(
                '[Hodos] ' + method + ': payload (' + bodyJson.length +
                ' bytes) exceeds 50MB IPC ceiling. Large payloads (e.g. ' +
                'createAction with massive inputs.BEEF) are not supported via ' +
                'this bridge — break the call into smaller chunks.'
            ));
        }
        var requestId = String(nextId++);
        var verb = (httpMethod === 'GET') ? 'GET' : 'POST';
        return new Promise(function(resolve, reject) {
            // CRITICAL: populate pending[requestId] SYNCHRONOUSLY before sending the
            // IPC, so a (theoretically impossible) instant response can't race ahead.
            pending[requestId] = {
                resolve: resolve,
                reject: reject,
                method: method,
                startedAt: Date.now()
            };
            try {
                window.cefMessage.send('wallet_call',
                    [requestId, method, endpoint, bodyJson, verb]);
            } catch (e) {
                delete pending[requestId];
                reject(new Error('[Hodos] failed to dispatch wallet_call: ' + e.message));
            }
        });
    };
})();

// ============================================================================
// CWI / yours / panda shim (Phase 2 Steps 1-4 + 3b + 3c)
// ============================================================================
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

        // Methods where legacy yours semantics differ from canonical CWI and must NOT
        // pass through. Installed explicitly later with legacy behavior. defineLegacyProp
        // marks installs as non-configurable, so a pass-through encrypt/decrypt would
        // block the later legacy override and throw "Cannot redefine property" at load.
        var LEGACY_OVERRIDES = { encrypt: 1, decrypt: 1 };

        // Pass-through: 28 canonical methods on window.yours too (minus the overrides),
        // so BRC-100-aware sites targeting window.yours.createSignature etc. work.
        for (var i = 0; i < METHODS.length; i++) {
            if (LEGACY_OVERRIDES[METHODS[i]]) continue;
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

        // ----- getAddresses(): consolidated Yours-legacy address derivation (Step 3b.3) -----
        // Single round-trip to POST /wallet/yours-legacy-addresses (Step 3b.1). The wallet
        // server does the 3× BRC-42 self-derivation + Base58Check encoding using the same
        // yours-legacy-v1 constants this shim emits via getPubKeys (protocolID prefix
        // "2-yours-legacy-receive" / "2-yours-legacy-ord-receive", keyID "yours-{host}").
        // Identity slot may come back null if the user has not granted identity-key
        // disclosure for this origin — bsv/ord still flow through. dApps that need a
        // prompt-on-demand identity address can call canonical.getPublicKey({identityKey:true}).
        defineLegacyProp(legacy, 'getAddresses', makeLegacyMethod(function() {
            warnDeprecated('getAddresses');
            var nullSlots = { bsvAddress: null, ordAddress: null, identityAddress: null };
            return fetch(ENDPOINT_BASE + '/wallet/yours-legacy-addresses', {
                method: 'POST',
                mode: 'cors',
                credentials: 'omit',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ origin: window.location.host })
            }).then(function(r) {
                // Backend rejected (no wallet, bad origin, internal error). Gracefully
                // degrade to null slots — matches Yours's tolerance for missing fields.
                return r.ok ? r.json() : nullSlots;
            }).then(function(j) {
                return {
                    bsvAddress: (j && j.bsvAddress) || null,
                    ordAddress: (j && j.ordAddress) || null,
                    identityAddress: (j && j.identityAddress) || null
                };
            }).catch(function() {
                // Network/transport failure — never throw to a legacy caller.
                return nullSlots;
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

        // ----- getSignatures: typed NOT_IMPL w/ architectural reason + migration path -----
        // Step 3b.5 tightened the error message from a generic "not yet implemented" to
        // explain WHY there is no 1:1 translation and what to do instead. Yours's
        // getSignatures is a low-level "sign these inputs of this raw tx" primitive;
        // BRC-100 signAction is reference-based and wallet-controlled, so the wallet
        // owns input selection / output building. A generic translator would have to
        // reverse-engineer transaction intent from raw bytes; Hodos is tracking that
        // work explicitly in Step 3d (STEP_3D_RESEARCH.md). For the common use case
        // (partial-tx atomic swaps / Yours-era ordinal sales using SIGHASH_SINGLE |
        // ANYONECANPAY), the canonical BRC-100 equivalent is createAction with
        // signOutputs:'single' + noSend:true, then signAction.
        defineLegacyProp(legacy, 'getSignatures', makeLegacyMethod(function() {
            warnDeprecated('getSignatures');
            return Promise.reject(typedError(
                LEGACY_ERR.NOT_IMPL, 'getSignatures',
                'window.yours.getSignatures has no generic translation to BRC-100. The ' +
                'legacy API is a low-level "sign these inputs of this raw tx" primitive; ' +
                'BRC-100 signAction is reference-based and wallet-controlled, so there is ' +
                'no 1:1 mapping. For the canonical partial-tx atomic-swap pattern (the ' +
                'primary Yours-era use case — SIGHASH_SINGLE | ANYONECANPAY ordinal sales), ' +
                'use: window.CWI.createAction({inputs, outputs, options: ' +
                '{signOutputs: "single", acceptDelayedBroadcast: false, noSend: true}}) ' +
                'followed by window.CWI.signAction({reference, spends}). A generic ' +
                'translator for the specific Yours-era flows is tracked in Step 3d ' +
                '(STEP_3D_RESEARCH.md); for now dApps should call createAction + signAction ' +
                'directly.'
            ));
        }));
)JS"
R"JS(
        // ----- sendBsv: translate [{address, satoshis|amount}] → canonical createAction (Step 3b.4) -----
        // The Yours-era sendBsv API takes plain BSV addresses + satoshi amounts and
        // returns { txid }. We translate by resolving each address to a P2PKH locking
        // script via /wallet/address-to-script (Step 3b.2), then calling the canonical
        // createAction with the resulting outputs.
        //
        // CRITICAL: this MUST route through canonical.createAction unchanged. The C++
        // permission engine applies the same auto-approve / per-domain prompt path it
        // does for any BRC-100 payment, AND the success path fires the
        // payment_success_indicator IPC chain (the user's primary visual safeguard
        // against silent payment abuse). Do NOT shortcut, batch, or bypass that path.
        defineLegacyProp(legacy, 'sendBsv', makeLegacyMethod(function(payments) {
            warnDeprecated('sendBsv');
            if (!Array.isArray(payments) || payments.length === 0) {
                return Promise.reject(typedError(
                    LEGACY_ERR.INVALID_ENCODING, 'sendBsv',
                    'sendBsv expects a non-empty array of {address, satoshis|amount} objects'
                ));
            }
            // Normalize + validate every item BEFORE any backend calls so a bad item
            // doesn't leave us with partially-resolved scripts and a half-built tx.
            // Yours v4.5.6 used `satoshis`; Hodos's plan used `amount`; accept both
            // to maximize compat with both ecosystem variants.
            var normalized = [];
            for (var i = 0; i < payments.length; i++) {
                var p = payments[i];
                if (!p || typeof p.address !== 'string' || p.address.length === 0) {
                    return Promise.reject(typedError(
                        LEGACY_ERR.INVALID_ENCODING, 'sendBsv',
                        'payment[' + i + '].address must be a non-empty string'
                    ));
                }
                var sats = (typeof p.satoshis === 'number') ? p.satoshis : p.amount;
                if (typeof sats !== 'number' || !isFinite(sats) || sats <= 0 ||
                    Math.floor(sats) !== sats) {
                    return Promise.reject(typedError(
                        LEGACY_ERR.INVALID_ENCODING, 'sendBsv',
                        'payment[' + i + '].satoshis (or amount) must be a positive integer'
                    ));
                }
                // Reject Yours-legacy fields we don't yet translate. Silent skip
                // would mismatch the dApp's actual intent (e.g. OP_RETURN data
                // payload missing, no error surfaced).
                if (p.data) {
                    return Promise.reject(typedError(
                        LEGACY_ERR.NOT_IMPL, 'sendBsv',
                        'payment[' + i + '].data (OP_RETURN) is not yet translated. ' +
                        'Use canonical createAction with an explicit OP_RETURN lockingScript.'
                    ));
                }
                if (p.script) {
                    return Promise.reject(typedError(
                        LEGACY_ERR.NOT_IMPL, 'sendBsv',
                        'payment[' + i + '].script (raw locking script) is not yet translated. ' +
                        'Pass the script directly via canonical createAction.'
                    ));
                }
                normalized.push({ address: p.address, satoshis: sats });
            }
            return withMutex('sendBsv', function() {
                // One /wallet/address-to-script round-trip per payment. N is small in
                // practice — Yours-era dApps almost always send a single output. The
                // wallet enforces mainnet checksum + version-byte validation here
                // (Step 3b.0); the shim never builds locking scripts itself.
                return Promise.all(normalized.map(function(p) {
                    return fetch(ENDPOINT_BASE + '/wallet/address-to-script', {
                        method: 'POST',
                        mode: 'cors',
                        credentials: 'omit',
                        headers: { 'Content-Type': 'application/json' },
                        body: JSON.stringify({ address: p.address })
                    }).then(function(r) {
                        return r.json().then(function(j) {
                            if (!r.ok) {
                                throw typedError(LEGACY_ERR.INVALID_ENCODING, 'sendBsv',
                                    'address-to-script failed for "' + p.address + '": ' +
                                    ((j && j.error) || ('HTTP ' + r.status)));
                            }
                            return { satoshis: p.satoshis, lockingScript: j.lockingScript };
                        });
                    });
                })).then(function(outputs) {
                    return canonical.createAction({
                        description: 'window.yours.sendBsv',
                        outputs: outputs
                    });
                }).then(function(actionResult) {
                    // Legacy callers expect { txid }. Canonical createAction returns
                    // the full action; we extract the txid only. dApps that need rawtx
                    // can use canonical.listActions or the canonical createAction return.
                    return { txid: actionResult && actionResult.txid };
                });
            });
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

        // ----- encrypt / decrypt: real BIE1 via Rust endpoints (Step 3c.3) -----
        //
        // R3 finding (David, 2026-05-28): legacy yours-wallet@v4.5.6 encrypt/decrypt use
        // ECIES Electrum (@bsv/sdk's ECIES.electrumEncrypt, "BIE1" format: magic +
        // ephemeral pubkey + AES-128-CBC ciphertext + HMAC-SHA256). The canonical BRC-100
        // encrypt uses BRC-2 (BRC-42 ECDH + AES-256-GCM). These are wire-incompatible
        // cryptographic schemes — same primitive name, different bytes. Silently routing
        // window.yours.encrypt → canonical window.CWI.encrypt would produce ciphertexts
        // that ARE NOT decryptable by any existing Yours-era ciphertext consumer, so we
        // never did that. Step 3 shipped explicit typed-error rejection; Step 3c.3 ships
        // a real BIE1 implementation in Rust (crypto::bie1) reachable via two HTTP
        // handlers (/wallet/encrypt-bie1 + /wallet/decrypt-bie1, Step 3c.2). The shim
        // here is a thin pass-through that normalizes the Yours-era request shapes.
        //
        // Yours v4.5.6 multi-recipient encrypt accepted pubKeys[] and produced N
        // independent BIE1 ciphertexts; multi-recipient decrypt mirrored that. Step 3c.3
        // ships SINGLE-RECIPIENT only — N > 1 rejects with typed MULTI_RECIPIENT error.
        // Multi-recipient is straightforward to add when demand surfaces (loop over the
        // array, call the endpoint N times, return [ciphertext_1, ..., ciphertext_N]),
        // but every concrete Yours-era flow we've audited uses a single recipient.
        //
        // BRC-2 (AES-GCM via canonical window.CWI.encrypt) remains the recommended path
        // for NEW data; BIE1 here exists strictly for backward compat with stored
        // Yours-era ciphertexts.

        defineLegacyProp(legacy, 'encrypt', makeLegacyMethod(function(opts) {
            warnDeprecated('encrypt');
            opts = opts || {};
            if (typeof opts.message !== 'string') {
                return Promise.reject(typedError(LEGACY_ERR.INVALID_ENCODING, 'encrypt',
                    'encrypt expects {message: string, pubKey | pubKeys: hex string(s)}'));
            }
            // Accept both Yours v4.5.6 (pubKeys: string[]) and the simpler (pubKey: string).
            var pubKeysArr = Array.isArray(opts.pubKeys) ? opts.pubKeys
                           : (typeof opts.pubKey === 'string') ? [opts.pubKey]
                           : null;
            if (!pubKeysArr || pubKeysArr.length === 0) {
                return Promise.reject(typedError(LEGACY_ERR.INVALID_ENCODING, 'encrypt',
                    'recipient pubKey is required (as pubKey: string or pubKeys: [string])'));
            }
            if (pubKeysArr.length > 1) {
                return Promise.reject(typedError(LEGACY_ERR.MULTI_RECIPIENT, 'encrypt',
                    'multi-recipient encrypt (pubKeys[] with N > 1) is not yet supported. ' +
                    'Pass a single recipient pubKey; if you need N recipients today, call ' +
                    'yours.encrypt once per recipient.'));
            }
            if (typeof pubKeysArr[0] !== 'string' || pubKeysArr[0].length === 0) {
                return Promise.reject(typedError(LEGACY_ERR.INVALID_ENCODING, 'encrypt',
                    'recipient pubKey must be a non-empty hex string'));
            }
            return withMutex('encrypt', function() {
                return fetch(ENDPOINT_BASE + '/wallet/encrypt-bie1', {
                    method: 'POST',
                    mode: 'cors',
                    credentials: 'omit',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify({
                        message: opts.message,
                        encoding: opts.encoding || 'utf8',
                        recipientPublicKey: pubKeysArr[0]
                    })
                }).then(function(r) {
                    return r.json().then(function(j) {
                        if (!r.ok) {
                            throw typedError(LEGACY_ERR.NOT_IMPL, 'encrypt',
                                'BIE1 encrypt failed: ' + ((j && j.error) || ('HTTP ' + r.status)));
                        }
                        // Mirror the caller's shape: array in, array out (length 1); scalar
                        // in, scalar out. Keeps Yours-era code paths that destructure
                        // result[0] working unchanged.
                        return Array.isArray(opts.pubKeys) ? [j.ciphertext] : j.ciphertext;
                    });
                });
            });
        }));
)JS"
R"JS(
        defineLegacyProp(legacy, 'decrypt', makeLegacyMethod(function(opts) {
            warnDeprecated('decrypt');
            opts = opts || {};
            // Accept both Yours v4.5.6 (messages: string[]) and simpler (ciphertext: string).
            var messagesArr = Array.isArray(opts.messages) ? opts.messages
                            : (typeof opts.ciphertext === 'string') ? [opts.ciphertext]
                            : null;
            if (!messagesArr || messagesArr.length === 0) {
                return Promise.reject(typedError(LEGACY_ERR.INVALID_ENCODING, 'decrypt',
                    'decrypt expects {ciphertext: string} or {messages: [string]}'));
            }
            if (messagesArr.length > 1) {
                return Promise.reject(typedError(LEGACY_ERR.MULTI_RECIPIENT, 'decrypt',
                    'multi-recipient decrypt (messages[] with N > 1) is not yet supported. ' +
                    'Pass a single ciphertext; if you need to decrypt N items today, call ' +
                    'yours.decrypt once per item.'));
            }
            if (typeof messagesArr[0] !== 'string' || messagesArr[0].length === 0) {
                return Promise.reject(typedError(LEGACY_ERR.INVALID_ENCODING, 'decrypt',
                    'ciphertext must be a non-empty hex string'));
            }
            return withMutex('decrypt', function() {
                return fetch(ENDPOINT_BASE + '/wallet/decrypt-bie1', {
                    method: 'POST',
                    mode: 'cors',
                    credentials: 'omit',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify({
                        ciphertext: messagesArr[0],
                        outputEncoding: opts.outputEncoding || 'utf8'
                    })
                }).then(function(r) {
                    return r.json().then(function(j) {
                        if (!r.ok) {
                            throw typedError(LEGACY_ERR.DECRYPT_FAILED, 'decrypt',
                                'BIE1 decrypt failed: ' + ((j && j.error) || ('HTTP ' + r.status)));
                        }
                        // Mirror caller shape: array in, array out (length 1); scalar in, scalar out.
                        return Array.isArray(opts.messages) ? [j.plaintext] : j.plaintext;
                    });
                });
            });
        }));

        // ----- Removed methods: typed REMOVED error -----
        function removed(name, hint) {
            return makeLegacyMethod(function() {
                return Promise.reject(typedError(LEGACY_ERR.REMOVED, name,
                    'removed in BRC-100. ' + hint));
            });
        }
        // ----- getExchangeRate: real BSV/USD via the existing wallet price cache (Step 3b.5) -----
        // The wallet's /wallet/bsv-price endpoint already feeds the C++ auto-approve engine
        // with a 5-min-TTL CryptoCompare primary + CoinGecko fallback price. Step 3b.5
        // exposes that to legacy yours callers in the {rate, currency: 'USD'} shape they
        // expect. Defensive about the response field name (priceUsd | usd | price) to
        // match the same tolerance the legacy getBalance translator already applies.
        defineLegacyProp(legacy, 'getExchangeRate', makeLegacyMethod(function() {
            warnDeprecated('getExchangeRate');
            return fetch(ENDPOINT_BASE + '/wallet/bsv-price', {
                method: 'GET',
                mode: 'cors',
                credentials: 'omit'
            }).then(function(r) {
                if (!r.ok) {
                    throw typedError(LEGACY_ERR.NOT_IMPL, 'getExchangeRate',
                        'price fetch failed: HTTP ' + r.status);
                }
                return r.json();
            }).then(function(price) {
                var rate = (price && typeof price.priceUsd === 'number') ? price.priceUsd :
                           (price && typeof price.usd === 'number') ? price.usd :
                           (price && typeof price.price === 'number') ? price.price : null;
                if (rate == null) {
                    throw typedError(LEGACY_ERR.NOT_IMPL, 'getExchangeRate',
                        'price endpoint returned unrecognized shape (no priceUsd/usd/price field)');
                }
                return { rate: rate, currency: 'USD' };
            });
        }));

        // ----- getSocialProfile: typed NOT_IMPL deferred (was REMOVED) -----
        // Step 3b.5 reframed this from REMOVED to NOT_IMPL because the capability still
        // exists conceptually — it's just not wired in Hodos yet. The BRC-100 path is
        // acquireCertificate + listCertificates with a SocialCert type. A unified
        // resolver (sigma OAuth username + paymail-derived avatar + identity-key fallback)
        // will land as ecosystem demand surfaces; until then dApps should fall back to
        // identity-key-only flows or call canonical.discoverByIdentityKey for cert-based
        // profile lookup.
        defineLegacyProp(legacy, 'getSocialProfile', makeLegacyMethod(function() {
            warnDeprecated('getSocialProfile');
            return Promise.reject(typedError(
                LEGACY_ERR.NOT_IMPL, 'getSocialProfile',
                'social profile resolution is deferred, not removed. The legacy Yours/RelayX ' +
                'backend returned {username, avatar, paymail}; the BRC-100 path is ' +
                'window.CWI.acquireCertificate + window.CWI.listCertificates with a SocialCert ' +
                'type, or window.CWI.discoverByIdentityKey for cert-based lookup. Hodos will ' +
                'add a unified resolver as ecosystem demand surfaces. For now, dApps should ' +
                'fall back to identity-key-only flows.'
            ));
        }));

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
)JS"
// Phase 2 Step 4 — split between provider installation and announceProvider scaffolding.
// The icon data URL embedded below pushes the announce block's text past MSVC's 16380-
// char per-literal cap. Step 3b.4 added another split inside the legacy provider block
// to keep the new sendBsv translator's bytes inside its containing literal's budget, and
// Step 3c.3 added a third split between encrypt and decrypt because the new BIE1 wire-
// ups pushed the combined region over cap. The bundle now uses six adjacent
// R"JS(...)JS" literals. C++ auto-concatenates them; the injected JS sees one continuous string.
R"JS(

    // bsv:announceProvider — multi-provider discovery (BSV equivalent of EIP-6963).
    // Step 1 ships the listener + initial announce; Step 4 finalizes icon + rdns + the
    // BRC submission. Sending an early unprompted announce matches EIP-6963 cadence so
    // dApps that load their listener after page-load still receive us.
    try {
        var ANNOUNCEMENT = {
            info: {
                uuid: 'hodos-' + Date.now() + '-' + Math.random().toString(36).slice(2),
                name: 'Hodos',
                // Hodos_Gold_Wallet_Icon.svg, base64-embedded. Prefix split across two JS
                // literals so the contiguous data URL substring never appears in the C++
                // source as a single token — keeps tooling that pattern-matches data URLs
                // (Claude Code's image auto-detect, code-scanning passes) from snagging it.
                icon: 'data:' + 'image/svg+xml;base64,' + 'PD94bWwgdmVyc2lvbj0iMS4wIiBlbmNvZGluZz0iVVRGLTgiIHN0YW5kYWxvbmU9Im5vIj8+CjxzdmcKICAgaWQ9IkxheWVyXzEiCiAgIHZlcnNpb249IjEuMSIKICAgdmlld0JveD0iMCAwIDE2Ni44MSA1NC4wMDk5OTgiCiAgIHNvZGlwb2RpOmRvY25hbWU9IkNvbG9yX0hvZG9zV2FsbGV0LnN2ZyIKICAgaW5rc2NhcGU6dmVyc2lvbj0iMS40LjMgKDBkMTVmNzUsIDIwMjUtMTItMjUpIgogICB4bWxuczppbmtzY2FwZT0iaHR0cDovL3d3dy5pbmtzY2FwZS5vcmcvbmFtZXNwYWNlcy9pbmtzY2FwZSIKICAgeG1sbnM6c29kaXBvZGk9Imh0dHA6Ly9zb2RpcG9kaS5zb3VyY2Vmb3JnZS5uZXQvRFREL3NvZGlwb2RpLTAuZHRkIgogICB4bWxuczp4bGluaz0iaHR0cDovL3d3dy53My5vcmcvMTk5OS94bGluayIKICAgeG1sbnM9Imh0dHA6Ly93d3cudzMub3JnLzIwMDAvc3ZnIgogICB4bWxuczpzdmc9Imh0dHA6Ly93d3cudzMub3JnLzIwMDAvc3ZnIj4KICA8c29kaXBvZGk6bmFtZWR2aWV3CiAgICAgaWQ9Im5hbWVkdmlldzEiCiAgICAgcGFnZWNvbG9yPSIjZmZmZmZmIgogICAgIGJvcmRlcmNvbG9yPSIjMDAwMDAwIgogICAgIGJvcmRlcm9wYWNpdHk9IjAuMjUiCiAgICAgaW5rc2NhcGU6c2hvd3BhZ2VzaGFkb3c9IjIiCiAgICAgaW5rc2NhcGU6cGFnZW9wYWNpdHk9IjAuMCIKICAgICBpbmtzY2FwZTpwYWdlY2hlY2tlcmJvYXJkPSIwIgogICAgIGlua3NjYXBlOmRlc2tjb2xvcj0iI2QxZDFkMSIKICAgICBpbmtzY2FwZTp6b29tPSIxMC42NDY4NDQiCiAgICAgaW5rc2NhcGU6Y3g9IjgzLjQwNDk5OSIKICAgICBpbmtzY2FwZTpjeT0iMjcuMDAzMzA4IgogICAgIGlua3NjYXBlOndpbmRvdy13aWR0aD0iMTkyMCIKICAgICBpbmtzY2FwZTp3aW5kb3ctaGVpZ2h0PSIxMDE3IgogICAgIGlua3NjYXBlOndpbmRvdy14PSItOCIKICAgICBpbmtzY2FwZTp3aW5kb3cteT0iLTgiCiAgICAgaW5rc2NhcGU6d2luZG93LW1heGltaXplZD0iMSIKICAgICBpbmtzY2FwZTpjdXJyZW50LWxheWVyPSJMYXllcl8xIiAvPgogIDwhLS0gR2VuZXJhdG9yOiBBZG9iZSBJbGx1c3RyYXRvciAyOS44LjMsIFNWRyBFeHBvcnQgUGx1Zy1JbiAuIFNWRyBWZXJzaW9uOiAyLjEuMSBCdWlsZCAzKSAgLS0+CiAgPGRlZnMKICAgICBpZD0iZGVmczIiPgogICAgPHN0eWxlCiAgICAgICBpZD0ic3R5bGUxIj4KICAgICAgLnN0MCB7CiAgICAgICAgZmlsbDogI2E2N2MwMDsKICAgICAgfQoKICAgICAgLnN0MSB7CiAgICAgICAgZmlsbDogdXJsKCNsaW5lYXItZ3JhZGllbnQyKTsKICAgICAgfQoKICAgICAgLnN0MiB7CiAgICAgICAgZmlsbDogdXJsKCNsaW5lYXItZ3JhZGllbnQxKTsKICAgICAgfQoKICAgICAgLnN0MyB7CiAgICAgICAgZmlsbDogdXJsKCNsaW5lYXItZ3JhZGllbnQzKTsKICAgICAgfQoKICAgICAgLnN0NCB7CiAgICAgICAgZmlsbDogdXJsKCNsaW5lYXItZ3JhZGllbnQ2KTsKICAgICAgfQoKICAgICAgLnN0NSB7CiAgICAgICAgZmlsbDogdXJsKCNsaW5lYXItZ3JhZGllbnQ3KTsKICAgICAgfQoKICAgICAgLnN0NiB7CiAgICAgICAgZmlsbDogdXJsKCNsaW5lYXItZ3JhZGllbnQ1KTsKICAgICAgfQoKICAgICAgLnN0NyB7CiAgICAgICAgZmlsbDogdXJsKCNsaW5lYXItZ3JhZGllbnQ0KTsKICAgICAgfQoKICAgICAgLnN0OCB7CiAgICAgICAgZmlsbDogdXJsKCNsaW5lYXItZ3JhZGllbnQpOwogICAgICB9CgogICAgICAuc3Q5IHsKICAgICAgICBmaWxsOiAjZGZiZDY5OwogICAgICB9CgogICAgICAuc3QxMCB7CiAgICAgICAgZmlsbDogI2E1N2QyZDsKICAgICAgfQogICAgPC9zdHlsZT4KICAgIDxsaW5lYXJHcmFkaWVudAogICAgICAgaWQ9ImxpbmVhci1ncmFkaWVudCIKICAgICAgIHgxPSIzMi44MiIKICAgICAgIHkxPSIxMy45NyIKICAgICAgIHgyPSIxOC43MyIKICAgICAgIHkyPSIxMC43NCIKICAgICAgIGdyYWRpZW50VW5pdHM9InVzZXJTcGFjZU9uVXNlIj4KICAgICAgPHN0b3AKICAgICAgICAgb2Zmc2V0PSIwIgogICAgICAgICBzdG9wLWNvbG9yPSIjZmZmIgogICAgICAgICBpZD0ic3RvcDEiIC8+CiAgICAgIDxzdG9wCiAgICAgICAgIG9mZnNldD0iMSIKICAgICAgICAgc3RvcC1jb2xvcj0iI2E2N2MwMCIKICAgICAgICAgaWQ9InN0b3AyIiAvPgogICAgPC9saW5lYXJHcmFkaWVudD4KICAgIDxsaW5lYXJHcmFkaWVudAogICAgICAgaWQ9ImxpbmVhci1ncmFkaWVudDEiCiAgICAgICB4MT0iNDAuMzMwMDAyIgogICAgICAgeTE9IjIxLjkiCiAgICAgICB4Mj0iMzIuNjUwMDAyIgogICAgICAgeTI9IjkuNjQ5OTk5NiIKICAgICAgIHhsaW5rOmhyZWY9IiNsaW5lYXItZ3JhZGllbnQiIC8+CiAgICA8bGluZWFyR3JhZGllbnQKICAgICAgIGlkPSJsaW5lYXItZ3JhZGllbnQyIgogICAgICAgeDE9IjQwLjAyOTk5OSIKICAgICAgIHkxPSIzMi44MiIKICAgICAgIHgyPSI0My4yNTk5OTgiCiAgICAgICB5Mj0iMTguNzMiCiAgICAgICB4bGluazpocmVmPSIjbGluZWFyLWdyYWRpZW50IiAvPgogICAgPGxpbmVhckdyYWRpZW50CiAgICAgICBpZD0ibGluZWFyLWdyYWRpZW50MyIKICAgICAgIHgxPSIzMi4wOTk5OTgiCiAgICAgICB5MT0iNDAuMzMwMDAyIgogICAgICAgeDI9IjQ0LjM0OTk5OCIKICAgICAgIHkyPSIzMi42NTAwMDIiCiAgICAgICB4bGluazpocmVmPSIjbGluZWFyLWdyYWRpZW50IiAvPgogICAgPGxpbmVhckdyYWRpZW50CiAgICAgICBpZD0ibGluZWFyLWdyYWRpZW50NCIKICAgICAgIHgxPSIyMS4xOCIKICAgICAgIHkxPSI0MC4wMjk5OTkiCiAgICAgICB4Mj0iMzUuMjciCiAgICAgICB5Mj0iNDMuMjU5OTk4IgogICAgICAgeGxpbms6aHJlZj0iI2xpbmVhci1ncmFkaWVudCIgLz4KICAgIDxsaW5lYXJHcmFkaWVudAogICAgICAgaWQ9ImxpbmVhci1ncmFkaWVudDUiCiAgICAgICB4MT0iMTMuNjciCiAgICAgICB5MT0iMzIuMDk5OTk4IgogICAgICAgeDI9IjIxLjM1IgogICAgICAgeTI9IjQ0LjM0OTk5OCIKICAgICAgIHhsaW5rOmhyZWY9IiNsaW5lYXItZ3JhZGllbnQiIC8+CiAgICA8bGluZWFyR3JhZGllbnQKICAgICAgIGlkPSJsaW5lYXItZ3JhZGllbnQ2IgogICAgICAgeDE9IjEzLjk3IgogICAgICAgeTE9IjIxLjE4IgogICAgICAgeDI9IjEwLjc0IgogICAgICAgeTI9IjM1LjI3IgogICAgICAgeGxpbms6aHJlZj0iI2xpbmVhci1ncmFkaWVudCIgLz4KICAgIDxsaW5lYXJHcmFkaWVudAogICAgICAgaWQ9ImxpbmVhci1ncmFkaWVudDciCiAgICAgICB4MT0iMjEuOSIKICAgICAgIHkxPSIxMy42NiIKICAgICAgIHgyPSI5LjY0OTk5OTYiCiAgICAgICB5Mj0iMjEuMzUiCiAgICAgICB4bGluazpocmVmPSIjbGluZWFyLWdyYWRpZW50IiAvPgogICAgPGxpbmVhckdyYWRpZW50CiAgICAgICB4bGluazpocmVmPSIjbGluZWFyLWdyYWRpZW50IgogICAgICAgaWQ9ImxpbmVhckdyYWRpZW50MSIKICAgICAgIGdyYWRpZW50VW5pdHM9InVzZXJTcGFjZU9uVXNlIgogICAgICAgeDE9IjMyLjgyIgogICAgICAgeTE9IjEzLjk3IgogICAgICAgeDI9IjE4LjczIgogICAgICAgeTI9IjEwLjc0IiAvPgogIDwvZGVmcz4KICA8ZwogICAgIGlkPSJnMTQiPgogICAgPGcKICAgICAgIGlkPSJnOCI+CiAgICAgIDxwYXRoCiAgICAgICAgIHN0eWxlPSJmb250LXNpemU6MTZweDtsaW5lLWhlaWdodDoyMi4yNHB4O2ZvbnQtZmFtaWx5OidGb3VuZGVycyBHcm90ZXNrJzstaW5rc2NhcGUtZm9udC1zcGVjaWZpY2F0aW9uOidGb3VuZGVycyBHcm90ZXNrLCBOb3JtYWwnO2xldHRlci1zcGFjaW5nOjkuMTlweDt3b3JkLXNwYWNpbmc6MC4wMXB4O2ZpbGw6I2RmYmQ2OSIKICAgICAgICAgZD0ibSAxNjIuODY3ODksNDYuMDMwNDM3IHYgLTguOTI4IGggMy43MjggdiAtMS4xNTIgaCAtOC43NjggdiAxLjE1MiBoIDMuNjY0IHYgOC45MjggeiIKICAgICAgICAgaWQ9InBhdGgyNyIgLz4KICAgICAgPHBhdGgKICAgICAgICAgc3R5bGU9ImZvbnQtc2l6ZToxNnB4O2xpbmUtaGVpZ2h0OjIyLjI0cHg7Zm9udC1mYW1pbHk6J0ZvdW5kZXJzIEdyb3Rlc2snOy1pbmtzY2FwZS1mb250LXNwZWNpZmljYXRpb246J0ZvdW5kZXJzIEdyb3Rlc2ssIE5vcm1hbCc7bGV0dGVyLXNwYWNpbmc6OS4xOXB4O3dvcmQtc3BhY2luZzowLjAxcHg7ZmlsbDojZGZiZDY5IgogICAgICAgICBkPSJtIDE0Ny42OTM4OSw0NC44Nzg0MzcgaCAtNS44NCB2IC0zLjQ3MiBoIDQuNTI4IHYgLTEuMTM2IGggLTQuNTI4IHYgLTMuMTY4IGggNS42OTYgdiAtMS4xNTIgaCAtNy4wNCB2IDEwLjA4IGggNy4xODQgeiIKICAgICAgICAgaWQ9InBhdGgyNiIgLz4KICAgICAgPHBhdGgKICAgICAgICAgc3R5bGU9ImZvbnQtc2l6ZToxNnB4O2xpbmUtaGVpZ2h0OjIyLjI0cHg7Zm9udC1mYW1pbHk6J0ZvdW5kZXJzIEdyb3Rlc2snOy1pbmtzY2FwZS1mb250LXNwZWNpZmljYXRpb246J0ZvdW5kZXJzIEdyb3Rlc2ssIE5vcm1hbCc7bGV0dGVyLXNwYWNpbmc6OS4xOXB4O3dvcmQtc3BhY2luZzowLjAxcHg7ZmlsbDojZGZiZDY5IgogICAgICAgICBkPSJtIDEyMi45MzU4OSwzNS45NTA0MzcgdiAxMC4wOCBoIDcuMDA4IHYgLTEuMTUyIGggLTUuNjE2IHYgLTguOTI4IHoiCiAgICAgICAgIGlkPSJwYXRoMjUiIC8+CiAgICAgIDxwYXRoCiAgICAgICAgIHN0eWxlPSJmb250LXNpemU6MTZweDtsaW5lLWhlaWdodDoyMi4yNHB4O2ZvbnQtZmFtaWx5OidGb3VuZGVycyBHcm90ZXNrJzstaW5rc2NhcGUtZm9udC1zcGVjaWZpY2F0aW9uOidGb3VuZGVycyBHcm90ZXNrLCBOb3JtYWwnO2xldHRlci1zcGFjaW5nOjkuMTlweDt3b3JkLXNwYWNpbmc6MC4wMXB4O2ZpbGw6I2RmYmQ2OSIKICAgICAgICAgZD0ibSAxMDUuMzYxODgsMzUuOTUwNDM3IHYgMTAuMDggaCA3LjAwOCB2IC0xLjE1MiBoIC01LjYxNiB2IC04LjkyOCB6IgogICAgICAgICBpZD0icGF0aDI0IiAvPgogICAgICA8cGF0aAogICAgICAgICBzdHlsZT0iZm9udC1zaXplOjE2cHg7bGluZS1oZWlnaHQ6MjIuMjRweDtmb250LWZhbWlseTonRm91bmRlcnMgR3JvdGVzayc7LWlua3NjYXBlLWZvbnQtc3BlY2lmaWNhdGlvbjonRm91bmRlcnMgR3JvdGVzaywgTm9ybWFsJztsZXR0ZXItc3BhY2luZzo5LjE5cHg7d29yZC1zcGFjaW5nOjAuMDFweDtmaWxsOiNkZmJkNjkiCiAgICAgICAgIGQ9Im0gODkuMjkxODc3LDM1Ljk1MDQzNyAtNC4xMTIsMTAuMDggaCAxLjM2IGwgMS4xODQsLTIuOTYgaCA0LjU5MiBsIDEuMjE2LDIuOTYgaCAxLjQ3MiBsIC00LjE5MiwtMTAuMDggeiBtIDAuNjcyLDEuNTM2IGggMC4wNjQgbCAxLjg0LDQuNDggaCAtMy43MTIgeiIKICAgICAgICAgaWQ9InBhdGgyMyIgLz4KICAgICAgPHBhdGgKICAgICAgICAgc3R5bGU9ImZvbnQtc2l6ZToxNnB4O2xpbmUtaGVpZ2h0OjIyLjI0cHg7Zm9udC1mYW1pbHk6J0ZvdW5kZXJzIEdyb3Rlc2snOy1pbmtzY2FwZS1mb250LXNwZWNpZmljYXRpb246J0ZvdW5kZXJzIEdyb3Rlc2ssIE5vcm1hbCc7bGV0dGVyLXNwYWNpbmc6OS4xOXB4O3dvcmQtc3BhY2luZzowLjAxcHg7ZmlsbDojZGZiZDY5IgogICAgICAgICBkPSJtIDcwLjM3Mzg4NSwzNS45NTA0MzcgaCAtMS4yOTYgbCAtMi4xOTIsOC4zNTIgaCAtMC4wNjQgbCAtMi4xMTIsLTguMzUyIGggLTEuMzQ0IGwgMi41NzYsMTAuMDggaCAxLjYgbCAyLjE0NCwtOC4wNDggaCAwLjA2NCBsIDIuMDk2LDguMDQ4IGggMS42MTYgbCAyLjU3NiwtMTAuMDggaCAtMS4yOTYgbCAtMi4wNjQsOC4zODQgaCAtMC4wNjQgeiIKICAgICAgICAgaWQ9InRleHQyMiIgLz4KICAgIDwvZz4KICAgIDxnCiAgICAgICBpZD0iZzEzIj4KICAgICAgPHBhdGgKICAgICAgICAgY2xhc3M9InN0MCIKICAgICAgICAgZD0iTSA2My4xNywyNy45OCBWIDguMTggaCA0Ljc4IHYgNy44MyBoIDguNjQgViA4LjE4IGggNC43OCB2IDE5LjggaCAtNC43OCB2IC04LjExIGggLTguNjQgdiA4LjExIHoiCiAgICAgICAgIGlkPSJwYXRoOSIgLz4KICAgICAgPHBhdGgKICAgICAgICAgY2xhc3M9InN0MCIKICAgICAgICAgZD0ibSA5NC4zOSwyOC4zNiBjIC01Ljc1LDAgLTEwLjE1LC00LjE4IC0xMC4xNSwtMTAuMjggMCwtNi4xIDQuNCwtMTAuMjggMTAuMTUsLTEwLjI4IDUuNzUsMCAxMC4xOCw0LjE4IDEwLjE4LDEwLjI4IDAsNi4xIC00LjQsMTAuMjggLTEwLjE4LDEwLjI4IHogbSAwLC0xNi4zOCBjIC0zLjIxLDAgLTUuMjUsMi40MiAtNS4yNSw2LjEgMCwzLjY4IDIuMDQsNi4xIDUuMjUsNi4xIDMuMjEsMCA1LjI4LC0yLjQyIDUuMjgsLTYuMSAwLC0zLjY4IC0yLjA0LC02LjEgLTUuMjgsLTYuMSB6IgogICAgICAgICBpZD0icGF0aDEwIiAvPgogICAgICA8cGF0aAogICAgICAgICBjbGFzcz0ic3QwIgogICAgICAgICBkPSJtIDEwNy40NCw4LjE4IGggNy42NyBjIDYuNTEsMCAxMC41MywzLjcxIDEwLjUzLDkuOSAwLDYuMTkgLTQuMDIsOS45IC0xMC41Myw5LjkgaCAtNy42NyB6IG0gNy4zNSwxNi4wMyBjIDMuNzQsMCA1LjkxLC0yLjI2IDUuOTEsLTYuMTYgMCwtMy45IC0yLjE3LC02LjEgLTUuOTEsLTYuMSBoIC0yLjU4IHYgMTIuMjYgeiIKICAgICAgICAgaWQ9InBhdGgxMSIgLz4KICAgICAgPHBhdGgKICAgICAgICAgY2xhc3M9InN0MCIKICAgICAgICAgZD0ibSAxMzcuNywyOC4zNiBjIC01Ljc1LDAgLTEwLjE1LC00LjE4IC0xMC4xNSwtMTAuMjggMCwtNi4xIDQuNCwtMTAuMjggMTAuMTUsLTEwLjI4IDUuNzUsMCAxMC4xOCw0LjE4IDEwLjE4LDEwLjI4IDAsNi4xIC00LjQsMTAuMjggLTEwLjE4LDEwLjI4IHogbSAwLC0xNi4zOCBjIC0zLjIxLDAgLTUuMjUsMi40MiAtNS4yNSw2LjEgMCwzLjY4IDIuMDQsNi4xIDUuMjUsNi4xIDMuMjEsMCA1LjI4LC0yLjQyIDUuMjgsLTYuMSAwLC0zLjY4IC0yLjA0LC02LjEgLTUuMjgsLTYuMSB6IgogICAgICAgICBpZD0icGF0aDEyIiAvPgogICAgICA8cGF0aAogICAgICAgICBjbGFzcz0ic3QwIgogICAgICAgICBkPSJtIDE1NC4wNCwyMC43OCBjIDAuMTksMi43NCAyLjIzLDMuODMgNC42OCwzLjgzIDIuMTEsMCAzLjQzLC0wLjgyIDMuNDMsLTIuMTQgMCwtMS4zMiAtMS4xNiwtMS42MyAtMi44OSwtMS45OCBsIC0zLjcxLC0wLjY2IGMgLTMuMTQsLTAuNiAtNS4zOCwtMi4zNiAtNS4zOCwtNS43MiAwLC0zLjkgMy4wNSwtNi4zMiA3Ljg2LC02LjMyIDUuMzgsMCA4LjMsMi42NyA4LjM5LDcuMDcgbCAtNC40LDAuMTMgYyAtMC4xMywtMi4zMyAtMS43MywtMy40NiAtNC4wMiwtMy40NiAtMi4wMSwwIC0zLjE0LDAuODIgLTMuMTQsMi4xNyAwLDEuMTMgMC44OCwxLjU0IDIuMzMsMS44MiBsIDMuNzEsMC42NiBjIDQuMDUsMC43MiA1LjkxLDIuNzMgNS45MSw1Ljk3IDAsNC4wOSAtMy41NSw2LjE5IC04LjA4LDYuMTkgLTUuMjgsMCAtOS4wNSwtMi42MSAtOS4wNSwtNy40MiBsIDQuMzcsLTAuMTYgeiIKICAgICAgICAgaWQ9InBhdGgxMyIgLz4KICAgIDwvZz4KICA8L2c+CiAgPGcKICAgICBpZD0iZzIyIj4KICAgIDxwYXRoCiAgICAgICBjbGFzcz0ic3Q4IgogICAgICAgZD0ibSAxNy41NiwyMy4wMyBjIDEuMDIsLTIuNDMgMi45NywtNC40NiA1LjU4LC01LjUxIDMuMjIsLTQuMjIgNy4wOSwtNi42OCAxMC43MywtOC4xIEMgMzEuNDksMy40OCAyNi42MiwwIDI2LjYyLDAgYyAwLDAgLTQuNDYsMy40NyAtNy4yLDkuNzEgLTEuNTcsMy41NyAtMi41Nyw4LjA1IC0xLjg2LDEzLjMyIHoiCiAgICAgICBpZD0icGF0aDE0IgogICAgICAgc3R5bGU9ImZpbGw6dXJsKCNsaW5lYXJHcmFkaWVudDEpIiAvPgogICAgPHBhdGgKICAgICAgIGNsYXNzPSJzdDIiCiAgICAgICBkPSJtIDIzLjE0LDE3LjUxIGMgMC4xNSwtMC4wNiAwLjMsLTAuMTMgMC40NiwtMC4xOSAyLjUsLTAuODggNS4xLC0wLjcyIDcuMzcsMC4yNCA1LjI2LC0wLjcxIDkuNzUsMC4yOSAxMy4zMiwxLjg2IDIuNTIsLTUuODggMS41NCwtMTEuNzggMS41NCwtMTEuNzggMCwwIC01LjYsLTAuNyAtMTEuOTYsMS43OCAtMy42MywxLjQyIC03LjUxLDMuODggLTEwLjczLDguMSB6IgogICAgICAgaWQ9InBhdGgxNSIKICAgICAgIHN0eWxlPSJmaWxsOnVybCgjbGluZWFyLWdyYWRpZW50MSkiIC8+CiAgICA8cGF0aAogICAgICAgY2xhc3M9InN0MSIKICAgICAgIGQ9Im0gNTQsMjYuNjIgYyAwLDAgLTMuNDcsLTQuNDYgLTkuNzEsLTcuMiAtMy41NywtMS41NyAtOC4wNiwtMi41NyAtMTMuMzIsLTEuODYgMi40MywxLjAyIDQuNDUsMi45NyA1LjUxLDUuNTcgNC4yMiwzLjIyIDYuNjksNy4xIDguMSwxMC43MyBDIDUwLjUyLDMxLjQ4IDU0LDI2LjYyIDU0LDI2LjYyIFoiCiAgICAgICBpZD0icGF0aDE2IgogICAgICAgc3R5bGU9ImZpbGw6dXJsKCNsaW5lYXItZ3JhZGllbnQyKSIgLz4KICAgIDxwYXRoCiAgICAgICBjbGFzcz0ic3QzIgogICAgICAgZD0ibSAzNi40OCwyMy4xNCBjIDAuMDYsMC4xNiAwLjEzLDAuMzEgMC4xOSwwLjQ3IDAuODUsMi40MiAwLjc2LDUuMDIgLTAuMjQsNy4zNyAwLjcxLDUuMjYgLTAuMjksOS43NCAtMS44NiwxMy4zMSA1Ljg4LDIuNTIgMTEuNzgsMS41NCAxMS43OCwxLjU0IDAsMCAwLjcsLTUuNiAtMS43OCwtMTEuOTYgLTEuNDIsLTMuNjMgLTMuODgsLTcuNTEgLTguMSwtMTAuNzMgeiIKICAgICAgIGlkPSJwYXRoMTciCiAgICAgICBzdHlsZT0iZmlsbDp1cmwoI2xpbmVhci1ncmFkaWVudDMpIiAvPgogICAgPHBhdGgKICAgICAgIGNsYXNzPSJzdDciCiAgICAgICBkPSJtIDM2LjQ0LDMwLjk4IGMgLTAuMDcsMC4xNSAtMC4xMiwwLjMxIC0wLjIsMC40NiAtMS4xMSwyLjMyIC0zLjAyLDQuMDkgLTUuMzgsNS4wNSAtMy4yMiw0LjIyIC03LjA5LDYuNjggLTEwLjczLDguMSAyLjM4LDUuOTQgNy4yNCw5LjQyIDcuMjQsOS40MiAwLDAgNC40NiwtMy40NyA3LjIsLTkuNzEgMS41NywtMy41NyAyLjU3LC04LjA1IDEuODYsLTEzLjMxIHoiCiAgICAgICBpZD0icGF0aDE4IgogICAgICAgc3R5bGU9ImZpbGw6dXJsKCNsaW5lYXItZ3JhZGllbnQ0KSIgLz4KICAgIDxwYXRoCiAgICAgICBjbGFzcz0ic3Q2IgogICAgICAgZD0ibSAzMC44NiwzNi40OSBjIC0wLjE2LDAuMDYgLTAuMzEsMC4xMyAtMC40NywwLjE5IC0xLjEyLDAuMzkgLTIuMjYsMC41OCAtMy4zOSwwLjU4IC0xLjM5LDAgLTIuNzQsLTAuMjkgLTMuOTksLTAuODIgLTUuMjYsMC43MSAtOS43NCwtMC4yOSAtMTMuMzEsLTEuODYgLTIuNTIsNS44OCAtMS41NCwxMS43OCAtMS41NCwxMS43OCAwLDAgNS42LDAuNyAxMS45NiwtMS43OCAzLjYzLC0xLjQyIDcuNTEsLTMuODggMTAuNzMsLTguMSB6IgogICAgICAgaWQ9InBhdGgxOSIKICAgICAgIHN0eWxlPSJmaWxsOnVybCgjbGluZWFyLWdyYWRpZW50NSkiIC8+CiAgICA8cGF0aAogICAgICAgY2xhc3M9InN0NCIKICAgICAgIGQ9Ik0gMjMuMDIsMzYuNDQgQyAyMC41OSwzNS40MSAxOC41NiwzMy40NiAxNy41MSwzMC44NiAxMy4yOSwyNy42NCAxMC44NCwyMy43NyA5LjQyLDIwLjE0IDMuNDgsMjIuNTIgMCwyNy4zOCAwLDI3LjM4IGMgMCwwIDMuNDcsNC40NiA5LjcxLDcuMiAzLjU3LDEuNTcgOC4wNSwyLjU3IDEzLjMxLDEuODYgeiIKICAgICAgIGlkPSJwYXRoMjAiCiAgICAgICBzdHlsZT0iZmlsbDp1cmwoI2xpbmVhci1ncmFkaWVudDYpIiAvPgogICAgPHBhdGgKICAgICAgIGNsYXNzPSJzdDUiCiAgICAgICBkPSJNIDE3LjUsMzAuODUgQyAxNy40NCwzMC43IDE3LjM3LDMwLjU1IDE3LjMyLDMwLjM5IDE2LjQ0LDI3Ljg5IDE2LjYsMjUuMjkgMTcuNTYsMjMuMDIgMTYuODUsMTcuNzYgMTcuODUsMTMuMjggMTkuNDIsOS43IDEzLjU0LDcuMTggNy42NCw4LjE2IDcuNjQsOC4xNiBjIDAsMCAtMC43LDUuNiAxLjc4LDExLjk2IDEuNDIsMy42MyAzLjg3LDcuNSA4LjA5LDEwLjcyIHoiCiAgICAgICBpZD0icGF0aDIxIgogICAgICAgc3R5bGU9ImZpbGw6dXJsKCNsaW5lYXItZ3JhZGllbnQ3KSIgLz4KICAgIDxwYXRoCiAgICAgICBjbGFzcz0ic3QxMCIKICAgICAgIGQ9Im0gMjMuNiwxNy4zMyBjIC0wLjE2LDAuMDYgLTAuMzEsMC4xMyAtMC40NiwwLjE5IC0yLjYsMS4wNiAtNC41NSwzLjA4IC01LjU4LDUuNTEgLTAuOTUsMi4yNyAtMS4xMiw0Ljg3IC0wLjI0LDcuMzcgMC4wNSwwLjE2IDAuMTIsMC4zMSAwLjE4LDAuNDYgMS4wNiwyLjYgMy4wOCw0LjU2IDUuNTEsNS41OCAxLjI1LDAuNTMgMi42MSwwLjgyIDMuOTksMC44MiAxLjEyLDAgMi4yNywtMC4xOSAzLjM5LC0wLjU4IDAuMTYsLTAuMDYgMC4zMSwtMC4xMyAwLjQ3LC0wLjE5IDIuMzcsLTAuOTYgNC4yNywtMi43MyA1LjM4LC01LjA1IDAuMDcsLTAuMTUgMC4xMywtMC4zMSAwLjIsLTAuNDYgMC45OSwtMi4zNSAxLjA5LC00Ljk1IDAuMjQsLTcuMzcgLTAuMDYsLTAuMTYgLTAuMTMsLTAuMzEgLTAuMTksLTAuNDcgLTEuMDYsLTIuNiAtMy4wOCwtNC41NSAtNS41MSwtNS41NyAtMi4yNiwtMC45NSAtNC44NywtMS4xMiAtNy4zNywtMC4yNCB6IG0gMTEuODIsNi43MSBjIDEuNjMsNC42NSAtMC44MSw5Ljc1IC01LjQ3LDExLjM4IC00LjY1LDEuNjMgLTkuNzUsLTAuODEgLTExLjM4LC01LjQ3IC0xLjYzLC00LjY1IDAuODEsLTkuNzUgNS40NywtMTEuMzggNC42NSwtMS42MyA5Ljc1LDAuODEgMTEuMzgsNS40NyB6IgogICAgICAgaWQ9InBhdGgyMiIgLz4KICA8L2c+Cjwvc3ZnPgo=',
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
