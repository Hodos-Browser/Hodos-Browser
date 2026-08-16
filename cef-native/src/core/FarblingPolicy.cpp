#include "../../include/core/FarblingPolicy.h"

#include <algorithm>
#include <cctype>
#include <cstring>
#include <fstream>
#include <mutex>
#include <unordered_set>
#include <vector>

#include <nlohmann/json.hpp>
#include <openssl/hmac.h>
#include <openssl/sha.h>

#ifdef _WIN32
#include <windows.h>
#include <bcrypt.h>
#pragma comment(lib, "bcrypt.lib")
#elif defined(__APPLE__)
#include <Security/Security.h>
#endif

#include "../../include/core/Logger.h"

#define LOG_INFO_FP(msg) Logger::Log(msg, 1, 2)
#define LOG_WARN_FP(msg) Logger::Log(msg, 2, 2)
#define LOG_ERROR_FP(msg) Logger::Log(msg, 3, 2)

namespace FarblingPolicy {
namespace {

std::mutex g_seed_mutex;

// Cached once at startup so the per-navigation path never touches disk.
std::mutex g_cached_mutex;
std::array<uint8_t, 32> g_cached_seed{};
bool g_cached_valid = false;

// ---------------------------------------------------------------------------
// Multi-label public suffixes.
//
// This is a curated subset of the Public Suffix List, not the whole thing. It exists to
// answer one question correctly: "are the last two labels of this host actually a public
// suffix, such that the registrable domain is three labels rather than two?"
//
// Getting that wrong in the OVER-grouping direction is a privacy bug -- unrelated first
// parties would be handed the same farbling key and become linkable -- so the entries
// here are the ones that carry real traffic. Getting it wrong in the UNDER-grouping
// direction merely means two subdomains of one site farble differently, which costs
// consistency, not privacy.
//
// ⚠️ Known gap: an unlisted multi-label suffix over-groups. The correct long-term fix is
// to bundle the real PSL (or reach Chromium's net::registry_controlled_domains, which the
// CEF embedder API does not currently expose). Until then, ADD TO THIS TABLE rather than
// widening the fallback.
const std::unordered_set<std::string>& MultiLabelSuffixes() {
  static const std::unordered_set<std::string> kSuffixes = {
      // United Kingdom
      "co.uk", "org.uk", "me.uk", "ltd.uk", "plc.uk", "net.uk", "sch.uk", "ac.uk",
      "gov.uk", "nhs.uk", "police.uk", "mod.uk",
      // Australia / New Zealand
      "com.au", "net.au", "org.au", "edu.au", "gov.au", "asn.au", "id.au",
      "co.nz", "net.nz", "org.nz", "govt.nz", "ac.nz", "school.nz", "geek.nz",
      // Japan / Korea / China / Taiwan / Hong Kong
      "co.jp", "ne.jp", "or.jp", "ac.jp", "go.jp", "ad.jp", "ed.jp", "gr.jp", "lg.jp",
      "co.kr", "ne.kr", "or.kr", "re.kr", "pe.kr", "go.kr", "ac.kr",
      "com.cn", "net.cn", "org.cn", "gov.cn", "edu.cn", "ac.cn",
      "com.tw", "net.tw", "org.tw", "edu.tw", "gov.tw",
      "com.hk", "net.hk", "org.hk", "edu.hk", "gov.hk", "idv.hk",
      // South & South-East Asia
      "co.in", "net.in", "org.in", "gen.in", "firm.in", "ind.in", "gov.in", "ac.in",
      "edu.in", "res.in", "nic.in",
      "com.sg", "net.sg", "org.sg", "edu.sg", "gov.sg",
      "com.my", "net.my", "org.my", "edu.my", "gov.my",
      "co.id", "or.id", "ac.id", "go.id", "web.id", "sch.id",
      "co.th", "in.th", "ac.th", "go.th", "or.th", "net.th",
      "com.ph", "net.ph", "org.ph", "edu.ph", "gov.ph",
      "com.pk", "net.pk", "org.pk", "edu.pk", "gov.pk",
      "com.bd", "net.bd", "org.bd", "edu.bd", "gov.bd",
      "com.vn", "net.vn", "org.vn", "edu.vn", "gov.vn",
      // Middle East / Africa
      "co.il", "org.il", "net.il", "ac.il", "gov.il", "muni.il", "k12.il",
      "co.za", "org.za", "net.za", "web.za", "gov.za", "ac.za", "edu.za",
      "com.tr", "net.tr", "org.tr", "edu.tr", "gov.tr", "bel.tr", "web.tr",
      "com.sa", "net.sa", "org.sa", "edu.sa", "gov.sa",
      "com.eg", "net.eg", "org.eg", "edu.eg", "gov.eg",
      "co.ke", "or.ke", "ac.ke", "go.ke", "ne.ke",
      "com.ng", "net.ng", "org.ng", "edu.ng", "gov.ng",
      "com.gh", "com.ma", "co.ma",
      // Americas
      "com.br", "net.br", "org.br", "edu.br", "gov.br", "art.br", "blog.br",
      "com.ar", "net.ar", "org.ar", "edu.ar", "gob.ar", "gov.ar",
      "com.mx", "net.mx", "org.mx", "edu.mx", "gob.mx",
      "com.co", "net.co", "org.co", "edu.co", "gov.co",
      "com.pe", "net.pe", "org.pe", "edu.pe", "gob.pe",
      "com.ve", "net.ve", "org.ve", "edu.ve", "gob.ve",
      "com.cl", "co.cl", "gob.cl",
      "com.uy", "com.ec", "com.bo", "com.py", "com.do", "com.gt", "com.pa",
      "co.cr", "com.cu", "com.ni", "com.sv", "com.hn",
      // Europe
      "co.at", "or.at", "ac.at", "gv.at",
      "com.es", "org.es", "nom.es", "gob.es", "edu.es",
      "com.pl", "net.pl", "org.pl", "edu.pl", "gov.pl", "waw.pl", "info.pl",
      "com.pt", "org.pt", "edu.pt", "gov.pt",
      "com.gr", "net.gr", "org.gr", "edu.gr", "gov.gr",
      "com.ro", "org.ro", "net.ro", "gov.ro",
      "com.ua", "net.ua", "org.ua", "in.ua", "kiev.ua", "gov.ua", "edu.ua",
      "com.ru", "net.ru", "org.ru", "msk.ru", "spb.ru",
      "com.hr", "com.cy", "com.mt", "com.ee", "com.lv", "com.mk", "com.ba",
      "co.rs", "org.rs", "edu.rs", "gov.rs",
      "co.hu", "org.hu", "gov.hu",
      "gov.it", "edu.it",
      "asso.fr", "gouv.fr", "com.fr", "tm.fr",
      "co.no", "gov.se", "com.se", "org.se",
      // Generic / infrastructure that behave as public suffixes in practice
      "com.mm", "com.kh", "com.la", "com.np", "com.lk", "com.fj", "com.pg",
      // ---------------------------------------------------------------------------
      // SHARED HOSTING (added 2026-08-15, and this was a MEASURED bug, not a tidy-up).
      //
      // Every entry below hands each customer their own subdomain of one domain, so
      // without them the reduction collapses UNRELATED SITES ONTO ONE KEY and those
      // sites see identical farbled values -- i.e. either can recognise a visitor on
      // the other. That is a direct C-2 unlinkability violation.
      //
      // MEASURED before the fix (farbling_psl_linkability_check.py):
      //     squidfunk.github.io  ->  51724237
      //     microsoft.github.io  ->  51724237   <- same key, unrelated owners
      // with the separation control green (example.com != example.org), so the rig
      // could demonstrably see a key difference and these two genuinely had none.
      //
      // ⚠️ This is a BLOCKLIST and will always lag the real Public Suffix List. It
      // covers the platforms that host the overwhelming majority of affected traffic;
      // it does not make the reduction correct in general. The header's standing advice
      // applies -- ADD HERE rather than widening the fallback, and if the real PSL is
      // ever adopted it must REPLACE this reduction in one place used by both the
      // browser and the renderer. Two independent notions of "registrable domain" fail
      // closed silently, which is why hodos_farbling_registry.h forbids the renderer
      // deriving its own.
      "github.io", "gitlab.io", "githubusercontent.com",
      "workers.dev", "pages.dev", "r2.dev",
      "vercel.app", "netlify.app", "netlify.com",
      "herokuapp.com", "herokudns.com",
      "web.app", "firebaseapp.com", "appspot.com",
      "blogspot.com", "wordpress.com", "tumblr.com", "neocities.org",
      "glitch.me", "repl.co", "replit.dev", "surge.sh", "onrender.com",
      "azurewebsites.net", "cloudfront.net", "amplifyapp.com",
      "elasticbeanstalk.com",
      "myshopify.com", "squarespace.com", "webflow.io", "notion.site",
      "readthedocs.io", "gitbook.io", "substack.com",
  };
  return kSuffixes;
}

std::string ToLowerAscii(std::string s) {
  std::transform(s.begin(), s.end(), s.begin(),
                 [](unsigned char c) { return static_cast<char>(std::tolower(c)); });
  return s;
}

bool LooksLikeIpLiteral(const std::string& host) {
  if (host.find(':') != std::string::npos) {
    return true;  // IPv6 literal (already bracket-stripped by the caller).
  }
  // IPv4 if every character is a digit or a dot AND it has no alphabetic label.
  return !host.empty() &&
         host.find_first_not_of("0123456789.") == std::string::npos;
}

bool RandomBytes(uint8_t* out, size_t len) {
#ifdef _WIN32
  // BCryptGenRandom, NOT the CryptGenRandom used by FingerprintProtection::Initialize --
  // that API is deprecated by Microsoft. New key material should not extend it.
  NTSTATUS st = BCryptGenRandom(nullptr, out, static_cast<ULONG>(len),
                                BCRYPT_USE_SYSTEM_PREFERRED_RNG);
  return st == 0;  // STATUS_SUCCESS
#elif defined(__APPLE__)
  return SecRandomCopyBytes(kSecRandomDefault, len, out) == errSecSuccess;
#else
  return false;
#endif
}

std::string SettingsPath(const std::string& profile_dir) {
#ifdef _WIN32
  return profile_dir + "\\fingerprint_settings.json";
#else
  return profile_dir + "/fingerprint_settings.json";
#endif
}

}  // namespace

std::string RegistrableDomain(const std::string& host_in) {
  std::string host = ToLowerAscii(host_in);

  // Strip a bracketed IPv6 literal's brackets, any port, and a single trailing dot.
  if (!host.empty() && host.front() == '[') {
    size_t close = host.find(']');
    if (close != std::string::npos) {
      return host.substr(0, close + 1);  // IP literal: not a registrable domain.
    }
  }
  size_t colon = host.rfind(':');
  size_t last_dot_for_port = host.rfind('.');
  if (colon != std::string::npos &&
      (last_dot_for_port == std::string::npos || colon > last_dot_for_port)) {
    host = host.substr(0, colon);
  }
  if (!host.empty() && host.back() == '.') {
    host.pop_back();
  }
  if (host.empty()) {
    return "";
  }
  if (LooksLikeIpLiteral(host)) {
    return host;
  }

  // Split into labels.
  std::vector<std::string> labels;
  size_t start = 0;
  while (true) {
    size_t dot = host.find('.', start);
    if (dot == std::string::npos) {
      labels.push_back(host.substr(start));
      break;
    }
    labels.push_back(host.substr(start, dot - start));
    start = dot + 1;
  }

  // "localhost", or any single-label host, is its own site.
  if (labels.size() < 2) {
    return host;
  }

  const std::string last_two = labels[labels.size() - 2] + "." + labels.back();
  if (MultiLabelSuffixes().count(last_two) > 0) {
    // Public suffix is two labels, so the registrable domain is three.
    if (labels.size() < 3) {
      // The host IS the public suffix (e.g. "co.uk"). There is no registrable domain;
      // return it unchanged rather than inventing one.
      return host;
    }
    return labels[labels.size() - 3] + "." + last_two;
  }

  // Default: public suffix is one label, registrable domain is the last two. This is
  // correct for .com/.org/.io/.dev and every new gTLD.
  return last_two;
}

std::string RegistrableDomainFromUrl(const std::string& url) {
  size_t scheme_end = url.find("://");
  size_t host_start = (scheme_end != std::string::npos) ? scheme_end + 3 : 0;
  size_t host_end = url.find_first_of("/?#", host_start);
  std::string host = (host_end == std::string::npos)
                         ? url.substr(host_start)
                         : url.substr(host_start, host_end - host_start);
  // Drop any userinfo ("user:pass@host").
  size_t at = host.rfind('@');
  if (at != std::string::npos) {
    host = host.substr(at + 1);
  }
  return RegistrableDomain(host);
}

std::string EncodeHex(const std::array<uint8_t, 32>& bytes) {
  static const char* kHex = "0123456789abcdef";
  std::string out;
  out.reserve(bytes.size() * 2);
  for (uint8_t b : bytes) {
    out.push_back(kHex[(b >> 4) & 0x0F]);
    out.push_back(kHex[b & 0x0F]);
  }
  return out;
}

bool DecodeHex(const std::string& hex, std::array<uint8_t, 32>& out_bytes) {
  if (hex.size() != out_bytes.size() * 2) {
    return false;
  }
  auto nibble = [](char c, uint8_t& v) -> bool {
    if (c >= '0' && c <= '9') { v = static_cast<uint8_t>(c - '0'); return true; }
    if (c >= 'a' && c <= 'f') { v = static_cast<uint8_t>(c - 'a' + 10); return true; }
    if (c >= 'A' && c <= 'F') { v = static_cast<uint8_t>(c - 'A' + 10); return true; }
    return false;
  };
  for (size_t i = 0; i < out_bytes.size(); ++i) {
    uint8_t hi = 0, lo = 0;
    if (!nibble(hex[i * 2], hi) || !nibble(hex[i * 2 + 1], lo)) {
      return false;  // Malformed: reject wholesale, never decode partially.
    }
    out_bytes[i] = static_cast<uint8_t>((hi << 4) | lo);
  }
  return true;
}

bool EnsureProfileSeed(const std::string& profile_dir,
                       std::array<uint8_t, 32>& out_seed) {
  std::lock_guard<std::mutex> lock(g_seed_mutex);

  const std::string path = SettingsPath(profile_dir);

  // Read the whole document, not just our field: this file also holds the user's
  // per-site Privacy Shield toggles, and a read-modify-write that dropped them would
  // silently revoke a shipped user setting.
  nlohmann::json doc = nlohmann::json::object();
  try {
    std::ifstream in(path);
    if (in.is_open()) {
      in >> doc;
      if (!doc.is_object()) {
        doc = nlohmann::json::object();
      }
    }
  } catch (...) {
    // A corrupt settings file must not brick farbling; fall through and regenerate.
    LOG_WARN_FP("🔒 FarblingPolicy: fingerprint_settings.json unreadable/!json; "
                "regenerating the profile seed (per-site toggles may be lost)");
    doc = nlohmann::json::object();
  }

  if (doc.contains("profileSeed") && doc["profileSeed"].is_string()) {
    if (DecodeHex(doc["profileSeed"].get<std::string>(), out_seed)) {
      return true;
    }
    LOG_WARN_FP("🔒 FarblingPolicy: stored profileSeed is malformed; regenerating "
                "(this profile's fingerprint will change once)");
  }

  if (!RandomBytes(out_seed.data(), out_seed.size())) {
    LOG_ERROR_FP("🔒 FarblingPolicy: CSPRNG failed -- NOT farbling this profile. "
                 "A predictable seed would be worse than none.");
    return false;
  }

  doc["profileSeed"] = EncodeHex(out_seed);
  try {
    std::ofstream out(path);
    if (!out.is_open()) {
      // We have a usable in-memory seed, but it will differ next launch, which breaks
      // the cross-restart stability that is the whole point. Say so loudly.
      LOG_ERROR_FP("🔒 FarblingPolicy: generated a profile seed but could not persist "
                   "it to " + path + " -- fingerprint will NOT be stable across restarts");
      return true;
    }
    out << doc.dump(2);
  } catch (...) {
    LOG_ERROR_FP("🔒 FarblingPolicy: failed writing profile seed to " + path);
    return true;
  }

  LOG_INFO_FP("🔒 FarblingPolicy: generated a new persistent profile seed");
  return true;
}

void InitializeForProfile(const std::string& profile_dir) {
  std::lock_guard<std::mutex> lock(g_cached_mutex);
  if (g_cached_valid) {
    return;
  }
  std::array<uint8_t, 32> seed{};
  if (EnsureProfileSeed(profile_dir, seed)) {
    g_cached_seed = seed;
    g_cached_valid = true;
  } else {
    // Leave invalid: DomainKeyForUrl will refuse, and the browser will send
    // farbling_enabled = false rather than farble with a predictable key.
    LOG_ERROR_FP("🔒 FarblingPolicy: no profile seed -- farbling disabled for this run");
  }
}

bool DomainKeyForUrl(const std::string& url, std::array<uint8_t, 32>& out_key) {
  std::array<uint8_t, 32> seed{};
  {
    std::lock_guard<std::mutex> lock(g_cached_mutex);
    if (!g_cached_valid) {
      return false;
    }
    seed = g_cached_seed;
  }
  const std::string registrable = RegistrableDomainFromUrl(url);
  if (registrable.empty()) {
    return false;
  }
  return ComputeDomainKey(seed, registrable, out_key);
}

bool ComputeDomainKey(const std::array<uint8_t, 32>& profile_seed,
                      const std::string& registrable_domain,
                      std::array<uint8_t, 32>& out_key) {
  if (registrable_domain.empty()) {
    return false;
  }
  unsigned int out_len = 0;
  const unsigned char* result =
      HMAC(EVP_sha256(), profile_seed.data(), static_cast<int>(profile_seed.size()),
           reinterpret_cast<const unsigned char*>(registrable_domain.data()),
           registrable_domain.size(), out_key.data(), &out_len);
  if (result == nullptr || out_len != out_key.size()) {
    out_key.fill(0);
    return false;
  }
  return true;
}

}  // namespace FarblingPolicy
