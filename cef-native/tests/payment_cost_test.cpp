// Tests for include/core/PaymentCost.h — P0.5 finding 6.
//
// The property these exist to defend: a fund-moving request whose amount we
// CANNOT derive must report priceAvailable=false, so the Rust engine prompts
// instead of treating it as a 0-cent payment that passes every cap. The engine's
// own test (matrix_c.rs :: bsv_price_available_with_zero_cents_is_silent) asserts
// that price-available + 0 cents == Silent, so getting this wrong here silently
// auto-approves a full-balance sweep.
//
// NEGATIVE CONTROL: SendMaxIsNotDerivable and SendMaxIsNeverPriceAvailable both
// go red if someone "simplifies" the sendMax arm out of ExtractOutputSatoshis —
// which is the single most tempting edit in that function, because the body does
// carry an `amount` field that looks usable.

#include <gtest/gtest.h>
#include "../include/core/PaymentCost.h"

namespace {

constexpr double kPrice = 15.0;  // USD/BSV, roughly the live price during P0.5

// ---------------------------------------------------------------------------
// Endpoint classification
// ---------------------------------------------------------------------------

TEST(IsPaymentEndpoint, ClassicBrc100EndpointsMatch) {
    EXPECT_TRUE(hodos::IsPaymentEndpoint("/createAction"));
    EXPECT_TRUE(hodos::IsPaymentEndpoint("/acquireCertificate"));
    EXPECT_TRUE(hodos::IsPaymentEndpoint("/sendMessage"));
}

TEST(IsPaymentEndpoint, DirectFundMoversMatch) {
    // Added by P0.5 finding 6. Before this they arrived at Rust header-less and
    // rendered "0 sats" under a false price-outage cause.
    EXPECT_TRUE(hodos::IsPaymentEndpoint("/transaction/send"));
    EXPECT_TRUE(hodos::IsPaymentEndpoint("/wallet/peerpay/send"));
    EXPECT_TRUE(hodos::IsPaymentEndpoint("/wallet/paymail/send"));
}

TEST(IsPaymentEndpoint, NonPaymentEndpointsDoNotMatch) {
    EXPECT_FALSE(hodos::IsPaymentEndpoint("/getVersion"));
    EXPECT_FALSE(hodos::IsPaymentEndpoint("/wallet/status"));
    EXPECT_FALSE(hodos::IsPaymentEndpoint("/getPublicKey"));
    EXPECT_FALSE(hodos::IsPaymentEndpoint("/wallet/peerpay/inbox"));
    EXPECT_FALSE(hodos::IsPaymentEndpoint(""));
}

// ---------------------------------------------------------------------------
// The four body shapes
// ---------------------------------------------------------------------------

TEST(ExtractOutputSatoshis, CreateActionSumsOutputs) {
    EXPECT_EQ(hodos::ExtractOutputSatoshis(
                  R"({"outputs":[{"satoshis":1000},{"satoshis":2500}]})"),
              3500);
}

TEST(ExtractOutputSatoshis, TransactionSendReadsAmount) {
    EXPECT_EQ(hodos::ExtractOutputSatoshis(
                  R"({"toAddress":"1abc","amount":50000,"sendMax":false})"),
              50000);
}

TEST(ExtractOutputSatoshis, PeerPayReadsAmountSatoshis) {
    EXPECT_EQ(hodos::ExtractOutputSatoshis(
                  R"({"recipient_identity_key":"02ab","amount_satoshis":7777})"),
              7777);
}

TEST(ExtractOutputSatoshis, PaymailReadsAmountSatoshis) {
    EXPECT_EQ(hodos::ExtractOutputSatoshis(
                  R"({"paymail":"a@b.com","amount_satoshis":1234})"),
              1234);
}

// ⛔ THE LOAD-BEARING ONE. The body carries `amount`, and trusting it would
// under-price a sweep by an unbounded margin.
TEST(ExtractOutputSatoshis, SendMaxIsNotDerivable) {
    EXPECT_EQ(hodos::ExtractOutputSatoshis(
                  R"({"toAddress":"1abc","amount":1,"sendMax":true})"),
              hodos::kAmountNotDerivable);
}

TEST(ExtractOutputSatoshis, MalformedAndEmptyBodiesAreZeroNotCrash) {
    EXPECT_EQ(hodos::ExtractOutputSatoshis(""), 0);
    EXPECT_EQ(hodos::ExtractOutputSatoshis("not json at all"), 0);
    EXPECT_EQ(hodos::ExtractOutputSatoshis("{}"), 0);
    EXPECT_EQ(hodos::ExtractOutputSatoshis(R"({"amount":"a string"})"), 0);
    EXPECT_EQ(hodos::ExtractOutputSatoshis(R"({"outputs":"not an array"})"), 0);
}

// ---------------------------------------------------------------------------
// Pricing — where the fail-closed rule actually lives
// ---------------------------------------------------------------------------

TEST(ComputePaymentCost, NonPaymentEndpointIsNotPriced) {
    const auto c = hodos::ComputePaymentCost("/getVersion", R"({"amount":5000})", kPrice);
    EXPECT_FALSE(c.isPayment);
    EXPECT_EQ(c.satoshis, 0);
    EXPECT_EQ(c.cents, 0);
    EXPECT_FALSE(c.priceAvailable);
}

TEST(ComputePaymentCost, KnownAmountIsPricedAndPriceAvailable) {
    // 1 BSV at $15 == 1500 cents.
    const auto c = hodos::ComputePaymentCost(
        "/transaction/send", R"({"toAddress":"1abc","amount":100000000})", kPrice);
    EXPECT_TRUE(c.isPayment);
    EXPECT_EQ(c.satoshis, 100000000);
    EXPECT_EQ(c.cents, 1500);
    EXPECT_TRUE(c.priceAvailable);
}

// ⛔ THE SAFETY PROPERTY. priceAvailable MUST be false here even though the price
// cache is perfectly healthy — otherwise the engine sees a 0-cent payment, finds
// it under every cap, and silently approves a full-balance sweep.
TEST(ComputePaymentCost, SendMaxIsNeverPriceAvailable) {
    const auto c = hodos::ComputePaymentCost(
        "/transaction/send", R"({"toAddress":"1abc","amount":1,"sendMax":true})", kPrice);
    EXPECT_TRUE(c.isPayment);       // it IS a payment — it must still be gated
    EXPECT_FALSE(c.priceAvailable); // but we could not price it
    EXPECT_EQ(c.cents, 0);
    EXPECT_EQ(c.satoshis, 0);
}

TEST(ComputePaymentCost, GenuinePriceOutageIsNotPriceAvailable) {
    // The real price_unavailable case: amount known, cache has nothing.
    const auto c = hodos::ComputePaymentCost(
        "/transaction/send", R"({"toAddress":"1abc","amount":100000000})", 0.0);
    EXPECT_TRUE(c.isPayment);
    EXPECT_FALSE(c.priceAvailable);
    EXPECT_EQ(c.cents, 0);
    EXPECT_EQ(c.satoshis, 100000000);  // amount still reported, just unpriced
}

TEST(ComputePaymentCost, SubCentPaymentKeepsPriceAvailable) {
    // A tiny payment rounds to 0 cents. That must NOT be confused with the
    // unpriceable case — it is a real, known, tiny amount, and the gold pill
    // renders it as "< $0.01".
    const auto c = hodos::ComputePaymentCost(
        "/wallet/peerpay/send", R"({"recipient_identity_key":"02ab","amount_satoshis":10})",
        kPrice);
    EXPECT_TRUE(c.isPayment);
    EXPECT_TRUE(c.priceAvailable);
    EXPECT_EQ(c.cents, 0);
    EXPECT_EQ(c.satoshis, 10);
}

// ---------------------------------------------------------------------------
// P0.5 adversarial panel #2 — the two ways a real spend was priced at ZERO.
//
// Both mattered because 0 cents WITH a live price is not "unpriced": matrix_c
// reads it as under every cap and auto-approves SILENTLY, no modal. Each test
// below fails against the pre-fix first-match cascade.
// ---------------------------------------------------------------------------

// 1.1 — the decoy. An empty `outputs` array used to match the createAction
// branch and return 0, while Rust ignored `outputs` and spent `amount`.
TEST(ExtractOutputSatoshis, DecoyEmptyOutputsDoesNotMaskTheRealAmount) {
    EXPECT_EQ(hodos::ExtractOutputSatoshis(
                  R"({"outputs":[],"toAddress":"1abc","amount":100000000})"),
              100000000);
}

TEST(ExtractOutputSatoshis, DecoyEmptyOutputsDoesNotMaskAmountSatoshis) {
    EXPECT_EQ(hodos::ExtractOutputSatoshis(
                  R"({"outputs":[],"recipient_identity_key":"02ab","amount_satoshis":4200})"),
              4200);
}

// An empty outputs array on its own is still 0 — it genuinely carries no amount.
TEST(ExtractOutputSatoshis, EmptyOutputsAloneIsZero) {
    EXPECT_EQ(hodos::ExtractOutputSatoshis(R"({"outputs":[]})"), 0);
}

// Two POPULATED shapes are ambiguous: we cannot know which one the handler
// honours, so refuse to price rather than guess.
TEST(ExtractOutputSatoshis, TwoAmountShapesAreNotDerivable) {
    EXPECT_EQ(hodos::ExtractOutputSatoshis(
                  R"({"outputs":[{"satoshis":1}],"amount":100000000})"),
              hodos::kAmountNotDerivable);
}

// 1.2 — createAction reads sendMax from `options`, not the top level. Pricing
// this from outputs[] under-priced a full-wallet sweep as 1 satoshi.
TEST(ExtractOutputSatoshis, NestedOptionsSendMaxIsNotDerivable) {
    EXPECT_EQ(hodos::ExtractOutputSatoshis(
                  R"({"outputs":[{"satoshis":1}],"options":{"sendMax":true}})"),
              hodos::kAmountNotDerivable);
}

// options.sendMax:false must NOT trip the sentinel — ordinary createAction.
TEST(ExtractOutputSatoshis, NestedOptionsSendMaxFalseStillPrices) {
    EXPECT_EQ(hodos::ExtractOutputSatoshis(
                  R"({"outputs":[{"satoshis":9000}],"options":{"sendMax":false}})"),
              9000);
}

// The consequence the whole fix exists to prevent, asserted end to end:
// a decoy body must never reach the engine as "priced, 0 cents".
TEST(ComputePaymentCost, DecoyOutputsIsNeverPricedAtZeroWithPriceAvailable) {
    const auto c = hodos::ComputePaymentCost(
        "/transaction/send",
        R"({"outputs":[],"toAddress":"1abc","amount":100000000})", kPrice);
    EXPECT_TRUE(c.isPayment);
    EXPECT_EQ(c.satoshis, 100000000);
    EXPECT_TRUE(c.priceAvailable);
    EXPECT_GT(c.cents, 0);   // pre-fix this was 0 with priceAvailable=true
}

TEST(ComputePaymentCost, NestedSendMaxSweepIsNeverPriceAvailable) {
    const auto c = hodos::ComputePaymentCost(
        "/createAction",
        R"({"outputs":[{"satoshis":1}],"options":{"sendMax":true}})", kPrice);
    EXPECT_TRUE(c.isPayment);        // it IS a payment — it must still be gated
    EXPECT_FALSE(c.priceAvailable);  // but we refuse to price a sweep
    EXPECT_EQ(c.cents, 0);
}

}  // namespace
