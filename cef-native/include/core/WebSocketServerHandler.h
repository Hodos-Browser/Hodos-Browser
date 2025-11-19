#pragma once

#include "include/cef_server.h"
#include "include/cef_base.h"
#include "include/cef_request.h"
#include "include/cef_callback.h"
#include <string>
#include <map>

///
/// CEF WebSocket Server Handler for intercepting Babbage WebSocket connections
/// This server listens on localhost:3302 and proxies WebSocket connections to the Go daemon
///
class WebSocketServerHandler : public CefServerHandler {
public:
    explicit WebSocketServerHandler();
    ~WebSocketServerHandler();

    // CefServerHandler methods
    void OnServerCreated(CefRefPtr<CefServer> server) override;
    void OnServerDestroyed(CefRefPtr<CefServer> server) override;
    void OnClientConnected(CefRefPtr<CefServer> server, int connection_id) override;
    void OnClientDisconnected(CefRefPtr<CefServer> server, int connection_id) override;
    void OnHttpRequest(CefRefPtr<CefServer> server, int connection_id,
                      const CefString& client_address, CefRefPtr<CefRequest> request) override;
    void OnWebSocketRequest(CefRefPtr<CefServer> server, int connection_id,
                           const CefString& client_address, CefRefPtr<CefRequest> request,
                           CefRefPtr<CefCallback> callback) override;
    void OnWebSocketConnected(CefRefPtr<CefServer> server, int connection_id) override;
    void OnWebSocketMessage(CefRefPtr<CefServer> server, int connection_id,
                           const void* data, size_t data_size) override;

    // Static methods for server management
    static void StartWebSocketServer();
    static void StopWebSocketServer();
    static bool IsServerRunning();

private:
    // Connection management
    std::map<int, bool> active_connections_;

    // Server instance
    static CefRefPtr<CefServer> server_instance_;
    static bool server_running_;

    // Helper methods
    bool IsSocketIORequest(const CefString& url);
    void LogWebSocketActivity(const std::string& activity, int connection_id, const std::string& details = "");

    IMPLEMENT_REFCOUNTING(WebSocketServerHandler);
    DISALLOW_COPY_AND_ASSIGN(WebSocketServerHandler);
};
