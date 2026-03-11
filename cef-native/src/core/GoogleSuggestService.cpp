#include "GoogleSuggestService.h"
#include "Logger.h"
#include <sstream>
#include <iostream>
#include <iomanip>
#include <nlohmann/json.hpp>

#ifdef __APPLE__
#include <curl/curl.h>

static size_t GoogleSuggestWriteCallback(void* contents, size_t size, size_t nmemb, void* userp) {
    size_t totalSize = size * nmemb;
    std::string* resp = static_cast<std::string*>(userp);
    resp->append(static_cast<char*>(contents), totalSize);
    return totalSize;
}
#endif

// Convenience macros for logging
#define LOG_DEBUG_GOOGLE(msg) Logger::Log(msg, 0, 2)
#define LOG_ERROR_GOOGLE(msg) Logger::Log(msg, 3, 2)

GoogleSuggestService& GoogleSuggestService::GetInstance() {
    static GoogleSuggestService instance;
    return instance;
}

GoogleSuggestService::GoogleSuggestService()
    :
#ifdef _WIN32
      hSession_(nullptr),
#endif
      initialized_(false) {
    initialize();
}

GoogleSuggestService::~GoogleSuggestService() {
    cleanup();
}

bool GoogleSuggestService::initialize() {
#ifdef _WIN32
    // Initialize WinHTTP session
    hSession_ = WinHttpOpen(L"HodosBrowser/1.0",
                           WINHTTP_ACCESS_TYPE_DEFAULT_PROXY,
                           WINHTTP_NO_PROXY_NAME,
                           WINHTTP_NO_PROXY_BYPASS,
                           0);

    if (!hSession_) {
        LOG_ERROR_GOOGLE("Failed to initialize WinHTTP session for Google Suggest");
        return false;
    }

    initialized_ = true;
    LOG_DEBUG_GOOGLE("GoogleSuggestService initialized");
    return true;
#elif defined(__APPLE__)
    initialized_ = true;
    LOG_DEBUG_GOOGLE("GoogleSuggestService initialized (macOS/libcurl)");
    return true;
#else
    LOG_ERROR_GOOGLE("GoogleSuggestService not supported on this platform");
    return false;
#endif
}

void GoogleSuggestService::cleanup() {
#ifdef _WIN32
    if (hSession_) {
        WinHttpCloseHandle(hSession_);
        hSession_ = nullptr;
    }
    initialized_ = false;
#elif defined(__APPLE__)
    initialized_ = false;
#endif
}

std::string GoogleSuggestService::urlEncode(const std::string& str) {
    std::ostringstream escaped;
    escaped.fill('0');
    escaped << std::hex;

    for (char c : str) {
        // Keep alphanumeric and other safe characters
        if (isalnum(static_cast<unsigned char>(c)) || c == '-' || c == '_' || c == '.' || c == '~') {
            escaped << c;
        }
        // Encode space as +
        else if (c == ' ') {
            escaped << '+';
        }
        // Encode everything else
        else {
            escaped << '%' << std::setw(2) << int(static_cast<unsigned char>(c));
        }
    }

    return escaped.str();
}

std::vector<std::string> GoogleSuggestService::fetchSuggestions(const std::string& query, const std::string& engine) {
    std::vector<std::string> suggestions;

#ifdef _WIN32
    if (!initialized_ || !hSession_) {
        LOG_ERROR_GOOGLE("GoogleSuggestService not initialized");
        return suggestions; // Return empty vector
    }

    // Don't call API for very short queries
    if (query.length() < 2) {
        return suggestions;
    }

    bool useDDG = (engine != "google");
    LOG_DEBUG_GOOGLE("Fetching suggestions for query: " + query + " (engine: " + engine + ")");

    std::string encodedQuery = urlEncode(query);
    std::wstring host;
    std::wstring path;

    if (useDDG) {
        host = L"duckduckgo.com";
        path = L"/ac/?q=" + std::wstring(encodedQuery.begin(), encodedQuery.end());
    } else {
        host = L"suggestqueries.google.com";
        path = L"/complete/search?client=chrome&q=" +
               std::wstring(encodedQuery.begin(), encodedQuery.end()) +
               L"&hl=en";
    }

    // Connect to suggest server
    HINTERNET hConnect = WinHttpConnect(hSession_, host.c_str(),
                                       INTERNET_DEFAULT_HTTPS_PORT, 0);
    if (!hConnect) {
        LOG_ERROR_GOOGLE("Failed to connect to suggest server");
        return suggestions; // Return empty vector
    }

    // Open HTTPS request
    HINTERNET hRequest = WinHttpOpenRequest(hConnect, L"GET", path.c_str(),
                                           nullptr, WINHTTP_NO_REFERER,
                                           WINHTTP_DEFAULT_ACCEPT_TYPES,
                                           WINHTTP_FLAG_SECURE);

    if (!hRequest) {
        LOG_ERROR_GOOGLE("Failed to create Google Suggest request");
        WinHttpCloseHandle(hConnect);
        return suggestions; // Return empty vector
    }

    // Set timeout (5 seconds)
    DWORD timeout = 5000;
    WinHttpSetOption(hRequest, WINHTTP_OPTION_CONNECT_TIMEOUT, &timeout, sizeof(timeout));
    WinHttpSetOption(hRequest, WINHTTP_OPTION_RECEIVE_TIMEOUT, &timeout, sizeof(timeout));

    // Send request
    if (!WinHttpSendRequest(hRequest, WINHTTP_NO_ADDITIONAL_HEADERS, 0,
                           WINHTTP_NO_REQUEST_DATA, 0, 0, 0)) {
        LOG_ERROR_GOOGLE("Failed to send Google Suggest request");
        WinHttpCloseHandle(hRequest);
        WinHttpCloseHandle(hConnect);
        return suggestions; // Return empty vector
    }

    // Receive response
    if (!WinHttpReceiveResponse(hRequest, nullptr)) {
        LOG_ERROR_GOOGLE("Failed to receive Google Suggest response");
        WinHttpCloseHandle(hRequest);
        WinHttpCloseHandle(hConnect);
        return suggestions; // Return empty vector
    }

    // Check status code
    DWORD statusCode = 0;
    DWORD statusCodeSize = sizeof(statusCode);
    WinHttpQueryHeaders(hRequest, WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
                       nullptr, &statusCode, &statusCodeSize, nullptr);

    if (statusCode != 200) {
        LOG_ERROR_GOOGLE("Google Suggest returned status code: " + std::to_string(statusCode));
        WinHttpCloseHandle(hRequest);
        WinHttpCloseHandle(hConnect);
        return suggestions; // Return empty vector
    }

    // Read response data
    std::string responseData;
    DWORD bytesRead = 0;
    char buffer[4096];

    do {
        bytesRead = 0;
        if (!WinHttpReadData(hRequest, buffer, sizeof(buffer), &bytesRead)) {
            LOG_ERROR_GOOGLE("Failed to read Google Suggest response data");
            break;
        }
        if (bytesRead > 0) {
            responseData.append(buffer, bytesRead);
        }
    } while (bytesRead > 0);

    WinHttpCloseHandle(hRequest);
    WinHttpCloseHandle(hConnect);

    if (responseData.empty()) {
        LOG_ERROR_GOOGLE("Empty response from Google Suggest");
        return suggestions; // Return empty vector
    }

    LOG_DEBUG_GOOGLE("Raw response (first 500 chars): " + responseData.substr(0, 500));

    // Parse JSON response using nlohmann::json
    try {
        nlohmann::json jsonResponse = nlohmann::json::parse(responseData);

        if (useDDG) {
            // DDG format: [{"phrase":"s1"}, {"phrase":"s2"}, ...]
            if (!jsonResponse.is_array()) {
                LOG_ERROR_GOOGLE("Invalid DDG response: not an array");
                return suggestions;
            }
            for (const auto& item : jsonResponse) {
                if (item.is_object() && item.contains("phrase") && item["phrase"].is_string()) {
                    std::string phrase = item["phrase"].get<std::string>();
                    if (!phrase.empty()) {
                        suggestions.push_back(phrase);
                    }
                }
            }
        } else {
            // Google format: ["query", ["s1", "s2", ...], [], {...}]
            if (!jsonResponse.is_array() || jsonResponse.size() < 2) {
                LOG_ERROR_GOOGLE("Invalid Google response: not an array or too few elements");
                return suggestions;
            }
            const auto& suggestionsArray = jsonResponse[1];
            if (!suggestionsArray.is_array()) {
                LOG_ERROR_GOOGLE("Invalid Google response: suggestions element is not an array");
                return suggestions;
            }
            for (const auto& suggestion : suggestionsArray) {
                if (suggestion.is_string()) {
                    std::string suggestionStr = suggestion.get<std::string>();
                    if (!suggestionStr.empty()) {
                        suggestions.push_back(suggestionStr);
                    }
                }
            }
        }

        LOG_DEBUG_GOOGLE("Parsed " + std::to_string(suggestions.size()) + " suggestions from " + engine);

    } catch (const nlohmann::json::exception& e) {
        LOG_ERROR_GOOGLE("JSON parsing failed: " + std::string(e.what()));
        LOG_ERROR_GOOGLE("Response data: " + responseData.substr(0, 200));
        return suggestions; // Return empty vector on parse error
    }

#elif defined(__APPLE__)
    if (!initialized_) {
        LOG_ERROR_GOOGLE("GoogleSuggestService not initialized");
        return suggestions;
    }

    // Don't call API for very short queries
    if (query.length() < 2) {
        return suggestions;
    }

    bool useDDG = (engine != "google");
    LOG_DEBUG_GOOGLE("Fetching suggestions for query: " + query + " (engine: " + engine + ")");

    std::string encodedQuery = urlEncode(query);
    std::string url;

    if (useDDG) {
        url = "https://duckduckgo.com/ac/?q=" + encodedQuery;
    } else {
        url = "https://suggestqueries.google.com/complete/search?client=chrome&q=" + encodedQuery + "&hl=en";
    }

    CURL* curl = curl_easy_init();
    if (!curl) {
        LOG_ERROR_GOOGLE("Failed to init libcurl for Google Suggest");
        return suggestions;
    }

    std::string responseData;

    curl_easy_setopt(curl, CURLOPT_URL, url.c_str());
    curl_easy_setopt(curl, CURLOPT_WRITEFUNCTION, GoogleSuggestWriteCallback);
    curl_easy_setopt(curl, CURLOPT_WRITEDATA, &responseData);
    curl_easy_setopt(curl, CURLOPT_TIMEOUT_MS, 5000L);
    curl_easy_setopt(curl, CURLOPT_CONNECTTIMEOUT_MS, 5000L);
    curl_easy_setopt(curl, CURLOPT_USERAGENT, "HodosBrowser/1.0");

    CURLcode res = curl_easy_perform(curl);

    if (res != CURLE_OK) {
        LOG_ERROR_GOOGLE("libcurl error: " + std::string(curl_easy_strerror(res)));
        curl_easy_cleanup(curl);
        return suggestions;
    }

    long httpCode = 0;
    curl_easy_getinfo(curl, CURLINFO_RESPONSE_CODE, &httpCode);
    curl_easy_cleanup(curl);

    if (httpCode != 200) {
        LOG_ERROR_GOOGLE("Google Suggest returned status code: " + std::to_string(httpCode));
        return suggestions;
    }

    if (responseData.empty()) {
        LOG_ERROR_GOOGLE("Empty response from Google Suggest");
        return suggestions;
    }

    LOG_DEBUG_GOOGLE("Raw response (first 500 chars): " + responseData.substr(0, 500));

    // Parse JSON response using nlohmann::json
    try {
        nlohmann::json jsonResponse = nlohmann::json::parse(responseData);

        if (useDDG) {
            // DDG format: [{"phrase":"s1"}, {"phrase":"s2"}, ...]
            if (!jsonResponse.is_array()) {
                LOG_ERROR_GOOGLE("Invalid DDG response: not an array");
                return suggestions;
            }
            for (const auto& item : jsonResponse) {
                if (item.is_object() && item.contains("phrase") && item["phrase"].is_string()) {
                    std::string phrase = item["phrase"].get<std::string>();
                    if (!phrase.empty()) {
                        suggestions.push_back(phrase);
                    }
                }
            }
        } else {
            // Google format: ["query", ["s1", "s2", ...], [], {...}]
            if (!jsonResponse.is_array() || jsonResponse.size() < 2) {
                LOG_ERROR_GOOGLE("Invalid Google response: not an array or too few elements");
                return suggestions;
            }
            const auto& suggestionsArray = jsonResponse[1];
            if (!suggestionsArray.is_array()) {
                LOG_ERROR_GOOGLE("Invalid Google response: suggestions element is not an array");
                return suggestions;
            }
            for (const auto& suggestion : suggestionsArray) {
                if (suggestion.is_string()) {
                    std::string suggestionStr = suggestion.get<std::string>();
                    if (!suggestionStr.empty()) {
                        suggestions.push_back(suggestionStr);
                    }
                }
            }
        }

        LOG_DEBUG_GOOGLE("Parsed " + std::to_string(suggestions.size()) + " suggestions from " + engine);

    } catch (const nlohmann::json::exception& e) {
        LOG_ERROR_GOOGLE("JSON parsing failed: " + std::string(e.what()));
        LOG_ERROR_GOOGLE("Response data: " + responseData.substr(0, 200));
        return suggestions;
    }

#else
    LOG_ERROR_GOOGLE("GoogleSuggestService not supported on this platform");
#endif

    return suggestions;
}
