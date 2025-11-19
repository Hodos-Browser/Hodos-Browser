// cef_native/src/simple_render_process_handler.cpp
#include "../../include/handlers/simple_render_process_handler.h"
#include "../../include/core/IdentityHandler.h"
#include "../../include/core/NavigationHandler.h"
#include "../../include/core/AddressHandler.h"
#include "BRC100Handler.h"
#include "wrapper/cef_helpers.h"
#include "include/cef_v8.h"
#include <iostream>
#include <fstream>

// Forward declaration of Logger class from main shell
class Logger {
public:
    static void Log(const std::string& message, int level = 1, int process = 1);
};

// Convenience macros for easier logging
#define LOG_DEBUG_RENDER(msg) Logger::Log(msg, 0, 1)
#define LOG_INFO_RENDER(msg) Logger::Log(msg, 1, 1)
#define LOG_WARNING_RENDER(msg) Logger::Log(msg, 2, 1)
#define LOG_ERROR_RENDER(msg) Logger::Log(msg, 3, 1)

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
                // Handle array arguments - extract the first element if it's a string
                CefRefPtr<CefV8Value> array = arguments[i];
                std::cout << "Array with length: " << array->GetArrayLength() << std::endl;
                LOG_DEBUG_RENDER("Array with length: " + std::to_string(array->GetArrayLength()));
                if (array->GetArrayLength() > 0) {
                    CefRefPtr<CefV8Value> firstElement = array->GetValue(0);
                    if (firstElement->IsString()) {
                        std::string value = firstElement->GetStringValue();
                        std::cout << "Array[0] String: " << value << std::endl;
                        LOG_DEBUG_RENDER("Array[0] String: " + value);
                        args->SetString(i - 1, value);
                    } else if (firstElement->IsBool()) {
                        bool value = firstElement->GetBoolValue();
                        std::cout << "Array[0] Bool: " << (value ? "true" : "false") << std::endl;
                        LOG_DEBUG_RENDER("Array[0] Bool: " + std::string(value ? "true" : "false"));
                        args->SetBool(i - 1, value);
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

SimpleRenderProcessHandler::SimpleRenderProcessHandler() {
    LOG_DEBUG_RENDER("🔧 SimpleRenderProcessHandler constructor called!");
    LOG_DEBUG_RENDER("🔧 Process ID: " + std::to_string(GetCurrentProcessId()));
    LOG_DEBUG_RENDER("🔧 Thread ID: " + std::to_string(GetCurrentThreadId()));
}

void SimpleRenderProcessHandler::OnContextCreated(
    CefRefPtr<CefBrowser> browser,
    CefRefPtr<CefFrame> frame,
    CefRefPtr<CefV8Context> context) {

    CEF_REQUIRE_RENDERER_THREAD();

    LOG_DEBUG_RENDER("🔧 OnContextCreated called for browser ID: " + std::to_string(browser->GetIdentifier()));
    LOG_DEBUG_RENDER("🔧 Frame URL: " + frame->GetURL().ToString());
    LOG_DEBUG_RENDER("🔧 Process ID: " + std::to_string(GetCurrentProcessId()));
    LOG_DEBUG_RENDER("🔧 Thread ID: " + std::to_string(GetCurrentThreadId()));
    LOG_DEBUG_RENDER("🔧 RENDER PROCESS HANDLER IS WORKING!");
    LOG_DEBUG_RENDER("🔧 THIS IS THE RENDER PROCESS HANDLER!");

    // Check if this is an overlay browser (any browser that's not the main root browser)
    std::string url = frame->GetURL().ToString();
    bool isMainBrowser = (url == "http://127.0.0.1:5137" || url == "http://127.0.0.1:5137/");
    bool isOverlayBrowser = !isMainBrowser && url.find("127.0.0.1:5137") != std::string::npos;

    if (isOverlayBrowser) {
        LOG_DEBUG_RENDER("🎯 OVERLAY BROWSER V8 CONTEXT CREATED!");
        LOG_DEBUG_RENDER("🎯 URL: " + url);
        LOG_DEBUG_RENDER("🎯 Setting up bitcoinBrowser for overlay browser");
    }

    CefRefPtr<CefV8Value> global = context->GetGlobal();

    // Create the bitcoinBrowser object
    CefRefPtr<CefV8Value> bitcoinBrowser = CefV8Value::CreateObject(nullptr, nullptr);
    global->SetValue("bitcoinBrowser", bitcoinBrowser, V8_PROPERTY_ATTRIBUTE_READONLY);

    // Create the identity object inside bitcoinBrowser
    CefRefPtr<CefV8Value> identityObject = CefV8Value::CreateObject(nullptr, nullptr);
    bitcoinBrowser->SetValue("identity", identityObject, V8_PROPERTY_ATTRIBUTE_READONLY);

    // Bind the IdentityHandler instance
    CefRefPtr<IdentityHandler> identityHandler = new IdentityHandler();

    identityObject->SetValue("get",
        CefV8Value::CreateFunction("get", identityHandler),
        V8_PROPERTY_ATTRIBUTE_NONE);

    identityObject->SetValue("markBackedUp",
        CefV8Value::CreateFunction("markBackedUp", identityHandler),
        V8_PROPERTY_ATTRIBUTE_NONE);

    // Create the navigation object inside bitcoinBrowser
    CefRefPtr<CefV8Value> navigationObject = CefV8Value::CreateObject(nullptr, nullptr);
    bitcoinBrowser->SetValue("navigation", navigationObject, V8_PROPERTY_ATTRIBUTE_READONLY);

    // Bind the NavigationHandler instance
    CefRefPtr<NavigationHandler> navigationHandler = new NavigationHandler();

    navigationObject->SetValue("navigate",
        CefV8Value::CreateFunction("navigate", navigationHandler),
        V8_PROPERTY_ATTRIBUTE_NONE);

    // overlayPanel object removed - now using process-per-overlay architecture

    // Create the overlay object (for overlay browsers only)
    if (isOverlayBrowser) {
        LOG_DEBUG_RENDER("🎯 Creating overlay object for URL: " + url);

        CefRefPtr<CefV8Value> overlayObject = CefV8Value::CreateObject(nullptr, nullptr);
        bitcoinBrowser->SetValue("overlay", overlayObject, V8_PROPERTY_ATTRIBUTE_READONLY);

        // Add close method for overlay browsers - uses cefMessage internally
        overlayObject->SetValue("close",
            CefV8Value::CreateFunction("close", new OverlayCloseHandler()),
            V8_PROPERTY_ATTRIBUTE_NONE);

        LOG_DEBUG_RENDER("🎯 Overlay object created with close method");
    } else {
        LOG_DEBUG_RENDER("🎯 NOT creating overlay object for URL: " + url);
        LOG_DEBUG_RENDER("🎯 isMainBrowser: " + std::string(isMainBrowser ? "true" : "false"));
    }

    // Create the address object
    CefRefPtr<CefV8Value> addressObject = CefV8Value::CreateObject(nullptr, nullptr);
    bitcoinBrowser->SetValue("address", addressObject, V8_PROPERTY_ATTRIBUTE_READONLY);

    // Bind AddressHandler
    CefRefPtr<AddressHandler> addressHandler = new AddressHandler();
    addressObject->SetValue("generate",
        CefV8Value::CreateFunction("generate", addressHandler),
        V8_PROPERTY_ATTRIBUTE_NONE);

    // Create the cefMessage object for process communication
    CefRefPtr<CefV8Value> cefMessageObject = CefV8Value::CreateObject(nullptr, nullptr);
    global->SetValue("cefMessage", cefMessageObject, V8_PROPERTY_ATTRIBUTE_READONLY);

    // Create the send function for cefMessage
    CefRefPtr<CefV8Value> sendFunction = CefV8Value::CreateFunction("send", new CefMessageSendHandler());
    cefMessageObject->SetValue("send", sendFunction, V8_PROPERTY_ATTRIBUTE_NONE);

    // Register BRC-100 API
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

        if (message_name == "brc100_auth_request") {
            CefRefPtr<CefListValue> args = message->GetArgumentList();
            std::string domain = args->GetString(0);
            std::string method = args->GetString(1);
            std::string endpoint = args->GetString(2);
            std::string body = args->GetString(3);

            LOG_DEBUG_RENDER("🔐 BRC-100 auth request received: " + domain + " " + method + " " + endpoint);

            // Send message to React component
            std::string js = R"(
                window.dispatchEvent(new MessageEvent('message', {
                    data: {
                        type: 'brc100_auth_request',
                        payload: {
                            domain: ')" + domain + R"(',
                            method: ')" + method + R"(',
                            endpoint: ')" + endpoint + R"(',
                            body: ')" + body + R"('
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
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        std::string responseJson = args->GetString(0);

        std::cout << "✅ Send transaction response received: " << responseJson << std::endl;
        std::ofstream debugLog("debug_output.log", std::ios::app);
        debugLog << "✅ Send transaction response received: " << responseJson << std::endl;
        debugLog.close();

        // Execute JavaScript to call the callback function directly
        std::string js = "if (window.onSendTransactionResponse) { window.onSendTransactionResponse(" + responseJson + "); }";
        frame->ExecuteJavaScript(js, frame->GetURL(), 0);

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

    return false;
}
