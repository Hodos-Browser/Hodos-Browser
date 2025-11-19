package main

import (
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"time"

	"github.com/sirupsen/logrus"
)

// UTXOManager handles UTXO fetching and management
type UTXOManager struct {
	logger *logrus.Logger
	client *http.Client
}

// NewUTXOManager creates a new UTXO manager
func NewUTXOManager() *UTXOManager {
	logger := logrus.New()
	logger.SetLevel(logrus.InfoLevel)

	return &UTXOManager{
		logger: logger,
		client: &http.Client{
			Timeout: 30 * time.Second,
		},
	}
}

// BSV API response structures for different APIs
type WhatsOnChainResponse []WhatsOnChainUTXO
type WhatsOnChainUTXO struct {
	TxID   string `json:"tx_hash"`
	Vout   uint32 `json:"tx_pos"`
	Value  int64  `json:"value"`
	Script string `json:"script"`
}

type BitailsResponse struct {
	Data []BitailsUTXO `json:"data"`
}
type BitailsUTXO struct {
	TxID   string `json:"txid"`
	Vout   uint32 `json:"vout"`
	Value  int64  `json:"value"`
	Script string `json:"script"`
}

// FetchUTXOs fetches UTXOs for a given Bitcoin SV address
func (um *UTXOManager) FetchUTXOs(address string) ([]UTXO, error) {
	um.logger.Infof("Fetching UTXOs for address: %s", address)

	// Try WhatsOnChain API first (most reliable for BSV)
	utxos, err := um.fetchFromWhatsOnChain(address)
	if err != nil {
		um.logger.Warnf("WhatsOnChain API failed: %v", err)

		// Fallback to Bitails API
		utxos, err = um.fetchFromBitails(address)
		if err != nil {
			um.logger.Warnf("Bitails API failed: %v", err)

			// Final fallback - return empty UTXO list (address might have no UTXOs)
			um.logger.Info("No UTXOs found for address (this is normal for new addresses)")
			return []UTXO{}, nil
		}
	}

	um.logger.Infof("Successfully fetched %d UTXOs", len(utxos))
	return utxos, nil
}

// fetchFromWhatsOnChain fetches UTXOs from WhatsOnChain API
func (um *UTXOManager) fetchFromWhatsOnChain(address string) ([]UTXO, error) {
	url := "https://api.whatsonchain.com/v1/bsv/main/address/" + address + "/unspent"
	um.logger.Debugf("Fetching from WhatsOnChain: %s", url)

	resp, err := um.client.Get(url)
	if err != nil {
		return nil, fmt.Errorf("WhatsOnChain API request failed: %v", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return nil, fmt.Errorf("WhatsOnChain API returned status %d", resp.StatusCode)
	}

	body, err := io.ReadAll(resp.Body)
	if err != nil {
		return nil, fmt.Errorf("failed to read WhatsOnChain response: %v", err)
	}

	// Parse WhatsOnChain response
	var apiResp WhatsOnChainResponse
	if err := json.Unmarshal(body, &apiResp); err != nil {
		return nil, fmt.Errorf("failed to parse WhatsOnChain response: %v", err)
	}

	// Convert to our UTXO format
	var utxos []UTXO
	for _, bsvUTXO := range apiResp {
		utxos = append(utxos, UTXO{
			TxID:    bsvUTXO.TxID,
			Vout:    bsvUTXO.Vout,
			Amount:  bsvUTXO.Value,
			Script:  bsvUTXO.Script,
			Address: address, // Set the address field
		})
	}

	return utxos, nil
}

// fetchFromBitails fetches UTXOs from Bitails API
func (um *UTXOManager) fetchFromBitails(address string) ([]UTXO, error) {
	url := "https://api.bitails.io/address/" + address + "/utxo"
	um.logger.Debugf("Fetching from Bitails: %s", url)

	resp, err := um.client.Get(url)
	if err != nil {
		return nil, fmt.Errorf("Bitails API request failed: %v", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return nil, fmt.Errorf("Bitails API returned status %d", resp.StatusCode)
	}

	body, err := io.ReadAll(resp.Body)
	if err != nil {
		return nil, fmt.Errorf("failed to read Bitails response: %v", err)
	}

	// Parse Bitails response
	var apiResp BitailsResponse
	if err := json.Unmarshal(body, &apiResp); err != nil {
		return nil, fmt.Errorf("failed to parse Bitails response: %v", err)
	}

	// Convert to our UTXO format
	var utxos []UTXO
	for _, bsvUTXO := range apiResp.Data {
		utxos = append(utxos, UTXO{
			TxID:    bsvUTXO.TxID,
			Vout:    bsvUTXO.Vout,
			Amount:  bsvUTXO.Value,
			Script:  bsvUTXO.Script,
			Address: address, // Set the address field
		})
	}

	return utxos, nil
}

// SelectUTXOs implements coin selection algorithm
func (um *UTXOManager) SelectUTXOs(utxos []UTXO, targetAmount int64, feeRate int64) ([]UTXO, int64, error) {
	um.logger.Infof("Selecting UTXOs for amount %d satoshis with fee rate %d sat/byte", targetAmount, feeRate)

	if len(utxos) == 0 {
		return nil, 0, fmt.Errorf("no UTXOs available")
	}

	// Simple largest-first coin selection algorithm
	// TODO: Implement more sophisticated algorithms (smallest-first, random, etc.)
	selectedUTXOs := make([]UTXO, 0)
	totalSelected := int64(0)

	// Sort UTXOs by amount (largest first)
	// This is a simple implementation - in production, you'd want more sophisticated sorting
	for _, utxo := range utxos {
		selectedUTXOs = append(selectedUTXOs, utxo)
		totalSelected += utxo.Amount

		// Estimate transaction size and calculate fee
		estimatedSize := um.estimateTransactionSize(len(selectedUTXOs), 2) // 2 outputs (recipient + change)
		estimatedFee := int64(estimatedSize) * feeRate

		// Check if we have enough to cover target amount + fee
		if totalSelected >= targetAmount+estimatedFee {
			break
		}
	}

	// Check if we have enough funds
	estimatedSize := um.estimateTransactionSize(len(selectedUTXOs), 2)
	estimatedFee := int64(estimatedSize) * feeRate

	if totalSelected < targetAmount+estimatedFee {
		return nil, 0, fmt.Errorf("insufficient funds: need %d, have %d", targetAmount+estimatedFee, totalSelected)
	}

	um.logger.Infof("Selected %d UTXOs totaling %d satoshis with estimated fee %d",
		len(selectedUTXOs), totalSelected, estimatedFee)

	return selectedUTXOs, estimatedFee, nil
}

// estimateTransactionSize estimates transaction size in bytes
func (um *UTXOManager) estimateTransactionSize(inputCount, outputCount int) int {
	// Base transaction size: 4 bytes version + 4 bytes locktime
	baseSize := 8

	// Input size: 32 bytes txid + 4 bytes vout + 4 bytes script length + 107 bytes script + 4 bytes sequence
	inputSize := 32 + 4 + 4 + 107 + 4

	// Output size: 8 bytes value + 1 byte script length + 25 bytes script (P2PKH)
	outputSize := 8 + 1 + 25

	// Varint encoding overhead (rough estimate)
	varintOverhead := 2

	totalSize := baseSize + (inputCount * inputSize) + (outputCount * outputSize) + varintOverhead

	um.logger.Debugf("Estimated transaction size: %d bytes (%d inputs, %d outputs)",
		totalSize, inputCount, outputCount)

	return totalSize
}

// GetBalance calculates total balance from UTXOs
func (um *UTXOManager) GetBalance(utxos []UTXO) int64 {
	total := int64(0)
	for _, utxo := range utxos {
		total += utxo.Amount
	}
	return total
}

// FetchTransaction fetches a transaction by its ID
func (um *UTXOManager) FetchTransaction(txID string) (string, error) {
	// Try WhatsOnChain first
	url := fmt.Sprintf("https://api.whatsonchain.com/v1/bsv/main/tx/%s/hex", txID)

	um.logger.Infof("Fetching transaction from WhatsOnChain: %s", txID)
	um.logger.Infof("URL: %s", url)

	// Create a request with timeout
	req, err := http.NewRequest("GET", url, nil)
	if err != nil {
		return "", fmt.Errorf("failed to create request: %v", err)
	}

	// Set a shorter timeout for this specific request
	client := &http.Client{
		Timeout: 10 * time.Second,
	}

	resp, err := client.Do(req)
	if err != nil {
		return "", fmt.Errorf("failed to fetch transaction from WhatsOnChain: %v", err)
	}
	defer resp.Body.Close()

	um.logger.Infof("WhatsOnChain response status: %d", resp.StatusCode)

	if resp.StatusCode == http.StatusTooManyRequests {
		// Rate limited, wait and retry once
		um.logger.Infof("Rate limited by WhatsOnChain, waiting 2 seconds before retry...")
		time.Sleep(2 * time.Second)

		// Create a new request for the retry
		req2, err := http.NewRequest("GET", url, nil)
		if err != nil {
			return "", fmt.Errorf("failed to create retry request: %v", err)
		}

		resp, err = client.Do(req2)
		if err != nil {
			return "", fmt.Errorf("failed to fetch transaction from WhatsOnChain on retry: %v", err)
		}
		defer resp.Body.Close()

		um.logger.Infof("WhatsOnChain retry response status: %d", resp.StatusCode)
	}

	if resp.StatusCode != http.StatusOK {
		body, _ := io.ReadAll(resp.Body)
		return "", fmt.Errorf("WhatsOnChain returned status %d for transaction %s: %s", resp.StatusCode, txID, string(body))
	}

	body, err := io.ReadAll(resp.Body)
	if err != nil {
		return "", fmt.Errorf("failed to read response body: %v", err)
	}

	// WhatsOnChain returns the hex directly
	txHex := string(body)
	um.logger.Infof("Successfully fetched transaction %s (%d bytes)", txID, len(txHex))

	return txHex, nil
}
