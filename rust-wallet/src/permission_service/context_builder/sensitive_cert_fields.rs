//! Sensitive certificate field classifier — Rust mirror of
//! `cef-native/include/core/SensitiveCertFields.h`.
//!
//! Phase 2.6-C.1 (Sigma-BRC121-Sprint). Pure logic — no async, no DB, no
//! actix. Identifies (cert_type, field_name) tuples that should ALWAYS prompt
//! the user, regardless of any pre-approval row in `cert_field_permissions`.
//!
//! Parity with C++:
//!   - `KNOWN_SENSITIVE_PAIRS` mirrors the C++ `KnownSensitiveCertFieldPairs()`
//!   - `field_name_matches_sensitive_heuristic` mirrors the C++ same-named function
//!   - `is_sensitive_cert_field` mirrors the C++ `IsSensitiveCertField`
//!   - `any_requested_cert_field_sensitive` mirrors the C++ `AnyRequestedCertFieldSensitive`
//!
//! The two implementations are kept in sync by code review. There is no
//! compile-time linkage between them; the lists are duplicated by intent so
//! each binary is self-contained.
//!
//! Per Phase 2.6-C kickoff Q2, the classifier errs toward over-classification:
//! an extra prompt on a benign field is annoying-but-safe; a silent disclosure
//! of a sensitive field is a data leak and unacceptable.

/// (cert_type, field_name) pair always treated as sensitive. `cert_type`
/// empty means "any cert type"; non-empty matches the exact cert_type
/// identifier (typically a base64-encoded 32-byte BRC-52 type ID).
#[derive(Debug, Clone, Copy)]
pub struct SensitiveCertFieldPair {
    pub cert_type: &'static str,
    pub field_name: &'static str,
}

/// Hardcoded sensitive (cert_type, field_name) pairs. Empty at C.1 — the
/// heuristic covers the common cases. Add entries when a specific cert schema
/// has a benign-looking field name that maps to a sensitive value.
pub const KNOWN_SENSITIVE_PAIRS: &[SensitiveCertFieldPair] = &[
    // Reserved for cert-schema-specific overrides. None today.
];

/// Bare field-name tokens that are sensitive only when they ARE the whole
/// normalized field name. Avoids "email_address" false-positiving on "address".
pub const EXACT_SENSITIVE_TERMS: &[&str] = &[
    "address", // bare = postal/street
    "dob",
    "ssn",
    "tin", // Taxpayer Identification Number
    "nin", // National Insurance Number
    "ein", // Employer Identification Number
];

/// Multi-token substring patterns with low false-positive risk. Kept
/// alphabetized within category for review parity with the C++ list.
pub const SUBSTRING_SENSITIVE_PATTERNS: &[&str] = &[
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
    // Full legal name (display first/last names intentionally NOT sensitive)
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
];

/// Normalize a field name for comparison:
///   - camelCase boundaries (lower/digit → upper) split with '_'
///   - lowercase ASCII A-Z → a-z
///   - non-alphanumeric runs collapse to a single '_'
///   - leading/trailing '_' stripped
///
/// Examples: "SSN" → "ssn", "Date_Of_Birth" → "date_of_birth",
///           "drivers-license" → "drivers_license",
///           "passport.number" → "passport_number",
///           "legalName" → "legal_name", "governmentId" → "government_id".
pub fn normalize_field_name(input: &str) -> String {
    // Pass 1: insert '_' at camelCase boundaries (lowercase|digit → uppercase).
    // Acronym-internal transitions (upper → upper) stay together.
    let bytes = input.as_bytes();
    let mut split: Vec<u8> = Vec::with_capacity(bytes.len() + bytes.len() / 4);
    for (i, &b) in bytes.iter().enumerate() {
        if i > 0 && (b'A'..=b'Z').contains(&b) {
            let prev = bytes[i - 1];
            let prev_is_lower_or_digit =
                (b'a'..=b'z').contains(&prev) || (b'0'..=b'9').contains(&prev);
            if prev_is_lower_or_digit {
                split.push(b'_');
            }
        }
        split.push(b);
    }

    // Pass 2: lowercase + map non-alphanumeric → '_'.
    let mut flat: Vec<u8> = Vec::with_capacity(split.len());
    for &b in &split {
        if (b'A'..=b'Z').contains(&b) {
            flat.push(b + (b'a' - b'A'));
        } else if (b'a'..=b'z').contains(&b) || (b'0'..=b'9').contains(&b) {
            flat.push(b);
        } else {
            flat.push(b'_');
        }
    }

    // Pass 3: collapse runs of '_' and trim leading/trailing '_'.
    let mut out: Vec<u8> = Vec::with_capacity(flat.len());
    let mut last_underscore = true; // trim leading underscores
    for &b in &flat {
        if b == b'_' {
            if !last_underscore {
                out.push(b'_');
                last_underscore = true;
            }
        } else {
            out.push(b);
            last_underscore = false;
        }
    }
    if let Some(&b'_') = out.last() {
        out.pop();
    }
    // All bytes in `out` are ASCII alphanumerics or '_', so this is valid UTF-8.
    String::from_utf8(out).expect("normalized field name is ASCII")
}

/// Heuristic match against the normalized field name. Fires for ANY cert type.
pub fn field_name_matches_sensitive_heuristic(field_name: &str) -> bool {
    let n = normalize_field_name(field_name);
    if n.is_empty() {
        return false;
    }

    if EXACT_SENSITIVE_TERMS.iter().any(|term| *term == n) {
        return true;
    }

    if SUBSTRING_SENSITIVE_PATTERNS
        .iter()
        .any(|pat| n.contains(pat))
    {
        return true;
    }

    // Suffix: "*_address" not containing "email" — catches billing_address,
    // shipping_address, work_address, etc. without enumerating each.
    const ADDRESS_SUFFIX: &str = "_address";
    if n.ends_with(ADDRESS_SUFFIX) && !n.contains("email") {
        return true;
    }

    false
}

/// Single decision: is the (cert_type, field_name) tuple sensitive?
///   1. If `KNOWN_SENSITIVE_PAIRS` contains an exact match
///      (case-insensitive comparison via `normalize_field_name` on the field
///      name; exact-string comparison on the cert type), return true.
///   2. Fall through to `field_name_matches_sensitive_heuristic`.
///
/// An empty `cert_type` is acceptable — the heuristic still fires.
pub fn is_sensitive_cert_field(cert_type: &str, field_name: &str) -> bool {
    let norm_field = normalize_field_name(field_name);
    if !norm_field.is_empty() {
        for pair in KNOWN_SENSITIVE_PAIRS {
            if normalize_field_name(pair.field_name) != norm_field {
                continue;
            }
            if pair.cert_type.is_empty() || pair.cert_type == cert_type {
                return true;
            }
        }
    }
    field_name_matches_sensitive_heuristic(field_name)
}

/// Aggregate decision for a /proveCertificate request: returns true if ANY
/// requested field is sensitive. Routes the whole request to
/// `CallKind::SensitiveCertField` (always-prompt) when true.
pub fn any_requested_cert_field_sensitive(
    cert_type: &str,
    requested_fields: &[String],
) -> bool {
    requested_fields
        .iter()
        .any(|f| is_sensitive_cert_field(cert_type, f))
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---------- Normalization ----------

    #[test]
    fn normalize_lowercases_and_collapses_separators() {
        assert_eq!(normalize_field_name("SSN"), "ssn");
        assert_eq!(normalize_field_name("Date_Of_Birth"), "date_of_birth");
        assert_eq!(normalize_field_name("drivers-license"), "drivers_license");
        assert_eq!(normalize_field_name("passport.number"), "passport_number");
        assert_eq!(normalize_field_name("__leading"), "leading");
        assert_eq!(normalize_field_name("trailing___"), "trailing");
        assert_eq!(normalize_field_name("multi   space"), "multi_space");
        assert_eq!(normalize_field_name(""), "");
    }

    #[test]
    fn normalize_splits_camel_case_boundaries() {
        assert_eq!(normalize_field_name("legalName"), "legal_name");
        assert_eq!(normalize_field_name("governmentId"), "government_id");
        assert_eq!(normalize_field_name("passportNumber"), "passport_number");
        assert_eq!(normalize_field_name("dateOfBirth"), "date_of_birth");
        // Acronym-internal: SSN, IBAN, DOB stay together (no split between uppers)
        assert_eq!(normalize_field_name("mySSN"), "my_ssn");
        assert_eq!(normalize_field_name("myIBANCode"), "my_ibancode");
        // Digit→upper boundary also splits
        assert_eq!(normalize_field_name("abc123Def"), "abc123_def");
    }

    // ---------- Positive: known sensitive fields ----------

    #[test]
    fn ssn_is_sensitive() {
        assert!(is_sensitive_cert_field("", "SSN"));
        assert!(is_sensitive_cert_field("any-cert", "ssn"));
        assert!(is_sensitive_cert_field("", "Social_Security_Number"));
    }

    #[test]
    fn passport_fields_are_sensitive() {
        assert!(is_sensitive_cert_field("", "passport"));
        assert!(is_sensitive_cert_field("", "passportNumber"));
        assert!(is_sensitive_cert_field("", "passport_id"));
    }

    #[test]
    fn government_id_fields_are_sensitive() {
        assert!(is_sensitive_cert_field("", "drivers_license"));
        assert!(is_sensitive_cert_field("", "drivers-license"));
        assert!(is_sensitive_cert_field("", "driver_licence"));
        assert!(is_sensitive_cert_field("", "national_id"));
        assert!(is_sensitive_cert_field("", "governmentId"));
        assert!(is_sensitive_cert_field("", "state_id"));
        assert!(is_sensitive_cert_field("", "tax_id"));
        assert!(is_sensitive_cert_field("", "license_number"));
    }

    #[test]
    fn dob_fields_are_sensitive() {
        assert!(is_sensitive_cert_field("", "dob"));
        assert!(is_sensitive_cert_field("", "DOB"));
        assert!(is_sensitive_cert_field("", "date_of_birth"));
        assert!(is_sensitive_cert_field("", "birthDate"));
        assert!(is_sensitive_cert_field("", "birthday"));
    }

    #[test]
    fn full_legal_name_is_sensitive() {
        assert!(is_sensitive_cert_field("", "legal_name"));
        assert!(is_sensitive_cert_field("", "legalName"));
        assert!(is_sensitive_cert_field("", "full_legal_name"));
        assert!(is_sensitive_cert_field("", "maiden_name"));
    }

    #[test]
    fn residential_address_is_sensitive() {
        assert!(is_sensitive_cert_field("", "address"));
        assert!(is_sensitive_cert_field("", "home_address"));
        assert!(is_sensitive_cert_field("", "residential_address"));
        assert!(is_sensitive_cert_field("", "street_address"));
        // Suffix rule: any *_address without "email"
        assert!(is_sensitive_cert_field("", "billing_address"));
        assert!(is_sensitive_cert_field("", "shipping_address"));
        assert!(is_sensitive_cert_field("", "work_address"));
    }

    #[test]
    fn biometric_fields_are_sensitive() {
        assert!(is_sensitive_cert_field("", "biometric"));
        assert!(is_sensitive_cert_field("", "fingerprint"));
        assert!(is_sensitive_cert_field("", "face_print"));
        assert!(is_sensitive_cert_field("", "voice_print"));
        assert!(is_sensitive_cert_field("", "retina"));
        assert!(is_sensitive_cert_field("", "iris_scan"));
    }

    #[test]
    fn financial_account_fields_are_sensitive() {
        assert!(is_sensitive_cert_field("", "bank_account"));
        assert!(is_sensitive_cert_field("", "account_number"));
        assert!(is_sensitive_cert_field("", "routing_number"));
        assert!(is_sensitive_cert_field("", "credit_card"));
        assert!(is_sensitive_cert_field("", "card_number"));
        assert!(is_sensitive_cert_field("", "iban"));
        assert!(is_sensitive_cert_field("", "swift_code"));
    }

    // ---------- Negative: benign profile fields ----------

    #[test]
    fn display_name_is_not_sensitive() {
        assert!(!is_sensitive_cert_field("", "displayName"));
        assert!(!is_sensitive_cert_field("", "display_name"));
    }

    #[test]
    fn avatar_url_is_not_sensitive() {
        assert!(!is_sensitive_cert_field("", "avatarURL"));
        assert!(!is_sensitive_cert_field("", "avatar_url"));
        assert!(!is_sensitive_cert_field("", "avatar"));
    }

    #[test]
    fn email_address_is_not_sensitive() {
        // "email" itself isn't on the sensitive list AND the "_address" suffix
        // rule explicitly excludes email-containing fields.
        assert!(!is_sensitive_cert_field("", "email"));
        assert!(!is_sensitive_cert_field("", "email_address"));
        assert!(!is_sensitive_cert_field("", "emailAddress"));
        assert!(!is_sensitive_cert_field("", "work_email_address"));
    }

    #[test]
    fn first_last_full_name_are_not_sensitive() {
        // Display first/last names are common profile fields. We mark only
        // "legal_name", "full_legal_name", and "maiden_name" as sensitive.
        assert!(!is_sensitive_cert_field("", "firstName"));
        assert!(!is_sensitive_cert_field("", "lastName"));
        assert!(!is_sensitive_cert_field("", "given_name"));
        assert!(!is_sensitive_cert_field("", "family_name"));
        assert!(!is_sensitive_cert_field("", "full_name"));
        assert!(!is_sensitive_cert_field("", "surname"));
    }

    #[test]
    fn handle_and_username_are_not_sensitive() {
        assert!(!is_sensitive_cert_field("", "handle"));
        assert!(!is_sensitive_cert_field("", "username"));
        assert!(!is_sensitive_cert_field("", "screenName"));
    }

    #[test]
    fn empty_field_name_is_not_sensitive() {
        assert!(!is_sensitive_cert_field("", ""));
        assert!(!is_sensitive_cert_field("known-cert", ""));
    }

    // ---------- Heuristic: unknown cert type ----------

    #[test]
    fn heuristic_fires_for_unknown_cert_type() {
        // Passport pattern on a cert type we've never seen still trips the
        // heuristic — the over-classification principle.
        assert!(is_sensitive_cert_field("brand-new-cert-type-abc==", "passport_number"));
        assert!(is_sensitive_cert_field("brand-new-cert-type-abc==", "SSN"));
        assert!(is_sensitive_cert_field("brand-new-cert-type-abc==", "date_of_birth"));
    }

    #[test]
    fn heuristic_does_not_fire_for_benign_field_on_unknown_cert() {
        assert!(!is_sensitive_cert_field("brand-new-cert-type-abc==", "preferredName"));
        assert!(!is_sensitive_cert_field("brand-new-cert-type-abc==", "displayName"));
    }

    // ---------- Mixed-sensitivity request ----------

    #[test]
    fn mixed_request_with_any_sensitive_field_is_sensitive() {
        let fields: Vec<String> = ["displayName", "avatarURL", "SSN"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert!(any_requested_cert_field_sensitive("", &fields));
    }

    #[test]
    fn all_benign_request_is_not_sensitive() {
        let fields: Vec<String> = ["displayName", "avatarURL", "email", "handle"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert!(!any_requested_cert_field_sensitive("", &fields));
    }

    #[test]
    fn empty_request_is_not_sensitive() {
        let fields: Vec<String> = vec![];
        assert!(!any_requested_cert_field_sensitive("", &fields));
    }

    #[test]
    fn single_sensitive_field_routes_whole_request() {
        let fields: Vec<String> = vec!["dob".to_string()];
        assert!(any_requested_cert_field_sensitive("", &fields));
    }
}
