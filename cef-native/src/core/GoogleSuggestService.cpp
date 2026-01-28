#include "GoogleSuggestService.h"
#include "Logger.h"
#include <sstream>
#include <iostream>
#include <iomanip>

// Convenience macros for logging
#define LOG_DEBUG_GOOGLE(msg) Logger::Log(msg, 0, 2)
#define LOG_ERROR_GOOGLE(msg) Logger::Log(msg, 3, 2)

GoogleSuggestService& GoogleSuggestService::GetInstance() {
    static GoogleSuggestService instance;
    return instance;
}

GoogleSuggestService::GoogleSuggestService()
    : hSession_(nullptr), initialized_(false) {
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
#else
    LOG_ERROR_GOOGLE("GoogleSuggestService only supported on Windows");
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

std::vector<std::string> GoogleSuggestService::fetchSuggestions(const std::string& query) {
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

    LOG_DEBUG_GOOGLE("Fetching Google suggestions for query: " + query);

    // Build URL: https://suggestqueries.google.com/complete/search?client=chrome&q={query}&hl=en
    std::string encodedQuery = urlEncode(query);
    std::wstring path = L"/complete/search?client=chrome&q=" +
                        std::wstring(encodedQuery.begin(), encodedQuery.end()) +
                        L"&hl=en";

    // Connect to Google Suggest server
    HINTERNET hConnect = WinHttpConnect(hSession_, L"suggestqueries.google.com",
                                       INTERNET_DEFAULT_HTTPS_PORT, 0);
    if (!hConnect) {
        LOG_ERROR_GOOGLE("Failed to connect to Google Suggest server");
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

    // Parse JSON response
    // Response format: ["query", ["suggestion1", "suggestion2", ...], [], {"google:suggesttype":[...]}]
    // We need to extract the second array (index 1)

    // Simple manual parsing for the array (avoiding nlohmann::json dependency issues)
    // Find the second array opening bracket
    size_t firstArrayStart = responseData.find('[');
    if (firstArrayStart == std::string::npos) {
        LOG_ERROR_GOOGLE("Invalid Google Suggest response format (no array)");
        return suggestions;
    }

    size_t secondArrayStart = responseData.find('[', firstArrayStart + 1);
    if (secondArrayStart == std::string::npos) {
        LOG_ERROR_GOOGLE("Invalid Google Suggest response format (no second array)");
        return suggestions;
    }

    size_t secondArrayEnd = responseData.find(']', secondArrayStart + 1);
    if (secondArrayEnd == std::string::npos) {
        LOG_ERROR_GOOGLE("Invalid Google Suggest response format (unclosed second array)");
        return suggestions;
    }

    // Extract suggestions array content
    std::string suggestionsStr = responseData.substr(secondArrayStart + 1,
                                                     secondArrayEnd - secondArrayStart - 1);

    // Parse individual suggestions (simple string split on quotes)
    size_t pos = 0;
    bool inQuote = false;
    std::string currentSuggestion;

    for (size_t i = 0; i < suggestionsStr.length(); i++) {
        char c = suggestionsStr[i];

        if (c == '"' && (i == 0 || suggestionsStr[i-1] != '\\')) {
            if (inQuote) {
                // End of suggestion
                if (!currentSuggestion.empty()) {
                    suggestions.push_back(currentSuggestion);
                    currentSuggestion.clear();
                }
                inQuote = false;
            } else {
                // Start of suggestion
                inQuote = true;
            }
        } else if (inQuote) {
            // Handle escape sequences
            if (c == '\\' && i + 1 < suggestionsStr.length()) {
                char next = suggestionsStr[i + 1];
                if (next == '"' || next == '\\') {
                    currentSuggestion += next;
                    i++; // Skip next character
                } else {
                    currentSuggestion += c;
                }
            } else {
                currentSuggestion += c;
            }
        }
    }

    LOG_DEBUG_GOOGLE("Parsed " + std::to_string(suggestions.size()) + " suggestions from Google");

#else
    LOG_ERROR_GOOGLE("GoogleSuggestService only supported on Windows");
#endif

    return suggestions;
}
