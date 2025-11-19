package authentication

import (
	"crypto/rand"
	"encoding/hex"
	"fmt"
	"time"

	"github.com/sirupsen/logrus"
)

// Challenge represents an authentication challenge
type Challenge struct {
	ChallengeID string    `json:"challengeId"`
	Challenge   string    `json:"challenge"`
	AppDomain   string    `json:"appDomain"`
	CreatedAt   time.Time `json:"createdAt"`
	ExpiresAt   time.Time `json:"expiresAt"`
	Solved      bool      `json:"solved"`
}

// ChallengeResponse represents a response to an authentication challenge
type ChallengeResponse struct {
	ChallengeID   string `json:"challengeId"`
	Response      string `json:"response"`
	SessionID     string `json:"sessionId"`
	WalletAddress string `json:"walletAddress"`
	Signature     string `json:"signature"`
}

// ChallengeManager handles authentication challenges
type ChallengeManager struct {
	logger     *logrus.Logger
	challenges map[string]*Challenge
}

// NewChallengeManager creates a new challenge manager
func NewChallengeManager() *ChallengeManager {
	logger := logrus.New()
	logger.SetLevel(logrus.InfoLevel)

	return &ChallengeManager{
		logger:     logger,
		challenges: make(map[string]*Challenge),
	}
}

// CreateChallenge creates a new authentication challenge
func (cm *ChallengeManager) CreateChallenge(appDomain string) (*Challenge, error) {
	cm.logger.Infof("Creating challenge for app domain: %s", appDomain)

	// Generate challenge ID
	challengeID := cm.generateChallengeID()

	// Generate random challenge
	challenge := cm.generateRandomChallenge()

	// Create challenge
	challengeObj := &Challenge{
		ChallengeID: challengeID,
		Challenge:   challenge,
		AppDomain:   appDomain,
		CreatedAt:   time.Now(),
		ExpiresAt:   time.Now().Add(5 * time.Minute), // 5 minute expiration
		Solved:      false,
	}

	// Store challenge
	cm.challenges[challengeID] = challengeObj

	cm.logger.Infof("Challenge created successfully: %s", challengeID)
	return challengeObj, nil
}

// VerifyChallengeResponse verifies a challenge response
func (cm *ChallengeManager) VerifyChallengeResponse(response *ChallengeResponse) (bool, error) {
	cm.logger.Infof("Verifying challenge response: %s", response.ChallengeID)

	// Get challenge
	challenge, exists := cm.challenges[response.ChallengeID]
	if !exists {
		return false, fmt.Errorf("challenge not found: %s", response.ChallengeID)
	}

	// Check if challenge is expired
	if time.Now().After(challenge.ExpiresAt) {
		cm.logger.Warnf("Challenge expired: %s", response.ChallengeID)
		return false, fmt.Errorf("challenge expired: %s", response.ChallengeID)
	}

	// Check if challenge is already solved
	if challenge.Solved {
		cm.logger.Warnf("Challenge already solved: %s", response.ChallengeID)
		return false, fmt.Errorf("challenge already solved: %s", response.ChallengeID)
	}

	// Verify response format (simplified for now)
	if response.Response == "" {
		return false, fmt.Errorf("empty response")
	}

	// Verify signature (simplified for now)
	if response.Signature == "" {
		return false, fmt.Errorf("empty signature")
	}

	// Mark challenge as solved
	challenge.Solved = true

	cm.logger.Infof("Challenge response verified successfully: %s", response.ChallengeID)
	return true, nil
}

// GetChallenge retrieves a challenge by ID
func (cm *ChallengeManager) GetChallenge(challengeID string) (*Challenge, error) {
	cm.logger.Infof("Retrieving challenge: %s", challengeID)

	challenge, exists := cm.challenges[challengeID]
	if !exists {
		return nil, fmt.Errorf("challenge not found: %s", challengeID)
	}

	// Check if challenge is expired
	if time.Now().After(challenge.ExpiresAt) {
		cm.logger.Warnf("Challenge expired: %s", challengeID)
		return nil, fmt.Errorf("challenge expired: %s", challengeID)
	}

	cm.logger.Infof("Challenge retrieved successfully: %s", challengeID)
	return challenge, nil
}

// CleanupExpiredChallenges removes expired challenges
func (cm *ChallengeManager) CleanupExpiredChallenges() {
	cm.logger.Info("Cleaning up expired challenges")

	now := time.Now()
	expiredChallenges := make([]string, 0)

	for challengeID, challenge := range cm.challenges {
		if now.After(challenge.ExpiresAt) {
			expiredChallenges = append(expiredChallenges, challengeID)
		}
	}

	// Remove expired challenges
	for _, challengeID := range expiredChallenges {
		delete(cm.challenges, challengeID)
	}

	if len(expiredChallenges) > 0 {
		cm.logger.Infof("Cleaned up %d expired challenges", len(expiredChallenges))
	}
}

// generateChallengeID generates a unique challenge ID
func (cm *ChallengeManager) generateChallengeID() string {
	// Generate random bytes
	bytes := make([]byte, 16)
	rand.Read(bytes)
	return hex.EncodeToString(bytes)
}

// generateRandomChallenge generates a random challenge string
func (cm *ChallengeManager) generateRandomChallenge() string {
	// Generate random bytes
	bytes := make([]byte, 32)
	rand.Read(bytes)
	return hex.EncodeToString(bytes)
}

// CreateChallengeResponse creates a response to a challenge
func (cm *ChallengeManager) CreateChallengeResponse(challengeID, walletAddress, sessionID string) (*ChallengeResponse, error) {
	cm.logger.Infof("Creating challenge response for challenge: %s", challengeID)

	// Get challenge
	challenge, err := cm.GetChallenge(challengeID)
	if err != nil {
		return nil, fmt.Errorf("failed to get challenge: %v", err)
	}

	// Create response (simplified for now)
	response := &ChallengeResponse{
		ChallengeID:   challengeID,
		Response:      challenge.Challenge, // Echo the challenge back
		SessionID:     sessionID,
		WalletAddress: walletAddress,
		Signature:     "sig_" + challenge.Challenge[:16], // Simplified signature
	}

	cm.logger.Infof("Challenge response created successfully: %s", challengeID)
	return response, nil
}

// ValidateChallengeFormat validates challenge format
func (cm *ChallengeManager) ValidateChallengeFormat(challenge string) bool {
	// Check if challenge is valid hex
	if len(challenge) != 64 { // 32 bytes = 64 hex chars
		return false
	}

	_, err := hex.DecodeString(challenge)
	return err == nil
}

// ValidateResponseFormat validates response format
func (cm *ChallengeManager) ValidateResponseFormat(response *ChallengeResponse) bool {
	// Validate challenge ID
	if response.ChallengeID == "" {
		return false
	}

	// Validate response
	if response.Response == "" {
		return false
	}

	// Validate session ID
	if response.SessionID == "" {
		return false
	}

	// Validate wallet address
	if response.WalletAddress == "" {
		return false
	}

	// Validate signature
	if response.Signature == "" {
		return false
	}

	return true
}

// GetChallengeStats returns challenge statistics
func (cm *ChallengeManager) GetChallengeStats() map[string]interface{} {
	cm.logger.Info("Getting challenge statistics")

	total := len(cm.challenges)
	solved := 0
	expired := 0

	now := time.Now()
	for _, challenge := range cm.challenges {
		if challenge.Solved {
			solved++
		} else if now.After(challenge.ExpiresAt) {
			expired++
		}
	}

	stats := map[string]interface{}{
		"total":   total,
		"solved":  solved,
		"expired": expired,
		"active":  total - solved - expired,
	}

	cm.logger.Infof("Challenge statistics: %+v", stats)
	return stats
}
