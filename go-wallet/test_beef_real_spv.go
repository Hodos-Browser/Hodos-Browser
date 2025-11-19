package main

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"time"
)

// Test client for BEEF creation with real SPV data from blockchain
func main() {
	fmt.Println("🧪 Testing BEEF Creation with Real SPV Data")
	fmt.Println("=============================================")

	baseURL := "http://localhost:8080"

	testServerHealth(baseURL)
	testBEEFWithRealTransactionData(baseURL)
	testBEEFWithRealMerkleProofs(baseURL)

	fmt.Println("\n✅ All real SPV BEEF tests completed!")
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

func testBEEFWithRealTransactionData(baseURL string) {
	fmt.Println("\n📋 Test 2: BEEF Creation with Real Transaction Data")

	// Use the real transaction ID that we know exists and has confirmations
	realTxID := "d447c985c31de08f8e65059f4f3849da5cb02542b6f2c36cf7e1c0ca4a17272f"

	// Create BEEF transaction that references real blockchain data
	beefReq := map[string]interface{}{
		"actions": []map[string]interface{}{
			{
				"type": "identity_proof",
				"data": map[string]interface{}{
					"subject":     "1MBdcYaWTB3dYByNV3dBoLxkz6ibgv6Hmv",
					"issuer":      "Babbage-Browser-Wallet",
					"timestamp":   time.Now().Format(time.RFC3339),
					"transactionId": realTxID, // Reference to real transaction
					"purpose":     "Testing BEEF with real SPV data",
				},
			},
			{
				"type": "merkle_proof",
				"data": map[string]interface{}{
					"transactionId": realTxID,
					"purpose":       "SPV verification of transaction inclusion",
				},
			},
		},
		"sessionId":      "real_spv_session_123",
		"appDomain":      "test-app.example.com",
		"includeSPVData": true,
	}

	fmt.Printf("   Creating BEEF transaction with real transaction data: %s\n", realTxID)

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

	beefTx := beefResp["data"].(map[string]interface{})["beefTransaction"].(map[string]interface{})

	fmt.Printf("✅ BEEF transaction created with real transaction data successfully\n")
	fmt.Printf("   Session ID: %s\n", beefTx["sessionId"])
	fmt.Printf("   App Domain: %s\n", beefTx["appDomain"])
	fmt.Printf("   Actions Count: %v\n", len(beefTx["actions"].([]interface{})))

	// Check SPV data
	if spvData, exists := beefTx["spvData"]; exists && spvData != nil {
		spvMap := spvData.(map[string]interface{})
		fmt.Printf("   SPV Data: ✅ Present\n")
		fmt.Printf("     Merkle Proofs: %v\n", spvMap["merkleProofs"])
		fmt.Printf("     Block Headers: %v\n", spvMap["blockHeaders"])
		fmt.Printf("     Transaction Data: %v\n", spvMap["transactionData"])
		fmt.Printf("     Identity Proofs: %v\n", spvMap["identityProofs"])
		fmt.Printf("     Verification Time: %v\n", spvMap["verificationTime"])
	} else {
		fmt.Printf("   SPV Data: ❌ Not present\n")
	}

	// Test SPV info endpoint with this transaction
	testSPVInfoWithRealData(baseURL, beefTx)
}

func testBEEFWithRealMerkleProofs(baseURL string) {
	fmt.Println("\n📋 Test 3: BEEF Creation with Real Merkle Proofs")

	// Test SPV verification first to ensure we can get Merkle proofs
	realTxID := "d447c985c31de08f8e65059f4f3849da5cb02542b6f2c36cf7e1c0ca4a17272f"

	fmt.Printf("   Testing SPV verification for transaction: %s\n", realTxID)

	spvReq := map[string]interface{}{
		"transactionId": realTxID,
		"identityData": map[string]interface{}{
			"subject":   "1MBdcYaWTB3dYByNV3dBoLxkz6ibgv6Hmv",
			"issuer":    "Babbage-Browser-Wallet",
			"address":   "1MBdcYaWTB3dYByNV3dBoLxkz6ibgv6Hmv",
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

	// Now create BEEF transaction that should include this SPV data
	beefReq := map[string]interface{}{
		"actions": []map[string]interface{}{
			{
				"type": "spv_verified_identity",
				"data": map[string]interface{}{
					"transactionId": realTxID,
					"subject":       "1MBdcYaWTB3dYByNV3dBoLxkz6ibgv6Hmv",
					"issuer":        "Babbage-Browser-Wallet",
					"timestamp":     time.Now().Format(time.RFC3339),
					"spvVerified":   true,
				},
			},
		},
		"sessionId":      "real_merkle_session_456",
		"appDomain":      "test-app.example.com",
		"includeSPVData": true,
	}

	fmt.Printf("   Creating BEEF transaction with real Merkle proof data...\n")

	jsonData, err = json.Marshal(beefReq)
	if err != nil {
		fmt.Printf("❌ Failed to marshal BEEF request: %v\n", err)
		return
	}

	resp, err = http.Post(baseURL+"/brc100/beef/create", "application/json", bytes.NewBuffer(jsonData))
	if err != nil {
		fmt.Printf("❌ BEEF creation request failed: %v\n", err)
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

	if !beefResp["success"].(bool) {
		fmt.Printf("❌ BEEF creation failed: %+v\n", beefResp)
		return
	}

	beefTx := beefResp["data"].(map[string]interface{})["beefTransaction"].(map[string]interface{})

	fmt.Printf("✅ BEEF transaction created with real Merkle proof data successfully\n")
	fmt.Printf("   Session ID: %s\n", beefTx["sessionId"])

	// Check SPV data
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

func testSPVInfoWithRealData(baseURL string, beefTx map[string]interface{}) {
	fmt.Println("\n📋 Test 4: SPV Data Information with Real Data")

	fmt.Println("   Testing SPV data information endpoint with real transaction data...")

	spvInfoReq := map[string]interface{}{
		"beefTransaction": beefTx,
	}

	jsonData, err := json.Marshal(spvInfoReq)
	if err != nil {
		fmt.Printf("❌ Failed to marshal SPV info request: %v\n", err)
		return
	}

	resp, err := http.Post(baseURL+"/brc100/spv/info", "application/json", bytes.NewBuffer(jsonData))
	if err != nil {
		fmt.Printf("❌ SPV info request failed: %v\n", err)
		return
	}
	defer resp.Body.Close()

	body, err := io.ReadAll(resp.Body)
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
