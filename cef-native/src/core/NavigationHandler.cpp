//NavigationHandler.cpp
#include "include/cef_v8.h"
#include "../../include/core/Logger.h"
#include "include/cef_browser.h"
#include "include/cef_render_process_handler.h"
#include "include/cef_process_message.h"
#include "include/cef_frame.h"
#include "include/wrapper/cef_helpers.h"

#include "../../include/core/NavigationHandler.h"

#include <iostream>

// beta.3 Phase 2b: these were std::cout / std::cerr, which reach NOTHING.
//
// This class runs in the RENDER process, where Logger::Initialize is never called -- so
// Logger::Log falls through to the injected child-process sink and lands in cef_debug.log
// (see include/core/ChildProcessLogSink.h). That is the only sink a sandboxed renderer can
// reach; it cannot open the roaming app-data directory itself.
//
// ProcessType::RENDER (1) so the line is attributable to the right process.
#define LOG_DEBUG_NAV(msg)   Logger::Log(msg, 0, 1)
#define LOG_INFO_NAV(msg)    Logger::Log(msg, 1, 1)
#define LOG_WARNING_NAV(msg) Logger::Log(msg, 2, 1)
#define LOG_ERROR_NAV(msg)   Logger::Log(msg, 3, 1)


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

    // Handle hodos:// custom protocol - redirect to local frontend
    if (path.find("hodos://") == 0) {
        // Replace "hodos://" with "http://127.0.0.1:5137/"
        path.replace(0, 8, "http://127.0.0.1:5137/");
        LOG_DEBUG_NAV(LogFmt() << "📡 hodos:// protocol handler - redirecting to: " << path);
    }

    LOG_DEBUG_NAV(LogFmt() << "📡 Navigation request to: " << path);

    CefRefPtr<CefV8Context> context = CefV8Context::GetCurrentContext();
    CefRefPtr<CefFrame> frame = context->GetFrame();

    if (frame) {
        CefRefPtr<CefProcessMessage> message = CefProcessMessage::Create("navigate");
        message->GetArgumentList()->SetString(0, path);
        frame->SendProcessMessage(PID_BROWSER, message);
    } else {
        LOG_ERROR_NAV(LogFmt() << "💥 Failed to get frame context in NavigationHandler.");
        exception = "Unable to access frame context.";
        return false;
    }

    return true;
}
