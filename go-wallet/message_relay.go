package main

import (
	"encoding/json"
	"net/http"
	"sync"
	"time"

	"github.com/sirupsen/logrus"
)

// Message represents a BRC-33 PeerServ message
type Message struct {
	MessageID  int64  `json:"messageId"`
	Body       string `json:"body"`
	Sender     string `json:"sender"`
	MessageBox string `json:"messageBox"`
	Recipient  string `json:"recipient"`
	Timestamp  int64  `json:"timestamp"`
}

// MessageStore manages in-memory message storage for BRC-33 PeerServ
type MessageStore struct {
	messages      map[string][]Message // key: recipient_pubkey
	nextMessageID int64
	mu            sync.RWMutex
	logger        *logrus.Logger
}

// NewMessageStore creates a new in-memory message store
func NewMessageStore(logger *logrus.Logger) *MessageStore {
	return &MessageStore{
		messages:      make(map[string][]Message),
		nextMessageID: 1,
		logger:        logger,
	}
}

// SendMessage stores a message for a recipient (BRC-33 /sendMessage)
func (ms *MessageStore) SendMessage(sender, recipient, messageBox, body string) (int64, error) {
	ms.mu.Lock()
	defer ms.mu.Unlock()

	// Generate unique message ID
	messageID := ms.nextMessageID
	ms.nextMessageID++

	// Create message
	msg := Message{
		MessageID:  messageID,
		Body:       body,
		Sender:     sender,
		MessageBox: messageBox,
		Recipient:  recipient,
		Timestamp:  time.Now().Unix(),
	}

	// Store message for recipient
	ms.messages[recipient] = append(ms.messages[recipient], msg)

	ms.logger.WithFields(logrus.Fields{
		"messageId":  messageID,
		"sender":     sender,
		"recipient":  recipient,
		"messageBox": messageBox,
	}).Info("📬 Message stored")

	return messageID, nil
}

// ListMessages retrieves messages for a recipient from a specific message box (BRC-33 /listMessages)
func (ms *MessageStore) ListMessages(recipient, messageBox string) ([]Message, error) {
	ms.mu.RLock()
	defer ms.mu.RUnlock()

	// Get all messages for this recipient
	allMessages, exists := ms.messages[recipient]
	if !exists {
		ms.logger.WithFields(logrus.Fields{
			"recipient":  recipient,
			"messageBox": messageBox,
		}).Info("📭 No messages found for recipient")
		return []Message{}, nil
	}

	// Filter by message box
	var filteredMessages []Message
	for _, msg := range allMessages {
		if msg.MessageBox == messageBox {
			filteredMessages = append(filteredMessages, msg)
		}
	}

	ms.logger.WithFields(logrus.Fields{
		"recipient":  recipient,
		"messageBox": messageBox,
		"count":      len(filteredMessages),
	}).Info("📬 Messages retrieved")

	return filteredMessages, nil
}

// AcknowledgeMessages deletes messages by ID for a recipient (BRC-33 /acknowledgeMessage)
func (ms *MessageStore) AcknowledgeMessages(recipient string, messageIDs []int64) error {
	ms.mu.Lock()
	defer ms.mu.Unlock()

	// Get all messages for this recipient
	allMessages, exists := ms.messages[recipient]
	if !exists {
		ms.logger.WithFields(logrus.Fields{
			"recipient":  recipient,
			"messageIds": messageIDs,
		}).Warn("⚠️ No messages found for recipient to acknowledge")
		return nil
	}

	// Create a set of message IDs to delete
	deleteSet := make(map[int64]bool)
	for _, id := range messageIDs {
		deleteSet[id] = true
	}

	// Filter out acknowledged messages
	var remainingMessages []Message
	deletedCount := 0
	for _, msg := range allMessages {
		if deleteSet[msg.MessageID] {
			deletedCount++
		} else {
			remainingMessages = append(remainingMessages, msg)
		}
	}

	// Update message store
	if len(remainingMessages) == 0 {
		delete(ms.messages, recipient)
	} else {
		ms.messages[recipient] = remainingMessages
	}

	ms.logger.WithFields(logrus.Fields{
		"recipient":     recipient,
		"deletedCount":  deletedCount,
		"remainingCount": len(remainingMessages),
	}).Info("✅ Messages acknowledged and deleted")

	return nil
}

// GetMessageCount returns the total number of messages stored
func (ms *MessageStore) GetMessageCount() int {
	ms.mu.RLock()
	defer ms.mu.RUnlock()

	count := 0
	for _, messages := range ms.messages {
		count += len(messages)
	}
	return count
}

// HandleSendMessage handles POST /sendMessage endpoint
func HandleSendMessage(messageStore *MessageStore, walletService *WalletService) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		// Handle CORS preflight
		w.Header().Set("Access-Control-Allow-Origin", "*")
		w.Header().Set("Access-Control-Allow-Methods", "POST, OPTIONS")
		w.Header().Set("Access-Control-Allow-Headers", "Content-Type, Authorization")

		if r.Method == "OPTIONS" {
			w.WriteHeader(http.StatusOK)
			return
		}

		if r.Method != "POST" {
			http.Error(w, "Method not allowed", http.StatusMethodNotAllowed)
			return
		}

		// Parse request body
		var req struct {
			Message struct {
				Recipient  string `json:"recipient"`
				MessageBox string `json:"messageBox"`
				Body       string `json:"body"`
			} `json:"message"`
		}

		if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
			messageStore.logger.WithError(err).Error("Failed to parse /sendMessage request")
			http.Error(w, "Invalid request body", http.StatusBadRequest)
			return
		}

		// Get sender from authenticated session (BRC-31)
		// For now, use the wallet's public key as sender
		currentAddress, err := walletService.walletManager.GetCurrentAddress()
		if err != nil {
			messageStore.logger.WithError(err).Error("Failed to get sender identity")
			http.Error(w, "Failed to get sender identity", http.StatusInternalServerError)
			return
		}
		sender := currentAddress.PublicKey

		// Store message
		messageID, err := messageStore.SendMessage(sender, req.Message.Recipient, req.Message.MessageBox, req.Message.Body)
		if err != nil {
			messageStore.logger.WithError(err).Error("Failed to store message")
			http.Error(w, "Failed to store message", http.StatusInternalServerError)
			return
		}

		// Return success
		response := map[string]interface{}{
			"status":    "success",
			"messageId": messageID,
		}

		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(response)
	}
}

// HandleListMessages handles POST /listMessages endpoint
func HandleListMessages(messageStore *MessageStore, walletService *WalletService) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		// Handle CORS preflight
		w.Header().Set("Access-Control-Allow-Origin", "*")
		w.Header().Set("Access-Control-Allow-Methods", "POST, OPTIONS")
		w.Header().Set("Access-Control-Allow-Headers", "Content-Type, Authorization")

		if r.Method == "OPTIONS" {
			w.WriteHeader(http.StatusOK)
			return
		}

		if r.Method != "POST" {
			http.Error(w, "Method not allowed", http.StatusMethodNotAllowed)
			return
		}

		// Parse request body
		var req struct {
			MessageBox string `json:"messageBox"`
		}

		if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
			messageStore.logger.WithError(err).Error("Failed to parse /listMessages request")
			http.Error(w, "Invalid request body", http.StatusBadRequest)
			return
		}

		// Get recipient from authenticated session (BRC-31)
		// For now, use the wallet's public key as recipient
		currentAddress, err := walletService.walletManager.GetCurrentAddress()
		if err != nil {
			messageStore.logger.WithError(err).Error("Failed to get recipient identity")
			http.Error(w, "Failed to get recipient identity", http.StatusInternalServerError)
			return
		}
		recipient := currentAddress.PublicKey

		// Retrieve messages
		messages, err := messageStore.ListMessages(recipient, req.MessageBox)
		if err != nil {
			messageStore.logger.WithError(err).Error("Failed to retrieve messages")
			http.Error(w, "Failed to retrieve messages", http.StatusInternalServerError)
			return
		}

		// Format response (only include fields client expects)
		var formattedMessages []map[string]interface{}
		for _, msg := range messages {
			formattedMessages = append(formattedMessages, map[string]interface{}{
				"messageId": msg.MessageID,
				"body":      msg.Body,
				"sender":    msg.Sender,
			})
		}

		// Return success
		response := map[string]interface{}{
			"status":   "success",
			"messages": formattedMessages,
		}

		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(response)
	}
}

// HandleAcknowledgeMessage handles POST /acknowledgeMessage endpoint
func HandleAcknowledgeMessage(messageStore *MessageStore, walletService *WalletService) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		// Handle CORS preflight
		w.Header().Set("Access-Control-Allow-Origin", "*")
		w.Header().Set("Access-Control-Allow-Methods", "POST, OPTIONS")
		w.Header().Set("Access-Control-Allow-Headers", "Content-Type, Authorization")

		if r.Method == "OPTIONS" {
			w.WriteHeader(http.StatusOK)
			return
		}

		if r.Method != "POST" {
			http.Error(w, "Method not allowed", http.StatusMethodNotAllowed)
			return
		}

		// Parse request body
		var req struct {
			MessageIDs []int64 `json:"messageIds"`
		}

		if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
			messageStore.logger.WithError(err).Error("Failed to parse /acknowledgeMessage request")
			http.Error(w, "Invalid request body", http.StatusBadRequest)
			return
		}

		// Get recipient from authenticated session (BRC-31)
		// For now, use the wallet's public key as recipient
		currentAddress, err := walletService.walletManager.GetCurrentAddress()
		if err != nil {
			messageStore.logger.WithError(err).Error("Failed to get recipient identity")
			http.Error(w, "Failed to get recipient identity", http.StatusInternalServerError)
			return
		}
		recipient := currentAddress.PublicKey

		// Acknowledge messages
		if err := messageStore.AcknowledgeMessages(recipient, req.MessageIDs); err != nil {
			messageStore.logger.WithError(err).Error("Failed to acknowledge messages")
			http.Error(w, "Failed to acknowledge messages", http.StatusInternalServerError)
			return
		}

		// Return success
		response := map[string]interface{}{
			"status": "success",
		}

		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(response)
	}
}
