#pragma once

#include "include/cef_resource_handler.h"
#include <string>

// Global variable to store pending auth request data
struct PendingAuthRequest {
    std::string domain;
    std::string method;
    std::string endpoint;
    std::string body;
    bool isValid;
    CefRefPtr<CefResourceHandler> handler;
};

// External reference to pending auth request
extern PendingAuthRequest g_pendingAuthRequest;
