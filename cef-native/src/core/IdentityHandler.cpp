#include "../../include/core/IdentityHandler.h"
#include <fstream>
#include <cstdlib>

CefRefPtr<CefV8Value> jsonToV8(const nlohmann::json& j) {
    if (j.is_object()) {
        CefRefPtr<CefV8Value> obj = CefV8Value::CreateObject(nullptr, nullptr);
        for (auto it = j.begin(); it != j.end(); ++it) {
            const std::string& key = it.key();
            const auto& value = it.value();
            if (value.is_string()) {
                obj->SetValue(key, CefV8Value::CreateString(value.get<std::string>()), V8_PROPERTY_ATTRIBUTE_NONE);
            } else if (value.is_boolean()) {
                obj->SetValue(key, CefV8Value::CreateBool(value), V8_PROPERTY_ATTRIBUTE_NONE);
            } else if (value.is_number_integer()) {
                obj->SetValue(key, CefV8Value::CreateInt(value), V8_PROPERTY_ATTRIBUTE_NONE);
            } else if (value.is_number_float()) {
                obj->SetValue(key, CefV8Value::CreateDouble(value), V8_PROPERTY_ATTRIBUTE_NONE);
            } else {
                obj->SetValue(key, CefV8Value::CreateString(value.dump()), V8_PROPERTY_ATTRIBUTE_NONE);
            }
        }
        return obj;
    }
    return CefV8Value::CreateUndefined();
}

bool IdentityHandler::Execute(const CefString& name,
                               CefRefPtr<CefV8Value> object,
                               const CefV8ValueList& arguments,
                               CefRefPtr<CefV8Value>& retval,
                               CefString& exception) {
    std::cout << "💡 IdentityHandler started - Function: " << name.ToString() << std::endl;
    std::cout.flush(); // Force flush

    // Also try OutputDebugString for Windows
    std::string debugMsg = "💡 IdentityHandler started - Function: " + name.ToString();
    OutputDebugStringA(debugMsg.c_str());
    OutputDebugStringA("\n");

    // For identity.get(), first check if local identity file exists
    if (name == "get") {
        const char* homeDir = std::getenv("USERPROFILE");
        std::string identityPath = std::string(homeDir) + "\\AppData\\Roaming\\BabbageBrowser\\identity.json";
        std::ifstream identityFile(identityPath);
        if (identityFile.good()) {
            std::cout << "📁 Local identity file exists, reading from file" << std::endl;
            try {
                nlohmann::json identity;
                identityFile >> identity;
                identityFile.close();

                CefRefPtr<CefV8Value> identityObject = jsonToV8(identity);
                retval = identityObject;
                return true;
            } catch (const std::exception& e) {
                std::cerr << "💥 Error reading identity file: " << e.what() << std::endl;
                identityFile.close();
                // Fall through to daemon check
            }
        } else {
            std::cout << "📁 No local identity file found, will check daemon" << std::endl;
            identityFile.close();
        }
    }

    WalletService walletService;

    // Check if Go daemon is running
    if (!walletService.isConnected()) {
        std::cerr << "❌ Cannot connect to Go wallet daemon. Make sure it's running on port 3301." << std::endl;
        exception = "Go wallet daemon is not running. Please start the wallet daemon first.";
        return false;
    }

    // Check daemon health
    if (!walletService.isHealthy()) {
        std::cerr << "❌ Go wallet daemon is not healthy" << std::endl;
        exception = "Go wallet daemon is not responding properly.";
        return false;
    }

    if (name == "markBackedUp") {
        std::cout << "✅ Marking wallet as backed up via Go daemon" << std::endl;

        if (walletService.markWalletBackedUp()) {
            retval = CefV8Value::CreateString("success");
        } else {
            retval = CefV8Value::CreateString("error");
        }

        return true;
    }

    try {
        // Get wallet info from Go daemon
        nlohmann::json walletInfo = walletService.getWalletInfo();

        if (walletInfo.empty()) {
            std::cerr << "❌ Failed to get wallet info from Go daemon" << std::endl;
            exception = "Failed to retrieve wallet info from Go wallet daemon.";
            return false;
        }

        std::cout << "📦 Wallet info from Go daemon: " << walletInfo.dump() << std::endl;

        CefRefPtr<CefV8Value> walletObject = jsonToV8(walletInfo);
        retval = walletObject;

        return true;
    } catch (const std::exception& e) {
        std::cerr << "💥 Error in IdentityHandler: " << e.what() << std::endl;
        exception = "Exception in IdentityHandler: " + std::string(e.what());
        return false;
    }
}
