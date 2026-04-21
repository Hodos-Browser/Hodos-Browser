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
CefRefPtr<CefV8Value> jsonToV8(const nlohmann::json& j);
