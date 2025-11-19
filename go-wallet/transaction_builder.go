package main

import (
	"encoding/hex"
	"fmt"

	ec "github.com/bsv-blockchain/go-sdk/primitives/ec"
	"github.com/bsv-blockchain/go-sdk/script"
	"github.com/bsv-blockchain/go-sdk/transaction"
	"github.com/bsv-blockchain/go-sdk/transaction/template/p2pkh"
	"github.com/sirupsen/logrus"
)

// TransactionBuilder handles Bitcoin SV transaction creation and signing
type TransactionBuilder struct {
	walletService *WalletService
	utxoManager   *UTXOManager
	logger        *logrus.Logger
}

// NewTransactionBuilder creates a new transaction builder
func NewTransactionBuilder(walletService *WalletService) *TransactionBuilder {
	logger := logrus.New()
	logger.SetLevel(logrus.InfoLevel)

	return &TransactionBuilder{
		walletService: walletService,
		utxoManager:   NewUTXOManager(),
		logger:        logger,
	}
}

// CreateTransaction creates an unsigned Bitcoin SV transaction
func (tb *TransactionBuilder) CreateTransaction(req *TransactionRequest) (*TransactionResponse, error) {
	tb.logger.Infof("Creating transaction: %s -> %d satoshis", req.RecipientAddress, req.Amount)

	// Determine sender address and fetch UTXOs
	var senderAddress string
	var utxos []UTXO
	var err error

	if req.SenderAddress != "" {
		senderAddress = req.SenderAddress
		tb.logger.Infof("Using provided sender address: %s", senderAddress)

		// Fetch UTXOs for specified sender address
		utxos, err = tb.utxoManager.FetchUTXOs(senderAddress)
		if err != nil {
			return nil, fmt.Errorf("failed to fetch UTXOs: %v", err)
		}
	} else {
		// Fetch UTXOs from ALL addresses in our wallet
		allAddresses := tb.walletService.walletManager.GetAllAddresses()
		var allUTXOs []UTXO

		for _, addr := range allAddresses {
			addrUTXOs, err := tb.utxoManager.FetchUTXOs(addr.Address)
			if err != nil {
				tb.logger.Warnf("Failed to fetch UTXOs for address %s: %v", addr.Address, err)
				continue
			}
			// UTXOs are now automatically tagged with their address in the UTXO manager
			allUTXOs = append(allUTXOs, addrUTXOs...)
		}

		utxos = allUTXOs
		tb.logger.Infof("Fetched %d UTXOs from %d addresses", len(utxos), len(allAddresses))

		// Use current address as sender for change output
		currentAddress, err := tb.walletService.walletManager.GetCurrentAddress()
		if err != nil {
			return nil, fmt.Errorf("failed to get current address for change output: %v", err)
		}
		senderAddress = currentAddress.Address
	}

	// Select UTXOs for transaction
	selectedUTXOs, fee, err := tb.utxoManager.SelectUTXOs(utxos, req.Amount, req.FeeRate)
	if err != nil {
		return nil, fmt.Errorf("failed to select UTXOs: %v", err)
	}

	// Calculate change amount
	totalInput := int64(0)
	for _, utxo := range selectedUTXOs {
		totalInput += utxo.Amount
	}
	changeAmount := totalInput - req.Amount - fee

	// Build transaction structure using BSV SDK
	tx := transaction.NewTransaction()

	// Add inputs following BSV SDK documentation exactly
	for _, utxo := range selectedUTXOs {
		tb.logger.Infof("Processing UTXO: %s:%d (amount: %d, address: %s)", utxo.TxID, utxo.Vout, utxo.Amount, utxo.Address)

		// Fetch the source transaction hex
		sourceTxHex, err := tb.utxoManager.FetchTransaction(utxo.TxID)
		if err != nil {
			return nil, fmt.Errorf("failed to fetch source transaction %s: %v", utxo.TxID, err)
		}

		// Parse the source transaction
		sourceTransaction, err := transaction.NewTransactionFromHex(sourceTxHex)
		if err != nil {
			return nil, fmt.Errorf("failed to parse source transaction: %v", err)
		}

		// Get the private key for this UTXO's address
		utxoPrivateKeyHex, err := tb.walletService.walletManager.GetPrivateKeyForAddress(utxo.Address)
		if err != nil {
			return nil, fmt.Errorf("failed to get private key for address %s: %v", utxo.Address, err)
		}

		// Parse the private key
		utxoPrivateKeyBytes, err := hex.DecodeString(utxoPrivateKeyHex)
		if err != nil {
			return nil, fmt.Errorf("invalid private key for address %s: %v", utxo.Address, err)
		}

		// Create private key from bytes using BSV SDK
		utxoPrivateKey, _ := ec.PrivateKeyFromBytes(utxoPrivateKeyBytes)

		// Create unlocking script template
		unlockingScriptTemplate, err := p2pkh.Unlock(utxoPrivateKey, nil)
		if err != nil {
			return nil, fmt.Errorf("failed to create unlocking script template: %v", err)
		}

		// Create transaction input following BSV SDK documentation
		txInput := &transaction.TransactionInput{
			SourceTXID:              sourceTransaction.TxID(),
			SourceTxOutIndex:        utxo.Vout,
			SourceTransaction:       sourceTransaction,
			UnlockingScriptTemplate: unlockingScriptTemplate,
			SequenceNumber:          transaction.DefaultSequenceNumber,
		}

		// Add input to transaction
		tx.AddInput(txInput)
	}

	// Add recipient output following BSV SDK documentation
	tb.logger.Infof("Creating recipient output for address: %s, amount: %d", req.RecipientAddress, req.Amount)

	// Create address from string
	recipientAddress, err := script.NewAddressFromString(req.RecipientAddress)
	if err != nil {
		return nil, fmt.Errorf("failed to parse recipient address: %v", err)
	}

	// Create P2PKH lock script
	recipientLockScript, err := p2pkh.Lock(recipientAddress)
	if err != nil {
		return nil, fmt.Errorf("failed to create recipient lock script: %v", err)
	}

	recipientOutput := &transaction.TransactionOutput{
		Satoshis:      uint64(req.Amount),
		LockingScript: recipientLockScript,
	}
	tx.AddOutput(recipientOutput)

	// Add change output if needed
	if changeAmount > 0 {
		tb.logger.Infof("Creating change output for address: %s, amount: %d", senderAddress, changeAmount)

		// Create address from string
		changeAddress, err := script.NewAddressFromString(senderAddress)
		if err != nil {
			return nil, fmt.Errorf("failed to parse change address: %v", err)
		}

		// Create P2PKH lock script
		changeLockScript, err := p2pkh.Lock(changeAddress)
		if err != nil {
			return nil, fmt.Errorf("failed to create change lock script: %v", err)
		}

		changeOutput := &transaction.TransactionOutput{
			Satoshis:      uint64(changeAmount),
			LockingScript: changeLockScript,
		}
		tx.AddOutput(changeOutput)
	}

	// Serialize transaction to hex
	rawTx := tx.Hex()

	// Generate transaction ID (hash of raw transaction)
	txid := tx.TxID().String()

	// Log transaction structure
	tb.logger.Infof("Transaction has %d inputs and %d outputs", tx.InputCount(), tx.OutputCount())

	tb.logger.Infof("Transaction created successfully: %s", txid)

	// Store selected UTXOs and transaction object for signing
	response := &TransactionResponse{
		TxID:        txid,
		RawTx:       rawTx,
		Fee:         fee,
		Status:      "created",
		Broadcasted: false,
	}

	// Store the transaction object and UTXOs for signing
	tb.walletService.selectedUTXOs = selectedUTXOs
	tb.walletService.createdTransaction = tx // Store the transaction object with source transactions
	tb.logger.Infof("Stored %d selected UTXOs and transaction object for signing", len(selectedUTXOs))

	return response, nil
}

// SignTransaction signs a transaction using the BSV SDK's built-in signing
func (tb *TransactionBuilder) SignTransaction(rawTx string, privateKeyHex string, selectedUTXOs []UTXO) (*TransactionResponse, error) {
	tb.logger.Info("Signing transaction using BSV SDK")

	// Use the stored transaction object instead of parsing from hex
	tx := tb.walletService.createdTransaction
	if tx == nil {
		return nil, fmt.Errorf("no transaction object found for signing")
	}

	// Use the SDK's built-in signing method
	if err := tx.Sign(); err != nil {
		return nil, fmt.Errorf("failed to sign transaction: %v", err)
	}

	// Serialize signed transaction
	signedRawTx := tx.Hex()

	txid := tx.TxID().String()

	tb.logger.Infof("Transaction signed successfully: %s", txid)

	return &TransactionResponse{
		TxID:        txid,
		RawTx:       signedRawTx,
		Fee:         0, // Fee was calculated during creation
		Status:      "signed",
		Broadcasted: false,
	}, nil
}

// Helper methods

// findUTXOAddress finds which address owns a specific UTXO
func (tb *TransactionBuilder) findUTXOAddress(txInput *transaction.TransactionInput, selectedUTXOs []UTXO) string {
	// Convert transaction ID to hex (it's already in the correct format from the transaction)
	txIDHex := hex.EncodeToString(txInput.SourceTXID.CloneBytes())

	// Also try the reversed version since Bitcoin uses little-endian in raw transactions
	txIDBytes := txInput.SourceTXID.CloneBytes()
	reverseBytes(txIDBytes)
	txIDHexReversed := hex.EncodeToString(txIDBytes)

	tb.logger.Infof("Looking for UTXO: %s:%d (also trying reversed: %s:%d)", txIDHex, txInput.SourceTxOutIndex, txIDHexReversed, txInput.SourceTxOutIndex)
	tb.logger.Infof("Available UTXOs count: %d", len(selectedUTXOs))

	for i, utxo := range selectedUTXOs {
		tb.logger.Infof("UTXO %d: %s:%d (address: %s)", i, utxo.TxID, utxo.Vout, utxo.Address)
		if (utxo.TxID == txIDHex || utxo.TxID == txIDHexReversed) && utxo.Vout == txInput.SourceTxOutIndex {
			tb.logger.Infof("Found matching UTXO: %s:%d -> address: %s", utxo.TxID, utxo.Vout, utxo.Address)
			return utxo.Address
		}
	}

	tb.logger.Errorf("No matching UTXO found for %s:%d", txIDHex, txInput.SourceTxOutIndex)
	return "" // Should not happen if UTXO selection is correct
}

func (tb *TransactionBuilder) addressToScript(address string) (*script.Script, error) {
	tb.logger.Infof("Converting address to script: %s", address)

	// Convert Bitcoin SV address to script pubkey using BSV SDK
	tb.logger.Infof("Calling script.NewAddressFromString...")
	addr, err := script.NewAddressFromString(address)
	if err != nil {
		tb.logger.Errorf("Failed to parse address %s: %v", address, err)
		return nil, fmt.Errorf("failed to parse address %s: %v", address, err)
	}
	tb.logger.Infof("Address parsed successfully, public key hash: %x", addr.PublicKeyHash)

	// Create P2PKH script from address
	// P2PKH script: OP_DUP OP_HASH160 <pubKeyHash> OP_EQUALVERIFY OP_CHECKSIG
	tb.logger.Infof("Creating P2PKH script...")
	scriptBytes := []byte{0x76, 0xa9, 0x14} // OP_DUP OP_HASH160 OP_PUSHDATA20
	scriptBytes = append(scriptBytes, addr.PublicKeyHash...)
	scriptBytes = append(scriptBytes, 0x88, 0xac) // OP_EQUALVERIFY OP_CHECKSIG

	tb.logger.Infof("Creating script from bytes...")
	scriptObj := script.NewFromBytes(scriptBytes)

	tb.logger.Infof("Script created successfully")
	return scriptObj, nil
}

// Helper function to reverse byte slice (Bitcoin uses little-endian for transaction IDs)
func reverseBytes(data []byte) {
	for i, j := 0, len(data)-1; i < j; i, j = i+1, j-1 {
		data[i], data[j] = data[j], data[i]
	}
}

// getPreviousOutputScriptFromUTXO retrieves the script pubkey from stored UTXOs
func (tb *TransactionBuilder) getPreviousOutputScriptFromUTXO(txInput *transaction.TransactionInput, selectedUTXOs []UTXO) (*script.Script, error) {
	// Convert transaction ID to hex (it's already in the correct format from the transaction)
	txIDHex := hex.EncodeToString(txInput.SourceTXID.CloneBytes())

	// Also try the reversed version since Bitcoin uses little-endian in raw transactions
	txIDBytes := txInput.SourceTXID.CloneBytes()
	reverseBytes(txIDBytes)
	txIDHexReversed := hex.EncodeToString(txIDBytes)

	tb.logger.Infof("Looking for UTXO script: %s:%d (also trying reversed: %s:%d)", txIDHex, txInput.SourceTxOutIndex, txIDHexReversed, txInput.SourceTxOutIndex)

	// Find the matching UTXO in our stored UTXOs
	for _, utxo := range selectedUTXOs {
		if (utxo.TxID == txIDHex || utxo.TxID == txIDHexReversed) && utxo.Vout == txInput.SourceTxOutIndex {
			// Check if script is available
			if utxo.Script != "" {
				// Convert the script pubkey from hex to script object
				scriptBytes, err := hex.DecodeString(utxo.Script)
				if err != nil {
					return nil, fmt.Errorf("invalid script pubkey: %v", err)
				}
				tb.logger.Infof("Found UTXO script from stored data: %s", utxo.Script)
				return script.NewFromBytes(scriptBytes), nil
			} else {
				// Script is empty, derive it from the address
				tb.logger.Infof("UTXO script is empty, deriving from address: %s", utxo.Address)
				return tb.addressToScript(utxo.Address)
			}
		}
	}

	return nil, fmt.Errorf("previous output not found in stored UTXOs: %s:%d", txIDHex, txInput.SourceTxOutIndex)
}

// getPreviousOutputScript retrieves the script pubkey for a previous transaction output
func (tb *TransactionBuilder) getPreviousOutputScript(txID []byte, vout uint32, senderAddress string) (*script.Script, error) {
	// Convert txID back to hex string for UTXO lookup
	txIDHex := hex.EncodeToString(txID)

	// Fetch UTXOs for the sender address to find the script
	utxos, err := tb.utxoManager.FetchUTXOs(senderAddress)
	if err != nil {
		return nil, fmt.Errorf("failed to fetch UTXOs for address %s: %v", senderAddress, err)
	}

	// Find the matching UTXO
	for _, utxo := range utxos {
		if utxo.TxID == txIDHex && utxo.Vout == vout {
			// Check if script is available
			if utxo.Script != "" {
				// Convert the script pubkey from hex to script object
				scriptBytes, err := hex.DecodeString(utxo.Script)
				if err != nil {
					return nil, fmt.Errorf("invalid script pubkey: %v", err)
				}
				return script.NewFromBytes(scriptBytes), nil
			} else {
				// Script is empty, derive it from the address
				tb.logger.Infof("UTXO script is empty, deriving from address: %s", senderAddress)
				return tb.addressToScript(senderAddress)
			}
		}
	}

	return nil, fmt.Errorf("previous output not found: %s:%d for address %s", txIDHex, vout, senderAddress)
}
