#pragma once

#include <string>
#include <cstdint>
#include <array>
#include <mutex>
#include <unordered_map>
#include <random>

#ifdef _WIN32
#include <windows.h>
#include <wincrypt.h>
#pragma comment(lib, "advapi32.lib")
#elif defined(__APPLE__)
#include <Security/Security.h>
#endif

/// FingerprintProtection — per-session, per-domain fingerprint farbling seed system.
///
/// On startup, generates a random 32-byte session token (memory-only, never persisted).
/// For each domain, computes a deterministic seed via a simple hash so that:
///   - Same domain within a session → same seed → consistent farbling
///   - Different domains → different seeds
///   - Different sessions → different seeds
///
/// The seed is passed to the renderer process where it initializes a PRNG
/// used by Canvas/WebGL/Navigator/Audio farbling overrides.
class FingerprintProtection {
public:
    static FingerprintProtection& GetInstance() {
        static FingerprintProtection instance;
        return instance;
    }

    /// Initialize session token (called once at startup)
    void Initialize() {
        std::lock_guard<std::mutex> lock(mutex_);
        // Generate random session token
#ifdef _WIN32
        HCRYPTPROV hProv = 0;
        if (CryptAcquireContextA(&hProv, nullptr, nullptr, PROV_RSA_FULL,
                                  CRYPT_VERIFYCONTEXT | CRYPT_SILENT)) {
            CryptGenRandom(hProv, (DWORD)sessionToken_.size(), sessionToken_.data());
            CryptReleaseContext(hProv, 0);
        } else {
            // Fallback to mt19937
            std::random_device rd;
            std::mt19937_64 gen(rd());
            std::uniform_int_distribution<unsigned int> dist(0, 255);
            for (auto& byte : sessionToken_) {
                byte = static_cast<uint8_t>(dist(gen));
            }
        }
#elif defined(__APPLE__)
        (void)SecRandomCopyBytes(kSecRandomDefault, sessionToken_.size(), sessionToken_.data());
#else
        std::random_device rd;
        std::mt19937_64 gen(rd());
        std::uniform_int_distribution<unsigned int> dist(0, 255);
        for (auto& byte : sessionToken_) {
            byte = static_cast<uint8_t>(dist(gen));
        }
#endif
        initialized_ = true;
    }

    /// Get per-domain seed for fingerprint farbling.
    /// Extracts eTLD+1 from URL and computes a hash with the session token.
    uint32_t GetDomainSeed(const std::string& url) {
        std::lock_guard<std::mutex> lock(mutex_);
        if (!initialized_) return 0;

        std::string domain = ExtractDomain(url);

        // Check cache
        auto it = seedCache_.find(domain);
        if (it != seedCache_.end()) {
            return it->second;
        }

        // Compute seed: simple hash combining session token + domain
        uint32_t seed = 0;
        // Mix in session token
        for (size_t i = 0; i < sessionToken_.size(); i += 4) {
            uint32_t chunk = 0;
            for (size_t j = 0; j < 4 && (i + j) < sessionToken_.size(); j++) {
                chunk |= static_cast<uint32_t>(sessionToken_[i + j]) << (j * 8);
            }
            seed ^= chunk;
            seed = (seed << 13) | (seed >> 19);
            seed *= 0x5bd1e995;
        }
        // Mix in domain
        for (char c : domain) {
            seed ^= static_cast<uint32_t>(c);
            seed = (seed << 5) | (seed >> 27);
            seed *= 0x1b873593;
        }
        seed ^= seed >> 16;
        seed *= 0x85ebca6b;
        seed ^= seed >> 13;

        seedCache_[domain] = seed;
        return seed;
    }

    /// Check if fingerprint protection is enabled
    bool IsEnabled() const {
        return initialized_ && enabled_;
    }

    void SetEnabled(bool enabled) {
        enabled_ = enabled;
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
        // Check against known auth domains + reCAPTCHA/resource domains.
        // Fingerprint farbling (canvas noise, WebGL spoofing) breaks bot
        // detection / anti-fraud checks on these domains.
        static const char* authDomains[] = {
            "accounts.google.com",
            "myaccount.google.com",
            "www.google.com",       // reCAPTCHA challenge page
            "www.gstatic.com",      // reCAPTCHA JS/assets
            "ssl.gstatic.com",      // Google login page static assets
            "login.microsoftonline.com",
            "login.live.com",
            "login.microsoft.com",
            "appleid.apple.com",
            "github.com",
            "www.facebook.com",
            "discord.com",
            "x.com",
            "twitter.com",
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
    std::array<uint8_t, 32> sessionToken_{};
    std::unordered_map<std::string, uint32_t> seedCache_;
    bool initialized_ = false;
    bool enabled_ = true;
};
