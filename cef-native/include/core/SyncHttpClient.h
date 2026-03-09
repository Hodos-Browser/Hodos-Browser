#pragma once

#include <string>

// Cross-platform synchronous HTTP client.
// Windows: WinHTTP, macOS: libcurl.
// Designed for internal localhost requests to wallet (31301) and adblock (31302) backends.

struct HttpResponse {
    int statusCode = 0;
    std::string body;
    bool success = false;
};

class SyncHttpClient {
public:
    // Synchronous GET request. Returns response body and status.
    // timeoutMs applies to connect + transfer combined.
    static HttpResponse Get(const std::string& url, int timeoutMs = 5000);

    // Synchronous POST request with body and content type.
    static HttpResponse Post(const std::string& url,
                             const std::string& body,
                             const std::string& contentType = "application/json",
                             int timeoutMs = 5000);
};
