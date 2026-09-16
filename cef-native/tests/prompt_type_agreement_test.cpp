// prompt_type_agreement_test.cpp — beta.3 Phase 10e, panel finding `F3-10b`.
//
// Covers hodos::IsConnectPromptType / IsDomainTrustPromptType in
// include/core/PromptTypes.h. ⭐ These are the REAL functions the production code
// calls, not a mirror of them — that is the whole point of the header existing.
// The defect this guards was two copies of one list in two files, silently drifting.
//
// The invariant, stated once: a CONNECT prompt must ALSO be a domain-trust prompt.
// `isDomainTrustPrompt` decides whether a single-use `X-User-Approved` token is
// attached to the pending entry, and connect entries are resolved by
// `popConnectForDomain`'s deliberate fan-out — one Allow resumes every waiting call
// from that domain. A connect entry carrying a live token would be replayed with it,
// which is CU-1, the defect Phase 10b closed.
//
// ⛔ NEGATIVE CONTROL (CLAUDE.md hard rule). The pre-fix state is reproduced by
// dropping `brc100_auth` from IsDomainTrustPromptType, i.e. giving it its own list
// again:
//
//   inline bool IsDomainTrustPromptType(const std::string& t) {
//       return t == "domain_approval" || t == "manifest_connect_bundle";
//   }
//
// Expected RED: ConnectImpliesDomainTrust.EveryConnectTypeIsDomainTrust fails on
// "brc100_auth" — the exact drift that was live in the tree until 2026-09-16.

#include <gtest/gtest.h>
#include "core/PromptTypes.h"

#include <string>
#include <vector>

namespace {

// Every prompt type this build can produce. Kept explicit rather than derived, so
// adding a type to the product without thinking about this file shows up here.
const std::vector<std::string> kConnectTypes = {
    "domain_approval",
    "brc100_auth",
    "manifest_connect_bundle",
};

const std::vector<std::string> kKindTypes = {
    "payment_confirmation",
    "rate_limit_exceeded",
    "certificate_disclosure",
    "identity_key_reveal",
    "key_linkage_reveal",
    "protocol_permission",
    "basket_permission",
    "counterparty_permission",
};

}  // namespace

// ⭐ The load-bearing assertion. If this ever fails, a connect prompt is being
// handed a single-use approval token and the fan-out will replay it.
TEST(ConnectImpliesDomainTrust, EveryConnectTypeIsDomainTrust) {
    for (const auto& t : kConnectTypes) {
        EXPECT_TRUE(hodos::IsConnectPromptType(t)) << t << " should be a connect type";
        EXPECT_TRUE(hodos::IsDomainTrustPromptType(t))
            << t << " is a connect prompt but would be given an X-User-Approved token — "
            << "that is CU-1 reopened for this type";
    }
}

// The other half: a kind prompt authorises ONE call and must keep its token.
TEST(KindPrompts, AreNeitherConnectNorDomainTrust) {
    for (const auto& t : kKindTypes) {
        EXPECT_FALSE(hodos::IsConnectPromptType(t)) << t << " must not fan out";
        EXPECT_FALSE(hodos::IsDomainTrustPromptType(t))
            << t << " must keep its single-use token, or the call it authorises cannot proceed";
    }
}

// Control: the classification is exact, not a prefix or substring match. A hostile
// or merely careless type string must not fall into the connect class.
TEST(PromptTypes, UnknownStringsAreNotConnect) {
    const char* strangers[] = {
        "", " ", "domain", "domain_approval ", " domain_approval", "DOMAIN_APPROVAL",
        "domain_approval_x", "xdomain_approval", "manifest_connect", "brc100",
    };
    for (const char* t : strangers) {
        EXPECT_FALSE(hodos::IsConnectPromptType(t)) << "unexpectedly connect: [" << t << "]";
        EXPECT_FALSE(hodos::IsDomainTrustPromptType(t)) << "unexpectedly trust: [" << t << "]";
    }
}
