package spv

import (
	"fmt"
	"sync"
	"time"

	ec "github.com/bsv-blockchain/go-sdk/primitives/ec"
	"github.com/bsv-blockchain/go-sdk/transaction"
	"github.com/sirupsen/logrus"
)

// SPVManager manages SPV verification operations
type SPVManager struct {
	verifier *SPVVerifier
	logger   *logrus.Logger
	mu       sync.RWMutex
}

// NewSPVManager creates a new SPV manager
func NewSPVManager() *SPVManager {
	return &SPVManager{
		verifier: NewSPVVerifier(),
		logger:   logrus.New(),
	}
}

// VerifyIdentityProof verifies an identity proof using SPV
func (sm *SPVManager) VerifyIdentityProof(proof *IdentityProof) (*SPVVerificationResult, error) {
	sm.logger.Info("Starting SPV verification through manager")

	sm.mu.RLock()
	defer sm.mu.RUnlock()

	return sm.verifier.VerifyIdentityProof(proof)
}

// CreateIdentityProof creates a new identity proof
func (sm *SPVManager) CreateIdentityProof(txID string, identityData map[string]interface{}) (*IdentityProof, error) {
	sm.logger.Info("Creating identity proof through manager")

	sm.mu.RLock()
	defer sm.mu.RUnlock()

	return sm.verifier.CreateIdentityProof(txID, identityData)
}

// VerifyIdentityOnChain verifies that an identity is valid on-chain
func (sm *SPVManager) VerifyIdentityOnChain(identityData map[string]interface{}) (bool, error) {
	sm.logger.Info("Verifying identity on-chain through manager")

	sm.mu.RLock()
	defer sm.mu.RUnlock()

	return sm.verifier.VerifyIdentityOnChain(identityData)
}

// GetTransactionFromID fetches a transaction by its ID
func (sm *SPVManager) GetTransactionFromID(txID string) (*transaction.Transaction, error) {
	sm.logger.WithField("txID", txID).Info("Fetching transaction by ID through manager")

	sm.mu.RLock()
	defer sm.mu.RUnlock()

	return sm.verifier.GetTransactionFromID(txID)
}

// GetMerkleProof fetches a Merkle proof for a transaction
func (sm *SPVManager) GetMerkleProof(txID string, blockHeight uint32) (*transaction.MerklePath, error) {
	sm.logger.WithFields(logrus.Fields{
		"txID":        txID,
		"blockHeight": blockHeight,
	}).Info("Fetching Merkle proof through manager")

	sm.mu.RLock()
	defer sm.mu.RUnlock()

	return sm.verifier.GetMerkleProof(txID, blockHeight)
}

// ValidateBlockHeader validates a block header
func (sm *SPVManager) ValidateBlockHeader(blockHash string, blockHeight int64) (bool, error) {
	sm.logger.WithFields(logrus.Fields{
		"blockHash":   blockHash,
		"blockHeight": blockHeight,
	}).Info("Validating block header through manager")

	sm.mu.RLock()
	defer sm.mu.RUnlock()

	return sm.verifier.ValidateBlockHeader(blockHash, blockHeight)
}

// GetChainTip gets the current chain tip
func (sm *SPVManager) GetChainTip() (int64, string, error) {
	sm.logger.Info("Getting chain tip through manager")

	sm.mu.RLock()
	defer sm.mu.RUnlock()

	return sm.verifier.GetChainTip()
}

// IsTransactionConfirmed checks if a transaction has sufficient confirmations
func (sm *SPVManager) IsTransactionConfirmed(txID string, requiredConfirmations int64) (bool, error) {
	sm.logger.WithFields(logrus.Fields{
		"txID":                   txID,
		"requiredConfirmations":  requiredConfirmations,
	}).Info("Checking transaction confirmation through manager")

	sm.mu.RLock()
	defer sm.mu.RUnlock()

	return sm.verifier.IsTransactionConfirmed(txID, requiredConfirmations)
}

// VerifyTransactionSignature verifies a transaction signature
func (sm *SPVManager) VerifyTransactionSignature(tx *transaction.Transaction, publicKey *ec.PublicKey) (bool, error) {
	sm.logger.Info("Verifying transaction signature through manager")

	sm.mu.RLock()
	defer sm.mu.RUnlock()

	return sm.verifier.VerifyTransactionSignature(tx, publicKey)
}

// BatchVerifyIdentityProofs verifies multiple identity proofs in batch
func (sm *SPVManager) BatchVerifyIdentityProofs(proofs []*IdentityProof) ([]*SPVVerificationResult, error) {
	sm.logger.WithField("count", len(proofs)).Info("Starting batch verification of identity proofs")

	results := make([]*SPVVerificationResult, len(proofs))

	// Process proofs concurrently
	var wg sync.WaitGroup
	errChan := make(chan error, len(proofs))

	for i, proof := range proofs {
		wg.Add(1)
		go func(index int, p *IdentityProof) {
			defer wg.Done()

			result, err := sm.VerifyIdentityProof(p)
			if err != nil {
				errChan <- fmt.Errorf("proof %d verification failed: %v", index, err)
				return
			}

			results[index] = result
		}(i, proof)
	}

	wg.Wait()
	close(errChan)

	// Check for errors
	var errors []error
	for err := range errChan {
		errors = append(errors, err)
	}

	if len(errors) > 0 {
		return results, fmt.Errorf("batch verification failed: %v", errors)
	}

	sm.logger.Info("Batch verification completed successfully")
	return results, nil
}

// GetVerificationStats returns statistics about SPV verification operations
func (sm *SPVManager) GetVerificationStats() map[string]interface{} {
	sm.logger.Info("Getting verification statistics")

	// In a real implementation, this would track actual statistics
	// For now, we'll return simulated stats
	return map[string]interface{}{
		"totalVerifications": 0,
		"successfulVerifications": 0,
		"failedVerifications": 0,
		"averageVerificationTime": "0ms",
		"lastVerificationTime": time.Now(),
	}
}

// HealthCheck performs a health check on the SPV manager
func (sm *SPVManager) HealthCheck() error {
	sm.logger.Info("Performing SPV manager health check")

	// Check if verifier is available
	if sm.verifier == nil {
		return fmt.Errorf("SPV verifier is not initialized")
	}

	// Check if we can get chain tip
	_, _, err := sm.GetChainTip()
	if err != nil {
		return fmt.Errorf("failed to get chain tip: %v", err)
	}

	sm.logger.Info("SPV manager health check passed")
	return nil
}

// Stop stops the SPV manager
func (sm *SPVManager) Stop() {
	sm.logger.Info("Stopping SPV manager")

	sm.mu.Lock()
	defer sm.mu.Unlock()

	// Cleanup resources
	sm.verifier = nil

	sm.logger.Info("SPV manager stopped")
}
