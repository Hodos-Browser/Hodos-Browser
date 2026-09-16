// PromptTypes.h — the prompt-type classifications, in ONE place.
//
// beta.3 Phase 10e (panel `F3-10b`). These two predicates must agree, and they
// lived in different files and did not:
//
//   PendingAuthRequest.h :: isConnectPromptType   domain_approval | brc100_auth | manifest_connect_bundle
//   HttpRequestInterceptor.cpp :: isDomainTrustPrompt   domain_approval | manifest_connect_bundle
//
// ⛔ Why the disagreement is dangerous. `isDomainTrustPrompt` decides whether a
// single-use `X-User-Approved` token is attached to the pending entry. A CONNECT
// prompt must never carry one, because connect entries are resolved by
// `popConnectForDomain`'s deliberate fan-out — one Allow resumes every waiting
// call from that domain. A connect-type entry holding a live token would therefore
// be replayed with that token, which is CU-1, the exact defect Phase 10b closed.
//
// `brc100_auth` sat in the first list and not the second. It is not reachable
// today — `PromptType::Brc100Auth` exists in the Rust engine (`decision.rs`) and is
// never constructed, and `openBRC100AuthApprovalModal` stores its entries as
// `domain_approval` — so nothing is broken right now. It was one enum arm away.
//
// ⭐ The fix is not "add the missing string". It is that there is now one list, and
// a test (`prompt_type_agreement_test.cpp`) asserts the invariant directly against
// these functions rather than against a copy of them.
#ifndef HODOS_PROMPT_TYPES_H_
#define HODOS_PROMPT_TYPES_H_

#include <string>

namespace hodos {

// A prompt that establishes or extends the site's TRUST, rather than authorising one
// specific call. Answering it resumes every waiting call from that domain.
inline bool IsConnectPromptType(const std::string& type) {
    return type == "domain_approval"
        || type == "brc100_auth"
        || type == "manifest_connect_bundle";
}

// ⛔ MUST be true for every connect type. A prompt of this class is re-issued WITHOUT
// a token and re-evaluated by the engine; anything else gets the single-use
// `X-User-Approved` header bound to its own body.
inline bool IsDomainTrustPromptType(const std::string& promptType) {
    return IsConnectPromptType(promptType);
}

}  // namespace hodos

#endif  // HODOS_PROMPT_TYPES_H_
