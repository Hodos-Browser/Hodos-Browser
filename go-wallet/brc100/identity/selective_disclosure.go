package identity

import (
	"crypto/aes"
	"crypto/cipher"
	"crypto/rand"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"io"

	"github.com/sirupsen/logrus"
)

// SelectiveDisclosureManager handles selective disclosure operations
type SelectiveDisclosureManager struct {
	logger *logrus.Logger
}

// NewSelectiveDisclosureManager creates a new selective disclosure manager
func NewSelectiveDisclosureManager() *SelectiveDisclosureManager {
	logger := logrus.New()
	logger.SetLevel(logrus.InfoLevel)

	return &SelectiveDisclosureManager{
		logger: logger,
	}
}

// EncryptSelectiveData encrypts selective data using AES-256-GCM
func (sdm *SelectiveDisclosureManager) EncryptSelectiveData(data map[string]interface{}, encryptionKey []byte) (map[string]interface{}, error) {
	sdm.logger.Info("Encrypting selective data")

	// Convert data to JSON for encryption
	jsonData, err := json.Marshal(data)
	if err != nil {
		return nil, fmt.Errorf("failed to marshal data for encryption: %v", err)
	}

	// Create AES cipher
	block, err := aes.NewCipher(encryptionKey)
	if err != nil {
		return nil, fmt.Errorf("failed to create AES cipher: %v", err)
	}

	// Create GCM mode
	gcm, err := cipher.NewGCM(block)
	if err != nil {
		return nil, fmt.Errorf("failed to create GCM mode: %v", err)
	}

	// Generate random nonce
	nonce := make([]byte, gcm.NonceSize())
	if _, err := io.ReadFull(rand.Reader, nonce); err != nil {
		return nil, fmt.Errorf("failed to generate nonce: %v", err)
	}

	// Encrypt data
	ciphertext := gcm.Seal(nonce, nonce, jsonData, nil)

	// Return encrypted data with nonce
	encryptedData := map[string]interface{}{
		"encrypted": true,
		"data":      hex.EncodeToString(ciphertext),
		"nonce":     hex.EncodeToString(nonce),
		"algorithm": "AES-256-GCM",
	}

	sdm.logger.Info("Selective data encrypted successfully")
	return encryptedData, nil
}

// DecryptSelectiveData decrypts selective data using AES-256-GCM
func (sdm *SelectiveDisclosureManager) DecryptSelectiveData(encryptedData map[string]interface{}, encryptionKey []byte) (map[string]interface{}, error) {
	sdm.logger.Info("Decrypting selective data")

	// Check if data is encrypted
	if encrypted, ok := encryptedData["encrypted"].(bool); !ok || !encrypted {
		return nil, fmt.Errorf("data is not encrypted")
	}

	// Get encrypted data and nonce
	dataHex, ok := encryptedData["data"].(string)
	if !ok {
		return nil, fmt.Errorf("invalid encrypted data format")
	}

	nonceHex, ok := encryptedData["nonce"].(string)
	if !ok {
		return nil, fmt.Errorf("invalid nonce format")
	}

	// Decode hex data
	ciphertext, err := hex.DecodeString(dataHex)
	if err != nil {
		return nil, fmt.Errorf("failed to decode ciphertext: %v", err)
	}

	nonce, err := hex.DecodeString(nonceHex)
	if err != nil {
		return nil, fmt.Errorf("failed to decode nonce: %v", err)
	}

	// Create AES cipher
	block, err := aes.NewCipher(encryptionKey)
	if err != nil {
		return nil, fmt.Errorf("failed to create AES cipher: %v", err)
	}

	// Create GCM mode
	gcm, err := cipher.NewGCM(block)
	if err != nil {
		return nil, fmt.Errorf("failed to create GCM mode: %v", err)
	}

	// Decrypt data
	plaintext, err := gcm.Open(nil, nonce, ciphertext, nil)
	if err != nil {
		return nil, fmt.Errorf("failed to decrypt data: %v", err)
	}

	// Unmarshal decrypted JSON
	var data map[string]interface{}
	if err := json.Unmarshal(plaintext, &data); err != nil {
		return nil, fmt.Errorf("failed to unmarshal decrypted data: %v", err)
	}

	sdm.logger.Info("Selective data decrypted successfully")
	return data, nil
}

// GenerateEncryptionKey generates a 256-bit encryption key from a password
func (sdm *SelectiveDisclosureManager) GenerateEncryptionKey(password string) []byte {
	sdm.logger.Info("Generating encryption key from password")

	// Use SHA-256 to generate 256-bit key from password
	hash := sha256.Sum256([]byte(password))
	return hash[:]
}

// CreateFieldMask creates a field mask for selective disclosure
func (sdm *SelectiveDisclosureManager) CreateFieldMask(requestedFields []string) map[string]bool {
	sdm.logger.Infof("Creating field mask for %d fields", len(requestedFields))

	fieldMask := make(map[string]bool)
	for _, field := range requestedFields {
		fieldMask[field] = true
	}

	return fieldMask
}

// ApplyFieldMask applies a field mask to identity data
func (sdm *SelectiveDisclosureManager) ApplyFieldMask(data map[string]interface{}, fieldMask map[string]bool) map[string]interface{} {
	sdm.logger.Info("Applying field mask to identity data")

	filteredData := make(map[string]interface{})
	for field, value := range data {
		if fieldMask[field] {
			filteredData[field] = value
		}
	}

	sdm.logger.Infof("Field mask applied, %d fields included", len(filteredData))
	return filteredData
}
