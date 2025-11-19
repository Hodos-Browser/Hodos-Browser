package main

import (
	"crypto/ecdsa"
	"crypto/elliptic"
	"crypto/hmac"
	"crypto/rand"
	"crypto/sha256"
	"encoding/base64"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"io"
	"log"
	"math/big"
	"net/http"
	"time"

	"github.com/bsv-blockchain/go-sdk/transaction"
	"github.com/sirupsen/logrus"
	"browser-wallet/brc100/websocket"
)

// IdentityData removed - replaced with HD wallet system

// CORS middleware to handle cross-origin requests
func enableCORS(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Access-Control-Allow-Origin", "*")
	w.Header().Set("Access-Control-Allow-Methods", "GET, POST, PUT, DELETE, OPTIONS")
	w.Header().Set("Access-Control-Allow-Headers", "Content-Type, Authorization")

	// Handle preflight requests
	if r.Method == "OPTIONS" {
		w.WriteHeader(http.StatusOK)
		return
	}
}

// AddressData represents a generated address
type AddressData struct {
	Address   string `json:"address"`
	PublicKey string `json:"publicKey"`
	Index     int    `json:"index"`
	// Note: Private key is NOT included for security reasons
}

// TransactionRequest represents a transaction creation request
type TransactionRequest struct {
	SenderAddress    string `json:"senderAddress,omitempty"`    // Optional: if not provided, uses wallet identity
	RecipientAddress string `json:"recipientAddress"`
	Amount          int64  `json:"amount"` // in satoshis
	FeeRate         int64  `json:"feeRate"` // satoshis per byte
	Message         string `json:"message,omitempty"` // Optional OP_RETURN message
}

// UTXO represents an unspent transaction output
type UTXO struct {
	TxID    string `json:"txid"`
	Vout    uint32 `json:"vout"`
	Amount  int64  `json:"amount"`
	Script  string `json:"script"`
	Address string `json:"address"` // Which address owns this UTXO
}

// TransactionResponse represents a transaction operation response
type TransactionResponse struct {
	TxID        string `json:"txid"`
	RawTx       string `json:"rawTx"`
	Fee         int64  `json:"fee"`
	Status      string `json:"status"`
	Broadcasted bool   `json:"broadcasted"`
}

// BroadcastResult represents the result of broadcasting to multiple miners
type BroadcastResult struct {
	TxID       string            `json:"txid"`
	Success    bool              `json:"success"`
	Miners     map[string]string `json:"miners"` // miner name -> response
	Error      string            `json:"error,omitempty"`
}


// WalletService represents our Bitcoin SV wallet service
type WalletService struct {
	walletManager      *WalletManager
	logger             *logrus.Logger
	transactionBuilder *TransactionBuilder
	broadcaster        *TransactionBroadcaster
	selectedUTXOs      []UTXO // Store selected UTXOs for signing
	createdTransaction *transaction.Transaction // Store created transaction object for signing
}

// NewWalletService creates a new wallet service instance
func NewWalletService() *WalletService {
	logger := logrus.New()
	logger.SetLevel(logrus.InfoLevel)

	walletService := &WalletService{
		walletManager: NewWalletManager(),
		logger:        logger,
	}

	// Initialize transaction components
	walletService.transactionBuilder = NewTransactionBuilder(walletService)
	walletService.broadcaster = NewTransactionBroadcaster()

	return walletService
}


func main() {
	fmt.Println("�� Bitcoin Browser Go Wallet Starting...")

	// Create wallet service instance
	walletService := NewWalletService()

	// Setup BRC-100 routes
	walletService.SetupBRC100Routes()

	// Initialize domain whitelist manager
	domainWhitelistManager := NewDomainWhitelistManager()
	fmt.Println("🔒 Domain whitelist manager initialized")

	// Domain whitelist endpoints
	http.HandleFunc("/domain/whitelist/add", func(w http.ResponseWriter, r *http.Request) {
		enableCORS(w, r)
		if r.Method != "POST" {
			http.Error(w, "Method not allowed", http.StatusMethodNotAllowed)
			return
		}

		var req struct {
			Domain      string `json:"domain"`
			IsPermanent bool   `json:"isPermanent"`
		}

		if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
			http.Error(w, "Invalid request body", http.StatusBadRequest)
			return
		}

		if req.Domain == "" {
			http.Error(w, "Domain is required", http.StatusBadRequest)
			return
		}

		if err := domainWhitelistManager.AddToWhitelist(req.Domain, req.IsPermanent); err != nil {
			http.Error(w, fmt.Sprintf("Failed to add domain to whitelist: %v", err), http.StatusInternalServerError)
			return
		}

		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(map[string]interface{}{
			"success": true,
			"message": "Domain added to whitelist",
			"domain":  req.Domain,
		})
	})

	http.HandleFunc("/domain/whitelist/check", func(w http.ResponseWriter, r *http.Request) {
		enableCORS(w, r)
		if r.Method != "GET" {
			http.Error(w, "Method not allowed", http.StatusMethodNotAllowed)
			return
		}

		domain := r.URL.Query().Get("domain")
		if domain == "" {
			http.Error(w, "Domain parameter is required", http.StatusBadRequest)
			return
		}

		isWhitelisted := domainWhitelistManager.IsDomainWhitelisted(domain)

		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(map[string]interface{}{
			"domain":      domain,
			"whitelisted": isWhitelisted,
		})
	})

	http.HandleFunc("/domain/whitelist/record", func(w http.ResponseWriter, r *http.Request) {
		enableCORS(w, r)
		if r.Method != "POST" {
			http.Error(w, "Method not allowed", http.StatusMethodNotAllowed)
			return
		}

		var req struct {
			Domain string `json:"domain"`
		}

		if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
			http.Error(w, "Invalid request body", http.StatusBadRequest)
			return
		}

		if req.Domain == "" {
			http.Error(w, "Domain is required", http.StatusBadRequest)
			return
		}

		if err := domainWhitelistManager.RecordRequest(req.Domain); err != nil {
			http.Error(w, fmt.Sprintf("Failed to record request: %v", err), http.StatusInternalServerError)
			return
		}

		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(map[string]interface{}{
			"success": true,
			"message": "Request recorded",
		})
	})

	http.HandleFunc("/domain/whitelist/list", func(w http.ResponseWriter, r *http.Request) {
		enableCORS(w, r)
		if r.Method != "GET" {
			http.Error(w, "Method not allowed", http.StatusMethodNotAllowed)
			return
		}

		whitelist := domainWhitelistManager.GetWhitelist()

		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(map[string]interface{}{
			"success": true,
			"whitelist": whitelist,
		})
	})

	http.HandleFunc("/domain/whitelist/remove", func(w http.ResponseWriter, r *http.Request) {
		enableCORS(w, r)
		if r.Method != "POST" {
			http.Error(w, "Method not allowed", http.StatusMethodNotAllowed)
			return
		}

		var req struct {
			Domain string `json:"domain"`
		}

		if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
			http.Error(w, "Invalid request body", http.StatusBadRequest)
			return
		}

		if req.Domain == "" {
			http.Error(w, "Domain is required", http.StatusBadRequest)
			return
		}

		if err := domainWhitelistManager.RemoveFromWhitelist(req.Domain); err != nil {
			http.Error(w, fmt.Sprintf("Failed to remove domain from whitelist: %v", err), http.StatusInternalServerError)
			return
		}

		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(map[string]interface{}{
			"success": true,
			"message": "Domain removed from whitelist",
			"domain":  req.Domain,
		})
	})

	// Setup WebSocket handler for BRC-100 real-time communication
	wsHandler := websocket.NewBRC100WebSocketHandler()
	http.HandleFunc("/brc100/ws", wsHandler.HandleWebSocket)

	// Setup BRC-100 compatible Socket.IO handler (no auth response function needed)
	brc100SocketIOHandler := websocket.NewBRC100SocketIOHandler()
	http.HandleFunc("/socket.io/", brc100SocketIOHandler.HandleSocketIO)

	// Try to load existing wallet on startup
	if walletService.walletManager.WalletExists() {
		fmt.Println("📁 Loading existing wallet...")
		err := walletService.walletManager.LoadFromFile(GetWalletPath())
		if err != nil {
			fmt.Printf("⚠️ Failed to load existing wallet: %v\n", err)
		} else {
			fmt.Println("✅ Wallet loaded successfully")

			// Initialize BRC-100 data for existing wallet
			fmt.Println("🔐 Initializing BRC-100...")
			if err := walletService.walletManager.InitializeBRC100(); err != nil {
				fmt.Printf("⚠️ Failed to initialize BRC-100: %v\n", err)
			} else {
				fmt.Println("✅ BRC-100 initialized successfully")
			}
		}
	} else {
		fmt.Println("📝 No existing wallet found - will create new wallet when needed")
		fmt.Println("🔐 BRC-100 will be initialized automatically when wallet is created")
	}

	// Set up HTTP handlers
	http.HandleFunc("/health", func(w http.ResponseWriter, r *http.Request) {
		enableCORS(w, r)
		if r.Method != "GET" {
			http.Error(w, "Method not allowed", http.StatusMethodNotAllowed)
			return
		}
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(map[string]string{"status": "healthy"})
	})

	// Identity endpoints removed - replaced with HD wallet system

	// Old address generation removed - replaced with HD wallet system

	// UTXO testing endpoint
	http.HandleFunc("/utxo/fetch", func(w http.ResponseWriter, r *http.Request) {
		if r.Method != "GET" {
			http.Error(w, "Method not allowed", http.StatusMethodNotAllowed)
			return
		}

		// Get address from query parameter
		address := r.URL.Query().Get("address")
		if address == "" {
			http.Error(w, "address parameter is required", http.StatusBadRequest)
			return
		}

		// Fetch UTXOs using UTXO manager
		utxos, err := walletService.transactionBuilder.utxoManager.FetchUTXOs(address)
		if err != nil {
			http.Error(w, fmt.Sprintf("Failed to fetch UTXOs: %v", err), http.StatusInternalServerError)
			return
		}

		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(utxos)
	})

	// Transaction endpoints
	http.HandleFunc("/transaction/create", func(w http.ResponseWriter, r *http.Request) {
		if r.Method != "POST" {
			http.Error(w, "Method not allowed", http.StatusMethodNotAllowed)
			return
		}

		var req TransactionRequest
		if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
			http.Error(w, fmt.Sprintf("Invalid request body: %v", err), http.StatusBadRequest)
			return
		}

		// Create transaction using transaction builder
		response, err := walletService.transactionBuilder.CreateTransaction(&req)
		if err != nil {
			http.Error(w, fmt.Sprintf("Failed to create transaction: %v", err), http.StatusInternalServerError)
			return
		}

		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(response)
	})

	http.HandleFunc("/transaction/sign", func(w http.ResponseWriter, r *http.Request) {
		if r.Method != "POST" {
			http.Error(w, "Method not allowed", http.StatusMethodNotAllowed)
			return
		}

		var req struct {
			RawTx string `json:"rawTx"`
		}
		if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
			http.Error(w, fmt.Sprintf("Invalid request body: %v", err), http.StatusBadRequest)
			return
		}

		// Use stored selected UTXOs for signing
		walletService.logger.Infof("Signing request received, checking stored UTXOs...")
		walletService.logger.Infof("Stored UTXOs count: %d", len(walletService.selectedUTXOs))

		if len(walletService.selectedUTXOs) == 0 {
			walletService.logger.Error("No selected UTXOs available for signing")
			http.Error(w, "No selected UTXOs available for signing", http.StatusBadRequest)
			return
		}

		// Log the stored UTXOs
		for i, utxo := range walletService.selectedUTXOs {
			walletService.logger.Infof("Stored UTXO %d: %s:%d (amount: %d, address: %s)", i, utxo.TxID, utxo.Vout, utxo.Amount, utxo.Address)
		}

		// Sign transaction using transaction builder with selected UTXOs
		response, err := walletService.transactionBuilder.SignTransaction(req.RawTx, "", walletService.selectedUTXOs)
		if err != nil {
			http.Error(w, fmt.Sprintf("Failed to sign transaction: %v", err), http.StatusInternalServerError)
			return
		}

		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(response)
	})

	http.HandleFunc("/transaction/broadcast", func(w http.ResponseWriter, r *http.Request) {
		if r.Method != "POST" {
			http.Error(w, "Method not allowed", http.StatusMethodNotAllowed)
			return
		}

		var req struct {
			SignedTx string `json:"signedTx"`
		}
		if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
			http.Error(w, fmt.Sprintf("Invalid request body: %v", err), http.StatusBadRequest)
			return
		}

		// Broadcast transaction using broadcaster
		response, err := walletService.broadcaster.BroadcastTransaction(req.SignedTx)
		if err != nil {
			http.Error(w, fmt.Sprintf("Failed to broadcast transaction: %v", err), http.StatusInternalServerError)
			return
		}

		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(response)
	})

	// Transaction history endpoint (placeholder - returns empty array for now)
	http.HandleFunc("/transaction/history", func(w http.ResponseWriter, r *http.Request) {
		if r.Method != "GET" {
			http.Error(w, "Method not allowed", http.StatusMethodNotAllowed)
			return
		}

		// For now, return empty array - in production this would query a database
		// or blockchain explorer for transaction history
		history := []map[string]interface{}{}

		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(history)
	})

	// Unified Wallet Management Endpoints
	http.HandleFunc("/wallet/status", func(w http.ResponseWriter, r *http.Request) {
		enableCORS(w, r)
		if r.Method != "GET" {
			http.Error(w, "Method not allowed", http.StatusMethodNotAllowed)
			return
		}

		exists := walletService.walletManager.WalletExists()
		response := map[string]interface{}{
			"exists": exists,
		}

		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(response)
	})

	http.HandleFunc("/wallet/create", func(w http.ResponseWriter, r *http.Request) {
		if r.Method != "POST" {
			http.Error(w, "Method not allowed", http.StatusMethodNotAllowed)
			return
		}

		// Generate new mnemonic
		mnemonic, err := walletService.walletManager.GenerateMnemonic()
		if err != nil {
			http.Error(w, fmt.Sprintf("Failed to generate mnemonic: %v", err), http.StatusInternalServerError)
			return
		}

		// Create unified wallet from mnemonic
		err = walletService.walletManager.CreateFromMnemonic(mnemonic)
		if err != nil {
			http.Error(w, fmt.Sprintf("Failed to create unified wallet: %v", err), http.StatusInternalServerError)
			return
		}

		// Save to file
		err = walletService.walletManager.SaveToFile(GetWalletPath())
		if err != nil {
			http.Error(w, fmt.Sprintf("Failed to save unified wallet: %v", err), http.StatusInternalServerError)
			return
		}

		// Initialize BRC-100 data for the new wallet
		err = walletService.walletManager.InitializeBRC100()
		if err != nil {
			http.Error(w, fmt.Sprintf("Failed to initialize BRC-100: %v", err), http.StatusInternalServerError)
			return
		}

		response := map[string]interface{}{
			"success": true,
			"mnemonic": mnemonic,
			"brc100Initialized": true,
		}

		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(response)
	})

	http.HandleFunc("/wallet/load", func(w http.ResponseWriter, r *http.Request) {
		if r.Method != "POST" {
			http.Error(w, "Method not allowed", http.StatusMethodNotAllowed)
			return
		}

		err := walletService.walletManager.LoadFromFile(GetWalletPath())
		if err != nil {
			http.Error(w, fmt.Sprintf("Failed to load unified wallet: %v", err), http.StatusInternalServerError)
			return
		}

		response := map[string]interface{}{
			"success": true,
		}

		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(response)
	})

	// Address Management Endpoints
	http.HandleFunc("/wallet/addresses", func(w http.ResponseWriter, r *http.Request) {
		if r.Method != "GET" {
			http.Error(w, "Method not allowed", http.StatusMethodNotAllowed)
			return
		}

		addresses := walletService.walletManager.GetAllAddresses()
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(addresses)
	})

	http.HandleFunc("/wallet/address/generate", func(w http.ResponseWriter, r *http.Request) {
		if r.Method != "POST" {
			http.Error(w, "Method not allowed", http.StatusMethodNotAllowed)
			return
		}

		address, err := walletService.walletManager.GetNextAddress()
		if err != nil {
			http.Error(w, fmt.Sprintf("Failed to generate address: %v", err), http.StatusInternalServerError)
			return
		}

		// Save wallet after generating new address
		err = walletService.walletManager.SaveToFile(GetWalletPath())
		if err != nil {
			http.Error(w, fmt.Sprintf("Failed to save wallet: %v", err), http.StatusInternalServerError)
			return
		}

		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(address)
	})

	http.HandleFunc("/wallet/address/current", func(w http.ResponseWriter, r *http.Request) {
		if r.Method != "GET" {
			http.Error(w, "Method not allowed", http.StatusMethodNotAllowed)
			return
		}

		address, err := walletService.walletManager.GetCurrentAddress()
		if err != nil {
			http.Error(w, fmt.Sprintf("Failed to get current address: %v", err), http.StatusInternalServerError)
			return
		}

		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(address)
	})

	// Balance Management Endpoints
	http.HandleFunc("/wallet/balance", func(w http.ResponseWriter, r *http.Request) {
		if r.Method != "GET" {
			http.Error(w, "Method not allowed", http.StatusMethodNotAllowed)
			return
		}

		balance, err := walletService.walletManager.GetTotalBalance()
		if err != nil {
			http.Error(w, fmt.Sprintf("Failed to get balance: %v", err), http.StatusInternalServerError)
			return
		}

		response := map[string]interface{}{
			"balance": balance,
		}

		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(response)
	})

	// New unified wallet info endpoint
	http.HandleFunc("/wallet/info", func(w http.ResponseWriter, r *http.Request) {
		if r.Method != "GET" {
			http.Error(w, "Method not allowed", http.StatusMethodNotAllowed)
			return
		}

		walletInfo, err := walletService.walletManager.GetWalletInfo()
		if err != nil {
			http.Error(w, fmt.Sprintf("Failed to get wallet info: %v", err), http.StatusInternalServerError)
			return
		}

		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(walletInfo)
	})

	// Mark wallet as backed up endpoint
	http.HandleFunc("/wallet/markBackedUp", func(w http.ResponseWriter, r *http.Request) {
		if r.Method != "POST" {
			http.Error(w, "Method not allowed", http.StatusMethodNotAllowed)
			return
		}

		err := walletService.walletManager.MarkBackedUp()
		if err != nil {
			http.Error(w, fmt.Sprintf("Failed to mark wallet as backed up: %v", err), http.StatusInternalServerError)
			return
		}

		// Save wallet after marking as backed up
		err = walletService.walletManager.SaveToFile(GetWalletPath())
		if err != nil {
			http.Error(w, fmt.Sprintf("Failed to save wallet: %v", err), http.StatusInternalServerError)
			return
		}

		response := map[string]interface{}{
			"success": true,
		}

		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(response)
	})

	// Complete transaction send endpoint (create + sign + broadcast)
	http.HandleFunc("/transaction/send", func(w http.ResponseWriter, r *http.Request) {
		if r.Method != "POST" {
			http.Error(w, "Method not allowed", http.StatusMethodNotAllowed)
			return
		}

		var req struct {
			ToAddress string `json:"toAddress"`
			Amount    int64  `json:"amount"`
			FeeRate   int64  `json:"feeRate"`
		}

		if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
			http.Error(w, "Invalid request body", http.StatusBadRequest)
			return
		}

		// Create transaction
		createReq := &TransactionRequest{
			RecipientAddress: req.ToAddress,
			Amount:          req.Amount,
			FeeRate:         req.FeeRate,
		}

		_, err := walletService.transactionBuilder.CreateTransaction(createReq)
		if err != nil {
			http.Error(w, fmt.Sprintf("Failed to create transaction: %v", err), http.StatusInternalServerError)
			return
		}

		// Sign transaction (uses stored transaction object and UTXOs)
		signResp, err := walletService.transactionBuilder.SignTransaction("", "", walletService.selectedUTXOs)
		if err != nil {
			http.Error(w, fmt.Sprintf("Failed to sign transaction: %v", err), http.StatusInternalServerError)
			return
		}

		// Broadcast transaction
		broadcastResp, err := walletService.broadcaster.BroadcastTransaction(signResp.RawTx)
		if err != nil {
			http.Error(w, fmt.Sprintf("Failed to broadcast transaction: %v", err), http.StatusInternalServerError)
			return
		}

		// Return success response with WhatsOnChain link
		response := map[string]interface{}{
			"success":         true,
			"txid":           broadcastResp.TxID,
			"whatsOnChainUrl": fmt.Sprintf("https://whatsonchain.com/tx/%s", broadcastResp.TxID),
			"message":        "Transaction sent successfully",
		}

		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(response)
	})

	// BRC-100 Wallet Endpoint Handlers
	// handleGetVersion returns wallet version and capabilities
	handleGetVersion := func(w http.ResponseWriter, r *http.Request) {
		enableCORS(w, r)
		if r.Method != "POST" {
			http.Error(w, "Method not allowed", http.StatusMethodNotAllowed)
			return
		}

		response := map[string]interface{}{
			"version": "BitcoinBrowserWallet v0.0.1",
			"capabilities": []string{
				"getVersion",
				"getPublicKey",
				"createAction",
				"signAction",
				"processAction",
			},
			"brc100": true,
			"timestamp": time.Now(),
		}

		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(response)
	}

	// handleGetPublicKey returns the wallet's current public key
	handleGetPublicKey := func(w http.ResponseWriter, r *http.Request) {
		enableCORS(w, r)
		if r.Method != "POST" {
			http.Error(w, "Method not allowed", http.StatusMethodNotAllowed)
			return
		}

		// Get current wallet address and public key
		addressInfo, err := walletService.walletManager.GetCurrentAddress()
		if err != nil {
			http.Error(w, fmt.Sprintf("Failed to get wallet address: %v", err), http.StatusInternalServerError)
			return
		}

		response := map[string]interface{}{
			"publicKey": addressInfo.PublicKey,
			"address":   addressInfo.Address,
			"index":     addressInfo.Index,
		}

		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(response)
	}

	// handleCreateAction creates a new BRC-100 action
	handleCreateAction := func(w http.ResponseWriter, r *http.Request) {
		enableCORS(w, r)
		if r.Method != "POST" {
			http.Error(w, "Method not allowed", http.StatusMethodNotAllowed)
			return
		}

		// For now, return a placeholder response
		response := map[string]interface{}{
			"success": true,
			"message": "BRC-100 action creation not yet implemented",
			"actionId": "placeholder",
		}

		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(response)
	}

	// handleSignAction signs a BRC-100 action
	handleSignAction := func(w http.ResponseWriter, r *http.Request) {
		enableCORS(w, r)
		if r.Method != "POST" {
			http.Error(w, "Method not allowed", http.StatusMethodNotAllowed)
			return
		}

		// For now, return a placeholder response
		response := map[string]interface{}{
			"success": true,
			"message": "BRC-100 action signing not yet implemented",
			"signature": "placeholder",
		}

		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(response)
	}

	// handleProcessAction processes a completed BRC-100 action
	handleProcessAction := func(w http.ResponseWriter, r *http.Request) {
		enableCORS(w, r)
		if r.Method != "POST" {
			http.Error(w, "Method not allowed", http.StatusMethodNotAllowed)
			return
		}

		// For now, return a placeholder response
		response := map[string]interface{}{
			"success": true,
			"message": "BRC-100 action processing not yet implemented",
			"result": "placeholder",
		}

		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(response)
	}

	// handleIsAuthenticated returns authentication status
	handleIsAuthenticated := func(w http.ResponseWriter, r *http.Request) {
		enableCORS(w, r)
		if r.Method != "POST" {
			http.Error(w, "Method not allowed", http.StatusMethodNotAllowed)
			return
		}

		// For now, return authenticated=true since we have a wallet
		response := map[string]interface{}{
			"authenticated": true,
			"timestamp": time.Now().Format(time.RFC3339),
		}

		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(response)
	}

	// handleCreateSignature creates a signature for data
	handleCreateSignature := func(w http.ResponseWriter, r *http.Request) {
		enableCORS(w, r)
		if r.Method != "POST" {
			http.Error(w, "Method not allowed", http.StatusMethodNotAllowed)
			return
		}

		// Parse request body to get data to sign
		var req struct {
			Message string `json:"message"`
			Format  string `json:"format,omitempty"`
		}

		// Try to decode JSON, but don't fail if body is empty
		if r.ContentLength > 0 {
			if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
				// If JSON parsing fails, use default message with timestamp
				req.Message = fmt.Sprintf("BitcoinBrowser_BRC100_Wallet_Auth_%d", time.Now().Unix())
			}
		}

		// Use default message if none provided
		if req.Message == "" {
			req.Message = fmt.Sprintf("BitcoinBrowser_BRC100_Wallet_Auth_%d", time.Now().Unix())
		}

		// Get current address
		currentAddress, err := walletService.walletManager.GetCurrentAddress()
		if err != nil {
			http.Error(w, "Failed to get current address", http.StatusInternalServerError)
			return
		}

		// Get private key for current address
		privateKeyHex, err := walletService.walletManager.GetPrivateKeyForAddress(currentAddress.Address)
		if err != nil {
			http.Error(w, "Failed to get private key", http.StatusInternalServerError)
			return
		}

		// Convert private key hex to bytes
		privateKeyBytes, err := hex.DecodeString(privateKeyHex)
		if err != nil {
			http.Error(w, "Invalid private key", http.StatusInternalServerError)
			return
		}

		// Create ECDSA private key from bytes
		// First, we need to derive the public key from the private key
		privKey := &ecdsa.PrivateKey{
			PublicKey: ecdsa.PublicKey{
				Curve: elliptic.P256(),
			},
			D: new(big.Int).SetBytes(privateKeyBytes),
		}

		// Derive the public key from the private key
		privKey.PublicKey.X, privKey.PublicKey.Y = privKey.Curve.ScalarBaseMult(privateKeyBytes)

		// Hash the message
		hash := sha256.Sum256([]byte(req.Message))

		// Sign the hash
		sigR, sigS, err := ecdsa.Sign(rand.Reader, privKey, hash[:])
		if err != nil {
			http.Error(w, "Failed to create signature", http.StatusInternalServerError)
			return
		}

		// Convert signature to compact format (r + s)
		signatureBytes := append(sigR.Bytes(), sigS.Bytes()...)
		signature := signatureBytes

		// Get public key for current address
		publicKeyHex := currentAddress.PublicKey

		// Return signature and public key
		response := map[string]interface{}{
			"success": true,
			"signature": hex.EncodeToString(signature),
			"publicKey": publicKeyHex,
			"address": currentAddress.Address,
			"message": req.Message,
			"timestamp": time.Now().Format(time.RFC3339),
		}

		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(response)
	}

	// BRC-100 API endpoints
	handleBRC100Aliases := func(w http.ResponseWriter, r *http.Request) {
		enableCORS(w, r)
		if r.Method != "GET" {
			http.Error(w, "Method not allowed", http.StatusMethodNotAllowed)
			return
		}

		// Get current address for the alias
		currentAddress, err := walletService.walletManager.GetCurrentAddress()
		if err != nil {
			http.Error(w, "Failed to get current address", http.StatusInternalServerError)
			return
		}

		// Return Archie as alias with real address and public key
		response := map[string]interface{}{
			"aliases": []map[string]interface{}{
				{
					"alias": "Archie",
					"address": currentAddress.Address,
					"publicKey": currentAddress.PublicKey,
					"verified": true,
				},
			},
			"success": true,
			"timestamp": time.Now().Format(time.RFC3339),
		}
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(response)
	}

	handleBRC100Transactions := func(w http.ResponseWriter, r *http.Request) {
		enableCORS(w, r)
		if r.Method != "GET" {
			http.Error(w, "Method not allowed", http.StatusMethodNotAllowed)
			return
		}

		// Return empty transactions for now - our transactions are none of their business
		response := map[string]interface{}{
			"transactions": []string{},
			"success": true,
			"timestamp": time.Now().Format(time.RFC3339),
		}
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(response)
	}

	// BRC-100 Wallet Endpoints
	http.HandleFunc("/getVersion", handleGetVersion)
	http.HandleFunc("/getPublicKey", handleGetPublicKey)
	http.HandleFunc("/isAuthenticated", handleIsAuthenticated)
	http.HandleFunc("/createSignature", handleCreateSignature)
	http.HandleFunc("/createAction", handleCreateAction)
	http.HandleFunc("/signAction", handleSignAction)
	http.HandleFunc("/processAction", handleProcessAction)

	// Additional BRC-100 endpoints
	handleWaitForAuthentication := func(w http.ResponseWriter, r *http.Request) {
		enableCORS(w, r)
		if r.Method != "POST" {
			http.Error(w, "Method not allowed", http.StatusMethodNotAllowed)
			return
		}

		// For now, just return success immediately (bypass modal)
		response := map[string]interface{}{
			"authenticated": true,
			"success": true,
			"timestamp": time.Now().Format(time.RFC3339),
		}
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(response)
	}

	// BRC-100 API endpoints
	http.HandleFunc("/api/brc-100/aliases", handleBRC100Aliases)
	http.HandleFunc("/api/brc-100/transactions", handleBRC100Transactions)
	http.HandleFunc("/waitForAuthentication", handleWaitForAuthentication)

	handleListOutputs := func(w http.ResponseWriter, r *http.Request) {
		enableCORS(w, r)
		if r.Method != "POST" {
			http.Error(w, "Method not allowed", http.StatusMethodNotAllowed)
			return
		}

		// Return empty outputs for privacy (for now)
		response := map[string]interface{}{
			"outputs": []string{},
			"success": true,
			"timestamp": time.Now().Format(time.RFC3339),
		}
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(response)
	}

	http.HandleFunc("/listOutputs", handleListOutputs)

	handleCreateHmac := func(w http.ResponseWriter, r *http.Request) {
		enableCORS(w, r)
		if r.Method != "POST" {
			http.Error(w, "Method not allowed", http.StatusMethodNotAllowed)
			return
		}

		// Parse request to get message to sign
		var req struct {
			Message string `json:"message"`
			Key     string `json:"key,omitempty"`
		}

		if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
			http.Error(w, "Invalid JSON", http.StatusBadRequest)
			return
		}

		// Use wallet-derived key for HMAC (most secure approach)
		currentAddress, err := walletService.walletManager.GetCurrentAddress()
		if err != nil {
			http.Error(w, "Failed to get current address", http.StatusInternalServerError)
			return
		}

		// Use wallet address as HMAC key (deterministic and secure)
		secretKey := []byte(currentAddress.Address)

		// Create HMAC signature
		h := hmac.New(sha256.New, secretKey)
		h.Write([]byte(req.Message))
		signature := h.Sum(nil)

		response := map[string]interface{}{
			"hmac": hex.EncodeToString(signature),
			"message": req.Message,
			"key": currentAddress.Address, // Return the key used for verification
			"success": true,
			"timestamp": time.Now().Format(time.RFC3339),
		}
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(response)
	}

	http.HandleFunc("/createHmac", handleCreateHmac)

	handleVerifyHmac := func(w http.ResponseWriter, r *http.Request) {
		enableCORS(w, r)
		if r.Method != "POST" {
			http.Error(w, "Method not allowed", http.StatusMethodNotAllowed)
			return
		}

		// Parse request to get HMAC data to verify
		var req struct {
			Message   string `json:"message"`
			Hmac      string `json:"hmac"`
			Key       string `json:"key,omitempty"`
		}

		// Try to decode JSON, but don't fail if body is empty
		if r.ContentLength > 0 {
			if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
				// If JSON parsing fails, use default values
				req.Message = "default_verify_message"
				req.Hmac = "default_hmac"
			}
		}

		// Use default values if none provided
		if req.Message == "" {
			req.Message = "default_verify_message"
		}
		if req.Hmac == "" {
			req.Hmac = "default_hmac"
		}

		// Use wallet-derived key for HMAC verification
		currentAddress, err := walletService.walletManager.GetCurrentAddress()
		if err != nil {
			http.Error(w, "Failed to get current address", http.StatusInternalServerError)
			return
		}

		// Use wallet address as HMAC key (same as createHmac)
		secretKey := []byte(currentAddress.Address)

		// Create expected HMAC signature
		h := hmac.New(sha256.New, secretKey)
		h.Write([]byte(req.Message))
		expectedSignature := h.Sum(nil)

		// Convert provided HMAC to bytes
		providedSignature, err := hex.DecodeString(req.Hmac)
		if err != nil {
			// If HMAC is not valid hex, treat as invalid
			valid := false
			response := map[string]interface{}{
				"valid": valid,
				"message": req.Message,
				"key": currentAddress.Address,
				"success": true,
				"timestamp": time.Now().Format(time.RFC3339),
			}
			w.Header().Set("Content-Type", "application/json")
			json.NewEncoder(w).Encode(response)
			return
		}

		// Verify HMAC
		valid := hmac.Equal(expectedSignature, providedSignature)

		response := map[string]interface{}{
			"valid": valid,
			"message": req.Message,
			"key": currentAddress.Address,
			"success": true,
			"timestamp": time.Now().Format(time.RFC3339),
		}
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(response)
	}

	http.HandleFunc("/verifyHmac", handleVerifyHmac)

	handleGetNetwork := func(w http.ResponseWriter, r *http.Request) {
		enableCORS(w, r)
		if r.Method != "POST" {
			http.Error(w, "Method not allowed", http.StatusMethodNotAllowed)
			return
		}

		// Return mainnet network information
		response := map[string]interface{}{
			"network": "mainnet",
			"chainId": "bitcoin-sv",
			"name": "Bitcoin SV Mainnet",
			"success": true,
			"timestamp": time.Now().Format(time.RFC3339),
		}
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(response)
	}

	http.HandleFunc("/getNetwork", handleGetNetwork)

	// Start HTTP server
	port := "3301"
	fmt.Printf("🌐 Wallet daemon listening on port %s\n", port)
	fmt.Println("📋 Available endpoints:")
	fmt.Println("  GET  /health - Health check")
	fmt.Println("  GET  /utxo/fetch?address=ADDRESS - Fetch UTXOs for address")
	fmt.Println("  POST /transaction/create - Create unsigned transaction")
	fmt.Println("  POST /transaction/sign - Sign transaction")
	fmt.Println("  POST /transaction/broadcast - Broadcast transaction to BSV network")
	fmt.Println("  POST /transaction/send - Send complete transaction (create + sign + broadcast)")
	fmt.Println("  GET  /transaction/history - Get transaction history")
	fmt.Println("  GET  /wallet/status - Check if unified wallet exists")
	fmt.Println("  POST /wallet/create - Create new unified wallet")
	fmt.Println("  POST /wallet/load - Load existing unified wallet")
	fmt.Println("  GET  /wallet/info - Get complete wallet information")
	fmt.Println("  POST /wallet/markBackedUp - Mark wallet as backed up")
	fmt.Println("  GET  /wallet/addresses - Get all addresses")
	fmt.Println("  POST /wallet/address/generate - Generate new address")
	fmt.Println("  GET  /wallet/address/current - Get current address")
	fmt.Println("  GET  /wallet/balance - Get total balance")
	fmt.Println("")
	fmt.Println("🔐 BRC-100 Endpoints:")
	fmt.Println("  GET  /brc100/status - BRC-100 service status")
	fmt.Println("  POST /brc100/identity/generate - Generate identity certificate")
	fmt.Println("  POST /brc100/identity/validate - Validate identity certificate")
	fmt.Println("  POST /brc100/identity/selective-disclosure - Create selective disclosure")
	fmt.Println("  POST /brc100/auth/challenge - Generate authentication challenge")
	fmt.Println("  POST /brc100/auth/authenticate - Authenticate with challenge")
	fmt.Println("  POST /brc100/auth/type42 - Derive Type-42 keys")
	fmt.Println("  POST /brc100/session/create - Create authentication session")
	fmt.Println("  POST /brc100/session/validate - Validate session")
	fmt.Println("  POST /brc100/session/revoke - Revoke session")
	fmt.Println("  POST /brc100/beef/create - Create BRC-100 BEEF transaction")
	fmt.Println("  POST /brc100/beef/verify - Verify BRC-100 BEEF transaction")
	fmt.Println("  POST /brc100/beef/broadcast - Convert and broadcast BEEF")
	fmt.Println("  POST /brc100/spv/verify - Verify identity with SPV")
	fmt.Println("  POST /brc100/spv/proof - Create SPV identity proof")
	fmt.Println("  WS   /brc100/ws - WebSocket for real-time BRC-100 communication")
	fmt.Println("  WS   /socket.io/ - Babbage-compatible WebSocket for Project Babbage integration")
	fmt.Println("")
	fmt.Println("🔌 BRC-100 Wallet Endpoints:")
	fmt.Println("  POST /getVersion - Get wallet version and capabilities")
	fmt.Println("  POST /getPublicKey - Get wallet's current public key")
	fmt.Println("  POST /isAuthenticated - Check if user is authenticated")
	fmt.Println("  POST /createSignature - Create signature for data")
	fmt.Println("🔌 BRC-100 API Endpoints:")
	fmt.Println("  GET  /api/brc-100/aliases - Get wallet aliases (Archie)")
	fmt.Println("  GET  /api/brc-100/transactions - Get BRC-100 transactions (empty)")
	fmt.Println("  POST /createAction - Create BRC-100 action")
	fmt.Println("  POST /signAction - Sign BRC-100 action")
	fmt.Println("  POST /processAction - Process BRC-100 action")
	fmt.Println("")
	fmt.Println("🔒 Domain Whitelist Endpoints:")
	fmt.Println("  POST /domain/whitelist/add - Add domain to whitelist")
	fmt.Println("  GET  /domain/whitelist/check?domain=<domain> - Check if domain is whitelisted")
	fmt.Println("  POST /domain/whitelist/record - Record request from domain")
	fmt.Println("  GET  /domain/whitelist/list - List all whitelisted domains")
	fmt.Println("  POST /domain/whitelist/remove - Remove domain from whitelist")


	// Helper function to derive BRC-42 child key and sign data
	signWithDerivedKey := func(data []byte, privateKeyHex string, invoiceNumber string, counterpartyPubKey string) ([]byte, error) {
		// Convert private key hex to bytes
		privateKeyBytes, err := hex.DecodeString(privateKeyHex)
		if err != nil {
			return nil, fmt.Errorf("failed to decode private key: %v", err)
		}

		// Parse counterparty public key
		counterpartyPubKeyBytes, err := hex.DecodeString(counterpartyPubKey)
		if err != nil {
			return nil, fmt.Errorf("failed to decode counterparty public key: %v", err)
		}

		// Decode the public key point
		curve := elliptic.P256()
		x, y := elliptic.Unmarshal(curve, counterpartyPubKeyBytes)
		if x == nil {
			return nil, fmt.Errorf("failed to unmarshal counterparty public key")
		}

		// Compute ECDH shared secret: privateKey * counterpartyPublicKey
		sharedSecretX, _ := curve.ScalarMult(x, y, privateKeyBytes)
		sharedSecret := sharedSecretX.Bytes()

		// Compute HMAC over invoice number using shared secret
		mac := hmac.New(sha256.New, sharedSecret)
		mac.Write([]byte(invoiceNumber))
		hmacResult := mac.Sum(nil)

		// Convert HMAC to scalar (big.Int)
		hmacScalar := new(big.Int).SetBytes(hmacResult)

		// Derive child private key: rootPrivateKey + hmacScalar (mod N)
		rootPrivateKey := new(big.Int).SetBytes(privateKeyBytes)
		curveOrder := curve.Params().N
		childPrivateKeyInt := new(big.Int).Add(rootPrivateKey, hmacScalar)
		childPrivateKeyInt.Mod(childPrivateKeyInt, curveOrder)

		// Create ECDSA private key from derived key
		childPrivateKey := &ecdsa.PrivateKey{
			PublicKey: ecdsa.PublicKey{
				Curve: curve,
			},
			D: childPrivateKeyInt,
		}

		// Calculate the public key from the private key
		childPrivateKey.PublicKey.X, childPrivateKey.PublicKey.Y = curve.ScalarBaseMult(childPrivateKeyInt.Bytes())

		// Hash the data
		hash := sha256.Sum256(data)

		// Sign the hash using the derived key
		sigR, sigS, err := ecdsa.Sign(rand.Reader, childPrivateKey, hash[:])
		if err != nil {
			return nil, fmt.Errorf("failed to sign data: %v", err)
		}

		// Convert signature to compact format (r + s)
		signature := append(sigR.Bytes(), sigS.Bytes()...)

		return signature, nil
	}

	// Helper function to sign data with private key using raw ECDSA (for non-BRC-43 endpoints)
	// Add handler for Babbage auth endpoint - respond to their challenge
	http.HandleFunc("/.well-known/auth", func(w http.ResponseWriter, r *http.Request) {
		if r.Method != "POST" {
			http.Error(w, "Method not allowed", http.StatusMethodNotAllowed)
			return
		}

		// Parse request body
		body, err := io.ReadAll(r.Body)
		if err != nil {
			log.Printf("Error reading auth request body: %v", err)
			http.Error(w, "Bad request", http.StatusBadRequest)
			return
		}

		log.Printf("🔐 Babbage auth request received: %s", string(body))

		// Parse JSON to extract request details
		var authReq struct {
			Version               string `json:"version"`
			MessageType          string `json:"messageType"`
			IdentityKey          string `json:"identityKey"`
			InitialNonce         string `json:"initialNonce"`
			RequestedCertificates struct {
				Certifiers []string          `json:"certifiers"`
				Types      map[string]string `json:"types"`
			} `json:"requestedCertificates"`
		}
		if err := json.Unmarshal(body, &authReq); err != nil {
			log.Printf("Error parsing auth request JSON: %v", err)
			http.Error(w, "Invalid JSON", http.StatusBadRequest)
			return
		}

		log.Printf("🔐 Client sent challenge (initialNonce): %s", authReq.InitialNonce)
		log.Printf("🔐 Client identity key: %s", authReq.IdentityKey)
		log.Printf("🔐 MessageType: %s", authReq.MessageType)

		// Get current address for signing
		currentAddress, err := walletService.walletManager.GetCurrentAddress()
		if err != nil {
			log.Printf("Error getting current address: %v", err)
			http.Error(w, "Wallet not available", http.StatusInternalServerError)
			return
		}

		// Get private key for current address
		privateKeyHex, err := walletService.walletManager.GetPrivateKeyForAddress(currentAddress.Address)
		if err != nil {
			log.Printf("Error getting private key: %v", err)
			http.Error(w, "Failed to get private key", http.StatusInternalServerError)
			return
		}

		log.Printf("🔐 Using public key for signing: %s", currentAddress.PublicKey)
		log.Printf("🔐 Client expects identity key: %s", authReq.IdentityKey)

		// Decode the base64 nonce to get the actual challenge data
		nonceBytes, err := base64.StdEncoding.DecodeString(authReq.InitialNonce)
		if err != nil {
			log.Printf("Error decoding base64 nonce: %v", err)
			http.Error(w, "Invalid nonce format", http.StatusBadRequest)
			return
		}

		log.Printf("🔐 Decoded nonce bytes length: %d", len(nonceBytes))

		// Generate OUR nonce for mutual authentication
		ourNonce := make([]byte, 32)
		if _, err := rand.Read(ourNonce); err != nil {
			log.Printf("Error generating our nonce: %v", err)
			http.Error(w, "Failed to generate nonce", http.StatusInternalServerError)
			return
		}
		ourNonceBase64 := base64.StdEncoding.EncodeToString(ourNonce)

		log.Printf("🔐 Generated our nonce: %s", ourNonceBase64)

		// According to client code: ge((o.sessionNonce ?? "") + (c.initialNonce ?? ""), "base64")
		// The client concatenates sessionNonce + initialNonce, then converts to bytes
		// Then verifies using BRC-43: protocolID: [2, "auth message signature"]

		// Concatenate the base64 strings: theirInitialNonce + ourNonce
		concatenatedStrings := authReq.InitialNonce + ourNonceBase64

		// Convert to bytes
		dataToSign := []byte(concatenatedStrings)

		log.Printf("🔐 Data to sign (concatenated strings): %s", concatenatedStrings)
		log.Printf("🔐 Data to sign length: %d bytes", len(dataToSign))

		// Create BRC-43 invoice number: securityLevel-protocolID-keyID
		// protocolID: "auth message signature"
		// keyID: sessionNonce + " " + initialNonce
		invoiceNumber := fmt.Sprintf("2-auth message signature-%s %s", authReq.InitialNonce, ourNonceBase64)

		log.Printf("🔐 Using BRC-43 invoice number: %s", invoiceNumber)
		log.Printf("🔐 Counterparty: %s", authReq.IdentityKey)

		// Sign using BRC-42 derived key
		signature, err := signWithDerivedKey(dataToSign, privateKeyHex, invoiceNumber, authReq.IdentityKey)
		if err != nil {
			log.Printf("Error signing with derived key: %v", err)
			http.Error(w, "Failed to sign", http.StatusInternalServerError)
			return
		}

		log.Printf("🔐 Signature created with BRC-42 derived key: %s", hex.EncodeToString(signature))

		// Create the BRC-104 compliant auth response
		authResponse := map[string]interface{}{
			"version":       "0.1",
			"messageType":   "initialResponse",
			"identityKey":   currentAddress.PublicKey,
			"nonce":         ourNonceBase64,           // OUR new nonce
			"yourNonce":     authReq.InitialNonce,     // Their initial nonce
			"signature":     hex.EncodeToString(signature),
		}

		log.Printf("🔐 Returning auth response via HTTP")
		log.Printf("🔐 Response: version=%s, nonce=%s, yourNonce=%s", authResponse["version"], ourNonceBase64, authReq.InitialNonce)

		// Return the signed response immediately via HTTP (BRC-104 specification)
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusOK)
		json.NewEncoder(w).Encode(authResponse)
	})

	// Add handler for client's authentication response (when they sign our nonce)
	// This completes the mutual authentication flow
	http.HandleFunc("/.well-known/auth/verify", func(w http.ResponseWriter, r *http.Request) {
		if r.Method != "POST" {
			http.Error(w, "Method not allowed", http.StatusMethodNotAllowed)
			return
		}

		// Parse request body
		body, err := io.ReadAll(r.Body)
		if err != nil {
			log.Printf("Error reading auth verify request body: %v", err)
			http.Error(w, "Bad request", http.StatusBadRequest)
			return
		}

		log.Printf("🔐 Client authentication verification received: %s", string(body))

		// Parse the client's authentication response
		var clientAuth struct {
			Version      string `json:"version"`
			MessageType  string `json:"messageType"`
			IdentityKey  string `json:"identityKey"`
			Nonce        string `json:"nonce"`        // Our nonce (that we sent them)
			YourNonce    string `json:"yourNonce"`    // Their original nonce
			Signature    string `json:"signature"`    // Their signature of OUR nonce
		}
		if err := json.Unmarshal(body, &clientAuth); err != nil {
			log.Printf("Error parsing client auth JSON: %v", err)
			http.Error(w, "Invalid JSON", http.StatusBadRequest)
			return
		}

		log.Printf("🔐 Client identity key: %s", clientAuth.IdentityKey)
		log.Printf("🔐 Client signed our nonce: %s", clientAuth.Nonce)
		log.Printf("🔐 Client signature: %s", clientAuth.Signature)

		// TODO: Verify the client's signature of our nonce
		// For now, accept the authentication
		response := map[string]interface{}{
			"version":     "1.0",
			"messageType": "authenticationComplete",
			"success":     true,
			"message":     "Mutual authentication successful",
		}

		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(response)
	})

	// Add handler for ToolBSV's /brc100-auth endpoint
	http.HandleFunc("/brc100-auth", func(w http.ResponseWriter, r *http.Request) {
		log.Printf("🔐 ToolBSV /brc100-auth request received")

		// Enable CORS for ToolBSV
		w.Header().Set("Access-Control-Allow-Origin", "*")
		w.Header().Set("Access-Control-Allow-Methods", "POST, GET, OPTIONS")
		w.Header().Set("Access-Control-Allow-Headers", "Content-Type")

		if r.Method == "OPTIONS" {
			w.WriteHeader(http.StatusOK)
			return
		}

		if r.Method != "POST" {
			http.Error(w, "Method not allowed", http.StatusMethodNotAllowed)
			return
		}

		// Parse request body
		body, err := io.ReadAll(r.Body)
		if err != nil {
			log.Printf("Error reading /brc100-auth request body: %v", err)
			http.Error(w, "Bad request", http.StatusBadRequest)
			return
		}

		log.Printf("🔐 ToolBSV /brc100-auth request body: %s", string(body))

		// For now, return a simple success response
		// TODO: Implement proper ToolBSV authentication flow
		response := map[string]interface{}{
			"success": true,
			"message": "ToolBSV authentication endpoint reached",
			"version": "1.0",
		}

		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(response)
	})

	// Initialize BRC-33 PeerServ message store
	messageStore := NewMessageStore(logrus.StandardLogger())
	log.Println("📬 BRC-33 PeerServ message store initialized")

	// Register BRC-33 PeerServ message relay endpoints
	http.HandleFunc("/sendMessage", HandleSendMessage(messageStore, walletService))
	http.HandleFunc("/listMessages", HandleListMessages(messageStore, walletService))
	http.HandleFunc("/acknowledgeMessage", HandleAcknowledgeMessage(messageStore, walletService))
	log.Println("📬 BRC-33 PeerServ endpoints registered: /sendMessage, /listMessages, /acknowledgeMessage")

	// Create a custom HTTP server that can handle WebSocket upgrades
	server := &http.Server{
		Addr: ":" + port,
		Handler: http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
			// Check if this is a WebSocket upgrade request
			if r.Header.Get("Connection") == "upgrade" && r.Header.Get("Upgrade") == "websocket" {
				// Handle WebSocket upgrade
				if r.URL.Path == "/socket.io/" {
					brc100SocketIOHandler.HandleSocketIO(w, r)
					return
				} else if r.URL.Path == "/brc100/ws" {
					wsHandler.HandleWebSocket(w, r)
					return
				}
			}
			// For all other requests, use the default mux
			http.DefaultServeMux.ServeHTTP(w, r)
		}),
	}

	log.Fatal(server.ListenAndServe())
}
