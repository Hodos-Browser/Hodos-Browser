#pragma once

#include "include/cef_browser.h"
#include "include/cef_process_message.h"
#include "include/cef_v8.h"
#include <iostream>

class NavigationHandler : public CefV8Handler {
public:
    bool Execute(const CefString& name,
                 CefRefPtr<CefV8Value> object,
                 const CefV8ValueList& arguments,
                 CefRefPtr<CefV8Value>& retval,
                 CefString& exception) override;

    IMPLEMENT_REFCOUNTING(NavigationHandler);
};
