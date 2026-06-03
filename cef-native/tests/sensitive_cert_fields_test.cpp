// sensitive_cert_fields_test.cpp — unit tests for SensitiveCertFields.h.
//
// Phase 2.6-C.1. Mirrors the Rust unit tests in
// rust-wallet/src/permission_service/context_builder/sensitive_cert_fields.rs
// 1:1 so any divergence between the two implementations shows up immediately.
//
// Test discipline:
//   - One concern per test, present-tense indicative naming
//     ("SSN_IsSensitive" not "TestSensitiveField1").
//   - No fixture state — each test constructs its own inputs.
//   - Cover positive (must-prompt) / negative (benign) / heuristic
//     (unknown cert type) / mixed-sensitivity (whole-request routing).

#include "core/SensitiveCertFields.h"

#include <gtest/gtest.h>

#include <string>
#include <vector>

using hodos::AnyRequestedCertFieldSensitive;
using hodos::FieldNameMatchesSensitiveHeuristic;
using hodos::IsSensitiveCertField;
using hodos::NormalizeFieldName;

// ============================================================================
// Normalization
// ============================================================================

TEST(SensitiveCertFields, NormalizeLowercasesAscii) {
    EXPECT_EQ(NormalizeFieldName("SSN"), "ssn");
    EXPECT_EQ(NormalizeFieldName("Date_Of_Birth"), "date_of_birth");
}

TEST(SensitiveCertFields, NormalizeCollapsesSeparators) {
    EXPECT_EQ(NormalizeFieldName("drivers-license"), "drivers_license");
    EXPECT_EQ(NormalizeFieldName("passport.number"), "passport_number");
    EXPECT_EQ(NormalizeFieldName("multi   space"), "multi_space");
}

TEST(SensitiveCertFields, NormalizeStripsLeadingAndTrailingUnderscores) {
    EXPECT_EQ(NormalizeFieldName("__leading"), "leading");
    EXPECT_EQ(NormalizeFieldName("trailing___"), "trailing");
}

TEST(SensitiveCertFields, NormalizeEmptyStringIsEmpty) {
    EXPECT_EQ(NormalizeFieldName(""), "");
    EXPECT_EQ(NormalizeFieldName("___"), "");
}

TEST(SensitiveCertFields, NormalizeSplitsCamelCaseBoundaries) {
    EXPECT_EQ(NormalizeFieldName("legalName"), "legal_name");
    EXPECT_EQ(NormalizeFieldName("governmentId"), "government_id");
    EXPECT_EQ(NormalizeFieldName("passportNumber"), "passport_number");
    EXPECT_EQ(NormalizeFieldName("dateOfBirth"), "date_of_birth");
    // Acronym-internal: SSN, IBAN, DOB stay together (no split between uppers)
    EXPECT_EQ(NormalizeFieldName("mySSN"), "my_ssn");
    EXPECT_EQ(NormalizeFieldName("myIBANCode"), "my_ibancode");
    // Digit→upper boundary also splits
    EXPECT_EQ(NormalizeFieldName("abc123Def"), "abc123_def");
}

// ============================================================================
// Positive: known sensitive fields (must always route to SensitiveCertField)
// ============================================================================

TEST(SensitiveCertFields, SsnIsSensitive) {
    EXPECT_TRUE(IsSensitiveCertField("", "SSN"));
    EXPECT_TRUE(IsSensitiveCertField("any-cert", "ssn"));
    EXPECT_TRUE(IsSensitiveCertField("", "Social_Security_Number"));
}

TEST(SensitiveCertFields, PassportFieldsAreSensitive) {
    EXPECT_TRUE(IsSensitiveCertField("", "passport"));
    EXPECT_TRUE(IsSensitiveCertField("", "passportNumber"));
    EXPECT_TRUE(IsSensitiveCertField("", "passport_id"));
}

TEST(SensitiveCertFields, GovernmentIdFieldsAreSensitive) {
    EXPECT_TRUE(IsSensitiveCertField("", "drivers_license"));
    EXPECT_TRUE(IsSensitiveCertField("", "drivers-license"));
    EXPECT_TRUE(IsSensitiveCertField("", "driver_licence"));
    EXPECT_TRUE(IsSensitiveCertField("", "national_id"));
    EXPECT_TRUE(IsSensitiveCertField("", "governmentId"));
    EXPECT_TRUE(IsSensitiveCertField("", "state_id"));
    EXPECT_TRUE(IsSensitiveCertField("", "tax_id"));
    EXPECT_TRUE(IsSensitiveCertField("", "license_number"));
}

TEST(SensitiveCertFields, DateOfBirthFieldsAreSensitive) {
    EXPECT_TRUE(IsSensitiveCertField("", "dob"));
    EXPECT_TRUE(IsSensitiveCertField("", "DOB"));
    EXPECT_TRUE(IsSensitiveCertField("", "date_of_birth"));
    EXPECT_TRUE(IsSensitiveCertField("", "birthDate"));
    EXPECT_TRUE(IsSensitiveCertField("", "birthday"));
}

TEST(SensitiveCertFields, FullLegalNameIsSensitive) {
    EXPECT_TRUE(IsSensitiveCertField("", "legal_name"));
    EXPECT_TRUE(IsSensitiveCertField("", "legalName"));
    EXPECT_TRUE(IsSensitiveCertField("", "full_legal_name"));
    EXPECT_TRUE(IsSensitiveCertField("", "maiden_name"));
}

TEST(SensitiveCertFields, ResidentialAddressIsSensitive) {
    EXPECT_TRUE(IsSensitiveCertField("", "address"));
    EXPECT_TRUE(IsSensitiveCertField("", "home_address"));
    EXPECT_TRUE(IsSensitiveCertField("", "residential_address"));
    EXPECT_TRUE(IsSensitiveCertField("", "street_address"));
    // Suffix rule: any *_address (without "email") is sensitive
    EXPECT_TRUE(IsSensitiveCertField("", "billing_address"));
    EXPECT_TRUE(IsSensitiveCertField("", "shipping_address"));
    EXPECT_TRUE(IsSensitiveCertField("", "work_address"));
}

TEST(SensitiveCertFields, BiometricFieldsAreSensitive) {
    EXPECT_TRUE(IsSensitiveCertField("", "biometric"));
    EXPECT_TRUE(IsSensitiveCertField("", "fingerprint"));
    EXPECT_TRUE(IsSensitiveCertField("", "face_print"));
    EXPECT_TRUE(IsSensitiveCertField("", "voice_print"));
    EXPECT_TRUE(IsSensitiveCertField("", "retina"));
    EXPECT_TRUE(IsSensitiveCertField("", "iris_scan"));
}

TEST(SensitiveCertFields, FinancialAccountFieldsAreSensitive) {
    EXPECT_TRUE(IsSensitiveCertField("", "bank_account"));
    EXPECT_TRUE(IsSensitiveCertField("", "account_number"));
    EXPECT_TRUE(IsSensitiveCertField("", "routing_number"));
    EXPECT_TRUE(IsSensitiveCertField("", "credit_card"));
    EXPECT_TRUE(IsSensitiveCertField("", "card_number"));
    EXPECT_TRUE(IsSensitiveCertField("", "iban"));
    EXPECT_TRUE(IsSensitiveCertField("", "swift_code"));
}

// ============================================================================
// Negative: benign profile fields (must NOT route to SensitiveCertField)
// ============================================================================

TEST(SensitiveCertFields, DisplayNameIsNotSensitive) {
    EXPECT_FALSE(IsSensitiveCertField("", "displayName"));
    EXPECT_FALSE(IsSensitiveCertField("", "display_name"));
}

TEST(SensitiveCertFields, AvatarFieldsAreNotSensitive) {
    EXPECT_FALSE(IsSensitiveCertField("", "avatarURL"));
    EXPECT_FALSE(IsSensitiveCertField("", "avatar_url"));
    EXPECT_FALSE(IsSensitiveCertField("", "avatar"));
}

TEST(SensitiveCertFields, EmailAddressIsNotSensitive) {
    // "email" itself isn't on the sensitive list AND the "_address" suffix
    // rule explicitly excludes email-containing fields.
    EXPECT_FALSE(IsSensitiveCertField("", "email"));
    EXPECT_FALSE(IsSensitiveCertField("", "email_address"));
    EXPECT_FALSE(IsSensitiveCertField("", "emailAddress"));
    EXPECT_FALSE(IsSensitiveCertField("", "work_email_address"));
}

TEST(SensitiveCertFields, FirstLastFullNameAreNotSensitive) {
    // Display first/last names are common profile fields. Only "legal_name",
    // "full_legal_name", and "maiden_name" are flagged.
    EXPECT_FALSE(IsSensitiveCertField("", "firstName"));
    EXPECT_FALSE(IsSensitiveCertField("", "lastName"));
    EXPECT_FALSE(IsSensitiveCertField("", "given_name"));
    EXPECT_FALSE(IsSensitiveCertField("", "family_name"));
    EXPECT_FALSE(IsSensitiveCertField("", "full_name"));
    EXPECT_FALSE(IsSensitiveCertField("", "surname"));
}

TEST(SensitiveCertFields, HandleAndUsernameAreNotSensitive) {
    EXPECT_FALSE(IsSensitiveCertField("", "handle"));
    EXPECT_FALSE(IsSensitiveCertField("", "username"));
    EXPECT_FALSE(IsSensitiveCertField("", "screenName"));
}

TEST(SensitiveCertFields, EmptyFieldNameIsNotSensitive) {
    EXPECT_FALSE(IsSensitiveCertField("", ""));
    EXPECT_FALSE(IsSensitiveCertField("known-cert", ""));
}

// ============================================================================
// Heuristic: unknown cert type
// ============================================================================

TEST(SensitiveCertFields, HeuristicFiresForUnknownCertType) {
    // The heuristic does not care about cert type — passport patterns trip on
    // any cert. Realizes the "over-classification" principle from kickoff Q2.
    EXPECT_TRUE(IsSensitiveCertField("brand-new-cert-type-abc==", "passport_number"));
    EXPECT_TRUE(IsSensitiveCertField("brand-new-cert-type-abc==", "SSN"));
    EXPECT_TRUE(IsSensitiveCertField("brand-new-cert-type-abc==", "date_of_birth"));
}

TEST(SensitiveCertFields, HeuristicDoesNotFireForBenignFieldOnUnknownCert) {
    EXPECT_FALSE(IsSensitiveCertField("brand-new-cert-type-abc==", "preferredName"));
    EXPECT_FALSE(IsSensitiveCertField("brand-new-cert-type-abc==", "displayName"));
}

TEST(SensitiveCertFields, HeuristicHandlesEmptyFieldName) {
    EXPECT_FALSE(FieldNameMatchesSensitiveHeuristic(""));
    EXPECT_FALSE(FieldNameMatchesSensitiveHeuristic("_____"));
}

// ============================================================================
// Mixed-sensitivity request — whole request routing
// ============================================================================

TEST(SensitiveCertFields, MixedRequestWithAnySensitiveFieldIsSensitive) {
    std::vector<std::string> fields = {"displayName", "avatarURL", "SSN"};
    EXPECT_TRUE(AnyRequestedCertFieldSensitive("", fields));
}

TEST(SensitiveCertFields, AllBenignRequestIsNotSensitive) {
    std::vector<std::string> fields = {"displayName", "avatarURL", "email", "handle"};
    EXPECT_FALSE(AnyRequestedCertFieldSensitive("", fields));
}

TEST(SensitiveCertFields, EmptyRequestIsNotSensitive) {
    std::vector<std::string> fields;
    EXPECT_FALSE(AnyRequestedCertFieldSensitive("", fields));
}

TEST(SensitiveCertFields, SingleSensitiveFieldRoutesWholeRequest) {
    std::vector<std::string> fields = {"dob"};
    EXPECT_TRUE(AnyRequestedCertFieldSensitive("", fields));
}
