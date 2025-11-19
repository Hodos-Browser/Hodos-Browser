package websocket

import (
	"encoding/json"
	"fmt"
	"net/http"
	"sync"
	"time"

	"github.com/gorilla/websocket"
	"github.com/sirupsen/logrus"
)

// BRC100WebSocketHandler handles WebSocket connections for BRC-100 real-time communication
type BRC100WebSocketHandler struct {
	upgrader websocket.Upgrader
	clients  map[string]*websocket.Conn
	mutex    sync.RWMutex
	logger   *logrus.Logger
}

// WebSocketMessage represents a message sent over WebSocket
type WebSocketMessage struct {
	Type      string                 `json:"type"`
	SessionID string                 `json:"sessionId,omitempty"`
	Data      map[string]interface{} `json:"data,omitempty"`
	Error     string                 `json:"error,omitempty"`
	Timestamp time.Time              `json:"timestamp"`
}

// AuthenticationRequest represents a BRC-100 authentication request via WebSocket
type AuthRequest struct {
	AppDomain string `json:"appDomain"`
	Purpose   string `json:"purpose"`
	SessionID string `json:"sessionId,omitempty"`
}

// AuthenticationResponse represents the response to an authentication request
type AuthResponse struct {
	Success   bool   `json:"success"`
	SessionID string `json:"sessionId,omitempty"`
	Error     string `json:"error,omitempty"`
}

// NewBRC100WebSocketHandler creates a new WebSocket handler for BRC-100
func NewBRC100WebSocketHandler() *BRC100WebSocketHandler {
	logger := logrus.New()
	logger.SetLevel(logrus.InfoLevel)

	return &BRC100WebSocketHandler{
		upgrader: websocket.Upgrader{
			CheckOrigin: func(r *http.Request) bool {
				// Allow connections from localhost for development
				// In production, implement proper origin checking
				return true
			},
			ReadBufferSize:  1024,
			WriteBufferSize: 1024,
		},
		clients: make(map[string]*websocket.Conn),
		logger:  logger,
	}
}

// HandleWebSocket handles WebSocket connections for BRC-100
func (h *BRC100WebSocketHandler) HandleWebSocket(w http.ResponseWriter, r *http.Request) {
	h.logger.Info("New WebSocket connection attempt")

	// Upgrade HTTP connection to WebSocket
	conn, err := h.upgrader.Upgrade(w, r, nil)
	if err != nil {
		h.logger.WithError(err).Error("Failed to upgrade connection to WebSocket")
		return
	}
	defer conn.Close()

	h.logger.Info("WebSocket connection established successfully")

	// Generate a unique client ID for this connection
	clientID := h.generateClientID()
	h.registerClient(clientID, conn)
	defer h.unregisterClient(clientID)

	// Send welcome message
	welcomeMsg := WebSocketMessage{
		Type:      "welcome",
		SessionID: clientID,
		Data: map[string]interface{}{
			"message": "Connected to BRC-100 WebSocket server",
			"clientId": clientID,
		},
		Timestamp: time.Now(),
	}
	h.sendMessage(conn, welcomeMsg)

	// Handle incoming messages
	h.handleMessages(conn, clientID)
}

// handleMessages processes incoming WebSocket messages
func (h *BRC100WebSocketHandler) handleMessages(conn *websocket.Conn, clientID string) {
	for {
		// Read message from client
		_, messageBytes, err := conn.ReadMessage()
		if err != nil {
			if websocket.IsUnexpectedCloseError(err, websocket.CloseGoingAway, websocket.CloseAbnormalClosure) {
				h.logger.WithError(err).Error("WebSocket connection closed unexpectedly")
			} else {
				h.logger.WithError(err).Info("WebSocket connection closed")
			}
			break
		}

		// Parse incoming message
		var incomingMsg WebSocketMessage
		if err := json.Unmarshal(messageBytes, &incomingMsg); err != nil {
			h.logger.WithError(err).Error("Failed to parse incoming WebSocket message")
			h.sendError(conn, "Invalid message format")
			continue
		}

		h.logger.WithFields(logrus.Fields{
			"clientId": clientID,
			"type":     incomingMsg.Type,
		}).Info("Received WebSocket message")

		// Process message based on type
		switch incomingMsg.Type {
		case "auth_request":
			h.handleAuthenticationRequest(conn, incomingMsg, clientID)
		case "ping":
			h.handlePing(conn, incomingMsg)
		case "session_status":
			h.handleSessionStatus(conn, incomingMsg, clientID)
		default:
			h.logger.WithField("type", incomingMsg.Type).Warn("Unknown message type")
			h.sendError(conn, fmt.Sprintf("Unknown message type: %s", incomingMsg.Type))
		}
	}
}

// handleAuthenticationRequest processes BRC-100 authentication requests
func (h *BRC100WebSocketHandler) handleAuthenticationRequest(conn *websocket.Conn, msg WebSocketMessage, clientID string) {
	h.logger.WithField("clientId", clientID).Info("Processing authentication request")

	// Extract authentication request data
	authData, ok := msg.Data["authRequest"].(map[string]interface{})
	if !ok {
		h.sendError(conn, "Invalid authentication request format")
		return
	}

	// Parse authentication request
	authReq := AuthRequest{
		AppDomain: getStringFromMap(authData, "appDomain"),
		Purpose:   getStringFromMap(authData, "purpose"),
		SessionID: getStringFromMap(authData, "sessionId"),
	}

	// Validate authentication request
	if authReq.AppDomain == "" {
		h.sendError(conn, "App domain is required for authentication")
		return
	}

	// Generate session ID if not provided
	if authReq.SessionID == "" {
		authReq.SessionID = h.generateSessionID(authReq.AppDomain)
	}

	// TODO: Integrate with actual BRC-100 authentication logic
	// For now, simulate successful authentication
	authResp := AuthResponse{
		Success:   true,
		SessionID: authReq.SessionID,
	}

	// Send authentication response
	responseMsg := WebSocketMessage{
		Type:      "auth_response",
		SessionID: authReq.SessionID,
		Data: map[string]interface{}{
			"authResponse": authResp,
		},
		Timestamp: time.Now(),
	}

	h.sendMessage(conn, responseMsg)
	h.logger.WithFields(logrus.Fields{
		"clientId":  clientID,
		"sessionId": authReq.SessionID,
		"appDomain": authReq.AppDomain,
	}).Info("Authentication request processed successfully")
}

// handlePing responds to ping messages
func (h *BRC100WebSocketHandler) handlePing(conn *websocket.Conn, msg WebSocketMessage) {
	pongMsg := WebSocketMessage{
		Type:      "pong",
		SessionID: msg.SessionID,
		Data: map[string]interface{}{
			"message": "pong",
		},
		Timestamp: time.Now(),
	}
	h.sendMessage(conn, pongMsg)
}

// handleSessionStatus provides session status information
func (h *BRC100WebSocketHandler) handleSessionStatus(conn *websocket.Conn, msg WebSocketMessage, clientID string) {
	h.logger.WithField("clientId", clientID).Info("Handling session status request")

	// TODO: Integrate with actual session management
	// For now, return basic status
	statusMsg := WebSocketMessage{
		Type:      "session_status_response",
		SessionID: msg.SessionID,
		Data: map[string]interface{}{
			"status":    "active",
			"clientId":  clientID,
			"timestamp": time.Now(),
		},
		Timestamp: time.Now(),
	}
	h.sendMessage(conn, statusMsg)
}

// BroadcastToClient sends a message to a specific client
func (h *BRC100WebSocketHandler) BroadcastToClient(sessionID string, message interface{}) {
	h.mutex.RLock()
	conn, exists := h.clients[sessionID]
	h.mutex.RUnlock()

	if !exists {
		h.logger.WithField("sessionId", sessionID).Warn("Client not found for broadcast")
		return
	}

	msg := WebSocketMessage{
		Type:      "broadcast",
		SessionID: sessionID,
		Data: map[string]interface{}{
			"message": message,
		},
		Timestamp: time.Now(),
	}

	h.sendMessage(conn, msg)
	h.logger.WithField("sessionId", sessionID).Info("Message broadcasted to client")
}

// BroadcastToAllClients sends a message to all connected clients
func (h *BRC100WebSocketHandler) BroadcastToAllClients(message interface{}) {
	h.mutex.RLock()
	clients := make(map[string]*websocket.Conn)
	for id, conn := range h.clients {
		clients[id] = conn
	}
	h.mutex.RUnlock()

	for clientID, conn := range clients {
		msg := WebSocketMessage{
			Type:      "broadcast",
			SessionID: clientID,
			Data: map[string]interface{}{
				"message": message,
			},
			Timestamp: time.Now(),
		}
		h.sendMessage(conn, msg)
	}

	h.logger.WithField("clientCount", len(clients)).Info("Message broadcasted to all clients")
}

// Helper methods

func (h *BRC100WebSocketHandler) registerClient(clientID string, conn *websocket.Conn) {
	h.mutex.Lock()
	defer h.mutex.Unlock()
	h.clients[clientID] = conn
	h.logger.WithField("clientId", clientID).Info("Client registered")
}

func (h *BRC100WebSocketHandler) unregisterClient(clientID string) {
	h.mutex.Lock()
	defer h.mutex.Unlock()
	delete(h.clients, clientID)
	h.logger.WithField("clientId", clientID).Info("Client unregistered")
}

func (h *BRC100WebSocketHandler) sendMessage(conn *websocket.Conn, msg WebSocketMessage) {
	msgBytes, err := json.Marshal(msg)
	if err != nil {
		h.logger.WithError(err).Error("Failed to marshal WebSocket message")
		return
	}

	if err := conn.WriteMessage(websocket.TextMessage, msgBytes); err != nil {
		h.logger.WithError(err).Error("Failed to send WebSocket message")
	}
}

func (h *BRC100WebSocketHandler) sendError(conn *websocket.Conn, errorMsg string) {
	errorResponse := WebSocketMessage{
		Type: "error",
		Error: errorMsg,
		Timestamp: time.Now(),
	}
	h.sendMessage(conn, errorResponse)
}

func (h *BRC100WebSocketHandler) generateClientID() string {
	return fmt.Sprintf("client_%d", time.Now().UnixNano())
}

func (h *BRC100WebSocketHandler) generateSessionID(appDomain string) string {
	return fmt.Sprintf("session_%s_%d", appDomain, time.Now().UnixNano())
}


// GetConnectedClients returns the number of connected clients
func (h *BRC100WebSocketHandler) GetConnectedClients() int {
	h.mutex.RLock()
	defer h.mutex.RUnlock()
	return len(h.clients)
}

// GetClientList returns a list of connected client IDs
func (h *BRC100WebSocketHandler) GetClientList() []string {
	h.mutex.RLock()
	defer h.mutex.RUnlock()

	clients := make([]string, 0, len(h.clients))
	for clientID := range h.clients {
		clients = append(clients, clientID)
	}
	return clients
}
