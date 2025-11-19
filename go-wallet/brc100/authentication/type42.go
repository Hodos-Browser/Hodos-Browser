package authentication

import (
	"crypto/aes"
	"crypto/cipher"
	"crypto/hmac"
	"crypto/rand"
	"crypto/sha256"
	"encoding/hex"
	"fmt"
	"io"
	"time"

	"github.com/sirupsen/logrus"
)

// Type42Keys represents the result of Type-42 key derivation
type Type42Keys struct {
	SharedSecret  []byte    `json:"sharedSecret"`
	EncryptionKey []byte    `json:"encryptionKey"`
	SigningKey    []byte    `json:"signingKey"`
	SessionID     string    `json:"sessionId"`
	CreatedAt     time.Time `json:"createdAt"`
	ExpiresAt     time.Time `json:"expiresAt"`
}

// P2PMessage represents a P2P message for BRC-100 communication
type P2PMessage struct {
	MessageID string    `json:"messageId"`
	SessionID string    `json:"sessionId"`
	Content   []byte    `json:"content"`
	Signature []byte    `json:"signature"`
	Timestamp time.Time `json:"timestamp"`
	Encrypted bool      `json:"encrypted"`
}

// Type42Manager handles Type-42 key derivation and P2P authentication
type Type42Manager struct {
	logger *logrus.Logger
}

// NewType42Manager creates a new Type-42 manager
func NewType42Manager() *Type42Manager {
	logger := logrus.New()
	logger.SetLevel(logrus.InfoLevel)

	return &Type42Manager{
		logger: logger,
	}
}

// DeriveType42Keys derives Type-42 keys from wallet and app keys
func (tm *Type42Manager) DeriveType42Keys(walletKey, appKey []byte) (*Type42Keys, error) {
	tm.logger.Info("Deriving Type-42 keys")

	// Generate shared secret using ECDH-like key agreement
	sharedSecret, err := tm.GenerateSharedSecret(walletKey, appKey)
	if err != nil {
		return nil, fmt.Errorf("failed to generate shared secret: %v", err)
	}

	// Derive encryption key from shared secret
	encryptionKey, err := tm.DeriveEncryptionKey(sharedSecret)
	if err != nil {
		return nil, fmt.Errorf("failed to derive encryption key: %v", err)
	}

	// Derive signing key from shared secret
	signingKey, err := tm.DeriveSigningKey(sharedSecret)
	if err != nil {
		return nil, fmt.Errorf("failed to derive signing key: %v", err)
	}

	// Generate session ID
	sessionID := tm.GenerateSessionID(walletKey, appKey)

	// Create Type-42 keys
	keys := &Type42Keys{
		SharedSecret:  sharedSecret,
		EncryptionKey: encryptionKey,
		SigningKey:    signingKey,
		SessionID:     sessionID,
		CreatedAt:     time.Now(),
		ExpiresAt:     time.Now().Add(24 * time.Hour), // 24 hour session
	}

	tm.logger.Info("Type-42 keys derived successfully")
	return keys, nil
}

// GenerateSharedSecret generates a shared secret from two keys
func (tm *Type42Manager) GenerateSharedSecret(walletKey, appKey []byte) ([]byte, error) {
	tm.logger.Info("Generating shared secret")

	// Combine keys using HMAC-SHA256
	h := hmac.New(sha256.New, walletKey)
	h.Write(appKey)
	sharedSecret := h.Sum(nil)

	tm.logger.Info("Shared secret generated successfully")
	return sharedSecret, nil
}

// DeriveEncryptionKey derives an encryption key from shared secret
func (tm *Type42Manager) DeriveEncryptionKey(sharedSecret []byte) ([]byte, error) {
	tm.logger.Info("Deriving encryption key")

	// Use HKDF-like key derivation
	// For simplicity, we'll use SHA-256 with a salt
	salt := []byte("BRC100-ENCRYPTION-KEY")
	h := hmac.New(sha256.New, salt)
	h.Write(sharedSecret)
	h.Write([]byte("encryption"))
	encryptionKey := h.Sum(nil)

	tm.logger.Info("Encryption key derived successfully")
	return encryptionKey, nil
}

// DeriveSigningKey derives a signing key from shared secret
func (tm *Type42Manager) DeriveSigningKey(sharedSecret []byte) ([]byte, error) {
	tm.logger.Info("Deriving signing key")

	// Use HKDF-like key derivation
	// For simplicity, we'll use SHA-256 with a salt
	salt := []byte("BRC100-SIGNING-KEY")
	h := hmac.New(sha256.New, salt)
	h.Write(sharedSecret)
	h.Write([]byte("signing"))
	signingKey := h.Sum(nil)

	tm.logger.Info("Signing key derived successfully")
	return signingKey, nil
}

// GenerateSessionID generates a unique session ID
func (tm *Type42Manager) GenerateSessionID(walletKey, appKey []byte) string {
	tm.logger.Info("Generating session ID")

	// Combine keys and timestamp for uniqueness
	timestamp := time.Now().Unix()
	data := fmt.Sprintf("%x%x%d", walletKey, appKey, timestamp)

	// Create hash
	hash := sha256.Sum256([]byte(data))
	sessionID := hex.EncodeToString(hash[:8]) // 16 character hex string

	tm.logger.Infof("Session ID generated: %s", sessionID)
	return sessionID
}

// EncryptMessage encrypts a message using AES-256-GCM
func (tm *Type42Manager) EncryptMessage(message []byte, encryptionKey []byte) ([]byte, error) {
	tm.logger.Info("Encrypting message")

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

	// Encrypt message
	ciphertext := gcm.Seal(nonce, nonce, message, nil)

	tm.logger.Info("Message encrypted successfully")
	return ciphertext, nil
}

// DecryptMessage decrypts a message using AES-256-GCM
func (tm *Type42Manager) DecryptMessage(encryptedMessage []byte, encryptionKey []byte) ([]byte, error) {
	tm.logger.Info("Decrypting message")

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

	// Extract nonce and ciphertext
	nonceSize := gcm.NonceSize()
	if len(encryptedMessage) < nonceSize {
		return nil, fmt.Errorf("encrypted message too short")
	}

	nonce, ciphertext := encryptedMessage[:nonceSize], encryptedMessage[nonceSize:]

	// Decrypt message
	plaintext, err := gcm.Open(nil, nonce, ciphertext, nil)
	if err != nil {
		return nil, fmt.Errorf("failed to decrypt message: %v", err)
	}

	tm.logger.Info("Message decrypted successfully")
	return plaintext, nil
}

// SignMessage signs a message using HMAC-SHA256
func (tm *Type42Manager) SignMessage(message []byte, signingKey []byte) ([]byte, error) {
	tm.logger.Info("Signing message")

	// Create HMAC signature
	h := hmac.New(sha256.New, signingKey)
	h.Write(message)
	signature := h.Sum(nil)

	tm.logger.Info("Message signed successfully")
	return signature, nil
}

// VerifyMessage verifies a message signature
func (tm *Type42Manager) VerifyMessage(message, signature []byte, signingKey []byte) (bool, error) {
	tm.logger.Info("Verifying message signature")

	// Create expected signature
	expectedSignature, err := tm.SignMessage(message, signingKey)
	if err != nil {
		return false, fmt.Errorf("failed to create expected signature: %v", err)
	}

	// Compare signatures
	isValid := hmac.Equal(signature, expectedSignature)

	if isValid {
		tm.logger.Info("Message signature is valid")
	} else {
		tm.logger.Warn("Message signature is invalid")
	}

	return isValid, nil
}

// CreateP2PMessage creates a P2P message
func (tm *Type42Manager) CreateP2PMessage(content []byte, sessionID string, signingKey []byte) (*P2PMessage, error) {
	tm.logger.Info("Creating P2P message")

	// Generate message ID
	messageID := tm.GenerateMessageID()

	// Sign the content
	signature, err := tm.SignMessage(content, signingKey)
	if err != nil {
		return nil, fmt.Errorf("failed to sign message: %v", err)
	}

	// Create P2P message
	message := &P2PMessage{
		MessageID: messageID,
		SessionID: sessionID,
		Content:   content,
		Signature: signature,
		Timestamp: time.Now(),
		Encrypted: false, // Will be set to true if encrypted
	}

	tm.logger.Info("P2P message created successfully")
	return message, nil
}

// VerifyP2PMessage verifies a P2P message
func (tm *Type42Manager) VerifyP2PMessage(message *P2PMessage, signingKey []byte) (bool, error) {
	tm.logger.Info("Verifying P2P message")

	// Verify signature
	isValid, err := tm.VerifyMessage(message.Content, message.Signature, signingKey)
	if err != nil {
		return false, fmt.Errorf("failed to verify message signature: %v", err)
	}

	// Check message age (reject messages older than 5 minutes)
	age := time.Since(message.Timestamp)
	if age > 5*time.Minute {
		tm.logger.Warn("Message is too old")
		return false, fmt.Errorf("message is too old: %v", age)
	}

	if isValid {
		tm.logger.Info("P2P message is valid")
	} else {
		tm.logger.Warn("P2P message is invalid")
	}

	return isValid, nil
}

// GenerateMessageID generates a unique message ID
func (tm *Type42Manager) GenerateMessageID() string {
	// Generate random bytes
	bytes := make([]byte, 16)
	if _, err := io.ReadFull(rand.Reader, bytes); err != nil {
		// Fallback to timestamp-based ID
		return fmt.Sprintf("msg_%d", time.Now().UnixNano())
	}

	return hex.EncodeToString(bytes)
}
