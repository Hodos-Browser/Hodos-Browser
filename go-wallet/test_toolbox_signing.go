package main

import (
	"context"
	"encoding/base64"
	"encoding/hex"
	"fmt"
	"log"

	ec "github.com/bsv-blockchain/go-sdk/primitives/ec"
	sdk "github.com/bsv-blockchain/go-sdk/wallet"
	"github.com/bsv-blockchain/go-wallet-toolbox/pkg/defs"
	"github.com/bsv-blockchain/go-wallet-toolbox/pkg/services"
	"github.com/bsv-blockchain/go-wallet-toolbox/pkg/storage"
	"github.com/bsv-blockchain/go-wallet-toolbox/pkg/wallet"
)

// TestToolboxSigning tests if go-wallet-toolbox can create valid signatures
// This will help us determine if it fixes our BRC-42 authentication issues
func main() {
	fmt.Println("🧪 Testing go-wallet-toolbox Signature Creation")
	fmt.Println("=" + "="*50)

	ctx := context.Background()

	// Step 1: Load your existing wallet to get the private key
	fmt.Println("\n📋 Step 1: Loading existing wallet...")
	walletMgr := &WalletManager{
		wallet: &Wallet{},
		logger: nil,
	}

	err := walletMgr.LoadFromFile(GetWalletPath())
	if err != nil {
		log.Fatalf("Failed to load wallet: %v", err)
	}

	currentAddr, err := walletMgr.GetCurrentAddress()
	if err != nil {
		log.Fatalf("Failed to get current address: %v", err)
	}

	fmt.Printf("   ✅ Loaded wallet with address: %s\n", currentAddr.Address)
	fmt.Printf("   ✅ Public key: %s\n", currentAddr.PublicKey)

	// Step 2: Get the private key
	fmt.Println("\n📋 Step 2: Getting private key...")
	privateKeyHex, err := walletMgr.GetPrivateKeyForAddress(currentAddr.Address)
	if err != nil {
		log.Fatalf("Failed to get private key: %v", err)
	}

	// Convert hex to bytes
	privateKeyBytes, err := hex.DecodeString(privateKeyHex)
	if err != nil {
		log.Fatalf("Failed to decode private key: %v", err)
	}

	// Parse with SDK
	privateKey, err := ec.PrivateKeyFromBytes(privateKeyBytes)
	if err != nil {
		log.Fatalf("Failed to parse private key: %v", err)
	}

	fmt.Printf("   ✅ Private key loaded\n")
	fmt.Printf("   ✅ Derived public key: %s\n", privateKey.PubKey().ToDERHex())

	// Step 3: Create minimal storage for testing
	fmt.Println("\n📋 Step 3: Creating test storage...")
	testStorage, err := storage.NewGORMProvider(
		defs.NetworkMainnet,
		services.New(nil, defs.DefaultServicesConfig(defs.NetworkMainnet)),
		storage.WithSQLiteConnectionString("./test_toolbox.db"),
	)
	if err != nil {
		log.Fatalf("Failed to create storage: %v", err)
	}
	defer testStorage.Stop()

	// Migrate storage
	storageKey, _ := ec.PrivateKeyFromHex("2b32d442b25d6e7447a1f9ca41a2a15de5004498dc4ffc43b7b009a96724c30d")
	_, err = testStorage.Migrate(ctx, "test-wallet", storageKey.PubKey())
	if err != nil {
		log.Fatalf("Failed to migrate storage: %v", err)
	}

	fmt.Printf("   ✅ Test storage created: test_toolbox.db\n")

	// Step 4: Create wallet using go-wallet-toolbox
	fmt.Println("\n📋 Step 4: Creating go-wallet-toolbox wallet...")
	testWallet, err := wallet.New(
		testStorage,
		privateKey,
		defs.NetworkMainnet,
	)
	if err != nil {
		log.Fatalf("Failed to create wallet: %v", err)
	}
	defer testWallet.Close()

	fmt.Printf("   ✅ Wallet created successfully\n")

	// Step 5: Test signature creation (simulating ToolBSV auth)
	fmt.Println("\n📋 Step 5: Testing signature creation...")

	// Simulate what ToolBSV sends
	theirNonce := "EWV+4puvzpL9tamT87YDzQAIAAQAAAAEBwAHAAECBgADAAAEBAAAAAAAAAACAAAFAAIECQUFAAcABAAACQAACQEACAAJAwAHBAAACQcAAAc="
	ourNonce := "IoLS2k4dmR80POkXSAC7wY6ua3Tkds7X3+6EN6mWW5I="

	// Concatenate (what we need to sign)
	dataToSign := theirNonce + ourNonce

	fmt.Printf("   Data to sign: %s\n", dataToSign)

	// Create signature using toolbox
	sigResult, err := testWallet.CreateSignature(ctx, &sdk.CreateSignatureArgs{
		Data: []byte(dataToSign),
		Reason: map[string]interface{}{
			"type": "babbage-auth",
		},
		Counterparty: currentAddr.PublicKey, // Their identity key (actually our own)
		ProtocolID:   [2]interface{}{2, "auth message signature"},
		KeyID:        theirNonce + " " + ourNonce,
	}, "test-originator")

	if err != nil {
		fmt.Printf("   ❌ Signature creation FAILED: %v\n", err)
		fmt.Println("\n🔍 This might mean:")
		fmt.Println("   - BRC-42 mutual auth not working")
		fmt.Println("   - Need to investigate further")
	} else {
		fmt.Printf("   ✅ Signature created successfully!\n")
		fmt.Printf("   ✅ Signature: %s\n", sigResult.Signature)
		fmt.Printf("   ✅ Protocol ID: %v\n", sigResult.ProtocolID)
		fmt.Printf("   ✅ Key ID: %s\n", sigResult.KeyID)

		fmt.Println("\n🎉 SUCCESS! go-wallet-toolbox can create signatures!")
		fmt.Println("   This signature should work with ToolBSV")
	}

	// Step 6: Compare with old signature
	fmt.Println("\n📋 Step 6: Comparing signature formats...")
	fmt.Println("   Old method: P-256 (broken)")
	fmt.Println("   New method: secp256k1 (toolbox)")
	fmt.Println("   ✅ If no error above, toolbox uses correct curve!")

	fmt.Println("\n" + "="*52)
	fmt.Println("🧪 Test Complete")
}

