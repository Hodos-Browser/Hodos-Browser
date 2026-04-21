#pragma once

#include "include/cef_v8.h"
#include "BRC100Bridge.h"

class BRC100Handler : public CefV8Handler {
public:
    BRC100Handler();
    ~BRC100Handler();

    // CefV8Handler methods
    bool Execute(const CefString& name,
                 CefRefPtr<CefV8Value> object,
                 const CefV8ValueList& arguments,
                 CefRefPtr<CefV8Value>& retval,
                 CefString& exception) override;

    // Initialize BRC-100 API in JavaScript context
    static void RegisterBRC100API(CefRefPtr<CefV8Context> context);

private:
    std::unique_ptr<BRC100Bridge> bridge_;

    // API method handlers
    bool HandleStatus(CefRefPtr<CefV8Value> object, const CefV8ValueList& arguments, CefRefPtr<CefV8Value>& retval, CefString& exception);
    bool HandleIsAvailable(CefRefPtr<CefV8Value> object, const CefV8ValueList& arguments, CefRefPtr<CefV8Value>& retval, CefString& exception);

    // Identity methods
    bool HandleGenerateIdentity(CefRefPtr<CefV8Value> object, const CefV8ValueList& arguments, CefRefPtr<CefV8Value>& retval, CefString& exception);
    bool HandleValidateIdentity(CefRefPtr<CefV8Value> object, const CefV8ValueList& arguments, CefRefPtr<CefV8Value>& retval, CefString& exception);
    bool HandleSelectiveDisclosure(CefRefPtr<CefV8Value> object, const CefV8ValueList& arguments, CefRefPtr<CefV8Value>& retval, CefString& exception);

    // Authentication methods
    bool HandleGenerateChallenge(CefRefPtr<CefV8Value> object, const CefV8ValueList& arguments, CefRefPtr<CefV8Value>& retval, CefString& exception);
    bool HandleAuthenticate(CefRefPtr<CefV8Value> object, const CefV8ValueList& arguments, CefRefPtr<CefV8Value>& retval, CefString& exception);
    bool HandleDeriveType42Keys(CefRefPtr<CefV8Value> object, const CefV8ValueList& arguments, CefRefPtr<CefV8Value>& retval, CefString& exception);

    // Session methods
    bool HandleCreateSession(CefRefPtr<CefV8Value> object, const CefV8ValueList& arguments, CefRefPtr<CefV8Value>& retval, CefString& exception);
    bool HandleValidateSession(CefRefPtr<CefV8Value> object, const CefV8ValueList& arguments, CefRefPtr<CefV8Value>& retval, CefString& exception);
    bool HandleRevokeSession(CefRefPtr<CefV8Value> object, const CefV8ValueList& arguments, CefRefPtr<CefV8Value>& retval, CefString& exception);

    // BEEF transaction methods
    bool HandleCreateBEEF(CefRefPtr<CefV8Value> object, const CefV8ValueList& arguments, CefRefPtr<CefV8Value>& retval, CefString& exception);
    bool HandleVerifyBEEF(CefRefPtr<CefV8Value> object, const CefV8ValueList& arguments, CefRefPtr<CefV8Value>& retval, CefString& exception);
    bool HandleBroadcastBEEF(CefRefPtr<CefV8Value> object, const CefV8ValueList& arguments, CefRefPtr<CefV8Value>& retval, CefString& exception);

    // SPV methods
    bool HandleVerifySPV(CefRefPtr<CefV8Value> object, const CefV8ValueList& arguments, CefRefPtr<CefV8Value>& retval, CefString& exception);
    bool HandleCreateSPVProof(CefRefPtr<CefV8Value> object, const CefV8ValueList& arguments, CefRefPtr<CefV8Value>& retval, CefString& exception);

    // Helper methods
    nlohmann::json V8ValueToJSON(CefRefPtr<CefV8Value> value);
    CefRefPtr<CefV8Value> JSONToV8Value(const nlohmann::json& json);
    std::string V8StringToStdString(const CefString& cefStr);

    IMPLEMENT_REFCOUNTING(BRC100Handler);
    DISALLOW_COPY_AND_ASSIGN(BRC100Handler);
};
