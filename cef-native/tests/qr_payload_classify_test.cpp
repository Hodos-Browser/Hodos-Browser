// Phase 0.6 — unit tests for the screen-capture QR classifier (site #1).
//
// Exercises the REAL compiled classifier (include/core/QRPayloadClassify.h), the
// same code QRScreenCapture.cpp runs after quirc decodes a QR. Deterministic and
// negative-controllable — the substitute for the physical drag-capture path, which
// cannot be driven by injected input in a headless/agent environment.
//
// Rows covered: A1 (bsv into a bip21 result, full address), A3 (bitcoin: kept),
// A4 (foreign rejected + scheme named), A5 (amount units preserved verbatim),
// A6 (strip by first ':', no truncation), plus the amount-injection fix
// (MEASUREMENT_amount_injection.md) and the non-numeric-amount robustness fix.

#include <gtest/gtest.h>
#include "core/QRPayloadClassify.h"

#include <string>

using hodos::ClassifyQRPayload;
using hodos::SchemeForMessage;

namespace {

// Tiny substring helper so the assertions read as intent.
bool Contains(const std::string& hay, const std::string& needle) {
    return hay.find(needle) != std::string::npos;
}

const char* kBsvAddr = "16cezrim1PR2DGuFivZr8kWUSan1LD6XFZ";

} // namespace

// ---- A1 / A6: bsv: accepted, FULL address, valid amount kept ----------------
TEST(QrClassify, BsvSchemeAcceptedFullAddressAndAmount) {
    std::string json = ClassifyQRPayload(
        "bsv:16cezrim1PR2DGuFivZr8kWUSan1LD6XFZ?amount=0.11828417&label=PaiyBit%20media");
    ASSERT_FALSE(json.empty());
    EXPECT_TRUE(Contains(json, "\"type\":\"bip21\""));
    // A6: strip by first ':', not slice(8) — the address field must be whole, not the
    // slice(8) truncation "zrim1PR2…". (Note: the full address itself contains the
    // substring "zrim1PR2", so assert on the address FIELD's leading chars.)
    EXPECT_TRUE(Contains(json, std::string("\"address\":\"") + kBsvAddr + "\""));
    EXPECT_FALSE(Contains(json, "\"address\":\"zrim"));
    // A5: amount units preserved verbatim (whole-coin BSV), unquoted number.
    EXPECT_TRUE(Contains(json, "\"amount\":0.11828417"));
    // label URL-decoded + escaped
    EXPECT_TRUE(Contains(json, "\"label\":\"PaiyBit media\""));
}

// ---- A3: bitcoin: still works (rule widened, not swapped) --------------------
TEST(QrClassify, BitcoinSchemeStillWorks) {
    std::string json = ClassifyQRPayload(
        "bitcoin:1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa?amount=0.001&label=Test");
    ASSERT_FALSE(json.empty());
    EXPECT_TRUE(Contains(json, "\"address\":\"1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa\""));
    EXPECT_TRUE(Contains(json, "\"amount\":0.001"));
}

// ---- A4: foreign scheme rejected, scheme name available for the message ------
TEST(QrClassify, ForeignSchemeRejectedButNamed) {
    EXPECT_EQ(ClassifyQRPayload("https://example.com/pay"), "");
    EXPECT_EQ(SchemeForMessage("https://example.com/pay"), "https");
    EXPECT_EQ(SchemeForMessage("mailto:a@b.com"), "mailto");
    // Sanitized + capped so a hostile payload can't bloat the user-facing string.
    EXPECT_LE(SchemeForMessage(std::string(200, 'x')).size(), 12u);
}

// ---- Scheme allowlist stays an allowlist ------------------------------------
TEST(QrClassify, SchemeAllowlistNotAnyScheme) {
    EXPECT_EQ(ClassifyQRPayload("web+bsv:16cezrim1PR2DGuFivZr8kWUSan1LD6XFZ"), "");
    EXPECT_EQ(ClassifyQRPayload("bsvx:16cezrim1PR2DGuFivZr8kWUSan1LD6XFZ"), "");
}

// ---- INJECTION FIX: a non-numeric amount is DROPPED, never emitted raw -------
// Pre-fix this produced ...,"amount":(window.__hodos_qr_probe=typeof fetch,0),...
// which executed as JS in the wallet overlay. See MEASUREMENT_amount_injection.md.
TEST(QrClassify, InjectionAmountIsDroppedNotEmitted) {
    std::string json = ClassifyQRPayload(
        "bitcoin:16cezrim1PR2DGuFivZr8kWUSan1LD6XFZ?amount=(window.__hodos_qr_probe=typeof fetch,0)");
    ASSERT_FALSE(json.empty());                        // still a valid bip21 result...
    EXPECT_TRUE(Contains(json, std::string("\"address\":\"") + kBsvAddr + "\""));
    // ...but the injectable amount FIELD is gone entirely. The raw expression may
    // still appear inside the QUOTED, ESCAPED "value" echo — that is inert (a JS
    // string literal). The vulnerability was the UNQUOTED "amount":(expr) — assert
    // that exact shape can never occur.
    EXPECT_FALSE(Contains(json, "\"amount\":"));
    EXPECT_FALSE(Contains(json, "\"amount\":(window"));
    // The delivered JSON must be valid JSON (it was NOT pre-fix, which is what let
    // it be eval'd as code). Structural end check.
    EXPECT_TRUE(Contains(json, "\"source\":\"screen\"}"));
}

// ---- Robustness: a non-numeric amount is dropped (no silent JS error) --------
TEST(QrClassify, NonNumericAmountDropped) {
    std::string json = ClassifyQRPayload(
        "bitcoin:16cezrim1PR2DGuFivZr8kWUSan1LD6XFZ?amount=abc");
    ASSERT_FALSE(json.empty());
    EXPECT_FALSE(Contains(json, "\"amount\":"));   // the amount FIELD is dropped
}

// ---- Amount edge cases: only plain decimals pass ----------------------------
TEST(QrClassify, AmountValidationBoundaries) {
    auto amt = [](const char* a) {
        return ClassifyQRPayload(std::string("bsv:16cezrim1PR2DGuFivZr8kWUSan1LD6XFZ?amount=") + a);
    };
    // Assert on the amount FIELD ("amount":) — the URI echo in "value" also contains
    // the word "amount", so a bare "amount" needle would always match.
    EXPECT_TRUE(Contains(amt("0"), "\"amount\":0"));
    EXPECT_TRUE(Contains(amt("12.5"), "\"amount\":12.5"));
    EXPECT_FALSE(Contains(amt("1e8"), "\"amount\":"));   // scientific notation rejected
    EXPECT_FALSE(Contains(amt("0x10"), "\"amount\":"));  // hex rejected
    EXPECT_FALSE(Contains(amt("-1"), "\"amount\":"));    // negative rejected
    EXPECT_FALSE(Contains(amt(".5"), "\"amount\":"));    // leading-dot rejected (needs a digit)
    EXPECT_FALSE(Contains(amt("1."), "\"amount\":"));    // trailing-dot rejected
}

// ---- label can't break out of its JSON string (escaped) ---------------------
TEST(QrClassify, LabelIsEscaped) {
    std::string json = ClassifyQRPayload(
        "bsv:16cezrim1PR2DGuFivZr8kWUSan1LD6XFZ?label=%22%2C%22x%22%3A1");  // ","x":1
    ASSERT_FALSE(json.empty());
    // The quote must be backslash-escaped, not a raw string break.
    EXPECT_TRUE(Contains(json, "\\\""));
}

// ---- other BSV recipient shapes still classify ------------------------------
TEST(QrClassify, PlainAddressIdentityKeyPaymail) {
    EXPECT_TRUE(Contains(ClassifyQRPayload(kBsvAddr), "\"type\":\"address\""));
    // 02 + 64 hex chars = a compressed pubkey shape (RE_IDENTITY_KEY).
    EXPECT_TRUE(Contains(
        ClassifyQRPayload("020000000000000000000000000000000000000000000000000000000000000000"),
        "\"type\":\"identity_key\""));
    EXPECT_TRUE(Contains(ClassifyQRPayload("user@example.com"), "\"type\":\"paymail\""));
}
