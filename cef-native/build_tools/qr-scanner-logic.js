// QR Scanner Logic — injected into page context after jsQR library
// Scans DOM elements for QR codes, filters for BSV payment patterns,
// and sends results back via cefMessage IPC.
// Async to support SVG image loading.
(async function() {
    'use strict';

    // jsQR is available as jsQRLib (or jsQRLib.default for UMD)
    var decode = (typeof jsQRLib === 'function') ? jsQRLib :
                 (jsQRLib && typeof jsQRLib.default === 'function') ? jsQRLib.default : null;
    if (!decode) {
        if (window.cefMessage && window.cefMessage.send) {
            window.cefMessage.send('qr_found', [JSON.stringify([])]);
        }
        return;
    }

    // BSV pattern matchers (mirrors TransactionForm.tsx regexes)
    var BSV_ADDRESS_RE = /^[13][a-km-zA-HJ-NP-Z1-9]{25,34}$/;
    var IDENTITY_KEY_RE = /^(02|03)[0-9a-fA-F]{64}$/;
    var PAYMAIL_RE = /^(\$[a-zA-Z0-9_]+|[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,})$/;
    var BIP21_RE = /^bitcoin:/i;

    function parseBIP21(uri) {
        if (!BIP21_RE.test(uri)) return null;
        var rest = uri.slice(8); // remove "bitcoin:"
        var qIdx = rest.indexOf('?');
        var address = qIdx >= 0 ? rest.slice(0, qIdx) : rest;
        var params = {};
        if (qIdx >= 0) {
            rest.slice(qIdx + 1).split('&').forEach(function(pair) {
                var eq = pair.indexOf('=');
                if (eq >= 0) {
                    params[decodeURIComponent(pair.slice(0, eq))] = decodeURIComponent(pair.slice(eq + 1));
                }
            });
        }
        return {
            address: address,
            amount: params.amount ? parseFloat(params.amount) : undefined,
            label: params.label || undefined
        };
    }

    function classifyQR(text) {
        if (!text || typeof text !== 'string') return null;
        text = text.trim();

        // BIP21 URI — accept any BSV recipient pattern in the address position
        // (standard BSV address, identity key, paymail, or $handle)
        if (BIP21_RE.test(text)) {
            var parsed = parseBIP21(text);
            if (parsed && (BSV_ADDRESS_RE.test(parsed.address) ||
                           IDENTITY_KEY_RE.test(parsed.address) ||
                           PAYMAIL_RE.test(parsed.address))) {
                return {
                    type: 'bip21',
                    value: text,
                    address: parsed.address,
                    amount: parsed.amount,
                    label: parsed.label
                };
            }
            return null; // BIP21 but unrecognized recipient format
        }

        // Plain BSV address
        if (BSV_ADDRESS_RE.test(text)) {
            return { type: 'address', value: text, address: text };
        }

        // Identity key (compressed public key)
        if (IDENTITY_KEY_RE.test(text)) {
            return { type: 'identity_key', value: text };
        }

        // Paymail or $handle
        if (PAYMAIL_RE.test(text)) {
            return { type: 'paymail', value: text };
        }

        return null; // Not a BSV pattern
    }

    function decodeFromImageData(imageData) {
        try {
            var result = decode(imageData.data, imageData.width, imageData.height);
            if (result && result.data) {
                return classifyQR(result.data);
            }
        } catch (e) { /* ignore decode errors */ }
        return null;
    }

    function scanCanvas(canvas) {
        try {
            var w = canvas.width, h = canvas.height;
            if (w < 20 || h < 20) return null;
            var ctx = canvas.getContext('2d');
            if (!ctx) return null;
            var imageData = ctx.getImageData(0, 0, w, h);
            return decodeFromImageData(imageData);
        } catch (e) { return null; } // CORS or other error
    }

    function scanImage(img) {
        try {
            if (!img.complete || img.naturalWidth < 20 || img.naturalHeight < 20) return null;
            var canvas = document.createElement('canvas');
            // Cap at 1000px to avoid huge memory allocation
            var scale = Math.min(1, 1000 / Math.max(img.naturalWidth, img.naturalHeight));
            canvas.width = Math.round(img.naturalWidth * scale);
            canvas.height = Math.round(img.naturalHeight * scale);
            var ctx = canvas.getContext('2d');
            if (!ctx) return null;
            ctx.drawImage(img, 0, 0, canvas.width, canvas.height);
            var imageData = ctx.getImageData(0, 0, canvas.width, canvas.height);
            var result = decodeFromImageData(imageData);
            if (result) result.source = 'image';
            return result;
        } catch (e) { return null; } // CORS SecurityError
    }

    // Async: loads SVG into an Image, waits for onload, then decodes
    function scanSVG(svg) {
        return new Promise(function(resolve) {
            try {
                var svgRect = svg.getBoundingClientRect();
                if (svgRect.width < 20 || svgRect.height < 20) { resolve(null); return; }

                var serializer = new XMLSerializer();
                var svgStr = serializer.serializeToString(svg);
                var blob = new Blob([svgStr], { type: 'image/svg+xml;charset=utf-8' });
                var url = URL.createObjectURL(blob);

                var img = new Image();
                img.onload = function() {
                    try {
                        var canvas = document.createElement('canvas');
                        canvas.width = Math.round(svgRect.width);
                        canvas.height = Math.round(svgRect.height);
                        var ctx = canvas.getContext('2d');
                        if (!ctx) { URL.revokeObjectURL(url); resolve(null); return; }

                        ctx.drawImage(img, 0, 0, canvas.width, canvas.height);
                        var imageData = ctx.getImageData(0, 0, canvas.width, canvas.height);
                        URL.revokeObjectURL(url);
                        var result = decodeFromImageData(imageData);
                        if (result) result.source = 'svg';
                        resolve(result);
                    } catch (e) {
                        URL.revokeObjectURL(url);
                        resolve(null);
                    }
                };
                img.onerror = function() {
                    URL.revokeObjectURL(url);
                    resolve(null);
                };
                img.src = url;

                // Safety timeout — don't wait more than 2s for any single SVG
                setTimeout(function() { resolve(null); }, 2000);
            } catch (e) { resolve(null); }
        });
    }

    function scanVideo(video) {
        try {
            if (video.readyState < 2) return null; // HAVE_CURRENT_DATA
            var w = video.videoWidth, h = video.videoHeight;
            if (w < 20 || h < 20) return null;
            var canvas = document.createElement('canvas');
            var scale = Math.min(1, 1000 / Math.max(w, h));
            canvas.width = Math.round(w * scale);
            canvas.height = Math.round(h * scale);
            var ctx = canvas.getContext('2d');
            if (!ctx) return null;
            ctx.drawImage(video, 0, 0, canvas.width, canvas.height);
            var imageData = ctx.getImageData(0, 0, canvas.width, canvas.height);
            var result = decodeFromImageData(imageData);
            if (result) result.source = 'video';
            return result;
        } catch (e) { return null; } // CORS
    }

    // Main scan — collect all BSV QR results from the page
    var results = [];
    var seen = new Set();
    var MAX_ELEMENTS = 50; // Cap to prevent hangs on image-heavy pages
    var scanned = 0;

    function addResult(r) {
        if (r && !seen.has(r.value)) {
            seen.add(r.value);
            results.push(r);
        }
    }

    // Scan <img> elements (sync)
    var imgs = document.querySelectorAll('img');
    for (var i = 0; i < imgs.length && scanned < MAX_ELEMENTS; i++, scanned++) {
        addResult(scanImage(imgs[i]));
    }

    // Scan <canvas> elements (sync)
    var canvases = document.querySelectorAll('canvas');
    for (var i = 0; i < canvases.length && scanned < MAX_ELEMENTS; i++, scanned++) {
        var r = scanCanvas(canvases[i]);
        if (r) r.source = 'canvas';
        addResult(r);
    }

    // Scan <svg> elements (async — each needs image load)
    var svgs = document.querySelectorAll('svg');
    var svgPromises = [];
    for (var i = 0; i < svgs.length && scanned < MAX_ELEMENTS; i++, scanned++) {
        svgPromises.push(scanSVG(svgs[i]));
    }
    var svgResults = await Promise.all(svgPromises);
    svgResults.forEach(function(r) { addResult(r); });

    // Scan <video> elements (sync — captures current frame)
    var videos = document.querySelectorAll('video');
    for (var i = 0; i < videos.length && scanned < MAX_ELEMENTS; i++, scanned++) {
        addResult(scanVideo(videos[i]));
    }

    // Send results back via IPC
    if (window.cefMessage && window.cefMessage.send) {
        window.cefMessage.send('qr_found', [JSON.stringify(results)]);
    }
})();
