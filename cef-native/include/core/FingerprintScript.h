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
// What remains in this script is farbling ONLY: WebGL readPixels and WebAudio. Each fragment
// is deleted in the same commit its native Blink replacement lands (the atomic per-value
// teardown rule, I-4).
// NOTE: Canvas farbling REMOVED 2026-08-06 (P4a / C3) — now native in Blink. See the marker
//       where it used to be, below, for why leaving it alongside the native patch would have
//       CORRUPTED canvases rather than merely double-farbling them.
// Remaining: WebGL readPixels (P4b), WebAudio (P4c).
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

    // === Canvas Farbling — DELETED 2026-08-06 (P4a / C3) ===
    // Canvas is now farbled natively in Blink, at API-call time, inside
    // BaseRenderingContext2D::getImageDataInternal and the two encode callers of
    // HTMLCanvasElement::Snapshot (ToDataURLInternal, toBlob). Deleted in the SAME
    // commit the native patch landed, per the atomic per-value teardown rule (I-4).
    //
    // Leaving both live would not merely double-perturb. The toDataURL/toBlob
    // overrides here farbled by doing a getImageData -> putImageData ROUND-TRIP,
    // so once native farbling is on they would read native-farbled pixels and
    // write them back INTO the canvas -- mutating the source, not just the
    // readback, and compounding fresh noise on every read until the image is
    // destroyed. That is corruption, not double-farbling.
    //
    // WebGL readPixels and WebAudio below are deliberately still here: their
    // native replacements land in P4b and P4c, and each goes out with its own.

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
