//NavigationHandler.cpp
#include "include/cef_v8.h"
#include "include/cef_browser.h"
#include "include/cef_render_process_handler.h"
#include "include/cef_process_message.h"
#include "include/cef_frame.h"
#include "include/wrapper/cef_helpers.h"

#include "../../include/core/NavigationHandler.h"

#include <iostream>

bool NavigationHandler::Execute(const CefString& name,
                                CefRefPtr<CefV8Value> object,
                                const CefV8ValueList& arguments,
                                CefRefPtr<CefV8Value>& retval,
                                CefString& exception) {
    CEF_REQUIRE_RENDERER_THREAD();

    if (arguments.empty() || !arguments[0]->IsString()) {
        exception = "Expected a string as the first argument to navigate.";
        return false;
    }

    std::string path = arguments[0]->GetStringValue();
    std::cout << "📡 Navigation request to: " << path << std::endl;

    CefRefPtr<CefV8Context> context = CefV8Context::GetCurrentContext();
    CefRefPtr<CefFrame> frame = context->GetFrame();

    if (frame) {
        CefRefPtr<CefProcessMessage> message = CefProcessMessage::Create("navigate");
        message->GetArgumentList()->SetString(0, path);
        frame->SendProcessMessage(PID_BROWSER, message);
    } else {
        std::cerr << "💥 Failed to get frame context in NavigationHandler." << std::endl;
        exception = "Unable to access frame context.";
        return false;
    }

    return true;
}
