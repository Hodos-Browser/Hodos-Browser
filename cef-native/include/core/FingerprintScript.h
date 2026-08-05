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
// NOTE: navigator.plugins + navigator.webdriver spoofing REMOVED 2026-08-05 (BOT-1) —
//       native Chromium already emits the correct values and our overrides were themselves
//       detectable. Details, evidence and the tripwire pointer are inline below.
//
// What remains in this script is farbling ONLY: Canvas, WebGL readPixels, WebAudio. Each of
// those three fragments is deleted in the same commit its native Blink replacement lands
// (P4a canvas, P4b WebGL, P4c audio — the atomic per-value teardown rule, I-4).
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

    // === Navigator Plugins / navigator.webdriver — REMOVED 2026-08-05 (BOT-1) ===
    //
    // Both overrides lived here and both were REMOVED, not re-homed, because measurement
    // against our own M150 build showed native Chromium already does the right thing and
    // our overrides were making us MORE detectable, not less:
    //
    //   navigator.plugins  — we shipped a 5-entry list naming "Chrome PDF Plugin". Chromium
    //     returns the spec'd hard-coded list from whatwg/html#6738 (blink DOMPluginArray,
    //     gated on IsPdfViewerAvailable()), which names "Chromium PDF Viewer" in that slot;
    //     "Chrome PDF Plugin" is the pre-2021 name. So our spoof produced a plugin list no
    //     real Chrome has — a one-line diff against a published constant. We build with
    //     enable_pdf=true, so the native list is present and correct.
    //
    //   navigator.webdriver — native is already false (Blink's AutomationControlled feature
    //     is off unless --enable-automation / --headless / --remote-debugging-pipe /
    //     --remote-debugging-port=0; we pass none). Redefining it here put an own-property
    //     accessor on the navigator instance where real Chrome has a prototype accessor —
    //     which is precisely the prototype-tamper signal Turnstile/DataDome look for, i.e.
    //     the same class of tell this whole Blink migration exists to remove.
    //
    // Deleting them also makes both values correct when the user turns farbling OFF for a
    // site — the old overrides vanished with the script, so an opt-out silently changed the
    // bot signature. The guarantee is now structural, not injected.
    //
    // The one path that can still flip webdriver to true is the remote-debugging-port block
    // in cef_browser_shell.cpp; see the TRIPWIRE comment there. Both values are asserted at
    // P6 farbling acceptance so this is re-checked every release rather than assumed.

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
