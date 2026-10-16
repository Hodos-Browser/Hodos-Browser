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

#include "PortConfig.h"  // RequestPathForMatching — see IsPaymentEndpoint below

namespace hodos {

// Sentinel: this body moves funds but the amount is NOT derivable from it.
// {sendMax:true}, where the amount is the entire spendable balance and only the
// Rust wallet knows it; and (beta.6 P3) any amount that is not a real amount of
// money — see ReadSatoshis.
constexpr int64_t kAmountNotDerivable = -1;

// The most satoshis that can exist: 21,000,000 BSV. Mirrors rust-wallet
// handlers.rs :: MAX_SATOSHIS.
constexpr int64_t kMaxSatoshis = 2100000000000000LL;

// beta.6 P3 (`P3-A2`, TSA-141). One amount, as the wallet will read it: an integer
// in [0, kMaxSatoshis]. A NUMBER that is anything else — negative, fractional, above
// the maximum, or an unsigned value that would wrap in int64 — is not an amount, and
// the caller must fail closed. (Non-number fields keep their old handling: ignored,
// as `MalformedAndEmptyBodiesAreZeroNotCrash` pins; the wallet refuses them itself.) ⛔ A negative output used to be summed as-is, so
// outputs [1000000, -999998] priced as 2 sats and passed every cap.
inline bool ReadSatoshis(const nlohmann::json& v, int64_t& out) {
    if (v.is_number_unsigned()) {
        const uint64_t u = v.get<uint64_t>();
        if (u > static_cast<uint64_t>(kMaxSatoshis)) return false;
        out = static_cast<int64_t>(u);
        return true;
    }
    if (v.is_number_integer()) {
        const int64_t i = v.get<int64_t>();
        if (i < 0 || i > kMaxSatoshis) return false;
        out = i;
        return true;
    }
    return false;
}

inline bool IsPaymentEndpoint(const std::string& endpoint) {
    // ⛔ NORMALIZE FIRST — never match the raw target. `/%70rocessAction` routes
    // to `process_action` in actix but did not match here, so the call was
    // priced blind. See PortConfig.h :: RequestPathForMatching for the full
    // account. Normalizing INSIDE the predicate is the point: the callers below
    // (the IPC/page arm in HttpRequestInterceptor.cpp and every future one)
    // cannot forget to do it.
    const std::string path = RequestPathForMatching(endpoint);
    return path.find("/createAction") != std::string::npos
        || path.find("/acquireCertificate") != std::string::npos
        || path.find("/sendMessage") != std::string::npos
        // P0.5 finding 6 — the three direct fund-movers. Before this they reached
        // Rust with no X-Payment-* headers, so request_gate.rs substituted
        // {satoshis:0, cents:0, price_available:false} and matrix_c.rs rendered a
        // modal reading "0 sats", blaming a price-feed outage that was not
        // occurring, over what could be a full-balance sweep.
        || path.find("/transaction/send") != std::string::npos
        || path.find("/wallet/peerpay/send") != std::string::npos
        || path.find("/wallet/paymail/send") != std::string::npos
        // P0.5 Task A / §4o (`P0.5-X6`) — /processAction is create + sign +
        // BROADCAST in one call, and it was absent here, so C++ never stamped the
        // X-Payment-* headers and never treated it as a spend. Its body is the
        // SAME {outputs:[{satoshis}]} shape as /createAction, so ExtractOutputSatoshis
        // already handles it — no new body shape. ⛔ Pairs with the Rust-side gate
        // in handlers.rs :: process_action; do both or neither.
        || path.find("/processAction") != std::string::npos
        // P0.5 panel #3 — /wallet/pay402 mints and signs a BRC-121 BEEF payment
        // for an arbitrary amount, and it was absent here. That absence was not
        // merely a missing price: `handlers.rs :: pay_402` gated itself on
        // `X-Payment-Satoshis || X-User-Approved` being PRESENT, so not listing
        // the endpoint here GUARANTEED the headers were missing, which
        // GUARANTEED the gate was skipped. An approved dApp minted an
        // uncapped, unmetered, unprompted payment. Measured 2026-08-21.
        // ⛔ Its body is a FIFTH shape — {satoshis} at top level — taught to
        // ExtractOutputSatoshis below. Do both or neither: listing it here
        // without that shape would price every pay402 at 0 cents, which with a
        // live price reads as under every cap and auto-approves SILENTLY.
        || path.find("/wallet/pay402") != std::string::npos;
}

// FIVE body shapes, because five endpoint families reach here:
//   /createAction         {outputs:[{satoshis}, ...]}   (and /processAction)
//   /transaction/send     {toAddress, amount, sendMax}
//   /wallet/peerpay/send  {recipient_identity_key, amount_satoshis}
//   /wallet/paymail/send  {paymail, amount_satoshis}
//   /wallet/pay402        {server_pubkey_hex, satoshis, original_url}
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
                if (output.is_object() && output.contains("satoshis")
                    && output["satoshis"].is_number()) {
                    int64_t sats = 0;
                    if (!ReadSatoshis(output["satoshis"], sats)) return kAmountNotDerivable;
                    total += sats;  // both terms <= kMaxSatoshis: cannot overflow
                    if (total > kMaxSatoshis) return kAmountNotDerivable;
                }
            }
            amount = total;
            ++shapes;
        }

        // /transaction/send — {toAddress, amount}.
        if (json.contains("amount") && json["amount"].is_number()) {
            if (!ReadSatoshis(json["amount"], amount)) return kAmountNotDerivable;
            ++shapes;
        }

        // PeerPay + Paymail share the {..., amount_satoshis} shape.
        if (json.contains("amount_satoshis") && json["amount_satoshis"].is_number()) {
            if (!ReadSatoshis(json["amount_satoshis"], amount)) return kAmountNotDerivable;
            ++shapes;
        }

        // /wallet/pay402 — {server_pubkey_hex, satoshis, original_url}. TOP-LEVEL
        // `satoshis`, which is why it is its own shape and not the createAction
        // one: createAction carries `satoshis` only INSIDE `outputs[]`, and this
        // branch deliberately does not look in there. A body carrying both is
        // two shapes and falls to the ambiguity rule below, so the decoy family
        // that motivated that rule cannot be re-opened through this branch.
        if (json.contains("satoshis") && json["satoshis"].is_number()) {
            if (!ReadSatoshis(json["satoshis"], amount)) return kAmountNotDerivable;
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

// B5-T1-P2 (👤 owner, 2026-10-04). A payment the wallet could not broadcast YET but
// keeps sending — the BRC-100 WERR_REVIEW_ACTIONS (code 5) reply whose subject
// transaction is still "sending" — is money committed: it usually lands minutes
// later, from the wallet's background retry. The gold pill is the user's safeguard
// against a silent spend, so it lights for this reply too. A permanent refusal
// (invalidTx / doubleSpend) or an abandoned action ("failed") does not.
inline bool IsInFlightPaymentReply(const std::string& body) {
    try {
        const auto j = nlohmann::json::parse(body);
        if (!j.is_object() || j.value("isError", false) != true) return false;
        if (!j.contains("code") || !j["code"].is_number_integer() || j["code"].get<int>() != 5) return false;
        const auto txid = j.value("txid", std::string());
        if (txid.empty() || !j.contains("sendWithResults") || !j["sendWithResults"].is_array()) return false;
        for (const auto& r : j["sendWithResults"]) {
            if (r.is_object() && r.value("txid", std::string()) == txid)
                return r.value("status", std::string()) == "sending";
        }
    } catch (...) {}
    return false;
}

// Does a wallet reply to a PAYMENT endpoint light the gold pill? `ok` is the
// transport's success (2xx on the IPC path; UR_SUCCESS on the HTTP path). Unchanged
// rule: ok and no "error" key (an unparseable body counts as no error, as before) —
// plus the in-flight reply above.
inline bool PaymentReplyLightsPill(bool ok, const std::string& body) {
    if (ok) {
        bool hasError = false;
        try {
            hasError = nlohmann::json::parse(body).contains("error");
        } catch (...) {}
        if (!hasError) return true;
    }
    return IsInFlightPaymentReply(body);
}

}  // namespace hodos

#endif  // HODOS_PAYMENT_COST_H_
