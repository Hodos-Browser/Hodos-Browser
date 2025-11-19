package spv

import (
	"fmt"
	"log"
	"testing"
)

func main() {
	fmt.Println("=== Testing Merkle Proof Implementation ===")

	// Create blockchain client
	client := NewBlockchainAPIClient()

	// Test with a known BSV transaction
	// Using a real transaction ID from our previous testing
	txID := "859656" // This was the transaction we tested earlier
	blockHeight := int64(859656) // This was the block height

	fmt.Printf("Testing Merkle proof for transaction: %s\n", txID)
	fmt.Printf("Block height: %d\n", blockHeight)

	// Try to fetch Merkle proof
	proof, err := client.GetMerkleProofFromBlockchain(txID, blockHeight)
	if err != nil {
		fmt.Printf("❌ Error fetching Merkle proof: %v\n", err)
		return
	}

	fmt.Printf("✅ Merkle proof fetched successfully!\n")
	fmt.Printf("   Transaction ID: %s\n", proof.TxID)
	fmt.Printf("   Block Height: %d\n", proof.BlockHeight)
	fmt.Printf("   Merkle Root: %s\n", proof.MerkleRoot)
	fmt.Printf("   Merkle Path Length: %d\n", len(proof.MerklePath))
	fmt.Printf("   Merkle Path: %v\n", proof.MerklePath)

	// Test SPV verification
	fmt.Println("\n=== Testing SPV Verification ===")

	// Create SPV verifier
	verifier := NewSPVVerifier()

	// Create identity proof
	identityData := map[string]interface{}{
		"name": "Test User",
		"email": "test@example.com",
		"address": "1MBdcYaWTB3dYByNV3dBoLxkz6ibgv6Hmv",
	}

	identityProof, err := verifier.CreateIdentityProof(txID, identityData)
	if err != nil {
		fmt.Printf("❌ Error creating identity proof: %v\n", err)
		return
	}

	fmt.Printf("✅ Identity proof created successfully!\n")
	fmt.Printf("   Transaction ID: %s\n", identityProof.TxID)
	fmt.Printf("   Block Height: %d\n", identityProof.BlockHeight)
	fmt.Printf("   Merkle Path Length: %d\n", len(identityProof.MerklePath.Path))

	// Verify the proof
	valid, err := verifier.VerifyIdentityProof(identityProof)
	if err != nil {
		fmt.Printf("❌ Error verifying identity proof: %v\n", err)
		return
	}

	if valid {
		fmt.Printf("✅ Identity proof verification successful!\n")
	} else {
		fmt.Printf("❌ Identity proof verification failed!\n")
	}

	fmt.Println("\n=== Test Complete ===")
}
