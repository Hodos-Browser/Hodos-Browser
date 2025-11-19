#include "../../include/core/HttpRequestInterceptor.h"
#include "include/wrapper/cef_helpers.h"
#include "include/cef_urlrequest.h"
#include "include/cef_request.h"
#include "include/cef_request_context.h"
#include "include/cef_browser.h"
#include "include/cef_task.h"
#include "include/cef_v8.h"
#include "include/cef_frame.h"
#include "../handlers/simple_handler.h"
#include "../handlers/simple_app.h"
#include "../../include/core/WebSocketServerHandler.h"
#include <iostream>
#include <regex>

#include "../include/core/PendingAuthRequest.h"

// Forward declaration
class AsyncWalletResourceHandler;

// Global variable to store pending auth request data
PendingAuthRequest g_pendingAuthRequest = {"", "", "", "", false, nullptr};

// Global variable to track if a modal is currently pending
std::string g_pendingModalDomain = "";
#include <sstream>
#include <algorithm>
#include <fstream>
#include <mutex>
#include <condition_variable>
#include <nlohmann/json.hpp>
#include <cstdlib>
#include <ctime>
#include <chrono>
#include <iomanip>

// Logger class for proper debug logging
class Logger {
private:
    static std::string GetTimestamp() {
        auto now = std::chrono::system_clock::now();
        auto time_t = std::chrono::system_clock::to_time_t(now);
        auto ms = std::chrono::duration_cast<std::chrono::milliseconds>(
            now.time_since_epoch()) % 1000;

        std::stringstream ss;
        ss << std::put_time(std::localtime(&time_t), "%Y-%m-%d %H:%M:%S");
        ss << "." << std::setfill('0') << std::setw(3) << ms.count();
        return ss.str();
    }

    static std::string GetProcessName(int process) {
        switch (process) {
            case 0: return "MAIN";
            case 1: return "RENDER";
            case 2: return "BROWSER";
            default: return "UNKNOWN";
        }
    }

    static std::string GetLogLevelName(int level) {
        switch (level) {
            case 0: return "DEBUG";
            case 1: return "INFO";
            case 2: return "WARN";
            case 3: return "ERROR";
            default: return "UNKNOWN";
        }
    }

public:
    static void Log(const std::string& message, int level = 1, int process = 2) {
        std::string logEntry = "[" + GetTimestamp() + "] [" + GetProcessName(process) + "] [" + GetLogLevelName(level) + "] " + message;

        // Write to file
        std::ofstream logFile("debug_output.log", std::ios::app);
        if (logFile.is_open()) {
            logFile << logEntry << std::endl;
            logFile.close();
        }

        // Also output to console
        std::cout << logEntry << std::endl;
    }
};

// Logging macros for HTTP interceptor
#define LOG_DEBUG_HTTP(msg) Logger::Log(msg, 0, 2)
#define LOG_INFO_HTTP(msg) Logger::Log(msg, 1, 2)
#define LOG_WARNING_HTTP(msg) Logger::Log(msg, 2, 2)
#define LOG_ERROR_HTTP(msg) Logger::Log(msg, 3, 2)

// Domain verification class
class DomainVerifier {
private:
    std::string whitelistFilePath;

public:
    DomainVerifier() {
        // Set path to domainWhitelist.json
        char* homeDir = std::getenv("USERPROFILE");
        whitelistFilePath = std::string(homeDir) + "\\AppData\\Roaming\\HodosBrowser\\wallet\\domainWhitelist.json";
    }

    bool isDomainWhitelisted(const std::string& domain) {
        // Read whitelist file and check domain
        std::ifstream file(whitelistFilePath);
        if (!file.is_open()) {
            std::cout << "🔒 Domain whitelist file not found: " << whitelistFilePath << std::endl;
            return false;
        }

        try {
            nlohmann::json whitelist;
            file >> whitelist;
            file.close();

            for (const auto& entry : whitelist) {
                if (entry["domain"] == domain) {
                    // Domain is whitelisted - allow it regardless of request count
                    // (one-time domains should remain approved for the session)
                    std::cout << "🔒 Domain " << domain << " is whitelisted" << std::endl;
                    return true;
                }
            }

            std::cout << "🔒 Domain " << domain << " is not whitelisted" << std::endl;
            return false;
        } catch (const std::exception& e) {
            std::cout << "🔒 Error reading domain whitelist: " << e.what() << std::endl;
            file.close();
            return false;
        }
    }

    void addToWhitelist(const std::string& domain, bool isPermanent) {
        // Read existing whitelist
        nlohmann::json whitelist = nlohmann::json::array();
        std::ifstream file(whitelistFilePath);
        if (file.is_open()) {
            try {
                file >> whitelist;
                file.close();
            } catch (const std::exception& e) {
                std::cout << "🔒 Error reading whitelist for update: " << e.what() << std::endl;
                file.close();
            }
        }

        // Add new entry
        nlohmann::json newEntry;
        newEntry["domain"] = domain;
        newEntry["addedAt"] = std::time(nullptr);
        newEntry["lastUsed"] = std::time(nullptr);
        newEntry["requestCount"] = 0;
        newEntry["isPermanent"] = isPermanent;

        whitelist.push_back(newEntry);

        // Write back to file
        std::ofstream outFile(whitelistFilePath);
        if (outFile.is_open()) {
            outFile << whitelist.dump(2);
            outFile.close();
            std::cout << "🔒 Added domain " << domain << " to whitelist" << std::endl;
        } else {
            std::cout << "🔒 Error writing to whitelist file" << std::endl;
        }
    }

    void recordRequest(const std::string& domain) {
        // Read existing whitelist
        nlohmann::json whitelist = nlohmann::json::array();
        std::ifstream file(whitelistFilePath);
        if (file.is_open()) {
            try {
                file >> whitelist;
                file.close();
            } catch (const std::exception& e) {
                std::cout << "🔒 Error reading whitelist for recording: " << e.what() << std::endl;
                file.close();
                return;
            }
        }

        // Update request count
        for (auto& entry : whitelist) {
            if (entry["domain"] == domain) {
                entry["lastUsed"] = std::time(nullptr);
                entry["requestCount"] = entry["requestCount"].get<int>() + 1;
                break;
            }
        }

        // Write back to file
        std::ofstream outFile(whitelistFilePath);
        if (outFile.is_open()) {
            outFile << whitelist.dump(2);
            outFile.close();
            std::cout << "🔒 Recorded request from domain " << domain << std::endl;
        } else {
            std::cout << "🔒 Error writing to whitelist file for recording" << std::endl;
        }
    }
};

// Forward declaration
class AsyncHTTPClient;


// Async Resource Handler for managing wallet HTTP requests
class AsyncWalletResourceHandler : public CefResourceHandler {
public:
    AsyncWalletResourceHandler(const std::string& method,
                              const std::string& endpoint,
                              const std::string& body,
                              const std::string& requestDomain,
                              CefRefPtr<CefBrowser> browser,
                              const CefRequest::HeaderMap& headers)
        : method_(method), endpoint_(endpoint), body_(body), requestDomain_(requestDomain),
          responseOffset_(0), requestCompleted_(false), browser_(browser), originalHeaders_(headers) {
        LOG_DEBUG_HTTP("🌐 AsyncWalletResourceHandler constructor called for " + method + " " + endpoint + " from domain " + requestDomain);
        LOG_DEBUG_HTTP("🌐 Forwarding " + std::to_string(headers.size()) + " original headers");
    }

    bool Open(CefRefPtr<CefRequest> request,
              bool& handle_request,
              CefRefPtr<CefCallback> callback) override {
        CEF_REQUIRE_IO_THREAD();

        LOG_DEBUG_HTTP("🌐 AsyncWalletResourceHandler::Open called");

        // Check if domain is whitelisted - NO BYPASSES
        DomainVerifier domainVerifier;
        if (!domainVerifier.isDomainWhitelisted(requestDomain_)) {
            // Domain not whitelisted, check if this is a BRC-100 authentication request
            if (endpoint_.find("/brc100/auth/") != std::string::npos) {
                // This is a BRC-100 authentication request
                LOG_DEBUG_HTTP("🔐 BRC-100 auth request from non-whitelisted domain: " + requestDomain_);
                triggerBRC100AuthApprovalModal(requestDomain_, method_, endpoint_, body_, this);

                // Don't return error response immediately - wait for user response
                LOG_DEBUG_HTTP("🔐 Waiting for user response to BRC-100 auth request");
                handle_request = true;
                return true;
            } else {
                // ALL other requests (including wallet endpoints) require whitelist approval
                LOG_DEBUG_HTTP("🔒 Domain " + requestDomain_ + " not whitelisted for endpoint " + endpoint_ + ", triggering approval modal");
                triggerDomainApprovalModal(requestDomain_, method_, endpoint_);

                // Don't return error response immediately - wait for user response
                LOG_DEBUG_HTTP("🔐 Waiting for user response to domain approval request");
                handle_request = true;
                return true;
            }
        }

        // Domain is whitelisted, proceed with request
        LOG_DEBUG_HTTP("🔒 Domain " + requestDomain_ + " is whitelisted, proceeding with request");
        domainVerifier.recordRequest(requestDomain_);

        handle_request = true;

        // Start async HTTP request to Go daemon
        LOG_DEBUG_HTTP("🌐 About to start async HTTP request...");
        startAsyncHTTPRequest();
        LOG_DEBUG_HTTP("🌐 Async HTTP request started");

        // Don't call callback->Continue() yet - wait for HTTP response
        return true;
    }

    void GetResponseHeaders(scoped_refptr<CefResponse> response,
                           int64_t& response_length,
                           CefString& redirectUrl) override {
        CEF_REQUIRE_IO_THREAD();

        LOG_DEBUG_HTTP("🌐 AsyncWalletResourceHandler::GetResponseHeaders called");

        response->SetStatus(200);
        response->SetStatusText("OK");
        response->SetMimeType("application/json");
        response->SetHeaderByName("Access-Control-Allow-Origin", "*", true);
        response->SetHeaderByName("Access-Control-Allow-Methods", "GET, POST, PUT, DELETE, OPTIONS", true);
        response->SetHeaderByName("Access-Control-Allow-Headers", "Content-Type, Authorization", true);
        response->SetHeaderByName("Access-Control-Max-Age", "86400", true);

        response_length = static_cast<int64_t>(responseData_.length());
    }

    bool ReadResponse(void* data_out,
                     int bytes_to_read,
                     int& bytes_read,
                     CefRefPtr<CefCallback> callback) override {
        CEF_REQUIRE_IO_THREAD();

        LOG_DEBUG_HTTP("🌐 AsyncWalletResourceHandler::ReadResponse called, completed: " + std::to_string(requestCompleted_));

        if (!requestCompleted_) {
            // Store callback for later use
            readCallback_ = callback;
            return true; // Wait for HTTP response
        }

        // Send response data to frontend
        if (responseOffset_ >= responseData_.length()) {
            bytes_read = 0;
            return false; // No more data
        }

        int remaining = static_cast<int>(responseData_.length() - responseOffset_);
        bytes_read = (bytes_to_read < remaining) ? bytes_to_read : remaining;
        memcpy(data_out, responseData_.c_str() + responseOffset_, bytes_read);
        responseOffset_ += bytes_read;

        return true;
    }

    void Cancel() override {
        CEF_REQUIRE_IO_THREAD();
        LOG_DEBUG_HTTP("🌐 AsyncWalletResourceHandler::Cancel called");

        if (urlRequest_) {
            urlRequest_->Cancel();
            urlRequest_ = nullptr;
        }
    }

    // Called by AsyncHTTPClient when HTTP response is received
    void onHTTPResponseReceived(const std::string& data) {
        LOG_DEBUG_HTTP("🌐 AsyncWalletResourceHandler received HTTP response: " + data);

        responseData_ = data;
        requestCompleted_ = true;

        LOG_DEBUG_HTTP("🌐 About to call readCallback_->Continue()");
        // Now we can continue with the response
        if (readCallback_) {
            readCallback_->Continue();
            LOG_DEBUG_HTTP("🌐 readCallback_->Continue() called successfully");
        }
    }

    // Called when user approves authentication request
    void onAuthResponseReceived(const std::string& data) {
        LOG_DEBUG_HTTP("🔐 AsyncWalletResourceHandler received auth response: " + data);

        responseData_ = data;
        requestCompleted_ = true;

        LOG_DEBUG_HTTP("🔐 About to call readCallback_->Continue() for auth response");
        // Now we can continue with the response
        if (readCallback_) {
            readCallback_->Continue();
            LOG_DEBUG_HTTP("🔐 readCallback_->Continue() called successfully for auth response");
        }
    }

    // Trigger domain approval modal using the existing BRC-100 auth modal system
    void triggerDomainApprovalModal(const std::string& domain, const std::string& method, const std::string& endpoint) {
        LOG_DEBUG_HTTP("🔒 Triggering domain approval modal for " + domain);

        // Check if a modal is already pending for this domain
        if (g_pendingModalDomain == domain) {
            LOG_DEBUG_HTTP("🔒 Modal already pending for domain " + domain + ", skipping duplicate request");
            return;
        }

        // Set pending modal domain
        g_pendingModalDomain = domain;

        // Store domain approval request data for later message passing (using BRC-100 auth system)
        g_pendingAuthRequest.domain = domain;
        g_pendingAuthRequest.method = method;
        g_pendingAuthRequest.endpoint = endpoint;
        g_pendingAuthRequest.body = ""; // No body for domain approval
        g_pendingAuthRequest.isValid = true;
        g_pendingAuthRequest.handler = nullptr; // Will be set when we create the handler

        // Send message to frontend to create overlay with domain approval request data
        CefRefPtr<CefBrowser> header_browser = SimpleHandler::GetHeaderBrowser();
        if (header_browser && header_browser->GetMainFrame()) {
            std::string js = R"(
                console.log('🔒 Domain approval request received in header browser');
                // Set the pending BRC-100 auth request data (reusing existing system)
                window.pendingBRC100AuthRequest = {
                    domain: ')" + domain + R"(',
                    method: ')" + method + R"(',
                    endpoint: ')" + endpoint + R"(',
                    body: '',
                    type: 'domain_approval'
                };
                console.log('🔒 Set pending BRC-100 auth request for domain approval:', window.pendingBRC100AuthRequest);
                // Create the settings overlay (which will show the BRC-100 auth modal)
                if (window.hodosBrowser && window.hodosBrowser.overlay && window.hodosBrowser.overlay.show) {
                    console.log('🔒 Creating overlay for domain approval modal');
                    window.hodosBrowser.overlay.show();
                } else {
                    console.error('🔒 Overlay show function not available');
                }
            )";
            header_browser->GetMainFrame()->ExecuteJavaScript(js, header_browser->GetMainFrame()->GetURL(), 0);
            LOG_DEBUG_HTTP("🔒 Sent domain approval request to frontend");
        } else {
            LOG_DEBUG_HTTP("🔒 Header browser not available for domain approval request");
        }

        LOG_DEBUG_HTTP("🔒 Domain approval needed for: " + domain + " requesting " + method + " " + endpoint);
    }


    // Trigger BRC-100 authentication approval modal
void triggerBRC100AuthApprovalModal(const std::string& domain, const std::string& method, const std::string& endpoint, const std::string& body, CefRefPtr<AsyncWalletResourceHandler> handler) {
    LOG_DEBUG_HTTP("🔐 Triggering BRC-100 auth approval modal for " + domain);

    // Check if a modal is already pending for this domain
    if (g_pendingModalDomain == domain) {
        LOG_DEBUG_HTTP("🔐 Modal already pending for domain " + domain + ", skipping duplicate request");
        return;
    }

    // Set pending modal domain
    g_pendingModalDomain = domain;

    // Store auth request data for later message passing
    g_pendingAuthRequest.domain = domain;
    g_pendingAuthRequest.method = method;
    g_pendingAuthRequest.endpoint = endpoint;
    g_pendingAuthRequest.body = body;
    g_pendingAuthRequest.isValid = true;
    g_pendingAuthRequest.handler = handler;

    // Send message to frontend to create overlay with auth request data
    CefRefPtr<CefBrowser> header_browser = SimpleHandler::GetHeaderBrowser();
    if (header_browser && header_browser->GetMainFrame()) {
        std::string js = R"(
            console.log('🔐 BRC-100 auth request received in header browser');
            // Set the pending auth request data
            window.pendingBRC100AuthRequest = {
                domain: ')" + domain + R"(',
                method: ')" + method + R"(',
                endpoint: ')" + endpoint + R"(',
                body: ')" + body + R"('
            };
            console.log('🔐 Set pending auth request:', window.pendingBRC100AuthRequest);
            // Create the settings overlay (which will show the BRC-100 auth modal)
            if (window.hodosBrowser && window.hodosBrowser.overlay && window.hodosBrowser.overlay.show) {
                console.log('🔐 Creating overlay for BRC-100 auth modal');
                window.hodosBrowser.overlay.show();
            } else {
                console.error('🔐 Overlay show function not available');
            }
        )";
        header_browser->GetMainFrame()->ExecuteJavaScript(js, header_browser->GetMainFrame()->GetURL(), 0);
        LOG_DEBUG_HTTP("🔐 Sent BRC-100 auth request to frontend");
    } else {
        LOG_DEBUG_HTTP("🔐 Header browser not available for BRC-100 auth request");
    }

    LOG_DEBUG_HTTP("🔐 BRC-100 auth approval needed for: " + domain + " requesting " + method + " " + endpoint);
}


    // Static method to create CefURLRequest on UI thread (called by URLRequestCreationTask)
    static void createURLRequestOnUIThread(AsyncWalletResourceHandler* handler,
                                          CefRefPtr<CefRequest> httpRequest,
                                          AsyncHTTPClient* client,
                                          CefRefPtr<CefRequestContext> context);

private:
    void startAsyncHTTPRequest();

    // Request data
    std::string method_;
    std::string endpoint_;
    std::string body_;
    std::string requestDomain_;
    CefRequest::HeaderMap originalHeaders_;  // BRC-31 authentication headers

    // Response management
    std::string responseData_;
    size_t responseOffset_;
    bool requestCompleted_;

    // Browser reference for modal triggering
    CefRefPtr<CefBrowser> browser_;

    // CEF request management
    CefRefPtr<CefURLRequest> urlRequest_;
    CefRefPtr<CefCallback> readCallback_;

    IMPLEMENT_REFCOUNTING(AsyncWalletResourceHandler);
    DISALLOW_COPY_AND_ASSIGN(AsyncWalletResourceHandler);
};

// Function to store pending auth request data
void storePendingAuthRequest(const std::string& domain, const std::string& method, const std::string& endpoint, const std::string& body) {
    g_pendingAuthRequest.domain = domain;
    g_pendingAuthRequest.method = method;
    g_pendingAuthRequest.endpoint = endpoint;
    g_pendingAuthRequest.body = body;
    g_pendingAuthRequest.isValid = true;
    LOG_DEBUG_HTTP("🔐 Stored pending auth request data");
}

// Handler for domain whitelist requests
class AsyncDomainWhitelistHandler : public CefURLRequestClient {
public:
    explicit AsyncDomainWhitelistHandler(const std::string& domain, bool permanent)
        : domain_(domain), permanent_(permanent) {}

    void OnRequestComplete(CefRefPtr<CefURLRequest> request) override {
        LOG_DEBUG_HTTP("🔐 AsyncDomainWhitelistHandler::OnRequestComplete called for domain: " + domain_);
        CefURLRequest::Status status = request->GetRequestStatus();
        LOG_DEBUG_HTTP("🔐 Request status: " + std::to_string(status));
        if (status == UR_SUCCESS) {
            LOG_DEBUG_HTTP("🔐 Successfully added domain to whitelist: " + domain_);
        } else {
            LOG_DEBUG_HTTP("🔐 Failed to add domain to whitelist: " + domain_ + " (status: " + std::to_string(status) + ")");
        }
    }

    void OnDownloadData(CefRefPtr<CefURLRequest> request, const void* data, size_t data_length) override {
        // Handle response data if needed
    }

    void OnUploadProgress(CefRefPtr<CefURLRequest> request, int64_t current, int64_t total) override {
        // Not needed for this use case
    }

    void OnDownloadProgress(CefRefPtr<CefURLRequest> request, int64_t current, int64_t total) override {
        // Not needed for this use case
    }

    bool GetAuthCredentials(bool isProxy, const CefString& host, int port, const CefString& realm, const CefString& scheme, CefRefPtr<CefAuthCallback> callback) override {
        // No authentication needed
        return false;
    }

private:
    std::string domain_;
    bool permanent_;
    IMPLEMENT_REFCOUNTING(AsyncDomainWhitelistHandler);
    DISALLOW_COPY_AND_ASSIGN(AsyncDomainWhitelistHandler);
};

// Task class for creating domain whitelist request on UI thread
class DomainWhitelistTask : public CefTask {
public:
    DomainWhitelistTask(const std::string& domain, bool permanent)
        : domain_(domain), permanent_(permanent) {}

    void Execute() override {
        LOG_DEBUG_HTTP("🔐 DomainWhitelistTask executing on UI thread for domain: " + domain_);

        // Create request
        CefRefPtr<CefRequest> cefRequest = CefRequest::Create();
        cefRequest->SetURL("http://localhost:3301/domain/whitelist/add");
        cefRequest->SetMethod("POST");
        cefRequest->SetHeaderByName("Content-Type", "application/json", true);

        // Create JSON body
        std::string jsonBody = "{\"domain\":\"" + domain_ + "\",\"isPermanent\":" + (permanent_ ? "true" : "false") + "}";
        LOG_DEBUG_HTTP("🔐 Domain whitelist JSON body: " + jsonBody);

        // Create post data
        CefRefPtr<CefPostData> postData = CefPostData::Create();
        CefRefPtr<CefPostDataElement> element = CefPostDataElement::Create();
        element->SetToBytes(jsonBody.length(), jsonBody.c_str());
        postData->AddElement(element);
        cefRequest->SetPostData(postData);

        LOG_DEBUG_HTTP("🔐 About to create CefURLRequest for domain whitelist");
        // Make HTTP request to add domain to whitelist
        CefRefPtr<CefURLRequest> request = CefURLRequest::Create(
            cefRequest,
            new AsyncDomainWhitelistHandler(domain_, permanent_),
            nullptr
        );

        if (request) {
            LOG_DEBUG_HTTP("🔐 Domain whitelist request created successfully");
        } else {
            LOG_DEBUG_HTTP("🔐 Failed to create domain whitelist request");
        }
    }

private:
    std::string domain_;
    bool permanent_;
    IMPLEMENT_REFCOUNTING(DomainWhitelistTask);
    DISALLOW_COPY_AND_ASSIGN(DomainWhitelistTask);
};

// Function to add domain to whitelist
void addDomainToWhitelist(const std::string& domain, bool permanent) {
    LOG_DEBUG_HTTP("🔐 Adding domain to whitelist: " + domain + " (permanent: " + std::to_string(permanent) + ")");

    // Post task to UI thread - CefURLRequest::Create must be called from UI thread
    CefPostTask(TID_UI, new DomainWhitelistTask(domain, permanent));
    LOG_DEBUG_HTTP("🔐 Domain whitelist task posted to UI thread");
}

// Function to handle auth response and send it back to the original request
void handleAuthResponse(const std::string& responseData) {
    LOG_DEBUG_HTTP("🔐 handleAuthResponse called with data: " + responseData);

    // Clear the pending modal domain when user responds
    g_pendingModalDomain = "";

    if (g_pendingAuthRequest.isValid && g_pendingAuthRequest.handler) {
        LOG_DEBUG_HTTP("🔐 Found pending auth request, sending response to original handler");

        // Cast the handler to AsyncWalletResourceHandler and call onAuthResponseReceived
        // We know the type since we stored it ourselves
        AsyncWalletResourceHandler* walletHandler = static_cast<AsyncWalletResourceHandler*>(g_pendingAuthRequest.handler.get());
        if (walletHandler) {
            walletHandler->onAuthResponseReceived(responseData);
            LOG_DEBUG_HTTP("🔐 Auth response sent to original HTTP request");
        } else {
            LOG_DEBUG_HTTP("🔐 Failed to cast handler to AsyncWalletResourceHandler");
        }

        // Clear the pending request
        g_pendingAuthRequest.isValid = false;
    } else {
        LOG_DEBUG_HTTP("🔐 No pending auth request or handler found");
    }
}

// Function to send auth request data to overlay (called after overlay loads)
void sendAuthRequestDataToOverlay() {
    if (!g_pendingAuthRequest.isValid) {
        LOG_DEBUG_HTTP("🔐 No pending auth request data to send");
        return;
    }

    CefRefPtr<CefBrowser> auth_browser = SimpleHandler::GetBRC100AuthBrowser();
    if (auth_browser && auth_browser->GetMainFrame()) {
        CefRefPtr<CefProcessMessage> message = CefProcessMessage::Create("brc100_auth_request");
        CefRefPtr<CefListValue> args = message->GetArgumentList();
        args->SetString(0, g_pendingAuthRequest.domain);
        args->SetString(1, g_pendingAuthRequest.method);
        args->SetString(2, g_pendingAuthRequest.endpoint);
        args->SetString(3, g_pendingAuthRequest.body);

        auth_browser->GetMainFrame()->SendProcessMessage(PID_RENDERER, message);
        LOG_DEBUG_HTTP("🔐 Sent auth request data to overlay");

        // Don't clear the pending request here - it will be cleared after auth response is processed
    } else {
        LOG_DEBUG_HTTP("🔐 Auth browser not available for sending data");
    }
}

// Async HTTP Client for handling CEF URL requests
class AsyncHTTPClient : public CefURLRequestClient {
public:
    explicit AsyncHTTPClient(AsyncWalletResourceHandler* parent)
        : parent_(parent), completed_(false) {
        LOG_DEBUG_HTTP("🌐 AsyncHTTPClient constructor called");
    }

    void OnRequestComplete(CefRefPtr<CefURLRequest> request) override {
        std::lock_guard<std::mutex> lock(mutex_);
        completed_ = true;

        LOG_DEBUG_HTTP("🌐 AsyncHTTPClient::OnRequestComplete called, response size: " + std::to_string(responseData_.length()));

        // Notify parent handler that HTTP request completed
        if (parent_) {
            parent_->onHTTPResponseReceived(responseData_);
        }
    }

    void OnUploadProgress(CefRefPtr<CefURLRequest> request, int64_t current, int64_t total) override {
        // Not needed for our use case
    }

    void OnDownloadProgress(CefRefPtr<CefURLRequest> request, int64_t current, int64_t total) override {
        // Not needed for our use case
    }

    void OnDownloadData(CefRefPtr<CefURLRequest> request, const void* data, size_t data_length) override {
        std::lock_guard<std::mutex> lock(mutex_);
        responseData_.append(static_cast<const char*>(data), data_length);
        LOG_DEBUG_HTTP("🌐 AsyncHTTPClient::OnDownloadData received " + std::to_string(data_length) + " bytes");
    }

    bool GetAuthCredentials(bool isProxy,
                           const CefString& host,
                           int port,
                           const CefString& realm,
                           const CefString& scheme,
                           CefRefPtr<CefAuthCallback> callback) override {
        return false; // No authentication needed
    }

private:
    AsyncWalletResourceHandler* parent_;
    std::mutex mutex_;
    bool completed_;
    std::string responseData_;

    IMPLEMENT_REFCOUNTING(AsyncHTTPClient);
    DISALLOW_COPY_AND_ASSIGN(AsyncHTTPClient);
};

// Task class for creating CefURLRequest on UI thread
class URLRequestCreationTask : public CefTask {
public:
    URLRequestCreationTask(AsyncWalletResourceHandler* handler,
                          CefRefPtr<CefRequest> httpRequest,
                          AsyncHTTPClient* client,
                          CefRefPtr<CefRequestContext> context)
        : handler_(handler), httpRequest_(httpRequest), client_(client), context_(context) {}

    void Execute() override {
        LOG_DEBUG_HTTP("🌐 URLRequestCreationTask::Execute called on UI thread");
        AsyncWalletResourceHandler::createURLRequestOnUIThread(handler_, httpRequest_, client_, context_);
    }

private:
    AsyncWalletResourceHandler* handler_;
    CefRefPtr<CefRequest> httpRequest_;
    AsyncHTTPClient* client_;
    CefRefPtr<CefRequestContext> context_;

    IMPLEMENT_REFCOUNTING(URLRequestCreationTask);
    DISALLOW_COPY_AND_ASSIGN(URLRequestCreationTask);
};

// Implementation of AsyncWalletResourceHandler::startAsyncHTTPRequest
void AsyncWalletResourceHandler::startAsyncHTTPRequest() {
    LOG_DEBUG_HTTP("🌐 Starting async HTTP request to: " + endpoint_);

    // Create CEF HTTP request
    LOG_DEBUG_HTTP("🌐 Creating CEF HTTP request");
    CefRefPtr<CefRequest> httpRequest = CefRequest::Create();
    std::string fullUrl = "http://localhost:3301" + endpoint_;
    httpRequest->SetURL(fullUrl);
    httpRequest->SetMethod(method_);

    LOG_DEBUG_HTTP("🌐 Setting headers for request");
    // Start with standard headers
    CefRequest::HeaderMap headers;
    headers.insert(std::make_pair("Content-Type", "application/json"));
    headers.insert(std::make_pair("Accept", "application/json"));

    // Forward original headers (including BRC-31 Authrite headers)
    LOG_DEBUG_HTTP("🌐 Forwarding " + std::to_string(originalHeaders_.size()) + " original headers");
    for (const auto& header : originalHeaders_) {
        std::string headerName = header.first.ToString();
        std::string headerValue = header.second.ToString();

        // Log BRC-31 authentication headers
        if (headerName.find("x-authrite-") != std::string::npos ||
            headerName.find("X-Authrite-") != std::string::npos ||
            headerName.find("x-bsv-") != std::string::npos ||
            headerName.find("X-BSV-") != std::string::npos) {
            LOG_DEBUG_HTTP("🔐 Forwarding auth header: " + headerName + " = " + headerValue.substr(0, 50) + "...");
        }

        headers.insert(std::make_pair(headerName, headerValue));
    }

    httpRequest->SetHeaderMap(headers);

    // Set POST body if needed
    if (method_ == "POST" && !body_.empty()) {
        LOG_DEBUG_HTTP("🌐 Setting POST body");
        CefRefPtr<CefPostData> postData = CefPostData::Create();
        CefRefPtr<CefPostDataElement> element = CefPostDataElement::Create();
        element->SetToBytes(body_.length(), body_.c_str());
        postData->AddElement(element);
        httpRequest->SetPostData(postData);
    }

    // Start async request
    LOG_DEBUG_HTTP("🌐 About to create CefURLRequest");
    LOG_DEBUG_HTTP("🌐 Creating AsyncHTTPClient");
    AsyncHTTPClient* client = new AsyncHTTPClient(this);
    LOG_DEBUG_HTTP("🌐 AsyncHTTPClient created successfully");

    LOG_DEBUG_HTTP("🌐 Getting global request context");
    CefRefPtr<CefRequestContext> context = CefRequestContext::GetGlobalContext();
    LOG_DEBUG_HTTP("🌐 Global request context obtained");

    LOG_DEBUG_HTTP("🌐 About to call CefURLRequest::Create");
    LOG_DEBUG_HTTP("🌐 HTTP Request URL: " + httpRequest->GetURL().ToString());
    LOG_DEBUG_HTTP("🌐 HTTP Request Method: " + httpRequest->GetMethod().ToString());

    CefRequest::HeaderMap requestHeaders;
    httpRequest->GetHeaderMap(requestHeaders);
    LOG_DEBUG_HTTP("🌐 HTTP Request Headers count: " + std::to_string(requestHeaders.size()));

    try {
        LOG_DEBUG_HTTP("🌐 Inside try block, about to create URL request");
        LOG_DEBUG_HTTP("🌐 Posting task to UI thread for CefURLRequest creation");

        // Post task to UI thread - CefURLRequest::Create must be called from UI thread
        // Create a simple task that will call our method
        CefPostTask(TID_UI, new URLRequestCreationTask(this, httpRequest, client, context));
        LOG_DEBUG_HTTP("🌐 Task posted to UI thread successfully");

    } catch (const std::exception& e) {
        LOG_DEBUG_HTTP("🌐 Exception caught: " + std::string(e.what()));
    } catch (...) {
        LOG_DEBUG_HTTP("🌐 Unknown exception caught");
    }
}

// Static method to create CefURLRequest on UI thread
void AsyncWalletResourceHandler::createURLRequestOnUIThread(AsyncWalletResourceHandler* handler,
                                                           CefRefPtr<CefRequest> httpRequest,
                                                           AsyncHTTPClient* client,
                                                           CefRefPtr<CefRequestContext> context) {
    LOG_DEBUG_HTTP("🌐 createURLRequestOnUIThread called on UI thread");

    try {
        LOG_DEBUG_HTTP("🌐 Creating CefURLRequest on UI thread");
        handler->urlRequest_ = CefURLRequest::Create(httpRequest, client, context);
        LOG_DEBUG_HTTP("🌐 CefURLRequest created successfully on UI thread");
    } catch (const std::exception& e) {
        LOG_DEBUG_HTTP("🌐 Exception in UI thread: " + std::string(e.what()));
    } catch (...) {
        LOG_DEBUG_HTTP("🌐 Unknown exception in UI thread");
    }
}

HttpRequestInterceptor::HttpRequestInterceptor() {
    LOG_DEBUG_HTTP("🌐 HttpRequestInterceptor created");
}

HttpRequestInterceptor::~HttpRequestInterceptor() {
    LOG_DEBUG_HTTP("🌐 HttpRequestInterceptor destroyed");
}

CefRefPtr<CefResourceHandler> HttpRequestInterceptor::GetResourceHandler(
    CefRefPtr<CefBrowser> browser,
    CefRefPtr<CefFrame> frame,
    CefRefPtr<CefRequest> request) {

    CEF_REQUIRE_IO_THREAD();

    std::string url = request->GetURL().ToString();
    std::string method = request->GetMethod().ToString();

    LOG_DEBUG_HTTP("🌐 HTTP Request intercepted: " + method + " " + url);

    // Normalize BRC-100 wallet requests to our standard port 3301
    std::string originalUrl = url;

    // Handle localhost port redirection
    std::regex localhostPortPattern(R"(localhost:\d{4})");
    if (std::regex_search(url, localhostPortPattern)) {
        // Only redirect if it's not already port 3301
        if (url.find("localhost:3301") == std::string::npos) {
            url = std::regex_replace(url, localhostPortPattern, "localhost:3301");
            LOG_DEBUG_HTTP("🌐 localhost Port redirection: " + originalUrl + " -> " + url);
            request->SetURL(url);
        }
    }

    // Handle 127.0.0.1 port redirection
    std::regex localhostIPPattern(R"(127\.0\.0\.1:\d{4})");
    if (std::regex_search(url, localhostIPPattern)) {
        // Only redirect if it's not already port 3301
        if (url.find("127.0.0.1:3301") == std::string::npos) {
            url = std::regex_replace(url, localhostIPPattern, "127.0.0.1:3301");
            LOG_DEBUG_HTTP("🌐 127.0.0.1 Port redirection: " + originalUrl + " -> " + url);
            request->SetURL(url);
        }
    }

    LOG_DEBUG_HTTP("🌐 About to check if wallet endpoint...");

    // BRC-104 authentication endpoint interception
    // ONLY redirect /.well-known/auth if it's meant for the LOCAL WALLET (localhost/127.0.0.1)
    // DO NOT redirect auth requests to external app backends!
    if (url.find("/.well-known/auth") != std::string::npos) {
        // Check if this is a request to localhost or 127.0.0.1 (wallet auth)
        bool isLocalhost = (url.find("localhost") != std::string::npos || url.find("127.0.0.1") != std::string::npos);

        if (isLocalhost) {
            LOG_DEBUG_HTTP("🌐 BRC-104 /.well-known/auth request to localhost detected, redirecting to local wallet");

            // Extract the original domain for logging
            std::string originalDomain = url;
            size_t protocolEnd = originalDomain.find("://");
            if (protocolEnd != std::string::npos) {
                originalDomain = originalDomain.substr(protocolEnd + 3);
                size_t pathStart = originalDomain.find("/");
                if (pathStart != std::string::npos) {
                    originalDomain = originalDomain.substr(0, pathStart);
                }
            }

            // Replace the domain with localhost:3301
            std::regex domainPattern(R"(https?://[^/]+)");
            url = std::regex_replace(url, domainPattern, "http://localhost:3301");

            LOG_DEBUG_HTTP("🌐 BRC-104 auth redirection: " + originalUrl + " -> " + url);
            request->SetURL(url);
        } else {
            LOG_DEBUG_HTTP("🌐 BRC-104 /.well-known/auth request to external backend detected: " + url);
            LOG_DEBUG_HTTP("🌐 NOT intercepting - allowing CEF to handle normally");
            return nullptr; // Let CEF handle external backend auth requests normally
        }
    }

    // Check if this is a Babbage messagebox request that needs redirection
    if (url.find("messagebox.babbage.systems") != std::string::npos) {
        LOG_DEBUG_HTTP("🌐 ===== MESSAGEBOX REQUEST DETECTED =====");
        LOG_DEBUG_HTTP("🌐 Method: " + method);
        LOG_DEBUG_HTTP("🌐 Full URL: " + url);

        // Log all headers
        CefRequest::HeaderMap messageboxHeaders;
        request->GetHeaderMap(messageboxHeaders);
        LOG_DEBUG_HTTP("🌐 Headers (" + std::to_string(messageboxHeaders.size()) + " total):");
        for (const auto& header : messageboxHeaders) {
            LOG_DEBUG_HTTP("🌐   " + header.first.ToString() + ": " + header.second.ToString());
        }

        // Log POST body if present
        CefRefPtr<CefPostData> postData = request->GetPostData();
        if (postData) {
            CefPostData::ElementVector elements;
            postData->GetElements(elements);
            for (auto& element : elements) {
                if (element->GetType() == PDE_TYPE_BYTES) {
                    size_t size = element->GetBytesCount();
                    std::vector<char> buffer(size);
                    element->GetBytes(size, buffer.data());
                    std::string bodyContent(buffer.data(), size);
                    LOG_DEBUG_HTTP("🌐 POST Body: " + bodyContent);
                }
            }
        }
        LOG_DEBUG_HTTP("🌐 ========================================");

        // Check if this is a WebSocket upgrade request
        std::string connection = request->GetHeaderByName("Connection");
        std::string upgrade = request->GetHeaderByName("Upgrade");
        bool isWebSocketUpgrade = (connection == "upgrade" && upgrade == "websocket");

        if (isWebSocketUpgrade) {
            LOG_DEBUG_HTTP("🌐 WebSocket upgrade request detected for messagebox.babbage.systems");
            LOG_DEBUG_HTTP("🌐 Redirecting WebSocket to Go daemon on localhost:3301");

            // Redirect WebSocket connections to Go daemon
            std::string redirectedUrl = url;
            size_t pos = redirectedUrl.find("messagebox.babbage.systems");
            if (pos != std::string::npos) {
                redirectedUrl.replace(pos, 26, "localhost:3301");
                // Change wss to ws for WebSocket connections
                if (redirectedUrl.find("wss://") == 0) {
                    redirectedUrl.replace(0, 6, "ws://");
                }
                LOG_DEBUG_HTTP("🌐 WebSocket redirection: " + url + " -> " + redirectedUrl);
                request->SetURL(redirectedUrl);
                url = redirectedUrl;
            }
        } else {
            // Redirect HTTP requests to Go daemon
            std::string redirectedUrl = url;
            size_t pos = redirectedUrl.find("messagebox.babbage.systems");
            if (pos != std::string::npos) {
                redirectedUrl.replace(pos, 26, "localhost:3301");
                // Also change https to http since our daemon only supports HTTP
                if (redirectedUrl.find("https://") == 0) {
                    redirectedUrl.replace(0, 8, "http://");
                }
                LOG_DEBUG_HTTP("🌐 HTTP redirection: " + url + " -> " + redirectedUrl);
                request->SetURL(redirectedUrl);
                url = redirectedUrl;
            }
        }
    }

    // Check if this is a Socket.IO connection first
    if (isSocketIOConnection(url)) {
        LOG_DEBUG_HTTP("🌐 Socket.IO connection detected");

        // Extract domain using existing logic
        std::string domain = extractDomain(browser, request);
        LOG_DEBUG_HTTP("🌐 Extracted domain for Socket.IO: " + domain);

        // Check whitelist (for logging only - no modal for now)
        DomainVerifier domainVerifier;
        if (!domainVerifier.isDomainWhitelisted(domain)) {
            LOG_DEBUG_HTTP("🔒 Socket.IO connection from non-whitelisted domain: " + domain + " - allowing for now");
        } else {
            LOG_DEBUG_HTTP("🔒 Socket.IO connection from whitelisted domain: " + domain);
        }

        // Create AsyncWalletResourceHandler for Socket.IO requests
        LOG_DEBUG_HTTP("🌐 Creating AsyncWalletResourceHandler for Socket.IO request");

        // Extract endpoint from URL
        std::string endpoint;
        size_t pos = url.find("://");
        if (pos != std::string::npos) {
            pos = url.find("/", pos + 3);
            if (pos != std::string::npos) {
                endpoint = url.substr(pos);
            }
        }

        LOG_DEBUG_HTTP("🌐 Socket.IO endpoint: " + endpoint);

        // Get request body
        std::string body;
        CefRefPtr<CefPostData> postData = request->GetPostData();
        if (postData) {
            LOG_DEBUG_HTTP("🌐 Processing Socket.IO POST data...");
            CefPostData::ElementVector elements;
            postData->GetElements(elements);
            for (auto& element : elements) {
                if (element->GetType() == PDE_TYPE_BYTES) {
                    size_t size = element->GetBytesCount();
                    std::vector<char> buffer(size);
                    element->GetBytes(size, buffer.data());
                    body = std::string(buffer.data(), size);
                }
            }
        }

        // Get headers for Socket.IO forwarding
        CefRequest::HeaderMap socketHeaders;
        request->GetHeaderMap(socketHeaders);

        // Create AsyncWalletResourceHandler for Socket.IO
        return new AsyncWalletResourceHandler(method, endpoint, body, domain, browser, socketHeaders);
    }

    // Check if this is a wallet endpoint
    if (!isWalletEndpoint(url)) {
        LOG_DEBUG_HTTP("🌐 Not a wallet endpoint, allowing normal processing");
        return nullptr; // Let CEF handle it normally
    }

    LOG_DEBUG_HTTP("🌐 Wallet endpoint detected, creating async handler");

    // Get request body
    std::string body;
    CefRefPtr<CefPostData> postData = request->GetPostData();
    if (postData) {
        LOG_DEBUG_HTTP("🌐 Processing POST data...");
        CefPostData::ElementVector elements;
        postData->GetElements(elements);
        for (auto& element : elements) {
            if (element->GetType() == PDE_TYPE_BYTES) {
                size_t size = element->GetBytesCount();
                std::vector<char> buffer(size);
                element->GetBytes(size, buffer.data());
                body = std::string(buffer.data(), size);
            }
        }
    }

    // Extract endpoint from URL
    std::string endpoint;
    size_t pos = url.find("://");
    if (pos != std::string::npos) {
        pos = url.find("/", pos + 3);
        if (pos != std::string::npos) {
            endpoint = url.substr(pos);
        }
    }

    LOG_DEBUG_HTTP("🌐 Extracted endpoint: " + endpoint);

    // Log all available frame information
    LOG_DEBUG_HTTP("🌐 === FRAME DEBUGGING START ===");

    if (frame) {
        LOG_DEBUG_HTTP("🌐 Frame exists: YES");
        LOG_DEBUG_HTTP("🌐 Frame URL: " + frame->GetURL().ToString());
        LOG_DEBUG_HTTP("🌐 Frame Name: " + frame->GetName().ToString());
        LOG_DEBUG_HTTP("🌐 Frame Identifier: " + frame->GetIdentifier().ToString());
        LOG_DEBUG_HTTP("🌐 Frame Is Main: " + std::string(frame->IsMain() ? "YES" : "NO"));
        LOG_DEBUG_HTTP("🌐 Frame Is Valid: " + std::string(frame->IsValid() ? "YES" : "NO"));
    } else {
        LOG_DEBUG_HTTP("🌐 Frame exists: NO");
    }

    if (browser) {
        LOG_DEBUG_HTTP("🌐 Browser exists: YES");
        CefRefPtr<CefFrame> mainFrame = browser->GetMainFrame();
        if (mainFrame) {
            LOG_DEBUG_HTTP("🌐 Main Frame URL: " + mainFrame->GetURL().ToString());
            LOG_DEBUG_HTTP("🌐 Main Frame Name: " + mainFrame->GetName().ToString());
            LOG_DEBUG_HTTP("🌐 Main Frame Identifier: " + mainFrame->GetIdentifier().ToString());
        } else {
            LOG_DEBUG_HTTP("🌐 Main Frame: NULL");
        }
    } else {
        LOG_DEBUG_HTTP("🌐 Browser exists: NO");
    }

    // Log request information
    LOG_DEBUG_HTTP("🌐 Request URL: " + request->GetURL().ToString());
    LOG_DEBUG_HTTP("🌐 Request Method: " + request->GetMethod().ToString());
    LOG_DEBUG_HTTP("🌐 Request Referrer URL: " + request->GetReferrerURL().ToString());
    LOG_DEBUG_HTTP("🌐 Request Referrer Policy: " + std::to_string(request->GetReferrerPolicy()));

    // Log request headers
    CefRequest::HeaderMap headers;
    request->GetHeaderMap(headers);
    LOG_DEBUG_HTTP("🌐 Request Headers Count: " + std::to_string(headers.size()));
    for (const auto& header : headers) {
        LOG_DEBUG_HTTP("🌐 Header: " + header.first.ToString() + " = " + header.second.ToString());
    }

    LOG_DEBUG_HTTP("🌐 === FRAME DEBUGGING END ===");

    // Extract source domain from the main frame that made the request
    std::string domain = extractDomain(browser, request);
    LOG_DEBUG_HTTP("🌐 Final extracted source domain: " + domain);

    if (!endpoint.empty()) {
        LOG_DEBUG_HTTP("🌐 About to create AsyncWalletResourceHandler...");
        // Create and return async handler
        AsyncWalletResourceHandler* handler = new AsyncWalletResourceHandler(method, endpoint, body, domain, browser, headers);
        LOG_DEBUG_HTTP("🌐 AsyncWalletResourceHandler created successfully");
        return handler;
    }

    LOG_DEBUG_HTTP("🌐 Could not extract endpoint from URL: " + url);
    return nullptr;
}

void HttpRequestInterceptor::OnResourceRedirect(CefRefPtr<CefBrowser> browser,
                                               CefRefPtr<CefFrame> frame,
                                               CefRefPtr<CefRequest> request,
                                               CefRefPtr<CefResponse> response,
                                               CefString& new_url) {
    CEF_REQUIRE_IO_THREAD();
    LOG_DEBUG_HTTP("🌐 Resource redirect: " + new_url.ToString());
}

bool HttpRequestInterceptor::OnResourceResponse(CefRefPtr<CefBrowser> browser,
                                              CefRefPtr<CefFrame> frame,
                                              CefRefPtr<CefRequest> request,
                                              CefRefPtr<CefResponse> response) {
    CEF_REQUIRE_IO_THREAD();
    return false;
}


bool HttpRequestInterceptor::isWalletEndpoint(const std::string& url) {
    // Check if URL contains wallet endpoints
    return (url.find("/brc100/") != std::string::npos ||
            url.find("/wallet/") != std::string::npos ||
            url.find("/transaction/") != std::string::npos ||
            url.find("/getVersion") != std::string::npos ||
            url.find("/getPublicKey") != std::string::npos ||
            url.find("/createAction") != std::string::npos ||
            url.find("/signAction") != std::string::npos ||
            url.find("/processAction") != std::string::npos ||
            url.find("/isAuthenticated") != std::string::npos ||
            url.find("/createSignature") != std::string::npos ||
            url.find("/api/brc-100/") != std::string::npos ||
            url.find("/waitForAuthentication") != std::string::npos ||
            url.find("/listOutputs") != std::string::npos ||
            url.find("/createHmac") != std::string::npos ||
            url.find("/verifyHmac") != std::string::npos ||
            url.find("/verifySignature") != std::string::npos ||
            url.find("/getNetwork") != std::string::npos ||
            url.find("/socket.io/") != std::string::npos ||
            url.find("/.well-known/auth") != std::string::npos ||
            url.find("/listMessages") != std::string::npos ||
            url.find("/sendMessage") != std::string::npos ||
            url.find("/acknowledgeMessage") != std::string::npos);
}

bool HttpRequestInterceptor::isSocketIOConnection(const std::string& url) {
    // Check if this is a Socket.IO connection to our daemon or Babbage messagebox
    bool isLocalhost = url.find("localhost:3301") != std::string::npos;
    bool isBabbageMessagebox = url.find("messagebox.babbage.systems/socket.io/") != std::string::npos;
    bool isSocketIO = url.find("/socket.io/") != std::string::npos;

    LOG_DEBUG_HTTP("🌐 Checking Socket.IO connection: " + url + " - localhost: " + (isLocalhost ? "true" : "false") + ", babbage: " + (isBabbageMessagebox ? "true" : "false") + ", socket.io: " + (isSocketIO ? "true" : "false"));

    return (isLocalhost && isSocketIO) || isBabbageMessagebox;
}

std::string HttpRequestInterceptor::extractDomain(CefRefPtr<CefBrowser> browser, CefRefPtr<CefRequest> request) {
    std::string domain;

    // Use main frame URL as the primary source (most reliable)
    if (browser) {
        CefRefPtr<CefFrame> mainFrame = browser->GetMainFrame();
        if (mainFrame && mainFrame->GetURL().length() > 0) {
            std::string mainFrameUrl = mainFrame->GetURL().ToString();
            LOG_DEBUG_HTTP("🌐 Using main frame URL for domain extraction: " + mainFrameUrl);
            size_t protocolPos = mainFrameUrl.find("://");
            if (protocolPos != std::string::npos) {
                size_t domainStart = protocolPos + 3;
                size_t domainEnd = mainFrameUrl.find("/", domainStart);
                if (domainEnd != std::string::npos) {
                    domain = mainFrameUrl.substr(domainStart, domainEnd - domainStart);
                } else {
                    domain = mainFrameUrl.substr(domainStart);
                }
            }
        }
    }

    // Fallback to referrer URL if main frame URL is not available
    if (domain.empty()) {
        std::string referrerUrl = request->GetReferrerURL().ToString();
        if (!referrerUrl.empty()) {
            LOG_DEBUG_HTTP("🌐 Using referrer URL for domain extraction (fallback): " + referrerUrl);
            size_t protocolPos = referrerUrl.find("://");
            if (protocolPos != std::string::npos) {
                size_t domainStart = protocolPos + 3;
                size_t domainEnd = referrerUrl.find("/", domainStart);
                if (domainEnd != std::string::npos) {
                    domain = referrerUrl.substr(domainStart, domainEnd - domainStart);
                } else {
                    domain = referrerUrl.substr(domainStart);
                }
            }
        }
    }

    LOG_DEBUG_HTTP("🌐 Extracted domain: " + domain);
    return domain;
}
