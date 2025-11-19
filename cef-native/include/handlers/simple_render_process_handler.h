// cef_native/src/simple_render_process_handler.h
#pragma once

#include "include/cef_render_process_handler.h"
#include "include/cef_v8.h"

class SimpleRenderProcessHandler : public CefRenderProcessHandler {
public:
    SimpleRenderProcessHandler();

    void OnContextCreated(
        CefRefPtr<CefBrowser> browser,
        CefRefPtr<CefFrame> frame,
        CefRefPtr<CefV8Context> context) override;

    bool OnProcessMessageReceived(
        CefRefPtr<CefBrowser> browser,
        CefRefPtr<CefFrame> frame,
        CefProcessId source_process,
        CefRefPtr<CefProcessMessage> message) override;

private:
    IMPLEMENT_REFCOUNTING(SimpleRenderProcessHandler);
};
