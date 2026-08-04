#include "../../include/core/IdentityHandler.h"
#include "../../include/core/AppPaths.h"
#include <fstream>
#include <cstdlib>
#include <cstdint>
#include <filesystem>

namespace {

// Convert a JSON *scalar* to V8. Shared by both branches of jsonToV8 so that
// integers are widened consistently.
//
// ⚠️ The int64 case is why this exists. CefV8Value::CreateInt takes an int32, so a
// Chromium-epoch timestamp (microseconds since 1601 — comfortably past 2^31) used to
// be silently truncated to garbage. JS numbers are IEEE doubles anyway, so emitting
// CreateDouble for anything that doesn't fit int32 is both correct and lossless up to
// 2^53. Returns null for a non-scalar so callers can decide how to render it.
CefRefPtr<CefV8Value> jsonScalarToV8(const nlohmann::json& v) {
    if (v.is_string())  return CefV8Value::CreateString(v.get<std::string>());
    if (v.is_boolean()) return CefV8Value::CreateBool(v.get<bool>());
    if (v.is_null())    return CefV8Value::CreateNull();
    if (v.is_number_integer()) {
        const int64_t n = v.get<int64_t>();
        if (n >= INT32_MIN && n <= INT32_MAX) {
            return CefV8Value::CreateInt(static_cast<int32_t>(n));
        }
        return CefV8Value::CreateDouble(static_cast<double>(n));
    }
    if (v.is_number_float()) return CefV8Value::CreateDouble(v.get<double>());
    return nullptr;
}

}  // namespace

CefRefPtr<CefV8Value> jsonToV8(const nlohmann::json& j) {
    // Array + top-level-scalar support was added for the history-over-IPC move (2a).
    if (j.is_array()) {
        CefRefPtr<CefV8Value> arr = CefV8Value::CreateArray(static_cast<int>(j.size()));
        for (size_t i = 0; i < j.size(); ++i) {
            arr->SetValue(static_cast<int>(i), jsonToV8(j[i]));
        }
        return arr;
    }

    if (j.is_object()) {
        CefRefPtr<CefV8Value> obj = CefV8Value::CreateObject(nullptr, nullptr);
        for (auto it = j.begin(); it != j.end(); ++it) {
            const std::string& key = it.key();
            const auto& value = it.value();
            if (CefRefPtr<CefV8Value> scalar = jsonScalarToV8(value)) {
                obj->SetValue(key, scalar, V8_PROPERTY_ATTRIBUTE_NONE);
            } else {
                // Nested object/array. Kept as the .dump() string ON PURPOSE — the
                // existing identity.get / wallet-info callers parse it that way, and
                // recursing here would be a silent contract change for them. History
                // entries are flat, so this branch is not on the 2a path.
                obj->SetValue(key, CefV8Value::CreateString(value.dump()), V8_PROPERTY_ATTRIBUTE_NONE);
            }
        }
        return obj;
    }

    if (CefRefPtr<CefV8Value> scalar = jsonScalarToV8(j)) return scalar;
    return CefV8Value::CreateUndefined();
}

bool IdentityHandler::Execute(const CefString& name,
                               CefRefPtr<CefV8Value> object,
                               const CefV8ValueList& arguments,
                               CefRefPtr<CefV8Value>& retval,
                               CefString& exception) {
    std::cout << "IdentityHandler started - Function: " << name.ToString() << std::endl;
    std::cout.flush();

#ifdef _WIN32
    std::string debugMsg = "IdentityHandler started - Function: " + name.ToString();
    OutputDebugStringA(debugMsg.c_str());
    OutputDebugStringA("\n");
#endif

    // For identity.get(), first check if local identity file exists
    if (name == "get") {
#ifdef _WIN32
        const char* homeDir = std::getenv("USERPROFILE");
        std::string identityPath = std::string(homeDir ? homeDir : "") + "\\AppData\\Roaming\\" + AppPaths::GetAppDirName() + "\\identity.json";
#elif defined(__APPLE__)
        const char* homeDir = std::getenv("HOME");
        std::string identityPath = std::string(homeDir ? homeDir : "") + "/Library/Application Support/" + AppPaths::GetAppDirName() + "/identity.json";
#else
        const char* homeDir = std::getenv("HOME");
        std::string identityPath = std::string(homeDir ? homeDir : "") + "/.config/" + AppPaths::GetAppDirName() + "/identity.json";
#endif
        std::ifstream identityFile(identityPath);
        if (identityFile.good()) {
            std::cout << "Local identity file exists, reading from file" << std::endl;
            try {
                nlohmann::json identity;
                identityFile >> identity;
                identityFile.close();

                CefRefPtr<CefV8Value> identityObject = jsonToV8(identity);
                retval = identityObject;
                return true;
            } catch (const std::exception& e) {
                std::cerr << "Error reading identity file: " << e.what() << std::endl;
                identityFile.close();
                // Fall through to daemon check
            }
        } else {
            std::cout << "No local identity file found, will check daemon" << std::endl;
            identityFile.close();
        }
    }

    WalletService walletService;

    // Check if Go daemon is running
    if (!walletService.isConnected()) {
        std::cerr << "Cannot connect to Go wallet daemon. Make sure it's running on port 31301." << std::endl;
        exception = "Go wallet daemon is not running. Please start the wallet daemon first.";
        return false;
    }

    // Check daemon health
    if (!walletService.isHealthy()) {
        std::cerr << "Go wallet daemon is not healthy" << std::endl;
        exception = "Go wallet daemon is not responding properly.";
        return false;
    }

    if (name == "markBackedUp") {
        std::cout << "Marking wallet as backed up via Go daemon" << std::endl;

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
            std::cerr << "Failed to get wallet info from Go daemon" << std::endl;
            exception = "Failed to retrieve wallet info from Go wallet daemon.";
            return false;
        }

        std::cout << "Wallet info from Go daemon: " << walletInfo.dump() << std::endl;

        CefRefPtr<CefV8Value> walletObject = jsonToV8(walletInfo);
        retval = walletObject;

        return true;
    } catch (const std::exception& e) {
        std::cerr << "Error in IdentityHandler: " << e.what() << std::endl;
        exception = "Exception in IdentityHandler: " + std::string(e.what());
        return false;
    }
}
