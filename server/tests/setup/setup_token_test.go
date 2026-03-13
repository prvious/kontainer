package setup_test

import (
	"strings"
	"testing"

	"github.com/prvious/kontainer/server/app/helpers"
)

func TestGenerateSetupToken(t *testing.T) {
	token := helpers.GenerateSetupToken()

	if !strings.HasPrefix(token, "ktsetup_") {
		t.Errorf("expected prefix 'ktsetup_', got '%s'", token)
	}

	if len(token) != 20 {
		t.Errorf("expected length 20, got %d", len(token))
	}

	token2 := helpers.GenerateSetupToken()
	if token == token2 {
		t.Error("expected unique tokens")
	}
}

func TestGetSetupToken_FromEnv(t *testing.T) {
	t.Setenv("KONTAINER_SETUP_TOKEN", "custom-token")
	token := helpers.GetSetupToken()
	if token != "custom-token" {
		t.Errorf("expected 'custom-token', got '%s'", token)
	}
}

func TestGetSetupToken_Generated(t *testing.T) {
	t.Setenv("KONTAINER_SETUP_TOKEN", "")
	token := helpers.GetSetupToken()
	if !strings.HasPrefix(token, "ktsetup_") {
		t.Errorf("expected generated token with prefix, got '%s'", token)
	}
}
