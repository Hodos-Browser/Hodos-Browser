// UpdateStager.cpp — Windows auto-update: Hodos-owned download → stage → verify.
// See UpdateStager.h for the design contract. WINDOWS_AUTOUPDATE_PLAN commit 4.

#include "../../include/core/UpdateStager.h"
#include "../../include/core/SyncHttpClient.h"
#include "../../include/core/UpdateApply.h"   // 6c.3: FileManifest / ParseManifest (signed buildNumber)
#include "../../include/core/Logger.h"

#include <openssl/evp.h>

#include <nlohmann/json.hpp>

#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <ctime>
#include <fstream>
#include <filesystem>
#include <vector>

#ifdef _WIN32
#ifndef WIN32_LEAN_AND_MEAN
#define WIN32_LEAN_AND_MEAN
#endif
#include <windows.h>
#include <softpub.h>
#include <wincrypt.h>
#include <wintrust.h>
#pragma comment(lib, "wintrust.lib")
#pragma comment(lib, "crypt32.lib")
#endif

#define LOG_INFO_UPD(msg) Logger::Log(std::string("[UpdateStager] ") + (msg), 1, 0)
#define LOG_WARN_UPD(msg) Logger::Log(std::string("[UpdateStager] ") + (msg), 2, 0)
#define LOG_ERR_UPD(msg)  Logger::Log(std::string("[UpdateStager] ") + (msg), 3, 0)

namespace hodos {

namespace {

// ---- base64 decode (standard alphabet, padding-tolerant, whitespace-ignoring).
// Returns false on any invalid character / malformed input.
bool Base64Decode(const std::string& in, std::vector<unsigned char>& out) {
    auto val = [](unsigned char c) -> int {
        if (c >= 'A' && c <= 'Z') return c - 'A';
        if (c >= 'a' && c <= 'z') return c - 'a' + 26;
        if (c >= '0' && c <= '9') return c - '0' + 52;
        if (c == '+') return 62;
        if (c == '/') return 63;
        return -1;
    };
    out.clear();
    int accum = 0, nbits = 0;
    for (unsigned char c : in) {
        if (c == '\n' || c == '\r' || c == ' ' || c == '\t') continue;
        if (c == '=') break;  // padding — stop
        int v = val(c);
        if (v < 0) return false;
        accum = (accum << 6) | v;
        nbits += 6;
        if (nbits >= 8) {
            nbits -= 8;
            out.push_back(static_cast<unsigned char>((accum >> nbits) & 0xFF));
        }
    }
    return true;
}

// ---- tiny XML field extractors for the (controlled, Hodos-generated) appcast.
// Not a general parser — just enough to pull our known fields robustly.

// Text between the first occurrence of <tag> ... </tag> within `s` (searching
// from `from`). Empty string if not found. `tag` is the full element name incl.
// any namespace prefix, e.g. "sparkle:version".
std::string TagText(const std::string& s, const std::string& tag, size_t from = 0) {
    const std::string open = "<" + tag + ">";
    const std::string close = "</" + tag + ">";
    size_t a = s.find(open, from);
    if (a == std::string::npos) return "";
    a += open.size();
    size_t b = s.find(close, a);
    if (b == std::string::npos) return "";
    return s.substr(a, b - a);
}

// Value of attr `name="..."` inside `s`. Empty string if not found.
std::string AttrValue(const std::string& s, const std::string& name) {
    const std::string key = name + "=\"";
    size_t a = s.find(key);
    if (a == std::string::npos) return "";
    a += key.size();
    size_t b = s.find('"', a);
    if (b == std::string::npos) return "";
    return s.substr(a, b - a);
}

std::string IsoUtcNow() {
    std::time_t t = std::time(nullptr);
    std::tm tmv{};
#ifdef _WIN32
    gmtime_s(&tmv, &t);
#else
    gmtime_r(&t, &tmv);
#endif
    char buf[32];
    std::strftime(buf, sizeof(buf), "%Y-%m-%dT%H:%M:%SZ", &tmv);
    return std::string(buf);
}

// Same-directory sibling URL: replace the last path segment of `url` with `name`
// (6c.3: the expected-new-manifest is published next to the installer enclosure).
std::string SiblingUrl(const std::string& url, const std::string& name) {
    size_t slash = url.find_last_of('/');
    return (slash == std::string::npos) ? name : url.substr(0, slash + 1) + name;
}

// MUST stay byte-identical to UpdateFs::ManifestSignaturePrefix() (the apply-time
// verifier). Duplicated here so the stage-time check doesn't pull UpdateFs.
const char* kManifestSigPrefix = "hodos-manifest-v1\n";

// SHA-256 (hex) of an in-memory buffer — so the marker hash describes the SAME
// bytes EdDSA verified (no second read of the file → no local write-race).
std::string Sha256Buffer(const std::string& data) {
    unsigned char digest[EVP_MAX_MD_SIZE];
    unsigned int dlen = 0;
    if (EVP_Digest(data.data(), data.size(), digest, &dlen, EVP_sha256(), nullptr) != 1) {
        return "";
    }
    static const char* h = "0123456789abcdef";
    std::string hex;
    hex.reserve(dlen * 2);
    for (unsigned int i = 0; i < dlen; ++i) {
        hex.push_back(h[digest[i] >> 4]);
        hex.push_back(h[digest[i] & 0xF]);
    }
    return hex;
}

// A staged installer filename must be a bare filename (no path separators, no
// "..", non-empty) — the idempotency branch reads it from an on-disk marker
// that a local attacker could have written, and commit 6 will feed it to the
// installer. Reject anything that could escape the pending dir.
bool IsSafeFileName(const std::string& name) {
    if (name.empty()) return false;
    if (name.find('/') != std::string::npos) return false;
    if (name.find('\\') != std::string::npos) return false;
    if (name.find("..") != std::string::npos) return false;
    if (name.find(':') != std::string::npos) return false;  // drive / ADS
    return true;
}

}  // namespace

const char* UpdateStager::PublicKeyBase64() {
    // Same Ed25519 key macOS Sparkle uses (Info.plist SUPublicEDKey). One key,
    // both platforms.
    return "GVq3mpDl8eelsG0A5wqC5FBYZd3fy7U3we9iZ9+Tq3Q=";
}

const char* UpdateStager::ExpectedSigner() {
    return "Marston Enterprises";  // Azure Trusted Signing identity
}

// ---------------------------------------------------------------------------
// Pure: appcast parse
// ---------------------------------------------------------------------------
AppcastEntry UpdateStager::ParseWindowsAppcastItem(const std::string& xml) {
    AppcastEntry e;
    // Walk each <item> ... </item> block, pick the one whose <sparkle:os> is
    // "windows" (the generator emits one Windows item + one macOS item).
    size_t pos = 0;
    while (true) {
        size_t itemStart = xml.find("<item>", pos);
        if (itemStart == std::string::npos) break;
        size_t itemEnd = xml.find("</item>", itemStart);
        if (itemEnd == std::string::npos) break;
        std::string item = xml.substr(itemStart, itemEnd - itemStart);
        pos = itemEnd + 7;

        std::string os = TagText(item, "sparkle:os");
        if (os != "windows") continue;

        // Enclosure attributes.
        size_t encPos = item.find("<enclosure");
        if (encPos == std::string::npos) continue;
        size_t encEnd = item.find('>', encPos);
        std::string enc = item.substr(encPos, encEnd == std::string::npos
                                                   ? std::string::npos
                                                   : encEnd - encPos);

        e.version = TagText(item, "sparkle:version");
        std::string bn = TagText(item, "hodosBuildNumber");
        e.buildNumber = bn.empty() ? 0 : std::strtol(bn.c_str(), nullptr, 10);
        e.enclosureUrl = AttrValue(enc, "url");
        std::string len = AttrValue(enc, "length");
        e.enclosureSize = len.empty() ? 0 : std::strtoll(len.c_str(), nullptr, 10);
        e.edSignature = AttrValue(enc, "sparkle:edSignature");

        // Minimum viable item: a URL + an EdDSA signature + a usable build number.
        e.valid = !e.enclosureUrl.empty() && !e.edSignature.empty() && e.buildNumber > 0;
        return e;  // first windows item wins
    }
    return e;  // {valid=false}
}

// ---------------------------------------------------------------------------
// Pure: Ed25519 verify (OpenSSL)
// ---------------------------------------------------------------------------
bool UpdateStager::VerifyEd25519(const std::string& data,
                                 const std::string& signatureBase64,
                                 const std::string& publicKeyBase64) {
    std::vector<unsigned char> pub, sig;
    if (!Base64Decode(publicKeyBase64, pub) || pub.size() != 32) return false;
    if (!Base64Decode(signatureBase64, sig) || sig.size() != 64) return false;

    EVP_PKEY* pkey = EVP_PKEY_new_raw_public_key(EVP_PKEY_ED25519, nullptr,
                                                 pub.data(), pub.size());
    if (!pkey) return false;

    bool ok = false;
    EVP_MD_CTX* ctx = EVP_MD_CTX_new();
    if (ctx) {
        // Ed25519: md MUST be null; verification is one-shot (no streaming).
        if (EVP_DigestVerifyInit(ctx, nullptr, nullptr, nullptr, pkey) == 1) {
            int rc = EVP_DigestVerify(
                ctx, sig.data(), sig.size(),
                reinterpret_cast<const unsigned char*>(data.data()), data.size());
            ok = (rc == 1);
        }
        EVP_MD_CTX_free(ctx);
    }
    EVP_PKEY_free(pkey);
    return ok;
}

// ---------------------------------------------------------------------------
// Pure: whole-appcast-document signature (anti-replay / anti-tamper)
// ---------------------------------------------------------------------------
const char* UpdateStager::AppcastSignaturePrefix() {
    return "hodos-appcast-v1\n";
}

bool UpdateStager::VerifyAppcastDocument(const std::string& body,
                                         const std::string& signatureBase64,
                                         const std::string& publicKeyBase64) {
    // Domain-separated: sign/verify over prefix||body so an appcast-doc signature
    // can never be confused with an installer signature (and vice-versa).
    return VerifyEd25519(std::string(AppcastSignaturePrefix()) + body,
                         signatureBase64, publicKeyBase64);
}

// ---------------------------------------------------------------------------
// Pure: integer anti-rollback
// ---------------------------------------------------------------------------
bool UpdateStager::IsNewerBuild(long candidateBuildNumber, long currentBuildNumber) {
    // Strictly newer, and never accept an absent/zero candidate build number.
    return candidateBuildNumber > 0 && candidateBuildNumber > currentBuildNumber;
}

// ---------------------------------------------------------------------------
// SHA-256 of a file (OpenSSL EVP, streamed)
// ---------------------------------------------------------------------------
std::string UpdateStager::Sha256File(const std::string& path) {
    std::ifstream in(path, std::ios::binary);
    if (!in.is_open()) return "";

    EVP_MD_CTX* ctx = EVP_MD_CTX_new();
    if (!ctx) return "";
    std::string hex;
    if (EVP_DigestInit_ex(ctx, EVP_sha256(), nullptr) == 1) {
        char buf[65536];
        bool ok = true;
        while (in) {
            in.read(buf, sizeof(buf));
            std::streamsize n = in.gcount();
            if (n > 0 && EVP_DigestUpdate(ctx, buf, static_cast<size_t>(n)) != 1) {
                ok = false;
                break;
            }
        }
        if (ok && !in.bad()) {
            unsigned char digest[EVP_MAX_MD_SIZE];
            unsigned int dlen = 0;
            if (EVP_DigestFinal_ex(ctx, digest, &dlen) == 1) {
                static const char* h = "0123456789abcdef";
                hex.reserve(dlen * 2);
                for (unsigned int i = 0; i < dlen; ++i) {
                    hex.push_back(h[digest[i] >> 4]);
                    hex.push_back(h[digest[i] & 0xF]);
                }
            }
        }
    }
    EVP_MD_CTX_free(ctx);
    return hex;
}

// ---------------------------------------------------------------------------
// Marker (de)serialize
// ---------------------------------------------------------------------------
std::string UpdateStager::SerializeMarker(const StagedUpdateMarker& m) {
    nlohmann::json j;
    j["buildNumber"] = m.buildNumber;
    j["version"] = m.version;
    j["installerFileName"] = m.installerFileName;
    j["sha256"] = m.sha256;
    j["edVerified"] = m.edVerified;
    j["authenticodeVerified"] = m.authenticodeVerified;
    j["signer"] = m.signer;
    j["signerThumbprint"] = m.signerThumbprint;
    j["stagedAt"] = m.stagedAt;
    return j.dump(2);
}

bool UpdateStager::ParseMarker(const std::string& json, StagedUpdateMarker& out) {
    try {
        auto j = nlohmann::json::parse(json);
        out.buildNumber = j.value("buildNumber", 0L);
        out.version = j.value("version", std::string());
        out.installerFileName = j.value("installerFileName", std::string());
        out.sha256 = j.value("sha256", std::string());
        out.edVerified = j.value("edVerified", false);
        out.authenticodeVerified = j.value("authenticodeVerified", false);
        out.signer = j.value("signer", std::string());
        out.signerThumbprint = j.value("signerThumbprint", std::string());
        out.stagedAt = j.value("stagedAt", std::string());
        return true;
    } catch (...) {
        return false;
    }
}

#ifdef _WIN32
// ---------------------------------------------------------------------------
// Authenticode (Windows): WinVerifyTrust + signer CN / thumbprint extraction
// ---------------------------------------------------------------------------
UpdateStager::AuthenticodeResult UpdateStager::VerifyAuthenticode(
    const std::string& path, const std::string& expectedSigner) {
    AuthenticodeResult r;

    // Proper wide conversion (the naive char-by-char widen mangles any byte >127).
    std::wstring wpath = std::filesystem::path(path).wstring();

    // 1) WinVerifyTrust — chain + policy.
    WINTRUST_FILE_INFO fileInfo = {};
    fileInfo.cbStruct = sizeof(fileInfo);
    fileInfo.pcwszFilePath = wpath.c_str();

    GUID policy = WINTRUST_ACTION_GENERIC_VERIFY_V2;
    WINTRUST_DATA wd = {};
    wd.cbStruct = sizeof(wd);
    wd.dwUIChoice = WTD_UI_NONE;
    // Deliberate: no online revocation. EdDSA is the PRIMARY integrity gate;
    // Authenticode is the secondary OS-trust gate. An online CRL/OCSP fetch could
    // hang or fail offline and fail-close the update (violates update-stability).
    // Cert-compromise revocation is covered by the signer-continuity / thumbprint
    // auto-degrade gate (WINDOWS_AUTOUPDATE_PLAN §H.6), not here.
    wd.fdwRevocationChecks = WTD_REVOKE_NONE;
    wd.dwUnionChoice = WTD_CHOICE_FILE;
    wd.pFile = &fileInfo;
    wd.dwStateAction = WTD_STATEACTION_VERIFY;
    wd.dwProvFlags = WTD_SAFER_FLAG;

    LONG status = WinVerifyTrust(static_cast<HWND>(INVALID_HANDLE_VALUE), &policy, &wd);
    r.trusted = (status == ERROR_SUCCESS);

    // Always release the trust state we allocated.
    wd.dwStateAction = WTD_STATEACTION_CLOSE;
    WinVerifyTrust(static_cast<HWND>(INVALID_HANDLE_VALUE), &policy, &wd);

    // 2) Pull the signer certificate (CN + SHA-1 thumbprint), independent of
    //    trust so a near-miss can still be logged / recorded in the marker.
    HCERTSTORE hStore = nullptr;
    HCRYPTMSG hMsg = nullptr;
    if (CryptQueryObject(CERT_QUERY_OBJECT_FILE, wpath.c_str(),
                         CERT_QUERY_CONTENT_FLAG_PKCS7_SIGNED_EMBED,
                         CERT_QUERY_FORMAT_FLAG_BINARY, 0, nullptr, nullptr,
                         nullptr, &hStore, &hMsg, nullptr)) {
        DWORD signerInfoSize = 0;
        if (CryptMsgGetParam(hMsg, CMSG_SIGNER_INFO_PARAM, 0, nullptr, &signerInfoSize)
            && signerInfoSize > 0) {
            std::vector<BYTE> signerInfoBuf(signerInfoSize);
            if (CryptMsgGetParam(hMsg, CMSG_SIGNER_INFO_PARAM, 0, signerInfoBuf.data(),
                                 &signerInfoSize)) {
                auto* signerInfo = reinterpret_cast<CMSG_SIGNER_INFO*>(signerInfoBuf.data());
                CERT_INFO certInfo = {};
                certInfo.Issuer = signerInfo->Issuer;
                certInfo.SerialNumber = signerInfo->SerialNumber;
                PCCERT_CONTEXT cert = CertFindCertificateInStore(
                    hStore, X509_ASN_ENCODING | PKCS_7_ASN_ENCODING, 0,
                    CERT_FIND_SUBJECT_CERT, &certInfo, nullptr);
                if (cert) {
                    // Common name.
                    char cn[256] = {0};
                    if (CertGetNameStringA(cert, CERT_NAME_SIMPLE_DISPLAY_TYPE, 0,
                                           nullptr, cn, sizeof(cn)) > 1) {
                        r.signer = cn;
                    }
                    // SHA-1 thumbprint (hex).
                    BYTE thumb[20];
                    DWORD thumbLen = sizeof(thumb);
                    if (CertGetCertificateContextProperty(cert, CERT_SHA1_HASH_PROP_ID,
                                                          thumb, &thumbLen)) {
                        static const char* h = "0123456789abcdef";
                        for (DWORD i = 0; i < thumbLen; ++i) {
                            r.thumbprint.push_back(h[thumb[i] >> 4]);
                            r.thumbprint.push_back(h[thumb[i] & 0xF]);
                        }
                    }
                    CertFreeCertificateContext(cert);
                }
            }
        }
    }
    if (hMsg) CryptMsgClose(hMsg);
    if (hStore) CertCloseStore(hStore, 0);

    // Trust requires BOTH the chain (WinVerifyTrust) AND a signer CN that
    // contains the expected identity. (CVE-2013-3900 trailing-data hardening is
    // the system EnableCertPaddingCheck mitigation — see commit notes.)
    bool signerMatches = !expectedSigner.empty()
                         && r.signer.find(expectedSigner) != std::string::npos;
    r.trusted = r.trusted && signerMatches;
    return r;
}
#endif  // _WIN32

// ---------------------------------------------------------------------------
// Orchestrator (network + filesystem). Wired into startup in commit 4d.
// ---------------------------------------------------------------------------
StageResult UpdateStager::StagePendingUpdate(const std::string& appcastUrl,
                                             const std::string& pendingDir,
                                             long currentBuildNumber,
                                             const std::atomic<bool>* abort) {
    namespace fs = std::filesystem;
    auto aborted = [&]() { return abort && abort->load(); };

    // Inert under HODOS_DEV unless the test seam is set.
    const char* dev = std::getenv("HODOS_DEV");
    const char* test = std::getenv("HODOS_UPDATE_TEST");
    const bool isDev = dev && std::string(dev) == "1";
    const bool isTest = test && std::string(test) == "1";
    if (isDev && !isTest) {
        LOG_INFO_UPD("HODOS_DEV without HODOS_UPDATE_TEST — staging inert.");
        return StageResult::Skipped;
    }

    // The Ed25519 public key used for BOTH the appcast-document signature and the
    // installer signature. Always the embedded production key; the test seam
    // (compiled OUT of the shipped browser) lets the localhost rig substitute its
    // throwaway key — there is no env-triggerable override in production.
    std::string pubKey = PublicKeyBase64();
#ifdef HODOS_UPDATE_TEST_SEAM
    if (isTest) {
        const char* testPub = std::getenv("HODOS_UPDATE_TEST_PUBKEY");
        if (testPub) pubKey = testPub;
    }
#endif

    // 1) Fetch the appcast + its detached signature sidecar.
    HttpResponse feed = SyncHttpClient::Get(appcastUrl, 10000);
    if (!feed.success || feed.body.empty()) {
        LOG_WARN_UPD("appcast fetch failed (status " + std::to_string(feed.statusCode) + ")");
        return StageResult::NetworkFailed;
    }
    HttpResponse sig = SyncHttpClient::Get(appcastUrl + ".ed", 10000);
    if (!sig.success || sig.body.empty()) {
        LOG_WARN_UPD("appcast signature sidecar fetch failed (status "
                     + std::to_string(sig.statusCode) + ") — refusing unsigned feed");
        return StageResult::NetworkFailed;
    }

    // 2) VERIFY THE WHOLE DOCUMENT BEFORE PARSING (anti-tamper / anti-replay).
    //    A tampered or forged feed never reaches the XML extractors below.
    if (!VerifyAppcastDocument(feed.body, sig.body, pubKey)) {
        LOG_ERR_UPD("appcast document signature INVALID — rejecting feed (fail-closed)");
        return StageResult::VerifyFailed;
    }

    // 3) Parse the (now-authenticated) Windows item.
    AppcastEntry entry = ParseWindowsAppcastItem(feed.body);
    if (!entry.valid) {
        LOG_INFO_UPD("no usable Windows item in appcast");
        return StageResult::NoUpdate;
    }

    // 4) Integer anti-rollback (current side baked via -DAPP_BUILD_NUMBER).
    if (!IsNewerBuild(entry.buildNumber, currentBuildNumber)) {
        LOG_INFO_UPD("up to date (feed build " + std::to_string(entry.buildNumber)
                     + " <= current " + std::to_string(currentBuildNumber) + ")");
        return StageResult::UpToDate;
    }

    std::error_code ec;
    fs::create_directories(pendingDir, ec);

    // 5) Idempotency: if a verified marker for the same-or-newer build is already
    //    staged with its installer present, don't re-download.
    const fs::path markerPath = fs::path(pendingDir) / "update-info.json";
    if (fs::exists(markerPath)) {
        std::ifstream mf(markerPath, std::ios::binary);
        std::string mjson((std::istreambuf_iterator<char>(mf)), {});
        StagedUpdateMarker existing;
        if (ParseMarker(mjson, existing) && existing.edVerified
            && existing.buildNumber >= entry.buildNumber
            && IsSafeFileName(existing.installerFileName)
            && fs::exists(fs::path(pendingDir) / existing.installerFileName)) {
            LOG_INFO_UPD("build " + std::to_string(existing.buildNumber)
                         + " already staged — skipping re-download");
            return StageResult::Staged;
        }
    }

    // Stage exactly one installer at a time: clear any prior stage first.
    for (const auto& p : fs::directory_iterator(pendingDir, ec)) {
        if (p.path().filename() != "rollback") fs::remove_all(p.path(), ec);
    }

    if (aborted()) { LOG_INFO_UPD("aborted before download (shutdown)"); return StageResult::Skipped; }

    // 6) Download the installer.
    const std::string installerName =
        "HodosBrowser-" + (entry.version.empty() ? std::to_string(entry.buildNumber)
                                                  : entry.version) + "-setup.exe";
    const fs::path installerPath = fs::path(pendingDir) / installerName;
    HttpResponse dl = SyncHttpClient::Download(entry.enclosureUrl, installerPath.string(), 600000);
    if (!dl.success || !fs::exists(installerPath)) {
        LOG_WARN_UPD("installer download failed (status " + std::to_string(dl.statusCode) + ")");
        return StageResult::NetworkFailed;
    }
    if (aborted()) {
        LOG_INFO_UPD("aborted after download (shutdown) — discarding stage");
        std::error_code rec; fs::remove(installerPath, rec);
        return StageResult::Skipped;
    }

    // 7) Verify gates (fail-closed). Read the file ONCE; hash + EdDSA over the
    //    same in-memory bytes (no second read → no verify/hash TOCTOU). Pass the
    //    fs::path (not .string()) so non-ASCII pending dirs open losslessly.
    std::string bytes;
    {
        std::ifstream in(installerPath, std::ios::binary);
        bytes.assign((std::istreambuf_iterator<char>(in)), {});
    }
    std::string sha = Sha256Buffer(bytes);

    // EdDSA over the installer bytes — HARD gate, same key as the doc signature
    // (pubKey computed once at the top; production = embedded key).
    bool edOk = VerifyEd25519(bytes, entry.edSignature, pubKey);
    std::string().swap(bytes);  // free the ~95MB buffer promptly (clear() keeps capacity)
    if (!edOk) {
        LOG_ERR_UPD("EdDSA verification FAILED — rejecting installer");
        fs::remove(installerPath, ec);
        return StageResult::VerifyFailed;
    }

    StagedUpdateMarker marker;
    marker.buildNumber = entry.buildNumber;
    marker.version = entry.version;
    marker.installerFileName = installerName;
    marker.sha256 = sha;
    marker.edVerified = true;
    marker.stagedAt = IsoUtcNow();

#ifdef _WIN32
    std::string expectedSigner = ExpectedSigner();
#ifdef HODOS_UPDATE_TEST_SEAM
    if (isTest) {
        const char* testSigner = std::getenv("HODOS_UPDATE_TEST_SIGNER");
        if (testSigner) expectedSigner = testSigner;
    }
#endif
    AuthenticodeResult ac = VerifyAuthenticode(installerPath.string(), expectedSigner);
    marker.authenticodeVerified = ac.trusted;
    marker.signer = ac.signer;
    marker.signerThumbprint = ac.thumbprint;

    bool authenticodeOk = ac.trusted;
#ifdef HODOS_UPDATE_TEST_SEAM
    // TEST-BUILD ONLY (compiled OUT of production): a self-signed rig installer
    // won't chain to a trusted root, so stage on the EdDSA gate alone. In
    // production Authenticode is mandatory and this relaxation does not exist.
    if (isTest && !authenticodeOk) {
        LOG_WARN_UPD("Authenticode not trusted (signer='" + ac.signer
                     + "') — allowed in TEST build; EdDSA gate passed");
        authenticodeOk = true;
    }
#endif
    if (!authenticodeOk) {
        LOG_ERR_UPD("Authenticode verification FAILED (signer='" + ac.signer
                    + "') — rejecting installer");
        fs::remove(installerPath, ec);
        return StageResult::VerifyFailed;
    }
#endif

    // 7c) Download + verify the SIGNED expected-new manifest (6c.3) into pending\.
    //     The silent apply bootstrap REQUIRES it (the apply-time IntegrityGate verifies
    //     the new {app} tree against it AND reads its bound buildNumber for anti-rollback,
    //     review #2). Verify the sig here too (fail-closed early) + bind its buildNumber
    //     to this feed's build. Sibling-of-installer URL (no appcast change).
    {
        const std::string manifestUrl = SiblingUrl(entry.enclosureUrl, "expected-new-manifest.json");
        const fs::path manPath = fs::path(pendingDir) / "expected-new-manifest.json";
        HttpResponse mdl = SyncHttpClient::Download(manifestUrl, manPath.string(), 60000);
        HttpResponse msig = SyncHttpClient::Get(manifestUrl + ".ed", 10000);
        if (!mdl.success || !fs::exists(manPath) || !msig.success || msig.body.empty()) {
            LOG_WARN_UPD("expected-new-manifest download failed — refusing stage (silent path needs it)");
            fs::remove(installerPath, ec); fs::remove(manPath, ec);
            return StageResult::NetworkFailed;
        }
        std::string mbytes;
        { std::ifstream in(manPath, std::ios::binary); mbytes.assign((std::istreambuf_iterator<char>(in)), {}); }
        if (!VerifyEd25519(std::string(kManifestSigPrefix) + mbytes, msig.body, pubKey)) {
            LOG_ERR_UPD("expected-new-manifest signature INVALID — rejecting stage (fail-closed)");
            fs::remove(installerPath, ec); fs::remove(manPath, ec);
            return StageResult::VerifyFailed;
        }
        FileManifest sm;
        if (!ParseManifest(mbytes, sm) || sm.buildNumber != entry.buildNumber) {
            LOG_ERR_UPD("expected-new-manifest buildNumber (" + std::to_string(sm.buildNumber)
                        + ") != feed build (" + std::to_string(entry.buildNumber) + ") — rejecting stage");
            fs::remove(installerPath, ec); fs::remove(manPath, ec);
            return StageResult::VerifyFailed;
        }
        // Persist the verified sidecar next to the manifest (the apply-time gate reads it).
        std::ofstream out((manPath.string() + ".ed"), std::ios::binary | std::ios::trunc);
        out << msig.body;
        if (!out) {
            LOG_ERR_UPD("failed to write manifest sidecar — unstaging");
            fs::remove(installerPath, ec); fs::remove(manPath, ec);
            return StageResult::VerifyFailed;
        }
    }

    // 8) Write the arm marker atomically (temp + rename) so a crash mid-write
    //    can't leave a torn marker that commit 6 might half-read.
    const fs::path markerTmp = markerPath.string() + ".tmp";
    {
        std::ofstream out(markerTmp, std::ios::binary | std::ios::trunc);
        out << SerializeMarker(marker);
        out.close();
        if (!out) {
            LOG_ERR_UPD("failed to write marker — unstaging");
            fs::remove(markerTmp, ec);
            fs::remove(installerPath, ec);
            return StageResult::VerifyFailed;
        }
    }
    fs::rename(markerTmp, markerPath, ec);
    if (ec) {
        LOG_ERR_UPD("failed to commit marker — unstaging");
        fs::remove(markerTmp, ec);
        fs::remove(installerPath, ec);
        return StageResult::VerifyFailed;
    }

    LOG_INFO_UPD("staged build " + std::to_string(entry.buildNumber)
                 + " (" + entry.version + "), sha256=" + sha.substr(0, 12) + "...");
    return StageResult::Staged;
}

// ---- Kill-list (commit 6e.2 / §H.7 + H4) ------------------------------------
UpdateStager::KillList UpdateStager::ParseKillList(const std::string& jsonStr) {
    KillList out;
    auto j = nlohmann::json::parse(jsonStr, nullptr, /*allow_exceptions=*/false);
    if (j.is_discarded() || !j.is_object()) return out;  // valid stays false
    if (auto it = j.find("generation"); it != j.end() && it->is_number_integer())
        out.generation = it->get<long>();
    if (auto it = j.find("retractedBuilds"); it != j.end() && it->is_array()) {
        for (const auto& e : *it) if (e.is_number_integer()) out.retractedBuilds.push_back(e.get<long>());
    }
    out.valid = true;
    return out;
}

const char* UpdateStager::KillListSignaturePrefix() { return "hodos-killlist-v1\n"; }

bool UpdateStager::IsBuildRetracted(long buildNumber, const std::string& killListUrl) {
    HttpResponse kl = SyncHttpClient::Get(killListUrl, 5000);
    HttpResponse ks = SyncHttpClient::Get(killListUrl + ".ed", 5000);
    if (!kl.success || kl.body.empty() || !ks.success || ks.body.empty()) {
        LOG_INFO_UPD("kill-list unavailable (network) — fail-open (proceeding)");
        return false;
    }
    if (!VerifyEd25519(std::string(KillListSignaturePrefix()) + kl.body, ks.body, PublicKeyBase64())) {
        LOG_WARN_UPD("kill-list signature invalid — ignoring (fail-open)");
        return false;
    }
    KillList list = ParseKillList(kl.body);
    if (!list.valid) { LOG_WARN_UPD("kill-list unparseable — fail-open"); return false; }
    for (long b : list.retractedBuilds) {
        if (b == buildNumber) {
            LOG_ERR_UPD("build " + std::to_string(buildNumber) + " is RETRACTED by kill-list gen "
                        + std::to_string(list.generation) + " — refusing apply");
            return true;
        }
    }
    return false;
}

}  // namespace hodos
