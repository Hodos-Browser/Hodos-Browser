#pragma once

#include <string>
#include <cstdint>
#include <atomic>
#include <mutex>
#include <unordered_map>
#include <fstream>

#include <nlohmann/json.hpp>

// No platform crypto include here any more: the per-session token this class used to
// generate died with the JS injection path. The persistent per-profile seed that
// replaced it is generated and owned by FarblingPolicy (BCryptGenRandom / SecRandomCopyBytes).

/// FingerprintProtection — the browser-process POLICY inputs to fingerprint farbling.
///
/// It no longer computes or delivers any seed. Farbling itself is native, inside Blink
/// (fork patches C1/C3/C4/C5/C6), keyed by a persistent per-profile seed that
/// `FarblingPolicy` owns and that libcef's registry delivers to the renderer. What is
/// left here is the three POLICY inputs that `simple_handler.cpp :: OnBeforeBrowse`
/// collapses into the single `enabled` bit it sends with `hodos_farble_key`:
///
///   1. the global on/off toggle       -- IsEnabled() / SetEnabled()
///   2. the auth/OAuth allowlist       -- IsAuthDomain()          (host-precise)
///   3. the user's per-site opt-out    -- IsSiteEnabled() / SetSiteEnabled()
///
/// The renderer never re-decides any of this, and never sees the allowlist.
///
/// ⚠️ Everything here is SHIPPED USER-FACING CONTROL. (2) and (3) are what keep logins
/// working on auth sites and what the Privacy Shield toggle drives; do not delete them
/// while tidying. The 2026-08-09 teardown removed the JS-injection half of this class
/// (`GetDomainSeed`, the per-session token, the seed cache) because its consumer --
/// FINGERPRINT_PROTECTION_SCRIPT -- is gone. The policy half stays.
class FingerprintProtection {
public:
    static FingerprintProtection& GetInstance() {
        static FingerprintProtection instance;
        return instance;
    }

    /// Marks startup configuration as complete. Called once, immediately before
    /// LoadSiteSettings() + SetEnabled() at browser startup.
    ///
    /// ⚠️ Do NOT delete this as "empty" now that the session token is gone. IsEnabled()
    /// is `initialized_ && enabled_`, and `enabled_` defaults to TRUE -- so without this
    /// flag the class would report "farbling on" during the window before the user's
    /// stored settings have been read. It is a startup-ordering gate, not a leftover.
    void Initialize() {
        std::lock_guard<std::mutex> lock(mutex_);
        initialized_ = true;
    }

    /// Check if fingerprint protection is enabled
    bool IsEnabled() const {
        return initialized_ && enabled_;
    }

    void SetEnabled(bool enabled) {
        enabled_ = enabled;
    }

    /// Returns true if fingerprint protection is enabled for the given domain.
    /// Returns false only if the domain has an explicit per-site override set to false.
    /// Falls back to true (enabled) for unknown domains.
    bool IsSiteEnabled(const std::string& domain) {
        std::lock_guard<std::mutex> lock(siteMutex_);
        auto it = siteOverrides_.find(domain);
        if (it != siteOverrides_.end()) {
            return it->second;
        }
        return true;
    }

    /// Set per-site fingerprint protection override.
    /// If enabled=true, removes any existing override (reverts to default).
    /// If enabled=false, stores an explicit disable override and persists to disk.
    void SetSiteEnabled(const std::string& domain, bool enabled) {
        {
            std::lock_guard<std::mutex> lock(siteMutex_);
            if (enabled) {
                siteOverrides_.erase(domain);
            } else {
                siteOverrides_[domain] = false;
            }
        }
        SaveSiteSettings();
    }

    /// Load per-site overrides from fingerprint_settings.json in profileDir.
    /// Called once at startup after Initialize().
    void LoadSiteSettings(const std::string& profileDir) {
#ifdef _WIN32
        settingsFilePath_ = profileDir + "\\fingerprint_settings.json";
#else
        settingsFilePath_ = profileDir + "/fingerprint_settings.json";
#endif
        try {
            std::ifstream file(settingsFilePath_);
            if (!file.is_open()) return;
            nlohmann::json j = nlohmann::json::parse(file);
            if (j.contains("siteSettings") && j["siteSettings"].is_object()) {
                std::lock_guard<std::mutex> lock(siteMutex_);
                for (auto& [domain, settings] : j["siteSettings"].items()) {
                    if (settings.contains("enabled") && settings["enabled"].is_boolean()) {
                        siteOverrides_[domain] = settings["enabled"].get<bool>();
                    }
                }
            }
        } catch (...) {}
    }

    /// Persist current per-site overrides to fingerprint_settings.json.
    ///
    /// ⚠️ READ-MODIFY-WRITE, deliberately. This file is shared with the farbling
    /// migration: `FarblingPolicy::EnsureProfileSeed` stores the persistent per-profile
    /// `profileSeed` here too. An earlier version of this function built a fresh JSON
    /// object containing only `siteSettings` and overwrote the file, which would have
    /// silently destroyed the seed every time a user toggled Privacy Shield for any
    /// site -- rotating that profile's whole fingerprint and breaking the
    /// stable-across-restarts property the seed exists to provide. So: load whatever is
    /// there, replace only our key, write it back. Do not "simplify" this.
    void SaveSiteSettings() {
        if (settingsFilePath_.empty()) return;
        try {
            nlohmann::json j = nlohmann::json::object();
            {
                std::ifstream in(settingsFilePath_);
                if (in.is_open()) {
                    try {
                        in >> j;
                    } catch (...) {
                        j = nlohmann::json::object();
                    }
                    if (!j.is_object()) j = nlohmann::json::object();
                }
            }

            nlohmann::json site_obj = nlohmann::json::object();
            {
                std::lock_guard<std::mutex> lock(siteMutex_);
                for (auto& [domain, enabled] : siteOverrides_) {
                    site_obj[domain]["enabled"] = enabled;
                }
            }
            j["siteSettings"] = site_obj;

            std::ofstream file(settingsFilePath_);
            if (file.is_open()) {
                file << j.dump(2);
            }
        } catch (...) {}
    }

    /// Returns true if the URL is for an auth domain that should NOT get
    /// fingerprint farbling (it breaks bot detection / anti-fraud checks).
    static bool IsAuthDomain(const std::string& url) {
        std::string domain = ExtractDomain(url);
        // Convert to lowercase for comparison
        std::string lower;
        lower.resize(domain.size());
        for (size_t i = 0; i < domain.size(); i++) {
            lower[i] = static_cast<char>(std::tolower(static_cast<unsigned char>(domain[i])));
        }
        // Auth and anti-fraud domains where canvas/WebGL/audio farbling
        // can trigger bot detection or break login flows. Includes CAPTCHA
        // services, major auth providers, banking, and e-commerce sites.
        // Keep in sync with hodos-unbreak.txt auth domain exceptions.
        static const char* authDomains[] = {
            // --- Bot detection / CAPTCHA services ---
            "challenges.cloudflare.com",  // Cloudflare managed challenge
            "cf-turnstile.com",           // Cloudflare Turnstile widget
            "www.google.com",             // reCAPTCHA challenge page
            "www.gstatic.com",            // reCAPTCHA JS/assets
            "recaptcha.net",              // reCAPTCHA alternate domain
            "www.recaptcha.net",          // reCAPTCHA alternate domain
            "hcaptcha.com",               // hCaptcha
            "js.hcaptcha.com",            // hCaptcha JS
            "newassets.hcaptcha.com",     // hCaptcha assets

            // --- Per-site webcompat exceptions for Cloudflare Turnstile ---
            // Skipping only the challenge iframe is insufficient: Turnstile
            // reads the parent window's Canvas/WebGL/Audio fingerprints to
            // score the browser. Farbling the parent while leaving the iframe
            // native produces an inconsistent signal that Turnstile rejects
            // (Brave hits the same problem — see brave/brave-browser#45608).
            "whatsonchain.com",           // WoC BSV explorer — uses Turnstile
            "www.whatsonchain.com",
            "test.whatsonchain.com",

            // --- Google Auth ---
            "accounts.google.com",        // Primary Google login
            "accounts.youtube.com",       // YouTube login (Google SSO)
            "myaccount.google.com",       // Account management

            // --- Microsoft Auth ---
            "login.microsoftonline.com",  // Azure AD / Microsoft 365
            "login.live.com",             // Microsoft account
            "login.microsoft.com",        // Microsoft login

            // --- Apple Auth ---
            "appleid.apple.com",          // Apple ID login

            // --- Social Auth ---
            "www.facebook.com",           // Facebook login / OAuth
            "x.com",                      // X/Twitter — JS farbling detected as bot
            "www.x.com",
            "twitter.com",
            "www.twitter.com",
            "api.twitter.com",            // X/Twitter OAuth

            // --- Developer Platforms ---
            "github.com",                 // GitHub login + OAuth

            // --- Financial ---
            "chase.com",
            "www.chase.com",
            "bankofamerica.com",
            "www.bankofamerica.com",
            "wellsfargo.com",
            "www.wellsfargo.com",
            "paypal.com",
            "www.paypal.com",

            // --- E-commerce ---
            "amazon.com",
            "www.amazon.com",

            // --- BSV ---
            "sigmaidentity.com",
        };
        for (const auto& auth : authDomains) {
            if (lower == auth) return true;
        }
        return false;
    }

private:
    FingerprintProtection() = default;

    static std::string ExtractDomain(const std::string& url) {
        size_t start = url.find("://");
        if (start == std::string::npos) return url;
        start += 3;
        size_t end = url.find_first_of(":/", start);
        if (end == std::string::npos) end = url.size();
        return url.substr(start, end - start);
    }

    std::mutex mutex_;
    bool initialized_ = false;
    std::atomic<bool> enabled_{true};

    std::unordered_map<std::string, bool> siteOverrides_;
    std::mutex siteMutex_;
    std::string settingsFilePath_;
};
