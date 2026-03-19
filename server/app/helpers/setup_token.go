package helpers

import (
	"crypto/rand"
	"encoding/hex"

	"github.com/goravel/framework/facades"
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

// GetSetupToken returns the configured setup token if set, otherwise generates a new token.
func GetSetupToken() string {
	if token := facades.Config().GetString("app.setup_token"); token != "" {
		return token
	}
	return GenerateSetupToken()
}

// RandomHex generates a cryptographically random hex string of nBytes length.
func RandomHex(nBytes int) string {
	b := make([]byte, nBytes)
	if _, err := rand.Read(b); err != nil {
		panic("crypto/rand failed: " + err.Error())
	}
	return hex.EncodeToString(b)
}
