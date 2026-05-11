// manifest_fetcher_test.cpp — parse-only tests for ManifestFetcher.
//
// We test ParseFromJson directly because it's pure: same input, same output,
// no network. Fetch() is a thin wrapper over SyncHttpClient + ParseFromJson;
// once parse is solid, Fetch is just URL building + size cap + delegation.
// Network behavior is verified by hand via curl smoke against a real server.

#include "core/ManifestFetcher.h"

#include <gtest/gtest.h>

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

    EXPECT_EQ(m.version, "1.0");
    EXPECT_EQ(m.name, "1Sat Market");
    EXPECT_EQ(m.description, "BSV NFT marketplace and trading platform");
    EXPECT_EQ(m.iconUrl, "https://1sat.market/icon.png");
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

TEST(ManifestFetcher, MinimalEmptyObjectIsValid) {
    // An empty `{}` is technically valid JSON — describes an app with no
    // declared permissions. Parse should succeed; consumer logic decides
    // whether to treat empty-permissions as "no manifest needed."
    Manifest m = ManifestFetcher::ParseFromJson("{}");
    EXPECT_TRUE(m.valid);
    EXPECT_EQ(m.name, "");
    EXPECT_TRUE(m.protocols.empty());
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
