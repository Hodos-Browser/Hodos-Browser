#pragma once

#include "include/cef_v8.h"
#include "include/cef_browser.h"
#include "include/cef_frame.h"
#include "include/wrapper/cef_helpers.h"
#include <string>

class AddressHandler : public CefV8Handler {
public:
    AddressHandler();
    ~AddressHandler();

    // CefV8Handler methods
    bool Execute(const CefString& name,
                 CefRefPtr<CefV8Value> object,
                 const CefV8ValueList& arguments,
                 CefRefPtr<CefV8Value>& retval,
                 CefString& exception) override;

private:
    // Address generation method
    CefRefPtr<CefV8Value> generateAddress();

    IMPLEMENT_REFCOUNTING(AddressHandler);
    DISALLOW_COPY_AND_ASSIGN(AddressHandler);
};
