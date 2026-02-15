#pragma once

#ifdef _WIN32
#include <windows.h>
#include <winhttp.h>
#endif
#include <string>
#include <vector>

class GoogleSuggestService {
public:
    static GoogleSuggestService& GetInstance();

    // Fetch suggestions from Google - returns empty vector on failure
    std::vector<std::string> fetchSuggestions(const std::string& query);

private:
    GoogleSuggestService();
    ~GoogleSuggestService();

#ifdef _WIN32
    HINTERNET hSession_;
#endif
    bool initialized_;

    bool initialize();
    void cleanup();
    static std::string urlEncode(const std::string& str);

    // Prevent copying
    GoogleSuggestService(const GoogleSuggestService&) = delete;
    GoogleSuggestService& operator=(const GoogleSuggestService&) = delete;
};
