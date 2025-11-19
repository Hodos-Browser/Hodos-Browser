package authentication

import (
	"crypto/sha256"
	"encoding/hex"
	"fmt"
	"sync"
	"time"

	"github.com/sirupsen/logrus"
)

// BRCSession represents a BRC-100 session
type BRCSession struct {
	SessionID     string                 `json:"sessionId"`
	AppDomain     string                 `json:"appDomain"`
	IdentityCert  map[string]interface{} `json:"identityCert"`
	CreatedAt     time.Time              `json:"createdAt"`
	ExpiresAt     time.Time              `json:"expiresAt"`
	LastActivity  time.Time              `json:"lastActivity"`
	Authenticated bool                   `json:"authenticated"`
	Permissions   []string               `json:"permissions"`
}

// SessionManager manages BRC-100 sessions
type SessionManager struct {
	logger    *logrus.Logger
	sessions  map[string]*BRCSession
	mutex     sync.RWMutex
	cleanupTicker *time.Ticker
}

// NewSessionManager creates a new session manager
func NewSessionManager() *SessionManager {
	logger := logrus.New()
	logger.SetLevel(logrus.InfoLevel)

	sm := &SessionManager{
		logger:   logger,
		sessions: make(map[string]*BRCSession),
	}

	// Start cleanup routine
	sm.startCleanupRoutine()

	return sm
}

// GenerateBRCSessionID creates a BSV-native session ID
func GenerateBRCSessionID(walletAddress, appDomain string) string {
	data := fmt.Sprintf("%s:%s:%d", walletAddress, appDomain, time.Now().Unix())
	hash := sha256.Sum256([]byte(data))
	return hex.EncodeToString(hash[:8])
}

// ValidateBRCSessionID validates a session ID format
func ValidateBRCSessionID(sessionID string) bool {
	if len(sessionID) != 16 {
		return false
	}
	_, err := hex.DecodeString(sessionID)
	return err == nil
}

// CreateSession creates a new BRC-100 session
func (sm *SessionManager) CreateSession(appDomain string, identityCert map[string]interface{}, permissions []string) (*BRCSession, error) {
	sm.logger.Infof("Creating session for app domain: %s", appDomain)

	// Generate session ID
	sessionID := GenerateBRCSessionID("wallet", appDomain)

	// Create session
	session := &BRCSession{
		SessionID:     sessionID,
		AppDomain:     appDomain,
		IdentityCert:  identityCert,
		CreatedAt:     time.Now(),
		ExpiresAt:     time.Now().Add(24 * time.Hour), // 24 hour session
		LastActivity:  time.Now(),
		Authenticated: false,
		Permissions:   permissions,
	}

	// Store session
	sm.mutex.Lock()
	sm.sessions[sessionID] = session
	sm.mutex.Unlock()

	sm.logger.Infof("Session created successfully: %s", sessionID)
	return session, nil
}

// GetSession retrieves a session by ID
func (sm *SessionManager) GetSession(sessionID string) (*BRCSession, error) {
	sm.logger.Infof("Retrieving session: %s", sessionID)

	sm.mutex.RLock()
	session, exists := sm.sessions[sessionID]
	sm.mutex.RUnlock()

	if !exists {
		return nil, fmt.Errorf("session not found: %s", sessionID)
	}

	// Check if session is expired
	if time.Now().After(session.ExpiresAt) {
		sm.logger.Warnf("Session expired: %s", sessionID)
		sm.DeleteSession(sessionID)
		return nil, fmt.Errorf("session expired: %s", sessionID)
	}

	// Update last activity
	session.LastActivity = time.Now()

	sm.logger.Infof("Session retrieved successfully: %s", sessionID)
	return session, nil
}

// AuthenticateSession authenticates a session
func (sm *SessionManager) AuthenticateSession(sessionID string) error {
	sm.logger.Infof("Authenticating session: %s", sessionID)

	session, err := sm.GetSession(sessionID)
	if err != nil {
		return fmt.Errorf("failed to get session: %v", err)
	}

	// Mark session as authenticated
	session.Authenticated = true
	session.LastActivity = time.Now()

	sm.logger.Infof("Session authenticated successfully: %s", sessionID)
	return nil
}

// UpdateSessionPermissions updates session permissions
func (sm *SessionManager) UpdateSessionPermissions(sessionID string, permissions []string) error {
	sm.logger.Infof("Updating session permissions: %s", sessionID)

	session, err := sm.GetSession(sessionID)
	if err != nil {
		return fmt.Errorf("failed to get session: %v", err)
	}

	// Update permissions
	session.Permissions = permissions
	session.LastActivity = time.Now()

	sm.logger.Infof("Session permissions updated successfully: %s", sessionID)
	return nil
}

// DeleteSession deletes a session
func (sm *SessionManager) DeleteSession(sessionID string) error {
	sm.logger.Infof("Deleting session: %s", sessionID)

	sm.mutex.Lock()
	delete(sm.sessions, sessionID)
	sm.mutex.Unlock()

	sm.logger.Infof("Session deleted successfully: %s", sessionID)
	return nil
}

// GetActiveSessions returns all active sessions
func (sm *SessionManager) GetActiveSessions() []*BRCSession {
	sm.logger.Info("Retrieving active sessions")

	sm.mutex.RLock()
	sessions := make([]*BRCSession, 0, len(sm.sessions))
	for _, session := range sm.sessions {
		// Only return non-expired sessions
		if time.Now().Before(session.ExpiresAt) {
			sessions = append(sessions, session)
		}
	}
	sm.mutex.RUnlock()

	sm.logger.Infof("Retrieved %d active sessions", len(sessions))
	return sessions
}

// GetSessionsByAppDomain returns sessions for a specific app domain
func (sm *SessionManager) GetSessionsByAppDomain(appDomain string) []*BRCSession {
	sm.logger.Infof("Retrieving sessions for app domain: %s", appDomain)

	sm.mutex.RLock()
	sessions := make([]*BRCSession, 0)
	for _, session := range sm.sessions {
		if session.AppDomain == appDomain && time.Now().Before(session.ExpiresAt) {
			sessions = append(sessions, session)
		}
	}
	sm.mutex.RUnlock()

	sm.logger.Infof("Retrieved %d sessions for app domain: %s", len(sessions), appDomain)
	return sessions
}

// IsSessionAuthenticated checks if a session is authenticated
func (sm *SessionManager) IsSessionAuthenticated(sessionID string) (bool, error) {
	sm.logger.Infof("Checking authentication status for session: %s", sessionID)

	session, err := sm.GetSession(sessionID)
	if err != nil {
		return false, fmt.Errorf("failed to get session: %v", err)
	}

	return session.Authenticated, nil
}

// ExtendSession extends a session's expiration time
func (sm *SessionManager) ExtendSession(sessionID string, duration time.Duration) error {
	sm.logger.Infof("Extending session: %s by %v", sessionID, duration)

	session, err := sm.GetSession(sessionID)
	if err != nil {
		return fmt.Errorf("failed to get session: %v", err)
	}

	// Extend expiration time
	session.ExpiresAt = time.Now().Add(duration)
	session.LastActivity = time.Now()

	sm.logger.Infof("Session extended successfully: %s", sessionID)
	return nil
}

// startCleanupRoutine starts the session cleanup routine
func (sm *SessionManager) startCleanupRoutine() {
	sm.logger.Info("Starting session cleanup routine")

	// Run cleanup every 5 minutes
	sm.cleanupTicker = time.NewTicker(5 * time.Minute)

	go func() {
		for range sm.cleanupTicker.C {
			sm.cleanupExpiredSessions()
		}
	}()
}

// cleanupExpiredSessions removes expired sessions
func (sm *SessionManager) cleanupExpiredSessions() {
	sm.logger.Info("Cleaning up expired sessions")

	now := time.Now()
	expiredSessions := make([]string, 0)

	sm.mutex.RLock()
	for sessionID, session := range sm.sessions {
		if now.After(session.ExpiresAt) {
			expiredSessions = append(expiredSessions, sessionID)
		}
	}
	sm.mutex.RUnlock()

	// Delete expired sessions
	sm.mutex.Lock()
	for _, sessionID := range expiredSessions {
		delete(sm.sessions, sessionID)
	}
	sm.mutex.Unlock()

	if len(expiredSessions) > 0 {
		sm.logger.Infof("Cleaned up %d expired sessions", len(expiredSessions))
	}
}

// Stop stops the session manager
func (sm *SessionManager) Stop() {
	sm.logger.Info("Stopping session manager")

	if sm.cleanupTicker != nil {
		sm.cleanupTicker.Stop()
	}

	sm.logger.Info("Session manager stopped")
}
