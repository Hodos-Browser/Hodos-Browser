// update_stager_test.cpp — unit tests for the auto-updater verify core
// (WINDOWS_AUTOUPDATE_PLAN commit 4b). Pure-logic coverage: appcast parse,
// Ed25519 verify, integer anti-rollback, SHA-256, marker round-trip. Plus a
// Windows-only Authenticode test against an OS-signed system binary.

#include "core/UpdateStager.h"

#include <gtest/gtest.h>
#include <openssl/evp.h>

#include <cstdio>
#include <cstdlib>
#include <fstream>
#include <iterator>
#include <string>

using hodos::UpdateStager;
using hodos::AppcastEntry;
using hodos::StagedUpdateMarker;

namespace {

// Standard base64 encode (test-local; production has its own decoder).
std::string B64Encode(const unsigned char* data, size_t len) {
    static const char* tbl =
        "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    std::string out;
    int val = 0, bits = -6;
    for (size_t i = 0; i < len; ++i) {
        val = (val << 8) + data[i];
        bits += 8;
        while (bits >= 0) {
            out.push_back(tbl[(val >> bits) & 0x3F]);
            bits -= 6;
        }
    }
    if (bits > -6) out.push_back(tbl[((val << 8) >> (bits + 8)) & 0x3F]);
    while (out.size() % 4) out.push_back('=');
    return out;
}

// Generate a throwaway Ed25519 keypair, sign `msg`, return raw-pubkey + sig as
// base64 — exactly the shapes the appcast carries.
bool MakeEd25519Sig(const std::string& msg, std::string& pubB64, std::string& sigB64) {
    EVP_PKEY* pkey = nullptr;
    EVP_PKEY_CTX* pctx = EVP_PKEY_CTX_new_id(EVP_PKEY_ED25519, nullptr);
    if (!pctx) return false;
    bool ok = EVP_PKEY_keygen_init(pctx) == 1 && EVP_PKEY_keygen(pctx, &pkey) == 1;
    EVP_PKEY_CTX_free(pctx);
    if (!ok || !pkey) return false;

    unsigned char pub[32];
    size_t publen = sizeof(pub);
    ok = EVP_PKEY_get_raw_public_key(pkey, pub, &publen) == 1 && publen == 32;

    unsigned char sig[64];
    size_t siglen = sizeof(sig);
    if (ok) {
        EVP_MD_CTX* mctx = EVP_MD_CTX_new();
        ok = mctx && EVP_DigestSignInit(mctx, nullptr, nullptr, nullptr, pkey) == 1
             && EVP_DigestSign(mctx, sig, &siglen,
                               reinterpret_cast<const unsigned char*>(msg.data()),
                               msg.size()) == 1
             && siglen == 64;
        if (mctx) EVP_MD_CTX_free(mctx);
    }
    EVP_PKEY_free(pkey);
    if (!ok) return false;
    pubB64 = B64Encode(pub, publen);
    sigB64 = B64Encode(sig, siglen);
    return true;
}

const char* kDualItemAppcast = R"(<?xml version='1.0' encoding='utf-8'?>
<rss version="2.0">
  <channel>
    <title>Hodos Browser Updates</title>
    <item>
      <title>Version 0.4.0</title>
      <sparkle:version>40099</sparkle:version>
      <sparkle:os>macos</sparkle:os>
      <enclosure url="https://example.com/HodosBrowser-0.4.0.dmg" length="180000000" type="application/octet-stream" sparkle:edSignature="MACSIGdummy" />
    </item>
    <item>
      <title>Version 0.4.0</title>
      <sparkle:version>0.4.0</sparkle:version>
      <sparkle:os>windows</sparkle:os>
      <hodosBuildNumber>40099</hodosBuildNumber>
      <enclosure url="https://example.com/HodosBrowser-0.4.0-setup.exe" length="95000000" type="application/octet-stream" sparkle:dsaSignature="DSAdummy" sparkle:edSignature="EDSIGdummy" />
    </item>
  </channel>
</rss>)";

}  // namespace

// ---------------------------------------------------------------------------
// ParseWindowsAppcastItem
// ---------------------------------------------------------------------------
TEST(ParseAppcast, PicksWindowsItemEvenWhenMacItemAppearsFirst) {
    AppcastEntry e = UpdateStager::ParseWindowsAppcastItem(kDualItemAppcast);
    ASSERT_TRUE(e.valid);
    EXPECT_EQ(e.version, "0.4.0");
    EXPECT_EQ(e.buildNumber, 40099);
    EXPECT_EQ(e.enclosureUrl, "https://example.com/HodosBrowser-0.4.0-setup.exe");
    EXPECT_EQ(e.enclosureSize, 95000000LL);
    EXPECT_EQ(e.edSignature, "EDSIGdummy");
}

TEST(ParseAppcast, NoWindowsItemIsInvalid) {
    std::string xml = R"(<rss><channel>
      <item><sparkle:os>macos</sparkle:os>
      <enclosure url="x.dmg" sparkle:edSignature="s" /></item>
    </channel></rss>)";
    EXPECT_FALSE(UpdateStager::ParseWindowsAppcastItem(xml).valid);
}

TEST(ParseAppcast, MissingEdSignatureIsInvalid) {
    std::string xml = R"(<rss><channel><item>
      <sparkle:os>windows</sparkle:os>
      <hodosBuildNumber>40099</hodosBuildNumber>
      <enclosure url="https://x/setup.exe" length="10" />
    </item></channel></rss>)";
    EXPECT_FALSE(UpdateStager::ParseWindowsAppcastItem(xml).valid);
}

TEST(ParseAppcast, MissingBuildNumberIsInvalid) {
    std::string xml = R"(<rss><channel><item>
      <sparkle:os>windows</sparkle:os>
      <enclosure url="https://x/setup.exe" sparkle:edSignature="s" />
    </item></channel></rss>)";
    AppcastEntry e = UpdateStager::ParseWindowsAppcastItem(xml);
    EXPECT_FALSE(e.valid);
    EXPECT_EQ(e.buildNumber, 0);
}

TEST(ParseAppcast, EmptyAndGarbageAreInvalid) {
    EXPECT_FALSE(UpdateStager::ParseWindowsAppcastItem("").valid);
    EXPECT_FALSE(UpdateStager::ParseWindowsAppcastItem("not xml at all").valid);
    EXPECT_FALSE(UpdateStager::ParseWindowsAppcastItem("<item><sparkle:os>windows").valid);
}

// ---------------------------------------------------------------------------
// VerifyEd25519
// ---------------------------------------------------------------------------
TEST(VerifyEd25519, AcceptsAValidSignature) {
    std::string msg = "the installer bytes go here";
    std::string pub, sig;
    ASSERT_TRUE(MakeEd25519Sig(msg, pub, sig));
    EXPECT_TRUE(UpdateStager::VerifyEd25519(msg, sig, pub));
}

TEST(VerifyEd25519, RejectsTamperedData) {
    std::string msg = "original bytes";
    std::string pub, sig;
    ASSERT_TRUE(MakeEd25519Sig(msg, pub, sig));
    EXPECT_FALSE(UpdateStager::VerifyEd25519("tampered bytes", sig, pub));
}

TEST(VerifyEd25519, RejectsTamperedSignature) {
    std::string msg = "bytes";
    std::string pub, sig;
    ASSERT_TRUE(MakeEd25519Sig(msg, pub, sig));
    // Flip a character in the (base64) signature.
    sig[0] = (sig[0] == 'A') ? 'B' : 'A';
    EXPECT_FALSE(UpdateStager::VerifyEd25519(msg, sig, pub));
}

TEST(VerifyEd25519, RejectsWrongKey) {
    std::string msg = "bytes";
    std::string pub1, sig1, pub2, sig2;
    ASSERT_TRUE(MakeEd25519Sig(msg, pub1, sig1));
    ASSERT_TRUE(MakeEd25519Sig(msg, pub2, sig2));
    EXPECT_FALSE(UpdateStager::VerifyEd25519(msg, sig1, pub2));  // sig1 under key2
}

TEST(VerifyEd25519, RejectsMalformedInputs) {
    std::string msg = "bytes";
    std::string pub, sig;
    ASSERT_TRUE(MakeEd25519Sig(msg, pub, sig));
    EXPECT_FALSE(UpdateStager::VerifyEd25519(msg, sig, "not-base64!!"));   // bad key b64
    EXPECT_FALSE(UpdateStager::VerifyEd25519(msg, "@@@", pub));            // bad sig b64
    EXPECT_FALSE(UpdateStager::VerifyEd25519(msg, sig, "QQ=="));           // wrong key length
    EXPECT_FALSE(UpdateStager::VerifyEd25519(msg, "QQ==", pub));           // wrong sig length
}

// ---------------------------------------------------------------------------
// VerifyAppcastDocument (whole-doc signature, domain-separated) — commit 4c
// ---------------------------------------------------------------------------
TEST(VerifyAppcastDocument, PrefixIsStableAndMatchesSigner) {
    // Drift guard: must stay byte-identical to scripts/sign-appcast.py's
    // APPCAST_SIGNATURE_PREFIX. A mismatch silently fails every appcast verify.
    EXPECT_STREQ(UpdateStager::AppcastSignaturePrefix(), "hodos-appcast-v1\n");
}

TEST(VerifyAppcastDocument, AcceptsCorrectlySignedDocument) {
    std::string body = "<rss><channel><item>...</item></channel></rss>";
    std::string msg = std::string(UpdateStager::AppcastSignaturePrefix()) + body;
    std::string pub, sig;
    ASSERT_TRUE(MakeEd25519Sig(msg, pub, sig));
    EXPECT_TRUE(UpdateStager::VerifyAppcastDocument(body, sig, pub));
}

TEST(VerifyAppcastDocument, RejectsTamperedBody) {
    std::string body = "original appcast bytes";
    std::string msg = std::string(UpdateStager::AppcastSignaturePrefix()) + body;
    std::string pub, sig;
    ASSERT_TRUE(MakeEd25519Sig(msg, pub, sig));
    EXPECT_FALSE(UpdateStager::VerifyAppcastDocument("tampered appcast bytes", sig, pub));
}

TEST(VerifyAppcastDocument, RejectsSignatureMissingTheDomainPrefix) {
    // A signature over body-ONLY (no domain prefix) must NOT pass doc-verify —
    // proves the domain separation is actually enforced, not decorative.
    std::string body = "the appcast body";
    std::string pub, sig;
    ASSERT_TRUE(MakeEd25519Sig(body, pub, sig));  // signs body WITHOUT the prefix
    EXPECT_FALSE(UpdateStager::VerifyAppcastDocument(body, sig, pub));
}

// ---------------------------------------------------------------------------
// IsNewerBuild
// ---------------------------------------------------------------------------
TEST(AntiRollback, StrictlyNewerOnly) {
    EXPECT_TRUE(UpdateStager::IsNewerBuild(40099, 40016));
    EXPECT_FALSE(UpdateStager::IsNewerBuild(40016, 40016));  // equal
    EXPECT_FALSE(UpdateStager::IsNewerBuild(30018, 40016));  // older
}

TEST(AntiRollback, RejectsZeroCandidate) {
    EXPECT_FALSE(UpdateStager::IsNewerBuild(0, 0));
    EXPECT_FALSE(UpdateStager::IsNewerBuild(0, 40016));
}

TEST(AntiRollback, NoLexicalConfusion) {
    // 0.4.10 (40110) must beat 0.4.2 (40102) — integers, not string compare.
    EXPECT_TRUE(UpdateStager::IsNewerBuild(40110, 40102));
}

// ---------------------------------------------------------------------------
// Marker round-trip
// ---------------------------------------------------------------------------
TEST(Marker, RoundTripsAllFields) {
    StagedUpdateMarker m;
    m.buildNumber = 40099;
    m.version = "0.4.0";
    m.installerFileName = "HodosBrowser-0.4.0-setup.exe";
    m.sha256 = "deadbeef";
    m.edVerified = true;
    m.authenticodeVerified = true;
    m.signer = "Marston Enterprises";
    m.signerThumbprint = "aabbcc";
    m.stagedAt = "2026-06-29T00:00:00Z";

    StagedUpdateMarker out;
    ASSERT_TRUE(UpdateStager::ParseMarker(UpdateStager::SerializeMarker(m), out));
    EXPECT_EQ(out.buildNumber, m.buildNumber);
    EXPECT_EQ(out.version, m.version);
    EXPECT_EQ(out.installerFileName, m.installerFileName);
    EXPECT_EQ(out.sha256, m.sha256);
    EXPECT_TRUE(out.edVerified);
    EXPECT_TRUE(out.authenticodeVerified);
    EXPECT_EQ(out.signer, m.signer);
    EXPECT_EQ(out.signerThumbprint, m.signerThumbprint);
    EXPECT_EQ(out.stagedAt, m.stagedAt);
}

TEST(Marker, ParseRejectsGarbage) {
    StagedUpdateMarker out;
    EXPECT_FALSE(UpdateStager::ParseMarker("not json", out));
    EXPECT_FALSE(UpdateStager::ParseMarker("", out));
}

// ---------------------------------------------------------------------------
// Sha256File
// ---------------------------------------------------------------------------
TEST(Sha256File, MatchesKnownVector) {
    // SHA-256("abc") = ba7816bf...20015ad
    std::string path = std::string(testing::TempDir()) + "/hodos_sha_abc.bin";
    { std::ofstream f(path, std::ios::binary); f << "abc"; }
    EXPECT_EQ(UpdateStager::Sha256File(path),
              "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    std::remove(path.c_str());
}

TEST(Sha256File, MissingFileReturnsEmpty) {
    EXPECT_TRUE(UpdateStager::Sha256File("/no/such/hodos/file.bin").empty());
}

// ---------------------------------------------------------------------------
// Integration: full appcast-fetch → download → EdDSA verify → stage + marker,
// against a localhost feed. SKIPPED unless the rig sets the env below. Driven by
// scripts/test-update-feed.ps1, which stands up the Ed25519 keypair + signed
// installer + appcast + http.server and exports:
//   HODOS_UPDATE_TEST=1, HODOS_UPDATE_TEST_PUBKEY=<rig pubkey b64>,
//   HODOS_UPDATE_RIG_URL=<appcast url>, HODOS_UPDATE_RIG_PENDING=<temp dir>
// ---------------------------------------------------------------------------
TEST(UpdateStagerRig, StagesFromLocalFeed) {
    const char* url = std::getenv("HODOS_UPDATE_RIG_URL");
    const char* pending = std::getenv("HODOS_UPDATE_RIG_PENDING");
    if (!url || !pending) GTEST_SKIP() << "rig env not set (run scripts/test-update-feed.ps1)";

    // current build = 0 so any feed build > 0 is "newer".
    auto result = UpdateStager::StagePendingUpdate(url, pending, /*currentBuildNumber=*/0);
    EXPECT_EQ(result, hodos::StageResult::Staged);

    std::string markerPath = std::string(pending) + "/update-info.json";
    std::ifstream mf(markerPath, std::ios::binary);
    ASSERT_TRUE(mf.is_open()) << "marker not written";
    std::string j((std::istreambuf_iterator<char>(mf)), {});
    StagedUpdateMarker m;
    ASSERT_TRUE(UpdateStager::ParseMarker(j, m));
    EXPECT_TRUE(m.edVerified);
    EXPECT_GT(m.buildNumber, 0);
    EXPECT_FALSE(m.sha256.empty());
}

#ifdef _WIN32
// ---------------------------------------------------------------------------
// Authenticode (Windows) — validate against an OS-signed system binary so the
// trust + signer-extraction logic is exercised deterministically. kernel32.dll
// is Authenticode-signed by Microsoft on every supported Windows.
// ---------------------------------------------------------------------------
TEST(Authenticode, TrustsAnOsSignedMicrosoftBinary) {
    auto r = UpdateStager::VerifyAuthenticode("C:\\Windows\\System32\\kernel32.dll",
                                              "Microsoft");
    EXPECT_TRUE(r.trusted);
    EXPECT_FALSE(r.signer.empty());
    EXPECT_EQ(r.thumbprint.size(), 40u);  // SHA-1 = 20 bytes = 40 hex chars
}

TEST(Authenticode, RejectsWhenExpectedSignerDoesNotMatch) {
    // Real Microsoft binary, but we demand a different signer → not trusted.
    auto r = UpdateStager::VerifyAuthenticode("C:\\Windows\\System32\\kernel32.dll",
                                              "Marston Enterprises");
    EXPECT_FALSE(r.trusted);
}

TEST(Authenticode, UnsignedFileIsNotTrusted) {
    std::string path = std::string(testing::TempDir()) + "/hodos_unsigned.exe";
    { std::ofstream f(path, std::ios::binary); f << "MZ not really a signed pe"; }
    auto r = UpdateStager::VerifyAuthenticode(path, "Microsoft");
    EXPECT_FALSE(r.trusted);
    std::remove(path.c_str());
}
#endif  // _WIN32
