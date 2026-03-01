#pragma once

// Fingerprint protection script injected via OnContextCreated.
// FINGERPRINT_SEED placeholder is replaced with the actual per-domain seed at injection time.
// NOTE: Screen resolution spoofing deliberately REMOVED — breakage > entropy benefit (only 3-4 bits).
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
    var _getImageData = CanvasRenderingContext2D.prototype.getImageData;
    CanvasRenderingContext2D.prototype.getImageData = function() {
        var data = _getImageData.apply(this, arguments);
        // Only farble small canvases (likely fingerprinting probes, not visible content)
        if (data.width * data.height < 65536) {
            for (var i = 0; i < data.data.length; i += 4) {
                if (rng() < 0.1) {
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

    // === WebGL Fingerprinting ===
    function protectWebGL(proto) {
        var _getParameter = proto.getParameter;
        proto.getParameter = function(param) {
            var debugInfo = this.getExtension('WEBGL_debug_renderer_info');
            if (debugInfo) {
                if (param === debugInfo.UNMASKED_VENDOR_WEBGL) {
                    return 'Google Inc. (NVIDIA)';
                }
                if (param === debugInfo.UNMASKED_RENDERER_WEBGL) {
                    return 'ANGLE (NVIDIA, NVIDIA GeForce Graphics, OpenGL 4.5)';
                }
            }
            return _getParameter.call(this, param);
        };

        var _readPixels = proto.readPixels;
        proto.readPixels = function() {
            _readPixels.apply(this, arguments);
            var pixels = arguments[arguments.length - 1];
            if (pixels && pixels.length && pixels.length < 262144) {
                for (var i = 0; i < pixels.length; i += 4) {
                    if (rng() < 0.1) {
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

    // === Navigator Properties ===
    var fakeHardwareConcurrency = 2 + Math.floor(rng() * 7);
    Object.defineProperty(navigator, 'hardwareConcurrency', {
        get: function() { return fakeHardwareConcurrency; },
        enumerable: true, configurable: true
    });

    Object.defineProperty(navigator, 'deviceMemory', {
        get: function() { return 8; },
        enumerable: true, configurable: true
    });

    Object.defineProperty(navigator, 'plugins', {
        get: function() { return []; },
        enumerable: true, configurable: true
    });

    // === AudioContext Farbling ===
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
