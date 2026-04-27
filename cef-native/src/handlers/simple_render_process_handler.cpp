// cef_native/src/simple_render_process_handler.cpp
#include "../../include/handlers/simple_render_process_handler.h"

// V8 handlers (cross-platform)
#include "../../include/core/IdentityHandler.h"
#include "BRC100Handler.h"

// Cross-platform handlers (work on both platforms)
#include "../../include/core/NavigationHandler.h"
#include "../../include/core/HistoryManager.h"
#include "../../include/core/AddressHandler.h"
#include "../../include/core/AppPaths.h"

#include "wrapper/cef_helpers.h"
#include "include/cef_v8.h"
#include <iostream>
#include <fstream>
#include <cstdio>

#include "../../include/core/Logger.h"
#include "../../include/core/FingerprintScript.h"
#include "../../include/core/FingerprintProtection.h"

#include <unordered_map>
#include <unordered_set>
#include <mutex>

// Static cache for pre-loaded cosmetic scriptlets (8e-2).
// Browser process sends scriptlets via IPC before page loads;
// OnContextCreated injects them synchronously before page JS runs.
static std::mutex s_scriptCacheMutex;
static std::unordered_map<std::string, std::string> s_scriptCache; // URL → scriptlet JS

// Sprint 12c: Static cache for fingerprint seeds (URL → seed)
static std::mutex s_seedMutex;
static std::unordered_map<std::string, uint32_t> s_domainSeeds;

// Per-site fingerprint disable tracking: URLs for which the browser process
// has signalled that fingerprint protection should be skipped.
static std::mutex s_fpDisabledMutex;
static std::unordered_set<std::string> s_fingerprintDisabledUrls;

// Convenience macros for easier logging
#define LOG_DEBUG_RENDER(msg) Logger::Log(msg, 0, 1)
#define LOG_INFO_RENDER(msg) Logger::Log(msg, 1, 1)
#define LOG_WARNING_RENDER(msg) Logger::Log(msg, 2, 1)
#define LOG_ERROR_RENDER(msg) Logger::Log(msg, 3, 1)

// Helper function to escape JSON string for safe insertion into JavaScript
// Uses a simple approach: escape only the critical characters for single-quoted strings
static std::string escapeJsonForJs(const std::string& json) {
    std::string escaped;
    escaped.reserve(json.length() * 2); // Reserve space for worst case

    for (char c : json) {
        switch (c) {
            case '\\': escaped += "\\\\"; break;
            case '\'': escaped += "\\'"; break;  // Critical for single-quoted strings
            case '\n': escaped += "\\n"; break;
            case '\r': escaped += "\\r"; break;
            case '\t': escaped += "\\t"; break;
            case '\0': escaped += "\\0"; break;
            default:
                // For most characters, just append them
                // JSON should only contain valid UTF-8, so this should be safe
                if (static_cast<unsigned char>(c) >= 32 || c == '\t' || c == '\n' || c == '\r') {
                    escaped += c;
                } else {
                    // Escape other control characters as \xXX (snprintf avoids heap alloc — F7 perf fix)
                    char buf[5];
                    snprintf(buf, sizeof(buf), "\\x%02x",
                             static_cast<unsigned int>(static_cast<unsigned char>(c)));
                    escaped.append(buf, 4);
                }
                break;
        }
    }

    return escaped;
}

// Handler for cefMessage.send() function
class CefMessageSendHandler : public CefV8Handler {
public:
    CefMessageSendHandler() {}

    bool Execute(const CefString& name,
                 CefRefPtr<CefV8Value> object,
                 const CefV8ValueList& arguments,
                 CefRefPtr<CefV8Value>& retval,
                 CefString& exception) override {

        CEF_REQUIRE_RENDERER_THREAD();

        if (arguments.size() < 1) {
            exception = "cefMessage.send() requires at least one argument (message name)";
            return true;
        }

        std::string messageName = arguments[0]->GetStringValue();
        std::cout << "📤 cefMessage.send() called with message: " << messageName << std::endl;
        std::cout << "📤 Arguments count: " << arguments.size() << std::endl;

        // Try multiple logging approaches
        LOG_DEBUG_RENDER("📤 cefMessage.send() called with message: " + messageName);
        LOG_DEBUG_RENDER("📤 Arguments count: " + std::to_string(arguments.size()));

        // Also try writing to a different file
        std::ofstream testLog("test_debug.log", std::ios::app);
        testLog << "📤 cefMessage.send() called with message: " << messageName << std::endl;
        testLog.flush();
        testLog.close();

        // Create the process message
        CefRefPtr<CefProcessMessage> message = CefProcessMessage::Create(messageName);
        CefRefPtr<CefListValue> args = message->GetArgumentList();

        // Add arguments if provided (skip first argument which is the message name)
        for (size_t i = 1; i < arguments.size(); i++) {
            std::cout << "📤 Processing argument " << (i-1) << ": ";
            LOG_DEBUG_RENDER("📤 Processing argument " + std::to_string(i-1) + ": ");

            if (arguments[i]->IsString()) {
                std::string value = arguments[i]->GetStringValue();
                std::cout << "String: " << value << std::endl;
                LOG_DEBUG_RENDER("String: " + value);
                args->SetString(i - 1, value);
            } else if (arguments[i]->IsBool()) {
                bool value = arguments[i]->GetBoolValue();
                std::cout << "Bool: " << (value ? "true" : "false") << std::endl;
                LOG_DEBUG_RENDER("Bool: " + std::string(value ? "true" : "false"));
                args->SetBool(i - 1, value);
            } else if (arguments[i]->IsInt()) {
                int value = arguments[i]->GetIntValue();
                std::cout << "Int: " << value << std::endl;
                LOG_DEBUG_RENDER("Int: " + std::to_string(value));
                args->SetInt(i - 1, value);
            } else if (arguments[i]->IsDouble()) {
                double value = arguments[i]->GetDoubleValue();
                std::cout << "Double: " << value << std::endl;
                LOG_DEBUG_RENDER("Double: " + std::to_string(value));
                args->SetDouble(i - 1, value);
            } else if (arguments[i]->IsArray()) {
                // Expand ALL array elements into the message args list
                CefRefPtr<CefV8Value> array = arguments[i];
                std::cout << "Array with length: " << array->GetArrayLength() << std::endl;
                LOG_DEBUG_RENDER("Array with length: " + std::to_string(array->GetArrayLength()));
                for (int j = 0; j < array->GetArrayLength(); j++) {
                    CefRefPtr<CefV8Value> element = array->GetValue(j);
                    if (element->IsString()) {
                        std::string value = element->GetStringValue();
                        LOG_DEBUG_RENDER("Array[" + std::to_string(j) + "] String: " + value);
                        args->SetString(j, value);
                    } else if (element->IsBool()) {
                        bool value = element->GetBoolValue();
                        LOG_DEBUG_RENDER("Array[" + std::to_string(j) + "] Bool: " + std::string(value ? "true" : "false"));
                        args->SetBool(j, value);
                    } else if (element->IsInt()) {
                        int value = element->GetIntValue();
                        LOG_DEBUG_RENDER("Array[" + std::to_string(j) + "] Int: " + std::to_string(value));
                        args->SetInt(j, value);
                    } else if (element->IsDouble()) {
                        double value = element->GetDoubleValue();
                        LOG_DEBUG_RENDER("Array[" + std::to_string(j) + "] Double: " + std::to_string(value));
                        args->SetDouble(j, value);
                    }
                }
            } else {
                std::cout << "Unknown type" << std::endl;
                LOG_DEBUG_RENDER("Unknown type");
            }
        }

        // Send the message to the browser process
        CefRefPtr<CefV8Context> context = CefV8Context::GetCurrentContext();
        if (context && context->GetFrame()) {
            context->GetFrame()->SendProcessMessage(PID_BROWSER, message);
            std::cout << "✅ Process message sent to browser process: " << messageName << std::endl;
            LOG_DEBUG_RENDER("✅ Process message sent to browser process: " + messageName);
        } else {
            std::cout << "❌ Failed to get frame context for sending process message" << std::endl;
            LOG_ERROR_RENDER("❌ Failed to get frame context for sending process message");
        }

        return true;
    }

private:
    IMPLEMENT_REFCOUNTING(CefMessageSendHandler);
};

// Handler for overlay.close() function
class OverlayCloseHandler : public CefV8Handler {
public:
    OverlayCloseHandler() {}

    bool Execute(const CefString& name,
                 CefRefPtr<CefV8Value> object,
                 const CefV8ValueList& arguments,
                 CefRefPtr<CefV8Value>& retval,
                 CefString& exception) override {

        CEF_REQUIRE_RENDERER_THREAD();

        std::cout << "🎯 overlay.close() called from overlay browser" << std::endl;
        LOG_DEBUG_RENDER("🎯 overlay.close() called from overlay browser");

        // Send overlay_close message via cefMessage
        CefRefPtr<CefV8Context> context = CefV8Context::GetCurrentContext();
        if (context && context->GetFrame()) {
            CefRefPtr<CefProcessMessage> message = CefProcessMessage::Create("overlay_close");
            context->GetFrame()->SendProcessMessage(PID_BROWSER, message);

            std::cout << "✅ overlay.close() sent overlay_close message" << std::endl;
            LOG_DEBUG_RENDER("✅ overlay.close() sent overlay_close message");
        }

        return true;
    }

private:
    IMPLEMENT_REFCOUNTING(OverlayCloseHandler);
};

// Handler for omnibox overlay.close() - sends omnibox_hide message
class OmniboxCloseHandler : public CefV8Handler {
public:
    OmniboxCloseHandler() {}

    bool Execute(const CefString& name,
                 CefRefPtr<CefV8Value> object,
                 const CefV8ValueList& arguments,
                 CefRefPtr<CefV8Value>& retval,
                 CefString& exception) override {

        CEF_REQUIRE_RENDERER_THREAD();

        LOG_DEBUG_RENDER("🔍 omnibox overlay.close() called");

        // Send omnibox_hide message
        CefRefPtr<CefV8Context> context = CefV8Context::GetCurrentContext();
        if (context && context->GetFrame()) {
            CefRefPtr<CefProcessMessage> message = CefProcessMessage::Create("omnibox_hide");
            context->GetFrame()->SendProcessMessage(PID_BROWSER, message);
            LOG_DEBUG_RENDER("✅ omnibox overlay.close() sent omnibox_hide message");
        }

        return true;
    }

private:
    IMPLEMENT_REFCOUNTING(OmniboxCloseHandler);
};

// Handler for history operations (cross-platform)
class HistoryV8Handler : public CefV8Handler {
public:
    HistoryV8Handler() {}

    bool Execute(const CefString& name,
                 CefRefPtr<CefV8Value> object,
                 const CefV8ValueList& arguments,
                 CefRefPtr<CefV8Value>& retval,
                 CefString& exception) override {

        CEF_REQUIRE_RENDERER_THREAD();

        LOG_INFO_RENDER("📚 HistoryV8Handler::Execute called with name: " + name.ToString());

        auto& manager = HistoryManager::GetInstance();

        // Note: Database opens lazily on first access
        // Manager is always "initialized" even if database doesn't exist yet

        if (name == "get") {
            LOG_INFO_RENDER("📚 history.get() called - calling BROWSER process");

            // Get parameters
            int limit = 50;
            int offset = 0;

            if (arguments.size() > 0 && arguments[0]->IsObject()) {
                CefRefPtr<CefV8Value> params = arguments[0];
                if (params->HasValue("limit") && params->GetValue("limit")->IsInt()) {
                    limit = params->GetValue("limit")->GetIntValue();
                }
                if (params->HasValue("offset") && params->GetValue("offset")->IsInt()) {
                    offset = params->GetValue("offset")->GetIntValue();
                }
            }

            // For now, call GetInstance() which will be uninitialized in render process
            // This returns empty array - the proper fix is to use process messages
            // but that requires async callbacks which complicates the API
            auto entries = manager.GetHistory(limit, offset);

            // Convert to V8 array
            retval = CefV8Value::CreateArray(static_cast<int>(entries.size()));
            for (size_t i = 0; i < entries.size(); i++) {
                CefRefPtr<CefV8Value> entry_obj = CefV8Value::CreateObject(nullptr, nullptr);
                entry_obj->SetValue("url", CefV8Value::CreateString(entries[i].url), V8_PROPERTY_ATTRIBUTE_NONE);
                entry_obj->SetValue("title", CefV8Value::CreateString(entries[i].title), V8_PROPERTY_ATTRIBUTE_NONE);
                entry_obj->SetValue("visitCount", CefV8Value::CreateInt(entries[i].visit_count), V8_PROPERTY_ATTRIBUTE_NONE);
                entry_obj->SetValue("visitTime", CefV8Value::CreateDouble(static_cast<double>(entries[i].visit_time)), V8_PROPERTY_ATTRIBUTE_NONE);
                entry_obj->SetValue("transition", CefV8Value::CreateInt(entries[i].transition), V8_PROPERTY_ATTRIBUTE_NONE);

                retval->SetValue(static_cast<int>(i), entry_obj);
            }

            LOG_INFO_RENDER("📚 Returning " + std::to_string(entries.size()) + " entries from RENDER process");
            return true;
        }
        else if (name == "search") {
            // arguments[0] = { search, startTime, endTime, limit, offset }
            if (arguments.size() == 0 || !arguments[0]->IsObject()) {
                exception = "search() requires a parameters object";
                return true;
            }

            HistorySearchParams params;
            params.limit = 50;
            params.offset = 0;
            params.start_time = 0;
            params.end_time = 0;

            CefRefPtr<CefV8Value> search_params = arguments[0];

            if (search_params->HasValue("search") && search_params->GetValue("search")->IsString()) {
                params.search_term = search_params->GetValue("search")->GetStringValue().ToString();
            }

            if (search_params->HasValue("limit") && search_params->GetValue("limit")->IsInt()) {
                params.limit = search_params->GetValue("limit")->GetIntValue();
            }

            if (search_params->HasValue("offset") && search_params->GetValue("offset")->IsInt()) {
                params.offset = search_params->GetValue("offset")->GetIntValue();
            }

            if (search_params->HasValue("startTime") && search_params->GetValue("startTime")->IsDouble()) {
                params.start_time = static_cast<int64_t>(search_params->GetValue("startTime")->GetDoubleValue());
            }

            if (search_params->HasValue("endTime") && search_params->GetValue("endTime")->IsDouble()) {
                params.end_time = static_cast<int64_t>(search_params->GetValue("endTime")->GetDoubleValue());
            }

            std::cout << "🔍 history.search() called with term: " << params.search_term << std::endl;

            auto results = manager.SearchHistory(params);

            // Convert to V8 array
            retval = CefV8Value::CreateArray(static_cast<int>(results.size()));
            for (size_t i = 0; i < results.size(); i++) {
                CefRefPtr<CefV8Value> entry_obj = CefV8Value::CreateObject(nullptr, nullptr);
                entry_obj->SetValue("url", CefV8Value::CreateString(results[i].url), V8_PROPERTY_ATTRIBUTE_NONE);
                entry_obj->SetValue("title", CefV8Value::CreateString(results[i].title), V8_PROPERTY_ATTRIBUTE_NONE);
                entry_obj->SetValue("visitCount", CefV8Value::CreateInt(results[i].visit_count), V8_PROPERTY_ATTRIBUTE_NONE);
                entry_obj->SetValue("visitTime", CefV8Value::CreateDouble(static_cast<double>(results[i].visit_time)), V8_PROPERTY_ATTRIBUTE_NONE);

                retval->SetValue(static_cast<int>(i), entry_obj);
            }

            std::cout << "✅ Search returned " << results.size() << " entries" << std::endl;
            return true;
        }
        else if (name == "delete") {
            // arguments[0] = url string
            if (arguments.size() == 0 || !arguments[0]->IsString()) {
                exception = "delete() requires a URL string";
                return true;
            }

            std::string url = arguments[0]->GetStringValue().ToString();
            std::cout << "🗑️ history.delete() called for URL: " << url << std::endl;

            bool success = manager.DeleteHistoryEntry(url);
            retval = CefV8Value::CreateBool(success);

            return true;
        }
        else if (name == "clearAll") {
            std::cout << "🗑️ history.clearAll() called" << std::endl;

            bool success = manager.DeleteAllHistory();
            retval = CefV8Value::CreateBool(success);

            return true;
        }
        else if (name == "clearRange") {
            // arguments[0] = { startTime, endTime }
            if (arguments.size() == 0 || !arguments[0]->IsObject()) {
                exception = "clearRange() requires a parameters object with startTime and endTime";
                return true;
            }

            CefRefPtr<CefV8Value> params = arguments[0];

            if (!params->HasValue("startTime") || !params->HasValue("endTime")) {
                exception = "clearRange() requires startTime and endTime parameters";
                return true;
            }

            int64_t start_time = static_cast<int64_t>(params->GetValue("startTime")->GetDoubleValue());
            int64_t end_time = static_cast<int64_t>(params->GetValue("endTime")->GetDoubleValue());

            std::cout << "🗑️ history.clearRange() called" << std::endl;

            bool success = manager.DeleteHistoryRange(start_time, end_time);
            retval = CefV8Value::CreateBool(success);

            return true;
        }
        else if (name == "test") {
            LOG_INFO_RENDER("📚 history.test() called - running simple query");

            auto results = manager.GetHistorySimple(10);

            LOG_INFO_RENDER("📚 Simple query returned " + std::to_string(results.size()) + " entries");

            // Convert to V8 array
            retval = CefV8Value::CreateArray(static_cast<int>(results.size()));
            for (size_t i = 0; i < results.size(); i++) {
                CefRefPtr<CefV8Value> entry_obj = CefV8Value::CreateObject(nullptr, nullptr);
                entry_obj->SetValue("url", CefV8Value::CreateString(results[i].url), V8_PROPERTY_ATTRIBUTE_NONE);
                entry_obj->SetValue("title", CefV8Value::CreateString(results[i].title), V8_PROPERTY_ATTRIBUTE_NONE);
                entry_obj->SetValue("visitCount", CefV8Value::CreateInt(results[i].visit_count), V8_PROPERTY_ATTRIBUTE_NONE);

                retval->SetValue(static_cast<int>(i), entry_obj);
            }

            return true;
        }
        else if (name == "searchWithFrecency") {
            // arguments[0] = { query: string, limit?: number }
            if (arguments.size() == 0 || !arguments[0]->IsObject()) {
                exception = "searchWithFrecency() requires a parameters object with query";
                return true;
            }

            CefRefPtr<CefV8Value> params = arguments[0];

            std::string query = "";
            int limit = 6;

            if (params->HasValue("query") && params->GetValue("query")->IsString()) {
                query = params->GetValue("query")->GetStringValue().ToString();
            }

            if (params->HasValue("limit") && params->GetValue("limit")->IsInt()) {
                limit = params->GetValue("limit")->GetIntValue();
            }

            LOG_INFO_RENDER("🔍 history.searchWithFrecency() called with query: " + query);

            auto results = manager.SearchHistoryWithFrecency(query, limit);

            // Convert to V8 array with score
            retval = CefV8Value::CreateArray(static_cast<int>(results.size()));
            for (size_t i = 0; i < results.size(); i++) {
                CefRefPtr<CefV8Value> entry_obj = CefV8Value::CreateObject(nullptr, nullptr);
                entry_obj->SetValue("url", CefV8Value::CreateString(results[i].entry.url), V8_PROPERTY_ATTRIBUTE_NONE);
                entry_obj->SetValue("title", CefV8Value::CreateString(results[i].entry.title), V8_PROPERTY_ATTRIBUTE_NONE);
                entry_obj->SetValue("visitCount", CefV8Value::CreateInt(results[i].entry.visit_count), V8_PROPERTY_ATTRIBUTE_NONE);
                entry_obj->SetValue("lastVisitTime", CefV8Value::CreateDouble(static_cast<double>(results[i].entry.last_visit_time)), V8_PROPERTY_ATTRIBUTE_NONE);
                entry_obj->SetValue("frecencyScore", CefV8Value::CreateDouble(results[i].frecency_score), V8_PROPERTY_ATTRIBUTE_NONE);

                retval->SetValue(static_cast<int>(i), entry_obj);
            }

            LOG_INFO_RENDER("✅ searchWithFrecency returned " + std::to_string(results.size()) + " entries");
            return true;
        }

        return false;
    }

private:
    IMPLEMENT_REFCOUNTING(HistoryV8Handler);
};

// ========== GOOGLE SUGGEST V8 HANDLER ==========
// Handler for window.hodosBrowser.googleSuggest API
class GoogleSuggestV8Handler : public CefV8Handler {
public:
    GoogleSuggestV8Handler() : nextRequestId_(1) {}

    bool Execute(const CefString& name,
                 CefRefPtr<CefV8Value> object,
                 const CefV8ValueList& arguments,
                 CefRefPtr<CefV8Value>& retval,
                 CefString& exception) override {

        CEF_REQUIRE_RENDERER_THREAD();

        if (name == "fetch") {
            // Expect one argument: query string
            if (arguments.size() < 1 || !arguments[0]->IsString()) {
                exception = "fetch() requires one string argument (query)";
                return true;
            }

            std::string query = arguments[0]->GetStringValue();
            int requestId = nextRequestId_++;

            LOG_DEBUG_RENDER("🔍 googleSuggest.fetch() called with query: " + query + " (requestId: " + std::to_string(requestId) + ")");

            // Send IPC message to browser process with requestId
            CefRefPtr<CefProcessMessage> message = CefProcessMessage::Create("google_suggest_request");
            CefRefPtr<CefListValue> args = message->GetArgumentList();
            args->SetString(0, query);
            args->SetInt(1, requestId);

            CefRefPtr<CefV8Context> context = CefV8Context::GetCurrentContext();
            CefRefPtr<CefBrowser> browser = context->GetBrowser();
            browser->GetMainFrame()->SendProcessMessage(PID_BROWSER, message);

            LOG_DEBUG_RENDER("🔍 google_suggest_request sent to browser process with requestId: " + std::to_string(requestId));

            // Return the request ID so JavaScript can match responses
            retval = CefV8Value::CreateInt(requestId);
            return true;
        }

        return false;
    }

private:
    int nextRequestId_;
    IMPLEMENT_REFCOUNTING(GoogleSuggestV8Handler);
};

SimpleRenderProcessHandler::SimpleRenderProcessHandler() {
    LOG_DEBUG_RENDER("🔧 SimpleRenderProcessHandler constructor called!");

#ifdef _WIN32
    LOG_DEBUG_RENDER("🔧 Process ID: " + std::to_string(GetCurrentProcessId()));
    LOG_DEBUG_RENDER("🔧 Thread ID: " + std::to_string(GetCurrentThreadId()));

    // Initialize HistoryManager for render process (Windows-only)
    std::string appdata_path = std::getenv("APPDATA") ? std::getenv("APPDATA") : "";
    std::string user_data_path = appdata_path + "\\" + AppPaths::GetAppDirName() + "\\Default";

    LOG_DEBUG_RENDER("🔧 Initializing HistoryManager in RENDER process");
    if (HistoryManager::GetInstance().Initialize(user_data_path)) {
        LOG_DEBUG_RENDER("✅ HistoryManager initialized in RENDER process");
    } else {
        LOG_ERROR_RENDER("❌ Failed to initialize HistoryManager in RENDER process");
    }
#else
    LOG_DEBUG_RENDER("🔧 HistoryManager not available on macOS - stubbed");
#endif
}

void SimpleRenderProcessHandler::OnContextCreated(
    CefRefPtr<CefBrowser> browser,
    CefRefPtr<CefFrame> frame,
    CefRefPtr<CefV8Context> context) {

    CEF_REQUIRE_RENDERER_THREAD();

    LOG_DEBUG_RENDER("🔧 OnContextCreated called for browser ID: " + std::to_string(browser->GetIdentifier()));
    LOG_DEBUG_RENDER("🔧 Frame URL: " + frame->GetURL().ToString());
#ifdef _WIN32
    LOG_DEBUG_RENDER("🔧 Process ID: " + std::to_string(GetCurrentProcessId()));
    LOG_DEBUG_RENDER("🔧 Thread ID: " + std::to_string(GetCurrentThreadId()));
#endif
    LOG_DEBUG_RENDER("🔧 RENDER PROCESS HANDLER IS WORKING!");
    LOG_DEBUG_RENDER("🔧 THIS IS THE RENDER PROCESS HANDLER!");

    // 8e-2: Inject pre-cached scriptlets IMMEDIATELY — before any page JS runs.
    // This is the earliest possible injection point in CEF.
    std::string url = frame->GetURL().ToString();
    if (!url.empty() && url.find("127.0.0.1") == std::string::npos) {
        std::lock_guard<std::mutex> lock(s_scriptCacheMutex);
        auto it = s_scriptCache.find(url);
        if (it != s_scriptCache.end() && !it->second.empty()) {
            LOG_INFO_RENDER("💉 OnContextCreated: injecting scriptlets for " + url +
                " (" + std::to_string(it->second.size()) + " chars)");
            frame->ExecuteJavaScript(it->second, url, 0);
            s_scriptCache.erase(it); // One-shot: don't re-inject on subframe contexts
        }
    }

    // Sprint 12d: Inject fingerprint protection script for external pages
    // Skip auth domains and per-site disabled URLs.
    if (!url.empty() && url.find("127.0.0.1") == std::string::npos &&
        url.find("localhost") == std::string::npos &&
        !FingerprintProtection::IsAuthDomain(url)) {

        // Check if the browser process sent a disable signal for this URL
        bool fpDisabled = false;
        {
            std::lock_guard<std::mutex> lock(s_fpDisabledMutex);
            auto disabledIt = s_fingerprintDisabledUrls.find(url);
            if (disabledIt != s_fingerprintDisabledUrls.end()) {
                fpDisabled = true;
                if (frame->IsMain()) {
                    s_fingerprintDisabledUrls.erase(disabledIt); // One-shot for main frame
                }
                LOG_DEBUG_RENDER("🛡️ Fingerprint injection skipped (site disabled) for " + url);
            }
        }

        if (!fpDisabled) {
        uint32_t seed = 0;
        {
            std::lock_guard<std::mutex> lock(s_seedMutex);
            auto it = s_domainSeeds.find(url);
            if (it != s_domainSeeds.end()) {
                seed = it->second;
                if (frame->IsMain()) {
                    s_domainSeeds.erase(it); // One-shot for main frame
                }
            } else {
                // Fallback: use URL hash as seed
                seed = static_cast<uint32_t>(std::hash<std::string>{}(url) & 0xFFFFFFFF);
            }
        }
        if (seed != 0) {
            std::string script = FINGERPRINT_PROTECTION_SCRIPT;
            std::string seedStr = std::to_string(seed);
            size_t pos = script.find("FINGERPRINT_SEED");
            if (pos != std::string::npos) {
                script.replace(pos, 16, seedStr);
            }
            LOG_DEBUG_RENDER("🛡️ Injecting fingerprint protection (seed=" + seedStr + ") for " + url);
            frame->ExecuteJavaScript(script, url, 0);
        }
        } // end !fpDisabled
    }

    // Inject window.chrome stub on external pages so bot detection sees a real Chrome signal.
    // Injected separately from fingerprint script so it works even when FP protection is disabled.
    bool isExternalPage = !url.empty() &&
        url.find("127.0.0.1") == std::string::npos &&
        url.find("localhost") == std::string::npos;
    if (isExternalPage) {
        std::string chromeStub = R"JS(
(function() {
    'use strict';
    if (typeof window.chrome === 'undefined') {
        window.chrome = {
            runtime: {
                connect: function() { return {}; },
                sendMessage: function() {},
                onMessage: { addListener: function() {}, removeListener: function() {} },
                id: undefined
            },
            loadTimes: function() { return {}; },
            csi: function() { return {}; }
        };
    }
})();
)JS";
        frame->ExecuteJavaScript(chromeStub, url, 0);
    }

    // Check if this is an overlay browser (any browser that's not the main root browser)
    bool isMainBrowser = (url == "http://127.0.0.1:5137" || url == "http://127.0.0.1:5137/");
    bool isOverlayBrowser = !isMainBrowser && url.find("127.0.0.1:5137") != std::string::npos;
    bool isOmniboxOverlay = (url.find("/omnibox") != std::string::npos);
    bool isInternalPage = (url.find("127.0.0.1:5137") != std::string::npos);

    if (isOverlayBrowser) {
        LOG_DEBUG_RENDER("🎯 OVERLAY BROWSER V8 CONTEXT CREATED!");
        LOG_DEBUG_RENDER("🎯 URL: " + url);
        LOG_DEBUG_RENDER("🎯 Setting up hodosBrowser for overlay browser");
        if (isOmniboxOverlay) {
            LOG_DEBUG_RENDER("🔍 Detected omnibox overlay - will inject overlay.close()");
        }
    }

    CefRefPtr<CefV8Value> global = context->GetGlobal();

    // Create the hodosBrowser object — available on all pages for BRC-100 protocol.
    // On external pages, only expose brc100 sub-object to minimize fingerprint surface.
    CefRefPtr<CefV8Value> hodosBrowser = CefV8Value::CreateObject(nullptr, nullptr);
    global->SetValue("hodosBrowser", hodosBrowser, V8_PROPERTY_ATTRIBUTE_READONLY);

    // Expose the host OS to React so we can conditionally render Windows vs
    // macOS chrome (traffic lights vs our own min/max/close, tab bar padding,
    // etc.). Always injected, even on external pages — it's a bare string and
    // not fingerprint-sensitive beyond what the user-agent already reveals.
#if defined(__APPLE__)
    hodosBrowser->SetValue("platform",
        CefV8Value::CreateString("macos"),
        V8_PROPERTY_ATTRIBUTE_READONLY);
#else
    hodosBrowser->SetValue("platform",
        CefV8Value::CreateString("windows"),
        V8_PROPERTY_ATTRIBUTE_READONLY);
#endif

    // Identity, navigation, address, history, overlay APIs — internal pages only.
    // External pages get only BRC-100 + cefMessage (injected below).
    if (isInternalPage || isOverlayBrowser) {

    // Create the identity object inside hodosBrowser
    CefRefPtr<CefV8Value> identityObject = CefV8Value::CreateObject(nullptr, nullptr);
    hodosBrowser->SetValue("identity", identityObject, V8_PROPERTY_ATTRIBUTE_READONLY);

    // Bind the IdentityHandler instance
    CefRefPtr<IdentityHandler> identityHandler = new IdentityHandler();

    identityObject->SetValue("get",
        CefV8Value::CreateFunction("get", identityHandler),
        V8_PROPERTY_ATTRIBUTE_NONE);

    identityObject->SetValue("markBackedUp",
        CefV8Value::CreateFunction("markBackedUp", identityHandler),
        V8_PROPERTY_ATTRIBUTE_NONE);

#ifdef _WIN32
    // Create the navigation object inside hodosBrowser (Windows-only)
    CefRefPtr<CefV8Value> navigationObject = CefV8Value::CreateObject(nullptr, nullptr);
    hodosBrowser->SetValue("navigation", navigationObject, V8_PROPERTY_ATTRIBUTE_READONLY);

    // Bind the NavigationHandler instance
    CefRefPtr<NavigationHandler> navigationHandler = new NavigationHandler();

    navigationObject->SetValue("navigate",
        CefV8Value::CreateFunction("navigate", navigationHandler),
        V8_PROPERTY_ATTRIBUTE_NONE);
#else
    // macOS: Navigation handler is cross-platform, inject it
    std::cout << "🔧 macOS: Injecting navigation API..." << std::endl;
    LOG_DEBUG_RENDER("🔧 macOS: Injecting navigation API...");

    CefRefPtr<CefV8Value> navigationObject = CefV8Value::CreateObject(nullptr, nullptr);
    if (!navigationObject) {
        std::cout << "❌ Failed to create navigationObject!" << std::endl;
        LOG_ERROR_RENDER("❌ Failed to create navigationObject!");
    } else {
        std::cout << "✅ navigationObject created" << std::endl;
    }

    bool setResult = hodosBrowser->SetValue("navigation", navigationObject, V8_PROPERTY_ATTRIBUTE_READONLY);
    std::cout << "🔧 SetValue('navigation') result: " << setResult << std::endl;

    CefRefPtr<NavigationHandler> navHandler = new NavigationHandler();
    CefRefPtr<CefV8Value> navFunction = CefV8Value::CreateFunction("navigate", navHandler);

    bool setFuncResult = navigationObject->SetValue("navigate", navFunction, V8_PROPERTY_ATTRIBUTE_NONE);
    std::cout << "🔧 SetValue('navigate' function) result: " << setFuncResult << std::endl;

    LOG_DEBUG_RENDER("✅ Navigation API injection completed on macOS");
    std::cout << "✅ Navigation API injection completed on macOS" << std::endl;
#endif

    // overlayPanel object removed - now using process-per-overlay architecture

    // Create the overlay object (for overlay browsers only)
    if (isOverlayBrowser) {
        LOG_DEBUG_RENDER("🎯 Creating overlay object for URL: " + url);

        CefRefPtr<CefV8Value> overlayObject = CefV8Value::CreateObject(nullptr, nullptr);
        hodosBrowser->SetValue("overlay", overlayObject, V8_PROPERTY_ATTRIBUTE_READONLY);

        // Add close method for overlay browsers
        // Omnibox overlay sends "omnibox_hide", other overlays send "overlay_close"
        if (isOmniboxOverlay) {
            overlayObject->SetValue("close",
                CefV8Value::CreateFunction("close", new OmniboxCloseHandler()),
                V8_PROPERTY_ATTRIBUTE_NONE);
            LOG_DEBUG_RENDER("🔍 Omnibox overlay.close() injected (sends omnibox_hide)");
        } else {
            overlayObject->SetValue("close",
                CefV8Value::CreateFunction("close", new OverlayCloseHandler()),
                V8_PROPERTY_ATTRIBUTE_NONE);
        }

        LOG_DEBUG_RENDER("🎯 Overlay object created with close method");
    } else {
        LOG_DEBUG_RENDER("🎯 NOT creating overlay object for URL: " + url);
        LOG_DEBUG_RENDER("🎯 isMainBrowser: " + std::string(isMainBrowser ? "true" : "false"));
    }

#ifdef _WIN32
    // Create the address object (Windows)
    CefRefPtr<CefV8Value> addressObject = CefV8Value::CreateObject(nullptr, nullptr);
    hodosBrowser->SetValue("address", addressObject, V8_PROPERTY_ATTRIBUTE_READONLY);

    // Bind AddressHandler
    CefRefPtr<AddressHandler> addressHandler = new AddressHandler();
    addressObject->SetValue("generate",
        CefV8Value::CreateFunction("generate", addressHandler),
        V8_PROPERTY_ATTRIBUTE_NONE);
#else
    // macOS: AddressHandler is cross-platform now
    CefRefPtr<CefV8Value> addressObject = CefV8Value::CreateObject(nullptr, nullptr);
    hodosBrowser->SetValue("address", addressObject, V8_PROPERTY_ATTRIBUTE_READONLY);

    CefRefPtr<AddressHandler> addressHandler = new AddressHandler();

    addressObject->SetValue("generate",
        CefV8Value::CreateFunction("generate", addressHandler),
        V8_PROPERTY_ATTRIBUTE_NONE);

    addressObject->SetValue("getAll",
        CefV8Value::CreateFunction("getAll", addressHandler),
        V8_PROPERTY_ATTRIBUTE_NONE);

    addressObject->SetValue("getCurrent",
        CefV8Value::CreateFunction("getCurrent", addressHandler),
        V8_PROPERTY_ATTRIBUTE_NONE);

    LOG_DEBUG_RENDER("✅ Address API enabled on macOS");
#endif

    // Create the history object (cross-platform)
    LOG_DEBUG_RENDER("📚 Creating history object for V8 context");
    CefRefPtr<CefV8Value> historyObject = CefV8Value::CreateObject(nullptr, nullptr);
    hodosBrowser->SetValue("history", historyObject, V8_PROPERTY_ATTRIBUTE_READONLY);

    // Bind HistoryV8Handler
    LOG_DEBUG_RENDER("📚 Binding HistoryV8Handler functions");
    CefRefPtr<HistoryV8Handler> historyHandler = new HistoryV8Handler();
    historyObject->SetValue("get",
        CefV8Value::CreateFunction("get", historyHandler),
        V8_PROPERTY_ATTRIBUTE_NONE);
    historyObject->SetValue("search",
        CefV8Value::CreateFunction("search", historyHandler),
        V8_PROPERTY_ATTRIBUTE_NONE);
    historyObject->SetValue("searchWithFrecency",
        CefV8Value::CreateFunction("searchWithFrecency", historyHandler),
        V8_PROPERTY_ATTRIBUTE_NONE);
    historyObject->SetValue("delete",
        CefV8Value::CreateFunction("delete", historyHandler),
        V8_PROPERTY_ATTRIBUTE_NONE);
    historyObject->SetValue("clearAll",
        CefV8Value::CreateFunction("clearAll", historyHandler),
        V8_PROPERTY_ATTRIBUTE_NONE);
    historyObject->SetValue("clearRange",
        CefV8Value::CreateFunction("clearRange", historyHandler),
        V8_PROPERTY_ATTRIBUTE_NONE);
    historyObject->SetValue("test",
        CefV8Value::CreateFunction("test", historyHandler),
        V8_PROPERTY_ATTRIBUTE_NONE);

    LOG_DEBUG_RENDER("📚 History object created with " + std::to_string(7) + " functions");

    } // end isInternalPage || isOverlayBrowser — external pages only get BRC-100 + cefMessage

    // Create the cefMessage object for process communication
    CefRefPtr<CefV8Value> cefMessageObject = CefV8Value::CreateObject(nullptr, nullptr);
    global->SetValue("cefMessage", cefMessageObject, V8_PROPERTY_ATTRIBUTE_READONLY);

    // Create the send function for cefMessage
    CefRefPtr<CefV8Value> sendFunction = CefV8Value::CreateFunction("send", new CefMessageSendHandler());
    cefMessageObject->SetValue("send", sendFunction, V8_PROPERTY_ATTRIBUTE_NONE);

    // Inject Google Suggest API for omnibox overlay only
    if (isOmniboxOverlay) {
        CefRefPtr<CefV8Value> googleSuggestObject = CefV8Value::CreateObject(nullptr, nullptr);
        hodosBrowser->SetValue("googleSuggest", googleSuggestObject, V8_PROPERTY_ATTRIBUTE_READONLY);

        CefRefPtr<GoogleSuggestV8Handler> googleSuggestHandler = new GoogleSuggestV8Handler();
        googleSuggestObject->SetValue("fetch",
            CefV8Value::CreateFunction("fetch", googleSuggestHandler),
            V8_PROPERTY_ATTRIBUTE_NONE);

        LOG_DEBUG_RENDER("🔍 Google Suggest API injected for omnibox overlay");
    }

    // Register BRC-100 API (cross-platform)
    BRC100Handler::RegisterBRC100API(context);

    // For overlay browsers, signal that all systems are ready
    if (isOverlayBrowser) {
        std::string js = R"(
            console.log("🎯 All systems ready - V8 context created, APIs injected");
            // Set a flag that all systems are ready
            window.allSystemsReady = true;
            // Dispatch a custom event to signal all systems are ready
            window.dispatchEvent(new CustomEvent('allSystemsReady'));
            console.log("🎯 allSystemsReady event dispatched");
        )";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);

        LOG_DEBUG_RENDER("🎯 All systems ready - V8 context created, APIs injected");
    }
}

bool SimpleRenderProcessHandler::OnProcessMessageReceived(
    CefRefPtr<CefBrowser> browser,
    CefRefPtr<CefFrame> frame,
    CefProcessId source_process,
    CefRefPtr<CefProcessMessage> message) {

    CEF_REQUIRE_RENDERER_THREAD();

    std::string message_name = message->GetName();
    std::cout << "📨 Render process received message: " << message_name << std::endl;
    std::cout << "🔍 Browser ID: " << browser->GetIdentifier() << std::endl;
    std::cout << "🔍 Frame URL: " << frame->GetURL().ToString() << std::endl;
    std::cout << "🔍 Source Process: " << source_process << std::endl;

        if (message_name == "tab_list_response") {
            CefRefPtr<CefListValue> args = message->GetArgumentList();
            std::string tabListJson = args->GetString(0);

            LOG_DEBUG_RENDER("📑 Tab list response received, dispatching to React");

            // Escape the JSON string for JavaScript
            std::string escaped_json = tabListJson;
            size_t pos = 0;
            while ((pos = escaped_json.find("\\", pos)) != std::string::npos) {
                escaped_json.replace(pos, 1, "\\\\");
                pos += 2;
            }
            pos = 0;
            while ((pos = escaped_json.find("\"", pos)) != std::string::npos) {
                escaped_json.replace(pos, 1, "\\\"");
                pos += 2;
            }

            // Send message to React component
            std::string js = R"(
                window.dispatchEvent(new MessageEvent('message', {
                    data: {
                        type: 'tab_list_response',
                        data: ')" + escaped_json + R"('
                    }
                }));
            )";
            frame->ExecuteJavaScript(js, frame->GetURL(), 0);
            return true;
        }

        // ========== FIND IN PAGE ==========
        if (message_name == "find_show") {
            LOG_DEBUG_RENDER("🔍 find_show received, dispatching to React");
            std::string js = R"(
                console.log('[CEF] Executing find_show JS in frame');
                window.postMessage({ type: 'find_show' }, '*');
            )";
            frame->ExecuteJavaScript(js, frame->GetURL(), 0);
            return true;
        }

        if (message_name == "focus_address_bar") {
            LOG_DEBUG_RENDER("⌨️ focus_address_bar received, dispatching to React");
            std::string js = R"(
                window.dispatchEvent(new MessageEvent('message', {
                    data: { type: 'focus_address_bar' }
                }));
            )";
            frame->ExecuteJavaScript(js, frame->GetURL(), 0);
            return true;
        }

        if (message_name == "find_result") {
            CefRefPtr<CefListValue> args = message->GetArgumentList();
            std::string resultJson = args->GetString(0);

            LOG_DEBUG_RENDER("🔍 find_result received, dispatching to React");

            std::string escaped = escapeJsonForJs(resultJson);
            std::string js = R"(
                window.dispatchEvent(new MessageEvent('message', {
                    data: {
                        type: 'find_result',
                        data: ')" + escaped + R"('
                    }
                }));
            )";
            frame->ExecuteJavaScript(js, frame->GetURL(), 0);
            return true;
        }

        if (message_name == "qr_scan_result") {
            // QR scan results forwarded from the active page back to the wallet overlay.
            // The JSON is a stringified array produced by our own scanner script (not user input).
            std::string json = message->GetArgumentList()->GetString(0).ToString();
            LOG_INFO_RENDER("📷 qr_scan_result dispatching to React (" + std::to_string(json.size()) + " chars)");

            std::string js = "window.dispatchEvent(new MessageEvent('message',{data:{type:'qr_scan_result',data:" + json + "}}));";
            frame->ExecuteJavaScript(js, frame->GetURL(), 0);
            return true;
        }

        if (message_name == "download_state_update") {
            CefRefPtr<CefListValue> args = message->GetArgumentList();
            std::string downloadsJson = args->GetString(0);

            LOG_DEBUG_RENDER("📥 Download state update received, dispatching to React");

            std::string escaped = escapeJsonForJs(downloadsJson);
            std::string js = R"(
                window.dispatchEvent(new MessageEvent('message', {
                    data: {
                        type: 'download_state_update',
                        data: ')" + escaped + R"('
                    }
                }));
            )";
            frame->ExecuteJavaScript(js, frame->GetURL(), 0);
            return true;
        }

        // ========== PAYMENT SUCCESS INDICATOR (tab badge) ==========
        if (message_name == "payment_success_indicator") {
            CefRefPtr<CefListValue> args = message->GetArgumentList();
            std::string payloadJson = args->GetString(0);

            LOG_DEBUG_RENDER("💰 Payment success indicator received, dispatching to React");

            std::string escaped = escapeJsonForJs(payloadJson);
            std::string js = R"(
                window.dispatchEvent(new MessageEvent('message', {
                    data: {
                        type: 'payment_success_indicator',
                        data: ')" + escaped + R"('
                    }
                }));
            )";
            frame->ExecuteJavaScript(js, frame->GetURL(), 0);
            return true;
        }

        // ========== DOWNLOAD FOLDER PICKER RESULT ==========
        if (message_name == "download_folder_selected") {
            CefRefPtr<CefListValue> args = message->GetArgumentList();
            std::string path = args->GetString(0);
            std::string escaped = escapeJsonForJs(path);

            LOG_DEBUG_RENDER("📂 download_folder_selected: " + path);

            std::string js = "if (window.onDownloadFolderSelected) { window.onDownloadFolderSelected('" + escaped + "'); }";
            frame->ExecuteJavaScript(js, frame->GetURL(), 0);
            return true;
        }

        // ========== NEW TAB PAGE (G4) ==========

        if (message_name == "most_visited_response") {
            CefRefPtr<CefListValue> args = message->GetArgumentList();
            std::string jsonStr = args->GetString(0);
            std::string escaped = escapeJsonForJs(jsonStr);

            std::string js = R"(
                window.dispatchEvent(new MessageEvent('message', {
                    data: {
                        type: 'most_visited_response',
                        data: ')" + escaped + R"('
                    }
                }));
            )";
            frame->ExecuteJavaScript(js, frame->GetURL(), 0);
            return true;
        }

        if (message_name == "session_blocked_total_response") {
            CefRefPtr<CefListValue> args = message->GetArgumentList();
            std::string jsonStr = args->GetString(0);
            std::string escaped = escapeJsonForJs(jsonStr);

            std::string js = R"(
                window.dispatchEvent(new MessageEvent('message', {
                    data: {
                        type: 'session_blocked_total_response',
                        data: ')" + escaped + R"('
                    }
                }));
            )";
            frame->ExecuteJavaScript(js, frame->GetURL(), 0);
            return true;
        }

        // ========== COSMETIC FILTERING (Sprint 8e) ==========

        // 8e-2: Pre-cache scriptlets for early injection in OnContextCreated
        if (message_name == "preload_cosmetic_script") {
            CefRefPtr<CefListValue> args = message->GetArgumentList();
            std::string url = args->GetString(0);
            std::string script = args->GetString(1);

            if (!url.empty() && !script.empty()) {
                std::lock_guard<std::mutex> lock(s_scriptCacheMutex);
                s_scriptCache[url] = script;
                LOG_INFO_RENDER("💉 Pre-cached scriptlets for " + url +
                    " (" + std::to_string(script.size()) + " chars)");
            }
            return true;
        }

        // Sprint 12c: Cache fingerprint seed for injection in OnContextCreated
        if (message_name == "fingerprint_seed") {
            CefRefPtr<CefListValue> args = message->GetArgumentList();
            uint32_t seed = static_cast<uint32_t>(args->GetInt(0));
            std::string url = args->GetString(1).ToString();

            if (!url.empty() && seed != 0) {
                std::lock_guard<std::mutex> lock(s_seedMutex);
                s_domainSeeds[url] = seed;
                LOG_DEBUG_RENDER("🛡️ Cached fingerprint seed " + std::to_string(seed) + " for " + url);
            }
            return true;
        }

        // Per-site fingerprint disable: browser process signals this URL should
        // skip fingerprint injection (auth domain or user-disabled site).
        if (message_name == "fingerprint_site_disabled") {
            CefRefPtr<CefListValue> args = message->GetArgumentList();
            std::string url = args->GetString(0).ToString();
            if (!url.empty()) {
                std::lock_guard<std::mutex> lock(s_fpDisabledMutex);
                s_fingerprintDisabledUrls.insert(url);
                LOG_DEBUG_RENDER("🛡️ Fingerprint disabled for URL: " + url);
            }
            return true;
        }

        if (message_name == "inject_cosmetic_css") {
            CefRefPtr<CefListValue> args = message->GetArgumentList();
            std::string selectors = args->GetString(0);

            if (!selectors.empty()) {
                LOG_DEBUG_RENDER("🎨 Injecting cosmetic CSS (" + std::to_string(selectors.size()) + " chars)");

                // Escape selectors for safe JS string embedding
                std::string escaped;
                escaped.reserve(selectors.size() + 64);
                for (char c : selectors) {
                    switch (c) {
                        case '\\': escaped += "\\\\"; break;
                        case '\'': escaped += "\\'"; break;
                        case '\n': escaped += "\\n"; break;
                        case '\r': escaped += "\\r"; break;
                        default: escaped += c; break;
                    }
                }

                // Inject or append to <style> tag to hide matched elements
                std::string js = R"(
                    (function() {
                        var rule = ')" + escaped + R"( { display: none !important; }';
                        var existing = document.getElementById('hodos-cosmetic-css');
                        if (existing) {
                            existing.textContent += '\n' + rule;
                        } else {
                            var style = document.createElement('style');
                            style.id = 'hodos-cosmetic-css';
                            style.textContent = rule;
                            (document.head || document.documentElement).appendChild(style);
                        }
                    })();
                )";
                frame->ExecuteJavaScript(js, frame->GetURL(), 0);
            }
            return true;
        }

        if (message_name == "inject_cosmetic_script") {
            CefRefPtr<CefListValue> args = message->GetArgumentList();
            std::string script = args->GetString(0);

            if (!script.empty()) {
                LOG_DEBUG_RENDER("💉 Injecting cosmetic scriptlets (" + std::to_string(script.size()) + " chars)");

                // Execute scriptlets directly — they are self-contained JS from adblock engine
                frame->ExecuteJavaScript(script, "about:blank", 0);
            }
            return true;
        }

        if (message_name == "brc100_auth_request") {
            CefRefPtr<CefListValue> args = message->GetArgumentList();
            std::string domain = args->GetString(0);
            std::string method = args->GetString(1);
            std::string endpoint = args->GetString(2);
            std::string body = args->GetString(3);
            std::string notifType = (args->GetSize() >= 6) ? args->GetString(5).ToString() : "domain_approval";

            LOG_DEBUG_RENDER("🔐 BRC-100 auth request received: " + domain + " type=" + notifType);

            // Send message to React component
            std::string js = R"(
                window.dispatchEvent(new MessageEvent('message', {
                    data: {
                        type: 'brc100_auth_request',
                        payload: {
                            domain: ')" + domain + R"(',
                            method: ')" + method + R"(',
                            endpoint: ')" + endpoint + R"(',
                            body: ')" + body + R"(',
                            notificationType: ')" + notifType + R"('
                        }
                    }
                }));
            )";
            frame->ExecuteJavaScript(js, frame->GetURL(), 0);
            return true;
        }

        if (message_name == "address_generate_response") {
            CefRefPtr<CefListValue> args = message->GetArgumentList();
            std::string addressDataJson = args->GetString(0);

            std::cout << "✅ Address generation response received: " << addressDataJson << std::endl;
            LOG_DEBUG_RENDER("✅ Address generation response received: " + addressDataJson);

            // Execute JavaScript to call the callback function directly
            std::string js = "if (window.onAddressGenerated) { window.onAddressGenerated(" + addressDataJson + "); }";
            frame->ExecuteJavaScript(js, frame->GetURL(), 0);

            return true;
        }

    if (message_name == "identity_status_check_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string responseJson = args->GetString(0);

        std::cout << "✅ Identity status check response received: " << responseJson << std::endl;
        std::cout << "🔍 Browser ID: " << browser->GetIdentifier() << std::endl;
        std::cout << "🔍 Frame URL: " << frame->GetURL().ToString() << std::endl;

        // Execute JavaScript to dispatch the response event
        std::string js = "window.dispatchEvent(new CustomEvent('cefMessageResponse', { detail: { message: 'identity_status_check_response', args: ['" + responseJson + "'] } }));";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);

        return true;
    }

    if (message_name == "create_identity_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string responseJson = args->GetString(0);

        std::cout << "✅ Create identity response received: " << responseJson << std::endl;
        std::cout << "🔍 Browser ID: " << browser->GetIdentifier() << std::endl;
        std::cout << "🔍 Frame URL: " << frame->GetURL().ToString() << std::endl;

        // Execute JavaScript to dispatch the response event
        std::string js = "window.dispatchEvent(new CustomEvent('cefMessageResponse', { detail: { message: 'create_identity_response', args: ['" + responseJson + "'] } }));";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);

        return true;
    }

    if (message_name == "mark_identity_backed_up_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string responseJson = args->GetString(0);

        std::cout << "✅ Mark identity backed up response received: " << responseJson << std::endl;
        std::cout << "🔍 Browser ID: " << browser->GetIdentifier() << std::endl;
        std::cout << "🔍 Frame URL: " << frame->GetURL().ToString() << std::endl;

        // Execute JavaScript to dispatch the response event
        std::string js = "window.dispatchEvent(new CustomEvent('cefMessageResponse', { detail: { message: 'mark_identity_backed_up_response', args: ['" + responseJson + "'] } }));";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);

        return true;
    }

    if (message_name == "address_generate_error") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string errorMessage = args->GetString(0);

        std::cout << "❌ Address generation error received: " << errorMessage << std::endl;

        // Execute JavaScript to handle the error
        std::string js = "if (window.onAddressError) { window.onAddressError('" + errorMessage + "'); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);

        return true;
    }

    // Transaction Response Handlers

        if (message_name == "address_generate_response") {
            CefRefPtr<CefListValue> args = message->GetArgumentList();
            std::string responseJson = args->GetString(0);

            std::cout << "✅ Address generation response received: " << responseJson << std::endl;
            LOG_DEBUG_RENDER("✅ Address generation response received: " + responseJson);

            // Execute JavaScript to call the callback function directly
            std::string js = "if (window.onAddressGenerated) { window.onAddressGenerated(" + responseJson + "); }";
            frame->ExecuteJavaScript(js, frame->GetURL(), 0);

            return true;
        }

        if (message_name == "address_generate_error") {
            CefRefPtr<CefListValue> args = message->GetArgumentList();
            std::string errorJson = args->GetString(0);

            std::cout << "❌ Address generation error received: " << errorJson << std::endl;
            LOG_DEBUG_RENDER("❌ Address generation error received: " + errorJson);

            // Execute JavaScript to call the error callback function directly
            std::string js = "if (window.onAddressError) { window.onAddressError(" + errorJson + "); }";
            frame->ExecuteJavaScript(js, frame->GetURL(), 0);

            return true;
        }

        if (message_name == "create_transaction_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string responseJson = args->GetString(0);

        std::cout << "✅ Create transaction response received: " << responseJson << std::endl;
        std::cout << "🔍 Browser ID: " << browser->GetIdentifier() << std::endl;
        std::cout << "🔍 Frame URL: " << frame->GetURL().ToString() << std::endl;
        LOG_DEBUG_RENDER("✅ Create transaction response received: " + responseJson);
        LOG_DEBUG_RENDER("🔍 Browser ID: " + std::to_string(browser->GetIdentifier()));
        LOG_DEBUG_RENDER("🔍 Frame URL: " + frame->GetURL().ToString());

        // Execute JavaScript to call the callback function directly
        std::string js = "if (window.onCreateTransactionResponse) { window.onCreateTransactionResponse(" + responseJson + "); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);

        return true;
    }

    if (message_name == "create_transaction_error") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string errorMessage = args->GetString(0);

        std::cout << "❌ Create transaction error received: " << errorMessage << std::endl;
        LOG_DEBUG_RENDER("❌ Create transaction error received: " + errorMessage);

        // Execute JavaScript to handle the error
        std::string js = "if (window.onCreateTransactionError) { window.onCreateTransactionError('" + errorMessage + "'); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);

        return true;
    }

    if (message_name == "sign_transaction_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string responseJson = args->GetString(0);

        std::cout << "✅ Sign transaction response received: " << responseJson << std::endl;
        std::cout << "🔍 Browser ID: " << browser->GetIdentifier() << std::endl;
        std::cout << "🔍 Frame URL: " << frame->GetURL().ToString() << std::endl;
        LOG_DEBUG_RENDER("✅ Sign transaction response received: " + responseJson);
        LOG_DEBUG_RENDER("🔍 Browser ID: " + std::to_string(browser->GetIdentifier()));
        LOG_DEBUG_RENDER("🔍 Frame URL: " + frame->GetURL().ToString());

        // Execute JavaScript to dispatch the response event
        std::string js = "window.dispatchEvent(new CustomEvent('cefMessageResponse', { detail: { message: 'sign_transaction_response', args: ['" + responseJson + "'] } }));";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);

        return true;
    }

    if (message_name == "sign_transaction_error") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string errorMessage = args->GetString(0);

        std::cout << "❌ Sign transaction error received: " << errorMessage << std::endl;
        std::ofstream debugLog("debug_output.log", std::ios::app);
        debugLog << "❌ Sign transaction error received: " << errorMessage << std::endl;
        debugLog.close();

        // Execute JavaScript to handle the error
        std::string js = "if (window.onSignTransactionError) { window.onSignTransactionError('" + errorMessage + "'); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);

        return true;
    }

    if (message_name == "broadcast_transaction_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string responseJson = args->GetString(0);

        std::cout << "✅ Broadcast transaction response received: " << responseJson << std::endl;
        std::cout << "🔍 Browser ID: " << browser->GetIdentifier() << std::endl;
        std::cout << "🔍 Frame URL: " << frame->GetURL().ToString() << std::endl;
        std::ofstream debugLog("debug_output.log", std::ios::app);
        debugLog << "✅ Broadcast transaction response received: " << responseJson << std::endl;
        debugLog << "🔍 Browser ID: " << browser->GetIdentifier() << std::endl;
        debugLog << "🔍 Frame URL: " << frame->GetURL().ToString() << std::endl;
        debugLog.close();

        // Execute JavaScript to dispatch the response event
        std::string js = "window.dispatchEvent(new CustomEvent('cefMessageResponse', { detail: { message: 'broadcast_transaction_response', args: ['" + responseJson + "'] } }));";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);

        return true;
    }

    if (message_name == "broadcast_transaction_error") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string errorMessage = args->GetString(0);

        std::cout << "❌ Broadcast transaction error received: " << errorMessage << std::endl;
        std::ofstream debugLog("debug_output.log", std::ios::app);
        debugLog << "❌ Broadcast transaction error received: " << errorMessage << std::endl;
        debugLog.close();

        // Execute JavaScript to handle the error
        std::string js = "if (window.onBroadcastTransactionError) { window.onBroadcastTransactionError('" + errorMessage + "'); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);

        return true;
    }

    if (message_name == "send_transaction_response") {
        try {
            CefRefPtr<CefListValue> args = message->GetArgumentList();
            if (!args || args->GetSize() == 0) {
                std::cerr << "❌ send_transaction_response: No arguments" << std::endl;
                return true;
            }

            std::string responseJson = args->GetString(0);
            std::cout << "✅ Send transaction response received (length: " << responseJson.length() << ")" << std::endl;
            std::ofstream debugLog("debug_output.log", std::ios::app);
            debugLog << "✅ Send transaction response received (length: " << responseJson.length() << "): " << responseJson << std::endl;
            debugLog.close();

            // Execute JavaScript to call the callback function directly
            // Use JSON.parse() to safely parse the JSON string and avoid injection issues
            // Escape the JSON string to prevent JavaScript injection
            try {
                std::string escapedJson = escapeJsonForJs(responseJson);
                std::cout << "🔍 Escaped JSON (length: " << escapedJson.length() << ")" << std::endl;

                std::string js = "if (window.onSendTransactionResponse) { try { window.onSendTransactionResponse(JSON.parse('" +
                                 escapedJson + "')); } catch(e) { console.error('Failed to parse transaction response:', e); } }";

                std::cout << "🔍 Executing JavaScript (length: " << js.length() << ")" << std::endl;
                debugLog.open("debug_output.log", std::ios::app);
                debugLog << "🔍 Executing JavaScript (length: " << js.length() << ")" << std::endl;
                debugLog.close();

                if (frame) {
                    frame->ExecuteJavaScript(js, frame->GetURL(), 0);
                    std::cout << "✅ JavaScript executed successfully" << std::endl;
                } else {
                    std::cerr << "❌ Frame is null, cannot execute JavaScript" << std::endl;
                }
            } catch (const std::exception& e) {
                std::cerr << "❌ Failed to execute JavaScript for send_transaction_response: " << e.what() << std::endl;
                std::ofstream debugLog("debug_output.log", std::ios::app);
                debugLog << "❌ Failed to execute JavaScript: " << e.what() << std::endl;
                debugLog.close();
            }
        } catch (const std::exception& e) {
            std::cerr << "❌ Exception in send_transaction_response handler: " << e.what() << std::endl;
            std::ofstream debugLog("debug_output.log", std::ios::app);
            debugLog << "❌ Exception in send_transaction_response handler: " << e.what() << std::endl;
            debugLog.close();
        }

        return true;
    }

    if (message_name == "send_transaction_error") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string errorMessage = args->GetString(0);

        std::cout << "❌ Send transaction error received: " << errorMessage << std::endl;
        std::ofstream debugLog("debug_output.log", std::ios::app);
        debugLog << "❌ Send transaction error received: " << errorMessage << std::endl;
        debugLog.close();

        // Execute JavaScript to handle the error
        std::string js = "if (window.onSendTransactionError) { window.onSendTransactionError('" + errorMessage + "'); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);

        return true;
    }

    if (message_name == "get_balance_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string responseJson = args->GetString(0);

        std::cout << "✅ Get balance response received: " << responseJson << std::endl;
        std::cout << "🔍 Browser ID: " << browser->GetIdentifier() << std::endl;
        std::cout << "🔍 Frame URL: " << frame->GetURL().ToString() << std::endl;
        std::ofstream debugLog("debug_output.log", std::ios::app);
        debugLog << "✅ Get balance response received: " << responseJson << std::endl;
        debugLog << "🔍 Browser ID: " << browser->GetIdentifier() << std::endl;
        debugLog << "🔍 Frame URL: " << frame->GetURL().ToString() << std::endl;
        debugLog.close();

        // Execute JavaScript to call the callback function directly
        std::string js = "if (window.onGetBalanceResponse) { window.onGetBalanceResponse(" + responseJson + "); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);

        return true;
    }

    if (message_name == "get_balance_error") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string errorMessage = args->GetString(0);

        std::cout << "❌ Get balance error received: " << errorMessage << std::endl;
        std::ofstream debugLog("debug_output.log", std::ios::app);
        debugLog << "❌ Get balance error received: " << errorMessage << std::endl;
        debugLog.close();

        // Execute JavaScript to handle the error
        std::string js = "if (window.onGetBalanceError) { window.onGetBalanceError('" + errorMessage + "'); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);

        return true;
    }

    if (message_name == "get_transaction_history_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string responseJson = args->GetString(0);

        std::cout << "✅ Get transaction history response received: " << responseJson << std::endl;
        std::cout << "🔍 Browser ID: " << browser->GetIdentifier() << std::endl;
        std::cout << "🔍 Frame URL: " << frame->GetURL().ToString() << std::endl;
        std::ofstream debugLog("debug_output.log", std::ios::app);
        debugLog << "✅ Get transaction history response received: " << responseJson << std::endl;
        debugLog << "🔍 Browser ID: " << browser->GetIdentifier() << std::endl;
        debugLog << "🔍 Frame URL: " << frame->GetURL().ToString() << std::endl;
        debugLog.close();

        // Execute JavaScript to dispatch the response event
        std::string js = "window.dispatchEvent(new CustomEvent('cefMessageResponse', { detail: { message: 'get_transaction_history_response', args: ['" + responseJson + "'] } }));";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);

        return true;
    }

    if (message_name == "get_transaction_history_error") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string errorMessage = args->GetString(0);

        std::cout << "❌ Get transaction history error received: " << errorMessage << std::endl;
        std::ofstream debugLog("debug_output.log", std::ios::app);
        debugLog << "❌ Get transaction history error received: " << errorMessage << std::endl;
        debugLog.close();

        // Execute JavaScript to handle the error
        std::string js = "if (window.onGetTransactionHistoryError) { window.onGetTransactionHistoryError('" + errorMessage + "'); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);

        return true;
    }

    // Wallet Response Handlers

    // Settings persistence response
    if (message_name == "settings_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string settingsJson = args->GetString(0);

        std::cout << "✅ Settings response received" << std::endl;
        std::ofstream debugLog("debug_output.log", std::ios::app);
        debugLog << "✅ Settings response received: " << settingsJson.substr(0, 100) << "..." << std::endl;
        debugLog.close();

        // Execute JavaScript to call the callback function directly
        std::string js = "if (window.onSettingsResponse) { window.onSettingsResponse(" + settingsJson + "); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);

        return true;
    }

    // Profile Manager responses
    if (message_name == "profiles_result") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string profilesJson = args->GetString(0);

        std::cout << "👤 Profiles result received" << std::endl;

        std::string js = "if (window.onProfilesResult) { window.onProfilesResult(" + profilesJson + "); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);

        return true;
    }

    // Profile import responses
    if (message_name == "import_profiles_result") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string profilesJson = args->GetString(0);

        std::cout << "📂 Import profiles result received" << std::endl;
        std::ofstream debugLog("debug_output.log", std::ios::app);
        debugLog << "📂 Import profiles result: " << profilesJson << std::endl;
        debugLog.close();

        std::string js = "if (window.onImportProfilesResult) { window.onImportProfilesResult(" + profilesJson + "); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);

        return true;
    }

    if (message_name == "import_complete") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string resultJson = args->GetString(0);

        std::cout << "📦 Import complete" << std::endl;
        std::ofstream debugLog("debug_output.log", std::ios::app);
        debugLog << "📦 Import complete: " << resultJson << std::endl;
        debugLog.close();

        std::string js = "if (window.onImportComplete) { window.onImportComplete(" + resultJson + "); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);

        return true;
    }

    if (message_name == "wallet_status_check_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string responseJson = args->GetString(0);

        std::cout << "✅ Wallet status check response received: " << responseJson << std::endl;
        std::ofstream debugLog("debug_output.log", std::ios::app);
        debugLog << "✅ Wallet status check response received: " << responseJson << std::endl;
        debugLog.close();

        // Execute JavaScript to call the callback function directly
        std::string js = "if (window.onWalletStatusResponse) { window.onWalletStatusResponse(" + responseJson + "); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);

        return true;
    }

    if (message_name == "create_wallet_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string responseJson = args->GetString(0);

        std::cout << "✅ Create wallet response received: " << responseJson << std::endl;
        std::ofstream debugLog("debug_output.log", std::ios::app);
        debugLog << "✅ Create wallet response received: " << responseJson << std::endl;
        debugLog.close();

        // Execute JavaScript to call the callback function directly
        std::string js = "if (window.onCreateWalletResponse) { window.onCreateWalletResponse(" + responseJson + "); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);

        return true;
    }

    if (message_name == "load_wallet_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string responseJson = args->GetString(0);

        std::cout << "✅ Load wallet response received: " << responseJson << std::endl;
        std::ofstream debugLog("debug_output.log", std::ios::app);
        debugLog << "✅ Load wallet response received: " << responseJson << std::endl;
        debugLog.close();

        // Execute JavaScript to call the callback function directly
        std::string js = "if (window.onLoadWalletResponse) { window.onLoadWalletResponse(" + responseJson + "); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);

        return true;
    }

    if (message_name == "get_wallet_info_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string responseJson = args->GetString(0);

        std::cout << "✅ Get wallet info response received: " << responseJson << std::endl;
        std::ofstream debugLog("debug_output.log", std::ios::app);
        debugLog << "✅ Get wallet info response received: " << responseJson << std::endl;
        debugLog.close();

        // Execute JavaScript to call the callback function directly
        std::string js = "if (window.onGetWalletInfoResponse) { window.onGetWalletInfoResponse(" + responseJson + "); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);

        return true;
    }

    if (message_name == "get_all_addresses_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string responseJson = args->GetString(0);

        std::cout << "✅ Get all addresses response received: " << responseJson << std::endl;
        std::ofstream debugLog("debug_output.log", std::ios::app);
        debugLog << "✅ Get all addresses response received: " << responseJson << std::endl;
        debugLog.close();

        // Execute JavaScript to call the callback function directly
        std::string js = "if (window.onGetAllAddressesResponse) { window.onGetAllAddressesResponse(" + responseJson + "); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);

        return true;
    }

    if (message_name == "get_current_address_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string responseJson = args->GetString(0);

        std::cout << "✅ Get current address response received: " << responseJson << std::endl;
        std::ofstream debugLog("debug_output.log", std::ios::app);
        debugLog << "✅ Get current address response received: " << responseJson << std::endl;
        debugLog.close();

        // Execute JavaScript to call the callback function directly
        std::string js = "if (window.onGetCurrentAddressResponse) { window.onGetCurrentAddressResponse(" + responseJson + "); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);

        return true;
    }

    if (message_name == "mark_wallet_backed_up_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string responseJson = args->GetString(0);

        std::cout << "✅ Mark wallet backed up response received: " << responseJson << std::endl;
        std::ofstream debugLog("debug_output.log", std::ios::app);
        debugLog << "✅ Mark wallet backed up response received: " << responseJson << std::endl;
        debugLog.close();

        // Execute JavaScript to call the callback function directly
        std::string js = "if (window.onMarkWalletBackedUpResponse) { window.onMarkWalletBackedUpResponse(" + responseJson + "); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);

        return true;
    }

    if (message_name == "get_addresses_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string responseJson = args->GetString(0);

        std::cout << "✅ Get addresses response received: " << responseJson << std::endl;
        std::ofstream debugLog("debug_output.log", std::ios::app);
        debugLog << "✅ Get addresses response received: " << responseJson << std::endl;
        debugLog.close();

        // Execute JavaScript to call the callback function directly
        std::string js = "if (window.onGetAddressesResponse) { window.onGetAddressesResponse(" + responseJson + "); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);

        return true;
    }

    if (message_name == "get_backup_modal_state_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string responseJson = args->GetString(0);

        LOG_DEBUG_RENDER("✅ Backup modal state response received: " + responseJson);

        // Execute JavaScript callback
        std::string js = "if (window.onGetBackupModalStateResponse) { window.onGetBackupModalStateResponse(" + responseJson + "); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);

        return true;
    }

    if (message_name == "set_backup_modal_state_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string responseJson = args->GetString(0);

        LOG_DEBUG_RENDER("✅ Set backup modal state response received: " + responseJson);

        // Execute JavaScript callback
        std::string js = "if (window.onSetBackupModalStateResponse) { window.onSetBackupModalStateResponse(" + responseJson + "); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);

        return true;
    }

    // ========== OMNIBOX QUERY UPDATE ==========
    if (message_name == "omnibox_query_update") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string query = args->GetString(0);

        LOG_DEBUG_RENDER("🔍 Omnibox query update received in renderer: " + query);

        // Escape query for JavaScript string
        std::string escapedQuery = escapeJsonForJs(query);

        // Dispatch CustomEvent to JavaScript
        std::string js = "window.dispatchEvent(new CustomEvent('omniboxQueryUpdate', { detail: { query: '" + escapedQuery + "' } }));";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);

        LOG_DEBUG_RENDER("🔍 omniboxQueryUpdate event dispatched");
        return true;
    }

    // ========== OMNIBOX SELECT (ARROW KEY NAVIGATION) ==========
    if (message_name == "omnibox_select") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string direction = args->GetString(0);

        LOG_DEBUG_RENDER("🔍 Omnibox select received in renderer: " + direction);

        std::string js = "window.dispatchEvent(new CustomEvent('omniboxSelect', "
                         "{ detail: { direction: '" + direction + "' } }));";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);

        LOG_DEBUG_RENDER("🔍 omniboxSelect event dispatched");
        return true;
    }

    // ========== OMNIBOX AUTOCOMPLETE UPDATE ==========
    if (message_name == "omnibox_autocomplete_update") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string suggestion = args->GetString(0);

        LOG_DEBUG_RENDER("🔍 Omnibox autocomplete update received in renderer: " + suggestion);

        // Escape suggestion for JavaScript string
        std::string escapedSuggestion = escapeJsonForJs(suggestion);

        // Dispatch via window.postMessage (MainBrowserView listens for this)
        std::string js = "window.postMessage({ type: 'omnibox_autocomplete', suggestion: '" + escapedSuggestion + "' }, '*');";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);

        LOG_DEBUG_RENDER("🔍 omnibox_autocomplete message posted to window");
        return true;
    }

    // ========== WALLET PAYMENT DISMISSED (forwarded from wallet overlay) ==========
    if (message_name == "wallet_payment_dismissed") {
        frame->ExecuteJavaScript("window.postMessage({ type: 'wallet_payment_dismissed' }, '*');", frame->GetURL(), 0);
        LOG_DEBUG_RENDER("wallet_payment_dismissed posted to header window");
        return true;
    }

    // ========== GOOGLE SUGGEST RESPONSE ==========
    if (message_name == "google_suggest_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string suggestionsJson = args->GetString(0);
        int requestId = args->GetSize() > 1 ? args->GetInt(1) : 0;

        LOG_DEBUG_RENDER("🔍 Google Suggest response received (length: " + std::to_string(suggestionsJson.length()) + "): " + suggestionsJson.substr(0, 200) + " (requestId: " + std::to_string(requestId) + ")");

        try {
            // Escape JSON for JavaScript (using existing escapeJsonForJs helper)
            std::string escapedJson = escapeJsonForJs(suggestionsJson);

            LOG_DEBUG_RENDER("🔍 Escaped JSON (length: " + std::to_string(escapedJson.length()) + "): " + escapedJson.substr(0, 200));

            // Use try-catch in JavaScript to prevent crashes
            std::string js =
                "try { "
                "  var parsedSuggestions = JSON.parse('" + escapedJson + "'); "
                "  window.dispatchEvent(new CustomEvent('googleSuggestResponse', { "
                "    detail: { suggestions: parsedSuggestions, requestId: " + std::to_string(requestId) + " } "
                "  })); "
                "} catch(e) { "
                "  console.error('Failed to parse Google suggestions:', e, 'JSON:', '" + escapedJson + "'); "
                "}";

            if (frame) {
                frame->ExecuteJavaScript(js, frame->GetURL(), 0);
                LOG_DEBUG_RENDER("🔍 Google Suggest response dispatched to window with requestId: " + std::to_string(requestId));
            } else {
                LOG_DEBUG_RENDER("⚠️ Frame is null, cannot dispatch Google Suggest response");
            }
        } catch (const std::exception& e) {
            LOG_DEBUG_RENDER("❌ Exception in google_suggest_response handler: " + std::string(e.what()));
        }

        return true;
    }

    // ========== COOKIE/CACHE RESPONSE HANDLERS ==========

    if (message_name == "cookie_get_all_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string cookiesJson = args->GetString(0).ToString();
        std::string escaped = escapeJsonForJs(cookiesJson);
        std::string js = "if (window.onCookieGetAllResponse) { window.onCookieGetAllResponse(JSON.parse('" + escaped + "')); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);
        return true;
    }

    if (message_name == "cookie_delete_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string responseJson = args->GetString(0).ToString();
        std::string escaped = escapeJsonForJs(responseJson);
        std::string js = "if (window.onCookieDeleteResponse) { window.onCookieDeleteResponse(JSON.parse('" + escaped + "')); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);
        return true;
    }

    if (message_name == "cookie_delete_domain_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string responseJson = args->GetString(0).ToString();
        std::string escaped = escapeJsonForJs(responseJson);
        std::string js = "if (window.onCookieDeleteDomainResponse) { window.onCookieDeleteDomainResponse(JSON.parse('" + escaped + "')); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);
        return true;
    }

    if (message_name == "cookie_delete_all_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string responseJson = args->GetString(0).ToString();
        std::string escaped = escapeJsonForJs(responseJson);
        std::string js = "if (window.onCookieDeleteAllResponse) { window.onCookieDeleteAllResponse(JSON.parse('" + escaped + "')); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);
        return true;
    }

    if (message_name == "cache_clear_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string responseJson = args->GetString(0).ToString();
        std::string escaped = escapeJsonForJs(responseJson);
        std::string js = "if (window.onCacheClearResponse) { window.onCacheClearResponse(JSON.parse('" + escaped + "')); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);
        return true;
    }

    if (message_name == "cache_get_size_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string responseJson = args->GetString(0).ToString();
        std::string escaped = escapeJsonForJs(responseJson);
        std::string js = "if (window.onCacheGetSizeResponse) { window.onCacheGetSizeResponse(JSON.parse('" + escaped + "')); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);
        return true;
    }

    // ========== COOKIE BLOCKING RESPONSE HANDLERS ==========

    if (message_name == "cookie_block_domain_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string responseJson = args->GetString(0).ToString();
        std::string escaped = escapeJsonForJs(responseJson);
        std::string js = "if (window.onCookieBlockDomainResponse) { window.onCookieBlockDomainResponse(JSON.parse('" + escaped + "')); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);
        return true;
    }

    if (message_name == "cookie_unblock_domain_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string responseJson = args->GetString(0).ToString();
        std::string escaped = escapeJsonForJs(responseJson);
        std::string js = "if (window.onCookieUnblockDomainResponse) { window.onCookieUnblockDomainResponse(JSON.parse('" + escaped + "')); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);
        return true;
    }

    if (message_name == "cookie_blocklist_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string responseJson = args->GetString(0).ToString();
        std::string escaped = escapeJsonForJs(responseJson);
        std::string js = "if (window.onCookieBlocklistResponse) { window.onCookieBlocklistResponse(JSON.parse('" + escaped + "')); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);
        return true;
    }

    if (message_name == "cookie_allow_third_party_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string responseJson = args->GetString(0).ToString();
        std::string escaped = escapeJsonForJs(responseJson);
        std::string js = "if (window.onCookieAllowThirdPartyResponse) { window.onCookieAllowThirdPartyResponse(JSON.parse('" + escaped + "')); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);
        return true;
    }

    if (message_name == "cookie_remove_third_party_allow_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string responseJson = args->GetString(0).ToString();
        std::string escaped = escapeJsonForJs(responseJson);
        std::string js = "if (window.onCookieRemoveThirdPartyAllowResponse) { window.onCookieRemoveThirdPartyAllowResponse(JSON.parse('" + escaped + "')); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);
        return true;
    }

    if (message_name == "cookie_block_log_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string responseJson = args->GetString(0).ToString();
        std::string escaped = escapeJsonForJs(responseJson);
        std::string js = "if (window.onCookieBlockLogResponse) { window.onCookieBlockLogResponse(JSON.parse('" + escaped + "')); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);
        return true;
    }

    if (message_name == "cookie_clear_block_log_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string responseJson = args->GetString(0).ToString();
        std::string escaped = escapeJsonForJs(responseJson);
        std::string js = "if (window.onCookieClearBlockLogResponse) { window.onCookieClearBlockLogResponse(JSON.parse('" + escaped + "')); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);
        return true;
    }

    if (message_name == "cookie_blocked_count_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string responseJson = args->GetString(0).ToString();
        std::string escaped = escapeJsonForJs(responseJson);
        std::string js = "if (window.onCookieBlockedCountResponse) { window.onCookieBlockedCountResponse(JSON.parse('" + escaped + "')); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);
        return true;
    }

    if (message_name == "cookie_reset_blocked_count_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string responseJson = args->GetString(0).ToString();
        std::string escaped = escapeJsonForJs(responseJson);
        std::string js = "if (window.onCookieResetBlockedCountResponse) { window.onCookieResetBlockedCountResponse(JSON.parse('" + escaped + "')); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);
        return true;
    }

    if (message_name == "cookie_check_site_allowed_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string responseJson = args->GetString(0).ToString();
        std::string escaped = escapeJsonForJs(responseJson);
        std::string js = "if (window.onCookieCheckSiteAllowedResponse) { window.onCookieCheckSiteAllowedResponse(JSON.parse('" + escaped + "')); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);
        return true;
    }

    // ========== ADBLOCK RESPONSE HANDLERS (Sprint 8c) ==========

    if (message_name == "adblock_blocked_count_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string responseJson = args->GetString(0).ToString();
        std::string escaped = escapeJsonForJs(responseJson);
        std::string js = "if (window.onAdblockBlockedCountResponse) { window.onAdblockBlockedCountResponse(JSON.parse('" + escaped + "')); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);
        return true;
    }

    if (message_name == "adblock_reset_blocked_count_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string responseJson = args->GetString(0).ToString();
        std::string escaped = escapeJsonForJs(responseJson);
        std::string js = "if (window.onAdblockResetBlockedCountResponse) { window.onAdblockResetBlockedCountResponse(JSON.parse('" + escaped + "')); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);
        return true;
    }

    if (message_name == "adblock_site_toggle_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string responseJson = args->GetString(0).ToString();
        std::string escaped = escapeJsonForJs(responseJson);
        std::string js = "if (window.onAdblockSiteToggleResponse) { window.onAdblockSiteToggleResponse(JSON.parse('" + escaped + "')); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);
        return true;
    }

    if (message_name == "adblock_scriptlet_toggle_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string responseJson = args->GetString(0).ToString();
        std::string escaped = escapeJsonForJs(responseJson);
        std::string js = "if (window.onAdblockScriptletToggleResponse) { window.onAdblockScriptletToggleResponse(JSON.parse('" + escaped + "')); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);
        return true;
    }

    if (message_name == "adblock_check_site_enabled_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string responseJson = args->GetString(0).ToString();
        std::string escaped = escapeJsonForJs(responseJson);
        std::string js = "if (window.onAdblockCheckSiteEnabledResponse) { window.onAdblockCheckSiteEnabledResponse(JSON.parse('" + escaped + "')); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);
        return true;
    }

    if (message_name == "fingerprint_get_site_enabled_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string responseJson = args->GetString(0).ToString();
        std::string escaped = escapeJsonForJs(responseJson);
        std::string js = "if (window.onFingerprintSiteEnabledResponse) { window.onFingerprintSiteEnabledResponse(JSON.parse('" + escaped + "')); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);
        return true;
    }

    if (message_name == "adblock_check_scriptlets_enabled_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string responseJson = args->GetString(0).ToString();
        std::string escaped = escapeJsonForJs(responseJson);
        std::string js = "if (window.onAdblockCheckScriptletsEnabledResponse) { window.onAdblockCheckScriptletsEnabledResponse(JSON.parse('" + escaped + "')); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);
        return true;
    }

    // ========== BOOKMARK RESPONSE HANDLERS ==========

    if (message_name == "bookmark_add_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string responseJson = args->GetString(0).ToString();
        std::string escaped = escapeJsonForJs(responseJson);
        std::string js = "if (window.onBookmarkAddResponse) { window.onBookmarkAddResponse(JSON.parse('" + escaped + "')); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);
        return true;
    }

    if (message_name == "bookmark_get_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string responseJson = args->GetString(0).ToString();
        std::string escaped = escapeJsonForJs(responseJson);
        std::string js = "if (window.onBookmarkGetResponse) { window.onBookmarkGetResponse(JSON.parse('" + escaped + "')); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);
        return true;
    }

    if (message_name == "bookmark_update_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string responseJson = args->GetString(0).ToString();
        std::string escaped = escapeJsonForJs(responseJson);
        std::string js = "if (window.onBookmarkUpdateResponse) { window.onBookmarkUpdateResponse(JSON.parse('" + escaped + "')); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);
        return true;
    }

    if (message_name == "bookmark_remove_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string responseJson = args->GetString(0).ToString();
        std::string escaped = escapeJsonForJs(responseJson);
        std::string js = "if (window.onBookmarkRemoveResponse) { window.onBookmarkRemoveResponse(JSON.parse('" + escaped + "')); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);
        return true;
    }

    if (message_name == "bookmark_search_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string responseJson = args->GetString(0).ToString();
        std::string escaped = escapeJsonForJs(responseJson);
        std::string js = "if (window.onBookmarkSearchResponse) { window.onBookmarkSearchResponse(JSON.parse('" + escaped + "')); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);
        return true;
    }

    if (message_name == "bookmark_get_all_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string responseJson = args->GetString(0).ToString();
        std::string escaped = escapeJsonForJs(responseJson);
        std::string js = "if (window.onBookmarkGetAllResponse) { window.onBookmarkGetAllResponse(JSON.parse('" + escaped + "')); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);
        return true;
    }

    if (message_name == "bookmark_is_bookmarked_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string responseJson = args->GetString(0).ToString();
        std::string escaped = escapeJsonForJs(responseJson);
        std::string js = "if (window.onBookmarkIsBookmarkedResponse) { window.onBookmarkIsBookmarkedResponse(JSON.parse('" + escaped + "')); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);
        return true;
    }

    if (message_name == "bookmark_get_all_tags_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string responseJson = args->GetString(0).ToString();
        std::string escaped = escapeJsonForJs(responseJson);
        std::string js = "if (window.onBookmarkGetAllTagsResponse) { window.onBookmarkGetAllTagsResponse(JSON.parse('" + escaped + "')); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);
        return true;
    }

    if (message_name == "bookmark_update_last_accessed_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string responseJson = args->GetString(0).ToString();
        std::string escaped = escapeJsonForJs(responseJson);
        std::string js = "if (window.onBookmarkUpdateLastAccessedResponse) { window.onBookmarkUpdateLastAccessedResponse(JSON.parse('" + escaped + "')); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);
        return true;
    }

    if (message_name == "bookmark_folder_create_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string responseJson = args->GetString(0).ToString();
        std::string escaped = escapeJsonForJs(responseJson);
        std::string js = "if (window.onBookmarkFolderCreateResponse) { window.onBookmarkFolderCreateResponse(JSON.parse('" + escaped + "')); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);
        return true;
    }

    if (message_name == "bookmark_folder_list_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string responseJson = args->GetString(0).ToString();
        std::string escaped = escapeJsonForJs(responseJson);
        std::string js = "if (window.onBookmarkFolderListResponse) { window.onBookmarkFolderListResponse(JSON.parse('" + escaped + "')); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);
        return true;
    }

    if (message_name == "bookmark_folder_update_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string responseJson = args->GetString(0).ToString();
        std::string escaped = escapeJsonForJs(responseJson);
        std::string js = "if (window.onBookmarkFolderUpdateResponse) { window.onBookmarkFolderUpdateResponse(JSON.parse('" + escaped + "')); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);
        return true;
    }

    if (message_name == "bookmark_folder_remove_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string responseJson = args->GetString(0).ToString();
        std::string escaped = escapeJsonForJs(responseJson);
        std::string js = "if (window.onBookmarkFolderRemoveResponse) { window.onBookmarkFolderRemoveResponse(JSON.parse('" + escaped + "')); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);
        return true;
    }

    if (message_name == "bookmark_folder_get_tree_response") {
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string responseJson = args->GetString(0).ToString();
        std::string escaped = escapeJsonForJs(responseJson);
        std::string js = "if (window.onBookmarkFolderGetTreeResponse) { window.onBookmarkFolderGetTreeResponse(JSON.parse('" + escaped + "')); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);
        return true;
    }

    return false;
}
