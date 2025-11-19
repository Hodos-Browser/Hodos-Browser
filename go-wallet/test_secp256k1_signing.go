package main

import (
	"crypto/sha256"
	"encoding/hex"
	"fmt"
	"log"

	ec "github.com/bsv-blockchain/go-sdk/primitives/ec"
)

// TestSecp256k1Signing tests if BSV SDK can sign correctly with secp256k1
// This is a SIMPLE test to see if we can fix the authentication issue
func main() {
	fmt.Println("🧪 Testing BSV SDK secp256k1 Signing")
	fmt.Println(string([]byte("=" + "=" + "=" + "=" + "=" + "=" + "=" + "=" + "=" + "=" + "=" + "=" + "=" + "=" + "=" + "=" + "=" + "=")))

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
	fmt.Printf("   ✅ Public key (hex): %s\n", currentAddr.PublicKey)

	// Step 2: Parse the private key using BSV SDK
	fmt.Println("\n📋 Step 2: Parsing private key with BSV SDK...")
	privateKeyHex, err := walletMgr.GetPrivateKeyForAddress(currentAddr.Address)
	if err != nil {
		log.Fatalf("Failed to get private key: %v", err)
	}

	// Try to parse as secp256k1 using BSV SDK
	privateKey, err := ec.PrivateKeyFromHex(privateKeyHex)
	if err != nil {
		fmt.Printf("   ❌ Failed to parse with BSV SDK: %v\n", err)
		fmt.Println("\n🔍 This means our private key format is incompatible")
		fmt.Println("   We may need to convert the key format first")
		return
	}

	fmt.Printf("   ✅ Private key parsed successfully\n")

	// Verify public key matches
	derivedPubKey := privateKey.PubKey()
	derivedPubKeyHex := derivedPubKey.ToDERHex()

	fmt.Printf("   ✅ Derived public key: %s\n", derivedPubKeyHex)

	if derivedPubKeyHex == currentAddr.PublicKey {
		fmt.Println("   ✅ Public keys MATCH! (secp256k1 compatible)")
	} else {
		fmt.Println("   ⚠️  Public keys DON'T match")
		fmt.Printf("       Expected: %s\n", currentAddr.PublicKey)
		fmt.Printf("       Got:      %s\n", derivedPubKeyHex)
	}

	// Step 3: Test signature creation
	fmt.Println("\n📋 Step 3: Creating signature with secp256k1...")

	// Simulate ToolBSV authentication data
	theirNonce := "EWV+4puvzpL9tamT87YDzQAIAAQAAAAEBwAHAAECBgADAAAEBAAAAAAAAAACAAAFAAIECQUFAAcABAAACQAACQEACAAJAwAHBAAACQcAAAc="
	ourNonce := "IoLS2k4dmR80POkXSAC7wY6ua3Tkds7X3+6EN6mWW5I="

	// Concatenate nonces
	dataToSign := theirNonce + ourNonce
	fmt.Printf("   Data to sign: %s\n", dataToSign[:50]+"...")
	fmt.Printf("   Data length: %d bytes\n", len(dataToSign))

	// Hash the data
	hash := sha256.Sum256([]byte(dataToSign))
	fmt.Printf("   SHA256 hash: %s\n", hex.EncodeToString(hash[:]))

	// Sign with BSV SDK
	signature, err := privateKey.Sign(hash[:])
	if err != nil {
		fmt.Printf("   ❌ Signing FAILED: %v\n", err)
		return
	}

	fmt.Printf("   ✅ Signature created successfully!\n")

	// Get signature in DER format
	sigHex, err := signature.ToDER()
	if err != nil {
		fmt.Printf("   ❌ Failed to convert signature to DER: %v\n", err)
		return
	}

	fmt.Printf("   ✅ Signature (DER hex): %s\n", sigHex)
	fmt.Printf("   ✅ Signature length: %d characters\n", len(sigHex))

	// Step 4: Verify the signature
	fmt.Println("\n📋 Step 4: Verifying signature...")

	publicKey := privateKey.PubKey()
	valid := publicKey.Verify(hash[:], signature)

	if valid {
		fmt.Println("   ✅ Signature verification PASSED!")
		fmt.Println("   ✅ This signature is valid secp256k1!")
	} else {
		fmt.Println("   ❌ Signature verification FAILED")
	}

	// Summary
	fmt.Println("\n" + string([]byte("=" + "=" + "=" + "=" + "=" + "=" + "=" + "=" + "=" + "=" + "=" + "=" + "=" + "=" + "=" + "=" + "=" + "=")))
	fmt.Println("🎯 CONCLUSION:")
	fmt.Println()

	if valid {
		fmt.Println("✅ BSV SDK can sign with secp256k1 correctly!")
		fmt.Println("✅ This should fix ToolBSV authentication!")
		fmt.Println()
		fmt.Println("📋 Next Steps:")
		fmt.Println("   1. Update signWithDerivedKey to use ec.PrivateKeyFromHex()")
		fmt.Println("   2. Use privateKey.Sign() instead of ecdsa.Sign()")
		fmt.Println("   3. Test with ToolBSV")
	} else {
		fmt.Println("❌ Signature verification failed")
		fmt.Println("   Need to investigate SDK usage further")
	}
}
