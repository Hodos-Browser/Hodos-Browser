// ipc_role_guard_test.cpp — P0.5-B1 (the self-nav grant-forgery gate)
//
// Covers hodos::IsGrantApproveMessage / IsApprovalOverlayRole / IsTabRole in
// include/core/IpcAuth.h. Together they are the whole fix: at the single Layer-2
// choke in SimpleHandler::OnProcessMessageReceived a message is DENIED iff
//   IsGrantApproveMessage(name) && !IsApprovalOverlayRole(role)
// (plus the narrow tab-only denial of domain_permission_invalidate). This mirrors
// that decision here as GateDenies(), so the authorization logic is falsifiable
// without a live CEF browser — the same evidence class as wallet_ssrf_guard_test.
//
// ⛔ NEGATIVE CONTROL (CLAUDE.md hard rule; HARNESS.md §6). Every assertion must
// be seen to FAIL against the pre-fix behaviour. The pre-fix state is "no gate on
// the sibling arms", reproduced by weakening either predicate to accept-all:
//
//   IsApprovalOverlayRole := return true;   // pre-fix: any internal role passed
//   IsGrantApproveMessage := return false;  // pre-fix: family not recognised
//
// Expected RED under IsApprovalOverlayRole := true:
//   SelfNavTab.* — every EXPECT_TRUE(GateDenies(tab, msg)) flips (a self-navved
//                  tab is no longer denied any grant/approve/reveal) — i.e. the
//                  exact hole this fix closes.
// Expected RED under IsGrantApproveMessage := false:
//   FamilyClassified.* — the family messages are no longer recognised, so
//                        GateDenies() returns false for them and SelfNavTab.* flip.
// The ApprovalOverlay.Allows* and Legit.* suites are the other half of the
// control: they must stay GREEN under BOTH weakenings, proving the gate does not
// over-reject the real approval-overlay flow or the settings/wallet revoke flow.

#include <gtest/gtest.h>
#include <string>
#include "core/IpcAuth.h"

namespace {

// Mirrors the Layer-2 decision for the grant/approve/reveal family.
bool GateDenies(const std::string& message, const std::string& role) {
    return hodos::IsGrantApproveMessage(message) &&
           !hodos::IsApprovalOverlayRole(role);
}

// The exact family, as emitted only by BRC100AuthOverlayRoot.tsx.
const char* kFamily[] = {
    "add_domain_permission",
    "add_domain_permission_advanced",
    "grant_scoped_permission",
    "approve_cert_fields",
    "approve_identity_key_reveal",
    "approve_key_linkage_reveal",
    "brc100_auth_response",
};

// ---- Predicate: message classification ----

TEST(FamilyClassified, EveryPrivilegedArmIsRecognised) {
    for (const char* m : kFamily) {
        EXPECT_TRUE(hodos::IsGrantApproveMessage(m)) << "family message: " << m;
    }
}

TEST(FamilyClassified, NonFamilyMessagesAreNot) {
    // Web-allowlisted / benign IPCs must NOT be swept into the grant gate.
    EXPECT_FALSE(hodos::IsGrantApproveMessage("wallet_call"));
    EXPECT_FALSE(hodos::IsGrantApproveMessage("cosmetic_class_id_query"));
    EXPECT_FALSE(hodos::IsGrantApproveMessage("qr_found"));
    EXPECT_FALSE(hodos::IsGrantApproveMessage("find_result_js"));
    // Deliberately handled by the separate tab-only guard, not the family:
    EXPECT_FALSE(hodos::IsGrantApproveMessage("domain_permission_invalidate"));
    EXPECT_FALSE(hodos::IsGrantApproveMessage(""));
    EXPECT_FALSE(hodos::IsGrantApproveMessage("grant_scoped_permission_x"));
}

// ---- Predicate: approval-overlay role ----

TEST(ApprovalOverlay, AllowsExactlyTheTwoOverlayRoles) {
    EXPECT_TRUE(hodos::IsApprovalOverlayRole("notification"));
    EXPECT_TRUE(hodos::IsApprovalOverlayRole("brc100auth"));
}

TEST(ApprovalOverlay, RejectsEverythingElse) {
    EXPECT_FALSE(hodos::IsApprovalOverlayRole("tab_7"));
    EXPECT_FALSE(hodos::IsApprovalOverlayRole("wallet"));
    EXPECT_FALSE(hodos::IsApprovalOverlayRole("settings"));
    EXPECT_FALSE(hodos::IsApprovalOverlayRole("backup"));
    EXPECT_FALSE(hodos::IsApprovalOverlayRole(""));
    // Regression guard for the 15a3422 role-string bug: the underscore spelling
    // must NOT be accepted (that arm would be dead, not authorised).
    EXPECT_FALSE(hodos::IsApprovalOverlayRole("brc100_auth"));
}

// ---- Predicate: tab role ----

TEST(TabRole, MatchesTabIdsOnly) {
    EXPECT_TRUE(hodos::IsTabRole("tab_0"));
    EXPECT_TRUE(hodos::IsTabRole("tab_12345"));
    EXPECT_FALSE(hodos::IsTabRole("notification"));
    EXPECT_FALSE(hodos::IsTabRole("wallet"));
    EXPECT_FALSE(hodos::IsTabRole("ta"));
    EXPECT_FALSE(hodos::IsTabRole(""));
}

// ---- The gate decision (the property under test) ----

TEST(SelfNavTab, IsDeniedEveryPrivilegedArm) {
    // A self-navigated web page always has role "tab_<id>".
    for (const char* m : kFamily) {
        EXPECT_TRUE(GateDenies(m, "tab_9")) << "tab must be denied: " << m;
    }
}

TEST(SelfNavTab, OtherNonApprovalRolesAlsoDenied) {
    // Even other internal overlays (wallet/settings) are not the approval overlay
    // and never legitimately send these — deny them too (default-deny).
    for (const char* m : kFamily) {
        EXPECT_TRUE(GateDenies(m, "wallet")) << "wallet must be denied: " << m;
        EXPECT_TRUE(GateDenies(m, "settings")) << "settings must be denied: " << m;
    }
}

TEST(ApprovalOverlay, IsAllowedEveryPrivilegedArm) {
    for (const char* m : kFamily) {
        EXPECT_FALSE(GateDenies(m, "notification")) << "notification must pass: " << m;
        EXPECT_FALSE(GateDenies(m, "brc100auth")) << "brc100auth must pass: " << m;
    }
}

TEST(Legit, NonFamilyMessagesUnaffectedByTheFamilyGate) {
    // wallet_call from a tab is the dApp bridge — the family gate must not touch
    // it (it is gated downstream by the Rust engine, not here).
    EXPECT_FALSE(GateDenies("wallet_call", "tab_9"));
    EXPECT_FALSE(GateDenies("cosmetic_class_id_query", "tab_9"));
    // domain_permission_invalidate is not in the family gate; its tab-only denial
    // is a separate rule. From settings/wallet it must remain allowed.
    EXPECT_FALSE(hodos::IsTabRole("settings"));
    EXPECT_FALSE(hodos::IsTabRole("wallet"));
    EXPECT_TRUE(hodos::IsTabRole("tab_9"));  // the one case the separate rule denies
}

}  // namespace
