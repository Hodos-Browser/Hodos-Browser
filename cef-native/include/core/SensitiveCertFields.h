// SensitiveCertFields — classifier for high-sensitivity certificate fields.
//
// Phase 2.6-C.1 (Sigma-BRC121-Sprint). Closes the gap recorded in memory
// `project_sensitive_cert_field_classifier_gap`: prior to this header, the
// engine's PermissionCallKind::SensitiveCertField branch (PermissionEngine.cpp:43)
// was wired but unreachable, because classifyCallKind in
// HttpRequestInterceptor.cpp routed every /proveCertificate to
// CertificateDisclosure regardless of field shape.
//
// This header provides the pure-logic decision for "is this field one we
// should ALWAYS prompt the user on, regardless of any cert_field_permissions
// row that might pre-approve it?". Examples: SSN, passport number, date of
// birth, full legal name, residential address, biometric identifiers,
// financial account identifiers. The decision is over-inclusive by design —
// per Phase 2.6-C kickoff Q2, an extra prompt on a benign field is annoying
// but safe; a silent disclosure of a sensitive field is a data leak and
// unacceptable.
//
// Design intent:
//   - PURE LOGIC. No CEF dependencies, no globals, no I/O. Same input always
//     produces the same output. Trivially unit-testable; the first C++ tests
//     in cef-native/tests/sensitive_cert_fields_test.cpp use these functions
//     directly.
//   - HEADER-ONLY. All implementations are inline. Mirrors the small,
//     constants-driven design of PermissionEngine's branch helpers.
//   - PARITY with Rust. The same list of patterns and the same heuristic
//     live in rust-wallet/src/permission_service/context_builder.rs under
//     `sensitive_cert_fields`. The two implementations are kept in sync by
//     code review (no compile-time linkage between them).

#pragma once

#include <cstddef>
#include <string>
#include <vector>

namespace hodos {

// (cert_type, field_name) pair we always treat as sensitive.
// `certType` empty string means "any cert type"; non-empty matches the exact
// cert_type identifier (typically a base64-encoded 32-byte certifier-issued
// type ID, see BRC-52). Field name is compared after NormalizeFieldName().
struct SensitiveCertFieldPair {
    const char* certType;
    const char* fieldName;
};

// Normalize a field name for comparison:
//   - camelCase boundaries (lower/digit → upper) split with '_'
//   - lowercase ASCII A-Z → a-z
//   - non-alphanumeric runs collapse to a single '_'
//   - leading/trailing '_' stripped
//
// Examples: "SSN" → "ssn", "Date_Of_Birth" → "date_of_birth",
//           "drivers-license" → "drivers_license",
//           "passport.number" → "passport_number",
//           "legalName" → "legal_name", "governmentId" → "government_id".
inline std::string NormalizeFieldName(const std::string& in) {
    // Pass 1: insert '_' at camelCase boundaries (lowercase|digit → uppercase).
    // Acronym-internal transitions (upper → upper) stay together.
    std::string split;
    split.reserve(in.size() + in.size() / 4);
    for (std::size_t i = 0; i < in.size(); ++i) {
        char c = in[i];
        if (i > 0 && c >= 'A' && c <= 'Z') {
            char prev = in[i - 1];
            const bool prev_is_lower_or_digit =
                (prev >= 'a' && prev <= 'z') || (prev >= '0' && prev <= '9');
            if (prev_is_lower_or_digit) {
                split.push_back('_');
            }
        }
        split.push_back(c);
    }

    // Pass 2: lowercase + map non-alphanumeric → '_'.
    std::string flat;
    flat.reserve(split.size());
    for (char c : split) {
        if (c >= 'A' && c <= 'Z') {
            flat.push_back(static_cast<char>(c + ('a' - 'A')));
        } else if ((c >= 'a' && c <= 'z') || (c >= '0' && c <= '9')) {
            flat.push_back(c);
        } else {
            flat.push_back('_');
        }
    }

    // Pass 3: collapse runs of '_' and trim leading/trailing '_'.
    std::string out;
    out.reserve(flat.size());
    bool last_underscore = true;  // trim leading underscores
    for (char c : flat) {
        if (c == '_') {
            if (!last_underscore) {
                out.push_back('_');
                last_underscore = true;
            }
        } else {
            out.push_back(c);
            last_underscore = false;
        }
    }
    if (!out.empty() && out.back() == '_') out.pop_back();
    return out;
}

// Hardcoded sensitive (cert_type, field_name) pairs. The list is empty at
// C.1 — the heuristic in FieldNameMatchesSensitiveHeuristic() covers the
// common cases. Add entries here when a specific cert schema is identified
// where a benign-looking field name maps to a sensitive value (e.g. a cert
// type whose "uid" field is actually a Social Security Number).
//
// Returned by value as a static const reference for stable iterator stability.
inline const std::vector<SensitiveCertFieldPair>& KnownSensitiveCertFieldPairs() {
    static const std::vector<SensitiveCertFieldPair> kPairs = {
        // Reserved for cert-schema-specific overrides. None today.
    };
    return kPairs;
}

// Heuristic match against the normalized field name. Fires for ANY cert type.
// Patterns are split into:
//   - Exact: terms that are sensitive only when they are the WHOLE field name
//     (avoids "email_address" false-positiving on "address").
//   - Substring: multi-character tokens with low false-positive risk.
//   - Suffix: "*_address" tail (excluding email-related fields).
inline bool FieldNameMatchesSensitiveHeuristic(const std::string& fieldName) {
    const std::string n = NormalizeFieldName(fieldName);
    if (n.empty()) return false;

    // Exact-match terms — ambiguous as substrings.
    static const std::vector<std::string> kExact = {
        "address",          // bare "address" = postal/street; "email_address" handled below
        "dob",
        "ssn",
        "tin",              // Taxpayer Identification Number
        "nin",              // National Insurance Number
        "ein",              // Employer Identification Number
    };
    for (const auto& term : kExact) {
        if (n == term) return true;
    }

    // Substring patterns — multi-token, low false-positive risk.
    // Kept alphabetized within category for review-friendliness.
    static const std::vector<std::string> kSubstrings = {
        // Government / national IDs
        "drivers_license",
        "driver_license",
        "drivers_licence",
        "driver_licence",
        "government_id",
        "govt_id",
        "id_number",
        "license_number",
        "licence_number",
        "national_id",
        "national_insurance",
        "passport",
        "social_security",
        "state_id",
        "tax_id",
        "tax_number",

        // Date of birth
        "birth_date",
        "birthdate",
        "birthday",
        "date_of_birth",

        // Full legal name (display first/last names are intentionally NOT sensitive)
        "full_legal_name",
        "legal_name",
        "maiden_name",

        // Physical / postal address
        "home_address",
        "mailing_address",
        "physical_address",
        "postal_address",
        "residential_address",
        "street_address",

        // Biometric data
        "biometric",
        "face_print",
        "fingerprint",
        "iris_scan",
        "retina",
        "voice_print",

        // Financial account identifiers
        "account_number",
        "bank_account",
        "card_number",
        "credit_card",
        "iban",
        "routing_number",
        "swift_code",
    };
    for (const auto& pat : kSubstrings) {
        if (n.find(pat) != std::string::npos) return true;
    }

    // Suffix: any "*_address" not containing "email" — catches billing_address,
    // shipping_address, work_address, etc. without enumerating each.
    static constexpr const char kAddressSuffix[] = "_address";
    constexpr size_t kAddressSuffixLen = sizeof(kAddressSuffix) - 1;
    if (n.size() >= kAddressSuffixLen
        && n.compare(n.size() - kAddressSuffixLen, kAddressSuffixLen, kAddressSuffix) == 0
        && n.find("email") == std::string::npos) {
        return true;
    }

    return false;
}

// Single decision: is the (certType, fieldName) tuple sensitive?
//   1. If KnownSensitiveCertFieldPairs() contains an exact match
//      (case-insensitive comparison via NormalizeFieldName on the field name,
//      exact-string comparison on the cert type), return true.
//   2. Fall through to FieldNameMatchesSensitiveHeuristic.
//
// An empty certType is acceptable — the heuristic still fires. This matches
// the C++ classifyCallKind situation where /proveCertificate may have an
// empty cert type in malformed/partial bodies.
inline bool IsSensitiveCertField(const std::string& certType, const std::string& fieldName) {
    const std::string normField = NormalizeFieldName(fieldName);
    if (!normField.empty()) {
        for (const auto& pair : KnownSensitiveCertFieldPairs()) {
            if (!pair.fieldName) continue;
            if (NormalizeFieldName(pair.fieldName) != normField) continue;
            const std::string pairCertType = pair.certType ? std::string(pair.certType) : std::string();
            if (pairCertType.empty() || pairCertType == certType) {
                return true;
            }
        }
    }
    return FieldNameMatchesSensitiveHeuristic(fieldName);
}

// Aggregate decision for a /proveCertificate request: returns true if ANY
// requested field is sensitive. Routes the whole request to
// PermissionCallKind::SensitiveCertField (always-prompt) when true.
inline bool AnyRequestedCertFieldSensitive(
    const std::string& certType,
    const std::vector<std::string>& requestedFields
) {
    for (const auto& field : requestedFields) {
        if (IsSensitiveCertField(certType, field)) return true;
    }
    return false;
}

}  // namespace hodos
