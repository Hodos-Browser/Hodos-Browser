package main

import (
	"encoding/hex"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"time"

	ec "github.com/bsv-blockchain/go-sdk/primitives/ec"
	"github.com/bsv-blockchain/go-sdk/script"
	"github.com/sirupsen/logrus"
	"github.com/tyler-smith/go-bip32"
	"github.com/tyler-smith/go-bip39"
)

// Wallet represents a unified wallet with HD capabilities
type Wallet struct {
	Version      string          `json:"version"`
	CreatedAt    time.Time       `json:"createdAt"`
	LastUsed     time.Time       `json:"lastUsed"`
	Mnemonic     string          `json:"mnemonic"`
	MasterKey    string          `json:"masterKey"`
	Addresses    []AddressInfo   `json:"addresses"`
	CurrentIndex int             `json:"currentIndex"`
	BackedUp     bool            `json:"backedUp"`
	Settings     WalletSettings  `json:"settings"`
	BRC100       *BRC100Data     `json:"brc100,omitempty"`
}

// WalletSettings contains wallet configuration
type WalletSettings struct {
	Network       string `json:"network"`
	DefaultFeeRate int64 `json:"defaultFeeRate"`
}

// AddressInfo represents a single address in the HD wallet
type AddressInfo struct {
	Index     int    `json:"index"`
	Address   string `json:"address"`
	PublicKey string `json:"publicKey"`
	Used      bool   `json:"used"`
	Balance   int64  `json:"balance"`
}

// BRC100Data represents BRC-100 related data stored in the wallet
type BRC100Data struct {
	Version      string                   `json:"version"`
	Identities   []BRC100Identity         `json:"identities"`
	Sessions     []BRC100Session          `json:"sessions"`
	Challenges   []BRC100Challenge        `json:"challenges"`
	Settings     BRC100Settings           `json:"settings"`
	CreatedAt    time.Time                `json:"createdAt"`
	LastUpdated  time.Time                `json:"lastUpdated"`
}

// BRC100Identity represents a BRC-100 identity certificate
type BRC100Identity struct {
	ID           string                 `json:"id"`
	Subject      string                 `json:"subject"`
	Issuer       string                 `json:"issuer"`
	PublicKey    string                 `json:"publicKey"`
	Certificate  map[string]interface{} `json:"certificate"`
	CreatedAt    time.Time              `json:"createdAt"`
	ExpiresAt    time.Time              `json:"expiresAt"`
	Revoked      bool                   `json:"revoked"`
	Transactions []string               `json:"transactions"` // Transaction IDs where this identity was used
}

// BRC100Session represents an active BRC-100 session
type BRC100Session struct {
	SessionID     string    `json:"sessionId"`
	AppDomain     string    `json:"appDomain"`
	IdentityID    string    `json:"identityId"`
	Permissions   []string  `json:"permissions"`
	CreatedAt     time.Time `json:"createdAt"`
	ExpiresAt     time.Time `json:"expiresAt"`
	LastUsed      time.Time `json:"lastUsed"`
	Authenticated bool      `json:"authenticated"`
}

// BRC100Challenge represents a pending authentication challenge
type BRC100Challenge struct {
	ChallengeID   string    `json:"challengeId"`
	AppDomain     string    `json:"appDomain"`
	Challenge     string    `json:"challenge"`
	CreatedAt     time.Time `json:"createdAt"`
	ExpiresAt     time.Time `json:"expiresAt"`
	Solved        bool      `json:"solved"`
	Response      string    `json:"response,omitempty"`
	SessionID     string    `json:"sessionId,omitempty"`
}

// BRC100Settings contains BRC-100 specific settings
type BRC100Settings struct {
	AutoApprove        bool     `json:"autoApprove"`
	DefaultPermissions []string `json:"defaultPermissions"`
	SessionTimeout     int      `json:"sessionTimeout"` // minutes
	ChallengeTimeout   int      `json:"challengeTimeout"` // minutes
	MaxSessions        int      `json:"maxSessions"`
}

// WalletManager manages unified wallet operations
type WalletManager struct {
	wallet *Wallet
	logger *logrus.Logger
}

// NewWalletManager creates a new unified wallet manager
func NewWalletManager() *WalletManager {
	logger := logrus.New()
	logger.SetLevel(logrus.InfoLevel)

	return &WalletManager{
		logger: logger,
	}
}

// GenerateMnemonic generates a new 12-word mnemonic phrase
func (wm *WalletManager) GenerateMnemonic() (string, error) {
	wm.logger.Info("Generating new mnemonic phrase...")

	// Generate 128 bits of entropy (12 words)
	entropy, err := bip39.NewEntropy(128)
	if err != nil {
		return "", fmt.Errorf("failed to generate entropy: %v", err)
	}

	// Generate mnemonic from entropy
	mnemonic, err := bip39.NewMnemonic(entropy)
	if err != nil {
		return "", fmt.Errorf("failed to generate mnemonic: %v", err)
	}

	wm.logger.Info("Mnemonic generated successfully")
	return mnemonic, nil
}

// CreateFromMnemonic creates a unified wallet from a mnemonic phrase
func (wm *WalletManager) CreateFromMnemonic(mnemonic string) error {
	wm.logger.Info("Creating unified wallet from mnemonic...")

	// Validate mnemonic
	if !bip39.IsMnemonicValid(mnemonic) {
		return fmt.Errorf("invalid mnemonic phrase")
	}

	// Generate seed from mnemonic
	seed := bip39.NewSeed(mnemonic, "")

	// Create master key from seed
	masterKey, err := bip32.NewMasterKey(seed)
	if err != nil {
		return fmt.Errorf("failed to create master key: %v", err)
	}

	// Create unified wallet
	wm.wallet = &Wallet{
		Version:      "1.0.0",
		CreatedAt:    time.Now(),
		LastUsed:     time.Now(),
		Mnemonic:     mnemonic,
		MasterKey:    masterKey.B58Serialize(),
		Addresses:    []AddressInfo{},
		CurrentIndex: 0,
		BackedUp:     false,
		Settings: WalletSettings{
			Network:       "mainnet",
			DefaultFeeRate: 1,
		},
	}

	wm.logger.Info("Unified wallet created successfully")
	return nil
}

// GenerateAddress generates a new address at the specified index
func (wm *WalletManager) GenerateAddress(index int) (*AddressInfo, error) {
	wm.logger.Infof("Generating address at index %d", index)

	if wm.wallet == nil {
		return nil, fmt.Errorf("wallet not initialized")
	}

	// Parse master key
	masterKey, err := bip32.B58Deserialize(wm.wallet.MasterKey)
	if err != nil {
		return nil, fmt.Errorf("failed to parse master key: %v", err)
	}

	// Derive key using BIP44 path: m/44'/236'/0'/0/{index}
	// 44' = BIP44, 236' = Bitcoin SV, 0' = Account 0, 0 = External chain
	derivedKey, err := masterKey.NewChildKey(uint32(index))
	if err != nil {
		return nil, fmt.Errorf("failed to derive key for index %d: %v", index, err)
	}

	// Get public key
	publicKeyBytes := derivedKey.PublicKey().Key
	publicKey, err := ec.PublicKeyFromBytes(publicKeyBytes)
	if err != nil {
		return nil, fmt.Errorf("failed to create public key: %v", err)
	}

	// Generate Bitcoin SV address
	address, err := script.NewAddressFromPublicKey(publicKey, true) // true = mainnet
	if err != nil {
		return nil, fmt.Errorf("failed to generate address: %v", err)
	}

	addressInfo := &AddressInfo{
		Index:     index,
		Address:   address.AddressString,
		PublicKey: hex.EncodeToString(publicKey.Compressed()),
		Used:      false,
		Balance:   0,
	}

	wm.logger.Infof("Address generated: %s", addressInfo.Address)
	return addressInfo, nil
}

// GetNextAddress generates the next address in sequence
func (wm *WalletManager) GetNextAddress() (*AddressInfo, error) {
	if wm.wallet == nil {
		return nil, fmt.Errorf("wallet not initialized")
	}

	// Generate address at current index
	addressInfo, err := wm.GenerateAddress(wm.wallet.CurrentIndex)
	if err != nil {
		return nil, err
	}

	// Add to wallet addresses
	wm.wallet.Addresses = append(wm.wallet.Addresses, *addressInfo)
	wm.wallet.CurrentIndex++
	wm.wallet.LastUsed = time.Now()

	return addressInfo, nil
}

// GetAllAddresses returns all generated addresses
func (wm *WalletManager) GetAllAddresses() []AddressInfo {
	if wm.wallet == nil {
		return []AddressInfo{}
	}
	return wm.wallet.Addresses
}

// GetCurrentAddress returns the most recently generated address
func (wm *WalletManager) GetCurrentAddress() (*AddressInfo, error) {
	if wm.wallet == nil || len(wm.wallet.Addresses) == 0 {
		return nil, fmt.Errorf("no addresses generated")
	}

	lastIndex := len(wm.wallet.Addresses) - 1
	return &wm.wallet.Addresses[lastIndex], nil
}

// GetPrivateKeyForAddress gets the private key for a specific address
func (wm *WalletManager) GetPrivateKeyForAddress(address string) (string, error) {
	if wm.wallet == nil {
		return "", fmt.Errorf("wallet not initialized")
	}

	// Find the address in our list
	var addressIndex int = -1
	for i, addr := range wm.wallet.Addresses {
		if addr.Address == address {
			addressIndex = i
			break
		}
	}

	if addressIndex == -1 {
		return "", fmt.Errorf("address not found in wallet")
	}

	// Parse master key
	masterKey, err := bip32.B58Deserialize(wm.wallet.MasterKey)
	if err != nil {
		return "", fmt.Errorf("failed to parse master key: %v", err)
	}

	// Derive key for the specific index
	derivedKey, err := masterKey.NewChildKey(uint32(addressIndex))
	if err != nil {
		return "", fmt.Errorf("failed to derive key for index %d: %v", addressIndex, err)
	}

	// Get private key bytes
	privateKeyBytes := derivedKey.Key
	return hex.EncodeToString(privateKeyBytes), nil
}

// GetTotalBalance calculates the total balance across all addresses by fetching live UTXOs
func (wm *WalletManager) GetTotalBalance() (int64, error) {
	if wm.wallet == nil {
		return 0, fmt.Errorf("wallet not initialized")
	}

	totalBalance := int64(0)
	utxoManager := NewUTXOManager()

	for i := range wm.wallet.Addresses {
		address := wm.wallet.Addresses[i].Address

		// Fetch live UTXOs for this address
		utxos, err := utxoManager.FetchUTXOs(address)
		if err != nil {
			// Log error but continue with other addresses
			fmt.Printf("Warning: Failed to fetch UTXOs for address %s: %v\n", address, err)
			continue
		}

		// Sum UTXOs for this address
		addressBalance := int64(0)
		for _, utxo := range utxos {
			addressBalance += utxo.Amount
		}

		// Update stored balance
		wm.wallet.Addresses[i].Balance = addressBalance
		totalBalance += addressBalance

		fmt.Printf("Address %s: %d satoshis (%d UTXOs)\n", address, addressBalance, len(utxos))
	}

	fmt.Printf("Total balance across all addresses: %d satoshis\n", totalBalance)
	return totalBalance, nil
}

// SaveToFile saves the unified wallet to a JSON file
func (wm *WalletManager) SaveToFile(filePath string) error {
	if wm.wallet == nil {
		return fmt.Errorf("wallet not initialized")
	}

	wm.logger.Infof("Saving unified wallet to: %s", filePath)

	// Create directory if it doesn't exist
	dir := filepath.Dir(filePath)
	if err := os.MkdirAll(dir, 0755); err != nil {
		return fmt.Errorf("failed to create directory: %v", err)
	}

	// Marshal to JSON
	data, err := json.MarshalIndent(wm.wallet, "", "  ")
	if err != nil {
		return fmt.Errorf("failed to marshal wallet: %v", err)
	}

	// Write to file
	if err := os.WriteFile(filePath, data, 0600); err != nil {
		return fmt.Errorf("failed to write wallet file: %v", err)
	}

	wm.logger.Info("Unified wallet saved successfully")
	return nil
}

// LoadFromFile loads the unified wallet from a JSON file
func (wm *WalletManager) LoadFromFile(filePath string) error {
	wm.logger.Infof("Loading unified wallet from: %s", filePath)

	// Check if file exists
	if _, err := os.Stat(filePath); os.IsNotExist(err) {
		return fmt.Errorf("wallet file does not exist")
	}

	// Read file
	data, err := os.ReadFile(filePath)
	if err != nil {
		return fmt.Errorf("failed to read wallet file: %v", err)
	}

	// Unmarshal JSON
	var wallet Wallet
	if err := json.Unmarshal(data, &wallet); err != nil {
		return fmt.Errorf("failed to unmarshal wallet: %v", err)
	}

	wm.wallet = &wallet
	wm.logger.Info("Unified wallet loaded successfully")
	return nil
}

// GetWalletPath returns the standard unified wallet file path
func GetWalletPath() string {
	homeDir, _ := os.UserHomeDir()
	return filepath.Join(homeDir, "AppData", "Roaming", "BabbageBrowser", "wallet", "wallet.json")
}

// WalletExists checks if a unified wallet file exists
func (wm *WalletManager) WalletExists() bool {
	_, err := os.Stat(GetWalletPath())
	return !os.IsNotExist(err)
}

// MarkBackedUp marks the wallet as backed up
func (wm *WalletManager) MarkBackedUp() error {
	if wm.wallet == nil {
		return fmt.Errorf("wallet not initialized")
	}

	wm.wallet.BackedUp = true
	wm.wallet.LastUsed = time.Now()

	wm.logger.Info("Wallet marked as backed up")
	return nil
}

// GetWalletInfo returns complete wallet information for frontend
func (wm *WalletManager) GetWalletInfo() (*Wallet, error) {
	if wm.wallet == nil {
		return nil, fmt.Errorf("wallet not initialized")
	}

	return wm.wallet, nil
}

// ============================================================================
// BRC-100 Methods
// ============================================================================

// InitializeBRC100 initializes BRC-100 data for the wallet
func (wm *WalletManager) InitializeBRC100() error {
	if wm.wallet == nil {
		return fmt.Errorf("wallet not initialized")
	}

	// Initialize BRC-100 data if it doesn't exist
	if wm.wallet.BRC100 == nil {
		wm.logger.Info("Initializing BRC-100 data for wallet")

		wm.wallet.BRC100 = &BRC100Data{
			Version:     "1.0.0",
			Identities:  []BRC100Identity{},
			Sessions:    []BRC100Session{},
			Challenges:  []BRC100Challenge{},
			CreatedAt:   time.Now(),
			LastUpdated: time.Now(),
			Settings: BRC100Settings{
				AutoApprove:        false,
				DefaultPermissions: []string{"read_profile"},
				SessionTimeout:     60,    // 60 minutes
				ChallengeTimeout:   5,     // 5 minutes
				MaxSessions:        10,    // Maximum 10 concurrent sessions
			},
		}

		// Save the updated wallet
		if err := wm.SaveToFile(GetWalletPath()); err != nil {
			return fmt.Errorf("failed to save wallet with BRC-100 data: %v", err)
		}

		wm.logger.Info("BRC-100 data initialized successfully")
	}

	return nil
}

// GetBRC100Data returns the BRC-100 data from the wallet
func (wm *WalletManager) GetBRC100Data() (*BRC100Data, error) {
	if wm.wallet == nil {
		return nil, fmt.Errorf("wallet not initialized")
	}

	if wm.wallet.BRC100 == nil {
		// Initialize BRC-100 data if it doesn't exist
		if err := wm.InitializeBRC100(); err != nil {
			return nil, fmt.Errorf("failed to initialize BRC-100 data: %v", err)
		}
	}

	return wm.wallet.BRC100, nil
}

// SaveBRC100Data saves BRC-100 data to the wallet
func (wm *WalletManager) SaveBRC100Data() error {
	if wm.wallet == nil {
		return fmt.Errorf("wallet not initialized")
	}

	if wm.wallet.BRC100 != nil {
		wm.wallet.BRC100.LastUpdated = time.Now()
	}

	return wm.SaveToFile(GetWalletPath())
}

// AddBRC100Identity adds a new BRC-100 identity to the wallet
func (wm *WalletManager) AddBRC100Identity(identity *BRC100Identity) error {
	brc100Data, err := wm.GetBRC100Data()
	if err != nil {
		return err
	}

	// Check if identity already exists
	for _, existing := range brc100Data.Identities {
		if existing.ID == identity.ID {
			return fmt.Errorf("identity with ID %s already exists", identity.ID)
		}
	}

	brc100Data.Identities = append(brc100Data.Identities, *identity)

	if err := wm.SaveBRC100Data(); err != nil {
		return fmt.Errorf("failed to save BRC-100 identity: %v", err)
	}

	wm.logger.WithField("identityId", identity.ID).Info("BRC-100 identity added successfully")
	return nil
}

// AddBRC100Session adds a new BRC-100 session to the wallet
func (wm *WalletManager) AddBRC100Session(session *BRC100Session) error {
	brc100Data, err := wm.GetBRC100Data()
	if err != nil {
		return err
	}

	// Check if session already exists
	for _, existing := range brc100Data.Sessions {
		if existing.SessionID == session.SessionID {
			return fmt.Errorf("session with ID %s already exists", session.SessionID)
		}
	}

	brc100Data.Sessions = append(brc100Data.Sessions, *session)

	if err := wm.SaveBRC100Data(); err != nil {
		return fmt.Errorf("failed to save BRC-100 session: %v", err)
	}

	wm.logger.WithField("sessionId", session.SessionID).Info("BRC-100 session added successfully")
	return nil
}

// AddBRC100Challenge adds a new BRC-100 challenge to the wallet
func (wm *WalletManager) AddBRC100Challenge(challenge *BRC100Challenge) error {
	brc100Data, err := wm.GetBRC100Data()
	if err != nil {
		return err
	}

	// Check if challenge already exists
	for _, existing := range brc100Data.Challenges {
		if existing.ChallengeID == challenge.ChallengeID {
			return fmt.Errorf("challenge with ID %s already exists", challenge.ChallengeID)
		}
	}

	brc100Data.Challenges = append(brc100Data.Challenges, *challenge)

	if err := wm.SaveBRC100Data(); err != nil {
		return fmt.Errorf("failed to save BRC-100 challenge: %v", err)
	}

	wm.logger.WithField("challengeId", challenge.ChallengeID).Info("BRC-100 challenge added successfully")
	return nil
}

// GetBRC100IdentityByID retrieves a BRC-100 identity by ID
func (wm *WalletManager) GetBRC100IdentityByID(identityID string) (*BRC100Identity, error) {
	brc100Data, err := wm.GetBRC100Data()
	if err != nil {
		return nil, err
	}

	for _, identity := range brc100Data.Identities {
		if identity.ID == identityID {
			return &identity, nil
		}
	}

	return nil, fmt.Errorf("identity with ID %s not found", identityID)
}

// GetBRC100SessionByID retrieves a BRC-100 session by ID
func (wm *WalletManager) GetBRC100SessionByID(sessionID string) (*BRC100Session, error) {
	brc100Data, err := wm.GetBRC100Data()
	if err != nil {
		return nil, err
	}

	for _, session := range brc100Data.Sessions {
		if session.SessionID == sessionID {
			return &session, nil
		}
	}

	return nil, fmt.Errorf("session with ID %s not found", sessionID)
}

// GetBRC100ChallengeByID retrieves a BRC-100 challenge by ID
func (wm *WalletManager) GetBRC100ChallengeByID(challengeID string) (*BRC100Challenge, error) {
	brc100Data, err := wm.GetBRC100Data()
	if err != nil {
		return nil, err
	}

	for _, challenge := range brc100Data.Challenges {
		if challenge.ChallengeID == challengeID {
			return &challenge, nil
		}
	}

	return nil, fmt.Errorf("challenge with ID %s not found", challengeID)
}

// CleanupExpiredBRC100Data removes expired sessions and challenges
func (wm *WalletManager) CleanupExpiredBRC100Data() error {
	brc100Data, err := wm.GetBRC100Data()
	if err != nil {
		return err
	}

	now := time.Now()
	cleaned := false

	// Clean up expired sessions
	var activeSessions []BRC100Session
	for _, session := range brc100Data.Sessions {
		if session.ExpiresAt.After(now) {
			activeSessions = append(activeSessions, session)
		} else {
			cleaned = true
			wm.logger.WithField("sessionId", session.SessionID).Info("Removed expired BRC-100 session")
		}
	}
	brc100Data.Sessions = activeSessions

	// Clean up expired challenges
	var activeChallenges []BRC100Challenge
	for _, challenge := range brc100Data.Challenges {
		if challenge.ExpiresAt.After(now) {
			activeChallenges = append(activeChallenges, challenge)
		} else {
			cleaned = true
			wm.logger.WithField("challengeId", challenge.ChallengeID).Info("Removed expired BRC-100 challenge")
		}
	}
	brc100Data.Challenges = activeChallenges

	if cleaned {
		if err := wm.SaveBRC100Data(); err != nil {
			return fmt.Errorf("failed to save cleaned BRC-100 data: %v", err)
		}
		wm.logger.Info("BRC-100 expired data cleanup completed")
	}

	return nil
}
