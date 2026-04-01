#pragma once

// Fingerprint protection script injected via OnContextCreated.
// FINGERPRINT_SEED placeholder is replaced with the actual per-domain seed at injection time.
//
// Design: Brave-style "subtle farbling" — small, imperceptible perturbations to
// high-entropy fingerprinting APIs (canvas, WebGL pixels, audio). Does NOT override
// real hardware values (GPU, CPU cores, RAM) because inconsistencies between spoofed
// values and real behavior are detectable and trigger bot detection.
//
// NOTE: Screen resolution spoofing deliberately REMOVED — breakage > entropy benefit (only 3-4 bits).
// NOTE: hardwareConcurrency/deviceMemory spoofing REMOVED — low entropy (~3-4 bits each),
//       cross-referenced by anti-fraud systems against real performance characteristics.
// NOTE: WebGL vendor/renderer spoofing REMOVED — hardcoded GPU string creates detectable
//       inconsistency with actual WebGL extension list and rendering behavior.
static const char* FINGERPRINT_PROTECTION_SCRIPT = R"JS(
(function(seed) {
    'use strict';

    // Mulberry32 PRNG seeded with per-domain session seed
    function mulberry32(a) {
        return function() {
            a |= 0; a = a + 0x6D2B79F5 | 0;
            var t = Math.imul(a ^ a >>> 15, 1 | a);
            t = t + Math.imul(t ^ t >>> 7, 61 | t) ^ t;
            return ((t ^ t >>> 14) >>> 0) / 4294967296;
        };
    }
    var rng = mulberry32(seed);

    // === Canvas Farbling ===
    // Subtle pixel noise on small canvases (fingerprinting probes).
    // 3% of pixels get LSB flipped — imperceptible but changes the hash.
    var _getImageData = CanvasRenderingContext2D.prototype.getImageData;
    CanvasRenderingContext2D.prototype.getImageData = function() {
        var data = _getImageData.apply(this, arguments);
        if (data.width * data.height < 65536) {
            for (var i = 0; i < data.data.length; i += 4) {
                if (rng() < 0.03) {
                    data.data[i] ^= 1;
                }
            }
        }
        return data;
    };

    var _toDataURL = HTMLCanvasElement.prototype.toDataURL;
    HTMLCanvasElement.prototype.toDataURL = function() {
        var canvas = this;
        if (canvas.width * canvas.height < 65536) {
            var ctx = canvas.getContext('2d');
            if (ctx) {
                var imgData = ctx.getImageData(0, 0, canvas.width, canvas.height);
                ctx.putImageData(imgData, 0, 0);
            }
        }
        return _toDataURL.apply(this, arguments);
    };

    var _toBlob = HTMLCanvasElement.prototype.toBlob;
    HTMLCanvasElement.prototype.toBlob = function(callback) {
        var canvas = this;
        if (canvas.width * canvas.height < 65536) {
            var ctx = canvas.getContext('2d');
            if (ctx) {
                var imgData = ctx.getImageData(0, 0, canvas.width, canvas.height);
                ctx.putImageData(imgData, 0, 0);
            }
        }
        return _toBlob.apply(this, arguments);
    };

    // === WebGL readPixels Farbling ===
    // Subtle pixel noise on WebGL readPixels (fingerprinting probes).
    // Vendor/renderer strings are NOT spoofed — hardcoded GPU strings create
    // detectable inconsistencies with real WebGL extensions and performance.
    function protectWebGL(proto) {
        var _readPixels = proto.readPixels;
        proto.readPixels = function() {
            _readPixels.apply(this, arguments);
            var pixels = arguments[arguments.length - 1];
            if (pixels && pixels.length && pixels.length < 262144) {
                for (var i = 0; i < pixels.length; i += 4) {
                    if (rng() < 0.03) {
                        pixels[i] ^= 1;
                    }
                }
            }
        };
    }

    if (typeof WebGLRenderingContext !== 'undefined') {
        protectWebGL(WebGLRenderingContext.prototype);
    }
    if (typeof WebGL2RenderingContext !== 'undefined') {
        protectWebGL(WebGL2RenderingContext.prototype);
    }

    // === Navigator Plugins (realistic Chrome 136 set) ===
    // Real Chrome exposes 5 PDF-related plugins. Empty array is a bot signal.
    var fakePluginData = [
        { name: 'PDF Viewer', filename: 'internal-pdf-viewer', description: 'Portable Document Format' },
        { name: 'Chrome PDF Plugin', filename: 'internal-pdf-viewer', description: 'Portable Document Format' },
        { name: 'Chrome PDF Viewer', filename: 'internal-pdf-viewer', description: 'Portable Document Format' },
        { name: 'Microsoft Edge PDF Viewer', filename: 'internal-pdf-viewer', description: 'Portable Document Format' },
        { name: 'WebKit built-in PDF', filename: 'internal-pdf-viewer', description: 'Portable Document Format' }
    ];
    var fakePluginArray = {length: fakePluginData.length};
    for (var pi = 0; pi < fakePluginData.length; pi++) {
        var fp = {
            name: fakePluginData[pi].name,
            filename: fakePluginData[pi].filename,
            description: fakePluginData[pi].description,
            length: 1
        };
        fakePluginArray[pi] = fp;
        fakePluginArray[fakePluginData[pi].name] = fp;
    }
    fakePluginArray.item = function(i) { return this[i] || null; };
    fakePluginArray.namedItem = function(n) { return this[n] || null; };
    fakePluginArray.refresh = function() {};
    Object.setPrototypeOf(fakePluginArray, PluginArray.prototype);
    Object.defineProperty(navigator, 'plugins', {
        get: function() { return fakePluginArray; },
        enumerable: true, configurable: true
    });

    // === navigator.webdriver ===
    // Explicitly set to false — absence or true triggers bot detection.
    Object.defineProperty(navigator, 'webdriver', {
        get: function() { return false; },
        enumerable: true, configurable: true
    });

    // === AudioContext Farbling ===
    // Subtle audio sample noise — imperceptible but changes the fingerprint hash.
    if (typeof AudioBuffer !== 'undefined') {
        var _getChannelData = AudioBuffer.prototype.getChannelData;
        AudioBuffer.prototype.getChannelData = function(channel) {
            var data = _getChannelData.call(this, channel);
            var fudge = 1.0 + (rng() - 0.5) * 0.0000004;
            for (var i = 0; i < data.length; i++) {
                data[i] *= fudge;
            }
            return data;
        };
    }

    if (typeof AnalyserNode !== 'undefined') {
        var _getFloatFrequencyData = AnalyserNode.prototype.getFloatFrequencyData;
        AnalyserNode.prototype.getFloatFrequencyData = function(array) {
            _getFloatFrequencyData.call(this, array);
            var fudge = 1.0 + (rng() - 0.5) * 0.0000004;
            for (var i = 0; i < array.length; i++) {
                array[i] *= fudge;
            }
        };
    }

})(FINGERPRINT_SEED);
)JS";
