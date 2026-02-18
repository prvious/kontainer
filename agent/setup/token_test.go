package setup

import (
	"strings"
	"testing"
)

func TestGenerateSetupToken(t *testing.T) {
	token := GenerateSetupToken()

	if !strings.HasPrefix(token, "ktsetup_") {
		t.Errorf("token %q should have prefix 'ktsetup_'", token)
	}

	if len(token) != 20 {
		t.Errorf("token length = %d, want 20", len(token))
	}

	// The 12 chars after prefix should be valid hex
	suffix := token[len("ktsetup_"):]
	for _, c := range suffix {
		if !((c >= '0' && c <= '9') || (c >= 'a' && c <= 'f')) {
			t.Errorf("token suffix %q contains non-hex character %q", suffix, string(c))
		}
	}

	// Uniqueness: generate multiple tokens and ensure they differ
	seen := make(map[string]bool)
	for i := 0; i < 100; i++ {
		tok := GenerateSetupToken()
		if seen[tok] {
			t.Errorf("duplicate token generated: %q", tok)
		}
		seen[tok] = true
	}
}

func TestGetSetupToken_EnvOverride(t *testing.T) {
	t.Setenv("KONTAINER_SETUP_TOKEN", "ktsetup_customtoken1")

	token := GetSetupToken()
	if token != "ktsetup_customtoken1" {
		t.Errorf("GetSetupToken() = %q, want %q", token, "ktsetup_customtoken1")
	}
}

func TestGetSetupToken_AutoGenerate(t *testing.T) {
	t.Setenv("KONTAINER_SETUP_TOKEN", "")

	token := GetSetupToken()

	if !strings.HasPrefix(token, "ktsetup_") {
		t.Errorf("auto-generated token %q should have prefix 'ktsetup_'", token)
	}

	if len(token) != 20 {
		t.Errorf("auto-generated token length = %d, want 20", len(token))
	}
}
