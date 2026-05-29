#include "../../include/core/SyncHttpClient.h"
#include "../../include/core/Logger.h"
#include <string>

#define LOG_DEBUG_SYNC(msg) Logger::Log(msg, 0, 2)
#define LOG_WARNING_SYNC(msg) Logger::Log(msg, 2, 2)

// =============================================================================
// Windows implementation: WinHTTP
// =============================================================================
#ifdef _WIN32

#include <windows.h>
#include <winhttp.h>
#pragma comment(lib, "winhttp.lib")

// Parse URL into host, port, path components for WinHTTP
static bool ParseUrl(const std::string& url, std::wstring& host, INTERNET_PORT& port, std::wstring& path) {
    // Expect http://host:port/path
    std::string work = url;
    if (work.substr(0, 7) == "http://") {
        work = work.substr(7);
    } else if (work.substr(0, 8) == "https://") {
        work = work.substr(8);
    }

    // Split host:port from path
    size_t pathStart = work.find('/');
    std::string hostPort = (pathStart != std::string::npos) ? work.substr(0, pathStart) : work;
    std::string pathStr = (pathStart != std::string::npos) ? work.substr(pathStart) : "/";

    // Split host from port
    size_t colonPos = hostPort.find(':');
    std::string hostStr;
    if (colonPos != std::string::npos) {
        hostStr = hostPort.substr(0, colonPos);
        port = static_cast<INTERNET_PORT>(std::stoi(hostPort.substr(colonPos + 1)));
    } else {
        hostStr = hostPort;
        port = INTERNET_DEFAULT_HTTP_PORT;
    }

    host = std::wstring(hostStr.begin(), hostStr.end());
    path = std::wstring(pathStr.begin(), pathStr.end());
    return true;
}

static HttpResponse WinHttpRequest(const std::string& method, const std::string& url,
                                   const std::string& body,
                                   const std::map<std::string, std::string>& headers,
                                   int timeoutMs) {
    HttpResponse response;

    std::wstring host, path;
    INTERNET_PORT port;
    if (!ParseUrl(url, host, port, path)) {
        return response;
    }

    HINTERNET hSession = WinHttpOpen(L"SyncHttpClient/1.0",
                                     WINHTTP_ACCESS_TYPE_DEFAULT_PROXY,
                                     WINHTTP_NO_PROXY_NAME,
                                     WINHTTP_NO_PROXY_BYPASS, 0);
    if (!hSession) return response;

    DWORD timeout = static_cast<DWORD>(timeoutMs);
    WinHttpSetOption(hSession, WINHTTP_OPTION_CONNECT_TIMEOUT, &timeout, sizeof(timeout));
    WinHttpSetOption(hSession, WINHTTP_OPTION_RECEIVE_TIMEOUT, &timeout, sizeof(timeout));
    WinHttpSetOption(hSession, WINHTTP_OPTION_SEND_TIMEOUT, &timeout, sizeof(timeout));

    HINTERNET hConnect = WinHttpConnect(hSession, host.c_str(), port, 0);
    if (!hConnect) {
        WinHttpCloseHandle(hSession);
        return response;
    }

    std::wstring wideMethod(method.begin(), method.end());
    HINTERNET hRequest = WinHttpOpenRequest(hConnect, wideMethod.c_str(),
                                            path.c_str(),
                                            nullptr,
                                            WINHTTP_NO_REFERER,
                                            WINHTTP_DEFAULT_ACCEPT_TYPES, 0);
    if (!hRequest) {
        WinHttpCloseHandle(hConnect);
        WinHttpCloseHandle(hSession);
        return response;
    }

    // Assemble headers as a single CRLF-joined string for WinHttpSendRequest.
    // WinHTTP wants "Header-Name: value\r\nOther-Header: value" — no trailing CRLF.
    std::string asciiHeaders;
    for (const auto& kv : headers) {
        if (!asciiHeaders.empty()) asciiHeaders += "\r\n";
        asciiHeaders += kv.first + ": " + kv.second;
    }
    std::wstring wideHeaders(asciiHeaders.begin(), asciiHeaders.end());

    BOOL sendOk;
    if (!body.empty()) {
        sendOk = WinHttpSendRequest(hRequest,
                                    wideHeaders.empty() ? WINHTTP_NO_ADDITIONAL_HEADERS : wideHeaders.c_str(),
                                    wideHeaders.empty() ? 0 : static_cast<DWORD>(wideHeaders.length()),
                                    (LPVOID)body.c_str(), static_cast<DWORD>(body.size()),
                                    static_cast<DWORD>(body.size()), 0);
    } else {
        sendOk = WinHttpSendRequest(hRequest,
                                    wideHeaders.empty() ? WINHTTP_NO_ADDITIONAL_HEADERS : wideHeaders.c_str(),
                                    wideHeaders.empty() ? 0 : static_cast<DWORD>(wideHeaders.length()),
                                    WINHTTP_NO_REQUEST_DATA, 0, 0, 0);
    }

    if (!sendOk || !WinHttpReceiveResponse(hRequest, nullptr)) {
        WinHttpCloseHandle(hRequest);
        WinHttpCloseHandle(hConnect);
        WinHttpCloseHandle(hSession);
        return response;
    }

    // Read response body
    DWORD bytesRead = 0;
    char buffer[4096];
    do {
        if (!WinHttpReadData(hRequest, buffer, sizeof(buffer), &bytesRead)) break;
        response.body.append(buffer, bytesRead);
    } while (bytesRead > 0);

    // Get status code
    DWORD statusCode = 0;
    DWORD statusSize = sizeof(statusCode);
    WinHttpQueryHeaders(hRequest, WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
                        WINHTTP_HEADER_NAME_BY_INDEX, &statusCode, &statusSize,
                        WINHTTP_NO_HEADER_INDEX);
    response.statusCode = static_cast<int>(statusCode);
    response.success = (statusCode >= 200 && statusCode < 300);

    WinHttpCloseHandle(hRequest);
    WinHttpCloseHandle(hConnect);
    WinHttpCloseHandle(hSession);
    return response;
}

HttpResponse SyncHttpClient::Get(const std::string& url, int timeoutMs) {
    return WinHttpRequest("GET", url, "", {}, timeoutMs);
}

HttpResponse SyncHttpClient::Get(const std::string& url,
                                 const std::map<std::string, std::string>& headers,
                                 int timeoutMs) {
    return WinHttpRequest("GET", url, "", headers, timeoutMs);
}

HttpResponse SyncHttpClient::Post(const std::string& url, const std::string& body,
                                  const std::string& contentType, int timeoutMs) {
    std::map<std::string, std::string> headers;
    if (!contentType.empty()) headers["Content-Type"] = contentType;
    return WinHttpRequest("POST", url, body, headers, timeoutMs);
}

HttpResponse SyncHttpClient::Post(const std::string& url, const std::string& body,
                                  const std::map<std::string, std::string>& headers,
                                  int timeoutMs) {
    // Default Content-Type to application/json if caller did not specify one.
    std::map<std::string, std::string> merged = headers;
    if (merged.find("Content-Type") == merged.end()) {
        merged["Content-Type"] = "application/json";
    }
    return WinHttpRequest("POST", url, body, merged, timeoutMs);
}

HttpResponse SyncHttpClient::Request(const std::string& method, const std::string& url,
                                     const std::string& body,
                                     const std::map<std::string, std::string>& headers,
                                     int timeoutMs) {
    return WinHttpRequest(method, url, body, headers, timeoutMs);
}

// =============================================================================
// macOS implementation: libcurl
// =============================================================================
#elif defined(__APPLE__)

#include <curl/curl.h>

static size_t SyncWriteCallback(void* contents, size_t size, size_t nmemb, void* userp) {
    size_t totalSize = size * nmemb;
    std::string* resp = static_cast<std::string*>(userp);
    resp->append(static_cast<char*>(contents), totalSize);
    return totalSize;
}

static HttpResponse CurlRequest(const std::string& method, const std::string& url,
                                const std::string& body,
                                const std::map<std::string, std::string>& headers,
                                int timeoutMs) {
    HttpResponse response;

    CURL* curl = curl_easy_init();
    if (!curl) {
        LOG_WARNING_SYNC("SyncHttpClient: failed to init libcurl");
        return response;
    }

    curl_easy_setopt(curl, CURLOPT_URL, url.c_str());
    curl_easy_setopt(curl, CURLOPT_WRITEFUNCTION, SyncWriteCallback);
    curl_easy_setopt(curl, CURLOPT_WRITEDATA, &response.body);

    // Timeout: convert ms to seconds (libcurl uses seconds for CURLOPT_TIMEOUT)
    // Use CURLOPT_TIMEOUT_MS for millisecond precision
    curl_easy_setopt(curl, CURLOPT_TIMEOUT_MS, static_cast<long>(timeoutMs));
    curl_easy_setopt(curl, CURLOPT_CONNECTTIMEOUT_MS, static_cast<long>(timeoutMs));

    // Headers
    struct curl_slist* curlHeaders = nullptr;
    for (const auto& kv : headers) {
        std::string headerLine = kv.first + ": " + kv.second;
        curlHeaders = curl_slist_append(curlHeaders, headerLine.c_str());
    }
    if (curlHeaders) {
        curl_easy_setopt(curl, CURLOPT_HTTPHEADER, curlHeaders);
    }

    // Method + body
    if (method == "POST") {
        curl_easy_setopt(curl, CURLOPT_POST, 1L);
        curl_easy_setopt(curl, CURLOPT_POSTFIELDS, body.c_str());
        curl_easy_setopt(curl, CURLOPT_POSTFIELDSIZE, static_cast<long>(body.size()));
    }

    CURLcode res = curl_easy_perform(curl);

    if (curlHeaders) {
        curl_slist_free_all(curlHeaders);
    }

    if (res != CURLE_OK) {
        LOG_DEBUG_SYNC("SyncHttpClient: curl error for " + url + ": " + std::string(curl_easy_strerror(res)));
        curl_easy_cleanup(curl);
        return response;
    }

    long httpCode = 0;
    curl_easy_getinfo(curl, CURLINFO_RESPONSE_CODE, &httpCode);
    response.statusCode = static_cast<int>(httpCode);
    response.success = (httpCode >= 200 && httpCode < 300);

    curl_easy_cleanup(curl);
    return response;
}

HttpResponse SyncHttpClient::Get(const std::string& url, int timeoutMs) {
    return CurlRequest("GET", url, "", {}, timeoutMs);
}

HttpResponse SyncHttpClient::Get(const std::string& url,
                                 const std::map<std::string, std::string>& headers,
                                 int timeoutMs) {
    return CurlRequest("GET", url, "", headers, timeoutMs);
}

HttpResponse SyncHttpClient::Post(const std::string& url, const std::string& body,
                                  const std::string& contentType, int timeoutMs) {
    std::map<std::string, std::string> headers;
    if (!contentType.empty()) headers["Content-Type"] = contentType;
    return CurlRequest("POST", url, body, headers, timeoutMs);
}

HttpResponse SyncHttpClient::Post(const std::string& url, const std::string& body,
                                  const std::map<std::string, std::string>& headers,
                                  int timeoutMs) {
    std::map<std::string, std::string> merged = headers;
    if (merged.find("Content-Type") == merged.end()) {
        merged["Content-Type"] = "application/json";
    }
    return CurlRequest("POST", url, body, merged, timeoutMs);
}

HttpResponse SyncHttpClient::Request(const std::string& method, const std::string& url,
                                     const std::string& body,
                                     const std::map<std::string, std::string>& headers,
                                     int timeoutMs) {
    // libcurl's CurlRequest helper only honors a body when method == "POST". For
    // DELETE / PUT / PATCH we need CURLOPT_CUSTOMREQUEST and CURLOPT_POSTFIELDS
    // to be set explicitly. Open-coded here to keep CurlRequest unchanged for
    // the typed Get/Post callers.
    HttpResponse response;
    CURL* curl = curl_easy_init();
    if (!curl) {
        LOG_WARNING_SYNC("SyncHttpClient: failed to init libcurl");
        return response;
    }

    curl_easy_setopt(curl, CURLOPT_URL, url.c_str());
    curl_easy_setopt(curl, CURLOPT_WRITEFUNCTION, SyncWriteCallback);
    curl_easy_setopt(curl, CURLOPT_WRITEDATA, &response.body);
    curl_easy_setopt(curl, CURLOPT_TIMEOUT_MS, static_cast<long>(timeoutMs));
    curl_easy_setopt(curl, CURLOPT_CONNECTTIMEOUT_MS, static_cast<long>(timeoutMs));
    curl_easy_setopt(curl, CURLOPT_CUSTOMREQUEST, method.c_str());

    struct curl_slist* curlHeaders = nullptr;
    for (const auto& kv : headers) {
        std::string headerLine = kv.first + ": " + kv.second;
        curlHeaders = curl_slist_append(curlHeaders, headerLine.c_str());
    }
    if (curlHeaders) curl_easy_setopt(curl, CURLOPT_HTTPHEADER, curlHeaders);

    if (!body.empty()) {
        curl_easy_setopt(curl, CURLOPT_POSTFIELDS, body.c_str());
        curl_easy_setopt(curl, CURLOPT_POSTFIELDSIZE, static_cast<long>(body.size()));
    }

    CURLcode res = curl_easy_perform(curl);
    if (curlHeaders) curl_slist_free_all(curlHeaders);

    if (res != CURLE_OK) {
        LOG_DEBUG_SYNC("SyncHttpClient::Request curl error: " + std::string(curl_easy_strerror(res)));
        curl_easy_cleanup(curl);
        return response;
    }

    long httpCode = 0;
    curl_easy_getinfo(curl, CURLINFO_RESPONSE_CODE, &httpCode);
    response.statusCode = static_cast<int>(httpCode);
    response.success = (httpCode >= 200 && httpCode < 300);
    curl_easy_cleanup(curl);
    return response;
}

#else
// Fallback for other platforms
HttpResponse SyncHttpClient::Get(const std::string& url, int timeoutMs) {
    return HttpResponse{};
}
HttpResponse SyncHttpClient::Get(const std::string& url,
                                 const std::map<std::string, std::string>& headers,
                                 int timeoutMs) {
    return HttpResponse{};
}
HttpResponse SyncHttpClient::Post(const std::string& url, const std::string& body,
                                  const std::string& contentType, int timeoutMs) {
    return HttpResponse{};
}
HttpResponse SyncHttpClient::Post(const std::string& url, const std::string& body,
                                  const std::map<std::string, std::string>& headers,
                                  int timeoutMs) {
    return HttpResponse{};
}
HttpResponse SyncHttpClient::Request(const std::string& method, const std::string& url,
                                     const std::string& body,
                                     const std::map<std::string, std::string>& headers,
                                     int timeoutMs) {
    return HttpResponse{};
}
#endif
