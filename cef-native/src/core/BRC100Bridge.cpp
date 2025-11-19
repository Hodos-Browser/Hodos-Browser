#include "BRC100Bridge.h"
#include <iostream>
#include <sstream>

BRC100Bridge::BRC100Bridge()
    : baseUrl_("http://localhost:3301"),
      hSession_(nullptr),
      hConnect_(nullptr),
      connected_(false),
      hWebSocket_(nullptr),
      webSocketConnected_(false) {
    initializeConnection();
}

BRC100Bridge::~BRC100Bridge() {
    cleanupConnection();
    cleanupWebSocket();
}

bool BRC100Bridge::initializeConnection() {
    // Initialize WinHTTP session
    hSession_ = WinHttpOpen(L"BRC100Bridge/1.0",
                           WINHTTP_ACCESS_TYPE_DEFAULT_PROXY,
                           WINHTTP_NO_PROXY_NAME,
                           WINHTTP_NO_PROXY_BYPASS, 0);

    if (!hSession_) {
        std::cerr << "Failed to initialize WinHTTP session" << std::endl;
        return false;
    }

    // Convert baseUrl to wide string
    std::wstring wideUrl(baseUrl_.begin(), baseUrl_.end());
    size_t protocolEnd = wideUrl.find(L"://");
    if (protocolEnd == std::wstring::npos) {
        std::cerr << "Invalid URL format" << std::endl;
        return false;
    }

    std::wstring host = wideUrl.substr(protocolEnd + 3);
    size_t portStart = host.find(L":");
    size_t pathStart = host.find(L"/");

    std::wstring serverName;
    INTERNET_PORT port = INTERNET_DEFAULT_HTTP_PORT;

    if (portStart != std::wstring::npos && (pathStart == std::wstring::npos || portStart < pathStart)) {
        serverName = host.substr(0, portStart);
        std::wstring portStr = host.substr(portStart + 1, pathStart - portStart - 1);
        port = static_cast<INTERNET_PORT>(_wtoi(portStr.c_str()));
    } else if (pathStart != std::wstring::npos) {
        serverName = host.substr(0, pathStart);
    } else {
        serverName = host;
    }

    // Connect to server
    hConnect_ = WinHttpConnect(hSession_, serverName.c_str(), port, 0);
    if (!hConnect_) {
        std::cerr << "Failed to connect to server" << std::endl;
        cleanupConnection();
        return false;
    }

    connected_ = true;
    return true;
}

void BRC100Bridge::cleanupConnection() {
    if (hConnect_) {
        WinHttpCloseHandle(hConnect_);
        hConnect_ = nullptr;
    }
    if (hSession_) {
        WinHttpCloseHandle(hSession_);
        hSession_ = nullptr;
    }
    connected_ = false;
}

bool BRC100Bridge::isConnected() {
    return connected_ && hConnect_ != nullptr;
}

void BRC100Bridge::setBaseUrl(const std::string& url) {
    baseUrl_ = url;
    cleanupConnection();
    initializeConnection();
}

nlohmann::json BRC100Bridge::makeHttpRequest(const std::string& method, const std::string& endpoint, const nlohmann::json& body) {
    if (!isConnected()) {
        return nlohmann::json{{"error", "Not connected to server"}};
    }

    std::wstring wideEndpoint(endpoint.begin(), endpoint.end());
    HINTERNET hRequest = WinHttpOpenRequest(hConnect_,
                                           std::wstring(method.begin(), method.end()).c_str(),
                                           wideEndpoint.c_str(),
                                           nullptr,
                                           WINHTTP_NO_REFERER,
                                           WINHTTP_DEFAULT_ACCEPT_TYPES,
                                           0);

    if (!hRequest) {
        return nlohmann::json{{"error", "Failed to create request"}};
    }

    // Set headers
    std::wstring headers = L"Content-Type: application/json\r\n";
    WinHttpAddRequestHeaders(hRequest, headers.c_str(), -1, WINHTTP_ADDREQ_FLAG_ADD);

    // Send request
    std::string bodyStr = body.dump();
    bool success = false;

    if (method == "GET" || bodyStr.empty()) {
        success = WinHttpSendRequest(hRequest, WINHTTP_NO_ADDITIONAL_HEADERS, 0, nullptr, 0, 0, 0);
    } else {
        success = WinHttpSendRequest(hRequest, WINHTTP_NO_ADDITIONAL_HEADERS, 0,
                                   const_cast<char*>(bodyStr.c_str()), bodyStr.length(), 0, 0);
    }

    if (!success) {
        WinHttpCloseHandle(hRequest);
        return nlohmann::json{{"error", "Failed to send request"}};
    }

    // Receive response
    if (!WinHttpReceiveResponse(hRequest, nullptr)) {
        WinHttpCloseHandle(hRequest);
        return nlohmann::json{{"error", "Failed to receive response"}};
    }

    // Read response
    std::string response = readResponse(hRequest);
    WinHttpCloseHandle(hRequest);

    try {
        return nlohmann::json::parse(response);
    } catch (const std::exception& e) {
        return nlohmann::json{{"error", "Invalid JSON response: " + std::string(e.what())}};
    }
}

std::string BRC100Bridge::readResponse(HINTERNET hRequest) {
    std::string response;
    DWORD bytesRead;
    char buffer[4096];

    do {
        if (!WinHttpReadData(hRequest, buffer, sizeof(buffer), &bytesRead)) {
            break;
        }
        response.append(buffer, bytesRead);
    } while (bytesRead > 0);

    return response;
}

// Status & Detection
nlohmann::json BRC100Bridge::getStatus() {
    return makeHttpRequest("GET", "/brc100/status");
}

bool BRC100Bridge::isAvailable() {
    auto response = getStatus();
    return response.contains("available") && response["available"].get<bool>();
}

// Identity Management
nlohmann::json BRC100Bridge::generateIdentity(const nlohmann::json& identityData) {
    return makeHttpRequest("POST", "/brc100/identity/generate", identityData);
}

nlohmann::json BRC100Bridge::validateIdentity(const nlohmann::json& identityData) {
    return makeHttpRequest("POST", "/brc100/identity/validate", identityData);
}

nlohmann::json BRC100Bridge::createSelectiveDisclosure(const nlohmann::json& disclosureData) {
    return makeHttpRequest("POST", "/brc100/identity/selective-disclosure", disclosureData);
}

// Authentication
nlohmann::json BRC100Bridge::generateChallenge(const nlohmann::json& challengeData) {
    return makeHttpRequest("POST", "/brc100/auth/challenge", challengeData);
}

nlohmann::json BRC100Bridge::authenticate(const nlohmann::json& authData) {
    return makeHttpRequest("POST", "/brc100/auth/authenticate", authData);
}

nlohmann::json BRC100Bridge::deriveType42Keys(const nlohmann::json& keyData) {
    return makeHttpRequest("POST", "/brc100/auth/type42", keyData);
}

// Session Management
nlohmann::json BRC100Bridge::createSession(const nlohmann::json& sessionData) {
    return makeHttpRequest("POST", "/brc100/session/create", sessionData);
}

nlohmann::json BRC100Bridge::validateSession(const nlohmann::json& sessionData) {
    return makeHttpRequest("POST", "/brc100/session/validate", sessionData);
}

nlohmann::json BRC100Bridge::revokeSession(const nlohmann::json& sessionData) {
    return makeHttpRequest("POST", "/brc100/session/revoke", sessionData);
}

// BEEF Transaction Management
nlohmann::json BRC100Bridge::createBEEF(const nlohmann::json& beefData) {
    return makeHttpRequest("POST", "/brc100/beef/create", beefData);
}

nlohmann::json BRC100Bridge::verifyBEEF(const nlohmann::json& beefData) {
    return makeHttpRequest("POST", "/brc100/beef/verify", beefData);
}

nlohmann::json BRC100Bridge::broadcastBEEF(const nlohmann::json& beefData) {
    return makeHttpRequest("POST", "/brc100/beef/broadcast", beefData);
}

// SPV Operations
nlohmann::json BRC100Bridge::verifySPV(const nlohmann::json& spvData) {
    return makeHttpRequest("POST", "/brc100/spv/verify", spvData);
}

nlohmann::json BRC100Bridge::createSPVProof(const nlohmann::json& proofData) {
    return makeHttpRequest("POST", "/brc100/spv/proof", proofData);
}

// WebSocket Support (placeholder implementation)
bool BRC100Bridge::connectWebSocket() {
    // TODO: Implement WebSocket connection
    webSocketConnected_ = true;
    return true;
}

void BRC100Bridge::disconnectWebSocket() {
    webSocketConnected_ = false;
}

bool BRC100Bridge::sendWebSocketMessage(const std::string& message) {
    // TODO: Implement WebSocket message sending
    return webSocketConnected_;
}

std::string BRC100Bridge::receiveWebSocketMessage() {
    // TODO: Implement WebSocket message receiving
    return "";
}

bool BRC100Bridge::initializeWebSocket() {
    // TODO: Implement WebSocket initialization
    return true;
}

void BRC100Bridge::cleanupWebSocket() {
    disconnectWebSocket();
}
