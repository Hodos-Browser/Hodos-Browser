package websocket

import (
	"time"
	"browser-wallet/brc100/authentication"
	"browser-wallet/brc100/identity"
)

// BRC100ProtocolMessage represents the base structure for all BRC-100 messages
type BRC100ProtocolMessage struct {
	Type      string                 `json:"type"`
	Data      map[string]interface{} `json:"data,omitempty"`
	Nonce     string                 `json:"nonce,omitempty"`
	Signature string                 `json:"signature,omitempty"`
	PeerID    string                 `json:"peerId,omitempty"`
	Timestamp time.Time              `json:"timestamp"`
	RequestID string                 `json:"requestId,omitempty"`
}

// BRC100AuthenticationRequest represents a BRC-100 authentication request
type BRC100AuthenticationRequest struct {
	Type            string `json:"type"`
	AppDomain       string `json:"appDomain"`
	Purpose         string `json:"purpose"`
	RequestID       string `json:"requestId"`
	Timestamp       time.Time `json:"timestamp"`
}

// BRC100AuthenticationResponse represents a BRC-100 authentication response
type BRC100AuthenticationResponse struct {
	Type        string                          `json:"type"`
	Success     bool                            `json:"success"`
	SessionID   string                          `json:"sessionId,omitempty"`
	Certificate *identity.IdentityCertificate   `json:"certificate,omitempty"`
	Type42Keys  *authentication.Type42Keys      `json:"type42Keys,omitempty"`
	Message     string                          `json:"message,omitempty"`
	RequestID   string                          `json:"requestId"`
	Timestamp   time.Time                       `json:"timestamp"`
}

// BRC100PaymentRequest represents a BRC-100 payment request
type BRC100PaymentRequest struct {
	Type        string  `json:"type"`
	Amount      float64 `json:"amount"`
	Currency    string  `json:"currency"`
	Recipient   string  `json:"recipient"`
	Description string  `json:"description"`
	RequestID   string  `json:"requestId"`
	Timestamp   time.Time `json:"timestamp"`
}

// BRC100PaymentResponse represents a BRC-100 payment response
type BRC100PaymentResponse struct {
	Type          string    `json:"type"`
	Success       bool      `json:"success"`
	TransactionID string    `json:"transactionId,omitempty"`
	BEEFData      string    `json:"beefData,omitempty"`
	Message       string    `json:"message,omitempty"`
	RequestID     string    `json:"requestId"`
	Timestamp     time.Time `json:"timestamp"`
}

// BRC100IdentityRequest represents a BRC-100 identity request
type BRC100IdentityRequest struct {
	Type      string   `json:"type"`
	Fields    []string `json:"fields"`
	RequestID string   `json:"requestId"`
	Timestamp time.Time `json:"timestamp"`
}

// BRC100IdentityResponse represents a BRC-100 identity response
type BRC100IdentityResponse struct {
	Type      string                 `json:"type"`
	Identity  map[string]interface{} `json:"identity"`
	RequestID string                 `json:"requestId"`
	Timestamp time.Time              `json:"timestamp"`
}

// BRC100ErrorResponse represents a BRC-100 error response
type BRC100ErrorResponse struct {
	Type      string    `json:"type"`
	Error     string    `json:"error"`
	Code      int       `json:"code"`
	RequestID string    `json:"requestId"`
	Timestamp time.Time `json:"timestamp"`
}

// BRC100ChallengeRequest represents a BRC-100 challenge request
type BRC100ChallengeRequest struct {
	Type      string `json:"type"`
	AppDomain string `json:"appDomain"`
	RequestID string `json:"requestId"`
	Timestamp time.Time `json:"timestamp"`
}

// BRC100ChallengeResponse represents a BRC-100 challenge response
type BRC100ChallengeResponse struct {
	Type        string `json:"type"`
	ChallengeID string `json:"challengeId"`
	Challenge   string `json:"challenge"`
	RequestID   string `json:"requestId"`
	Timestamp   time.Time `json:"timestamp"`
}

// BRC100ChallengeVerification represents a BRC-100 challenge verification
type BRC100ChallengeVerification struct {
	Type        string `json:"type"`
	ChallengeID string `json:"challengeId"`
	Response    string `json:"response"`
	Signature   string `json:"signature"`
	RequestID   string `json:"requestId"`
	Timestamp   time.Time `json:"timestamp"`
}

// BRC100SessionStatus represents BRC-100 session status
type BRC100SessionStatus struct {
	Type        string                 `json:"type"`
	SessionID   string                 `json:"sessionId"`
	Status      string                 `json:"status"`
	Permissions []string               `json:"permissions"`
	Data        map[string]interface{} `json:"data,omitempty"`
	RequestID   string                 `json:"requestId"`
	Timestamp   time.Time              `json:"timestamp"`
}
