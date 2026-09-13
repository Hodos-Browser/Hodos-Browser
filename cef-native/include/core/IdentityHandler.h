// IdentityHandler.h
#pragma once

#include "include/cef_v8.h"
#include "../core/WalletService.h"
#include <nlohmann/json.hpp>
#include <iostream>

class IdentityHandler : public CefV8Handler {
public:
    IdentityHandler() {}

    bool Execute(const CefString& name,
                 CefRefPtr<CefV8Value> object,
                 const CefV8ValueList& arguments,
                 CefRefPtr<CefV8Value>& retval,
                 CefString& exception) override;

    IMPLEMENT_REFCOUNTING(IdentityHandler);
};

// Declare jsonToV8 function
// `deep = false` (default): nested objects/arrays inside an OBJECT are delivered as their
// `.dump()` string — the identity.get contract, which its callers parse themselves.
// `deep = true`: fully recursive, i.e. exactly what the page would get from evaluating
// the JSON as a literal. The Phase 8c bridge uses `deep = true`, because that is what
// the `window.on*(<json literal>)` path it replaces delivered (found by batch 4:
// `bookmarks.getAll()` came back with `bookmarks` as a STRING).
CefRefPtr<CefV8Value> jsonToV8(const nlohmann::json& j, bool deep = false);
