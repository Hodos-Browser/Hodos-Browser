#pragma once

#include <string>
#include <nlohmann/json.hpp>
#ifdef _WIN32
#include <windows.h>
#include <winhttp.h>
#endif
#include <thread>
#include <atomic>

class BRC100Bridge {
public:
    BRC100Bridge();
    ~BRC100Bridge();

    // Connection management
    bool isConnected();
    void setBaseUrl(const std::string& url);
    bool initializeConnection();
    void cleanupConnection();

    // Status & Detection
    nlohmann::json getStatus();
    bool isAvailable();

    // Identity Management
    nlohmann::json generateIdentity(const nlohmann::json& identityData);
    nlohmann::json validateIdentity(const nlohmann::json& identityData);
    nlohmann::json createSelectiveDisclosure(const nlohmann::json& disclosureData);

    // Authentication
    nlohmann::json generateChallenge(const nlohmann::json& challengeData);
    nlohmann::json authenticate(const nlohmann::json& authData);
    nlohmann::json deriveType42Keys(const nlohmann::json& keyData);

    // Session Management
    nlohmann::json createSession(const nlohmann::json& sessionData);
    nlohmann::json validateSession(const nlohmann::json& sessionData);
    nlohmann::json revokeSession(const nlohmann::json& sessionData);

    // BEEF Transaction Management
    nlohmann::json createBEEF(const nlohmann::json& beefData);
    nlohmann::json verifyBEEF(const nlohmann::json& beefData);
    nlohmann::json broadcastBEEF(const nlohmann::json& beefData);

    // SPV Operations
    nlohmann::json verifySPV(const nlohmann::json& spvData);
    nlohmann::json createSPVProof(const nlohmann::json& proofData);

    // WebSocket Support
    bool connectWebSocket();
    void disconnectWebSocket();
    bool sendWebSocketMessage(const std::string& message);
    std::string receiveWebSocketMessage();

private:
    std::string baseUrl_;
#ifdef _WIN32
    HINTERNET hSession_;
    HINTERNET hConnect_;
#endif
    bool connected_;

#ifdef _WIN32
    // WebSocket connection
    HINTERNET hWebSocket_;
#endif
    bool webSocketConnected_;

    // HTTP helper methods
    nlohmann::json makeHttpRequest(const std::string& method, const std::string& endpoint, const nlohmann::json& body = nlohmann::json());
#ifdef _WIN32
    std::string readResponse(HINTERNET hRequest);
    bool sendRequest(HINTERNET hRequest, const std::string& body);
#endif

    // WebSocket helper methods
    bool initializeWebSocket();
    void cleanupWebSocket();
};
