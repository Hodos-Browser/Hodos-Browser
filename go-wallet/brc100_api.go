package main

import (
	"encoding/json"
	"fmt"
	"net/http"
	"time"

	"browser-wallet/brc100/authentication"
	"browser-wallet/brc100/beef"
	"browser-wallet/brc100/identity"
	"browser-wallet/brc100/spv"

	"github.com/sirupsen/logrus"
)

// BRC100Service handles BRC-100 related operations
type BRC100Service struct {
	identityManager     *identity.IdentityManager
	type42Manager       *authentication.Type42Manager
	sessionManager      *authentication.SessionManager
	challengeManager    *authentication.ChallengeManager
	beefManager         *beef.BRC100BEEFManager
	spvManager          *spv.SPVManager
	walletManager       *WalletManager
	logger              *logrus.Logger
}

// NewBRC100Service creates a new BRC-100 service instance
func NewBRC100Service(walletManager *WalletManager) *BRC100Service {
	logger := logrus.New()
	logger.SetLevel(logrus.InfoLevel)

	service := &BRC100Service{
		identityManager:  identity.NewIdentityManager(),
		type42Manager:    authentication.NewType42Manager(),
		sessionManager:   authentication.NewSessionManager(),
		challengeManager: authentication.NewChallengeManager(),
		beefManager:      beef.NewBRC100BEEFManager(),
		spvManager:       spv.NewSPVManager(),
		walletManager:    walletManager,
		logger:           logger,
	}

	return service
}

// getCurrentWalletAddress returns the current wallet address for BRC-100 operations
func (service *BRC100Service) getCurrentWalletAddress() (string, error) {
	if service.walletManager == nil {
		return "", fmt.Errorf("wallet manager not initialized")
	}

	addressInfo, err := service.walletManager.GetCurrentAddress()
	if err != nil {
		return "", fmt.Errorf("failed to get current address: %v", err)
	}

	return addressInfo.Address, nil
}

// signWithWalletPrivateKey signs data using the wallet's private key
func (service *BRC100Service) signWithWalletPrivateKey(data []byte) (string, error) {
	if service.walletManager == nil {
		return "", fmt.Errorf("wallet manager not initialized")
	}

	addressInfo, err := service.walletManager.GetCurrentAddress()
	if err != nil {
		return "", fmt.Errorf("failed to get current address: %v", err)
	}

	privateKeyHex, err := service.walletManager.GetPrivateKeyForAddress(addressInfo.Address)
	if err != nil {
		return "", fmt.Errorf("failed to get private key: %v", err)
	}

	// For now, we'll use a simple signature approach
	// In a production system, you'd use proper cryptographic signing
	signature := fmt.Sprintf("wallet_signature_%x_%s", data, privateKeyHex[:16])
	return signature, nil
}

// BRC100Request represents a generic BRC-100 request
type BRC100Request struct {
	Data map[string]interface{} `json:"data"`
}

// BRC100Response represents a generic BRC-100 response
type BRC100Response struct {
	Success bool                   `json:"success"`
	Data    map[string]interface{} `json:"data,omitempty"`
	Error   string                 `json:"error,omitempty"`
}

// IdentityRequest represents an identity-related request
type IdentityRequest struct {
	Subject    string                 `json:"subject"`
	Attributes map[string]interface{} `json:"attributes,omitempty"`
	Selective  []string               `json:"selective,omitempty"`
}

// AuthenticationRequest represents an authentication request
type AuthenticationRequest struct {
	AppID       string `json:"appId"`
	Challenge   string `json:"challenge"`
	Response    string `json:"response"`
	SessionID   string `json:"sessionId,omitempty"`
	IdentityID  string `json:"identityId,omitempty"`
}

// SessionRequest represents a session management request
type SessionRequest struct {
	SessionID  string `json:"sessionId,omitempty"`
	IdentityID string `json:"identityId"`
	AppID      string `json:"appId"`
}

// BEEFRequest represents a BEEF transaction request
type BEEFRequest struct {
	Actions        []beef.BRC100Action `json:"actions"`
	SessionID      string              `json:"sessionId,omitempty"`
	AppDomain      string              `json:"appDomain,omitempty"`
	IncludeSPVData bool                `json:"includeSPVData,omitempty"` // New: Include SPV data in BEEF
}

// SPVRequest represents an SPV verification request
type SPVRequest struct {
	TransactionID string                 `json:"transactionId"`
	IdentityData  map[string]interface{} `json:"identityData"`
}

// SetupBRC100Routes sets up all BRC-100 related HTTP routes
func (ws *WalletService) SetupBRC100Routes() {
	brc100Service := NewBRC100Service(ws.walletManager)

	// Identity Management Endpoints
	http.HandleFunc("/brc100/identity/generate", func(w http.ResponseWriter, r *http.Request) {
		handleIdentityGenerate(w, r, brc100Service)
	})

	http.HandleFunc("/brc100/identity/validate", func(w http.ResponseWriter, r *http.Request) {
		handleIdentityValidate(w, r, brc100Service)
	})

	http.HandleFunc("/brc100/identity/selective-disclosure", func(w http.ResponseWriter, r *http.Request) {
		handleSelectiveDisclosure(w, r, brc100Service)
	})

	// Authentication Endpoints
	http.HandleFunc("/brc100/auth/challenge", func(w http.ResponseWriter, r *http.Request) {
		handleAuthChallenge(w, r, brc100Service)
	})

	http.HandleFunc("/brc100/auth/authenticate", func(w http.ResponseWriter, r *http.Request) {
		handleAuthAuthenticate(w, r, brc100Service)
	})

	http.HandleFunc("/brc100/auth/type42", func(w http.ResponseWriter, r *http.Request) {
		handleType42KeyDerivation(w, r, brc100Service)
	})

	// Session Management Endpoints
	http.HandleFunc("/brc100/session/create", func(w http.ResponseWriter, r *http.Request) {
		handleSessionCreate(w, r, brc100Service)
	})

	http.HandleFunc("/brc100/session/validate", func(w http.ResponseWriter, r *http.Request) {
		handleSessionValidate(w, r, brc100Service)
	})

	http.HandleFunc("/brc100/session/revoke", func(w http.ResponseWriter, r *http.Request) {
		handleSessionRevoke(w, r, brc100Service)
	})

	// BEEF Transaction Endpoints
	http.HandleFunc("/brc100/beef/create", func(w http.ResponseWriter, r *http.Request) {
		handleBEEFCreate(w, r, brc100Service)
	})

	http.HandleFunc("/brc100/beef/create-from-tx", func(w http.ResponseWriter, r *http.Request) {
		handleBEEFCreateFromTransaction(w, r, brc100Service)
	})

	http.HandleFunc("/brc100/beef/verify", func(w http.ResponseWriter, r *http.Request) {
		handleBEEFVerify(w, r, brc100Service)
	})

	http.HandleFunc("/brc100/beef/broadcast", func(w http.ResponseWriter, r *http.Request) {
		handleBEEFBroadcast(w, r, brc100Service)
	})

	// SPV Verification Endpoints
	http.HandleFunc("/brc100/spv/verify", func(w http.ResponseWriter, r *http.Request) {
		handleSPVVerify(w, r, brc100Service)
	})

	http.HandleFunc("/brc100/spv/proof", func(w http.ResponseWriter, r *http.Request) {
		handleSPVProof(w, r, brc100Service)
	})

	// Health and Status Endpoints
	http.HandleFunc("/brc100/status", func(w http.ResponseWriter, r *http.Request) {
		handleBRC100Status(w, r, brc100Service)
	})

	// SPV Data Information
	http.HandleFunc("/brc100/spv/info", func(w http.ResponseWriter, r *http.Request) {
		handleSPVInfo(w, r, brc100Service)
	})
}

// Identity Management Handlers

func handleIdentityGenerate(w http.ResponseWriter, r *http.Request, service *BRC100Service) {
	if r.Method != "POST" {
		http.Error(w, "Method not allowed", http.StatusMethodNotAllowed)
		return
	}

	var req IdentityRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		http.Error(w, fmt.Sprintf("Invalid request body: %v", err), http.StatusBadRequest)
		return
	}

	// Create selective disclosure map from attributes
	selectiveDisclosure := make(map[string]bool)
	for key := range req.Attributes {
		selectiveDisclosure[key] = true
	}

	certificate, err := service.identityManager.GenerateIdentityCertificate(req.Subject, selectiveDisclosure)
	if err != nil {
		response := BRC100Response{
			Success: false,
			Error:   fmt.Sprintf("Failed to generate identity certificate: %v", err),
		}
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(response)
		return
	}

	response := BRC100Response{
		Success: true,
		Data: map[string]interface{}{
			"certificate": certificate,
		},
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(response)
}

func handleIdentityValidate(w http.ResponseWriter, r *http.Request, service *BRC100Service) {
	if r.Method != "POST" {
		http.Error(w, "Method not allowed", http.StatusMethodNotAllowed)
		return
	}

	var req struct {
		Certificate *identity.IdentityCertificate `json:"certificate"`
	}
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		http.Error(w, fmt.Sprintf("Invalid request body: %v", err), http.StatusBadRequest)
		return
	}

	valid, err := service.identityManager.ValidateIdentityCertificate(req.Certificate)
	if err != nil {
		response := BRC100Response{
			Success: false,
			Error:   fmt.Sprintf("Failed to validate identity certificate: %v", err),
		}
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(response)
		return
	}

	response := BRC100Response{
		Success: true,
		Data: map[string]interface{}{
			"valid": valid,
		},
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(response)
}

func handleSelectiveDisclosure(w http.ResponseWriter, r *http.Request, service *BRC100Service) {
	if r.Method != "POST" {
		http.Error(w, "Method not allowed", http.StatusMethodNotAllowed)
		return
	}

	var req struct {
		IdentityData map[string]interface{} `json:"identityData"`
		Fields       []string               `json:"fields"`
	}
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		http.Error(w, fmt.Sprintf("Invalid request body: %v", err), http.StatusBadRequest)
		return
	}

	disclosure := service.identityManager.CreateSelectiveDisclosure(req.IdentityData, req.Fields)

	response := BRC100Response{
		Success: true,
		Data: map[string]interface{}{
			"disclosure": disclosure,
		},
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(response)
}

// Authentication Handlers

func handleAuthChallenge(w http.ResponseWriter, r *http.Request, service *BRC100Service) {
	// Enable CORS
	w.Header().Set("Access-Control-Allow-Origin", "*")
	w.Header().Set("Access-Control-Allow-Methods", "GET, POST, PUT, DELETE, OPTIONS")
	w.Header().Set("Access-Control-Allow-Headers", "Content-Type, Authorization")

	if r.Method == "OPTIONS" {
		w.WriteHeader(http.StatusOK)
		return
	}

	if r.Method != "POST" {
		http.Error(w, "Method not allowed", http.StatusMethodNotAllowed)
		return
	}

	var req struct {
		AppID string `json:"appId"`
	}
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		http.Error(w, fmt.Sprintf("Invalid request body: %v", err), http.StatusBadRequest)
		return
	}

	challenge, err := service.challengeManager.CreateChallenge(req.AppID)
	if err != nil {
		response := BRC100Response{
			Success: false,
			Error:   fmt.Sprintf("Failed to generate challenge: %v", err),
		}
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(response)
		return
	}

	// Save challenge to wallet's BRC-100 data
	brc100Data, err := service.walletManager.GetBRC100Data()
	if err != nil {
		response := BRC100Response{
			Success: false,
			Error:   fmt.Sprintf("Failed to get BRC-100 data: %v", err),
		}
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(response)
		return
	}

	// Add challenge to BRC-100 data
	brc100Challenge := BRC100Challenge{
		ChallengeID: challenge.ChallengeID,
		Challenge:   challenge.Challenge,
		AppDomain:   challenge.AppDomain,
		CreatedAt:   challenge.CreatedAt,
		ExpiresAt:   challenge.ExpiresAt,
		Solved:      challenge.Solved,
	}
	brc100Data.Challenges = append(brc100Data.Challenges, brc100Challenge)

	// Challenge added to BRC-100 data

	// Update the wallet's BRC-100 data
	service.walletManager.wallet.BRC100 = brc100Data

	// Save updated BRC-100 data
	if err := service.walletManager.SaveBRC100Data(); err != nil {
		response := BRC100Response{
			Success: false,
			Error:   fmt.Sprintf("Failed to save BRC-100 data: %v", err),
		}
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(response)
		return
	}

	response := BRC100Response{
		Success: true,
		Data: map[string]interface{}{
			"challenge": challenge,
		},
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(response)
}

func handleAuthAuthenticate(w http.ResponseWriter, r *http.Request, service *BRC100Service) {
	if r.Method != "POST" {
		http.Error(w, "Method not allowed", http.StatusMethodNotAllowed)
		return
	}

	var req AuthenticationRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		http.Error(w, fmt.Sprintf("Invalid request body: %v", err), http.StatusBadRequest)
		return
	}

	// Note: In a real implementation, we would verify the signature here
	// For now, we're just checking if the challenge exists and is valid

	// Get BRC-100 data to find the challenge
	brc100Data, err := service.walletManager.GetBRC100Data()
	if err != nil {
		response := BRC100Response{
			Success: false,
			Error:   fmt.Sprintf("Failed to get BRC-100 data: %v", err),
		}
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(response)
		return
	}

	// Looking for challenge in stored challenges

	// Find the challenge in the stored challenges
	var foundChallenge *BRC100Challenge
	for i, challenge := range brc100Data.Challenges {
		if challenge.Challenge == req.Challenge {
			foundChallenge = &brc100Data.Challenges[i]
			break
		}
	}

	if foundChallenge == nil {
		response := BRC100Response{
			Success: false,
			Error:   fmt.Sprintf("Failed to verify challenge: challenge not found: %s", req.Challenge),
		}
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(response)
		return
	}

	// Check if challenge is expired
	if time.Now().After(foundChallenge.ExpiresAt) {
		response := BRC100Response{
			Success: false,
			Error:   "Failed to verify challenge: challenge expired",
		}
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(response)
		return
	}

	// Mark challenge as solved
	foundChallenge.Solved = true

	// Update the wallet's BRC-100 data
	service.walletManager.wallet.BRC100 = brc100Data

	if err := service.walletManager.SaveBRC100Data(); err != nil {
		response := BRC100Response{
			Success: false,
			Error:   fmt.Sprintf("Failed to save BRC-100 data: %v", err),
		}
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(response)
		return
	}

	valid := true // Challenge found and not expired

	response := BRC100Response{
		Success: true,
		Data: map[string]interface{}{
			"authenticated": valid,
		},
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(response)
}

func handleType42KeyDerivation(w http.ResponseWriter, r *http.Request, service *BRC100Service) {
	if r.Method != "POST" {
		http.Error(w, "Method not allowed", http.StatusMethodNotAllowed)
		return
	}

	var req struct {
		WalletPublicKey string `json:"walletPublicKey"`
		AppPublicKey    string `json:"appPublicKey"`
	}
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		http.Error(w, fmt.Sprintf("Invalid request body: %v", err), http.StatusBadRequest)
		return
	}

	// Convert string keys to byte arrays (simplified for demo)
	walletKey := []byte(req.WalletPublicKey)
	appKey := []byte(req.AppPublicKey)

	keys, err := service.type42Manager.DeriveType42Keys(walletKey, appKey)
	if err != nil {
		response := BRC100Response{
			Success: false,
			Error:   fmt.Sprintf("Failed to derive Type-42 keys: %v", err),
		}
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(response)
		return
	}

	response := BRC100Response{
		Success: true,
		Data: map[string]interface{}{
			"keys": keys,
		},
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(response)
}

// Session Management Handlers

func handleSessionCreate(w http.ResponseWriter, r *http.Request, service *BRC100Service) {
	if r.Method != "POST" {
		http.Error(w, "Method not allowed", http.StatusMethodNotAllowed)
		return
	}

	var req SessionRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		http.Error(w, fmt.Sprintf("Invalid request body: %v", err), http.StatusBadRequest)
		return
	}

	// Create a simple identity certificate for the session
	identityCert := map[string]interface{}{
		"subject": req.IdentityID,
		"issuer":  "Babbage-Browser-Wallet",
	}

	session, err := service.sessionManager.CreateSession(req.AppID, identityCert, []string{"read", "write"})
	if err != nil {
		response := BRC100Response{
			Success: false,
			Error:   fmt.Sprintf("Failed to create session: %v", err),
		}
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(response)
		return
	}

	response := BRC100Response{
		Success: true,
		Data: map[string]interface{}{
			"session": session,
		},
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(response)
}

func handleSessionValidate(w http.ResponseWriter, r *http.Request, service *BRC100Service) {
	if r.Method != "POST" {
		http.Error(w, "Method not allowed", http.StatusMethodNotAllowed)
		return
	}

	var req struct {
		SessionID string `json:"sessionId"`
	}
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		http.Error(w, fmt.Sprintf("Invalid request body: %v", err), http.StatusBadRequest)
		return
	}

	session, err := service.sessionManager.GetSession(req.SessionID)
	valid := err == nil
	if err != nil {
		response := BRC100Response{
			Success: false,
			Error:   fmt.Sprintf("Failed to validate session: %v", err),
		}
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(response)
		return
	}

	response := BRC100Response{
		Success: true,
		Data: map[string]interface{}{
			"valid":   valid,
			"session": session,
		},
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(response)
}

func handleSessionRevoke(w http.ResponseWriter, r *http.Request, service *BRC100Service) {
	if r.Method != "POST" {
		http.Error(w, "Method not allowed", http.StatusMethodNotAllowed)
		return
	}

	var req struct {
		SessionID string `json:"sessionId"`
	}
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		http.Error(w, fmt.Sprintf("Invalid request body: %v", err), http.StatusBadRequest)
		return
	}

	err := service.sessionManager.DeleteSession(req.SessionID)
	if err != nil {
		response := BRC100Response{
			Success: false,
			Error:   fmt.Sprintf("Failed to revoke session: %v", err),
		}
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(response)
		return
	}

	response := BRC100Response{
		Success: true,
		Data: map[string]interface{}{
			"revoked": true,
		},
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(response)
}

// BEEF Transaction Handlers

func handleBEEFCreate(w http.ResponseWriter, r *http.Request, service *BRC100Service) {
	// Enable CORS
	w.Header().Set("Access-Control-Allow-Origin", "*")
	w.Header().Set("Access-Control-Allow-Methods", "GET, POST, PUT, DELETE, OPTIONS")
	w.Header().Set("Access-Control-Allow-Headers", "Content-Type, Authorization")

	if r.Method == "OPTIONS" {
		w.WriteHeader(http.StatusOK)
		return
	}

	if r.Method != "POST" {
		http.Error(w, "Method not allowed", http.StatusMethodNotAllowed)
		return
	}

	var req BEEFRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		http.Error(w, fmt.Sprintf("Invalid request body: %v", err), http.StatusBadRequest)
		return
	}

	// Create a simple identity context for the BEEF transaction
	// Get current wallet address for identity context
	walletAddress, err := service.getCurrentWalletAddress()
	if err != nil {
		response := BRC100Response{
			Success: false,
			Error:   fmt.Sprintf("Failed to get wallet address: %v", err),
		}
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(response)
		return
	}

	// Extract app domain from request or use a default
	appDomain := "babbage-browser.app" // Default domain
	if req.AppDomain != "" {
		appDomain = req.AppDomain
	}

	identityContext := &beef.IdentityContext{
		Certificate: map[string]interface{}{
			"subject": walletAddress,
			"issuer":  "Babbage-Browser-Wallet",
		},
		SessionID:  req.SessionID,
		AppDomain:  appDomain,
		Timestamp:  time.Now(),
	}

	// Create BEEF transaction with optional SPV data
	beefTx, err := service.beefManager.CreateBRC100BEEFTransactionWithSPV(req.Actions, identityContext, req.IncludeSPVData)
	if err != nil {
		response := BRC100Response{
			Success: false,
			Error:   fmt.Sprintf("Failed to create BRC-100 BEEF: %v", err),
		}
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(response)
		return
	}

	response := BRC100Response{
		Success: true,
		Data: map[string]interface{}{
			"beefTransaction": beefTx,
		},
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(response)
}

func handleBEEFVerify(w http.ResponseWriter, r *http.Request, service *BRC100Service) {
	if r.Method != "POST" {
		http.Error(w, "Method not allowed", http.StatusMethodNotAllowed)
		return
	}

	var req struct {
		BEEFTransaction *beef.BRC100BEEFTransaction `json:"beefTransaction"`
	}
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		http.Error(w, fmt.Sprintf("Invalid request body: %v", err), http.StatusBadRequest)
		return
	}

	valid, err := service.beefManager.VerifyBRC100BEEFTransaction(req.BEEFTransaction)
	if err != nil {
		response := BRC100Response{
			Success: false,
			Error:   fmt.Sprintf("Failed to verify BRC-100 BEEF: %v", err),
		}
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(response)
		return
	}

	response := BRC100Response{
		Success: true,
		Data: map[string]interface{}{
			"valid": valid,
		},
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(response)
}

func handleBEEFBroadcast(w http.ResponseWriter, r *http.Request, service *BRC100Service) {
	if r.Method != "POST" {
		http.Error(w, "Method not allowed", http.StatusMethodNotAllowed)
		return
	}

	var req struct {
		BEEFTransaction *beef.BRC100BEEFTransaction `json:"beefTransaction"`
	}
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		http.Error(w, fmt.Sprintf("Invalid request body: %v", err), http.StatusBadRequest)
		return
	}

	// Convert BRC-100 BEEF to standard BEEF for broadcasting
	standardBEEF, err := service.beefManager.ConvertToBEEF(req.BEEFTransaction)
	if err != nil {
		response := BRC100Response{
			Success: false,
			Error:   fmt.Sprintf("Failed to convert to standard BEEF: %v", err),
		}
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(response)
		return
	}

	response := BRC100Response{
		Success: true,
		Data: map[string]interface{}{
			"standardBEEF": standardBEEF,
			"message":      "BEEF converted to standard format - ready for broadcasting",
		},
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(response)
}

// SPV Verification Handlers

func handleSPVVerify(w http.ResponseWriter, r *http.Request, service *BRC100Service) {
	if r.Method != "POST" {
		http.Error(w, "Method not allowed", http.StatusMethodNotAllowed)
		return
	}

	var req SPVRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		http.Error(w, fmt.Sprintf("Invalid request body: %v", err), http.StatusBadRequest)
		return
	}

	proof, err := service.spvManager.CreateIdentityProof(req.TransactionID, req.IdentityData)
	if err != nil {
		response := BRC100Response{
			Success: false,
			Error:   fmt.Sprintf("Failed to create identity proof: %v", err),
		}
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(response)
		return
	}

	result, err := service.spvManager.VerifyIdentityProof(proof)
	if err != nil {
		response := BRC100Response{
			Success: false,
			Error:   fmt.Sprintf("Failed to verify identity proof: %v", err),
		}
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(response)
		return
	}

	response := BRC100Response{
		Success: true,
		Data: map[string]interface{}{
			"proof":   proof,
			"result":  result,
			"valid":   result.Valid,
		},
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(response)
}

func handleSPVProof(w http.ResponseWriter, r *http.Request, service *BRC100Service) {
	if r.Method != "POST" {
		http.Error(w, "Method not allowed", http.StatusMethodNotAllowed)
		return
	}

	var req SPVRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		http.Error(w, fmt.Sprintf("Invalid request body: %v", err), http.StatusBadRequest)
		return
	}

	proof, err := service.spvManager.CreateIdentityProof(req.TransactionID, req.IdentityData)
	if err != nil {
		response := BRC100Response{
			Success: false,
			Error:   fmt.Sprintf("Failed to create identity proof: %v", err),
		}
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(response)
		return
	}

	response := BRC100Response{
		Success: true,
		Data: map[string]interface{}{
			"proof": proof,
		},
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(response)
}

// Status Handler

func handleBRC100Status(w http.ResponseWriter, r *http.Request, service *BRC100Service) {
	// Enable CORS
	w.Header().Set("Access-Control-Allow-Origin", "*")
	w.Header().Set("Access-Control-Allow-Methods", "GET, POST, PUT, DELETE, OPTIONS")
	w.Header().Set("Access-Control-Allow-Headers", "Content-Type, Authorization")

	if r.Method == "OPTIONS" {
		w.WriteHeader(http.StatusOK)
		return
	}

	if r.Method != "GET" {
		http.Error(w, "Method not allowed", http.StatusMethodNotAllowed)
		return
	}

	status := map[string]interface{}{
		"service":      "BRC-100",
		"version":      "1.0.0",
		"status":       "operational",
		"timestamp":    time.Now(),
		"components": map[string]interface{}{
			"identity":    "operational",
			"authentication": "operational",
			"session":     "operational",
			"beef":        "operational",
			"spv":         "operational",
		},
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(status)
}

// handleSPVInfo handles requests for SPV data information
func handleSPVInfo(w http.ResponseWriter, r *http.Request, service *BRC100Service) {
	if r.Method != "POST" {
		http.Error(w, "Method not allowed", http.StatusMethodNotAllowed)
		return
	}

	var req struct {
		BEEFTransaction *beef.BRC100BEEFTransaction `json:"beefTransaction"`
	}
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		http.Error(w, fmt.Sprintf("Invalid request body: %v", err), http.StatusBadRequest)
		return
	}

	if req.BEEFTransaction == nil {
		response := BRC100Response{
			Success: false,
			Error:   "BEEF transaction is required",
		}
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(response)
		return
	}

	// Get SPV data information
	spvInfo := service.beefManager.GetSPVDataInfo(req.BEEFTransaction.SPVData)

	response := BRC100Response{
		Success: true,
		Data: map[string]interface{}{
			"spvInfo": spvInfo,
		},
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(response)
}

// handleBEEFCreateFromTransaction handles BEEF creation with SPV data from a specific transaction
func handleBEEFCreateFromTransaction(w http.ResponseWriter, r *http.Request, service *BRC100Service) {
	if r.Method != "POST" {
		http.Error(w, "Method not allowed", http.StatusMethodNotAllowed)
		return
	}

	var req struct {
		Actions   []map[string]interface{} `json:"actions"`
		SessionID string                   `json:"sessionId"`
		AppDomain string                   `json:"appDomain"`
		TxID      string                   `json:"txId"` // Transaction ID to collect SPV data from
	}

	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		http.Error(w, fmt.Sprintf("Invalid request body: %v", err), http.StatusBadRequest)
		return
	}

	// Convert actions to BRC100Action structs
	actions := make([]beef.BRC100Action, len(req.Actions))
	for i, actionData := range req.Actions {
		actions[i] = beef.BRC100Action{
			Type:      actionData["type"].(string),
			Data:      actionData["data"].(map[string]interface{}),
			Timestamp: time.Now(),
		}
	}

	// Create identity context
	identity := &beef.IdentityContext{
		SessionID: req.SessionID,
		AppDomain: req.AppDomain,
		Timestamp: time.Now(),
	}

	// Create BEEF transaction with SPV data from the specific transaction
	beefTx, err := service.beefManager.CreateBRC100BEEFTransactionWithSPVFromTransaction(actions, identity, req.TxID)
	if err != nil {
		http.Error(w, fmt.Sprintf("Failed to create BEEF transaction: %v", err), http.StatusInternalServerError)
		return
	}

	response := map[string]interface{}{
		"success": true,
		"data": map[string]interface{}{
			"beefTransaction": beefTx,
		},
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(response)
}
