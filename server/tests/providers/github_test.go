package providers_test

import (
	"testing"

	"github.com/prvious/kontainer/server/app/services/auth/providers"
)

func TestGitHubProvider_Name(t *testing.T) {
	p := providers.NewGitHubProvider("id", "secret", "http://localhost/callback", "")
	if p.Name() != "github" {
		t.Errorf("expected 'github', got '%s'", p.Name())
	}
}

func TestGitHubProvider_OAuthConfig(t *testing.T) {
	p := providers.NewGitHubProvider("client-id", "client-secret", "http://localhost/callback", "myorg")
	cfg := p.OAuthConfig()
	if cfg.ClientID != "client-id" {
		t.Errorf("expected client-id, got '%s'", cfg.ClientID)
	}
	if cfg.RedirectURL != "http://localhost/callback" {
		t.Errorf("expected redirect URL, got '%s'", cfg.RedirectURL)
	}
}

func TestProviderRegistry(t *testing.T) {
	registry := providers.NewProviderRegistry()

	p := providers.NewGitHubProvider("id", "secret", "http://localhost/callback", "")
	registry.Register(p)

	names := registry.Names()
	if len(names) != 1 || names[0] != "github" {
		t.Errorf("expected ['github'], got %v", names)
	}

	got, err := registry.Get("github")
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if got.Name() != "github" {
		t.Errorf("expected 'github', got '%s'", got.Name())
	}

	_, err = registry.Get("unknown")
	if err == nil {
		t.Error("expected error for unknown provider")
	}
}

func TestAccessDeniedError(t *testing.T) {
	err := &providers.AccessDeniedError{Message: "access denied"}
	if err.Error() != "access denied" {
		t.Errorf("expected 'access denied', got '%s'", err.Error())
	}
}
