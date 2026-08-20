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
inline int64_t ExtractOutputSatoshis(const std::string& body) {
    if (body.empty()) return 0;
    try {
        auto json = nlohmann::json::parse(body);

        // createAction — sum the requested outputs.
        if (json.contains("outputs") && json["outputs"].is_array()) {
            int64_t total = 0;
            for (const auto& output : json["outputs"]) {
                if (output.contains("satoshis") && output["satoshis"].is_number()) {
                    total += output["satoshis"].get<int64_t>();
                }
            }
            return total;
        }

        // sendMax wins over amount: when it is set the handler IGNORES the body's
        // `amount`, so trusting that field here would under-price a full sweep.
        if (json.contains("sendMax") && json["sendMax"].is_boolean()
            && json["sendMax"].get<bool>()) {
            return kAmountNotDerivable;
        }
        if (json.contains("amount") && json["amount"].is_number()) {
            return json["amount"].get<int64_t>();
        }

        // PeerPay + Paymail share the {..., amount_satoshis} shape.
        if (json.contains("amount_satoshis") && json["amount_satoshis"].is_number()) {
            return json["amount_satoshis"].get<int64_t>();
        }

        return 0;
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
