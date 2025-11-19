package main

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"time"
)

// Test client for enhanced BEEF creation with SPV data
func main() {
	fmt.Println("🧪 Testing Enhanced BEEF Creation with SPV Data")
	fmt.Println("================================================")

	baseURL := "http://localhost:8080"

	testServerHealth(baseURL)
	testEnhancedBEEFCreation(baseURL)
	testSPVDataInfo(baseURL)

	fmt.Println("\n✅ All BEEF SPV tests completed!")
}

func testServerHealth(baseURL string) {
	fmt.Println("\n📋 Test 1: Server Health Check")
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

	var healthResp map[string]string
	if err := json.Unmarshal(body, &healthResp); err != nil {
		fmt.Printf("❌ Failed to parse health response: %v\n", err)
		return
	}

	fmt.Printf("✅ Server health: %+v\n", healthResp)
}

func testEnhancedBEEFCreation(baseURL string) {
	fmt.Println("\n📋 Test 2: Enhanced BEEF Creation with SPV Data")

	// Test 2.1: Create BEEF without SPV data
	fmt.Println("   Step 2.1: Creating BEEF transaction without SPV data...")
	beefReqWithoutSPV := map[string]interface{}{
		"actions": []map[string]interface{}{
			{
				"type": "identity_proof",
				"data": map[string]interface{}{
					"subject": "1MBdcYaWTB3dYByNV3dBoLxkz6ibgv6Hmv",
					"issuer":  "Babbage-Browser-Wallet",
					"timestamp": time.Now().Format(time.RFC3339),
				},
			},
		},
		"sessionId":      "test_session_123",
		"appDomain":      "test-app.example.com",
		"includeSPVData": false,
	}

	testBEEFCreation(baseURL, beefReqWithoutSPV, "without SPV data")

	// Test 2.2: Create BEEF with SPV data
	fmt.Println("   Step 2.2: Creating BEEF transaction with SPV data...")
	beefReqWithSPV := map[string]interface{}{
		"actions": []map[string]interface{}{
			{
				"type": "identity_proof",
				"data": map[string]interface{}{
					"subject": "1MBdcYaWTB3dYByNV3dBoLxkz6ibgv6Hmv",
					"issuer":  "Babbage-Browser-Wallet",
					"timestamp": time.Now().Format(time.RFC3339),
				},
			},
		},
		"sessionId":      "test_session_456",
		"appDomain":      "test-app.example.com",
		"includeSPVData": true,
	}

	testBEEFCreation(baseURL, beefReqWithSPV, "with SPV data")
}

func testBEEFCreation(baseURL string, request map[string]interface{}, description string) {
	jsonData, err := json.Marshal(request)
	if err != nil {
		fmt.Printf("❌ Failed to marshal BEEF request: %v\n", err)
		return
	}

	resp, err := http.Post(baseURL+"/brc100/beef/create", "application/json", bytes.NewBuffer(jsonData))
	if err != nil {
		fmt.Printf("❌ BEEF creation request failed: %v\n", err)
		return
	}
	defer resp.Body.Close()

	body, err := io.ReadAll(resp.Body)
	if err != nil {
		fmt.Printf("❌ Failed to read BEEF response: %v\n", err)
		return
	}

	var beefResp map[string]interface{}
	if err := json.Unmarshal(body, &beefResp); err != nil {
		fmt.Printf("❌ Failed to parse BEEF response: %v\n", err)
		return
	}

	if !beefResp["success"].(bool) {
		fmt.Printf("❌ BEEF creation failed %s: %+v\n", description, beefResp)
		return
	}

	beefTx := beefResp["data"].(map[string]interface{})["beefTransaction"].(map[string]interface{})

	fmt.Printf("✅ BEEF transaction created %s successfully\n", description)
	fmt.Printf("   Session ID: %s\n", beefTx["sessionId"])
	fmt.Printf("   App Domain: %s\n", beefTx["appDomain"])
	fmt.Printf("   Actions Count: %v\n", len(beefTx["actions"].([]interface{})))

	// Check if SPV data is present
	if spvData, exists := beefTx["spvData"]; exists && spvData != nil {
		spvMap := spvData.(map[string]interface{})
		fmt.Printf("   SPV Data: ✅ Present\n")
		fmt.Printf("     Merkle Proofs: %v\n", spvMap["merkleProofs"])
		fmt.Printf("     Block Headers: %v\n", spvMap["blockHeaders"])
		fmt.Printf("     Transaction Data: %v\n", spvMap["transactionData"])
		fmt.Printf("     Identity Proofs: %v\n", spvMap["identityProofs"])
	} else {
		fmt.Printf("   SPV Data: ❌ Not present\n")
	}
}

func testSPVDataInfo(baseURL string) {
	fmt.Println("\n📋 Test 3: SPV Data Information")

	// First create a BEEF transaction with SPV data
	beefReq := map[string]interface{}{
		"actions": []map[string]interface{}{
			{
				"type": "identity_proof",
				"data": map[string]interface{}{
					"subject": "1MBdcYaWTB3dYByNV3dBoLxkz6ibgv6Hmv",
					"issuer":  "Babbage-Browser-Wallet",
					"timestamp": time.Now().Format(time.RFC3339),
				},
			},
		},
		"sessionId":      "test_session_spv_info",
		"appDomain":      "test-app.example.com",
		"includeSPVData": true,
	}

	jsonData, err := json.Marshal(beefReq)
	if err != nil {
		fmt.Printf("❌ Failed to marshal BEEF request: %v\n", err)
		return
	}

	resp, err := http.Post(baseURL+"/brc100/beef/create", "application/json", bytes.NewBuffer(jsonData))
	if err != nil {
		fmt.Printf("❌ BEEF creation request failed: %v\n", err)
		return
	}
	defer resp.Body.Close()

	body, err := io.ReadAll(resp.Body)
	if err != nil {
		fmt.Printf("❌ Failed to read BEEF response: %v\n", err)
		return
	}

	var beefResp map[string]interface{}
	if err := json.Unmarshal(body, &beefResp); err != nil {
		fmt.Printf("❌ Failed to parse BEEF response: %v\n", err)
		return
	}

	if !beefResp["success"].(bool) {
		fmt.Printf("❌ BEEF creation failed: %+v\n", beefResp)
		return
	}

	beefTx := beefResp["data"].(map[string]interface{})["beefTransaction"]

	// Now test SPV info endpoint
	fmt.Println("   Testing SPV data information endpoint...")
	spvInfoReq := map[string]interface{}{
		"beefTransaction": beefTx,
	}

	jsonData, err = json.Marshal(spvInfoReq)
	if err != nil {
		fmt.Printf("❌ Failed to marshal SPV info request: %v\n", err)
		return
	}

	resp, err = http.Post(baseURL+"/brc100/spv/info", "application/json", bytes.NewBuffer(jsonData))
	if err != nil {
		fmt.Printf("❌ SPV info request failed: %v\n", err)
		return
	}
	defer resp.Body.Close()

	body, err = io.ReadAll(resp.Body)
	if err != nil {
		fmt.Printf("❌ Failed to read SPV info response: %v\n", err)
		return
	}

	var spvInfoResp map[string]interface{}
	if err := json.Unmarshal(body, &spvInfoResp); err != nil {
		fmt.Printf("❌ Failed to parse SPV info response: %v\n", err)
		return
	}

	if !spvInfoResp["success"].(bool) {
		fmt.Printf("❌ SPV info request failed: %+v\n", spvInfoResp)
		return
	}

	spvInfo := spvInfoResp["data"].(map[string]interface{})["spvInfo"].(map[string]interface{})

	fmt.Printf("✅ SPV data information retrieved successfully\n")
	fmt.Printf("   Has SPV Data: %v\n", spvInfo["hasSPVData"])
	if spvInfo["hasSPVData"].(bool) {
		fmt.Printf("   Merkle Proofs: %v\n", spvInfo["merkleProofs"])
		fmt.Printf("   Block Headers: %v\n", spvInfo["blockHeaders"])
		fmt.Printf("   Transaction Data: %v\n", spvInfo["transactionData"])
		fmt.Printf("   Identity Proofs: %v\n", spvInfo["identityProofs"])
		fmt.Printf("   Verification Time: %v\n", spvInfo["verificationTime"])
		if txIDs, exists := spvInfo["transactionIds"]; exists {
			fmt.Printf("   Transaction IDs: %v\n", txIDs)
		}
	}
}
