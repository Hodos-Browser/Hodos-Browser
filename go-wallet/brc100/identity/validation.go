package identity

import (
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"regexp"
	"time"

	"github.com/sirupsen/logrus"
)

// ValidationManager handles validation of BRC-100 identity components
type ValidationManager struct {
	logger *logrus.Logger
}

// NewValidationManager creates a new validation manager
func NewValidationManager() *ValidationManager {
	logger := logrus.New()
	logger.SetLevel(logrus.InfoLevel)

	return &ValidationManager{
		logger: logger,
	}
}

// ValidateCertificateStructure validates the structure of an identity certificate
func (vm *ValidationManager) ValidateCertificateStructure(cert *IdentityCertificate) error {
	vm.logger.Info("Validating certificate structure")

	// Validate version
	if cert.Version == "" {
		return fmt.Errorf("certificate version is required")
	}

	// Validate issuer
	if cert.Issuer == "" {
		return fmt.Errorf("certificate issuer is required")
	}

	// Validate subject
	if cert.Subject == "" {
		return fmt.Errorf("certificate subject is required")
	}

	// Validate public key format (simplified check)
	if cert.PublicKey == "" {
		return fmt.Errorf("certificate public key is required")
	}

	// Validate timestamp
	if cert.Timestamp.IsZero() {
		return fmt.Errorf("certificate timestamp is required")
	}

	// Validate expiration
	if cert.ExpiresAt.IsZero() {
		return fmt.Errorf("certificate expiration is required")
	}

	// Validate expiration is after timestamp
	if cert.ExpiresAt.Before(cert.Timestamp) {
		return fmt.Errorf("certificate expiration must be after timestamp")
	}

	vm.logger.Info("Certificate structure is valid")
	return nil
}

// ValidateSignature validates the signature of an identity certificate
func (vm *ValidationManager) ValidateSignature(cert *IdentityCertificate) error {
	vm.logger.Info("Validating certificate signature")

	// Check if signature exists
	if cert.Signature == "" {
		return fmt.Errorf("certificate signature is required")
	}

	// Create a copy of the certificate without signature for validation
	certCopy := *cert
	certCopy.Signature = ""

	// Marshal the certificate data
	certData, err := json.Marshal(certCopy)
	if err != nil {
		return fmt.Errorf("failed to marshal certificate for signature validation: %v", err)
	}

	// Create hash of certificate data
	hash := sha256.Sum256(certData)
	expectedHash := hex.EncodeToString(hash[:])

	// For now, we'll use a simple signature validation
	// In production, this would use proper ECDSA signature verification
	if cert.Signature != "sig_"+expectedHash[:16] {
		return fmt.Errorf("invalid certificate signature")
	}

	vm.logger.Info("Certificate signature is valid")
	return nil
}

// ValidateSelectiveDisclosureRequest validates a selective disclosure request
func (vm *ValidationManager) ValidateSelectiveDisclosureRequest(req *SelectiveDisclosureRequest) error {
	vm.logger.Info("Validating selective disclosure request")

	// Validate app domain
	if req.AppDomain == "" {
		return fmt.Errorf("app domain is required")
	}

	// Validate app domain format (basic URL validation)
	domainRegex := regexp.MustCompile(`^[a-zA-Z0-9][a-zA-Z0-9-]{1,61}[a-zA-Z0-9]?\.[a-zA-Z]{2,}$`)
	if !domainRegex.MatchString(req.AppDomain) {
		return fmt.Errorf("invalid app domain format")
	}

	// Validate purpose
	if req.Purpose == "" {
		return fmt.Errorf("purpose is required")
	}

	// Validate requested fields
	if len(req.RequestedFields) == 0 {
		return fmt.Errorf("at least one field must be requested")
	}

	// Validate field names (basic format check)
	fieldRegex := regexp.MustCompile(`^[a-zA-Z][a-zA-Z0-9_]*$`)
	for _, field := range req.RequestedFields {
		if !fieldRegex.MatchString(field) {
			return fmt.Errorf("invalid field name format: %s", field)
		}
	}

	vm.logger.Info("Selective disclosure request is valid")
	return nil
}

// ValidateIdentityContext validates an identity context
func (vm *ValidationManager) ValidateIdentityContext(ctx *IdentityContext) error {
	vm.logger.Info("Validating identity context")

	// Validate certificate
	if ctx.Certificate == nil {
		return fmt.Errorf("certificate is required in identity context")
	}

	if err := vm.ValidateCertificateStructure(ctx.Certificate); err != nil {
		return fmt.Errorf("invalid certificate in context: %v", err)
	}

	// Validate session ID
	if ctx.SessionID == "" {
		return fmt.Errorf("session ID is required")
	}

	// Validate session ID format (should be 16 hex characters)
	sessionIDRegex := regexp.MustCompile(`^[a-fA-F0-9]{16}$`)
	if !sessionIDRegex.MatchString(ctx.SessionID) {
		return fmt.Errorf("invalid session ID format")
	}

	// Validate app domain
	if ctx.AppDomain == "" {
		return fmt.Errorf("app domain is required")
	}

	// Validate timestamp
	if ctx.Timestamp.IsZero() {
		return fmt.Errorf("timestamp is required")
	}

	// Validate timestamp is not in the future
	if ctx.Timestamp.After(time.Now().Add(5 * time.Minute)) {
		return fmt.Errorf("timestamp cannot be in the future")
	}

	vm.logger.Info("Identity context is valid")
	return nil
}

// ValidateCertificateExpiration checks if a certificate is expired
func (vm *ValidationManager) ValidateCertificateExpiration(cert *IdentityCertificate) error {
	vm.logger.Info("Validating certificate expiration")

	now := time.Now()
	if now.After(cert.ExpiresAt) {
		return fmt.Errorf("certificate expired at %s", cert.ExpiresAt.Format(time.RFC3339))
	}

	// Check if certificate expires soon (within 30 days)
	expirationWarning := cert.ExpiresAt.Add(-30 * 24 * time.Hour)
	if now.After(expirationWarning) {
		vm.logger.Warnf("Certificate expires soon: %s", cert.ExpiresAt.Format(time.RFC3339))
	}

	vm.logger.Info("Certificate expiration is valid")
	return nil
}

// ValidateCertificateRevocation checks if a certificate is revoked
func (vm *ValidationManager) ValidateCertificateRevocation(cert *IdentityCertificate) error {
	vm.logger.Info("Validating certificate revocation status")

	if cert.Revoked {
		return fmt.Errorf("certificate has been revoked")
	}

	vm.logger.Info("Certificate is not revoked")
	return nil
}

// ValidateCompleteCertificate performs complete certificate validation
func (vm *ValidationManager) ValidateCompleteCertificate(cert *IdentityCertificate) error {
	vm.logger.Info("Performing complete certificate validation")

	// Validate structure
	if err := vm.ValidateCertificateStructure(cert); err != nil {
		return fmt.Errorf("structure validation failed: %v", err)
	}

	// Validate signature
	if err := vm.ValidateSignature(cert); err != nil {
		return fmt.Errorf("signature validation failed: %v", err)
	}

	// Validate expiration
	if err := vm.ValidateCertificateExpiration(cert); err != nil {
		return fmt.Errorf("expiration validation failed: %v", err)
	}

	// Validate revocation
	if err := vm.ValidateCertificateRevocation(cert); err != nil {
		return fmt.Errorf("revocation validation failed: %v", err)
	}

	vm.logger.Info("Complete certificate validation passed")
	return nil
}
