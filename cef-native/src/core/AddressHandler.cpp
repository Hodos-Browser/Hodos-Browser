#include "../../include/core/AddressHandler.h"
#include "../../include/core/Logger.h"
#include "../../include/core/LogSafeUrl.h"
#include "../../include/core/WalletService.h"
#include "include/cef_v8.h"
#include "include/cef_browser.h"
#include "include/cef_frame.h"
#include "include/wrapper/cef_helpers.h"
#include <iostream>
#include <string>
#include <fstream>

// beta.3 Phase 2b: these were std::cout / std::cerr, which reach NOTHING.
//
// This class runs in the RENDER process, where Logger::Initialize is never called -- so
// Logger::Log falls through to the injected child-process sink and lands in cef_debug.log
// (see include/core/ChildProcessLogSink.h). That is the only sink a sandboxed renderer can
// reach; it cannot open the roaming app-data directory itself.
//
// ProcessType::RENDER (1) so the line is attributable to the right process.
#define LOG_DEBUG_ADDR(msg)   Logger::Log(msg, 0, 1)
#define LOG_INFO_ADDR(msg)    Logger::Log(msg, 1, 1)
#define LOG_WARNING_ADDR(msg) Logger::Log(msg, 2, 1)
#define LOG_ERROR_ADDR(msg)   Logger::Log(msg, 3, 1)


AddressHandler::AddressHandler() {}

AddressHandler::~AddressHandler() {}

bool AddressHandler::Execute(const CefString& name,
                            CefRefPtr<CefV8Value> object,
                            const CefV8ValueList& arguments,
                            CefRefPtr<CefV8Value>& retval,
                            CefString& exception) {
    LOG_DEBUG_ADDR(LogFmt() << "💡 AddressHandler started - Function: " << name.ToString());
    LOG_DEBUG_ADDR(LogFmt() << "💡 AddressHandler - Browser ID: " << CefV8Context::GetCurrentContext()->GetBrowser()->GetIdentifier());
    LOG_DEBUG_ADDR(LogFmt() << "💡 AddressHandler - Frame URL: " << hodos::LogSafeUrl(CefV8Context::GetCurrentContext()->GetFrame()->GetURL().ToString()));
    std::cout.flush(); // Force flush

    // Platform-specific debug output
#ifdef _WIN32
    std::string debugMsg = "💡 AddressHandler started - Function: " + name.ToString();
    OutputDebugStringA(debugMsg.c_str());
    OutputDebugStringA("\n");
#endif

    WalletService walletService;

    // Check if Go daemon is running
    if (!walletService.isConnected()) {
        LOG_DEBUG_ADDR(LogFmt() << "❌ Go daemon not connected");
        exception = "Go daemon not connected";
        return false;
    }

            if (name == "generate") {
                LOG_DEBUG_ADDR(LogFmt() << "🔑 Address generation requested via V8 - checking if overlay browser");

                CefRefPtr<CefV8Context> context = CefV8Context::GetCurrentContext();
                CefRefPtr<CefBrowser> browser = context->GetBrowser();
                CefRefPtr<CefFrame> frame = context->GetFrame();

                std::string frameUrl = frame->GetURL().ToString();
                LOG_DEBUG_ADDR(LogFmt() << "🔍 Frame URL: " << frameUrl);

                // Check if this is an overlay browser (wallet, settings, backup)
                if (frameUrl.find("/wallet") != std::string::npos ||
                    frameUrl.find("/settings") != std::string::npos ||
                    frameUrl.find("/backup") != std::string::npos ||
                    frameUrl.find("/overlay") != std::string::npos) {
                    LOG_DEBUG_ADDR(LogFmt() << "🎯 This is an overlay browser - using direct V8 communication");

                    // For overlay browser, use direct V8 communication
                    try {
                        WalletService walletService;
                        if (!walletService.isConnected()) {
                            LOG_DEBUG_ADDR(LogFmt() << "❌ Go daemon not connected");
                            exception = "Go daemon not connected";
                            return false;
                        }

                        nlohmann::json addressData = walletService.generateAddress();
                        LOG_DEBUG_ADDR(LogFmt() << "✅ Address generated directly: " << addressData.dump());
                        LOG_DEBUG_ADDR(LogFmt() << "✅ Address: " << addressData["address"].get<std::string>());
                        LOG_DEBUG_ADDR(LogFmt() << "✅ Public Key: " << addressData["publicKey"].get<std::string>());

                        // Create V8 object from JSON
                        CefRefPtr<CefV8Value> result = CefV8Value::CreateObject(nullptr, nullptr);
                        result->SetValue("address", CefV8Value::CreateString(addressData["address"].get<std::string>()), V8_PROPERTY_ATTRIBUTE_NONE);
                        result->SetValue("publicKey", CefV8Value::CreateString(addressData["publicKey"].get<std::string>()), V8_PROPERTY_ATTRIBUTE_NONE);
                        result->SetValue("index", CefV8Value::CreateInt(addressData["index"].get<int>()), V8_PROPERTY_ATTRIBUTE_NONE);

                        LOG_DEBUG_ADDR(LogFmt() << "🔍 V8 object created, setting retval...");

                        retval = result;
                        LOG_DEBUG_ADDR(LogFmt() << "✅ retval set, returning true");
                        return true;

                    } catch (const std::exception& e) {
                        LOG_DEBUG_ADDR(LogFmt() << "❌ Address generation failed: " << e.what());
                        exception = e.what();
                        return false;
                    }
                } else {
                    LOG_DEBUG_ADDR(LogFmt() << "🔑 This is the main browser - using process messages");

                    // For main browser, use process messages
                    if (browser) {
                        CefRefPtr<CefProcessMessage> message = CefProcessMessage::Create("address_generate");
                        browser->GetMainFrame()->SendProcessMessage(PID_BROWSER, message);
                        LOG_DEBUG_ADDR(LogFmt() << "📤 Address generation message sent to main process");

                        // Return a promise-like object that will be resolved by the response handler
                        CefRefPtr<CefV8Value> promise = CefV8Value::CreateObject(nullptr, nullptr);
                        promise->SetValue("then", CefV8Value::CreateFunction("then", this), V8_PROPERTY_ATTRIBUTE_NONE);
                        promise->SetValue("catch", CefV8Value::CreateFunction("catch", this), V8_PROPERTY_ATTRIBUTE_NONE);

                        retval = promise;
                        return true;
                    } else {
                        exception = "Browser not available";
                        return false;
                    }
                }
            }

    if (name == "getAll") {
        LOG_DEBUG_ADDR(LogFmt() << "📋 Get all addresses requested");

        // Send process message to browser process
        CefRefPtr<CefV8Context> context = CefV8Context::GetCurrentContext();
        CefRefPtr<CefBrowser> browser = context->GetBrowser();

        if (browser) {
            CefRefPtr<CefProcessMessage> message = CefProcessMessage::Create("get_all_addresses");
            browser->GetMainFrame()->SendProcessMessage(PID_BROWSER, message);
            LOG_DEBUG_ADDR(LogFmt() << "✅ get_all_addresses message sent to browser process");

            // Return promise-like object (matches generate pattern)
            CefRefPtr<CefV8Value> promise = CefV8Value::CreateObject(nullptr, nullptr);
            promise->SetValue("then", CefV8Value::CreateFunction("then", this), V8_PROPERTY_ATTRIBUTE_NONE);
            promise->SetValue("catch", CefV8Value::CreateFunction("catch", this), V8_PROPERTY_ATTRIBUTE_NONE);

            retval = promise;
            return true;
        } else {
            exception = "Browser not available";
            return false;
        }
    }

    if (name == "getCurrent") {
        LOG_DEBUG_ADDR(LogFmt() << "📍 Get current address requested");

        // Send process message to browser process
        CefRefPtr<CefV8Context> context = CefV8Context::GetCurrentContext();
        CefRefPtr<CefBrowser> browser = context->GetBrowser();

        if (browser) {
            CefRefPtr<CefProcessMessage> message = CefProcessMessage::Create("get_current_address");
            browser->GetMainFrame()->SendProcessMessage(PID_BROWSER, message);
            LOG_DEBUG_ADDR(LogFmt() << "✅ get_current_address message sent to browser process");

            // Return promise-like object (matches generate pattern)
            CefRefPtr<CefV8Value> promise = CefV8Value::CreateObject(nullptr, nullptr);
            promise->SetValue("then", CefV8Value::CreateFunction("then", this), V8_PROPERTY_ATTRIBUTE_NONE);
            promise->SetValue("catch", CefV8Value::CreateFunction("catch", this), V8_PROPERTY_ATTRIBUTE_NONE);

            retval = promise;
            return true;
        } else {
            exception = "Browser not available";
            return false;
        }
    }

    exception = "Unknown function: " + name.ToString();
    return false;
}
