package identity

import (
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"time"

	"github.com/sirupsen/logrus"
)

// IdentityCertificate represents a BRC-52/103 compliant identity certificate
type IdentityCertificate struct {
	Version       string                 `json:"version"`
	Issuer        string                 `json:"issuer"`
	Subject       string                 `json:"subject"`
	PublicKey     string                 `json:"publicKey"`
	SelectiveData map[string]interface{} `json:"selectiveData"`
	Signature     string                 `json:"signature"`
	Timestamp     time.Time              `json:"timestamp"`
	ExpiresAt     time.Time              `json:"expiresAt"`
	Revoked       bool                   `json:"revoked"`
}

// SelectiveDisclosureRequest represents a request for selective data disclosure
type SelectiveDisclosureRequest struct {
	RequestedFields []string `json:"requestedFields"`
	AppDomain       string   `json:"appDomain"`
	Purpose         string   `json:"purpose"`
}

// IdentityContext represents the context for BEEF transactions
type IdentityContext struct {
	Certificate *IdentityCertificate `json:"certificate"`
	SessionID   string               `json:"sessionId"`
	AppDomain   string               `json:"appDomain"`
	Timestamp   time.Time            `json:"timestamp"`
}

// IdentityManager manages BRC-100 identity certificates
type IdentityManager struct {
	logger *logrus.Logger
}

// NewIdentityManager creates a new identity manager
func NewIdentityManager() *IdentityManager {
	logger := logrus.New()
	logger.SetLevel(logrus.InfoLevel)

	return &IdentityManager{
		logger: logger,
	}
}

// GenerateIdentityCertificate creates a new BRC-100 identity certificate
func (im *IdentityManager) GenerateIdentityCertificate(userID string, selectiveDisclosure map[string]bool) (*IdentityCertificate, error) {
	im.logger.Infof("Generating identity certificate for user: %s", userID)

	// Create selective data based on disclosure preferences
	selectiveData := make(map[string]interface{})
	for field, disclosed := range selectiveDisclosure {
		if disclosed {
			selectiveData[field] = "disclosed"
		} else {
			selectiveData[field] = "hidden"
		}
	}

	// Create identity certificate
	certificate := &IdentityCertificate{
		Version:       "1.0.0",
		Issuer:        "Babbage-Browser-Wallet",
		Subject:       userID,
		PublicKey:     "", // Will be set when signing
		SelectiveData: selectiveData,
		Signature:     "", // Will be set when signing
		Timestamp:     time.Now(),
		ExpiresAt:     time.Now().Add(365 * 24 * time.Hour), // 1 year validity
		Revoked:       false,
	}

	im.logger.Info("Identity certificate generated successfully")
	return certificate, nil
}

// SignIdentityCertificate signs an identity certificate with a private key
func (im *IdentityManager) SignIdentityCertificate(cert *IdentityCertificate, privateKey string) error {
	im.logger.Info("Signing identity certificate")

	// Create a hash of the certificate data for signing
	certData, err := json.Marshal(cert)
	if err != nil {
		return fmt.Errorf("failed to marshal certificate: %v", err)
	}

	// Create hash of certificate data
	hash := sha256.Sum256(certData)
	hashHex := hex.EncodeToString(hash[:])

	// For now, we'll use a simple signature (in production, use proper ECDSA signing)
	cert.Signature = "sig_" + hashHex[:16]
	cert.PublicKey = "pub_" + privateKey[:16] // Simplified for now

	im.logger.Info("Identity certificate signed successfully")
	return nil
}

// ValidateIdentityCertificate validates an identity certificate
func (im *IdentityManager) ValidateIdentityCertificate(cert *IdentityCertificate) (bool, error) {
	im.logger.Info("Validating identity certificate")

	// Check if certificate is expired
	if time.Now().After(cert.ExpiresAt) {
		im.logger.Warn("Certificate has expired")
		return false, fmt.Errorf("certificate has expired")
	}

	// Check if certificate is revoked
	if cert.Revoked {
		im.logger.Warn("Certificate has been revoked")
		return false, fmt.Errorf("certificate has been revoked")
	}

	// Validate signature (simplified for now)
	if cert.Signature == "" {
		im.logger.Warn("Certificate has no signature")
		return false, fmt.Errorf("certificate has no signature")
	}

	im.logger.Info("Identity certificate is valid")
	return true, nil
}

// RevokeIdentityCertificate revokes an identity certificate
func (im *IdentityManager) RevokeIdentityCertificate(cert *IdentityCertificate) error {
	im.logger.Infof("Revoking identity certificate for subject: %s", cert.Subject)

	cert.Revoked = true
	cert.Timestamp = time.Now() // Update timestamp

	im.logger.Info("Identity certificate revoked successfully")
	return nil
}

// CreateSelectiveDisclosure creates a selective disclosure of identity data
func (im *IdentityManager) CreateSelectiveDisclosure(fullData map[string]interface{}, requestedFields []string) map[string]interface{} {
	im.logger.Infof("Creating selective disclosure for %d requested fields", len(requestedFields))

	selectiveData := make(map[string]interface{})

	// Only include requested fields
	for _, field := range requestedFields {
		if value, exists := fullData[field]; exists {
			selectiveData[field] = value
		}
	}

	im.logger.Infof("Selective disclosure created with %d fields", len(selectiveData))
	return selectiveData
}

// ValidateSelectiveDisclosure validates that selective disclosure contains only requested fields
func (im *IdentityManager) ValidateSelectiveDisclosure(data map[string]interface{}, requestedFields []string) bool {
	im.logger.Info("Validating selective disclosure")

	// Check if all requested fields are present
	for _, field := range requestedFields {
		if _, exists := data[field]; !exists {
			im.logger.Warnf("Requested field '%s' not found in selective disclosure", field)
			return false
		}
	}

	// Check if no extra fields are present
	if len(data) > len(requestedFields) {
		im.logger.Warn("Selective disclosure contains extra fields")
		return false
	}

	im.logger.Info("Selective disclosure is valid")
	return true
}
