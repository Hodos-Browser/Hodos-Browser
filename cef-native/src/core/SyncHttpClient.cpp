#include "../../include/core/SyncHttpClient.h"
#include "../../include/core/Logger.h"
#include <string>
#include <fstream>
#include <cstdio>

#define LOG_DEBUG_SYNC(msg) Logger::Log(msg, 0, 2)
#define LOG_WARNING_SYNC(msg) Logger::Log(msg, 2, 2)

// =============================================================================
// Windows implementation: WinHTTP
// =============================================================================
#ifdef _WIN32

#include <windows.h>
#include <winhttp.h>
#pragma comment(lib, "winhttp.lib")

// Parse URL into host, port, path components for WinHTTP. Sets isHttps so the
// caller can apply WINHTTP_FLAG_SECURE and the correct default port (443 vs 80).
static bool ParseUrl(const std::string& url, std::wstring& host, INTERNET_PORT& port,
                     std::wstring& path, bool& isHttps) {
    // Expect http(s)://host:port/path
    std::string work = url;
    isHttps = false;
    if (work.substr(0, 7) == "http://") {
        work = work.substr(7);
    } else if (work.substr(0, 8) == "https://") {
        work = work.substr(8);
        isHttps = true;
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
        port = isHttps ? INTERNET_DEFAULT_HTTPS_PORT : INTERNET_DEFAULT_HTTP_PORT;
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
    bool isHttps = false;
    if (!ParseUrl(url, host, port, path, isHttps)) {
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
                                            WINHTTP_DEFAULT_ACCEPT_TYPES,
                                            isHttps ? WINHTTP_FLAG_SECURE : 0);
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

// Stream a GET response straight to a file (no in-memory buffering of the body).
// Used for the large auto-updater installer download. Writes to a sibling
// "<dest>.partial", verifies the transfer is COMPLETE (Content-Length match when
// the server provides it), and only then renames it onto destPath. A
// pre-existing good file at destPath is therefore never clobbered or deleted by
// a failed/truncated download — destPath changes only on a fully-verified 2xx.
static HttpResponse WinHttpDownload(const std::string& url, const std::string& destPath,
                                    int timeoutMs) {
    HttpResponse response;
    const std::string partialPath = destPath + ".partial";

    std::wstring host, path;
    INTERNET_PORT port;
    bool isHttps = false;
    if (!ParseUrl(url, host, port, path, isHttps)) {
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

    // WinHTTP's default redirect policy follows https->https (and disallows the
    // downgrade to http), which is exactly GitHub release-asset behavior — so a
    // GitHub /releases/download/ URL that 302s to objects.githubusercontent.com
    // is followed automatically with no extra config.
    HINTERNET hRequest = WinHttpOpenRequest(hConnect, L"GET", path.c_str(), nullptr,
                                            WINHTTP_NO_REFERER,
                                            WINHTTP_DEFAULT_ACCEPT_TYPES,
                                            isHttps ? WINHTTP_FLAG_SECURE : 0);
    if (!hRequest) {
        WinHttpCloseHandle(hConnect);
        WinHttpCloseHandle(hSession);
        return response;
    }

    bool ok = WinHttpSendRequest(hRequest, WINHTTP_NO_ADDITIONAL_HEADERS, 0,
                                 WINHTTP_NO_REQUEST_DATA, 0, 0, 0)
              && WinHttpReceiveResponse(hRequest, nullptr);

    if (ok) {
        DWORD statusCode = 0;
        DWORD statusSize = sizeof(statusCode);
        WinHttpQueryHeaders(hRequest, WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
                            WINHTTP_HEADER_NAME_BY_INDEX, &statusCode, &statusSize,
                            WINHTTP_NO_HEADER_INDEX);
        response.statusCode = static_cast<int>(statusCode);

        // Expected length, if the server declares it (unsigned 64-bit). 0 ==
        // not provided → we cannot assert completeness from length alone.
        unsigned long long expectedLen = 0;
        {
            wchar_t lenBuf[32] = {0};
            DWORD lenSize = sizeof(lenBuf);
            if (WinHttpQueryHeaders(hRequest, WINHTTP_QUERY_CONTENT_LENGTH,
                                    WINHTTP_HEADER_NAME_BY_INDEX, lenBuf, &lenSize,
                                    WINHTTP_NO_HEADER_INDEX)) {
                expectedLen = _wcstoui64(lenBuf, nullptr, 10);
            }
        }

        // Only commit bytes to disk on a 2xx — never write an error page to the
        // installer path. Write to <dest>.partial; rename onto destPath only
        // after a complete transfer.
        if (statusCode >= 200 && statusCode < 300) {
            std::ofstream out(partialPath, std::ios::binary | std::ios::trunc);
            if (out.is_open()) {
                char buffer[65536];
                DWORD bytesRead = 0;
                bool writeOk = true;
                unsigned long long totalWritten = 0;
                do {
                    if (!WinHttpReadData(hRequest, buffer, sizeof(buffer), &bytesRead)) {
                        writeOk = false;  // mid-stream read error → truncated
                        break;
                    }
                    if (bytesRead > 0) {
                        out.write(buffer, bytesRead);
                        if (!out) { writeOk = false; break; }
                        totalWritten += bytesRead;
                    }
                } while (bytesRead > 0);
                out.close();

                // Completeness gate: a clean read loop AND (if the server gave a
                // Content-Length) a byte-count match. Guards against a connection
                // that drops at EOF being reported as a successful short file.
                bool complete = writeOk && out.good()
                                && (expectedLen == 0 || totalWritten == expectedLen);
                if (complete) {
                    // Atomic-ish replace onto the real destination.
                    if (MoveFileExA(partialPath.c_str(), destPath.c_str(),
                                    MOVEFILE_REPLACE_EXISTING)) {
                        response.success = true;
                    } else {
                        LOG_WARNING_SYNC("SyncHttpClient::Download — rename to dest failed: " + destPath);
                    }
                } else if (expectedLen != 0 && totalWritten != expectedLen) {
                    LOG_WARNING_SYNC("SyncHttpClient::Download — truncated transfer ("
                                     + std::to_string(totalWritten) + "/"
                                     + std::to_string(expectedLen) + " bytes)");
                }
            } else {
                LOG_WARNING_SYNC("SyncHttpClient::Download — cannot open partial file: " + partialPath);
            }
        }
    }

    WinHttpCloseHandle(hRequest);
    WinHttpCloseHandle(hConnect);
    WinHttpCloseHandle(hSession);

    // Never touch destPath on failure (a previously-staged good file survives);
    // only the partial is cleaned up.
    if (!response.success) {
        std::remove(partialPath.c_str());
    }
    return response;
}

HttpResponse SyncHttpClient::Download(const std::string& url, const std::string& destPath,
                                      int timeoutMs) {
    return WinHttpDownload(url, destPath, timeoutMs);
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

    // Follow https redirects (e.g. hodosbrowser.com appcast / GitHub release
    // assets); harmless for localhost backends which never redirect.
    curl_easy_setopt(curl, CURLOPT_FOLLOWLOCATION, 1L);
    curl_easy_setopt(curl, CURLOPT_MAXREDIRS, 5L);

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

// Stream a GET response straight to a file (no in-memory buffering). Writes the
// destination ONLY on a 2xx response; removes the partial on any failure.
static size_t SyncFileWriteCallback(void* contents, size_t size, size_t nmemb, void* userp) {
    size_t totalSize = size * nmemb;
    std::ofstream* out = static_cast<std::ofstream*>(userp);
    out->write(static_cast<char*>(contents), totalSize);
    return out->good() ? totalSize : 0;  // returning < totalSize aborts the transfer
}

HttpResponse SyncHttpClient::Download(const std::string& url, const std::string& destPath,
                                      int timeoutMs) {
    HttpResponse response;
    const std::string partialPath = destPath + ".partial";

    CURL* curl = curl_easy_init();
    if (!curl) {
        LOG_WARNING_SYNC("SyncHttpClient::Download — failed to init libcurl");
        return response;
    }

    // Write to <dest>.partial; rename onto destPath only on a complete transfer
    // so a pre-existing good file is never clobbered by a failed download.
    std::ofstream out(partialPath, std::ios::binary | std::ios::trunc);
    if (!out.is_open()) {
        LOG_WARNING_SYNC("SyncHttpClient::Download — cannot open partial file: " + partialPath);
        curl_easy_cleanup(curl);
        return response;
    }

    curl_easy_setopt(curl, CURLOPT_URL, url.c_str());
    curl_easy_setopt(curl, CURLOPT_WRITEFUNCTION, SyncFileWriteCallback);
    curl_easy_setopt(curl, CURLOPT_WRITEDATA, &out);
    curl_easy_setopt(curl, CURLOPT_FOLLOWLOCATION, 1L);
    curl_easy_setopt(curl, CURLOPT_MAXREDIRS, 5L);
    curl_easy_setopt(curl, CURLOPT_FAILONERROR, 1L);  // treat >=400 as a curl error
    // CURLE_PARTIAL_FILE if the transfer is shorter than a declared Content-Length.
    curl_easy_setopt(curl, CURLOPT_TIMEOUT_MS, static_cast<long>(timeoutMs));
    curl_easy_setopt(curl, CURLOPT_CONNECTTIMEOUT_MS, static_cast<long>(timeoutMs));

    CURLcode res = curl_easy_perform(curl);
    out.close();

    long httpCode = 0;
    curl_easy_getinfo(curl, CURLINFO_RESPONSE_CODE, &httpCode);
    response.statusCode = static_cast<int>(httpCode);

    bool complete = (res == CURLE_OK) && out.good() && (httpCode >= 200 && httpCode < 300);
    if (complete && std::rename(partialPath.c_str(), destPath.c_str()) == 0) {
        response.success = true;
    }
    curl_easy_cleanup(curl);

    if (!response.success) {
        LOG_DEBUG_SYNC("SyncHttpClient::Download failed for " + url + " (curl="
                       + std::to_string(res) + ", http=" + std::to_string(httpCode) + ")");
        std::remove(partialPath.c_str());  // never touch destPath on failure
    }
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
HttpResponse SyncHttpClient::Download(const std::string& url, const std::string& destPath,
                                      int timeoutMs) {
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
