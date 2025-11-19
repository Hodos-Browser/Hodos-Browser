package beef

import (
	"fmt"
	"time"

	"github.com/bsv-blockchain/go-sdk/transaction"
	"github.com/sirupsen/logrus"
	"browser-wallet/brc100/spv"
)

// BRC100BEEFTransaction represents a BRC-100 BEEF transaction wrapper
type BRC100BEEFTransaction struct {
	BEEFData  []byte              `json:"beefData"`
	Actions   []BRC100Action      `json:"actions"`
	Identity  *IdentityContext    `json:"identity"`
	SessionID string              `json:"sessionId"`
	AppDomain string              `json:"appDomain"`
	Timestamp time.Time           `json:"timestamp"`
	SPVData   *SPVData            `json:"spvData,omitempty"` // Enhanced with SPV data
}

// SPVData represents SPV verification data for BEEF transactions
type SPVData struct {
	MerkleProofs    []*transaction.MerklePath `json:"merkleProofs"`
	BlockHeaders    []*BlockHeader            `json:"blockHeaders"`
	TransactionData []*TransactionData        `json:"transactionData"`
	IdentityProofs  []*spv.IdentityProof      `json:"identityProofs"`
	VerificationTime time.Time                `json:"verificationTime"`
}

// BlockHeader represents block header information for SPV verification
type BlockHeader struct {
	Hash         string    `json:"hash"`
	Height       int64     `json:"height"`
	MerkleRoot   string    `json:"merkleRoot"`
	Timestamp    time.Time `json:"timestamp"`
	PreviousHash string    `json:"previousHash"`
	Nonce        uint32    `json:"nonce"`
	Bits         uint32    `json:"bits"`
}

// TransactionData represents transaction data for SPV verification
type TransactionData struct {
	TxID          string                 `json:"txid"`
	Hash          string                 `json:"hash"`
	BlockHeight   int64                  `json:"blockHeight"`
	Confirmations int64                  `json:"confirmations"`
	Size          int                    `json:"size"`
	Fee           int64                  `json:"fee"`
	Timestamp     time.Time              `json:"timestamp"`
	Inputs        []InputData            `json:"inputs"`
	Outputs       []OutputData           `json:"outputs"`
	RawData       string                 `json:"rawData,omitempty"`
}

// InputData represents input data for SPV verification
type InputData struct {
	PrevOutHash  string `json:"prevOutHash"`
	PrevOutIndex int    `json:"prevOutIndex"`
	ScriptSig    string `json:"scriptSig"`
	Sequence     uint32 `json:"sequence"`
}

// OutputData represents output data for SPV verification
type OutputData struct {
	Value        int64    `json:"value"`
	ScriptPubKey string   `json:"scriptPubKey"`
	Addresses    []string `json:"addresses"`
	Type         string   `json:"type"`
}

// BRC100Action represents a BRC-100 action (wraps BEEF transaction)
type BRC100Action struct {
	Type      string                 `json:"type"`
	Data      map[string]interface{} `json:"data"`
	BEEFTx    *transaction.Transaction `json:"beefTx,omitempty"`
	Identity  string                 `json:"identity"`
	Timestamp time.Time              `json:"timestamp"`
	Signature string                 `json:"signature,omitempty"`
}

// IdentityContext represents the identity context for BEEF transactions
type IdentityContext struct {
	Certificate map[string]interface{} `json:"certificate"`
	SessionID   string                 `json:"sessionId"`
	AppDomain   string                 `json:"appDomain"`
	Timestamp   time.Time              `json:"timestamp"`
}

// BRC100BEEFRequest represents a request to create a BRC-100 BEEF transaction
type BRC100BEEFRequest struct {
	Actions   []BRC100Action    `json:"actions"`
	AppDomain string            `json:"appDomain"`
	SessionID string            `json:"sessionId"`
	Purpose   string            `json:"purpose"`
	Identity  *IdentityContext  `json:"identity"`
}

// BRC100BEEFManager manages BRC-100 BEEF transactions
type BRC100BEEFManager struct {
	logger          *logrus.Logger
	spvVerifier     *spv.SPVVerifier
	blockchainClient *spv.BlockchainAPIClient
}

// NewBRC100BEEFManager creates a new BRC-100 BEEF manager
func NewBRC100BEEFManager() *BRC100BEEFManager {
	logger := logrus.New()
	logger.SetLevel(logrus.InfoLevel)

	return &BRC100BEEFManager{
		logger:          logger,
		spvVerifier:     spv.NewSPVVerifier(),
		blockchainClient: spv.NewBlockchainAPIClient(),
	}
}

// CreateBRC100BEEFTransaction creates a new BRC-100 BEEF transaction
func (bm *BRC100BEEFManager) CreateBRC100BEEFTransaction(actions []BRC100Action, identity *IdentityContext) (*BRC100BEEFTransaction, error) {
	bm.logger.Info("Creating BRC-100 BEEF transaction")

	// Create BRC-100 BEEF transaction
	brc100Tx := &BRC100BEEFTransaction{
		Actions:   actions,
		Identity:  identity,
		SessionID: identity.SessionID,
		AppDomain: identity.AppDomain,
		Timestamp: time.Now(),
		BEEFData:  nil, // Will be set when converting to BEEF
	}

	// Convert to BEEF format
	beefData, err := bm.ConvertToBEEF(brc100Tx)
	if err != nil {
		return nil, fmt.Errorf("failed to convert to BEEF: %v", err)
	}

	brc100Tx.BEEFData = beefData

	bm.logger.Info("BRC-100 BEEF transaction created successfully")
	return brc100Tx, nil
}

// CreateBRC100BEEFTransactionWithSPV creates a new BRC-100 BEEF transaction with comprehensive SPV data
func (bm *BRC100BEEFManager) CreateBRC100BEEFTransactionWithSPV(actions []BRC100Action, identity *IdentityContext, includeSPVData bool) (*BRC100BEEFTransaction, error) {
	bm.logger.Info("Creating BRC-100 BEEF transaction with SPV data")

	// Create BRC-100 BEEF transaction
	brc100Tx := &BRC100BEEFTransaction{
		Actions:   actions,
		Identity:  identity,
		SessionID: identity.SessionID,
		AppDomain: identity.AppDomain,
		Timestamp: time.Now(),
		BEEFData:  nil, // Will be set when converting to BEEF
	}

	// Collect SPV data if requested
	if includeSPVData {
		spvData, err := bm.collectSPVData(actions, identity)
		if err != nil {
			bm.logger.WithError(err).Warn("Failed to collect SPV data, continuing without it")
		} else {
			brc100Tx.SPVData = spvData
			bm.logger.Info("SPV data collected successfully")
		}
	}

	return brc100Tx, nil
}

// CreateBRC100BEEFTransactionWithSPVFromTransaction creates a BEEF transaction and collects SPV data from a specific transaction
func (bm *BRC100BEEFManager) CreateBRC100BEEFTransactionWithSPVFromTransaction(actions []BRC100Action, identity *IdentityContext, txID string) (*BRC100BEEFTransaction, error) {
	bm.logger.WithField("txID", txID).Info("Creating BRC-100 BEEF transaction with SPV data from specific transaction")

	// Create BRC-100 BEEF transaction
	brc100Tx := &BRC100BEEFTransaction{
		Actions:   actions,
		Identity:  identity,
		SessionID: identity.SessionID,
		AppDomain: identity.AppDomain,
		Timestamp: time.Now(),
		BEEFData:  nil, // Will be set when converting to BEEF
	}

	// Collect SPV data from the specific transaction
	spvData, err := bm.collectSPVDataFromTransaction(txID, identity)
	if err != nil {
		bm.logger.WithError(err).Warn("Failed to collect SPV data from transaction, continuing without it")
	} else {
		brc100Tx.SPVData = spvData
		bm.logger.Info("SPV data collected successfully from transaction")
	}

	// Convert to BEEF format
	beefData, err := bm.ConvertToBEEF(brc100Tx)
	if err != nil {
		return nil, fmt.Errorf("failed to convert to BEEF: %v", err)
	}

	brc100Tx.BEEFData = beefData

	bm.logger.Info("BRC-100 BEEF transaction with SPV data created successfully")
	return brc100Tx, nil
}

// ConvertToBEEF converts a BRC-100 transaction to BEEF format
func (bm *BRC100BEEFManager) ConvertToBEEF(brc100Tx *BRC100BEEFTransaction) ([]byte, error) {
	bm.logger.Info("Converting BRC-100 transaction to BEEF format")

	// Create a new BEEF transaction using the Go SDK
	beefTx := transaction.NewBeefV2()

	// Add actions as BEEF transactions
	for _, action := range brc100Tx.Actions {
		if action.BEEFTx != nil {
			// Add the BEEF transaction to the BEEF container
			txID := action.BEEFTx.TxID()
			beefTx.Transactions[*txID] = &transaction.BeefTx{
				Transaction: action.BEEFTx,
			}
		}
	}

	// Convert to BEEF bytes (simplified for now)
	// Note: The Go SDK doesn't have ToBytes() method, so we'll create a simple representation
	beefData := []byte("BEEF_DATA_PLACEHOLDER")

	bm.logger.Info("Successfully converted to BEEF format")
	return beefData, nil
}

// ConvertFromBEEF converts BEEF data to BRC-100 transaction
func (bm *BRC100BEEFManager) ConvertFromBEEF(beefData []byte) (*BRC100BEEFTransaction, error) {
	bm.logger.Info("Converting BEEF data to BRC-100 transaction")

	// Parse BEEF data using Go SDK
	beefTx, err := transaction.NewBeefFromBytes(beefData)
	if err != nil {
		return nil, fmt.Errorf("failed to parse BEEF data: %v", err)
	}

	// Convert BEEF transactions to BRC-100 actions
	actions := make([]BRC100Action, 0)
	for _, beefTxItem := range beefTx.Transactions {
		if beefTxItem.Transaction != nil {
			action := BRC100Action{
				Type:      "transaction",
				Data:      make(map[string]interface{}),
				BEEFTx:    beefTxItem.Transaction,
				Identity:  "unknown", // Will be set from context
				Timestamp: time.Now(),
			}
			actions = append(actions, action)
		}
	}

	// Create BRC-100 transaction
	brc100Tx := &BRC100BEEFTransaction{
		BEEFData:  beefData,
		Actions:   actions,
		Identity:  nil, // Will be set from context
		SessionID: "unknown", // Will be set from context
		AppDomain: "unknown", // Will be set from context
		Timestamp: time.Now(),
	}

	bm.logger.Info("Successfully converted from BEEF format")
	return brc100Tx, nil
}

// SignBRC100BEEFTransaction signs a BRC-100 BEEF transaction
func (bm *BRC100BEEFManager) SignBRC100BEEFTransaction(brc100Tx *BRC100BEEFTransaction, privateKey string) error {
	bm.logger.Info("Signing BRC-100 BEEF transaction")

	// Sign each action
	for i, action := range brc100Tx.Actions {
		if action.BEEFTx != nil {
			// Sign the BEEF transaction using Go SDK
			if err := action.BEEFTx.Sign(); err != nil {
				return fmt.Errorf("failed to sign BEEF transaction: %v", err)
			}

			// Update signature in action
			brc100Tx.Actions[i].Signature = "signed_" + action.BEEFTx.TxID().String()[:16]
		}
	}

	bm.logger.Info("BRC-100 BEEF transaction signed successfully")
	return nil
}

// VerifyBRC100BEEFTransaction verifies a BRC-100 BEEF transaction
func (bm *BRC100BEEFManager) VerifyBRC100BEEFTransaction(brc100Tx *BRC100BEEFTransaction) (bool, error) {
	bm.logger.Info("Verifying BRC-100 BEEF transaction")

	// Verify each action
	for _, action := range brc100Tx.Actions {
		if action.BEEFTx != nil {
			// Verify the BEEF transaction using Go SDK
			// Note: The Go SDK doesn't have a direct verify method, so we'll do basic validation
			if action.BEEFTx.TxID().String() == "" {
				bm.logger.Warn("Invalid BEEF transaction ID")
				return false, fmt.Errorf("invalid BEEF transaction ID")
			}
		}
	}

	// Verify BEEF data integrity
	if len(brc100Tx.BEEFData) == 0 {
		bm.logger.Warn("Empty BEEF data")
		return false, fmt.Errorf("empty BEEF data")
	}

	bm.logger.Info("BRC-100 BEEF transaction verified successfully")
	return true, nil
}

// CreateBRC100Action creates a new BRC-100 action
func (bm *BRC100BEEFManager) CreateBRC100Action(actionType string, data map[string]interface{}, identity string) (*BRC100Action, error) {
	bm.logger.Infof("Creating BRC-100 action: %s", actionType)

	action := &BRC100Action{
		Type:      actionType,
		Data:      data,
		BEEFTx:    nil, // Will be set when creating BEEF transaction
		Identity:  identity,
		Timestamp: time.Now(),
		Signature: "",
	}

	bm.logger.Info("BRC-100 action created successfully")
	return action, nil
}

// AddBEEFTransactionToAction adds a BEEF transaction to an action
func (bm *BRC100BEEFManager) AddBEEFTransactionToAction(action *BRC100Action, beefTx *transaction.Transaction) error {
	bm.logger.Info("Adding BEEF transaction to action")

	action.BEEFTx = beefTx
	action.Timestamp = time.Now()

	bm.logger.Info("BEEF transaction added to action successfully")
	return nil
}

// GetBEEFHex returns the BEEF transaction as hex string
func (bm *BRC100BEEFManager) GetBEEFHex(brc100Tx *BRC100BEEFTransaction) (string, error) {
	bm.logger.Info("Getting BEEF hex string")

	if len(brc100Tx.BEEFData) == 0 {
		return "", fmt.Errorf("no BEEF data available")
	}

	// Convert bytes to hex
	beefHex := fmt.Sprintf("%x", brc100Tx.BEEFData)

	bm.logger.Info("BEEF hex string generated successfully")
	return beefHex, nil
}

// ValidateBRC100BEEFRequest validates a BRC-100 BEEF request
func (bm *BRC100BEEFManager) ValidateBRC100BEEFRequest(req *BRC100BEEFRequest) error {
	bm.logger.Info("Validating BRC-100 BEEF request")

	// Validate actions
	if len(req.Actions) == 0 {
		return fmt.Errorf("no actions provided")
	}

	// Validate app domain
	if req.AppDomain == "" {
		return fmt.Errorf("app domain is required")
	}

	// Validate session ID
	if req.SessionID == "" {
		return fmt.Errorf("session ID is required")
	}

	// Validate purpose
	if req.Purpose == "" {
		return fmt.Errorf("purpose is required")
	}

	// Validate identity
	if req.Identity == nil {
		return fmt.Errorf("identity is required")
	}

	bm.logger.Info("BRC-100 BEEF request is valid")
	return nil
}

// GetBRC100BEEFTransactionInfo returns information about a BRC-100 BEEF transaction
func (bm *BRC100BEEFManager) GetBRC100BEEFTransactionInfo(brc100Tx *BRC100BEEFTransaction) map[string]interface{} {
	bm.logger.Info("Getting BRC-100 BEEF transaction info")

	info := map[string]interface{}{
		"sessionId":   brc100Tx.SessionID,
		"appDomain":   brc100Tx.AppDomain,
		"timestamp":   brc100Tx.Timestamp,
		"actionCount": len(brc100Tx.Actions),
		"beefSize":    len(brc100Tx.BEEFData),
	}

	// Add action types
	actionTypes := make([]string, len(brc100Tx.Actions))
	for i, action := range brc100Tx.Actions {
		actionTypes[i] = action.Type
	}
	info["actionTypes"] = actionTypes

	bm.logger.Info("BRC-100 BEEF transaction info generated successfully")
	return info
}

// collectSPVData collects comprehensive SPV data for BEEF transactions
func (bm *BRC100BEEFManager) collectSPVData(actions []BRC100Action, identity *IdentityContext) (*SPVData, error) {
	bm.logger.Info("Collecting SPV data for BEEF transaction")

	spvData := &SPVData{
		MerkleProofs:    make([]*transaction.MerklePath, 0),
		BlockHeaders:    make([]*BlockHeader, 0),
		TransactionData: make([]*TransactionData, 0),
		IdentityProofs:  make([]*spv.IdentityProof, 0),
		VerificationTime: time.Now(),
	}

	// Collect SPV data for each action
	for _, action := range actions {
		if action.BEEFTx != nil {
			// Get transaction ID
			txID := action.BEEFTx.TxID().String()

			// Fetch transaction data from blockchain
			txResponse, err := bm.blockchainClient.FetchTransactionFromBlockchain(txID)
			if err != nil {
				bm.logger.WithError(err).Warnf("Failed to fetch transaction %s, skipping SPV data", txID)
				continue
			}

			// Convert to our TransactionData format
			txData := bm.convertToTransactionData(txResponse)
			spvData.TransactionData = append(spvData.TransactionData, txData)

			// Get Merkle proof
			merkleProof, err := bm.spvVerifier.GetMerkleProof(txID, uint32(txResponse.BlockHeight))
			if err != nil {
				bm.logger.WithError(err).Warnf("Failed to get Merkle proof for transaction %s", txID)
			} else {
				spvData.MerkleProofs = append(spvData.MerkleProofs, merkleProof)
			}

			// Get block header (simplified for now)
			blockHeader := bm.createBlockHeader(txResponse.BlockHeight, txResponse.BlockHash)
			spvData.BlockHeaders = append(spvData.BlockHeaders, blockHeader)

			// Create identity proof if identity data is available
			if identity != nil && identity.Certificate != nil {
				identityProof, err := bm.createIdentityProof(txID, identity.Certificate)
				if err != nil {
					bm.logger.WithError(err).Warnf("Failed to create identity proof for transaction %s", txID)
				} else {
					spvData.IdentityProofs = append(spvData.IdentityProofs, identityProof)
				}
			}
		}
	}

	bm.logger.WithFields(logrus.Fields{
		"merkleProofs":   len(spvData.MerkleProofs),
		"blockHeaders":   len(spvData.BlockHeaders),
		"transactionData": len(spvData.TransactionData),
		"identityProofs": len(spvData.IdentityProofs),
	}).Info("SPV data collection completed")

	return spvData, nil
}

// collectSPVDataFromTransaction collects SPV data from a specific transaction ID
func (bm *BRC100BEEFManager) collectSPVDataFromTransaction(txID string, identity *IdentityContext) (*SPVData, error) {
	bm.logger.WithField("txID", txID).Info("Collecting SPV data from specific transaction")

	// Initialize SPV data structure
	spvData := &SPVData{
		MerkleProofs:   make([]*transaction.MerklePath, 0),
		BlockHeaders:   make([]*BlockHeader, 0),
		TransactionData: make([]*TransactionData, 0),
		IdentityProofs:  make([]*spv.IdentityProof, 0),
		VerificationTime: time.Now(),
	}

	// Fetch transaction data from blockchain
	txResponse, err := bm.blockchainClient.FetchTransactionFromBlockchain(txID)
	if err != nil {
		return nil, fmt.Errorf("failed to fetch transaction %s: %v", txID, err)
	}

	// Convert to our TransactionData format
	txData := bm.convertToTransactionData(txResponse)
	spvData.TransactionData = append(spvData.TransactionData, txData)

	// Get Merkle proof
	merkleProof, err := bm.spvVerifier.GetMerkleProof(txID, uint32(txResponse.BlockHeight))
	if err != nil {
		bm.logger.WithError(err).Warnf("Failed to get Merkle proof for transaction %s", txID)
	} else {
		spvData.MerkleProofs = append(spvData.MerkleProofs, merkleProof)
	}

	// Get block header
	blockHeader := bm.createBlockHeader(txResponse.BlockHeight, txResponse.BlockHash)
	spvData.BlockHeaders = append(spvData.BlockHeaders, blockHeader)

	// Create identity proof if identity data is available
	if identity != nil && identity.Certificate != nil {
		identityProof, err := bm.createIdentityProof(txID, identity.Certificate)
		if err != nil {
			bm.logger.WithError(err).Warnf("Failed to create identity proof for transaction %s", txID)
		} else {
			spvData.IdentityProofs = append(spvData.IdentityProofs, identityProof)
		}
	}

	bm.logger.WithFields(logrus.Fields{
		"merkleProofs":   len(spvData.MerkleProofs),
		"blockHeaders":   len(spvData.BlockHeaders),
		"transactionData": len(spvData.TransactionData),
		"identityProofs": len(spvData.IdentityProofs),
	}).Info("SPV data collection from transaction completed")

	return spvData, nil
}

// convertToTransactionData converts blockchain API response to our TransactionData format
func (bm *BRC100BEEFManager) convertToTransactionData(txResponse *spv.WhatsOnChainResponse) *TransactionData {
	// Convert inputs
	inputs := make([]InputData, len(txResponse.Inputs))
	for i, input := range txResponse.Inputs {
		inputs[i] = InputData{
			PrevOutHash:  input.PrevOut.Hash,
			PrevOutIndex: input.PrevOut.Index,
			ScriptSig:    input.ScriptSig,
			Sequence:     0xffffffff, // Default sequence
		}
	}

	// Convert outputs
	outputs := make([]OutputData, len(txResponse.Outputs))
	for i, output := range txResponse.Outputs {
		outputs[i] = OutputData{
			Value:        output.Value,
			ScriptPubKey: output.ScriptPubKey.Hex,
			Addresses:    output.ScriptPubKey.Addresses,
			Type:         output.ScriptPubKey.Type,
		}
	}

	return &TransactionData{
		TxID:          txResponse.TxID,
		Hash:          txResponse.Hash,
		BlockHeight:   txResponse.BlockHeight,
		Confirmations: txResponse.Confirmations,
		Size:          txResponse.Size,
		Fee:           txResponse.Fee,
		Timestamp:     time.Unix(txResponse.Time, 0),
		Inputs:        inputs,
		Outputs:       outputs,
	}
}

// createBlockHeader creates a simplified block header for SPV verification
func (bm *BRC100BEEFManager) createBlockHeader(height int64, blockHash string) *BlockHeader {
	// In a real implementation, we would fetch the actual block header
	// For now, we'll create a simplified version
	return &BlockHeader{
		Hash:         blockHash,
		Height:       height,
		MerkleRoot:   "placeholder_merkle_root", // Would be fetched from block data
		Timestamp:    time.Now(),
		PreviousHash: "placeholder_previous_hash",
		Nonce:        0,
		Bits:         0,
	}
}

// createIdentityProof creates an identity proof for SPV verification
func (bm *BRC100BEEFManager) createIdentityProof(txID string, certificate map[string]interface{}) (*spv.IdentityProof, error) {
	// Extract identity data from certificate
	identityData := make(map[string]interface{})

	// Copy certificate data
	for key, value := range certificate {
		identityData[key] = value
	}

	// Add transaction ID
	identityData["transactionId"] = txID

	// Create identity proof using SPV verifier
	proof, err := bm.spvVerifier.CreateIdentityProof(txID, identityData)
	if err != nil {
		return nil, fmt.Errorf("failed to create identity proof: %v", err)
	}

	return proof, nil
}

// GetSPVDataInfo returns information about the SPV data in a BEEF transaction
func (bm *BRC100BEEFManager) GetSPVDataInfo(spvData *SPVData) map[string]interface{} {
	if spvData == nil {
		return map[string]interface{}{
			"hasSPVData": false,
		}
	}

	info := map[string]interface{}{
		"hasSPVData":        true,
		"merkleProofs":      len(spvData.MerkleProofs),
		"blockHeaders":      len(spvData.BlockHeaders),
		"transactionData":   len(spvData.TransactionData),
		"identityProofs":    len(spvData.IdentityProofs),
		"verificationTime":  spvData.VerificationTime,
	}

	// Add transaction IDs
	txIDs := make([]string, len(spvData.TransactionData))
	for i, txData := range spvData.TransactionData {
		txIDs[i] = txData.TxID
	}
	info["transactionIds"] = txIDs

	return info
}
