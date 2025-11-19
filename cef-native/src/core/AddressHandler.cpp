#include "../../include/core/AddressHandler.h"
#include "../../include/core/WalletService.h"
#include "include/cef_v8.h"
#include "include/cef_browser.h"
#include "include/cef_frame.h"
#include "include/wrapper/cef_helpers.h"
#include <iostream>
#include <string>
#include <fstream>

AddressHandler::AddressHandler() {}

AddressHandler::~AddressHandler() {}

bool AddressHandler::Execute(const CefString& name,
                            CefRefPtr<CefV8Value> object,
                            const CefV8ValueList& arguments,
                            CefRefPtr<CefV8Value>& retval,
                            CefString& exception) {
    std::cout << "💡 AddressHandler started - Function: " << name.ToString() << std::endl;
    std::cout << "💡 AddressHandler - Browser ID: " << CefV8Context::GetCurrentContext()->GetBrowser()->GetIdentifier() << std::endl;
    std::cout << "💡 AddressHandler - Frame URL: " << CefV8Context::GetCurrentContext()->GetFrame()->GetURL().ToString() << std::endl;
    std::cout.flush(); // Force flush

    // Also try OutputDebugString for Windows
    std::string debugMsg = "💡 AddressHandler started - Function: " + name.ToString();
    OutputDebugStringA(debugMsg.c_str());
    OutputDebugStringA("\n");

    WalletService walletService;

    // Check if Go daemon is running
    if (!walletService.isConnected()) {
        std::cout << "❌ Go daemon not connected" << std::endl;
        exception = "Go daemon not connected";
        return false;
    }

            if (name == "generate") {
                std::cout << "🔑 Address generation requested via V8 - checking if overlay browser" << std::endl;

                CefRefPtr<CefV8Context> context = CefV8Context::GetCurrentContext();
                CefRefPtr<CefBrowser> browser = context->GetBrowser();
                CefRefPtr<CefFrame> frame = context->GetFrame();

                std::string frameUrl = frame->GetURL().ToString();
                std::cout << "🔍 Frame URL: " << frameUrl << std::endl;

                // Check if this is an overlay browser (wallet, settings, backup)
                if (frameUrl.find("/wallet") != std::string::npos ||
                    frameUrl.find("/settings") != std::string::npos ||
                    frameUrl.find("/backup") != std::string::npos ||
                    frameUrl.find("/overlay") != std::string::npos) {
                    std::cout << "🎯 This is an overlay browser - using direct V8 communication" << std::endl;

                    // For overlay browser, use direct V8 communication
                    try {
                        WalletService walletService;
                        if (!walletService.isConnected()) {
                            std::cout << "❌ Go daemon not connected" << std::endl;
                            exception = "Go daemon not connected";
                            return false;
                        }

                        nlohmann::json addressData = walletService.generateAddress();
                        std::cout << "✅ Address generated directly: " << addressData.dump() << std::endl;
                        std::cout << "✅ Address: " << addressData["address"].get<std::string>() << std::endl;
                        std::cout << "✅ Public Key: " << addressData["publicKey"].get<std::string>() << std::endl;
                        std::cout << "✅ Private Key: " << addressData["privateKey"].get<std::string>() << std::endl;

                        // Create V8 object from JSON
                        CefRefPtr<CefV8Value> result = CefV8Value::CreateObject(nullptr, nullptr);
                        result->SetValue("address", CefV8Value::CreateString(addressData["address"].get<std::string>()), V8_PROPERTY_ATTRIBUTE_NONE);
                        result->SetValue("publicKey", CefV8Value::CreateString(addressData["publicKey"].get<std::string>()), V8_PROPERTY_ATTRIBUTE_NONE);
                        result->SetValue("privateKey", CefV8Value::CreateString(addressData["privateKey"].get<std::string>()), V8_PROPERTY_ATTRIBUTE_NONE);
                        result->SetValue("index", CefV8Value::CreateInt(addressData["index"].get<int>()), V8_PROPERTY_ATTRIBUTE_NONE);

                        std::cout << "🔍 V8 object created, setting retval..." << std::endl;
                        std::ofstream debugLog("debug_output.log", std::ios::app);
                        debugLog << "🔍 V8 object created, setting retval..." << std::endl;
                        debugLog.close();

                        retval = result;
                        std::cout << "✅ retval set, returning true" << std::endl;
                        std::ofstream debugLog2("debug_output.log", std::ios::app);
                        debugLog2 << "✅ retval set, returning true" << std::endl;
                        debugLog2.close();
                        return true;

                    } catch (const std::exception& e) {
                        std::cout << "❌ Address generation failed: " << e.what() << std::endl;
                        exception = e.what();
                        return false;
                    }
                } else {
                    std::cout << "🔑 This is the main browser - using process messages" << std::endl;

                    // For main browser, use process messages
                    if (browser) {
                        CefRefPtr<CefProcessMessage> message = CefProcessMessage::Create("address_generate");
                        browser->GetMainFrame()->SendProcessMessage(PID_BROWSER, message);
                        std::cout << "📤 Address generation message sent to main process" << std::endl;

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

    exception = "Unknown function: " + name.ToString();
    return false;
}
