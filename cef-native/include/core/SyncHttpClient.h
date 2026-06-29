#pragma once

#include <map>
#include <string>

// Cross-platform synchronous HTTP client.
// Windows: WinHTTP, macOS: libcurl.
// Originally for internal localhost requests to wallet (31301) and adblock
// (31302) backends; also supports external https:// hosts (auto-updater appcast
// + installer fetch) with redirect following.

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

    // GET with custom headers (Phase 2.5 — wallet IPC bridge needs to
    // propagate X-Requesting-Domain from the calling frame's origin).
    // Content-Type is unset by default for GET (no body); add it via
    // headers if a specific endpoint requires it.
    static HttpResponse Get(const std::string& url,
                            const std::map<std::string, std::string>& headers,
                            int timeoutMs = 5000);

    // Synchronous POST request with body and content type.
    static HttpResponse Post(const std::string& url,
                             const std::string& body,
                             const std::string& contentType = "application/json",
                             int timeoutMs = 5000);

    // POST with custom headers map. Content-Type defaults to application/json if
    // not present in headers; an explicit Content-Type entry in headers wins.
    static HttpResponse Post(const std::string& url,
                             const std::string& body,
                             const std::map<std::string, std::string>& headers,
                             int timeoutMs = 5000);

    // Stream a (potentially large, binary) GET response straight to a file on
    // disk instead of buffering it in memory. The auto-updater uses this to
    // fetch the installer (~95 MB) without holding it all in a std::string.
    // Supports https:// + redirects. Writes to "<destPath>.partial" and renames
    // it onto destPath ONLY after a COMPLETE 2xx transfer (Content-Length match
    // when the server provides one). A non-2xx, truncated, or transport failure
    // leaves destPath untouched (a previously-staged good file survives) and
    // removes the partial. Returns statusCode + success; body stays empty.
    // Default timeout is generous (120 s) for a large transfer.
    static HttpResponse Download(const std::string& url,
                                 const std::string& destPath,
                                 int timeoutMs = 120000);

    // Generic method dispatch (GET / POST / DELETE / PUT / etc.) for callers
    // that need a verb the typed methods above don't cover. The wallet IPC
    // bridge uses this for DELETE on /domain/permissions and similar revoke
    // endpoints. body is sent only for methods that conventionally carry one;
    // for GET / DELETE pass an empty body.
    static HttpResponse Request(const std::string& method,
                                const std::string& url,
                                const std::string& body,
                                const std::map<std::string, std::string>& headers,
                                int timeoutMs = 5000);
};
