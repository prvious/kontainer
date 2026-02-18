package setup

import (
	"crypto/rand"
	"encoding/hex"
	"os"
)

const tokenPrefix = "ktsetup_"

// GenerateSetupToken generates a setup token with the format "ktsetup_" + 12 random hex characters.
// The total token length is 20 characters. Uses crypto/rand for secure randomness.
func GenerateSetupToken() string {
	b := make([]byte, 6) // 6 bytes = 12 hex chars
	if _, err := rand.Read(b); err != nil {
		panic("crypto/rand failed: " + err.Error())
	}
	return tokenPrefix + hex.EncodeToString(b)
}

// GetSetupToken returns the KONTAINER_SETUP_TOKEN environment variable if set and non-empty,
// otherwise generates a new token using GenerateSetupToken.
func GetSetupToken() string {
	if token := os.Getenv("KONTAINER_SETUP_TOKEN"); token != "" {
		return token
	}
	return GenerateSetupToken()
}
