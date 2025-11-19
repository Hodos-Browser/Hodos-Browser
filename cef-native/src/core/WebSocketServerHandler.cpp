#include "../../include/core/WebSocketServerHandler.h"
#include <iostream>
#include <sstream>
#include <fstream>
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

public:
    static void Log(const std::string& message, int level = 1, int process = 2) {
        std::ofstream logFile("debug_output.log", std::ios::app);
        if (logFile.is_open()) {
            logFile << "[" << GetTimestamp() << "] [BROWSER] [DEBUG] " << message << std::endl;
            logFile.close();
        }
        std::cout << "[" << GetTimestamp() << "] [BROWSER] [DEBUG] " << message << std::endl;
    }
};

// Convenience macro for easier logging
#define LOG_DEBUG_BROWSER(msg) Logger::Log(msg, 0, 2)

// Static member definitions
CefRefPtr<CefServer> WebSocketServerHandler::server_instance_ = nullptr;
bool WebSocketServerHandler::server_running_ = false;

WebSocketServerHandler::WebSocketServerHandler() {
    LOG_DEBUG_BROWSER("🌐 WebSocketServerHandler created");
}

WebSocketServerHandler::~WebSocketServerHandler() {
    LOG_DEBUG_BROWSER("🌐 WebSocketServerHandler destroyed");
}

void WebSocketServerHandler::OnServerCreated(CefRefPtr<CefServer> server) {
    LOG_DEBUG_BROWSER("🌐 WebSocket Server created successfully");
    LOG_DEBUG_BROWSER("🌐 Server address: " + server->GetAddress().ToString());
    server_running_ = true;
}

void WebSocketServerHandler::OnServerDestroyed(CefRefPtr<CefServer> server) {
    LOG_DEBUG_BROWSER("🌐 WebSocket Server destroyed");
    server_running_ = false;
    server_instance_ = nullptr;
}

void WebSocketServerHandler::OnClientConnected(CefRefPtr<CefServer> server, int connection_id) {
    LOG_DEBUG_BROWSER("🌐 WebSocket client connected: " + std::to_string(connection_id));
    active_connections_[connection_id] = true;
}

void WebSocketServerHandler::OnClientDisconnected(CefRefPtr<CefServer> server, int connection_id) {
    LOG_DEBUG_BROWSER("🌐 WebSocket client disconnected: " + std::to_string(connection_id));
    active_connections_.erase(connection_id);
}

void WebSocketServerHandler::OnHttpRequest(CefRefPtr<CefServer> server, int connection_id,
                                         const CefString& client_address, CefRefPtr<CefRequest> request) {
    std::string url = request->GetURL().ToString();
    std::string method = request->GetMethod().ToString();

    LOG_DEBUG_BROWSER("🌐 HTTP request received: " + method + " " + url);

    // Check if this is a Socket.IO polling request
    if (IsSocketIORequest(url)) {
        LOG_DEBUG_BROWSER("🌐 Socket.IO HTTP request detected, redirecting to Go daemon");
        // For now, send a 404 - we'll implement proper proxying later
        server->SendHttp404Response(connection_id);
    } else {
        LOG_DEBUG_BROWSER("🌐 Non-Socket.IO HTTP request, sending 404");
        server->SendHttp404Response(connection_id);
    }
}

void WebSocketServerHandler::OnWebSocketRequest(CefRefPtr<CefServer> server, int connection_id,
                                              const CefString& client_address, CefRefPtr<CefRequest> request,
                                              CefRefPtr<CefCallback> callback) {
    std::string url = request->GetURL().ToString();
    std::string method = request->GetMethod().ToString();

    LOG_DEBUG_BROWSER("🌐 WebSocket upgrade request received: " + method + " " + url);
    LOG_DEBUG_BROWSER("🌐 Client address: " + client_address.ToString());

    // Check if this is a Socket.IO WebSocket request
    if (IsSocketIORequest(url)) {
        LOG_DEBUG_BROWSER("🌐 Socket.IO WebSocket request detected - ACCEPTING");
        LogWebSocketActivity("WebSocket upgrade accepted", connection_id, url);

        // Accept the WebSocket connection
        callback->Continue();
    } else {
        LOG_DEBUG_BROWSER("🌐 Non-Socket.IO WebSocket request - REJECTING");
        LogWebSocketActivity("WebSocket upgrade rejected", connection_id, url);

        // Reject the WebSocket connection
        callback->Cancel();
    }
}

void WebSocketServerHandler::OnWebSocketConnected(CefRefPtr<CefServer> server, int connection_id) {
    LOG_DEBUG_BROWSER("🌐 WebSocket connection established: " + std::to_string(connection_id));
    LogWebSocketActivity("WebSocket connected", connection_id);

    // TODO: Here we would establish a connection to the Go daemon's WebSocket handler
    // For now, we'll just log the connection
}

void WebSocketServerHandler::OnWebSocketMessage(CefRefPtr<CefServer> server, int connection_id,
                                              const void* data, size_t data_size) {
    std::string message(static_cast<const char*>(data), data_size);
    LOG_DEBUG_BROWSER("🌐 WebSocket message received from " + std::to_string(connection_id) + ": " + message);

    // TODO: Forward this message to the Go daemon's WebSocket handler
    // For now, we'll just echo it back
    server->SendWebSocketMessage(connection_id, data, data_size);
}

bool WebSocketServerHandler::IsSocketIORequest(const CefString& url) {
    std::string url_str = url.ToString();
    return url_str.find("/socket.io/") != std::string::npos;
}

void WebSocketServerHandler::LogWebSocketActivity(const std::string& activity, int connection_id, const std::string& details) {
    std::stringstream ss;
    ss << "🌐 [WS-" << connection_id << "] " << activity;
    if (!details.empty()) {
        ss << " - " << details;
    }
    LOG_DEBUG_BROWSER(ss.str());
}

// Static methods
void WebSocketServerHandler::StartWebSocketServer() {
    if (server_running_) {
        LOG_DEBUG_BROWSER("🌐 WebSocket server already running");
        return;
    }

    LOG_DEBUG_BROWSER("🌐 Starting WebSocket server on localhost:3302");

    CefRefPtr<WebSocketServerHandler> handler = new WebSocketServerHandler();
    CefServer::CreateServer("127.0.0.1", 3302, 10, handler);
}

void WebSocketServerHandler::StopWebSocketServer() {
    if (server_instance_ && server_running_) {
        LOG_DEBUG_BROWSER("🌐 Stopping WebSocket server");
        server_instance_->Shutdown();
    }
}

bool WebSocketServerHandler::IsServerRunning() {
    return server_running_ && server_instance_ && server_instance_->IsRunning();
}
