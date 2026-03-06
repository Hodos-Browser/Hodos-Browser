# Cloudflare & Bot-Detection Bypass Implementation Plan

**Date**: March 6, 2026
**Project**: Hodos Browser
**Status**: Research & Strategy (Draft)

---

## 1. Executive Summary

Modern bot-detection (Cloudflare Turnstile, Akamai, DataDome) uses **Multi-Layer Fingerprinting** to distinguish between "Retail Chrome" and "Automated Browsers" (like CEF/Playwright). Hodos Browser must achieve **Protocol-Level Impersonation** to ensure users don't get stuck on "Just a moment" pages or CAPTCHA loops.

The goal is to move from "Fingerprint Protection" (which makes the user unique) to **"Fingerprint Alignment"** (which makes the user look like one of millions of Chrome users).

---

## 2. Implementation Strategy: The Four Pillars

### 2.1 TLS Handshake Alignment (The "Invisible" Layer)
*   **The Problem**: Cloudflare uses JA4 to hash the TLS `Client Hello`. CEF's default cipher suite order and extension list often differ from Retail Chrome.
*   **Source-Build Action**: Modify `third_party/boringssl/src/ssl/t1_lib.cc` to:
    1.  Ensure **TLS Extension Permutation** matches Chrome 136 exactly.
    2.  Align the **Cipher Suite** list and priority order with the current stable Chrome release.
    3.  Match **ALPN** (Application-Layer Protocol Negotiation) strings.
*   **Why**: This check happens *before* any JS or HTML is sent. If this fails, you get the "Checking your browser" loop.

### 2.2 HTTP/2 Fingerprint Matching
*   **The Problem**: Modern CDNs fingerprint the H2 layer (Window sizes, SETTINGS frames, pseudo-header ordering).
*   **Source-Build Action**: Adjust the default values in `net/http/http2_settings.h` to match Chrome 136:
    *   `SETTINGS_INITIAL_WINDOW_SIZE`
    *   `SETTINGS_MAX_CONCURRENT_STREAMS`
    *   `SETTINGS_HEADER_TABLE_SIZE`
*   **Why**: A mismatch between the User-Agent (claiming to be Chrome) and the H2 settings (looking like a generic library) is an instant red flag.

### 2.3 JavaScript Environment & API Mocking
*   **The Problem**: Bot-detection scripts check for specific "Chrome-isms" that CEF sometimes lacks.
*   **Frontend/CEF Action**:
    1.  **Inject `window.chrome`**: Ensure this object is present and populated with standard sub-properties (`csi`, `loadTimes`, `runtime`).
    2.  **Webdriver Spoofing**: Ensure `navigator.webdriver` is strictly `false` (CEF sometimes leaves it `undefined` or `true`).
    3.  **Plugin Enumeration**: Match the default list of "dummy" plugins that Chrome reports (PDF Viewer, etc.).

### 2.4 Privacy-Shield "Alignment" (Default Settings)
*   **The Problem**: Aggressive fingerprinting protection (randomizing canvas, WebGL) makes the user look **extremely unique**, which bot-detectors treat as "Suspicious/Automated."
*   **Recommended Default**:
    1.  **Level 1: Standard (Default)** — Hodos identifies as a standard Chrome 136 user. Privacy protections use "Alignment" (returning the most common Chrome values for canvas/fonts) rather than "Randomization."
    2.  **Level 2: Strict (User Choice)** — Hodos uses aggressive randomization. The UI warns the user: *"This may cause some websites (like Twitter or Discord) to block access."*

---

## 3. Automation & Auto-Detection

To ensure users never get "stuck," we will implement an **Auto-Relaxation** engine:

1.  **Signature Detection**: The C++ Shell monitors page titles and DOM markers for "Just a moment...", "Checking your browser", or specific Cloudflare/DataDome div IDs.
2.  **Automatic Shift**: If a bot-challenge is detected, Hodos automatically shifts that specific tab into **"Stealth Alignment Mode"** for 60 seconds.
3.  **UI Feedback**: A subtle "Shield" icon animation in the toolbar indicates Hodos is "negotiating" the bot-wall on the user's behalf.

---

## 4. Verification & Testing Tools

Once the build is integrated, we must verify against:
*   [JA4er.com](https://ja4er.com/) — Verify TLS JA4 fingerprint matches Chrome.
*   [BrowserLeaks TLS](https://tls.browserleaks.com/) — Deep dive into cipher suites.
*   [CreepJS](https://abrahamjuliot.github.io/creepjs/) — Advanced JS fingerprinting audit.
*   [SFP (Scraping-Fingerprint-Pool)](https://github.com/kaliiiiiiiiii/scripter-fingerprint) — Industry-standard benchmark.

---

## 5. Maintenance Roadmap

Bot-detection is an arms race.
*   **Quarterly Audit**: Every time we update the CEF branch (e.g., from 136 to 138), we must re-verify the TLS/H2 fingerprints against the corresponding Retail Chrome release.
*   **BoringSSL Patches**: Maintain a set of `git patches` for the `boringssl` submodule to easily re-apply our "Stealth Handshake" during new builds.

---

*Document created March 6, 2026. This strategy prioritizes "being one of many" over "being invisible."*
