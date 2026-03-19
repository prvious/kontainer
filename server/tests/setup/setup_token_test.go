package setup_test

import (
	"strings"
	"testing"

	"github.com/goravel/framework/facades"
	"github.com/prvious/kontainer/server/app/helpers"
	_ "github.com/prvious/kontainer/server/tests"
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

func TestGetSetupToken_FromConfig(t *testing.T) {
	facades.Config().Add("app.setup_token", "custom-token")
	defer facades.Config().Add("app.setup_token", "")

	token := helpers.GetSetupToken()
	if token != "custom-token" {
		t.Errorf("expected 'custom-token', got '%s'", token)
	}
}

func TestGetSetupToken_Generated(t *testing.T) {
	facades.Config().Add("app.setup_token", "")

	token := helpers.GetSetupToken()
	if !strings.HasPrefix(token, "ktsetup_") {
		t.Errorf("expected generated token with prefix, got '%s'", token)
	}
}
