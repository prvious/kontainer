package helpers

import (
	"crypto/rand"
	"encoding/hex"
	"os"
)

const tokenPrefix = "ktsetup_"

// GenerateSetupToken generates a setup token: "ktsetup_" + 12 random hex characters.
func GenerateSetupToken() string {
	b := make([]byte, 6) // 6 bytes = 12 hex chars
	if _, err := rand.Read(b); err != nil {
		panic("crypto/rand failed: " + err.Error())
	}
	return tokenPrefix + hex.EncodeToString(b)
}

// GetSetupToken returns KONTAINER_SETUP_TOKEN env var if set, otherwise generates a new token.
func GetSetupToken() string {
	if token := os.Getenv("KONTAINER_SETUP_TOKEN"); token != "" {
		return token
	}
	return GenerateSetupToken()
}
