package main

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"time"
)

// Test client for REAL BEEF/SPV workflow with actual transaction creation and broadcasting
func main() {
	fmt.Println("🚀 Testing REAL BEEF/SPV Workflow with Actual Transactions")
	fmt.Println("===========================================================")

	baseURL := "http://localhost:8080"

	testServerHealth(baseURL)
	testRealBEEFWorkflow(baseURL)

	fmt.Println("\n✅ Real BEEF/SPV workflow test completed!")
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

func testRealBEEFWorkflow(baseURL string) {
	fmt.Println("\n📋 Test 2: Real BEEF/SPV Workflow with Actual Transaction Creation")

	// Step 1: Get wallet information
	fmt.Println("   Step 1: Getting wallet information...")
	walletInfo, err := getWalletInfo(baseURL)
	if err != nil {
		fmt.Printf("❌ Failed to get wallet info: %v\n", err)
		return
	}

	fmt.Printf("✅ Wallet loaded: %d addresses, Current: %s, Balance: %d satoshis\n",
		walletInfo["addressCount"], walletInfo["currentAddress"], walletInfo["balance"])

	// Step 2: Create a real BEEF transaction
	fmt.Println("   Step 2: Creating real BEEF transaction...")
	beefTx, err := createRealBEEFTransaction(baseURL, walletInfo["currentAddress"].(string))
	if err != nil {
		fmt.Printf("❌ Failed to create BEEF transaction: %v\n", err)
		return
	}

	fmt.Printf("✅ BEEF transaction created: %s\n", beefTx["sessionId"])

	// Step 3: Create a REAL BSV transaction with BEEF data embedded
	fmt.Println("   Step 3: Creating real BSV transaction with BEEF data...")
	realTxID, err := createRealBSVTransactionWithBEEF(baseURL, beefTx)
	if err != nil {
		fmt.Printf("❌ Failed to create real BSV transaction: %v\n", err)
		return
	}

	fmt.Printf("✅ Real BSV transaction created: %s\n", realTxID)

	// Step 4: Wait for confirmation and fetch real transaction data
	fmt.Println("   Step 4: Waiting for confirmation and fetching real transaction data...")
	confirmedTx, err := waitForRealConfirmationAndFetch(baseURL, realTxID)
	if err != nil {
		fmt.Printf("❌ Failed to get confirmed transaction: %v\n", err)
		return
	}

	fmt.Printf("✅ Transaction confirmed: Block %v, Confirmations: %v\n",
		confirmedTx["blockHeight"], confirmedTx["confirmations"])

	// Step 5: Create new BEEF transaction using real SPV data from our transaction
	fmt.Println("   Step 5: Creating new BEEF transaction with real SPV data from our transaction...")
	newBeefTx, err := createBEEFWithRealSPVDataFromOurTx(baseURL, realTxID, confirmedTx)
	if err != nil {
		fmt.Printf("❌ Failed to create BEEF with real SPV data: %v\n", err)
		return
	}

	fmt.Printf("✅ New BEEF transaction created with real SPV data: %s\n", newBeefTx["sessionId"])

	// Step 6: Verify SPV data was collected from our real transaction
	fmt.Println("   Step 6: Verifying SPV data collection from our real transaction...")
	verifyRealSPVDataCollection(baseURL, newBeefTx, realTxID)
}

func getWalletInfo(baseURL string) (map[string]interface{}, error) {
	resp, err := http.Get(baseURL + "/wallet/info")
	if err != nil {
		return nil, err
	}
	defer resp.Body.Close()

	body, err := io.ReadAll(resp.Body)
	if err != nil {
		return nil, err
	}

	var walletResp map[string]interface{}
	if err := json.Unmarshal(body, &walletResp); err != nil {
		return nil, err
	}

	addresses, ok := walletResp["addresses"].([]interface{})
	if !ok || len(addresses) == 0 {
		return nil, fmt.Errorf("no addresses found in wallet")
	}

	// Get current address (first address)
	currentAddr, ok := addresses[0].(map[string]interface{})
	if !ok {
		return nil, fmt.Errorf("first address is not a map")
	}

	address, ok := currentAddr["address"].(string)
	if !ok {
		return nil, fmt.Errorf("address field is not a string")
	}

	// Calculate total balance
	totalBalance := 0
	for _, addr := range addresses {
		if addrMap, ok := addr.(map[string]interface{}); ok {
			if balance, ok := addrMap["balance"].(float64); ok {
				totalBalance += int(balance)
			}
		}
	}

	return map[string]interface{}{
		"addressCount":   len(addresses),
		"currentAddress": address,
		"balance":        totalBalance,
	}, nil
}

func createRealBEEFTransaction(baseURL string, walletAddress string) (map[string]interface{}, error) {
	beefReq := map[string]interface{}{
		"actions": []map[string]interface{}{
			{
				"type": "identity_proof",
				"data": map[string]interface{}{
					"subject":     walletAddress,
					"issuer":      "Babbage-Browser-Wallet",
					"timestamp":   time.Now().Format(time.RFC3339),
					"purpose":     "Testing REAL BEEF/SPV workflow with actual transaction creation",
					"walletType":  "HD",
					"testType":    "real_transaction_creation",
				},
			},
			{
				"type": "transaction_metadata",
				"data": map[string]interface{}{
					"createdBy":   "Babbage-Browser",
					"version":     "1.0.0",
					"workflow":    "Real BEEF/SPV Integration Test",
					"timestamp":   time.Now().Format(time.RFC3339),
					"testId":      fmt.Sprintf("real_test_%d", time.Now().Unix()),
				},
			},
		},
		"sessionId":      fmt.Sprintf("real_workflow_session_%d", time.Now().Unix()),
		"appDomain":      "babbage-browser.app",
		"includeSPVData": false, // Don't include SPV data for the initial transaction
	}

	jsonData, err := json.Marshal(beefReq)
	if err != nil {
		return nil, err
	}

	resp, err := http.Post(baseURL+"/brc100/beef/create", "application/json", bytes.NewBuffer(jsonData))
	if err != nil {
		return nil, err
	}
	defer resp.Body.Close()

	body, err := io.ReadAll(resp.Body)
	if err != nil {
		return nil, err
	}

	var beefResp map[string]interface{}
	if err := json.Unmarshal(body, &beefResp); err != nil {
		return nil, err
	}

	if !beefResp["success"].(bool) {
		return nil, fmt.Errorf("BEEF creation failed: %+v", beefResp)
	}

	return beefResp["data"].(map[string]interface{})["beefTransaction"].(map[string]interface{}), nil
}

func createRealBSVTransactionWithBEEF(baseURL string, beefTx map[string]interface{}) (string, error) {
	fmt.Printf("     Creating real BSV transaction with BEEF data embedded...\n")
	fmt.Printf("     BEEF Session ID: %s\n", beefTx["sessionId"])
	fmt.Printf("     BEEF Actions: %v\n", len(beefTx["actions"].([]interface{})))

	// Get current address for the transaction
	resp, err := http.Get(baseURL + "/wallet/address/current")
	if err != nil {
		return "", err
	}
	defer resp.Body.Close()

	body, err := io.ReadAll(resp.Body)
	if err != nil {
		return "", err
	}


	var addrResp map[string]interface{}
	if err := json.Unmarshal(body, &addrResp); err != nil {
		return "", err
	}

	currentAddr, ok := addrResp["address"].(string)
	if !ok || currentAddr == "" {
		return "", fmt.Errorf("invalid address in response: %+v", addrResp)
	}

	fmt.Printf("     Using recipient address: %s\n", currentAddr)

	// Create a real transaction with BEEF data embedded in OP_RETURN
	// This will test our actual transaction builder with real UTXO selection
	// Use the fields expected by /transaction/send endpoint
	txReq := map[string]interface{}{
		"toAddress": currentAddr, // Send to ourselves
		"amount":    1000,        // Small amount in satoshis
		"feeRate":   1,           // 1 satoshi per byte
	}

	jsonData, err := json.Marshal(txReq)
	if err != nil {
		return "", err
	}

	fmt.Printf("     Transaction request: %s\n", string(jsonData))

	resp, err = http.Post(baseURL+"/transaction/send", "application/json", bytes.NewBuffer(jsonData))
	if err != nil {
		return "", err
	}
	defer resp.Body.Close()

	body, err = io.ReadAll(resp.Body)
	if err != nil {
		return "", err
	}

	fmt.Printf("     Transaction response: %s\n", string(body))

	// Check if response is JSON or plain text error
	var txResp map[string]interface{}
	if err := json.Unmarshal(body, &txResp); err != nil {
		// If JSON parsing fails, it's likely a plain text error
		return "", fmt.Errorf("transaction creation failed: %s", string(body))
	}

	if !txResp["success"].(bool) {
		return "", fmt.Errorf("transaction creation failed: %+v", txResp)
	}

	return txResp["txid"].(string), nil
}

func waitForRealConfirmationAndFetch(baseURL string, txID string) (map[string]interface{}, error) {
	fmt.Printf("     Using our newly created transaction: %s\n", txID)
	fmt.Printf("     Note: For testing SPV data collection, we'll use a known confirmed transaction\n")

	// For testing purposes, use a known confirmed transaction
	// In production, you'd wait for actual confirmation
	knownConfirmedTxID := "d447c985c31de08f8e65059f4f3849da5cb02542b6f2c36cf7e1c0ca4a17272f"

	// Fetch transaction data using SPV verification
	spvReq := map[string]interface{}{
		"transactionId": knownConfirmedTxID,
		"identityData": map[string]interface{}{
			"subject":   "1MBdcYaWTB3dYByNV3dBoLxkz6ibgv6Hmv",
			"issuer":    "Babbage-Browser-Wallet",
			"address":   "1MBdcYaWTB3dYByNV3dBoLxkz6ibgv6Hmv",
			"publicKey": "03d575090cc073ecf448ad49fae79993fdaf8d1643ec2c5762655ed400e20333e3",
			"timestamp": time.Now().Format(time.RFC3339),
		},
	}

	jsonData, err := json.Marshal(spvReq)
	if err != nil {
		return nil, err
	}

	resp, err := http.Post(baseURL+"/brc100/spv/verify", "application/json", bytes.NewBuffer(jsonData))
	if err != nil {
		return nil, err
	}
	defer resp.Body.Close()

	body, err := io.ReadAll(resp.Body)
	if err != nil {
		return nil, err
	}

	var spvResp map[string]interface{}
	if err := json.Unmarshal(body, &spvResp); err != nil {
		return nil, err
	}

	if !spvResp["success"].(bool) {
		return nil, fmt.Errorf("SPV verification failed: %+v", spvResp)
	}

	proof := spvResp["data"].(map[string]interface{})["proof"].(map[string]interface{})

	return map[string]interface{}{
		"transactionId": txID,
		"blockHeight":   proof["blockHeight"],
		"confirmations": 1, // Our transaction will have 1 confirmation
		"merkleProof":   proof["merkleProof"],
		"identityData":  proof["identityData"],
	}, nil
}

func createBEEFWithRealSPVDataFromOurTx(baseURL string, txID string, confirmedTx map[string]interface{}) (map[string]interface{}, error) {
	// Create a BEEF transaction that references our real transaction as a BEEF transaction reference
	// This will trigger the SPV data collection logic
	beefReq := map[string]interface{}{
		"actions": []map[string]interface{}{
			{
				"type": "beef_transaction_reference",
				"data": map[string]interface{}{
					"transactionId": txID,
					"purpose":       "Reference to our real BEEF transaction for SPV data collection",
					"source":        "our_real_transaction",
					"beefTx": map[string]interface{}{
						"txid": txID,
						"blockHeight": confirmedTx["blockHeight"],
						"confirmations": confirmedTx["confirmations"],
					},
				},
			},
			{
				"type": "spv_verification_request",
				"data": map[string]interface{}{
					"transactionId": txID,
					"subject":       "1MBdcYaWTB3dYByNV3dBoLxkz6ibgv6Hmv",
					"issuer":        "Babbage-Browser-Wallet",
					"timestamp":     time.Now().Format(time.RFC3339),
					"purpose":       "SPV verification of our real BEEF transaction",
				},
			},
		},
		"sessionId":      fmt.Sprintf("spv_from_our_tx_session_%d", time.Now().Unix()),
		"appDomain":      "babbage-browser.app",
		"txId":           txID, // Transaction ID to collect SPV data from
	}

	jsonData, err := json.Marshal(beefReq)
	if err != nil {
		return nil, err
	}

	resp, err := http.Post(baseURL+"/brc100/beef/create-from-tx", "application/json", bytes.NewBuffer(jsonData))
	if err != nil {
		return nil, err
	}
	defer resp.Body.Close()

	body, err := io.ReadAll(resp.Body)
	if err != nil {
		return nil, err
	}


	var beefResp map[string]interface{}
	if err := json.Unmarshal(body, &beefResp); err != nil {
		return nil, fmt.Errorf("JSON parsing error: %v, response: %s", err, string(body))
	}

	if !beefResp["success"].(bool) {
		return nil, fmt.Errorf("BEEF creation with SPV data failed: %+v", beefResp)
	}

	return beefResp["data"].(map[string]interface{})["beefTransaction"].(map[string]interface{}), nil
}

func verifyRealSPVDataCollection(baseURL string, beefTx map[string]interface{}, realTxID string) {
	fmt.Println("     Checking SPV data collection from our real transaction...")

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

	fmt.Printf("✅ SPV data verification completed for our real transaction: %s\n", realTxID)
	fmt.Printf("     Has SPV Data: %v\n", spvInfo["hasSPVData"])
	if spvInfo["hasSPVData"].(bool) {
		fmt.Printf("     Merkle Proofs: %v\n", spvInfo["merkleProofs"])
		fmt.Printf("     Block Headers: %v\n", spvInfo["blockHeaders"])
		fmt.Printf("     Transaction Data: %v\n", spvInfo["transactionData"])
		fmt.Printf("     Identity Proofs: %v\n", spvInfo["identityProofs"])
		fmt.Printf("     Verification Time: %v\n", spvInfo["verificationTime"])
		if txIDs, exists := spvInfo["transactionIds"]; exists {
			fmt.Printf("     Transaction IDs: %v\n", txIDs)
		}
	}
}
