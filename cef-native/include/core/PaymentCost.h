// PaymentCost.h — how much money does this request move?
//
// Extracted from HttpRequestInterceptor.cpp during P0.5 finding 6, for the same
// reason JsStringEscape.h was extracted by the F6 audit: this is pure logic on a
// money path, and it cannot be unit-tested while it lives in a CEF-dependent TU.
// Nothing here touches CEF, HTTP, or a singleton — the BSV price is passed IN.
//
// ⛔ THE TWO FUNCTIONS BELOW ARE ONE CHANGE. isPaymentEndpoint says "this request
// moves funds, price it"; ExtractOutputSatoshis says how to read the amount out of
// that particular body. Listing an endpoint in the first without teaching the
// second its body shape prices the call at 0 cents, which the engine reads as
// under every cap and SILENTLY AUTO-APPROVES (matrix_c.rs ::
// bsv_price_available_with_zero_cents_is_silent asserts exactly that). Do both or
// neither — this is the d33741a rule, written down where it can be tested.

#ifndef HODOS_PAYMENT_COST_H_
#define HODOS_PAYMENT_COST_H_

#include <cstdint>
#include <string>
#include <nlohmann/json.hpp>

namespace hodos {

// Sentinel: this body moves funds but the amount is NOT derivable from it.
// Today the only such shape is {sendMax:true}, where the amount is the entire
// spendable balance and only the Rust wallet knows it.
constexpr int64_t kAmountNotDerivable = -1;

inline bool IsPaymentEndpoint(const std::string& endpoint) {
    return endpoint.find("/createAction") != std::string::npos
        || endpoint.find("/acquireCertificate") != std::string::npos
        || endpoint.find("/sendMessage") != std::string::npos
        // P0.5 finding 6 — the three direct fund-movers. Before this they reached
        // Rust with no X-Payment-* headers, so request_gate.rs substituted
        // {satoshis:0, cents:0, price_available:false} and matrix_c.rs rendered a
        // modal reading "0 sats", blaming a price-feed outage that was not
        // occurring, over what could be a full-balance sweep.
        || endpoint.find("/transaction/send") != std::string::npos
        || endpoint.find("/wallet/peerpay/send") != std::string::npos
        || endpoint.find("/wallet/paymail/send") != std::string::npos;
}

// FOUR body shapes, because four endpoint families reach here:
//   /createAction         {outputs:[{satoshis}, ...]}
//   /transaction/send     {toAddress, amount, sendMax}
//   /wallet/peerpay/send  {recipient_identity_key, amount_satoshis}
//   /wallet/paymail/send  {paymail, amount_satoshis}
// ⛔ NEVER RETURN THE FIRST SHAPE THAT MATCHES. (P0.5 panel #2, findings 1.1 + 1.2)
//
// This function used to test each shape in turn and return from the first branch
// that matched. Two ways that under-priced a real spend to ZERO — and 0 cents with
// a live price reads as "under every cap", which auto-approves SILENTLY:
//
//   1. DECOY KEY. `{"outputs":[], "toAddress":"1…", "amount":100000000}` matched the
//      `outputs` branch, summed an empty array, and returned 0 — while Rust's
//      `SendTransactionRequest` (no `deny_unknown_fields`) ignored the decoy
//      `outputs` and spent the real `amount`. Any amount, one Allow click.
//
//   2. NESTED sendMax. Only TOP-LEVEL `sendMax` was checked, but createAction reads
//      it from `options` (`handlers.rs :: create_action_internal`, "Extract send_max
//      early"). `{"outputs":[{"satoshis":1}], "options":{"sendMax":true}}` priced at
//      1 satoshi and swept the wallet. Both layers re-read the same wrong quantity,
//      so the Rust defence-in-depth missed it identically.
//
// The rule now: find EVERY amount shape the body carries. Exactly one → price it.
// More than one → the body is ambiguous about what it spends, so refuse to price it
// and let Rust resolve or the user confirm. An EMPTY `outputs` array is not a shape
// at all and falls through, so a decoy cannot mask the field that really spends.
inline bool IsSendMaxAnywhere(const nlohmann::json& json) {
    auto truthy = [](const nlohmann::json& v) {
        return v.is_boolean() && v.get<bool>();
    };
    if (json.contains("sendMax") && truthy(json["sendMax"])) return true;
    // createAction carries it here, and this is the half that was missed.
    if (json.contains("options") && json["options"].is_object()) {
        const auto& opts = json["options"];
        if (opts.contains("sendMax") && truthy(opts["sendMax"])) return true;
    }
    return false;
}

inline int64_t ExtractOutputSatoshis(const std::string& body) {
    if (body.empty()) return 0;
    try {
        auto json = nlohmann::json::parse(body);
        if (!json.is_object()) return 0;

        // Checked before any amount shape: when sendMax is set the handler IGNORES
        // whatever amount the body carries, so pricing that amount under-prices a
        // full-balance sweep.
        if (IsSendMaxAnywhere(json)) return kAmountNotDerivable;

        int shapes = 0;
        int64_t amount = 0;

        // createAction — sum the requested outputs. An empty array is NOT a shape:
        // it carries no amount, and treating it as "0 satoshis" is exactly the
        // decoy above.
        if (json.contains("outputs") && json["outputs"].is_array()
            && !json["outputs"].empty()) {
            int64_t total = 0;
            for (const auto& output : json["outputs"]) {
                if (output.contains("satoshis") && output["satoshis"].is_number()) {
                    total += output["satoshis"].get<int64_t>();
                }
            }
            amount = total;
            ++shapes;
        }

        // /transaction/send — {toAddress, amount}.
        if (json.contains("amount") && json["amount"].is_number()) {
            amount = json["amount"].get<int64_t>();
            ++shapes;
        }

        // PeerPay + Paymail share the {..., amount_satoshis} shape.
        if (json.contains("amount_satoshis") && json["amount_satoshis"].is_number()) {
            amount = json["amount_satoshis"].get<int64_t>();
            ++shapes;
        }

        // Two or more spendable shapes in one body: we cannot know which one the
        // handler will honour, so fail CLOSED rather than guess. Rust resolves the
        // real figure, or the user is asked.
        if (shapes > 1) return kAmountNotDerivable;
        return amount;  // shapes == 1 → that amount; shapes == 0 → 0
    } catch (...) {
        return 0;
    }
}

struct PaymentCost {
    bool isPayment = false;
    int64_t satoshis = 0;
    int64_t cents = 0;
    bool priceAvailable = false;
};

// The pricing of a fund-moving request. ONE rule, computed ONCE.
//
// `bsvPriceUsd` is the BSV/USD price, or <= 0 when the cache has none.
//
// priceAvailable is deliberately NOT just "the cache has a price" — it also
// requires that the amount was DERIVABLE. For {sendMax:true} it is not, so this
// reports priceAvailable=false and Rust resolves the real sweep amount before
// gating. If that override ever fails to run, the engine sees an unpriced payment
// and PROMPTS, which is the fail-closed direction. Reporting priceAvailable=true
// with 0 cents would instead auto-approve a full-balance sweep.
inline PaymentCost ComputePaymentCost(const std::string& endpoint,
                                      const std::string& body,
                                      double bsvPriceUsd) {
    PaymentCost c;
    c.isPayment = IsPaymentEndpoint(endpoint);
    if (!c.isPayment) return c;

    const int64_t satoshis = ExtractOutputSatoshis(body);
    if (satoshis == kAmountNotDerivable) {
        // Fund-moving, amount unknowable here. Fail closed; Rust resolves it.
        return c;
    }

    c.satoshis = satoshis;
    c.priceAvailable = (bsvPriceUsd > 0);
    if (c.priceAvailable && satoshis > 0) {
        c.cents = static_cast<int64_t>(
            (static_cast<double>(satoshis) / 100000000.0) * bsvPriceUsd * 100.0);
    }
    return c;
}

}  // namespace hodos

#endif  // HODOS_PAYMENT_COST_H_
