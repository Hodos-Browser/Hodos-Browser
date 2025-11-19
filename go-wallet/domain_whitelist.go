package main

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"sync"
	"time"
)

// DomainWhitelistEntry represents a whitelisted domain
type DomainWhitelistEntry struct {
	Domain       string    `json:"domain"`
	AddedAt      time.Time `json:"addedAt"`
	LastUsed     time.Time `json:"lastUsed"`
	RequestCount int       `json:"requestCount"`
	IsPermanent  bool      `json:"isPermanent"` // true for whitelist, false for one-time
}

// DomainWhitelistManager manages the domain whitelist
type DomainWhitelistManager struct {
	whitelist map[string]*DomainWhitelistEntry
	mutex     sync.RWMutex
	filePath  string
}

// NewDomainWhitelistManager creates a new domain whitelist manager
func NewDomainWhitelistManager() *DomainWhitelistManager {
	homeDir, _ := os.UserHomeDir()
	filePath := filepath.Join(homeDir, "AppData", "Roaming", "BabbageBrowser", "wallet", "domainWhitelist.json")

	manager := &DomainWhitelistManager{
		whitelist: make(map[string]*DomainWhitelistEntry),
		filePath:  filePath,
	}

	// Load existing whitelist
	manager.loadWhitelist()

	return manager
}

// loadWhitelist loads the whitelist from file
func (dwm *DomainWhitelistManager) loadWhitelist() error {
	dwm.mutex.Lock()
	defer dwm.mutex.Unlock()

	// Create directory if it doesn't exist
	dir := filepath.Dir(dwm.filePath)
	if err := os.MkdirAll(dir, 0755); err != nil {
		return fmt.Errorf("failed to create directory: %v", err)
	}

	// Check if file exists
	if _, err := os.Stat(dwm.filePath); os.IsNotExist(err) {
		// File doesn't exist, start with empty whitelist
		return nil
	}

	// Read file
	data, err := os.ReadFile(dwm.filePath)
	if err != nil {
		return fmt.Errorf("failed to read whitelist file: %v", err)
	}

	// Parse JSON
	var entries []DomainWhitelistEntry
	if err := json.Unmarshal(data, &entries); err != nil {
		return fmt.Errorf("failed to parse whitelist file: %v", err)
	}

	// Convert to map
	for _, entry := range entries {
		dwm.whitelist[entry.Domain] = &entry
	}

	return nil
}

// saveWhitelist saves the whitelist to file
// Note: Caller must hold the mutex
func (dwm *DomainWhitelistManager) saveWhitelist() error {

	// Convert map to slice
	var entries []DomainWhitelistEntry
	for _, entry := range dwm.whitelist {
		entries = append(entries, *entry)
	}

	// Marshal to JSON
	data, err := json.MarshalIndent(entries, "", "  ")
	if err != nil {
		return fmt.Errorf("failed to marshal whitelist: %v", err)
	}

	// Write to file
	if err := os.WriteFile(dwm.filePath, data, 0644); err != nil {
		return fmt.Errorf("failed to write whitelist file: %v", err)
	}

	return nil
}

// IsDomainWhitelisted checks if a domain is whitelisted
func (dwm *DomainWhitelistManager) IsDomainWhitelisted(domain string) bool {
	dwm.mutex.RLock()
	defer dwm.mutex.RUnlock()

	entry, exists := dwm.whitelist[domain]
	if !exists {
		return false
	}

	// Check if it's a one-time entry that has been used
	if !entry.IsPermanent && entry.RequestCount > 0 {
		return false
	}

	return true
}

// AddToWhitelist adds a domain to the whitelist
func (dwm *DomainWhitelistManager) AddToWhitelist(domain string, isPermanent bool) error {
	dwm.mutex.Lock()
	defer dwm.mutex.Unlock()

	now := time.Now()
	entry := &DomainWhitelistEntry{
		Domain:       domain,
		AddedAt:      now,
		LastUsed:     now,
		RequestCount: 0,
		IsPermanent:  isPermanent,
	}

	dwm.whitelist[domain] = entry

	// Save to file
	return dwm.saveWhitelist()
}

// RecordRequest records a request from a domain
func (dwm *DomainWhitelistManager) RecordRequest(domain string) error {
	dwm.mutex.Lock()
	defer dwm.mutex.Unlock()

	entry, exists := dwm.whitelist[domain]
	if !exists {
		return fmt.Errorf("domain not in whitelist: %s", domain)
	}

	entry.LastUsed = time.Now()
	entry.RequestCount++

	// Save to file
	return dwm.saveWhitelist()
}

// RemoveFromWhitelist removes a domain from the whitelist
func (dwm *DomainWhitelistManager) RemoveFromWhitelist(domain string) error {
	dwm.mutex.Lock()
	defer dwm.mutex.Unlock()

	delete(dwm.whitelist, domain)

	// Save to file
	return dwm.saveWhitelist()
}

// GetWhitelist returns the current whitelist
func (dwm *DomainWhitelistManager) GetWhitelist() map[string]*DomainWhitelistEntry {
	dwm.mutex.RLock()
	defer dwm.mutex.RUnlock()

	// Return a copy
	result := make(map[string]*DomainWhitelistEntry)
	for domain, entry := range dwm.whitelist {
		result[domain] = entry
	}

	return result
}

// GetWhitelistAsJSON returns the whitelist as JSON for API responses
func (dwm *DomainWhitelistManager) GetWhitelistAsJSON() ([]byte, error) {
	dwm.mutex.RLock()
	defer dwm.mutex.RUnlock()

	var entries []DomainWhitelistEntry
	for _, entry := range dwm.whitelist {
		entries = append(entries, *entry)
	}

	return json.MarshalIndent(entries, "", "  ")
}
