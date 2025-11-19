#include "BRC100Handler.h"
#include "BRC100Bridge.h"
#include "include/cef_v8.h"
#include <iostream>
#include <sstream>

BRC100Handler::BRC100Handler() {
    bridge_ = std::make_unique<BRC100Bridge>();
}

BRC100Handler::~BRC100Handler() {
}

bool BRC100Handler::Execute(const CefString& name,
                           CefRefPtr<CefV8Value> object,
                           const CefV8ValueList& arguments,
                           CefRefPtr<CefV8Value>& retval,
                           CefString& exception) {

    std::string methodName = V8StringToStdString(name);

    if (methodName == "status") {
        return HandleStatus(object, arguments, retval, exception);
    } else if (methodName == "isAvailable") {
        return HandleIsAvailable(object, arguments, retval, exception);
    } else if (methodName == "generateIdentity") {
        return HandleGenerateIdentity(object, arguments, retval, exception);
    } else if (methodName == "validateIdentity") {
        return HandleValidateIdentity(object, arguments, retval, exception);
    } else if (methodName == "selectiveDisclosure") {
        return HandleSelectiveDisclosure(object, arguments, retval, exception);
    } else if (methodName == "generateChallenge") {
        return HandleGenerateChallenge(object, arguments, retval, exception);
    } else if (methodName == "authenticate") {
        return HandleAuthenticate(object, arguments, retval, exception);
    } else if (methodName == "deriveType42Keys") {
        return HandleDeriveType42Keys(object, arguments, retval, exception);
    } else if (methodName == "createSession") {
        return HandleCreateSession(object, arguments, retval, exception);
    } else if (methodName == "validateSession") {
        return HandleValidateSession(object, arguments, retval, exception);
    } else if (methodName == "revokeSession") {
        return HandleRevokeSession(object, arguments, retval, exception);
    } else if (methodName == "createBEEF") {
        return HandleCreateBEEF(object, arguments, retval, exception);
    } else if (methodName == "verifyBEEF") {
        return HandleVerifyBEEF(object, arguments, retval, exception);
    } else if (methodName == "broadcastBEEF") {
        return HandleBroadcastBEEF(object, arguments, retval, exception);
    } else if (methodName == "verifySPV") {
        return HandleVerifySPV(object, arguments, retval, exception);
    } else if (methodName == "createSPVProof") {
        return HandleCreateSPVProof(object, arguments, retval, exception);
    }

    exception = "Unknown method: " + methodName;
    return false;
}

void BRC100Handler::RegisterBRC100API(CefRefPtr<CefV8Context> context) {
    CefRefPtr<CefV8Value> global = context->GetGlobal();

    // Create bitcoinBrowser object if it doesn't exist
    CefRefPtr<CefV8Value> bitcoinBrowser = global->GetValue("bitcoinBrowser");
    if (bitcoinBrowser->IsUndefined()) {
        bitcoinBrowser = CefV8Value::CreateObject(nullptr, nullptr);
        global->SetValue("bitcoinBrowser", bitcoinBrowser, V8_PROPERTY_ATTRIBUTE_NONE);
    }

    // Create brc100 object
    CefRefPtr<CefV8Value> brc100 = CefV8Value::CreateObject(nullptr, nullptr);
    CefRefPtr<BRC100Handler> handler = new BRC100Handler();

    // Register all BRC-100 methods
    brc100->SetValue("status", CefV8Value::CreateFunction("status", handler), V8_PROPERTY_ATTRIBUTE_NONE);
    brc100->SetValue("isAvailable", CefV8Value::CreateFunction("isAvailable", handler), V8_PROPERTY_ATTRIBUTE_NONE);

    // Identity methods
    brc100->SetValue("generateIdentity", CefV8Value::CreateFunction("generateIdentity", handler), V8_PROPERTY_ATTRIBUTE_NONE);
    brc100->SetValue("validateIdentity", CefV8Value::CreateFunction("validateIdentity", handler), V8_PROPERTY_ATTRIBUTE_NONE);
    brc100->SetValue("selectiveDisclosure", CefV8Value::CreateFunction("selectiveDisclosure", handler), V8_PROPERTY_ATTRIBUTE_NONE);

    // Authentication methods
    brc100->SetValue("generateChallenge", CefV8Value::CreateFunction("generateChallenge", handler), V8_PROPERTY_ATTRIBUTE_NONE);
    brc100->SetValue("authenticate", CefV8Value::CreateFunction("authenticate", handler), V8_PROPERTY_ATTRIBUTE_NONE);
    brc100->SetValue("deriveType42Keys", CefV8Value::CreateFunction("deriveType42Keys", handler), V8_PROPERTY_ATTRIBUTE_NONE);

    // Session methods
    brc100->SetValue("createSession", CefV8Value::CreateFunction("createSession", handler), V8_PROPERTY_ATTRIBUTE_NONE);
    brc100->SetValue("validateSession", CefV8Value::CreateFunction("validateSession", handler), V8_PROPERTY_ATTRIBUTE_NONE);
    brc100->SetValue("revokeSession", CefV8Value::CreateFunction("revokeSession", handler), V8_PROPERTY_ATTRIBUTE_NONE);

    // BEEF transaction methods
    brc100->SetValue("createBEEF", CefV8Value::CreateFunction("createBEEF", handler), V8_PROPERTY_ATTRIBUTE_NONE);
    brc100->SetValue("verifyBEEF", CefV8Value::CreateFunction("verifyBEEF", handler), V8_PROPERTY_ATTRIBUTE_NONE);
    brc100->SetValue("broadcastBEEF", CefV8Value::CreateFunction("broadcastBEEF", handler), V8_PROPERTY_ATTRIBUTE_NONE);

    // SPV methods
    brc100->SetValue("verifySPV", CefV8Value::CreateFunction("verifySPV", handler), V8_PROPERTY_ATTRIBUTE_NONE);
    brc100->SetValue("createSPVProof", CefV8Value::CreateFunction("createSPVProof", handler), V8_PROPERTY_ATTRIBUTE_NONE);

    // Add brc100 to bitcoinBrowser
    bitcoinBrowser->SetValue("brc100", brc100, V8_PROPERTY_ATTRIBUTE_NONE);

    std::cout << "BRC-100 API registered successfully" << std::endl;
}

// Status & Detection
bool BRC100Handler::HandleStatus(CefRefPtr<CefV8Value> object, const CefV8ValueList& arguments, CefRefPtr<CefV8Value>& retval, CefString& exception) {
    try {
        auto response = bridge_->getStatus();
        retval = JSONToV8Value(response);
        return true;
    } catch (const std::exception& e) {
        exception = "Status request failed: " + std::string(e.what());
        return false;
    }
}

bool BRC100Handler::HandleIsAvailable(CefRefPtr<CefV8Value> object, const CefV8ValueList& arguments, CefRefPtr<CefV8Value>& retval, CefString& exception) {
    try {
        bool available = bridge_->isAvailable();
        retval = CefV8Value::CreateBool(available);
        return true;
    } catch (const std::exception& e) {
        exception = "Availability check failed: " + std::string(e.what());
        return false;
    }
}

// Identity Management
bool BRC100Handler::HandleGenerateIdentity(CefRefPtr<CefV8Value> object, const CefV8ValueList& arguments, CefRefPtr<CefV8Value>& retval, CefString& exception) {
    if (arguments.size() != 1 || !arguments[0]->IsObject()) {
        exception = "Invalid arguments for generateIdentity";
        return false;
    }

    try {
        auto identityData = V8ValueToJSON(arguments[0]);
        auto response = bridge_->generateIdentity(identityData);
        retval = JSONToV8Value(response);
        return true;
    } catch (const std::exception& e) {
        exception = "Identity generation failed: " + std::string(e.what());
        return false;
    }
}

bool BRC100Handler::HandleValidateIdentity(CefRefPtr<CefV8Value> object, const CefV8ValueList& arguments, CefRefPtr<CefV8Value>& retval, CefString& exception) {
    if (arguments.size() != 1 || !arguments[0]->IsObject()) {
        exception = "Invalid arguments for validateIdentity";
        return false;
    }

    try {
        auto identityData = V8ValueToJSON(arguments[0]);
        auto response = bridge_->validateIdentity(identityData);
        retval = JSONToV8Value(response);
        return true;
    } catch (const std::exception& e) {
        exception = "Identity validation failed: " + std::string(e.what());
        return false;
    }
}

bool BRC100Handler::HandleSelectiveDisclosure(CefRefPtr<CefV8Value> object, const CefV8ValueList& arguments, CefRefPtr<CefV8Value>& retval, CefString& exception) {
    if (arguments.size() != 1 || !arguments[0]->IsObject()) {
        exception = "Invalid arguments for selectiveDisclosure";
        return false;
    }

    try {
        auto disclosureData = V8ValueToJSON(arguments[0]);
        auto response = bridge_->createSelectiveDisclosure(disclosureData);
        retval = JSONToV8Value(response);
        return true;
    } catch (const std::exception& e) {
        exception = "Selective disclosure creation failed: " + std::string(e.what());
        return false;
    }
}

// Authentication
bool BRC100Handler::HandleGenerateChallenge(CefRefPtr<CefV8Value> object, const CefV8ValueList& arguments, CefRefPtr<CefV8Value>& retval, CefString& exception) {
    if (arguments.size() != 1 || !arguments[0]->IsObject()) {
        exception = "Invalid arguments for generateChallenge";
        return false;
    }

    try {
        auto challengeData = V8ValueToJSON(arguments[0]);
        auto response = bridge_->generateChallenge(challengeData);
        retval = JSONToV8Value(response);
        return true;
    } catch (const std::exception& e) {
        exception = "Challenge generation failed: " + std::string(e.what());
        return false;
    }
}

bool BRC100Handler::HandleAuthenticate(CefRefPtr<CefV8Value> object, const CefV8ValueList& arguments, CefRefPtr<CefV8Value>& retval, CefString& exception) {
    if (arguments.size() != 1 || !arguments[0]->IsObject()) {
        exception = "Invalid arguments for authenticate";
        return false;
    }

    try {
        auto authData = V8ValueToJSON(arguments[0]);
        auto response = bridge_->authenticate(authData);
        retval = JSONToV8Value(response);
        return true;
    } catch (const std::exception& e) {
        exception = "Authentication failed: " + std::string(e.what());
        return false;
    }
}

bool BRC100Handler::HandleDeriveType42Keys(CefRefPtr<CefV8Value> object, const CefV8ValueList& arguments, CefRefPtr<CefV8Value>& retval, CefString& exception) {
    if (arguments.size() != 1 || !arguments[0]->IsObject()) {
        exception = "Invalid arguments for deriveType42Keys";
        return false;
    }

    try {
        auto keyData = V8ValueToJSON(arguments[0]);
        auto response = bridge_->deriveType42Keys(keyData);
        retval = JSONToV8Value(response);
        return true;
    } catch (const std::exception& e) {
        exception = "Type-42 key derivation failed: " + std::string(e.what());
        return false;
    }
}

// Session Management
bool BRC100Handler::HandleCreateSession(CefRefPtr<CefV8Value> object, const CefV8ValueList& arguments, CefRefPtr<CefV8Value>& retval, CefString& exception) {
    if (arguments.size() != 1 || !arguments[0]->IsObject()) {
        exception = "Invalid arguments for createSession";
        return false;
    }

    try {
        auto sessionData = V8ValueToJSON(arguments[0]);
        auto response = bridge_->createSession(sessionData);
        retval = JSONToV8Value(response);
        return true;
    } catch (const std::exception& e) {
        exception = "Session creation failed: " + std::string(e.what());
        return false;
    }
}

bool BRC100Handler::HandleValidateSession(CefRefPtr<CefV8Value> object, const CefV8ValueList& arguments, CefRefPtr<CefV8Value>& retval, CefString& exception) {
    if (arguments.size() != 1 || !arguments[0]->IsObject()) {
        exception = "Invalid arguments for validateSession";
        return false;
    }

    try {
        auto sessionData = V8ValueToJSON(arguments[0]);
        auto response = bridge_->validateSession(sessionData);
        retval = JSONToV8Value(response);
        return true;
    } catch (const std::exception& e) {
        exception = "Session validation failed: " + std::string(e.what());
        return false;
    }
}

bool BRC100Handler::HandleRevokeSession(CefRefPtr<CefV8Value> object, const CefV8ValueList& arguments, CefRefPtr<CefV8Value>& retval, CefString& exception) {
    if (arguments.size() != 1 || !arguments[0]->IsObject()) {
        exception = "Invalid arguments for revokeSession";
        return false;
    }

    try {
        auto sessionData = V8ValueToJSON(arguments[0]);
        auto response = bridge_->revokeSession(sessionData);
        retval = JSONToV8Value(response);
        return true;
    } catch (const std::exception& e) {
        exception = "Session revocation failed: " + std::string(e.what());
        return false;
    }
}

// BEEF Transaction Management
bool BRC100Handler::HandleCreateBEEF(CefRefPtr<CefV8Value> object, const CefV8ValueList& arguments, CefRefPtr<CefV8Value>& retval, CefString& exception) {
    if (arguments.size() != 1 || !arguments[0]->IsObject()) {
        exception = "Invalid arguments for createBEEF";
        return false;
    }

    try {
        auto beefData = V8ValueToJSON(arguments[0]);
        auto response = bridge_->createBEEF(beefData);
        retval = JSONToV8Value(response);
        return true;
    } catch (const std::exception& e) {
        exception = "BEEF creation failed: " + std::string(e.what());
        return false;
    }
}

bool BRC100Handler::HandleVerifyBEEF(CefRefPtr<CefV8Value> object, const CefV8ValueList& arguments, CefRefPtr<CefV8Value>& retval, CefString& exception) {
    if (arguments.size() != 1 || !arguments[0]->IsObject()) {
        exception = "Invalid arguments for verifyBEEF";
        return false;
    }

    try {
        auto beefData = V8ValueToJSON(arguments[0]);
        auto response = bridge_->verifyBEEF(beefData);
        retval = JSONToV8Value(response);
        return true;
    } catch (const std::exception& e) {
        exception = "BEEF verification failed: " + std::string(e.what());
        return false;
    }
}

bool BRC100Handler::HandleBroadcastBEEF(CefRefPtr<CefV8Value> object, const CefV8ValueList& arguments, CefRefPtr<CefV8Value>& retval, CefString& exception) {
    if (arguments.size() != 1 || !arguments[0]->IsObject()) {
        exception = "Invalid arguments for broadcastBEEF";
        return false;
    }

    try {
        auto beefData = V8ValueToJSON(arguments[0]);
        auto response = bridge_->broadcastBEEF(beefData);
        retval = JSONToV8Value(response);
        return true;
    } catch (const std::exception& e) {
        exception = "BEEF broadcast failed: " + std::string(e.what());
        return false;
    }
}

// SPV Operations
bool BRC100Handler::HandleVerifySPV(CefRefPtr<CefV8Value> object, const CefV8ValueList& arguments, CefRefPtr<CefV8Value>& retval, CefString& exception) {
    if (arguments.size() != 1 || !arguments[0]->IsObject()) {
        exception = "Invalid arguments for verifySPV";
        return false;
    }

    try {
        auto spvData = V8ValueToJSON(arguments[0]);
        auto response = bridge_->verifySPV(spvData);
        retval = JSONToV8Value(response);
        return true;
    } catch (const std::exception& e) {
        exception = "SPV verification failed: " + std::string(e.what());
        return false;
    }
}

bool BRC100Handler::HandleCreateSPVProof(CefRefPtr<CefV8Value> object, const CefV8ValueList& arguments, CefRefPtr<CefV8Value>& retval, CefString& exception) {
    if (arguments.size() != 1 || !arguments[0]->IsObject()) {
        exception = "Invalid arguments for createSPVProof";
        return false;
    }

    try {
        auto proofData = V8ValueToJSON(arguments[0]);
        auto response = bridge_->createSPVProof(proofData);
        retval = JSONToV8Value(response);
        return true;
    } catch (const std::exception& e) {
        exception = "SPV proof creation failed: " + std::string(e.what());
        return false;
    }
}

// Helper methods
nlohmann::json BRC100Handler::V8ValueToJSON(CefRefPtr<CefV8Value> value) {
    if (value->IsBool()) {
        return nlohmann::json(value->GetBoolValue());
    } else if (value->IsInt()) {
        return nlohmann::json(value->GetIntValue());
    } else if (value->IsDouble()) {
        return nlohmann::json(value->GetDoubleValue());
    } else if (value->IsString()) {
        return nlohmann::json(V8StringToStdString(value->GetStringValue()));
    } else if (value->IsArray()) {
        nlohmann::json arr = nlohmann::json::array();
        for (int i = 0; i < value->GetArrayLength(); i++) {
            arr.push_back(V8ValueToJSON(value->GetValue(i)));
        }
        return arr;
    } else if (value->IsObject()) {
        nlohmann::json obj = nlohmann::json::object();
        std::vector<CefString> keys;
        value->GetKeys(keys);
        for (const auto& key : keys) {
            obj[V8StringToStdString(key)] = V8ValueToJSON(value->GetValue(key));
        }
        return obj;
    }
    return nlohmann::json(nullptr);
}

CefRefPtr<CefV8Value> BRC100Handler::JSONToV8Value(const nlohmann::json& json) {
    if (json.is_null()) {
        return CefV8Value::CreateNull();
    } else if (json.is_boolean()) {
        return CefV8Value::CreateBool(json.get<bool>());
    } else if (json.is_number_integer()) {
        return CefV8Value::CreateInt(json.get<int>());
    } else if (json.is_number_float()) {
        return CefV8Value::CreateDouble(json.get<double>());
    } else if (json.is_string()) {
        return CefV8Value::CreateString(json.get<std::string>());
    } else if (json.is_array()) {
        CefRefPtr<CefV8Value> arr = CefV8Value::CreateArray(json.size());
        for (size_t i = 0; i < json.size(); i++) {
            arr->SetValue(i, JSONToV8Value(json[i]));
        }
        return arr;
    } else if (json.is_object()) {
        CefRefPtr<CefV8Value> obj = CefV8Value::CreateObject(nullptr, nullptr);
        for (auto& item : json.items()) {
            obj->SetValue(item.key(), JSONToV8Value(item.value()), V8_PROPERTY_ATTRIBUTE_NONE);
        }
        return obj;
    }
    return CefV8Value::CreateNull();
}

std::string BRC100Handler::V8StringToStdString(const CefString& cefStr) {
    return cefStr.ToString();
}
