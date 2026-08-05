#pragma once

#include <array>
#include <cstdint>
#include <string>

/// FarblingPolicy — the browser-process half of the Blink farbling migration (C2).
///
/// Owns the three things the renderer must NOT do for itself:
///   1. the persistent per-profile master seed,
///   2. the first-party registrable domain (eTLD+1) the seed is keyed on,
///   3. the HMAC that turns those into the per-origin key we actually deliver.
///
/// THREAT MODEL. The master `profile_seed` never leaves the browser process and never
/// appears on any command line -- a cmdline value is readable by every local process.
/// Only the derived per-origin `domain_key` crosses into a renderer, so a compromised
/// renderer learns its own site's key and nothing about any other site.
///
/// WHY PERSISTENT. The seed is generated once per profile and kept. A per-session seed
/// (which is what Brave does) changes the fingerprint on every launch, which reads as a
/// new device to any site doing fingerprint-assisted re-auth -- that is the login
/// breakage this migration exists to fix. Stable across restarts, different per site,
/// different per profile.
///
/// This is browsing-privacy state, NOT wallet or key material (Invariants #1/#2 are
/// untouched). It lives beside the other privacy settings in the profile directory.
namespace FarblingPolicy {

/// Returns the registrable domain ("eTLD+1") for a host, e.g.
///   "accounts.google.com"  -> "google.com"
///   "www.example.co.uk"    -> "example.co.uk"
///   "localhost"            -> "localhost"
///
/// ⚠️ WHY NOT `EphemeralCookieManager::ExtractSiteFromUrl`. That helper takes the last
/// two labels and documents "example.co.uk -> co.uk" as an accepted limitation. For
/// cookie session grouping that is tolerable. For farbling it is NOT: it would collapse
/// every *.co.uk site onto a single seed, so unrelated first parties would share one
/// fingerprint -- exactly the cross-site linkage first-party keying exists to prevent.
/// Deliberately a separate implementation; do not "de-duplicate" these two.
///
/// Accepts a bare host, not a URL. Input is lowercased; a trailing dot and any port are
/// stripped. IP literals and single-label hosts are returned unchanged.
std::string RegistrableDomain(const std::string& host);

/// Convenience: extract the host from a URL and reduce it to its registrable domain.
std::string RegistrableDomainFromUrl(const std::string& url);

/// Loads the profile's master seed, generating and persisting one on first use.
///
/// `profile_dir` is the per-profile directory; the seed is stored as a hex string in the
/// existing `fingerprint_settings.json` beside the per-site toggles, so farbling state
/// stays in one file and is cleared as a unit when the user clears that profile's data.
///
/// Returns false only if a seed could neither be read nor generated, in which case the
/// caller MUST NOT farble -- see the fail-closed note on ComputeDomainKey.
bool EnsureProfileSeed(const std::string& profile_dir,
                       std::array<uint8_t, 32>& out_seed);

/// domain_key = HMAC-SHA256(profile_seed, registrable_domain)
///
/// Deterministic, so the same profile + site always yields the same key, which is what
/// makes a site's farbled values stable across restarts. One-way, so a site that somehow
/// recovered its own key still cannot derive another site's.
///
/// Returns false on any crypto failure. FAIL CLOSED: on false the caller must deliver
/// `farbling_enabled = false` and let the renderer return native values. Never fall back
/// to a zero or constant key -- a degenerate constant-seeded perturbation is a WORSE
/// fingerprint than not farbling at all, and it would silently defeat the auth-domain
/// exemption, which depends on the bypass being a true native pass-through.
bool ComputeDomainKey(const std::array<uint8_t, 32>& profile_seed,
                      const std::string& registrable_domain,
                      std::array<uint8_t, 32>& out_key);

/// Loads (or generates) this profile's seed once at startup and caches it in-process.
/// Call beside FingerprintProtection::LoadSiteSettings, with the same profile dir.
/// Safe to call more than once; only the first call touches disk.
void InitializeForProfile(const std::string& profile_dir);

/// Per-navigation convenience: derive the key for `url`'s first party using the
/// cached seed. Returns false when the seed is unavailable or the key cannot be
/// derived -- and on false the caller MUST NOT farble (see ComputeDomainKey).
bool DomainKeyForUrl(const std::string& url, std::array<uint8_t, 32>& out_key);

/// Hex helpers for persistence (lowercase, no separators). DecodeHex returns false on
/// any malformed input rather than partially decoding.
std::string EncodeHex(const std::array<uint8_t, 32>& bytes);
bool DecodeHex(const std::string& hex, std::array<uint8_t, 32>& out_bytes);

}  // namespace FarblingPolicy
