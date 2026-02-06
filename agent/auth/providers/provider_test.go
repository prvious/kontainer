package providers

import (
	"context"
	"testing"

	"golang.org/x/oauth2"
)

type fakeProvider struct {
	name string
}

func (f *fakeProvider) Name() string { return f.name }
func (f *fakeProvider) OAuthConfig() *oauth2.Config {
	return &oauth2.Config{ClientID: "fake-id"}
}
func (f *fakeProvider) FetchUser(_ context.Context, _ *oauth2.Token) (*ProviderUser, error) {
	return &ProviderUser{ID: "1", Name: "Fake", Provider: f.name}, nil
}

func TestProviderRegistry_RegisterAndGet(t *testing.T) {
	r := NewProviderRegistry()
	r.Register(&fakeProvider{name: "github"})

	p, err := r.Get("github")
	if err != nil {
		t.Fatalf("Get failed: %v", err)
	}
	if p.Name() != "github" {
		t.Errorf("Name = %q, want %q", p.Name(), "github")
	}
}

func TestProviderRegistry_GetUnknown(t *testing.T) {
	r := NewProviderRegistry()

	_, err := r.Get("unknown")
	if err == nil {
		t.Fatal("Get should fail for unknown provider")
	}
}

func TestProviderRegistry_MultipleProviders(t *testing.T) {
	r := NewProviderRegistry()
	r.Register(&fakeProvider{name: "github"})
	r.Register(&fakeProvider{name: "google"})

	gh, err := r.Get("github")
	if err != nil {
		t.Fatalf("Get github failed: %v", err)
	}
	if gh.Name() != "github" {
		t.Errorf("expected github, got %q", gh.Name())
	}

	gl, err := r.Get("google")
	if err != nil {
		t.Fatalf("Get google failed: %v", err)
	}
	if gl.Name() != "google" {
		t.Errorf("expected google, got %q", gl.Name())
	}
}

func TestProviderRegistry_OverwriteProvider(t *testing.T) {
	r := NewProviderRegistry()
	r.Register(&fakeProvider{name: "github"})
	r.Register(&fakeProvider{name: "github"})

	p, err := r.Get("github")
	if err != nil {
		t.Fatalf("Get failed: %v", err)
	}
	if p.Name() != "github" {
		t.Errorf("Name = %q, want %q", p.Name(), "github")
	}
}
