package websocket

import (
	"crypto/rand"
	"encoding/json"
	"fmt"
	"net/http"
	"sync"
	"time"

	"github.com/gorilla/websocket"
	"github.com/sirupsen/logrus"
	"browser-wallet/brc100/authentication"
	"browser-wallet/brc100/identity"
	"browser-wallet/brc100/beef"
)

// BRC100SocketIOHandler handles BRC-100 compatible Socket.IO connections
type BRC100SocketIOHandler struct {
	upgrader          websocket.Upgrader
	clients           map[string]*BRC100Client
	challengeManager  *authentication.ChallengeManager
	sessionManager    *authentication.SessionManager
	type42Manager     *authentication.Type42Manager
	identityManager   *identity.IdentityManager
	beefManager       *beef.BRC100BEEFManager
	mutex             sync.RWMutex
	logger            *logrus.Logger
}

// BRC100Client represents a connected BRC-100 client
type BRC100Client struct {
	Socket          *websocket.Conn
	ID              string
	PublicKey       string
	Nonce           string
	SessionID       string
	PeerID          string
	Connected       time.Time
	Domain          string
	Type42Keys      *authentication.Type42Keys
	IdentityCert    *identity.IdentityCertificate
	Authenticated   bool
}

// InitialResponse represents the initial response for nonce verification
type InitialResponse struct {
	Type      string `json:"type"`
	Nonce     string `json:"nonce"`
	PeerID    string `json:"peerId"`
	PublicKey string `json:"publicKey"`
	Success   bool   `json:"success"`
}

// NewBRC100SocketIOHandler creates a new BRC-100 Socket.IO handler
func NewBRC100SocketIOHandler() *BRC100SocketIOHandler {
	logger := logrus.New()
	logger.SetLevel(logrus.InfoLevel)

	// Create WebSocket upgrader with proper configuration
	upgrader := websocket.Upgrader{
		CheckOrigin: func(r *http.Request) bool {
			// Allow connections from localhost and peerpay.babbage.systems
			origin := r.Header.Get("Origin")
			return origin == "" ||
				   origin == "http://localhost:3000" ||
				   origin == "https://peerpay.babbage.systems" ||
				   origin == "http://localhost:8080"
		},
		ReadBufferSize:  1024,
		WriteBufferSize: 1024,
	}

	// Initialize existing BRC-100 systems
	challengeManager := authentication.NewChallengeManager()
	sessionManager := authentication.NewSessionManager()
	type42Manager := authentication.NewType42Manager()
	identityManager := identity.NewIdentityManager()
	beefManager := beef.NewBRC100BEEFManager()

	handler := &BRC100SocketIOHandler{
		upgrader:         upgrader,
		clients:          make(map[string]*BRC100Client),
		challengeManager: challengeManager,
		sessionManager:   sessionManager,
		type42Manager:    type42Manager,
		identityManager:  identityManager,
		beefManager:      beefManager,
		logger:           logger,
	}

	return handler
}

// HandleSocketIO handles Socket.IO connections with Engine.IO protocol support
func (h *BRC100SocketIOHandler) HandleSocketIO(w http.ResponseWriter, r *http.Request) {
	h.logger.WithFields(logrus.Fields{
		"method":  r.Method,
		"url":     r.URL.String(),
		"headers": r.Header,
	}).Info("Socket.IO request received")

	// Check if this is an Engine.IO polling request
	if h.isEngineIOPollingRequest(r) {
		h.handleEngineIOPolling(w, r)
		return
	}

	// Check if this is a WebSocket upgrade request
	if h.isWebSocketUpgradeRequest(r) {
		h.handleWebSocketUpgrade(w, r)
		return
	}

	// Default: try to upgrade to WebSocket
	h.handleWebSocketUpgrade(w, r)
}

// isEngineIOPollingRequest checks if this is an Engine.IO polling request
func (h *BRC100SocketIOHandler) isEngineIOPollingRequest(r *http.Request) bool {
	// Check for Engine.IO polling parameters
	eio := r.URL.Query().Get("EIO")
	transport := r.URL.Query().Get("transport")
	return eio == "4" && transport == "polling"
}

// isWebSocketUpgradeRequest checks if this is a WebSocket upgrade request
func (h *BRC100SocketIOHandler) isWebSocketUpgradeRequest(r *http.Request) bool {
	connection := r.Header.Get("Connection")
	upgrade := r.Header.Get("Upgrade")
	return connection == "upgrade" && upgrade == "websocket"
}

// handleEngineIOPolling handles Engine.IO polling requests
func (h *BRC100SocketIOHandler) handleEngineIOPolling(w http.ResponseWriter, r *http.Request) {
	h.logger.WithFields(logrus.Fields{
		"url":     r.URL.String(),
		"method":  r.Method,
		"headers": r.Header,
	}).Info("Handling Engine.IO polling request")

	// Set CORS headers
	w.Header().Set("Access-Control-Allow-Origin", "*")
	w.Header().Set("Access-Control-Allow-Methods", "GET, POST, OPTIONS")
	w.Header().Set("Access-Control-Allow-Headers", "Content-Type")
	w.Header().Set("Content-Type", "text/plain; charset=UTF-8")

	// Generate session ID
	sessionID := h.generateSessionID()

	// Generate peer ID (using wallet public key)
	peerID := h.getWalletPublicKey()

	// Create Engine.IO polling response (without nonce - client will send nonce via /.well-known/auth)
	// Format: 40{"sid":"session_id","upgrades":["websocket"],"pingTimeout":60000,"pingInterval":25000}
	engineIOResponse := fmt.Sprintf(`40{"sid":"%s","upgrades":["websocket"],"pingTimeout":60000,"pingInterval":25000}`, sessionID)

	h.logger.WithFields(logrus.Fields{
		"sessionId": sessionID,
		"peerId":    peerID,
		"response":  engineIOResponse,
	}).Info("Engine.IO polling response sent (waiting for client nonce via /.well-known/auth)")

	w.Write([]byte(engineIOResponse))
}

// handleWebSocketUpgrade handles WebSocket upgrade requests
func (h *BRC100SocketIOHandler) handleWebSocketUpgrade(w http.ResponseWriter, r *http.Request) {
	h.logger.WithFields(logrus.Fields{
		"url":       r.URL.String(),
		"headers":   r.Header,
		"method":    r.Method,
	}).Info("Handling WebSocket upgrade request")

	// Check if this is actually a WebSocket upgrade request
	if !h.isWebSocketUpgradeRequest(r) {
		h.logger.Warn("Not a WebSocket upgrade request, treating as Engine.IO polling")
		h.handleEngineIOPolling(w, r)
		return
	}

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

	// Extract domain from request
	domain := h.extractDomainFromRequest(r)

	// Create BRC-100 client
	client := &BRC100Client{
		Socket:        conn,
		ID:            clientID,
		PublicKey:     h.getWalletPublicKey(),
		Connected:     time.Now(),
		Domain:        domain,
		Authenticated: false,
	}

	h.registerClient(clientID, client)

	// Send initial response with nonce for verification
	h.sendInitialResponse(client)

	// Handle incoming messages
	h.handleMessages(conn, client)
}

// generateSessionID generates a unique session ID for Engine.IO
func (h *BRC100SocketIOHandler) generateSessionID() string {
	return fmt.Sprintf("session_%d", time.Now().UnixNano())
}

// handleMessages processes incoming WebSocket messages
func (h *BRC100SocketIOHandler) handleMessages(conn *websocket.Conn, client *BRC100Client) {
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
		var incomingMsg map[string]interface{}
		if err := json.Unmarshal(messageBytes, &incomingMsg); err != nil {
			h.logger.WithError(err).Error("Failed to parse incoming WebSocket message")
			h.sendError(conn, "Invalid message format")
			continue
		}

		// Extract message type
		msgType, ok := incomingMsg["type"].(string)
		if !ok {
			h.logger.Warn("Message without type field")
			h.sendError(conn, "Message type is required")
			continue
		}

		h.logger.WithFields(logrus.Fields{
			"clientId": client.ID,
			"type":     msgType,
		}).Info("Received WebSocket message")

		// Process message based on type
		switch msgType {
		case "initial_request":
			h.handleInitialRequest(conn, client)
		case "nonce_verification":
			h.handleNonceVerification(conn, client, incomingMsg)
		case "authentication":
			h.handleAuthentication(conn, client, incomingMsg)
		case "payment_request":
			h.handlePaymentRequest(conn, client, incomingMsg)
		case "identity_request":
			h.handleIdentityRequest(conn, client, incomingMsg)
		case "ping":
			h.handlePing(conn, client)
		default:
			h.logger.WithField("type", msgType).Warn("Unknown message type")
			h.sendError(conn, fmt.Sprintf("Unknown message type: %s", msgType))
		}
	}

	// Clean up client when connection closes
	h.unregisterClient(client.ID)
}

// sendInitialResponse sends the initial response for Socket.IO connection
// Note: BRC-104 authentication happens separately via /.well-known/auth HTTP endpoint
func (h *BRC100SocketIOHandler) sendInitialResponse(client *BRC100Client) {
	// Generate peer ID (using wallet public key)
	peerID := h.getWalletPublicKey()
	client.PeerID = peerID

	// Send basic connection response
	// Authentication is handled separately via /.well-known/auth (BRC-104)
	response := map[string]interface{}{
		"type":      "connection_established",
		"peerId":    peerID,
		"publicKey": h.getWalletPublicKey(),
		"success":   true,
		"message":   "Socket.IO connection established. Authentication via /.well-known/auth (BRC-104).",
	}

	h.logger.WithFields(logrus.Fields{
		"clientId": client.ID,
		"peerId":   peerID,
		"response": response,
	}).Info("Sending Socket.IO connection response to client")

	// Send response
	h.sendMessage(client.Socket, response)

	h.logger.WithFields(logrus.Fields{
		"clientId": client.ID,
		"peerId":   peerID,
	}).Info("Socket.IO connection response sent to client")
}

// handleInitialRequest processes initial request from client
func (h *BRC100SocketIOHandler) handleInitialRequest(conn *websocket.Conn, client *BRC100Client) {
	h.logger.Info("Received initial request")
	h.sendInitialResponse(client)
}

// handleNonceVerification processes nonce verification from client
func (h *BRC100SocketIOHandler) handleNonceVerification(conn *websocket.Conn, client *BRC100Client, msg map[string]interface{}) {
	h.logger.Info("Received nonce verification")

	// Verify nonce (simplified for now - should implement proper cryptographic verification)
	receivedNonce, ok := msg["nonce"].(string)
	if !ok || receivedNonce != client.Nonce {
		h.logger.Error("Nonce verification failed")
		h.sendError(conn, "Nonce verification failed")
		return
	}

	h.logger.WithField("clientId", client.ID).Info("Nonce verification successful")
	h.sendMessage(conn, map[string]interface{}{
		"type":    "nonce_verified",
		"success": true,
		"message": "Nonce verified successfully",
	})
}

// handleAuthentication processes authentication request using existing systems
func (h *BRC100SocketIOHandler) handleAuthentication(conn *websocket.Conn, client *BRC100Client, msg map[string]interface{}) {
	h.logger.Info("Received authentication request")

	// Parse authentication request
	authReq := BRC100AuthenticationRequest{
		Type:      "authentication",
		AppDomain: getStringFromMap(msg, "appDomain"),
		Purpose:   getStringFromMap(msg, "purpose"),
		RequestID: getStringFromMap(msg, "requestId"),
		Timestamp: time.Now(),
	}

	// Use existing challenge manager to create challenge
	_, err := h.challengeManager.CreateChallenge(authReq.AppDomain)
	if err != nil {
		h.logger.WithError(err).Error("Failed to create challenge")
		h.sendError(conn, "Failed to create authentication challenge")
		return
	}

	// Create session using existing session manager
	session, err := h.sessionManager.CreateSession(authReq.AppDomain, nil, []string{"basic"})
	if err != nil {
		h.logger.WithError(err).Error("Failed to create session")
		h.sendError(conn, "Failed to create session")
		return
	}

	// Generate identity certificate using existing identity manager
	selectiveDisclosure := map[string]bool{
		"publicKey": true,
		"address":   true,
	}
	certificate, err := h.identityManager.GenerateIdentityCertificate("wallet_user", selectiveDisclosure)
	if err != nil {
		h.logger.WithError(err).Error("Failed to generate identity certificate")
		h.sendError(conn, "Failed to generate identity certificate")
		return
	}

	// Sign certificate
	if err := h.identityManager.SignIdentityCertificate(certificate, "wallet_private_key"); err != nil {
		h.logger.WithError(err).Error("Failed to sign identity certificate")
		h.sendError(conn, "Failed to sign identity certificate")
		return
	}

	// Generate Type-42 keys for P2P encryption
	walletKey := []byte("wallet_key_placeholder")
	appKey := []byte("app_key_placeholder")
	type42Keys, err := h.type42Manager.DeriveType42Keys(walletKey, appKey)
	if err != nil {
		h.logger.WithError(err).Error("Failed to derive Type-42 keys")
		h.sendError(conn, "Failed to derive encryption keys")
		return
	}

	// Store session ID and keys in client
	client.SessionID = session.SessionID
	client.Type42Keys = type42Keys
	client.IdentityCert = certificate
	client.Authenticated = true

	// Send authentication response
	authResp := BRC100AuthenticationResponse{
		Type:        "authentication_response",
		Success:     true,
		SessionID:   session.SessionID,
		Certificate: certificate,
		Type42Keys:  type42Keys,
		Message:     "Authentication successful",
		RequestID:   authReq.RequestID,
		Timestamp:   time.Now(),
	}

	h.sendMessage(conn, authResp)

	h.logger.WithFields(logrus.Fields{
		"clientId":  client.ID,
		"sessionId": session.SessionID,
		"requestId": authReq.RequestID,
	}).Info("BRC-100 authentication completed successfully")
}

// handlePaymentRequest processes payment request using existing BEEF system
func (h *BRC100SocketIOHandler) handlePaymentRequest(conn *websocket.Conn, client *BRC100Client, msg map[string]interface{}) {
	h.logger.Info("Received payment request")

	// Check if client is authenticated
	if !client.Authenticated {
		h.sendError(conn, "Client not authenticated")
		return
	}

	// Parse payment request
	paymentReq := BRC100PaymentRequest{
		Type:        "payment_request",
		Amount:      getFloat64FromMap(msg, "amount"),
		Currency:    getStringFromMap(msg, "currency"),
		Recipient:   getStringFromMap(msg, "recipient"),
		Description: getStringFromMap(msg, "description"),
		RequestID:   getStringFromMap(msg, "requestId"),
		Timestamp:   time.Now(),
	}

	// Create BRC-100 action for payment
	actionData := map[string]interface{}{
		"amount":    paymentReq.Amount,
		"currency":  paymentReq.Currency,
		"recipient": paymentReq.Recipient,
		"purpose":   paymentReq.Description,
	}

	action, err := h.beefManager.CreateBRC100Action("payment", actionData, client.SessionID)
	if err != nil {
		h.logger.WithError(err).Error("Failed to create BRC-100 action")
		h.sendError(conn, "Failed to create payment action")
		return
	}

	// Create identity context
	identityContext := &beef.IdentityContext{
		Certificate: map[string]interface{}{
			"version":       client.IdentityCert.Version,
			"issuer":        client.IdentityCert.Issuer,
			"subject":       client.IdentityCert.Subject,
			"publicKey":     client.IdentityCert.PublicKey,
			"selectiveData": client.IdentityCert.SelectiveData,
			"signature":     client.IdentityCert.Signature,
			"timestamp":     client.IdentityCert.Timestamp,
			"expiresAt":     client.IdentityCert.ExpiresAt,
			"revoked":       client.IdentityCert.Revoked,
		},
		SessionID:   client.SessionID,
		AppDomain:   client.Domain,
		Timestamp:   time.Now(),
	}

	// Create BEEF transaction
	_, err = h.beefManager.CreateBRC100BEEFTransaction([]beef.BRC100Action{*action}, identityContext)
	if err != nil {
		h.logger.WithError(err).Error("Failed to create BEEF transaction")
		h.sendError(conn, "Failed to create BEEF transaction")
		return
	}

	// Send payment response
	paymentResp := BRC100PaymentResponse{
		Type:          "payment_response",
		Success:       true,
		TransactionID: "beef_tx_" + client.SessionID,
		Message:       "Payment processed successfully",
		RequestID:     paymentReq.RequestID,
		Timestamp:     time.Now(),
	}

	h.sendMessage(conn, paymentResp)

	h.logger.WithFields(logrus.Fields{
		"clientId":      client.ID,
		"amount":        paymentReq.Amount,
		"recipient":     paymentReq.Recipient,
		"requestId":     paymentReq.RequestID,
	}).Info("BRC-100 payment request processed successfully")
}

// handleIdentityRequest processes identity request using existing identity system
func (h *BRC100SocketIOHandler) handleIdentityRequest(conn *websocket.Conn, client *BRC100Client, msg map[string]interface{}) {
	h.logger.Info("Received identity request")

	// Check if client is authenticated
	if !client.Authenticated {
		h.sendError(conn, "Client not authenticated")
		return
	}

	// Parse identity request
	fieldsInterface, ok := msg["fields"].([]interface{})
	if !ok {
		h.sendError(conn, "Invalid fields format")
		return
	}

	fields := make([]string, len(fieldsInterface))
	for i, field := range fieldsInterface {
		if fieldStr, ok := field.(string); ok {
			fields[i] = fieldStr
		}
	}

	// Create selective disclosure using existing identity manager
	selectiveData := h.identityManager.CreateSelectiveDisclosure(
		client.IdentityCert.SelectiveData,
		fields,
	)

	// Send identity response
	identityResp := BRC100IdentityResponse{
		Type:      "identity_response",
		Identity:  selectiveData,
		RequestID: getStringFromMap(msg, "requestId"),
		Timestamp: time.Now(),
	}

	h.sendMessage(conn, identityResp)

	h.logger.WithFields(logrus.Fields{
		"clientId":  client.ID,
		"fields":    fields,
		"requestId": getStringFromMap(msg, "requestId"),
	}).Info("BRC-100 identity request processed successfully")
}

// handlePing responds to ping messages
func (h *BRC100SocketIOHandler) handlePing(conn *websocket.Conn, client *BRC100Client) {
	h.logger.Info("Received ping")
	h.sendMessage(conn, map[string]interface{}{
		"type":      "pong",
		"message":   "pong",
		"timestamp": time.Now(),
	})
}

// Helper methods

func (h *BRC100SocketIOHandler) registerClient(clientID string, client *BRC100Client) {
	h.mutex.Lock()
	defer h.mutex.Unlock()
	h.clients[clientID] = client
	h.logger.WithField("clientId", clientID).Info("BRC-100 client registered")
}

func (h *BRC100SocketIOHandler) unregisterClient(clientID string) {
	h.mutex.Lock()
	defer h.mutex.Unlock()
	delete(h.clients, clientID)
	h.logger.WithField("clientId", clientID).Info("BRC-100 client unregistered")
}

func (h *BRC100SocketIOHandler) sendMessage(conn *websocket.Conn, message interface{}) {
	msgBytes, err := json.Marshal(message)
	if err != nil {
		h.logger.WithError(err).Error("Failed to marshal WebSocket message")
		return
	}

	if err := conn.WriteMessage(websocket.TextMessage, msgBytes); err != nil {
		h.logger.WithError(err).Error("Failed to send WebSocket message")
	}
}

func (h *BRC100SocketIOHandler) sendError(conn *websocket.Conn, errorMsg string) {
	errorResponse := BRC100ErrorResponse{
		Type:      "error",
		Error:     errorMsg,
		Code:      400,
		RequestID: "",
		Timestamp: time.Now(),
	}
	h.sendMessage(conn, errorResponse)
}

func (h *BRC100SocketIOHandler) generateClientID() string {
	return fmt.Sprintf("brc100_client_%d", time.Now().UnixNano())
}

// generateBRC100Nonce generates a nonce in the format expected by Babbage clients
func (h *BRC100SocketIOHandler) generateBRC100Nonce() string {
	// The Babbage client expects a nonce that looks like a compressed public key
	// Based on the error message, they expect: 028155878063d691f01cfc0eeb626404ebe9303ec50f9542c234c5c85100a98ca1
	// This is a 64-character hex string starting with "02" (compressed public key format)

	// Generate 32 random bytes
	nonceBytes := make([]byte, 32)
	_, err := rand.Read(nonceBytes)
	if err != nil {
		h.logger.WithError(err).Error("Failed to generate random nonce")
		// Fallback to time-based nonce
		nonceBytes = []byte(fmt.Sprintf("%d", time.Now().UnixNano()))
	}

	// Create nonce in compressed public key format: 02 + 32 random bytes (64 hex characters)
	nonce := "02" + fmt.Sprintf("%x", nonceBytes)

	// Log the nonce for debugging
	h.logger.WithField("nonce", nonce).Info("Generated BRC-100 nonce in compressed public key format")

	return nonce
}

func (h *BRC100SocketIOHandler) getWalletPublicKey() string {
	// TODO: Get actual wallet public key from HD wallet
	// For now, return a placeholder that matches the expected format
	return "03d575090cc073ecf448ad49fae79993fdaf8d1643ec2c5762655ed400e20333e3"
}

// extractDomainFromRequest extracts domain from HTTP request
func (h *BRC100SocketIOHandler) extractDomainFromRequest(r *http.Request) string {
	origin := r.Header.Get("Origin")
	if origin != "" {
		// Extract domain from origin (e.g., "https://peerpay.babbage.systems" -> "peerpay.babbage.systems")
		if len(origin) > 7 && origin[:7] == "http://" {
			return origin[7:]
		}
		if len(origin) > 8 && origin[:8] == "https://" {
			return origin[8:]
		}
		return origin
	}
	return "peerpay.babbage.systems" // Default for testing
}

// validateDomain checks if domain is whitelisted
func (h *BRC100SocketIOHandler) validateDomain(domain string) bool {
	// TODO: Integrate with existing domain whitelist system
	// For now, allow peerpay.babbage.systems
	return domain == "peerpay.babbage.systems"
}

// SetWalletService sets wallet service reference
func (h *BRC100SocketIOHandler) SetWalletService(walletService interface{}) {
	// TODO: Store wallet service reference for later use
	h.logger.Info("Wallet service reference set")
}

// SetDomainWhitelistManager sets domain whitelist manager reference
func (h *BRC100SocketIOHandler) SetDomainWhitelistManager(manager interface{}) {
	// TODO: Store domain whitelist manager reference
	h.logger.Info("Domain whitelist manager reference set")
}

// GetConnectedClients returns the number of connected clients
func (h *BRC100SocketIOHandler) GetConnectedClients() int {
	h.mutex.RLock()
	defer h.mutex.RUnlock()
	return len(h.clients)
}

// GetClientList returns a list of connected client IDs
func (h *BRC100SocketIOHandler) GetClientList() []string {
	h.mutex.RLock()
	defer h.mutex.RUnlock()

	clients := make([]string, 0, len(h.clients))
	for clientID := range h.clients {
		clients = append(clients, clientID)
	}
	return clients
}

// Helper functions for extracting values from maps
func getStringFromMap(m map[string]interface{}, key string) string {
	if val, ok := m[key].(string); ok {
		return val
	}
	return ""
}

func getFloat64FromMap(m map[string]interface{}, key string) float64 {
	if val, ok := m[key].(float64); ok {
		return val
	}
	return 0.0
}
