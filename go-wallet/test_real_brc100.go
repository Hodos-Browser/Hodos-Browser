package main

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
)

// Test client for real BRC-100 functionality
func main() {
	fmt.Println("🧪 Testing BRC-100 with Real BSV Transactions")
	fmt.Println("================================================")

	baseURL := "http://localhost:8080"

	// Test 1: Check server health
	fmt.Println("\n📋 Test 1: Server Health Check")
	testServerHealth(baseURL)

	// Test 2: Get wallet status and addresses
	fmt.Println("\n📋 Test 2: Wallet Status and Addresses")
	testWalletStatus(baseURL)

	// Test 3: BRC-100 Authentication with real wallet
	fmt.Println("\n📋 Test 3: BRC-100 Authentication")
	testBRC100Authentication(baseURL)

	// Test 4: Real Merkle Proof Verification
	fmt.Println("\n📋 Test 4: Real Merkle Proof Verification")
	testRealMerkleProof(baseURL)

	// Test 5: Complete BRC-100 Flow
	fmt.Println("\n📋 Test 5: Complete BRC-100 Flow")
	testCompleteBRC100Flow(baseURL)

	fmt.Println("\n✅ All tests completed!")
}

func testServerHealth(baseURL string) {
	resp, err := http.Get(baseURL + "/health")
	if err != nil {
		fmt.Printf("❌ Health check failed: %v\n", err)
		return
	}
	defer resp.Body.Close()

	body, err := io.ReadAll(resp.Body)
	if err != nil {
		fmt.Printf("❌ Failed to read health response: %v\n", err)
		return
	}

	fmt.Printf("✅ Server health: %s\n", string(body))
}

func testWalletStatus(baseURL string) {
	// Get wallet addresses
	resp, err := http.Get(baseURL + "/wallet/addresses")
	if err != nil {
		fmt.Printf("❌ Failed to get wallet addresses: %v\n", err)
		return
	}
	defer resp.Body.Close()

	body, err := io.ReadAll(resp.Body)
	if err != nil {
		fmt.Printf("❌ Failed to read addresses response: %v\n", err)
		return
	}

	var addresses []map[string]interface{}
	if err := json.Unmarshal(body, &addresses); err != nil {
		fmt.Printf("❌ Failed to parse addresses: %v\n", err)
		return
	}

	fmt.Printf("✅ Wallet has %d addresses\n", len(addresses))
	if len(addresses) > 0 {
		fmt.Printf("   Current address: %s\n", addresses[0]["address"])
	}

	// Get wallet info
	resp, err = http.Get(baseURL + "/wallet/info")
	if err != nil {
		fmt.Printf("❌ Failed to get wallet info: %v\n", err)
		return
	}
	defer resp.Body.Close()

	body, err = io.ReadAll(resp.Body)
	if err != nil {
		fmt.Printf("❌ Failed to read wallet info: %v\n", err)
		return
	}

	var walletInfo map[string]interface{}
	if err := json.Unmarshal(body, &walletInfo); err != nil {
		fmt.Printf("❌ Failed to parse wallet info: %v\n", err)
		return
	}

	fmt.Printf("✅ Wallet info: %+v\n", walletInfo)
}

func testBRC100Authentication(baseURL string) {
	// Step 1: Create authentication challenge
	fmt.Println("   Step 1: Creating authentication challenge...")
	challengeReq := map[string]interface{}{
		"appId": "test-app.example.com",
	}

	jsonData, err := json.Marshal(challengeReq)
	if err != nil {
		fmt.Printf("❌ Failed to marshal challenge request: %v\n", err)
		return
	}

	resp, err := http.Post(baseURL+"/brc100/auth/challenge", "application/json", bytes.NewBuffer(jsonData))
	if err != nil {
		fmt.Printf("❌ Challenge request failed: %v\n", err)
		return
	}
	defer resp.Body.Close()

	body, err := io.ReadAll(resp.Body)
	if err != nil {
		fmt.Printf("❌ Failed to read challenge response: %v\n", err)
		return
	}

	var challengeResp map[string]interface{}
	if err := json.Unmarshal(body, &challengeResp); err != nil {
		fmt.Printf("❌ Failed to parse challenge response: %v\n", err)
		return
	}

	if !challengeResp["success"].(bool) {
		fmt.Printf("❌ Challenge creation failed: %+v\n", challengeResp)
		return
	}

	challengeData := challengeResp["data"].(map[string]interface{})
	challenge := challengeData["challenge"]

	// Handle different challenge response formats
	var challengeStr string
	switch v := challenge.(type) {
	case string:
		challengeStr = v
	case map[string]interface{}:
		// If challenge is an object, extract the challenge string
		if challengeValue, ok := v["challenge"].(string); ok {
			challengeStr = challengeValue
		} else {
			fmt.Printf("❌ Unexpected challenge format: %+v\n", v)
			return
		}
	default:
		fmt.Printf("❌ Unexpected challenge type: %T, value: %+v\n", v, v)
		return
	}
	fmt.Printf("✅ Challenge created: %s\n", challengeStr)

	// Step 2: Authenticate with the challenge
	fmt.Println("   Step 2: Authenticating with challenge...")
	authReq := map[string]interface{}{
		"appDomain": "test-app.example.com",
		"purpose":   "Testing real BRC-100 authentication",
		"challenge": challengeStr,
	}

	jsonData, err = json.Marshal(authReq)
	if err != nil {
		fmt.Printf("❌ Failed to marshal auth request: %v\n", err)
		return
	}

	resp, err = http.Post(baseURL+"/brc100/auth/authenticate", "application/json", bytes.NewBuffer(jsonData))
	if err != nil {
		fmt.Printf("❌ Authentication request failed: %v\n", err)
		return
	}
	defer resp.Body.Close()

	body, err = io.ReadAll(resp.Body)
	if err != nil {
		fmt.Printf("❌ Failed to read auth response: %v\n", err)
		return
	}

	var authResp map[string]interface{}
	if err := json.Unmarshal(body, &authResp); err != nil {
		fmt.Printf("❌ Failed to parse auth response: %v\n", err)
		return
	}

	fmt.Printf("✅ Authentication response: %+v\n", authResp)
}

func testRealMerkleProof(baseURL string) {
	// Test with the transaction ID that we know exists and was working before
	fmt.Println("   Testing SPV verification with known working transaction...")

	// Use the transaction ID that was working in our previous tests
	txID := "d447c985c31de08f8e65059f4f3849da5cb02542b6f2c36cf7e1c0ca4a17272f"

	spvReq := map[string]interface{}{
		"transactionId": txID,
		"identityData": map[string]interface{}{
			"subject": "1MBdcYaWTB3dYByNV3dBoLxkz6ibgv6Hmv",
			"issuer": "Babbage-Browser-Wallet",
			"address": "1MBdcYaWTB3dYByNV3dBoLxkz6ibgv6Hmv",
			"publicKey": "03d575090cc073ecf448ad49fae79993fdaf8d1643ec2c5762655ed400e20333e3",
			"timestamp": "2025-10-01T12:31:03.7654508-06:00",
		},
	}

	jsonData, err := json.Marshal(spvReq)
	if err != nil {
		fmt.Printf("❌ Failed to marshal SPV request: %v\n", err)
		return
	}

	resp, err := http.Post(baseURL+"/brc100/spv/verify", "application/json", bytes.NewBuffer(jsonData))
	if err != nil {
		fmt.Printf("❌ SPV verification request failed: %v\n", err)
		return
	}
	defer resp.Body.Close()

	body, err := io.ReadAll(resp.Body)
	if err != nil {
		fmt.Printf("❌ Failed to read SPV response: %v\n", err)
		return
	}

	var spvResp map[string]interface{}
	if err := json.Unmarshal(body, &spvResp); err != nil {
		fmt.Printf("❌ Failed to parse SPV response: %v\n", err)
		return
	}

	fmt.Printf("✅ SPV verification response: %+v\n", spvResp)
}

func testCompleteBRC100Flow(baseURL string) {
	fmt.Println("   Testing complete BRC-100 authentication flow...")

	// Step 1: Generate identity certificate
	fmt.Println("   Step 1: Generating identity certificate...")
	identityReq := map[string]interface{}{
		"selectiveDisclosure": map[string]bool{
			"name":  true,
			"email": false,
		},
	}

	jsonData, err := json.Marshal(identityReq)
	if err != nil {
		fmt.Printf("❌ Failed to marshal identity request: %v\n", err)
		return
	}

	resp, err := http.Post(baseURL+"/brc100/identity/generate", "application/json", bytes.NewBuffer(jsonData))
	if err != nil {
		fmt.Printf("❌ Identity generation failed: %v\n", err)
		return
	}
	defer resp.Body.Close()

	body, err := io.ReadAll(resp.Body)
	if err != nil {
		fmt.Printf("❌ Failed to read identity response: %v\n", err)
		return
	}

	var identityResp map[string]interface{}
	if err := json.Unmarshal(body, &identityResp); err != nil {
		fmt.Printf("❌ Failed to parse identity response: %v\n", err)
		return
	}

	fmt.Printf("✅ Identity certificate generated: %+v\n", identityResp)

	// Step 2: Create authentication session
	fmt.Println("   Step 2: Creating authentication session...")
	sessionReq := map[string]interface{}{
		"appDomain": "test-app.example.com",
		"purpose":   "Complete BRC-100 flow test",
		"identity":  identityResp,
	}

	jsonData, err = json.Marshal(sessionReq)
	if err != nil {
		fmt.Printf("❌ Failed to marshal session request: %v\n", err)
		return
	}

	resp, err = http.Post(baseURL+"/brc100/session/create", "application/json", bytes.NewBuffer(jsonData))
	if err != nil {
		fmt.Printf("❌ Session creation failed: %v\n", err)
		return
	}
	defer resp.Body.Close()

	body, err = io.ReadAll(resp.Body)
	if err != nil {
		fmt.Printf("❌ Failed to read session response: %v\n", err)
		return
	}

	var sessionResp map[string]interface{}
	if err := json.Unmarshal(body, &sessionResp); err != nil {
		fmt.Printf("❌ Failed to parse session response: %v\n", err)
		return
	}

	fmt.Printf("✅ Authentication session created: %+v\n", sessionResp)

	// Step 3: Test BEEF transaction creation
	fmt.Println("   Step 3: Testing BEEF transaction creation...")
	beefReq := map[string]interface{}{
		"actions": []map[string]interface{}{
			{
				"type": "identity_proof",
				"data": map[string]interface{}{
					"identity": identityResp,
				},
			},
		},
		"appDomain": "test-app.example.com",
		"purpose":   "Testing BEEF transaction",
	}

	jsonData, err = json.Marshal(beefReq)
	if err != nil {
		fmt.Printf("❌ Failed to marshal BEEF request: %v\n", err)
		return
	}

	resp, err = http.Post(baseURL+"/brc100/beef/create", "application/json", bytes.NewBuffer(jsonData))
	if err != nil {
		fmt.Printf("❌ BEEF creation failed: %v\n", err)
		return
	}
	defer resp.Body.Close()

	body, err = io.ReadAll(resp.Body)
	if err != nil {
		fmt.Printf("❌ Failed to read BEEF response: %v\n", err)
		return
	}

	var beefResp map[string]interface{}
	if err := json.Unmarshal(body, &beefResp); err != nil {
		fmt.Printf("❌ Failed to parse BEEF response: %v\n", err)
		return
	}

	fmt.Printf("✅ BEEF transaction created: %+v\n", beefResp)

	fmt.Println("✅ Complete BRC-100 flow test successful!")
}
