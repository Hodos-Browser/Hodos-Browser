// manifest_fetcher_test.cpp — parse tests for ManifestFetcher.
//
// We test ParseFromJson directly because it's pure: same input, same output,
// no network. It is also the PRODUCTION path — the interceptor re-parses the
// bytes Rust embeds in its 202 with it, so the "N protocols" count in the
// connect log line comes from here (`P0.8-A1`).
//
// ManifestUrls() is tested too (`P0.8-A6` / `R-PATH`); the rest of Fetch() is a
// size cap plus delegation, and Rust owns the production fetch since 2.6-G.2.
//
// ⚠️ The fixture-driven suite at the bottom reads the SAME files
// `rust-wallet/src/manifest.rs` compiles in with include_str!. One canonical
// copy, both parsers — a fix in one layer with a test driven only by the other
// proves nothing.

#include "core/ManifestFetcher.h"

#include <gtest/gtest.h>
#include <nlohmann/json.hpp>

#include <fstream>
#include <sstream>
#include <string>

using hodos::Manifest;
using hodos::ManifestFetcher;

// ============================================================================
// Happy-path: full valid manifest
// ============================================================================

TEST(ManifestFetcher, ValidFullManifestParsesAllFields) {
    const std::string json = R"({
        "version": "1.0",
        "name": "1Sat Market",
        "description": "BSV NFT marketplace and trading platform",
        "iconUrl": "https://1sat.market/icon.png",
        "expiresAt": 1773427200,
        "permissions": {
            "protocols": [
                {"protocolID": [2, "1sat ordinal"], "keyID": "*", "purpose": "Sign NFT listings"}
            ],
            "baskets": [
                {"name": "1sat-ordinals", "access": "read_write", "purpose": "Manage your NFT collection"}
            ],
            "certificates": [
                {"type": "https://socialcert.io/v1", "fields": ["displayName", "avatar"], "purpose": "Show your name"}
            ],
            "spending": {
                "perTransactionUsd": 10,
                "perSessionUsd": 100,
                "purpose": "Marketplace fees"
            },
            "counterparties": [
                {"type": "list-1sat-marketplace", "purpose": "Encrypted bid messages"}
            ]
        }
    })";

    Manifest m = ManifestFetcher::ParseFromJson(json);
    ASSERT_TRUE(m.valid);
    EXPECT_EQ(m.sourceNamespace, hodos::kSourceHodosLegacy);

    EXPECT_EQ(m.version, "1.0");
    EXPECT_EQ(m.name, "1Sat Market");
    EXPECT_EQ(m.description, "BSV NFT marketplace and trading platform");
    EXPECT_EQ(m.iconUrl, "https://1sat.market/icon.png");  // https — kept
    EXPECT_EQ(m.expiresAt, 1773427200);

    ASSERT_EQ(m.protocols.size(), 1u);
    EXPECT_EQ(m.protocols[0].securityLevel, 2);
    EXPECT_EQ(m.protocols[0].name, "1sat ordinal");
    EXPECT_EQ(m.protocols[0].keyId, "*");
    EXPECT_EQ(m.protocols[0].purpose, "Sign NFT listings");

    ASSERT_EQ(m.baskets.size(), 1u);
    EXPECT_EQ(m.baskets[0].name, "1sat-ordinals");
    EXPECT_EQ(m.baskets[0].access, "read_write");

    ASSERT_EQ(m.certificates.size(), 1u);
    EXPECT_EQ(m.certificates[0].type, "https://socialcert.io/v1");
    ASSERT_EQ(m.certificates[0].fields.size(), 2u);
    EXPECT_EQ(m.certificates[0].fields[0], "displayName");
    EXPECT_EQ(m.certificates[0].fields[1], "avatar");

    EXPECT_EQ(m.spending.perTransactionUsd, 10);
    EXPECT_EQ(m.spending.perSessionUsd, 100);
    EXPECT_EQ(m.spending.purpose, "Marketplace fees");

    ASSERT_EQ(m.counterparties.size(), 1u);
    EXPECT_EQ(m.counterparties[0].type, "list-1sat-marketplace");
    EXPECT_EQ(m.counterparties[0].purpose, "Encrypted bid messages");
}

// ============================================================================
// Forward compatibility: unknown fields ignored
// ============================================================================

TEST(ManifestFetcher, UnknownTopLevelFieldsAreIgnored) {
    // A manifest from a wallet two versions ahead — we should ignore the new
    // top-level field and still parse the known parts.
    const std::string json = R"({
        "version": "2.0",
        "name": "Future App",
        "quantumKeyDerivation": true,
        "futureFeatureWeDontUnderstand": {"foo": "bar"},
        "permissions": {
            "protocols": [{"protocolID": [2, "messagebox"], "purpose": "send messages"}]
        }
    })";

    Manifest m = ManifestFetcher::ParseFromJson(json);
    ASSERT_TRUE(m.valid);
    EXPECT_EQ(m.name, "Future App");
    ASSERT_EQ(m.protocols.size(), 1u);
    EXPECT_EQ(m.protocols[0].name, "messagebox");
}

TEST(ManifestFetcher, UnknownPermissionScopesAreIgnored) {
    // A future permission scope ("quantumChannels") inside permissions —
    // we should not crash, and we should still parse the known scopes.
    const std::string json = R"({
        "name": "App",
        "permissions": {
            "protocols": [{"protocolID": [1, "auth"], "purpose": "log in"}],
            "quantumChannels": [{"name": "channel-A"}],
            "baskets": [{"name": "default-NEVER-GRANT-THIS", "access": "read", "purpose": "test"}]
        }
    })";

    Manifest m = ManifestFetcher::ParseFromJson(json);
    ASSERT_TRUE(m.valid);
    EXPECT_EQ(m.protocols.size(), 1u);
    EXPECT_EQ(m.baskets.size(), 1u);
    // No carrier for quantumChannels — just dropped.
}

// ============================================================================
// Defaults applied for missing optional fields
// ============================================================================

TEST(ManifestFetcher, MissingOptionalFieldsUseDefaults) {
    const std::string json = R"({
        "name": "Minimal App",
        "permissions": {
            "protocols": [{"protocolID": [2, "test proto"]}]
        }
    })";

    Manifest m = ManifestFetcher::ParseFromJson(json);
    ASSERT_TRUE(m.valid);
    EXPECT_EQ(m.name, "Minimal App");
    EXPECT_EQ(m.description, "");      // missing → empty string
    EXPECT_EQ(m.iconUrl, "");
    EXPECT_EQ(m.expiresAt, 0);

    ASSERT_EQ(m.protocols.size(), 1u);
    EXPECT_EQ(m.protocols[0].keyId, "*");  // missing keyID → wildcard
    EXPECT_EQ(m.protocols[0].purpose, ""); // missing purpose → empty
}

TEST(ManifestFetcher, BasketDefaultsToReadAccessWhenMissing) {
    const std::string json = R"({
        "name": "App",
        "permissions": {
            "baskets": [{"name": "test-basket", "purpose": "look at stuff"}]
        }
    })";

    Manifest m = ManifestFetcher::ParseFromJson(json);
    ASSERT_TRUE(m.valid);
    ASSERT_EQ(m.baskets.size(), 1u);
    EXPECT_EQ(m.baskets[0].access, "read");
}

// ============================================================================
// Malformed entries are dropped, not errored
// ============================================================================

TEST(ManifestFetcher, ProtocolWithMissingNameIsDropped) {
    // protocolID is present but has no name (string at index 1) — drop the entry,
    // keep parsing the rest of the manifest.
    const std::string json = R"({
        "name": "App",
        "permissions": {
            "protocols": [
                {"protocolID": [2], "purpose": "broken"},
                {"protocolID": [2, "valid one"], "purpose": "ok"}
            ]
        }
    })";

    Manifest m = ManifestFetcher::ParseFromJson(json);
    ASSERT_TRUE(m.valid);
    ASSERT_EQ(m.protocols.size(), 1u);
    EXPECT_EQ(m.protocols[0].name, "valid one");
}

TEST(ManifestFetcher, BasketWithInvalidAccessLevelIsDropped) {
    // "admin" is not a recognised access level — drop. Lenient parse: don't
    // upgrade silently to read_write or read; just skip the bad entry.
    const std::string json = R"({
        "name": "App",
        "permissions": {
            "baskets": [
                {"name": "bad", "access": "admin"},
                {"name": "good", "access": "read"}
            ]
        }
    })";

    Manifest m = ManifestFetcher::ParseFromJson(json);
    ASSERT_TRUE(m.valid);
    ASSERT_EQ(m.baskets.size(), 1u);
    EXPECT_EQ(m.baskets[0].name, "good");
}

TEST(ManifestFetcher, CertificateWithMissingTypeIsDropped) {
    const std::string json = R"({
        "name": "App",
        "permissions": {
            "certificates": [
                {"fields": ["displayName"]},
                {"type": "https://socialcert.io/v1", "fields": ["bio"]}
            ]
        }
    })";

    Manifest m = ManifestFetcher::ParseFromJson(json);
    ASSERT_TRUE(m.valid);
    ASSERT_EQ(m.certificates.size(), 1u);
    EXPECT_EQ(m.certificates[0].type, "https://socialcert.io/v1");
}

// ============================================================================
// Failure modes — never throw, return invalid
// ============================================================================

TEST(ManifestFetcher, MalformedJsonReturnsInvalid) {
    const std::string bad = R"({"name": "broken")";  // missing closing brace
    Manifest m = ManifestFetcher::ParseFromJson(bad);
    EXPECT_FALSE(m.valid);
}

TEST(ManifestFetcher, EmptyStringReturnsInvalid) {
    Manifest m = ManifestFetcher::ParseFromJson("");
    EXPECT_FALSE(m.valid);
}

TEST(ManifestFetcher, NonObjectJsonReturnsInvalid) {
    // Valid JSON but not an object — `j.contains` would throw if we didn't
    // catch. Lenient parse means we return invalid rather than crash.
    Manifest m = ManifestFetcher::ParseFromJson("[1, 2, 3]");
    EXPECT_FALSE(m.valid);
}

// 🚨 INVERTED BY beta.3 PHASE 0.8. This test used to assert `{}` is VALID,
// with the comment "consumer logic decides whether to treat empty-permissions
// as no manifest needed". That assumption is exactly the shipped defect: no
// consumer ever made that decision, so a permission-free document produced a
// `manifest_connect_bundle` modal itemising nothing while the site held real
// access. `valid` now means "declared at least one recognised permission".
// See PHASE_CONTRACT.md §1. ⛔ Do not re-invert.
TEST(ManifestFetcher, EmptyObjectDeclaresNothingSoIsNotAManifest) {
    Manifest m = ManifestFetcher::ParseFromJson("{}");
    EXPECT_FALSE(m.valid);
}

TEST(ManifestFetcher, ObjectWithMetadataButNoPermissionsIsNotAManifest) {
    // A plain W3C web-app manifest. Falls back to plain domain_approval.
    Manifest m = ManifestFetcher::ParseFromJson(
        R"({"name":"App","short_name":"App","description":"d","display":"standalone"})");
    EXPECT_FALSE(m.valid);
}

TEST(ManifestFetcher, EmptyPermissionContainersAreStillNoPermissions) {
    EXPECT_FALSE(ManifestFetcher::ParseFromJson(R"({"permissions":{}})").valid);
    EXPECT_FALSE(ManifestFetcher::ParseFromJson(R"({"permissions":{"protocols":[]}})").valid);
    EXPECT_FALSE(ManifestFetcher::ParseFromJson(R"({"metanet":{"groupPermissions":{}}})").valid);
    EXPECT_FALSE(ManifestFetcher::ParseFromJson(
        R"({"metanet":{"groupPermissions":{"protocolPermissions":[]}}})").valid);
}

// ============================================================================
// Security-level normalization
// ============================================================================

TEST(ManifestFetcher, SecurityLevelOutsideRangeFallsBackToDefault) {
    // BRC-43 levels are 0, 1, 2. A dApp claiming level 99 is either confused
    // or malicious — fall back to default rather than honor the claim.
    const std::string json = R"({
        "name": "App",
        "permissions": {
            "protocols": [{"protocolID": [99, "weird"], "purpose": "x"}]
        }
    })";

    Manifest m = ManifestFetcher::ParseFromJson(json);
    ASSERT_TRUE(m.valid);
    ASSERT_EQ(m.protocols.size(), 1u);
    EXPECT_EQ(m.protocols[0].securityLevel, 2);  // default
}

// ============================================================================
// beta.3 Phase 0.8 — BRC-73, fixture-driven
//
// These read the CANONICAL fixtures in `demos/manifest-shapes/` — the same
// files `rust-wallet/src/manifest.rs` compiles in with include_str!. The path
// comes from HODOS_MANIFEST_FIXTURE_DIR, set in tests/CMakeLists.txt.
//
// ⛔ A missing fixture FAILS. It never skips. A skip is not a pass, and a
// harness that quietly stops reading its inputs is how you get a green run
// that tests nothing (see CLAUDE.md, "NEGATIVE CONTROL").
// ============================================================================

#ifndef HODOS_MANIFEST_FIXTURE_DIR
#error "HODOS_MANIFEST_FIXTURE_DIR must be defined by the build (see tests/CMakeLists.txt)"
#endif

namespace {

std::string ReadFixture(const std::string& filename) {
    const std::string path = std::string(HODOS_MANIFEST_FIXTURE_DIR) + "/" + filename;
    std::ifstream in(path, std::ios::binary);
    // Fail loudly and name the path — a silently-empty fixture would make every
    // assertion below vacuous.
    EXPECT_TRUE(in.good()) << "canonical fixture missing: " << path;
    std::ostringstream ss;
    ss << in.rdbuf();
    std::string body = ss.str();
    EXPECT_FALSE(body.empty()) << "canonical fixture is empty: " << path;
    return body;
}

}  // namespace

// ── P0.8-A1 — the acceptance case, through the C++ parse that feeds the log ──

TEST(ManifestBrc73, A1_BitgeniusLiveCaptureParsesFourProtocols) {
    Manifest m = ManifestFetcher::ParseFromJson(ReadFixture("bitgenius-live-capture.json"));
    ASSERT_TRUE(m.valid);
    // The "Triggering manifest_connect_bundle ... (N protocols...)" line prints
    // exactly this. Pre-fix it printed 0 while the site declared 4.
    ASSERT_EQ(m.protocols.size(), 4u);
    EXPECT_EQ(m.name, "BitGenius");
    EXPECT_EQ(m.sourceNamespace, hodos::kSourceMetanet);

    EXPECT_EQ(m.protocols[0].securityLevel, 1);
    EXPECT_EQ(m.protocols[0].name, "identity key retrieval");
    EXPECT_EQ(m.protocols[0].counterparty, "");  // Level 1 MAY omit
    EXPECT_EQ(m.protocols[0].purpose,
              "Use your public identity key as your passwordless BitGenius account.");

    EXPECT_EQ(m.protocols[1].securityLevel, 2);
    EXPECT_EQ(m.protocols[1].name, "server hmac");
    EXPECT_EQ(m.protocols[1].counterparty, "self");

    EXPECT_EQ(m.protocols[3].name, "3241645161d8");
    EXPECT_EQ(m.protocols[3].counterparty,
              "0279887cddd8cb44fc34793fa568dfb86badb1e8f1eace5976a6f4cf3f786cc893");

    // Every entry carries the site's own description — that text IS the modal.
    for (const auto& p : m.protocols) {
        EXPECT_FALSE(p.purpose.empty()) << "protocol " << p.name << " has no description";
    }
}

TEST(ManifestBrc73, MetanetFixtureParsesFourProtocolsWithWildcardKeyId) {
    Manifest m = ManifestFetcher::ParseFromJson(ReadFixture("brc73-metanet-protocols.json"));
    ASSERT_TRUE(m.valid);
    ASSERT_EQ(m.protocols.size(), 4u);
    EXPECT_EQ(m.sourceNamespace, hodos::kSourceMetanet);
    EXPECT_EQ(m.groupDescription, "Sign in and approve each checkout with your wallet.");
    for (const auto& p : m.protocols) {
        EXPECT_EQ(p.keyId, "*") << "BRC-73 has no keyID; wildcard is the translation";
    }
}

TEST(ManifestBrc73, BabbageLegacyNamespaceStillParses) {
    Manifest m = ManifestFetcher::ParseFromJson(ReadFixture("brc73-babbage-legacy.json"));
    ASSERT_TRUE(m.valid);
    EXPECT_EQ(m.protocols.size(), 2u);
    EXPECT_EQ(m.sourceNamespace, hodos::kSourceBabbage);
}

// ── P0.8-A8 — namespace precedence ──

TEST(ManifestBrc73, A8_MetanetWinsOverBabbage) {
    Manifest m = ManifestFetcher::ParseFromJson(ReadFixture("brc73-both-namespaces.json"));
    ASSERT_TRUE(m.valid);
    EXPECT_EQ(m.sourceNamespace, hodos::kSourceMetanet);
    ASSERT_EQ(m.protocols.size(), 4u) << "metanet's four, not babbage's one";
    for (const auto& p : m.protocols) {
        EXPECT_NE(p.name, "STALE legacy protocol")
            << "the deprecated namespace's content leaked through — precedence is wrong";
    }
}

TEST(ManifestBrc73, BabbageUsedWhenMetanetHasNoGroupPermissions) {
    // socialcert.net's real shape: a namespace key with no groupPermissions.
    Manifest m = ManifestFetcher::ParseFromJson(R"({
        "name":"X",
        "metanet":{"schemaVersion":1},
        "babbage":{"groupPermissions":{"protocolPermissions":[
            {"protocolID":[1,"identity key retrieval"],"description":"d"}]}}
    })");
    ASSERT_TRUE(m.valid);
    EXPECT_EQ(m.sourceNamespace, hodos::kSourceBabbage);
    EXPECT_EQ(m.protocols.size(), 1u);
}

TEST(ManifestBrc73, MetanetWinsOverOurLegacyTopLevelShape) {
    Manifest m = ManifestFetcher::ParseFromJson(R"({
        "metanet":{"groupPermissions":{"protocolPermissions":[
            {"protocolID":[1,"standard"],"description":"d"}]}},
        "permissions":{"protocols":[{"protocolID":[1,"legacy"],"purpose":"p"}]}
    })");
    ASSERT_TRUE(m.valid);
    EXPECT_EQ(m.sourceNamespace, hodos::kSourceMetanet);
    ASSERT_EQ(m.protocols.size(), 1u);
    EXPECT_EQ(m.protocols[0].name, "standard");
}

// ── Goal 5 — all four BRC-73 categories are data we already carry ──

TEST(ManifestBrc73, AllFourCategoriesParse) {
    Manifest m = ManifestFetcher::ParseFromJson(ReadFixture("brc73-all-categories.json"));
    ASSERT_TRUE(m.valid);
    EXPECT_EQ(m.protocols.size(), 4u);
    ASSERT_EQ(m.baskets.size(), 2u);
    ASSERT_EQ(m.certificates.size(), 1u);

    EXPECT_EQ(m.baskets[0].name, "tickets");
    EXPECT_EQ(m.baskets[0].access, "read_write")
        << "BRC-116 4.3: basket grants are binary and cover insert/list/remove";

    ASSERT_EQ(m.certificates[0].fields.size(), 2u);
    EXPECT_EQ(m.certificates[0].fields[0], "firstName");
    EXPECT_EQ(m.certificates[0].verifierPublicKey,
              "0279887cddd8cb44fc34793fa568dfb86badb1e8f1eace5976a6f4cf3f786cc893")
        << "BRC-116 4.4 scopes cert access by verifier — the user must see who receives it";
}

// ── P0.8-A5 / R-CAPS — the declared spend must never reach our cap fields ──
//
// NEGATIVE CONTROL: in applyGroupPermissions, assign the parsed `amount` to
// m.spending.perTransactionUsd instead of monthlySatoshis. This test goes red,
// and the frontend pre-fill harness then shows the site's number reaching the
// domain_permissions row.
TEST(ManifestBrc73, A5_SpendingAuthorizationNeverPopulatesOurCapFields) {
    Manifest m = ManifestFetcher::ParseFromJson(ReadFixture("brc73-all-categories.json"));
    ASSERT_TRUE(m.valid);
    EXPECT_EQ(m.spending.monthlySatoshis, 5000000);
    EXPECT_EQ(m.spending.perTransactionUsd, 0)
        << "monthly satoshis must never be read as a per-transaction USD cap";
    EXPECT_EQ(m.spending.perSessionUsd, 0)
        << "monthly satoshis must never be read as a per-session USD cap";
}

TEST(ManifestBrc73, SpendingAuthorizationAloneIsADeclaredPermission) {
    Manifest m = ManifestFetcher::ParseFromJson(
        R"({"metanet":{"groupPermissions":{"spendingAuthorization":{"amount":10000,"description":"tips"}}}})");
    ASSERT_TRUE(m.valid);
    EXPECT_EQ(m.spending.monthlySatoshis, 10000);
}

TEST(ManifestBrc73, NegativeSpendingAmountClampsToZero) {
    Manifest m = ManifestFetcher::ParseFromJson(R"({"metanet":{"groupPermissions":{
        "protocolPermissions":[{"protocolID":[1,"p"],"description":"d"}],
        "spendingAuthorization":{"amount":-999999,"description":"d"}}}})");
    ASSERT_TRUE(m.valid);
    EXPECT_EQ(m.spending.monthlySatoshis, 0);
}

// ── P0.8-A3 / A7 — the two shapes that must NOT produce a bundle ──

TEST(ManifestBrc73, A3_UnrecognisedShapeDeclaresNothingSoIsNotAManifest) {
    Manifest m = ManifestFetcher::ParseFromJson(ReadFixture("unrecognised-shape.json"));
    EXPECT_FALSE(m.valid)
        << "a permission-free manifest must fall back to domain_approval, "
           "not render as a permission-free itemised bundle";
}

TEST(ManifestBrc73, A7_HtmlPageYieldsNoManifest) {
    Manifest m = ManifestFetcher::ParseFromJson(ReadFixture("not-a-manifest.html"));
    EXPECT_FALSE(m.valid) << "a 200 text/html SPA page is never a manifest";
}

// ── Our legacy shape — regression guard ──

TEST(ManifestBrc73, HodosLegacyShapeStillParses) {
    Manifest m = ManifestFetcher::ParseFromJson(ReadFixture("hodos-legacy-permissions.json"));
    ASSERT_TRUE(m.valid);
    EXPECT_EQ(m.sourceNamespace, hodos::kSourceHodosLegacy);
    // Pre-Phase-0.8 this was 0: the parser only read protocolID:[lvl,name],
    // and our OWN documented example uses the flattened name + securityLevel
    // spelling. Our shape never parsed its own example.
    ASSERT_EQ(m.protocols.size(), 1u);
    EXPECT_EQ(m.protocols[0].name, "identity key retrieval");
    EXPECT_EQ(m.protocols[0].securityLevel, 1);
    ASSERT_EQ(m.baskets.size(), 1u);
    EXPECT_EQ(m.baskets[0].access, "read") << "our shape's explicit default";
    // Our shape's spending IS in our unit and period.
    EXPECT_EQ(m.spending.perTransactionUsd, 100);
    EXPECT_EQ(m.spending.perSessionUsd, 1000);
    EXPECT_EQ(m.spending.monthlySatoshis, 0);
}

// ── PACT is not mapped ──

TEST(ManifestBrc73, CounterpartyPermissionsAreNotMappedToCounterpartyGrants) {
    Manifest m = ManifestFetcher::ParseFromJson(R"({"metanet":{
        "groupPermissions":{"protocolPermissions":[{"protocolID":[1,"p"],"description":"d"}]},
        "counterpartyPermissions":{"protocols":[{"protocolName":"convo","description":"peer"}]}
    }})");
    ASSERT_TRUE(m.valid);
    EXPECT_TRUE(m.counterparties.empty())
        << "PACT declares Level-2 protocol NAMES, not pubkeys — mapping them "
           "into counterparty grants would write a grant nobody asked for";
}

// ── Hostile input ──

TEST(ManifestBrc73, ArrayLengthsAreCapped) {
    std::string entries;
    for (int i = 0; i < 500; ++i) {
        if (i) entries += ",";
        entries += R"({"protocolID":[1,"p)" + std::to_string(i) + R"("],"description":"d"})";
    }
    Manifest m = ManifestFetcher::ParseFromJson(
        R"({"metanet":{"groupPermissions":{"protocolPermissions":[)" + entries + "]}}}");
    ASSERT_TRUE(m.valid);
    EXPECT_EQ(m.protocols.size(), hodos::kMaxManifestEntries)
        << "500 declared entries must not all reach the modal";
}

TEST(ManifestBrc73, StringsAreTruncatedNotRejected) {
    const std::string longStr(5000, 'A');
    Manifest m = ManifestFetcher::ParseFromJson(
        R"({"name":")" + longStr + R"(","metanet":{"groupPermissions":{"protocolPermissions":[
            {"protocolID":[1,"p"],"description":")" + longStr + R"("}]}}})");
    ASSERT_TRUE(m.valid);
    EXPECT_EQ(m.name.size(), hodos::kMaxManifestStr);
    EXPECT_EQ(m.protocols[0].purpose.size(), hodos::kMaxManifestStr);
}

TEST(ManifestBrc73, MultibyteStringsTruncateOnCodepointBoundaries) {
    // "e-acute" is 2 bytes. Cutting at kMaxManifestStr lands mid-sequence; the
    // parser must walk back so the result is still valid UTF-8.
    std::string longStr;
    for (int i = 0; i < 5000; ++i) longStr += "\xC3\xA9";
    Manifest m = ManifestFetcher::ParseFromJson(
        R"({"metanet":{"groupPermissions":{"protocolPermissions":[
            {"protocolID":[1,"p"],"description":")" + longStr + R"("}]}}})");
    ASSERT_TRUE(m.valid);
    const std::string& out = m.protocols[0].purpose;
    EXPECT_LE(out.size(), hodos::kMaxManifestStr);
    EXPECT_EQ(out.size() % 2, 0u) << "truncated mid-codepoint — invalid UTF-8";
    // nlohmann throws on invalid UTF-8 when serializing; this is the real test,
    // because the interceptor json-dumps these strings into the modal payload.
    EXPECT_NO_THROW({ nlohmann::json j = out; (void)j.dump(); });
}

TEST(ManifestBrc73, CertFieldsAreCapped) {
    std::string fields;
    for (int i = 0; i < 500; ++i) {
        if (i) fields += ",";
        fields += "\"f" + std::to_string(i) + "\"";
    }
    Manifest m = ManifestFetcher::ParseFromJson(
        R"({"metanet":{"groupPermissions":{"certificateAccess":[
            {"type":"t","fields":[)" + fields + R"(],"description":"d"}]}}})");
    ASSERT_TRUE(m.valid);
    EXPECT_EQ(m.certificates[0].fields.size(), hodos::kMaxManifestFields);
}

TEST(ManifestBrc73, IconUrlAcceptsHttpsOnly) {
    const auto mk = [](const char* icon) {
        return std::string(R"({"iconUrl":")") + icon +
               R"(","metanet":{"groupPermissions":{"protocolPermissions":[
                   {"protocolID":[1,"p"],"description":"d"}]}}})";
    };
    EXPECT_EQ(ManifestFetcher::ParseFromJson(mk("https://x.example/i.png")).iconUrl,
              "https://x.example/i.png");
    // The connect modal renders iconUrl into <img src>. Nothing but https.
    for (const char* bad : {"javascript:alert(1)",
                            "data:image/svg+xml;base64,PHN2Zz48L3N2Zz4=",
                            "http://x.example/i.png",
                            "/relative.png",
                            "https://"}) {
        EXPECT_EQ(ManifestFetcher::ParseFromJson(mk(bad)).iconUrl, "")
            << "iconUrl must be dropped: " << bad;
    }
}

// ── P0.8-A6 / R-PATH — where we look, and where we refuse to look ──

TEST(ManifestUrls, A6_BothLocationsStandardFirst) {
    const auto urls = ManifestFetcher::ManifestUrls("teragun.com");
    ASSERT_EQ(urls.size(), 2u);
    EXPECT_EQ(urls[0], "https://teragun.com/manifest.json");
    EXPECT_EQ(urls[1], "https://teragun.com/.well-known/wallet-manifest.json");
}

TEST(ManifestUrls, ExplicitSchemePreservedAndTrailingSlashStripped) {
    const auto urls = ManifestFetcher::ManifestUrls("https://app.example.com/");
    ASSERT_EQ(urls.size(), 2u);
    EXPECT_EQ(urls[0], "https://app.example.com/manifest.json");
}

TEST(ManifestUrls, RejectAnythingThatIsNotABareAuthority) {
    for (const char* bad : {"", "   ", "/", "https://",
                            "evil.com/a/b", "https://evil.com/deep/path",
                            "evil.com?x=1", "evil.com#frag",
                            "https://127.0.0.1:5137@evil.com",
                            "evil.com\\..\\x", "evil .com", "evil.com\tx",
                            "file:///etc/passwd", "javascript://evil.com"}) {
        EXPECT_TRUE(ManifestFetcher::ManifestUrls(bad).empty())
            << "origin must not produce a fetch URL: " << bad;
    }
}

TEST(ManifestUrls, HttpIsUpgradedExceptOnLoopback) {
    EXPECT_EQ(ManifestFetcher::ManifestUrls("http://evil.com")[0],
              "https://evil.com/manifest.json");
    EXPECT_EQ(ManifestFetcher::ManifestUrls("http://localhost:8080")[0],
              "http://localhost:8080/manifest.json");
    EXPECT_EQ(ManifestFetcher::ManifestUrls("http://127.0.0.1:3000")[0],
              "http://127.0.0.1:3000/manifest.json");
    // A host that merely starts with "localhost" is not loopback.
    EXPECT_EQ(ManifestFetcher::ManifestUrls("http://localhost.evil.com")[0],
              "https://localhost.evil.com/manifest.json");
}

TEST(ManifestUrls, SurroundingWhitespaceIsTrimmed) {
    EXPECT_EQ(ManifestFetcher::ManifestUrls("  evil.com\n")[0],
              "https://evil.com/manifest.json");
}
